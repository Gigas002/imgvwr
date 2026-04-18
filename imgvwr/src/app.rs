use std::path::PathBuf;
#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
use std::time::Instant;

use tracing::{debug, info, warn};

#[cfg(feature = "decorations")]
use std::path::Path;

use image::DynamicImage;
#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
use libimgvwr::renderer::gpu::GpuContext;
use libimgvwr::{
    keybinds::{Action, Keysym},
    loader,
    navigator::Navigator,
    renderer,
    viewport::ViewportState,
    wayland::{InputEvent, WaylandContext},
};

use crate::settings::AppSettings;

/// Holds either a static single image or an animated sequence of frames.
enum ImageHolder {
    Static(DynamicImage),
    #[cfg(any(
        feature = "gif",
        feature = "avif-anim",
        feature = "jxl-anim",
        feature = "webp-anim",
        feature = "apng"
    ))]
    Animated {
        frames: Vec<(DynamicImage, std::time::Duration)>,
        current: usize,
        next_at: Instant,
    },
}

impl ImageHolder {
    fn current(&self) -> &DynamicImage {
        match self {
            Self::Static(img) => img,
            #[cfg(any(
                feature = "gif",
                feature = "avif-anim",
                feature = "jxl-anim",
                feature = "webp-anim",
                feature = "apng"
            ))]
            Self::Animated {
                frames, current, ..
            } => &frames[*current].0,
        }
    }

    /// Advance animation by one frame if its display time has elapsed.
    /// Returns `true` if the frame changed and a redraw is needed.
    fn tick(&mut self) -> bool {
        #[cfg(any(
            feature = "gif",
            feature = "avif-anim",
            feature = "jxl-anim",
            feature = "webp-anim",
            feature = "apng"
        ))]
        if let Self::Animated {
            frames,
            current,
            next_at,
        } = self
        {
            let now = Instant::now();
            if now >= *next_at {
                *current = (*current + 1) % frames.len();
                let delay = frames[*current].1;
                *next_at = now + delay;
                return true;
            }
        }
        false
    }
}

#[derive(Default)]
struct EventOutcome {
    dirty: bool,
    quit: bool,
    navigated: bool,
}

#[cfg(feature = "decorations")]
fn make_title(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("imgvwr");
    format!("{name} — imgvwr")
}

fn fit_scale(img: &DynamicImage, window: (u32, u32), min_scale: f32, max_scale: f32) -> f32 {
    let sw = window.0 as f32 / img.width() as f32;
    let sh = window.1 as f32 / img.height() as f32;
    sw.min(sh).clamp(min_scale, max_scale)
}

fn load_image(path: &std::path::Path) -> Result<ImageHolder, loader::LoadError> {
    let _ext = path
        .extension()
        .and_then(|e: &std::ffi::OsStr| e.to_str())
        .map(str::to_ascii_lowercase);

    #[cfg(feature = "gif")]
    if _ext.as_deref() == Some("gif") {
        let anim = loader::load_gif_frames(path)?;
        return Ok(anim_frames_to_holder(anim));
    }

    #[cfg(feature = "jxl-anim")]
    if _ext.as_deref() == Some("jxl")
        && let Ok(anim) = loader::load_jxl_anim_frames(path)
    {
        return Ok(anim_frames_to_holder(anim));
    }

    #[cfg(feature = "avif-anim")]
    if _ext.as_deref() == Some("avif")
        && let Ok(anim) = loader::load_avif_anim_frames(path)
    {
        return Ok(anim_frames_to_holder(anim));
    }

    #[cfg(feature = "webp-anim")]
    if _ext.as_deref() == Some("webp")
        && let Ok(anim) = loader::load_webp_anim_frames(path)
    {
        return Ok(anim_frames_to_holder(anim));
    }

    #[cfg(feature = "apng")]
    if _ext.as_deref() == Some("png")
        && let Ok(anim) = loader::load_apng_frames(path)
    {
        return Ok(anim_frames_to_holder(anim));
    }

    loader::load(path).map(ImageHolder::Static)
}

#[cfg(any(
    feature = "gif",
    feature = "avif-anim",
    feature = "jxl-anim",
    feature = "webp-anim",
    feature = "apng"
))]
fn anim_frames_to_holder(anim: loader::AnimFrames) -> ImageHolder {
    if anim.frames.len() > 1 {
        let next_at = Instant::now() + anim.frames[0].1;
        ImageHolder::Animated {
            frames: anim.frames,
            current: 0,
            next_at,
        }
    } else {
        let img = anim.frames.into_iter().next().map(|(img, _)| img);
        ImageHolder::Static(img.unwrap_or_else(|| image::DynamicImage::new_rgba8(1, 1)))
    }
}

fn navigate_to(path: PathBuf, image: &mut ImageHolder, viewport: &mut ViewportState) -> bool {
    match load_image(&path) {
        Ok(img) => {
            *image = img;
            viewport.reset();
            info!(path = %path.display(), "navigated to image");
            true
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to load image");
            false
        }
    }
}

fn on_navigate_prev(
    navigator: &mut Navigator,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let path = navigator.prev().to_path_buf();
    let success = navigate_to(path, image, viewport);
    EventOutcome {
        dirty: success,
        navigated: success,
        quit: false,
    }
}

fn on_navigate_next(
    navigator: &mut Navigator,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let path = navigator.next().to_path_buf();
    let success = navigate_to(path, image, viewport);
    EventOutcome {
        dirty: success,
        navigated: success,
        quit: false,
    }
}

fn on_rotate_left(viewport: &mut ViewportState) -> EventOutcome {
    viewport.rotate_left();
    debug!(rotation = viewport.rotation, "rotated left");
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_rotate_right(viewport: &mut ViewportState) -> EventOutcome {
    viewport.rotate_right();
    debug!(rotation = viewport.rotation, "rotated right");
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_delete_file(
    navigator: &mut Navigator,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let path = navigator.current().to_path_buf();
    match std::fs::remove_file(&path) {
        Ok(()) => {
            info!(path = %path.display(), "deleted file");
            match navigator.remove_current() {
                Some(next) => {
                    let next = next.to_path_buf();
                    let success = navigate_to(next, image, viewport);
                    EventOutcome {
                        dirty: success,
                        navigated: success,
                        quit: false,
                    }
                }
                None => EventOutcome {
                    quit: true,
                    ..Default::default()
                },
            }
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to delete file");
            EventOutcome::default()
        }
    }
}

fn on_scroll(
    delta: f32,
    cursor: (f32, f32),
    window: (u32, u32),
    settings: &AppSettings,
    viewport: &mut ViewportState,
) -> EventOutcome {
    viewport.zoom_by_at(
        delta * settings.scale_step,
        settings.min_scale,
        settings.max_scale,
        cursor,
        window,
    );
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_pointer_motion(dx: f32, dy: f32, viewport: &mut ViewportState) -> EventOutcome {
    viewport.pan(dx, dy);
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_key_action(
    sym: Keysym,
    settings: &AppSettings,
    navigator: &mut Navigator,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
) -> EventOutcome {
    if sym == settings.key_left {
        on_navigate_prev(navigator, image, viewport)
    } else if sym == settings.key_right {
        on_navigate_next(navigator, image, viewport)
    } else if let Some(action) = settings.keybind_map.lookup(sym) {
        match action {
            Action::Quit => EventOutcome {
                quit: true,
                ..Default::default()
            },
            Action::RotateLeft => on_rotate_left(viewport),
            Action::RotateRight => on_rotate_right(viewport),
            Action::DeleteFile => on_delete_file(navigator, image, viewport),
        }
    } else {
        EventOutcome::default()
    }
}

fn process_event(
    event: InputEvent,
    settings: &AppSettings,
    navigator: &mut Navigator,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
    window: (u32, u32),
) -> EventOutcome {
    match event {
        InputEvent::Key(sym) => on_key_action(sym, settings, navigator, image, viewport),
        InputEvent::Scroll { delta, cursor } => {
            on_scroll(delta, cursor, window, settings, viewport)
        }
        InputEvent::PointerMotion { dx, dy } => on_pointer_motion(dx, dy, viewport),
        InputEvent::PointerButton { .. } => EventOutcome::default(),
    }
}

pub fn run(settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    if settings.paths.is_empty() {
        return Err("no image paths given".into());
    }

    info!(
        decorations = settings.decorations,
        antialiasing = settings.antialiasing,
        filter = ?settings.filter,
        "imgvwr starting"
    );

    #[cfg(all(
        any(feature = "gpu-vulkan", feature = "gpu-gles"),
        not(feature = "dmabuf")
    ))]
    let gpu_ctx = GpuContext::new()?;

    let mut navigator = Navigator::from_path(&settings.paths[0])?;
    let mut image = load_image(navigator.current())?;
    info!(path = %navigator.current().display(), "loaded first image");

    let mut viewport = ViewportState::default();
    let mut wayland = WaylandContext::connect((800, 600), settings.decorations)?;

    // dmabuf: init GPU context after surface is created, using Wayland handles.
    #[cfg(feature = "dmabuf")]
    let mut gpu_ctx = {
        let (w, h) = wayland.state.window_size;
        GpuContext::new_with_surface(wayland.display_ptr(), wayland.surface_ptr(), w, h)?
    };
    #[cfg(feature = "dmabuf")]
    let mut last_surface_size = wayland.state.window_size;

    viewport.scale = fit_scale(
        image.current(),
        wayland.state.window_size,
        settings.min_scale,
        settings.max_scale,
    );

    #[cfg(feature = "decorations")]
    if settings.decorations {
        wayland.set_title(&make_title(navigator.current()));
    }

    loop {
        wayland.dispatch(16)?;

        let events: Vec<InputEvent> = wayland.state.pending_events.drain(..).collect();
        let mut dirty = wayland.state.needs_redraw;
        wayland.state.needs_redraw = false;
        let mut any_navigated = false;

        dirty |= image.tick();

        for event in events {
            let outcome = process_event(
                event,
                &settings,
                &mut navigator,
                &mut image,
                &mut viewport,
                wayland.state.window_size,
            );
            dirty |= outcome.dirty;
            any_navigated |= outcome.navigated;
            if outcome.quit {
                wayland.state.closed = true;
            }
        }

        if any_navigated {
            viewport.scale = fit_scale(
                image.current(),
                wayland.state.window_size,
                settings.min_scale,
                settings.max_scale,
            );
            dirty = true;

            #[cfg(feature = "decorations")]
            if settings.decorations {
                wayland.set_title(&make_title(navigator.current()));
            }
        }

        if dirty {
            let (w, h) = wayland.state.window_size;
            let effective_filter = if settings.antialiasing {
                settings.filter
            } else {
                renderer::FilterMethod::Nearest
            };

            #[cfg(feature = "dmabuf")]
            {
                if (w, h) != last_surface_size {
                    gpu_ctx.configure_surface(w, h);
                    last_surface_size = (w, h);
                }
                gpu_ctx.render_and_present(image.current(), &viewport, w, h, effective_filter)?;
            }

            #[cfg(not(feature = "dmabuf"))]
            {
                let pixels = renderer::render(
                    image.current(),
                    &viewport,
                    w,
                    h,
                    effective_filter,
                    #[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]
                    &gpu_ctx,
                );
                wayland.commit_frame(&pixels, w, h)?;
            }
        }

        if wayland.state.closed {
            break;
        }
    }

    Ok(())
}
