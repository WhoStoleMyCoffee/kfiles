use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{search, ToPrettyString};



#[derive(Debug, Error)]
pub enum AddEntryError {
    #[error("path does not exist")]
    NonexistentPath,
    #[error("already contained")]
    AlreadyContained,
}


/// TODO hashset?
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


