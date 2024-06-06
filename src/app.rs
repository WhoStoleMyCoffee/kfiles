use std::time::Duration;

use iced::event::Status;
use iced::widget::container;
use iced::{self, Element, Event};
use iced::{time, Application, Command, Theme};

pub mod main_screen;
pub mod tag_list_screen;
pub mod tag_edit_screen;

use main_screen::MainScreen;

use crate::tag::Tag;

use self::tag_edit_screen::TagEditScreen;
use self::tag_list_screen::TagListScreen;

const UPDATE_RATE_MS: u64 = 100;


// TODO Message::NotifyError
#[derive(Debug, Clone)]
pub enum Message {
    /// Does nothing.
    /// Useful for widgets that are interactable but don't really do anything on their own
    Empty,
    Tick,
    Event(Event, Status),
    Screen(ScreenMessage),
    IconsFontLoaded(Result<(), iced::font::Error>),
    SwitchToMainScreen,
    SwitchToTagListScreen,
    SwitchToTagEditScreen(Tag),
}

impl From<ScreenMessage> for Message {
    fn from(value: ScreenMessage) -> Self {
        Self::Screen(value)
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
        let (main_screen, command) = MainScreen::new();

        (
            TagExplorer {
                current_screen: Screen::Main(main_screen),
            },
            Command::batch(vec![
                iced::font::load(iced_aw::BOOTSTRAP_FONT_BYTES).map(Message::IconsFontLoaded),
                command
            ]),
        )
    }

    fn title(&self) -> String {
        "Tag Explorer".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Empty => Command::none(),

            Message::Tick => self.current_screen.tick(),

            Message::Screen(screen_message) =>
                self.current_screen.update(screen_message),

            Message::Event(event, status) => self.handle_event(event, status),

            Message::IconsFontLoaded(res) => {
                if let Err(err) = res {
                    println!("ERROR Failed to load icons font: {err:?}");
                } else {
                    println!("INFO: Icons font loaded");
                }
                Command::none()
            }

            Message::SwitchToMainScreen => {
                let (main_screen, command) = MainScreen::new();
                self.current_screen = Screen::Main(main_screen);
                command
            }

            Message::SwitchToTagListScreen => {
                let (tag_list_screen, command) = TagListScreen::new();
                self.current_screen = Screen::TagList(tag_list_screen);
                command
            }

            Message::SwitchToTagEditScreen(tag) => {
                let (tag_edit_screen, command) = TagEditScreen::new(tag);
                self.current_screen = Screen::TagEdit(tag_edit_screen);
                command
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
    Main(main_screen::Message),
    TagList(tag_list_screen::Message),
    TagEdit(tag_edit_screen::Message),
}

#[derive(Debug)]
enum Screen {
    Main(MainScreen),
    TagList(TagListScreen),
    TagEdit(TagEditScreen),
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
        match message {
            ScreenMessage::Main(message) => if let Screen::Main(main) = self {
                return main.update(message);
            },

            ScreenMessage::TagList(message) => if let Screen::TagList(tag_list) = self {
                return tag_list.update(message);
            }

            ScreenMessage::TagEdit(message) => if let Screen::TagEdit(tag_edit) = self {
                return tag_edit.update(message);
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        match self {
            Screen::Main(main) => main.view().into(),
            Screen::TagList(tag_list) => tag_list.view().into(),
            Screen::TagEdit(tag_edit) => tag_edit.view().into(),
        }
    }

    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        match self {
            Screen::Main(main) => main.handle_event(event, status),
            Screen::TagList(tag_list) => tag_list.handle_event(event, status),
            Screen::TagEdit(tag_edit) => tag_edit.handle_event(event, status),
        }
    }
}



