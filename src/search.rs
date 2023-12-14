use std::collections::VecDeque;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use console_engine::{
    pixel, rect_style::BorderStyle, screen::Screen, Color, KeyCode, KeyEventKind,
};

use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use console_engine::events::Event;
use console_engine::forms::{Form, FormField, FormOptions, FormStyle, FormValue, Text};

use crate::config::{ColorTheme, Configs, Invert};
use crate::{themevar, util::*};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub prefix: Option<String>,
    pub path: PathBuf,
}

impl FileEntry {
    pub fn prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(prefix.to_string());
        self
    }

    pub fn prefix_idx(mut self, index: usize) -> Self {
        self.prefix = Some(format!("{index}:"));
        self
    }
}

impl AsRef<PathBuf> for FileEntry {
    fn as_ref(&self) -> &PathBuf {
        &self.path
    }
}

impl Deref for FileEntry {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl From<&Path> for FileEntry {
    fn from(value: &Path) -> Self {
        Self {
            prefix: None,
            path: value.to_path_buf(),
        }
    }
}

impl From<PathBuf> for FileEntry {
    fn from(value: PathBuf) -> Self {
        Self {
            prefix: None,
            path: value,
        }
    }
}

impl ToString for FileEntry {
    fn to_string(&self) -> String {
        if let Some(prefix) = &self.prefix {
            format!("{}{}", prefix, self.to_string_lossy())
        } else {
            format!("{}", self.to_string_lossy())
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SearchPanelState {
    Running,
    Exit(Option<PathBuf>),
}

// Used for quick-searching files, folders, and favorites
// The actual searching happens in SearchQuery
pub struct SearchPanel {
    screen: Screen,
    form: Form, // Input box
    title: String,
    color: Color,
    selected_index: usize,
    query: SearchQuery,
    pub state: SearchPanelState,
}

impl SearchPanel {
    pub fn new(width: u32, height: u32, mode: SearchQueryMode) -> Self {
        let cfg: &Configs = Configs::global();

        let max_result_count: usize = (height - 5) as usize;
        let max_stack_size: usize = cfg.max_search_stack;
        let ignore_types: String = cfg.search_ignore_types.clone();

        Self {
            screen: Screen::new(width, height),
            form: SearchPanel::build_form(width - 2, cfg.theme.bg_color.into()),
            title: "Search".to_string(),
            color: cfg.theme.file_color.into(),
            selected_index: 0,
            query: SearchQuery::new(mode, max_result_count, max_stack_size, ignore_types),
            state: SearchPanelState::Running,
        }
    }

    pub fn set_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn set_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    fn build_form(width: u32, bg_color: Color) -> Form {
        let theme = FormStyle {
            border: Some(BorderStyle::new_light().with_colors(themevar!(text_color), bg_color)),
            bg: bg_color,
            ..Default::default()
        };

        let mut form = Form::new(
            width,
            3,
            FormOptions {
                style: theme,
                ..Default::default()
            },
        );
        form.build_field::<Text>(
            "query",
            FormOptions {
                style: theme,
                ..Default::default()
            },
        );

        form.set_active(true);
        form
    }

    pub fn update(&mut self) {
        self.query.update();
        self.selected_index = self.selected_index.min(self.get_results().len().max(1) - 1);
    }

    pub fn is_running(&self) -> bool {
        self.state == SearchPanelState::Running
    }

    pub fn get_query_mode(&self) -> &SearchQueryMode {
        &self.query.mode
    }

    pub fn get_results(&self) -> &Vec<FileEntry> {
        &self.query.results
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent, y_offset: u16) {
        if let MouseEvent {
            kind: MouseEventKind::Down(_) | MouseEventKind::Drag(_),
            row,
            ..
        } = event
        {
            let offset: u16 = y_offset + 4;
            let urow: u16 = row.max(offset) - offset;
            self.selected_index = (urow as usize).min(self.get_results().len().max(1) - 1);
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }

        // No need to check for KeyEvent.kind because we only allowed KeyEventKind::Press up there
        match key_event {
            // Move cursor up
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.selected_index = self
                    .selected_index
                    .checked_sub(1)
                    .or_else(|| self.get_results().len().checked_sub(1))
                    .unwrap_or_default();
                return;
            }

            // Move cursor down
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                let len: usize = self.get_results().len();
                if len == 0 {
                    return;
                }
                self.selected_index = (self.selected_index + 1) % len;
                return;
            }

            // Esc to exit
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.state = SearchPanelState::Exit(None);
            }

            // Form input
            _ => {
                self.form.handle_event(Event::Key(key_event));

                if self.form.is_finished() {
                    let selected: &PathBuf = match self.get_results().get(self.selected_index) {
                        Some(de) => de,
                        None => return,
                    };
                    self.state = SearchPanelState::Exit(Some(selected.clone()));
                    return;
                }
            }
        }

        // Handle query
        let value: String = match self.form.get_field_output("query") {
            Some(FormValue::String(value)) => value.replace('/', r"\"),
            _ => return,
        };

        self.query.search(value);
    }

    fn display_results(&mut self) {
        if self.get_results().is_empty() {
            return;
        }

        let offset: (i32, i32) = (2, 4);
        let theme: &ColorTheme = Configs::theme();
        let bg_color = Color::from(theme.bg_color);

        // Highlight selected line
        self.screen.h_line(
            offset.0,
            self.selected_index as i32 + offset.1,
            self.screen.get_width() as i32 - 2,
            pixel::pxl_bg(' ', theme.comment_color.into()),
        );

        for (i, entry) in self.query.results.iter().enumerate() {
            if i as u32 + offset.1 as u32 >= self.screen.get_height() - 1 {
                break;
            }

            let bg: Color = if i == self.selected_index {
                theme.comment_color.into()
            } else {
                bg_color
            };

            let path: &Path = if let SearchQueryMode::Files(root_path)
            | SearchQueryMode::Folders(root_path) = &self.query.mode
            {
                entry.strip_prefix(root_path).unwrap_or(entry)
            } else {
                entry
            };

            let mut path_string: String = path2string(path).replace('\\', "/");

            if let Some(prefix) = &entry.prefix {
                path_string = format!("{prefix} {path_string}");
            }

            self.screen
                .print_fbg(offset.0, i as i32 + offset.1, &path_string, self.color, bg);
        }
    }

    // TODO optimize maybe
    pub fn draw(&mut self, tick: usize) -> &Screen {
        let theme: &ColorTheme = Configs::theme();
        let bg_color = theme.bg_color;
        let bg_inverted = theme.bg_color.inv();

        self.screen.fill(pixel::pxl_bg(' ', bg_color.into()));

        self.screen.rect_border(
            0,
            0,
            self.screen.get_width() as i32 - 1,
            self.screen.get_height() as i32 - 1,
            BorderStyle::new_light().with_colors(bg_inverted.into(), bg_color.into()),
        );

        self.screen.print_screen(1, 1, self.form.draw(tick));
        self.display_results();

        let text: String = format!(" {} ", &self.title);
        self.screen
            .print_fbg(2, 0, &text, bg_color.into(), bg_inverted.into());

        &self.screen
    }
}

#[derive(Debug, Clone)]
pub enum SearchQueryMode {
    Files(PathBuf),
    Folders(PathBuf),
    List(Vec<PathBuf>), // Title & file list
}

struct SearchQuery {
    query: String,
    mode: SearchQueryMode,
    results: Vec<FileEntry>,
    receiver: Option<Receiver<Vec<FileEntry>>>,
    max_result_count: usize,
    max_stack_size: usize,
    ignore_types: String,
}

impl SearchQuery {
    // max_result_count, max_stack_size, and ignore_types are values taked from Configs
    fn new(
        mode: SearchQueryMode,
        max_result_count: usize,
        max_stack_size: usize,
        ignore_types: String,
    ) -> Self {
        let mut q = Self {
            query: String::new(),
            mode,
            results: vec![],
            receiver: None,
            max_result_count,
            max_stack_size,
            ignore_types,
        };

        q.results = q.list();
        q
    }

    // Get a list of dirs. Mainly used when there's no query but you don't wanna leave the user with an empty screen, yknow
    fn list(&self) -> Vec<FileEntry> {
        match &self.mode {
            SearchQueryMode::List(paths) => paths
                .iter()
                .enumerate()
                .map(|(i, pathbuf)| FileEntry::from(pathbuf.as_path()).prefix_idx(i))
                .collect(),

            SearchQueryMode::Files(path) => {
                let mut results: Vec<PathBuf> = Vec::new();
                let mut stack: Vec<PathBuf> = vec![path.clone()];

                while results.len() < self.max_result_count {
                    let search_path = if let Some(p) = stack.pop() {
                        p
                    } else {
                        break;
                    };
                    let (mut files, folders) = match get_files_and_folders_at(search_path) {
                        Ok(pair) => pair,
                        Err(_) => continue,
                    };

                    results.append(&mut files);
                    stack.append(
                        &mut folders
                            .iter()
                            .take(self.max_stack_size - stack.len())
                            .cloned()
                            .collect(),
                    );
                }

                results
                    .iter()
                    .take(self.max_result_count)
                    .map(|pathbuf| FileEntry::from(pathbuf.as_path()))
                    .collect()
            }

            SearchQueryMode::Folders(path) => {
                let mut results: Vec<PathBuf> = match get_folders_at(path, self.max_result_count) {
                    Ok(v) => v,
                    Err(_) => return Vec::new(),
                };
                let mut idx: usize = 0;

                while results.len() < self.max_result_count {
                    let search_path = if let Some(path) = results.get(idx) {
                        path
                    } else {
                        break;
                    };
                    let mut folders =
                        match get_folders_at(search_path, self.max_result_count - results.len()) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                    results.append(&mut folders);
                    idx += 1;
                }

                results
                    .iter()
                    .take(self.max_result_count)
                    .map(|pathbuf| FileEntry::from(pathbuf.as_path()))
                    .collect()
            }
        }
    }

    fn update(&mut self) {
        let rx = match &mut self.receiver {
            Some(rx) => rx,
            None => return,
        };

        let max_search_results: usize = self.max_result_count;

        for received in rx.try_iter() {
            for entry in received
                .iter()
                .take(max_search_results - self.results.len())
            {
                self.results.push(entry.clone());
            }
        }

        if self.results.len() >= max_search_results {
            self.receiver = None;
        }
    }

    fn search(&mut self, query: String) {
        if query == self.query {
            return;
        }
        self.query = query;

        if self.query.is_empty() {
            self.results = self.list();
            return;
        }

        self.results.clear();
        let search_query = self.query.to_lowercase();

        // Files and Folders are done on threads. List isn't.
        // We use mpsc to send results back for this specific query
        // A new set of sender and receiver is created for each query, replacing the old self.reveiver
        // If that happens while the thread is running, stop searching (obviously)

        // Man, that's a disgusting amount of indentation lol
        match &self.mode {
            SearchQueryMode::List(paths) => {
                self.receiver = None;
                self.results = paths
                    .iter()
                    .enumerate()
                    .map(|(i, pathbuf)| FileEntry::from(pathbuf.as_path()).prefix_idx(i))
                    .filter(|entry| entry.to_string().to_lowercase().contains(&search_query))
                    .collect();
            }

            SearchQueryMode::Files(path) => {
                let (tx, rx) = mpsc::channel::<Vec<FileEntry>>();
                self.receiver = Some(rx);
                let path = path.clone();
                let max_stack_size = self.max_stack_size;
                let ignore_types: String = self.ignore_types.clone();

                thread::spawn(move || {
                    let mut stack: VecDeque<PathBuf> = VecDeque::from([path]);

                    while let Some(search_path) = stack.pop_front() {
                        let (files, folders) = match get_files_and_folders_at(search_path) {
                            Ok(pair) => pair,
                            Err(_) => continue,
                        };
                        stack.append(
                            &mut folders
                                .into_iter()
                                // Don't search inside folders that start with "." (like .git/)
                                .filter(|pathbuf| {
                                    !path2string(pathbuf.file_name().unwrap_or_default())
                                        .starts_with('.')
                                })
                                .take(max_stack_size - stack.len())
                                .collect(),
                        );

                        let files: Vec<FileEntry> = files
                            .into_iter()
                            .filter(|pathbuf| {
                                !ignore_types
                                    .contains(&path2string(pathbuf.extension().unwrap_or_default()))
                                    && path2string(pathbuf).to_lowercase().contains(&search_query)
                            })
                            .map(FileEntry::from)
                            .collect();

                        // If receiver is gone (new query or panel is closed)
                        if tx.send(files).is_err() {
                            break;
                        }
                    }
                });
            }

            SearchQueryMode::Folders(path) => {
                let (tx, rx) = mpsc::channel::<Vec<FileEntry>>();
                self.receiver = Some(rx);
                let path = path.clone();
                let max_stack_size = self.max_stack_size;

                thread::spawn(move || {
                    let mut stack: VecDeque<PathBuf> = VecDeque::from([path]);

                    while let Some(search_path) = stack.pop_front() {
                        let folders: Vec<PathBuf> = match get_all_folders_at(search_path) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        stack.append(
                            &mut folders
                                .iter()
                                // Don't search inside folders that start with "." (like .git/)
                                .filter(|pathbuf| {
                                    !path2string(pathbuf.file_name().unwrap_or_default())
                                        .starts_with('.')
                                })
                                .take(max_stack_size - stack.len())
                                .cloned()
                                .collect(),
                        );

                        let folders: Vec<FileEntry> = folders
                            .into_iter()
                            .filter(|pathbuf| {
                                path2string(pathbuf).to_lowercase().contains(&search_query)
                            })
                            .map(FileEntry::from)
                            .collect();

                        // If receiver is gone (new query or panel is closed)
                        if tx.send(folders).is_err() {
                            break;
                            // Behold, the avalanche of closing brackets:
                        }
                    }
                });
            }
        }
    }
}
// Beautiful
