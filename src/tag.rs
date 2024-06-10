use std::fmt::Display;
use std::fs::{create_dir_all, read_dir, remove_file, File};
use std::io::{self, Read, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use convert_case::{Case, Casing};
use thiserror::Error;
use nanoserde::{DeJson, DeJsonErr, SerJson};

use crate::app::main_screen::Item;
use crate::{search, ToPrettyString};


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

impl PartialEq<Tag> for TagID {
    fn eq(&self, other: &Tag) -> bool {
        other.id == *self
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Hash)]
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

    pub fn make_unique_in<T>(mut self, tags: &[T]) -> Self
    where T: PartialEq<TagID>
    {
        let mut count: u32 = 0;
        let mut new_id = self.clone();

        loop {
            // We have a duplicate
            if tags.iter().any(|t| *t == new_id) {
                count += 1;
                new_id = TagID(format!("{}-{}", self.0, count));
            } else {
                break;
            }
        }

        self = new_id;
        self
    }

    pub fn get_path(&self) -> PathBuf {
        get_save_dir().join(format!("{}.json", self.0))
    }

    pub fn exists(&self) -> bool {
        self.get_path().exists()
    }

    pub fn make_unique(&mut self) {
        todo!()
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

#[derive(Debug, Clone, Default)]
// TODO hashset?
pub struct Entries(Vec<PathBuf>);

impl Entries {
    #[inline]
    pub fn new() -> Entries {
        Entries::default()
    }

    pub fn push(&mut self, path: PathBuf) -> Result<(), AddEntryError> {
        if !path.exists() {
            return Err(AddEntryError::NonexistentPath);
        }

        if self.contains(&path) {
            return Err(AddEntryError::AlreadyContained);
        }

        self.0.retain(|pb| !pb.starts_with(&path));
        self.0.push(path);
        Ok(())
    }

    /// Create a new [`Entries`] that's a union of all `entries`, which means that
    /// it contains all of their paths
    pub fn union_of<I>(entries: I) -> Entries
    where I: IntoIterator<Item = Entries>
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

    // TODO un-generic this mf
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

    /// Remove and return any duplicate entries
    #[must_use]
    pub fn filter_duplicates(&mut self) -> Vec<PathBuf> {
        let entries: Vec<PathBuf> = self.0.drain(..) .collect();
        let mut duplicates: Vec<PathBuf> = Vec::new();

        for path in entries {
            if self.contains(&path) {
                duplicates.push(path);
                continue;
            }

            self.0.retain(|pb| {
                if pb.starts_with(&path) {
                    duplicates.push(pb.clone());
                    false
                } else {
                    true
                }
            });
            self.0.push(path);
        }

        duplicates
    }

    pub fn to_string_list(&self) -> String {
        let v: Vec<String> = self.0.iter()
            .map(|pb| pb.to_pretty_string())
            .collect();
        v.join("\n")
    }

    pub fn from_string_list(str: &str) -> Self {
        Entries::from(str.lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
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








#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, path::Path};

    #[test]
    fn serde() {
        let tag_id = TagID::from("test-serde");
        let mut tag = Tag::create(tag_id.clone());
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG").unwrap();
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();

        tag.save().unwrap();
        let tag2 = Tag::load(&tag_id).unwrap();

        assert_eq!(tag.entries.as_ref(), tag2.entries.as_ref());
    }

    #[test]
    fn entries_add_and_remove() {
        let tag_id = TagID::from("test-add-and-remove");
        let mut tag = Tag::create(tag_id);
        tag.add_entry("C:/Users/ddxte/Documents/").unwrap();
        tag.add_entry("C:/Users/ddxte/Videos/").unwrap();

        assert_eq!(tag.entries.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Videos/"),
        ]);

        assert!(tag.contains("C:/Users/ddxte/Documents/"));
        assert!(tag.contains("C:/Users/ddxte/Documents/Projects/music_tools.exe"));
        assert!(tag.contains("C:/Users/ddxte/Videos/Captures/"));

        // Adding already tagged dirs
        let add_err = tag.add_entry("C:/Users/ddxte/Documents/");
        match add_err {
            Err(AddEntryError::AlreadyContained) => {},
            other => panic!("Expected add_err to be Err(AddEntryError::AlreadyContained). Found {:?}", other),
        }

        let add_err = tag.add_entry("C:/Users/ddxte/Documents/Projects/music_tools.exe");
        match add_err {
            Err(AddEntryError::AlreadyContained) => {},
            other => panic!("Expected add_err to be Err(AddEntryError::AlreadyContained). Found {:?}", other),
        }

        // Entries haven't changed
        assert_eq!(tag.entries.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Videos/"),
        ]);

        tag.add_entry(PathBuf::from("C:/Users/ddxte/")) .unwrap();
        assert_eq!(tag.entries.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/"),
        ]);

        assert!( !tag.remove_entry(&Path::new("C:/Users/ddxte/Documents/")) );
        assert!( tag.remove_entry(&Path::new("C:/Users/ddxte/")) );
        assert!( tag.entries.is_empty() );
    }

    #[test]
    fn entries_filter_duplicates() {
        let mut e = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
            PathBuf::from("C:/Users/ddxte/Documents/"),
        ]);

        let duplicates = e.filter_duplicates();
        assert_eq!(duplicates, &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
        ]);
        assert_eq!(e.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
        ]);

        e.0.push( PathBuf::from("C:/Users/ddxte/") );
        let duplicates = e.filter_duplicates();
        assert_eq!(duplicates, &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
        ]);
        assert_eq!(e.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/"),
        ]);

    }

    #[test]
    fn subtags_dirs() {
        let mut tag = Tag::create("test-subtags-dirs");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG")
            .unwrap();

        // Creating subtag
        let mut tag2 = Tag::create("test-subtags-dirs-2").as_subtag_of(&mut tag);
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/")
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Projects/")
            .unwrap();

        assert!(tag2.is_subtag_of(&tag));

        // Save so we can get entries
        tag.save().unwrap();
        tag2.save().unwrap();

        // Getting merged entries
        let all_entries = tag.get_all_entries();
        assert_eq!(
            *all_entries,
            vec![
                PathBuf::from("C:/Users/ddxte/Documents/Apps/KFiles/"),
                PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
                PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            ]
        );

        // Checking for duplicates
        let mut uniq = HashSet::new();
        for pb in all_entries.into_iter() {
            assert!(uniq.insert(pb.clone()), "Path {} was a duplicate", pb.display());
        }

        assert!( tag.remove_subtag(&tag2.id) );
        assert!( tag.get_subtags().is_empty() );
    }

    #[test]
    #[should_panic = "not yet implemented"]
    fn cyclic_subtags() {
        todo!()
    }

    #[test]
    fn tag_id_parse() {
        let id_string = "test tagYeah";
        let id = TagID::parse(id_string);
        assert_eq!("test-tag-yeah", id.as_ref()); // Conversion

        assert_eq!("test-tag-yeah", *id); // PartialEq
        assert_eq!(id, id); // Eq
    }

    #[test]
    fn tag_id_unique() {
        let ids = vec![
            TagID::new("new-tag"),
            TagID::new("new-tag-1"),
            TagID::new("new-tag-2"),
        ];
        let id = TagID::new("new-tag")
            .make_unique_in(&ids);

        assert_eq!(id.as_ref(), "new-tag-3");
    }

    #[test]
    fn entries_operations() {
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

        // OR
        let c = HashSet::from_iter(a.or(&b));
        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
        ]);
        assert!(c.is_subset(&expected));
        assert!(HashSet::from_iter(b.or(&a)).is_subset(&c));

        // AND
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
    fn entries_string_list() {
        let entries = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Videos/"),
            PathBuf::from("C:/Users/ddxte/Desktop/temp/iced/examples/editor/fonts/icons.ttf"),
        ]);

        let list = entries.to_string_list();
        assert_eq!(list, r#"C:/Users/ddxte/Documents/Projects/
C:/Users/ddxte/Pictures/
C:/Users/ddxte/Videos/
C:/Users/ddxte/Desktop/temp/iced/examples/editor/fonts/icons.ttf"#);

        let entries2 = Entries::from_string_list(&list);
        assert_eq!(entries.as_ref(), entries2.as_ref());
    }
}
