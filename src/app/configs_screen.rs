use std::fs;
use std::path::PathBuf;

use iced::event::Status;
use iced::widget::{
    button, column, container, row, scrollable, text, Column, Container, Row, Slider, Text
};
use iced::{Color, Command, Element, Event, Length};

use iced_aw::{Bootstrap, Wrap};
use iced_aw::widgets::NumberInput;

use crate::app::notification::info_message;
use crate::app::Message as AppMessage;
use crate::configs::{self, Configs};
use crate::{ icon, log, send_message, simple_button, thumbnail, ToPrettyString, VERSION };

use super::notification::error_message;

// IDs
// const THUMBNAIL_CACHE_INPUT_ID: fn() -> text_input::Id = || { text_input::Id::new("thumbnail_cache_size_input") };

pub const DESCRIPTION_TEXT_COLOR: Color = Color {
    r: 0.6,
    g: 0.6,
    b: 0.7,
    a: 1.0
};


/// Creates a new `NumberInput` with the given `value`, maxxing at `ty::MAX`,
/// firing the given [`Message`]`::Variant` as an [`AppMessage`]
/// Usage:
/// ```
/// number_input!(value, ty, Variant)
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


/// Create a configurable entry
fn config_row<'a>(
    name: &str,
    inner: Element<'a, AppMessage>,
) -> Column<'a, AppMessage>
{
    column![
        text(name),
        container(inner)
            .padding([8, 24]),
    ]
    .spacing(4)
    .width(Length::Fill)
    .padding([4, 48, 4, 24])
}



/// Create a configurable entry
fn config_entry<'a>(
    name: &str,
    description: Element<'a, AppMessage>,
    default: Option<String>,
    value: Element<'a, AppMessage>
) -> Row<'a, AppMessage>
{
    row![
        // Name & description
        column![
            text(name),
        ]
        .push_maybe(default.map(|str|
            Text::new(format!("Default: {str}"))
                .style(DESCRIPTION_TEXT_COLOR)
                .size(12)
        ))
        .push(container(description) .padding([8, 24]))
        .width(Length::FillPortion(2)),

        // Value
        container(value)
            .center_y()
            .width(Length::FillPortion(1)),
    ]
    .spacing(4)
    .width(Length::Fill)
    .padding([4, 48, 4, 24])
}




/// Dimmed text for descriptions
fn desc_text(text: &str) -> Text {
    Text::new(text)
        .style(DESCRIPTION_TEXT_COLOR)
        .size(14)
}




#[derive(Debug, Clone)]
pub enum Message {
    Save,
    OpenThumbnailCacheDir,
    ClearThumbnailCache,
    UpdateRateInput(u64),
    MaxResultsPerTickInput(usize),
    MaxResultCountChanged(usize),
    ThumbnailCacheSizeInput(u64),
    ThumbnailThreadCountInput(u8),
    ThumbnailUpdateProbInput(f32),
    ThumbnailCheckCountInput(u32),
    OpenConfigsDir,
    OpenLogsDir,
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

            Message::OpenThumbnailCacheDir => {
                let path = thumbnail::get_cache_dir_or_create();
                if let Err(err) = opener::open(&path) {
                    return send_message!(error_message(
                        format!("Failed to open {}:\n{:?}", path.to_pretty_string(), err)
                    ));
                }
            }

            Message::ClearThumbnailCache => {
                use std::io::ErrorKind;

                match thumbnail::clear_thumbnails_cache() {
                    Ok(()) => return send_message!(info_message( "Thumbnail cache cleared".to_string() )),
                    Err(err) => match err.kind() {
                        ErrorKind::NotFound => {},
                        _ => return send_message!(error_message(
                            format!("Failed to delete cache:\n{err:?}")
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

            Message::ThumbnailCacheSizeInput(input) => {
                self.is_dirty = true;
                self.configs.thumbnail_cache_size = input;
            }

            Message::ThumbnailThreadCountInput(input) => {
                self.is_dirty = true;
                self.configs.thumbnail_thread_count = input;
            }

            Message::ThumbnailUpdateProbInput(input) => {
                self.is_dirty = true;
                self.configs.thumbnail_update_prob = input;
            }

            Message::ThumbnailCheckCountInput(input) => {
                self.is_dirty = true;
                self.configs.thumbnail_check_count = input;
            }

            Message::OpenConfigsDir => {
                let path: PathBuf = match configs::get_save_path() {
                    Ok(p) => p,
                    Err(err) => return send_message!(error_message(
                        format!("Failed to get configs path:\n{err:?}")
                    )),
                };

                if let Err(err) = opener::reveal(&path) {
                    return send_message!(error_message(
                        format!("Failed to open \"{}\":\n{:?}", path.to_pretty_string(), err)
                    ));
                }
            }

            Message::OpenLogsDir => {
                let Some(path) = log::get_logs_dir() else {
                    return send_message!(error_message(
                        format!("Failed to get logs path")
                    ));
                };

                // The logs dir should already exist because it gets created in `Log::get_sink()`
                // This is just in case the user deletes it while the app is running
                if !path.exists() {
                    if let Err(err) = fs::create_dir_all(&path) {
                        return send_message!(error_message(
                            format!("Failed create logs path:\n{:?}", err)
                        ));
                    }
                }

                if let Err(err) = opener::open(&path) {
                    return send_message!(error_message(
                        format!("Failed to open \"{}\":\n{:?}", path.to_pretty_string(), err)
                    ));
                }
                
            }

        }

        Command::none()
    }

    pub fn view(&self) -> Element<AppMessage> {
        column![
            row![
                // Back arrow
                simple_button!(icon = Bootstrap::ArrowLeft)
                    .on_press(AppMessage::SwitchToMainScreen),

                // Save icon
                button( icon!(Bootstrap::FloppyFill) )
                    .on_press_maybe( self.is_dirty.then_some(Message::Save.into()) ),

                text("Settings") .size(24),
            ]
            .spacing(12),

            text(format!("Version {}", VERSION))
                .size(11),

            self.view_entries(),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_entries(&self) -> Container<AppMessage> {
        let c: &Configs = &self.configs;
        let default = Configs::default();

        container(scrollable(
            column![
                // UPDATE RATE
                config_entry(
                    "Update rate",
                    desc_text("Application's update rate, in milliseconds").into(),
                    Some(default.update_rate_ms.to_string()),
                    number_input!(c.update_rate_ms, u64, UpdateRateInput)
                        .min(1)
                        .into()
                ),

                // MAX RESULTS PER TICK
                config_entry(
                    "Max results per tick",
                    desc_text("How many search results to take every update tick").into(),
                    Some(default.max_results_per_tick.to_string()),
                    number_input!(c.max_results_per_tick, usize, MaxResultsPerTickInput)
                        .min(1)
                        .into()
                ),

                // MAX RESULT COUNT
                config_entry(
                    "Max result count",
                    desc_text("How many results to show all at once").into(),
                    Some(default.max_result_count.to_string()),
                    number_input!(c.max_result_count, usize, MaxResultCountChanged)
                        .min(1)
                        .into()
                ),

                // THUMBNAIL CACHE SIZE
                config_entry(
                    "Thumbnail cache size",
                    column![
                        row![
                            button("Open") .on_press(Message::OpenThumbnailCacheDir.into()),
                            button("Clear") .on_press(Message::ClearThumbnailCache.into()),
                        ]
                        .spacing(4),
                        desc_text("Size of the thumbnail cache in bytes.\nThis is not a hard limit, the actual size may fluctuate around this value"),
                    ]
                    .spacing(4)
                    .into(),
                    Some(format!("{} bytes", default.thumbnail_cache_size)),
                    number_input!(c.thumbnail_cache_size, u64, ThumbnailCacheSizeInput) .into()
                ),

                // THUMBNAIL THREAD COUNT
                config_entry(
                    "Thumbnail builder thread count",
                    desc_text("How many threads to use for building thumbnails") .into(),
                    Some(default.thumbnail_thread_count.to_string()),
                    number_input!(c.thumbnail_thread_count, u8, ThumbnailThreadCountInput)
                        .min(1)
                        .into()
                ),

                // THUMBNAIL REBUILD PROB
                config_entry(
                    "Thumbnail update prob",
                    desc_text("The probability that a file's thumbnail will be updated. 
Useful if you make changes to an image file while this app is open, but will cause unnecessary re-computation if set too high"
                    ).into(),
                    Some(format!( "{}%", (default.thumbnail_update_prob * 100.0).round() )),
                    column![
                        text(format!( "{}%", (c.thumbnail_update_prob * 100.0).round() )),
                        Slider::new(0.0..=1.0, c.thumbnail_update_prob, |v| Message::ThumbnailUpdateProbInput(v).into())
                            .step(0.01)
                    ]
                    .into()
                ),

                // THUMBNAIL MAX CHECK DEPTH
                config_entry(
                    "Thumbnail max check count",
                    desc_text("How many items to check at a time for building thumbnails
Setting this nnumber higher may increase building speed, but could be wasteful if set too high").into(),
                    Some(format!( "{} items", default.thumbnail_check_count )),
                    number_input!(c.thumbnail_check_count, u32, ThumbnailCheckCountInput)
                        .min(1)
                        .into()
                ),

                // MISCELLANEOUS
                // TODO use iced_aw::Grid
                config_row(
                    "Miscellaneous",
                    column![
                        button("Open configs directory").on_press(Message::OpenConfigsDir.into()),
                        button("Open logs directory").on_press(Message::OpenLogsDir.into()),
                    ]
                    .spacing(4)
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

