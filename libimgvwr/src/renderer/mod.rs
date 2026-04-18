//! Rendering pipeline: transforms a [`DynamicImage`] into a
//! Wayland-compatible ARGB8888 pixel buffer.
//!
//! Without a GPU feature the pipeline is fully CPU-based:
//! 1. Scale the source image to `(scaled_w, scaled_h)` using `imageops`.
//! 2. Apply rotation if `viewport.rotation != 0`.
//! 3. Blit the result centred in a `dst_w × dst_h` buffer, offset by
//!    `viewport.offset`. Pixels outside the destination rectangle stay black.
//! 4. Convert each pixel from RGBA to little-endian ARGB8888
//!    (`wl_shm_format::ARGB8888`).
//!
//! With a GPU feature (`gpu-vulkan` or `gpu-gles`) the pipeline is:
//! 1. Upload the source image to a GPU texture.
//! 2. GPU resize + rotate → `Rgba8Unorm` texture.
//! 3. Readback to CPU as ARGB8888 bytes.
//! 4. CPU blit-center into the `dst_w × dst_h` output buffer.

#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
pub mod gpu;
#[cfg(test)]
mod tests;

use image::{DynamicImage, imageops};
#[cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]
use image::{ImageBuffer, Rgba};

use crate::viewport::ViewportState;

/// Scaling filter applied during image resize.
///
/// Maps 1-to-1 onto [`image::imageops::FilterType`]; exposed here so callers
/// don't need to depend on `image` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMethod {
    /// Nearest-neighbour — fastest, pixelated at high zoom.
    Nearest,
    /// Bilinear interpolation.
    Triangle,
    /// Catmull-Rom cubic spline.
    CatmullRom,
    /// Gaussian blur kernel.
    Gaussian,
    /// Lanczos with window 3 — best quality, slowest.
    #[default]
    Lanczos3,
}

impl From<FilterMethod> for imageops::FilterType {
    fn from(f: FilterMethod) -> imageops::FilterType {
        match f {
            FilterMethod::Nearest => imageops::FilterType::Nearest,
            FilterMethod::Triangle => imageops::FilterType::Triangle,
            FilterMethod::CatmullRom => imageops::FilterType::CatmullRom,
            FilterMethod::Gaussian => imageops::FilterType::Gaussian,
            FilterMethod::Lanczos3 => imageops::FilterType::Lanczos3,
        }
    }
}

/// Render `src` into a `dst_w × dst_h` ARGB8888 pixel buffer.
///
/// The image is scaled according to `viewport.scale`, rotated by
/// `viewport.rotation`, then blitted centred in the destination with
/// `viewport.offset` applied. Regions not covered by the image are filled
/// with opaque black (`0xFF000000`).
///
/// The returned `Vec<u8>` is suitable for writing directly into a Wayland SHM
/// pool (`wl_shm_format::ARGB8888`, 4 bytes per pixel, row-major).
///
/// When compiled with `gpu-vulkan` or `gpu-gles`, the resize and rotation
/// steps are executed on the GPU; the CPU is only used for the final
/// blit-center copy.
pub fn render(
    src: &DynamicImage,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
    #[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))] gpu: &gpu::GpuContext,
) -> Vec<u8> {
    let scaled_w = ((src.width() as f32) * viewport.scale).max(1.0) as u32;
    let scaled_h = ((src.height() as f32) * viewport.scale).max(1.0) as u32;

    #[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
    {
        gpu_render(src, viewport, dst_w, dst_h, scaled_w, scaled_h, filter, gpu)
    }

    #[cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]
    {
        let scaled: ImageBuffer<Rgba<u8>, Vec<u8>> =
            imageops::resize(src, scaled_w, scaled_h, filter.into());

        let rotated: ImageBuffer<Rgba<u8>, Vec<u8>> = match viewport.rotation {
            90 => imageops::rotate90(&scaled),
            180 => imageops::rotate180(&scaled),
            270 => imageops::rotate270(&scaled),
            _ => scaled,
        };

        let rot_w = rotated.width();
        let rot_h = rotated.height();
        let blit_x = (dst_w as i32 / 2) - (rot_w as i32 / 2) + viewport.offset.0 as i32;
        let blit_y = (dst_h as i32 / 2) - (rot_h as i32 / 2) + viewport.offset.1 as i32;
        let mut buf = vec![0u8; (dst_w * dst_h * 4) as usize];

        for sy in 0..rot_h {
            let dy = blit_y + sy as i32;
            if dy < 0 || dy >= dst_h as i32 {
                continue;
            }
            for sx in 0..rot_w {
                let dx = blit_x + sx as i32;
                if dx < 0 || dx >= dst_w as i32 {
                    continue;
                }
                let Rgba([r, g, b, a]) = *rotated.get_pixel(sx, sy);
                let dst_idx = (dy as u32 * dst_w + dx as u32) as usize * 4;
                // wl_shm ARGB8888 in little-endian memory: [B, G, R, A]
                buf[dst_idx] = b;
                buf[dst_idx + 1] = g;
                buf[dst_idx + 2] = r;
                buf[dst_idx + 3] = a;
            }
        }

        buf
    }
}

/// GPU render path with viewport culling.
///
/// When the scaled image fits inside the window, the full image is uploaded
/// and the result is blit-centred. When the scaled image is larger than the
/// window, only the visible crop of the source image is uploaded and resized
/// to the visible pixel count — this eliminates GPU texture-size limits for
/// zoom and avoids processing off-screen pixels.
#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
#[allow(clippy::too_many_arguments)]
fn gpu_render(
    src: &DynamicImage,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
    scaled_w: u32,
    scaled_h: u32,
    filter: FilterMethod,
    gpu: &gpu::GpuContext,
) -> Vec<u8> {
    match gpu_render_inner(src, viewport, dst_w, dst_h, scaled_w, scaled_h, filter, gpu) {
        None => vec![0u8; (dst_w * dst_h * 4) as usize],
        Some((out, win_x, win_y)) => {
            let (target_w, target_h) = (out.width(), out.height());
            let pixels = gpu::readback(gpu, &out, target_w, target_h);

            if win_x == 0 && win_y == 0 && target_w == dst_w && target_h == dst_h {
                return pixels;
            }

            let mut buf = vec![0u8; (dst_w * dst_h * 4) as usize];
            let copy_w = target_w.min(dst_w - win_x) as usize;
            let copy_h = target_h.min(dst_h - win_y);
            for sy in 0..copy_h {
                let src_off = (sy * target_w) as usize * 4;
                let dst_off = ((win_y + sy) * dst_w + win_x) as usize * 4;
                buf[dst_off..dst_off + copy_w * 4]
                    .copy_from_slice(&pixels[src_off..src_off + copy_w * 4]);
            }
            buf
        }
    }
}

/// Compute and upload the visible region of `src` for the current viewport.
///
/// Returns `Some((output_texture, win_x, win_y))` where the texture holds the
/// rendered visible pixels and `(win_x, win_y)` is its top-left position in
/// the destination window. Returns `None` when the image is completely
/// off-screen.
#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn gpu_render_inner(
    src: &DynamicImage,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
    scaled_w: u32,
    scaled_h: u32,
    filter: FilterMethod,
    gpu: &gpu::GpuContext,
) -> Option<(wgpu::Texture, u32, u32)> {
    let scale = viewport.scale;
    let rotation = viewport.rotation;

    let (rot_w, rot_h) = match rotation {
        90 | 270 => (scaled_h, scaled_w),
        _ => (scaled_w, scaled_h),
    };

    let blit_x = dst_w as i32 / 2 - rot_w as i32 / 2 + viewport.offset.0 as i32;
    let blit_y = dst_h as i32 / 2 - rot_h as i32 / 2 + viewport.offset.1 as i32;

    let vis_sx0 = (-blit_x).max(0) as u32;
    let vis_sy0 = (-blit_y).max(0) as u32;
    let vis_sx1 = rot_w.min((dst_w as i32 - blit_x).max(0) as u32);
    let vis_sy1 = rot_h.min((dst_h as i32 - blit_y).max(0) as u32);

    if vis_sx1 <= vis_sx0 || vis_sy1 <= vis_sy0 {
        return None;
    }

    let vis_w = vis_sx1 - vis_sx0;
    let vis_h = vis_sy1 - vis_sy0;
    let win_x = blit_x.max(0) as u32;
    let win_y = blit_y.max(0) as u32;

    let (crop_x, crop_y, crop_w, crop_h, resize_w, resize_h) = visible_source_crop(
        rotation,
        vis_sx0,
        vis_sy0,
        vis_sx1,
        vis_sy1,
        vis_w,
        vis_h,
        scale,
        src.width(),
        src.height(),
        scaled_w,
        scaled_h,
    );

    let crop = src.crop_imm(crop_x, crop_y, crop_w, crop_h);
    let tex = gpu::upload_texture(&gpu.device, &gpu.queue, &crop);
    let out = gpu::resize_blit(gpu, &tex, resize_w, resize_h, filter, rotation);
    Some((out, win_x, win_y))
}

/// Compute the source image crop and GPU resize dimensions needed to produce
/// exactly the visible `vis_w × vis_h` window region after `rotation`.
///
/// The rotation shader (rotate.wgsl) maps each output pixel back to a source
/// pixel. Inverting that mapping for the visible rectangle gives a rectangular
/// source crop that, when resized to `(resize_w, resize_h)` and rotated,
/// produces a `vis_w × vis_h` output — matching the window region exactly.
///
/// Returns `(crop_x, crop_y, crop_w, crop_h, resize_w, resize_h)`.
#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
#[allow(clippy::too_many_arguments)]
fn visible_source_crop(
    rotation: u16,
    vis_sx0: u32,
    vis_sy0: u32,
    vis_sx1: u32,
    vis_sy1: u32,
    vis_w: u32,
    vis_h: u32,
    scale: f32,
    src_w: u32,
    src_h: u32,
    scaled_w: u32,
    scaled_h: u32,
) -> (u32, u32, u32, u32, u32, u32) {
    // Helpers: map a range in scaled (resized-source) coordinates back to image coords.
    let sx = |v: u32| (v as f32 / scale) as u32;
    let sx_ceil = |v: u32| ((v as f32 / scale).ceil() as u32).min(src_w);
    let sy = |v: u32| (v as f32 / scale) as u32;
    let sy_ceil = |v: u32| ((v as f32 / scale).ceil() as u32).min(src_h);
    // Map from the flipped side: (scaled_dim - v) / scale, clamped.
    let fx = |v: u32| ((scaled_w as f32 - v as f32) / scale).max(0.0) as u32;
    let fx_ceil = |v: u32| {
        ((scaled_w as f32 - v as f32) / scale)
            .max(0.0)
            .ceil()
            .min(src_w as f32) as u32
    };
    let fy = |v: u32| ((scaled_h as f32 - v as f32) / scale).max(0.0) as u32;
    let fy_ceil = |v: u32| {
        ((scaled_h as f32 - v as f32) / scale)
            .max(0.0)
            .ceil()
            .min(src_h as f32) as u32
    };

    match rotation % 360 {
        // Shader: output(rx,ry) ← src(rx,ry)
        // Visible src x: [vis_sx0, vis_sx1), y: [vis_sy0, vis_sy1)
        0 => {
            let (x0, x1) = (sx(vis_sx0), sx_ceil(vis_sx1));
            let (y0, y1) = (sy(vis_sy0), sy_ceil(vis_sy1));
            (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1), vis_w, vis_h)
        }
        // Shader: output(rx,ry) ← src(ry, scaled_h-1-rx); output size = scaled_h × scaled_w
        // Visible src x: [vis_sy0, vis_sy1), y: [scaled_h-vis_sx1, scaled_h-vis_sx0)
        // Resize to (vis_h, vis_w) then rotate 90 → vis_w × vis_h
        90 => {
            let (x0, x1) = (sx(vis_sy0), sx_ceil(vis_sy1));
            let (y0, y1) = (fy(vis_sx1), fy_ceil(vis_sx0));
            (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1), vis_h, vis_w)
        }
        // Shader: output(rx,ry) ← src(scaled_w-1-rx, scaled_h-1-ry)
        // Visible src x: [scaled_w-vis_sx1, scaled_w-vis_sx0), y: [scaled_h-vis_sy1, scaled_h-vis_sy0)
        180 => {
            let (x0, x1) = (fx(vis_sx1), fx_ceil(vis_sx0));
            let (y0, y1) = (fy(vis_sy1), fy_ceil(vis_sy0));
            (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1), vis_w, vis_h)
        }
        // Shader: output(rx,ry) ← src(scaled_w-1-ry, rx); output size = scaled_h × scaled_w
        // Visible src x: [scaled_w-vis_sy1, scaled_w-vis_sy0), y: [vis_sx0, vis_sx1)
        // Resize to (vis_h, vis_w) then rotate 270 → vis_w × vis_h
        _ => {
            let (x0, x1) = (fx(vis_sy1), fx_ceil(vis_sy0));
            let (y0, y1) = (sy(vis_sx0), sy_ceil(vis_sx1));
            (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1), vis_h, vis_w)
        }
    }
}
