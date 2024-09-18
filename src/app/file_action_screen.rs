use std::collections::HashMap;
use std::path::PathBuf;

use iced::event::Status;
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, vertical_space, Container
};
use iced::{Color, Command, Element, Event, Length};
use iced_aw::widgets::Grid;
use iced_aw::{ grid_row, Bootstrap, Card, Wrap};

use crate::log::notification::Notification;
use crate::tagging::id::TagID;
use crate::tagging::tag::{LoadError, SaveError};
use crate::tagging::{self, tags_cache, Tag};
use crate::{error, icon, info, log, send_message, tag_list_menu, trace, ToPrettyString};
use crate::{app::Message as AppMessage, simple_button};
use crate::widget::dir_entry::DirEntry;

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);



#[derive(Debug, Clone)]
pub enum Message {
    AddTag(usize),
    RemoveTag(usize),
    RemoveAddingTag(TagID),
    RemoveRemovingTag(TagID),

    RemovePath(usize),
    EntryHovered(usize),
    CancelPressed,
    ApplyPressed,
    ClosePopup,
    ApplyChanges,
}

impl From<Message> for AppMessage {
    fn from(value: Message) -> Self {
        AppMessage::Screen(super::ScreenMessage::FileAction(value))
    }
}


#[derive(Debug)]
pub struct FileActionScreen {
    selected_paths: Vec<PathBuf>,
    hovered_path: Option<PathBuf>,
    popup: Option<Popup>,
    changes: Changes,
}

impl FileActionScreen {
    pub fn new(selected_paths: Vec<PathBuf>) -> (Self, Command<AppMessage>) {
        let load_res = tagging::load_tags();
        let commands: Vec<Command<AppMessage>> = load_res.log_errors::<Vec<Notification>>()
            .unwrap_or_default()
            .into_iter()
            .map(|n| send_message!(notif = n))
            .collect();

        tagging::set_tags_cache( load_res.get_tags().unwrap_or_default() );

        (
            FileActionScreen {
                selected_paths,
                // tags_union,
                hovered_path: None,
                popup: None,
                changes: Changes::new(),
            },
            Command::batch(commands),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::AddTag(index) => {
                if let Some(tag) = tags_cache().get(index) {
                    self.changes.add_tag(tag.id.clone(), true);
                }
                return Command::none();
            }

            Message::RemoveTag(index) => {
                if let Some(tag) = tags_cache().get(index) {
                    self.changes.remove_tag(tag.id.clone(), true);
                }
                return Command::none();
            }

            Message::RemoveAddingTag(tag_id) => {
                self.changes.add_tag(tag_id, false);
            }

            Message::RemoveRemovingTag(tag_id) => {
                self.changes.remove_tag(tag_id, false);
            }

            Message::RemovePath(index) => {
                self.selected_paths.remove(index);
            }

            Message::EntryHovered(index) => {
                self.hovered_path = self.selected_paths.get(index).cloned();
            }

            Message::CancelPressed => {
                self.popup = Some(Popup::Cancel);
            }

            Message::ApplyPressed => {
                self.popup = Some(Popup::Apply);
            }

            Message::ClosePopup => {
                self.popup = None;
            }

            Message::ApplyChanges => {
                trace!("[FileActionScreen::update() => ApplyChanges] Applying changes...");
                let changes = self.changes.apply(&self.selected_paths);
                log!("changes = {:#?}", &changes);

                let files_count = self.selected_paths.len();
                let (added_tags_count, removed_tags_count) = changes.tags.iter()
                    .flat_map(|(_, res)| res)
                    .fold((0usize, 0usize), |(a, r), report| (
                        a + !report.added.is_empty() as usize,
                        r + !report.removed.is_empty() as usize,
                    ));

                let message = match (added_tags_count > 0, removed_tags_count > 0) {
                    (true, true) => format!("{files_count} entries added to {added_tags_count} tags, and removed from {removed_tags_count} tags"),
                    (true, false) => format!("{files_count} entries added to {added_tags_count} tags"),
                    (false, true) => format!("{files_count} entries removed from {removed_tags_count} tags"),
                    (false, false) => "No changes made".to_string(),
                };

                return Command::batch(vec![
                    send_message!(AppMessage::SwitchToMainScreen),
                    send_message!(notif = info!(
                        notify, log_context = "FileActionScreen::update() => ApplyChanges";
                        "{}", message
                    )),
                ])
            }
        }

        Command::none()
    }

    pub fn view(&self) -> Element<AppMessage> {
        if let Some(popup) = &self.popup {
            return popup.view();
        }

        column![
            // Top bar
            row![
                simple_button!(text("Cancel")).on_press(Message::CancelPressed.into()),
                button("Apply").on_press_maybe(
                    (!self.changes.is_empty()).then_some(Message::ApplyPressed.into())
                ),
                text(format!("{} Files", self.selected_paths.len())).size(24),
            ],
            vertical_space() .height(Length::Fixed(12.0)),

            self.view_actions_tags(),

            horizontal_rule(8),

            // Main view
            scrollable( self.view_paths() )
                .height(Length::Fill),
        ]
        // Add hovered path text, if any
        .push_maybe(self.hovered_path.as_ref().map(|pb|
            text(pb.to_pretty_string()).size(12)
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    // TODO UN-DUPLICATE CODE
    fn view_actions_tags(&self) -> Container<AppMessage> {
        let palette = iced::theme::Palette::CATPPUCCIN_MOCHA;
        let dark_text_col = palette.text.inverse();

        let hscrolldir = scrollable::Direction::Horizontal(
            scrollable::Properties::default()
        );

        // TODO view changes on tag click
        let mut grid = Grid::new()
            .vertical_alignment(iced::alignment::Vertical::Center)
            .column_spacing(8);
        if !self.changes.add_tags.is_empty() {
            grid = grid.push(grid_row![
                "Add tags:",
                scrollable(row(self.changes.add_tags.iter().map(|tag_id| button(
                    row![
                        button(icon!(Bootstrap::X, dark_text_col))
                            .on_press(Message::RemoveAddingTag(tag_id.clone()).into())
                            .style(iced::theme::Button::Text)
                            .padding(0),
                        text(tag_id.to_string()),
                    ]
                    .spacing(2)
                )
                .on_press(AppMessage::Empty)
                .into())))
                .direction(hscrolldir.clone())
            ]);
        }
        if !self.changes.remove_tags.is_empty() {
            grid = grid.push(grid_row![
                "Remove tags:",
                scrollable(row(self.changes.remove_tags.iter().map(|tag_id| button(
                    row![
                        button(icon!(Bootstrap::X, dark_text_col))
                            .on_press(Message::RemoveRemovingTag(tag_id.clone()).into())
                            .style(iced::theme::Button::Text)
                            .padding(0),
                        text(tag_id.to_string()),
                    ]
                    .spacing(2)
                )
                .on_press(AppMessage::Empty)
                .into())))
                .direction(hscrolldir.clone())
            ]);
        }

        container(column![
            row![
                text("Tags") .size(24),

                // Add button
                tag_list_menu!(
                    button("+").on_press(AppMessage::Empty),
                    tags_cache().iter().enumerate()
                        .filter(|(_, tag)| !self.changes.add_tags.iter().any(|id| **tag == *id))
                        .map(|(i, tag)| {
                            simple_button!(text(tag.id.to_string()))
                                .on_press( Message::AddTag(i).into() )
                                .into()
                        })
                ),

                // Remove button
                tag_list_menu!(
                    button("-").on_press(AppMessage::Empty),
                    tags_cache().iter().enumerate()
                        .filter(|(_, tag)| !self.changes.remove_tags.iter().any(|id| **tag == *id))
                        .map(|(i, tag)| {
                            simple_button!(text(tag.id.to_string()))
                                .on_press( Message::RemoveTag(i).into() )
                                .into()
                        })
                ),

            ],

            grid,
        ])
        .width(Length::Fill)
    }

    fn view_paths(&self) -> Container<AppMessage> {
        container(Wrap::with_elements(
            self.selected_paths.iter().enumerate()
                .map(|(i, p)| row![
                        button(icon!(Bootstrap::X))
                            .on_press( Message::RemovePath(i).into() )
                            .style( iced::theme::Button::Text )
                            .padding(0),
                        DirEntry::new(p)
                            .width(ITEM_SIZE.0)
                            .height(ITEM_SIZE.1)
                            .on_activate(AppMessage::OpenPath(p.clone()))
                            .on_hover(Message::EntryHovered(i).into()),
                    ]
                    .spacing(0)
                    .into()
                )
                .collect(),
        )
        .width_items(Length::Fill)
        .spacing(ITEM_SPACING.0)
        .line_spacing(ITEM_SPACING.1))
    }

    fn view_tag_changes(&self, tag: &Tag, changes: &ChangeReportTagEntry) -> Container<AppMessage> {
        container(
            text(format!("tag = {}, changes = {:?}", tag.id, changes))
        )
    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }

    pub fn push(&mut self, path: PathBuf) -> bool {
        if self.selected_paths.contains(&path) {
            return false;
        }
        self.selected_paths.push(path);
        true
    }

    pub fn append(&mut self, mut paths: Vec<PathBuf>) {
        paths.retain(|p| !self.selected_paths.contains(&p));
        self.selected_paths.append(&mut paths);
    }
}


#[derive(Debug)]
enum Popup {
    Cancel,
    Apply,
}

impl Popup {
    fn view(&self) -> Element<AppMessage> {
        use iced::theme::Button as BtnStyle;

        let card = match self {
            Popup::Cancel => Card::new(
                text("Cancel"),
                container(column![
                    text("All changes will be lost.\nContinue?"),
                    vertical_space(),
                    row![
                        button("Yes")
                            .on_press(AppMessage::SwitchToMainScreen)
                            .style(BtnStyle::Destructive),
                        button("No")
                            .on_press(Message::ClosePopup.into()),
                    ]
                    .spacing(8)
                ])
                .width(Length::Fill)
                .height(Length::Fill)
            ),

            Popup::Apply => Card::new(
                text("Apply"),
                container(column![
                    text("Apply changes to files?"),
                    vertical_space(),
                    row![
                        button("Ok")
                            .on_press(Message::ApplyChanges.into())
                            .style(BtnStyle::Positive),
                        button("Cancel")
                            .on_press(Message::ClosePopup.into())
                            .style(BtnStyle::Secondary),
                    ]
                    .spacing(8)
                ])
                .width(Length::Fill)
                .height(Length::Fill)
            ),
        };

        container(card)
        .padding(120)
        .center_x()
        .center_y()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}




/// A chages list lol
#[derive(Debug)]
struct Changes {
    add_tags: Vec<TagID>,
    remove_tags: Vec<TagID>,
}

// TODO documentation
impl Changes {
    fn new() -> Changes {
        Self {
            add_tags: Vec::new(),
            remove_tags: Vec::new(),
        }
    }

    /// TODO separate fns for add and remove
    fn add_tag(&mut self, tag_id: TagID, on: bool) {
        if !on {
            if let Some(index) = self.add_tags.iter().position(|t| t == &tag_id) {
                self.add_tags.remove(index);
            }
            return;
        }

        if self.add_tags.contains(&tag_id) {
            return;
        }

        if let Some(index) = self.remove_tags.iter().position(|t| t == &tag_id) {
            self.remove_tags.remove(index);
        }

        self.add_tags.push(tag_id);
    }

    /// TODO separate fns for add and remove
    fn remove_tag(&mut self, tag_id: TagID, on: bool) {
        if !on {
            if let Some(index) = self.remove_tags.iter().position(|t| t == &tag_id) {
                self.remove_tags.remove(index);
            }
            return;
        }

        if self.remove_tags.contains(&tag_id) {
            return;
        }

        if let Some(index) = self.add_tags.iter().position(|t| t == &tag_id) {
            self.add_tags.remove(index);
        }

        self.remove_tags.push(tag_id);
    }

    /// Apply changes to the given paths and return the results
    fn apply(&self, paths: &[PathBuf]) -> ChangesReport {
        let mut report = ChangesReport::new();
        trace!("[file_action_screen::Changes::apply()] Applying changes to {} files...", paths.len());

        // ADD
        for tag_id in self.add_tags.iter() {
            let mut tag = match Tag::load(&tag_id) {
                Ok(t) => t,
                Err(err) => {
                    error!("[file_action_screen::Changes::apply()] Failed to load tag '{}':\n {:?}", tag_id, err);
                    report.tag_load_failed(tag_id.clone(), err);
                    continue;
                }
            };

            for path in paths.iter() {
                match tag.add_entry(path) {
                    Ok(true) => {
                        report.added_entry(path.clone(), &tag.id);
                    },
                    Ok(false) => {},
                    Err(err) => error!(
                        "[file_action_screen::Changes::apply()] Failed to add entry '{}':\n {:?}",
                        path.display(),
                        err
                    ),
                }
            }

            if let Err(err) = tag.save() {
                error!("[file_action_screen::Changes::apply()] Failed to save tag '{}':\n {:?}", &tag.id, err);
                report.tag_save_failed(tag.id.clone(), err);
            }
        }

        // REMOVE
        for tag_id in self.remove_tags.iter() {
            let mut tag = match Tag::load(&tag_id) {
                Ok(t) => t,
                Err(err) => {
                    error!("[file_action_screen::Changes::apply()] Failed to load tag '{}':\n {:?}", tag_id, err);
                    report.tag_load_failed(tag_id.clone(), err);
                    continue;
                }
            };

            for path in paths.iter() {
                if tag.remove_entry(path) {
                    report.removed_entry(path.clone(), tag_id);
                }
            }

            if let Err(err) = tag.save() {
                error!("[file_action_screen::Changes::apply()] Failed to save tag '{}':\n {:?}", &tag.id, err);
                report.tag_save_failed(tag_id.clone(), err);
            }
        }

        report
    }

    fn is_empty(&self) -> bool {
        self.add_tags.is_empty() && self.remove_tags.is_empty()
    }
}


#[derive(Debug)]
enum TagChangeError {
    LoadError(LoadError),
    SaveError(SaveError),
}


#[derive(Debug, Default, Clone)]
struct ChangeReportTagEntry {
    added: Vec<PathBuf>,
    removed: Vec<PathBuf>,
}


#[derive(Debug)]
struct ChangesReport {
    tags: HashMap<TagID, Result<ChangeReportTagEntry, TagChangeError>>,
}

impl ChangesReport {
    fn new() -> Self {
        Self {
            tags: HashMap::new(),
        }
    }

    fn added_entry(&mut self, path: PathBuf, tag_id: &TagID) {
        match self.tags.get_mut(tag_id) {
            Some(Ok(e)) => {
                e.added.push(path);
            }
            Some(Err(_)) => {}
            // New entry
            None => {
                self.tags.insert(tag_id.clone(), Ok(ChangeReportTagEntry {
                    added: vec![ path ],
                    ..Default::default()
                }));
            }
        }
    }
    
    fn removed_entry(&mut self, path: PathBuf, tag_id: &TagID) {
        match self.tags.get_mut(tag_id) {
            Some(Ok(e)) => {
                e.removed.push(path);
            }
            Some(Err(_)) => {}
            // New entry
            None => {
                self.tags.insert(tag_id.clone(), Ok(ChangeReportTagEntry {
                    removed: vec![ path ],
                    ..Default::default()
                }));
            }
        }
    }

    fn tag_save_failed(&mut self, tag_id: TagID, err: SaveError) {
        self.tags.insert(tag_id, Err(TagChangeError::SaveError(err)) );
    }

    fn tag_load_failed(&mut self, tag_id: TagID, err: LoadError) {
        self.tags.insert(tag_id, Err(TagChangeError::LoadError(err)) );
    }
}







