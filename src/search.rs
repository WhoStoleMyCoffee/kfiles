use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use walkdir::{DirEntry, WalkDir};

use crate::app::main_screen::Item;
use crate::error;
use crate::tagging::{ entries::Entries, Tag };

use self::constraint::ConstraintList;


fn is_direntry_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}



/// Iterates through all paths in the filesystem within an [`Entries`]
/// See also [`Searcher`]
pub fn iter_entries(entries: Entries) -> Box<dyn Iterator<Item = PathBuf>> {
    // Files and folders merged with subtags
    let (files, folders) = entries.into_iter()
        .partition::<Vec<PathBuf>, _>(|pb| pb.is_file());
    let mut iter: Box<dyn Iterator<Item = PathBuf>> = Box::new(files.into_iter());

    for dir in folders {
        let walker = WalkDir::new(dir).into_iter()
            .filter_entry(|de| !is_direntry_hidden(de))
            .flatten()
            .map(|de| de.into_path());
        iter = Box::new(iter.chain(walker));
    }

    iter
}




/// Iteratively searches through some [`Entries`]
/// See also [`iter_entries`]
pub struct Searcher {
    iter: Box<dyn Iterator<Item = PathBuf>>,
    constraints: ConstraintList,
}

impl Searcher {
    pub fn new(entries: Entries, constraints: ConstraintList) -> Self {
        Searcher {
            iter: iter_entries(entries),
            constraints,
        }
    }
}

impl Iterator for Searcher {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|pb| {
            self.constraints.score(&pb)
                .map(|s| Item(s, pb))
        })
    }
}




/// A query to search through `tags` with an optional `query`
/// Use [`Query::search`] to begin the search
#[derive(Debug, Default)]
pub struct Query {
    pub tags: Vec<Tag>,
    pub constraints: ConstraintList,
    pub receiver: Option< Receiver<Item> >,
    search_handle: Option<JoinHandle<()>>,
}

impl Query {
    pub fn empty() -> Self {
        Query::default()
    }

    pub fn parse(query: &str) -> Query {
        Query {
            tags: Vec::new(),
            constraints: ConstraintList::parse(query),
            receiver: None,
            search_handle: None,
        }
    }

    /// Returns whether this query has changed
    pub fn parse_query(&mut self, query: &str) -> bool {
        let new_constraints = ConstraintList::parse(query);
        if new_constraints == self.constraints {
            return false;
        }
        self.constraints = new_constraints;
        true
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

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Begins the search.
    pub fn search(&mut self) {
        let (tx, rx) = mpsc::channel::<Item>();

        let constraints = self.constraints.clone();

        let it = self.tags.iter()
            .map(|tag| tag.get_all_entries());

        let handle = if constraints.is_empty() {
            let entries = if self.tags.len() > 1 {
                Entries::intersection_of(it)
            } else {
                Entries::from( it.flatten().collect::<Vec<PathBuf>>() )
                    .filter_duplicates()
            };

            thread::spawn(move ||
                send_entries(tx, entries)
            )
        } else {
            let entries = Entries::intersection_of(it);
            thread::spawn(move ||
                search_entries(tx, entries, constraints)
            )
        };

        self.search_handle = Some(handle);
        self.receiver = Some(rx);
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        self.receiver = None;

        // Join threads
        if let Some(handle) = self.search_handle.take() {
            if let Err(err) = handle.join() {
                error!("[Query::drop()] Failed to join search handle:\n {err:?}");
            }
        }
    }

}


/// Send entries in `entries` over `sender`
fn send_entries(
    sender: Sender<Item>,
    entries: Entries,
) {
    let it = entries.into_iter()
        .filter(|pb| pb.exists())
        .map(|pb| Item(0, pb));

    for item in it {
        if sender.send(item).is_err() {
            return;
        }
    }
}

/// Searches through all paths in in `entries` with the given [`ConstraintList`], and sends it over
/// a `sender`
/// Since the search can potentially take a while, also specify a break switch that will stop the
/// searching if set to `true`
fn search_entries(
    sender: Sender<Item>,
    entries: Entries,
    constraints: ConstraintList,
) {
    for item in Searcher::new(entries, constraints) {
        if sender.send(item).is_err() {
            return;
        }
    }
}






mod constraint {
    use std::{ffi::{OsStr, OsString}, path::Path, sync::OnceLock};
    use regex::Regex;

    use crate::{strmatch::{StringMatcher, Sublime}, ToPrettyString};


    /// Constraint list for file searching, given a query (see [`ConstraintList::parse()`] )
    /// - Parts enclosed in quotes `"` will be matched in their entirety (simple case insensitive
    /// `contains()` check)
    ///     If no closing quotes are found, the rest of the string is included
    /// - Parts that match `.ext` will filter files with the extension `ext`
    /// - `--file` or `-f` will constrain the search to files only; while `--dir` or `-d`,
    /// directories (folders) only
    /// - Everything else will be scored via the [`Sublime`] string matcher
    ///
    /// Any of the above fields can be negated by adding a `!` before them
    /// E.g.
    /// ```
    /// "Hello \"World\"" // Score paths with "Hello", but only those that contain "World"
    /// "!.import \"all the rest" // Only files that contain "all the rest" that aren't .import files
    /// "-d !foo" // Search only directories, and exclude those that score "foo"
    /// "dino .png .ase" // Score paths with "dino", but only .png or .ase files
    /// ```
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ConstraintList {
        /// Score target path using a fuzzy search
        pub fuzzy: Vec<Fuzzy>,
        /// Look for specific strings in target
        /// All AND-ed together
        pub exact: Vec<Exact>,
        /// Look for specific file extensions
        /// All OR-ed together
        pub extensions: Vec<Extension>,
        /// Filter files or folders
        pub filetype: Option<FileType>,
    }

    impl ConstraintList {
        /// Parses a string into a [`ConstraintList`]
        pub fn parse(str: &str) -> ConstraintList {
            let mut constraints = ConstraintList::default();
            if str.is_empty() {
                return constraints;
            }

            let mut str: String = str.to_string();
            // Exact constraint
            constraints.exact = Exact::parse(&mut str);

            for arg in str.split(' ')
                .map(|str| str.trim())
                .filter(|str| !str.is_empty())
            {
                // File type constraint
                if let Some(c) = FileType::parse(arg) {
                    constraints.filetype = Some(c);
                    continue;
                }

                // File extension constraint
                if let Some(c) = Extension::parse(arg) {
                    constraints.extensions.push(c);
                    continue;
                }

                // Everything else -> Score constraint (aka fuzzy search)
                constraints.fuzzy.push(Fuzzy::parse(arg));
            }

            constraints
        }

        pub fn score(&self, path: &Path) -> Option<isize> {
            // 1. Filter file type
            match &self.filetype {
                Some(FileType::File) => if !path.is_file() { return None; },
                Some(FileType::Dir) => if !path.is_dir() { return None; },
                None => {},
            }

            // 2. OR extensions, or AND if inverted
            if !self.extensions.is_empty() {
                let ext = path.extension()?;

                let mut any_match: bool = false;
                for constraint in self.extensions.iter() {
                    // .png .png => (true, false) => any match = true; break
                    // .png .jpg => (false, false) => no match; continue
                    // !.png .png => (false, true) => exclude; return
                    // !.png .jpg => (true, true) => any match = true; continue
                    match (constraint.matches(ext), constraint.inverted) {
                        // Match!
                        (true, false) => {
                            any_match = true;
                            break;
                        },
                        // No match, continue
                        (false, false) => {},
                        // Encountered a file extension we want to exclude
                        (false, true) => return None,
                        (true, true) => {
                            any_match = true;
                        },
                    }
                }

                if !any_match {
                    return None;
                }
            }

            // 3. AND Exacts
            let pathstr = path.to_pretty_string();
            if !self.exact.is_empty() && !self.exact.iter() .all(|c| c.matches(&pathstr)) {
                return None;
            }

            let length_penalty: isize = pathstr.len() as isize;
            
            // 4. Score fuzzy
            if self.fuzzy.is_empty() {
                return Some(-length_penalty);
            }

            self.fuzzy.iter()
                .filter_map(|f| f.score(&pathstr))
                .reduce(|acc, s| acc + s)
                .map(|s| s - length_penalty)
        }

        pub fn is_empty(&self) -> bool {
            self.fuzzy.is_empty()
                && self.exact.is_empty()
                && self.extensions.is_empty()
                && self.filetype.is_none()
        }

        pub fn clear(&mut self) {
            self.fuzzy.clear();
            self.exact.clear();
            self.extensions.clear();
            self.filetype = None;
        }
    }

    /// Score using the [`Sublime`] matcher
    /// Can be inverted to exclude matches instead
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Fuzzy {
        pub matcher: Sublime,
        pub inverted: bool,
    }

    impl Fuzzy {
        pub fn parse(str: &str) -> Fuzzy {
            let (str, inverted) = str.strip_prefix('!')
                .map_or((str, false), |s| (s, true));

            Fuzzy {
                matcher: Sublime::default() .with_query(str),
                inverted,
            }
        }

        fn score(&self, str: &str) -> Option<isize> {
            match (self.matcher.score(&str), self.inverted) {
                (None, false) => None,
                (Some(s), false) => Some(s),
                (None, true) => Some(0),
                (Some(s), true) => Some(-s),
            }
        }
    }

    /// Do a simple case insensitive `contains` check
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Exact {
        pub query: String,
        pub inverted: bool,
    }

    impl Exact {
        /// Drains the parsed sections from the string
        /// This makes it easier to deal with in [`ConstraintList::parse`]
        pub fn parse(str: &mut String) -> Vec<Exact> {
            static REGEX: OnceLock<Regex> = OnceLock::new();

            #[allow(clippy::unwrap_used)]
            let re: &Regex = REGEX.get_or_init(||
                Regex::new(r#" ?(?<invert>!)?"(?<inner>[^"]+)("( |$)|$)"#)
                    .unwrap() // Will never fail
               // (?<invert>!)?     Optional `!` to invert
               // "(?<inner>[^"]+)  Inner query
               // ("( |$)|$)        Closing quote or EOL
            );

            let mut parsed: Vec<Exact> = Vec::new();
            let mut drain_ranges = Vec::new();
            // Parse
            for cap in re.captures_iter(str) {
                #[allow(clippy::unwrap_used)]
                let inner_match = cap.name("inner").unwrap();
                let inner: &str = inner_match.as_str();
                let mut range = inner_match.range();

                let inverted: bool = cap.name("invert").is_some();
                if inverted {
                    range.start -= 1;
                }
                drain_ranges.push(range);

                parsed.push(Exact {
                    query: inner.to_lowercase(),
                    inverted,
                });
            }

            // Drain captured ranges
            for r in drain_ranges.into_iter().rev() {
                let start = r.start - 1;
                let end = (r.end + 2).min(str.len());
                str.drain(start..end);
            }

            parsed
        }

        #[inline]
        fn matches(&self, str: &str) -> bool {
            // The `!=` basically negates it
            // t != t = f
            // f != t = t
            // t != f = t
            // f != f = f
            str.to_lowercase() .contains(&self.query) != self.inverted
        }
    }

    /// Filter file extensions
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Extension {
        pub extension: OsString,
        inverted: bool,
    }

    impl Extension {
        pub fn parse(str: &str) -> Option<Extension> {
            let (str, inverted) = str.strip_prefix('!')
                .map_or((str, false), |s| (s, true));

            let ext = str.strip_prefix('.')?;
            if ext.is_empty() { return None; }

            Some(Extension {
                extension: ext.to_lowercase().into(),
                inverted,
            })
        }

        #[inline]
        fn matches(&self, extension:  &OsStr) -> bool {
            // The `!=` basically negates it
            self.extension.eq_ignore_ascii_case(extension) != self.inverted
            // (self.extension == extension.to_ascii_lowercase()) != self.inverted
        }
    }

    /// Filter file types
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FileType {
        File,
        Dir,
    }

    impl FileType {
        pub fn parse(str: &str) -> Option<FileType> {
            match str {
                "--file" | "-f" => Some(FileType::File),
                "--dir" | "-d" => Some(FileType::Dir),
                _ => None,
            }
        }
    }




    #[cfg(test)]
    mod tests {
        use std::ffi::OsString;
        use std::path::Path;

        use crate::search::constraint::Fuzzy;
        use crate::strmatch::Sublime;

        use super::ConstraintList;
        use super::{ Exact, Extension, FileType };


        #[test]
        fn parsing() {
            let c = ConstraintList::parse("score .rs .png");
            assert_eq!(c.fuzzy, vec![
                Fuzzy {
                    matcher: Sublime::default() .with_query("score"),
                    inverted: false,
                }
            ]);
            assert_eq!(c.exact, vec![]);
            assert_eq!(c.extensions, vec![
                Extension { extension: OsString::from("rs"), inverted: false },
                Extension { extension: OsString::from("png"), inverted: false } 
            ]);
            assert_eq!(c.filetype, None);


            let c = ConstraintList::parse("score \"exact\" .txt -f --wot");
            assert_eq!(c.fuzzy, vec![
                Fuzzy {
                    matcher: Sublime::default() .with_query("score"),
                    inverted: false,
                },
                Fuzzy {
                    matcher: Sublime::default() .with_query("--wot"),
                    inverted: false,
                }
            ]);
            assert_eq!(c.exact, vec![
                Exact { query: "exact".to_string(), inverted: false }
            ]);
            assert_eq!(c.extensions, vec![
                Extension { extension: OsString::from("txt"), inverted: false } 
            ]);
            assert_eq!(c.filetype, Some(FileType::File));


            // Invalid queryies
            let c = ConstraintList::parse("\"\"");
            dbg!(&c.fuzzy);
            assert_eq!(c.fuzzy, vec![
                Fuzzy {
                    matcher: Sublime::default() .with_query("\"\""),
                    inverted: false,
                }
            ]);
            assert_eq!(c.exact, vec![]);
            assert_eq!(c.extensions, vec![]);
            assert_eq!(c.filetype, None);
        }

        #[test]
        fn searching() {
            let paths = vec![
                Path::new("C:/Users/ddxte/Pictures/art stuff/dino_cool.png"),
                Path::new("C:/Users/ddxte/Documents/Projects/music_tools.exe"),
                Path::new("C:/Users/ddxte/Pictures/"),
                Path::new("C:/Users/ddxte/Pictures/rendererwoooow.png"),
                Path::new("C:/Users/ddxte/Pictures/bread.JPG"),
            ];

            let a = |query: &str, v: &[&&Path]| {
                let constraints = ConstraintList::parse(query);
                let filtered: Vec<&&Path> = paths.iter()
                    .filter(|p| constraints.score(p).is_some())
                    .collect();

                assert_eq!(filtered, v, "ooooooh no query {query} failed whatever we shall do");
            };

            a("--dir", &vec![
                &Path::new("C:/Users/ddxte/Pictures/"),
            ]);

            a("--file", &vec![
                &Path::new("C:/Users/ddxte/Pictures/art stuff/dino_cool.png"),
                &Path::new("C:/Users/ddxte/Documents/Projects/music_tools.exe"),
                &Path::new("C:/Users/ddxte/Pictures/rendererwoooow.png"),
                &Path::new("C:/Users/ddxte/Pictures/bread.JPG"),
            ]);

            a(".png .jpg", &vec![
                &Path::new("C:/Users/ddxte/Pictures/art stuff/dino_cool.png"),
                &Path::new("C:/Users/ddxte/Pictures/rendererwoooow.png"),
                &Path::new("C:/Users/ddxte/Pictures/bread.JPG"),
            ]);

            a("\"din\" \"ool\"", &vec![
                &Path::new("C:/Users/ddxte/Pictures/art stuff/dino_cool.png"),
            ]);

            a("oopng", &vec![
                &Path::new("C:/Users/ddxte/Pictures/art stuff/dino_cool.png"),
                &Path::new("C:/Users/ddxte/Pictures/rendererwoooow.png"),
            ]);

            a("dp \"music\" .exe -f", &vec![
                &Path::new("C:/Users/ddxte/Documents/Projects/music_tools.exe"),
            ]);

        }

        #[test]
        fn inverting() {
            let dinocool = Path::new("C:/Users/ddxte/Pictures/art stuff/dino_cool.png");
            let tisdino = Path::new("C:/Users/ddxte/Pictures/art stuff/tankinsands/dino.ase");
            let pics = Path::new("C:/Users/ddxte/Pictures/");
            let bread = Path::new("C:/Users/ddxte/Pictures/bread.JPG");

            let c = ConstraintList::parse("!\"dino\"");
            assert_eq!( c.score(tisdino), None );
            assert_eq!( c.score(pics), Some(-24) ); // pics is 24 chars long

            let c = ConstraintList::parse("!dino");
            assert!( c.score(tisdino).is_some_and(|score| score < 0) );
            assert_eq!( c.score(pics), Some(-24) ); // again

            let c = ConstraintList::parse("!.png");
            assert_eq!( c.score(dinocool), None );
            assert_eq!( c.score(bread), Some(-33) ); // bread is 33 chars long
            assert_eq!( c.score(pics), None );

            let c = ConstraintList::parse("!.png !.ase");
            assert_eq!( c.score(dinocool), None );
            assert_eq!( c.score(tisdino), None );
            assert_eq!( c.score(pics), None );
            assert_eq!( c.score(bread), Some(-33) );

        }

        #[test]
        fn test_parse() {
            // let mut str: String = "abc 'bla' --other 'all the rest" .to_string();
            let mut str: String = "what is love 'baby don't hurt me'" .to_string();
            eprintln!("str = {:?}", str);

            str.push(' ');

            loop {
                let chars: Vec<(usize, char)> = str.char_indices().collect();
                let mut chars = chars.windows(2);

                if chars.find(|pair| pair[0].1 == ' ' && pair[1].1 == '\'') .is_none() {
                    break;
                }
                let Some(opening) = chars.next() else {
                    break;
                };

                let Some(closing) = chars.find(|pair| pair[0].1 == '\'' && pair[1].1 == ' ') else {
                    let opening_idx = opening[1].0;
                    let query = &str[opening_idx..];
                    eprintln!("query = {:?}", query);

                    let opening_idx = opening[0].0;
                    str.drain(opening_idx..);
                    eprintln!("str = {:?}", str);
                    break;
                };

                let opening_idx = opening[1].0;
                let closing_idx = closing[0].0;
                let query = &str[opening_idx..closing_idx];
                eprintln!("query = {:?}", query);

                let opening_idx = opening[0].0;
                let closing_idx = closing[1].0;
                str.drain(opening_idx..=closing_idx);
                eprintln!("str = {:?}", str);
            }

            println!("The end");

        }


        #[test]
        fn test_regex() {
            // YEEEEEEEEEESSSSSSSS
            use regex::Regex;

            let re = Regex::new(r#"(?i)( |^)(?<inverted>!)?"(?<inner>[^"]+)("( |$)|$)"#) .unwrap();
            let mut hay = r#"abc "query" def !"exclude me"#.to_string();
            eprintln!("hay = {:?}", hay);

            let mut drain_ranges = Vec::new();
            for cap in re.captures_iter(&hay) {
                let inner_match = cap.name("inner") .unwrap();
                let inner = inner_match.as_str();
                let mut range = inner_match.range();

                let is_inverted = cap.name("inverted").is_some();
                if is_inverted {
                    range.start -= 1;
                }

                eprintln!("inner = {:?}", inner);
                eprintln!("range = {:?}", range);
                eprintln!("is_inverted = {:?}", is_inverted);

                drain_ranges.push(range);
            }

            for r in drain_ranges.into_iter().rev() {
                let start = r.start - 1;
                let end = (r.end + 2).min(hay.len());
                hay.drain(start..end);
            }

            eprintln!("hay = {:?}", hay);

        }
    }



}
