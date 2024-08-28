use std::path::{ PathBuf, Path };
use std::time::{Duration, Instant};

use iced::alignment::Vertical;
use iced::widget::container::Appearance;
use iced::{Alignment, Color, Element, Length};
use iced::widget::{column, component, container, mouse_area, text, Component};

use crate::thumbnail::load_thumbnail_for_path;


const BG_COLOR: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

const HOVER_BG_ALPHA: f32 = 0.05;
const SELECTED_BG_ALPHA: f32 = 0.08;

const DOUBLE_CLICK_MILLIS: u64 = 500;



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





pub struct DirEntry<Message: Clone> {
    path: PathBuf,
    is_selected: bool,
    width: Length,
    height: Length,
    on_hover: Option<Message>,
    on_select: Option<Message>,
    on_activate: Option<Message>,
}

impl<Message: Clone> DirEntry<Message> {
    pub fn new<P>(path: P) -> Self
    where P: AsRef<Path>
    {
        DirEntry::<Message> {
            path: path.as_ref().to_path_buf(),
            is_selected: false,
            width: Length::Shrink,
            height: Length::Shrink,
            on_hover: None,
            on_select: None,
            on_activate: None,
        }
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn on_activate(mut self, message: Message) -> Self {
        self.on_activate = Some(message);
        self
    }

    /// Does nothing if culled (See [`cull`])
    pub fn on_select(mut self, message: Message) -> Self {
        self.on_select = Some(message);
        self
    }

    /// Does nothing if culled (See [`cull`])
    pub fn on_hover(mut self, message: Message) -> Self {
        self.on_hover = Some(message);
        self
    }

    pub fn is_selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
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
                state.is_hovered = self.on_hover.is_some();
                if let Some(on_hover) = &self.on_hover {
                    return Some(on_hover.clone());
                }
            }

            Event::Unhovered => {
                state.is_hovered = false;
            }

            Event::Pressed => {
                if let Some(on_activate) = state.last_pressed.replace(Instant::now())
                    .filter(|i| i.elapsed() < Duration::from_millis(DOUBLE_CLICK_MILLIS))
                    .and_then(|_| self.on_activate.as_ref())
                {
                    return Some(on_activate.clone());
                }
                return self.on_select.clone();
            }
        }

        None
    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::Element<'_, Self::Event, iced::Theme, iced::Renderer> {

        let file_name = self.path.file_name()
            .unwrap_or( std::ffi::OsStr::new("") )
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

        let a = (self.is_selected as u8 as f32 * SELECTED_BG_ALPHA)
            + (state.is_hovered as u8 as f32 * HOVER_BG_ALPHA);
        let appearance = Appearance::default().with_background(Color {
            a,
            ..BG_COLOR
        });

        container(
            mouse_area(inner)
                .on_enter(Event::Hovered)
                .on_exit(Event::Unhovered)
                .on_press(Event::Pressed)
        )
        .style(appearance)
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
