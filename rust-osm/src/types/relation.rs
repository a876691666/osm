//! Relation type for OSM data

use chrono::{DateTime, Utc};
use geo_types::Point;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{
    Bounds, ChangesetID, ElementID, FeatureID, NodeID, ObjectID, RelationID, Tags, Type, Update,
    Updates, UserID, WayID, WayNodes,
};

/// Relation is a collection of nodes, ways and other relations with defining attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    #[serde(rename = "type", default)]
    pub type_marker: RelationTypeMarker,
    pub id: RelationID,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user: String,
    #[serde(default, skip_serializing_if = "is_zero_user_id")]
    pub uid: UserID,
    #[serde(default)]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub version: i32,
    #[serde(default, skip_serializing_if = "is_zero_changeset_id")]
    pub changeset: ChangesetID,
    #[serde(default, skip_serializing_if = "is_default_timestamp")]
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    #[serde(default)]
    pub members: Members,
    /// Committed is the estimated time this object was committed
    /// and made visible in the central OSM database
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<DateTime<Utc>>,
    /// Updates are changes to the members of this relation independent
    /// of an update to the relation itself
    #[serde(default, skip_serializing_if = "Updates::is_empty")]
    pub updates: Updates,
    /// Bounds are included by overpass, and maybe others
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Bounds>,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn is_zero_user_id(v: &UserID) -> bool {
    v.0 == 0
}

fn is_zero_changeset_id(v: &ChangesetID) -> bool {
    v.0 == 0
}

fn is_default_timestamp(v: &DateTime<Utc>) -> bool {
    *v == DateTime::<Utc>::default()
}

/// Marker type to serialize "relation" as the type field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelationTypeMarker;

impl Serialize for RelationTypeMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("relation")
    }
}

impl<'de> Deserialize<'de> for RelationTypeMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = String::deserialize(deserializer)?;
        Ok(RelationTypeMarker)
    }
}

impl Default for Relation {
    fn default() -> Self {
        Relation {
            type_marker: RelationTypeMarker,
            id: RelationID(0),
            user: String::new(),
            uid: UserID(0),
            visible: true,
            version: 0,
            changeset: ChangesetID(0),
            timestamp: DateTime::default(),
            tags: Tags::new(),
            members: Members::new(),
            committed: None,
            updates: Updates::new(),
            bounds: None,
        }
    }
}

impl Relation {
    /// Returns the object id of the relation
    pub fn object_id(&self) -> ObjectID {
        self.id.object_id(self.version)
    }

    /// Returns the feature id of the relation
    pub fn feature_id(&self) -> FeatureID {
        self.id.feature_id()
    }

    /// Returns the element id of the relation
    pub fn element_id(&self) -> ElementID {
        self.id.element_id(self.version)
    }

    /// Returns the best estimate on when this element was written/committed into the database
    pub fn committed_at(&self) -> DateTime<Utc> {
        self.committed.unwrap_or(self.timestamp)
    }

    /// Returns the element tags as a key/value map
    pub fn tag_map(&self) -> HashMap<String, String> {
        self.tags.map()
    }

    /// Applies updates up to and including the given time
    pub fn apply_updates_up_to(&mut self, t: DateTime<Utc>) -> Result<(), super::UpdateError> {
        // Clone updates to avoid borrow issues
        let updates: Vec<Update> = self.updates.0.clone();
        let mut not_applied = Vec::new();

        for u in updates {
            if u.timestamp > t {
                not_applied.push(u);
                continue;
            }

            self.apply_update(&u)?;
        }

        self.updates = Updates(not_applied);
        Ok(())
    }

    /// Applies a single update to the relation
    fn apply_update(&mut self, u: &Update) -> Result<(), super::UpdateError> {
        if u.index >= self.members.0.len() {
            return Err(super::UpdateError::IndexOutOfRange { index: u.index });
        }

        self.members.0[u.index].version = u.version;
        self.members.0[u.index].changeset = u.changeset;
        self.members.0[u.index].lat = u.lat;
        self.members.0[u.index].lon = u.lon;

        if u.reverse {
            self.members.0[u.index].orientation *= -1;
        }

        Ok(())
    }
}

/// Implementation of Element trait for Relation
impl crate::Element for Relation {
    fn element_id(&self) -> ElementID {
        self.element_id()
    }

    fn feature_id(&self) -> FeatureID {
        self.feature_id()
    }

    fn tag_map(&self) -> HashMap<String, String> {
        self.tag_map()
    }
}

/// Implementation of Object trait for Relation
impl crate::Object for Relation {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::Relation
    }
}

/// Member is a member of a relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    #[serde(rename = "type")]
    pub member_type: Type,
    #[serde(rename = "ref")]
    pub ref_: i64,
    #[serde(default)]
    pub role: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub version: i32,
    #[serde(default, skip_serializing_if = "is_zero_changeset_id")]
    pub changeset: ChangesetID,
    /// Node location if Type == Node
    /// Closest vertex to centroid if Type == Way
    /// Empty/invalid if Type == Relation
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lat: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lon: f64,
    /// Orientation is the direction of the way around a ring of a multipolygon
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub orientation: i8,
    /// Nodes are sometimes included in members of type way to include the lat/lon path
    #[serde(default, skip_serializing_if = "WayNodes::is_empty")]
    pub nodes: WayNodes,
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

impl Default for Member {
    fn default() -> Self {
        Member {
            member_type: Type::Node,
            ref_: 0,
            role: String::new(),
            version: 0,
            changeset: ChangesetID(0),
            lat: 0.0,
            lon: 0.0,
            orientation: 0,
            nodes: WayNodes::new(),
        }
    }
}

impl Member {
    /// Returns the feature id of the member
    pub fn feature_id(&self) -> FeatureID {
        match self.member_type {
            Type::Node => NodeID(self.ref_).feature_id(),
            Type::Way => WayID(self.ref_).feature_id(),
            Type::Relation => RelationID(self.ref_).feature_id(),
            _ => panic!("unknown type"),
        }
    }

    /// Returns the element id of the member
    pub fn element_id(&self) -> ElementID {
        self.feature_id().element_id(self.version)
    }

    /// Returns the member location as a geo_types Point
    pub fn point(&self) -> Point<f64> {
        Point::new(self.lon, self.lat)
    }
}

/// Members represents an ordered list of relation members
#[derive(Debug, Clone, Default)]
pub struct Members(pub Vec<Member>);

impl Members {
    /// Creates an empty Members collection
    pub fn new() -> Self {
        Members(Vec::new())
    }

    /// Returns feature ids for all members
    pub fn feature_ids(&self) -> Vec<FeatureID> {
        self.0.iter().map(|m| m.feature_id()).collect()
    }

    /// Returns element ids for all members
    pub fn element_ids(&self) -> Vec<ElementID> {
        self.0.iter().map(|m| m.element_id()).collect()
    }

    /// Returns the number of members
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Members {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.is_empty() {
            // Serialize empty as []
            Vec::<Member>::new().serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let members = Vec::deserialize(deserializer)?;
        Ok(Members(members))
    }
}

impl From<Vec<Member>> for Members {
    fn from(v: Vec<Member>) -> Self {
        Members(v)
    }
}

/// Relations is a list of relations with helper functions
#[derive(Debug, Clone, Default)]
pub struct Relations(pub Vec<Relation>);

impl Relations {
    /// Creates an empty Relations collection
    pub fn new() -> Self {
        Relations(Vec::new())
    }

    /// Returns the ids for all the relations
    pub fn ids(&self) -> Vec<RelationID> {
        self.0.iter().map(|r| r.id).collect()
    }

    /// Returns the feature ids for all the relations
    pub fn feature_ids(&self) -> Vec<FeatureID> {
        self.0.iter().map(|r| r.feature_id()).collect()
    }

    /// Returns the element ids for all the relations
    pub fn element_ids(&self) -> Vec<ElementID> {
        self.0.iter().map(|r| r.element_id()).collect()
    }

    /// Sorts the relations first by id and then version in ascending order
    pub fn sort_by_id_version(&mut self) {
        self.0.sort_by(|a, b| {
            if a.id == b.id {
                a.version.cmp(&b.version)
            } else {
                a.id.cmp(&b.id)
            }
        });
    }

    /// Returns the number of relations
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds a relation
    pub fn push(&mut self, relation: Relation) {
        self.0.push(relation);
    }
}

impl From<Vec<Relation>> for Relations {
    fn from(v: Vec<Relation>) -> Self {
        Relations(v)
    }
}

impl IntoIterator for Relations {
    type Item = Relation;
    type IntoIter = std::vec::IntoIter<Relation>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Relations {
    type Item = &'a Relation;
    type IntoIter = std::slice::Iter<'a, Relation>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_ids() {
        let relation = Relation {
            id: RelationID(123),
            version: 5,
            ..Default::default()
        };

        assert_eq!(relation.feature_id().ref_(), 123);
        assert_eq!(relation.element_id().version(), 5);
    }

    #[test]
    fn test_member_feature_id() {
        let member = Member {
            member_type: Type::Way,
            ref_: 456,
            ..Default::default()
        };

        assert_eq!(member.feature_id().type_(), Type::Way);
        assert_eq!(member.feature_id().ref_(), 456);
    }

    #[test]
    fn test_relations_sort() {
        let mut relations = Relations(vec![
            Relation {
                id: RelationID(2),
                version: 1,
                ..Default::default()
            },
            Relation {
                id: RelationID(1),
                version: 2,
                ..Default::default()
            },
            Relation {
                id: RelationID(1),
                version: 1,
                ..Default::default()
            },
        ]);

        relations.sort_by_id_version();
        assert_eq!(relations.0[0].id, RelationID(1));
        assert_eq!(relations.0[0].version, 1);
        assert_eq!(relations.0[1].id, RelationID(1));
        assert_eq!(relations.0[1].version, 2);
        assert_eq!(relations.0[2].id, RelationID(2));
    }
}
