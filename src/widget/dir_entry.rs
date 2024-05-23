use std::path::{ PathBuf, Path };
use std::time::{Duration, Instant};

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



#[derive(Debug, Clone)]
pub enum Event {
    Hovered,
    Unhovered,
    Pressed,
}

#[derive(Debug, Default)]
pub struct State {
    is_hovered: bool,
    last_pressed: Option<Instant>,
}

impl State {
    fn get_appearance(&self) -> Appearance {
        match self.is_hovered {
            true => Appearance::default().with_background(HOVERED_COLOR),
            false => Appearance::default(),
        }
    }
}


pub struct DirEntry<Message: Clone> {
    path: PathBuf,
    do_cull: bool,
    width: Length,
    height: Length,
    on_select: Option<Message>,
    on_hover: Option<Message>,
}

impl<Message: Clone> DirEntry<Message> {
    pub fn new<P>(path: P) -> Self
    where P: AsRef<Path>
    {
        DirEntry::<Message> {
            path: path.as_ref().to_path_buf(),
            do_cull: false,
            width: Length::Shrink,
            height: Length::Shrink,
            on_select: None,
            on_hover: None,
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

    /// Does nothing if culled (See [`cull`])
    pub fn on_select(mut self, message: Message) -> Self {
        if !self.do_cull {
            self.on_select = Some(message);
        }
        self
    }

    /// Does nothing if culled (See [`cull`])
    pub fn on_hover(mut self, message: Message) -> Self {
        if !self.do_cull {
            self.on_hover = Some(message);
        }
        self
    }
}


impl<Message: Clone> Component<Message> for DirEntry<Message> {
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
                if let Some(on_hover) = &self.on_hover {
                    return Some(on_hover.clone());
                }
            }

            Event::Unhovered => {
                state.is_hovered = false;
            }

            Event::Pressed => {
                const DOUBLE_CLICK_MILLIS: u64 = 500;

                let Some(on_select) = &self.on_select else { return None; };
                if let Some(instant) = state.last_pressed.replace(Instant::now()) {
                    if instant.elapsed() < Duration::from_millis(DOUBLE_CLICK_MILLIS) {
                        return Some(on_select.clone());
                    }
                }

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
                .on_press(Event::Pressed)
        )
        .style(state.get_appearance())
        .into()
    }
}

impl<'a, Message> From<DirEntry<Message>> for Element<'a, Message>
where
    Message: 'a + Clone
{
    fn from(dir_entry: DirEntry<Message>) -> Self {
        component(dir_entry)
    }
}




