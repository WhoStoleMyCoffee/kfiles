use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use walkdir::{DirEntry, WalkDir};

use crate::tag::{ Entries, Tag };

fn is_direntry_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

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

    /// TODO turn this into a Result
    pub fn search(&self) -> Option<Receiver<PathBuf>> {
        let mut searcher = Searcher::from(self.tags.first()?);
        for tag in self.tags.iter().skip(1) {
            // or?
            searcher.and(&tag.get_all_entries());
        }

        let (tx, rx) = mpsc::channel::<PathBuf>();

        thread::spawn(move || {
            let it = searcher.search();
            for pb in it {
                if tx.send(pb).is_err() {
                    return;
                }
            }
        });

        Some(rx)
    }
}

/// TODO create from list of tags? -> Option<Self>
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

    pub fn search(&self) -> Box<dyn Iterator<Item = PathBuf>> {
        // Files and folders merged with subtags
        let (files, folders) = self
            .entries
            .iter()
            .cloned()
            .partition::<Vec<PathBuf>, _>(|pb| pb.is_file());

        let mut it: Box<dyn Iterator<Item = PathBuf>> = Box::new(files.into_iter());

        for pb in folders {
            let walker = WalkDir::new(pb)
                .into_iter()
                .filter_entry(|de| !is_direntry_hidden(de))
                .flatten()
                .map(|e| e.into_path());
            it = Box::new(it.chain(walker));
        }

        it
    }
}

impl From<&Tag> for Searcher {
    fn from(tag: &Tag) -> Self {
        Searcher {
            entries: Entries::from(tag.get_all_entries()),
        }
    }
}
