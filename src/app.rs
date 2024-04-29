use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use iced::{self, Length};
use iced::widget::{button, column, container, image, row, scrollable, text, text_input, Column, Container, Row};
use iced::{time, Application, Command, Theme};
use iced_aw::Wrap;

use crate::search::Query;
use crate::tag::{Tag, TagID};

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
                match tag_id.load() {
                    Ok(tag) => {
                        if self.query.add_tag(tag) {
                            self.update_query();
                        }
                    }

                    Err(err) => {
                        todo!()
                    }
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
            self.view_main(),
        ].into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::CatppuccinMocha // cat 🐈
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
        use iced::keyboard::{ Event, Key };

        let Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };

        // "No modifiers, please"
        if !modifiers.is_empty() { return None; }

        match key.as_ref() {
            Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => {
                Some(Message::FocusQuery)
            }
            _ => None,
        }
    }

    pub fn update_query(&mut self) {
        self.items.clear();
        self.receiver = self.query.search();
    }

    fn view_main(&self) -> Column<'static, Message> {
        let query_input = column![
            row(self.query.tags.iter()
                .map(|tag| {
                    let id = &tag.id;
                    button( text(id.as_ref().as_str()) .size(14) )
                        .on_press(Message::RemoveQueryTag(id.clone()))
                        .into()
                })
           ),
            text_input("Query...", &self.query.query)
                .id(text_input::Id::new("query_input"))
                .on_input(Message::QueryTextChanged)
        ];

        /*
        let mut column = Column::new();
        for _ in 0..10 {
            let mut row = Row::new();
            for _ in 0..10 {
                row = row.push( display_dir(Path::new("C:/Users/ddxte/")) )
            }
            column = column.push(row);
        }

        use scrollable::{ Properties, Direction };
        let results = column![
            text("Results:"),
            scrollable(column)
            .direction(Direction::Vertical(Properties::default()))
            .width(Length::Fill)
            .height(Length::Fill)
        ];
        */

        use scrollable::{ Properties, Direction };
        let results = column![
            text("Results:"),
            scrollable(Wrap::with_elements(
                self.items.iter().map(|pb|
                    display_dir(&pb).into()
                )
                .collect()
            ))
            .direction(Direction::Vertical(Properties::default()))
            .width(Length::Fill)
            .height(Length::Fill)
        ];

        column![query_input, results]
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



fn display_dir(path: &Path) -> Container<'static, Message> {
    container(
        image("assets/wimdy.jpg")
            .width(48)
    )
}


fn file_image(width: u16) -> image::Image<image::Handle> {
    image("assets/wimdy.jpg")
        .width(width)
}
