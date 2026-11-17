#[cfg(all(test, feature = "config"))]
mod tests;

#[cfg(feature = "config")]
use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

#[cfg(feature = "config")]
use tracing::info;

// ── Config structs ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
#[cfg_attr(feature = "config", derive(serde::Deserialize, serde::Serialize))]
pub struct Config {
    pub window: Option<Window>,
    pub viewer: Option<Viewer>,
    pub keybindings: Option<Keybindings>,
    #[cfg_attr(not(feature = "logging"), allow(dead_code))]
    pub logging: Option<Logging>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            window: Some(Window::default()),
            viewer: Some(Viewer::default()),
            keybindings: Some(Keybindings::default()),
            logging: None,
        }
    }
}

// ── load_merged: two cfg-gated implementations ───────────────────────────────

/// When the `config` feature is disabled every caller receives built-in defaults;
/// no file I/O or TOML parsing is compiled in.
#[cfg(not(feature = "config"))]
impl Config {
    pub fn load_merged(
        _override_path: Option<&std::path::Path>,
        _problems: &mut Vec<String>,
    ) -> Result<Config, String> {
        Ok(Config::default())
    }
}

/// Full implementation: built-in defaults → system → user (XDG/HOME) → override.
#[cfg(feature = "config")]
impl Config {
    /// Read and parse one config file. Keys nothing reads (typos) are added
    /// to `problems`.
    fn load(path: &Path, problems: &mut Vec<String>) -> Result<Config, Box<dyn Error>> {
        let raw = std::fs::read_to_string(path)?;
        let mut unknown = Vec::new();
        let config = serde_ignored::deserialize(toml::Deserializer::parse(&raw)?, |key| {
            unknown.push(key_path(&key));
        })?;
        for key in unknown {
            problems.push(format!("{}: unknown key `{key}` ignored", path.display()));
        }
        Ok(config)
    }

    /// Layer the system, user and `--config` files over the defaults.
    ///
    /// Problems are collected in `problems` for the caller to log once logging
    /// is set up (it is configured from this very config): a broken system or
    /// user file is skipped, while a `--config` file that cannot be read or
    /// parsed is an error, since the user asked for it explicitly.
    pub fn load_merged(
        override_path: Option<&Path>,
        problems: &mut Vec<String>,
    ) -> Result<Config, String> {
        let mut config = Config::default();

        let user = Config::get_xdg_path().or_else(|_| Config::get_home_path());
        if let Err(e) = &user {
            problems.push(format!("no user config: {e}"));
        }
        let implicit = std::iter::once(Config::get_system_path()).chain(user.ok());
        for path in implicit.filter(|p| p.exists()) {
            match Config::load(&path, problems) {
                Ok(c) => {
                    info!(path = %path.display(), "loaded config");
                    config = Config::merge(config, c);
                }
                Err(e) => problems.push(format!("{} ignored: {e}", path.display())),
            }
        }

        if let Some(path) = override_path {
            let c = Config::load(path, problems).map_err(|e| format!("{}: {e}", path.display()))?;
            info!(path = %path.display(), "loaded --config override");
            config = Config::merge(config, c);
        }

        Ok(config)
    }

    pub fn get_xdg_path() -> Result<PathBuf, Box<dyn Error>> {
        env::var_os("XDG_CONFIG_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| "XDG_CONFIG_HOME not set".into())
            .map(|p| p.join("imgvwr").join("config.toml"))
    }

    pub fn get_home_path() -> Result<PathBuf, Box<dyn Error>> {
        env::var_os("HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| "neither XDG_CONFIG_HOME nor HOME is set".into())
            .map(|p| p.join(".config").join("imgvwr").join("config.toml"))
    }

    pub fn get_system_path() -> PathBuf {
        PathBuf::from("/etc/imgvwr/config.toml")
    }

    pub fn merge(base: Config, overlay: Config) -> Config {
        Config {
            window: merge_section(base.window, overlay.window, |b, o| Window {
                decorations: o.decorations.or(b.decorations),
                antialiasing: o.antialiasing.or(b.antialiasing),
            }),
            viewer: merge_section(base.viewer, overlay.viewer, |b, o| Viewer {
                min_scale: o.min_scale.or(b.min_scale),
                max_scale: o.max_scale.or(b.max_scale),
                scale_step: o.scale_step.or(b.scale_step),
                filter_method: o.filter_method.or(b.filter_method),
            }),
            keybindings: merge_section(base.keybindings, overlay.keybindings, |b, o| Keybindings {
                quit: o.quit.or(b.quit),
                rotate_left: o.rotate_left.or(b.rotate_left),
                rotate_right: o.rotate_right.or(b.rotate_right),
                delete: o.delete.or(b.delete),
            }),
            logging: merge_section(base.logging, overlay.logging, |b, o| Logging {
                level: o.level.or(b.level),
            }),
        }
    }
}

/// `viewer.scale_stepp`: an ignored key as written in the file.
#[cfg(feature = "config")]
pub(crate) fn key_path(path: &serde_ignored::Path<'_>) -> String {
    use serde_ignored::Path;
    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => format!("{}[{index}]", key_path(parent)),
        Path::Map { parent, key } => match key_path(parent) {
            p if p.is_empty() => key.clone(),
            p => format!("{p}.{key}"),
        },
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => key_path(parent),
    }
}

#[cfg(feature = "config")]
fn merge_section<T, F: FnOnce(T, T) -> T>(base: Option<T>, overlay: Option<T>, f: F) -> Option<T> {
    match (base, overlay) {
        (Some(b), Some(o)) => Some(f(b, o)),
        (b, o) => o.or(b),
    }
}

// ── Section structs ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
#[cfg_attr(feature = "config", derive(serde::Deserialize, serde::Serialize))]
pub struct Window {
    pub decorations: Option<bool>,
    pub antialiasing: Option<bool>,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            decorations: Some(false),
            antialiasing: Some(false),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "config", derive(serde::Deserialize, serde::Serialize))]
pub struct Viewer {
    pub min_scale: Option<f32>,
    pub max_scale: Option<f32>,
    pub scale_step: Option<f32>,
    pub filter_method: Option<FilterMethod>,
}

impl Default for Viewer {
    fn default() -> Self {
        Viewer {
            min_scale: Some(0.1),
            max_scale: Some(100.0),
            scale_step: Some(0.08),
            filter_method: Some(FilterMethod::Nearest),
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
#[cfg_attr(feature = "config", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "config", serde(rename_all = "snake_case"))]
#[cfg_attr(not(feature = "config"), allow(dead_code))]
pub enum FilterMethod {
    #[default]
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "config", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(not(feature = "keybinds"), allow(dead_code))]
pub struct Keybindings {
    pub quit: Option<String>,
    pub rotate_left: Option<String>,
    pub rotate_right: Option<String>,
    pub delete: Option<String>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Keybindings {
            quit: Some("q".to_string()),
            rotate_left: Some("[".to_string()),
            rotate_right: Some("]".to_string()),
            delete: Some("Delete".to_string()),
        }
    }
}

/// Logging configuration. The `level` field accepts the same values as the
/// `RUST_LOG` environment variable (`"error"`, `"warn"`, `"info"`, `"debug"`,
/// `"trace"`). `RUST_LOG` always overrides this field when set.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "config", derive(serde::Deserialize, serde::Serialize))]
#[derive(Default)]
#[cfg_attr(not(feature = "logging"), allow(dead_code))]
pub struct Logging {
    pub level: Option<String>,
}
