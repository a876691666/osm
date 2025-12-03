//! Node type for OSM data

use chrono::{DateTime, Utc};
use geo_types::Point;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{ChangesetID, ElementID, FeatureID, NodeID, ObjectID, Tags, Type, UserID};

/// Node is an osm point and allows for marshalling to/from osm xml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    #[serde(rename = "type", default)]
    pub type_marker: NodeTypeMarker,
    pub id: NodeID,
    pub lat: f64,
    pub lon: f64,
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
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// Committed is the estimated time this object was committed
    /// and made visible in the central OSM database
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<DateTime<Utc>>,
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

/// Marker type to serialize "node" as the type field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodeTypeMarker;

impl Serialize for NodeTypeMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("node")
    }
}

impl<'de> Deserialize<'de> for NodeTypeMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = String::deserialize(deserializer)?;
        Ok(NodeTypeMarker)
    }
}

impl Default for Node {
    fn default() -> Self {
        Node {
            type_marker: NodeTypeMarker,
            id: NodeID(0),
            lat: 0.0,
            lon: 0.0,
            user: String::new(),
            uid: UserID(0),
            visible: true,
            version: 0,
            changeset: ChangesetID(0),
            timestamp: DateTime::default(),
            tags: Tags::new(),
            committed: None,
        }
    }
}

impl Node {
    /// Returns the object id of the node
    pub fn object_id(&self) -> ObjectID {
        self.id.object_id(self.version)
    }

    /// Returns the feature id of the node
    pub fn feature_id(&self) -> FeatureID {
        self.id.feature_id()
    }

    /// Returns the element id of the node
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

    /// Returns the node location as a geo_types Point
    pub fn point(&self) -> Point<f64> {
        Point::new(self.lon, self.lat)
    }
}

/// Implementation of Element trait for Node
impl crate::Element for Node {
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

/// Implementation of Object trait for Node
impl crate::Object for Node {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::Node
    }
}

/// Nodes is a list of nodes with helper functions
#[derive(Debug, Clone, Default)]
pub struct Nodes(pub Vec<Node>);

impl Nodes {
    /// Creates an empty Nodes collection
    pub fn new() -> Self {
        Nodes(Vec::new())
    }

    /// Returns the ids for all the nodes
    pub fn ids(&self) -> Vec<NodeID> {
        self.0.iter().map(|n| n.id).collect()
    }

    /// Returns the feature ids for all the nodes
    pub fn feature_ids(&self) -> Vec<FeatureID> {
        self.0.iter().map(|n| n.feature_id()).collect()
    }

    /// Returns the element ids for all the nodes
    pub fn element_ids(&self) -> Vec<ElementID> {
        self.0.iter().map(|n| n.element_id()).collect()
    }

    /// Sorts the nodes first by id and then version in ascending order
    pub fn sort_by_id_version(&mut self) {
        self.0.sort_by(|a, b| {
            if a.id == b.id {
                a.version.cmp(&b.version)
            } else {
                a.id.cmp(&b.id)
            }
        });
    }

    /// Returns the number of nodes
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds a node
    pub fn push(&mut self, node: Node) {
        self.0.push(node);
    }
}

impl From<Vec<Node>> for Nodes {
    fn from(v: Vec<Node>) -> Self {
        Nodes(v)
    }
}

impl IntoIterator for Nodes {
    type Item = Node;
    type IntoIter = std::vec::IntoIter<Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Nodes {
    type Item = &'a Node;
    type IntoIter = std::slice::Iter<'a, Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_ids() {
        let node = Node {
            id: NodeID(123),
            version: 5,
            ..Default::default()
        };

        assert_eq!(node.feature_id().ref_(), 123);
        assert_eq!(node.element_id().version(), 5);
    }

    #[test]
    fn test_node_point() {
        let node = Node {
            id: NodeID(1),
            lat: 51.5,
            lon: -0.1,
            ..Default::default()
        };

        let point = node.point();
        assert_eq!(point.x(), -0.1);
        assert_eq!(point.y(), 51.5);
    }

    #[test]
    fn test_nodes_sort() {
        let mut nodes = Nodes(vec![
            Node {
                id: NodeID(2),
                version: 1,
                ..Default::default()
            },
            Node {
                id: NodeID(1),
                version: 2,
                ..Default::default()
            },
            Node {
                id: NodeID(1),
                version: 1,
                ..Default::default()
            },
        ]);

        nodes.sort_by_id_version();
        assert_eq!(nodes.0[0].id, NodeID(1));
        assert_eq!(nodes.0[0].version, 1);
        assert_eq!(nodes.0[1].id, NodeID(1));
        assert_eq!(nodes.0[1].version, 2);
        assert_eq!(nodes.0[2].id, NodeID(2));
    }
}
