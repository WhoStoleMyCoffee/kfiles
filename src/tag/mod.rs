use std::fs::{create_dir_all, read_dir};
use std::io;
use std::path::PathBuf;

pub mod entries;
pub mod id;
pub mod tag;

use id::TagID;


pub type RenameError = tag::RenameError;
pub type LoadError = tag::LoadError;
pub type SaveError = tag::SaveError;
pub type AddEntryError = entries::AddEntryError;

pub type Tag = tag::Tag;


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
    PathBuf::from("C:/Users/ddxte/Documents/Projects/tag-explorer/tests/tags/")
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










#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::{Path, PathBuf}};

    use crate::tag::{entries::{AddEntryError, Entries}, id::TagID, tag::{SelfReferringSubtag, Tag}};

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

        // Adding already tagged dirs
        let add_err = tag.add_entry("C:/Users/ddxte/Documents/");
        match add_err {
            Err(AddEntryError::AlreadyContained) => {},
            other => panic!("Expected add_err to be Err(AddEntryError::AlreadyContained). Found {:?}", other),
        }

        let add_err = tag.add_entry("C:/Users/ddxte/Documents/Projects/music_tools.exe");
        match add_err {
            Err(AddEntryError::AlreadyContained) => {},
            other => panic!("Expected add_err to be Err(AddEntryError::AlreadyContained). Found {:?}", other),
        }

        // Entries haven't changed
        assert_eq!(tag.entries.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Videos/"),
        ]);

        tag.add_entry(PathBuf::from("C:/Users/ddxte/")) .unwrap();
        assert_eq!(tag.entries.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/"),
        ]);

        assert!( !tag.remove_entry(&Path::new("C:/Users/ddxte/Documents/")) );
        assert!( tag.remove_entry(&Path::new("C:/Users/ddxte/")) );
        assert!( tag.entries.is_empty() );
    }

    #[test]
    fn entries_filter_duplicates() {
        let mut e = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
            PathBuf::from("C:/Users/ddxte/Documents/"),
        ]);

        let duplicates = e.filter_duplicates();
        assert_eq!(duplicates, &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
        ]);
        assert_eq!(e.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
        ]);

        e.0.push( PathBuf::from("C:/Users/ddxte/") );
        let duplicates = e.filter_duplicates();
        assert_eq!(duplicates, &[
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
        ]);
        assert_eq!(e.as_ref(), &[
            PathBuf::from("C:/Users/ddxte/"),
        ]);

    }

    #[test]
    fn subtags_dirs() {
        let mut tag = Tag::create("test-subtags-dirs");
        tag.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/")
            .unwrap();
        tag.add_entry("C:/Users/ddxte/Pictures/bread.JPG")
            .unwrap();

        // Creating subtag
        let mut tag2 = Tag::create("test-subtags-dirs-2")
            .as_subtag_of(&mut tag)
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Apps/KFiles/screenshots/")
            .unwrap();
        tag2.add_entry("C:/Users/ddxte/Documents/Projects/")
            .unwrap();

        assert!(tag2.is_direct_subtag_of(&tag));

        {
            let res = Tag::create(tag.id.clone()) .as_subtag_of(&mut tag);
            assert!(
                matches!(res, Err(SelfReferringSubtag)),
                "Expected Err(SelfReferringSubtag) but found {:?}", res
                );
        }

        // Save so we can get entries
        tag.save().unwrap();
        tag2.save().unwrap();

        // Getting merged entries
        let all_entries = tag.get_all_entries();
        assert_eq!(
            *all_entries,
            vec![
                PathBuf::from("C:/Users/ddxte/Documents/Apps/KFiles/"),
                PathBuf::from("C:/Users/ddxte/Pictures/bread.JPG"),
                PathBuf::from("C:/Users/ddxte/Documents/Projects/"),
            ]
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
        // A <=> B
        let mut tag_a = Tag::create("cyclic-a");
        let mut tag_b = Tag::create("cyclic-b");

        tag_a.add_subtag(&tag_b.id) .unwrap();
        tag_b.add_subtag(&tag_a.id) .unwrap();
        tag_a.save() .unwrap();
        tag_b.save() .unwrap();

        assert_eq!(tag_a.get_all_subtags(), vec![ tag_b.id.clone() ]);
        assert_eq!(tag_b.get_all_subtags(), vec![ tag_a.id.clone() ]);

        // A <=> B => C
        let mut tag_c = Tag::create("cyclic-c");
        tag_b.add_subtag(&tag_c.id) .unwrap();
        tag_b.save() .unwrap();
        tag_c.save() .unwrap();

        assert_eq!(tag_a.get_all_subtags(), vec![
            tag_b.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(tag_b.get_all_subtags(), vec![
            tag_a.id.clone(),
            tag_c.id.clone(),
        ]);

        // A <=> B => C => A
        tag_c.add_subtag(&tag_a.id) .unwrap();
        tag_c.save() .unwrap();
        tag_a.save() .unwrap();

        assert_eq!(tag_a.get_all_subtags(), vec![
            tag_b.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(tag_b.get_all_subtags(), vec![
            tag_a.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(tag_c.get_all_subtags(), vec![
            tag_a.id.clone(),
            tag_b.id.clone(),
        ]);

        // A <=> B <=> C <=> A full 3-way cycle
        tag_c.add_subtag(&tag_b.id.clone()) .unwrap();
        tag_a.add_subtag(&tag_c.id.clone()) .unwrap();
        tag_a.save() .unwrap();
        tag_b.save() .unwrap();
        tag_c.save() .unwrap();

        assert_eq!(tag_a.get_all_subtags(), vec![
            tag_b.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(tag_b.get_all_subtags(), vec![
            tag_a.id.clone(),
            tag_c.id.clone(),
        ]);
        assert_eq!(tag_c.get_all_subtags(), vec![
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
    fn entries_operations() {
        let a = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);

        let b = Entries::from(vec![
            PathBuf::from("C:/Users/ddxte/Pictures/"),
            PathBuf::from("C:/Users/ddxte/Documents/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
        ]);

        // OR
        let c = HashSet::from_iter(a.or(&b));
        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/"),
        ]);
        assert!(c.is_subset(&expected));
        assert!(HashSet::from_iter(b.or(&a)).is_subset(&c));

        // AND
        let c = HashSet::from_iter(a.and(&b));
        let expected: HashSet<PathBuf> = HashSet::from_iter(vec![
            PathBuf::from("C:/Users/ddxte/Documents/TankInSands/"),
            PathBuf::from("C:/Users/ddxte/Music/"),
            PathBuf::from("C:/Users/ddxte/Pictures/bread.jpg"),
        ]);
        assert!(c.is_subset(&expected));
        assert!(HashSet::from_iter(b.and(&a)).is_subset(&c));
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
}
