use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{search, ToPrettyString};



#[derive(Debug, Error)]
pub enum AddEntryError {
    #[error("path does not exist")]
    NonexistentPath,
    #[error("already contained")]
    DuplicateEntry,
}



/// List of paths which a [`Tag`] contains
///
/// Duplicate entries are not allowed (obviously), but subpaths are
/// E.g. `[ a/b/, a/b/ ]` is not allowed, but `[ a/b/, a/b/c ]` is
/// The user will get to see the list of paths when they aren't searching / querying anything,
/// and they'll get squished together anyways when they are
/// E.g. `[ a/b/, a/b/c ]` -- User makes a query --> Search through `[ a/b/ ]`
/// Also, maybe use a hashset instead?
#[derive(Debug, Clone, Default)]
pub struct Entries(pub(super) Vec<PathBuf>);

impl Entries {
    #[inline]
    pub fn new() -> Entries {
        Entries::default()
    }

    pub fn push(&mut self, path: PathBuf) -> Result<(), AddEntryError> {
        if !path.exists() {
            return Err(AddEntryError::NonexistentPath);
        }

        if self.0.contains(&path) {
            return Err(AddEntryError::DuplicateEntry);
        }

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

    pub fn contains(&self, path: &Path) -> bool {
        self.0.iter().any(|p| path.starts_with(p))
    }

    /// Iterates through all the paths contained
    /// Same as [`search::iter_entries`]
    /// If you want to simply iterate over the paths defining this [`Entries`], please do
    /// `entries.as_ref().iter()`
    #[inline]
    pub fn iter(self) -> Box<dyn Iterator<Item = PathBuf>> {
        search::iter_entries( self.into() )
    }

    /// Remove and return any duplicate entries
    #[must_use]
    pub fn remove_duplicates(&mut self) -> Vec<PathBuf> {
        let mut paths: HashMap<PathBuf, bool> = HashMap::new();

        self.0.retain(|path| {
            !*paths.entry(path.clone())
                .and_modify(|v| *v = true)
                .or_insert(false)
        });

        paths.into_iter()
            .filter(|(_, v)| *v)
            .map(|(p, _)| p)
            .collect()
    }

    /// Remove any duplicate entries
    /// To get the removed paths, see [`Entries::remove_duplicates()`]
    pub fn filter_duplicates(&mut self) {
        let mut paths: HashSet<PathBuf> = HashSet::new();
        self.0.retain(|path| paths.insert(path.clone()));
    }
    
    /// Creates a new [`Entries`] that's the minimum of all paths in `self`
    /// Removes:
    /// - Any duplicate entries
    /// - Entries that are sub-paths of other entries
    pub fn trim(mut self) -> Entries {
        let paths: Vec<PathBuf> = self.0.drain(..) .collect();
        let mut new_entries = Entries::new();

        for path in paths.into_iter() {
            if new_entries.contains(&path) {
                continue;
            }

            new_entries.0.retain(|pb| !pb.starts_with(&path));
            new_entries.0.push(path);
        }

        new_entries
    }

    /// Converts this [`Entries`] into a list of paths separated by new line breaks
    pub fn to_string_list(&self) -> String {
        let v: Vec<String> = self.0.iter()
            .map(|pb| pb.to_pretty_string())
            .collect();
        v.join("\n")
    }

    /// Creates a new [`Entries`] from a list of paths separated by new line breaks
    /// Also removes any duplicates in the process
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

impl IntoIterator for Entries {
    type Item = PathBuf;
    type IntoIter = <Vec<PathBuf> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Vec<PathBuf>> for Entries {
    fn from(value: Vec<PathBuf>) -> Self {
        Entries(value.into())
    }
}

impl From<Entries> for Vec<PathBuf> {
    fn from(value: Entries) -> Self {
        value.0
    }
}
