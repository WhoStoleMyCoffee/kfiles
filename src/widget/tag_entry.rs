use iced::{Alignment, Color, Element, Length};
use iced::widget::{ button, column, component, container, horizontal_space, row, scrollable, text, Column, Component, Container, Row, Scrollable
};
use iced::widget::container::Appearance;
use iced_aw::Bootstrap;

use crate::app::theme;
use crate::tag::id::TagID;
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
    SubtagPressed(usize),
}

#[derive(Debug, Default)]
pub struct State {
    is_expanded: bool,
}




/// Displays surface level info about a [`Tag`]:
/// - Its name
/// - Entries under it along with an icon for any errors with them
/// - Optionally, an edit button
pub struct TagEntry<'a, Message: Clone> {
    tag: &'a Tag,
    on_edit_pressed: Option<Message>,
    on_subtag_pressed: Option< Box<dyn Fn(TagID) -> Message + 'a > >,
    /// Cache containing the indices of entries that don't exist
    erroneous_entries: Vec<usize>,
}

impl<'a, Message: Clone> TagEntry<'a, Message> {
    pub fn new(tag: &'a Tag) -> Self {
        TagEntry {
            tag,
            on_edit_pressed: None,
            on_subtag_pressed: None,
            erroneous_entries: tag.entries.as_ref().iter().enumerate()
                .filter(|(_, pb)| !pb.exists())
                .map(|(i, _)| i)
                .collect(),
        }
    }

    pub fn on_edit_pressed(mut self, message: Message) -> Self {
        self.on_edit_pressed = Some(message);
        self
    }

    pub fn on_subtag_pressed<F>(mut self, f: F) -> Self
    where F: 'a + Fn(TagID) -> Message,
    {
        self.on_subtag_pressed = Some(Box::new(f));
        self
    }

    fn view_contents(&self, _state: &State) -> Column<Event> {
        column(
            self.tag.entries.as_ref().iter().enumerate()
                .map(|(i, pb)| Row::new()
                    .push( text(pb.to_pretty_string()) .style(ENTRY_COLOR) )
                    .push_maybe(self.is_entry_index_erroneous(&i).then(||
                        icon!(Bootstrap::ExclamationCircleFill, theme::ERROR_COLOR)
                    ))
                    .spacing(12)
                    .align_items(Alignment::Center)
                    .into()
                )
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
            ]
            // Error if any
            .push_maybe((!self.erroneous_entries.is_empty()).then(||
                icon!(Bootstrap::ExclamationCircleFill, theme::WARNING_COLOR)
            ))

            // Subtags, if enabled
            .push_maybe(self.on_subtag_pressed.is_some()
                .then(|| self.view_subtags_list(state))
            )

            .push(horizontal_space())
            // Edit button
            .push_maybe(self.on_edit_pressed.is_some().then(||
                simple_button!(icon = Bootstrap::PencilSquare)
                    .on_press(Event::EditPressed)
            ))
            .align_items(Alignment::Center)
            .spacing(12)
        )
        .style( TOP_BAR_APPEARANCE() )
        .width(Length::Fill)
        .padding(8.0)
    }

    fn view_subtags_list(&self, _state: &State) -> Scrollable<Event> {
        use iced::widget::scrollable::{ Direction, Properties };

        scrollable(
            row(
                self.tag.get_subtags()
                .iter()
                .enumerate()
                .map(|(i, tag_id)|
                     button(text(tag_id.to_string()))
                     .on_press(Event::SubtagPressed(i))
                     .into()
                )
            )
            .spacing(4)
        )
        .direction(Direction::Horizontal(
            Properties::default()
                .width(2)
                .scroller_width(2)
        ))
    }

    fn is_entry_index_erroneous(&self, index: &usize) -> bool {
        self.erroneous_entries.binary_search(index).is_ok()
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

            Event::SubtagPressed(index) => {
                let callback = self.on_subtag_pressed.as_ref()?;
                let tag_id = self.tag.get_subtags().get(index)?
                    .clone();
                Some(callback(tag_id))
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


