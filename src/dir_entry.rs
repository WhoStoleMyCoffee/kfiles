use std::path::{ PathBuf, Path };

use iced::alignment::Vertical;
use iced::{Element, Length};
use iced::widget::{column, component, text, Component};

use crate::thumbnail::load_thumbnail_for_path;


pub fn dir_entry<Message, P>(path: P) -> DirEntry<Message>
where P: AsRef<Path>
{
    DirEntry::new(path)
}


#[derive(Debug, Clone)]
pub enum Event {
}


pub struct DirEntry<Message> {
    path: PathBuf,
    do_cull: bool,
    width: Length,
    height: Length,
    bup: Option<Message>,
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
            bup: None,
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

impl<Message> Component<Message> for DirEntry<Message>
{
    type State = ();
    type Event = Event;

    fn update(
        &mut self,
        state: &mut Self::State,
        event: Self::Event,
    ) -> Option<Message> {
        todo!()
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

        column![
            img.content_fit(iced::ContentFit::Contain),
            text(file_name)
                .size(14)
                .vertical_alignment(Vertical::Center),
        ]
        .width(self.width)
        .height(self.height)
        .clip(true)
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




