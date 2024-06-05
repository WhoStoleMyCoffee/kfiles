use std::path::PathBuf;

use iced::widget::text_editor::Content;
use iced::{widget, Color, Element, Length};
use iced::widget::{ button, column, component, container, horizontal_space, row, text, text_editor, text_input, Column, Component, Container
};
use iced::widget::container::Appearance;
use iced_aw::Bootstrap;
use rfd::FileDialog;

use crate::tag::{self, Entries, Tag};
use crate::ToPrettyString;

use super::context_menu::ContextMenu;
use crate::{ icon, icon_button };


pub const RENAME_INPUT_ID: fn() -> widget::text_input::Id = || { widget::text_input::Id::new("tag_rename_input") };


const CONTAINER_APPEARANCE: fn() -> Appearance = || {
    Appearance::default().with_background(Color {
        r: 0.1,
        g: 0.1,
        b: 0.15,
        a: 1.0,
    })
};

const TOP_BAR_APPEARANCE: fn() -> Appearance = || {
    Appearance::default().with_background(Color {
        r: 0.14,
        g: 0.15,
        b: 0.22,
        a: 1.0,
    })
};

const ENTRY_COLOR: Color = Color {
    r: 0.6,
    g: 0.6,
    b: 0.7,
    a: 1.0
};




#[derive(Debug, Clone)]
pub enum Event {
    Empty,
    ToggleExpand,
    AddEntryFilePressed,
    AddEntryFolderPressed,
    ToggleEdit,
    EditActionPerformed(text_editor::Action),
    Delete,
    StartRename,
    RenameInput(String),
    RenameSubmit,
}

pub enum TagOperation {}

#[derive(Debug, Default)]
pub struct State {
    is_expanded: bool,
}




/// TODO documentation
pub struct TagEntry<'a, Message: Clone> {
    tag: &'a Tag,

    editing_content: Option<EditingContent<'a, Message>>,
    on_entries_changed: Option<Box<dyn Fn(Entries) -> Message + 'a>>,
    on_start_edit:      Option<Box<dyn Fn() -> Message + 'a>>,
    on_end_edit:        Option<Box<dyn Fn() -> Message + 'a>>,

    on_delete: Option<Message>,
    renaming_content: Option<RenamingContent<'a, Message>>,
    on_start_rename: Option<Message>,
}

impl<'a, Message: Clone> TagEntry<'a, Message> {
    pub fn new(tag: &'a Tag) -> Self {
        TagEntry {
            tag,
            editing_content: None,
            on_entries_changed: None,
            on_start_edit: None,
            on_end_edit: None,
            on_delete: None,
            renaming_content: None,
            on_start_rename: None,
        }
    }

    pub fn editable(
        mut self,
        content: Option<&'a Content>,
        on_editor_action_performed: Box<dyn Fn(text_editor::Action) -> Message>,
        on_entries_changed: impl Fn(Entries) -> Message + 'a,
        on_start_edit: impl Fn() -> Message + 'a,
    ) -> Self {
        self.editing_content = content.map(|c| EditingContent {
            content: c,
            on_action_performed: on_editor_action_performed,
        });
        self.on_entries_changed = Some(Box::new(on_entries_changed));
        self.on_start_edit = Some(Box::new(on_start_edit));
        self
    }

    pub fn on_end_edit<F>(mut self, f: F) -> Self
        where F: Fn() -> Message + 'a,
    {
        self.on_end_edit = Some(Box::new(f));
        self
    }

    pub fn on_delete(mut self, message: Message) -> Self {
        self.on_delete = Some(message);
        self
    }

    pub fn renamable(
        mut self,
        content: Option<&'a String>,
        on_input: Box<dyn Fn(String) -> Message + 'a>,
        on_start_rename: Message,
        on_submit_rename: Message,
    ) -> Self {
        self.renaming_content = content.map(|c| RenamingContent {
            content: c,
            on_input,
            on_submit: on_submit_rename,
        });
        self.on_start_rename = Some(on_start_rename);
        self
    }

    fn view_contents(&self, _state: &State) -> Column<Event> {
        let contents = match self.editing_content {
            Some(EditingContent { content, .. }) => column![
                text_editor(content) .on_action(Event::EditActionPerformed)
            ],
            None => column(
                self.tag.entries.as_ref().iter()
                    .map(|pb| text(pb.to_pretty_string()) .style(ENTRY_COLOR) .into())
            ),
        };

        let bottom_row = if self.editing_content.is_some() {
            row![
                button(icon!(Bootstrap::FloppyFill)) .on_press(Event::ToggleEdit)
            ]
        } else {
            row![
                icon_button!(icon = Bootstrap::PencilSquare) .on_press(Event::ToggleEdit),
            ]
            // Add entry button only if we listen for entry changes
            .push_maybe(self.on_entries_changed.is_some().then(|| {
                ContextMenu::new(
                    icon_button!(icon = Bootstrap::FolderPlus) .on_press(Event::Empty),
                    || column![
                        button("Add folder") .on_press(Event::AddEntryFolderPressed),
                        button("Add file") .on_press(Event::AddEntryFilePressed),
                    ].into(),
                )
                .offset([12.0, 12.0])
                .left_click_release_activated()
            }))
        };

        contents
            .push(bottom_row)
            .spacing(8.0)
            .padding([0, 24])
    }

    fn view_top_bar(&self, state: &State) -> Container<Event> {
        let label = if let Some(RenamingContent { content, .. }) = &self.renaming_content {
            Element::from(text_input("Tag name", content)
                .id(RENAME_INPUT_ID())
                .on_input(Event::RenameInput)
                .on_submit(Event::RenameSubmit)
            )
        } else {
            Element::from(text(&self.tag.id))
        };

        container(row![
            // Dropdown icon
            icon_button!(icon = if state.is_expanded {
                Bootstrap::CaretDownFill
            } else {
                Bootstrap::CaretRight
            })
            .on_press(Event::ToggleExpand),

            // Label
            label,

            horizontal_space(),

            // Menu
            ContextMenu::new(
                icon_button!(icon = Bootstrap::ThreeDots) .on_press(Event::Empty),
                || column![
                    icon_button!(icon!(Bootstrap::Pen, light), "Rename")
                        .on_press(Event::StartRename)
                        .width(Length::Fill),
                    icon_button!(icon!(Bootstrap::TrashFill, Color::new(0.9, 0.1, 0.0, 1.0)), "Delete")
                        .on_press(Event::Delete)
                        .width(Length::Fill),
                ]
                .max_width(120)
                .into()
            )
            .left_click_release_activated(),
        ] .align_items(iced::Alignment::Center))
        .style( TOP_BAR_APPEARANCE() )
        .width(Length::Fill)
        .padding(8.0)
    }
}


impl<'a, Message: Clone> Component<Message> for TagEntry<'a, Message> {
    type State = State;
    type Event = Event;

    fn update(
        &mut self,
        state: &mut Self::State,
        event: Self::Event,
    ) -> Option<Message> {
        match event {
            Event::ToggleExpand => {
                state.is_expanded = !state.is_expanded;
                None
            }

            Event::Empty => None,

            Event::AddEntryFolderPressed => {
                let on_entries_changed = self.on_entries_changed.as_ref()?;
                let pick: PathBuf = FileDialog::new()
                    .set_directory("C:/Users/ddxte/")
                    .pick_folder()?;
                let mut entries: Entries = self.tag.entries.clone();
                entries.push(pick);
                Some(on_entries_changed(entries))
            }

            Event::AddEntryFilePressed => {
                let on_entries_changed = self.on_entries_changed.as_ref()?;
                let pick = FileDialog::new()
                    .set_directory("C:/Users/ddxte/")
                    .pick_file()?;
                let mut entries = self.tag.entries.clone();
                entries.push(pick);
                Some(on_entries_changed(entries))
            }

            Event::ToggleEdit => {
                match &self.editing_content {
                    Some(EditingContent { content, .. }) => {
                        let on_entries_changed = self.on_entries_changed.as_ref()?;
                        let entries = tag::Entries::from_list(&content.text());
                        Some(on_entries_changed(entries))
                    },
                    None => {
                        let on_start_edit = self.on_start_edit.as_ref()?;
                        Some(on_start_edit())
                    },
                }

            }

            Event::EditActionPerformed(action) => {
                let on_editor_action_performed = self.editing_content.as_ref()
                    .map(|EditingContent { on_action_performed, .. }| on_action_performed)?;
                Some(on_editor_action_performed(action))
            }

            Event::Delete => {
                self.on_delete.clone()
            }

            Event::StartRename => {
                self.on_start_rename.clone()
            }

            Event::RenameInput(input) => {
                self.renaming_content.as_ref()
                    .map(|RenamingContent { on_input, .. }| on_input(input))
            }

            Event::RenameSubmit => {
                self.renaming_content.as_ref()
                    .map(|RenamingContent { on_submit, .. }| on_submit.clone())
            }

        }

    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::Element<'_, Self::Event, iced::Theme, iced::Renderer> {
        container(Column::new()
            // Top bar
            .push( self.view_top_bar(state) )
            // Contents
            .push_maybe(state.is_expanded.then(||
                self.view_contents(state)
            ))
            .width(Length::Fill)
        )
        .style( CONTAINER_APPEARANCE() )
        .into()
    }
}

impl<'a, Message> From<TagEntry<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(dir_entry: TagEntry<'a, Message>) -> Self {
        component(dir_entry)
    }
}


struct EditingContent<'a, Message: Clone> {
    content: &'a Content,
    on_action_performed: Box<dyn Fn(text_editor::Action) -> Message + 'a>,
}

struct RenamingContent<'a, Message: Clone> {
    content: &'a String,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    on_submit: Message,
}



