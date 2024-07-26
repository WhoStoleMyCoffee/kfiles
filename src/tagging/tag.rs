use std::collections::{HashSet, VecDeque};
use std::fmt::Display;
use std::fs::{create_dir_all, remove_file, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use nanoserde::{DeJson, DeJsonErr, SerJson};

use crate::app::main_screen::Item;

use super::entries::{NonexistentPath, Entries};
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

/// Self-referring subtag error
#[derive(Debug)]
pub struct SelfReferringSubtag;

impl Display for SelfReferringSubtag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "self-referring subtag")
    }
}





#[derive(Debug, Clone)]
pub struct Tag {
    /// Corresponds with file name in which the data is stored
    pub id: TagID,

    /// Paths that this tag contains
    /// Automatically goes inside folders during search, and ignores those that
    /// start with "." (e.g. ".git/")
    pub entries: Entries,

    /// All tags that are tagged with this tag
    /// E.g. tag `"pictures"` could have `subtags = [ "animals", "memes" ]`
    /// Then, searching for tag `"pictures"` would reveal entries from all 3 tags,
    /// but `"animals"` and `"memes"` would only reveal entries from themselves
    pub(super) subtags: Vec<TagID>,
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

    pub fn with_entries(mut self, entries: Entries) -> Self {
        self.entries = entries;
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
    /// See also [`Entries::push`]
    #[inline]
    pub fn add_entry<P>(&mut self, path: P) -> Result<bool, NonexistentPath>
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
        self.entries.contains(path.as_ref())
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
        self.get_all_entries().contains(path.as_ref())
    }

    /// Get all entries under this [`Tag`], INCLUDING all subtags
    /// If you want to simply get the entries without subtags, please use [`Tag::entries`] directly
    pub fn get_all_entries(&self) -> Entries {
        let mut entries = self.entries.clone();

        // Merge subtags' entries into this one
        for mut tag in self.iter_all_subtags() {
            entries.as_mut().append(&mut tag.entries);
        }

        entries.filter_duplicates()
    }

    pub fn save(&self) -> Result<(), SaveError> {
        if self.id.is_empty() {
            return Err(SaveError::NoID);
        }

        let path = self.get_save_path();
        if !path.exists() {
            let dir = path.parent().expect("could not get parent dir");
            create_dir_all(dir)?;
        }

        let string = SerTag::from(self).serialize_json();
        File::create(path)?
            .write_all(string.as_bytes())?;
        Ok(())
    }

    pub fn load(id: &TagID) -> Result<Tag, LoadError> {
        Tag::load_from_path(&id.get_path())
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

    pub fn load_from_path(path: &Path) -> Result<Tag, LoadError> {
        let mut contents = String::new();
        File::open(path)?
            .read_to_string(&mut contents)?;
        let mut tag: Tag = SerTag::deserialize_json(&contents)?
            .into();

        let file_name: &str = path.file_stem()
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

    /// Get this tag's direct subtags
    #[inline]
    pub fn get_subtags(&self) -> &Vec<TagID> {
        &self.subtags
    }

    /// Get all of this tag's subtags, that is, including subtags' subtags
    /// Avoids infinite loops
    #[inline]
    pub fn iter_all_subtags(&self) -> Subtags {
        Subtags::new(self)
    }

    /// Returns whether the subtag was successfully added (i.e. whether it wasn't already contained)
    pub fn add_subtag(&mut self, tag_id: &TagID) -> Result<bool, SelfReferringSubtag> {
        if *tag_id == self.id {
            return Err(SelfReferringSubtag);
        }

        if self.subtags.contains(tag_id) {
            return Ok(false);
        }
        self.subtags.push(tag_id.clone());
        Ok(true)
    }

    /// Returns whether the subtag was successfully removed (i.e. whether it was contained)
    pub fn remove_subtag(&mut self, tag_id: &TagID) -> bool {
        if let Some(idx) = self.subtags.iter().position(|st| st == tag_id) {
            self.subtags.remove(idx);
            return true;
        }
        false
    }

    pub fn is_direct_subtag_of(&self, other: &Tag) -> bool {
        self.id.is_subtag_of(other)
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
            id: TagID( String::new() ),
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





/// Iterator over the subtags of a [`Tag`]
/// TODO optimizations perhaps?
pub struct Subtags {
    memo: HashSet<TagID>,
    queue: VecDeque<TagID>,
}

impl Subtags {
    fn new(tag: &Tag) -> Subtags {
        Subtags {
            memo: HashSet::from([ tag.id.clone() ]),
            queue: VecDeque::from(tag.subtags.clone()),
        }
    }
}

impl Iterator for Subtags {
    type Item = Tag;

    fn next(&mut self) -> Option<Self::Item> {
        let mut tag_id = self.queue.pop_front()?;
        while self.memo.contains(&tag_id) {
            tag_id = self.queue.pop_front()?;
        }

        let tag = tag_id.load().ok()?;

        self.memo.insert(tag_id);
        self.queue.extend( tag.subtags.clone() );

        Some(tag)
    }
}



