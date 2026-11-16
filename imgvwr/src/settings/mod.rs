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

/// Built-in limits, also used when a configured value is unusable.
const DEFAULT_MIN_SCALE: f32 = 0.1;
const DEFAULT_MAX_SCALE: f32 = 100.0;
const DEFAULT_SCALE_STEP: f32 = 0.08;
const LOG_LEVELS: [&str; 6] = ["off", "error", "warn", "info", "debug", "trace"];

impl AppSettings {
    /// Merge CLI over config over defaults. Values that cannot work are
    /// replaced by their defaults and described in `problems`.
    pub(crate) fn resolve(cli: &Cli, config: &Config, problems: &mut Vec<String>) -> Self {
        let window = config.window.clone().unwrap_or_default();
        let viewer = config.viewer.clone().unwrap_or_default();
        let keybindings = config.keybindings.clone().unwrap_or_default();
        let logging = config.logging.clone().unwrap_or_default();

        let (min_scale, max_scale) = scale_limits(
            cli.min_scale.or(viewer.min_scale),
            cli.max_scale.or(viewer.max_scale),
            problems,
        );

        AppSettings {
            paths: cli.paths.clone(),
            decorations: cli.decorations.or(window.decorations).unwrap_or(false),
            antialiasing: cli.antialiasing.or(window.antialiasing).unwrap_or(false),
            min_scale,
            max_scale,
            scale_step: scale_step(cli.scale_step.or(viewer.scale_step), problems),
            filter: to_render_filter(
                cli.filter_method
                    .as_ref()
                    .or(viewer.filter_method.as_ref())
                    .unwrap_or(&FilterMethod::Nearest),
            ),
            keybind_map: build_keybind_map(&keybindings, problems),
            key_left: keysym_from_str("Left").expect("Left keysym must resolve"),
            key_right: keysym_from_str("Right").expect("Right keysym must resolve"),
            log_level: log_level(cli.log_level.clone().or(logging.level), problems),
        }
    }
}

/// `min_scale` and `max_scale`, each positive and finite, min ≤ max.
fn scale_limits(min: Option<f32>, max: Option<f32>, problems: &mut Vec<String>) -> (f32, f32) {
    let mut positive = |name: &str, value: Option<f32>, default: f32| match value {
        Some(v) if v.is_finite() && v > 0.0 => v,
        Some(v) => {
            problems.push(format!(
                "{name} must be a positive number, not {v}; using {default}"
            ));
            default
        }
        None => default,
    };
    let min = positive("min_scale", min, DEFAULT_MIN_SCALE);
    let max = positive("max_scale", max, DEFAULT_MAX_SCALE);
    if min > max {
        problems.push(format!(
            "min_scale ({min}) is larger than max_scale ({max}); \
             using {DEFAULT_MIN_SCALE} and {DEFAULT_MAX_SCALE}"
        ));
        return (DEFAULT_MIN_SCALE, DEFAULT_MAX_SCALE);
    }
    (min, max)
}

/// Zoom per notch as a fraction: zooming out multiplies by `1 - step`, so
/// the step must stay strictly between 0 and 1.
fn scale_step(step: Option<f32>, problems: &mut Vec<String>) -> f32 {
    match step {
        Some(s) if s > 0.0 && s < 1.0 => s,
        Some(s) => {
            problems.push(format!(
                "scale_step must be between 0 and 1 (exclusive), not {s}; \
                 using {DEFAULT_SCALE_STEP}"
            ));
            DEFAULT_SCALE_STEP
        }
        None => DEFAULT_SCALE_STEP,
    }
}

fn log_level(level: Option<String>, problems: &mut Vec<String>) -> String {
    match level {
        Some(l) if LOG_LEVELS.contains(&l.to_ascii_lowercase().as_str()) => l.to_ascii_lowercase(),
        Some(l) => {
            problems.push(format!(
                "unknown log level `{l}` (expected one of {}); using warn",
                LOG_LEVELS.join(", ")
            ));
            "warn".to_string()
        }
        None => "warn".to_string(),
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
fn build_keybind_map(keybindings: &Keybindings, problems: &mut Vec<String>) -> KeybindMap {
    let mut key = |action: &str, name: Option<&str>, fallback: &str| {
        let fallback_sym = keysym_from_str(fallback).expect("fallback keysym must resolve");
        match name.map(keysym_from_str) {
            Some(Ok(sym)) => sym,
            Some(Err(e)) => {
                problems.push(format!("keybindings.{action}: {e}; using `{fallback}`"));
                fallback_sym
            }
            None => fallback_sym,
        }
    };
    KeybindMap::new(
        key("quit", keybindings.quit.as_deref(), "q"),
        key(
            "rotate_left",
            keybindings.rotate_left.as_deref(),
            "bracketleft",
        ),
        key(
            "rotate_right",
            keybindings.rotate_right.as_deref(),
            "bracketright",
        ),
        key("delete", keybindings.delete.as_deref(), "Delete"),
    )
}

#[cfg(not(feature = "keybinds"))]
fn build_keybind_map(_keybindings: &Keybindings, _problems: &mut Vec<String>) -> KeybindMap {
    KeybindMap::new(
        keysym_from_str("q").expect("q keysym must resolve"),
        keysym_from_str("bracketleft").expect("bracketleft keysym must resolve"),
        keysym_from_str("bracketright").expect("bracketright keysym must resolve"),
        keysym_from_str("Delete").expect("Delete keysym must resolve"),
    )
}
