use std::collections::HashSet;
use std::iter;
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::event::Status;
use iced::keyboard::Key;
use iced::widget::scrollable::Viewport;
use iced::widget::{self, button, column, container, horizontal_space, row, scrollable, text, text_input, tooltip, Column, Container};
use iced::{self, keyboard, Element, Event, Length, Rectangle};
use iced::Command;
use iced_aw::Bootstrap;
use rand::Rng;

use crate::configs::Configs;
use crate::log::notification::Notification;
use crate::search::Query;
use crate::tagging::{self, tag::Tag, id::TagID};
use crate::thumbnail::{self, get_thumbnail_cache_path, ThumbnailBuilder};
use crate::widget::file_inspector::FileInspector;
use crate::widget::fuzzy_input::FuzzyInput;
use crate::widget::file_list::{self, FileList};
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
    EntryHovered(usize),
    EntrySelected(usize),
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

impl AsRef<Path> for Item {
    fn as_ref(&self) -> &Path {
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
    /// TODO select multiple paths
    selected_path: Option<PathBuf>,
    tags_cache: Vec<TagID>,
}

impl MainScreen {
    pub fn new() -> (Self, Command<AppMessage>) {
        let mut commands: Vec<Command<AppMessage>> = vec![
            MainScreen::fetch_results_bounds() .map(|m| m.into()),
            text_input::focus( QUERY_INPUT_ID() ),
        ];

        let load_res = tagging::load_tags();
        commands.extend(
            load_res.log_errors::<Vec<Notification>>()
                .unwrap_or_default()
                .into_iter()
                .map(|n| send_message!(notif = n))
        );
        tagging::set_tags_cache( load_res.get_tags().unwrap_or_default() );
        let tags_cache: Vec<TagID> = tagging::tags_cache()
            .iter()
            .map(|t| t.id.clone())
            .collect();

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
                selected_path: None,
                tags_cache,
            },
            Command::batch(commands),
        )
    }

    pub fn tick(&mut self) -> Command<AppMessage> {
        self.thumbnail_handler.update();

        // Build thumbnails
        if let Some(range) = self.results_container_bounds.as_ref()
            .map(|Rectangle { width, height, .. }| file_list::get_visible_items_range(*width, *height, self.scroll))
        {
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

            Message::EntryHovered(index) => {
                if let Some(Item(_, path)) = self.items.get(index) {
                    self.hovered_path = Some(path.clone());
                }
            }

            Message::EntrySelected(index) => {
                self.selected_path = self.items.get(index).map(|Item(_, p)| p.clone());
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
        let query_input = self.view_query_input();

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

                row![
                    self.view_results(),
                ]
                // .push_maybe(self.selected_path.as_ref()
                //     .map(|path| FileInspector::new(path, &self.tags_cache)
                // ))

            ]
            // Add hovered path text, if any
            .push_maybe(self.hovered_path.as_ref().map(|pb|
                text(pb.to_pretty_string()) .size(12)
            ))
            .spacing(8),
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

    fn view_results(&self) -> Container<AppMessage> {
        use widget::scrollable::{ Direction, Properties };

        let list = FileList::new(&self.items)
            .cull( self.results_container_bounds.as_ref().map(|r| r.size()), self.scroll )
            .with_selected_maybe( self.selected_path.as_deref() )
            .on_item_hovered(|i| Message::EntryHovered(i).into())
            .on_item_selected(|i| Message::EntrySelected(i).into())
            .on_item_activated(|pb| AppMessage::OpenPath(pb));

        container(
            scrollable(list)
                .id(MAIN_SCROLLABLE_ID())
                .direction(Direction::Vertical(Properties::default()))
                .on_scroll(|vp| Message::ResultsScrolled(vp).into())
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .id(MAIN_RESULTS_ID())
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



