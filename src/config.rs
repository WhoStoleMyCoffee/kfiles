// use std::{path::{ PathBuf, Path }, ops::{Deref, DerefMut}, fs::File, io::{self, BufReader}};
use std::path::{ Path, PathBuf };
use std::ops::{ Deref, DerefMut };

use serde::{ Deserialize, Serialize };
use directories::UserDirs;



#[derive(Debug, Serialize, Deserialize)]
pub struct Configs {
	pub scroll_margin: u8,
	pub max_search_stack: usize,
	pub favorites: Vec<PathBuf>,
	pub default_path: PathBuf,
	pub update_rate: u32,
	pub search_ignore_types: String,

	pub max_recent_count: usize,

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
			update_rate: 12,
			search_ignore_types: String::new(),
			max_recent_count: 64,

			folder_color: ( 255, 209, 84 ),
			file_color: ( 206, 217, 214 ),
			special_color: ( 110, 209, 255 ),
			bg_color: ( 35, 47, 54 ),
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




#[derive(Debug)]
pub struct RecentList<I: PartialEq + From<String>> {
	pub max_len: usize,
	list: Vec<I>,
}

impl<I: PartialEq + From<String>> RecentList<I> {
	pub fn new(max_len: usize) -> Self {
		Self {
			max_len,
			list: Vec::new(),
		}
	}

	pub fn push(&mut self, item: I) {
		if let Some(i) = self.list.iter() .position(|element| element == &item) {
			self.list.remove(i);
		}

		self.list.insert(0, item);
		self.list.truncate(self.max_len);
	}
}

impl<I: PartialEq + From<String>> Deref for RecentList<I> {
	type Target = Vec<I>;

	fn deref(&self) -> &Self::Target {
		&self.list
	}
}

impl<I: PartialEq + From<String>> DerefMut for RecentList<I> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.list
	}
}

impl<I: PartialEq + From<String>> AsRef<Vec<I>> for RecentList<I> {
	fn as_ref(&self) -> &Vec<I> {
		&self.list
	}
}


