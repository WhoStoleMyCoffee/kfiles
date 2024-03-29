use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use std::ops::Deref;

use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use console_engine::screen::Screen;
use console_engine::{pixel, Color, ConsoleEngine, KeyCode, KeyEventKind, KeyModifiers};

use crate::config::{ColorTheme, Configs};
use crate::themevar;
use crate::util::{self, TruncateBack, Invert, file_name, str_match_cost};
use crate::{AppError, CONTROL_SHIFT};


/// Syntax:
/// ```rust
/// let_err!( some_result => self );
/// let_err!( some_result => self, "context" );
/// let_err!( some_result => self; else { ... } );
/// let_err!( some_result => self, "context"; else { ... } );
/// ```
#[macro_export]
macro_rules! try_err {
    ($e:expr => $fb:expr) => {
        if let Err(err) = $e {
            $fb.status_line.error(err, None);
        }
    };

    ($e:expr => $fb:expr, $ctx:expr) => {
        if let Err(err) = $e {
            $fb.status_line.error(err, Some($ctx));
        }
    };

    ($e:expr => $fb:expr; else $b:block) => {
        if let Err(err) = $e {
            $fb.status_line.error(err, None);
        } else $b
    };

    ($e:expr => $fb:expr, $ctx:expr; else $b:block) => {
        if let Err(err) = $e {
            $fb.status_line.error(err, Some($ctx));
        } else $b
    };
}

pub struct FileBuffer {
    pub path: PathBuf,
    screen: Screen,
    selected_index: usize,
    scroll: usize,
    pub entries: Vec<PathBuf>, // List of folders & files
    pub status_line: StatusLine,
}

impl FileBuffer {
    pub fn new(path: &Path, screen: Screen) -> Self {
        FileBuffer {
            path: PathBuf::from(path),
            screen,
            selected_index: 0,
            scroll: 0,
            entries: vec![],
            status_line: StatusLine {
                text: util::path2string(path),
                color: themevar!(text_color),
                state: StatusLineState::Normal,
            },
        }
    }

    pub fn calc_size_from_engine(engine: &ConsoleEngine) -> (u32, u32) {
        (engine.get_width() - 2, engine.get_height() - 2)
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        self.screen.resize(new_width, new_height);
    }

    pub fn get_state(&self) -> &StatusLineState {
        &self.status_line.state
    }

    pub fn get_state_mut(&mut self) -> &mut StatusLineState {
        &mut self.status_line.state
    }

    // Load files
    pub fn load_entries(&mut self) -> Result<(), AppError> {
        self.selected_index = 0;
        self.scroll = 0;

        match util::get_at_sorted(&self.path) {
            Ok(entries) => {
                self.entries = entries;
                Ok(())
            }

            Err(err) => {
                self.entries = Vec::new();
                Err(err.into())
            }
        }
    }

    pub fn display_path(&mut self) {
        self.status_line.set_text(&self.path.display()
                .to_string()
                .trunc_back(self.screen.get_width() as usize))
            .set_color(themevar!(text_color));
    }

    /// Sets the path
    /// If `path` is a file, set the path to the file's directory and automatically select it
    pub fn set_path(&mut self, path: &Path) {
        if !path.exists() {
            self.status_line.error( format!("\"{}\" does not exist", path.display()).into(), None);
            return;
        }

        if path.is_dir() {
            self.path = path.to_path_buf();
            try_err!(self.load_entries() => self; else {
                self.status_line.normal();
                self.display_path();
            });

        } else if path.is_file() {
            let Some(file_name) = path.file_name() else {
                self.status_line.error("Could not get valid file name".into(), Some("Error opening at file:\n "));
                return;
            };
            let Some(parent) = path.parent() else {
                self.status_line.error("Could not get parent directory".into(), Some("Error opening at file:\n "));
                return;
            };

            self.path = parent.to_path_buf();

            try_err!(self.load_entries() => self; else {
                self.status_line.normal();
                self.display_path();
                self.select(file_name);
            });
        }

    }

    pub fn select(&mut self, file_name: &OsStr) {
        if let Some(idx) = self.entries.iter()
            .position(|path| path.file_name() == Some(file_name))
        {
            self.selected_index = idx;
            self.update_scroll();
        }
    }

    /// Does a simple case-insensitive search over the entries
    pub fn search_pattern(&self, pattern: &str) -> Option<usize> {
        if pattern.is_empty() {
            return None;
        }

        let bup: Option<(usize, usize)> = self.entries.iter().enumerate()
            // (index, pathbuf) -> (index, cost)
            .filter_map(|(i, pathbuf)| {
                let file_name = file_name(pathbuf);
                str_match_cost(pattern, &file_name) .map(|cost| (i, cost))
            })
            .min_by_key(|(_i, cost)| *cost);
        bup.map(|(i, _cost)| i)
    }

    pub fn get_selected_path(&self) -> Option<&PathBuf> {
        self.entries.get(self.selected_index)
    }

    /// Returns whether it was a valid directory
    fn open_selected(&mut self) -> Result<bool, AppError> {
        let Some(pathbuf) = self.entries.get(self.selected_index) else {
            return Err("No directory selected".into());
        };

        // Open file
        if pathbuf.is_file() {
            if opener::open(pathbuf).is_ok() {
                return Ok(false);
            }
            opener::reveal(pathbuf)
                .map_err(|source| AppError::OpenError { source, path: pathbuf.clone() })?;

            return Err("Could not open file. Revealing in file explorer instead".into());
        } else if !pathbuf.is_dir() {
            return Ok(false);
        }

        // Open dir
        self.path.push(pathbuf.file_name().unwrap_or_default());
        self.load_entries().map(|_| Ok(true))?
    }

    pub fn reveal(&self) -> Result<(), opener::OpenError> {
        if let Some(pathbuf) = self.get_selected_path() {
            opener::reveal(pathbuf)
        } else {
            opener::open(&self.path)
        }
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent) {
        if self.entries.is_empty() {
            return;
        }
        match event {
            MouseEvent {
                kind: MouseEventKind::Down(_) | MouseEventKind::Drag(_),
                row,
                ..
            } => {
                let urow: usize = row.max(1) as usize - 1;
                self.selected_index = (urow + self.scroll).clamp(0, self.entries.len() - 1);
                self.update_scroll();
            }

            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                ..
            } => {
                self.scroll = self.scroll.saturating_sub(1);
            }

            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                ..
            } => {
                self.scroll = (self.scroll + 1).min(self.entries.len() - 1);
            }

            _ => {}
        }
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) {
        let StatusLineState::Prompt { prompt_line, mode } = &mut self.status_line.state else {
            self.handle_normal_mode_event(event);
            return;
        };

        match prompt_line.handle_key_event(event) {
            Some(PromptEvent::Cancel(_)) => {
                self.status_line.normal();
                self.display_path();
            }

            Some(PromptEvent::Enter(text)) => match mode {
                PromptMode::QuickSearch => {
                    prompt_line.clear();
                    match self.open_selected() {
                        Ok(true) => {}
                        Ok(false) => {
                            self.status_line.normal();
                            self.display_path();
                        }
                        Err(err) => {
                            self.status_line.error(err, None);
                        }
                    }
                }

                PromptMode::CreateDir => {
                    let dir_name: String = text.clone();
                    if dir_name.is_empty() {
                        self.status_line.error("Input field empty".into(), None);
                        return;
                    }

                    let full_path: PathBuf = self.path.join(&dir_name);
                    if full_path.exists() {
                        self.status_line.error("Folder already exists".into(), None);
                        return;
                    }

                    if let Err(err) = fs::create_dir_all(&full_path) {
                        self.status_line
                            .error(err.into(), Some("Failed to create folder \n"));
                        return;
                    }
                    try_err!(self.load_entries() => self; else {
                        self.status_line.normal()
                            .set_text( &format!("Created folder \"{}\"", &dir_name) )
                            .set_color( themevar!(special_color) );
                        self.select( &OsString::from(&dir_name) );
                    });
                }

                PromptMode::CreateFile => {
                    let file_name: String = text.clone();
                    if file_name.is_empty() {
                        self.status_line.error("Input field empty".into(), None);
                        return;
                    }

                    let full_path: PathBuf = self.path.join(&file_name);
                    if full_path.exists() {
                        self.status_line.error("File already exists".into(), None);
                        return;
                    }

                    if let Err(err) = fs::File::create(&full_path) {
                        self.status_line
                            .error(err.into(), Some("Failed to create file \n"));
                        return;
                    }
                    try_err!(self.load_entries() => self; else {
                        self.status_line.normal()
                            .set_text( &format!("Created file \"{}\"", &file_name) )
                            .set_color( themevar!(special_color) );
                        self.select( &OsString::from(&file_name) );
                    });
                }

                PromptMode::Delete(pathbuf) => {
                    if !text.eq_ignore_ascii_case("y") {
                        self.status_line.normal();
                        self.display_path();
                        return;
                    }

                    let selected_idx: usize = self.selected_index;
                    let res = if pathbuf.is_dir() {
                        fs::remove_dir_all(&pathbuf)
                    } else {
                        fs::remove_file(&pathbuf)
                    };

                    if let Err(err) = res {
                        self.status_line
                            .error(err.into(), Some("Failed to delete\n "));
                        return;
                    } else {
                        let text: String =
                            format!("Successfully deleted \"{}\"", util::file_name(pathbuf));
                        self.status_line
                            .set_text(&text)
                            .set_color(themevar!(special_color));
                    }

                    try_err!(self.load_entries() => self; else {
                        self.status_line.normal();
                        self.selected_index = selected_idx.clamp(0, self.entries.len());
                        self.update_scroll();
                    });
                }

                PromptMode::Rename(pathbuf) => {
                    let new_path: PathBuf = self.path.join(&text);

                    if let Err(err) = fs::rename(pathbuf, new_path) {
                        self.status_line
                            .error(err.into(), Some("Failed to rename\n "));
                        return;
                    } else {
                        self.status_line
                            .set_text(&format!("Successfully renamed to \"{}\"", &text))
                            .set_color(themevar!(special_color));
                    }

                    try_err!(self.load_entries() => self; else {
                        self.status_line.normal();
                        self.select( &OsString::from(&text) );
                    });
                }
            },

            Some(PromptEvent::Input(text)) => {
                match mode {
                    PromptMode::QuickSearch => (),
                    _ => return,
                }

                if let Some(index) = self.search_pattern(&text) {
                    self.selected_index = index;
                    self.update_scroll();
                }
            }

            None => {}
        }
    }

    fn handle_normal_mode_event(&mut self, event: KeyEvent) {
        if event.kind != KeyEventKind::Press {
            return;
        }

        // Normal mode
        match event {
            // Move cursor up
            KeyEvent {
                code: KeyCode::Char('k') | KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let len: usize = self.entries.len();
                if len == 0 {
                    return;
                }
                self.selected_index = self.selected_index.checked_sub(1).unwrap_or(len - 1);
                self.update_scroll();
            }

            // Move cursor down
            KeyEvent {
                code: KeyCode::Char('j') | KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let len: usize = self.entries.len();
                if len == 0 {
                    return;
                }
                self.selected_index = (self.selected_index + 1) % len;
                self.update_scroll();
            }

            // Move down half a page
            KeyEvent {
                code: KeyCode::Char('d') | KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let len: usize = self.entries.len();
                if len == 0 { return; }

                let half_height: usize = self.screen.get_height() as usize / 2;
                self.selected_index = (self.selected_index + half_height)
                    .clamp(0, len-1);
                self.update_scroll();
            }

            // Move up half a page
            KeyEvent {
                code: KeyCode::Char('u') | KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let len: usize = self.entries.len();
                if len == 0 { return; }

                let half_height: usize = self.screen.get_height() as usize / 2;
                self.selected_index = self.selected_index.clamp(half_height, len-1) - half_height;
                self.update_scroll();
            }

            // Open
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                try_err!(self.open_selected() => self; else {
                    self.status_line.normal();
                    self.display_path();
                });
            }

            // Go back
            KeyEvent {
                code: KeyCode::Char('-') | KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } | KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::ALT,
                ..
            }=> {
                let folder_name: Option<OsString> = self.path.file_name().map(|s| s.to_os_string());
                let went_back: bool = self.path.pop();
                self.display_path();
                if !went_back {
                    return;
                }

                if let Err(err) = self.load_entries() {
                    self.status_line.error(err, None);
                } else {
                    self.status_line.normal();
                }

                if let Some(folder_name) = folder_name {
                    self.select(&folder_name);
                }
            }

            // Jump to start
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let len: usize = self.entries.len();
                if len == 0 {
                    return;
                }
                self.selected_index = 0;
                self.update_scroll();
            }

            // Jump to end
            KeyEvent {
                code: KeyCode::Char('G'),
                ..
            } => {
                let len: usize = self.entries.len();
                if len == 0 {
                    return;
                }
                self.selected_index = len - 1;
                self.update_scroll();
            }

            // Start quick search
            KeyEvent {
                code: KeyCode::Char('/') | KeyCode::Char(';'),
                ..
            } => {
                if self.entries.is_empty() {
                    return;
                }

                self.status_line
                    .set_text("Searching for: ")
                    .set_color(themevar!(special_color))
                    .prompt(PromptLine::default(), PromptMode::QuickSearch);
            }

            // Create folder with Ctrl + Shift + n
            KeyEvent {
                code: KeyCode::Char('N'),
                modifiers,
                ..
            } if modifiers.bits() == CONTROL_SHIFT => {
                self.status_line
                    .set_text("Create folder: ")
                    .set_color(themevar!(special_color))
                    .prompt(PromptLine::default(), PromptMode::CreateDir);
            }

            // Create file with Ctrl + n
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.status_line
                    .set_text("Create file: ")
                    .set_color(themevar!(special_color))
                    .prompt(PromptLine::default(), PromptMode::CreateFile);
            }

            // Delete with Ctrl + d
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                let file_name: String = match self.get_selected_path() {
                    Some(p) => util::file_name(p),
                    None => return,
                };

                let Some(selected_path) = self.get_selected_path().cloned() else {
                    self.status_line.error("No directory selected".into(), None);
                    return;
                };
                self.status_line
                    .set_text(format!("Delete {}? (y/n): ", &file_name).as_str())
                    .set_color(themevar!(error_color))
                    .prompt(PromptLine::default(), PromptMode::Delete(selected_path));
            }

            // Rename with Ctrl + r
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                let file_name: String = match self.get_selected_path() {
                    Some(p) => util::file_name(p),
                    None => return,
                };

                let Some(selected_path) = self.get_selected_path().cloned() else {
                    self.status_line.error("No directory selected".into(), None);
                    return;
                };

                let i: usize = file_name.len()
                    - file_name
                        .chars()
                        .rev()
                        .position(|ch| ch == '.')
                        .map_or(0, |i| i + 1);
                self.status_line
                    .set_text(format!("Rename \"{}\" to: ", &file_name).as_str())
                    .set_color(themevar!(special_color))
                    .prompt(
                        PromptLine {
                            input: file_name,
                            cursor_pos: i,
                        },
                        PromptMode::Rename(selected_path),
                    );
            }

            _ => {}
        }
    }

    pub fn update_scroll(&mut self) {
        let u_scroll_margin: usize = Configs::global().scroll_margin as usize;
        let max_bound: usize = self.screen.get_height() as usize - u_scroll_margin;

        if self.selected_index < self.scroll + u_scroll_margin {
            self.scroll = self.selected_index.max(u_scroll_margin) - u_scroll_margin;
        } else if self.selected_index > self.scroll + max_bound {
            self.scroll = (self.selected_index - max_bound)
                .clamp(u_scroll_margin, self.entries.len() - u_scroll_margin);
        }
    }

    pub fn draw(&mut self) -> &Screen {
        let theme: &ColorTheme = Configs::theme();
        let bg_color: Color = theme.bg_color.into();
        self.screen.fill(pixel::pxl_bg(' ', bg_color));

        // Display empty
        if self.entries.is_empty() {
            self.screen
                .print_fbg(0, 0, "(Empty)", themevar!(comment_color), bg_color);
            return &self.screen;
        }

        let screen_selected_idx: isize = self.selected_index as isize - self.scroll as isize;
        let folder_color: Color = theme.folder_color.into();
        let file_color: Color = theme.file_color.into();

        // Highlight line
        self.screen.h_line(
            0,
            screen_selected_idx as i32,
            self.screen.get_width() as i32,
            pixel::pxl_bg(' ', theme.comment_color.into()),
        );

        // Display entries
        for (i, pathbuf) in self.entries.iter().skip(self.scroll).enumerate() {
            if i as u32 >= self.screen.get_height() {
                break;
            }
            let mut file_name: String = util::file_name(pathbuf);

            let bg: Color = if i == screen_selected_idx as usize {
                theme.comment_color.into()
            } else {
                bg_color
            };
            let fg: Color = if pathbuf.is_dir() {
                file_name.push('/');
                folder_color
            } else {
                file_color
            };

            self.screen.print_fbg(0, i as i32, &file_name, fg, bg);
        }
        &self.screen
    }
}

#[derive(Debug)]
pub enum PromptMode {
    QuickSearch,
    CreateDir,
    CreateFile,
    Delete(PathBuf),
    Rename(PathBuf),
}

#[derive(Debug)]
pub enum StatusLineState {
    Normal,
    Error(AppError),
    Prompt {
        prompt_line: PromptLine,
        mode: PromptMode,
    },
}

pub struct StatusLine {
    pub text: String,
    pub color: Color,
    pub state: StatusLineState,
}

impl StatusLine {
    pub fn set_text(&mut self, text: &str) -> &mut Self {
        self.text = text.to_string();
        self
    }

    pub fn set_color(&mut self, color: Color) -> &mut Self {
        self.color = color;
        self
    }

    pub fn normal(&mut self) -> &mut Self {
        self.state = StatusLineState::Normal;
        self
    }

    pub fn error(&mut self, err: AppError, prefix: Option<&str>) -> &mut Self {
        self.state = StatusLineState::Error(err);
        self.color = themevar!(error_color);
        self.text = prefix.unwrap_or("Error: \n ").to_string();
        self
    }

    pub fn prompt(&mut self, prompt_line: PromptLine, prompt_mode: PromptMode) -> &mut Self {
        self.state = StatusLineState::Prompt {
            prompt_line,
            mode: prompt_mode,
        };
        self
    }

    pub fn draw(&self, engine: &mut ConsoleEngine) {
        let text: String = self.to_string();

        let prompt_line: &PromptLine = match &self.state {
            StatusLineState::Prompt { prompt_line, .. } => prompt_line,
            
            // Draw text normally
            _ => {
                let h = engine.get_height() as i32 - text.lines().count() as i32;
                let bg_color = themevar!(bg_color);

                // Background
                engine.fill_rect(0, h,
                    engine.get_width() as i32,
                    engine.get_height() as i32,
                    pixel::pxl_bg(' ', bg_color),
                );

                // Text
                engine.print_fbg(0, h,
                    &text,
                    self.color,
                    bg_color,
                );
                return;
            }
        };

        let width: usize = engine.get_width() as usize;

        // Draw prompt line
        engine.print_fbg(
            0,
            engine.get_height() as i32 - 1,
            &text.trunc_back(width),
            self.color,
            themevar!(bg_color),
        );

        // Draw caret
        let i: i32 = prompt_line.cursor_pos as i32;
        let ch: &str = prompt_line.get(i as usize..i as usize + 1).unwrap_or(" ");
        let text_col = Configs::theme().text_color;
        engine.print_fbg(
            i + self.text.len() as i32,
            engine.get_height() as i32 - 1,
            ch,
            text_col.inv().into(),
            text_col.into(),
        )
    }
}

impl ToString for StatusLine {
    fn to_string(&self) -> String {
        match &self.state {
            StatusLineState::Normal => self.text.clone(),
            StatusLineState::Error(err) => format!("{}{}", &self.text, err),
            StatusLineState::Prompt { prompt_line, .. } => {
                format!("{}{}", &self.text, prompt_line.as_ref())
            }
        }
    }
}

pub enum PromptEvent {
    Input(String),
    Enter(String),
    Cancel(String),
}

#[derive(Debug, Default)]
pub struct PromptLine {
    pub input: String,
    pub cursor_pos: usize,
}

impl PromptLine {
    pub fn handle_key_event(&mut self, event: KeyEvent) -> Option<PromptEvent> {
        if event.kind != KeyEventKind::Press {
            return None;
        }

        match event.code {
            KeyCode::Esc => return Some(PromptEvent::Cancel(self.input.clone())),
            KeyCode::Enter => return Some(PromptEvent::Enter(self.input.clone())),
            KeyCode::Backspace => {
                if event.modifiers == KeyModifiers::CONTROL {
                    self.input.clear();
                    self.cursor_pos = 0;
                } else {
                    self.remove_char(1);
                }
                return Some(PromptEvent::Input(self.input.clone()));
            }
            KeyCode::Delete => {
                self.remove_char(-1);
                return Some(PromptEvent::Input(self.input.clone()));
            }

            KeyCode::Left => self.move_cursor(-1),
            KeyCode::Right => self.move_cursor(1),
            KeyCode::Home => self.move_cursor(i32::MIN),
            KeyCode::End => self.move_cursor(i32::MAX),

            // Write char
            KeyCode::Char(ch) => {
                self.put_char(ch);
                return Some(PromptEvent::Input(self.input.clone()));
            }

            _ => (),
        }

        None
    }

    /// Insert a character at the position of the cursor
    pub fn put_char(&mut self, chr: char) {
        let mut new_buffer = String::with_capacity(self.input.capacity() + 1);
        new_buffer.extend(
            self.input
                .chars()
                .take(self.cursor_pos)
                .chain(std::iter::once(chr))
                .chain(self.input.chars().skip(self.cursor_pos)),
        );
        self.input = new_buffer;
        self.move_cursor(1);
    }

    /// Removes a certain amount of characters either on the left (positive) or right (negative) side of the cursor
    pub fn remove_char(&mut self, amount: i32) {
        if amount == 0 {
            return;
        }

        let off_l = amount.max(0) as usize; // offset to the left from cursor, `positive` or 0
        let off_r = amount.min(0).unsigned_abs() as usize; // offset to the right from cursor,  `-negative` or 0
        let pos_l = self.cursor_pos.saturating_sub(off_l);
        let pos_r = self.cursor_pos.saturating_add(off_r).min(self.input.len());
        self.input = self.input.chars().take(pos_l).collect::<String>()
            + &self.input.chars().skip(pos_r).collect::<String>(); // this skips the cursor +/- offsets
        self.move_cursor(-amount.max(0));
    }

    pub fn move_cursor(&mut self, amt: i32) {
        self.cursor_pos =
            (self.cursor_pos as i64 + amt as i64).clamp(0, self.input.len() as i64) as usize;
    }

    pub fn set_cursor_pos(&mut self, p: usize) {
        self.cursor_pos = p.clamp(0, self.input.len());
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.move_cursor(i32::MIN);
    }
}

impl AsRef<String> for PromptLine {
    fn as_ref(&self) -> &String {
        &self.input
    }
}

impl Deref for PromptLine {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}
