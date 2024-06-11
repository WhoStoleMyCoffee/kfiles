use std::fs::{create_dir_all, remove_file, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use nanoserde::{DeJson, DeJsonErr, SerJson};

use crate::app::main_screen::Item;

use super::entries::{AddEntryError, Entries};
use super::id::TagID;


#[derive(Debug, Error)]
pub enum RenameError {
    #[error("tag already exists")]
    AlreadyExists,
    #[error(transparent)]
    IO(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("invalid file name")]
    InvalidName,
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error("failed to parse: {0}")]
    ParseError(#[from] DeJsonErr),
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("no ID set")]
    NoID,
    #[error(transparent)]
    IO(#[from] io::Error),
}






#[derive(Debug, Clone)]
pub struct Tag {
    pub id: TagID,

    pub entries: Entries,

    /// All tags that are tagged with this tag
    /// E.g. tag `"pictures"` could have `subtags = [ "trip", "cats" ]`
    /// Then, searching for tag `"pictures"` would reveal dirs from all 3 tags
    subtags: Vec<TagID>,
}

impl Tag {
    /// Create a new tag with the id `id`
    pub fn create<ID>(id: ID) -> Self
    where
        ID: Into<TagID>,
    {
        Tag {
            id: id.into(),
            entries: Entries(Vec::new()),
            subtags: Vec::new(),
        }
    }

    /// Make this tag a subtag of `parent_tag`
    pub fn as_subtag_of(self, parent_tag: &mut Tag) -> Self {
        parent_tag.add_subtag(&self.id);
        self
    }

    #[inline]
    pub fn get_save_path(&self) -> PathBuf {
        self.id.get_path()
    }

    #[inline]
    pub fn exists(&self) -> bool {
        self.id.exists()
    }

    /// Add an entry to this [`Tag`]
    #[inline]
    pub fn add_entry<P>(&mut self, path: P) -> Result<(), AddEntryError>
        where P: Into<PathBuf>
    {
        self.entries.push(path.into())
    }

    /// Remove `path` from this tag's entries
    /// Returns whether it was successful
    pub fn remove_entry<P>(&mut self, path: &P) -> bool
    where
        P: PartialEq<PathBuf>,
    {
        if let Some(index) = self.entries.as_ref()
            .iter().position(|p| path == p)
        {
            self.entries.remove(index);
            return true;
        }
        false
    }

    /// Remove `path` and all its subpaths from this tag's entries
    /// Returns the removed paths
    pub fn remove_entry_all<P>(&mut self, path: &P) -> Vec<PathBuf>
    where
        P: AsRef<Path>,
    {
        let path: &Path = path.as_ref();

        let mut removed: Vec<PathBuf> = Vec::new();
        self.entries.retain(|pb| {
            if pb.starts_with(path) {
                removed.push(pb.clone());
                true
            } else {
                false
            }
        });
        removed
    }

    /// Returns whether the given path is tagged with this [`Tag`], EXCLUDING subtags
    /// This is the same as doing
    /// ```
    /// tag.entries.contains(path)
    /// ```
    #[inline]
    pub fn contains<P>(&self, path: P) -> bool
    where P: AsRef<Path>,
    {
        self.entries.contains(path)
    }

    /// Returns whether the given path is tagged with this [`Tag`], INCLUDING subtags
    /// This is the same as doing
    /// ```
    /// tag.get_all_entries().contains(path)
    /// ```
    #[inline]
    pub fn all_contains<P>(&self, path: P) -> bool
    where P: AsRef<Path>,
    {
        self.get_all_entries().contains(path)
    }

    /// Get all entries under this [`Tag`], INCLUDING all subtags
    /// If you want to simply get the entries without subtags, please use [`Tag::entries`] directly
    /// TODO what if there's a cyclic dependency?
    pub fn get_all_entries(&self) -> Entries {
        let mut entries = self.entries.clone();

        // Merge subtags' entries into this one
        let it = self.subtags.iter().filter_map(|id| Tag::load(id).ok());
        for subtag in it {
            entries = entries.or(&subtag.get_all_entries());
        }

        entries
    }

    /// Saves this [`Tag`] to the disk
    pub fn save(&self) -> Result<(), SaveError> {
        let path = self.get_save_path();
        if self.id.is_empty() {
            return Err(SaveError::NoID);
        }

        if !path.exists() {
            let dir = path.parent().expect("could not get parent dir");
            create_dir_all(dir)?;
        }

        let string = SerTag::from(self).serialize_json();
        File::create(path)?.write_all(string.as_bytes())?;
        Ok(())
    }

    pub fn load(id: &TagID) -> Result<Tag, LoadError> {
        Tag::load_from_path(id.get_path())
    }

    /// Also removes the old file
    /// Returns bool:
    /// - `true` if the renaming was successful
    /// - `false` if there was no change
    pub fn rename(&mut self, new_id: &TagID) -> Result<bool, RenameError> {
        if *new_id == self.id {
            return Ok(false);
        }

        let new_path: PathBuf = new_id.get_path();
        if new_path.exists() {
            return Err(RenameError::AlreadyExists);
        }

        let path: PathBuf = self.id.get_path();
        if path.exists() {
            remove_file(path)?;
        }

        self.id.clone_from(new_id);
        Ok(true)
    }

    pub fn load_from_path<P>(path: P) -> Result<Tag, LoadError>
    where
        P: AsRef<Path>,
    {
        let mut contents = String::new();
        File::open(&path)?.read_to_string(&mut contents)?;
        let mut tag: Tag = SerTag::deserialize_json(&contents)?
            .into();

        let file_name: &str = path.as_ref()
            .file_stem()
            .and_then(|osstr| osstr.to_str())
            .ok_or(LoadError::InvalidName)?;
        tag.id = TagID(file_name.to_string());

        tag.subtags.retain(|tag_id| tag_id.exists());

        Ok(tag)
    }

    /// Get all directories under this [`Tag`], including all subtags
    pub fn get_dirs(&self) -> Box<dyn Iterator<Item = Item>> {
        unimplemented!()
    }

    #[inline]
    pub fn get_subtags(&mut self) -> &Vec<TagID> {
        &self.subtags
    }

    /// Returns whether the subtag was successfully added
    pub fn add_subtag(&mut self, tag_id: &TagID) -> bool {
        if self.subtags.contains(tag_id) {
            return false;
        }
        self.subtags.push(tag_id.clone());
        true
    }

    /// Returns whether the subtag was successfully removed
    pub fn remove_subtag(&mut self, tag_id: &TagID) -> bool {
        if let Some(idx) = self.subtags.iter().position(|st| st == tag_id) {
            self.subtags.remove(idx);
            return true;
        }
        false
    }

    pub fn is_subtag_of(&self, other: &Tag) -> bool {
        other.subtags.contains(&self.id)
    }
}

impl PartialEq<TagID> for Tag {
    fn eq(&self, other: &TagID) -> bool {
        self.id == *other
    }
}

impl PartialEq<Tag> for Tag {
    fn eq(&self, other: &Tag) -> bool {
        self.id == other.id
    }
}



#[derive(Debug, Clone, SerJson, DeJson)]
struct SerTag {
    entries: Vec<String>,
    subtags: Vec<String>,
}

impl From<&Tag> for SerTag {
    fn from(value: &Tag) -> Self {
        SerTag {
            entries: value.entries.as_ref().iter()
                .filter_map(|pb| pb.to_str())
                .map(|str| str.to_string())
                .collect(),
            subtags: value.subtags.iter()
                .map(|id| id.0.clone())
                .collect(),
        }
    }
}

impl From<SerTag> for Tag {
    fn from(value: SerTag) -> Self {
        Tag {
            id: TagID::new("uninitialized-tag"),
            entries: value.entries.into_iter()
                .map(PathBuf::from)
                .collect::<Vec<PathBuf>>()
                .into(),
            subtags: value.subtags.into_iter()
                .map(TagID)
                .collect(),
        }
    }
}



