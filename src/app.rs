use std::time::Duration;

use iced::event::Status;
use iced::widget::container;
use iced::{self, Element, Event};
use iced::{time, Application, Command, Theme};

pub mod mainscreen;
pub mod taglistscreen;

use mainscreen::MainScreen;

use self::taglistscreen::TagListScreen;

const UPDATE_RATE_MS: u64 = 100;


// TODO Message::NotifyError
#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Event(Event, Status),
    Screen(ScreenMessage),
    SwitchToMainScreen,
    SwitchToTagListScreen,
    IconsFontLoaded(Result<(), iced::font::Error>),
}

impl From<ScreenMessage> for Message {
    fn from(value: ScreenMessage) -> Self {
        Self::Screen(value)
    }
}

impl From<mainscreen::Message> for Message {
    fn from(value: mainscreen::Message) -> Self {
        Message::Screen(ScreenMessage::Main(value))
    }
}

impl From<taglistscreen::Message> for Message {
    fn from(value: taglistscreen::Message) -> Self {
        Message::Screen(ScreenMessage::TagList(value))
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
            Message::Tick => self.current_screen.tick(),

            Message::Screen(screen_message) =>
                self.current_screen.update(screen_message),

            Message::Event(event, status) => self.handle_event(event, status),

            Message::SwitchToMainScreen => {
                let (main_screen, command) = MainScreen::new();
                self.current_screen = Screen::Main(main_screen);
                command
            }

            Message::SwitchToTagListScreen => {
                let (taglist_screen, command) = TagListScreen::new();
                self.current_screen = Screen::TagList(taglist_screen);
                command
            }

            Message::IconsFontLoaded(res) => {
                if let Err(err) = res {
                    println!("ERROR Failed to load icons font: {err:?}");
                } else {
                    println!("INFO: Icons font loaded");
                }
                Command::none()
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
    Main(mainscreen::Message),
    TagList(taglistscreen::Message),
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

    fn view(&self) -> Element<Message> {
        match self {
            Screen::Main(main) => main.view().into(),
            Screen::TagList(taglist) => taglist.view().into(),
        }
    }

    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        match self {
            Screen::Main(main) => main.handle_event(event, status),
            Screen::TagList(taglist) => taglist.handle_event(event, status),
        }
    }
}



