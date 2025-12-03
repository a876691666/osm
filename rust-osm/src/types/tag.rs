//! Tag type for OSM elements

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Set of tags that are generally considered uninteresting
pub static UNINTERESTING_TAGS: &[&str] = &[
    "source",
    "source_ref",
    "source:ref",
    "history",
    "attribution",
    "created_by",
    "tiger:county",
    "tiger:tlid",
    "tiger:upload_uuid",
];

/// A key+value item attached to osm nodes, ways and relations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    #[serde(rename = "k")]
    pub key: String,
    #[serde(rename = "v")]
    pub value: String,
}

impl Tag {
    /// Creates a new Tag
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Tag {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// A collection of Tag objects with helper functions
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags(pub Vec<Tag>);

impl Tags {
    /// Creates an empty Tags collection
    pub fn new() -> Self {
        Tags(Vec::new())
    }

    /// Creates Tags from a vector of Tag
    pub fn from_vec(v: Vec<Tag>) -> Self {
        Tags(v)
    }

    /// Find will return the value for the key.
    /// Returns an empty string if not found.
    pub fn find(&self, key: &str) -> String {
        for tag in &self.0 {
            if tag.key == key {
                return tag.value.clone();
            }
        }
        String::new()
    }

    /// FindTag will return the Tag for the given key.
    /// Can be used to determine if a key exists, even with an empty value.
    /// Returns None if not found.
    pub fn find_tag(&self, key: &str) -> Option<&Tag> {
        for tag in &self.0 {
            if tag.key == key {
                return Some(tag);
            }
        }
        None
    }

    /// HasTag will return true if a tag exists for the given key.
    pub fn has_tag(&self, key: &str) -> bool {
        for tag in &self.0 {
            if tag.key == key {
                return true;
            }
        }
        false
    }

    /// Map returns the tags as a key/value HashMap.
    pub fn map(&self) -> HashMap<String, String> {
        let mut result = HashMap::with_capacity(self.0.len());
        for tag in &self.0 {
            result.insert(tag.key.clone(), tag.value.clone());
        }
        result
    }

    /// AnyInteresting will return true if there is at least one interesting tag.
    pub fn any_interesting(&self) -> bool {
        for tag in &self.0 {
            if !UNINTERESTING_TAGS.contains(&tag.key.as_str()) {
                return true;
            }
        }
        false
    }

    /// Sorts the tags by key then value
    pub fn sort_by_key_value(&mut self) {
        self.0.sort_by(|a, b| {
            if a.key == b.key {
                a.value.cmp(&b.value)
            } else {
                a.key.cmp(&b.key)
            }
        });
    }

    /// Returns the number of tags
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds a new tag
    pub fn push(&mut self, tag: Tag) {
        self.0.push(tag);
    }
}

impl Serialize for Tags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as a key/value object (like overpass osmjson)
        self.map().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Tags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize from a key/value object
        let map: HashMap<String, String> = HashMap::deserialize(deserializer)?;
        let tags: Vec<Tag> = map.into_iter().map(|(k, v)| Tag::new(k, v)).collect();
        Ok(Tags(tags))
    }
}

impl IntoIterator for Tags {
    type Item = Tag;
    type IntoIter = std::vec::IntoIter<Tag>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Tags {
    type Item = &'a Tag;
    type IntoIter = std::slice::Iter<'a, Tag>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl From<Vec<Tag>> for Tags {
    fn from(v: Vec<Tag>) -> Self {
        Tags(v)
    }
}

impl From<HashMap<String, String>> for Tags {
    fn from(map: HashMap<String, String>) -> Self {
        let tags: Vec<Tag> = map.into_iter().map(|(k, v)| Tag::new(k, v)).collect();
        Tags(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_find() {
        let tags = Tags::from_vec(vec![
            Tag::new("name", "Test"),
            Tag::new("highway", "primary"),
        ]);
        assert_eq!(tags.find("name"), "Test");
        assert_eq!(tags.find("highway"), "primary");
        assert_eq!(tags.find("nonexistent"), "");
    }

    #[test]
    fn test_tag_map() {
        let tags = Tags::from_vec(vec![
            Tag::new("name", "Test"),
            Tag::new("highway", "primary"),
        ]);
        let map = tags.map();
        assert_eq!(map.get("name"), Some(&"Test".to_string()));
        assert_eq!(map.get("highway"), Some(&"primary".to_string()));
    }

    #[test]
    fn test_any_interesting() {
        let tags = Tags::from_vec(vec![Tag::new("source", "test")]);
        assert!(!tags.any_interesting());

        let tags = Tags::from_vec(vec![Tag::new("name", "test")]);
        assert!(tags.any_interesting());
    }

    #[test]
    fn test_json_serialization() {
        let tags = Tags::from_vec(vec![
            Tag::new("name", "Test"),
            Tag::new("highway", "primary"),
        ]);
        let json = serde_json::to_string(&tags).unwrap();
        // JSON should be an object with key/value pairs
        assert!(json.contains("name"));
        assert!(json.contains("Test"));
    }
}
