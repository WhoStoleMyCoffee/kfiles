use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use iced::event::Status;
use iced::keyboard::key::Named;
use iced::widget::text_editor::{self, Content};
use iced::widget::{self, button, column, container, horizontal_space, row, scrollable, text, Column, Container};
use iced::{Color, Command, Event, Length};

use iced_aw::spinner;
use iced_aw::Bootstrap;

use crate::app::Message as AppMessage;
use crate::tag::{ self, Entries, Tag, TagID };
use crate::widget::tag_entry::{self, TagEntry as TagEntryWidget};
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
    TagEntriesChanged(usize, Entries),
    TagStartEdit(usize),
    TagEditActionPerformed(usize, text_editor::Action),
    CreateTag,
    DeleteTag(usize),
    TagStartRename(usize),
    TagRenameInput(String),
    TagSubmitRename,
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
                        .map(TagEntry::from)
                        .collect()),
                    Err(err) => TagList::Failed( Arc::into_inner(err) ),
                };
            }

            Message::OpenTagsDir => {
                let path: PathBuf = tag::get_save_dir();
                opener::open(path) .unwrap();
            }

            Message::TagEntriesChanged(index, entries) => {
                // Get tag at `index` or return Command::none()
                let Some(tag) = self.get_tag_at_index_mut(index) else {
                    return Command::none(); 
                };

                tag.editing_content = None;
                tag.entries = entries;
                tag.save() .unwrap();
            }

            Message::TagStartEdit(index) => {
                // Get tag at `index` or return Command::none()
                let Some(tag) = self.get_tag_at_index_mut(index) else {
                    return Command::none(); 
                };

                let text: String = tag.entries.to_list();
                tag.editing_content = Some(Content::with_text(&text));
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

            Message::CreateTag => {
                let TagList::Loaded(tag_list) = &mut self.tags else {
                    return Command::none();
                };

                let new_tag_id = TagID::new("new-tag");
                let mut tag = TagEntry::from(Tag::create(new_tag_id));
                tag.save() .unwrap(); // TODO error handling
                tag.is_dirty = true;
                tag_list.push(tag);

                return scrollable::snap_to(
                    LIST_SCROLLABLE_ID(),
                    scrollable::RelativeOffset { x: 0.0, y: 1.0, }
                )
            }

            Message::DeleteTag(index) => {
                let TagList::Loaded(tag_list) = &mut self.tags else {
                    return Command::none();
                };
                if index >= tag_list.len() {
                    return Command::none();
                }

                let tag: TagEntry = tag_list.remove(index);
                let path = tag.get_save_path();
                drop(tag); // ok byee
                if path.exists() {
                    fs::remove_file(path) .unwrap();
                }
            }

            Message::TagStartRename(index) => {
                let Some(tag) = self.get_tag_at_index(index) else {
                    return Command::none();
                };

                self.renaming_tag = Some((
                    index,
                    tag.id.as_ref().clone(),
                ));

                return widget::text_input::focus( tag_entry::RENAME_INPUT_ID() );
            }

            Message::TagRenameInput(input) => {
                let Some((_, str)) = &mut self.renaming_tag else {
                    return Command::none();
                };
                *str = input;
            }

            Message::TagSubmitRename => {
                let Some((i, str)) = &self.renaming_tag else {
                    return Command::none();
                };
                let tag_id = TagID::parse(str);

                let Some(tag) = self.get_tag_at_index_mut(*i) else {
                    return Command::none();
                };

                // TODO error handling
                tag.rename(tag_id) .unwrap();
                tag.save() .unwrap();

                self.renaming_tag = None;
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
        let tags: &Vec<TagEntry> = match &self.tags {
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
                column(tags.iter().enumerate()
                   .map(|(index, t)| 
                        // TODO man i dont like this
                        TagEntryWidget::new(t)
                            .editable(
                                t.editing_content.as_ref(),
                                Box::new(move |a| Message::TagEditActionPerformed(index, a).into()),
                                move |e| Message::TagEntriesChanged(index, e).into(),
                                move || Message::TagStartEdit(index).into(),
                            )
                            .on_delete(Message::DeleteTag(index).into())
                            .renamable(
                                self.renaming_tag.as_ref()
                                .filter(|(i, _)| *i == index)
                                .map(|(_, s)| s),
                                Box::new(|s| Message::TagRenameInput(s).into()),
                                Message::TagStartRename(index).into(),
                                Message::TagSubmitRename.into(),
                            )
                            .into()
                    )
                )
                .width(Length::Fill)
                .padding(12.0)
                .spacing(12.0)
            )
            .id(LIST_SCROLLABLE_ID())
        )

    }

    pub fn handle_event(&mut self, event: Event, status: Status) -> Command<AppMessage> {
        use iced::keyboard::{Event as KeyboardEvent, Key};

        if status != Status::Ignored {
            return Command::none();
        }

        let Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. }) = event else {
            return Command::none();
        };

        // Esc to cancel whatever
        if key == Key::Named(Named::Escape) && modifiers.is_empty() {
            // Cancel renaming tag
            if self.renaming_tag.is_some() {
                self.renaming_tag = None;
            }
            // Cancel all editing entries
            else if let TagList::Loaded(tags) = &mut self.tags {
                for te in tags.iter_mut() {
                    te.editing_content = None;
                }
            }

            return Command::none();
        }

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
    tag::get_all_tags()
        .map_err(|err| Arc::new(tag::LoadError::from(err)) )?
        .into_iter()
        .map(|path| Tag::load_from_path(path) .map_err(Arc::new) )
        .collect()

}




/// Entry for a [`Tag`] used in tandum with the [`crate::widget::tag_entry::TagEntry`] widget
#[derive(Debug)]
struct TagEntry {
    /// The contained tag
    tag: Tag,
    is_dirty: bool,
    /// The content of the text edit, if the user is editing the tag's entries
    editing_content: Option<Content>,
}

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


