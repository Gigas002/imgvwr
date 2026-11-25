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

    #[cfg(feature = "avif")]
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        == Some("avif")
    {
        return load_avif(path);
    }

    image::open(path).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        ImageError::Unsupported(_) => LoadError::UnsupportedFormat,
        other => LoadError::Decode(other),
    })
}

/// A [`jxl::api::JxlParallelRunner`] that spreads work across all available
/// CPU cores using a shared work-stealing counter.
#[cfg(feature = "jxl")]
struct JxlThreadPoolRunner {
    max_threads: usize,
}

#[cfg(feature = "jxl")]
impl Default for JxlThreadPoolRunner {
    fn default() -> Self {
        let max_threads = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        Self { max_threads }
    }
}

#[cfg(feature = "jxl")]
impl jxl::api::JxlParallelRunner for JxlThreadPoolRunner {
    fn num_threads(&self) -> usize {
        self.max_threads
    }

    fn run(
        &mut self,
        num: usize,
        fun: &jxl::api::JxlParallelRunnerFun<'_>,
    ) -> jxl::error::Result<()> {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let num_threads = self.max_threads.min(num);
        if num_threads <= 1 {
            for i in 0..num {
                fun(i)?;
            }
            return Ok(());
        }

        let next_task = AtomicUsize::new(0);
        let error = Mutex::new(None);

        std::thread::scope(|scope| {
            for _ in 0..num_threads {
                scope.spawn(|| {
                    loop {
                        if error.lock().unwrap().is_some() {
                            break;
                        }
                        let task = next_task.fetch_add(1, Ordering::Relaxed);
                        if task >= num {
                            break;
                        }
                        if let Err(e) = fun(task) {
                            error.lock().unwrap().get_or_insert(e);
                            break;
                        }
                    }
                });
            }
        });

        match error.into_inner().unwrap() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// Drive `decoder` until it emits an event other than `NeedMoreInput`.
///
/// The whole file is supplied up front, so a `NeedMoreInput` that consumes no
/// further bytes means the file is truncated.
#[cfg(feature = "jxl")]
fn jxl_next_event(
    decoder: &mut jxl::api::JxlDecoder,
    input: &mut &[u8],
    mut buffers: Option<&mut [jxl::api::JxlOutputBuffer<'_>]>,
    runner: &mut JxlThreadPoolRunner,
) -> Result<jxl::api::Event, LoadError> {
    use jxl::api::Event;

    loop {
        let remaining = input.len();
        match decoder
            .process(input, buffers.as_deref_mut(), Some(runner))
            .map_err(jxl_err)?
        {
            Event::NeedMoreInput { .. } if input.is_empty() || input.len() == remaining => {
                return Err(jxl_err(jxl::error::Error::OutOfBounds(0)));
            }
            Event::NeedMoreInput { .. } => {}
            event => return Ok(event),
        }
    }
}

#[cfg(feature = "jxl")]
fn load_jxl(path: &Path) -> Result<DynamicImage, LoadError> {
    use image::{ImageBuffer, Rgba};
    use jxl::api::{Event, JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat};

    let file_bytes = std::fs::read(path).map_err(LoadError::Io)?;
    let mut runner = JxlThreadPoolRunner::default();
    let mut decoder = JxlDecoder::new(JxlDecoderOptions::default());
    let mut input = file_bytes.as_slice();

    // Phase 1 — parse image header → get dimensions
    while !matches!(
        jxl_next_event(&mut decoder, &mut input, None, &mut runner)?,
        Event::BasicInfo
    ) {}

    let basic_info = decoder.basic_info().ok_or(LoadError::UnsupportedFormat)?;
    let (width, height) = basic_info.size;
    let stride = rgba_stride(width, height, MAX_IMAGE_BYTES)?;
    let num_extra = basic_info.extra_channels.len();
    decoder
        .set_pixel_format(JxlPixelFormat::rgba8(num_extra))
        .map_err(jxl_err)?;

    // Phase 2 — parse frame header
    loop {
        match jxl_next_event(&mut decoder, &mut input, None, &mut runner)? {
            Event::FrameHeader => break,
            Event::Complete => return Err(LoadError::UnsupportedFormat),
            _ => {}
        }
    }

    // Phase 3 — decode first frame into an RGBA u8 buffer
    let mut pixel_buf = vec![0u8; height * stride];
    loop {
        let mut out = [JxlOutputBuffer::new(&mut pixel_buf, height, stride)];
        if let Event::FrameComplete { .. } =
            jxl_next_event(&mut decoder, &mut input, Some(&mut out), &mut runner)?
        {
            break;
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

/// Most memory all decoded frames of one animation may take together.
///
/// Every frame is kept as full-canvas RGBA8, so a small file can describe
/// gigabytes: a 2 MB GIF of forty 8000×8000 frames decodes to 9.5 GiB.
/// Beyond this budget only the first frame is shown.
pub const MAX_ANIMATION_BYTES: usize = 1 << 30;

/// Largest single image the decoders may allocate; matches `image`'s
/// default allocation limit.
pub const MAX_IMAGE_BYTES: usize = 512 << 20;

/// Counts decoded frame bytes against [`MAX_ANIMATION_BYTES`].
#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
struct FrameBudget {
    used: usize,
    limit: usize,
}

#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
impl Default for FrameBudget {
    fn default() -> Self {
        Self::with_limit(MAX_ANIMATION_BYTES)
    }
}

#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
impl FrameBudget {
    fn with_limit(limit: usize) -> Self {
        Self { used: 0, limit }
    }

    /// Account for a `w × h` RGBA8 frame; false once over budget.
    fn admit(&mut self, w: u32, h: u32) -> bool {
        self.used = self
            .used
            .saturating_add((w as usize).saturating_mul(h as usize).saturating_mul(4));
        self.used <= self.limit
    }
}

/// Drop all frames but the first once an animation is over budget.
#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
fn first_frame_only(path: &Path, mut frames: Vec<(DynamicImage, Duration)>) -> AnimFrames {
    tracing::warn!(
        path = %path.display(),
        limit_mib = MAX_ANIMATION_BYTES >> 20,
        "animation too large to keep in memory; showing its first frame"
    );
    frames.truncate(1);
    AnimFrames { frames }
}

/// Decode the frames of an `image` animation within the frame budget.
#[cfg(any(feature = "gif", feature = "webp-anim", feature = "apng"))]
fn collect_budgeted(
    path: &Path,
    frames: image::Frames<'_>,
    mut budget: FrameBudget,
) -> Result<AnimFrames, LoadError> {
    let mut out = Vec::new();
    for frame in frames {
        let frame = frame.map_err(|e| match e {
            ImageError::IoError(io_err) => LoadError::Io(io_err),
            other => LoadError::Decode(other),
        })?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let ms = if denom == 0 {
            10
        } else {
            (numer as u64 / denom as u64).max(10)
        };
        let buffer = frame.into_buffer();
        let over = !budget.admit(buffer.width(), buffer.height());
        if over && !out.is_empty() {
            return Ok(first_frame_only(path, out));
        }
        out.push((DynamicImage::ImageRgba8(buffer), Duration::from_millis(ms)));
    }
    Ok(AnimFrames { frames: out })
}

/// An error for an image whose decoded pixels would exceed `limit` bytes.
#[cfg(feature = "jxl")]
fn too_large() -> LoadError {
    use image::error::{LimitError, LimitErrorKind};
    LoadError::Decode(ImageError::Limits(LimitError::from_kind(
        LimitErrorKind::InsufficientMemory,
    )))
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

    collect_budgeted(path, decoder.into_frames(), FrameBudget::default())
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

    collect_budgeted(path, decoder.into_frames(), FrameBudget::default())
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

    collect_budgeted(path, apng.into_frames(), FrameBudget::default())
}

/// Decoder settings shared by still and animated AVIF: 8-bit output (10- and
/// 12-bit files are scaled down) and at most [`MAX_IMAGE_BYTES`] per frame.
#[cfg(feature = "avif")]
fn avif_config() -> zenavif::DecoderConfig {
    zenavif::DecoderConfig::new()
        .prefer_8bit(true)
        .frame_size_limit((MAX_IMAGE_BYTES / 4) as u32)
}

/// Load a still AVIF (or the first frame of an animated one) with `zenavif`.
#[cfg(feature = "avif")]
fn load_avif(path: &Path) -> Result<DynamicImage, LoadError> {
    let data = std::fs::read(path).map_err(LoadError::Io)?;
    let pixels = zenavif::decode_with(&data, &avif_config(), &zenavif::Unstoppable)
        .map_err(|e| avif_err(e.error()))?;
    avif_to_rgba(&pixels)
}

/// Load an animated AVIF sequence from `path`, decoding one frame at a time
/// with `zenavif` so an over-budget animation stops early.
///
/// Returns `Err(LoadError::UnsupportedFormat)` for a still AVIF (an image item
/// rather than a sequence), so callers can fall back to `load()`.
#[cfg(feature = "avif-anim")]
pub fn load_avif_anim_frames(path: &Path) -> Result<AnimFrames, LoadError> {
    let data = std::fs::read(path).map_err(LoadError::Io)?;
    let mut decoder =
        zenavif::AnimationDecoder::new(&data, &avif_config()).map_err(|e| avif_err(e.error()))?;

    let mut frames = Vec::with_capacity(decoder.info().frame_count);
    let mut budget = FrameBudget::default();
    while let Some(frame) = decoder
        .next_frame(&zenavif::Unstoppable)
        .map_err(|e| avif_err(e.error()))?
    {
        if !budget.admit(frame.pixels.width(), frame.pixels.height()) && !frames.is_empty() {
            return Ok(first_frame_only(path, frames));
        }
        let ms = u64::from(frame.duration_ms).max(10);
        frames.push((avif_to_rgba(&frame.pixels)?, Duration::from_millis(ms)));
    }

    if frames.is_empty() {
        return Err(LoadError::UnsupportedFormat);
    }
    Ok(AnimFrames { frames })
}

/// Any decoded AVIF buffer (gray, RGB, RGBA, 8 or 16 bit) as RGBA8.
#[cfg(feature = "avif")]
fn avif_to_rgba(pixels: &zenavif::PixelBuffer) -> Result<DynamicImage, LoadError> {
    use zenpixels_convert::PixelBufferConvertTypedExt;

    let rgba = pixels.to_rgba8();
    image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
        rgba.width(),
        rgba.height(),
        rgba.copy_to_contiguous_bytes(),
    )
    .map(DynamicImage::ImageRgba8)
    .ok_or_else(|| avif_decode_err("buffer size mismatch".into()))
}

#[cfg(feature = "avif")]
fn avif_err(e: &zenavif::Error) -> LoadError {
    use image::error::{LimitError, LimitErrorKind};
    match e {
        zenavif::Error::Unsupported(_) => LoadError::UnsupportedFormat,
        zenavif::Error::ImageTooLarge { .. } => LoadError::Decode(ImageError::Limits(
            LimitError::from_kind(LimitErrorKind::InsufficientMemory),
        )),
        other => avif_decode_err(other.to_string()),
    }
}

#[cfg(feature = "avif")]
fn avif_decode_err(msg: String) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("AVIF".to_owned()),
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
    use jxl::api::{Event, JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat};

    let file_bytes = std::fs::read(path).map_err(LoadError::Io)?;
    let mut runner = JxlThreadPoolRunner::default();
    let mut decoder = JxlDecoder::new(JxlDecoderOptions::default());
    let mut input = file_bytes.as_slice();

    while !matches!(
        jxl_next_event(&mut decoder, &mut input, None, &mut runner)?,
        Event::BasicInfo
    ) {}

    let basic_info = decoder.basic_info().ok_or(LoadError::UnsupportedFormat)?;
    if basic_info.animation.is_none() {
        return Err(LoadError::UnsupportedFormat);
    }

    let (width, height) = basic_info.size;
    let stride = rgba_stride(width, height, MAX_IMAGE_BYTES)?;
    // Fold all extra channels (e.g. separate alpha) into the RGBA output by
    // marking them as None (no separate buffer).
    let num_extra = basic_info.extra_channels.len();
    decoder
        .set_pixel_format(JxlPixelFormat::rgba8(num_extra))
        .map_err(jxl_err)?;
    let mut frames = Vec::new();
    let mut budget = FrameBudget::default();

    loop {
        // Parse frame header
        match jxl_next_event(&mut decoder, &mut input, None, &mut runner)? {
            Event::FrameHeader => {}
            Event::Complete => break,
            _ => continue,
        }

        let duration_ms = decoder
            .frame_header()
            .and_then(|h| h.duration)
            .unwrap_or(100.0);
        let duration = Duration::from_millis((duration_ms.max(10.0)) as u64);

        if !budget.admit(width as u32, height as u32) && !frames.is_empty() {
            return Ok(first_frame_only(path, frames));
        }

        // Decode pixels
        let mut pixel_buf = vec![0u8; height * stride];
        let has_more_frames = loop {
            let mut out = [JxlOutputBuffer::new(&mut pixel_buf, height, stride)];
            if let Event::FrameComplete { has_more_frames } =
                jxl_next_event(&mut decoder, &mut input, Some(&mut out), &mut runner)?
            {
                break has_more_frames;
            }
        };

        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, pixel_buf)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| jxl_anim_err("buffer size mismatch"))?;

        frames.push((img, duration));

        if !has_more_frames {
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

/// Row length of a `width × height` RGBA8 buffer, refusing sizes over
/// `limit` bytes before anything is allocated (dimensions come straight
/// from the file header).
#[cfg(feature = "jxl")]
fn rgba_stride(width: usize, height: usize, limit: usize) -> Result<usize, LoadError> {
    let stride = width.checked_mul(4).ok_or_else(too_large)?;
    match stride.checked_mul(height) {
        Some(total) if total <= limit => Ok(stride),
        _ => Err(too_large()),
    }
}

#[cfg(feature = "jxl")]
fn jxl_err(e: jxl::error::Error) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("JXL".to_owned()),
        e.to_string(),
    )))
}
