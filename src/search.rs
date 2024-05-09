use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use walkdir::{DirEntry, WalkDir};

use crate::app::{ self, Item };
use crate::strmatch::{StringMatcher, Sublime};
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

        let matcher = Sublime::default()
            .with_query(&self.query);
        let entries = Entries::intersection_of(self.tags.iter()
            .map(|tag| tag.get_all_entries())
        );

        thread::spawn(move || {
            let iter = Searcher::new(matcher, entries);
            for item in iter {
                if tx.send(item).is_err() {
                    return;
                }
            }
        });

        rx
    }
}






pub struct Searcher<Matcher: StringMatcher> {
    iter: Box<dyn Iterator<Item = PathBuf>>,
    matcher: Matcher,
}

impl<Matcher: StringMatcher> Searcher<Matcher> {
    pub fn new(matcher: Matcher, entries: Entries) -> Self {
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

        Searcher::<Matcher> {
            iter,
            matcher,
        }
    }
}

impl<Matcher: StringMatcher> Iterator for Searcher<Matcher> {
    type Item = app::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|pb| {
            let str = pb.display()
                .to_string()
                .replace('\\', "/");
            self.matcher.score(&str)
                .map(|s| Item(s, pb))
        })
    }
}





#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use crate::{app::Item, tag::Entries};
    use crate::strmatch;
    use super::Searcher;

    #[test]
    #[ignore]
    fn test_itersearch() {
        let entries = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Pictures/"),
        ]);

        let matcher = strmatch::Contains("dino".to_string());
        let iter = Searcher::new(matcher, entries);
        for Item(_, pb) in iter {
            dbg!(&pb);
        }

    }
}


