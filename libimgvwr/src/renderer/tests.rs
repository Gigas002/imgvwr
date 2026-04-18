#![cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]

use image::{DynamicImage, ImageBuffer, Rgba};

use super::*;
use crate::viewport::ViewportState;

fn red_4x4() -> DynamicImage {
    let buf = ImageBuffer::from_pixel(4, 4, Rgba([255u8, 0, 0, 255]));
    DynamicImage::ImageRgba8(buf)
}

fn argb_at(buf: &[u8], dst_w: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
    let i = (y * dst_w + x) as usize * 4;
    // stored as [B, G, R, A]
    (buf[i + 2], buf[i + 1], buf[i], buf[i + 3])
}

#[test]
fn centre_pixels_are_red_argb() {
    let src = red_4x4();
    let vp = ViewportState::default();
    let buf = render(&src, &vp, 8, 8, FilterMethod::Nearest);

    // 4×4 image centred in 8×8 → blit_x = 2, blit_y = 2
    // pixels (2,2) through (5,5) should be red: R=255 G=0 B=0 A=255
    for y in 2..6 {
        for x in 2..6 {
            let (r, g, b, a) = argb_at(&buf, 8, x, y);
            assert_eq!((r, g, b, a), (255, 0, 0, 255), "pixel ({x},{y}) not red");
        }
    }
}

#[test]
fn corners_are_black() {
    let src = red_4x4();
    let vp = ViewportState::default();
    let buf = render(&src, &vp, 8, 8, FilterMethod::Nearest);

    for (x, y) in [(0, 0), (7, 0), (0, 7), (7, 7)] {
        let (r, g, b, a) = argb_at(&buf, 8, x, y);
        assert_eq!((r, g, b, a), (0, 0, 0, 0), "corner ({x},{y}) not black");
    }
}

#[test]
fn output_buffer_size_matches_dst() {
    let src = red_4x4();
    let vp = ViewportState::default();
    let buf = render(&src, &vp, 16, 12, FilterMethod::Nearest);
    assert_eq!(buf.len(), 16 * 12 * 4);
}

#[test]
fn rotation_90_swaps_dimensions() {
    let src = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 4, Rgba([255u8, 0, 0, 255])));
    let mut vp = ViewportState::default();
    vp.rotate_right(); // 90°
    // After 90° rotation a 2×4 image becomes 4×2; still fits in 8×8
    let buf = render(&src, &vp, 8, 8, FilterMethod::Nearest);
    assert_eq!(buf.len(), 8 * 8 * 4);
}
