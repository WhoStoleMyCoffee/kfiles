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
    pub fn empty() -> Self {
        Query::default()
    }

    pub fn new(query: &str) -> Query {
        Query {
            tags: Vec::new(),
            query: query.to_string(),
        }
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
    /// TODO refactor
    /// Begins the search.
    pub fn search(&self) -> Receiver<Item> {
        let (tx, rx) = mpsc::channel::<Item>();
        // let searcher = Searcher::from(self);

        let query = self.query.clone();
        let entries = Entries::intersection_of(self.tags.iter()
            .map(|tag| tag.get_all_entries())
        );

        thread::spawn(move || {
            let iter = itersearch::IterSearcher::new(query, entries);
            for item in iter {
                if tx.send(item).is_err() {
                    return;
                }
            }
        });

        rx
    }
}




/// A struct that searches through
/// TODO create from list of tags? -> Option<Self>
/// TODO just remove this...
#[derive(Debug, Default)]
pub struct Searcher {
    entries: Entries,
    query: String,
}

impl Searcher {
    /// Constructs a `Searcher` with the given query and tags
    pub fn new<'a, T>(query: &str, tags: T) -> Searcher
    where T: IntoIterator<Item = &'a Tag>
    {
        // What if we wanna use `or` instead of `and`?
        Searcher {
            entries: tags.into_iter()
                .map(|tag| tag.get_all_entries())
                .reduce(|acc, e| acc.and(&e))
                .unwrap_or_default(),
            query: query.to_string(),
        }
    }

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

        let query_lower = self.query.to_lowercase();

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
                .map(|de| de.into_path() )
                // .filter(|path| {
                //     let file_name = path.file_name()
                //         .unwrap()
                //         .to_string_lossy();
                //     file_name.to_lowercase().contains(&query_lower)
                // })
                .map(|path| Item(0, path));
            it = Box::new(it.chain(walker));
        }

        it
    }
}


impl From<&Query> for Searcher {
    fn from(query: &Query) -> Self {
        Searcher::new(
            &query.query,
            &query.tags,
       )
    }
}



mod itersearch {
    use std::path::PathBuf;

    use walkdir::WalkDir;

    use crate::{app, tag::Entries};

    use super::is_direntry_hidden;

    pub struct IterSearcher {
        iter: Box<dyn Iterator<Item = PathBuf>>,
        query: String,
    }

    impl IterSearcher {
        pub fn new(query: String, entries: Entries) -> Self {
            // Files and folders merged with subtags
            let (files, folders) = entries.iter()
                .cloned()
                .partition::<Vec<PathBuf>, _>(|pb| pb.is_file());
            let mut iter: Box<dyn Iterator<Item = PathBuf>> = Box::new(files.into_iter());

            for pb in folders {
                let walker = WalkDir::new(pb).into_iter()
                    .filter_entry(|de| !is_direntry_hidden(de))
                    .flatten()
                    .map(|de| de.into_path());
                iter = Box::new(iter.chain(walker));
            }

            IterSearcher {
                iter,
                query,
            }
        }
    }

    impl Iterator for IterSearcher {
        type Item = app::Item;

        fn next(&mut self) -> Option<Self::Item> {
            self.iter.find(|pb| {
                let file_name = pb.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_lowercase();
                file_name.contains( &self.query )
            })
            .map(|pb| app::Item(0, pb))
        }
    }

}



#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{app::Item, tag::Entries};

    use super::itersearch::IterSearcher;

    #[test]
    fn test_itersearch() {
        let query = "fat".to_string();
        let entries = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Pictures/")
        ]);

        let iter = IterSearcher::new(query, entries);
        for Item(_, pb) in iter {
            dbg!(&pb);
        }

    }
}


