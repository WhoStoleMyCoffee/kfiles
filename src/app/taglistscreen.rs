use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use iced::event::Status;
use iced::widget::text_editor::{self, Content};
use iced::widget::{button, column, container, row, scrollable, text, Column, Container};
use iced::{Color, Command, Event, Length};
use iced_aw::spinner;

use crate::app::Message as AppMessage;
use crate::tag::{ self, Entries, Tag, TagID };
use crate::widget::tag_entry::TagEntry as TagEntryWidget;

const ERROR_COLOR: Color = Color {
    r: 0.9,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};


type LoadTagsResult = Result< Vec<Tag>, Arc<tag::LoadError> >;


#[derive(Debug, Clone)]
pub enum Message {
    TagsLoaded(LoadTagsResult),
    OpenTagsDir,
    TagEntryChanged(usize, Entries),
    TagStartEdit(usize),
    TagEditActionPerformed(usize, text_editor::Action),
}


#[derive(Debug)]
enum TagList {
    Loading,
    Loaded(Vec<TagEntry>),
    Failed(Option<tag::LoadError>),
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
                        .map(|t| TagEntry::from(t))
                        .collect()),
                    Err(err) => TagList::Failed( Arc::into_inner(err) ),
                };
            }

            Message::OpenTagsDir => {
                let path: PathBuf = Tag::get_save_dir();
                opener::open(path) .unwrap();
            }

            Message::TagEntryChanged(index, entries) => {
                // Get tag at `index` or return Command::none()
                let Some(tag) = self.get_tag_at_index_mut(index) else {
                    return Command::none(); 
                };

                tag.editing_content = None;
                tag.entries = entries;
            }

            Message::TagStartEdit(index) => {
                // Get tag at `index` or return Command::none()
                let Some(tag) = self.get_tag_at_index_mut(index) else {
                    return Command::none(); 
                };

                let text: String = tag.entries.to_list();
                tag.editing_content = Some(Content::with_text(&text));

                println!("Index {index} started editing");
            }

            Message::TagEditActionPerformed(index, action) => {
                let Some(tag) = self.get_tag_at_index_mut(index) else {
                    return Command::none(); 
                };

                let Some(editing_content) = &mut tag.editing_content else {
                    return Command::none(); 
                };
                editing_content.perform(action);
            }
        }
        
        Command::none()
    }

    pub fn view(&self) -> Column<AppMessage> {
        use iced::widget::Space;

        let list = self.view_list();

        column![
            row![
                button("<") .on_press(AppMessage::SwitchToMainScreen),
                text("Tags List") .size(24),
                Space::with_width(Length::Fill),
                button("Open save directory") .on_press(Message::OpenTagsDir.into())
            ],
            list
        ]
    }

    fn view_list(&self) -> Container<AppMessage> {
        // Get tags or display whatever
        let tags: &Vec<TagEntry> = match &self.tags {
            TagList::Loaded(tags) => tags,

            TagList::Loading => {
                return container(spinner::Spinner::new()
                    .width(Length::Fixed(48.0))
                    .height(Length::Fixed(48.0))
                );
            }

            TagList::Failed(err_maybe) => {
                let error_message: String = match err_maybe {
                    Some(err) => format!("Failed to load tags:\n{err}"),
                    None => "TODO error message".to_string(),
                };

                return container(text(error_message)
                    .style(ERROR_COLOR)
                );
            }
        };

        // Contents
        container(scrollable(
            column(tags.iter().enumerate()
                .map(|(i, t)| {
                    TagEntryWidget::new(t)
                        .editable(
                            t.editing_content.as_ref(),
                            move |e| Message::TagEntryChanged(i, e).into(),
                            move || Message::TagStartEdit(i).into(),
                            Box::new(move |a| Message::TagEditActionPerformed(i, a).into()),
                        )
                        .into()
                })
            )
            .width(Length::Fill)
            .padding(12.0)
            .spacing(12.0)
        ))

    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }

    fn get_tag_at_index(&self, index: usize) -> Option<&TagEntry> {
        match &self.tags {
            TagList::Loaded(tags) => tags.get(index),
            _ => None,
        }
    }
    
    fn get_tag_at_index_mut(&mut self, index: usize) -> Option<&mut TagEntry> {
        match &mut self.tags {
            TagList::Loaded(tags) => tags.get_mut(index),
            _ => None,
        }
    }

}


async fn load_tags() -> LoadTagsResult {
    Tag::get_all_tags()
        .map_err(|err| Arc::new(tag::LoadError::from(err)) )?
        .into_iter()
        .map(|path| Tag::load_from_path(path) .map_err(Arc::new) )
        .collect()

}




#[derive(Debug)]
/// TODO documentation
struct TagEntry {
    tag: Tag,
    is_dirty: bool,
    editing_content: Option<Content>,
}

/* impl TagEntry {
    #[inline]
    fn is_dirty(&self) -> bool {
        self.is_dirty
    }
} */

impl From<Tag> for TagEntry {
    fn from(tag: Tag) -> Self {
        TagEntry {
            tag,
            is_dirty: false,
            editing_content: None,
        }
    }
}

impl Deref for TagEntry {
    type Target = Tag;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tag
    }
}

impl DerefMut for TagEntry {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.is_dirty = true;
        &mut self.tag
    }
}

impl Drop for TagEntry {
    fn drop(&mut self) {
        if !self.is_dirty {
            return;
        }

        if let Err(err) = self.tag.save() {
            println!("ERROR Failed to save tag: {:?}", err);
        }
    }
}


