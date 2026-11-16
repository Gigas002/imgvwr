//! Rendering pipeline: transforms a [`DynamicImage`] into a
//! Wayland-compatible ARGB8888 pixel buffer.
//!
//! Without a GPU feature the pipeline is fully CPU-based:
//! 1. Work out which part of the scaled, rotated image lands in the
//!    `dst_w × dst_h` window once centred and shifted by `viewport.offset`.
//! 2. Produce only that part: zoomed out, crop it from a scaled copy cached
//!    while the image and scale stay the same; zoomed in, resample just that
//!    region from the source with the `imageops::resize` filters.
//! 3. Rotate it if `viewport.rotation != 0` and blit it into the buffer.
//!    Pixels outside the image stay black.
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

#[cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]
mod cpu;
#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
pub mod gpu;
#[cfg(test)]
mod tests;

use image::{DynamicImage, imageops};

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
        cpu::render(src, viewport, dst_w, dst_h, filter)
    }
}
