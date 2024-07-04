use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use crate::{search, ToPrettyString};



#[derive(Debug)]
pub struct NonexistentPath;



/// List of paths which a [`Tag`] contains
/// All contained paths are guaranteed to exist
/// Duplicate entries are not allowed
#[derive(Debug, Clone, Default)]
pub struct Entries(pub(super) Vec<PathBuf>);

impl Entries {
    #[inline]
    pub fn new() -> Entries {
        Entries::default()
    }

    /// Adds an entry to this [`Entries`] list
    /// Returns `Err(NonexistentPath)` if the path doesn't exist
    /// Returns `Ok(bool)` containing whether it was added. i.e. `Ok(false)` if the entry is
    /// already contained
    pub fn push(&mut self, path: PathBuf) -> Result<bool, NonexistentPath> {
        if !path.exists() {
            return Err(NonexistentPath);
        }

        if self.0.contains(&path) {
            return Ok(false);
        }

        self.0.push(path);
        Ok(true)
    }

    /// Returns whether the given `path` is contained in this [`Entries`] list
    /// To get whether a path is an entry in this list, please use `entries.as_ref().contains()`
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
    /// See also [`Entries::filter_duplicates`]
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
    pub fn filter_duplicates(mut self) -> Self {
        let mut paths: HashSet<PathBuf> = HashSet::new();
        self.0.retain(|path| paths.insert(path.clone()));
        self
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

    /// Creates a new [`Entries`] that's the minimum of all paths in `self`
    /// Removes:
    /// - Any duplicate entries
    /// - Entries that are sub-paths of other entries
    pub fn trim(self) -> Entries {
        let mut new_entries = Entries::new();

        for path in self.0.into_iter() {
            if new_entries.contains(&path) {
                continue;
            }

            new_entries.0.retain(|pb| !pb.starts_with(&path));
            new_entries.0.push(path);
        }

        new_entries
    }

    /// Create a new [`Entries`] that's a union of all `paths`, which means that
    /// it contains all of their paths, and covers a larger or equal area
    /// TODO optimize in the future
    pub fn union_of<I>(entries: I) -> Entries
    where I: IntoIterator<Item = Entries>
    {
        let mut new_entries = Entries::new();

        for path in entries.into_iter().flatten() {
            if new_entries.contains(&path) {
                continue;
            }

            new_entries.0.retain(|pb| !pb.starts_with(&path));
            new_entries.0.push(path);
        }

        new_entries
    }

    /// Create a new [`Entries`] that's an intersection of all `paths`, which
    /// means that it only contains paths that are shared between them
    /// The resulting [`Entries`] will cover a smaller or equal area
    /// TODO optimize in the future
    pub fn intersection_of<I>(entries: I) -> Entries
    where I: IntoIterator<Item = Entries>
    {
        let mut it = entries.into_iter();
        let mut new_entries = it.next().unwrap_or_default();

        for e in it {
            let (mut e, f) = e.into_iter()
                .partition::<Vec<PathBuf>, _>(|bp| new_entries.0.iter()
                    .any(|ap| bp.starts_with(ap) && ap != bp)
                );

            new_entries.retain(|ap|
                e.iter().chain(&f)
                    .any(|bp| ap.starts_with(bp) || ap == bp)
            );
            new_entries.append(&mut e);
        }

        new_entries.trim()
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
        Entries(value)
    }
}

impl From<Entries> for Vec<PathBuf> {
    fn from(value: Entries) -> Self {
        value.0
    }
}

impl FromIterator<PathBuf> for Entries {
    fn from_iter<T: IntoIterator<Item = PathBuf>>(iter: T) -> Self {
        Entries(Vec::from_iter(iter))
    }
}





