use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use walkdir::{DirEntry, WalkDir};

use crate::app::Item;
use crate::tag::{ Entries, Tag };


fn is_direntry_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// A query to search through `tags` with an optional `query`
/// Use [`search`] to begin the search
#[derive(Debug, Default)]
pub struct Query {
    pub tags: Vec<Tag>,
    pub query: String,
}

impl Query {
    #[inline(always)]
    pub fn empty() -> Self {
        Query::default()
    }

    /// Returns whether the tag was added to the query
    /// If it wasn't, returns `false`
    pub fn add_tag(&mut self, tag: Tag) -> bool {
        if self.tags.contains(&tag) {
            return false;
        }
        self.tags.push(tag);
        true
    }

    /// Returns whether the tag was found and removed
    /// If the tag was not contained, returns `false`
    pub fn remove_tag<T>(&mut self, tag: &T) -> bool
    where
        T: PartialEq<Tag>,
    {
        if let Some(index) = self.tags.iter().position(|t| tag == t) {
            self.tags.remove(index);
            return true;
        }
        false
    }

    /// TODO turn this into a Result?
    /// Begins the search.
    /// Returns `None`if there is on query
    pub fn search(&self) -> Option<Receiver<Item>> {
        let (tx, rx) = mpsc::channel::<Item>();
        let searcher = Searcher::from(self);
        if searcher.is_empty() {
            return None;
        }

        thread::spawn(move || {
            let it = searcher.search();
            for item in it {
                if tx.send(item).is_err() {
                    return;
                }
            }
        });

        Some(rx)
    }
}

/// A struct that searches through
/// TODO create from list of tags? -> Option<Self>
#[derive(Debug, Default)]
pub struct Searcher {
    entries: Entries,
    // query: String,
}

impl Searcher {
    pub fn and(&mut self, entries: &Entries) -> &mut Self {
        self.entries = self.entries.and(entries);
        self
    }

    pub fn or(&mut self, entries: &Entries) -> &mut Self {
        self.entries = self.entries.or(entries);
        self
    }

    /// Returns whether this `Searcher` is empty
    /// Empty `Searcher`s will yield no results
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn search(&self) -> Box<dyn Iterator<Item = Item>> {
        if self.is_empty() {
            return Box::new(std::iter::empty());
        }

        // Files and folders merged with subtags
        let (files, folders) = self.entries.iter()
            .cloned()
            .partition::<Vec<PathBuf>, _>(|pb| pb.is_file());

        let mut it: Box<dyn Iterator<Item = Item>> = Box::new(files.into_iter()
            .map(|pb| Item(0, pb))
        );

        for pb in folders {
            let walker = WalkDir::new(pb).into_iter()
                .filter_entry(|de| !is_direntry_hidden(de))
                .flatten()
                .map(|de| Item(0, de.into_path()) );
            it = Box::new(it.chain(walker));
        }

        it
    }
}

impl From<&Tag> for Searcher {
    #[inline]
    fn from(tag: &Tag) -> Self {
        Searcher {
            entries: tag.get_all_entries(),
        }
    }
}

impl From<&Query> for Searcher {
    fn from(query: &Query) -> Self {
        let mut searcher = match query.tags.first() {
            Some(tag) => Searcher::from(tag),
            None => return Searcher::default(),
        };
        for tag in query.tags.iter().skip(1) {
            // or?
            searcher.and(&tag.get_all_entries());
        }

        searcher
    }
}
