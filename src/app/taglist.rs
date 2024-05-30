use iced::event::Status;
use iced::widget::button;
use iced::widget::container;
use iced::widget::Container;
use iced::Event;
use iced::Command;

use crate::app::{ Message, TAGS_CACHE };



#[derive(Debug, Clone)]
pub enum TagListMessage {}




#[derive(Debug)]
pub struct TagListScreen {}

impl TagListScreen {
    pub fn new() -> (Self, Command<Message>) {
        (
            TagListScreen {},
            Command::none(),
        )
    }

    pub fn update(&mut self, message: TagListMessage) -> Command<Message> {
        Command::none()
    }

    pub fn view(&self) -> Container<Message> {
        container(
            button("back") .on_press(Message::SwitchToMainScreen)
        )
    }

    pub fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        Command::none()
    }
}

