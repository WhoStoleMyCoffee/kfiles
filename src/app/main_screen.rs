use std::iter;
use std::path::PathBuf;

use iced::event::Status;
use iced::keyboard::Key;
use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, text_input, Column};
use iced::{self, keyboard, Element, Event, Length, Rectangle};
use iced::Command;
use iced_aw::Wrap;
use rand::Rng;

use crate::search::Query;
use crate::tagging::{self, tag::Tag, id::TagID};
use crate::thumbnail::{self, Thumbnail, ThumbnailBuilder};
use crate::widget::{dir_entry::DirEntry, fuzzy_input::FuzzyInput};
use crate::app::{theme, Message as AppMessage};
use crate::{configs, send_message, ToPrettyString};

use super::notification::error_message;
use super::KFiles;


// TODO make these configurable
const FOCUS_QUERY_KEYS: [keyboard::Key<&str>; 3] = [
    Key::Character("s"),
    Key::Character("/"),
    Key::Named(keyboard::key::Named::Tab),
];

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);
const TOTAL_ITEM_SIZE: (f32, f32) = (ITEM_SIZE.0 + ITEM_SPACING.0, ITEM_SIZE.1 + ITEM_SPACING.1);

// Ids
const QUERY_INPUT_ID: fn() -> text_input::Id = || { text_input::Id::new("query_input") };
const MAIN_RESULTS_ID: fn() -> container::Id = || { container::Id::new("main_results") };


#[derive(Debug, Clone)]
pub enum Message {
    QueryTextChanged(String),
    QuerySubmit,
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

impl From<Message> for AppMessage {
    fn from(value: Message) -> AppMessage {
        AppMessage::Screen(super::ScreenMessage::Main(value))
    }
}





#[derive(Debug)]
pub struct Item(pub isize, pub PathBuf);

impl AsRef<PathBuf> for Item {
    fn as_ref(&self) -> &PathBuf {
        &self.1
    }
}


/// See [`MainScreen::try_receive_results`]
pub enum RecvItemsResult {
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
    /// Tuple with the item index it's trying to build and the builder itself
    thumbnail_builder: (usize, ThumbnailBuilder),
    thumbnail_update_prob: f64,
    scroll: f32,
    results_container_bounds: Option<Rectangle>,
    hovered_path: Option<PathBuf>,
    tags_cache: Vec<TagID>,
}

impl MainScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        let mut command = MainScreen::fetch_results_bounds()
            .map(|m| m.into());

        let tags_cache = match tagging::get_all_tag_ids() {
            Ok(v) => v,
            Err(err) => {
                command = Command::batch(vec![
                    command,
                    send_message!(error_message(
                        format!("Failed to load tags:\n{}", err),
                    )),
                ]);
                Vec::new()
            }
        };

        let cfg = configs::global();

        (
            MainScreen {
                query: Query::empty(),
                query_text: String::default(),
                items: Vec::new(),
                thumbnail_builder: (
                    0,
                    ThumbnailBuilder::new(cfg.thumbnail_thread_count)
                ),
                thumbnail_update_prob: cfg.thumbnail_update_prob as f64,
                scroll: 0.0,
                results_container_bounds: None,
                hovered_path: None,
                tags_cache,
            },
            command,
        )
    }

    pub fn tick(&mut self) -> Command<AppMessage> {
        self.build_thumbnails();
        self.try_receive_results();

        Command::none()
    }

    /// Returns `None` if there is no search occurring
    pub fn try_receive_results(&mut self) -> Option<RecvItemsResult> {
        use std::sync::mpsc::TryRecvError;

        let has_search = !self.query.is_empty();
        let rx = self.query.receiver.as_mut()?;

        let (max_result_count, max_this_tick) = {
            let cfg = configs::global();
            (cfg.max_result_count, cfg.max_results_per_tick)
        };

        // Already full
        if self.items.len() >= max_result_count {
            self.query.receiver = None;
            return Some(RecvItemsResult::Full);
        }

        // We `try_recv()` first before `try_iter()` to check if the sender has disconnected
        // Because if it has, we also want to drop the receiver
        let first = match rx.try_recv() {
            Ok(item) => item,
            Err(TryRecvError::Empty) => return Some(RecvItemsResult::Empty),
            Err(TryRecvError::Disconnected) => {
                self.query.receiver = None;
                return Some(RecvItemsResult::Disconnected);
            }
        };

        // If there is no query, just append normally
        // I didn't mean for it to rhyme, I'm just low on time
        if !has_search {
            self.items.push(first);
            self.items.append(&mut rx.try_iter()
                .take(max_this_tick)
                .collect()
            );

            return Some(RecvItemsResult::Ok);
        }

        // Add new items in sorted order
        let it = iter::once(first)
            .chain( rx.try_iter().take(max_this_tick) );
        for item in it {
            let index = self.items.partition_point(|&Item(score, _)| score > item.0);
            self.items.insert(index, item);
        }

        Some(RecvItemsResult::Ok)
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

    pub fn open_first_result(&self) -> Option<Command<AppMessage>> {
        let Item(_, path) = self.items.first()?;
        Some(KFiles::open_path(path))
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
                    self.reset_search();
                }
            }

            Message::QuerySubmit => {
                return self.open_first_result() .unwrap_or(Command::none());
            }

            Message::ToggleQueryTag { tag_id, clear_input } => {
                let removed: bool = self.query.remove_tag(&tag_id);
                // If not removed, then add it
                if !removed {
                    let tag: Tag = match tag_id.load() {
                        Ok(t) => t,
                        Err(err) => return send_message!(error_message(
                            format!("Failed to load tag {}:\n{}", tag_id, err)
                        )),
                    };

                    if self.query.add_tag(tag) {
                        self.query.constraints.clear();
                        self.reset_search();
                    }
                }

                if clear_input {
                    self.query_text.clear();
                }
                self.reset_search();
            }


            Message::OpenPath(path) => {
                return KFiles::open_path(&path);
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

    pub fn view(&self) -> Element<AppMessage> {
        use scrollable::{Direction, Properties};

        let query_input = self.view_query_input();
        let results = self.view_results();

        container(
            column![
                row![
                    horizontal_space(),
                    button("tags") .on_press(AppMessage::SwitchToTagListScreen),
                    button("settings") .on_press(AppMessage::SwitchToConfigScreen),
                ],

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
        .into()
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
                &self.tags_cache,
                |tag_id| Message::ToggleQueryTag {
                    tag_id,
                    clear_input: true,
                }.into(),
            )
            .text_input(|text_input| {
                text_input
                    .id(QUERY_INPUT_ID())
                    .on_input(|text| Message::QueryTextChanged(text).into())
                    .on_submit(Message::QuerySubmit.into())
            })
            .hide_on_empty()
            .style(theme::Simple),
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

        if self.query.receiver.is_some() {
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

        // If thumbnail already exists, don't try to rebuild it some percentage of the time
        // (configurable)
        if path.get_thumbnail_cache_path().exists()
            && !rand::thread_rng().gen_bool(self.thumbnail_update_prob)
        {
            return;
        }

        // Build
        // println!("building {}", path.display());
        if let Err(err) = builder.build_for_path(path) {
            println!("Failed to build thumbnail for {}: {}", path.display(), err);
        }
    }

    /// Get the range of items which are visible in the main view
    /// come to think of it, there was probably a better way to do all this culling thing
    fn get_visible_items_range(&self) -> Option<std::ops::Range<usize>> {
        let Rectangle { width, height, .. } = self.results_container_bounds?;

        let items_per_row: usize = (width / TOTAL_ITEM_SIZE.0) as usize;
        //          (        Which row do we start at?       ) * items per row
        let start = (self.scroll / TOTAL_ITEM_SIZE.1) as usize * items_per_row;
        let end = start
        //  + (    How many rows does the view span?    ) * items per row
            + ((height / TOTAL_ITEM_SIZE.1) as usize + 2) * items_per_row;

        Some(start..end)
    }

    pub fn reset_search(&mut self) {
        self.items.clear();
        self.query.search();
        self.scroll = 0.0;
        self.hovered_path = None;
    }

    pub fn handle_event(&mut self, event: Event, status: Status) -> Command<AppMessage> {
        use iced::window::Event as WindowEvent;
        use iced::keyboard::{Event as KeyboardEvent, Key, key::Named};

        match event {
            // KEY PRESS
            Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. }) => {
                if modifiers.is_empty() && status == Status::Ignored {
                    let key: Key<&str> = key.as_ref();

                    // Focus query
                    if FOCUS_QUERY_KEYS.contains(&key) {
                        return MainScreen::focus_query() .map(|m| m.into());
                    }
                }

                // Other keys
                match key {
                    // Open first item
                    // TODO bug where this registers when you press ENTER to close another app, which then refocuses on kf
                    Key::Named(Named::Enter) if modifiers.is_empty() && status == Status::Ignored => {
                        return self.open_first_result() .unwrap_or(Command::none());
                    }

                    // Open first item and close
                    /*
                    Key::Named(Named::Enter) if modifiers.command() => {
                        if let Some(Item(_, path)) = self.items.first() {
                            return Command::batch(vec![
                                KFiles::open_path(path),
                                window::close(window::Id::MAIN),
                            ]);
                        }
                    }
                    */

                    _ => {}
                }
            },

            // WINDOW RESIZED
            Event::Window(_, WindowEvent::Resized { .. }) => {
                return MainScreen::fetch_results_bounds().map(|m| m.into());
            }

            _ => {}
        }

        Command::none()
    }
}



