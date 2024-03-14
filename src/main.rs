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
        FilterMethod,
        Handle,
        Image,
        Viewer
    },
    window::{
        Position,
        Settings as WindowSettings
    },
    Application,
    Command,
    ContentFit,
    Element,
    Length,
    Result,
    Settings,
    Subscription,
    Theme,
};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};
use image::{
    io::Reader,
    DynamicImage
};
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
        std::process::exit(0);
    }

    let config = args.get_config().unwrap_or_default();
    let flags = Flags::new(args, config);

    let ui_config = flags.config.ui.clone().unwrap_or_default();
    let title_bar = ui_config.title.unwrap_or_default();
    Imgvwr::run(Settings {
        window: WindowSettings {
            // TODO: consider option, why not?
            position: Position::Centered,
            decorations: title_bar,
            ..WindowSettings::default()
        },
        antialiasing: ui_config.antialiasing.clone().unwrap_or_default(),
        flags,
        ..Settings::default()
    })
}

struct Imgvwr {
    image: DynamicImage,
    image_id: usize,
    images: HashMap<usize, PathBuf>,
    min_scale: f32,
    max_scale: f32,
    scale: f32,
    keybindings: Keybindings,
    theme: Theme,
    filter_method: FilterMethod,
    content_fit: ContentFit,
}

impl Imgvwr {
    fn switch_image(&mut self, direction: Direction) {
        let current_id = &self.image_id;
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

        self.image_id = image_id;
        let image_path = self.get_image_path().unwrap();
        self.image = Imgvwr::get_image(image_path)
    }

    fn get_image_path(&self) -> Option<&PathBuf> {
        self.images.get(&self.image_id)
    }

    fn get_handle_from_path(&self) -> Handle {
        let image_path = self.get_image_path().unwrap();

        Handle::from_path(image_path)
    }

    // TODO: is there no way to pass reference instead of cloning whole fucking image?
    fn get_handle_from_pixels(&self) -> Handle {
        let image = self.image.clone();

        Handle::from_pixels(image.width(), image.height(), image.into_bytes())
    }

    fn get_image(image_path: &PathBuf) -> DynamicImage {
        let image = Reader::open(image_path)
            .expect("Failed to open image file")
            .decode()
            .expect("Failed to decode image file");

        DynamicImage::ImageRgba8(image.into_rgba8())
    }

    fn rotate_image(&mut self, direction: Direction) {
        self.image = match direction {
            Direction::Next => self.image.rotate90(),
            Direction::Previous => self.image.rotate270(),
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    Quit,
    Move(Direction),
    Rotate(Direction),
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
        let image_id = util::get_file_id(&flags.args.img, &images).expect("Couldn't get input file id");
        let image = Imgvwr::get_image(&flags.args.img);

        (
            Imgvwr {
                image,
                image_id,
                images,
                min_scale: viewer.min_scale.unwrap(),
                max_scale: viewer.max_scale.unwrap(),
                scale: viewer.scale_step.unwrap(),
                keybindings: config.keybindings.unwrap_or_default(),
                theme: Theme::from(ui.theme.unwrap_or_default()),
                filter_method: FilterMethod::from(viewer.filter_method.unwrap_or_default()),
                content_fit: ContentFit::from(viewer.content_fit.unwrap_or_default()),
            },
            Command::none()
        )
    }

    fn title(&self) -> String {
        let filename = self.get_image_path().and_then(|f| f.file_name()).unwrap().to_str().unwrap();
        format!("imgvwr | {filename}")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Quit => {
                std::process::exit(0);
            },
            Message::Move(direction) => {
                self.switch_image(direction);
                self.view();
            },
            Message::Rotate(direction) => {
                self.rotate_image(direction);
                self.view();
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        // let handle = self.get_handle_from_path();
        let handle = self.get_handle_from_pixels();

        let viewer = Viewer::new(handle)
            .scale_step(self.scale)
            .min_scale(self.min_scale)
            .max_scale(self.max_scale)
            .content_fit(self.content_fit)
            .filter_method(self.filter_method)
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
                Key::Character("[") => {
                    Some(Message::Rotate(Direction::Previous))
                },
                Key::Character("]") => {
                    Some(Message::Rotate(Direction::Next))
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
