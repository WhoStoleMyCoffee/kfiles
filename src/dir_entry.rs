use std::path::{ PathBuf, Path };

use iced::alignment::Vertical;
use iced::widget::container::Appearance;
use iced::{Alignment, Color, Element, Length};
use iced::widget::{column, component, container, mouse_area, text, Component};

use crate::thumbnail::load_thumbnail_for_path;


const HOVERED_COLOR: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.05,
};



pub fn dir_entry<Message, P>(path: P) -> DirEntry<Message>
where P: AsRef<Path>
{
    DirEntry::new(path)
}


#[derive(Debug, Clone)]
pub enum Event {
    Hovered,
    Unhovered,
}

#[derive(Debug, Default)]
pub struct State {
    is_hovered: bool,
}

impl State {
    fn get_appearance(&self) -> Appearance {
        match self.is_hovered {
            true => Appearance::default().with_background(HOVERED_COLOR),
            false => Appearance::default(),
        }
    }
}


pub struct DirEntry<Message> {
    path: PathBuf,
    do_cull: bool,
    width: Length,
    height: Length,
    on_press: Option<Message>
}

impl<Message> DirEntry<Message> {
    pub fn new<P>(path: P) -> Self
    where P: AsRef<Path>
    {
        DirEntry::<Message> {
            path: path.as_ref().to_path_buf(),
            do_cull: false,
            width: Length::Shrink,
            height: Length::Shrink,
            on_press: None,
        }
    }

    pub fn cull(mut self, do_cull: bool) -> Self {
        self.do_cull = do_cull;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}


impl<Message> Component<Message> for DirEntry<Message> {
    type State = State;
    type Event = Event;

    fn update(
        &mut self,
        state: &mut Self::State,
        event: Self::Event,
    ) -> Option<Message> {
        match event {
            Event::Hovered => {
                state.is_hovered = true;
            }

            Event::Unhovered => {
                state.is_hovered = false;
            }
        }

        None
    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::advanced::graphics::core::Element<'_, Self::Event, iced::Theme, iced::Renderer> {
        if self.do_cull {
            return column![]
                .width(self.width)
                .height(self.height)
                .into();
        }

        let file_name = self.path.file_name()
            .unwrap()
            .to_string_lossy();
        let img = load_thumbnail_for_path(&self.path);
        let inner = column![
                img.content_fit(iced::ContentFit::Contain),
                text(file_name)
                    .size(14)
                    .vertical_alignment(Vertical::Center),
            ]
            .width(self.width)
            .height(self.height)
            .align_items(Alignment::Center)
            .clip(true);

        container(
            mouse_area(inner)
                .on_enter(Event::Hovered)
                .on_exit(Event::Unhovered)
        )
        .style(state.get_appearance())
        .into()
    }
}

impl<'a, Message> From<DirEntry<Message>>
    for Element<'a, Message>
where
    Message: 'a
{
    fn from(dir_entry: DirEntry<Message>) -> Self {
        component(dir_entry)
    }
}




