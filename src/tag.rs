use std::fmt::Display;
use std::fs::{create_dir_all, read_dir, remove_file, File};
use std::io::{self, Read, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use convert_case::{Case, Casing};
use thiserror::Error;
use toml;

use serde::{Deserialize, Serialize};

use crate::app::mainscreen::Item;
use crate::{search, ToPrettyString};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    #[serde(skip)]
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
    pub fn add_entry<P>(&mut self, path: P) -> Result<(), AddEntryError>
    where
        P: AsRef<Path>,
    {
        if !path.as_ref().exists() {
            return Err(AddEntryError::NonexistentPath);
        }

        if self.entries.contains(&path) {
            return Err(AddEntryError::AlreadyContained);
        }
        self.entries.push(path.as_ref().to_path_buf());
        Ok(())
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
        self.save_to_path(path)
    }

    pub fn load(id: &TagID) -> Result<Tag, LoadError> {
        Tag::load_from_path(id.get_path())
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

        let file_name = path
            .as_ref()
            .file_stem()
            .and_then(|osstr| osstr.to_str())
            .ok_or(LoadError::InvalidName)?;
        tag.id = TagID(file_name.to_string());

        tag.entries.retain(|pb| pb.exists());
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

    pub fn add_subtag(&mut self, tag_id: &TagID) -> bool {
        if self.subtags.contains(tag_id) {
            return false;
        }
        self.subtags.push(tag_id.clone());
        true
    }

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

impl PartialEq<Tag> for TagID {
    fn eq(&self, other: &Tag) -> bool {
        other.id == *self
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct TagID(String);

impl TagID {
    pub fn new(value: &str) -> Self {
        TagID(value.to_string())
    }

    pub fn parse<T>(value: T) -> Self
    where
        T: AsRef<str>,
    {
        TagID(value.as_ref().to_case(Case::Kebab))
    }

    pub fn get_path(&self) -> PathBuf {
        get_save_dir().join(format!("{}.toml", self.0))
    }

    pub fn exists(&self) -> bool {
        self.get_path().exists()
    }

    #[inline]
    pub fn load(&self) -> Result<Tag, LoadError> {
        Tag::load(self)
    }
}

impl From<&str> for TagID {
    fn from(value: &str) -> Self {
        TagID::parse(value)
    }
}

impl From<&TagID> for PathBuf {
    fn from(value: &TagID) -> Self {
        value.get_path()
    }
}

/// Invalid file name error
pub struct InvalidFileName;

impl TryFrom<&Path> for TagID {
    type Error = InvalidFileName;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        Ok(TagID(
            value
                .file_stem()
                .and_then(|osstr| osstr.to_str())
                .ok_or(InvalidFileName)?
                .to_string(),
        ))
    }
}

impl AsRef<String> for TagID {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

impl Deref for TagID {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for TagID {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
// TODO hashset?
pub struct Entries(Vec<PathBuf>);

impl Entries {
    #[inline]
    pub fn new() -> Entries {
        Entries::default()
    }

    /// Create a new [`Entries`] that's a union of all `entries`, which means that
    /// it contains all of their paths
    pub fn union_of<I>(entries: I) -> Entries
    where
        I: IntoIterator<Item = Entries>
    {
        entries.into_iter()
            .reduce(|acc, e| acc.or(&e))
            .unwrap_or_default()
    }

    /// Create a new [`Entries`] that's an intersection of all `entries`, which
    /// means that it only contains paths that are shared between them
    pub fn intersection_of<I>(entries: I) -> Entries
    where
        I: IntoIterator<Item = Entries>
    {
        entries.into_iter()
            .reduce(|acc, e| acc.and(&e))
            .unwrap_or_default()
    }

    /// Combines this `Entry` with `other` by union
    pub fn or<E>(&self, other: &E) -> Entries
    where
        E: AsRef<Vec<PathBuf>>,
    {
        let c: Vec<PathBuf> = self.0.iter()
            .filter(|&ap| !other.as_ref().iter()
                .any(|bp| ap.starts_with(bp) || ap == bp)
            )
            .chain(other.as_ref().iter()
                .filter(|bp| !self.0.iter()
                    .any(|ap| bp.starts_with(ap))
                )
            )
            .cloned()
            .collect();

        Entries(c)
    }

    /// Combines this `Entry` with `other` by intersection
    pub fn and<E>(&self, other: &E) -> Entries
    where
        E: AsRef<Vec<PathBuf>>,
    {
        let c: Vec<PathBuf> = self.0.iter()
            .filter(|&ap| other.as_ref().iter()
                .any(|bp| ap.starts_with(bp) || ap == bp)
            )
            .chain(other.as_ref().iter()
                .filter(|bp| self.0.iter()
                    .any(|ap| bp.starts_with(ap))
                )
            )
            .cloned()
            .collect();

        Entries(c)
    }

    pub fn contains<P>(&self, path: P) -> bool
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        self.0.iter().any(|p| path.starts_with(p))
    }

    /// Iterates through all the paths contained
    /// Same as [`search::iter_entries`]
    /// If you want to simply iterate over the paths defining this [`Entries`], please do
    /// `entries.as_ref().iter()`
    #[inline]
    pub fn iter(self) -> Box<dyn Iterator<Item = PathBuf>> {
        search::iter_entries(self)
    }

    pub fn to_list(&self) -> String {
        let v: Vec<String> = self.0.iter()
            .map(|pb| pb.to_pretty_string())
            .collect();
        v.join("\n")
    }

    pub fn from_list(str: &str) -> Self {
        Entries::from(str.lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| PathBuf::from(s))
            .collect::<Vec<PathBuf>>())
    }

}

impl AsRef<Vec<PathBuf>> for Entries {
    fn as_ref(&self) -> &Vec<PathBuf> {
        &self.0
    }
}

impl AsMut<Vec<PathBuf>> for Entries {
    fn as_mut(&mut self) -> &mut Vec<PathBuf> {
        &mut self.0
    }
}

impl Deref for Entries {
    type Target = Vec<PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Entries {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Into<Vec<PathBuf>>> From<T> for Entries {
    fn from(value: T) -> Self {
        Entries(value.into())
    }
}

impl IntoIterator for Entries {
    type Item = PathBuf;
    type IntoIter = <Vec<PathBuf> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}






#[derive(Debug, Error)]
pub enum AddEntryError {
    #[error("path does not exist")]
    NonexistentPath,
    #[error("already contained")]
    AlreadyContained,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("could not get tag name from file")]
    InvalidName,

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




/// Returns whether the base dir already existed
pub fn initiate_save_dir() -> io::Result<bool> {
    let base_dir = get_save_dir();
    if base_dir.exists() {
        return Ok(true);
    }
    create_dir_all(&base_dir)?;
    Ok(false)
}

pub fn get_save_dir_or_create() -> io::Result<PathBuf> {
    let base_dir = get_save_dir();
    if base_dir.exists() {
        return Ok(base_dir);
    }
    initiate_save_dir().map(|_| base_dir)
}

/// Returns the base dir where all tags are stored
#[cfg(not(test))]
#[inline]
pub fn get_save_dir() -> PathBuf {
    const APP_NAME: &str = std::env!("CARGO_PKG_NAME");
    directories::BaseDirs::new()
        .expect("could not get base dirs")
        .config_dir()
        .to_path_buf()
        .join(APP_NAME.to_string() + "/tags/")
}

/// Returns the base dir where all tags are stored (for tests only)
#[cfg(test)]
#[inline]
pub fn get_save_dir() -> PathBuf {
    PathBuf::from("C:/Users/ddxte/Documents/Projects/tag-explorer/test_tags/")
}

/// Get all existing tags as paths
pub fn get_all_tags() -> io::Result<Vec<PathBuf>> {
    Ok(read_dir(get_save_dir_or_create()?)?
        .flatten()
        .map(|de| de.path())
        .filter(|pb| pb.is_file())
        .collect())
}

/// Get all existing tag ids
pub fn get_all_tag_ids() -> io::Result<Vec<TagID>> {
    Ok(get_all_tags()?
        .iter()
        .filter_map(|pb| TagID::try_from(pb.as_path()).ok())
        .collect())
}








#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, path::Path};

    #[test]
    fn serde() {
        let tag_id = TagID::from("test");
        let mut tag = Tag::create(tag_id.clone());
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();

        println!("Saving...");
        tag.save().unwrap();

        println!("Loading...");
        // let tag2 = Tag::load( &"test".into() ).unwrap();
        // let tag2 = Tag::load( &TagID::parse("test") ).unwrap();
        let tag2 = Tag::load(&tag_id).unwrap();

        assert_eq!(tag.entries.as_ref(), tag2.entries.as_ref());
    }

    #[test]
    fn add_and_remove() {
        let tag_id = TagID::from("test");
        let mut tag = Tag::create(tag_id);
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();

        assert!(tag.contains("C:/Users/ddxte/Documents/"));
        assert!(tag.contains("C:/Users/ddxte/Documents/Projects/music_tools.exe"));

        // Adding already tagged dir
        let add_err = tag.add_entry("C:/Users/ddxte/Documents/Projects/music_tools.exe");
        assert!(matches!(add_err, Err(AddEntryError::AlreadyContained)));
        assert_eq!(tag.entries.len(), 1);

        tag.remove_entry(&Path::new("C:/Users/ddxte/Documents/"));
        assert!(tag.entries.is_empty());
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

        assert!(tag2.is_subtag_of(&tag));

        println!("Saving...");
        tag.save().unwrap();
        tag2.save().unwrap();

        println!("Getting merged entries");
        let all_entries = tag.get_all_entries();
        assert_eq!(
            *all_entries,
            vec![
                PathBuf::from("C:/Users/ddxte/Documents/Apps/KFiles/"),
                PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
                PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            ]
        );
    }

    #[test]
    fn subtags_dirs() {
        let mut tag = Tag::create("test");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();

        let mut tag2 = Tag::create("bup").as_subtag_of(&mut tag);
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/")
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/godot/").unwrap();

        println!("Saving...");
        tag.save().unwrap();
        tag2.save().unwrap();

        println!("Getting paths...");
        let tag_paths: Vec<PathBuf> = tag.get_all_entries()
            .iter()
            .collect();

        println!("Checking for duplicates");
        let mut uniq = HashSet::new();
        let is_all_unique = tag_paths.into_iter().all(move |p| uniq.insert(p));
        assert!(is_all_unique);
    }

    #[test]
    fn tagid() {
        println!("Testing conversion");
        let id_string = "test tagYeah";
        let id = TagID::parse(id_string);
        assert_eq!("test-tag-yeah", id.as_ref()); // Conversion

        println!("Testing eq");
        assert_eq!("test-tag-yeah", *id); // PartialEq
        assert_eq!(id, id); // Eq
    }

    #[test]
    fn entries() {
        let a = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);

        let b = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Documents/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);

        println!("Testing or");
        let c = HashSet::from_iter(a.or(&b));
        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
        ]);
        assert!(c.is_subset(&expected));
        assert!(HashSet::from_iter(b.or(&a)).is_subset(&c));

        println!("Testing and");
        let c = HashSet::from_iter(a.and(&b));
        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
        ]);
        assert!(c.is_subset(&expected));
        assert!(HashSet::from_iter(b.and(&a)).is_subset(&c));
    }

    #[test]
    fn test_entries_str() {
        let entries = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Videos/"),
            PathBuf::from("C:/Users/ddxte/Desktop/temp/iced/examples/editor/fonts/icons.ttf"),
        ]);

        let list = entries.to_list();
        assert_eq!(list, r#"C:/Users/ddxte/Documents/Projects/
C:/Users/ddxte/Pictures/
C:/Users/ddxte/Videos/
C:/Users/ddxte/Desktop/temp/iced/examples/editor/fonts/icons.ttf"#);

        let entries2 = Entries::from_list(&list);
        assert_eq!(entries.as_ref(), entries2.as_ref());
    }
}
