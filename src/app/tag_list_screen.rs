use std::path::PathBuf;
use std::sync::Arc;

use iced::event::Status;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, Column, Container};
use iced::{Color, Command, Event, Length};

use iced_aw::spinner;
use iced_aw::Bootstrap;

use crate::app::Message as AppMessage;
use crate::tag::{ self, Tag, TagID };
use crate::widget::tag_entry::TagEntry as TagEntryWidget;
use crate::{ icon_button, icon };

const ERROR_COLOR: Color = Color {
    r: 0.9,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};

// Ids
const LIST_SCROLLABLE_ID: fn() -> scrollable::Id = || { scrollable::Id::new("tag_list") };


type LoadTagsResult = Result< Vec<Tag>, Arc<tag::LoadError> >;


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
    Failed(Option<tag::LoadError>),
}



/// TODO use hashset or hashmap instead of vec
#[derive(Debug)]
pub struct TagListScreen {
    tags: TagList,
    renaming_tag: Option<(usize, String)>,
}

impl TagListScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        (
            TagListScreen {
                tags: TagList::Loading,
                renaming_tag: None,
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
                let path: PathBuf = tag::get_save_dir();
                opener::open(path) .unwrap();
            }

            Message::CreateTag => {
                let TagList::Loaded(tag_list) = &mut self.tags else {
                    return Command::none();
                };

                let new_tag_id = TagID::new("new-tag");
                let tag = Tag::create(new_tag_id);
                tag.save() .unwrap(); // TODO error handling
                tag_list.push(tag);

                return scrollable::snap_to(
                    LIST_SCROLLABLE_ID(),
                    scrollable::RelativeOffset { x: 0.0, y: 1.0, }
                )
            }

        }
        
        Command::none()
    }

    pub fn view(&self) -> Column<AppMessage> {
        let list = self.view_list();

        column![
            row![
                // Back arrow
                icon_button!(icon = Bootstrap::ArrowLeft)
                    .on_press(AppMessage::SwitchToMainScreen),
                text("Tags List") .size(24),
                horizontal_space(),
                button("Open save directory") .on_press(Message::OpenTagsDir.into())
            ],

            button("New tag") .on_press(Message::CreateTag.into()),

            list
        ]
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

                return container(text(error_message)
                    .style(ERROR_COLOR)
                );
            }
        };

        // Contents
        container(
            scrollable(
                column(tags.iter().map(|t| 
                    // aaa i dont like the cloning
                    TagEntryWidget::new(t)
                        .on_edit_pressed(AppMessage::SwitchToTagEditScreen( t.clone() ))
                        .into()
                ))
                .width(Length::Fill)
                .padding(12.0)
                .spacing(12.0)
            )
            .id(LIST_SCROLLABLE_ID())
        )

    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }

    fn get_tag_at_index(&self, index: usize) -> Option<&Tag> {
        match &self.tags {
            TagList::Loaded(tags) => tags.get(index),
            _ => None,
        }
    }
    
    fn get_tag_at_index_mut(&mut self, index: usize) -> Option<&mut Tag> {
        match &mut self.tags {
            TagList::Loaded(tags) => tags.get_mut(index),
            _ => None,
        }
    }

}


async fn load_tags() -> LoadTagsResult {
    tag::get_all_tags()
        .map_err(|err| Arc::new(tag::LoadError::from(err)) )?
        .into_iter()
        .map(|path| Tag::load_from_path(path) .map_err(Arc::new) )
        .collect()

}


