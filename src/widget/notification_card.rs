use iced::widget::{column, component, row, Component, Text};
use iced::{Alignment, Color, Element, Length};
use iced_aw::{card, Bootstrap};

use crate::app::notification::Notification;
use crate::{app, icon, simple_button};

#[derive(Debug, Clone)]
pub enum Event {
    Close,
    ToggleExpand,
}

#[derive(Debug)]
pub struct State {
    is_expanded: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            is_expanded: true,
        }
    }
}



pub struct NotificationCard<'a, Message: Clone> {
    title: String,
    content: String,
    icon: Option<Text<'a>>,
    width: Length,
    height: Length,
    on_close: Option<Message>,
}

impl<'a, Message: Clone> NotificationCard<'a, Message> {
    pub fn new(title: &str, content: &str) -> Self {
        NotificationCard {
            title: title.to_string(),
            content: content.to_string(),
            icon: None,
            width: Length::Fill,
            height: Length::Shrink,
            on_close: None,
        }
    }


    pub fn from_notification(value: &'a Notification) -> Self {
        let mut nc = NotificationCard::new(&value.get_title(), &value.content);
        nc.icon = value.get_icon() .clone();
        nc
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    // pub fn icon(Bootstrap, Color)

    pub fn icon_text(mut self, icon_text: Text<'a>) -> Self {
        self.icon = Some(icon_text);
        self
    }

    pub fn on_close(mut self, message: Message) -> Self {
        self.on_close = Some(message);
        self
    }
}

impl<'a, Message: Clone> Component<Message> for NotificationCard<'a, Message> {
    type State = State;
    type Event = Event;

    fn update(&mut self, state: &mut Self::State, event: Self::Event) -> Option<Message> {
        match event {
            Event::Close => {
                if let Some(message) = &self.on_close {
                    Some(message.clone())
                } else {
                    None
                }
            }

            Event::ToggleExpand => {
                state.is_expanded = !state.is_expanded;
                None
            }
        }
    }

    fn view(
        &self,
        state: &Self::State,
    ) -> iced::Element<'_, Self::Event, iced::Theme, iced::Renderer> {
        let top_row = row![
            // Dropdown icon
            simple_button!(
                icon = if state.is_expanded {
                    Bootstrap::CaretDownFill
                } else {
                    Bootstrap::CaretRight
                }
            )
            .on_press(Event::ToggleExpand),
        ]
        .push_maybe(self.icon.clone())
        .push(self.title.as_ref())
        .align_items(Alignment::Center);

        let mut card = if state.is_expanded {
            card(top_row, self.content.as_ref())
        } else {
            card(top_row, column![])
        };

        if self.on_close.is_some() {
            card = card.on_close(Event::Close)
        }

        card
            // .padding_head(Padding::from(2.0))
            .style(iced_aw::CardStyles::Dark)
            .into()
    }
}

impl<'a, Message> From<NotificationCard<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(value: NotificationCard<'a, Message>) -> Self {
        component(value)
    }
}

