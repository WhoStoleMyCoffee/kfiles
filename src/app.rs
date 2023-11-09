use std::fs::File;
use std::io::Write;
use std::path::{PathBuf, Path};
use std::rc::Rc;
use std::cell::RefCell;

use console_engine::crossterm::ErrorKind;
use console_engine::pixel;
use console_engine::screen::Screen;
use console_engine::{
	ConsoleEngine,
	Color, KeyCode, KeyModifiers, KeyEventKind,
};
use console_engine::{events::Event, crossterm::event::KeyEvent};

use crate::util::{path2string, read_lines};
use crate::config::{Configs, RecentList};
use crate::filebuffer::{FileBuffer, BufferState};
use crate::search::{SearchPanel, SearchQueryMode, SearchPanelState};
use crate::{ APPNAME, SEARCH_PANEL_MARGIN, CONFIG_PATH, get_recent_dirs_path };

const CONTROL_SHIFT: u8 = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT).bits();



fn load_recent_list(path: &Path, max_count: usize) -> RecentList<PathBuf> {
	let mut list: RecentList<PathBuf> = RecentList::new(max_count);

	*list = if let Ok(lines) = read_lines(path) {
		lines.map_while(Result::ok)
			.map(PathBuf::from)
			.filter(|pathbuf| pathbuf.exists())
			.collect()
	} else {
		vec![]
	};

	list
}




pub enum AppError {
	ConfigError(confy::ConfyError),
	EngineError(ErrorKind)
}

impl From<confy::ConfyError> for AppError {
	fn from(value: confy::ConfyError) -> Self {
		Self::ConfigError(value)
	}
}

impl From<ErrorKind> for AppError {
	fn from(value: ErrorKind) -> Self {
		Self::EngineError(value)
	}
}



pub enum RunArg {
	TryFavorite(String), // When running  kfiles -f <path>
	AtPath(PathBuf), // When running  kfiles <path>
	AtDefaultPath, // When running kfiles with no arguments
}

impl RunArg {
	fn get_run_path(&self, cfg: &Rc<RefCell<Configs>>) -> PathBuf {
		match self {
			RunArg::AtPath(p) => p.clone(),
			RunArg::TryFavorite(p) => {
				let favorites = &cfg.borrow().favorites;
				let lower_p: String = p.to_lowercase();

				let res: Option<&PathBuf> = favorites.iter()
					.find(|pathbuf| path2string(pathbuf).to_lowercase() .contains(&lower_p));

				match res {
					Some(p) => p.clone(),
					None => cfg.borrow().default_path.clone(),
				}
			},
			RunArg::AtDefaultPath => cfg.borrow().default_path.clone()
		}
	}
}


// TODO CreateFile, CreateFolder, Delete, Rename
pub enum AppState {
	Running,
	Exit(Option<PathBuf>),
}


pub struct App {
	cfg: Rc<RefCell<Configs>>,
	engine: ConsoleEngine,
	file_buffer: FileBuffer,
	search_panel: Option<SearchPanel>,
	recent_files: RecentList<PathBuf>,
}

impl App {
	pub fn new(run_arg: RunArg) -> Result<Self, AppError> {
		let cfg: Rc<RefCell<Configs>> = Rc::new(RefCell::new( confy::load(APPNAME, Some(CONFIG_PATH))? ));
		let run_path: PathBuf = run_arg.get_run_path(&cfg);
		let engine: ConsoleEngine = ConsoleEngine::init_fill( cfg.borrow().target_fps )?;

		// Initialize file buffer
		let mut file_buffer = FileBuffer::new(
			&run_path,
			Screen::new(engine.get_width() - 2, engine.get_height() - 2),
			Rc::clone(&cfg)
		);
		file_buffer.load_entries();

		let max_recent_count: usize =  cfg.borrow().max_recent_count;
		Ok(Self {
			cfg,
			engine,
			file_buffer,
			search_panel: None,
			recent_files: load_recent_list(&get_recent_dirs_path(), max_recent_count),
		})
	}

	pub fn run(&mut self) -> AppState {
		if let Some(panel) = &mut self.search_panel {
			if panel.is_running() {
				panel.update();
			}
		}

		let bg_color: Color = Color::from(self.cfg.borrow().bg_color);

		match self.engine.poll() {
			Event::Frame => {
				if let Some(panel) = &mut self.search_panel {
					self.engine.print_screen( 
						SEARCH_PANEL_MARGIN.0 as i32,
						SEARCH_PANEL_MARGIN.1 as i32,
						panel.draw( (self.engine.frame_count % 8 > 3) as usize )
					);
				} else {
					self.engine.fill(pixel::pxl_bg(' ', bg_color));
					self.engine.print_screen(1, 1, self.file_buffer.draw());
				}

				self.engine.print_fbg(0, 0, "Ctrl-c to exit, run with --help for help", Color::DarkGrey, bg_color);

				let (status_text, fg) = &self.file_buffer.status_text;
				self.engine.print_fbg(0, self.engine.get_height() as i32 - 1, status_text, *fg, bg_color );

				self.engine.draw();
			},

			Event::Resize(w, h) => {
				self.engine.resize(w as u32, h as u32);
			},

			// Exit with Alt-F4
			Event::Key(KeyEvent {
				code: KeyCode::F(4),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::ALT, ..
			}) => {
				return AppState::Exit(Some( self.file_buffer.path.clone() ));
			},

			// Exit with Ctrl-c
			Event::Key(KeyEvent {
				code: KeyCode::Char('c'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				return AppState::Exit(Some( self.file_buffer.path.clone() ));
			},

			// Reveal in file explorer and close with Ctrl-Shift-e
			Event::Key(KeyEvent {
				code: KeyCode::Char('E'),
				kind: KeyEventKind::Press,
				modifiers, ..
			}) if modifiers.bits() == CONTROL_SHIFT => {
				self.file_buffer.reveal()
					.expect("Failed to reveal current directory");
				self.add_current_to_recent();
				return AppState::Exit(None);
			},

			// Reveal in file explorer with Ctrl-e
			Event::Key(KeyEvent {
				code: KeyCode::Char('e'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				self.file_buffer.reveal()
					.expect("Failed to reveal current directory");
				self.add_current_to_recent();
			},

			// Search folders with Ctrl-Shift-p
			Event::Key(KeyEvent {
				code: KeyCode::Char('P'),
				kind: KeyEventKind::Press,
				modifiers, ..
			}) if modifiers.bits() == CONTROL_SHIFT => {
				if self.search_panel.is_some() { return AppState::Running; }

				let panel: SearchPanel = self.create_search_panel(SearchQueryMode::Folders(self.file_buffer.path.clone()) )
					.set_title("Search Folders")
					.set_color(Color::from(self.cfg.borrow().folder_color));
				self.search_panel = Some(panel);
			},

			// Search files with Ctrl-p
			Event::Key(KeyEvent {
				code: KeyCode::Char('p'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				if self.search_panel.is_some() { return AppState::Running; }
				
				let panel: SearchPanel = self.create_search_panel(SearchQueryMode::Files(self.file_buffer.path.clone()) )
					.set_title("Search Files");
				self.search_panel = Some(panel);
			},

			// Recent files with Ctrl-o
			Event::Key(KeyEvent {
				code: KeyCode::Char('o'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				// If already open in favorites mode, close
				if let Some(panel) = &self.search_panel {
					if let SearchQueryMode::List(_) = panel.get_query_mode() {
						self.search_panel = None;
					}
					return AppState::Running;
				}

				let panel: SearchPanel = self.create_search_panel(SearchQueryMode::List( self.recent_files.clone() ))
					.set_title("Recent")
					.set_color(Color::from(self.cfg.borrow().folder_color));
				self.search_panel = Some(panel);
			},

			// Add to favorites with Ctrl-f
			Event::Key(KeyEvent {
				code: KeyCode::Char('f'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				if self.search_panel.is_some() { return AppState::Running; }

				self.file_buffer.state = BufferState::Normal;
				let added: bool = self.cfg.borrow_mut() .toggle_favorite( self.file_buffer.path.clone() );
				if let Err(err) = confy::store(APPNAME, Some(CONFIG_PATH), self.cfg.as_ref() ) {
					self.file_buffer.status_text = (
						format!("Error saving configs: {}", err),
						Color::Red
					);
				} else {
					self.file_buffer.status_text = (
						if added { String::from("Added path to favorites") } else { String::from("Removed path from favorites") },
						Color::from( self.cfg.borrow().special_color )
					);
				}
			},

			// Open / close favorites with `
			Event::Key(KeyEvent { code: KeyCode::Char('`'), kind: KeyEventKind::Press, ..  }) => {
				// If already open in favorites mode, close
				if let Some(panel) = &self.search_panel {
					if let SearchQueryMode::List(_) = panel.get_query_mode() {
						self.search_panel = None;
					}
					return AppState::Running;
				}

				let panel: SearchPanel = self.create_search_panel(SearchQueryMode::List(self.cfg.borrow().favorites.clone() ))
					.set_title("Favorites")
					.set_color(Color::from(self.cfg.borrow().special_color));
				self.search_panel = Some(panel);
			},

			Event::Mouse(mouse_event) => {
				if let Some(panel) = &mut self.search_panel {
					panel.handle_mouse_event(mouse_event, SEARCH_PANEL_MARGIN.1 as u16);
				} else {
					self.file_buffer.handle_mouse_event(mouse_event);
				}
			},

			Event::Key(key_event) => {
				// Try to update search panel first
				if self.search_panel.is_some() {
					if let Err(err) = self.searchpanel_handle_key_event(key_event) {
						self.file_buffer.status_text = ( format!("Error opening: {}", err), Color::Red );
					}

				// File buffer
				} else {
					self.file_buffer.handle_key_event(key_event);
				}
			},
		}

		AppState::Running
	}

	// I put this stuff in its own function because that would've been a disgusting amount of indentation
	fn searchpanel_handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), String> {
		let search_panel: &mut SearchPanel = unsafe {
			self.search_panel.as_mut() .unwrap_unchecked()
		};

		search_panel.handle_key_event(key_event);

		match &search_panel.state {
			SearchPanelState::Running => {},
			SearchPanelState::Exit(path_maybe) => {
				let path = match path_maybe {
					Some(p) => p,
					None => {
						self.search_panel = None;
						return Ok(())
					},
				};

				if path.is_dir() {
					self.file_buffer.open_dir(path);
				} else if path.is_file() {
					let file_name = path.file_name() .ok_or("Invalid file name")?;
					let path = path.parent() .ok_or("Parent directory not foud")?;

					self.file_buffer.open_dir(path);
					self.file_buffer.select(file_name);
				}

				self.search_panel = None;
				self.add_current_to_recent();
			},
		}

		Ok(())
	}

	fn create_search_panel(&self, mode: SearchQueryMode) -> SearchPanel {
		SearchPanel::new(
			self.engine.get_width() - SEARCH_PANEL_MARGIN.0 * 2,
			self.engine.get_height() - SEARCH_PANEL_MARGIN.1 * 2,
			mode,
			Rc::clone(&self.cfg),
		)
	}

	fn add_to_recent(&mut self, path: PathBuf) {
		self.recent_files.push(path);
	}

	fn add_current_to_recent(&mut self) {
		self.add_to_recent(self.file_buffer.path.clone());
	}

}


impl Drop for App {
	fn drop(&mut self) {
		self.add_current_to_recent();
		let bup = self.recent_files.iter()
			.filter_map(|pathbuf| pathbuf.as_path().to_str() )
			.collect::<Vec<&str>>()
			.join("\n");

		let mut file = File::create( get_recent_dirs_path() ) .unwrap();
		file.write_all( bup.as_bytes() ) .unwrap();
	}
}
