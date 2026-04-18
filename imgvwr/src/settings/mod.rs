#[cfg(test)]
mod tests;

use std::path::PathBuf;

use libimgvwr::{
    keybinds::{KeybindMap, Keysym, keysym_from_str},
    renderer,
};

use crate::{
    cli::Cli,
    config::{Config, FilterMethod, Keybindings},
};

pub(crate) struct AppSettings {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) decorations: bool,
    pub(crate) antialiasing: bool,
    pub(crate) min_scale: f32,
    pub(crate) max_scale: f32,
    pub(crate) scale_step: f32,
    pub(crate) filter: renderer::FilterMethod,
    pub(crate) keybind_map: KeybindMap,
    pub(crate) key_left: Keysym,
    pub(crate) key_right: Keysym,
    pub(crate) log_level: String,
}

impl AppSettings {
    pub(crate) fn resolve(cli: &Cli, config: &Config) -> Self {
        let window = config.window.clone().unwrap_or_default();
        let viewer = config.viewer.clone().unwrap_or_default();
        let keybindings = config.keybindings.clone().unwrap_or_default();
        let logging = config.logging.clone().unwrap_or_default();

        AppSettings {
            paths: cli.paths.clone(),
            decorations: cli.decorations.or(window.decorations).unwrap_or(false),
            antialiasing: cli.antialiasing.or(window.antialiasing).unwrap_or(false),
            min_scale: cli.min_scale.or(viewer.min_scale).unwrap_or(0.1),
            max_scale: cli.max_scale.or(viewer.max_scale).unwrap_or(100.0),
            scale_step: cli.scale_step.or(viewer.scale_step).unwrap_or(0.08),
            filter: to_render_filter(
                cli.filter_method
                    .as_ref()
                    .or(viewer.filter_method.as_ref())
                    .unwrap_or(&FilterMethod::Nearest),
            ),
            keybind_map: build_keybind_map(&keybindings),
            key_left: keysym_from_str("Left").expect("Left keysym must resolve"),
            key_right: keysym_from_str("Right").expect("Right keysym must resolve"),
            log_level: cli
                .log_level
                .clone()
                .or(logging.level)
                .unwrap_or_else(|| "warn".to_string()),
        }
    }
}

fn to_render_filter(f: &FilterMethod) -> renderer::FilterMethod {
    match f {
        FilterMethod::Nearest => renderer::FilterMethod::Nearest,
        FilterMethod::Triangle => renderer::FilterMethod::Triangle,
        FilterMethod::CatmullRom => renderer::FilterMethod::CatmullRom,
        FilterMethod::Gaussian => renderer::FilterMethod::Gaussian,
        FilterMethod::Lanczos3 => renderer::FilterMethod::Lanczos3,
    }
}

#[cfg(feature = "keybinds")]
fn build_keybind_map(keybindings: &Keybindings) -> KeybindMap {
    let quit = resolve_keysym(keybindings.quit.as_deref().unwrap_or("q"), "q");
    let rotate_left = resolve_keysym(
        keybindings.rotate_left.as_deref().unwrap_or("bracketleft"),
        "bracketleft",
    );
    let rotate_right = resolve_keysym(
        keybindings
            .rotate_right
            .as_deref()
            .unwrap_or("bracketright"),
        "bracketright",
    );
    let delete = resolve_keysym(keybindings.delete.as_deref().unwrap_or("Delete"), "Delete");
    KeybindMap::new(quit, rotate_left, rotate_right, delete)
}

#[cfg(not(feature = "keybinds"))]
fn build_keybind_map(_keybindings: &Keybindings) -> KeybindMap {
    KeybindMap::new(
        keysym_from_str("q").expect("q keysym must resolve"),
        keysym_from_str("bracketleft").expect("bracketleft keysym must resolve"),
        keysym_from_str("bracketright").expect("bracketright keysym must resolve"),
        keysym_from_str("Delete").expect("Delete keysym must resolve"),
    )
}

#[cfg(feature = "keybinds")]
fn resolve_keysym(name: &str, fallback: &str) -> Keysym {
    keysym_from_str(name)
        .or_else(|_| keysym_from_str(fallback))
        .expect("fallback keysym must resolve")
}
