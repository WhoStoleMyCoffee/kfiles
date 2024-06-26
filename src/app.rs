use std::path::Path;
use std::time::Duration;

use iced::event::Status;
use iced::widget::{column, scrollable};
use iced::{self, Alignment, Element, Event};
use iced::{time, Application, Command, Theme};
use iced_aw::floating_element;

pub mod main_screen;
pub mod tag_list_screen;
pub mod tag_edit_screen;
pub mod configs_screen;

use crate::tagging::Tag;
use crate::widget::notification_card::NotificationCard;
use crate::{configs, ToPrettyString};

use notification::Notification;
use main_screen::MainScreen;
use tag_edit_screen::TagEditScreen;
use tag_list_screen::TagListScreen;

use self::configs_screen::ConfigsScreen;



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
        Command::perform(async {}, move |_| $msg)
    };

    ($($msg:expr),*$(,)?) => {
        Command::batch(vec![
            $(
                Command::perform(async{}, move |_| $msg),
             )*
        ])
    };
}



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
    SwitchToConfigScreen,
    CloseNotification(usize),
    Notify(Notification),
}



pub struct KFiles {
    current_screen: Screen,
    notifications: Vec<Notification>,
}

impl Application for KFiles {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (main_screen, command) = MainScreen::new();

        (
            KFiles {
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
        "KFiles".to_string()
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

            Message::SwitchToConfigScreen => {
                let (configs_screen, command) = ConfigsScreen::new();
                self.current_screen = Screen::Configs(configs_screen);
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
        let update_rate = configs::global().update_rate_ms;

        iced::Subscription::batch(vec![
            time::every(Duration::from_millis(update_rate))
                .map(|_| Message::Tick),
            iced::event::listen_with(|event, status|
                Some(Message::Event(event, status))
            ),
        ])
    }
}

impl KFiles {
    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        self.current_screen.handle_event(event, status)
    }

    #[inline]
    fn update_notifications(&mut self) {
        self.notifications.retain(|n| !n.is_expired());
    }

    pub fn open_path(path: &Path) -> Command<Message> {
        let Err(err) = opener::open(path) else {
            return Command::none();
        };

        let pathstr: String = path.to_pretty_string();
        let mut command = send_message!(Message::Notify(Notification::new(
            notification::Type::Info,
            format!("Failed to open \"{}\":\n{}\nRevealing in file explorer instead", pathstr, err)
        )));

        if let Err(err) = opener::reveal(path) {
            let pathstr: String = path.to_pretty_string();
            command = Command::batch(vec![
                command,
                send_message!(notification::error_message(
                    format!("Failed to reveal {}:\n{}", pathstr, err)
                )),
            ]);
        }

        command
    }
}


#[derive(Debug, Clone)]
pub enum ScreenMessage {
    Main(main_screen::Message),
    TagList(tag_list_screen::Message),
    TagEdit(tag_edit_screen::Message),
    Configs(configs_screen::Message),
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
    Configs(ConfigsScreen),
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

            ScreenMessage::Configs(message) => if let Screen::Configs(configs) = self {
                return configs.update(message);
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        match self {
            Screen::Main(main) => main.view(),
            Screen::TagList(tag_list) => tag_list.view(),
            Screen::TagEdit(tag_edit) => tag_edit.view(),
            Screen::Configs(configs) => configs.view(),
        }
    }

    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        match self {
            Screen::Main(main) => main.handle_event(event, status),
            Screen::TagList(tag_list) => tag_list.handle_event(event, status),
            Screen::TagEdit(tag_edit) => tag_edit.handle_event(event, status),
            Screen::Configs(configs) => configs.handle_event(event, status),
        }
    }
}





pub mod theme {
    use iced::{theme, Border, Color, Vector};
    use iced::widget::button;
    use iced::overlay::menu;

    pub const LIGHT_TEXT_COLOR: Color = Color { r: 0.8, g: 0.84, b: 0.95, a: 1.0 };
    pub const INFO_COLOR: Color = Color { r: 0.2, g: 0.8, b: 1.0, a: 1.0 };
    pub const WARNING_COLOR: Color = Color { r: 0.95, g: 0.9, b: 0.2, a: 1.0 };
    pub const ERROR_COLOR: Color = Color { r: 1.0, g: 0.2, b: 0.2, a: 1.0 };

    pub struct Simple;

    impl button::StyleSheet for Simple {
        type Style = iced::Theme;

        fn active(&self, _style: &Self::Style) -> button::Appearance {
            button::Appearance {
                background: Some(Color::new(0.17, 0.17, 0.24, 1.0).into()),
                border: iced::Border::with_radius(4.0),
                ..Default::default()
            }
        }

        fn hovered(&self, style: &Self::Style) -> button::Appearance {
            let active = self.active(style);

            button::Appearance {
                background: Some(Color::new(0.20, 0.20, 0.28, 1.0).into()),
                shadow_offset: active.shadow_offset + Vector::new(0.0, 1.0),
                ..active
            }
        }

        /// Produces the pressed [`Appearance`] of a button.
        fn pressed(&self, style: &Self::Style) -> button::Appearance {
            button::Appearance {
                background: Some(Color::new(0.16, 0.15, 0.23, 1.0).into()),
                shadow_offset: Vector::default(),
                ..self.active(style)
            }
        }
    }

    impl From<Simple> for theme::Button {
        fn from(value: Simple) -> Self {
            theme::Button::custom(value)
        }
    }



    impl menu::StyleSheet for Simple {
        type Style = iced::Theme;

        fn appearance(&self, style: &Self::Style) -> menu::Appearance {
            let palette = style.palette();

            menu::Appearance {
                text_color: palette.text,
                background: Color::new(
                    palette.background.r * 1.19,
                    palette.background.g * 1.2,
                    palette.background.b * 1.15,
                    0.98,
                ).into(),
                border: Border::default(),
                selected_text_color: palette.text,
                selected_background: Color::new(
                    palette.background.r * 0.81,
                    palette.background.g * 0.8,
                    palette.background.b * 0.85,
                    1.0,
                ).into(),
            }
        }
    }

    impl From<Simple> for theme::Menu {
        fn from(value: Simple) -> Self {
            use std::rc::Rc;
            theme::Menu::Custom(Rc::new( value ))
        }
    }


}





pub mod notification {
    use std::time::{Duration, Instant};
    use iced::widget::Text;
    use iced_aw::Bootstrap;

    use crate::{app, icon};

    /// Create a new error notification wrapped in [`app::Message::Notify`]
    /// The created [`Notification`] will have the default lifetime
    pub fn error_message(content: String) -> app::Message {
        app::Message::Notify(
            Notification::new(
                Type::Error,
                content,
            )
        )
    }

    /// Create a new warning notification wrapped in [`app::Message::Notify`]
    /// The created [`Notification`] will have the default lifetime
    pub fn warning_message(content: String) -> app::Message {
        app::Message::Notify(
            Notification::new(
                Type::Warning,
                content,
            )
        )
    }
    
    /// Create a new info notification wrapped in [`app::Message::Notify`]
    /// The created [`Notification`] will have the default lifetime
    pub fn info_message(content: String) -> app::Message {
        app::Message::Notify(
            Notification::new(
                Type::Info,
                content,
            )
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
        pub fn get_icon(&self) -> Option<Text> {
            use app::theme;
            match self {
                Type::Text(_) => None,
                Type::Info => Some(icon!(Bootstrap::InfoCircle, theme::INFO_COLOR)),
                Type::Warning => Some(icon!(Bootstrap::ExclamationTriangle, theme::WARNING_COLOR)),
                Type::Error => Some(icon!(Bootstrap::ExclamationTriangleFill, theme::ERROR_COLOR)),
            }
        }

        pub fn get_title(&self) -> &str {
            match self {
                Type::Text(title) => title,
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
        pub const DEFAULT_LIFETIME: f32 = 10.0;

        /// Create a new [`Notification`]
        pub fn new(notification_type: Type, content: String) -> Self {
            Notification {
                notification_type,
                content,
                expire_at: Some(Instant::now() + Duration::from_secs_f32(Notification::DEFAULT_LIFETIME)),
            }
        }

        pub fn no_expiration(mut self) -> Self {
            self.expire_at = None;
            self
        }

        pub fn with_lifetime(mut self, duration_seconds: f32) -> Self {
            self.expire_at = Some(Instant::now() + Duration::from_secs_f32(duration_seconds));
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

