
use std::ffi::OsStr;
use std::fs;
use std::path::{ PathBuf, Path };
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

use opener;


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






#[derive(Debug)]
pub enum BufferState {
	Normal,
	QuickSearch(String),
	Error(std::io::Error),
	Exit,
}

const SCROLL_MARGIN: u8 = 4;

pub struct FileBuffer {
	pub path: PathBuf,
	screen: Screen,
	selected_index: usize,
	scroll: usize,
	pub entries: Vec<PathBuf>,
	pub state: BufferState,
	pub status_text: (String, Color),
}

impl FileBuffer {
	pub fn from_str(path: &str, screen: Screen) -> Self {
		FileBuffer {
			path: PathBuf::from(path),
			screen,
			selected_index: 0,
			scroll: 0,
			entries: vec![],
			state: BufferState::Normal,
			status_text: ( String::from(path), Color::White ),
		}
	}

	fn get_path_display(&self) -> String {
		self.path.display().to_string()
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
				self.status_text = ( self.get_path_display(), Color::White );
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
				if self.entries.len() == 0 { return; }
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
		let u_scroll_margin: usize = SCROLL_MARGIN as usize;
		let max_bound: usize = self.screen.get_height() as usize - u_scroll_margin;

		if self.selected_index < self.scroll + u_scroll_margin {
			self.scroll = self.selected_index.max(u_scroll_margin) - u_scroll_margin;
		} else if self.selected_index > self.scroll + max_bound {
            self.scroll = (self.selected_index - max_bound)
				.clamp(u_scroll_margin, self.entries.len() - u_scroll_margin);
        }
	}

	pub fn draw(&mut self) -> &Screen {
		// Get dir entries or display error
		if let BufferState::Error(err) = &self.state {
			self.screen.print_fbg( 0, 0,
				format!("Could not load path: \"{}\"", self.path.display()) .as_str(),
				Color::Red, Color::Black);
			self.screen.print_fbg( 0, 1,
				format!("Error: {}", err) .as_str(),
				Color::Red, Color::Black);
			return &self.screen;
		}

        let screen_selected_idx: isize = self.selected_index as isize - self.scroll as isize;
		self.screen.clear();
		self.screen.h_line(
			0, screen_selected_idx as i32,
			self.screen.get_width() as i32,
			pixel::pxl_bg(' ', Color::DarkGrey)
		);

		for (i, path) in self.entries.iter() .skip(self.scroll) .enumerate() {
			if i as u32 >= self.screen.get_height() { break; }
			let mut file_name: String = path.file_name().unwrap() .to_str().unwrap() .to_string();

			let bg: Color = if i == screen_selected_idx as usize { Color::DarkGrey } else { Color::Black };
			let fg: Color = if path.is_dir() {
				file_name.push('/');
				Color::Cyan
			} else {
				Color::White
			};

			self.screen.print_fbg(0, i as i32, &file_name, fg, bg );
		}
		&self.screen
	}
}







const MAX_SEARCH_RESULTS: usize = 30;


#[derive(Debug, PartialEq)]
pub enum SearchPanelState {
	Running,
	Exit( Option<PathBuf> ),
}



pub struct SearchPanel {
	path: PathBuf,
	screen: Screen,
	form: Form,
	selected_index: usize,
	query: SearchQuery,
	pub state: SearchPanelState,
}

impl SearchPanel {
	pub fn new<P>(width: u32, height: u32, path: &P, mode: SearchQueryMode) -> Self
	where P: AsRef<OsStr> {
		Self {
			path: PathBuf::from(path),
			screen: Screen::new(width, height),
			form: SearchPanel::build_form(width - 2),
			selected_index: 0,
			query: SearchQuery::new(path, mode),
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
			Some(FormValue::String(value)) => value.replace("/", r"\"),
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
			let file_name = path.strip_prefix(&self.path) .unwrap()
				.display()
				.to_string()
				.replace(r"\", "/");

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
			SearchQueryMode::Files => "Search Files",
			SearchQueryMode::Folders => "Search Folders",
		};
		self.screen.print_fbg(4, 0, text, Color::Yellow, Color::Black);

		&self.screen
	}
}






#[derive(Debug, Clone, Copy)]
pub enum SearchQueryMode {
	Files,
	Folders,
}

const MAX_SEARCH_STACK: usize = 255;

struct SearchQuery {
	path: PathBuf,
	query: String,
	mode: SearchQueryMode,
	results: Vec<PathBuf>,
	receiver: Option< Receiver<Vec<PathBuf>> >,
}

impl SearchQuery {
	fn new<P>(path: &P, mode: SearchQueryMode) -> Self
	where P: AsRef<OsStr> {
		let pathbuf = PathBuf::from(path);
		Self {
			path: pathbuf.clone(),
			query: String::new(),
			mode,
			results: SearchQuery::list(&pathbuf, mode, MAX_SEARCH_RESULTS),
			receiver: None,
		}
	}

	fn list(path: &PathBuf, mode: SearchQueryMode, max_result_count: usize) -> Vec<PathBuf> {
		match mode {
			SearchQueryMode::Files => {
				let mut results: Vec<PathBuf> = Vec::new();
				let mut stack: Vec<PathBuf> = vec![ path.clone() ];

				while results.len() < max_result_count {
					let search_path = if let Some(p) = stack.pop() { p } else { break; };
					let (mut files, folders) = get_files_and_folders_at(search_path)
						.unwrap();
					results.append(&mut files);
					stack.append(&mut folders.iter()
						.take( MAX_SEARCH_STACK - stack.len() )
						.cloned()
						.collect()
					);
				}

				results.truncate(max_result_count);
				results
			},

			SearchQueryMode::Folders => {
				let mut results: Vec<PathBuf> = get_folders_at(path, max_result_count) .unwrap();
				let mut idx: usize = 0;

				while results.len() < max_result_count {
					let search_path = if let Some(path) = results.get(idx) { path } else { break; };
					let mut folders = get_folders_at(search_path, max_result_count - results.len())
						.unwrap();
					results.append(&mut folders);
					idx += 1;
				}

				results.truncate(max_result_count);
				results
			},

		}
	}

	fn update(&mut self) {
		let rx = match &mut self.receiver {
			Some(rx) => rx,
			None => return,
		};

		for received in rx.try_iter() {
			for pathbuf in received.iter() .take(MAX_SEARCH_RESULTS - self.results.len()) {
				self.results.push( pathbuf.clone() );
			}
		}

		if self.results.len() >= MAX_SEARCH_RESULTS {
			self.receiver = None;
		}

	}

	fn search(&mut self, query: String) {
		if query == self.query { return; }
		self.query = query;

		if self.query.is_empty() {
			self.results = SearchQuery::list(&self.path, self.mode, MAX_SEARCH_RESULTS);
			return;
		}

		self.results.clear();
		let (tx, rx) = mpsc::channel::< Vec<PathBuf> >();
		let path = self.path.clone();
		let search_query = self.query.to_lowercase();

		match self.mode {
			SearchQueryMode::Files => {
				thread::spawn(move || {
					let mut stack: Vec<PathBuf> = vec![path];

					loop {
						let search_path: PathBuf = if let Some(p) = stack.pop() { p } else { break; };
						let (files, folders) = match get_files_and_folders_at(search_path) {
							Ok(pair) => pair,
							Err(_) => continue,
						};
						stack.append(&mut folders.into_iter()
							.take( MAX_SEARCH_STACK - stack.len() )
							.collect());

						let files: Vec<PathBuf> = files.into_iter()
							.filter( |pathbuf| pathbuf.display().to_string().to_lowercase() .contains(&search_query) )
							.collect();

						if tx.send(files).is_err() {
							break;
						}
					}
				});

			},

			SearchQueryMode::Folders => {
				thread::spawn(move || {
					let mut stack: Vec<PathBuf> = vec![path];

					loop {
						let search_path: PathBuf = if let Some(p) = stack.pop() { p } else { break; };
						let folders = match get_all_folders_at(search_path) {
							Ok(v) => v,
							Err(_) => continue,
						};
						stack.append(&mut folders.iter()
							.take(MAX_SEARCH_STACK - stack.len())
							.cloned()
							.collect());
						
						let folders: Vec<PathBuf> = folders.into_iter()
							.filter( |pathbuf| pathbuf.display().to_string().to_lowercase() .contains(&search_query) )
							.collect();

						if tx.send(folders).is_err() {
							break;
						}
					}
				});

			},
		}

		self.receiver = Some(rx);
	}

}

