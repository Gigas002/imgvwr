use std::io::Write;

use tempfile::Builder;

use super::*;

#[cfg(feature = "png")]
#[test]
fn load_png_4x4() {
    let png_bytes = include_bytes!("../../tests/fixtures/4x4.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(png_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "jpeg")]
#[test]
fn load_jpeg_4x4() {
    let jpeg_bytes = include_bytes!("../../tests/fixtures/4x4.jpg");
    let mut tmp = Builder::new().suffix(".jpg").tempfile().unwrap();
    tmp.write_all(jpeg_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "webp")]
#[test]
fn load_webp_4x4() {
    let webp_bytes = include_bytes!("../../tests/fixtures/4x4.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(webp_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "avif")]
#[test]
fn load_avif_4x4() {
    let avif_bytes = include_bytes!("../../tests/fixtures/4x4.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(avif_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "jxl")]
#[test]
fn load_jxl_4x4() {
    let jxl_bytes = include_bytes!("../../tests/fixtures/4x4.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(jxl_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "gif")]
#[test]
fn load_gif_animated_4x4() {
    let gif_bytes = include_bytes!("../../tests/fixtures/4x4_anim.gif");
    let mut tmp = Builder::new().suffix(".gif").tempfile().unwrap();
    tmp.write_all(gif_bytes).unwrap();
    let result = super::load_gif_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[cfg(feature = "jxl-anim")]
#[test]
fn load_jxl_anim_4x4() {
    let jxl_bytes = include_bytes!("../../tests/fixtures/4x4_anim.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(jxl_bytes).unwrap();
    let result = super::load_jxl_anim_frames(tmp.path()).unwrap();
    assert!(!result.frames.is_empty());
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[cfg(feature = "avif-anim")]
#[test]
fn load_avif_anim_4x4() {
    let avif_bytes = include_bytes!("../../tests/fixtures/4x4_anim.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(avif_bytes).unwrap();
    let result = super::load_avif_anim_frames(tmp.path()).unwrap();
    assert!(!result.frames.is_empty());
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[cfg(feature = "apng")]
#[test]
fn load_apng_anim_4x4() {
    let png_bytes = include_bytes!("../../tests/fixtures/4x4_anim.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(png_bytes).unwrap();
    let result = super::load_apng_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[cfg(feature = "webp-anim")]
#[test]
fn load_webp_anim_4x4() {
    let webp_bytes = include_bytes!("../../tests/fixtures/4x4_anim.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(webp_bytes).unwrap();
    let result = super::load_webp_anim_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[test]
fn load_unsupported_format() {
    let mut tmp = Builder::new().suffix(".xyz").tempfile().unwrap();
    tmp.write_all(b"not an image").unwrap();
    let result = load(tmp.path());
    assert!(matches!(result, Err(LoadError::UnsupportedFormat)));
}

// ── Color / pixel-content edge cases (per-decoder) ──────────────────────────

#[cfg(feature = "png")]
#[test]
fn load_png_grayscale() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_gray.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    // Grayscale pixels should have equal R, G, B channels when converted to RGBA.
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[0], px[1]);
    assert_eq!(px[1], px[2]);
}

#[cfg(feature = "png")]
#[test]
fn load_png_palette() {
    // Palette (indexed) PNG: image-rs expands to RGB/RGBA automatically.
    let bytes = include_bytes!("../../tests/fixtures/4x4_palette.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    // Palette index 0 was set to pure red.
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[0], 255); // R
    assert_eq!(px[1], 0); // G
    assert_eq!(px[2], 0); // B
}

#[cfg(feature = "png")]
#[test]
fn load_png_rgb_no_alpha() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_rgb.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    // Solid red with no alpha channel; converted pixel must have A = 255.
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px, [255, 0, 0, 255]);
}

#[cfg(feature = "png")]
#[test]
fn load_png_fully_transparent() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_transparent.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[3], 0); // fully transparent
}

#[cfg(feature = "png")]
#[test]
fn load_png_pixel_channel_order() {
    // 1×1 pure-red image catches RGBA/BGRA channel-swap bugs.
    let bytes = include_bytes!("../../tests/fixtures/1x1_red.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (1, 1));
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px, [255, 0, 0, 255]);
}

#[cfg(feature = "webp")]
#[test]
fn load_webp_pixel_channel_order() {
    // WebP has had historical BGRA/RGBA confusion; verify channel order.
    let bytes = include_bytes!("../../tests/fixtures/1x1_red.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (1, 1));
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px, [255, 0, 0, 255]);
}

#[cfg(feature = "webp")]
#[test]
fn load_webp_grayscale() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_gray.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[0], px[1], "R should equal G for grayscale");
    assert_eq!(px[1], px[2], "G should equal B for grayscale");
    assert_eq!(px[3], 255, "opaque");
}

// JPEG decoder tests

#[cfg(feature = "jpeg")]
#[test]
fn load_jpeg_grayscale() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_gray.jpg");
    let mut tmp = Builder::new().suffix(".jpg").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[0], px[1], "R should equal G for grayscale");
    assert_eq!(px[1], px[2], "G should equal B for grayscale");
    assert_eq!(px[3], 255, "JPEG has no alpha; must be fully opaque");
}

#[cfg(feature = "jpeg")]
#[test]
fn load_jpeg_opaque() {
    // JPEG does not support alpha; every pixel must be fully opaque.
    let bytes = include_bytes!("../../tests/fixtures/4x4.jpg");
    let mut tmp = Builder::new().suffix(".jpg").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    for y in 0..img.height() {
        for x in 0..img.width() {
            let px = img.to_rgba8().get_pixel(x, y).0;
            assert_eq!(px[3], 255, "pixel ({x},{y}) alpha should be 255");
        }
    }
}

#[cfg(feature = "jpeg")]
#[test]
fn load_jpeg_approximate_color() {
    // Lossy JPEG of a bright-red image; check the red channel dominates.
    let bytes = include_bytes!("../../tests/fixtures/4x4_red.jpg");
    let mut tmp = Builder::new().suffix(".jpg").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert!(px[0] > 200, "red channel should dominate (got {})", px[0]);
    assert!(px[1] < 40, "green should be low (got {})", px[1]);
    assert!(px[2] < 40, "blue should be low (got {})", px[2]);
    assert_eq!(px[3], 255);
}

// AVIF decoder tests

#[cfg(feature = "avif")]
#[test]
fn load_avif_grayscale() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_gray.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[0], px[1], "R should equal G for grayscale");
    assert_eq!(px[1], px[2], "G should equal B for grayscale");
    assert_eq!(px[3], 255, "opaque");
}

#[cfg(feature = "avif")]
#[test]
fn load_avif_opaque() {
    let bytes = include_bytes!("../../tests/fixtures/4x4.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    for y in 0..img.height() {
        for x in 0..img.width() {
            assert_eq!(img.to_rgba8().get_pixel(x, y).0[3], 255);
        }
    }
}

#[cfg(feature = "avif")]
#[test]
fn load_avif_pixel_channel_order() {
    // AVIF uses YUV internally; RGB (255,0,0) round-trips with rounding loss.
    // Verify the red channel dominates (no BGRA/RGBA swap), not exact equality.
    let bytes = include_bytes!("../../tests/fixtures/1x1_red.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert!(
        px[0] > px[1] && px[0] > px[2],
        "R should dominate (got {px:?})"
    );
    assert!(px[0] > 200, "R should be high (got {})", px[0]);
    assert_eq!(px[3], 255);
}

// JXL decoder tests (custom decoder path)

#[cfg(feature = "jxl")]
#[test]
fn load_jxl_grayscale() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_gray.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(px[0], px[1], "R should equal G for grayscale");
    assert_eq!(px[1], px[2], "G should equal B for grayscale");
}

#[cfg(feature = "jxl")]
#[test]
fn load_jxl_pixel_channel_order() {
    let bytes = include_bytes!("../../tests/fixtures/1x1_red.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(
        px,
        [255, 0, 0, 255],
        "lossless JXL should preserve exact RGBA"
    );
}

// Animated-format frame-color tests

#[cfg(feature = "gif")]
#[test]
fn load_gif_frame_pixel_values() {
    // Fixture: frame 0 = red, frame 1 = blue (GIF palette, lossless).
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.gif");
    let mut tmp = Builder::new().suffix(".gif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_gif_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    let f0 = result.frames[0].0.to_rgba8().get_pixel(0, 0).0;
    let f1 = result.frames[1].0.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(f0, [255, 0, 0, 255], "frame 0 should be red");
    assert_eq!(f1, [0, 0, 255, 255], "frame 1 should be blue");
}

#[cfg(feature = "apng")]
#[test]
fn load_apng_frame_pixel_values() {
    // Fixture: frame 0 = red, frame 1 = blue (lossless PNG).
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_apng_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    let f0 = result.frames[0].0.to_rgba8().get_pixel(0, 0).0;
    let f1 = result.frames[1].0.to_rgba8().get_pixel(0, 0).0;
    assert_eq!(f0, [255, 0, 0, 255], "frame 0 should be red");
    assert_eq!(f1, [0, 0, 255, 255], "frame 1 should be blue");
}

#[cfg(feature = "webp-anim")]
#[test]
fn load_webp_anim_frame_colors() {
    // Fixture: frame 0 ≈ red, frame 1 ≈ green (lossy WebP, allow ±2).
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_webp_anim_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    let f0 = result.frames[0].0.to_rgba8().get_pixel(0, 0).0;
    let f1 = result.frames[1].0.to_rgba8().get_pixel(0, 0).0;
    assert!(
        f0[0] > 250 && f0[1] <= 4 && f0[2] <= 4,
        "frame 0 should be ~red, got {f0:?}"
    );
    assert!(
        f1[1] > 250 && f1[0] <= 4 && f1[2] <= 4,
        "frame 1 should be ~green, got {f1:?}"
    );
}

#[cfg(feature = "avif-anim")]
#[test]
fn load_avif_anim_frames_distinct_colors() {
    // Frames should decode to visually distinct colors (not all the same).
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_avif_anim_frames(tmp.path()).unwrap();
    assert!(result.frames.len() >= 2);
    let f0 = result.frames[0].0.to_rgba8().get_pixel(0, 0).0;
    let f1 = result.frames[result.frames.len() - 1]
        .0
        .to_rgba8()
        .get_pixel(0, 0)
        .0;
    assert_ne!(
        f0[..3],
        f1[..3],
        "first and last frames should have different colors"
    );
}

#[cfg(feature = "jxl-anim")]
#[test]
fn load_jxl_anim_frames_distinct_colors() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_jxl_anim_frames(tmp.path()).unwrap();
    assert!(result.frames.len() >= 2);
    let f0 = result.frames[0].0.to_rgba8().get_pixel(0, 0).0;
    let f1 = result.frames[result.frames.len() - 1]
        .0
        .to_rgba8()
        .get_pixel(0, 0)
        .0;
    assert_ne!(
        f0[..3],
        f1[..3],
        "first and last frames should have different colors"
    );
}

// ── HDR / high bit-depth ─────────────────────────────────────────────────────

#[cfg(feature = "png")]
#[test]
fn load_png_16bit_does_not_panic() {
    // 16-bit RGBA PNG (0x8000, 0, 0, 0xFFFF per channel).
    // The renderer pipeline is ARGB8888, so image-rs silently downsamples to
    // 8-bit via to_rgba8() (right-shift 8). Verify it loads cleanly and the
    // red channel dominates after downsampling (~128 from 0x8000).
    let bytes = include_bytes!("../../tests/fixtures/4x4_16bit.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    let px = img.to_rgba8().get_pixel(0, 0).0;
    assert!(
        px[0] > px[1] && px[0] > px[2],
        "red should dominate after downsampling"
    );
    assert_eq!(px[3], 255);
}

// ── Error-path edge cases ────────────────────────────────────────────────────

#[test]
fn load_missing_file_returns_io_error() {
    let result = load(std::path::Path::new("/nonexistent/path/to/image.png"));
    assert!(matches!(result, Err(LoadError::Io(_))));
}

#[cfg(feature = "png")]
#[test]
fn load_corrupt_png_returns_decode_error() {
    // Valid PNG magic followed by garbage — the decoder recognises the format
    // but fails to parse the chunks.
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(b"\x89PNG\r\n\x1a\ngarbage data here")
        .unwrap();
    let result = load(tmp.path());
    assert!(matches!(result, Err(LoadError::Decode(_))));
}

#[cfg(feature = "gif")]
#[test]
fn load_corrupt_gif_returns_error() {
    let mut tmp = Builder::new().suffix(".gif").tempfile().unwrap();
    tmp.write_all(b"GIF89a\x00\x00garbage").unwrap();
    let result = super::load_gif_frames(tmp.path());
    assert!(result.is_err());
}

#[cfg(feature = "apng")]
#[test]
fn png_static_rejected_by_apng_loader() {
    let bytes = include_bytes!("../../tests/fixtures/4x4.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_apng_frames(tmp.path());
    assert!(matches!(result, Err(LoadError::UnsupportedFormat)));
}

#[cfg(feature = "webp-anim")]
#[test]
fn webp_static_rejected_by_anim_loader() {
    let bytes = include_bytes!("../../tests/fixtures/4x4.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_webp_anim_frames(tmp.path());
    assert!(matches!(result, Err(LoadError::UnsupportedFormat)));
}

#[cfg(feature = "jxl-anim")]
#[test]
fn jxl_static_rejected_by_anim_loader() {
    let bytes = include_bytes!("../../tests/fixtures/4x4.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_jxl_anim_frames(tmp.path());
    assert!(matches!(result, Err(LoadError::UnsupportedFormat)));
}

#[cfg(feature = "avif-anim")]
#[test]
fn avif_static_rejected_by_anim_loader() {
    // Static AVIF uses HEIF image items, not a video track; mp4parse either
    // fails to parse the container (Decode) or finds no AV1 track (UnsupportedFormat).
    let bytes = include_bytes!("../../tests/fixtures/4x4.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_avif_anim_frames(tmp.path());
    assert!(result.is_err());
}

#[cfg(feature = "gif")]
#[test]
fn gif_frame_delays_are_at_least_10ms() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.gif");
    let mut tmp = Builder::new().suffix(".gif").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_gif_frames(tmp.path()).unwrap();
    for (_, delay) in &result.frames {
        assert!(delay.as_millis() >= 10, "frame delay below 10 ms minimum");
    }
}

#[cfg(feature = "apng")]
#[test]
fn apng_frame_delays_are_at_least_10ms() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_apng_frames(tmp.path()).unwrap();
    for (_, delay) in &result.frames {
        assert!(delay.as_millis() >= 10, "frame delay below 10 ms minimum");
    }
}

#[cfg(feature = "webp-anim")]
#[test]
fn webp_anim_frame_delays_are_at_least_10ms() {
    let bytes = include_bytes!("../../tests/fixtures/4x4_anim.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(bytes).unwrap();
    let result = super::load_webp_anim_frames(tmp.path()).unwrap();
    for (_, delay) in &result.frames {
        assert!(delay.as_millis() >= 10, "frame delay below 10 ms minimum");
    }
}
