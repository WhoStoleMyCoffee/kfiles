use std::collections::HashMap;
use std::fmt::Write;
use std::fs::{create_dir_all, read_dir};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};

pub mod entries;
pub mod id;
pub mod tag;

use iced::Command;
use id::TagID;
use crate::log::notification::Notification;
use crate::log::Level as LogLevel;
use crate::{error, send_message, trace};
use crate::app::Message as AppMessage;

pub use tag::{ LoadError, RenameError, SaveError };
pub use tag::Tag;


static TAGS_CACHE: RwLock<Vec<Tag>> = RwLock::new(Vec::new());


/// TODO documentation
pub fn tags_cache() -> RwLockReadGuard<'static, Vec<tag::Tag>> {
    TAGS_CACHE.read() .unwrap_or_else(|err| {
        error!("Error while getting global tags cache:\n RwLock was poisonned");
        err.into_inner()
    })
}


/// TODO documentation
pub fn set_tags_cache(new_tags: Vec<Tag>) {
    match TAGS_CACHE.write() {
        Ok(mut tags) => {
            *tags = new_tags
        },
        Err(mut err) => {
            error!("Error while setting global tags cache:\n RwLock was poisonned");
            **err.get_mut() = new_tags;
            TAGS_CACHE.clear_poison();
        },
    }
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
    use crate::APP_NAME;

    directories::BaseDirs::new()
        .expect("failed to get base dirs")
        .config_dir()
        .to_path_buf()
        .join( format!("{APP_NAME}/tags/") )
}

/// Returns the base dir where all tags are stored (for tests only)
#[cfg(test)]
#[inline]
pub fn get_save_dir() -> PathBuf {
    PathBuf::from("C:/Users/ddxte/Documents/Projects/kfiles new/tests/tags/")
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



/// TODO documentation
/// TODO clean up
pub fn load_tags() -> TagLoadResult {
    use crate::{ send_message, error };

    trace!("[tagging::load_tags()] Loading tags...");

    let paths = match get_all_tags() {
        Ok(v) => v,
        Err(err) => {
            error!("[tagging::load_tags()] Failed to load tags dir:\n {:?}", err);
            return TagLoadResult::IO(err);
        }
    };

    let mut errors: HashMap<PathBuf, LoadError> = HashMap::new();
    let mut tags: Vec<Tag> = Vec::new();

    for path in paths.into_iter() {
        match Tag::load_from_path(&path) {
            Ok(tag) => tags.push(tag),
            Err(err) => {
                error!("[tagging::load_tags()] Failed to load tag at \"{}\":\n {:?}", path.display(), err);
                errors.insert(path, err);
            }
        }
    }

    TagLoadResult::Ok(tags, errors)
}


/// TODO documentation
pub enum TagLoadResult {
    /// initial load error
    IO(io::Error),
    /// per tag
    Ok(Vec<Tag>, HashMap<PathBuf, LoadError>),
}

impl TagLoadResult {
    pub fn get_tags(self) -> Option<Vec<Tag>> {
        match self {
            TagLoadResult::IO(_) => None,
            TagLoadResult::Ok(v, _) => Some(v),
        }
    }

    pub fn get_tags_errors(self) -> Option<HashMap<PathBuf, LoadError>> {
        match self {
            TagLoadResult::IO(_) => None,
            TagLoadResult::Ok(_, v) if v.is_empty() => None,
            TagLoadResult::Ok(_, v) => Some(v),
        }
    }

    pub fn log_errors<V>(&self) -> Option<V>
    where V: TagLoadResultLog + Default
    {
        let mut v = V::default();

        match self {
            TagLoadResult::IO(err) => {
                let content = format!("Failed to load tags:\n {}", err);
                v.push(content);
            }

            TagLoadResult::Ok(_, errs) if errs.is_empty() => {
                // No errors yay
                return None;
            }

            TagLoadResult::Ok(_, _) => {
                let content = "Failed to load all tags. See logs for more details".to_string();
                v.push(content);
            }
        }

        Some(v)
    }

    pub fn log_errors_verbose<V>(&self) -> Option<V>
    where V: TagLoadResultLog + Default
    {
        todo!()
    }

    /* pub fn notify_on_error(self, commands: &mut Vec<Command<AppMessage>>) -> Self {
        match &self {
            TagLoadResult::IO(err) => {
                let content = format!("Failed to load tags:\n {}", err);
                commands.push(send_message!(notif = Notification::new(
                    LogLevel::Error,
                    content,
                )))
            },

            TagLoadResult::Ok(_, errs) if errs.is_empty() => {},

            TagLoadResult::Ok(_, _) => {
                commands.push(send_message!(notif = Notification::new(
                    LogLevel::Error,
                    "Failed to load all tags. See logs for more details".to_string()
                )))
            }
        }

        self
    }

    pub fn notify_verbose(self, commands: &mut Vec<Command<AppMessage>>) -> Self {
        match &self {
            TagLoadResult::IO(err) => {
                let content = format!("Failed to load tags:\n {}", err);
                commands.push(send_message!(notif = Notification::new(
                    LogLevel::Error,
                    content,
                )))
            },

            TagLoadResult::Ok(_, errs) => {
                commands.extend(errs.iter()
                    .map(|(p, err)| {
                        let content = format!("Failed to load tag at \"{}\":\n {}", p.display(), err);
                        send_message!(notif = Notification::new(
                            LogLevel::Error,
                            content,
                        ))
                    })
                )
            }
        } // end match &self

        self
    } */
}


trait TagLoadResultLog {
    fn push(&mut self, line: String);
}

impl TagLoadResultLog for Vec<Notification> {
    fn push(&mut self, content: String) {
        self.push(Notification::new(
            LogLevel::Error,
            content,
        ));
    }
}

impl TagLoadResultLog for Vec<String> {
    fn push(&mut self, content: String) {
        self.push(content);
    }
}

impl TagLoadResultLog for String {
    fn push(&mut self, line: String) {
        writeln!(self, "{}", line) .expect("Writing a String to a String shouldn't fail");
    }
}








#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::{Path, PathBuf}};

    use crate::tagging::{entries::Entries, id::TagID, tag::{SelfReferringSubtag, Tag}};

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
        assert!(!tag.contains("C:/Users/ddxte/Music/"));

        // Adding already tagged dirs
        let add_err = tag.add_entry("C:/Users/ddxte/Documents/");
        match add_err {
            Ok(false) => {},
            other => panic!("Expected add_err to be Ok(false). Found {:?}", other),
        }

        let add_err = tag.add_entry("C:/Users/ddxte/Documents/Projects/music_tools.exe");
        match add_err {
            Ok(true) => {},
            other => panic!("Expected add_err to be Ok(true). Found {:?}", other),
        }

        // Entries haven't changed
        assert_eq!(tag.entries.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Videos/"),
            PathBuf::from("C:/Users/ddxte/Documents/Projects/music_tools.exe"),
        ]);

        // Removing nonexistent entry
        assert!( !tag.remove_entry(&Path::new("C:/Users/ddxte/Music/")) );
        assert!( !tag.remove_entry(&Path::new("C:/Users/ddxte/")) );
    }

    #[test]
    fn entries_duplicates() {
        let mut e = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/"),
        ]);

        let duplicates = e.remove_duplicates();
        assert_eq!(duplicates, &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
        ]);
        assert_eq!(e.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
            PathBuf::from("C:/Users/ddxte/"),
        ]);
    }

    #[test]
    fn entries_trim() {
        let e = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/"),
        ]);

        assert_eq!(
            e.trim().as_ref(),
            &[ PathBuf::from("C:/Users/ddxte/") ]
        );
    }

    #[test]
    fn entries_union() {
        let a = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);

        let b = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Documents/Projects/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);
        
        let mut c = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Downloads/"),
        ]);

        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
        ]);
        // Works both ways
        let union_ab = Entries::union_of(vec![ a.clone(), b.clone() ]);
        assert_eq!(HashSet::from_iter(union_ab), expected );
        let union_ba = Entries::union_of(vec![ b.clone(), a.clone() ]);
        assert_eq!(HashSet::from_iter(union_ba), expected );

        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Downloads/"),
        ]);
        // Test all 6 combinations
        let union_abc = Entries::union_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(union_abc), expected );
        let union_acb = Entries::union_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(union_acb), expected );
        let union_bac = Entries::union_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(union_bac), expected );
        let union_bca = Entries::union_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(union_bca), expected );
        let union_cab = Entries::union_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(union_cab), expected );
        let union_cba = Entries::union_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(union_cba), expected );

        c.as_mut().push(PathBuf::from("C:/Users/ddxte/"));
        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/")
        ]);
        let union_c = Entries::union_of(vec![ a.clone(), c.clone(), b.clone() ]);
        assert_eq!(HashSet::from_iter(union_c), expected );
    }

    #[test]
    fn entries_intersection() {
        let a = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);

        let b = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Documents/Projects/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);
        
        let c = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Downloads/"),
            PathBuf::from("C:/Users/ddxte/"),
        ]);

        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/Projects/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);
        // Works both ways
        let intersection_ab = Entries::intersection_of(vec![ a.clone(), b.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_ab), expected );
        let intersection_ba = Entries::intersection_of(vec![ b.clone(), a.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_ba), expected );

        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/Projects/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);
        // Test all 6 combinations
        let intersection_abc = Entries::intersection_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_abc), expected );
        let intersection_acb = Entries::intersection_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_acb), expected );
        let intersection_bac = Entries::intersection_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_bac), expected );
        let intersection_bca = Entries::intersection_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_bca), expected );
        let intersection_cab = Entries::intersection_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_cab), expected );
        let intersection_cba = Entries::intersection_of(vec![ a.clone(), b.clone(), c.clone() ]);
        assert_eq!(HashSet::from_iter(intersection_cba), expected );
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

    #[test]
    fn subtags() {
        let mut tag = Tag::create("test-subtags");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/") .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG") .unwrap();

        // Creating subtag
        let mut tag2 = Tag::create("test-subtags-2");
        tag.add_subtag(&tag2.id) .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/") .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Projects/") .unwrap();

        tag.save().unwrap();
        tag2.save().unwrap();

        assert!(tag2.is_direct_subtag_of(&tag));
        let mut it = tag.iter_all_subtags().map(|t| t.id);
        assert!( it.any(|id| id == tag2.id) );

        {
            let tagid = tag.id.clone();
            let res = tag.add_subtag(&tagid);
            assert!(
                matches!(res, Err(SelfReferringSubtag)),
                "Expected Err(SelfReferringSubtag) but found {:?}", res
            );
        }
    }

    #[test]
    fn subtags_deep() {
        let mut tag = Tag::create("test-subtags-deep");
        let mut tag2 = Tag::create("test-subtags-deep-2");
        tag.add_subtag(&tag2.id) .unwrap();
        let tag3 = Tag::create("test-subtags-deep-3");
        tag2.add_subtag(&tag3.id) .unwrap();

        tag.save().unwrap();
        tag2.save().unwrap();
        tag3.save().unwrap();

        assert_eq!(tag.iter_all_subtags().count(), 2);
        assert_eq!(tag2.iter_all_subtags().count(), 1);
        assert_eq!(tag3.iter_all_subtags().count(), 0);
    }

    #[test]
    fn subtags_entries() {
        let mut tag = Tag::create("test-subtags-entries");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG")
            .unwrap();

        // Creating subtag
        let mut tag2 = Tag::create("test-subtags-entries-2");
        tag.add_subtag(&tag2.id) .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/")
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Projects/")
            .unwrap();

        // Save so we can get entries
        tag.save().unwrap();
        tag2.save().unwrap();

        // Getting merged entries
        let all_entries = tag.get_all_entries();
        let expected = HashSet::<PathBuf>::from([
            PathBuf::from("C:/Users/ddxte/Documents/Apps/KFiles/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
            PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            PathBuf::from("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/"),
        ]);
        assert_eq!(
            HashSet::from_iter(all_entries.clone()),
            expected,
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
    fn cyclic_subtags() {
        fn get_subtags(tag: &Tag) -> Vec<TagID> {
            tag.iter_all_subtags().map(|t| t.id).collect()
        }

        // A <=> B
        let mut tag_a = Tag::create("cyclic-a");
        let mut tag_b = Tag::create("cyclic-b");

        tag_a.add_subtag(&tag_b.id) .unwrap();
        tag_b.add_subtag(&tag_a.id) .unwrap();
        tag_a.save() .unwrap();
        tag_b.save() .unwrap();

        assert_eq!(get_subtags(&tag_a), vec![ tag_b.id.clone() ]);
        assert_eq!(get_subtags(&tag_b), vec![ tag_a.id.clone() ]);

        // A <=> B => C
        let mut tag_c = Tag::create("cyclic-c");
        tag_b.add_subtag(&tag_c.id) .unwrap();
        tag_b.save() .unwrap();
        tag_c.save() .unwrap();

        assert_eq!(get_subtags(&tag_a), vec![
            tag_b.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(get_subtags(&tag_b), vec![
            tag_a.id.clone(),
            tag_c.id.clone(),
        ]);

        // A <=> B => C => A
        tag_c.add_subtag(&tag_a.id) .unwrap();
        tag_c.save() .unwrap();
        tag_a.save() .unwrap();

        assert_eq!(get_subtags(&tag_a), vec![
            tag_b.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(get_subtags(&tag_b), vec![
            tag_a.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(get_subtags(&tag_c), vec![
            tag_a.id.clone(),
            tag_b.id.clone(),
        ]);

        // A <=> B <=> C <=> A full 3-way cycle
        tag_c.add_subtag(&tag_b.id.clone()) .unwrap();
        tag_a.add_subtag(&tag_c.id.clone()) .unwrap();
        tag_a.save() .unwrap();
        tag_b.save() .unwrap();
        tag_c.save() .unwrap();

        assert_eq!(get_subtags(&tag_a), vec![
            tag_b.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(get_subtags(&tag_b), vec![
            tag_a.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(get_subtags(&tag_c), vec![
            tag_a.id.clone(),
            tag_b.id.clone(),
        ]);
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
    fn path_get_tags() {
        let mut tag_a = Tag::create("tag-a");
        tag_a.add_entry("C:/Users/ddxte/Documents/") .unwrap();
        let mut tag_b = Tag::create("tag-b");
        tag_b.add_entry("C:/Users/ddxte/Documents/Projects/") .unwrap();
        let mut tag_c = Tag::create("tag-b");
        tag_c.add_entry("C:/Users/ddxte/Pictures/") .unwrap();

        let all_tags = vec![ tag_a, tag_b, tag_c, ];
        let path = Path::new("C:/Users/ddxte/Documents/Projects/kfiles/");

        let contained_tags: HashSet<TagID> = get_tags_for_path(&path, &all_tags)
            .into_iter()
            .map(|i| all_tags[i].id.clone())
            .collect();
        let expected: HashSet<TagID> = HashSet::from_iter(vec![
            TagID::new("tag-a"),
            TagID::new("tag-b"),
        ]);

        assert_eq!(contained_tags, expected);
    }
}
