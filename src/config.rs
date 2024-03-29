use serde::{
    Serialize,
    Deserialize
};
use toml;
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub ui: Option<UI>,
    pub viewer: Option<Viewer>,
    pub keybindings: Option<Keybindings>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ui: Some(UI::default()),
            viewer: Some(Viewer::default()),
            keybindings: Some(Keybindings::default()),
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Option<Config> {
        let mut config_file = File::open(path).ok()?;
        let mut config_str = String::new();
        let _ = config_file.read_to_string(&mut config_str);

        toml::from_str(&mut config_str).ok()?
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UI {
    pub title: Option<bool>,
    pub antialiasing: Option<bool>,
    pub theme: Option<Theme>,
}

impl Default for UI {
    fn default() -> Self {
        UI {
            title: Some(false),
            antialiasing: Some(true),
            theme: Some(Theme::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
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

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FilterMethod {
    Linear,
    Nearest
}

impl From<FilterMethod> for iced::widget::image::FilterMethod {
    fn from(filter_method: FilterMethod) -> Self {
        match filter_method {
            FilterMethod::Linear => iced::widget::image::FilterMethod::Linear,
            FilterMethod::Nearest => iced::widget::image::FilterMethod::Nearest,
        }
    }
}

impl Default for FilterMethod {
    fn default() -> Self {
        FilterMethod::Linear
    }
}

impl Default for Viewer {
    fn default() -> Self {
        Viewer {
            min_scale: Some(0.0),
            max_scale: Some(100.0),
            scale_step: Some(0.05),
            filter_method: Some(FilterMethod::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keybindings {
    pub quit: Option<String>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Keybindings {
            quit: Some("q".to_string()),
        }
    }
}
