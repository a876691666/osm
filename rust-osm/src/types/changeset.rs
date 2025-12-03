//! Changeset type for OSM data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Bounds, ChangesetID, ObjectID, Tags, Type, UserID};

/// Changesets is a collection with helper functions
pub type Changesets = Vec<Changeset>;

/// A Changeset is a set of metadata around a set of osm changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changeset {
    #[serde(rename = "type", default)]
    pub type_marker: ChangesetTypeMarker,
    pub id: ChangesetID,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user: String,
    #[serde(default, skip_serializing_if = "is_zero_user_id")]
    pub uid: UserID,
    pub created_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    #[serde(default)]
    pub open: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub num_changes: i32,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub min_lat: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub max_lat: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub min_lon: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub max_lon: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub comments_count: i32,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discussion: Option<ChangesetDiscussion>,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_zero_user_id(v: &UserID) -> bool {
    v.0 == 0
}

/// Marker type to serialize "changeset" as the type field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChangesetTypeMarker;

impl Serialize for ChangesetTypeMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("changeset")
    }
}

impl<'de> Deserialize<'de> for ChangesetTypeMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = String::deserialize(deserializer)?;
        Ok(ChangesetTypeMarker)
    }
}

impl Default for Changeset {
    fn default() -> Self {
        Changeset {
            type_marker: ChangesetTypeMarker,
            id: ChangesetID(0),
            user: String::new(),
            uid: UserID(0),
            created_at: DateTime::default(),
            closed_at: DateTime::default(),
            open: false,
            num_changes: 0,
            min_lat: 0.0,
            max_lat: 0.0,
            min_lon: 0.0,
            max_lon: 0.0,
            comments_count: 0,
            tags: Tags::new(),
            discussion: None,
        }
    }
}

impl Changeset {
    /// Returns the object id of the changeset
    pub fn object_id(&self) -> ObjectID {
        self.id.object_id()
    }

    /// Returns the bounds of the changeset
    pub fn bounds(&self) -> Bounds {
        Bounds {
            min_lat: self.min_lat,
            max_lat: self.max_lat,
            min_lon: self.min_lon,
            max_lon: self.max_lon,
        }
    }

    /// Returns the changeset comment from the tag
    pub fn comment(&self) -> String {
        self.tags.find("comment")
    }

    /// Returns the changeset created by from the tag
    pub fn created_by(&self) -> String {
        self.tags.find("created_by")
    }

    /// Returns the changeset locale from the tag
    pub fn locale(&self) -> String {
        self.tags.find("locale")
    }

    /// Returns the changeset host from the tag
    pub fn host(&self) -> String {
        self.tags.find("host")
    }

    /// Returns imagery used for the changeset from the tag
    pub fn imagery_used(&self) -> String {
        self.tags.find("imagery_used")
    }

    /// Returns source for the changeset from the tag
    pub fn source(&self) -> String {
        self.tags.find("source")
    }

    /// Returns true if the bot tag is a yes
    pub fn bot(&self) -> bool {
        self.tags.find("bot") == "yes"
    }
}

/// Implementation of Object trait for Changeset
impl crate::Object for Changeset {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::Changeset
    }
}

/// ChangesetDiscussion is a conversation about a changeset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesetDiscussion {
    pub comments: Vec<ChangesetComment>,
}

impl ChangesetDiscussion {
    /// Creates a new empty discussion
    pub fn new() -> Self {
        ChangesetDiscussion {
            comments: Vec::new(),
        }
    }
}

impl Default for ChangesetDiscussion {
    fn default() -> Self {
        Self::new()
    }
}

/// ChangesetComment is a specific comment in a changeset discussion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesetComment {
    pub user: String,
    pub uid: UserID,
    #[serde(rename = "date")]
    pub timestamp: DateTime<Utc>,
    pub text: String,
}

impl Default for ChangesetComment {
    fn default() -> Self {
        ChangesetComment {
            user: String::new(),
            uid: UserID(0),
            timestamp: DateTime::default(),
            text: String::new(),
        }
    }
}

/// Helper function to get IDs from changesets
pub fn changeset_ids(changesets: &[Changeset]) -> Vec<ChangesetID> {
    changesets.iter().map(|c| c.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Tag;

    #[test]
    fn test_changeset_object_id() {
        let cs = Changeset {
            id: ChangesetID(123),
            ..Default::default()
        };
        let oid = cs.object_id();
        assert_eq!(oid.type_(), Type::Changeset);
    }

    #[test]
    fn test_changeset_helpers() {
        let cs = Changeset {
            id: ChangesetID(1),
            tags: Tags::from_vec(vec![
                Tag::new("comment", "Test change"),
                Tag::new("bot", "yes"),
            ]),
            ..Default::default()
        };

        assert_eq!(cs.comment(), "Test change");
        assert!(cs.bot());
    }
}
