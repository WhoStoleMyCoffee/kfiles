use std::path::{ PathBuf, Path };
use serde::{ Deserialize, Serialize };
use directories::UserDirs;



#[derive(Debug, Serialize, Deserialize)]
pub struct Configs {
	pub scroll_margin: u8,
	pub max_search_stack: usize,
	pub favorites: Vec<PathBuf>,
	pub default_path: PathBuf,
	pub target_fps: u32,
	pub search_ignore_types: String,

	// The best feature: colors
	pub folder_color: (u8, u8, u8),
	pub file_color: (u8, u8, u8),
	pub special_color: (u8, u8, u8),
	pub bg_color: (u8, u8, u8),
}

impl Default for Configs {
	fn default() -> Self {
		let userdir: UserDirs = UserDirs::new().expect("Could not find home directory");
		let home_dir: &Path = userdir.home_dir();

		Self {
			scroll_margin: 4,
			max_search_stack: 512,
			favorites: vec![ PathBuf::from(home_dir) ],
			default_path: PathBuf::from(home_dir),
			target_fps: 10,
			search_ignore_types: String::new(),

			folder_color: ( 105, 250, 255 ),
			file_color: (248, 242, 250),
			special_color: (255, 209, 84),
			bg_color: (21, 17, 23),
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
