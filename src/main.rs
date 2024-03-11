pub mod args;
pub mod config;

use iced::{
    executor::Default as ExecutorDefault,
    keyboard::{
        self,
        key,
        Key,
    },
    widget::image::{
        viewer,
        Handle,
        FilterMethod,
    },
    Result,
    Subscription,
    Application,
    Command,
    Element,
    Length,
    Theme,
    Settings,
    window::Settings as WindowSettings,
};
use std::{
    fs,
    path::PathBuf,
};
use image::io::Reader;
use clap::Parser;
use args::Args;
use config::{
    Config,
    Keybindings,
};

fn main() -> Result {
    let args: Args = Args::parse();
    let config = args.get_config();
    let flags = Flags::new(args, config);

    let ui_config = flags.config.ui.clone().unwrap_or_default();
    let title_bar = ui_config.title.unwrap_or_default();
    Imgvwr::run(Settings {
        window: WindowSettings {
            decorations: title_bar,
            ..WindowSettings::default()
        },
        antialiasing: ui_config.antialiasing.clone().unwrap_or_default(),
        flags,
        ..Settings::default()
    })
}

struct Imgvwr {
    image_path: PathBuf,
    min_scale: f32,
    max_scale: f32,
    scale: f32,
    keybindings: Keybindings,
    theme: Theme,
    filter_method: FilterMethod,
}

impl Imgvwr {
    fn switch_image(&mut self, direction: Direction) {
        let image_path = match direction {
            Direction::Next => Imgvwr::next_image(self.image_path.clone()),
            Direction::Previous => todo!(),
        };

        self.image_path = image_path.unwrap();
    }
    
    fn next_image(image_path: PathBuf) -> Option<PathBuf> {
        // TODO: this fn is a fucking shit and doesn't work properly

        let file_name = image_path.file_name()?.to_str()?;
        let dir = image_path.parent()?;

        let exts = vec!["jpg", "png"];

        for entry in fs::read_dir(dir).ok()? {
            let entry_path = entry.ok()?.path();
            let entry_name = entry_path.file_name()?.to_str()?;
            let entry_ext = entry_path.extension()?.to_str()?;

            if entry_path.is_file() && entry_name > file_name && exts.contains(&entry_ext) {
                return Some(entry_path);
            }
        }

        None
    }
}

#[derive(Clone, Debug)]
enum Message {
    Quit,
    Move(Direction),
}

#[derive(Clone, Debug, Default)]
struct Flags {
    args: Args,
    config: Config,
}

impl Flags {
    pub fn new(args: Args, config: Config) -> Self {
        Flags {
            args,
            config,
        }
    }
}

impl Application for Imgvwr {
    type Executor = ExecutorDefault;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Flags) -> (Self, Command<Message>) {
        let config = flags.config;
        let viewer = config.viewer.unwrap_or_default();
        let ui = config.ui.unwrap_or_default();

        (
            Imgvwr {
                image_path: flags.args.img,
                min_scale: viewer.min_scale.unwrap(),
                max_scale: viewer.max_scale.unwrap(),
                scale: viewer.scale_step.unwrap(),
                keybindings: config.keybindings.unwrap_or_default(),
                theme: Theme::from(ui.theme.unwrap_or_default()),
                filter_method: FilterMethod::from(viewer.filter_method.unwrap_or_default())
            },
            Command::none()
        )
    }

    fn title(&self) -> String {
        "imgvwr".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Quit => {
                std::process::exit(0);
            }
            Message::Move(direction) => {
                self.switch_image(direction);
                self.view();
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        // let img = Reader::open(self.image_path.clone())
        //     .expect("Failed to open image file")
        //     .decode()
        //     .expect("Failed to decode image file");
        // let img = image::DynamicImage::ImageRgba8(img.into_rgba8());
        // let handle = Handle::from_pixels(img.width(), img.height(), img.into_bytes());

        let handle = Handle::from_path(self.image_path.clone());

        let viewer = viewer(handle)
            .scale_step(self.scale)
            .min_scale(self.min_scale)
            .max_scale(self.max_scale)
            // TODO: requires #2324 to be merged to iced repo
            // .filter_method(self.filter_method)
            .width(Length::Fill)
            .height(Length::Fill);

        viewer.into()
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        // TODO: pass keybindings to fn pointer somehow
        keyboard::on_key_press(|key, _modifiers,| {
            match key.as_ref() {
                Key::Character("q") => {
                    Some(Message::Quit)
                },
                Key::Named(key) => {
                    let direction = match key {
                        key::Named::ArrowLeft => Some(Direction::Previous),
                        key::Named::ArrowRight => Some(Direction::Next),
                        _ => None,
                    };

                    direction.map(Message::Move)
                }
                _ => None,
            }
        })
    }
}

#[derive(Clone, Debug)]
enum Direction {
    Next,
    Previous,
}
