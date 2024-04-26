use crate::tag::{ TagID, Tag };


#[derive(Debug, Default)]
pub struct Query {
    pub tags: Vec<TagID>,
    pub query: String,
}

impl Query {
    #[inline(always)]
    pub fn empty() -> Self {
        Query::default()
    }

    /// Returns whether the tag was added to the query
    /// If it wasn't, returns `false`
    pub fn add_tag(&mut self, id: TagID) -> bool {
        if self.tags.contains(&id) {
            return false;
        }
        self.tags.push(id);
        true
    }

    /// Returns whether the tag was found and removed
    /// If the tag was not contained, returns `false`
    pub fn remove_tag<ID>(&mut self, id: &ID) -> bool
        where ID: PartialEq<TagID>
    {
        if let Some(index) = self.tags.iter().position(|t| id == t) {
            self.tags.remove(index);
            return true;
        }
        false
    }
}

