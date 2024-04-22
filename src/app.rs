use std::path::PathBuf;
use std::time::Duration;

use iced;
use iced::widget::{
    text_input, column, scrollable, text,
};
use iced::{
    Application,
    Command,
    Theme,
    time,
};

use crate::{ FOCUS_QUERY_KEYS, UPDATE_RATE_MS };



pub struct TagExplorer {
    query: String,
    items: Vec<PathBuf>,
}

impl Application for TagExplorer {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            TagExplorer {
                query: String::new(),
                items: Vec::new(),
            },
            Command::none()
        )
    }

    fn title(&self) -> String {
        "Tag Explorer".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick => {},

            Message::FocusQuery => {
                return text_input::focus( text_input::Id::new("query_input") );
            },

            Message::QueryChanged(new_query) => {
                self.query = new_query;
            },
        }

        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        let query_input = text_input("Query...", &self.query)
            .id(text_input::Id::new("query_input"))
            .on_input( Message::QueryChanged );

        let main = column![
            text("Results:"),
            scrollable(column(
                self.items.iter()
                    .map(|pb|
                        text( pb.display().to_string() )
                            .size(14)
                            .into()
                    )
            ))
                .direction(scrollable::Direction::Both {
                    horizontal: scrollable::Properties::default(),
                    vertical: scrollable::Properties::default(),
                })
        ];

        column![
            query_input,
            main,
        ].into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let tick = time::every(Duration::from_millis(UPDATE_RATE_MS)).map(|_| Message::Tick);

        let events = iced::event::listen_with(|event, status| {
            if status == iced::event::Status::Captured { return None; }
            match event {
                iced::Event::Keyboard(kb_event) => TagExplorer::unhandled_key_input(kb_event),
                _ => None,
            }
        });

        iced::Subscription::batch(vec![
            tick,
            events,
        ])
    }
}

impl TagExplorer {
    fn unhandled_key_input(event: iced::keyboard::Event) -> Option<Message> {
        let iced::keyboard::Event::KeyPressed {
            key,
            modifiers,
            ..
        } = event else {
            return None;
        };

        if !modifiers.is_empty() {
            return None;
        }

        match key.as_ref() {
            iced::keyboard::Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => {
                Some(Message::FocusQuery)
            },
            _ => None,
        }
    }
}


#[derive(Debug, Clone)]
pub enum Message {
    QueryChanged(String),
    Tick,
    FocusQuery,
}
