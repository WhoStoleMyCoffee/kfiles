use std::path::PathBuf;

use iced::widget::text_editor::Content;
use iced::{Color, Element, Length};
use iced::widget::{ button, column, component, container, horizontal_space, row, text, text_editor, Column, Component
};
use iced::widget::container::Appearance;
use iced_aw::Bootstrap;
use rfd::FileDialog;

use crate::tag::{self, Entries, Tag};
use crate::ToPrettyString;

use super::context_menu::ContextMenu;
use crate::{ icon, icon_button };


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
}

pub enum TagOperation {}

#[derive(Debug, Default)]
pub struct State {
    is_expanded: bool,
}




pub struct TagEntry<'a, Message: Clone, ActionPerformedFn> {
    tag: &'a Tag,
    editing_content: Option<(
        &'a Content,
        ActionPerformedFn,
    )>,
    on_entries_changed: Option<Box<dyn Fn(Entries) -> Message + 'a>>,
    on_start_edit:      Option<Box<dyn Fn() -> Message + 'a>>,
    on_end_edit:        Option<Box<dyn Fn() -> Message + 'a>>,
    on_delete: Option<Message>,
}

impl<'a, Message, ActionPerformedFn> TagEntry<'a, Message, ActionPerformedFn>
where
    Message: Clone,
    ActionPerformedFn: Fn(text_editor::Action) -> Message + 'a,
{
    pub fn new(tag: &'a Tag) -> Self {
        TagEntry {
            tag,
            editing_content: None,
            on_entries_changed: None,
            on_start_edit: None,
            on_end_edit: None,
            on_delete: None,
        }
    }

    pub fn editable(
        mut self,
        content: Option<&'a Content>,
        on_editor_action_performed: ActionPerformedFn,
        on_entries_changed: impl Fn(Entries) -> Message + 'a,
        on_start_edit: impl Fn() -> Message + 'a,
    ) -> Self {
        self.editing_content = content.map(|c| (c, on_editor_action_performed));
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

    fn view_contents(&self, _state: &State) -> Column<Event> {
        let contents = match self.editing_content {
            Some((content, _)) => column![
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
}


impl<'a, Message, ActionPerformedFn> Component<Message> for TagEntry<'a, Message, ActionPerformedFn>
where
    Message: Clone,
    ActionPerformedFn: Fn(text_editor::Action) -> Message + 'a,
{
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
                    Some((content, _)) => {
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
                    .map(|(_, on_action)| on_action)?;
                Some(on_editor_action_performed(action))
            }

            Event::Delete => {
                self.on_delete.clone()
            }

        }

    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::Element<'_, Self::Event, iced::Theme, iced::Renderer> {

        let top_bar = container(row![
            // Dropdown icon
            icon_button!(icon = if state.is_expanded {
                Bootstrap::CaretDownFill
            } else {
                Bootstrap::CaretRight
            })
            .on_press(Event::ToggleExpand),

            // Tag id
            text(&self.tag.id),

            horizontal_space(),

            ContextMenu::new(
                icon_button!(icon = Bootstrap::ThreeDots) .on_press(Event::Empty),
                || column![
                    icon_button!(icon!(Bootstrap::TrashFill, Color::new(0.9, 0.1, 0.0, 1.0)), "Delete")
                        .on_press(Event::Delete),
                ].into()
            )
            .left_click_release_activated(),
        ])
        .style( TOP_BAR_APPEARANCE() )
        .width(Length::Fill)
        .padding(8.0);


        let contents = state.is_expanded.then(||
            self.view_contents(state)
        );

        container(Column::new()
            .push(top_bar)
            .push_maybe(contents)
            .width(Length::Fill)
        )
        .style( CONTAINER_APPEARANCE() )
        .into()
    }
}

impl<'a, Message, AF> From<TagEntry<'a, Message, AF>> for Element<'a, Message>
where
    Message: 'a + Clone,
    AF: Fn(text_editor::Action) -> Message + 'a,
{
    fn from(dir_entry: TagEntry<'a, Message, AF>) -> Self {
        component(dir_entry)
    }
}




