use std::path::{PathBuf, Path};
use std::rc::Rc;
use std::cell::RefCell;
use std::thread;
use std::time::Duration;

use console_engine::pixel;
use console_engine::screen::Screen;
use console_engine::{
	ConsoleEngine,
	Color, KeyCode, KeyModifiers, KeyEventKind,
};
use console_engine::{events::Event, crossterm::event::KeyEvent};

use crate::config::{Configs, RecentList, FavoritesList};
use crate::filebuffer::FileBuffer;
use crate::search::{SearchPanel, SearchQueryMode, SearchPanelState};
use crate::{ APPNAME, SEARCH_PANEL_MARGIN, CONFIG_PATH, get_recent_dirs_path, Cfg, get_favorites_list_path };
use crate::try_err;

use crate::{ CONTROL_SHIFT, AppError };



pub enum AppState {
	Running,
	Exit(Option<PathBuf>),
}


pub struct App {
	cfg: Cfg,
	engine: ConsoleEngine,
	file_buffer: FileBuffer,
	search_panel: Option<SearchPanel>,
	recent_dirs: RecentList,
	favorites: FavoritesList,
}

impl App {
	pub fn new(at_path: &Path, cfg: Configs) -> Result<Self, AppError> {
		let engine: ConsoleEngine = ConsoleEngine::init_fill( cfg.update_rate )?;

		let cfg: Cfg = Rc::new(RefCell::new( cfg ));

		// Initialize file buffer
		let mut file_buffer = FileBuffer::new(
			at_path,
			Screen::new(engine.get_width() - 2, engine.get_height() - 2),
			Rc::clone(&cfg),
		);

	 	try_err!( file_buffer.load_entries() => file_buffer );

		let max_recent_count: usize = cfg.borrow().max_recent_count;
		Ok(Self {
			cfg,
			engine,
			file_buffer,
			search_panel: None,
			recent_dirs: RecentList::load( &get_recent_dirs_path()?, max_recent_count )
				.unwrap_or_else(|_| RecentList::new(max_recent_count) ),
			favorites: FavoritesList::load( &get_favorites_list_path()? ).unwrap_or_default()
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
				self.file_buffer.status_line .draw(&mut self.engine, bg_color);

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

				let panel: SearchPanel = self.create_search_panel(SearchQueryMode::List( self.recent_dirs.clone() ))
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

				self.file_buffer.status_line.normal();
				let added: bool = self.favorites.toggle( self.file_buffer.path.clone() );

				if let Err(err) = confy::store(APPNAME, Some(CONFIG_PATH), self.cfg.as_ref() ) {
					self.file_buffer.status_line.set_text( &format!("Error saving configs: {}", err) )
						.set_color(Color::Red);
				} else {
					self.file_buffer.status_line.set_text( if added { "Added path to favorites" } else { "Removed path from favorites" } )
						.set_color_as(self.cfg.borrow().special_color);
				}
			},

			// Open / close favorites with `
			Event::Key(KeyEvent { code: KeyCode::Char('`') | KeyCode::Tab, kind: KeyEventKind::Press, ..  }) => {
				// If already open in favorites mode, close
				if let Some(panel) = &self.search_panel {
					if let SearchQueryMode::List(_) = panel.get_query_mode() {
						self.search_panel = None;
					}
					return AppState::Running;
				}

				let panel: SearchPanel = self.create_search_panel(SearchQueryMode::List( self.favorites.clone() ))
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
						self.file_buffer.status_line.error( err.as_str().into(), Some("Error opening: \n" ));
						// self.file_buffer.status_line.set_text( &format!("Error opening: {}", err) )
							// .set_color(Color::Red);
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
		let search_panel: &mut SearchPanel = self.search_panel.as_mut() .expect("SearchPanel not set");

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

	pub fn add_to_recent(&mut self, path: PathBuf) {
		self.recent_dirs.push(path);
	}

	pub fn add_current_to_recent(&mut self) {
		self.add_to_recent(self.file_buffer.path.clone());
	}
}


impl Drop for App {
	fn drop(&mut self) {
		let recent_res = get_recent_dirs_path()
			.and_then(|path| {
				self.add_current_to_recent();
				self.recent_dirs.save(&path) .map_err(AppError::from)
			});

		let favorites_res = get_favorites_list_path()
			.and_then(|path| self.favorites.save(&path) .map_err(AppError::from));

		if recent_res.is_ok() && favorites_res.is_ok() { return; }

		let bg_color: Color = Color::from(self.cfg.borrow().bg_color);
		let text_color: Color = Color::from(self.cfg.borrow().file_color);
		let mut y: i32 = 0;

		self.engine.fill(pixel::pxl_bg(' ', bg_color));

		if let Err(err) = recent_res {
			self.engine.print_fbg(0, y,
				&format!("Error while saving recent directories list:\n {err}"),
				Color::Red, bg_color);
			y += 3;
		}

		if let Err(err) = favorites_res {
			self.engine.print_fbg(0, y,
				&format!("Error while saving favorites list:\n {err}"),
				Color::Red, bg_color);
			y += 3;
		}

		self.engine.print_fbg(0, y, "Press Enter to continue...", text_color, bg_color);
		self.engine.draw();

        let exit_codes: [KeyCode; 3] = [
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char(' '),
        ];

		while !exit_codes.iter() .any(|c| self.engine.is_key_pressed(c.clone()) ) {
			self.engine.wait_frame();
		}

	}
}


