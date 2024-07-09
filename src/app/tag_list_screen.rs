use std::path::PathBuf;
use std::sync::Arc;

use iced::event::Status;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, tooltip, Container};
use iced::{Command, Element, Event, Length};

use iced_aw::spinner;
use iced_aw::Bootstrap;

use crate::app::Message as AppMessage;
use crate::tagging::{ self, Tag, id::TagID };
use crate::widget::tag_entry::TagEntry as TagEntryWidget;
use crate::{ error, icon, send_message, simple_button, ToPrettyString };

use super::theme::ERROR_COLOR;

type LoadTagsResult = Result< Vec<Tag>, Arc<tagging::LoadError> >;


#[derive(Debug, Clone)]
pub enum Message {
    TagsLoaded(LoadTagsResult),
    OpenTagsDir,
    CreateTag,
}

impl From<Message> for AppMessage {
    fn from(value: Message) -> AppMessage {
        AppMessage::Screen(super::ScreenMessage::TagList(value))
    }
}


#[derive(Debug)]
enum TagList {
    Loading,
    Loaded(Vec<Tag>),
    Failed(Option<tagging::LoadError>),
}



#[derive(Debug)]
pub struct TagListScreen {
    tags: TagList,
}

impl TagListScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        (
            TagListScreen {
                tags: TagList::Loading,
            },
            Command::perform(
                load_tags(),
                |res| Message::TagsLoaded(res).into()
            ),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::TagsLoaded(result) => {
                self.tags = match result {
                    Ok(tags) => TagList::Loaded(tags.into_iter()
                        .collect()),
                    Err(err) => TagList::Failed( Arc::into_inner(err) ),
                };
            }

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
                let TagList::Loaded(tag_list) = &mut self.tags else {
                    return Command::none();
                };

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
        let tags: &Vec<Tag> = match &self.tags {
            TagList::Loaded(tags) => tags,

            TagList::Loading => {
                return container(spinner::Spinner::new()
                    .width(Length::Fixed(48.0))
                    .height(Length::Fixed(48.0))
                );
            }

            TagList::Failed(err_maybe) => {
                let error_message: String = err_maybe.as_ref().map_or(
                    "Reason unknown. Arc::into_inner() returned None".to_string(),
                    |err| err.to_string(),
                );
                let error_message = format!("Failed to load tags:\n{}", error_message);

                return container(
                    text(error_message).style(ERROR_COLOR)
                );
            }
        };

        // Contents
        container(scrollable(
            column(tags.iter().map(|t| 
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


async fn load_tags() -> LoadTagsResult {
    tagging::get_all_tags()
        .map_err(|err| Arc::new(tagging::LoadError::from(err)) )?
        .into_iter()
        .map(|path| Tag::load_from_path(&path) .map_err(Arc::new) )
        .collect()

}



