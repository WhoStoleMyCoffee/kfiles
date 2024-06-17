use iced::event::Status;
use iced::widget::{
    button, column, container, row, scrollable,
    text, Column, Container, Row
};
use iced::{Command, Element, Event, Length};

use iced_aw::Bootstrap;
use iced_aw::widgets::NumberInput;

use crate::app::notification::info_message;
use crate::app::Message as AppMessage;
use crate::configs::{self, Configs};
use crate::{ icon, send_message, simple_button, thumbnail, ToPrettyString };

use super::notification::error_message;

// IDs
// const THUMBNAIL_CACHE_INPUT_ID: fn() -> text_input::Id = || { text_input::Id::new("thumbnail_cache_size_input") };


/// Usage:
/// ```
/// number_input!(config.field, usize, MessageVariant)
/// ```
/// That's a weird lookin macro lol
macro_rules! number_input {
    ($n:expr, $type:ty, $msg:ident) => {
        NumberInput::new(
            $n,
            <$type>::MAX,
            |v| Message::$msg(v).into()
        )
    };
}



/// Could use `Into<Element>` but I wanna avoid too many generics
pub fn config_entry<'a>(
    name: &str,
    description: Element<'a, AppMessage>,
    value: Element<'a, AppMessage>
) -> Row<'a, AppMessage>
{
    row![
        // Name & description
        column![
            text(name),
            container(description) .padding([8, 24])
        ]
        .width(Length::FillPortion(1)),

        // Value
        container(value)
            .center_y()
            .width(Length::FillPortion(1)),
    ]
    .width(Length::Fill)
}




#[derive(Debug, Clone)]
pub enum Message {
    Save,
    ThumbnailCacheSizeInput(u64),
    OpenThumbnailCacheDir,
    ClearThumbnailCache,
    UpdateRateInput(u64),
    MaxResultsPerTickInput(usize),
    MaxResultCountChanged(usize),
}

impl From<Message> for AppMessage {
    fn from(value: Message) -> AppMessage {
        AppMessage::Screen(super::ScreenMessage::Configs(value))
    }
}


#[derive(Debug)]
pub struct ConfigsScreen {
    configs: Configs,
    is_dirty: bool,
}

impl ConfigsScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        let cfg = configs::global();

        (
            ConfigsScreen {
                configs: cfg.clone(),
                is_dirty: false,
            },
            Command::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::Save => {
                return self.save();
            }

            Message::ThumbnailCacheSizeInput(input) => {
                self.configs.thumbnail_cache_size = input;
                self.is_dirty = true;
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
                    Ok(()) => return send_message!(info_message( "Thumbnail cache cleared".to_string() )),
                    Err(err) => match err.kind() {
                        ErrorKind::NotFound => {},
                        _ => return send_message!(error_message(
                            format!("Failed to delete cache:\n{err}")
                        ))
                    }
                }
            }

            Message::UpdateRateInput(input) => {
                self.is_dirty = true;
                self.configs.update_rate_ms = input;
            }

            Message::MaxResultsPerTickInput(input) => {
                self.is_dirty = true;
                self.configs.max_results_per_tick = input;
            }

            Message::MaxResultCountChanged(input) => {
                self.is_dirty = true;
                self.configs.max_result_count = input;
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

                // Save icon
                button( icon!(Bootstrap::FloppyFill) )
                    .on_press_maybe( self.is_dirty.then_some(Message::Save.into()) ),

                text("Settings") .size(24),
                // horizontal_space(),
                // button("Open save directory") .on_press(Message::OpenTagsDir.into())
            ]
            .spacing(12),
            self.view_entries(),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
    }

    fn view_entries(&self) -> Container<AppMessage> {
        let c: &Configs = &self.configs;

        // TODO do the list rendering inside Configs struct?
        // TODO use macro?
        container(scrollable(
            column![
                // Update rate
                config_entry(
                    "Update rate",
                    "Application's update rate, in milliseconds".into(),
                    number_input!(c.update_rate_ms, u64, UpdateRateInput)
                        .min(1)
                        .into()
                ),

                // Max results per tick
                config_entry(
                    "Max results per tick",
                    "How many search results to take every update tick".into(),
                    number_input!(c.max_results_per_tick, usize, MaxResultsPerTickInput)
                        .min(1)
                        .into()
                ),

                // Max result count
                config_entry(
                    "Max result count",
                    "How many results to show all at once".into(),
                    number_input!(c.max_result_count, usize, MaxResultCountChanged)
                        .min(1)
                        .into()
                ),

                // Thumbnail cache size
                config_entry(
                    "Thumbnail cache size",
                    column![
                        row![
                            button("Open") .on_press(Message::OpenThumbnailCacheDir.into()),
                            button("Clear") .on_press(Message::ClearThumbnailCache.into()),
                        ],
                        "Size of the thumbnail cache in bytes.\nThis is not a hard limit, the actual size may fluctuate around this value",
                    ].into(),
                    number_input!(c.thumbnail_cache_size, u64, ThumbnailCacheSizeInput)
                        .into()
                ),

            ]
            .spacing(16)
            .width(Length::Fill)
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([12, 24])
    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }

    fn save(&mut self) -> Command<AppMessage> {
        self.is_dirty = false;

        *configs::global() = self.configs.clone();
        if let Err(err) = self.configs.save() {
            return send_message!(error_message(
                format!("Failed to save configs:\n{}", err)
            ));
        }

        send_message!(info_message( "Configs saved".to_string() ))
    }
}

