
use std::ffi::OsStr;
use std::fs;
use std::path::{ PathBuf, Path };
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use console_engine::events::Event;
use console_engine::rect_style::BorderStyle;
use console_engine::screen::Screen;
use console_engine::{
	pixel, Color, KeyCode, KeyModifiers,
	KeyEventKind
};
use console_engine::crossterm::event::KeyEvent;
use console_engine::forms::{
	Form, FormField, FormStyle, FormValue, FormOptions, Text,
};

use serde::{ Deserialize, Serialize };
use directories::UserDirs;



// ----------------------------------------------------------------
// UTIL
// ----------------------------------------------------------------

pub fn path2string<P>(path: P) -> String
where P: AsRef<Path> {
	String::from( path.as_ref() .to_string_lossy() )
}

pub fn get_at_sorted<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	let mut d: Vec<PathBuf> = Vec::new();
	let mut f: Vec<PathBuf> = Vec::new();

	for path in fs::read_dir(path)? .flatten() .map(|de| de.path()) {
		if path.is_dir() {
			d.push(path);
		} else if path.is_file() {
			f.push(path);
		}
	}

	d.append(&mut f);
	Ok(d)
}

pub fn get_files_at<P>(path: P, limit: usize) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf|
			pathbuf.is_file()
		)
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

pub fn get_all_folders_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf| pathbuf.is_dir())
		.collect())
}

pub fn get_all_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf| !pathbuf.is_symlink())
		.collect())
}

pub fn get_files_and_folders_at<P>(path: P) -> Result< (Vec<PathBuf>, Vec<PathBuf>), std::io::Error >
where P: AsRef<Path> {
	let mut d: Vec<PathBuf> = Vec::new();
	let mut f: Vec<PathBuf> = Vec::new();

	for path in fs::read_dir(path)? .flatten() .map(|de| de.path()) {
		if path.is_dir() {
			d.push(path);
		} else if path.is_file() {
			f.push(path);
		}
	}
	Ok((f, d))
}


// ----------------------------------------------------------------



#[derive(Debug, Serialize, Deserialize)]
pub struct Configs {
	pub scroll_margin: u8,
	pub max_search_results: usize,
	pub max_search_stack: usize,
	pub favorites: Vec<PathBuf>,
	pub folder_color: (u8, u8, u8),
}

impl Default for Configs {
	fn default() -> Self {
		Self {
			scroll_margin: 4,
			max_search_results: 30,
			max_search_stack: 255,
			favorites: UserDirs::new() .map_or(Vec::new(), |dirs| vec![ dirs.home_dir().into() ] ),
			folder_color: (0, 255, 255)
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
	QuickSearch(String),
	Error(std::io::Error),
	Exit,
}

pub struct FileBuffer {
	pub path: PathBuf,
	screen: Screen,
	selected_index: usize,
	scroll: usize,
	cfg: Rc<RefCell<Configs>>,
	pub entries: Vec<PathBuf>,
	pub state: BufferState,
	pub status_text: (String, Color),
}

impl FileBuffer {
	pub fn from_str(path: &str, screen: Screen, cfg: Rc<RefCell<Configs>>) -> Self {
		FileBuffer {
			path: PathBuf::from(path),
			screen,
			selected_index: 0,
			scroll: 0,
			cfg,
			entries: vec![],
			state: BufferState::Normal,
			status_text: ( String::from(path), Color::White ),
		}
	}

	pub fn load_entries(&mut self) {
		self.entries = match get_at_sorted(&self.path) {
			Ok(v) => v,
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
				self.status_text = ( path2string(&self.path), Color::White );
			},
			BufferState::QuickSearch(pattern) => {
				self.status_text = ( format!("Searching for: {}", pattern), Color::Yellow );
			},
			_ => {},
		}
	}

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
			self.path.push( pathbuf.file_name().unwrap() );
			self.load_entries();
		}
	}

	pub fn handle_key_event(&mut self, event: KeyEvent) {
		// QUICK SEARCH
		if let BufferState::QuickSearch(pattern) = &mut self.state {
			// Handle input
			match event {
				// Exit quick search
				KeyEvent { code: KeyCode::Esc, kind: KeyEventKind::Press, .. } => {
					self.state = BufferState::Normal;
					self.update_status_text();
					return;
				},

				// Enter to open
				KeyEvent { code: KeyCode::Enter, kind: KeyEventKind::Press, .. } => {
					pattern.clear();
					self.open_selected();
					return;
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

			let pattern_lowercase: String = pattern.to_lowercase();
			for (i, pathbuf) in self.entries.iter().enumerate() {
				let file_name = pathbuf.file_name()
					.and_then(|osstr| osstr.to_str()) .unwrap()
					.to_ascii_lowercase();

				if file_name.starts_with(&pattern_lowercase) {
					self.selected_index = i;
					break;
				}
			}

			self.update_scroll();
			self.update_status_text();
			return;
		}

		// NORMAL MODE
		match event {
			// Move cursor up
			KeyEvent { code: KeyCode::Char('k'), kind: KeyEventKind::Press, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = self.selected_index.checked_sub(1) .unwrap_or(len - 1);
				self.update_scroll();
			},

			// Move cursor down
			KeyEvent { code: KeyCode::Char('j'), kind: KeyEventKind::Press, .. } => {
				let len: usize = self.entries.len();
				if len == 0 { return; }
				self.selected_index = (self.selected_index + 1) % len;
				self.update_scroll();
			},

			// Open
			KeyEvent { code: KeyCode::Enter, kind: KeyEventKind::Press, .. } => {
				self.open_selected();
			},

			// Go back
			KeyEvent { code: KeyCode::Char('-'), kind: KeyEventKind::Press, .. } => {
				let went_back: bool = self.path.pop();
				self.update_status_text();
				if went_back {
					self.load_entries();
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
				let _ = opener::open( &self.path );
				self.state = BufferState::Exit;
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
		self.screen.clear();

		// Display error
		if let BufferState::Error(err) = &self.state {
			self.screen.print_fbg( 0, 0,
				format!("Could not load path: \"{}\"", self.path.display()) .as_str(),
				Color::Red, Color::Black);
			self.screen.print_fbg( 0, 1,
				format!("Error: {}", err) .as_str(),
				Color::Red, Color::Black);
			return &self.screen;
		}

		// Display empty
		if self.entries.is_empty() {
			self.screen.print_fbg(0, 0, "(empty)", Color::DarkGrey, Color::Black);
			return &self.screen;
		}

        let screen_selected_idx: isize = self.selected_index as isize - self.scroll as isize;
		self.screen.h_line(
			0, screen_selected_idx as i32,
			self.screen.get_width() as i32,
			pixel::pxl_bg(' ', Color::DarkGrey)
		);

		// Display entries
		for (i, path) in self.entries.iter() .skip(self.scroll) .enumerate() {
			if i as u32 >= self.screen.get_height() { break; }
			let mut file_name: String = path.file_name().unwrap() .to_str().unwrap() .to_string();

			let bg: Color = if i == screen_selected_idx as usize { Color::DarkGrey } else { Color::Black };
			let fg: Color = if path.is_dir() {
				file_name.push('/');
				Color::from( self.cfg.borrow().folder_color )
			} else {
				Color::White
			};

			self.screen.print_fbg(0, i as i32, &file_name, fg, bg );
		}
		&self.screen
	}
}







#[derive(Debug, PartialEq)]
pub enum SearchPanelState {
	Running,
	Exit( Option<PathBuf> ),
}


pub struct SearchPanel {
	screen: Screen,
	form: Form,
	selected_index: usize,
	query: SearchQuery,
	pub state: SearchPanelState,
}

impl SearchPanel {
	pub fn new(width: u32, height: u32, mode: SearchQueryMode, cfg: Rc<RefCell<Configs>>) -> Self {
		let max_result_count: usize = cfg.borrow().max_search_results;
		let max_stack_size: usize = cfg.borrow().max_search_stack;

		Self {
			screen: Screen::new(width, height),
			form: SearchPanel::build_form(width - 2),
			selected_index: 0,
			query: SearchQuery::new(mode, max_result_count, max_stack_size),
			state: SearchPanelState::Running,
		}
	}

	fn build_form(width: u32) -> Form {
		let theme = FormStyle {
			border: Some(BorderStyle::new_light()),
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
	}

	pub fn is_running(&self) -> bool {
		self.state == SearchPanelState::Running
	}

	pub fn get_query_mode(&self) -> &SearchQueryMode {
		&self.query.mode
	}

	pub fn handle_event(&mut self, event: Event) {
		// Only allow key press events from here on
		// Forms only process key events anyways
		let key_event = match event {
			Event::Key(k @ KeyEvent { kind: KeyEventKind::Press, .. }) => k,
			_ => return,
		};

		// No need to check for KeyEvent.kind because we only allowed KeyEventKind::Press up there
		match key_event {
			// Move cursor up
			KeyEvent { code: KeyCode::Up, .. } => {
				self.selected_index = self.selected_index.checked_sub(1)
					.unwrap_or(self.query.results.len() - 1);
				// self.update_scroll();
				return;
			},

			// Move cursor down
			KeyEvent { code: KeyCode::Down, .. } => {
				let len: usize = self.query.results.len();
				if len == 0 { return; }
				self.selected_index = (self.selected_index + 1) % len;
				// self.update_scroll();
				return;
			},

			// Esc to exit
			KeyEvent { code: KeyCode::Esc, .. } => {
				self.state = SearchPanelState::Exit( None );
			},

			_ => {
				self.form.handle_event(event);

				if self.form.is_finished() {
					let selected = match self.query.results.get(self.selected_index) {
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
		if self.query.results.is_empty() { return; }

		let offset: (i32, i32) = (2, 4);
		self.screen.h_line(
			offset.0, self.selected_index as i32 + offset.1,
			self.screen.get_width() as i32 - 2,
			pixel::pxl_bg(' ', Color::DarkGrey)
		);

		for (i, path) in self.query.results .iter().enumerate() {
			if i as u32 + offset.1 as u32 >= self.screen.get_height() - 1 { break; }

			let bg: Color = if i == self.selected_index { Color::DarkGrey } else { Color::Black };
			let file_name: String = path2string(match &self.query.mode {
				SearchQueryMode::Favorites(_) => path,
				SearchQueryMode::Files(root_path) | SearchQueryMode::Folders(root_path) => path.strip_prefix(root_path) .unwrap_or(path)
			}) .replace('\\', "/");

			self.screen.print_fbg(
				offset.0, i as i32 + offset.1,
				&file_name,
				Color::White,
				bg
			);
		}
	}

	pub fn draw(&mut self, tick: usize) -> &Screen {
		self.screen.clear();

		self.screen.rect_border(
			0, 0,
			self.screen.get_width() as i32 - 1, self.screen.get_height() as i32 - 1,
			BorderStyle::new_double()
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
}

impl SearchQuery {
	fn new(mode: SearchQueryMode, max_result_count: usize, max_stack_size: usize) -> Self {
		let mut q = Self {
			query: String::new(),
			mode,
			results: vec![],
			receiver: None,
			max_result_count,
			max_stack_size,
		};

		q.results = q.list();
		q
	}

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
					let (mut files, folders) = get_files_and_folders_at(search_path)
						.unwrap();
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
				let mut results: Vec<PathBuf> = get_folders_at(path, self.max_result_count) .unwrap();
				let mut idx: usize = 0;

				while results.len() < self.max_result_count {
					let search_path = if let Some(path) = results.get(idx) { path } else { break; };
					let mut folders = get_folders_at(search_path, self.max_result_count - results.len())
						.unwrap();
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

		// TODO maybe combine Files and Folders into one?
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

				thread::spawn(move || {
					let mut stack: Vec<PathBuf> = vec![path];

					while let Some(search_path) = stack.pop() {
						let (files, folders) = match get_files_and_folders_at(search_path) {
							Ok(pair) => pair,
							Err(_) => continue,
						};
						stack.append(&mut folders.into_iter()
							.take( max_stack_size - stack.len() )
							.collect());

						let files: Vec<PathBuf> = files.into_iter()
							.filter( |pathbuf| path2string(pathbuf) .to_lowercase() .contains(&search_query) )
							.collect();

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
					let mut stack: Vec<PathBuf> = vec![path];

					while let Some(search_path) = stack.pop() {
						let folders = match get_all_folders_at(search_path) {
							Ok(v) => v,
							Err(_) => continue,
						};
						stack.append(&mut folders.iter()
							.take(max_stack_size - stack.len())
							.cloned()
							.collect());
						
						let folders: Vec<PathBuf> = folders.into_iter()
							.filter( |pathbuf| path2string(pathbuf) .to_lowercase() .contains(&search_query) )
							.collect();

						if tx.send(folders).is_err() {
							break;
						}
					}
				});

			},
		}
	}

}

