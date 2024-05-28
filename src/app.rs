use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::OnceLock;
use std::time::Duration;

use iced::event::Status;
use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Container};
use iced::{self, Event, Length, Rectangle};
use iced::{time, Application, Command, Theme};
use iced_aw::Wrap;
use rand::Rng;

use crate::search::Query;
use crate::tag::{Tag, TagID};
use crate::thumbnail::{self, Thumbnail, ThumbnailBuilder};
use crate::widget::{dir_entry::DirEntry, fuzzy_input::FuzzyInput};

// TODO make these configurable
const UPDATE_RATE_MS: u64 = 100;
const FOCUS_QUERY_KEYS: [&str; 3] = ["s", "/", ";"];
const MAX_RESULT_COUNT: usize = 256;
const MAX_RESULTS_PER_TICK: usize = 10;

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);
const TOTAL_ITEM_SIZE: (f32, f32) = (ITEM_SIZE.0 + ITEM_SPACING.0, ITEM_SIZE.1 + ITEM_SPACING.1);

// Ids
const QUERY_INPUT_ID: fn() -> text_input::Id = || { text_input::Id::new("query_input") };
const MAIN_RESULTS_ID: fn() -> container::Id = || { container::Id::new("main_results") };

static TAGS_CACHE: OnceLock<Vec<TagID>> = OnceLock::new();



#[derive(Debug, Clone)]
pub enum Message {
    MainMessage(MainMessage),
    Tick,
    Event(Event, Status),
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
        // TODO find better cache system some day
        TAGS_CACHE.set(Tag::get_all_tag_ids().unwrap()).unwrap();

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
            Message::Tick => self.main.tick(),

            Message::MainMessage(main_message) => {
                self.main.update(main_message).map(Message::MainMessage)
            }

            Message::Event(event, status) => self.handle_event(event, status),
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        container(self.main.view()).into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::CatppuccinMocha // cat 🐈
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        iced::Subscription::batch(vec![
            time::every(Duration::from_millis(UPDATE_RATE_MS)).map(|_| Message::Tick),
            iced::event::listen_with(|event, status| Some(Message::Event(event, status))),
        ])
    }
}

impl TagExplorer {
    fn handle_event(&mut self, event: Event, status: Status) -> Command<Message> {
        use iced::window::Event as WindowEvent;

        match event {
            // Ignored keyboard events
            Event::Keyboard(event) if status == Status::Ignored => self.unhandled_key_input(event),

            Event::Window(_, WindowEvent::Resized { .. }) => {
                self.main.fetch_results_bounds().map(|m| m.into())
            }

            _ => Command::none(),
        }
    }

    fn unhandled_key_input(&mut self, event: iced::keyboard::Event) -> Command<Message> {
        use iced::keyboard::{Event, Key};

        let Event::KeyPressed { key, modifiers, .. } = event else {
            return Command::none();
        };

        // "No modifiers, please"
        if !modifiers.is_empty() {
            return Command::none();
        }

        match key.as_ref() {
            Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => {
                self.main.focus_query().map(Message::MainMessage)
            }

            _ => Command::none(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum MainMessage {
    QueryTextChanged(String),
    ToggleQueryTag {
        tag_id: TagID,
        clear_input: bool,
    },
    FocusQuery,
    ResultsScrolled(Viewport),
    ResultsBoundsFetched(Option<Rectangle>),
    OpenPath(PathBuf),
    EntryHovered(PathBuf),
}

pub struct Item(pub isize, pub PathBuf);

impl AsRef<PathBuf> for Item {
    fn as_ref(&self) -> &PathBuf {
        &self.1
    }
}

/// See [`Main::try_receive_results`]
enum RecvResultsError {
    /// Results successfully received
    Ok,
    /// Results were already full
    Full,
    /// Sender disconnected
    Disconnected,
    /// Nothing was sent
    Empty,
}

struct Main {
    query: Query,
    query_text: String,
    items: Vec<Item>,
    receiver: Option<Receiver<Item>>,
    /// Tuple with the item index it's trying to build and the builder itself
    thumbnail_builder: (usize, ThumbnailBuilder),
    scroll: f32,
    results_container_bounds: Option<Rectangle>,
    hovered_path: Option<PathBuf>,
}

impl Main {
    fn tick(&mut self) -> Command<Message> {
        self.build_thumbnails();
        self.try_receive_results();

        Command::none()
    }

    fn try_receive_results(&mut self) -> Option<RecvResultsError> {
        use std::sync::mpsc::TryRecvError;

        let Some(rx) = &mut self.receiver else {
            return None;
        };

        // Already full
        if self.items.len() >= MAX_RESULT_COUNT {
            self.receiver = None;
            return Some(RecvResultsError::Full);
        }

        // If there is no query, just append normally
        // I didn't mean for it to rhyme, I'm just low on time
        if self.query.is_empty() {
            self.items.append(&mut rx.try_iter()
                .take(MAX_RESULTS_PER_TICK)
                .collect()
            );

            return Some(RecvResultsError::Ok);
        }

        for _ in 0..MAX_RESULTS_PER_TICK {
            let item = match rx.try_recv() {
                Ok(item) => item,
                Err(TryRecvError::Empty) => return Some(RecvResultsError::Empty),
                Err(TryRecvError::Disconnected) => {
                    self.receiver = None;
                    return Some(RecvResultsError::Disconnected);
                }
            };

            // Insert
            let index = self.items.partition_point(|&Item(score, _)| score > item.0);
            self.items.insert(index, item);
        }

        Some(RecvResultsError::Ok)
    }

    fn focus_query(&self) -> Command<MainMessage> {
        let id = QUERY_INPUT_ID();
        Command::batch(vec![
            text_input::focus(id.clone()),
            text_input::select_all(id),
        ])
    }

    fn fetch_results_bounds(&self) -> Command<MainMessage> {
        container::visible_bounds(MAIN_RESULTS_ID())
            .map(MainMessage::ResultsBoundsFetched)
    }

    fn update(&mut self, message: MainMessage) -> Command<MainMessage> {
        match message {
            MainMessage::FocusQuery => {
                return self.focus_query();
            }

            MainMessage::QueryTextChanged(new_text) => {
                let has_changed = self.query.parse_query(&new_text);
                self.query_text = new_text;
                if has_changed {
                    self.update_search();
                }
            }

            MainMessage::ToggleQueryTag { tag_id, clear_input } => {
                let removed: bool = self.query.remove_tag(&tag_id);
                // If not removed, then add it
                if !removed {
                    let tag: Tag = tag_id.load().unwrap();
                    if self.query.add_tag(tag) {
                        self.query.constraints.clear();
                        self.update_search();
                    }
                }

                if clear_input {
                    self.query_text.clear();
                }
                self.update_search();
            }


            MainMessage::OpenPath(path) => {
                println!("Opening path {}", path.display());
                opener::open(&path).unwrap();
            }

            MainMessage::EntryHovered(path) => {
                self.hovered_path = Some(path);
            }

            MainMessage::ResultsScrolled(viewport) => {
                self.scroll = viewport.absolute_offset().y;
            }

            MainMessage::ResultsBoundsFetched(rect) => {
                self.results_container_bounds = rect;
            }
        }

        Command::none()
    }

    fn view(&self) -> Container<Message> {
        use scrollable::{Direction, Properties};

        let query_input = self.view_query_input();
        let results = self.view_results();

        container(
            column![
                query_input,
                text("Results:"),
                container(
                    scrollable(results)
                        .direction(Direction::Vertical(Properties::default()))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .on_scroll(|vp| MainMessage::ResultsScrolled(vp).into())
                )
                .id(MAIN_RESULTS_ID()),
            ]
            // Add hovered path text, if any
            .push_maybe(self.hovered_path.as_ref().map(|pb|
                text(pb.display().to_string().replace('\\', "/"))
                    .size(12)
            )),
        )
    }

    fn view_query_input(&self) -> Column<Message> {
        column![
            // Tags
            row(self.query.tags.iter().map(|tag| {
                let id = &tag.id;
                button(text(id).size(14))
                    .on_press(MainMessage::ToggleQueryTag {
                        tag_id: id.clone(),
                        clear_input: false,
                    }.into())
                    .into()
            })),

            // Fuzzy text input
            FuzzyInput::new(
                "Query...",
                &self.query_text,
                TAGS_CACHE.get().expect("Tags cache not initialized"),
                |tag_id| MainMessage::ToggleQueryTag {
                    tag_id,
                    clear_input: true,
                }.into(),
            )
            .text_input(|text_input| {
                text_input
                    .id(QUERY_INPUT_ID())
                    .on_input(|text| MainMessage::QueryTextChanged(text).into())
            }),
        ]
    }

    fn view_results(&self) -> Wrap<Message, iced_aw::direction::Horizontal> {
        match self.get_visible_items_range() {
            Some(range) => Wrap::with_elements(
                self.items
                    .iter()
                    .enumerate()
                    .map(|(i, Item(_score, pb))| {
                        DirEntry::new(pb)
                            .cull(!range.contains(&i))
                            .width(ITEM_SIZE.0)
                            .height(ITEM_SIZE.1)
                            .on_select(MainMessage::OpenPath(pb.clone()).into())
                            .on_hover(MainMessage::EntryHovered(pb.clone()).into())
                            .into()
                    })
                    .collect(),
            )
            .spacing(ITEM_SPACING.0)
            .line_spacing(ITEM_SPACING.1),

            None => Wrap::new(),
        }
    }

    fn build_thumbnails(&mut self) {
        let Some(range) = self.get_visible_items_range() else {
            return;
        };
        let (index, builder) = &mut self.thumbnail_builder;

        let Some(Item(_score, path)) = self.items.get(*index) else {
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
    fn get_visible_items_range(&self) -> Option<std::ops::Range<usize>> {
        let Rectangle { width, height, .. } = self.results_container_bounds?;

        let items_per_row: usize = (width / TOTAL_ITEM_SIZE.0) as usize;
        //          (        Which row do we start at?       ) * items per row
        let start = (self.scroll / TOTAL_ITEM_SIZE.1) as usize * items_per_row;
        let end = start
        //  + (           How many rows does the view span?                    ) * items per row
            + ((height / TOTAL_ITEM_SIZE.1) as usize + 2) * items_per_row;

        Some(start..end)
    }

    pub fn update_search(&mut self) {
        self.items.clear(); // TODO filter instead?
        self.receiver = Some(self.query.search());
        self.scroll = 0.0;
        self.hovered_path = None;
    }
}

impl Default for Main {
    fn default() -> Self {
        Main {
            query: Query::empty(),
            query_text: String::default(),
            items: Vec::new(),
            receiver: None,
            thumbnail_builder: (0, ThumbnailBuilder::new(4)),
            scroll: 0.0,
            results_container_bounds: None,
            hovered_path: None,
        }
    }
}
