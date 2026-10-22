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
//! With a GPU feature (`gpu-vulkan` or `gpu-gles`) the image is rendered by
//! [vello](https://github.com/linebender/vello):
//! 1. Pick a pre-filtered level of the source (cached per image) so vello never
//!    minifies by more than 2×; crop it to the visible region if it is too
//!    large for vello's image atlas.
//! 2. Encode a vello scene with one image fill whose transform carries the
//!    viewport scale, rotation and offset.
//! 3. vello rasterises the scene into a `dst_w × dst_h` `Rgba8Unorm` texture.
//! 4. Readback to CPU as ARGB8888 bytes.

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
/// When compiled with `gpu-vulkan` or `gpu-gles`, the frame is rasterised on
/// the GPU by vello and read back; `gpu` caches per-image state between calls.
/// If vello fails, the error is logged and a transparent frame is returned.
pub fn render(
    src: &DynamicImage,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
    #[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))] gpu: &mut gpu::GpuContext,
) -> Vec<u8> {
    #[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
    {
        let rendered = gpu.render_to_target(
            src,
            viewport,
            dst_w,
            dst_h,
            filter,
            vello::peniko::Color::TRANSPARENT,
        );
        match rendered {
            Ok(()) => gpu.read_target(),
            Err(e) => {
                tracing::error!(error = %e, "vello render failed");
                vec![0u8; (dst_w * dst_h * 4) as usize]
            }
        }
    }

    #[cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]
    {
        let scaled_w = ((src.width() as f32) * viewport.scale).max(1.0) as u32;
        let scaled_h = ((src.height() as f32) * viewport.scale).max(1.0) as u32;
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
