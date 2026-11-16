use std::path::PathBuf;
use std::time::Duration;
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

#[cfg(test)]
mod tests;

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
                // Keep the animation's own clock; only resync after a stall.
                *next_at += delay;
                if *next_at <= now {
                    *next_at = now + delay;
                }
                return true;
            }
        }
        false
    }

    /// Time until the next animation frame is due; `None` for a still image.
    fn next_frame_in(&self) -> Option<Duration> {
        match self {
            Self::Static(_) => None,
            #[cfg(any(
                feature = "gif",
                feature = "avif-anim",
                feature = "jxl-anim",
                feature = "webp-anim",
                feature = "apng"
            ))]
            Self::Animated { next_at, .. } => {
                Some(next_at.saturating_duration_since(Instant::now()))
            }
        }
    }
}

#[derive(Default)]
struct EventOutcome {
    dirty: bool,
    quit: bool,
    navigated: bool,
    /// The user zoomed or panned, so the view no longer follows the window.
    unfit: bool,
}

#[cfg(feature = "decorations")]
fn make_title(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("imgvwr");
    format!("{name} — imgvwr")
}

/// Scale at which `img`, rotated by `rotation`, fits inside `window`.
fn fit_scale(
    img: &DynamicImage,
    rotation: u16,
    window: (u32, u32),
    min_scale: f32,
    max_scale: f32,
) -> f32 {
    let (w, h) = match rotation {
        90 | 270 => (img.height(), img.width()),
        _ => (img.width(), img.height()),
    };
    let sw = window.0 as f32 / w.max(1) as f32;
    let sh = window.1 as f32 / h.max(1) as f32;
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

/// Show the first entry that loads, walking the list `forward` or back from
/// the cursor: from the cursor itself when `try_current`, else from the next
/// entry. Every entry is tried at most once, and the one on screen is not
/// reloaded. When nothing loads, the cursor goes back to where it was, so it
/// always names the image on screen (the one Delete removes).
fn show_loadable(
    navigator: &mut Navigator,
    forward: bool,
    try_current: bool,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
) -> bool {
    let start = navigator.current;
    for attempt in 0..navigator.paths.len() {
        if attempt > 0 || !try_current {
            if forward {
                navigator.next();
            } else {
                navigator.prev();
            }
            if !try_current && navigator.current == start {
                break;
            }
        }
        if navigate_to(navigator.current().to_path_buf(), image, viewport) {
            return true;
        }
    }
    navigator.current = start;
    false
}

fn on_navigate(
    forward: bool,
    navigator: &mut Navigator,
    image: &mut ImageHolder,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let success = show_loadable(navigator, forward, false, image, viewport);
    EventOutcome {
        dirty: success,
        navigated: success,
        ..Default::default()
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
            if navigator.remove_current().is_some()
                && show_loadable(navigator, true, true, image, viewport)
            {
                EventOutcome {
                    dirty: true,
                    navigated: true,
                    ..Default::default()
                }
            } else {
                // Nothing left that can be shown.
                EventOutcome {
                    quit: true,
                    ..Default::default()
                }
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
        unfit: true,
        ..Default::default()
    }
}

fn on_pointer_motion(dx: f32, dy: f32, viewport: &mut ViewportState) -> EventOutcome {
    viewport.pan(dx, dy);
    EventOutcome {
        dirty: true,
        unfit: true,
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
        on_navigate(false, navigator, image, viewport)
    } else if sym == settings.key_right {
        on_navigate(true, navigator, image, viewport)
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
    let mut gpu_ctx = GpuContext::new()?;

    let mut navigator = Navigator::from_paths(&settings.paths)?;
    let mut image = ImageHolder::Static(DynamicImage::new_rgba8(1, 1));
    let mut viewport = ViewportState::default();
    if !show_loadable(&mut navigator, true, true, &mut image, &mut viewport) {
        return Err("none of the given images could be loaded".into());
    }

    let mut wayland = WaylandContext::connect((800, 600), settings.decorations)?;

    // dmabuf: init GPU context after surface is created, using Wayland handles.
    #[cfg(feature = "dmabuf")]
    let mut gpu_ctx = {
        let (w, h) = wayland.state.window_size;
        GpuContext::new_with_surface(wayland.display_ptr(), wayland.surface_ptr(), w, h)?
    };
    #[cfg(feature = "dmabuf")]
    let mut last_surface_size = wayland.state.window_size;

    // The image follows the window size until the user zooms or pans.
    let mut fitted = true;

    #[cfg(feature = "decorations")]
    if settings.decorations {
        wayland.set_title(&make_title(navigator.current()));
    }

    // A frame is owed but the compositor has not asked for one yet.
    let mut redraw_pending = false;

    loop {
        let timeout = if wayland.state.needs_redraw || redraw_pending {
            // Draw right away if the compositor wants a frame, else sleep
            // until its frame callback (never, while the window is hidden).
            if wayland.ready_for_frame() {
                Some(Duration::ZERO)
            } else {
                None
            }
        } else {
            image.next_frame_in()
        };
        wayland.dispatch(timeout)?;

        let events: Vec<InputEvent> = std::mem::take(&mut wayland.state.pending_events);
        let mut dirty = wayland.state.needs_redraw || redraw_pending;
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
            if outcome.navigated {
                fitted = true;
            } else if outcome.unfit {
                fitted = false;
            }
            if outcome.quit {
                wayland.state.closed = true;
            }
        }

        if any_navigated {
            dirty = true;

            #[cfg(feature = "decorations")]
            if settings.decorations {
                wayland.set_title(&make_title(navigator.current()));
            }
        }

        redraw_pending = dirty && !wayland.ready_for_frame();
        if dirty && !redraw_pending {
            let frame_start = std::time::Instant::now();
            let (w, h) = wayland.state.window_size;
            if fitted {
                viewport.scale = fit_scale(
                    image.current(),
                    viewport.rotation,
                    (w, h),
                    settings.min_scale,
                    settings.max_scale,
                );
                viewport.offset = (0.0, 0.0);
            }
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
                    &mut gpu_ctx,
                );
                wayland.commit_frame(&pixels, w, h)?;
            }

            debug!(
                ms = frame_start.elapsed().as_secs_f64() * 1000.0,
                scale = viewport.scale,
                offset = ?viewport.offset,
                "frame"
            );
        }

        if wayland.state.closed {
            break;
        }
    }

    Ok(())
}
