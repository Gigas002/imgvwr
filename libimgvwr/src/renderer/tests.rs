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

#[cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]
mod cpu {
    use super::*;

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
}

#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
mod gpu {
    use super::*;
    use crate::renderer::gpu::*;
    use vello::kurbo::Point;

    fn viewport(scale: f32, rotation: u16, offset: (f32, f32)) -> ViewportState {
        ViewportState {
            scale,
            offset,
            rotation,
        }
    }

    fn assert_point(actual: Point, expected: (f64, f64)) {
        assert!(
            (actual.x - expected.0).abs() < 1e-9 && (actual.y - expected.1).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn mip_level_keeps_minification_below_two() {
        assert_eq!(mip_level_for_scale(4.0), 0);
        assert_eq!(mip_level_for_scale(1.0), 0);
        assert_eq!(mip_level_for_scale(0.75), 0);
        assert_eq!(mip_level_for_scale(0.5), 1);
        assert_eq!(mip_level_for_scale(0.3), 1);
        assert_eq!(mip_level_for_scale(0.25), 2);
        assert_eq!(mip_level_for_scale(0.1), 3);
        assert_eq!(mip_level_for_scale(0.0), 0);
    }

    #[test]
    fn level_size_halves_and_never_reaches_zero() {
        assert_eq!(level_size(100, 50, 0), (100, 50));
        assert_eq!(level_size(100, 50, 1), (50, 25));
        assert_eq!(level_size(100, 50, 6), (1, 1));
        assert_eq!(level_size(100, 50, 40), (1, 1));
    }

    #[test]
    fn identity_transform_centres_image() {
        let t = image_transform(4, 4, &viewport(1.0, 0, (0.0, 0.0)), 8, 8);
        assert_point(t * Point::new(0.0, 0.0), (2.0, 2.0));
        assert_point(t * Point::new(4.0, 4.0), (6.0, 6.0));
    }

    #[test]
    fn transform_applies_scale_and_offset() {
        let t = image_transform(4, 4, &viewport(2.0, 0, (3.0, -1.0)), 16, 16);
        assert_point(t * Point::new(0.0, 0.0), (7.0, 3.0));
        assert_point(t * Point::new(4.0, 4.0), (15.0, 11.0));
    }

    #[test]
    fn transform_rotates_clockwise() {
        // 4×2 image rotated 90° becomes 2×4, centred in 8×8 at (3, 2).
        let t = image_transform(4, 2, &viewport(1.0, 90, (0.0, 0.0)), 8, 8);
        // Source top-left lands on the top-right of the rotated box.
        assert_point(t * Point::new(0.0, 0.0), (5.0, 2.0));
        // Source bottom-left lands on the top-left.
        assert_point(t * Point::new(0.0, 2.0), (3.0, 2.0));

        let t = image_transform(4, 2, &viewport(1.0, 270, (0.0, 0.0)), 8, 8);
        // Source top-left lands on the bottom-left.
        assert_point(t * Point::new(0.0, 0.0), (3.0, 6.0));
    }

    #[test]
    fn transform_snaps_to_pixel_grid() {
        // 3×3 image in 8×8: unsnapped left edge would be 2.5.
        let t = image_transform(3, 3, &viewport(1.0, 0, (0.0, 0.0)), 8, 8);
        let p = t * Point::new(0.0, 0.0);
        assert_eq!(p.x.fract(), 0.0);
        assert_eq!(p.y.fract(), 0.0);
    }

    #[test]
    fn visible_crop_covers_whole_image_when_it_fits() {
        let t = image_transform(4, 4, &viewport(1.0, 0, (0.0, 0.0)), 8, 8);
        assert_eq!(visible_crop(t, 4, 4, 8, 8), Some((0, 0, 4, 4)));
    }

    #[test]
    fn visible_crop_limits_zoomed_in_region() {
        // 1000×1000 at 10× in a 100×100 window shows the central 10×10 pixels.
        let t = image_transform(1000, 1000, &viewport(10.0, 0, (0.0, 0.0)), 100, 100);
        assert_eq!(
            visible_crop(t, 1000, 1000, 100, 100),
            Some((493, 493, 14, 14))
        );
    }

    #[test]
    fn visible_crop_follows_rotation() {
        // Pan so only the right half of the window shows the image, rotated 180°:
        // the visible part is then the image's left side.
        let t = image_transform(100, 100, &viewport(1.0, 180, (50.0, 0.0)), 100, 100);
        let (x, _, w, _) = visible_crop(t, 100, 100, 100, 100).unwrap();
        assert!(x >= 48, "x = {x}");
        assert_eq!(x + w, 100);
    }

    #[test]
    fn visible_crop_none_when_off_screen() {
        let t = image_transform(4, 4, &viewport(1.0, 0, (100.0, 0.0)), 8, 8);
        assert_eq!(visible_crop(t, 4, 4, 8, 8), None);
    }

    #[test]
    fn filter_maps_to_vello_quality() {
        use vello::peniko::ImageQuality;
        assert_eq!(image_quality(FilterMethod::Nearest), ImageQuality::Low);
        assert_eq!(image_quality(FilterMethod::Triangle), ImageQuality::Medium);
        assert_eq!(image_quality(FilterMethod::Lanczos3), ImageQuality::High);
    }

    /// End-to-end render through vello. Skipped when the machine has no
    /// usable GPU adapter (e.g. CI runners without a GPU).
    #[test]
    fn renders_centred_image() {
        let Ok(mut ctx) = GpuContext::new() else {
            eprintln!("no GPU adapter available, skipping");
            return;
        };
        let src = red_4x4();
        let buf = render(
            &src,
            &ViewportState::default(),
            8,
            8,
            FilterMethod::Nearest,
            &mut ctx,
        );
        assert_eq!(buf.len(), 8 * 8 * 4);
        assert_eq!(argb_at(&buf, 8, 3, 3), (255, 0, 0, 255));
        assert_eq!(
            argb_at(&buf, 8, 0, 0).3,
            0,
            "background should be transparent"
        );
    }

    /// Images wider than vello's atlas are cropped to the visible region.
    #[test]
    fn renders_image_larger_than_atlas() {
        let Ok(mut ctx) = GpuContext::new() else {
            eprintln!("no GPU adapter available, skipping");
            return;
        };
        let src = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            MAX_ATLAS_SIDE + 100,
            4,
            Rgba([0u8, 255, 0, 255]),
        ));
        let buf = render(
            &src,
            &ViewportState::default(),
            16,
            8,
            FilterMethod::Nearest,
            &mut ctx,
        );
        assert_eq!(argb_at(&buf, 16, 8, 3), (0, 255, 0, 255));
        assert_eq!(argb_at(&buf, 16, 8, 0).3, 0);
    }

    /// Strong minification goes through the pre-scaled levels.
    #[test]
    fn renders_minified_image() {
        let Ok(mut ctx) = GpuContext::new() else {
            eprintln!("no GPU adapter available, skipping");
            return;
        };
        let src =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(256, 256, Rgba([0u8, 0, 255, 255])));
        let vp = ViewportState {
            scale: 0.05,
            ..Default::default()
        };
        let buf = render(&src, &vp, 32, 32, FilterMethod::Lanczos3, &mut ctx);
        assert_eq!(argb_at(&buf, 32, 16, 16), (0, 0, 255, 255));
        assert_eq!(argb_at(&buf, 32, 0, 0).3, 0);
    }
}
