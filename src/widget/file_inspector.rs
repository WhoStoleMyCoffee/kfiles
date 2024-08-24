use std::borrow::Cow;
use std::marker::PhantomData;
use std::path::PathBuf;

use iced::{widget, Element, Length};
use iced::widget::{column, component, container, horizontal_rule, scrollable, text, Component, Container};
use iced_aw::{grid, grid_row};
use iced_aw::widgets::Grid;

use crate::tagging::id::TagID;
use crate::tagging::tag::Tag;
use crate::ToPrettyString;



#[derive(Debug, Clone)]
pub enum Event {}

#[derive(Debug, Default)]
pub struct State {}



pub struct FileInspector<Message: Clone> {
    path: PathBuf,
    width: Length,
    height: Length,
    tags: Vec<TagID>,
    s: PhantomData<Message>,
}


impl<Message: Clone> FileInspector<Message> {
    pub fn new(path: PathBuf, all_tags: &[Tag]) -> Self {
        let tags: Vec<TagID> = all_tags.iter()
            .filter(|t| t.contains(&path))
            .map(|t| t.id.clone())
            .collect();

        FileInspector {
            path,
            width: Length::Shrink,
            height: Length::Fill,
            tags,
            s: PhantomData,
        }
    }

    fn view_info(&self) -> Container<Event> {
        container(
            grid![
                grid_row![ "Location: ", text(self.path.to_pretty_string()) ],
            ]
        )
    }
}


impl<Message: Clone> Component<Message> for FileInspector<Message> {
    type State = State;
    type Event = Event;

    fn update(
        &mut self,
        state: &mut Self::State,
        event: Self::Event,
    ) -> Option<Message> {
        None
    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::advanced::graphics::core::Element<'_, Self::Event, iced::Theme, iced::Renderer> {
        let scroll_properties = widget::scrollable::Properties::default();

        let file_name = self.path.file_name()
            .map_or_else(
                || Cow::from(if self.path.is_dir() { "Unnamed Folder" } else { "Unnamed File" }),
                |osstr| osstr.to_string_lossy()
            );

        container(
            scrollable(
                column![
                    text( file_name ),
                    horizontal_rule(2),

                    self.view_info()
                ]
            )
            .direction(scrollable::Direction::Both { vertical: scroll_properties, horizontal: scroll_properties })
        )
        .width(self.width)
        .height(self.height)
        .into()
    }
}



impl<'a, Message> From<FileInspector<Message>> for Element<'a, Message>
where 
    Message: 'a + Clone,
{
    fn from(value: FileInspector<Message>) -> Self {
        component(value)
    }
}


