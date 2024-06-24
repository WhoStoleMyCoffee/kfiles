use std::collections::HashMap;
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
        search::iter_entries(self)
    }

    /// Remove and return any duplicate entries
    #[must_use]
    pub fn filter_duplicates(&mut self) -> Vec<PathBuf> {
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
    
    /// Creates a new [`Entries`] that's the minimum of all paths in `self`
    /// Removes:
    /// - Any duplicate entries
    /// - Entries that are sub-paths of other entries
    #[must_use]
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


    /// Remove and return any duplicate entries
    /// TODO
    /// also find a better name for this function
    #[must_use]
    pub fn sterilize(&mut self) -> Vec<PathBuf> {
        println!("TODO Reminder: sterilize()");
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

