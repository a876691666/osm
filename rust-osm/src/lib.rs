//! OSM - A general purpose library for reading, writing and working with OpenStreetMap data
//!
//! This library provides Rust implementations of the core OSM data types and operations:
//!
//! - **Core Types**: Node, Way, Relation, Changeset, Note, User
//! - **Container Types**: OSM (container returned via API), Change (replication API), Diff (augmented diffs)
//! - **IDs**: NodeID, WayID, RelationID, FeatureID, ElementID, ObjectID
//! - **XML Parsing**: Scanner for reading OSM XML files
//! - **GeoJSON Conversion**: Convert OSM data to GeoJSON
//! - **API Client**: Access OSM REST API (nodes, ways, relations, changesets, notes, users)
//! - **Replication**: Access planet replication feeds (minute, hour, day, changeset)
//! - **Annotate**: Compute diffs and annotate elements with history data
//!
//! # Example
//!
//! ```rust
//! use osm::{Node, NodeID, Tags, Tag};
//!
//! let node = Node {
//!     id: NodeID(123),
//!     lat: 51.5074,
//!     lon: -0.1278,
//!     tags: Tags::from_vec(vec![
//!         Tag::new("name", "London"),
//!         Tag::new("place", "city"),
//!     ]),
//!     ..Default::default()
//! };
//!
//! assert_eq!(node.tags.find("name"), "London");
//! ```

pub mod annotate;
pub mod api;
pub mod container;
pub mod datasource;
pub mod geojson;
pub mod mputil;
pub mod polygon;
pub mod replication;
pub mod types;
pub mod xml;

// Re-export all types at the crate root for convenience
pub use container::*;
pub use datasource::*;
pub use polygon::*;
pub use types::*;

// Constants for OSM data
/// Copyright string for OSM data
pub const COPYRIGHT: &str = "OpenStreetMap and contributors";
/// Attribution URL for OSM data
pub const ATTRIBUTION: &str = "http://www.openstreetmap.org/copyright";
/// License URL for OSM data
pub const LICENSE: &str = "http://opendatacommons.org/licenses/odbl/1-0/";

/// Error type for scanner closed
#[derive(Debug, Clone, thiserror::Error)]
#[error("osm: scanner closed by user")]
pub struct ScannerClosedError;

use std::collections::HashMap;

/// An Element represents a Node, Way or Relation
pub trait Element: Object {
    fn element_id(&self) -> ElementID;
    fn feature_id(&self) -> FeatureID;
    fn tag_map(&self) -> HashMap<String, String>;
}

/// An Object represents a Node, Way, Relation, Changeset, Note or User
pub trait Object {
    fn object_id(&self) -> ObjectID;
    fn object_type(&self) -> Type;
}

/// A Scanner reads osm data from planet dump files
pub trait Scanner {
    type Item;
    type Error;

    /// Advances the scanner to the next item
    fn scan(&mut self) -> bool;

    /// Returns the current object
    fn object(&self) -> Option<&Self::Item>;

    /// Returns the last error encountered
    fn err(&self) -> Option<&Self::Error>;

    /// Closes the scanner
    fn close(&mut self) -> Result<(), Self::Error>;
}

/// Objects is a set of objects with some helpers
#[derive(Debug, Clone, Default)]
pub struct Objects(pub Vec<Box<dyn ObjectDyn>>);

/// Dynamic object trait for heterogeneous collections
pub trait ObjectDyn: std::fmt::Debug {
    fn object_id(&self) -> ObjectID;
    fn object_type(&self) -> Type;
    fn as_any(&self) -> &dyn std::any::Any;
}

impl ObjectDyn for Node {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectDyn for Way {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectDyn for Relation {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectDyn for Changeset {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectDyn for Note {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectDyn for User {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectDyn for Bounds {
    fn object_id(&self) -> ObjectID {
        Object::object_id(self)
    }

    fn object_type(&self) -> Type {
        Object::object_type(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Clone for Box<dyn ObjectDyn> {
    fn clone(&self) -> Box<dyn ObjectDyn> {
        // This is a simplified clone - in practice you'd need proper downcasting
        panic!("Clone not fully implemented for Box<dyn ObjectDyn>");
    }
}

/// Elements is a collection of Element types
#[derive(Debug, Clone, Default)]
pub struct Elements(pub Vec<Box<dyn ElementDyn>>);

/// Dynamic element trait for heterogeneous collections
pub trait ElementDyn: ObjectDyn {
    fn element_id_dyn(&self) -> ElementID;
    fn feature_id_dyn(&self) -> FeatureID;
    fn tag_map_dyn(&self) -> HashMap<String, String>;
}

impl ElementDyn for Node {
    fn element_id_dyn(&self) -> ElementID {
        Element::element_id(self)
    }

    fn feature_id_dyn(&self) -> FeatureID {
        Element::feature_id(self)
    }

    fn tag_map_dyn(&self) -> HashMap<String, String> {
        Element::tag_map(self)
    }
}

impl ElementDyn for Way {
    fn element_id_dyn(&self) -> ElementID {
        Element::element_id(self)
    }

    fn feature_id_dyn(&self) -> FeatureID {
        Element::feature_id(self)
    }

    fn tag_map_dyn(&self) -> HashMap<String, String> {
        Element::tag_map(self)
    }
}

impl ElementDyn for Relation {
    fn element_id_dyn(&self) -> ElementID {
        Element::element_id(self)
    }

    fn feature_id_dyn(&self) -> FeatureID {
        Element::feature_id(self)
    }

    fn tag_map_dyn(&self) -> HashMap<String, String> {
        Element::tag_map(self)
    }
}

impl Clone for Box<dyn ElementDyn> {
    fn clone(&self) -> Box<dyn ElementDyn> {
        panic!("Clone not fully implemented for Box<dyn ElementDyn>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(!COPYRIGHT.is_empty());
        assert!(!ATTRIBUTION.is_empty());
        assert!(!LICENSE.is_empty());
    }
}
