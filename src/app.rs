use std::time::Duration;

use iced::event::Status;
use iced::widget::{column, scrollable};
use iced::{self, Alignment, Element, Event};
use iced::{time, Application, Command, Theme};
use iced_aw::floating_element;

pub mod main_screen;
pub mod tag_list_screen;
pub mod tag_edit_screen;

use main_screen::MainScreen;
use crate::tag::Tag;
use crate::widget::notification_card::NotificationCard;
use self::notification::Notification;
use self::tag_edit_screen::TagEditScreen;
use self::tag_list_screen::TagListScreen;

const UPDATE_RATE_MS: u64 = 100;



/// Creates a [`iced::Command`] that produces the given message(s)
/// ```
/// // Send just one
/// send_message!(Message::MyMessage)
/// // Send multiple
/// send_message![ Message::MyMessage, Message::MyMessage2 ]
/// ```
#[macro_export]
macro_rules! send_message {
    ($msg:expr) => {
        Command::perform(async { () }, move |_| $msg)
    };

    ($($msg:expr),*$(,)?) => {
        Command::batch(vec![
            $(
                Command::perform(async{ () }, move |_| $msg),
             )*
        ])
    };
}



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
    CloseNotification(usize),
    Notify(Notification),
}



pub struct TagExplorer {
    current_screen: Screen,
    notifications: Vec<Notification>,
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
                notifications: Vec::new(),
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

            Message::Tick => {
                self.update_notifications();
                self.current_screen.tick()
            }

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

            Message::CloseNotification(index) => {
                if index < self.notifications.len() {
                    self.notifications.remove(index);
                }
                Command::none()
            }

            Message::Notify(notification) => {
                self.notifications.push(notification);
                Command::none()
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        let screen = self.current_screen.view();

        let notifications = scrollable(
            column(self.notifications.iter().enumerate()
                .map(|(i, n)| NotificationCard::from_notification(n)
                     .on_close(Message::CloseNotification(i))
                     .into()
                )
            )
            .width(400)
            .align_items(Alignment::End)
        );

        floating_element(screen, notifications)
            .anchor(floating_element::Anchor::SouthEast)
            .into()
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
        #[cfg(debug_assertions)]
        {
            match event {
                Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
                    modifiers,
                    .. 
                }) if modifiers.is_empty() =>
                {
                    return send_message![
                        Message::Notify(Notification::new(
                                notification::Type::Info,
                                "You've got mail!".to_string(),
                        )),
                        Message::Notify(Notification::new(
                                notification::Type::Warning,
                                "You've got mail!".to_string(),
                        )),
                        Message::Notify(Notification::new(
                                notification::Type::Error,
                                "You've got mail!".to_string(),
                        )),
                    ];
                },
                _ => {},
            }
        }

        self.current_screen.handle_event(event, status)
    }

    #[inline]
    fn update_notifications(&mut self) {
        self.notifications.retain(|n| !n.is_expired());
    }
}


#[derive(Debug, Clone)]
pub enum ScreenMessage {
    Main(main_screen::Message),
    TagList(tag_list_screen::Message),
    TagEdit(tag_edit_screen::Message),
}

impl From<ScreenMessage> for Message {
    fn from(value: ScreenMessage) -> Self {
        Self::Screen(value)
    }
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


pub mod notification {
    use std::time::{Duration, Instant};
    use iced::{widget::Text, Color};
    use iced_aw::Bootstrap;

    use crate::{app, icon};

    /// Create a new error notification wrapped in [`app::Message::Notify`]
    /// The created [`Notification`] will have a default lifetime of 10 seconds
    pub fn error_message(content: String) -> app::Message {
        app::Message::Notify(
            Notification::new(
                Type::Error,
                content,
            )
            .with_lifetime(Duration::from_secs(10))
        )
    }


    #[derive(Debug, Clone)]
    pub enum Type {
        Text(String),
        Info,
        Warning,
        Error,
    }

    impl Type {
        const INFO_COLOR: Color = Color { r: 0.2, g: 0.8, b: 1.0, a: 1.0 };
        const WARNING_COLOR: Color = Color { r: 0.95, g: 0.9, b: 0.2, a: 1.0 };
        const ERROR_COLOR: Color = Color { r: 1.0, g: 0.2, b: 0.2, a: 1.0 };

        pub fn get_icon(&self) -> Option<Text> {
            match self {
                Type::Text(_) => None,
                Type::Info => Some(icon!(Bootstrap::InfoCircle, Self::INFO_COLOR)),
                Type::Warning => Some(icon!(Bootstrap::ExclamationTriangle, Self::WARNING_COLOR)),
                Type::Error => Some(icon!(Bootstrap::XLg, Self::ERROR_COLOR)),
            }
        }

        pub fn get_title(&self) -> &str {
            match self {
                Type::Text(title) => &title,
                Type::Info => "Info",
                Type::Warning => "Warning",
                Type::Error => "Error",
            }
        }
    }


    #[derive(Debug, Clone)]
    pub struct Notification {
        pub notification_type: Type,
        pub content: String,
        pub expire_at: Option<Instant>,
    }

    impl Notification {
        pub fn new(notification_type: Type, content: String) -> Self {
            Notification {
                notification_type,
                content,
                expire_at: None,
            }
        }

        pub fn with_lifetime(mut self, duration: Duration) -> Self {
            self.expire_at = Some(Instant::now() + duration);
            self
        }

        /* pub fn will_expire(&self) -> bool {
            self.expire_at.is_some()
        } */

        pub fn is_expired(&self) -> bool {
            if let Some(expiration) = self.expire_at {
                return Instant::now() >= expiration;
            }
            false
        }

        #[inline]
        pub fn get_title(&self) -> &str {
            self.notification_type.get_title()
        }

        #[inline]
        pub fn get_icon(&self) -> Option<Text> {
            self.notification_type.get_icon()
        }
    }



}
