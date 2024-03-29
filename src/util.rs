use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Child;


pub fn start_terminal(at_dir: Option<&Path>) -> std::io::Result<Child> {
    use std::process::{ Command, Stdio };

    let mut command: Command;
    if cfg!(target_os = "windows") {
        command = Command::new("cmd");
        command.args([ "/C", "start" ]);

    } else {
        todo!("KFiles start_terminal not yet implemented for OS' other than windows")
    }

    if let Some(dir) = at_dir {
        command.current_dir(dir);
    }

    command.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}



// Idk if there's any builtin methods for this
#[inline]
pub fn path2string<P>(path: P) -> String
where
    P: AsRef<Path>,
{
    path.as_ref().display().to_string()
    // String::from(path.as_ref().to_string_lossy())
}

#[inline]
pub fn file_name(path: &Path) -> String {
    path2string(path.file_name().unwrap_or_default())
}

// Get files & folders and have folders come before files (ofc, alphabetically sorted)
pub fn get_at_sorted<P>(path: P) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    let (mut folders, mut files): (Vec<PathBuf>, Vec<PathBuf>) = fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|path| !path.is_symlink())
        .partition(|path| path.is_dir());

    folders.append(&mut files);
    Ok(folders)
}

pub fn get_files_at<P>(path: P, limit: usize) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| pathbuf.is_file())
        .take(limit)
        .collect())
}

pub fn get_folders_at<P>(path: P, limit: usize) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| pathbuf.is_dir())
        .take(limit)
        .collect())
}

// get_folders_at() but without limits
pub fn get_all_folders_at<P>(path: P) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| pathbuf.is_dir())
        .collect())
}

// get_files_at() but without limits
pub fn get_all_files_at<P>(path: P) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| pathbuf.is_file())
        .collect())
}

pub fn get_all_files_at_recursive<P>(path: P) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    let (mut results, queue) = get_files_and_folders_at(path)?;
    let mut queue = VecDeque::from(queue);

    while let Some(search_path) = queue.pop_front() {
        let Ok((mut files, folders)) = get_files_and_folders_at(search_path) else {
            continue;
        };

        results.append(&mut files);
        queue.append(&mut folders.into());
    }

    Ok(results)
}

pub fn get_all_at<P>(path: P) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| !pathbuf.is_symlink())
        .collect())
}

// Get files & folders, separated into tuples
// I don't know how it took me so long to discover Iterator.partition(). I almost implemented macro  segregate!(vec, condition)  no joke
pub fn get_files_and_folders_at<P>(path: P) -> io::Result<(Vec<PathBuf>, Vec<PathBuf>)>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|path| !path.is_symlink())
        .partition(|path| path.is_file()))
}

pub fn read_lines<P>(path: P) -> io::Result<io::Lines<BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(path)?;
    Ok(BufReader::new(file).lines())
}



pub fn str_match_cost(needle: &str, haystack: &str) -> Option<usize> {
    let mut cost: usize = 0;
    let mut h_chars = haystack.chars();
    let mut is_first_char: bool = true;
    // Find indices of `needle` chars inside haystack
    for nch in needle.chars() {
        let Some(i) = h_chars.position(|ch| ch.eq_ignore_ascii_case(&nch)) else {
            return None;
        };

        // if is_first_char { is_first_char = false; } else { cost += i; }
        cost += i * (!is_first_char as usize);
        is_first_char = false;
    }
    Some(cost)
}



pub trait TruncateBack {
    type Output;
    fn trunc_back(self, new_len: usize) -> Self::Output;
}


impl TruncateBack for String {
    type Output = String;

    fn trunc_back(self, new_len: usize) -> Self::Output {
        let t: usize = self.len().saturating_sub(new_len);
        self.chars()
            .skip(t)
            .collect::<String>()
    }
}


/// Literally `std::ops::Neg`
pub trait Invert {
    type Output;
    fn inv(self) -> Self::Output;
}

impl Invert for (u8, u8, u8) {
    type Output = Self;
    #[inline]
    fn inv(self) -> Self::Output {
        (255 - self.0, 255 - self.1, 255 - self.2)
    }
}

// Nah maybe someday
// impl Invert for Color {
//     type Output = Self;
//     fn inv(self) -> Self::Output {
//         todo!()
//     }
// }

