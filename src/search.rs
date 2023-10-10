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


use crate::config::Configs;
use crate::util::*;

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
// Beautiful
