use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;

use console_engine::screen::Screen;
use console_engine::{
	ConsoleEngine,
	Color, KeyCode, KeyModifiers, KeyEventKind,
};
use console_engine::{events::Event, crossterm::event::KeyEvent};

use filebuffer::*;


const SEARCH_PANEL_MARGIN: (u32, u32) = (4, 2);



macro_rules! search_panel {
	($engine:expr, $mode:expr, $cfg:expr) => {
		SearchPanel::new(
			$engine.get_width() - SEARCH_PANEL_MARGIN.0 * 2,
			$engine.get_height() - SEARCH_PANEL_MARGIN.1 * 2,
			$mode,
			Rc::clone($cfg),
		)
	};
}


fn main() {
	let config_path: &Path = Path::new("configs.toml");
	let cfg: Rc<RefCell<Configs>> = Rc::new(RefCell::new( confy::load_path(config_path) .unwrap() ));

	let mut engine = ConsoleEngine::init_fill(10)
		.unwrap();

	let mut file_buffer = FileBuffer::from_str(
		r"C:\Users\ddxte\Documents",
		Screen::new(engine.get_width() - 2, engine.get_height() - 2),
		Rc::clone(&cfg)
	);
	file_buffer.load_entries();

	let mut search_panel: Option<SearchPanel> = None;

	loop {
		if let BufferState::Exit = file_buffer.state { break; }

		if let Some(panel) = &mut search_panel {
			if panel.is_running() {
				panel.update();
			}
		}

		match engine.poll() {
			Event::Frame => {
				if let Some(panel) = &mut search_panel {
					engine.print_screen( 
						SEARCH_PANEL_MARGIN.0 as i32,
						SEARCH_PANEL_MARGIN.1 as i32,
						panel.draw( (engine.frame_count % 8 > 3) as usize )
					);
				} else {
					engine.clear_screen();
					engine.print_screen(1, 1, file_buffer.draw());
				}

				engine.print_fbg(0, 0, "Press Ctrl-c to exit", Color::DarkGrey, Color::Black);

				let (status_text, fg) = &file_buffer.status_text;
				engine.print_fbg(0, engine.get_height() as i32 - 1, status_text, *fg, Color::Black );

				engine.draw();
			},

			// Exit with Ctrl-c
			Event::Key(KeyEvent {
				code: KeyCode::Char('c'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => { break; },

			// Search folders with Ctrl-Shift-p
			Event::Key(KeyEvent {
				code: KeyCode::Char('P'),
				kind: KeyEventKind::Press,
				modifiers, ..
			}) if modifiers.bits() == KeyModifiers::CONTROL.union(KeyModifiers::SHIFT).bits() => {
				if search_panel.is_some() { continue; }
				search_panel = Some(search_panel!(
					&engine,
					SearchQueryMode::Folders( file_buffer.path.clone() ),
					&cfg
				));
			},

			// Search files with Ctrl-p
			Event::Key(KeyEvent {
				code: KeyCode::Char('p'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				if search_panel.is_some() { continue; }
				search_panel = Some(search_panel!(
					&engine,
					SearchQueryMode::Files( file_buffer.path.clone() ),
					&cfg
				));
			},

			// Add to favorites with Ctrl-f
			Event::Key(KeyEvent {
				code: KeyCode::Char('f'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL, ..
			}) => {
				if search_panel.is_some() { continue; }
				let added: bool = cfg.borrow_mut() .toggle_favorite( file_buffer.path.clone() );
				file_buffer.status_text = (
					if added { String::from("Added path to favorites") } else { String::from("Removed path from favorites") },
					Color::Yellow
				);

				confy::store_path( config_path, cfg.as_ref() ) .unwrap();
			},

			// Open / close favorites with `
			Event::Key(KeyEvent { code: KeyCode::Char('`'), kind: KeyEventKind::Press, ..  }) => {
				// If already open in favorites mode, close
				if let Some(panel) = &search_panel {
					if let SearchQueryMode::Favorites(_) = panel.get_query_mode() {
						search_panel = None;
					}
					continue;
				}

				search_panel = Some(search_panel!(
					&engine,
					SearchQueryMode::Favorites( cfg.borrow().favorites.clone() ),
					&cfg
				));
			},

			event => {
				// Try to update search panel first
				if let Some(panel) = &mut search_panel {
					panel.handle_event(event);
					handle_searchpanel_state(panel, &mut file_buffer);

					if let SearchPanelState::Exit(_) = panel.state {
						search_panel = None;
					}

				// File buffer
				} else if let Event::Key(key_event) = event {
					if let BufferState::Normal | BufferState::QuickSearch(_) = file_buffer.state {
						file_buffer.handle_key_event(key_event);
					}
				}

			},
		}

	}
}



fn handle_searchpanel_state(search_panel: &mut SearchPanel, file_buffer: &mut FileBuffer) {
	match &mut search_panel.state {
		SearchPanelState::Running => {},
		SearchPanelState::Exit(path_maybe) => {
			let path = match path_maybe {
				Some(p) => p,
				None => return,
			};

			if path.is_dir() {
				file_buffer.open_dir(path);
			} else if path.is_file() {
				let file_name = path.file_name() .unwrap();
				let path = path.parent() .unwrap();

				file_buffer.open_dir(path);
				file_buffer.select(file_name);
			}

		},
	}
}




#[cfg(test)]
mod tests {
    use std::{time::Duration, path::{Path, PathBuf}};

    use filebuffer::Configs;

	#[test]
	fn pathcmp() {
		let mut paths = vec![
			PathBuf::from(r"C:\Users\ddxte\Documents\Projects\kfiles"),
			PathBuf::from(r"C:\Users\ddxte"),
			PathBuf::from(r"C:\Users\ddxte\Pictures\art stuff")
		];

		// paths.sort();
		paths.sort_by(|a, b| a.components().count().cmp(&b.components().count()) );

		dbg!(&paths);
	}

	#[test]
	fn confeh() {
		use confy;

		let cfg: Configs = confy::load_path( Path::new("test_configs.toml") ) .unwrap();

		println!("{:?}", cfg);
	}

	#[test]
	fn pathtostring() {
		use std::path::PathBuf;

		let path = PathBuf::from(r"C:\Users\ddxte\Documents\Projects");
		dbg!(&path);

		// Probably best
		let string1: String = String::from( path.to_string_lossy() );
		// let string1: String = path.to_string_lossy().into();
		dbg!(&string1);

		let string2: String = path.into_os_string().into_string() .unwrap();
		dbg!(&string2);
	}

}
