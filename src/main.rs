pub mod args;
pub mod config;
pub mod imgvwr;
pub mod strings;
pub mod util;

use crate::{args::Args, config::Config, imgvwr::Imgvwr, strings::messages};
use clap::Parser;
use iced::{window::Settings as WindowSettings, Settings, Theme};
use std::fs;

fn main() -> iced::Result {
    let mut args: Args = Args::parse();
    args.img = fs::canonicalize(&args.img).expect(messages::ERR_NO_INPUT_FILE);
    if !util::is_file_supported(&args.img).unwrap_or_default() {
        let err = messages::ERR_INPUT_FILE_NOT_SUPPORTED;
        return Err(iced::Error::WindowCreationFailed(err.into()));
    }

    let config_path = args
        .config
        .unwrap_or(Config::get_default_path().unwrap_or_default());
    let config = Config::load(&config_path).unwrap_or_default();
    let window = config.window.unwrap_or_default();
    let viewer = config.viewer.unwrap_or_default();
    let keybindings = config.keybindings.unwrap_or_default();

    let decorations = window.decorations.unwrap_or_default();
    let antialiasing = window.antialiasing.unwrap_or_default();
    let theme = window.theme.unwrap_or_default();

    iced::program(Imgvwr::title, Imgvwr::update, Imgvwr::view)
        .settings(Settings {
            window: WindowSettings {
                decorations,
                ..WindowSettings::default()
            },
            ..Settings::default()
        })
        .subscription(Imgvwr::subscription)
        .antialiasing(antialiasing)
        .centered()
        .theme(move |_| Theme::from(theme.to_owned()))
        .run_with(move || Imgvwr::new(&args.img, &viewer, keybindings.to_owned()))
}
