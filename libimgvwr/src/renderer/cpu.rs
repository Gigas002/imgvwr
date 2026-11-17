// CPU rendering pipeline, used when no GPU feature is enabled.
//
// The image is scaled by `viewport.scale`, rotated, and blitted centred in
// the window. Only the part that lands inside the window is ever computed:
//
// - Zoomed out (scale < 1) the whole scaled image is never larger than the
//   source, so it is resized once with `imageops::resize` and cached; panning
//   then only copies from the cache.
// - Zoomed in, the scaled image can be far larger than the window (a 24 MP
//   photo at 5× is 3 GB), so only the visible region is resampled, with the
//   same arithmetic as `imageops::resize`. The result matches cropping a full
//   resize, at a cost bounded by the window size.

use std::cell::RefCell;

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage, imageops};

use super::FilterMethod;
use crate::viewport::ViewportState;

/// Pixel rectangle `[x0, x1) × [y0, y1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Region {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Region {
    fn width(&self) -> u32 {
        self.x1 - self.x0
    }

    fn height(&self) -> u32 {
        self.y1 - self.y0
    }
}

/// Size of `src` scaled by `scale`, as `imageops::resize` would be asked for.
pub(crate) fn scaled_size(src_w: u32, src_h: u32, scale: f32) -> (u32, u32) {
    let side = |v: u32| ((v as f32) * scale).max(1.0) as u32;
    (side(src_w), side(src_h))
}

/// The region of the unrotated `sw × sh` scaled image that ends up at
/// `visible`, a region of the image after clockwise rotation by `rotation`.
pub(crate) fn unrotated_region(rotation: u16, sw: u32, sh: u32, visible: Region) -> Region {
    let Region { x0, y0, x1, y1 } = visible;
    match rotation {
        // rotate90:  out(x, y) = in(y, sh - 1 - x)
        90 => Region {
            x0: y0,
            y0: sh - x1,
            x1: y1,
            y1: sh - x0,
        },
        // rotate180: out(x, y) = in(sw - 1 - x, sh - 1 - y)
        180 => Region {
            x0: sw - x1,
            y0: sh - y1,
            x1: sw - x0,
            y1: sh - y0,
        },
        // rotate270: out(x, y) = in(sw - 1 - y, x)
        270 => Region {
            x0: sw - y1,
            y0: x0,
            x1: sw - y0,
            y1: x1,
        },
        _ => visible,
    }
}

/// Kernel and support of `filter`, exactly as `imageops::resize` uses them.
fn kernel(filter: FilterMethod) -> (fn(f32) -> f32, f32) {
    fn sinc(t: f32) -> f32 {
        let a = t * std::f32::consts::PI;
        if t == 0.0 { 1.0 } else { a.sin() / a }
    }
    fn nearest(_: f32) -> f32 {
        1.0
    }
    fn triangle(x: f32) -> f32 {
        if x.abs() < 1.0 { 1.0 - x.abs() } else { 0.0 }
    }
    fn catmull_rom(x: f32) -> f32 {
        let (b, c) = (0.0f32, 0.5f32);
        let a = x.abs();
        let k = if a < 1.0 {
            (12.0 - 9.0 * b - 6.0 * c) * a.powi(3)
                + (-18.0 + 12.0 * b + 6.0 * c) * a.powi(2)
                + (6.0 - 2.0 * b)
        } else if a < 2.0 {
            (-b - 6.0 * c) * a.powi(3)
                + (6.0 * b + 30.0 * c) * a.powi(2)
                + (-12.0 * b - 48.0 * c) * a
                + (8.0 * b + 24.0 * c)
        } else {
            0.0
        };
        k / 6.0
    }
    fn gaussian(x: f32) -> f32 {
        let r = 0.5f32;
        ((2.0 * std::f32::consts::PI).sqrt() * r).recip() * (-x.powi(2) / (2.0 * r.powi(2))).exp()
    }
    fn lanczos3(x: f32) -> f32 {
        if x.abs() < 3.0 {
            sinc(x) * sinc(x / 3.0)
        } else {
            0.0
        }
    }
    match filter {
        FilterMethod::Nearest => (nearest, 0.0),
        FilterMethod::Triangle => (triangle, 1.0),
        FilterMethod::CatmullRom => (catmull_rom, 2.0),
        FilterMethod::Gaussian => (gaussian, 3.0),
        FilterMethod::Lanczos3 => (lanczos3, 3.0),
    }
}

/// Source taps of one output pixel: the first source index and the weights.
struct Taps {
    first: u32,
    weights: Vec<f32>,
}

/// Taps for output pixels `out` of a `src_len → dst_len` resample.
fn taps(src_len: u32, dst_len: u32, out: std::ops::Range<u32>, filter: FilterMethod) -> Vec<Taps> {
    let (kernel, support) = kernel(filter);
    let ratio = src_len as f32 / dst_len as f32;
    let sratio = if ratio < 1.0 { 1.0 } else { ratio };
    let src_support = support * sratio;
    out.map(|o| {
        let input = (o as f32 + 0.5) * ratio;
        let left = ((input - src_support).floor() as i64).clamp(0, src_len as i64 - 1) as u32;
        let right =
            ((input + src_support).ceil() as i64).clamp(left as i64 + 1, src_len as i64) as u32;
        let input = input - 0.5;
        let mut weights: Vec<f32> = (left..right)
            .map(|i| kernel((i as f32 - input) / sratio))
            .collect();
        let sum: f32 = weights.iter().sum();
        for w in &mut weights {
            *w /= sum;
        }
        Taps {
            first: left,
            weights,
        }
    })
    .collect()
}

/// Pixels `region` of `src` resized to `sw × sh`, computed like
/// `imageops::resize(src, sw, sh, filter)` followed by a crop, but touching
/// only the source pixels the region needs.
pub(crate) fn resample_region(
    src: &DynamicImage,
    sw: u32,
    sh: u32,
    region: Region,
    filter: FilterMethod,
) -> RgbaImage {
    let (w, h) = src.dimensions();
    if (sw, sh) == (w, h) {
        // imageops::resize copies when the size does not change.
        return src
            .view(region.x0, region.y0, region.width(), region.height())
            .to_image();
    }

    let rows = taps(h, sh, region.y0..region.y1, filter);
    let cols = taps(w, sw, region.x0..region.x1, filter);
    // Source columns any output column reads.
    let cx0 = cols.first().map_or(0, |t| t.first);
    let cx1 = cols
        .iter()
        .map(|t| t.first + t.weights.len() as u32)
        .max()
        .unwrap_or(cx0);
    let ncols = (cx1 - cx0) as usize;

    // Vertical pass into f32, like imageops' intermediate Rgba32FImage.
    let mut tmp = vec![[0f32; 4]; rows.len() * ncols];
    for (r, t) in rows.iter().enumerate() {
        let line = &mut tmp[r * ncols..(r + 1) * ncols];
        for (c, acc) in line.iter_mut().enumerate() {
            let x = cx0 + c as u32;
            for (i, wt) in t.weights.iter().enumerate() {
                let Rgba(p) = src.get_pixel(x, t.first + i as u32);
                for k in 0..4 {
                    acc[k] += p[k] as f32 * wt;
                }
            }
        }
    }

    // Horizontal pass, rounded and clamped like imageops.
    let mut out = RgbaImage::new(region.width(), region.height());
    for (r, line) in tmp.chunks_exact(ncols).enumerate() {
        for (c, t) in cols.iter().enumerate() {
            let mut acc = [0f32; 4];
            let start = (t.first - cx0) as usize;
            for (i, wt) in t.weights.iter().enumerate() {
                let p = line[start + i];
                for k in 0..4 {
                    acc[k] += p[k] * wt;
                }
            }
            let px = acc.map(|v| v.clamp(0.0, 255.0).round() as u8);
            out.put_pixel(c as u32, r as u32, Rgba(px));
        }
    }
    out
}

/// Identity of a cached scaled image. Images are recognised by their pixel
/// buffer's address and length, which stay put while the image is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ScaledKey {
    ptr: usize,
    len: usize,
    size: (u32, u32),
    filter: FilterMethod,
}

thread_local! {
    /// The last zoomed-out image, reused while only the pan offset changes.
    static SCALED: RefCell<Option<(ScaledKey, RgbaImage)>> = const { RefCell::new(None) };
}

/// `region` of `src` scaled to `sw × sh`, from the cache when possible.
fn scaled_region(
    src: &DynamicImage,
    sw: u32,
    sh: u32,
    region: Region,
    filter: FilterMethod,
    zoomed_out: bool,
) -> RgbaImage {
    if !zoomed_out {
        return resample_region(src, sw, sh, region, filter);
    }
    let bytes = src.as_bytes();
    let key = ScaledKey {
        ptr: bytes.as_ptr() as usize,
        len: bytes.len(),
        size: (sw, sh),
        filter,
    };
    SCALED.with_borrow_mut(|cache| {
        let fresh = matches!(cache, Some((k, _)) if *k == key);
        if !fresh {
            // Drop the old image before allocating the new one.
            *cache = None;
            *cache = Some((key, imageops::resize(src, sw, sh, filter.into())));
        }
        let (_, scaled) = cache.as_ref().expect("cache was just filled");
        imageops::crop_imm(
            scaled,
            region.x0,
            region.y0,
            region.width(),
            region.height(),
        )
        .to_image()
    })
}

/// Render `src` into a `dst_w × dst_h` ARGB8888 buffer; see
/// [`super::render`].
pub(crate) fn render(
    src: &DynamicImage,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
) -> Vec<u8> {
    let mut buf = vec![0u8; dst_w as usize * dst_h as usize * 4];
    if src.width() == 0 || src.height() == 0 {
        return buf;
    }

    let (sw, sh) = scaled_size(src.width(), src.height(), viewport.scale);
    let (rot_w, rot_h) = match viewport.rotation {
        90 | 270 => (sh, sw),
        _ => (sw, sh),
    };
    let blit_x = (dst_w as i64 / 2) - (rot_w as i64 / 2) + viewport.offset.0 as i64;
    let blit_y = (dst_h as i64 / 2) - (rot_h as i64 / 2) + viewport.offset.1 as i64;

    // Visible part of the rotated image, in its own coordinates.
    let clip = |blit: i64, len: u32, dst: u32| {
        let start = (-blit).clamp(0, len as i64) as u32;
        let end = (dst as i64 - blit).clamp(0, len as i64) as u32;
        (start, end)
    };
    let (rx0, rx1) = clip(blit_x, rot_w, dst_w);
    let (ry0, ry1) = clip(blit_y, rot_h, dst_h);
    if rx1 <= rx0 || ry1 <= ry0 {
        return buf;
    }
    let visible = Region {
        x0: rx0,
        y0: ry0,
        x1: rx1,
        y1: ry1,
    };

    let region = unrotated_region(viewport.rotation, sw, sh, visible);
    let pixels = scaled_region(src, sw, sh, region, filter, viewport.scale < 1.0);
    let pixels = match viewport.rotation {
        90 => imageops::rotate90(&pixels),
        180 => imageops::rotate180(&pixels),
        270 => imageops::rotate270(&pixels),
        _ => pixels,
    };
    debug_assert_eq!(pixels.dimensions(), (rx1 - rx0, ry1 - ry0));

    let left = (blit_x + rx0 as i64) as usize;
    let top = (blit_y + ry0 as i64) as usize;
    for (row, line) in pixels.rows().enumerate() {
        let start = ((top + row) * dst_w as usize + left) * 4;
        let dst = &mut buf[start..start + line.len() * 4];
        for (out, Rgba([r, g, b, a])) in dst.as_chunks_mut::<4>().0.iter_mut().zip(line) {
            // wl_shm ARGB8888 in little-endian memory: [B, G, R, A]
            out.copy_from_slice(&[*b, *g, *r, *a]);
        }
    }
    buf
}
