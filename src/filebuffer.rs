use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{ PathBuf, Path };
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::Deref;

use console_engine::screen::Screen;
use console_engine::{
	pixel, Color, KeyCode,
	KeyEventKind, KeyModifiers, ConsoleEngine
};
use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};

use crate::app::AppError;
use crate::config::Configs;
use crate::{util, CONTROL_SHIFT};



pub struct FileBuffer {
	pub path: PathBuf,
	screen: Screen,
	selected_index: usize,
	scroll: usize,
	cfg: Rc<RefCell<Configs>>,
	pub entries: Vec<PathBuf>, // List of folders & files
	pub status_line: StatusLine,
}

impl FileBuffer {
	pub fn new(path: &PathBuf, screen: Screen, cfg: Rc<RefCell<Configs>>) -> Self {
		FileBuffer {
			path: path.clone(),
			screen,
			selected_index: 0,
			scroll: 0,
			cfg,
			entries: vec![],
			status_line: StatusLine {
                text: util::path2string(path),
                color: Color::White,
                state: StatusLineState::Normal,
            },
		}
	}

    pub fn set_state(&mut self, state: StatusLineState) {
        self.status_line.set_state(state);
    }

    pub fn get_state(&self) -> &StatusLineState {
        &self.status_line.state
    }

    pub fn get_state_mut(&mut self) -> &mut StatusLineState {
        &mut self.status_line.state
    }

	// Load files
	pub fn load_entries(&mut self) {
		self.entries = match util::get_at_sorted(&self.path) {
			Ok(v) => { v },
			Err(err) => {
                self.status_line.set_error(err.into(), None);
				Vec::new()
			},
		};
		self.selected_index = 0;
		self.scroll = 0;
	}

	pub fn display_path(&mut self) {
        self.status_line.set_text( &self.path.display().to_string() )
            .set_color(Color::White);
	}

	/// Sets the path
	/// Don't confuse this with open_selected() lol
	pub fn open_dir(&mut self, path: &Path) {
		self.path = PathBuf::from(path);
		self.load_entries();
		self.set_state(StatusLineState::Normal);
		self.display_path();
	}

	pub fn select(&mut self, file_name: &OsStr) {
		if let Some(idx) = self.entries.iter() .position(|path| path.file_name() == Some(file_name)) {
			self.selected_index = idx;
			self.update_scroll();
		}
	}

    /// Does a simple case-insensitive search over the entries
    /// TODO clean up
    pub fn search_pattern(&self, pattern: &str) -> Option<usize> {
        if pattern.is_empty() { return None; }
        let pattern_lowercase: String = pattern.to_lowercase();

        self.entries.iter()
            .position(|pathbuf: &PathBuf| 
                pathbuf.file_name()
                .and_then(|osstr| osstr.to_str()) .unwrap_or_default()
                .to_ascii_lowercase()
                .starts_with(&pattern_lowercase)
            )
    }

	pub fn get_selected_path(&self) -> Option<&PathBuf> {
		self.entries.get(self.selected_index)
	}

    /// Returns whether it was a valid directory
	fn open_selected(&mut self) -> Result<bool, AppError> {
		let pathbuf: &PathBuf = match self.entries.get(self.selected_index) {
			Some(p) => p,
			None => return Err("No directory selected".into()),
		};

		if pathbuf.is_file() {
			if opener::open(pathbuf).is_ok() {
                return Ok(false);
			}
            if let Err(err) = opener::reveal(pathbuf) {
                return Err(err.into());
            }
            return Err("Could not open file. Revealing in file explorer instead".into());
		} else if pathbuf.is_dir() {
			self.path.push( pathbuf.file_name().unwrap_or_default() );
			self.load_entries();
            return Ok(true);
		}
        Ok(false)
	}

	pub fn reveal(&self) -> Result<(), opener::OpenError> {
		if let Some(pathbuf) = self.get_selected_path() {
			opener::reveal(pathbuf)
		} else {
			opener::open(&self.path)
		}
	}

	pub fn handle_mouse_event(&mut self, event: MouseEvent) {
        if self.entries.is_empty() { return; }
		match event {
			MouseEvent {
				kind: MouseEventKind::Down(_) | MouseEventKind::Drag(_),
				row,
				..
			} => {
				let urow: usize = row.max(1) as usize - 1;
				self.selected_index = (urow + self.scroll).clamp(0, self.entries.len() - 1);
				self.update_scroll();
			},

			MouseEvent { kind: MouseEventKind::ScrollUp, .. } => {
				self.scroll = self.scroll.saturating_sub(1);
			},

			MouseEvent { kind: MouseEventKind::ScrollDown, .. } => {
				self.scroll = (self.scroll + 1).min( self.entries.len() - 1 );
			},

			_ => {},
		}
	}

	pub fn handle_key_event(&mut self, event: KeyEvent) {
       let StatusLineState::Prompt { prompt_line, mode } = &mut self.status_line.state else {
           self.handle_normal_mode_event(event);
           return;
       };

        match prompt_line.handle_key_event(event) {
            Some(PromptEvent::Cancel(_)) => {
                self.set_state(StatusLineState::Normal);
                self.display_path();
            },

            Some(PromptEvent::Enter(text)) => {
                match mode {
                    PromptMode::QuickSearch => {
                        prompt_line.clear();
                        match self.open_selected() {
                            Ok(true) => {},
                            Ok(false) => {
                                self.status_line.set_state(StatusLineState::Normal);
                                self.display_path();
                            },
                            Err(err) => {
                                self.status_line.set_error(err, None);
                            },
                        }
                    },

                    PromptMode::CreateDir => {
                        let dir_name: String = text.clone();
                        self.status_line.set_state(StatusLineState::Normal);

                        if let Err(err) = fs::create_dir_all( self.path.join(&dir_name) ) {
                            self.status_line.set_error(err.into(), Some("Failed to create folder: "));
                        } else {
                            self.status_line.set_text( &format!("Created folder \"{}\"", &dir_name) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.load_entries();
                        self.select( &OsString::from(&dir_name) );
                    },

                    PromptMode::CreateFile => {
                        let file_name: String = text.clone();
                        self.status_line.set_state(StatusLineState::Normal);

                        if let Err(err) = fs::File::create( self.path.join(&file_name) ) {
                            self.status_line.set_error(err.into(), Some("Failed to create file: "));
                        } else {
                            self.status_line.set_text( &format!("Created file \"{}\"", &file_name) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.load_entries();
                        self.select( &OsString::from(&file_name) );
                    },

                    PromptMode::Delete(pathbuf) => {
                        if text.to_lowercase() != "y" {
                            self.status_line.set_state(StatusLineState::Normal);
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
                            self.status_line.set_error(err.into(), Some("Failed to delete: "));
                        } else {
                            let text: String = format!("Successfully deleted \"{}\"", util::file_name(pathbuf) );
                            self.status_line.set_text( &text )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.set_state(StatusLineState::Normal);
                        self.load_entries();
                        self.selected_index = selected_idx.clamp(0, self.entries.len());
                    },

                    PromptMode::Rename(pathbuf) => {
                        let new_path: PathBuf = self.path.join(&text);

                        if let Err(err) = fs::rename(pathbuf, new_path) {
                            self.status_line.set_error(err.into(), Some("Failed to rename: "));
                        } else {
                            self.status_line.set_text( &format!("Successfully renamed to \"{}\"", &text) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.set_state(StatusLineState::Normal);
                        self.load_entries();
                        self.select( &OsString::from(&text) );
                    },
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
            },

            None => {},
        }
    }

    fn handle_normal_mode_event(&mut self, event: KeyEvent) {
        if event.kind != KeyEventKind::Press { return; }

		// Normal mode
		match event {
			// Move cursor up
			KeyEvent { code: KeyCode::Char('k') | KeyCode::Up, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = self.selected_index.checked_sub(1) .unwrap_or(len - 1);
				self.update_scroll();
			},

			// Move cursor down
			KeyEvent { code: KeyCode::Char('j') | KeyCode::Down, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = (self.selected_index + 1) % len;
				self.update_scroll();
			},

			// Open
			KeyEvent { code: KeyCode::Enter, .. } => {
				self.set_state(StatusLineState::Normal);
				if let Err(err) = self.open_selected() {
                    self.status_line.set_error(err, None);
                } else {
                    self.display_path();
                }
			},

			// Go back
			KeyEvent { code: KeyCode::Char('-') | KeyCode::Backspace, .. } => {
				let folder_name: Option<OsString> = self.path.file_name() .map(|s| s.to_os_string());
				let went_back: bool = self.path.pop();
				self.display_path();
				if went_back {
					self.set_state(StatusLineState::Normal);
					self.load_entries();

					if let Some(folder_name) = folder_name {
						self.select(&folder_name);
					}
				}
			},

			// Jump to start
			KeyEvent { code: KeyCode::Char('g'), .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = 0;
				self.update_scroll();
			}

			// Jump to end
			KeyEvent { code: KeyCode::Char('G'), .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = len - 1;
				self.update_scroll();
			}

			// Start quick search
			KeyEvent { code: KeyCode::Char('/') | KeyCode::Char(';'), .. } => {
				if self.entries.is_empty() { return; }

                self.status_line.set_text("Searching for: ")
                    .set_color_as(self.cfg.borrow().special_color)
                    .set_prompt(PromptLine::default(), PromptMode::QuickSearch);
			},

			// Create folder with Ctrl + Shift + n
			KeyEvent { code: KeyCode::Char('N'), modifiers, .. }
            if modifiers.bits() == CONTROL_SHIFT => {
                self.status_line.set_text("Create folder: ")
                    .set_color_as(self.cfg.borrow().special_color)
                    .set_prompt(PromptLine::default(), PromptMode::CreateDir);
			}

			// Create file with Ctrl + n
			KeyEvent { code: KeyCode::Char('n'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.status_line.set_text("Create file: ")
                    .set_color_as(self.cfg.borrow().special_color)
                    .set_prompt(PromptLine::default(), PromptMode::CreateFile);
			}

			// Delete with Ctrl + d
			KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::CONTROL, .. } => {
                let file_name: String = match self.get_selected_path() {
                    Some(p) => util::file_name(p),
                    None => return,
                };

                let Some(selected_path) = self.get_selected_path().cloned() else {
                    self.status_line.set_error("No directory selected".into(), None);
                    return;
                };
                self.status_line.set_text( format!("Delete {}? (y/n): ", &file_name).as_str() )
                    .set_color(Color::Red)
                    .set_prompt( PromptLine::default(), PromptMode::Delete(selected_path) );
			},

			// Rename with Ctrl + r
			KeyEvent { code: KeyCode::Char('r'), modifiers: KeyModifiers::CONTROL, .. } => {
                let file_name: String = match self.get_selected_path() {
                    Some(p) => util::file_name(p),
                    None => return,
                };

                let Some(selected_path) = self.get_selected_path().cloned() else {
                    self.status_line.set_error("No directory selected".into(), None);
                    return;
                };

                let i: usize = file_name.len() - file_name.chars().rev()
                    .position(|ch| ch == '.')
                    .map_or(0, |i| i + 1);
                self.status_line.set_text( format!("Rename \"{}\" to: ", &file_name).as_str() )
                    .set_color_as(self.cfg.borrow().special_color)
                    .set_prompt(
                        PromptLine {
                            input: file_name,
                            cursor_pos: i,
                        },
                        PromptMode::Rename(selected_path)
                        );
			}

			_ => {},
		}

	}

	fn update_scroll(&mut self) {
		let u_scroll_margin: usize = self.cfg.borrow().scroll_margin as usize;
		let max_bound: usize = self.screen.get_height() as usize - u_scroll_margin;

		if self.selected_index < self.scroll + u_scroll_margin {
			self.scroll = self.selected_index.max(u_scroll_margin) - u_scroll_margin;
		} else if self.selected_index > self.scroll + max_bound {
			self.scroll = (self.selected_index - max_bound)
				.clamp(u_scroll_margin, self.entries.len() - u_scroll_margin);
		}
	}

	pub fn draw(&mut self) -> &Screen {
		let bg_color = Color::from(self.cfg.borrow().bg_color);
		self.screen.fill(pixel::pxl_bg(' ', bg_color));

		// Display error?
		// if let StatusLineState::Error(err) = &self.status_line.state {
		// 	self.screen.print_fbg( 0, 0,
		// 		format!("Could not load path: \"{}\"", self.path.display()) .as_str(),
		// 		Color::Red, bg_color);
		// 	self.screen.print_fbg( 0, 1,
		// 		format!("Error: {}", err) .as_str(),
		// 		Color::Red, bg_color);
		// 	return &self.screen;
		// }

		// Display empty
		if self.entries.is_empty() {
			self.screen.print_fbg(0, 0, "(empty)", Color::DarkGrey, bg_color);
			return &self.screen;
		}

		let screen_selected_idx: isize = self.selected_index as isize - self.scroll as isize;
		let folder_color: Color = Color::from( self.cfg.borrow().folder_color );
		let file_color: Color = Color::from( self.cfg.borrow().file_color );

		// Highlight line
		self.screen.h_line(
			0, screen_selected_idx as i32,
			self.screen.get_width() as i32,
			pixel::pxl_bg(' ', Color::DarkGrey)
		);

		// Display entries
		for (i, pathbuf) in self.entries.iter() .skip(self.scroll) .enumerate() {
			if i as u32 >= self.screen.get_height() { break; }
			let mut file_name: String = util::file_name(pathbuf);

			let bg: Color = if i == screen_selected_idx as usize { Color::DarkGrey } else { bg_color };
			let fg: Color = if pathbuf.is_dir() {
				file_name.push('/');
				folder_color
			} else {
				file_color
			};

			self.screen.print_fbg(0, i as i32, &file_name, fg, bg );
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

    pub fn set_color_as<C>(&mut self, color: C) -> &mut Self
    where C: Into<Color> {
        self.color = color.into();
        self
    }

    pub fn set_state(&mut self, state: StatusLineState) -> &mut Self {
        self.state = state;
        self
    }

    pub fn set_error(&mut  self, err: AppError, prefix: Option<&str>) -> &mut Self {
        self.state = StatusLineState::Error(err);
        self.color = Color::Red;
        self.text = prefix.unwrap_or("Error: \n").to_string();
        self
    }

    pub fn set_prompt(&mut self, prompt_line: PromptLine, prompt_mode: PromptMode) -> &mut Self {
        self.state = StatusLineState::Prompt {
            prompt_line,
            mode: prompt_mode,
        };
        self
    }

    pub fn draw(&self, engine: &mut ConsoleEngine, bg_color: Color) {
        let text: String = self.to_string();

        let prompt_line: &PromptLine = match &self.state {
            StatusLineState::Prompt { prompt_line, .. } => prompt_line,
            _ => {
                engine.print_fbg(
                    0,
                    engine.get_height() as i32 - text.lines().count() as i32,
                    &text,
                    self.color,
                    bg_color
                    );
                return;
            },
        };

        // Draw prompt line
        engine.print_fbg(
            0,
            engine.get_height() as i32 - 1,
            &text,
            self.color,
            bg_color
            );
        
        let i: i32 = prompt_line.cursor_pos as i32;
        let ch: &str = prompt_line.get(i as usize..i as usize+1) .unwrap_or(" ");
        engine.print_fbg(
            i + self.text.len() as i32,
            engine.get_height() as i32 - 1,
            ch,
            Color::Black,
            Color::White
            )

    }
}

impl ToString for StatusLine {
    fn to_string(&self) -> String {
        match &self.state {
            StatusLineState::Normal => self.text.clone(),
            StatusLineState::Error(err) => format!("{}{}", &self.text, err),
            StatusLineState::Prompt { prompt_line, .. } => format!("{}{}", &self.text, prompt_line.as_ref()),
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
            KeyCode::Esc => return Some(PromptEvent::Cancel( self.input.clone() )),
            KeyCode::Enter => return Some(PromptEvent::Enter( self.input.clone() )),
            KeyCode::Backspace => {
                self.remove_char(1);
                return Some(PromptEvent::Input( self.input.clone() ))
            },
            KeyCode::Delete => {
                self.remove_char(-1);
                return Some(PromptEvent::Input( self.input.clone() ))
            },

            KeyCode::Left => self.move_cursor(-1),
            KeyCode::Right => self.move_cursor(1),
            KeyCode::Home => self.move_cursor(i32::MIN),
            KeyCode::End => self.move_cursor(i32::MAX),

            // Write char
            KeyCode::Char(ch) => {
                self.put_char(ch);
                return Some(PromptEvent::Input( self.input.clone() ))
            },

            _ => (),
        }

        None
    }

    /// Insert a character at the position of the cursor
    pub fn put_char(&mut self, chr: char) {
        let mut new_buffer = String::with_capacity(self.input.capacity() + 1);
        new_buffer.extend(
            self.input.chars()
                .take(self.cursor_pos)
                .chain(std::iter::once(chr))
                .chain(self.input.chars().skip(self.cursor_pos)),
        );
        self.input = new_buffer;
        self.move_cursor(1);
    }

    /// Removes a certain amount of characters either on the left (positive) or right (negative) side of the cursor
    pub fn remove_char(&mut self, amount: i32) {
        if amount == 0 { return; }

        let off_l = amount.max(0) as usize; // offset to the left from cursor, `positive` or 0
        let off_r = amount.min(0).unsigned_abs() as usize; // offset to the right from cursor,  `-negative` or 0
        let pos_l = self.cursor_pos.saturating_sub(off_l);
        let pos_r = self.cursor_pos
            .saturating_add(off_r)
            .min(self.input.len());
        self.input = self.input.chars().take(pos_l).collect::<String>()
            + &self.input.chars().skip(pos_r).collect::<String>(); // this skips the cursor +/- offsets
        self.move_cursor(-amount.max(0));
    }


    pub fn move_cursor(&mut self, amt: i32) {
        self.cursor_pos = (self.cursor_pos as i64 + amt as i64)
            .clamp(0, self.input.len() as i64) as usize;
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

