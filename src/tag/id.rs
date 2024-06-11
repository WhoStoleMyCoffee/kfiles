use std::fmt::Display;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use convert_case::{Case, Casing};

use super::get_save_dir;
use super::tag::{LoadError, Tag};


#[derive(Debug, Default, PartialEq, Eq, Clone, Hash)]
pub struct TagID(pub(super) String);

impl TagID {
    pub fn new(value: &str) -> Self {
        TagID(value.to_string())
    }

    pub fn parse<T>(value: T) -> Self
    where
        T: AsRef<str>,
    {
        TagID(value.as_ref().to_case(Case::Kebab))
    }

    pub fn make_unique_in<T>(mut self, tags: &[T]) -> Self
    where T: PartialEq<TagID>
    {
        let mut count: u32 = 0;
        let mut new_id = self.clone();

        loop {
            // We have a duplicate
            if tags.iter().any(|t| *t == new_id) {
                count += 1;
                new_id = TagID(format!("{}-{}", self.0, count));
            } else {
                break;
            }
        }

        self = new_id;
        self
    }

    pub fn get_path(&self) -> PathBuf {
        get_save_dir().join(format!("{}.json", self.0))
    }

    pub fn exists(&self) -> bool {
        self.get_path().exists()
    }

    pub fn make_unique(&mut self) {
        todo!()
    }

    #[inline]
    pub fn load(&self) -> Result<Tag, LoadError> {
        Tag::load(self)
    }
}

impl From<&str> for TagID {
    fn from(value: &str) -> Self {
        TagID::parse(value)
    }
}

impl From<&TagID> for PathBuf {
    fn from(value: &TagID) -> Self {
        value.get_path()
    }
}

/// Invalid file name error
pub struct InvalidFileName;

impl TryFrom<&Path> for TagID {
    type Error = InvalidFileName;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        Ok(TagID(
            value
                .file_stem()
                .and_then(|osstr| osstr.to_str())
                .ok_or(InvalidFileName)?
                .to_string(),
        ))
    }
}

impl AsRef<String> for TagID {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

impl Deref for TagID {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for TagID {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl PartialEq<Tag> for TagID {
    fn eq(&self, other: &Tag) -> bool {
        other.id == *self
    }
}

