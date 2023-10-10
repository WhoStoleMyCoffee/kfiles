use std::path::PathBuf;
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

use crate::util::path2string;
use crate::config::Configs;
use crate::filebuffer::{FileBuffer, BufferState};
use crate::search::{SearchPanel, SearchQueryMode, SearchPanelState};
use crate::{ APPNAME, SEARCH_PANEL_MARGIN, CONFIG_PATH };

const CONTROL_SHIFT: u8 = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT).bits();


// Yes.
fn new_search_panel(app: &App, mode: SearchQueryMode) -> SearchPanel {
	SearchPanel::new(
		app.engine.get_width() - SEARCH_PANEL_MARGIN.0 * 2,
		app.engine.get_height() - SEARCH_PANEL_MARGIN.1 * 2,
		mode,
		Rc::clone(&app.cfg),
	)
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
}

impl App {
	pub fn new(run_arg: RunArg) -> Result<Self, AppError> {
		let cfg: Rc<RefCell<Configs>> = Rc::new(RefCell::new( confy::load(APPNAME, Some(CONFIG_PATH))? ));

		let run_path: PathBuf = match run_arg {
			RunArg::AtPath(p) => p,
			RunArg::TryFavorite(p) => {
				let favorites = &cfg.borrow().favorites;
				let lower_p: String = p.to_lowercase();

				let res: Option<&PathBuf> = favorites.iter()
					.find(|pathbuf| path2string(pathbuf).to_lowercase() .contains(&lower_p) );

				match res {
					Some(p) => p.clone(),
					None => cfg.borrow().default_path.clone(),
				}
			},
			RunArg::AtDefaultPath => cfg.borrow().default_path.clone(),
		};

		let engine: ConsoleEngine = ConsoleEngine::init_fill( cfg.borrow().target_fps )?;

		let mut file_buffer = FileBuffer::new(
			&run_path,
			Screen::new(engine.get_width() - 2, engine.get_height() - 2),
			Rc::clone(&cfg)
		);
		file_buffer.load_entries();

		Ok(Self {
			cfg,
			engine,
			file_buffer,
			search_panel: None,
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
			},

			// Search folders with Ctrl-Shift-p
			Event::Key(KeyEvent {
				code: KeyCode::Char('P'),
				kind: KeyEventKind::Press,
				modifiers, ..
			}) if modifiers.bits() == CONTROL_SHIFT => {
				if self.search_panel.is_some() { return AppState::Running; }
				self.search_panel = Some(new_search_panel( self, SearchQueryMode::Folders(self.file_buffer.path.clone()) ));
			},

			// Search files with Ctrl-p
			Event::Key(KeyEvent {
				code: KeyCode::Char('p'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				if self.search_panel.is_some() { return AppState::Running; }
				self.search_panel = Some(new_search_panel( self, SearchQueryMode::Files(self.file_buffer.path.clone()) ));
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
					if let SearchQueryMode::Favorites(_) = panel.get_query_mode() {
						self.search_panel = None;
					}
					return AppState::Running;
				}

				self.search_panel = Some(new_search_panel( self, SearchQueryMode::Favorites( self.cfg.borrow().favorites.clone() ) ));
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

			},
		}

		Ok(())
	}

}