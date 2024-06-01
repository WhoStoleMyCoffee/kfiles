use std::path::PathBuf;

use iced::{Color, Element, Length};
use iced::widget::{ button, column, component, container, row, text, Column, Component };
use iced::widget::container::Appearance;
use rfd::FileDialog;


use crate::tag::Tag;

use super::context_menu::ContextMenu;


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




#[derive(Debug, Clone)]
pub enum Event {
    ToggleExpand,
    AddEntryPressed,
    AddEntryFilePressed,
    AddEntryFolderPressed,
}

#[derive(Debug, Default)]
pub struct State {
    is_expanded: bool,
}



pub struct TagEntry<'a, Message: Clone> {
    tag: &'a Tag,
    on_add_entry: Option<Box<dyn Fn(PathBuf) -> Message + 'a>>,
}

impl<'a, Message: Clone> TagEntry<'a, Message> {
    pub fn new(tag: &'a Tag) -> Self {
        TagEntry::<Message> {
            tag,
            on_add_entry: None,
        }
    }

    pub fn on_add_entry<F>(mut self, f: F) -> Self
        where F: Fn(PathBuf) -> Message + 'a
    {
        self.on_add_entry = Some(Box::new(f));
        self
    }

    fn view_contents(&self, _state: &State) -> Column<Event> {
        let add_entry_button = if self.on_add_entry.is_none() { None } else {
            Some(ContextMenu::new(
                button("+") .on_press(Event::AddEntryPressed),
                || column![
                    button("Add folder") .on_press(Event::AddEntryFolderPressed),
                    button("Add file") .on_press(Event::AddEntryFilePressed),
                ].into(),
            )
            .offset([12.0, 12.0])
            .left_click_release_activated())
        };

        column(
            self.tag.get_entries().iter()
                .map(|pb| {
                    text(pb.display())
                        .style(Color::new(0.6, 0.6, 0.7, 1.0))
                        .into()
                })
        )
        .push_maybe(add_entry_button)
        .spacing(8.0)
        .padding([0, 24])
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

            Event::AddEntryPressed => None,

            Event::AddEntryFolderPressed => {
                let on_add_entry = self.on_add_entry.as_ref()?;
                let pick = FileDialog::new()
                    .set_directory("C:/Users/ddxte/")
                    .pick_folder()?;
                Some(on_add_entry(pick))
            }

            Event::AddEntryFilePressed => {
                let on_add_entry = self.on_add_entry.as_ref()?;
                let pick = FileDialog::new()
                    .set_directory("C:/Users/ddxte/")
                    .pick_file()?;
                Some(on_add_entry(pick))
            }
        }
    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::Element<'_, Self::Event, iced::Theme, iced::Renderer> {

        let top_bar = container(row![
            button( if state.is_expanded { "V" } else { ">" } )
                .on_press(Event::ToggleExpand),
            text(&self.tag.id),
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

impl<'a, Message> From<TagEntry<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone
{
    fn from(dir_entry: TagEntry<'a, Message>) -> Self {
        component(dir_entry)
    }
}




