use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use iced::widget::scrollable::Viewport;
use iced::widget::{
    button, column, container, image, row, scrollable, text, text_input, Column, Container
};
use iced::{self, alignment, Length, Rectangle};
use iced::{time, Application, Command, Theme};
use iced_aw::Wrap;

use crate::search::Query;
use crate::tag::{Tag, TagID};
use crate::thumbnail::{Thumbnail, ThumbnailBuilder};

const UPDATE_RATE_MS: u64 = 100;
const FOCUS_QUERY_KEYS: [&str; 3] = ["s", "/", ";"];
const MAX_RESULT_COUNT: usize = 256;
const MAX_RESULTS_PER_TICK: usize = 10;

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);
const TOTAL_ITEM_SIZE: (f32, f32) = (ITEM_SIZE.0 + ITEM_SPACING.0, ITEM_SIZE.1 + ITEM_SPACING.1);


pub struct TagExplorer {
    query: Query,
    items: Vec<PathBuf>,
    receiver: Option<Receiver<PathBuf>>,
    thumbnail_builder: ThumbnailBuilder,
    // TODO refactor
    scroll: f32,
    results_container_size: (f32, f32),
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
            thumbnail_builder: ThumbnailBuilder::default(),
            scroll: 0.0,
            results_container_size: (1.0, 1.0),
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
                    self.receiver = None;
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

            Message::AddQueryTag(tag_id) => match tag_id.load() {
                Ok(tag) => {
                    if self.query.add_tag(tag) {
                        self.update_query();
                    }
                }

                Err(err) => {
                    todo!()
                }
            },

            Message::RemoveQueryTag(tag_id) => {
                if self.query.remove_tag(&tag_id) {
                    self.update_query();
                }
            }

            Message::MainResultsScrolled(viewport) => {
                self.scroll = viewport.absolute_offset().y;

                self.get_visible_items_range();
            }

            Message::WindowResized(_width, _height) => {
                return container::visible_bounds(container::Id::new("main_results"))
                    .map(|rect| rect.map_or(Message::None, Message::MainResultsResized) );
            }

            Message::MainResultsResized(rect) => {
                // println!("Main resized: {rect:?}");
                self.results_container_size = (rect.width, rect.height);
            }

            Message::None => {}
        }

        Command::none()
    }

    /// TODO REFACTOR view()
    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        // Tags list
        let all_tags = Tag::get_all_tag_ids().unwrap(); // TODO cache these
        let tags_list = scrollable(column(all_tags.into_iter().map(|id| {
            let id_str = id.as_ref();
            button(text(format!("#{id_str}")))
                .on_press(Message::AddQueryTag(id))
                .into()
        })))
        .width(100);

        row![tags_list, self.view_main()].into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::CatppuccinMocha // cat 🐈
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let tick = time::every(Duration::from_millis(UPDATE_RATE_MS)).map(|_| Message::Tick);

        use iced::{ event, Event };
        let events = event::listen_with(|event, status| {
            match event {
                Event::Keyboard(kb_event) => {
                    if status == event::Status::Captured {
                        return None;
                    }
                    TagExplorer::unhandled_key_input(kb_event)
                },
                Event::Window(id, iced::window::Event::Resized { width, height }) => {
                    Some(Message::WindowResized(width as f32, height as f32))
                },
                _ => None,
            }
        });

        iced::Subscription::batch(vec![tick, events])
    }
}

impl TagExplorer {
    fn unhandled_key_input(event: iced::keyboard::Event) -> Option<Message> {
        use iced::keyboard::{Event, Key};

        let Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };

        // "No modifiers, please"
        if !modifiers.is_empty() {
            return None;
        }

        match key.as_ref() {
            Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => Some(Message::FocusQuery),
            _ => None,
        }
    }

    pub fn update_query(&mut self) {
        self.items.clear();
        self.receiver = self.query.search();
    }

    fn view_main(&self) -> Container<Message> {
        let query_input = column![
            row(self.query.tags.iter().map(|tag| {
                let id = &tag.id;
                button(text(id.as_ref().as_str()).size(14))
                    .on_press(Message::RemoveQueryTag(id.clone()))
                    .into()
            })),
            text_input("Query...", &self.query.query)
                .id(text_input::Id::new("query_input"))
                .on_input(Message::QueryTextChanged)
        ];


        use scrollable::{Direction, Properties};
        let visible_range = self.get_visible_items_range();
        let results = Wrap::with_elements(
                self.items.iter().enumerate()
                .map(|(i, pb)|
                     self.display_dir( &pb, visible_range.contains(&i) ).into()
                )
                .collect()
            )
            .spacing(ITEM_SPACING.0)
            .line_spacing(ITEM_SPACING.1);

        container(column![
            query_input,
            text("Results:"),
            container(scrollable(results)
                .direction(Direction::Vertical(Properties::default()))
                .width(Length::Fill)
                .height(Length::Fill)
                .on_scroll(Message::MainResultsScrolled)
            )
            .id(container::Id::new("main_results")),
        ])
    }

    fn build_thumbnails(&mut self) {}

    fn get_visible_items_range(&self) -> std::ops::Range<usize> {
        let items_per_row: usize = (self.results_container_size.0 / TOTAL_ITEM_SIZE.0) as usize;
        //          (        Which row do we start at?       ) * items per row
        let start = (self.scroll / TOTAL_ITEM_SIZE.1) as usize * items_per_row;
        //        start + (           How many rows does the view span?                    ) * items per row
        let end = start + ((self.results_container_size.1 / TOTAL_ITEM_SIZE.1) as usize + 1) * items_per_row;

        start..end
    }

    fn display_dir(&self, path: &Path, is_visible: bool) -> Column<'static, Message> {
        if !is_visible {
            return column![]
                .width(ITEM_SIZE.0)
                .height(ITEM_SIZE.1);
        }

        let file_name = path.file_name()
            .unwrap()
            .to_string_lossy();

        let img = if path.get_cache_path().exists() {
            image(path.get_cache_path())
        } else if path.is_dir() {
            image("assets/folder.png")
        } else {
            image("assets/file.png")
        };

        use iced::ContentFit;
        column![
            img.content_fit(ContentFit::Contain),
            text(file_name)
                .size(14)
                .vertical_alignment(alignment::Vertical::Center),
        ]
        .width(ITEM_SIZE.0)
        .height(ITEM_SIZE.1)
        .clip(true)
        .align_items(iced::Alignment::Center)
    }

}

#[derive(Debug, Clone)]
pub enum Message {
    None,
    QueryTextChanged(String),
    AddQueryTag(TagID),
    RemoveQueryTag(TagID),
    Tick,
    FocusQuery,
    MainResultsScrolled(Viewport),
    MainResultsResized(Rectangle),
    WindowResized(f32, f32),
}

