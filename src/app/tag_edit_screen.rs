use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use iced::event::Status;
use iced::keyboard::key::Named;
use iced::widget::text_editor::{Action, Content};
use iced::widget::{self, button, checkbox, column, container, horizontal_rule, horizontal_space, row, scrollable, text, text_editor, text_input, vertical_space, Column, Row};
use iced::{Alignment, Color, Command, Event, Length};

use iced_aw::{Bootstrap, Spinner};
use rfd::FileDialog;

use crate::app::notification::{error_message, info_message, warning_message};
use crate::app::Message as AppMessage;
use crate::tag::tag::SelfReferringSubtag;
use crate::tag::{ self, entries::Entries, Tag, id::TagID };
use crate::widget::context_menu::ContextMenu;
use crate::widget::tag_entry;
use crate::{ icon, send_message, simple_button, ToPrettyString };

use super::theme;


pub const RENAME_INPUT_ID: fn() -> widget::text_input::Id = || { widget::text_input::Id::new("tag_rename_input") };
const DANGER_COLOR: Color = Color {
    r: 0.9,
    g: 0.1,
    b: 0.0,
    a: 1.0,
};

// IDs
const MAIN_SCROLLABLE_ID: fn() -> scrollable::Id = || {
    scrollable::Id::new("tag_edit_scrollable") 
};


#[derive(Debug, Clone)]
pub enum Message {
    StartEntriesEdit,
    EndEntriesEdit,
    CancelEntriesEdit,
    EntriesEditActionPerformed(Action),
    AddFile,
    AddFolder,

    StartRename,
    EndRename,
    CancelRename,
    RenameInput(String),

    Delete,
    /// Just stop loading mate
    StopLoadingMate,

    /// Add or remove a subtag from the current [`Tag`]
    SubtagToggled(TagID, bool),
    SubtagPressed(usize),
}

impl From<Message> for AppMessage {
    fn from(message: Message) -> AppMessage {
        AppMessage::Screen(super::ScreenMessage::TagEdit(message))
    }
}




#[derive(Debug)]
pub struct TagEditScreen {
    tag: Tag,
    entries_editing_content: Option<Content>,
    renaming_content: Option<String>,
    is_loading: bool,
    tags_cache: Vec<TagID>,
}

impl TagEditScreen {
    pub fn new(tag: Tag) -> (Self, Command<AppMessage>) {
        let (tags_cache, tag_load_command) = match tag::get_all_tag_ids() {
            Ok(v) => (v, Command::none()),
            Err(err) => (
                Vec::new(),
                send_message!(error_message(
                    format!("Failed to load tags:\n{}", err),
                ))
            )
        };

        (
            TagEditScreen {
                tag,
                entries_editing_content: None,
                renaming_content: None,
                is_loading: false,
                tags_cache,
            },
            Command::batch(vec![
                tag_load_command,
                scrollable::snap_to(
                    MAIN_SCROLLABLE_ID(),
                    scrollable::RelativeOffset::START,
                ),
            ])
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        if self.is_loading {
            if matches!(message, Message::StopLoadingMate) {
                self.is_loading = false;
            }
            return Command::none();
        }

        match message {
            Message::StartEntriesEdit => {
                let text: String = self.tag.entries.to_string_list();
                self.entries_editing_content = Some(Content::with_text(&text));
            },

            Message::EntriesEditActionPerformed(action) => {
                let Some(content) = &mut self.entries_editing_content else {
                    return Command::none();
                };
                content.perform(action);
            },

            Message::Delete => {
                self.is_loading = true;
                let path: PathBuf = self.tag.get_save_path();
                if path.exists() {
                    if let Err(err) = fs::remove_file(&path) {
                        let pathstr: String = path.to_pretty_string();
                        return send_message![
                            error_message(format!("Failed to remove file {}:\n{}", pathstr, err)),
                            AppMessage::SwitchToTagListScreen,
                        ]
                    }
                }

                return Command::perform(
                    wait_for_path_deletion(path),
                    |_| AppMessage::SwitchToTagListScreen,
                );
            }

            Message::EndEntriesEdit => {
                let Some(content) = self.entries_editing_content.take() else {
                    return Command::none();
                };

                let entries = &mut self.tag.entries;
                *entries = Entries::from_string_list(&content.text());

                return Command::batch(vec![
                    self.filter_duplicate_entries(),
                    self.save(),
                ]);
            }

            Message::CancelEntriesEdit => {
                self.entries_editing_content = None;
            }

            Message::AddFile => {
                let Some(pick) = FileDialog::new().pick_file() else {
                    return Command::none();
                };
                return self.add_entry(pick);
            }

            Message::AddFolder => {
                let Some(pick) = FileDialog::new().pick_folder() else {
                    return Command::none();
                };

                return self.add_entry(pick);
            }

            Message::StartRename => {
                self.renaming_content = Some(self.tag.id.as_ref().clone());
                return Command::batch(vec![
                    widget::text_input::focus(RENAME_INPUT_ID()),
                    widget::text_input::select_all(RENAME_INPUT_ID()),
                ]);
            },

            Message::EndRename => {
                let Some(content) = self.renaming_content.take() else {
                    return Command::none();
                };

                let new_id = TagID::parse(content);
                return self.rename(new_id);
            }

            Message::CancelRename => {
                self.renaming_content = None;
            }

            Message::RenameInput(str) => {
                self.renaming_content = Some(str);
            }

            // Do nothing since we're currently not loading
            Message::StopLoadingMate => {}

            Message::SubtagPressed(index) => {
                let Some(tag_id) = self.tag.get_subtags().get(index) else {
                    return Command::none();
                };

                // TODO handle unwrap
                let tag = Tag::load(tag_id) .unwrap();
                return send_message!(AppMessage::SwitchToTagEditScreen(tag))
            }

            Message::SubtagToggled(tag_id, is_on) => {
                if is_on {
                    if let Err(SelfReferringSubtag) = self.tag.add_subtag(&tag_id) {
                        self.tag.remove_subtag(&tag_id);
                        return send_message!(error_message(
                            format!("Cannot to subtag \"{}\" with itself", tag_id)
                        ));
                    };
                } else {
                    self.tag.remove_subtag(&tag_id);
                }

                return self.save();
            }
        }

        Command::none()
    }

    fn view_entries(&self) -> Column<AppMessage> {
        let content = column![
            text("Entries:").size(24)
        ]
        .spacing(8.0)
        .padding([0, 24]);

        // The actual entries
        let content = content.push(match &self.entries_editing_content {
            Some(c) => column![
                text_editor(c)
                    .on_action(|a| Message::EntriesEditActionPerformed(a).into()),
            ],
            None => column(
                self.tag.entries.as_ref().iter()
                    .map(|pb| {
                        let row = Row::new()
                            .push( text(pb.to_pretty_string()).style(tag_entry::ENTRY_COLOR) )
                            .spacing(8);
                        if pb.exists() {
                            return row.into();
                        }
                        return row.extend(vec![
                            horizontal_space().width(64).into(),
                            icon!(Bootstrap::ExclamationCircleFill, theme::ERROR_COLOR).into(),
                            text("Path doesn't exist") .style(theme::ERROR_COLOR).into(),
                        ])
                        .into()
                    })
            ),
        });

        // Bottom row
        let bottom_row = if self.entries_editing_content.is_some() {
            row![
                button(icon!(Bootstrap::FloppyFill))
                    .on_press(Message::EndEntriesEdit.into()),
                simple_button!(icon = Bootstrap::X)
                    .on_press(Message::CancelEntriesEdit.into()),
            ]
        } else {
            row![
                // Edit button
                simple_button!(icon = Bootstrap::PencilSquare)
                    .on_press(Message::StartEntriesEdit.into()),

                // Add entry button
                ContextMenu::new(
                    simple_button!(icon = Bootstrap::FolderPlus).on_press(AppMessage::Empty),
                    || column![
                        button("Add folder").on_press(Message::AddFolder.into()),
                        button("Add file")  .on_press(Message::AddFile.into()),
                    ].into()
                )
                .left_click_release_activated(),
            ]
        };

        content
            .push(bottom_row)
            .push(vertical_space().height(64))

    }

    fn view_label(&self) -> Row<AppMessage> {
        match &self.renaming_content {
            Some(content) => row![
                text_input("Tag name", content)
                    .id(RENAME_INPUT_ID())
                    .on_input(|str| Message::RenameInput(str).into())
                    .on_submit(Message::EndRename.into()),
                button(icon!(Bootstrap::FloppyFill))
                    .on_press(Message::EndRename.into()),
                simple_button!(icon = Bootstrap::X)
                    .on_press(Message::CancelRename.into()),
            ],
            None => row![
                text(&self.tag.id)
                    .size(32),
                simple_button!(icon = Bootstrap::Pen)
                    .on_press_maybe((!self.is_loading).then_some(Message::StartRename.into())),
            ],
        }
    }

    fn view_top(&self) -> Column<AppMessage> {
        column![
            row![
                // Back arrow
                simple_button!(icon = Bootstrap::ArrowLeft)
                    .on_press_maybe((!self.is_loading).then_some(AppMessage::SwitchToTagListScreen)),
                self.view_label(),
                horizontal_space(),
            ]
            // Menu
            .push_maybe((!self.is_loading && self.renaming_content.is_none()).then(|| 
                ContextMenu::new(
                    simple_button!(icon = Bootstrap::ThreeDots) .on_press(AppMessage::Empty),
                    || column![
                        simple_button!(icon!(Bootstrap::TrashFill, DANGER_COLOR), "Delete")
                            .on_press(Message::Delete.into())
                            .width(Length::Fill),
                    ]
                    .max_width(120)
                    .into()
                )
                .left_click_release_activated(),
            ))
            .align_items(Alignment::Center)
            .spacing(16),

            self.view_subtags_row(),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
    }

    fn view_subtags_row(&self) -> Row<AppMessage> {
        row![
            // Context menu button
            ContextMenu::new(
                button("t") .on_press(AppMessage::Empty),
                || container(scrollable(column(
                    self.tags_cache.iter()
                        .filter(|id| **id != self.tag.id)
                        .map(|id|
                             checkbox(id.to_string(), id.is_subtag_of(&self.tag))
                                .on_toggle(|is_on| Message::SubtagToggled(id.clone(), is_on).into())
                                .into()
                        )
                )))
                .max_width(240)
                .max_height(480)
                .style(container::Appearance::default() .with_background(Color::BLACK))
                .into()
            )
            .left_click_release_activated(),
        ]
        // List
        .extend(self.tag.get_subtags().iter().enumerate()
            .map(|(i, tag_id)|
                button(text( tag_id.to_string() ))
                    .on_press(Message::SubtagPressed(i).into())
                    .into()
            )
        )
        .align_items(Alignment::Center)
    }

    pub fn view(&self) -> Column<AppMessage> {
        use scrollable::{Direction, Properties};

        let col = self.view_top();

        if self.is_loading {
            return col.push(
                container(Spinner::new() .width(64).height(64))
                    .width(Length::Fill)
                    .center_x()
            );
        }

        // MAIN
        col.extend(vec![
            horizontal_rule(1).into(),
            scrollable(self.view_entries())
            .id(MAIN_SCROLLABLE_ID())
            .direction(Direction::Both {
                vertical: Properties::default(),
                horizontal: Properties::default() .width(4).scroller_width(4),
            })
            .into(),
        ])
    }

    pub fn handle_event(&mut self, event: Event, status: Status) -> Command<AppMessage> {
        use iced::keyboard::{Event as KeyboardEvent, Key};

        if status != Status::Ignored {
            return Command::none();
        }

        let Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. }) = event else {
            return Command::none();
        };

        // Esc to cancel whatever
        if key == Key::Named(Named::Escape) && modifiers.is_empty() {
            // Cancel renaming tag
            if self.renaming_content.is_some() {
                self.renaming_content = None;
            }
            // Cancel editing entries
            else {
                self.entries_editing_content = None;
            }

            return Command::none();
        }

        Command::none()
    }

    /// Rename the inner tag to `new_id`
    /// This function is technically recursive, but there will be no infinite recursion because we
    /// make the tag unique
    /// There will be *at most* 1 recursive call in case the tag already exists, and that's it
    fn rename(&mut self, new_id: TagID) -> Command<AppMessage> {
        use crate::tag::RenameError;

        let old_path = self.tag.get_save_path();
        let new_path = new_id.get_path();

        match self.tag.rename(&new_id) {
            // Renaming was successful
            Ok(true) => {
                self.is_loading = true;

                // We don't use `self.save()` because we don't want to wait if it fails
                if let Err(err) = self.tag.save() {
                    return send_message!(error_message(
                        format!("Failed to save tag:\n{}", err)
                    ))
                }

                return Command::perform(
                    wait_for_path_rename(old_path, new_path),
                    |_| Message::StopLoadingMate.into(),
                );
            }

            // Nothing has changed
            Ok(false) => {}

            Err(err) => match err {
                // Already exists; make name unique and try again
                RenameError::AlreadyExists => {
                    let id: String = new_id.as_ref().clone();

                    let tags = match tag::get_all_tag_ids() {
                        Ok(v) => v,
                        Err(err) => return send_message!(error_message(
                            format!("Failed to get tags list:\n{}", err)
                        ))
                    };

                    return Command::batch(vec![
                        // Already exists warning
                        send_message!(warning_message(
                            format!("Tag \"{}\" already exists", id)
                        )),

                        // There will be no infinite recursion because we make the tag unique
                        self.rename(new_id.make_unique_in(&tags)) 
                    ]);
                }

                RenameError::IO(err) => {
                    return send_message!(error_message(
                        format!("Failed to rename tag:\n{}", err)
                    ));
                }
            }
        }

        Command::none()
    }

    /// Save the current tag to disk and notify any errors via a [`Command`]
    fn save(&self) -> Command<AppMessage> {
        let Err(err) = self.tag.save() else {
            return Command::none();
        };
        send_message!(error_message(
            format!("Failed to save tag:\n{}", err)
        ))
    }

    /// Add the entry `path` to the current tag's [`Entries`] and notify any errors via a [`Command`]
    fn add_entry(&mut self, path: PathBuf) -> Command<AppMessage> {
        use tag::AddEntryError;

        match self.tag.add_entry(&path) {
            Ok(()) => self.save(),
            Err(AddEntryError::AlreadyContained) => {
                let pathstr: String = path.to_pretty_string();
                send_message!(info_message(
                    format!("Entry \"{}\" is already contained", pathstr)
                ))
            }
            Err(err) => {
                let pathstr: String = path.to_pretty_string();
                send_message!(error_message(
                    format!("Failed to add entry \"{}\":\n{}", pathstr, err)
                ))
            }
        }
    }

    fn filter_duplicate_entries(&mut self) -> Command<AppMessage> {
        Command::batch(
            self.tag.entries.filter_duplicates()
                .into_iter()
                .map(|pb| 
                    send_message!(info_message(
                        format!("Entry \"{}\" is already contained", pb.to_pretty_string())
                    )),
                )
        )
    }

}



async fn wait_for_path_deletion(path: PathBuf) {
    use crate::fs::{DeletionWatcher, Watcher};
    let _ = DeletionWatcher {
        path,
        check_interval: Duration::from_millis(500),
    }
    .wait(Duration::from_secs(10));
}

async fn wait_for_path_creation(path: PathBuf) {
    use crate::fs::{CreationWatcher, Watcher};
    let _ = CreationWatcher {
        path,
        check_interval: Duration::from_millis(500),
    }
    .wait(Duration::from_secs(10));
}

async fn wait_for_path_rename(old_path: PathBuf, new_path: PathBuf) {
    wait_for_path_deletion(old_path).await;
    wait_for_path_creation(new_path).await;
}



