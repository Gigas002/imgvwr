pub mod args;
pub mod config;
pub mod util;

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
    collections::HashMap,
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
    let mut args: Args = Args::parse();
    args.img = fs::canonicalize(&args.img).expect("Couldn't get absolute path for input image");
    if !util::is_file_supported(&args.img).unwrap_or_default() {
        // panic!("Input file extension not supported");
        std::process::exit(0);
    }

    let config = args.get_config().unwrap_or_default();
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
    current_image_id: usize,
    images: HashMap<usize, PathBuf>,
    min_scale: f32,
    max_scale: f32,
    scale: f32,
    keybindings: Keybindings,
    theme: Theme,
    filter_method: FilterMethod,
}

impl Imgvwr {
    fn switch_current_image(&mut self, direction: Direction) {
        let current_id = &self.current_image_id;
        let min_id = self.images.keys().min().unwrap();
        let max_id = self.images.keys().max().unwrap();

        let image_id = match direction {
            Direction::Next => {
                match current_id + 1 {
                    next_id if next_id > *max_id => *min_id,
                    next_id => next_id,
                }
            },
            Direction::Previous => {
                match current_id.checked_sub(1) {
                    Some(prev_id) => prev_id,
                    None => *max_id,
                }
            }
        };

        self.current_image_id = image_id;
    }

    fn get_current_image_path(&self) -> Option<&PathBuf> {
        self.images.get(&self.current_image_id)
    }

    fn get_current_image(&self) -> Handle {
        let image_path = self.get_current_image_path().unwrap();

        // TODO: check if all `image` extensions are supported directly by iced
        // and we don't need to load them through image create

        // let img = Reader::open(image_path)
        //     .expect("Failed to open image file")
        //     .decode()
        //     .expect("Failed to decode image file");
        // let img = image::DynamicImage::ImageRgba8(img.into_rgba8());
        // Handle::from_pixels(img.width(), img.height(), img.into_bytes())

        Handle::from_path(image_path)
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

        let images = util::get_files(&flags.args.img).expect("No files in input directory");
        let current_image_id = util::get_current_file_id(&flags.args.img, &images).expect("Couldn't get input file id");

        (
            Imgvwr {
                images,
                current_image_id,
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
        // TODO: add current file and/or app version
        "imgvwr".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Quit => {
                std::process::exit(0);
            }
            Message::Move(direction) => {
                self.switch_current_image(direction);
                // TODO: the previous viewer is killed... right?
                self.view();
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let handle = self.get_current_image();

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
