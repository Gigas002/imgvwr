//! Image loading via `image-rs`.
//!
//! Wraps [`image::open`] with a typed error that distinguishes I/O failures,
//! decode failures, and formats not compiled in via Cargo features.

#[cfg(test)]
mod tests;

use std::path::Path;
#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
use std::time::Duration;

use image::{DynamicImage, ImageError};

/// Errors that can occur when loading an image.
#[derive(Debug)]
pub enum LoadError {
    /// An OS-level I/O error (file not found, permission denied, etc.).
    Io(std::io::Error),
    /// The file was found but could not be decoded.
    Decode(ImageError),
    /// The format is not supported by the currently enabled Cargo features.
    UnsupportedFormat,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "IO error: {e}"),
            LoadError::Decode(e) => write!(f, "Decode error: {e}"),
            LoadError::UnsupportedFormat => write!(f, "unsupported image format"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Load an image from `path`, returning a [`DynamicImage`] on success.
///
/// Blocking, single-threaded. Only the first frame is loaded for formats that
/// support animation; subsequent frames are ignored.
pub fn load(path: &Path) -> Result<DynamicImage, LoadError> {
    #[cfg(feature = "jxl")]
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        == Some("jxl")
    {
        return load_jxl(path);
    }

    image::open(path).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        ImageError::Unsupported(_) => LoadError::UnsupportedFormat,
        other => LoadError::Decode(other),
    })
}

#[cfg(feature = "jxl")]
fn load_jxl(path: &Path) -> Result<DynamicImage, LoadError> {
    use image::{ImageBuffer, Rgba};
    use jxl::api::states::Initialized;
    use jxl::api::{
        JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat, ProcessingResult,
    };

    let file_bytes = std::fs::read(path).map_err(LoadError::Io)?;
    let options = JxlDecoderOptions::default();

    // Phase 1 — parse image header → get dimensions
    let mut decoder = JxlDecoder::<Initialized>::new(options);
    let mut input = file_bytes.as_slice();
    let mut decoder_info = loop {
        match decoder.process(&mut input).map_err(jxl_err)? {
            ProcessingResult::Complete { result } => break result,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder = fallback,
        }
    };

    let (width, height) = decoder_info.basic_info().size;
    let num_extra = decoder_info.basic_info().extra_channels.len();
    use jxl::api::{JxlColorType, JxlDataFormat};
    decoder_info.set_pixel_format(JxlPixelFormat {
        color_type: JxlColorType::Rgba,
        color_data_format: Some(JxlDataFormat::U8 { bit_depth: 8 }),
        extra_channel_format: vec![None; num_extra],
    });

    // Phase 2 — parse frame header
    let mut decoder_frame = loop {
        match decoder_info.process(&mut input).map_err(jxl_err)? {
            ProcessingResult::Complete { result } => break result,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder_info = fallback,
        }
    };

    // Phase 3 — decode first frame into an RGBA u8 buffer
    let stride = width * 4;
    let mut pixel_buf = vec![0u8; height * stride];
    loop {
        let out = JxlOutputBuffer::new(&mut pixel_buf, height, stride);
        match decoder_frame
            .process(&mut input, &mut [out])
            .map_err(jxl_err)?
        {
            ProcessingResult::Complete { .. } => break,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder_frame = fallback,
        }
    }

    ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, pixel_buf)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| {
            use image::error::{DecodingError, ImageFormatHint};
            LoadError::Decode(ImageError::Decoding(DecodingError::new(
                ImageFormatHint::Name("JXL".to_owned()),
                "buffer size mismatch",
            )))
        })
}

/// A decoded animation: one or more frames with per-frame display durations.
#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
pub struct AnimFrames {
    pub frames: Vec<(DynamicImage, Duration)>,
}

/// Load an animated GIF from `path`, returning all frames with their display durations.
///
/// Frames with a zero delay are clamped to 10 ms (browser convention).
/// Static GIFs (single frame) are returned as a one-element `AnimFrames`.
#[cfg(feature = "gif")]
pub fn load_gif_frames(path: &Path) -> Result<AnimFrames, LoadError> {
    use std::io::BufReader;

    use image::AnimationDecoder;
    use image::codecs::gif::GifDecoder;

    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        other => LoadError::Decode(other),
    })?;

    let raw_frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| match e {
            ImageError::IoError(io_err) => LoadError::Io(io_err),
            other => LoadError::Decode(other),
        })?;

    let frames = raw_frames
        .into_iter()
        .map(|frame: image::Frame| {
            let (numer, denom) = frame.delay().numer_denom_ms();
            let ms = if denom == 0 {
                10
            } else {
                (numer as u64 / denom as u64).max(10)
            };
            let duration = Duration::from_millis(ms);
            let img = DynamicImage::ImageRgba8(frame.into_buffer());
            (img, duration)
        })
        .collect();

    Ok(AnimFrames { frames })
}

/// Load an animated WebP from `path`, returning all frames with their display durations.
///
/// Frames with a zero delay are clamped to 10 ms (browser convention).
/// Static WebPs (single frame) are returned as a one-element `AnimFrames`.
#[cfg(feature = "webp-anim")]
pub fn load_webp_anim_frames(path: &Path) -> Result<AnimFrames, LoadError> {
    use std::io::BufReader;

    use image::AnimationDecoder;
    use image::codecs::webp::WebPDecoder;

    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    let decoder = WebPDecoder::new(BufReader::new(file)).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        other => LoadError::Decode(other),
    })?;

    if !decoder.has_animation() {
        return Err(LoadError::UnsupportedFormat);
    }

    let raw_frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| match e {
            ImageError::IoError(io_err) => LoadError::Io(io_err),
            other => LoadError::Decode(other),
        })?;

    let frames = raw_frames
        .into_iter()
        .map(|frame: image::Frame| {
            let (numer, denom) = frame.delay().numer_denom_ms();
            let ms = if denom == 0 {
                10
            } else {
                (numer as u64 / denom as u64).max(10)
            };
            let duration = Duration::from_millis(ms);
            let img = DynamicImage::ImageRgba8(frame.into_buffer());
            (img, duration)
        })
        .collect();

    Ok(AnimFrames { frames })
}

/// Load an animated PNG (APNG) from `path`, returning all frames with their display durations.
///
/// Returns `Err(LoadError::UnsupportedFormat)` for plain (non-animated) PNGs so
/// callers can fall back to `load()`.
#[cfg(feature = "apng")]
pub fn load_apng_frames(path: &Path) -> Result<AnimFrames, LoadError> {
    use std::io::BufReader;

    use image::AnimationDecoder;
    use image::codecs::png::PngDecoder;

    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    let decoder = PngDecoder::new(BufReader::new(file)).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        other => LoadError::Decode(other),
    })?;

    let is_apng = decoder.is_apng().map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        other => LoadError::Decode(other),
    })?;

    if !is_apng {
        return Err(LoadError::UnsupportedFormat);
    }

    let apng = decoder.apng().map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        other => LoadError::Decode(other),
    })?;

    let raw_frames = apng.into_frames().collect_frames().map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        other => LoadError::Decode(other),
    })?;

    let frames = raw_frames
        .into_iter()
        .map(|frame: image::Frame| {
            let (numer, denom) = frame.delay().numer_denom_ms();
            let ms = if denom == 0 {
                10
            } else {
                (numer as u64 / denom as u64).max(10)
            };
            let duration = Duration::from_millis(ms);
            let img = DynamicImage::ImageRgba8(frame.into_buffer());
            (img, duration)
        })
        .collect();

    Ok(AnimFrames { frames })
}

/// Load an animated AVIF sequence from `path`.
///
/// Parses the ISOBMFF container with `mp4parse`, extracts per-frame AV1 OBU
/// data, decodes each frame with `dav1d`, and converts YUV → RGBA.
/// Returns `Err(LoadError::UnsupportedFormat)` when no AV1 video track is found
/// (e.g. a static AVIF stored as an image item rather than a video sequence).
#[cfg(feature = "avif-anim")]
pub fn load_avif_anim_frames(path: &Path) -> Result<AnimFrames, LoadError> {
    use std::io::Cursor;

    use mp4parse::unstable::{CheckedInteger, create_sample_table};
    use mp4parse::{SampleEntry, TrackType, VideoCodecSpecific, read_mp4};

    let file_bytes = std::fs::read(path).map_err(LoadError::Io)?;

    let mut cursor = Cursor::new(&file_bytes);
    let ctx = read_mp4(&mut cursor).map_err(|e| avif_anim_container_err(format!("{e:?}")))?;

    // Find the first AV1 video track.
    let track = ctx
        .tracks
        .iter()
        .find(|t| {
            matches!(t.track_type, TrackType::Video | TrackType::Picture)
                && t.stsd.as_ref().is_some_and(|stsd| {
                    stsd.descriptions.iter().any(|d| {
                        matches!(
                            d,
                            SampleEntry::Video(v)
                                if matches!(v.codec_specific, VideoCodecSpecific::AV1Config(_))
                        )
                    })
                })
        })
        .ok_or(LoadError::UnsupportedFormat)?;

    // Extract the AV1 Sequence Header OBU from the av1C box.
    let config_obus: Vec<u8> = track
        .stsd
        .as_ref()
        .and_then(|stsd| {
            stsd.descriptions.iter().find_map(|d| {
                if let SampleEntry::Video(v) = d
                    && let VideoCodecSpecific::AV1Config(av1c) = &v.codec_specific
                {
                    return Some(av1c.config_obus().to_vec());
                }
                None
            })
        })
        .unwrap_or_default();

    let timescale = track.timescale.as_ref().map(|ts| ts.0).unwrap_or(90_000);

    let sample_table =
        create_sample_table(track, CheckedInteger(0)).ok_or(LoadError::UnsupportedFormat)?;

    if sample_table.is_empty() {
        return Err(LoadError::UnsupportedFormat);
    }

    let mut settings = dav1d::Settings::new();
    // Single-threaded ensures each send_data immediately produces a picture.
    settings.set_n_threads(1);
    let mut decoder = dav1d::Decoder::with_settings(&settings)
        .map_err(|e| avif_anim_decode_err(format!("decoder init: {e:?}")))?;

    let mut frames = Vec::with_capacity(sample_table.len());

    for indice in sample_table.iter() {
        let start = indice.start_offset.0 as usize;
        let end = indice.end_offset.0 as usize;
        if start >= file_bytes.len() || end > file_bytes.len() || start >= end {
            continue;
        }

        // Prepend the Sequence Header OBU so each frame is self-contained.
        let mut obu = config_obus.clone();
        obu.extend_from_slice(&file_bytes[start..end]);

        decoder
            .send_data(obu, None, None, None)
            .map_err(|e| avif_anim_decode_err(format!("send_data: {e:?}")))?;

        let picture = decoder
            .get_picture()
            .map_err(|e| avif_anim_decode_err(format!("get_picture: {e:?}")))?;

        let img = yuv_to_rgba(&picture)?;

        let ticks = (indice.end_composition.0 - indice.start_composition.0).max(0) as u64;
        let ms = (ticks * 1000).checked_div(timescale).unwrap_or(100).max(10);

        frames.push((img, Duration::from_millis(ms)));
    }

    if frames.is_empty() {
        return Err(LoadError::UnsupportedFormat);
    }

    Ok(AnimFrames { frames })
}

#[cfg(feature = "avif-anim")]
fn yuv_to_rgba(picture: &dav1d::Picture) -> Result<DynamicImage, LoadError> {
    use dav1d::pixel::YUVRange;
    use dav1d::{PixelLayout, PlanarImageComponent};

    let width = picture.width() as usize;
    let height = picture.height() as usize;
    let full_range = picture.color_range() == YUVRange::Full;

    let y_plane = picture.plane(PlanarImageComponent::Y);
    let stride_y = picture.stride(PlanarImageComponent::Y) as usize;

    let mut pixels = vec![0u8; width * height * 4];

    match picture.pixel_layout() {
        PixelLayout::I400 => {
            for row in 0..height {
                for col in 0..width {
                    let y = scale_y(y_plane[row * stride_y + col], full_range);
                    let v = y.clamp(0.0, 255.0) as u8;
                    let idx = (row * width + col) * 4;
                    pixels[idx] = v;
                    pixels[idx + 1] = v;
                    pixels[idx + 2] = v;
                    pixels[idx + 3] = 255;
                }
            }
        }
        PixelLayout::I420 => {
            let u_plane = picture.plane(PlanarImageComponent::U);
            let v_plane = picture.plane(PlanarImageComponent::V);
            let stride_uv = picture.stride(PlanarImageComponent::U) as usize;
            for row in 0..height {
                for col in 0..width {
                    let y = scale_y(y_plane[row * stride_y + col], full_range);
                    let u = scale_uv(u_plane[(row / 2) * stride_uv + col / 2], full_range);
                    let v = scale_uv(v_plane[(row / 2) * stride_uv + col / 2], full_range);
                    write_rgb(&mut pixels, row, col, width, bt709(y, u, v));
                }
            }
        }
        PixelLayout::I422 => {
            let u_plane = picture.plane(PlanarImageComponent::U);
            let v_plane = picture.plane(PlanarImageComponent::V);
            let stride_uv = picture.stride(PlanarImageComponent::U) as usize;
            for row in 0..height {
                for col in 0..width {
                    let y = scale_y(y_plane[row * stride_y + col], full_range);
                    let u = scale_uv(u_plane[row * stride_uv + col / 2], full_range);
                    let v = scale_uv(v_plane[row * stride_uv + col / 2], full_range);
                    write_rgb(&mut pixels, row, col, width, bt709(y, u, v));
                }
            }
        }
        PixelLayout::I444 => {
            let u_plane = picture.plane(PlanarImageComponent::U);
            let v_plane = picture.plane(PlanarImageComponent::V);
            let stride_uv = picture.stride(PlanarImageComponent::U) as usize;
            for row in 0..height {
                for col in 0..width {
                    let y = scale_y(y_plane[row * stride_y + col], full_range);
                    let u = scale_uv(u_plane[row * stride_uv + col], full_range);
                    let v = scale_uv(v_plane[row * stride_uv + col], full_range);
                    write_rgb(&mut pixels, row, col, width, bt709(y, u, v));
                }
            }
        }
    }

    image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width as u32, height as u32, pixels)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| avif_anim_decode_err("buffer size mismatch".into()))
}

#[cfg(feature = "avif-anim")]
#[inline]
fn scale_y(raw: u8, full_range: bool) -> f32 {
    if full_range {
        raw as f32
    } else {
        (raw as f32 - 16.0) * (255.0 / 219.0)
    }
}

#[cfg(feature = "avif-anim")]
#[inline]
fn scale_uv(raw: u8, full_range: bool) -> f32 {
    if full_range {
        raw as f32 - 128.0
    } else {
        (raw as f32 - 128.0) * (255.0 / 224.0)
    }
}

// BT.709 YCbCr → RGB
#[cfg(feature = "avif-anim")]
#[inline]
fn bt709(y: f32, u: f32, v: f32) -> (u8, u8, u8) {
    (
        (y + 1.5748 * v).clamp(0.0, 255.0) as u8,
        (y - 0.1873 * u - 0.4681 * v).clamp(0.0, 255.0) as u8,
        (y + 1.8556 * u).clamp(0.0, 255.0) as u8,
    )
}

#[cfg(feature = "avif-anim")]
#[inline]
fn write_rgb(pixels: &mut [u8], row: usize, col: usize, width: usize, rgb: (u8, u8, u8)) {
    let idx = (row * width + col) * 4;
    pixels[idx] = rgb.0;
    pixels[idx + 1] = rgb.1;
    pixels[idx + 2] = rgb.2;
    pixels[idx + 3] = 255;
}

#[cfg(feature = "avif-anim")]
fn avif_anim_container_err(msg: String) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("AVIF-anim".to_owned()),
        msg,
    )))
}

#[cfg(feature = "avif-anim")]
fn avif_anim_decode_err(msg: String) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("AVIF-anim".to_owned()),
        msg,
    )))
}

/// Load all frames from an animated JXL file.
///
/// Returns `Err(LoadError::UnsupportedFormat)` when the file has no animation header
/// (i.e. it is a still image), allowing callers to fall back to `load()`.
#[cfg(feature = "jxl-anim")]
pub fn load_jxl_anim_frames(path: &Path) -> Result<AnimFrames, LoadError> {
    use image::{ImageBuffer, Rgba};
    use jxl::api::states::Initialized;
    use jxl::api::{
        JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat, ProcessingResult,
    };

    let file_bytes = std::fs::read(path).map_err(LoadError::Io)?;
    let options = JxlDecoderOptions::default();

    let mut decoder = JxlDecoder::<Initialized>::new(options);
    let mut input = file_bytes.as_slice();
    let mut decoder_info = loop {
        match decoder.process(&mut input).map_err(jxl_err)? {
            ProcessingResult::Complete { result } => break result,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder = fallback,
        }
    };

    if decoder_info.basic_info().animation.is_none() {
        return Err(LoadError::UnsupportedFormat);
    }

    let (width, height) = decoder_info.basic_info().size;
    // Fold all extra channels (e.g. separate alpha) into the RGBA output by
    // marking them as None (no separate buffer).
    let num_extra = decoder_info.basic_info().extra_channels.len();
    use jxl::api::{JxlColorType, JxlDataFormat};
    let fmt = JxlPixelFormat {
        color_type: JxlColorType::Rgba,
        color_data_format: Some(JxlDataFormat::U8 { bit_depth: 8 }),
        extra_channel_format: vec![None; num_extra],
    };
    decoder_info.set_pixel_format(fmt);
    let stride = width * 4;
    let mut frames = Vec::new();

    loop {
        // Parse frame header: WithImageInfo → WithFrameInfo
        let mut decoder_frame = loop {
            match decoder_info.process(&mut input).map_err(jxl_err)? {
                ProcessingResult::Complete { result } => break result,
                ProcessingResult::NeedsMoreInput { fallback, .. } => decoder_info = fallback,
            }
        };

        // Decode pixels: WithFrameInfo → WithImageInfo
        let mut pixel_buf = vec![0u8; height * stride];
        decoder_info = loop {
            let out = JxlOutputBuffer::new(&mut pixel_buf, height, stride);
            match decoder_frame
                .process(&mut input, &mut [out])
                .map_err(jxl_err)?
            {
                ProcessingResult::Complete { result } => break result,
                ProcessingResult::NeedsMoreInput { fallback, .. } => decoder_frame = fallback,
            }
        };

        let duration_ms = decoder_info
            .scanned_frames()
            .last()
            .map(|f| f.duration_ms)
            .unwrap_or(100.0);
        let duration = Duration::from_millis((duration_ms.max(10.0)) as u64);

        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, pixel_buf)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| jxl_anim_err("buffer size mismatch"))?;

        frames.push((img, duration));

        if !decoder_info.has_more_frames() {
            break;
        }
    }

    if frames.is_empty() {
        return Err(LoadError::UnsupportedFormat);
    }

    Ok(AnimFrames { frames })
}

#[cfg(feature = "jxl-anim")]
fn jxl_anim_err(msg: &str) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("JXL-anim".to_owned()),
        msg.to_owned(),
    )))
}

#[cfg(feature = "jxl")]
fn jxl_err(e: jxl::error::Error) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("JXL".to_owned()),
        e.to_string(),
    )))
}
