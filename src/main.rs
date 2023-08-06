// TODO check out dialoguer and indicatif on crates.io

use std::fs::{self, DirEntry, FileType};
use std::path::{PathBuf, Path};
use std::process::exit;

use console_engine::KeyEventKind;
use console_engine::screen::Screen;
use console_engine::{
	ConsoleEngine,
	pixel, Color, KeyCode, KeyModifiers,
	rect_style::BorderStyle,
};
use console_engine::forms::{
	Form, FormField, FormStyle, FormValue, FormOptions, Text,
};
use console_engine::{events::Event, crossterm::event::KeyEvent};

use opener;


const SCROLL_MARGIN: u8 = 4;

#[derive(Debug, PartialEq)]
enum BufferMode {
	Normal,
	QuickSearch(String),
	Exit,
}




struct FileBuffer {
	path: PathBuf,
	screen: Screen,
	selected_index: usize,
	scroll: usize,
	entries: Result<Vec<DirEntry>, std::io::Error>,
	mode: BufferMode,
	status_text: (String, Color),
}

impl FileBuffer {
	fn from_str(path: &str, screen: Screen) -> Self {
		FileBuffer {
			path: PathBuf::from(path),
			screen,
			selected_index: 0,
			scroll: 0,
			entries: FileBuffer::get_dirs_at_sorted(path),
			mode: BufferMode::Normal,
			status_text: ( String::from(path), Color::White ),
		}
	}

	fn get_dirs_at_sorted<P>(path: P) -> Result<Vec<DirEntry>, std::io::Error>
	where P: AsRef<Path> {
		let mut d: Vec<DirEntry> = Vec::new();
		let mut f: Vec<DirEntry> = Vec::new();

		for de in fs::read_dir(path)? .flatten() {
			let file_type = de.file_type().unwrap();
			if file_type.is_symlink() { continue; }
			if file_type.is_dir() {
				d.push(de);
			} else {
				f.push(de);
			}
		}

		d.append(&mut f);
		Ok(d)
	}

	fn get_dirs_at<P>(path: P) -> Result<Vec<DirEntry>, std::io::Error>
	where P: AsRef<Path> {
		Ok(fs::read_dir(path)?
			.flatten()
			.filter(|de|
				!de.file_type().unwrap() .is_symlink()
			)
			.collect())
	}

	fn len(&self) -> usize {
        self.entries.as_ref().unwrap() .len()
	}

	fn get_path_display(&self) -> String {
		self.path.display().to_string()
	}

	fn reload_entries(&mut self) {
		self.entries = FileBuffer::get_dirs_at_sorted(&self.path);
		self.selected_index = 0;
		self.scroll = 0;
	}

	fn update_status_text(&mut self) {
		match &self.mode {
			BufferMode::Normal => {
				self.status_text = ( self.get_path_display(), Color::White );
			},
			BufferMode::QuickSearch(pattern) => {
				self.status_text = ( format!("Searching for: {}", pattern), Color::Yellow );
			},
			_ => {},
		}
	}

	fn open_selected(&mut self) -> Option<()> {
		let dirs: &Vec<DirEntry> = self.entries.as_ref() .ok()?;
		let de: &DirEntry = dirs.get(self.selected_index)?;
		let file_type: FileType = de.file_type() .ok()?;

		if file_type.is_file() {
			if opener::open( de.path() ).is_err() {
				self.status_text = ( String::from("Could not open file. Revealing in file explorer instead"), Color::Red );
				let _ = opener::reveal( de.path() );
			}
		} else if file_type.is_dir() {
			let file_name = de.file_name();
			self.path.push( file_name );
			self.reload_entries();
		}

		Some(())
	}

	fn handle_key_event(&mut self, event: KeyEvent) {
		if let BufferMode::QuickSearch(pattern) = &mut self.mode {
			// Handle input
			match event {
				// Exit quick search
				KeyEvent { code: KeyCode::Esc, kind: KeyEventKind::Press, .. } => {
					self.mode = BufferMode::Normal;
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
			for (i, de) in self.entries.as_ref().unwrap() .iter().enumerate() {
				let file_name = de.file_name().into_string().unwrap() .to_lowercase();
				if file_name.starts_with( pattern_lowercase.as_str() ) {
					self.selected_index = i;
					break;
				}
			}

			self.update_scroll();
			self.update_status_text();
			return;
		}

		let len: usize = self.len();
		match event {
			// Move cursor up
			KeyEvent { code: KeyCode::Char('k'), kind: KeyEventKind::Press, .. } => {
				if len == 0 { return; }
				self.selected_index = self.selected_index.checked_sub(1) .unwrap_or(len - 1);
				self.update_scroll();
			},

			// Move cursor down
			KeyEvent { code: KeyCode::Char('j'), kind: KeyEventKind::Press, .. } => {
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
					self.reload_entries();
				}
			},

			// Jump to start
			KeyEvent { code: KeyCode::Char('g'), kind: KeyEventKind::Press, .. } => {
				if len == 0 { return; }
				self.selected_index = 0;
				self.update_scroll();
			}

			// Jump to end
			KeyEvent { code: KeyCode::Char('G'), kind: KeyEventKind::Press, .. } => {
				if len == 0 { return; }
				self.selected_index = len - 1;
				self.update_scroll();
			}

			// Start quick search
			KeyEvent { code: KeyCode::Char('/'), kind : KeyEventKind::Press, .. } => {
				if len == 0 { return; }
				self.mode = BufferMode::QuickSearch( String::new() );
			},

			// Reveal in file explorer
			KeyEvent {
				code: KeyCode::Char('e'),
				kind: KeyEventKind::Press,
				modifiers: KeyModifiers::CONTROL,
				.. 
			} => {
				let _ = opener::open( &self.path );
				self.mode = BufferMode::Exit;
			},

			_ => {},
		}

	}

	fn update_scroll(&mut self) {
		let u_scroll_margin = SCROLL_MARGIN as usize;
		let max_bound = self.screen.get_height() as usize - u_scroll_margin;

		if self.selected_index < self.scroll + u_scroll_margin {
			self.scroll = self.selected_index.max(u_scroll_margin) - u_scroll_margin;
		} else if self.selected_index > self.scroll + max_bound {
            self.scroll = (self.selected_index - max_bound)
				.clamp(u_scroll_margin, self.len() - u_scroll_margin);
        }
	}

	fn draw(&mut self) -> &Screen {
		// Get dir entries or display error
		let dirs: &Vec<DirEntry> = match self.entries.as_ref() {
			Ok(dirs) => dirs,
			Err(err) => {
				self.screen.print_fbg( 0, 0,
					format!("Could not load path: \"{}\"", self.path.display()) .as_str(),
					Color::Red, Color::Black);
				self.screen.print_fbg( 0, 1,
					format!("Error: {}", err) .as_str(),
					Color::Red, Color::Black);
				return &self.screen
			},
		};

        let screen_selected_idx: isize = self.selected_index as isize - self.scroll as isize;
		self.screen.clear();
		self.screen.h_line(0, screen_selected_idx as i32, self.screen.get_width() as i32, pixel::pxl_bg(' ', Color::DarkGrey));

		for (i, dir) in dirs.iter() .skip(self.scroll) .enumerate() {
			if i as u32 >= self.screen.get_height() { break; }
			let mut file_name: String = dir.file_name().into_string() .unwrap();
			let file_type: FileType = dir.file_type() .unwrap();

			let bg: Color = if i == screen_selected_idx as usize { Color::DarkGrey } else { Color::Black };
			let fg: Color = if file_type.is_dir() {
				file_name.push('/');
				Color::Cyan
			} else {
				Color::White
			};

			self.screen.print_fbg(0, i as i32, file_name.as_str(), fg, bg );
		}
		&self.screen
	}
}




fn main() {
	let mut engine = ConsoleEngine::init_fill(10)
		.unwrap();

	let mut file_buffer = FileBuffer::from_str(r"C:\Users\ddxte\Documents",
		Screen::new(engine.get_width() - 2, engine.get_height() - 2));

	loop {
		if file_buffer.mode == BufferMode::Exit { break; }

		match engine.poll() {
			Event::Frame => {
				engine.clear_screen();
				
				engine.print_screen(1, 1, file_buffer.draw());

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
				modifiers: KeyModifiers::CONTROL,
				kind: _, state: _,
			}) => { break; },

			Event::Key(key_event) => {
				if file_buffer.entries.is_ok() {
					file_buffer.handle_key_event(key_event);
				}
			},

			_ => {},
		}
	}
}





#[cfg(test)]
mod tests {
	use console_engine::{events::Event, crossterm::event::KeyEvent};

use super::*;

	#[test]
	fn test_form() {
		let mut engine = ConsoleEngine::init_fill(5) .unwrap();

		let theme = FormStyle { ..Default::default() };
		let mut form = Form::new(
			12,
			6,
			FormOptions { style: theme, ..Default::default() }
		);
		form.build_field::<Text>(
			"last_name",
			FormOptions { style: theme, label: Some("Last Name:"), ..Default::default() }
		);

		form.set_active(true);

		loop {
			match engine.poll() {
				Event::Frame => {
					engine.clear_screen();
					engine.print_screen(1, 1, form.draw( (engine.frame_count % 8 > 3) as usize ));
					engine.draw();
				},

				// Exit with Ctrl-c
				Event::Key(KeyEvent {
					code: KeyCode::Char('c'),
					modifiers: KeyModifiers::CONTROL,
					..
				}) => { break; }

				event => {
					form.handle_event(event);
					if form.is_finished() { break; }
				},
			}
		}

		drop(engine); // ok byyeeeeeee
		
		if !form.is_finished() {
			println!("Form cancelled");
			return;
		}

		let mut last_name = String::new();

		if let Ok(FormValue::String(name)) = form.get_validated_field_output("last_name") {
			last_name = name;
		}

		println!("Hello, {}!", last_name);

	}

}
