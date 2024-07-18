use std::collections::HashSet;
use std::iter;
use std::ops::Range;
use std::path::PathBuf;

use iced::event::Status;
use iced::keyboard::Key;
use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, text_input, tooltip, Column};
use iced::{self, keyboard, Element, Event, Length, Rectangle};
use iced::Command;
use iced_aw::{Bootstrap, Wrap};
use rand::Rng;

use crate::configs::Configs;
use crate::search::Query;
use crate::tagging::{self, tag::Tag, id::TagID};
use crate::thumbnail::{self, get_thumbnail_cache_path, ThumbnailBuilder};
use crate::widget::{dir_entry::DirEntry, fuzzy_input::FuzzyInput};
use crate::app::{theme, Message as AppMessage};
use crate::{configs, error, icon, send_message, warn, ToPrettyString};


/// Keys that focus the query input
/// These are hard coded for now
/// TODO make these configurable
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
const MAIN_SCROLLABLE_ID: fn() -> scrollable::Id = || { scrollable::Id::new("main_scrollable") };


#[derive(Debug, Clone)]
pub enum Message {
    QueryTextChanged(String),
    QuerySubmit,
    ToggleQueryTag(TagID),
    RemoveQueryTag(TagID),
    QueryTagPressed(TagID),
    FocusQuery,
    ResultsScrolled(Viewport),
    ResultsBoundsFetched(Option<Rectangle>),
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
    query_input: String,
    items: Vec<Item>,
    thumbnail_handler: ThumbnailHandler,
    scroll: f32,
    results_container_bounds: Option<Rectangle>,
    hovered_path: Option<PathBuf>,
    tags_cache: Vec<TagID>,
}

impl MainScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        let mut commands: Vec<Command<AppMessage>> = vec![
            MainScreen::fetch_results_bounds() .map(|m| m.into()),
            text_input::focus( QUERY_INPUT_ID() ),
        ];

        let tags_cache = match tagging::get_all_tag_ids() {
            Ok(v) => v,
            Err(err) => {
                commands.push(send_message!(notif = error!(
                    notify, log_context = "MainScreen::new()";
                    "Failed to load tags:\n{}", err
                )));
                Vec::new()
            }
        };

        let cfg = configs::global();

        (
            MainScreen {
                query: Query::empty(),
                query_input: String::default(),
                items: Vec::new(),
                thumbnail_handler: ThumbnailHandler::new(&cfg),
                scroll: 0.0,
                results_container_bounds: None,
                hovered_path: None,
                tags_cache,
            },
            Command::batch(commands),
        )
    }

    pub fn tick(&mut self) -> Command<AppMessage> {
        self.thumbnail_handler.update();
        if let Some(range) = self.get_visible_items_range() {
            self.thumbnail_handler.build(&self.items, range);
        }
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
        let path = path.to_path_buf();
        Some(send_message!( AppMessage::OpenPath(path) ))
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::FocusQuery => {
                return MainScreen::focus_query() .map(|m| m.into());
            }

            Message::QueryTextChanged(new_text) => {
                let has_changed: bool = self.set_query_input(new_text);
                if has_changed {
                    return self.restart_search();
                }
            }

            Message::QuerySubmit => {
                return self.open_first_result() .unwrap_or(Command::none());
            }

            Message::ToggleQueryTag(tag_id) => {
                let removed: bool = self.query.remove_tag(&tag_id);
                // If not removed, then add it
                if !removed {
                    let tag: Tag = match tag_id.load() {
                        Ok(t) => t,
                        Err(err) => return send_message!(notif = error!(
                            notify, log_context = "MainScreen::update() => ToggleQueryTag";
                            "Failed to load tag `{}`:\n{}", tag_id, err
                        )),
                    };

                    self.query.add_tag(tag);
                }

                self.set_query_input(String::new());
                return self.restart_search();
            }

            Message::RemoveQueryTag(tag_id) => {
                self.query.remove_tag(&tag_id);
                return self.restart_search();
            }

            Message::QueryTagPressed(tag_id) => {
                let tag = match tag_id.load() {
                    Ok(tag) => tag,
                    Err(err) => return send_message!(notif = error!(
                        notify, log_context = "MainScreen::update() => QueryTagPressed";
                        "Failed to load tag `{}`:\n{:?}", tag_id, err
                    )),
                };
                return send_message!(AppMessage::SwitchToTagEditScreen(tag));
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
                    tooltip(
                        button( icon!(Bootstrap::BookmarkStar) ) .on_press(AppMessage::SwitchToTagListScreen),
                        "Tags",
                        tooltip::Position::Bottom
                    ),
                    tooltip(
                        button( icon!(Bootstrap::GearFill) ) .on_press(AppMessage::SwitchToConfigScreen),
                        "Settings",
                        tooltip::Position::Bottom
                    )
                ]
                .spacing(8),

                query_input,
                text("Results:"),
                container(
                    scrollable(results)
                        .id(MAIN_SCROLLABLE_ID())
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
        let palette = iced::theme::Palette::CATPPUCCIN_MOCHA;
        let dark_text_col = palette.text.inverse();

        column![
            // Tags
            row(self.query.tags.iter().map(|tag| {
                let id = &tag.id;
                button(
                    row![
                        button(icon!(Bootstrap::X, dark_text_col))
                            .on_press( Message::RemoveQueryTag(id.clone()).into() )
                            .style( iced::theme::Button::Text )
                            .padding(0),
                        text(id).size(14),
                    ]
                    .align_items(iced::Alignment::Center)
                )
                .on_press( Message::QueryTagPressed(id.clone()).into() )
                .into()
            }))
            .spacing(2),

            // Fuzzy text input
            FuzzyInput::new(
                "Query...",
                &self.query_input,
                &self.tags_cache,
                |tag_id| Message::ToggleQueryTag(tag_id).into(),
            )
            .text_input(|text_input| {
                text_input
                    .id(QUERY_INPUT_ID())
                    .on_input(|text| Message::QueryTextChanged(text).into())
                    .on_submit(Message::QuerySubmit.into())
            })
            .hide_on_empty( !self.query.tags.is_empty() )
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
                        .on_select(AppMessage::OpenPath(pb.clone()))
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

    /// Get the range of items which are visible in the main view
    /// come to think of it, there was probably a better way to do all this culling thing
    fn get_visible_items_range(&self) -> Option<Range<usize>> {
        let Rectangle { width, height, .. } = self.results_container_bounds?;

        let items_per_row: usize = (width / TOTAL_ITEM_SIZE.0) as usize;
        //          (        Which row do we start at?       ) * items per row
        let start = (self.scroll / TOTAL_ITEM_SIZE.1) as usize * items_per_row;
        let end = start
        //  + (    How many rows does the view span?    ) * items per row
            + ((height / TOTAL_ITEM_SIZE.1) as usize + 2) * items_per_row;

        Some(start..end)
    }

    pub fn restart_search(&mut self) -> Command<AppMessage> {
        self.items.clear();
        self.query.search();
        self.scroll = 0.0;
        self.hovered_path = None;

        iced::widget::scrollable::snap_to(
            MAIN_SCROLLABLE_ID(),
            scrollable::RelativeOffset { x: 0.0, y: 0.0 }
        )
    }

    pub fn set_query_input(&mut self, new_text: String) -> bool {
        let has_changed: bool = self.query.parse_query(&new_text);
        self.query_input = new_text;
        has_changed
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




#[derive(Debug)]
struct ThumbnailHandler {
    /// Current index in [`MainScreen::items`]
    index: usize,
    builder: ThumbnailBuilder,
    update_prob: f64,
    /// List of paths that have previously failed to have their thumbnails built, so we don't
    /// try to rebuild them repeatedly
    failed_paths: HashSet<PathBuf>,
    /// How far to check for available paths
    max_check_depth: u32,
}

impl ThumbnailHandler {
    pub fn new(configs: &Configs) -> ThumbnailHandler {
        ThumbnailHandler {
            index: 0,
            builder: ThumbnailBuilder::new(configs.thumbnail_thread_count),
            update_prob: configs.thumbnail_update_prob as f64,
            failed_paths: HashSet::new(),
            max_check_depth: 10,
        }
    }

    pub fn update(&mut self) {
        use thumbnail::BuildError;

        for BuildError { path, error } in self.builder.update().into_iter() .filter_map(|r| r.err())
        {
            warn!("Failed to build thumbnail for \"{}\":\n {:?}", path.display(), error);
            self.failed_paths.insert(path);
        }

    }

    pub fn build(
        &mut self,
        items: &Vec<Item>,
        visible_items_range: Range<usize>,
    ) {
        let Some(path) = self.next(items, visible_items_range) else {
            return;
        };

        self.builder.build(path);
    }

    fn next<'a>(
        &mut self,
        items: &'a [Item],
        visible_items_range: Range<usize>,
    ) -> Option<&'a PathBuf>
    {
        for _ in 0..self.max_check_depth {
            let Some(Item(_, path)) = items.get(self.index) else {
                self.index = visible_items_range.start;
                return None;
            };
            
            if !visible_items_range.contains(&self.index) {
                self.index = visible_items_range.start;
                return None;
            }

            self.index += 1;

            if path.is_dir() || !thumbnail::is_file_supported(path) {
                continue;
            }

            // If thumbnail already exists, don't try to rebuild it some percentage
            // of the time (configurable)
            if get_thumbnail_cache_path(path).exists()
                && !rand::thread_rng().gen_bool(self.update_prob)
            {
                continue;
            }

            // Don't rebuild those that have failed before, but do give them a little chance
            if self.failed_paths.contains(path) {
                if !rand::thread_rng().gen_bool(self.update_prob) {
                    continue;
                }
                self.failed_paths.remove(path);
            }

            return Some(path);
        }

        None
    }
}



