// TODO check out dialoguer and indicatif on crates.io

use std::fs::{self, DirEntry, FileType};
use std::path::{PathBuf, Path};

use console_engine::screen::Screen;
use console_engine::{
	ConsoleEngine,
	pixel, Color, KeyCode, KeyModifiers
};

use opener;


const SCROLL_MARGIN: u8 = 4;


enum CommandType {
	CursorUp,
	CursorDown,
	CursorJump(isize),
	Open,
	GoBack,
}


struct InputBuffer(String);

impl InputBuffer {
	fn new() -> Self {
		Self( String::new() )
	}

	fn handle_input(&mut self, engine: &ConsoleEngine) -> Option<CommandType> {
		if engine.is_key_held(KeyCode::Char('j')) || engine.is_key_held(KeyCode::Down) {
			return Some(CommandType::CursorDown);
		}

		if engine.is_key_held(KeyCode::Char('k')) || engine.is_key_held(KeyCode::Up) {
			return Some(CommandType::CursorUp);
		}

		if engine.is_key_pressed(KeyCode::Enter) {
			return Some(CommandType::Open);
		}

		if engine.is_key_pressed(KeyCode::Char('-')) {
			return Some(CommandType::GoBack);
		}

		if engine.is_key_pressed(KeyCode::Char('g')) {
			return Some(CommandType::CursorJump(0));
		}

		if engine.is_key_pressed(KeyCode::Char('G')) {
			return Some(CommandType::CursorJump(-1));
		}

		None
	}
}



struct FileBuffer {
	path: PathBuf,
	screen: Screen,
	input_buffer: InputBuffer,
	selected_index: usize,
	scroll: usize,
	entries: Result<Vec<DirEntry>, std::io::Error>,
}

impl FileBuffer {
	fn from_str(path: &str, screen: Screen) -> Self {
		FileBuffer {
			path: PathBuf::from(path),
			screen,
			input_buffer: InputBuffer::new(),
			selected_index: 0,
			scroll: 0,
			entries: FileBuffer::get_dirs_at_sorted(path),
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

	fn update(&mut self, engine: &ConsoleEngine) {
		let cmd: CommandType = match self.input_buffer.handle_input(engine) {
			Some(cmd) => cmd,
			None => return,
		};

		let len: usize = self.len();
		match cmd {
			CommandType::CursorUp => {
				self.selected_index = self.selected_index.checked_sub(1) .unwrap_or(len - 1);
				self.update_scroll();
			},

			CommandType::CursorDown => {
				self.selected_index = (self.selected_index + 1) % len;
				self.update_scroll();
			},

			CommandType::Open => {
				let dirs: &Vec<DirEntry> = self.entries.as_ref() .unwrap();
				let de: &DirEntry = dirs.get(self.selected_index) .unwrap();
				let file_type: FileType = de.file_type().unwrap();

				if file_type.is_file() {
					let _ = opener::open( de.path() );
				} else if file_type.is_dir() {
					let file_name = de.file_name();
					self.path.push( file_name );
					self.reload_entries();
				}
			},

			CommandType::GoBack => {
				let went_back: bool = self.path.pop();
				if went_back {
					self.reload_entries();
				}
			},

			CommandType::CursorJump(idx) => {
				self.selected_index = if idx < 0 {
					(len as isize + idx) as usize
				} else {
					idx as usize
				};
				self.update_scroll();
			},
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
		engine.wait_frame();
		engine.clear_screen();

		if engine.is_key_pressed_with_modifier(KeyCode::Char('c'), KeyModifiers::CONTROL, console_engine::KeyEventKind::Press) {
			break;
		}

        if file_buffer.entries.is_ok() {
            file_buffer.update(&engine);
        }
		engine.print_screen(1, 1, file_buffer.draw() );

		engine.print(0, 0, "Press Ctrl-c to exit");
		engine.print(0, engine.get_height() as i32 - 1,
			file_buffer.get_path_display().as_str());
		engine.draw();
	}
}



#[cfg(test)]
mod tests {
	use std::fs;
	use std::path::PathBuf;

	#[test]
	fn display_path() {
		let path = PathBuf::from("C:/Users/ddxte/Documents");
		let stuff = fs::read_dir(&path) .unwrap();

		for dir in stuff {
			println!("Name: {}", dir.unwrap() .path() .display());
		}
	}
}
