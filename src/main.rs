pub mod args;
pub mod config;
pub mod util;
pub mod strings;

use iced::{
    advanced::Application,
    executor::Default as ExecutorDefault,
    keyboard::{
        self,
        key,
        Key,
    },
    widget::{
        container,
        image::{
            FilterMethod,
            Handle,
            Image
        },
        row
    },
    window::{
        Position,
        Settings as WindowSettings
    },
    Alignment,
    // Application,
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
use strings::messages;

fn main() -> Result {
    let mut args: Args = Args::parse();
    args.img = fs::canonicalize(&args.img).expect(messages::ERR_NO_INPUT_FILE);
    if !util::is_file_supported(&args.img).unwrap_or_default() {
        std::process::exit(0);
    }

    let config = args.get_config().unwrap_or_default();
    let flags = Flags::new(args, config);

    let window_config = flags.config.window.clone().unwrap_or_default();
    // TODO: use iced::run or iced::program() instead of implementing Application trait?
    Imgvwr::run(Settings {
        window: WindowSettings {
            position: Position::Centered,
            decorations: window_config.title.unwrap_or_default(),
            ..WindowSettings::default()
        },
        antialiasing: window_config.antialiasing.clone().unwrap_or_default(),
        flags,
        ..Settings::default()
    })
}

struct Imgvwr {
    image: DynamicImage,
    image_id: usize,
    images: Vec<PathBuf>,
    min_scale: f32,
    max_scale: f32,
    scale: f32,
    keybindings: Keybindings,
    theme: Theme,
    filter_method: FilterMethod,
    content_fit: ContentFit,
    rotation: f32,
}

impl Imgvwr {
    fn switch_image(&mut self, direction: Direction) {
        let current_id = &self.image_id;
        let max_id = self.images.len().checked_sub(1).unwrap();

        let image_id = match direction {
            Direction::Next => {
                match current_id + 1 {
                    next_id if next_id > max_id => 0,
                    next_id => next_id,
                }
            },
            Direction::Previous => {
                match current_id.checked_sub(1) {
                    Some(prev_id) => prev_id,
                    None => max_id,
                }
            }
        };

        let image_path = self.images.get(image_id).unwrap();
        self.image_id = image_id;
        self.image = Imgvwr::get_image(image_path)
    }

    fn get_image(image_path: &PathBuf) -> DynamicImage {
        let image = Reader::open(image_path)
            .expect(messages::ERR_CANT_OPEN_IMAGE)
            .decode()
            .expect(messages::ERR_CANT_DECODE_IMAGE);

        DynamicImage::ImageRgba8(image.into_rgba8())
    }

    fn rotate_image(&mut self, direction: Direction) {
        self.image = match direction {
            Direction::Next => { 
                self.rotation += 90.0_f32.to_radians();
                self.image.rotate90()
            }
            Direction::Previous => {
                self.rotation -= 90.0_f32.to_radians();
                self.image.rotate270()
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    Move(Direction),
    KeyPressed(String),
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
        let window = config.window.unwrap_or_default();

        let images = util::get_files(&flags.args.img).expect(messages::ERR_NO_FILES_INPUT_DIR);
        let image_id = util::get_file_id(&flags.args.img, &images).expect(messages::ERR_CANT_GET_FILE_ID);
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
                theme: Theme::from(window.theme.unwrap_or_default()),
                filter_method: FilterMethod::from(viewer.filter_method.unwrap_or_default()),
                content_fit: ContentFit::from(viewer.content_fit.unwrap_or_default()),
                rotation: 0.0,
            },
            Command::none()
        )
    }

    fn title(&self) -> String {
        let filename = self.images.get(self.image_id).and_then(|f| f.file_name()).unwrap().to_str().unwrap();
        let app_name = strings::APPLICATION_NAME;
        format!("{app_name} | {filename}")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        let keybindings = self.keybindings.clone();
        match message {
            Message::KeyPressed(key) => {
                match key {
                    _ if key == keybindings.quit.unwrap() => std::process::exit(0),
                    _ if key == keybindings.rotate_left.unwrap() => self.rotate_image(Direction::Previous),
                    _ if key == keybindings.rotate_right.unwrap() => self.rotate_image(Direction::Next),
                    _ => {}
                }
            }
            Message::Move(direction) => {
                self.switch_image(direction);
            },
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        // TODO: removal depends on #2330 and #2334
        let image = self.image.clone();
        let handle = Handle::from_pixels(image.width(), image.height(), image.into_bytes());

        let image_path = self.images.get(self.image_id).unwrap();
        let handle_2 = Handle::from_path(&image_path);

        let viewer = Image::new(handle)
            // .scale_step(self.scale)
            // .min_scale(self.min_scale)
            // .max_scale(self.max_scale)
            .content_fit(self.content_fit)
            .filter_method(self.filter_method)
            .width(Length::Fill)
            .height(Length::Fill);

        let image = Image::new(handle_2)
            // .scale_step(self.scale)
            // .min_scale(self.min_scale)
            // .max_scale(self.max_scale)
            .content_fit(self.content_fit)
            .filter_method(self.filter_method)
            .width(Length::Fill)
            .height(Length::Fill)
            .rotation(self.rotation);

        let content = row![viewer, image]
            .width(Length::Fill)
            .height(Length::Fill)
            .align_items(Alignment::Center);

        let container = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y();

        container.into()
        // viewer.into()
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, _modifiers,| {
            match key.as_ref() {
                Key::Character(c) => Some(Message::KeyPressed(c.to_string())),
                Key::Named(key) => {
                    let direction = match key {
                        key::Named::ArrowLeft => Some(Direction::Previous),
                        key::Named::ArrowRight => Some(Direction::Next),
                        _ => None
                    };

                    direction.map(Message::Move)
                },
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
