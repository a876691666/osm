//! Change type for OSM replication data

use serde::{Deserialize, Serialize};

use super::{OSM, OsmObject};

/// Change is the structure of a changeset to be
/// uploaded or downloaded from the osm api server.
/// See: http://wiki.openstreetmap.org/wiki/OsmChange
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Change {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generator: String,

    /// To indicate the origin of the data
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub copyright: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribution: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<OSM>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modify: Option<OSM>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<OSM>,
}

impl Change {
    /// Creates a new empty Change
    pub fn new() -> Self {
        Change::default()
    }

    /// Appends an object to the Create OSM object
    pub fn append_create(&mut self, obj: OsmObject) {
        if self.create.is_none() {
            self.create = Some(OSM::new());
        }
        if let Some(osm) = &mut self.create {
            osm.append(obj);
        }
    }

    /// Appends an object to the Modify OSM object
    pub fn append_modify(&mut self, obj: OsmObject) {
        if self.modify.is_none() {
            self.modify = Some(OSM::new());
        }
        if let Some(osm) = &mut self.modify {
            osm.append(obj);
        }
    }

    /// Appends an object to the Delete OSM object
    pub fn append_delete(&mut self, obj: OsmObject) {
        if self.delete.is_none() {
            self.delete = Some(OSM::new());
        }
        if let Some(osm) = &mut self.delete {
            osm.append(obj);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Node, NodeID};

    #[test]
    fn test_change_new() {
        let change = Change::new();
        assert!(change.create.is_none());
        assert!(change.modify.is_none());
        assert!(change.delete.is_none());
    }

    #[test]
    fn test_change_append_create() {
        let mut change = Change::new();
        change.append_create(OsmObject::Node(Node {
            id: NodeID(1),
            ..Default::default()
        }));
        assert!(change.create.is_some());
        assert_eq!(change.create.unwrap().nodes.len(), 1);
    }
}
