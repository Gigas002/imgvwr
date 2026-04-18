// GPU-accelerated rendering pipeline via wgpu (Vulkan preferred, GL/EGL fallback).
// Compiled only when `gpu-vulkan` or `gpu-gles` feature is enabled.
//
// All render pipelines and bind group layouts are compiled once at GpuContext
// initialisation; per-frame work is limited to texture upload, uniform buffer
// writes, bind group creation, and draw calls.

use super::FilterMethod;
use image::DynamicImage;

/// Errors produced during GPU initialisation.
#[derive(Debug)]
pub enum GpuError {
    NoAdapter,
    DeviceError(wgpu::RequestDeviceError),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no suitable GPU adapter found"),
            GpuError::DeviceError(e) => write!(f, "GPU device error: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<wgpu::RequestDeviceError> for GpuError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        GpuError::DeviceError(e)
    }
}

// ─── Pipeline cache ──────────────────────────────────────────────────────────

/// All render pipelines compiled once at init.
///
/// Two bind group layout shapes are used:
/// - `sampler_bgl`:  binding 0 = texture (filterable float), binding 1 = sampler
/// - `uniform_bgl`:  binding 0 = texture (non-filterable float), binding 1 = uniform buffer
pub(crate) struct GpuPipelines {
    pub sampler_bgl: wgpu::BindGroupLayout,
    pub uniform_bgl: wgpu::BindGroupLayout,

    pub blit: wgpu::RenderPipeline, // blit.wgsl         → Rgba8Unorm
    pub lanczos3_h: wgpu::RenderPipeline, // lanczos3 horiz     → Rgba16Float
    pub lanczos3_v: wgpu::RenderPipeline, // lanczos3 vert      → Rgba8Unorm
    pub catmull_h: wgpu::RenderPipeline, // catmull_rom horiz  → Rgba16Float
    pub catmull_v: wgpu::RenderPipeline, // catmull_rom vert   → Rgba8Unorm
    pub rotate: wgpu::RenderPipeline, // rotate.wgsl        → Rgba8Unorm
}

impl GpuPipelines {
    fn new(device: &wgpu::Device) -> Self {
        let sampler_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sampler_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let blit = make_pipeline(
            device,
            &sampler_bgl,
            include_str!("shaders/blit.wgsl"),
            "vs_main",
            "fs_main",
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let lanczos3_h = make_pipeline(
            device,
            &uniform_bgl,
            include_str!("shaders/lanczos3.wgsl"),
            "vs_main",
            "fs_horizontal",
            wgpu::TextureFormat::Rgba16Float,
        );
        let lanczos3_v = make_pipeline(
            device,
            &uniform_bgl,
            include_str!("shaders/lanczos3.wgsl"),
            "vs_main",
            "fs_vertical",
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let catmull_h = make_pipeline(
            device,
            &uniform_bgl,
            include_str!("shaders/catmull_rom.wgsl"),
            "vs_main",
            "fs_horizontal",
            wgpu::TextureFormat::Rgba16Float,
        );
        let catmull_v = make_pipeline(
            device,
            &uniform_bgl,
            include_str!("shaders/catmull_rom.wgsl"),
            "vs_main",
            "fs_vertical",
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let rotate = make_pipeline(
            device,
            &uniform_bgl,
            include_str!("shaders/rotate.wgsl"),
            "vs_main",
            "fs_main",
            wgpu::TextureFormat::Rgba8Unorm,
        );

        Self {
            sampler_bgl,
            uniform_bgl,
            blit,
            lanczos3_h,
            lanczos3_v,
            catmull_h,
            catmull_v,
            rotate,
        }
    }
}

fn make_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    shader_src: &str,
    vs_entry: &str,
    fs_entry: &str,
    out_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(vs_entry),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fs_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format: out_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

// ─── GpuContext ───────────────────────────────────────────────────────────────

/// Owns the wgpu device, queue, and all compiled pipelines for the lifetime
/// of the application.
pub struct GpuContext {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) pipelines: GpuPipelines,
    /// Present swapchain state. `None` when running the SHM/readback path.
    #[cfg(feature = "dmabuf")]
    pub(crate) swapchain: Option<SwapchainState>,
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

            let pipelines = GpuPipelines::new(&device);
            Ok(Self {
                device,
                queue,
                pipelines,
                #[cfg(feature = "dmabuf")]
                swapchain: None,
            })
        })
    }
}

// ─── Per-frame GPU operations ─────────────────────────────────────────────────

/// Upload a [`DynamicImage`] to a GPU texture (`Rgba8Unorm`).
pub(crate) fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &DynamicImage,
) -> wgpu::Texture {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let texture = make_texture(
        device,
        width,
        height,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    texture
}

/// Resize `src` to `(dst_w, dst_h)` then optionally rotate by `rotation` degrees.
///
/// `rotation` must be a multiple of 90; any other value is treated as 0.
/// Dispatches resize to the appropriate GPU path:
/// - `Lanczos3` / `CatmullRom` → two-pass separable kernel
/// - All others → sampler-based blit (nearest or bilinear)
pub(crate) fn resize_blit(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
    rotation: u16,
) -> wgpu::Texture {
    let resized = match filter {
        FilterMethod::Lanczos3 => resize_kernel_two_pass(
            ctx,
            src,
            dst_w,
            dst_h,
            &ctx.pipelines.lanczos3_h,
            &ctx.pipelines.lanczos3_v,
        ),
        FilterMethod::CatmullRom => resize_kernel_two_pass(
            ctx,
            src,
            dst_w,
            dst_h,
            &ctx.pipelines.catmull_h,
            &ctx.pipelines.catmull_v,
        ),
        _ => resize_sampler(ctx, src, dst_w, dst_h, filter),
    };

    if rotation.is_multiple_of(360) {
        resized
    } else {
        rotate_texture(ctx, &resized, rotation)
    }
}

/// Read back a GPU texture as a Wayland-compatible ARGB8888 byte buffer.
///
/// Copies `tex` into a CPU-visible staging buffer, maps it synchronously,
/// strips wgpu's row-alignment padding, and byte-swaps RGBA → `[B, G, R, A]`
/// (little-endian ARGB8888, matching `wl_shm::Format::Argb8888`).
pub(crate) fn readback(ctx: &GpuContext, tex: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
    const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let row_bytes = w * 4;
    let padded_row = row_bytes.div_ceil(ALIGN) * ALIGN;
    let buf_size = (padded_row * h) as u64;

    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buf_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let (sender, receiver) = std::sync::mpsc::channel();
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    ctx.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
    receiver.recv().unwrap().unwrap();

    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded_row) as usize;
        let row_data = &raw[start..start + row_bytes as usize];
        for chunk in row_data.chunks_exact(4) {
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

/// Rotate `src` by `rotation` degrees (must be 90, 180, or 270; others → identity).
///
/// For 90° and 270° the output texture dimensions are swapped relative to `src`.
/// Uses a pixel-exact `textureLoad` shader — no sampler blur.
pub(crate) fn rotate_texture(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    rotation: u16,
) -> wgpu::Texture {
    let src_w = src.width();
    let src_h = src.height();

    let rot_code = match rotation % 360 {
        90 => 1u32,
        180 => 2u32,
        270 => 3u32,
        _ => 0u32,
    };
    let (out_w, out_h) = if rot_code == 1 || rot_code == 3 {
        (src_h, src_w)
    } else {
        (src_w, src_h)
    };

    let dst = make_texture(
        &ctx.device,
        out_w,
        out_h,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );

    let mut ub = [0u8; 16];
    ub[0..4].copy_from_slice(&src_w.to_ne_bytes());
    ub[4..8].copy_from_slice(&src_h.to_ne_bytes());
    ub[8..12].copy_from_slice(&rot_code.to_ne_bytes());

    let bind_group = uniform_bind_group(ctx, src, &ub);
    let dst_view = dst.create_view(&wgpu::TextureViewDescriptor::default());
    draw_fullscreen(ctx, &ctx.pipelines.rotate, &bind_group, &dst_view);
    dst
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn resize_sampler(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
) -> wgpu::Texture {
    let filter_mode = match filter {
        FilterMethod::Nearest => wgpu::FilterMode::Nearest,
        _ => wgpu::FilterMode::Linear,
    };

    let dst = make_texture(
        &ctx.device,
        dst_w,
        dst_h,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );

    let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: filter_mode,
        min_filter: filter_mode,
        ..Default::default()
    });
    let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
    let dst_view = dst.create_view(&wgpu::TextureViewDescriptor::default());

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &ctx.pipelines.sampler_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    draw_fullscreen(ctx, &ctx.pipelines.blit, &bind_group, &dst_view);
    dst
}

/// Two-pass separable kernel resize using pre-compiled pipelines.
///
/// Pass 1: `(src_w, src_h)` → `(dst_w, src_h)` in `Rgba16Float`.
/// Pass 2: `(dst_w, src_h)` → `(dst_w, dst_h)` in `Rgba8Unorm`.
fn resize_kernel_two_pass(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32,
    dst_h: u32,
    h_pipeline: &wgpu::RenderPipeline,
    v_pipeline: &wgpu::RenderPipeline,
) -> wgpu::Texture {
    let src_w = src.width();
    let src_h = src.height();

    let intermediate = run_kernel_pass(
        ctx,
        src,
        src_w,
        src_h,
        dst_w,
        src_h,
        wgpu::TextureFormat::Rgba16Float,
        h_pipeline,
    );
    run_kernel_pass(
        ctx,
        &intermediate,
        dst_w,
        src_h,
        dst_w,
        dst_h,
        wgpu::TextureFormat::Rgba8Unorm,
        v_pipeline,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_kernel_pass(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    out_format: wgpu::TextureFormat,
    pipeline: &wgpu::RenderPipeline,
) -> wgpu::Texture {
    let dst = make_texture(
        &ctx.device,
        dst_w,
        dst_h,
        out_format,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );

    let mut ub = [0u8; 16];
    ub[0..4].copy_from_slice(&src_w.to_ne_bytes());
    ub[4..8].copy_from_slice(&src_h.to_ne_bytes());
    ub[8..12].copy_from_slice(&dst_w.to_ne_bytes());
    ub[12..16].copy_from_slice(&dst_h.to_ne_bytes());

    let bind_group = uniform_bind_group(ctx, src, &ub);
    let dst_view = dst.create_view(&wgpu::TextureViewDescriptor::default());
    draw_fullscreen(ctx, pipeline, &bind_group, &dst_view);
    dst
}

/// Create a bind group for shaders that take a texture + 16-byte uniform buffer.
fn uniform_bind_group(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    uniform_data: &[u8; 16],
) -> wgpu::BindGroup {
    let uniform_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&uniform_buf, 0, uniform_data);

    let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &ctx.pipelines.uniform_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buf,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    })
}

/// Submit a single full-screen quad draw into `dst_view`.
fn draw_fullscreen(
    ctx: &GpuContext,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    dst_view: &wgpu::TextureView,
) {
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, bind_group, &[]);
        rpass.draw(0..4, 0..1);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
}

fn make_texture(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
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
}

#[cfg(feature = "dmabuf")]
impl std::fmt::Display for PresentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresentError::CreateSurface(e) => write!(f, "surface creation error: {e}"),
            PresentError::Unavailable(reason) => write!(f, "swapchain unavailable: {reason}"),
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
    /// Blit pipeline whose output format matches the swapchain format.
    pub present_pipeline: wgpu::RenderPipeline,
    /// Bind-group layout for the present blit (filterable texture + sampler).
    pub present_bgl: wgpu::BindGroupLayout,
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
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
            };
            surface.configure(&device, &config);

            let pipelines = GpuPipelines::new(&device);

            let present_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("present_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
            let present_pipeline = make_pipeline(
                &device,
                &present_bgl,
                include_str!("shaders/blit.wgsl"),
                "vs_main",
                "fs_main",
                surface_format,
            );

            let swapchain = SwapchainState {
                surface,
                config,
                present_pipeline,
                present_bgl,
            };

            Ok(Self {
                device,
                queue,
                pipelines,
                swapchain: Some(swapchain),
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
    /// On `wgpu::SurfaceError::Outdated` the swapchain is reconfigured and the
    /// frame is skipped. All other errors are returned to the caller.
    pub fn render_and_present(
        &mut self,
        src: &image::DynamicImage,
        viewport: &crate::viewport::ViewportState,
        dst_w: u32,
        dst_h: u32,
        filter: super::FilterMethod,
    ) -> Result<(), PresentError> {
        let scaled_w = ((src.width() as f32) * viewport.scale).max(1.0) as u32;
        let scaled_h = ((src.height() as f32) * viewport.scale).max(1.0) as u32;

        let rendered = super::gpu_render_inner(
            src, viewport, dst_w, dst_h, scaled_w, scaled_h, filter, self,
        );

        // Acquire swapchain frame. Inner block ensures `sc` is dropped before
        // any `&mut self` call (configure_surface).
        let frame = {
            let (result, dims) = {
                let sc = self.swapchain.as_ref().expect("swapchain not initialised");
                let result = sc.surface.get_current_texture();
                let dims = (sc.config.width, sc.config.height);
                (result, dims)
                // sc dropped here
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

        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            // Clear the full frame to opaque black.
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if let Some((out_tex, win_x, win_y)) = rendered {
                let vis_w = out_tex.width();
                let vis_h = out_tex.height();
                let src_view = out_tex.create_view(&wgpu::TextureViewDescriptor::default());
                let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });
                // Re-borrow sc for pipeline/bgl after frame acquisition.
                let sc = self.swapchain.as_ref().unwrap();
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &sc.present_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&src_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });

                rpass.set_pipeline(&sc.present_pipeline);
                rpass.set_bind_group(0, &bind_group, &[]);
                rpass.set_viewport(
                    win_x as f32,
                    win_y as f32,
                    vis_w as f32,
                    vis_h as f32,
                    0.0,
                    1.0,
                );
                rpass.set_scissor_rect(win_x, win_y, vis_w, vis_h);
                rpass.draw(0..4, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
