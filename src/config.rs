use crate::strings::{self, keybindings, messages};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io::Read, path::PathBuf};
use toml;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub window: Option<Window>,
    pub viewer: Option<Viewer>,
    pub keybindings: Option<Keybindings>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            window: Some(Window::default()),
            viewer: Some(Viewer::default()),
            keybindings: Some(Keybindings::default()),
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Config, Box<dyn Error>> {
        let mut config_file = File::open(path)?;
        let mut config_str = String::new();
        config_file.read_to_string(&mut config_str)?;

        toml::from_str(&config_str).map_err(|err| err.into())
    }

    pub fn get_default_path() -> Result<PathBuf, Box<dyn Error>> {
        dirs::config_local_dir()
            .ok_or_else(|| messages::ERR_NO_DOTCONFIG.into())
            .map(|path| {
                path.join(strings::APPLICATION_NAME)
                    .join(strings::CONFIG_FILENAME)
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub decorations: Option<bool>,
    pub antialiasing: Option<bool>,
    pub theme: Option<Theme>,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            decorations: Some(true),
            antialiasing: Some(true),
            theme: Some(Theme::default()),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    #[default]
    Dark,
    Dracula,
    Nord,
    SolarizedLight,
    SolarizedDark,
    GruvboxLight,
    GruvboxDark,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    CatppuccinMocha,
    TokyoNight,
    TokyoNightStorm,
    TokyoNightLight,
    KanagawaWave,
    KanagawaDragon,
    KanagawaLotus,
    Moonfly,
    Nightfly,
    Oxocarbon,
}

impl From<Theme> for iced::Theme {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Light => iced::Theme::Light,
            Theme::Dark => iced::Theme::Dark,
            Theme::Dracula => iced::Theme::Dracula,
            Theme::Nord => iced::Theme::Nord,
            Theme::SolarizedLight => iced::Theme::SolarizedLight,
            Theme::SolarizedDark => iced::Theme::SolarizedDark,
            Theme::GruvboxLight => iced::Theme::GruvboxLight,
            Theme::GruvboxDark => iced::Theme::GruvboxDark,
            Theme::CatppuccinLatte => iced::Theme::CatppuccinLatte,
            Theme::CatppuccinFrappe => iced::Theme::CatppuccinFrappe,
            Theme::CatppuccinMacchiato => iced::Theme::CatppuccinMacchiato,
            Theme::CatppuccinMocha => iced::Theme::CatppuccinMocha,
            Theme::TokyoNight => iced::Theme::TokyoNight,
            Theme::TokyoNightStorm => iced::Theme::TokyoNightStorm,
            Theme::TokyoNightLight => iced::Theme::TokyoNightLight,
            Theme::KanagawaWave => iced::Theme::KanagawaWave,
            Theme::KanagawaDragon => iced::Theme::KanagawaDragon,
            Theme::KanagawaLotus => iced::Theme::KanagawaLotus,
            Theme::Moonfly => iced::Theme::Moonfly,
            Theme::Nightfly => iced::Theme::Nightfly,
            Theme::Oxocarbon => iced::Theme::Oxocarbon,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Viewer {
    pub min_scale: Option<f32>,
    pub max_scale: Option<f32>,
    pub scale_step: Option<f32>,
    pub filter_method: Option<FilterMethod>,
    pub content_fit: Option<ContentFit>,
}

impl Default for Viewer {
    fn default() -> Self {
        Viewer {
            min_scale: Some(0.0),
            max_scale: Some(100.0),
            scale_step: Some(0.08),
            filter_method: Some(FilterMethod::default()),
            content_fit: Some(ContentFit::default()),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMethod {
    #[default]
    Linear,
    Nearest,
}

impl From<FilterMethod> for iced::widget::image::FilterMethod {
    fn from(filter_method: FilterMethod) -> Self {
        match filter_method {
            FilterMethod::Linear => iced::widget::image::FilterMethod::Linear,
            FilterMethod::Nearest => iced::widget::image::FilterMethod::Nearest,
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFit {
    Contain,
    Cover,
    Fill,
    #[default]
    None,
    ScaleDown,
}

impl From<ContentFit> for iced::ContentFit {
    fn from(content_fit: ContentFit) -> Self {
        match content_fit {
            ContentFit::Contain => iced::ContentFit::Contain,
            ContentFit::Cover => iced::ContentFit::Cover,
            ContentFit::Fill => iced::ContentFit::Fill,
            ContentFit::None => iced::ContentFit::None,
            ContentFit::ScaleDown => iced::ContentFit::ScaleDown,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keybindings {
    pub quit: Option<String>,
    pub rotate_left: Option<String>,
    pub rotate_right: Option<String>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Keybindings {
            quit: Some(keybindings::QUIT.to_string()),
            rotate_left: Some(keybindings::ROTATE_LEFT.to_string()),
            rotate_right: Some(keybindings::ROTATE_RIGHT.to_string()),
        }
    }
}
