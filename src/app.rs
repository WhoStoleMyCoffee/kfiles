use std::sync::OnceLock;
use std::time::Duration;

use iced::event::Status;
use iced::widget::{container, Container};
use iced::{self, Event};
use iced::{time, Application, Command, Theme};

use crate::tag::{Tag, TagID};

pub mod mainscreen;
pub mod taglist;

use mainscreen::{ MainMessage, MainScreen };

use self::taglist::{TagListMessage, TagListScreen};

const UPDATE_RATE_MS: u64 = 100;

static TAGS_CACHE: OnceLock<Vec<TagID>> = OnceLock::new();



#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Event(Event, Status),
    Screen(ScreenMessage),
    SwitchToMainScreen,
    SwitchToTagListScreen,
}

impl From<ScreenMessage> for Message {
    fn from(value: ScreenMessage) -> Self {
        Self::Screen(value)
    }
}

impl From<MainMessage> for Message {
    fn from(value: MainMessage) -> Self {
        Message::Screen(ScreenMessage::Main(value))
    }
}



pub struct TagExplorer {
    current_screen: Screen,
}

impl Application for TagExplorer {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        // TODO find better cache system some day
        TAGS_CACHE.set(Tag::get_all_tag_ids().unwrap()).unwrap();

        let (main_screen, command) = MainScreen::new();

        (
            TagExplorer {
                current_screen: Screen::Main(main_screen),
            },
            command,
        )
    }

    fn title(&self) -> String {
        "Tag Explorer".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick => return self.current_screen.tick(),

            Message::Screen(screen_message) =>
                return self.current_screen.update(screen_message),

            Message::Event(event, status) => return self.handle_event(event, status),

            Message::SwitchToMainScreen => {
                let (main_screen, command) = MainScreen::new();
                self.current_screen = Screen::Main(main_screen);
                return command;
            }

            Message::SwitchToTagListScreen => {
                let (taglist_screen, command) = TagListScreen::new();
                self.current_screen = Screen::TagList(taglist_screen);
                return command;
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        container(self.current_screen.view()).into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::CatppuccinMocha // cat 🐈
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        iced::Subscription::batch(vec![
            time::every(Duration::from_millis(UPDATE_RATE_MS)).map(|_| Message::Tick),
            iced::event::listen_with(|event, status| Some(Message::Event(event, status))),
        ])
    }
}

impl TagExplorer {
    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        // match event { ... }
        
        self.current_screen.handle_event(event, status)
    }
}


#[derive(Debug, Clone)]
pub enum ScreenMessage {
    Main(MainMessage),
    TagList(TagListMessage),
}

#[derive(Debug)]
enum Screen {
    Main(MainScreen),
    TagList(TagListScreen),
    // Settings,
}

impl Screen {
    fn tick(&mut self) -> Command<Message> {
        match self {
            Self::Main(main) => main.tick(),
            _ => Command::none(),
        }
    }

    fn update(&mut self, message: ScreenMessage) -> Command<Message> {
        match (self, message) {
            (Screen::Main(main), ScreenMessage::Main(message)) =>
                main.update(message),
            (Screen::TagList(taglist), ScreenMessage::TagList(message)) =>
                taglist.update(message),
            _ => Command::none(),
        }
    }

    fn view(&self) -> Container<Message> {
        match self {
            Screen::Main(main) => main.view(),
            Screen::TagList(taglist) => taglist.view(),
        }
    }

    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        match self {
            Screen::Main(main) => main.handle_event(event, status),
            Screen::TagList(taglist) => taglist.handle_event(event, status),
        }
    }
}



