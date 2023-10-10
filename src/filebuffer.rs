use std::ffi::{OsStr, OsString};
use std::path::{ PathBuf, Path };
use std::rc::Rc;
use std::cell::RefCell;

use console_engine::screen::Screen;
use console_engine::{
	pixel, Color, KeyCode,
	KeyEventKind
};
use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};

use crate::config::Configs;
use crate::util;




#[derive(Debug)]
pub enum BufferState {
	Normal,
	QuickSearch(String), // When using '/' search
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
	pub status_text: (String, Color),
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
			status_text: ( util::path2string(path), Color::White ),
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

	pub fn update_status_text(&mut self) {
		match &self.state {
			BufferState::Normal => {
				self.status_text = ( self.path.display().to_string(), Color::White );
			},
			BufferState::QuickSearch(pattern) => {
				let col: Color = Color::from( self.cfg.borrow().special_color );
				self.status_text = ( format!("Searching for: {}", pattern), col );
			},
			_ => {},
		}
	}

	// Sets the path
	// Don't confuse this with open_selected() lol
	pub fn open_dir(&mut self, path: &Path) {
		self.path = PathBuf::from(path);
		self.load_entries();
		self.state = BufferState::Normal;
		self.update_status_text();
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

	fn open_selected(&mut self) {
		let pathbuf: &PathBuf = match self.entries.get(self.selected_index) {
			Some(p) => p,
			None => return,
		};

		if pathbuf.is_file() {
			if opener::open(pathbuf).is_err() {
				self.status_text = ( String::from("Could not open file. Revealing in file explorer instead"), Color::Red );
				let _ = opener::reveal(pathbuf);
			}
		} else if pathbuf.is_dir() {
			self.path.push( pathbuf.file_name().unwrap_or_default() );
			self.load_entries();
			self.update_status_text();
		}
	}

	pub fn reveal(&self) -> Result<(), opener::OpenError> {
		if let Some(pathbuf) = self.get_selected_path() {
			opener::reveal(pathbuf)
		} else {
			opener::open(&self.path)
		}
	}

	// Returns whether the state was QuickSearch. Aaaa why
	pub fn quicksearch_handle_key_event(&mut self, event: KeyEvent) -> bool {
		let pattern = match &mut self.state {
			BufferState::QuickSearch(p) => p,
			_ => return false,
		};

		match event {
			// Exit quick search
			KeyEvent { code: KeyCode::Esc, kind: KeyEventKind::Press, .. } => {
				self.state = BufferState::Normal;
				self.update_status_text();
				return true;
			},

			// Enter to open
			KeyEvent { code: KeyCode::Enter, kind: KeyEventKind::Press, .. } => {
				pattern.clear();
				self.open_selected();
				return true;
			},

			// Backspace to delete char
			KeyEvent { code: KeyCode::Backspace, kind: KeyEventKind::Press, .. } => {
				pattern.pop();
			},

			// Add char and update
			KeyEvent { code: KeyCode::Char(ch), kind: KeyEventKind::Press, ..  } => {
				pattern.push(ch);
			},

			_ => {},
		}

		// Search
		let pattern_lowercase: String = pattern.to_lowercase();
		for (i, pathbuf) in self.entries.iter().enumerate() {
			let file_name = pathbuf.file_name()
				.and_then(|osstr| osstr.to_str()) .unwrap_or_default()
				.to_ascii_lowercase();

			if file_name.starts_with(&pattern_lowercase) {
				self.selected_index = i;
				break;
			}
		}

		self.update_scroll();
		self.update_status_text();
		true
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
		// Quick search
		let is_quicksearch_wait_why_did_i_do_this_again_god_theres_gotta_be_a_better_way_to_do_this: bool = self.quicksearch_handle_key_event(event);
		if is_quicksearch_wait_why_did_i_do_this_again_god_theres_gotta_be_a_better_way_to_do_this { return; }

		// Normal mode
		match event {
			// Move cursor up
			KeyEvent { code: KeyCode::Char('k') | KeyCode::Up, kind: KeyEventKind::Press, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = self.selected_index.checked_sub(1) .unwrap_or(len - 1);
				self.update_scroll();
			},

			// Move cursor down
			KeyEvent { code: KeyCode::Char('j') | KeyCode::Down, kind: KeyEventKind::Press, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = (self.selected_index + 1) % len;
				self.update_scroll();
			},

			// Open
			KeyEvent { code: KeyCode::Enter, kind: KeyEventKind::Press, .. } => {
				self.state = BufferState::Normal;
				self.open_selected();
			},

			// Go back
			KeyEvent { code: KeyCode::Char('-'), kind: KeyEventKind::Press, .. } => {
				let folder_name: Option<OsString> = self.path.file_name() .map(|s| s.to_os_string());
				let went_back: bool = self.path.pop();
				self.update_status_text();
				if went_back {
					self.state = BufferState::Normal;
					self.load_entries();

					if let Some(folder_name) = folder_name {
						self.select(&folder_name);
					}
				}
			},

			// Jump to start
			KeyEvent { code: KeyCode::Char('g'), kind: KeyEventKind::Press, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = 0;
				self.update_scroll();
			}

			// Jump to end
			KeyEvent { code: KeyCode::Char('G'), kind: KeyEventKind::Press, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = len - 1;
				self.update_scroll();
			}

			// Start quick search
			KeyEvent { code: KeyCode::Char('/'), kind : KeyEventKind::Press, .. } => {
				if self.entries.is_empty() { return; }
				self.state = BufferState::QuickSearch( String::new() );
			},

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
		for (i, path) in self.entries.iter() .skip(self.scroll) .enumerate() {
			if i as u32 >= self.screen.get_height() { break; }
			let mut file_name: String = util::path2string( path.file_name().unwrap_or_default() );

			let bg: Color = if i == screen_selected_idx as usize { Color::DarkGrey } else { bg_color };
			let fg: Color = if path.is_dir() {
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

