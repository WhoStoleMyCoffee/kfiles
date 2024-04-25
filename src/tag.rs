use std::fs::{create_dir_all, read_dir, remove_file, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml;

use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Error)]
pub enum AddEntryError {
    #[error("path '{}' does not exist", .0.display())]
    NonexistentPath(PathBuf),
    #[error("already exists")]
    AlreadyContained,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("could not get tag name from file '{}'", .0.display())]
    InvalidName(PathBuf),
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error("failed to parse: {0}")]
    ParseError(#[from] toml::de::Error),
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("no ID set")]
    NoID,
    #[error("failed to parse: {0}")]
    ParseError(#[from] toml::ser::Error),
    #[error(transparent)]
    IO(#[from] io::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    #[serde(skip)]
    pub id: String,

    entries: Vec<PathBuf>,

    /// All tags that are tagged with this tag
    /// E.g. tag `"pictures"` could have `subtags = [ "trip", "cats" ]`
    /// Then, searching for tag `"pictures"` would reveal dirs from all 3 tags
    subtags: Vec<String>,
}

impl Tag {
    /// Create a new tag with the id `id`
    pub fn create(id: &str) -> Self {
        Tag {
            id: id.to_string(),
            entries: Vec::new(),
            subtags: Vec::new(),
        }
    }

    /// Make this tag a subtag of `parent_tag`
    pub fn as_subtag_of(self, parent_tag: &mut Tag) -> Self {
        parent_tag.add_subtag(&self.id);
        self
    }

    /// Returns whether the base dir already existed
    pub fn initiate_base_dir() -> io::Result<bool> {
        let base_dir = Tag::get_base_dir();
        if base_dir.exists() {
            return Ok(true);
        }
        create_dir_all(&base_dir)?;
        Ok(false)
    }

    pub fn get_base_dir_or_create() -> io::Result<PathBuf> {
        let base_dir = Tag::get_base_dir();
        if base_dir.exists() {
            return Ok(base_dir);
        }
        Tag::initiate_base_dir().map(|_| base_dir)
    }

    /// Returns the base dir where all tags are stored
    // #[cfg(not(test))]
    #[inline]
    pub fn get_base_dir() -> PathBuf {
        let base_dirs = directories::BaseDirs::new() .expect("could not get base dirs");

        let mut p = base_dirs.config_dir().to_path_buf();
        // dbg!(&p);
        // dbg!( std::env!("CARGO_PKG_NAME") );

        let app_name = std::env!("CARGO_PKG_NAME").to_string();
        p.push(app_name + "/tags/");

        p
    }

    /// Returns the base dir where all tags are stored (for tests only)
    /* #[cfg(test)]
    #[inline]
    pub fn base_dir() -> PathBuf {
        PathBuf::from("C:/Users/ddxte/Documents/Projects/tag-explorer/test_tags/")
    }
    */

    pub fn get_path_for_tag(with_name: &str) -> PathBuf {
        Tag::get_base_dir().join(format!("{with_name}.toml"))
    }

    #[inline]
    pub fn get_save_path(&self) -> PathBuf {
        Tag::get_path_for_tag(&self.id)
    }

    #[inline]
    pub fn tag_exists(tag_id: &str) -> bool {
        Tag::get_path_for_tag(tag_id).exists()
    }

    #[inline]
    pub fn exists(&self) -> bool {
        Tag::tag_exists(&self.id)
    }

    /// Get all existing tags as paths
    pub fn get_all_tags() -> io::Result<Vec<PathBuf>> {
        Ok(read_dir(Tag::get_base_dir_or_create()?)?
            .flatten()
            .map(|de| de.path())
            .filter(|pb| pb.is_file())
            .collect())
    }

    /// Get all existing tag ids
    pub fn get_all_tag_ids() -> io::Result<Vec<String>> {
        Ok(Tag::get_all_tags()?
            .iter()
            .filter_map(|pb| {
                pb.file_stem()
                    .and_then(|osstr| osstr.to_str())
                    .map(|str| str.to_string())
            })
            .collect())
    }

    /// Add an entry to this [`Tag`]
    pub fn add_entry<P>(&mut self, path: P) -> Result<(), AddEntryError>
    where
        P: AsRef<Path>,
    {
        if !path.as_ref().exists() {
            return Err(AddEntryError::NonexistentPath(path.as_ref().to_path_buf()));
        }

        if self.contains(&path) {
            return Err(AddEntryError::AlreadyContained);
        }
        self.entries.push(path.as_ref().to_path_buf());
        Ok(())
    }

    /// Try to remove `path` from the entries
    /// Returns whether it was successful
    pub fn remove_entry<P>(&mut self, path: &P) -> bool
    where
        P: PartialEq<PathBuf>,
    {
        if let Some(index) = self.entries.iter().position(|p| path == p) {
            self.entries.remove(index);
            return true;
        }
        false
    }

    /// Returns whether the given path is tagged with this [`Tag`]
    pub fn contains<P>(&self, path: P) -> bool
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        self.entries.iter().any(|p| path.starts_with(p))
    }

    /// Get entries under this [`Tag`], NOT including all subtags
    #[inline]
    pub fn get_entries(&self) -> &Vec<PathBuf> {
        &self.entries
    }

    /// Get all entries under this [`Tag`], including all subtags
    pub fn get_all_entries(&self) -> Vec<PathBuf> {
        let mut entries = self.entries.clone();

        // Merge subtags' entries into this one
        let it = self.subtags.iter().filter_map(|id| Tag::load(id).ok());
        for subtag in it {
            let subtag_entries = subtag.get_all_entries();
            let mut st_entries_filtered: Vec<PathBuf> = subtag_entries
                .into_iter()
                .filter(|sub_pb| !entries.iter().any(|p| sub_pb.starts_with(p)))
                .collect();
            entries.append(&mut st_entries_filtered);
        }

        entries
    }

    pub fn save(&self) -> Result<bool, SaveError> {
        let path = self.get_save_path();

        if !self.entries.is_empty() {
            self.save_to_path(path)?;
            return Ok(true);
        } else if path.exists() {
            remove_file(path)?;
        }
        Ok(false)
    }

    pub fn load(name: &str) -> Result<Tag, LoadError> {
        Tag::load_from_path(Tag::get_path_for_tag(name))
    }

    pub fn save_to_path<P>(&self, path: P) -> Result<(), SaveError>
    where
        P: AsRef<Path>,
    {
        if self.id.is_empty() {
            return Err(SaveError::NoID);
        }

        let string = toml::to_string_pretty(self)?;

        if !path.as_ref().exists() {
            let dir = path.as_ref().parent().expect("could not get parent dir");
            create_dir_all(dir)?;
        }

        File::create(path)?.write_all(string.as_bytes())?;
        Ok(())
    }

    pub fn load_from_path<P>(path: P) -> Result<Tag, LoadError>
    where
        P: AsRef<Path>,
    {
        let mut contents = String::new();
        File::open(&path)?.read_to_string(&mut contents)?;
        let mut tag = toml::from_str::<Tag>(&contents)?;

        tag.id = path
            .as_ref()
            .file_stem()
            .and_then(|osstr| osstr.to_str())
            .ok_or_else(|| LoadError::InvalidName(path.as_ref().to_path_buf()))?
            .to_string();

        tag.entries.retain(|pb| pb.exists());
        tag.subtags.retain(|tag_id| Tag::tag_exists(tag_id));

        Ok(tag)
    }

    /// Get all directories under this [`Tag`], including all subtags
    pub fn get_dirs(&self) -> Box<dyn Iterator<Item = PathBuf>> {
        // Files and folders merged with subtags
        let (files, folders) = self
            .get_all_entries()
            .iter()
            .cloned()
            .partition::<Vec<PathBuf>, _>(|pb| pb.is_file());

        let mut it: Box<dyn Iterator<Item = PathBuf>> = Box::new(files.into_iter());

        for pathbuf in folders {
            let walker = WalkDir::new(pathbuf)
                .into_iter()
                .filter_entry(|de| !is_direntry_hidden(de))
                .flatten()
                .map(|e| e.into_path());
            it = Box::new(it.chain(walker));
        }

        it
    }

    #[inline]
    pub fn get_subtags(&mut self) -> &Vec<String> {
        &self.subtags
    }

    pub fn add_subtag(&mut self, tag_id: &str) -> bool {
        let s = tag_id.to_string();
        if self.subtags.contains(&s) {
            return false;
        }
        self.subtags.push(s);
        true
    }

    pub fn remove_subtag(&mut self, tag_id: &str) -> bool {
        if let Some(idx) = self.subtags.iter().position(|st| st == tag_id) {
            self.subtags.remove(idx);
            return true;
        }
        false
    }
}

fn is_direntry_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn serde() {
        let mut tag = Tag::create("test");
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();

        println!("Saving...");
        tag.save().unwrap();

        println!("Loading...");
        let tag2 = Tag::load("test").unwrap();

        assert_eq!(tag.get_entries(), tag2.get_entries());
    }

    #[test]
    fn add_and_remove() {
        let mut tag = Tag::create("test");
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();

        assert!(tag.contains("C:/Users/ddxte/Documents/"));
        assert!(tag.contains("C:/Users/ddxte/Documents/Projects/music_tools.exe"));

        // Adding already tagged dir
        let add_err = tag.add_entry("C:/Users/ddxte/Documents/Projects/music_tools.exe");
        assert!(matches!(add_err, Err(AddEntryError::AlreadyContained)));
        assert_eq!(tag.get_entries().len(), 1);

        tag.remove_entry(&Path::new("C:/Users/ddxte/Documents/"));
        assert!(tag.get_entries().is_empty());
    }

    #[test]
    fn save_empty() {
        let tag_id = "empty tag".to_string();

        let mut tag = Tag::create(&tag_id);
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();
        tag.save().unwrap();

        assert!(Tag::get_all_tag_ids().unwrap().contains(&tag_id));

        let tag = Tag::create(&tag_id);
        tag.save().unwrap();

        assert!(!Tag::get_all_tag_ids().unwrap().contains(&tag_id));
    }

    #[test]
    #[ignore = "no assertions"]
    fn get_paths() {
        let mut tag = Tag::create("test");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();

        for path in tag.get_dirs() {
            dbg!(&path);
        }
    }

    #[test]
    fn subtags_basic() {
        let mut tag = Tag::create("test");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();

        println!("Creating subtag");
        let mut tag2 = Tag::create("bup").as_subtag_of(&mut tag);
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/")
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Projects/")
            .unwrap();

        assert!(tag.get_subtags().contains(&tag2.id));

        println!("Saving...");
        tag.save().unwrap();
        tag2.save().unwrap();

        println!("Getting merged entries");
        let all_entries = tag.get_all_entries();
        assert_eq!(
            all_entries,
            vec![
                PathBuf::from("C:/Users/ddxte/Documents/Apps/KFiles/"),
                PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
                PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            ]
        );
    }

    #[test]
    fn subtags_dirs() {
        use std::collections::HashSet;

        let mut tag = Tag::create("test");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();

        let mut tag2 = Tag::create("bup").as_subtag_of(&mut tag);
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/")
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/godot/").unwrap();

        // TODO autosave on drop?
        println!("Saving...");
        tag.save().unwrap();
        tag2.save().unwrap();

        println!("Getting paths...");
        let tag_paths = tag.get_dirs().collect::<Vec<PathBuf>>();
        dbg!(&tag_paths);

        println!("Checking for duplicates");
        let mut uniq = HashSet::new();
        let is_all_unique = tag_paths.into_iter().all(move |p| uniq.insert(p));
        assert!(is_all_unique);
    }
}
