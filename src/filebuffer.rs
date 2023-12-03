use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{ PathBuf, Path };
use std::rc::Rc;
use std::cell::RefCell;

use console_engine::screen::Screen;
use console_engine::{
	pixel, Color, KeyCode,
	KeyEventKind, KeyModifiers
};
use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};

use crate::config::Configs;
use crate::{util, CONTROL_SHIFT};



pub struct StatusLine {
    pub text: String,
    pub color: Color,
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

    pub fn set_from_promptline(&mut self, prompt_line: &PromptLine) -> &mut Self {
        self.text = prompt_line.get_text();
        self.color = prompt_line.color;
        self
    }
}




#[derive(Debug)]
pub enum BufferState {
	Normal,
	QuickSearch(PromptLine), // When using '/' search
    CreateDir(PromptLine),
    CreateFile(PromptLine),
    Delete(PromptLine),
    Rename(PromptLine),
	Error(std::io::Error),
}

pub struct FileBuffer {
	pub path: PathBuf,
	screen: Screen,
	selected_index: usize,
	scroll: usize,
	cfg: Rc<RefCell<Configs>>,
	pub entries: Vec<PathBuf>, // List of folders & files
	pub state: BufferState,
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
			state: BufferState::Normal,
			status_line: StatusLine {
                text: util::path2string(path),
                color: Color::White
            },
		}
	}

	// Load files
	pub fn load_entries(&mut self) {
		self.entries = match util::get_at_sorted(&self.path) {
			Ok(v) => { v },
			Err(err) => {
				self.state = BufferState::Error(err);
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

	// Sets the path
	// Don't confuse this with open_selected() lol
	pub fn open_dir(&mut self, path: &Path) {
		self.path = PathBuf::from(path);
		self.load_entries();
		self.state = BufferState::Normal;
		self.display_path();
	}

	pub fn select(&mut self, file_name: &OsStr) {
		if let Some(idx) = self.entries.iter() .position(|path| path.file_name() == Some(file_name)) {
			self.selected_index = idx;
			self.update_scroll();
		}
	}

	pub fn get_selected_path(&self) -> Option<&PathBuf> {
		self.entries.get(self.selected_index)
	}

    // Returns whether it was a valid directory
	fn open_selected(&mut self) -> bool {
		let pathbuf: &PathBuf = match self.entries.get(self.selected_index) {
			Some(p) => p,
			None => return false,
		};

		if pathbuf.is_file() {
			if opener::open(pathbuf).is_err() {
				self.status_line.set_text("Could not open file. Revealing in file explorer instead")
                    .set_color(Color::Red);
				let _ = opener::reveal(pathbuf);
			}
            return false;
		} else if pathbuf.is_dir() {
			self.path.push( pathbuf.file_name().unwrap_or_default() );
			self.load_entries();
            return true;
		}
        false
	}

	pub fn reveal(&self) -> Result<(), opener::OpenError> {
		if let Some(pathbuf) = self.get_selected_path() {
			opener::reveal(pathbuf)
		} else {
			opener::open(&self.path)
		}
	}

	pub fn handle_mouse_event(&mut self, event: MouseEvent) {
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
        match &mut self.state {
            BufferState::Normal => self.handle_normal_mode_event(event),

            BufferState::Error(err) => {
				self.status_line.set_text( &format!("Error: {}", err) )
                    .set_color(Color::Red);
            },

            BufferState::QuickSearch(prompt_line) => {
                match prompt_line.handle_key_event(event) {
                    Some(PromptEvent::Cancel(_)) => {
                        self.state = BufferState::Normal;
                        self.display_path();
                    },

                    Some(PromptEvent::Enter(_)) => {
                        prompt_line.clear();
                        self.status_line.set_from_promptline(prompt_line);
                        let valid_dir: bool = self.open_selected();

                        if !valid_dir {
                            self.state = BufferState::Normal;
                            self.display_path();
                        }
                    },

                    // Search
                    Some(PromptEvent::Input(pattern)) => {
                        if pattern.is_empty() { return; }

                        let pattern_lowercase: String = pattern.to_lowercase();
                        for (i, pathbuf) in self.entries.iter().enumerate() {
                            let file_name: String = pathbuf.file_name()
                                .and_then(|osstr| osstr.to_str()). unwrap_or_default()
                                .to_ascii_lowercase();

                            if file_name.starts_with(&pattern_lowercase) {
                                self.selected_index = i;
                                break;
                            }
                        }

                        self.status_line.set_from_promptline(prompt_line);
                        self.update_scroll();
                    },

                    None => {},
                }
            },

            BufferState::CreateDir(prompt_line) => {
                match prompt_line.handle_key_event(event) {
                    Some(PromptEvent::Cancel(_)) => {
                        self.state = BufferState::Normal;
                        self.display_path();
                    },

                    Some(PromptEvent::Enter(dir_name)) => {
                        let dir_name = dir_name.clone();
                        self.state = BufferState::Normal;

                        if let Err(err) = fs::create_dir_all( self.path.join(&dir_name) ) {
                            self.status_line.set_text(&format!("Failed to create folder: {}", err))
                                .set_color(Color::Red);
                        } else {
                            self.status_line.set_text( &format!("Created folder \"{}\"", &dir_name) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.load_entries();
                        self.select( &OsString::from(&dir_name) );
                    },

                    Some(PromptEvent::Input(_)) => {
                        self.status_line.set_from_promptline(prompt_line);
                    },

                    None => {},
                }
            },
            
            BufferState::CreateFile(prompt_line) => {
                match prompt_line.handle_key_event(event) {
                    Some(PromptEvent::Cancel(_)) => {
                        self.state = BufferState::Normal;
                        self.display_path();
                    },

                    Some(PromptEvent::Enter(file_name)) => {
                        let file_name = file_name.clone();
                        self.state = BufferState::Normal;

                        if let Err(err) = fs::File::create( self.path.join(&file_name) ) {
                            self.status_line.set_text( &format!("Failed to create file: {}", err) )
                                .set_color(Color::Red);
                        } else {
                            self.status_line.set_text( &format!("Created file \"{}\"", &file_name) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.load_entries();
                        self.select( &OsString::from(&file_name) );
                    },

                    Some(PromptEvent::Input(_)) => {
                        self.status_line.set_from_promptline(prompt_line);
                    },

                    None => {},
                }
            },

            BufferState::Delete(prompt_line) => {
                match prompt_line.handle_key_event(event) {
                    Some(PromptEvent::Cancel(_)) => {
                        self.state = BufferState::Normal;
                        self.display_path();
                    },

                    Some(PromptEvent::Enter(text)) => {
                        if text.to_lowercase() != "y" {
                            self.state = BufferState::Normal;
                            self.display_path();
                            return;
                        }

                        let pathbuf: &PathBuf = if let Some(p) = self.get_selected_path() { p } else { return; };
                        let res = if pathbuf.is_dir() {
                            fs::remove_dir_all( self.path.join(pathbuf) )
                        } else {
                            fs::remove_file( self.path.join(pathbuf) )
                        };

                        if let Err(err) = res {
                            self.status_line.set_text( &format!("Failed to delete: {}", err) )
                                .set_color(Color::Red);
                        } else {
                            self.status_line.set_text( &format!("Successfully deleted \"{}\"", util::file_name(pathbuf) ) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.state = BufferState::Normal;
                        self.load_entries();
                    },

                    Some(PromptEvent::Input(_)) => {
                        self.status_line.set_from_promptline(prompt_line);
                    },

                    None => {},
                }
            },

            BufferState::Rename(prompt_line) => {
                match prompt_line.handle_key_event(event) {
                    Some(PromptEvent::Cancel(_)) => {
                        self.state = BufferState::Normal;
                        self.display_path();
                    },

                    Some(PromptEvent::Enter(new_file_name)) => {
                        let new_path = self.path.join(&new_file_name);
                        let old_path = if let Some(p) = self.get_selected_path() { self.path.join(p) } else { return };

                        if let Err(err) = fs::rename(old_path, new_path) {
                            self.status_line.set_text( &format!("Failed to rename: {}", err) )
                                .set_color(Color::Red);
                        } else {
                            self.status_line.set_text( &format!("Successfully renamed to \"{}\"", &new_file_name) )
                                .set_color_as(self.cfg.borrow().special_color);
                        }

                        self.state = BufferState::Normal;
                        self.load_entries();
                    },

                    Some(PromptEvent::Input(_)) => {
                        self.status_line.set_from_promptline(prompt_line);
                    },

                    None => {},
                }
            },

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
				self.state = BufferState::Normal;
				self.open_selected();
                self.display_path();
			},

			// Go back
			KeyEvent { code: KeyCode::Char('-') | KeyCode::Backspace, .. } => {
				let folder_name: Option<OsString> = self.path.file_name() .map(|s| s.to_os_string());
				let went_back: bool = self.path.pop();
				self.display_path();
				if went_back {
					self.state = BufferState::Normal;
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

                let prompt_line: PromptLine = PromptLine::default()
                        .with_color( Color::from(self.cfg.borrow().special_color) )
                        .with_prefix("Searching for: ");
                self.status_line.set_from_promptline(&prompt_line);
				self.state = BufferState::QuickSearch(prompt_line);
			},

			// Create folder with Ctrl + Shift + n
			KeyEvent { code: KeyCode::Char('N'), modifiers, .. }
            if modifiers.bits() == CONTROL_SHIFT => {
                let prompt_line: PromptLine = PromptLine::default()
                    .with_prefix("Create folder: ");
                self.status_line.set_from_promptline(&prompt_line);
                self.state = BufferState::CreateDir(prompt_line);
			}

			// Create file with Ctrl + n
			KeyEvent { code: KeyCode::Char('n'), modifiers: KeyModifiers::CONTROL, .. } => {
                let prompt_line: PromptLine = PromptLine::default()
                    .with_prefix("Create file: ");
                self.status_line.set_from_promptline(&prompt_line);
                self.state = BufferState::CreateFile(prompt_line);
			}

			// Delete with Ctrl + d
			KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::CONTROL, .. } => {
                let file_name: String = match self.get_selected_path() {
                    Some(p) => util::file_name(p),
                    None => return,
                };

                let prompt_line: PromptLine = PromptLine::default()
                    .with_prefix( format!("Delete {}? (y/n): ", file_name).as_str() )
                    .with_color(Color::Red);
                self.status_line.set_from_promptline(&prompt_line);
                self.state = BufferState::Delete(prompt_line);
			},

			// Rename with Ctrl + r
			KeyEvent { code: KeyCode::Char('r'), modifiers: KeyModifiers::CONTROL, .. } => {
                let file_name: String = match self.get_selected_path() {
                    Some(p) => util::file_name(p),
                    None => return,
                };

                let prompt_line: PromptLine = PromptLine::default()
                    .with_prefix( format!("Rename \"{}\" to: ", &file_name).as_str() )
                    .with_color(Color::from(self.cfg.borrow().special_color))
                    .with_initial_text(&file_name);
                self.status_line.set_from_promptline(&prompt_line);
                self.state = BufferState::Rename(prompt_line);
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

		// Display error
		if let BufferState::Error(err) = &self.state {
			self.screen.print_fbg( 0, 0,
				format!("Could not load path: \"{}\"", self.path.display()) .as_str(),
				Color::Red, bg_color);
			self.screen.print_fbg( 0, 1,
				format!("Error: {}", err) .as_str(),
				Color::Red, bg_color);
			return &self.screen;
		}

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





pub enum PromptEvent {
    Input(String),
    Enter(String),
    Cancel(String),
}


#[derive(Debug)]
pub struct PromptLine {
    pub color: Color,
    pub prefix: String,
    pub input: String,
}

impl PromptLine {
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    pub fn with_initial_text(mut self, initial_text: &str) -> Self {
        self.input = initial_text.to_string();
        self
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) -> Option<PromptEvent> {
        if event.kind != KeyEventKind::Press {
            return None;
        }

        match event {
            // Exit
            KeyEvent { code: KeyCode::Esc, .. } => Some(PromptEvent::Cancel( self.input.clone() )),
            // Enter
            KeyEvent { code: KeyCode::Enter, .. } => Some(PromptEvent::Enter( self.input.clone() )),
            // Backspace
            KeyEvent { code: KeyCode::Backspace, .. } => {
                self.input.pop();
                Some(PromptEvent::Input( self.input.clone() ))
            },

            // Write char
            KeyEvent { code: KeyCode::Char(ch), .. } => {
                self.input.push(ch);
                Some(PromptEvent::Input( self.input.clone() ))
            },

            _ => None,
        }
    }

    pub fn get_text(&self) -> String {
        format!("{}{}", self.prefix, self.input)
    }

    pub fn clear(&mut self) {
        self.input.clear();
    }

}

impl Default for PromptLine {
    fn default() -> Self {
        Self {
            color: Color::White,
            prefix: String::new(),
            input: String::new(),
        }
    }
}
