use std::collections::HashSet;
use std::path::PathBuf;

use iced::event::Status;
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, text_input, vertical_space, Container
};
use iced::{Command, Element, Event, Length};
use iced_aw::{Bootstrap, Card, Wrap};

use crate::tagging::id::TagID;
use crate::tagging::Tag;
use crate::widget::fuzzy_input::FuzzyInput;
use crate::{error, icon, send_message, tagging, trace, ToPrettyString};
use crate::{app::Message as AppMessage, simple_button};
use crate::widget::dir_entry::DirEntry;

use super::theme;


const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);

// Ids
const QUERY_INPUT_ID: fn() -> text_input::Id = || text_input::Id::new("query_input");



#[derive(Debug, Clone)]
pub enum Message {
    RemovePath(usize),
    ToggleTag(TagID),
    RemoveTag(usize),
    TagTextChanged(String),
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
    selected_tags: Vec<TagID>,
    tags_cache: Vec<TagID>,
    tag_text_input: String,
    hovered_path: Option<PathBuf>,
    popup: Option<Popup>,
}

impl FileActionScreen {
    pub fn new(selected_paths: Vec<PathBuf>) -> (Self, Command<AppMessage>) {
        let mut commands: Vec<Command<AppMessage>> = Vec::new();
        let tags_cache: Vec<Tag> = load_tags(&mut commands);
        let intersecting_tags = get_intersecting_tags(&selected_paths, &tags_cache);

        (
            FileActionScreen {
                selected_paths,
                selected_tags: intersecting_tags,
                tags_cache: tags_cache.into_iter().map(|t| t.id).collect(),
                tag_text_input: String::new(),
                hovered_path: None,
                popup: None,
            },
            Command::batch(commands),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<AppMessage> {
        match message {
            Message::RemovePath(index) => {
                self.selected_paths.remove(index);
            }

            Message::ToggleTag(tag_id) => {
                if let Some(index) = self.selected_tags.iter().position(|id| id == &tag_id) {
                    self.selected_tags.remove(index);
                } else {
                    self.selected_tags.push(tag_id);
                }

                self.tag_text_input.clear();
            }

            Message::RemoveTag(index) => {
                if index < self.selected_tags.len() {
                    self.selected_tags.remove(index);
                } else {
                    error!("[FileActionScreen::update() => RemoveQueryTag] Index out of bounds:\n index = {}\n selected_tags.len() = {}",
                        index,
                        self.selected_tags.len()
                    )
                }
            },

            Message::TagTextChanged(input) => {
                self.tag_text_input = input;
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
                self.apply_changes();
                return send_message!(AppMessage::SwitchToMainScreen);
            }
        }

        Command::none()
    }

    pub fn view(&self) -> Element<AppMessage> {
        let palette = iced::theme::Palette::CATPPUCCIN_MOCHA;

        if let Some(popup) = &self.popup {
            return popup.view();
        }

        column![
            // Top bar
            row![
                simple_button!( text("Cancel").style(palette.text) )
                    .on_press(Message::CancelPressed.into()),
                button("Apply") .on_press(Message::ApplyPressed.into()),
                text( format!("{} Files", self.selected_paths.len()) ) .size(24),
            ],
            vertical_space() .height(Length::Fixed(12.0)),

            self.view_actions_tags(),

            horizontal_rule(8),
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

    fn view_actions_tags(&self) -> Container<AppMessage> {
        let palette = iced::theme::Palette::CATPPUCCIN_MOCHA;
        let dark_text_col = palette.text.inverse();

        container(column![
            text("Tags:"),
            // Tags
            container(scrollable(Wrap::with_elements(
                    self.selected_tags.iter().enumerate()
                        .map(|(i, tag_id)| {
                            button(
                                row![
                                    button(icon!(Bootstrap::X, dark_text_col))
                                        .on_press(Message::RemoveTag(i).into())
                                        .style(iced::theme::Button::Text)
                                        .padding(0),
                                    text(&tag_id).size(14),
                                ]
                                .align_items(iced::Alignment::Center),
                            )
                            .on_press(AppMessage::Empty)
                            .into()
                        })
                        .collect()
                )
                .spacing(2.0)
            ))
            .max_height(120),

            // Fuzzy text input
            FuzzyInput::new(
                "Tags...",
                &self.tag_text_input,
                &self.tags_cache,
                |tag_id| Message::ToggleTag(tag_id).into(),
            )
            .text_input(|text_input| text_input
                .id(QUERY_INPUT_ID())
                .on_input(|text| Message::TagTextChanged(text).into())
            )
            .style(theme::Simple),
        ])
        .width(Length::Fill)
    }

    fn view_paths(&self) -> Wrap<AppMessage, iced_aw::direction::Horizontal> {
        Wrap::with_elements(
            self.selected_paths.iter().enumerate()
                .map(|(i, p)| row![
                        button(icon!(Bootstrap::X))
                            .on_press( Message::RemovePath(i).into() )
                            .style( iced::theme::Button::Text )
                            .padding(0),
                        DirEntry::new(p)
                            .width(ITEM_SIZE.0)
                            .height(ITEM_SIZE.1)
                            .on_select(AppMessage::OpenPath(p.clone()))
                            .on_hover(Message::EntryHovered(i).into()),
                    ]
                    .spacing(0)
                    .into()
                )
                .collect(),
        )
        .width_items(Length::Fill)
        .spacing(ITEM_SPACING.0)
        .line_spacing(ITEM_SPACING.1)
    }

    pub fn handle_event(&mut self, _event: Event, _status: Status) -> Command<AppMessage> {
        Command::none()
    }

    /// Doesn't update the selected tags
    pub fn push(&mut self, path: PathBuf) -> bool {
        if self.selected_paths.contains(&path) {
            return false;
        }
        self.selected_paths.push(path);
        true
    }

    /// Doesn't update the selected tags
    pub fn append(&mut self, mut paths: Vec<PathBuf>) {
        paths.retain(|p| !self.selected_paths.contains(&p));
        self.selected_paths.append(&mut paths);
    }

    /// TODO documentation
    pub fn apply_changes(&mut self) -> Command<AppMessage> {
        trace!("[FileActionScreen::apply_changes()] Applying changes...");

        let mut commands = Vec::new();
        let selected_set = HashSet::<&TagID>::from_iter(self.selected_tags.iter());

        for  mut tag in load_tags(&mut commands).into_iter() {
            let mut tag_changed: bool = false;

            // Add all selected paths to this tag
            if selected_set.contains(&tag.id) {
                for path in self.selected_paths.iter() {
                    match tag.add_entry(path) {
                        Err(err) => {
                            error!("[FileActionScreen::apply_changes()] Failed to add path {}:\n {:?}",
                                path.display(), err
                            );
                        }
                        Ok(true) => {
                            trace!("[FileActionScreen::apply_changes()] Added path {} to tag {}",
                                path.display(), &tag.id
                            );
                            tag_changed = true;
                        }
                        Ok(false) => {}
                    }
                }
            } else {
                // Remove all selected paths from this tag
                for path in self.selected_paths.iter() {
                    if tag.remove_entry(path) {
                        trace!("[FileActionScreen::apply_changes()] Removed path {} from tag {}",
                            path.display(), &tag.id
                        );
                        tag_changed = true;
                    }
                }
            }

            // Save tag if there was a change
            if !tag_changed { continue; }

            if let Err(err) = tag.save() {
                let tag_id = tag.id.clone();
                commands.push(send_message!(notif = error!(
                    notify, log_context = "FileActionScreen::apply_changes()";
                    "Failed to save tag \"{}\":\n {:?}", tag_id, err
                )));
            }
        }

        Command::batch(commands)
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



/// TODO documentation
fn get_intersecting_tags(paths: &[PathBuf], tags: &[Tag]) -> Vec<TagID> {
    paths.iter()
        .map(|path| tagging::get_tags_for_path(path, tags))
        .reduce(|acc, t| &acc & &t)
        .unwrap_or_default()
        .into_iter()
        .map(|i| tags[i].id.clone())
        .collect()
}


/// TODO documentation
fn load_tags(commands: &mut Vec<Command<AppMessage>>) -> Vec<Tag> {
    let tags_cache = match tagging::get_all_tags() {
        Ok(v) => v,
        Err(err) => {
            commands.push(send_message!(notif = error!(
                notify, log_context = "file_action_screen::load_tags()";
                "Failed to load tags:\n {}", err
            )));

            Vec::new()
        }
    };

    let mut load_failed: bool = false;
    let tags_cache: Vec<Tag> = tags_cache.into_iter()
        .flat_map(|p| {
            let res = Tag::load_from_path(&p);

            if let Err(ref err) = res {
                let err = err.to_string();
                error!(notify, log_context = "file_action_screen::load_tags()";
                    "Failed to load tag at \"{}\":\n {}", p.display(), err
                );
                load_failed = true;
            }

            res
        })
        .collect();

    if load_failed {
        commands.push(send_message!(notif = error!(
            notify, log_context = "TagEditScreen::new()";
            "Failed to load tags. See logs for more details",
        )))
    }

    tags_cache
}




