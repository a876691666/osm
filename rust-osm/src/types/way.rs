//! Way type for OSM data

use chrono::{DateTime, Utc};
use geo_types::{Coord, LineString, Point};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{
    Bounds, ChangesetID, ElementID, FeatureID, NodeID, ObjectID, Tags, Type, Update, Updates,
    UserID, WayID,
};

/// Way is an osm way, ie collection of nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Way {
    #[serde(rename = "type", default)]
    pub type_marker: WayTypeMarker,
    pub id: WayID,
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
    #[serde(default)]
    pub nodes: WayNodes,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// Committed is the estimated time this object was committed
    /// and made visible in the central OSM database
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<DateTime<Utc>>,
    /// Updates are changes to the nodes of this way independent
    /// of an update to the way itself
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

/// Marker type to serialize "way" as the type field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WayTypeMarker;

impl Serialize for WayTypeMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("way")
    }
}

impl<'de> Deserialize<'de> for WayTypeMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = String::deserialize(deserializer)?;
        Ok(WayTypeMarker)
    }
}

impl Default for Way {
    fn default() -> Self {
        Way {
            type_marker: WayTypeMarker,
            id: WayID(0),
            user: String::new(),
            uid: UserID(0),
            visible: true,
            version: 0,
            changeset: ChangesetID(0),
            timestamp: DateTime::default(),
            nodes: WayNodes::new(),
            tags: Tags::new(),
            committed: None,
            updates: Updates::new(),
            bounds: None,
        }
    }
}

impl Way {
    /// Returns the object id of the way
    pub fn object_id(&self) -> ObjectID {
        self.id.object_id(self.version)
    }

    /// Returns the feature id of the way
    pub fn feature_id(&self) -> FeatureID {
        self.id.feature_id()
    }

    /// Returns the element id of the way
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

    /// Applies a single update to the way
    fn apply_update(&mut self, u: &Update) -> Result<(), super::UpdateError> {
        if u.index >= self.nodes.0.len() {
            return Err(super::UpdateError::IndexOutOfRange { index: u.index });
        }

        self.nodes.0[u.index].version = u.version;
        self.nodes.0[u.index].changeset = u.changeset;
        self.nodes.0[u.index].lat = u.lat;
        self.nodes.0[u.index].lon = u.lon;

        Ok(())
    }

    /// Converts the annotated nodes into a LineString
    pub fn line_string(&self) -> LineString<f64> {
        let coords: Vec<Coord<f64>> = self
            .nodes
            .0
            .iter()
            .filter(|n| n.version != 0 || n.lon != 0.0 || n.lat != 0.0)
            .map(|n| Coord {
                x: n.lon,
                y: n.lat,
            })
            .collect();
        LineString::new(coords)
    }

    /// Returns the LineString from annotated points at the given time
    pub fn line_string_at(&self, t: DateTime<Utc>) -> LineString<f64> {
        let mut ls: Vec<Coord<f64>> = self
            .nodes
            .0
            .iter()
            .map(|n| Coord {
                x: n.lon,
                y: n.lat,
            })
            .collect();

        for u in &self.updates.0 {
            if u.timestamp > t {
                break;
            }
            if u.index < ls.len() {
                ls[u.index].x = u.lon;
                ls[u.index].y = u.lat;
            }
        }

        // Remove all zeros
        let coords: Vec<Coord<f64>> = ls
            .into_iter()
            .enumerate()
            .filter(|(i, _)| {
                let n = &self.nodes.0[*i];
                n.version != 0 || n.lon != 0.0 || n.lat != 0.0
            })
            .map(|(_, c)| c)
            .collect();

        LineString::new(coords)
    }
}

/// Implementation of Element trait for Way
impl crate::Element for Way {
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

/// Implementation of Object trait for Way
impl crate::Object for Way {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::Way
    }
}

/// WayNode is a short node used as part of ways
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WayNode {
    #[serde(rename = "ref", default)]
    pub id: NodeID,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub version: i32,
    #[serde(default, skip_serializing_if = "is_zero_changeset_id")]
    pub changeset: ChangesetID,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lat: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lon: f64,
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

impl Default for WayNode {
    fn default() -> Self {
        WayNode {
            id: NodeID(0),
            version: 0,
            changeset: ChangesetID(0),
            lat: 0.0,
            lon: 0.0,
        }
    }
}

impl WayNode {
    /// Returns the feature id of the way node
    pub fn feature_id(&self) -> FeatureID {
        self.id.feature_id()
    }

    /// Returns the element id of the way node
    pub fn element_id(&self) -> ElementID {
        self.id.element_id(self.version)
    }

    /// Returns the node location as a geo_types Point
    pub fn point(&self) -> Point<f64> {
        Point::new(self.lon, self.lat)
    }
}

/// WayNodes represents a collection of way nodes
#[derive(Debug, Clone, Default)]
pub struct WayNodes(pub Vec<WayNode>);

impl WayNodes {
    /// Creates an empty WayNodes collection
    pub fn new() -> Self {
        WayNodes(Vec::new())
    }

    /// Computes the bounds for the given way nodes
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::default();

        for n in &self.0 {
            b.min_lat = b.min_lat.min(n.lat);
            b.max_lat = b.max_lat.max(n.lat);
            b.min_lon = b.min_lon.min(n.lon);
            b.max_lon = b.max_lon.max(n.lon);
        }

        b
    }

    /// Returns element ids for the way nodes
    pub fn element_ids(&self) -> Vec<ElementID> {
        self.0.iter().map(|n| n.element_id()).collect()
    }

    /// Returns feature ids for the way nodes
    pub fn feature_ids(&self) -> Vec<FeatureID> {
        self.0.iter().map(|n| n.feature_id()).collect()
    }

    /// Returns node ids for the way nodes
    pub fn node_ids(&self) -> Vec<NodeID> {
        self.0.iter().map(|n| n.id).collect()
    }

    /// Returns the number of way nodes
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for WayNodes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as an array of node ids (like overpass osmjson)
        let ids: Vec<i64> = self.0.iter().map(|n| n.id.0).collect();
        ids.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WayNodes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ids: Vec<i64> = Vec::deserialize(deserializer)?;
        let nodes: Vec<WayNode> = ids
            .into_iter()
            .map(|id| WayNode {
                id: NodeID(id),
                ..Default::default()
            })
            .collect();
        Ok(WayNodes(nodes))
    }
}

impl From<Vec<WayNode>> for WayNodes {
    fn from(v: Vec<WayNode>) -> Self {
        WayNodes(v)
    }
}

/// Ways is a list of ways with helper functions
#[derive(Debug, Clone, Default)]
pub struct Ways(pub Vec<Way>);

impl Ways {
    /// Creates an empty Ways collection
    pub fn new() -> Self {
        Ways(Vec::new())
    }

    /// Returns the ids for all the ways
    pub fn ids(&self) -> Vec<WayID> {
        self.0.iter().map(|w| w.id).collect()
    }

    /// Returns the feature ids for all the ways
    pub fn feature_ids(&self) -> Vec<FeatureID> {
        self.0.iter().map(|w| w.feature_id()).collect()
    }

    /// Returns the element ids for all the ways
    pub fn element_ids(&self) -> Vec<ElementID> {
        self.0.iter().map(|w| w.element_id()).collect()
    }

    /// Sorts the ways first by id and then version in ascending order
    pub fn sort_by_id_version(&mut self) {
        self.0.sort_by(|a, b| {
            if a.id == b.id {
                a.version.cmp(&b.version)
            } else {
                a.id.cmp(&b.id)
            }
        });
    }

    /// Returns the number of ways
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds a way
    pub fn push(&mut self, way: Way) {
        self.0.push(way);
    }
}

impl From<Vec<Way>> for Ways {
    fn from(v: Vec<Way>) -> Self {
        Ways(v)
    }
}

impl IntoIterator for Ways {
    type Item = Way;
    type IntoIter = std::vec::IntoIter<Way>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Ways {
    type Item = &'a Way;
    type IntoIter = std::slice::Iter<'a, Way>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_way_ids() {
        let way = Way {
            id: WayID(123),
            version: 5,
            ..Default::default()
        };

        assert_eq!(way.feature_id().ref_(), 123);
        assert_eq!(way.element_id().version(), 5);
    }

    #[test]
    fn test_way_nodes_bounds() {
        let nodes = WayNodes(vec![
            WayNode {
                id: NodeID(1),
                lat: 10.0,
                lon: 20.0,
                ..Default::default()
            },
            WayNode {
                id: NodeID(2),
                lat: -10.0,
                lon: -20.0,
                ..Default::default()
            },
        ]);

        let b = nodes.bounds();
        assert_eq!(b.min_lat, -10.0);
        assert_eq!(b.max_lat, 10.0);
        assert_eq!(b.min_lon, -20.0);
        assert_eq!(b.max_lon, 20.0);
    }

    #[test]
    fn test_ways_sort() {
        let mut ways = Ways(vec![
            Way {
                id: WayID(2),
                version: 1,
                ..Default::default()
            },
            Way {
                id: WayID(1),
                version: 2,
                ..Default::default()
            },
            Way {
                id: WayID(1),
                version: 1,
                ..Default::default()
            },
        ]);

        ways.sort_by_id_version();
        assert_eq!(ways.0[0].id, WayID(1));
        assert_eq!(ways.0[0].version, 1);
        assert_eq!(ways.0[1].id, WayID(1));
        assert_eq!(ways.0[1].version, 2);
        assert_eq!(ways.0[2].id, WayID(2));
    }
}
