use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use iced::event::Status;
use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Container};
use iced::{self, Event, Length, Rectangle};
use iced::Command;
use iced_aw::Wrap;
use rand::Rng;

use crate::search::Query;
use crate::tag::{Tag, TagID};
use crate::thumbnail::{self, Thumbnail, ThumbnailBuilder};
use crate::widget::{dir_entry::DirEntry, fuzzy_input::FuzzyInput};
use crate::app::{ Message as AppMessage, TAGS_CACHE };
use crate::ToPrettyString;

// TODO make these configurable
const FOCUS_QUERY_KEYS: [&str; 3] = ["s", "/", ";"];
const MAX_RESULT_COUNT: usize = 256;
const MAX_RESULTS_PER_TICK: usize = 10;

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);
const TOTAL_ITEM_SIZE: (f32, f32) = (ITEM_SIZE.0 + ITEM_SPACING.0, ITEM_SIZE.1 + ITEM_SPACING.1);

// Ids
const QUERY_INPUT_ID: fn() -> text_input::Id = || { text_input::Id::new("query_input") };
const MAIN_RESULTS_ID: fn() -> container::Id = || { container::Id::new("main_results") };


#[derive(Debug, Clone)]
pub enum Message {
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




#[derive(Debug)]
pub struct Item(pub isize, pub PathBuf);

impl AsRef<PathBuf> for Item {
    fn as_ref(&self) -> &PathBuf {
        &self.1
    }
}


/// See [`Main::try_receive_results`]
pub enum RecvResultsError {
    /// Results successfully received
    Ok,
    /// Results were already full
    Full,
    /// Sender disconnected
    Disconnected,
    /// Nothing was sent
    Empty,
}

#[derive(Debug)]
pub struct MainScreen {
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

impl MainScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        (
            MainScreen {
                query: Query::empty(),
                query_text: String::default(),
                items: Vec::new(),
                receiver: None,
                thumbnail_builder: (0, ThumbnailBuilder::new(4)),
                scroll: 0.0,
                results_container_bounds: None,
                hovered_path: None,
            },
            MainScreen::fetch_results_bounds() .map(|m| m.into()),
        )
    }

    pub fn tick(&mut self) -> Command<AppMessage> {
        self.build_thumbnails();
        self.try_receive_results();

        Command::none()
    }

    pub fn try_receive_results(&mut self) -> Option<RecvResultsError> {
        use std::sync::mpsc::TryRecvError;

        let rx = self.receiver.as_mut()?;

        // Already full
        if self.items.len() >= MAX_RESULT_COUNT {
            self.receiver = None;
            return Some(RecvResultsError::Full);
        }

        // If there is no query, just append normally
        // I didn't mean for it to rhyme, I'm just low on time
        if self.query.is_empty() {
            // We `try_recv()` first before `try_iter()` to check if the sender has disconnected
            // Because if it has, we also want to drop the receiver
            let item = match rx.try_recv() {
                Ok(item) => item,
                Err(TryRecvError::Empty) => return Some(RecvResultsError::Empty),
                Err(TryRecvError::Disconnected) => {
                    self.receiver = None;
                    return Some(RecvResultsError::Disconnected);
                }
            };

            self.items.push(item);
            self.items.append(&mut rx.try_iter()
                .take(MAX_RESULTS_PER_TICK)
                .collect()
            );

            return Some(RecvResultsError::Ok);
        }

        for _ in 0..MAX_RESULTS_PER_TICK {
            // We do a loop of `try_recv()` insead of `try_iter()` for the same reasons
            // same match statement as above...
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

    pub fn focus_query() -> Command<Message> {
        let id = QUERY_INPUT_ID();
        Command::batch(vec![
            text_input::focus(id.clone()),
            text_input::select_all(id),
        ])
    }

    pub fn fetch_results_bounds() -> Command<Message> {
        container::visible_bounds(MAIN_RESULTS_ID())
            .map(Message::ResultsBoundsFetched)
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::FocusQuery => {
                return MainScreen::focus_query() .map(|m| m.into());
            }

            Message::QueryTextChanged(new_text) => {
                let has_changed = self.query.parse_query(&new_text);
                self.query_text = new_text;
                if has_changed {
                    self.update_search();
                }
            }

            Message::ToggleQueryTag { tag_id, clear_input } => {
                let removed: bool = self.query.remove_tag(&tag_id);
                // If not removed, then add it
                if !removed {
                    let tag: Tag = tag_id.load() .unwrap();
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


            Message::OpenPath(path) => {
                println!("Opening path {}", path.display());

                if let Err(err) = opener::open(&path) {
                    println!("Failed to open {}:\n\t{:?}\n\tRevealing in file explorer instead", &path.display(), err);
                    opener::reveal(&path) .unwrap();
                }

            }

            Message::EntryHovered(path) => {
                self.hovered_path = Some(path);
            }

            Message::ResultsScrolled(viewport) => {
                self.scroll = viewport.absolute_offset().y;
            }

            Message::ResultsBoundsFetched(rect) => {
                self.results_container_bounds = rect;
            }
        }

        Command::none()
    }

    pub fn view(&self) -> Container<AppMessage> {
        use scrollable::{Direction, Properties};

        let query_input = self.view_query_input();
        let results = self.view_results();

        container(
            column![
                button("tags")
                    .on_press(AppMessage::SwitchToTagListScreen),
                query_input,
                text("Results:"),
                container(
                    scrollable(results)
                        .direction(Direction::Vertical(Properties::default()))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .on_scroll(|vp| Message::ResultsScrolled(vp).into())
                )
                .id(MAIN_RESULTS_ID()),
            ]
            // Add hovered path text, if any
            .push_maybe(self.hovered_path.as_ref().map(|pb|
                text(pb.to_pretty_string())
                    .size(12)
            )),
        )
    }

    fn view_query_input(&self) -> Column<AppMessage> {
        column![
            // Tags
            row(self.query.tags.iter().map(|tag| {
                let id = &tag.id;
                button(text(id).size(14))
                    .on_press(Message::ToggleQueryTag {
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
                |tag_id| Message::ToggleQueryTag {
                    tag_id,
                    clear_input: true,
                }.into(),
            )
            .text_input(|text_input| {
                text_input
                    .id(QUERY_INPUT_ID())
                    .on_input(|text| Message::QueryTextChanged(text).into())
            }),
        ]
    }

    fn view_results(&self) -> Wrap<AppMessage, iced_aw::direction::Horizontal> {
        let mut wrap = match self.get_visible_items_range() {
            Some(range) => Wrap::with_elements(
                self.items.iter().enumerate()
                .map(|(i, Item(_score, pb))| {
                    DirEntry::new(pb)
                        .cull(!range.contains(&i))
                        .width(ITEM_SIZE.0)
                        .height(ITEM_SIZE.1)
                        .on_select(Message::OpenPath(pb.clone()).into())
                        .on_hover(Message::EntryHovered(pb.clone()).into())
                        .into()
                })
                .collect(),
            )
            .spacing(ITEM_SPACING.0)
            .line_spacing(ITEM_SPACING.1),

            None => Wrap::new(),
        };

        if self.receiver.is_some() {
            wrap = wrap.push(iced_aw::Spinner::new()
                .width(Length::Fixed(ITEM_SIZE.0))
                .height(Length::Fixed(ITEM_SIZE.1))
            );
        }

        wrap
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

    pub fn handle_event(&mut self, event: Event, status: Status) -> Command<AppMessage> {
        use iced::window::Event as WindowEvent;
        use iced::keyboard::{Event as KeyboardEvent, Key};

        match event {
            // UNHANDLED KEY PRESS
            Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. })
                if status == Status::Ignored =>
            {
                if !modifiers.is_empty() {
                    return Command::none();
                }

                match key.as_ref() {
                    Key::Character(ch) if FOCUS_QUERY_KEYS.contains(&ch) => {
                        MainScreen::focus_query().map(|m| m.into())
                    }

                    _ => Command::none(),
                }
            },

            // WINDOW RESIZED
            Event::Window(_, WindowEvent::Resized { .. }) => {
                MainScreen::fetch_results_bounds().map(|m| m.into())
            }

            _ => Command::none()
        }
    }
}
