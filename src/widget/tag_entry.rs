use iced::{Alignment, Color, Element, Length};
use iced::widget::{ column, component, container, horizontal_space, row, text, Column, Component, Container
};
use iced::widget::container::Appearance;
use iced_aw::Bootstrap;

use crate::tag::Tag;
use crate::ToPrettyString;
use crate::{ icon, simple_button };


const CONTAINER_APPEARANCE: fn() -> Appearance = || {
    Appearance::default()
        .with_background(Color::new(0.1, 0.1, 0.15, 1.0))
};

const TOP_BAR_APPEARANCE: fn() -> Appearance = || {
    Appearance::default()
        .with_background(Color::new(0.14, 0.15, 0.22, 1.0))
};

pub const ENTRY_COLOR: Color = Color {
    r: 0.6,
    g: 0.6,
    b: 0.7,
    a: 1.0
};




#[derive(Debug, Clone)]
pub enum Event {
    /// Empty event that does nothing
    /// Useful for [`ContextMenu`] buttons that are drop-down-able but don't do
    /// anything on their own
    Empty,
    ToggleExpand,
    EditPressed,
}

pub enum TagOperation {}

#[derive(Debug, Default)]
pub struct State {
    is_expanded: bool,
}




/// TODO documentation
pub struct TagEntry<'a, Message: Clone> {
    tag: &'a Tag,
    on_edit_pressed: Option<Message>,
}

impl<'a, Message: Clone> TagEntry<'a, Message> {
    pub fn new(tag: &'a Tag) -> Self {
        TagEntry {
            tag,
            on_edit_pressed: None,
        }
    }

    pub fn on_edit_pressed(mut self, message: Message) -> Self {
        self.on_edit_pressed = Some(message);
        self
    }

    fn view_contents(&self, _state: &State) -> Column<Event> {
        column(
            self.tag.entries.as_ref().iter()
                .map(|pb| text(pb.to_pretty_string()) .style(ENTRY_COLOR) .into())
        )
        .spacing(8.0)
        .padding([0, 24])
    }

    fn view_top_bar(&self, state: &State) -> Container<Event> {
        container(row![
            // Dropdown icon
            simple_button!(icon = if state.is_expanded {
                Bootstrap::CaretDownFill
            } else {
                Bootstrap::CaretRight
            })
            .on_press(Event::ToggleExpand),

            // Label
            text(&self.tag.id),

            horizontal_space(),
        ]
        .push_maybe(self.on_edit_pressed.is_some().then(||
            simple_button!(icon = Bootstrap::PencilSquare)
                .on_press(Event::EditPressed)
        ))
        .align_items(Alignment::Center))
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
            Event::Empty => None,

            Event::ToggleExpand => {
                state.is_expanded = !state.is_expanded;
                None
            }

            Event::EditPressed => {
                self.on_edit_pressed.clone()
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


