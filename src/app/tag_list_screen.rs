use std::path::PathBuf;

use iced::event::Status;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, tooltip, Container};
use iced::{Command, Element, Event, Length};

use iced_aw::Bootstrap;

use crate::app::Message as AppMessage;
use crate::tagging::{ self, Tag, id::TagID };
use crate::widget::tag_entry::TagEntry as TagEntryWidget;
use crate::{ error, icon, send_message, simple_button, ToPrettyString };

use super::theme::ERROR_COLOR;


#[derive(Debug, Clone)]
pub enum Message {
    OpenTagsDir,
    CreateTag,
}

impl From<Message> for AppMessage {
    fn from(value: Message) -> AppMessage {
        AppMessage::Screen(super::ScreenMessage::TagList(value))
    }
}



#[derive(Debug)]
pub struct TagListScreen {
    error_message: Option<String>,
    loaded_tags: Vec<Tag>,
}

impl TagListScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        let load_res = tagging::load_tags();
        let error_message = load_res.log_errors::<String>();
        let tags_cache = load_res.get_tags().unwrap_or_default();
        tagging::set_tags_cache(tags_cache);

        (
            TagListScreen {
                error_message,
                loaded_tags: tagging::tags_cache().clone(),
            },
            Command::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::OpenTagsDir => {
                let path: PathBuf = tagging::get_save_dir();
                if let Err(err) = opener::open(&path) {
                    return send_message!(notif = error!(
                        notify, log_context = "TagListScreen::update() => OpenTagsDir";
                        "Failed to open {}:\n{}", path.to_pretty_string(), err
                    ));
                }
            }

            Message::CreateTag => {
                let tag_list = &tagging::tags_cache();

                let new_tag_id = TagID::new("new-tag") .make_unique_in(tag_list);
                let tag = Tag::create(new_tag_id);
                if let Err(err) = tag.save() {
                    return send_message!(notif = error!(
                        notify, log_context = "TagListScreen::update() => CreateTag";
                        "Failed to create tag:\n{}", err
                    ));
                }

                return send_message!(AppMessage::SwitchToTagEditScreen(tag))
            }

        }
        
        Command::none()
    }

    pub fn view(&self) -> Element<AppMessage> {
        let list = self.view_list();

        column![
            row![
                // Back arrow
                simple_button!(icon = Bootstrap::ArrowLeft)
                    .on_press(AppMessage::SwitchToMainScreen),
                text("Tags List") .size(24),
                horizontal_space(),
                tooltip(
                    simple_button!(icon = Bootstrap::Folder) .on_press(Message::OpenTagsDir.into()),
                    "Open tags directory",
                    tooltip::Position::Bottom
                ),
            ],

            tooltip(
                button( icon!(Bootstrap::BookmarkPlus) ) .on_press(Message::CreateTag.into()),
                "Create new tag",
                tooltip::Position::Right
            ),

            list
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_list(&self) -> Container<AppMessage> {
        // Get tags or display whatever

        if let Some(error_message) = &self.error_message {
            return container(
                text(error_message).style(ERROR_COLOR)
            );
        }


        // Contents
        container(scrollable(
            column(self.loaded_tags.iter().map(|t|
                // aaa i dont like the cloning
                TagEntryWidget::new(t)
                    .on_edit_pressed(AppMessage::SwitchToTagEditScreen(t.clone()))
                    .on_subtag_pressed(|id| match id.load() {
                        Ok(tag) => AppMessage::SwitchToTagEditScreen(tag),
                        Err(err) => AppMessage::Notify(error!(
                            notify;
                            "Failed to load tag \"{}\".\n{:?}", id, err
                        )),
                    })
                    .into()
            ))
            .width(Length::Fill)
            .padding(12.0)
            .spacing(12.0)
        ))

    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }
}



