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

    /// The renderer before visible-region rendering: resize the whole image,
    /// rotate it, then blit. The fast path must produce the same pixels.
    fn reference_render(
        src: &DynamicImage,
        viewport: &ViewportState,
        dst_w: u32,
        dst_h: u32,
        filter: FilterMethod,
    ) -> Vec<u8> {
        let scaled_w = ((src.width() as f32) * viewport.scale).max(1.0) as u32;
        let scaled_h = ((src.height() as f32) * viewport.scale).max(1.0) as u32;
        let scaled = image::imageops::resize(src, scaled_w, scaled_h, filter.into());
        let rotated = match viewport.rotation {
            90 => image::imageops::rotate90(&scaled),
            180 => image::imageops::rotate180(&scaled),
            270 => image::imageops::rotate270(&scaled),
            _ => scaled,
        };
        let blit_x = (dst_w as i32 / 2) - (rotated.width() as i32 / 2) + viewport.offset.0 as i32;
        let blit_y = (dst_h as i32 / 2) - (rotated.height() as i32 / 2) + viewport.offset.1 as i32;
        let mut buf = vec![0u8; (dst_w * dst_h * 4) as usize];
        for (sx, sy, Rgba([r, g, b, a])) in rotated.enumerate_pixels() {
            let (dx, dy) = (blit_x + sx as i32, blit_y + sy as i32);
            if dx < 0 || dy < 0 || dx >= dst_w as i32 || dy >= dst_h as i32 {
                continue;
            }
            let i = (dy as u32 * dst_w + dx as u32) as usize * 4;
            buf[i..i + 4].copy_from_slice(&[*b, *g, *r, *a]);
        }
        buf
    }

    /// A deterministic, non-uniform test image.
    fn noise(w: u32, h: u32) -> DynamicImage {
        let mut state = 0x2545_f491_u32;
        DynamicImage::ImageRgba8(ImageBuffer::from_fn(w, h, |_, _| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            };
            Rgba([next(), next(), next(), next() | 0x80])
        }))
    }

    #[test]
    fn matches_full_resize_for_every_view() {
        let src = noise(37, 23);
        let filters = [
            FilterMethod::Nearest,
            FilterMethod::Triangle,
            FilterMethod::CatmullRom,
            FilterMethod::Gaussian,
            FilterMethod::Lanczos3,
        ];
        for scale in [0.31, 0.77, 1.0, 1.6, 3.3, 7.0] {
            for rotation in [0, 90, 180, 270] {
                for offset in [(0.0, 0.0), (-50.0, 13.0), (61.0, -40.0), (500.0, 0.0)] {
                    for filter in filters {
                        let vp = ViewportState {
                            scale,
                            offset,
                            rotation,
                        };
                        let got = render(&src, &vp, 64, 48, filter);
                        let want = reference_render(&src, &vp, 64, 48, filter);
                        assert!(
                            got == want,
                            "scale {scale} rotation {rotation} offset {offset:?} {filter:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn deep_zoom_does_not_allocate_the_whole_scaled_image() {
        // 4000 × 3000 at 100× would be a 480 GB buffer if scaled in full.
        let src = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            4000,
            3000,
            Rgba([10u8, 200, 30, 255]),
        ));
        let vp = ViewportState {
            scale: 100.0,
            offset: (0.0, 0.0),
            rotation: 90,
        };
        let buf = render(&src, &vp, 320, 200, FilterMethod::Lanczos3);
        assert_eq!(argb_at(&buf, 320, 160, 100), (10, 200, 30, 255));
        assert_eq!(argb_at(&buf, 320, 0, 0), (10, 200, 30, 255));
    }

    #[test]
    fn panning_reuses_the_zoomed_out_image() {
        let src = noise(300, 200);
        let mut vp = ViewportState {
            scale: 0.5,
            offset: (0.0, 0.0),
            rotation: 0,
        };
        let first = render(&src, &vp, 100, 80, FilterMethod::Triangle);
        vp.pan(7.0, -3.0);
        let panned = render(&src, &vp, 100, 80, FilterMethod::Triangle);
        assert_ne!(first, panned);
        assert_eq!(
            panned,
            reference_render(&src, &vp, 100, 80, FilterMethod::Triangle)
        );
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
        assert_eq!(level_size(100, 50, 6), (2, 1));
        assert_eq!(level_size(100, 50, 40), (1, 1));
    }

    #[test]
    fn level_size_matches_the_levels_halve_builds() {
        // Odd sides round up; flooring them made the level and the size the
        // transform assumed disagree (and tripped a debug assertion).
        let (mut w, mut h) = (6001u32, 3999u32);
        let mut data = vec![0u8; (w * h * 4) as usize];
        for level in 1..=4 {
            let next = halve(&data, w, h, FilterMethod::Triangle);
            (w, h) = next.dimensions();
            assert_eq!(level_size(6001, 3999, level), (w, h), "level {level}");
            data = next.into_raw();
        }
    }

    #[test]
    fn halve_averages_blocks_and_repeats_odd_edges() {
        // 3×1: [0, 100, 200] → [avg(0,100), 200]
        let px = |v: u8| [v, v, v, 255];
        let src: Vec<u8> = [px(0), px(100), px(200)].concat();
        let out = halve(&src, 3, 1, FilterMethod::Lanczos3);
        assert_eq!(out.dimensions(), (2, 1));
        assert_eq!(out.get_pixel(0, 0).0, [50, 50, 50, 255]);
        assert_eq!(out.get_pixel(1, 0).0, [200, 200, 200, 255]);
    }

    #[test]
    fn halve_keeps_hard_pixels_for_nearest() {
        let px = |v: u8| [v, v, v, 255];
        let src: Vec<u8> = [px(0), px(100), px(200), px(255)].concat();
        let out = halve(&src, 2, 2, FilterMethod::Nearest);
        assert_eq!(out.dimensions(), (1, 1));
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 0, 255]);
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
