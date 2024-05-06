use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use iced::widget::scrollable::Viewport;
use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Container,
};
use iced::{self, Length, Rectangle};
use iced::{time, Application, Command, Theme};
use iced_aw::Wrap;
use rand::Rng;

use crate::dir_entry::dir_entry;
use crate::search::Query;
use crate::tag::{Tag, TagID};
use crate::thumbnail::{self, Thumbnail, ThumbnailBuilder};

const UPDATE_RATE_MS: u64 = 100;
const FOCUS_QUERY_KEYS: [&str; 3] = ["s", "/", ";"];
const MAX_RESULT_COUNT: usize = 256;
const MAX_RESULTS_PER_TICK: usize = 10;

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);
const TOTAL_ITEM_SIZE: (f32, f32) = (ITEM_SIZE.0 + ITEM_SPACING.0, ITEM_SIZE.1 + ITEM_SPACING.1);

#[derive(Debug, Clone)]
pub enum Message {
    None,
    MainMessage(MainMessage),
    Tick,
    WindowResized(f32, f32),
}

impl From<MainMessage> for Message {
    fn from(value: MainMessage) -> Self {
        Self::MainMessage(value)
    }
}

pub struct TagExplorer {
    main: Main,
}

impl Application for TagExplorer {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            TagExplorer {
                main: Main::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Tag Explorer".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick => {
                return self.main.tick();
            }

            Message::MainMessage(main_message) => {
                return self.main.update(main_message);
            }

            Message::WindowResized(_width, _height) => {
                return container::visible_bounds(container::Id::new("main_results"))
                    .map(|rect|
                        rect.map_or(Message::None, |rect| MainMessage::MainResultsResized(rect).into())
                    );
            }

            Message::None => {}
        }

        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        // Tags list
        let all_tags = Tag::get_all_tag_ids().unwrap(); // TODO cache these
        let tags_list = scrollable(column(all_tags.into_iter().map(|id| {
            let id_str = id.as_ref();
            button(text(format!("#{id_str}")))
                .on_press( MainMessage::AddQueryTag(id).into() )
                .into()
        })))
        .width(100);

        row![
            tags_list,
            self.main.view(),
        ].into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::CatppuccinMocha // cat 🐈
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let tick = time::every(Duration::from_millis(UPDATE_RATE_MS)).map(|_| Message::Tick);

        use iced::{event, Event};
        let events = event::listen_with(|event, status| match event {
            Event::Keyboard(kb_event) => {
                if status == event::Status::Captured {
                    return None;
                }
                TagExplorer::unhandled_key_input(kb_event)
            }
            Event::Window(_id, iced::window::Event::Resized { width, height }) => {
                Some(Message::WindowResized(width as f32, height as f32))
            }
            _ => None,
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
            Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => Some(MainMessage::FocusQuery.into()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum MainMessage {
    QueryTextChanged(String),
    AddQueryTag(TagID),
    RemoveQueryTag(TagID),
    FocusQuery,
    MainResultsScrolled(Viewport),
    MainResultsResized(Rectangle),
}

struct Main {
    query: Query,
    items: Vec<PathBuf>,
    receiver: Option<Receiver<PathBuf>>,
    thumbnail_builder: (usize, ThumbnailBuilder),
    scroll: f32,
    results_container_size: (f32, f32),
}

impl Main {
    fn tick(&mut self) -> Command<Message> {
        self.build_thumbnails();

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

        Command::none()
    }

    fn update(&mut self, message: MainMessage) -> Command<Message> {
        match message {
            MainMessage::FocusQuery => {
                return text_input::focus(text_input::Id::new("query_input"));
            }

            MainMessage::QueryTextChanged(new_text) => {
                self.query.query = new_text;
                self.update_query();
            }

            MainMessage::AddQueryTag(tag_id) => match tag_id.load() {
                Ok(tag) => {
                    if self.query.add_tag(tag) {
                        self.update_query();
                    }
                }

                Err(err) => {
                    todo!()
                }
            }

            MainMessage::RemoveQueryTag(tag_id) => {
                if self.query.remove_tag(&tag_id) {
                    self.update_query();
                }
            }

            MainMessage::MainResultsScrolled(viewport) => {
                self.scroll = viewport.absolute_offset().y;
            }

            MainMessage::MainResultsResized(rect) => {
                self.results_container_size = (rect.width, rect.height);
            }
        }

        Command::none()
    }

    fn view(&self) -> Container<Message> {
        let query_input = column![
            row(self.query.tags.iter().map(|tag| {
                let id = &tag.id;
                button(text(id.as_ref().as_str()).size(14))
                    .on_press(MainMessage::RemoveQueryTag(id.clone()).into())
                    .into()
            })),
            text_input("Query...", &self.query.query)
                .id(text_input::Id::new("query_input"))
                .on_input(|text| MainMessage::QueryTextChanged(text).into())
        ];

        use scrollable::{Direction, Properties};
        let visible_range = self.get_visible_items_range();
        let results = Wrap::with_elements(
            self.items
                .iter()
                .enumerate()
                .map(|(i, pb)| {
                    dir_entry(&pb)
                        .cull(!visible_range.contains(&i))
                        .width(ITEM_SIZE.0)
                        .height(ITEM_SIZE.1)
                        .into()
                })
                .collect(),
        )
        .spacing(ITEM_SPACING.0)
        .line_spacing(ITEM_SPACING.1);

        container(column![
            query_input,
            text("Results:"),
            container(
                scrollable(results)
                    .direction(Direction::Vertical(Properties::default()))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .on_scroll(|vp| MainMessage::MainResultsScrolled(vp).into())
            )
            .id(container::Id::new("main_results")),
        ])
    }

    fn build_thumbnails(&mut self) {
        let range = self.get_visible_items_range();
        let (index, builder) = &mut self.thumbnail_builder;

        let Some(path) = self.items.get(*index) else {
            *index = 0;
            return;
        };

        // Move on to the next one
        *index += 1;
        if !range.contains(index) {
            *index = range.start;
        }

        if path.is_dir() || !thumbnail::is_file_supported(path) {
            return;
        }

        // If thumbnail already exists, don't try to rebuild it 90% of the time
        // TODO make the probability configurable
        if path.get_thumbnail_cache_path().exists() && rand::thread_rng().gen_bool(0.9) {
            return;
        }

        // Build
        builder.build_for_path(path);
    }

    /// Get the range of items which are visible in the main view
    fn get_visible_items_range(&self) -> std::ops::Range<usize> {
        let items_per_row: usize = (self.results_container_size.0 / TOTAL_ITEM_SIZE.0) as usize;
        //          (        Which row do we start at?       ) * items per row
        let start = (self.scroll / TOTAL_ITEM_SIZE.1) as usize * items_per_row;
        let end = start
        //  + (           How many rows does the view span?                    ) * items per row
            + ((self.results_container_size.1 / TOTAL_ITEM_SIZE.1) as usize + 2) * items_per_row;

        start..end
    }

    pub fn update_query(&mut self) {
        self.items.clear();
        self.receiver = self.query.search();
    }
}

impl Default for Main {
    fn default() -> Self {
        Main {
            query: Query::empty(),
            items: Vec::new(),
            receiver: None,
            thumbnail_builder: (0, ThumbnailBuilder::new(4)),
            scroll: 0.0,
            results_container_size: (1.0, 1.0),
        }
    }
}
