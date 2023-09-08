use std::ffi::{OsStr, OsString};
use std::path::{ PathBuf, Path };
use std::rc::Rc;
use std::cell::RefCell;

use console_engine::screen::Screen;
use console_engine::{
	pixel, Color, KeyCode, KeyModifiers,
	KeyEventKind
};
use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};

use serde::{ Deserialize, Serialize };
use directories::UserDirs;



pub mod util {
	use std::path::{Path, PathBuf};
	use std::fs;

	// Idk if there's any builtin methods for this
	pub fn path2string<P>(path: P) -> String
	where P: AsRef<Path> {
		String::from( path.as_ref() .to_string_lossy() )
	}

	// Get files & folders and have folders come before files (ofc, alphabetically sorted)
	pub fn get_at_sorted<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
	where P: AsRef<Path> {
		let (mut folders, mut files): (Vec<PathBuf>, Vec<PathBuf>) = fs::read_dir(path)?
			.flatten()
			.map(|de| de.path())
			.filter(|path| !path.is_symlink())
			.partition(|path| path.is_dir());

		folders.append(&mut files);
		Ok(folders)
	}

	pub fn get_files_at<P>(path: P, limit: usize) -> Result<Vec<PathBuf>, std::io::Error>
	where P: AsRef<Path> {
		Ok(fs::read_dir(path)?
			.flatten()
			.map(|de| de.path())
			.filter(|pathbuf| pathbuf.is_file())
			.take(limit)
			.collect())
	}

	pub fn get_folders_at<P>(path: P, limit: usize) -> Result<Vec<PathBuf>, std::io::Error>
	where P: AsRef<Path> {
		Ok(fs::read_dir(path)?
			.flatten()
			.map(|de| de.path())
			.filter(|pathbuf| pathbuf.is_dir())
			.take(limit)
			.collect())
	}

	// get_folders_at() but without limits
	pub fn get_all_folders_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
	where P: AsRef<Path> {
		Ok(fs::read_dir(path)?
			.flatten()
			.map(|de| de.path())
			.filter(|pathbuf| pathbuf.is_dir())
			.collect())
	}

	// Bro just use io::Result
	pub fn get_all_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
	where P: AsRef<Path> {
		Ok(fs::read_dir(path)?
			.flatten()
			.map(|de| de.path())
			.filter(|pathbuf| !pathbuf.is_symlink())
			.collect())
	}

	// Get files & folders, separated into tuples
	// I don't know how it took me so long to discover Iterator.partition(). I almost implemented macro  segregate!(vec, condition)  no joke
	pub fn get_files_and_folders_at<P>(path: P) -> Result< (Vec<PathBuf>, Vec<PathBuf>), std::io::Error >
	where P: AsRef<Path> {
		Ok(fs::read_dir(path)?
			.flatten()
			.map(|de| de.path())
			.filter(|path| !path.is_symlink())
			.partition(|path| path.is_file())
		)
	}

}




#[derive(Debug, Serialize, Deserialize)]
pub struct Configs {
	pub scroll_margin: u8,
	pub max_search_stack: usize,
	pub favorites: Vec<PathBuf>,
	pub default_path: PathBuf,
	pub target_fps: u32,
	pub search_ignore_types: String,

	// The best feature: colors
	pub folder_color: (u8, u8, u8),
	pub file_color: (u8, u8, u8),
	pub special_color: (u8, u8, u8),
	pub bg_color: (u8, u8, u8),
}

impl Default for Configs {
	fn default() -> Self {
		let userdir: UserDirs = UserDirs::new().expect("Could not find home directory");
		let home_dir: &Path = userdir.home_dir();

		Self {
			scroll_margin: 4,
			max_search_stack: 512,
			favorites: vec![ PathBuf::from(home_dir) ],
			default_path: PathBuf::from(home_dir),
			target_fps: 10,
			search_ignore_types: String::new(),

			folder_color: ( 105, 250, 255 ),
			file_color: (248, 242, 250),
			special_color: (255, 209, 84),
			bg_color: (21, 17, 23),
		}
	}
}

impl Configs {
	// Returns true if path was added to favorites, false otherwise
	pub fn toggle_favorite(&mut self, path: PathBuf) -> bool {
		if let Some(index) = self.favorites.iter() .position(|p| p == &path) {
			self.favorites.remove(index);
			return false;
		}
		self.favorites.push(path);
		true
	}
}





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
				self.status_text = ( util::path2string(&self.path), Color::White );
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

			// Reveal in file explorer
			KeyEvent {
				code: KeyCode::Char('e'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL,
				.. 
			} => {
				if let Some(pathbuf) = self.entries.get(self.selected_index) {
					let _ = opener::reveal(pathbuf);
				} else {
					let _ = opener::open(&self.path);
				}
				return true;
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
				self.update_status_text();
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

			// Reveal in file explorer
			KeyEvent {
				code: KeyCode::Char('e'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL,
				.. 
			} => {
				if let Some(pathbuf) = self.entries.get(self.selected_index) {
					let _ = opener::reveal(pathbuf);
				} else {
					let _ = opener::open(&self.path);
				}
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






pub mod querying {

	use std::collections::VecDeque;
	use std::path::{PathBuf, Path};
	use std::rc::Rc;
	use std::cell::RefCell;
	use std::sync::mpsc::{self, Receiver};
	use std::thread;

	use console_engine::{
		pixel, Color, KeyCode, KeyEventKind, screen::Screen, rect_style::BorderStyle
	};

	use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
	use console_engine::events::Event;
	use console_engine::forms::{
		Form, FormField, FormStyle, FormValue, FormOptions, Text,
	};


	use super::{
		Configs,
		util::*,
	};


	#[derive(Debug, PartialEq)]
	pub enum SearchPanelState {
		Running,
		Exit( Option<PathBuf> ),
	}


	// Used for quick-searching files, folders, and favorites
	// The actual searching happens in SearchQuery
	pub struct SearchPanel {
		screen: Screen,
		form: Form, // Input box
		selected_index: usize,
		query: SearchQuery,
		cfg: Rc<RefCell<Configs>>,
		pub state: SearchPanelState,
	}

	impl SearchPanel {
		pub fn new(width: u32, height: u32, mode: SearchQueryMode, cfg: Rc<RefCell<Configs>>) -> Self {
			let max_result_count: usize = (height - 5) as usize;
			let max_stack_size: usize = cfg.borrow().max_search_stack;
			let ignore_types: String = cfg.borrow().search_ignore_types.clone();
			let bg_color = Color::from(cfg.borrow().bg_color);

			Self {
				screen: Screen::new(width, height),
				form: SearchPanel::build_form(width - 2, bg_color),
				selected_index: 0,
				query: SearchQuery::new(mode, max_result_count, max_stack_size, ignore_types),
				cfg,
				state: SearchPanelState::Running,
			}
		}

		fn build_form(width: u32, bg_color: Color) -> Form {
			let theme = FormStyle {
				border: Some(BorderStyle::new_light().with_colors(Color::White, bg_color)),
				bg: bg_color,
				..Default::default()
			};

			let mut form = Form::new(
				width, 3,
				FormOptions { style: theme, ..Default::default() }
			);
			form.build_field::<Text>(
				"query",
				FormOptions { style: theme, ..Default::default() }
			);

			form.set_active(true);
			form
		}

		pub fn update(&mut self) {
			self.query.update();
			self.selected_index = self.selected_index.clamp(0, self.get_results().len());
		}

		pub fn is_running(&self) -> bool {
			self.state == SearchPanelState::Running
		}

		pub fn get_query_mode(&self) -> &SearchQueryMode {
			&self.query.mode
		}

		pub fn get_results(&self) -> &Vec<PathBuf> {
			&self.query.results
		}

		pub fn handle_mouse_event(&mut self, event: MouseEvent, y_offset: u16) {
			if let MouseEvent {
				kind: MouseEventKind::Down(_) | MouseEventKind::Drag(_),
				row,
				.. 
			} = event {
				let offset: u16 = y_offset + 4;
				let urow: u16 = row.max(offset) - offset;
				self.selected_index = (urow as usize).min(self.get_results().len() - 1);
			}
		}

		pub fn handle_key_event(&mut self, key_event: KeyEvent) {
			if key_event.kind != KeyEventKind::Press { return; }

			// No need to check for KeyEvent.kind because we only allowed KeyEventKind::Press up there
			match key_event {
				// Move cursor up
				KeyEvent { code: KeyCode::Up, .. } => {
					self.selected_index = self.selected_index.checked_sub(1)
						.unwrap_or(self.get_results().len() - 1);
					// self.update_scroll();
					return;
				},

				// Move cursor down
				KeyEvent { code: KeyCode::Down, .. } => {
					let len: usize = self.get_results().len();
					if len == 0 { return; }
					self.selected_index = (self.selected_index + 1) % len;
					// self.update_scroll();
					return;
				},

				// Esc to exit
				KeyEvent { code: KeyCode::Esc, .. } => {
					self.state = SearchPanelState::Exit(None);
				},

				// Form input
				_ => {
					self.form.handle_event(Event::Key(key_event));

					if self.form.is_finished() {
						let selected = match self.get_results().get(self.selected_index) {
							Some(de) => de,
							None => return,
						};
						self.state = SearchPanelState::Exit( Some(selected.clone()) );
						return;
					}

				},
			}

			// Handle query
			let value: String = match self.form.get_field_output("query") {
				Some(FormValue::String(value)) => value.replace('/', r"\"),
				_ => return,
			};

			self.query.search(value);
		}

		fn display_results(&mut self) {
			if self.get_results().is_empty() { return; }

			let offset: (i32, i32) = (2, 4);
			let bg_color = Color::from(self.cfg.borrow().bg_color);

			// Highlight selected line
			self.screen.h_line(
				offset.0, self.selected_index as i32 + offset.1,
				self.screen.get_width() as i32 - 2,
				pixel::pxl_bg(' ', Color::DarkGrey)
			);

			for (i, path) in self.query.results .iter().enumerate() {
				if i as u32 + offset.1 as u32 >= self.screen.get_height() - 1 { break; }

				let bg: Color = if i == self.selected_index { Color::DarkGrey } else { bg_color };

				// Format file name so it's easier to read
				// Ironically, this code is hard to read.
				let (path, fg): (&Path, (u8, u8, u8)) = match &self.query.mode {
					SearchQueryMode::Favorites(_) => (path, self.cfg.borrow().special_color),
					SearchQueryMode::Files(root_path) => ( path.strip_prefix(root_path) .unwrap_or(path), self.cfg.borrow().file_color ),
					SearchQueryMode::Folders(root_path) => ( path.strip_prefix(root_path) .unwrap_or(path), self.cfg.borrow().folder_color ),
				};
				let file_name: String = path2string(path).replace('\\', "/");

				self.screen.print_fbg(
					offset.0, i as i32 + offset.1,
					&file_name, Color::from(fg), bg
				);
			}
		}

		pub fn draw(&mut self, tick: usize) -> &Screen {
			let bg_color = Color::from(self.cfg.borrow().bg_color);
			self.screen.fill(pixel::pxl_bg(' ', bg_color));

			self.screen.rect_border(
				0, 0,
				self.screen.get_width() as i32 - 1, self.screen.get_height() as i32 - 1,
				BorderStyle::new_light().with_colors(Color::Grey, bg_color)
			);

			self.screen.print_screen( 1, 1, self.form.draw(tick) );
			self.display_results();

			let text: &str = match self.query.mode {
				SearchQueryMode::Files(_) => " Search Files ",
				SearchQueryMode::Folders(_) => " Search Folders ",
				SearchQueryMode::Favorites(_) => " Favorites ",
			};
			self.screen.print_fbg(2, 0, text, Color::Black, Color::Grey);

			&self.screen
		}
	}






	#[derive(Debug, Clone)]
	pub enum SearchQueryMode {
		Files(PathBuf),
		Folders(PathBuf),
		Favorites(Vec<PathBuf>),
	}

	struct SearchQuery {
		query: String,
		mode: SearchQueryMode,
		results: Vec<PathBuf>,
		receiver: Option< Receiver<Vec<PathBuf>> >,
		max_result_count: usize,
		max_stack_size: usize,
		ignore_types: String,
	}

	impl SearchQuery {
		// max_result_count, max_stack_size, and ignore_types are values taked from Configs
		fn new(mode: SearchQueryMode, max_result_count: usize, max_stack_size: usize, ignore_types: String) -> Self {
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
		fn list(&self) -> Vec<PathBuf> {
			match &self.mode {
				SearchQueryMode::Favorites(favorites) => {
					favorites.clone()
				},

				SearchQueryMode::Files(path) => {
					let mut results: Vec<PathBuf> = Vec::new();
					let mut stack: Vec<PathBuf> = vec![ path.clone() ];

					while results.len() < self.max_result_count {
						let search_path = if let Some(p) = stack.pop() { p } else { break; };
						let (mut files, folders) = match get_files_and_folders_at(search_path) {
							Ok(pair) => pair,
							Err(_) => continue,
						};

						results.append(&mut files);
						stack.append(&mut folders.iter()
							.take( self.max_stack_size - stack.len() )
							.cloned()
							.collect()
						);
					}

					results.truncate(self.max_result_count);
					results
				},

				SearchQueryMode::Folders(path) => {
					let mut results: Vec<PathBuf> = match get_folders_at(path, self.max_result_count) {
						Ok(v) => v,
						Err(_) => return Vec::new(),
					};
					let mut idx: usize = 0;

					while results.len() < self.max_result_count {
						let search_path = if let Some(path) = results.get(idx) { path } else { break; };
						let mut folders = match get_folders_at(search_path, self.max_result_count - results.len()) {
							Ok(v) => v,
							Err(_) => continue,
						};
						results.append(&mut folders);
						idx += 1;
					}

					results.truncate(self.max_result_count);
					results
				},

			}
		}

		fn update(&mut self) {
			let rx = match &mut self.receiver {
				Some(rx) => rx,
				None => return,
			};

			let max_search_results: usize = self.max_result_count;

			for received in rx.try_iter() {
				for pathbuf in received.iter() .take(max_search_results - self.results.len()) {
					self.results.push( pathbuf.clone() );
				}
			}

			if self.results.len() >= max_search_results {
				self.receiver = None;
			}

		}

		fn search(&mut self, query: String) {
			if query == self.query { return; }
			self.query = query;

			if self.query.is_empty() {
				self.results = self.list();
				return;
			}

			self.results.clear();
			let search_query = self.query.to_lowercase();

			// Files and Folders are done on threads. Favorites isn't.
			// We use mpsc to send results back for this specific query
			// A new set of sender and receiver is created for each query, replacing the old self.reveiver
			// If that happens while the thread is running, stop searching (obviously)

			// Man, that's a disgusting amount of indentation lol
			match &self.mode {
				SearchQueryMode::Favorites(favorites) => {
					self.receiver = None;
					self.results = favorites.iter()
						.filter(|pathbuf| path2string(pathbuf) .to_lowercase() .contains(&search_query) )
						.cloned()
						.collect();
				},

				SearchQueryMode::Files(path) => {
					let (tx, rx) = mpsc::channel::< Vec<PathBuf> >();
					self.receiver = Some(rx);
					let path = path.clone();
					let max_stack_size = self.max_stack_size;
					let ignore_types: String = self.ignore_types.clone();

					thread::spawn(move || {
						let mut stack: VecDeque<PathBuf> = VecDeque::from([ path ]);

						while let Some(search_path) = stack.pop_front() {
							let (files, folders) = match get_files_and_folders_at(search_path) {
								Ok(pair) => pair,
								Err(_) => continue,
							};
							stack.append(&mut folders.into_iter()
								// Don't search inside folders that start with "." (like .git/)
								.filter(|pathbuf| !path2string(pathbuf.file_name().unwrap_or_default()) .starts_with('.') )
								.take( max_stack_size - stack.len() )
								.collect());

							let files: Vec<PathBuf> = files.into_iter()
								.filter(|pathbuf|
									!ignore_types.contains( &path2string(pathbuf.extension().unwrap_or_default()) )
									&& path2string(pathbuf) .to_lowercase() .contains(&search_query)
								)
								.collect();

							// If receiver is gone (new query or panel is closed)
							if tx.send(files).is_err() {
								break;
							}
						}
					});

				},

				SearchQueryMode::Folders(path) => {
					let (tx, rx) = mpsc::channel::< Vec<PathBuf> >();
					self.receiver = Some(rx);
					let path = path.clone();
					let max_stack_size = self.max_stack_size;

					thread::spawn(move || {
						let mut stack: VecDeque<PathBuf> = VecDeque::from([ path ]);

						while let Some(search_path) = stack.pop_front() {
							let folders: Vec<PathBuf> = match get_all_folders_at(search_path) {
								Ok(v) => v,
								Err(_) => continue,
							};
							stack.append(&mut folders.iter()
								// Don't search inside folders that start with "." (like .git/)
								.filter(|pathbuf| !path2string(pathbuf.file_name().unwrap_or_default()) .starts_with('.') )
								.take(max_stack_size - stack.len())
								.cloned()
								.collect());
							
							let folders: Vec<PathBuf> = folders.into_iter()
								.filter( |pathbuf| path2string(pathbuf).to_lowercase() .contains(&search_query) )
								.collect();

							// If receiver is gone (new query or panel is closed)
							if tx.send(folders).is_err() {
								break;
// Behold, the avalanche of closing brackets:
							}
						}
					});

				},
			}
		}

	}


}
// Beautiful

