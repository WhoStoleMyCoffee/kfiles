use iced::event::Status;
use iced::widget::{
    button, column, container, row, scrollable,
    text, text_input, Column, Container, Row
};
use iced::{Command, Element, Event, Length};

use iced_aw::Bootstrap;

use crate::app::Message as AppMessage;
use crate::configs::{self, Configs};
use crate::widget::operations;
use crate::{ icon, send_message, simple_button, thumbnail, ToPrettyString };

use super::notification::error_message;

// IDs
const THUMBNAIL_CACHE_INPUT_ID: fn() -> text_input::Id = || { text_input::Id::new("thumbnail_cache_size_input") };



#[derive(Debug, Clone)]
pub enum Message {
    ThumbnailCacheSizeInput(String),
    ThumbnailCacheSizeSubmit,
    OpenThumbnailCacheDir,
    ClearThumbnailCache,
}

impl From<Message> for AppMessage {
    fn from(value: Message) -> AppMessage {
        AppMessage::Screen(super::ScreenMessage::Configs(value))
    }
}


#[derive(Debug)]
pub struct ConfigsScreen {
    configs: Configs,
    thumbnail_cache_size: String,
}

impl ConfigsScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        let cfg = configs::global();

        (
            ConfigsScreen {
                configs: cfg.clone(),
                thumbnail_cache_size: cfg.thumbnail_cache_size.to_string(),
            },
            Command::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::ThumbnailCacheSizeInput(input) => {
                self.thumbnail_cache_size = input
            }

            Message::ThumbnailCacheSizeSubmit => {
                return Command::batch(vec![
                    self.apply(),
                    Command::widget( operations::unfocus(THUMBNAIL_CACHE_INPUT_ID().into()) )
                        .map(|()| AppMessage::Empty),
                ])
            }

            Message::OpenThumbnailCacheDir => {
                let path = thumbnail::get_cache_dir_or_create();
                if let Err(err) = opener::open(&path) {
                    return send_message!(error_message(
                        format!("Failed to open {}:\n{}", path.to_pretty_string(), err)
                    ));
                }
            }

            Message::ClearThumbnailCache => {
                use std::io::ErrorKind;

                match thumbnail::clear_thumbnails() {
                    Ok(()) => {},
                    Err(err) => match err.kind() {
                        ErrorKind::NotFound => {},
                        _ => return send_message!(error_message(
                            format!("Failed to delete cache:\n{err}")
                        ))
                    }
                }
            }

        }

        Command::none()
    }

    pub fn view(&self) -> Column<AppMessage> {
        column![
            row![
                // Back arrow
                simple_button!(icon = Bootstrap::ArrowLeft)
                    .on_press(AppMessage::SwitchToMainScreen),
                text("Settings") .size(24),
                // horizontal_space(),
                // button("Open save directory") .on_press(Message::OpenTagsDir.into())
            ],
            self.view_entries(),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
    }

    fn view_entries(&self) -> Container<AppMessage> {
        // TODO do the list rendering inside Configs struct?
        container(scrollable(
            column![

                // Thumbnail cache size
                config_entry(
                    "Thumbnail cache size",
                    Some(column![
                        row![
                            button("Open") .on_press(Message::OpenThumbnailCacheDir.into()),
                            button("Clear") .on_press(Message::ClearThumbnailCache.into()),
                        ],
                        "Size of the thumbnail cache in bytes",
                    ].into()),
                    text_input("bytes", &self.thumbnail_cache_size)
                        .id(THUMBNAIL_CACHE_INPUT_ID())
                        .on_input(|s| Message::ThumbnailCacheSizeInput(s).into())
                        .on_submit(Message::ThumbnailCacheSizeSubmit.into())
                        .into()
                ),

            ]
            .spacing(12)
            .width(Length::Fill)
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([12, 24])
    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }

    /// TODO documentation
    fn apply(&mut self) -> Command<AppMessage> {
        // Thumbnail cache size
        match self.thumbnail_cache_size.parse::<u64>() {
            Ok(v) => self.configs.thumbnail_cache_size = v,
            Err(err) => {
                self.thumbnail_cache_size = self.configs.thumbnail_cache_size.to_string();
                println!("TODO error handling {err}");
            }
        }

        Command::none()
    }
}





/// Could use `Into<Element>` but I wanna avoid too many generics
pub fn config_entry<'a>(
    name: &str,
    description: Option<Element<'a, AppMessage>>,
    value: Element<'a, AppMessage>
) -> Row<'a, AppMessage>
{
    row![
        // Name & description
        Column::new()
            .push(text(name))
            .push_maybe(description.map(|desc|
                container(desc)
                    .padding([8, 24])
            ))
            .width(Length::FillPortion(1)),

        // Value
        container(value)
            .center_y()
            .width(Length::FillPortion(1)),
    ]
    .width(Length::Fill)
}



