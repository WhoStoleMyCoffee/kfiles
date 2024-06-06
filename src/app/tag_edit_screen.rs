use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use iced::event::Status;
use iced::keyboard::key::Named;
use iced::widget::text_editor::{Action, Content};
use iced::widget::{self, button, column, container, horizontal_space, row, text, text_editor, text_input, Column, Row};
use iced::{Alignment, Color, Command, Event, Length};

use iced_aw::{Bootstrap, Spinner};
use rfd::FileDialog;

use crate::app::Message as AppMessage;
use crate::tag::{ Entries, Tag, TagID };
use crate::widget::context_menu::ContextMenu;
use crate::widget::tag_entry;
use crate::{ icon, icon_button, ToPrettyString };


pub const RENAME_INPUT_ID: fn() -> widget::text_input::Id = || { widget::text_input::Id::new("tag_rename_input") };
const DANGER_COLOR: Color = Color {
    r: 0.9,
    g: 0.1,
    b: 0.0,
    a: 1.0,
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
}

impl TagEditScreen {
    pub fn new(tag: Tag) -> (Self, Command<AppMessage>) {
        (
            TagEditScreen {
                tag,
                entries_editing_content: None,
                renaming_content: None,
                is_loading: false,
            },
            Command::none(),
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
                    fs::remove_file(&path) .unwrap();
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

                self.tag.entries = Entries::from_string_list(&content.text());
                self.save();
            }

            Message::CancelEntriesEdit => {
                self.entries_editing_content = None;
            }

            Message::AddFile => {
                let Some(pick) = FileDialog::new().pick_file() else {
                    return Command::none();
                };

                match self.tag.add_entry(pick) {
                    Ok(()) => {
                        self.save();
                    },
                    Err(err) => {
                        println!("{err}");
                    },
                }
            }

            Message::AddFolder => {
                let Some(pick) = FileDialog::new().pick_folder() else {
                    return Command::none();
                };

                match self.tag.add_entry(pick) {
                    Ok(()) => {
                        self.save();
                    },
                    Err(err) => {
                        println!("Error while adding entry: {err}");
                    },
                }
            }

            Message::StartRename => {
                self.renaming_content = Some(self.tag.id.as_ref().clone());
                return widget::text_input::focus(RENAME_INPUT_ID())
            },

            Message::EndRename => {
                let Some(content) = self.renaming_content.take() else {
                    return Command::none();
                };

                let new_tag = TagID::parse(&content);
                if new_tag == self.tag.id { // ID hasn't changed
                    return Command::none();
                }

                self.is_loading = true;
                let old_path = self.tag.get_save_path();
                self.tag.rename(new_tag) .unwrap();
                let new_path = self.tag.get_save_path();
                self.save();

                return Command::perform(
                    wait_for_path_rename(old_path, new_path),
                    |_| Message::StopLoadingMate.into(),
                );
            }

            Message::CancelRename => {
                self.renaming_content = None;
            }

            Message::RenameInput(str) => {
                self.renaming_content = Some(str);
            }

            // Do nothing since we're currently not loading
            Message::StopLoadingMate => {}
        }

        Command::none()
    }

    fn view_entries(&self) -> Column<AppMessage> {
        let content = match &self.entries_editing_content {
            Some(c) => column![
                text_editor(&c)
                    .on_action(|a| Message::EntriesEditActionPerformed(a).into()),
            ],
            None => column(
                self.tag.entries.as_ref().iter()
                    .map(|pb| text(pb.to_pretty_string())
                         .style(tag_entry::ENTRY_COLOR)
                         .into()
                    )
            ),
        }
        .spacing(8.0)
        .padding([0, 24]);

        // Bottom row
        content.push(if self.entries_editing_content.is_some() {
            row![
                button(icon!(Bootstrap::FloppyFill))
                    .on_press(Message::EndEntriesEdit.into()),
                icon_button!(icon = Bootstrap::X)
                    .on_press(Message::CancelEntriesEdit.into()),
            ]
        } else {
            row![
                // Edit button
                icon_button!(icon = Bootstrap::PencilSquare)
                    .on_press(Message::StartEntriesEdit.into()),

                // Add entry button
                ContextMenu::new(
                    icon_button!(icon = Bootstrap::FolderPlus).on_press(AppMessage::Empty),
                    || column![
                        button("Add folder").on_press(Message::AddFolder.into()),
                        button("Add file")  .on_press(Message::AddFile.into()),
                    ].into()
                )
                .left_click_release_activated(),
            ]
        })
    }

    fn view_label(&self) -> Row<AppMessage> {
        match &self.renaming_content {
            Some(content) => row![
                text_input("Tag name", &content)
                    .id(RENAME_INPUT_ID())
                    .on_input(|str| Message::RenameInput(str).into())
                    .on_submit(Message::EndRename.into()),
                button(icon!(Bootstrap::FloppyFill))
                    .on_press(Message::EndRename.into()),
                icon_button!(icon = Bootstrap::X)
                    .on_press(Message::CancelRename.into()),
            ],
            None => row![
                text(&self.tag.id)
                    .size(24),
                icon_button!(icon = Bootstrap::Pen)
                    .on_press_maybe((!self.is_loading).then_some(Message::StartRename.into())),
            ],
        }

    }

    pub fn view(&self) -> Column<AppMessage> {
        let col = column![
            row![
                // Back arrow
                icon_button!(icon = Bootstrap::ArrowLeft)
                    .on_press_maybe((!self.is_loading).then_some(AppMessage::SwitchToTagListScreen)),
                self.view_label(),
                horizontal_space(),
            ]
            // Menu
            .push_maybe((!self.is_loading && self.renaming_content.is_none()).then(|| 
                ContextMenu::new(
                    icon_button!(icon = Bootstrap::ThreeDots) .on_press(AppMessage::Empty),
                    || column![
                        icon_button!(icon!(Bootstrap::TrashFill, DANGER_COLOR), "Delete")
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
        ];

        if self.is_loading {
            col.push(
                container(Spinner::new() .width(64).height(64))
                    .width(Length::Fill)
                    .center_x()
            )
        } else {
            col.push( self.view_entries() )
        }
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

    fn save(&self) {
        self.tag.save() .unwrap(); // TODO error handling
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



