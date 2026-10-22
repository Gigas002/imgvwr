// GPU-accelerated rendering pipeline via vello on top of wgpu (Vulkan
// preferred, GL/EGL fallback). Compiled only when `gpu-vulkan` or `gpu-gles`
// feature is enabled.
//
// Each frame builds a vello `Scene` containing a single image fill whose
// affine transform encodes scale, rotation and pan. vello rasterises the scene
// into an `Rgba8Unorm` storage texture, which is either read back for the SHM
// path or blitted onto the swapchain for the dmabuf path.
//
// vello samples images with at most bicubic filtering and without mipmaps, so
// strong minification would alias. To avoid that, the source is kept as a
// chain of pre-filtered half-size levels (built once per image with the
// configured filter on the CPU) and the level closest to the target scale is
// handed to vello. Levels that exceed vello's image atlas are cropped to the
// visible region every frame instead.

use std::sync::Arc;

use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage, imageops};
use vello::kurbo::{Affine, Point, Rect};
use vello::peniko::{
    Blob, Color, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality,
};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

use super::FilterMethod;
use crate::viewport::ViewportState;

/// Largest image side vello can place in its image atlas.
pub(crate) const MAX_ATLAS_SIDE: u32 = 8192;

/// Extra source pixels kept around a visible crop so filter taps at the
/// window edge read real neighbours instead of clamped edge pixels.
const CROP_MARGIN: f64 = 2.0;

/// Errors produced during GPU initialisation.
#[derive(Debug)]
pub enum GpuError {
    NoAdapter,
    DeviceError(wgpu::RequestDeviceError),
    Renderer(vello::Error),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no suitable GPU adapter found"),
            GpuError::DeviceError(e) => write!(f, "GPU device error: {e}"),
            GpuError::Renderer(e) => write!(f, "vello renderer error: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<wgpu::RequestDeviceError> for GpuError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        GpuError::DeviceError(e)
    }
}

impl From<vello::Error> for GpuError {
    fn from(e: vello::Error) -> Self {
        GpuError::Renderer(e)
    }
}

// ─── Geometry ────────────────────────────────────────────────────────────────

/// Map a [`FilterMethod`] onto vello's image sampling quality.
pub(crate) fn image_quality(filter: FilterMethod) -> ImageQuality {
    match filter {
        FilterMethod::Nearest => ImageQuality::Low,
        FilterMethod::Triangle | FilterMethod::Gaussian => ImageQuality::Medium,
        FilterMethod::CatmullRom | FilterMethod::Lanczos3 => ImageQuality::High,
    }
}

/// Pick the pre-scaled level for `scale`: the smallest level that is still at
/// least as large as the on-screen image, so vello only ever minifies by less
/// than 2×. Level `n` is the source halved `n` times.
pub(crate) fn mip_level_for_scale(scale: f32) -> u32 {
    if scale >= 1.0 || scale <= 0.0 {
        return 0;
    }
    (1.0 / scale as f64).log2().floor() as u32
}

/// Dimensions of level `n` for a `width × height` source.
pub(crate) fn level_size(width: u32, height: u32, level: u32) -> (u32, u32) {
    let shrink = |v: u32| (v >> level.min(31)).max(1);
    (shrink(width), shrink(height))
}

/// Transform from source-image pixel space to window pixel space.
///
/// The image is scaled by `viewport.scale`, rotated clockwise by
/// `viewport.rotation` about its centre, centred in the window and shifted by
/// `viewport.offset`. The top-left corner of the rotated image is snapped to
/// the pixel grid so that 1:1 views stay sharp.
pub(crate) fn image_transform(
    src_w: u32,
    src_h: u32,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
) -> Affine {
    let scale = viewport.scale as f64;
    let (w, h) = (src_w as f64 * scale, src_h as f64 * scale);
    let (rot_w, rot_h) = match viewport.rotation % 360 {
        90 | 270 => (h, w),
        _ => (w, h),
    };
    let left = (dst_w as f64 / 2.0 + viewport.offset.0 as f64 - rot_w / 2.0).round();
    let top = (dst_h as f64 / 2.0 + viewport.offset.1 as f64 - rot_h / 2.0).round();

    Affine::translate((left + rot_w / 2.0, top + rot_h / 2.0))
        * Affine::rotate((viewport.rotation as f64).to_radians())
        * Affine::scale(scale)
        * Affine::translate((-(src_w as f64) / 2.0, -(src_h as f64) / 2.0))
}

/// Integer pixel rectangle `(x, y, w, h)` inside an image.
pub(crate) type CropRect = (u32, u32, u32, u32);

/// Region of an `img_w × img_h` image that lands inside the `dst_w × dst_h`
/// window under `transform`, padded by [`CROP_MARGIN`].
///
/// Returns `None` when the image is entirely off-screen.
pub(crate) fn visible_crop(
    transform: Affine,
    img_w: u32,
    img_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Option<CropRect> {
    let inv = transform.inverse();
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(dst_w as f64, 0.0),
        Point::new(0.0, dst_h as f64),
        Point::new(dst_w as f64, dst_h as f64),
    ]
    .map(|p| inv * p);
    let bounds = corners
        .iter()
        .skip(1)
        .fold(Rect::from_points(corners[0], corners[0]), |r, &p| {
            r.union_pt(p)
        })
        .inflate(CROP_MARGIN, CROP_MARGIN)
        .intersect(Rect::new(0.0, 0.0, img_w as f64, img_h as f64));

    let x0 = bounds.x0.floor().max(0.0) as u32;
    let y0 = bounds.y0.floor().max(0.0) as u32;
    let x1 = (bounds.x1.ceil() as u32).min(img_w);
    let y1 = (bounds.y1.ceil() as u32).min(img_h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0, y0, x1 - x0, y1 - y0))
}

// ─── Source cache ────────────────────────────────────────────────────────────

/// Identity of the source image a [`SourceCache`] was built from.
///
/// Images are recognised by the address and size of their pixel buffer: the
/// caller keeps the displayed image alive until its successor is loaded, so a
/// new image can never reuse the cached one's buffer. Mutating an image in
/// place between frames is not detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceKey {
    ptr: usize,
    len: usize,
    width: u32,
    height: u32,
    filter: FilterMethod,
}

impl SourceKey {
    fn new(src: &DynamicImage, filter: FilterMethod) -> Self {
        let bytes = src.as_bytes();
        Self {
            ptr: bytes.as_ptr() as usize,
            len: bytes.len(),
            width: src.width(),
            height: src.height(),
            filter,
        }
    }
}

/// Pre-scaled RGBA8 copies of the current source image.
///
/// Level 0 is the full-resolution image; each further level halves both sides
/// using the configured filter. Levels are built lazily and live as vello
/// [`ImageData`] so that unchanged levels are uploaded to the atlas only once.
#[derive(Default)]
struct SourceCache {
    key: Option<SourceKey>,
    levels: Vec<ImageData>,
    /// Last visible crop handed to vello, reused while the view is unchanged.
    crop: Option<(u32, CropRect, ImageData)>,
}

impl SourceCache {
    fn prepare(&mut self, src: &DynamicImage, filter: FilterMethod) {
        let key = SourceKey::new(src, filter);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.levels = vec![rgba_image_data(src.to_rgba8())];
        self.crop = None;
    }

    fn level(&mut self, level: u32, filter: FilterMethod) -> &ImageData {
        while self.levels.len() <= level as usize {
            let prev = self.levels.last().expect("level 0 is always present");
            let (w, h) = (
                prev.width.div_ceil(2).max(1),
                prev.height.div_ceil(2).max(1),
            );
            let next = imageops::resize(&image_view(prev), w, h, filter.into());
            self.levels.push(rgba_image_data(next));
        }
        &self.levels[level as usize]
    }

    fn crop(&mut self, level: u32, rect: CropRect) -> &ImageData {
        let fresh = matches!(&self.crop, Some((l, r, _)) if *l == level && *r == rect);
        if !fresh {
            let cropped = crop_rgba(&self.levels[level as usize], rect);
            self.crop = Some((level, rect, rgba_image_data(cropped)));
        }
        &self.crop.as_ref().expect("crop was just set").2
    }
}

fn rgba_image_data(img: RgbaImage) -> ImageData {
    let (width, height) = img.dimensions();
    ImageData {
        data: Blob::new(Arc::new(img.into_raw())),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    }
}

/// Copy the `rect` region out of RGBA8 `data`.
fn crop_rgba(data: &ImageData, (x, y, w, h): CropRect) -> RgbaImage {
    let stride = data.width as usize * 4;
    let bytes = data.data.data();
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for row in y..y + h {
        let start = row as usize * stride + x as usize * 4;
        out.extend_from_slice(&bytes[start..start + w as usize * 4]);
    }
    RgbaImage::from_raw(w, h, out).expect("crop buffer matches its dimensions")
}

fn image_view(data: &ImageData) -> ImageBuffer<Rgba<u8>, &[u8]> {
    ImageBuffer::from_raw(data.width, data.height, data.data.data())
        .expect("image data matches its dimensions")
}

// ─── GpuContext ───────────────────────────────────────────────────────────────

/// Owns the wgpu device and queue, the vello renderer, and all per-image
/// caches for the lifetime of the application.
pub struct GpuContext {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    renderer: Renderer,
    scene: Scene,
    source: SourceCache,
    /// vello render target, recreated whenever the window size changes.
    target: Option<wgpu::Texture>,
    /// Present swapchain state. `None` when running the SHM/readback path.
    #[cfg(feature = "dmabuf")]
    pub(crate) swapchain: Option<SwapchainState>,
}

fn create_renderer(device: &wgpu::Device) -> Result<Renderer, GpuError> {
    Ok(Renderer::new(
        device,
        RendererOptions {
            antialiasing_support: AaSupport::area_only(),
            ..Default::default()
        },
    )?)
}

impl GpuContext {
    /// Initialise a GPU context. Returns `Err` if no suitable adapter is found;
    /// the caller should treat this as a fatal error and exit.
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(async {
            let mut backends = wgpu::Backends::empty();
            #[cfg(feature = "gpu-vulkan")]
            {
                backends |= wgpu::Backends::VULKAN;
            }
            #[cfg(feature = "gpu-gles")]
            {
                backends |= wgpu::Backends::GL;
            }

            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|_| GpuError::NoAdapter)?;

            let info = adapter.get_info();
            tracing::info!(
                backend = ?info.backend,
                adapter = %info.name,
                "GPU adapter selected"
            );

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await?;

            let renderer = create_renderer(&device)?;
            Ok(Self {
                device,
                queue,
                renderer,
                scene: Scene::new(),
                source: SourceCache::default(),
                target: None,
                #[cfg(feature = "dmabuf")]
                swapchain: None,
            })
        })
    }

    /// Render `src` for `viewport` into a `dst_w × dst_h` `Rgba8Unorm` texture
    /// owned by this context. Pixels not covered by the image are filled with
    /// `background`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_to_target(
        &mut self,
        src: &DynamicImage,
        viewport: &ViewportState,
        dst_w: u32,
        dst_h: u32,
        filter: FilterMethod,
        background: Color,
    ) -> Result<(), vello::Error> {
        self.build_scene(src, viewport, dst_w, dst_h, filter);

        let stale = self
            .target
            .as_ref()
            .is_none_or(|t| t.width() != dst_w || t.height() != dst_h);
        if stale {
            self.target = Some(make_target(&self.device, dst_w, dst_h));
        }
        let target = self.target.as_ref().expect("target was just created");
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        self.renderer.render_to_texture(
            &self.device,
            &self.queue,
            &self.scene,
            &view,
            &RenderParams {
                base_color: background,
                width: dst_w,
                height: dst_h,
                antialiasing_method: AaConfig::Area,
            },
        )
    }

    /// Read the last rendered frame back as Wayland ARGB8888 bytes.
    pub(crate) fn read_target(&self) -> Vec<u8> {
        let target = self.target.as_ref().expect("render_to_target ran first");
        readback(
            &self.device,
            &self.queue,
            target,
            target.width(),
            target.height(),
        )
    }

    /// Encode the image draw for this frame into `self.scene`.
    fn build_scene(
        &mut self,
        src: &DynamicImage,
        viewport: &ViewportState,
        dst_w: u32,
        dst_h: u32,
        filter: FilterMethod,
    ) {
        self.scene.reset();
        self.source.prepare(src, filter);

        let to_window = image_transform(src.width(), src.height(), viewport, dst_w, dst_h);
        let mut level = mip_level_for_scale(viewport.scale);

        loop {
            let (lw, lh) = level_size(src.width(), src.height(), level);
            // Level pixels → source pixels → window pixels.
            let to_window = to_window
                * Affine::scale_non_uniform(
                    src.width() as f64 / lw as f64,
                    src.height() as f64 / lh as f64,
                );

            let fits_atlas = lw <= MAX_ATLAS_SIDE && lh <= MAX_ATLAS_SIDE;
            let image = self.source.level(level, filter);
            debug_assert_eq!((image.width, image.height), (lw, lh));

            let (image, transform) = if fits_atlas {
                (image, to_window)
            } else {
                let Some(rect) = visible_crop(to_window, lw, lh, dst_w, dst_h) else {
                    return;
                };
                if rect.2 > MAX_ATLAS_SIDE || rect.3 > MAX_ATLAS_SIDE {
                    // Even the visible part is too big: fall back to a smaller level.
                    level += 1;
                    continue;
                }
                let crop = self.source.crop(level, rect);
                (
                    crop,
                    to_window * Affine::translate((rect.0 as f64, rect.1 as f64)),
                )
            };

            let brush = ImageBrush::new(image.clone()).with_quality(image_quality(filter));
            self.scene.draw_image(&brush, transform);
            return;
        }
    }
}

fn make_target(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vello_target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

// ─── Readback ────────────────────────────────────────────────────────────────

/// Read back a GPU texture as a Wayland-compatible ARGB8888 byte buffer.
///
/// Copies `tex` into a CPU-visible staging buffer, maps it synchronously,
/// strips wgpu's row-alignment padding, and byte-swaps RGBA → `[B, G, R, A]`
/// (little-endian ARGB8888, matching `wl_shm::Format::Argb8888`).
fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    w: u32,
    h: u32,
) -> Vec<u8> {
    const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let row_bytes = w * 4;
    let padded_row = row_bytes.div_ceil(ALIGN) * ALIGN;
    let buf_size = (padded_row * h) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buf_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let (sender, receiver) = std::sync::mpsc::channel();
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
    receiver.recv().unwrap().unwrap();

    let raw = slice.get_mapped_range().expect("mapped staging buffer");
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded_row) as usize;
        let row_data = &raw[start..start + row_bytes as usize];
        for chunk in row_data.as_chunks::<4>().0 {
            // RGBA → little-endian ARGB8888: [B, G, R, A]
            out.push(chunk[2]);
            out.push(chunk[1]);
            out.push(chunk[0]);
            out.push(chunk[3]);
        }
    }
    drop(raw);
    staging.unmap();
    out
}

// ─── DMA-BUF / swapchain path ─────────────────────────────────────────────────

/// Errors produced by [`GpuContext::render_and_present`].
#[cfg(feature = "dmabuf")]
#[derive(Debug)]
pub enum PresentError {
    /// wgpu surface creation failed during init.
    CreateSurface(wgpu::CreateSurfaceError),
    /// Swapchain texture acquisition failed with a non-recoverable status.
    Unavailable(&'static str),
    /// vello failed to render the scene.
    Render(vello::Error),
}

#[cfg(feature = "dmabuf")]
impl std::fmt::Display for PresentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresentError::CreateSurface(e) => write!(f, "surface creation error: {e}"),
            PresentError::Unavailable(reason) => write!(f, "swapchain unavailable: {reason}"),
            PresentError::Render(e) => write!(f, "render error: {e}"),
        }
    }
}

#[cfg(feature = "dmabuf")]
impl std::error::Error for PresentError {}

/// Per-frame swapchain state, stored in [`GpuContext`] when `dmabuf` is active.
#[cfg(feature = "dmabuf")]
pub(crate) struct SwapchainState {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    /// Copies the vello target onto the swapchain texture. vello renders with
    /// compute shaders into a storage texture, which swapchain images usually
    /// do not support directly.
    pub blitter: wgpu::util::TextureBlitter,
}

#[cfg(feature = "dmabuf")]
impl GpuContext {
    /// Initialise a GPU context backed by a Wayland swapchain.
    ///
    /// `display_ptr` is the raw `*mut wl_display` and `surface_ptr` is the raw
    /// `*mut wl_surface` obtained from [`crate::wayland::WaylandContext::display_ptr`] /
    /// [`crate::wayland::WaylandContext::surface_ptr`]. Both must remain valid for the lifetime
    /// of this `GpuContext`.
    ///
    /// Returns `Err` if no suitable adapter or the swapchain cannot be
    /// configured; the caller should treat this as fatal.
    pub fn new_with_surface(
        display_ptr: std::ptr::NonNull<std::ffi::c_void>,
        surface_ptr: std::ptr::NonNull<std::ffi::c_void>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };

        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });

            let display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_ptr));
            let window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr));

            // SAFETY: caller guarantees the Wayland handles remain valid.
            let surface = unsafe {
                instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(display_handle),
                        raw_window_handle: window_handle,
                    })
                    .map_err(|_| GpuError::NoAdapter)?
            };

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|_| GpuError::NoAdapter)?;

            let info = adapter.get_info();
            tracing::info!(
                backend = ?info.backend,
                adapter = %info.name,
                "GPU adapter selected (dmabuf/swapchain)"
            );

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await?;

            let caps = surface.get_capabilities(&adapter);
            let surface_format = caps
                .formats
                .iter()
                .copied()
                .find(|f| {
                    matches!(
                        f,
                        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                    )
                })
                .unwrap_or(caps.formats[0]);

            tracing::info!(format = ?surface_format, "swapchain format selected");

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
            };
            surface.configure(&device, &config);

            let renderer = create_renderer(&device)?;
            let blitter = wgpu::util::TextureBlitter::new(&device, surface_format);

            Ok(Self {
                device,
                queue,
                renderer,
                scene: Scene::new(),
                source: SourceCache::default(),
                target: None,
                swapchain: Some(SwapchainState {
                    surface,
                    config,
                    blitter,
                }),
            })
        })
    }

    /// Reconfigure the swapchain after a window resize.
    pub fn configure_surface(&mut self, width: u32, height: u32) {
        if let Some(sc) = &mut self.swapchain {
            sc.config.width = width;
            sc.config.height = height;
            sc.surface.configure(&self.device, &sc.config);
        }
    }

    /// Render `src` for the given viewport and present it to the Wayland surface
    /// via the wgpu swapchain (zero PCIe readback).
    ///
    /// On `Outdated` / `Lost` the swapchain is reconfigured and the frame is
    /// skipped. All other errors are returned to the caller.
    pub fn render_and_present(
        &mut self,
        src: &image::DynamicImage,
        viewport: &crate::viewport::ViewportState,
        dst_w: u32,
        dst_h: u32,
        filter: super::FilterMethod,
    ) -> Result<(), PresentError> {
        // Acquire swapchain frame. Inner block ensures `sc` is dropped before
        // any `&mut self` call (configure_surface).
        let frame = {
            let (result, dims) = {
                let sc = self.swapchain.as_ref().expect("swapchain not initialised");
                (
                    sc.surface.get_current_texture(),
                    (sc.config.width, sc.config.height),
                )
            };
            match result {
                wgpu::CurrentSurfaceTexture::Success(f)
                | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.configure_surface(dims.0, dims.1);
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(PresentError::Unavailable("validation error"));
                }
            }
        };

        self.render_to_target(src, viewport, dst_w, dst_h, filter, Color::BLACK)
            .map_err(PresentError::Render)?;
        let target_view = self
            .target
            .as_ref()
            .expect("target was just rendered")
            .create_view(&wgpu::TextureViewDescriptor::default());
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let sc = self.swapchain.as_ref().expect("swapchain not initialised");
        sc.blitter
            .copy(&self.device, &mut encoder, &target_view, &frame_view);

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}
