use std::path::{Path, PathBuf};
use std::fs::{ self, File };
use std::io::{ self, BufReader, BufRead };

// Idk if there's any builtin methods for this
pub fn path2string<P>(path: P) -> String
where P: AsRef<Path> {
	String::from( path.as_ref() .to_string_lossy() )
}

pub fn file_name(pathbuf: &Path) -> String {
    path2string(pathbuf.file_name().unwrap_or_default())
}

// Get files & folders and have folders come before files (ofc, alphabetically sorted)
pub fn get_at_sorted<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	let (mut folders, mut files): (Vec<PathBuf>, Vec<PathBuf>) = fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|path| !path.is_symlink())
		.partition(|path| path.is_dir());

	folders.append(&mut files);
	Ok(folders)
}

pub fn get_files_at<P>(path: P, limit: usize) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf| pathbuf.is_file())
		.take(limit)
		.collect())
}

pub fn get_folders_at<P>(path: P, limit: usize) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf| pathbuf.is_dir())
		.take(limit)
		.collect())
}

// get_folders_at() but without limits
pub fn get_all_folders_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf| pathbuf.is_dir())
		.collect())
}

// Bro just use io::Result
pub fn get_all_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|pathbuf| !pathbuf.is_symlink())
		.collect())
}

// Get files & folders, separated into tuples
// I don't know how it took me so long to discover Iterator.partition(). I almost implemented macro  segregate!(vec, condition)  no joke
pub fn get_files_and_folders_at<P>(path: P) -> Result< (Vec<PathBuf>, Vec<PathBuf>), std::io::Error >
where P: AsRef<Path> {
	Ok(fs::read_dir(path)?
		.flatten()
		.map(|de| de.path())
		.filter(|path| !path.is_symlink())
		.partition(|path| path.is_file())
	)
}


pub fn read_lines<P>(path: P) -> io::Result<io::Lines<BufReader<File>>>
where P: AsRef<Path> {
	let file = File::open(path)?;
	Ok(BufReader::new(file).lines())
}
