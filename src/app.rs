use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::event::Status;
use iced::widget::{column, container, scrollable, text};
use iced::{self, Alignment, Border, Color, Element, Event, Length};
use iced::{time, Application, Command, Theme};
use iced_aw::{floating_element, Spinner};

pub mod main_screen;
pub mod tag_list_screen;
pub mod tag_edit_screen;
pub mod configs_screen;
pub mod file_action_screen;

use crate::log::notification::Notification;
use crate::tagging::Tag;
use crate::widget::notification_card::NotificationCard;
use crate::{configs, error, info, trace, ToPrettyString};

use main_screen::MainScreen;
use tag_edit_screen::TagEditScreen;
use tag_list_screen::TagListScreen;

use self::configs_screen::ConfigsScreen;
use self::file_action_screen::FileActionScreen;


/// Creates a [`iced::Command`] that produces the given message(s)
/// ```
/// // Send just one
/// send_message!(Message::MyMessage)
/// // Send multiple
/// send_message!(notif = $notif)
/// ```
#[macro_export]
macro_rules! send_message {
    (notif = $notif:expr) => {
        Command::perform(async {}, move |_|
            $crate::app::Message::Notify($notif)
        )
    };

    ($msg:expr) => {
        Command::perform(async {}, move |_| $msg)
    };
}



/// TODO documentation
#[derive(Debug, Clone)]
enum FileHoverState {
    None,
    Hovering(Vec<PathBuf>),
    Dropping {
        paths: Vec<PathBuf>,
        expected_remaining: Option<usize>,
        drop_start: Instant,
    },
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
    CloseNotification(usize),
    Notify(Notification),
    OpenPath(PathBuf),

    SwitchToMainScreen,
    SwitchToTagListScreen,
    SwitchToTagEditScreen(Tag),
    SwitchToConfigScreen,
    SwitchToFileActionScreen(Vec<PathBuf>),
}




pub struct KFiles {
    current_screen: Screen,
    notifications: Vec<Notification>,
    has_focus: Option<Instant>,
    hovered_files: FileHoverState,
}

impl Application for KFiles {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        trace!("Program started");
    
        let (main_screen, command) = MainScreen::new();

        (
            KFiles {
                current_screen: Screen::Main(main_screen),
                notifications: Vec::new(),
                has_focus: None,
                hovered_files: FileHoverState::None,
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

                if let FileHoverState::Dropping { paths, expected_remaining, drop_start } = &self.hovered_files {
                    if expected_remaining.is_some_and(|x| x == 0) || drop_start.elapsed() >= KFiles::FILE_DROP_BUFFER {
                        let paths = paths.to_vec();
                        self.hovered_files = FileHoverState::None;
                        return self.drop_paths(paths);
                    }
                }

                self.current_screen.tick()
            }

            Message::Screen(screen_message) =>
                self.current_screen.update(screen_message),

            Message::Event(event, status) => self.handle_event(event, status),

            Message::IconsFontLoaded(res) => {
                if let Err(err) = res {
                    error!("Failed to load icons font:\n {err:?}");
                } else {
                    info!("Icons font successfully loaded");
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

            Message::SwitchToFileActionScreen(paths) => {
                let (file_action_screen, command) = FileActionScreen::new(paths);
                self.current_screen = Screen::FileAction(file_action_screen);
                command
            },

            Message::CloseNotification(index) => {
                if index < self.notifications.len() {
                    self.notifications.remove(index);
                }
                Command::none()
            }

            Message::Notify(notification) => {
                self.notify(notification);
                Command::none()
            }

            Message::OpenPath(path) => {
                self.open_path(&path)
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        match &self.hovered_files {
            FileHoverState::None => {
                self.view_main()
            },

            FileHoverState::Hovering(_) => {
                let palette = self.theme().palette();
                let border_color: Color = palette.primary;

                container(
                    container( text("Drop files...") )
                        .style(container::Appearance {
                            border: Border {
                                color: border_color,
                                width: 3.0,
                                radius: 4.0.into(),
                            },
                            ..Default::default()
                        })
                        .center_x()
                        .center_y()
                        .width(600)
                        .height(300)
                )
                .center_x()
                .center_y()
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            },

            FileHoverState::Dropping { .. } => {
                let palette = self.theme().palette();
                let border_color: Color = palette.text;

                container(
                    container(Spinner::new())
                    .style(container::Appearance {
                        border: Border {
                            color: border_color,
                            width: 3.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .center_x()
                    .center_y()
                    .width(600)
                    .height(300)
                )
                .center_x()
                .center_y()
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            },
        }
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
    /// Don't register window focuses for this long
    /// This is necessary to avoid bugs caused by some input events being registered too quickly.
    /// E.g. `:q<ENTER>` closes a VIM window, which re-focuses this app, and the same `<ENTER>` key
    /// press getting registered to open the first search result in [`MainScreen`]
    const FOCUS_BUFFER: Duration = Duration::from_millis(500);

    /// TODO documentation
    const FILE_DROP_BUFFER: Duration = Duration::from_millis(2000);

    fn view_main(&self) -> iced::Element<'_, Message, iced::Theme, iced::Renderer> {
        let screen = self.current_screen.view();

        let notifications = scrollable(
            column(self.notifications.iter().enumerate().map(|(i, n)| {
                NotificationCard::from_notification(n)
                    .on_close(Message::CloseNotification(i))
                    .into()
            }))
            .width(400)
            .align_items(Alignment::End),
        );

        floating_element(screen, notifications)
            .anchor(floating_element::Anchor::SouthEast)
            .into()
    }

    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        use iced::window::Event as WindowEvent;

        match event {
            // WINDOW EVENT
            Event::Window(_, ref window_event) => match window_event {
                WindowEvent::Focused => {
                    self.has_focus = Some(Instant::now());
                }

                WindowEvent::Unfocused => {
                    self.has_focus = None;
                },

                WindowEvent::FileDropped(path) => {
                    let path: PathBuf = path.clone();

                    match &mut self.hovered_files {
                        FileHoverState::Dropping { paths, expected_remaining, .. } => {
                            paths.push(path);
                            if let Some(count) = expected_remaining {
                                *count -= 1;
                            }

                        },

                        FileHoverState::Hovering(paths) => {
                            self.hovered_files = FileHoverState::Dropping {
                                paths: vec![ path ],
                                expected_remaining: Some(paths.len() - 1),
                                drop_start: Instant::now(),
                            };
                        },

                        FileHoverState::None => {
                            self.hovered_files = FileHoverState::Dropping {
                                paths: vec![ path ],
                                expected_remaining: None,
                                drop_start: Instant::now(),
                            };
                        },
                    }
                },

                WindowEvent::FileHovered(path) => {
                    let path: PathBuf = path.clone();

                    match &mut self.hovered_files {
                        FileHoverState::Hovering(files) => {
                            files.push(path);
                        },
                        _ => {
                            self.hovered_files = FileHoverState::Hovering(vec![ path ])
                        },
                    }
                },

                WindowEvent::FilesHoveredLeft => {
                    self.hovered_files = FileHoverState::None;
                },

                _ => {},
            },

            _ => {},
        }

        self.current_screen.handle_event(event, status)
    }

    #[inline]
    fn update_notifications(&mut self) {
        self.notifications.retain(|n| !n.is_expired());
    }

    #[inline]
    pub fn notify(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Opens the given `path` using [`opener`], and returns any errors as [`Command`]s containing
    /// [`Notification`]s
    pub fn open_path(&self, path: &Path) -> Command<Message> {
        if !self.has_focus() {
            return Command::none();
        }

        trace!("[KFiles::open_path()] Opening path \"{}\"", path.display());
        let Err(err) = opener::open(path) else {
            return Command::none();
        };

        // Open path
        let pathstr: String = path.to_pretty_string();
        let mut command = send_message!(notif = error!(
            notify;
            "Failed to open \"{}\":\n{:?}\nRevealing in file explorer instead", pathstr, err
        ));

        // Open path failed
        // Reveal it in parent dir insetad
        let res = opener::reveal(path).err()
            // If that fails, open parent dir
            .and_then(|err| {
                let Some(path) = path.parent() else { return Some(err); };
                error!("Reveal path failed, opening parent dir instead");
                opener::open(path).err()
            });
        if let Some(err) = res {
            let pathstr: String = path.to_pretty_string();
            command = Command::batch(vec![
                command,
                send_message!(notif = error!(
                    notify;
                    "Failed to reveal {}:\n{:?}", pathstr, err
                )),
            ]);
        }

        command
    }

    pub fn has_focus(&self) -> bool {
        self.has_focus.is_some_and(|t| t.elapsed() > KFiles::FOCUS_BUFFER)
    }

    pub fn drop_paths(&mut self, paths: Vec<PathBuf>) -> Command<Message> {
        let command = match &mut self.current_screen {
            Screen::FileAction(file_action_screen) => {
                file_action_screen.append(paths);
                Command::none()
            },
            _ => {
                let (file_action_screen, command) = FileActionScreen::new(paths);
                self.current_screen = Screen::FileAction(file_action_screen);
                command
            }
        };

        command
    }
}


#[derive(Debug, Clone)]
pub enum ScreenMessage {
    Main(main_screen::Message),
    TagList(tag_list_screen::Message),
    TagEdit(tag_edit_screen::Message),
    Configs(configs_screen::Message),
    FileAction(file_action_screen::Message),
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
    FileAction(FileActionScreen),
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

            ScreenMessage::FileAction(message) => if let Screen::FileAction(file_action) = self {
                return file_action.update(message);
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
            Screen::FileAction(file_action) => file_action.view(),
        }
    }

    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        match self {
            Screen::Main(main) => main.handle_event(event, status),
            Screen::TagList(tag_list) => tag_list.handle_event(event, status),
            Screen::TagEdit(tag_edit) => tag_edit.handle_event(event, status),
            Screen::Configs(configs) => configs.handle_event(event, status),
            Screen::FileAction(file_action) => file_action.handle_event(event, status),
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

        fn active(&self, style: &Self::Style) -> button::Appearance {
            let palette = style.palette();

            button::Appearance {
                background: Some(Color::new(0.17, 0.17, 0.24, 1.0).into()),
                border: iced::Border::with_radius(4.0),
                text_color: palette.text,
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
                    0.9,
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






