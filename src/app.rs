use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use std::{io, thread};

use iced;
use iced::widget::{button, column, row, scrollable, text, text_input};
use iced::{time, Application, Command, Theme};

use crate::search::Query;
use crate::tag::{self, Tag, TagID};

const UPDATE_RATE_MS: u64 = 100;
const FOCUS_QUERY_KEYS: [&str; 3] = ["s", "/", ";"];
const MAX_RESULT_COUNT: usize = 256;
const MAX_RESULTS_PER_TICK: usize = 10;



pub struct TagExplorer {
    query: Query,
    items: Vec<PathBuf>,
    receiver: Option<Receiver<PathBuf>>,
}

impl Application for TagExplorer {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let te: TagExplorer = TagExplorer {
            query: Query::empty(),
            items: Vec::new(),
            receiver: None,
        };

        (te, Command::none())
    }

    fn title(&self) -> String {
        "Tag Explorer".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick => {
                let Some(rx) = &mut self.receiver else {
                    return Command::none();
                };

                if self.items.len() >= MAX_RESULT_COUNT {
                    return Command::none();
                }

                self.items.append(&mut rx.try_iter()
                    .take(MAX_RESULTS_PER_TICK)
                    .collect()
                );
            }

            Message::FocusQuery => {
                return text_input::focus(text_input::Id::new("query_input"));
            }

            Message::QueryTextChanged(new_query) => {
                self.query.query = new_query;
                self.update_query();
            }

            Message::AddQueryTag(tag_id) => {
                if self.query.add_tag(tag_id) {
                    self.update_query();
                }
            }

            Message::RemoveQueryTag(tag_id) => {
                if self.query.remove_tag(&tag_id) {
                    self.update_query();
                }
            }
        }

        Command::none()
    }

    /// TODO REFACTOR view()
    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        let query_input = column![
            row(self.query.tags.iter()
                .map(|id| button(id.as_ref().as_str())
                     .on_press(Message::RemoveQueryTag(id.clone()))
                     .into()
                 )
           ),
            text_input("Query...", &self.query.query)
                .id(text_input::Id::new("query_input"))
                .on_input(Message::QueryTextChanged)
        ];

        let main = column![
            text("Results:"),
            scrollable(column(
                self.items.iter()
                    .map(|pb| text(pb.display().to_string())
                         .size(14)
                         .into()
                    )
            ))
            .direction(scrollable::Direction::Both {
                horizontal: scrollable::Properties::default(),
                vertical: scrollable::Properties::default(),
            })
        ];

        // Tags list
        let all_tags = Tag::get_all_tag_ids().unwrap(); // TODO cache these
        let tags_list = scrollable(column(
            all_tags.into_iter()
                .map(|id| {
                    let id_str = id.as_ref();
                    button(text( format!("#{id_str}") ))
                        .on_press( Message::AddQueryTag(id) )
                        .into()
                }),
        ))
        .width(100);

        row![
            tags_list,
            column![query_input, main]
        ].into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let tick = time::every(Duration::from_millis(UPDATE_RATE_MS)).map(|_| Message::Tick);

        let events = iced::event::listen_with(|event, status| {
            if status == iced::event::Status::Captured {
                return None;
            }
            match event {
                iced::Event::Keyboard(kb_event) => TagExplorer::unhandled_key_input(kb_event),
                _ => None,
            }
        });

        iced::Subscription::batch(vec![tick, events])
    }
}

impl TagExplorer {
    fn unhandled_key_input(event: iced::keyboard::Event) -> Option<Message> {
        let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };

        if !modifiers.is_empty() {
            return None;
        }

        match key.as_ref() {
            iced::keyboard::Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => {
                Some(Message::FocusQuery)
            }
            _ => None,
        }
    }

    /// TODO
    pub fn update_query(&mut self) {
        self.items.clear();

        let Some(query_tag) = self.query.tags.first() else {
            self.receiver = None;
            return;
        };
        let tag = match Tag::load(&query_tag) {
            Ok(tag) => tag,
            Err(tag::LoadError::IO(err)) if err.kind() == io::ErrorKind::NotFound => {
                return;
            }
            Err(err) => {
                panic!("failed to load tag: {}", err);
            }
        };

        let (tx, rx) = mpsc::channel::<PathBuf>();
        self.receiver = Some(rx);

        thread::spawn(move || {
            let it = tag.get_dirs();
            for pb in it {
                if tx.send(pb).is_err() {
                    return;
                }
            }
        });
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    QueryTextChanged(String),
    AddQueryTag(TagID),
    RemoveQueryTag(TagID),
    Tick,
    FocusQuery,
}
