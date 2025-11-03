use crate::{
    config::{self, Keybindings},
    strings::{self, messages},
    util,
};
use iced::{
    ContentFit, Element, Length, Subscription, Task, exit,
    keyboard::{
        self,
        key::{Key, Named},
    },
    widget::image::{FilterMethod, Handle, Viewer},
};
use std::path::PathBuf;

pub struct Imgvwr {
    image_id: usize,
    images: Vec<PathBuf>,
    min_scale: f32,
    max_scale: f32,
    scale: f32,
    keybindings: Keybindings,
    filter_method: FilterMethod,
    content_fit: ContentFit,
    rotation: f32,
}

impl Imgvwr {
    fn switch_image(&mut self, direction: &Direction) -> Task<Message> {
        let current_id = self.image_id;
        let max_id = self.images.len().checked_sub(1).unwrap();

        let image_id = match direction {
            Direction::Next => match current_id + 1 {
                next_id if next_id > max_id => 0,
                next_id => next_id,
            },
            Direction::Previous => match current_id.checked_sub(1) {
                Some(prev_id) => prev_id,
                None => max_id,
            },
        };

        self.image_id = image_id;

        Task::none()
    }

    fn rotate_image(&mut self, rotation: &Rotation) -> Task<Message> {
        match rotation {
            Rotation::Right => {
                self.rotation += 90.0_f32.to_radians();
            }
            Rotation::Left => {
                self.rotation -= 90.0_f32.to_radians();
            }
        };

        Task::none()
    }

    pub fn new(
        img: &PathBuf,
        viewer: &config::Viewer,
        keybindings: config::Keybindings,
    ) -> (Self, Task<Message>) {
        let images = util::get_files(img).expect(messages::ERR_NO_FILES_INPUT_DIR);
        let image_id = util::get_file_id(img, &images).expect(messages::ERR_CANT_GET_FILE_ID);
        let min_scale = viewer.min_scale.unwrap_or_default();
        let max_scale = viewer.max_scale.unwrap();
        let scale = viewer.scale_step.unwrap();
        let filter_method = viewer.filter_method.to_owned().unwrap_or_default();
        let content_fit = viewer.content_fit.to_owned().unwrap_or_default();

        (
            Imgvwr {
                images,
                image_id,
                min_scale,
                max_scale,
                scale,
                keybindings,
                filter_method: FilterMethod::from(filter_method),
                content_fit: ContentFit::from(content_fit),
                rotation: 0.0,
            },
            Task::none(),
        )
    }

    pub fn title(&self) -> String {
        let filename = self
            .images
            .get(self.image_id)
            .and_then(|f| f.file_name())
            .unwrap()
            .to_str()
            .unwrap();
        let app_name = strings::APPLICATION_NAME;
        format!("{app_name} | {filename}")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyPressed(key) => match key {
                _ if key.eq(self.keybindings.quit.as_ref().unwrap()) => exit(),
                _ if key.eq(self.keybindings.rotate_left.as_ref().unwrap()) => {
                    self.rotate_image(&Rotation::Left)
                }
                _ if key.eq(self.keybindings.rotate_right.as_ref().unwrap()) => {
                    self.rotate_image(&Rotation::Right)
                }
                _ => Task::none(),
            },
            Message::Move(direction) => self.switch_image(&direction),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let image_path = self.images.get(self.image_id).unwrap();
        let handle = Handle::from_path(image_path);

        let viewer = Viewer::new(handle)
            .scale_step(self.scale)
            .min_scale(self.min_scale)
            .max_scale(self.max_scale)
            .content_fit(self.content_fit)
            .filter_method(self.filter_method)
            .width(Length::Fill)
            .height(Length::Fill)
            .rotation(self.rotation);

        viewer.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, _modifiers| match key.as_ref() {
            Key::Character(c) => Some(Message::KeyPressed(c.to_string())),
            Key::Named(key) => {
                let direction = match key {
                    Named::ArrowLeft => Some(Direction::Previous),
                    Named::ArrowRight => Some(Direction::Next),
                    _ => None,
                };

                direction.map(Message::Move)
            }
            _ => None,
        })
    }
}

#[derive(Clone, Debug)]
pub enum Direction {
    Next,
    Previous,
}

#[derive(Clone, Debug)]
pub enum Rotation {
    Right,
    Left,
}

#[derive(Clone, Debug)]
pub enum Message {
    Move(Direction),
    KeyPressed(String),
}
