// TODO check out dialoguer and indicatif on crates.io

use std::path::{self, PathBuf};

use console_engine::screen::Screen;
use console_engine::{
	ConsoleEngine,
	Color, KeyCode, KeyModifiers, KeyEventKind,
};
use console_engine::forms::{
	Form, FormField, FormStyle, FormValue, FormOptions, Text,
};
use console_engine::{events::Event, crossterm::event::KeyEvent};

use filebuffer::*;


const SEARCH_PANEL_MARGIN: (u32, u32) = (4, 2);



macro_rules! search_panel {
	($screen_w:expr, $screen_h:expr, $path:expr, $mode:expr) => {
		SearchPanel::new(
			$screen_w - SEARCH_PANEL_MARGIN.0 * 2,
			$screen_h - SEARCH_PANEL_MARGIN.1 * 2,
			$path,
			$mode
		)
	};
}


fn main() {
	let mut engine = ConsoleEngine::init_fill(10)
		.unwrap();

	let mut file_buffer = FileBuffer::from_str(
		r"C:\Users\ddxte\Documents",
		Screen::new(engine.get_width() - 2, engine.get_height() - 2)
	);
	file_buffer.load_entries();

	let mut search_panel: Option<SearchPanel> = None;

	loop {
		if let BufferState::Exit = file_buffer.state {
			break;
		}

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

				engine.print(0, 0, "Press Ctrl-c to exit");
				{
					let (status_text, fg) = &file_buffer.status_text;
					engine.print_fbg(0, engine.get_height() as i32 - 1, status_text, *fg, Color::Black );
				}

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
					engine.get_width(),
					engine.get_height(),
					&file_buffer.path,
					SearchQueryMode::Folders
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
					engine.get_width(),
					engine.get_height(),
					&file_buffer.path,
					SearchQueryMode::Files
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
    use std::{time::Duration, path::Path};


	#[test]
	fn test_threads() {
		use std::sync::mpsc;
		use std::thread;

		let (tx, rx) = mpsc::channel::<String>();

		thread::spawn(move || {
			for i in 0..6 {
				thread::sleep( Duration::from_secs(1) );
				let val = format!("Hello {i}");
				tx.send(val) .unwrap();
			}
		});

		loop {
			match rx.try_recv() {
				Ok(res) => {
					println!("Got {res}");
				},
				Err(mpsc::TryRecvError::Empty) => {},
				Err(mpsc::TryRecvError::Disconnected) => {
					break;
				},
			}
		}

		println!("finito!");
	}

	#[test]
	fn test_paths() {
		println!("{}", Path::new(r"c:\windows\").display());
	}

}
