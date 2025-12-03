//! OSM ID types and type constants

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Type constants for different OSM object types
pub const TYPE_NODE: &str = "node";
pub const TYPE_WAY: &str = "way";
pub const TYPE_RELATION: &str = "relation";
pub const TYPE_CHANGESET: &str = "changeset";
pub const TYPE_NOTE: &str = "note";
pub const TYPE_USER: &str = "user";
pub const TYPE_BOUNDS: &str = "bounds";

/// Bit manipulation constants for ID encoding
const VERSION_BITS: u64 = 16;
const VERSION_MASK: i64 = 0x000000000000FFFF;
const REF_MASK: i64 = 0x00FFFFFFFFFF0000;
const FEATURE_MASK: i64 = 0x7FFFFFFFFFFF0000;
const TYPE_MASK: i64 = 0x7F00000000000000;
const REF_VERSION_MASK: i64 = REF_MASK | VERSION_MASK;
const TYPE_BITS: u64 = 8;
const BOUNDS_MASK: i64 = 0x0800000000000000;
const NODE_MASK: i64 = 0x1000000000000000;
const WAY_MASK: i64 = 0x2000000000000000;
const RELATION_MASK: i64 = 0x3000000000000000;
const CHANGESET_MASK: i64 = 0x4000000000000000;
const NOTE_MASK: i64 = 0x5000000000000000;
const USER_MASK: i64 = 0x6000000000000000;
const TYPE_VERSION_BITS: u64 = TYPE_BITS + VERSION_BITS;

/// Type of OSM element/object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Node,
    Way,
    Relation,
    Changeset,
    Note,
    User,
    Bounds,
}

impl Type {
    /// Returns the string representation of the type
    pub fn as_str(&self) -> &'static str {
        match self {
            Type::Node => TYPE_NODE,
            Type::Way => TYPE_WAY,
            Type::Relation => TYPE_RELATION,
            Type::Changeset => TYPE_CHANGESET,
            Type::Note => TYPE_NOTE,
            Type::User => TYPE_USER,
            Type::Bounds => TYPE_BOUNDS,
        }
    }

    /// Creates a FeatureID from this type and a reference ID
    pub fn feature_id(&self, ref_id: i64) -> Result<FeatureID, String> {
        match self {
            Type::Node => Ok(NodeID(ref_id).feature_id()),
            Type::Way => Ok(WayID(ref_id).feature_id()),
            Type::Relation => Ok(RelationID(ref_id).feature_id()),
            _ => Err(format!("unknown type: {:?}", self)),
        }
    }

    /// Creates an ObjectID from this type, reference ID and version
    pub fn object_id(&self, ref_id: i64, version: i32) -> Result<ObjectID, String> {
        match self {
            Type::Node => Ok(NodeID(ref_id).object_id(version)),
            Type::Way => Ok(WayID(ref_id).object_id(version)),
            Type::Relation => Ok(RelationID(ref_id).object_id(version)),
            Type::Changeset => Ok(ChangesetID(ref_id).object_id()),
            Type::Note => Ok(NoteID(ref_id).object_id()),
            Type::User => Ok(UserID(ref_id).object_id()),
            Type::Bounds => Ok(ObjectID(BOUNDS_MASK)),
            #[allow(unreachable_patterns)]
            _ => Err(format!("unknown type: {:?}", self)),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            TYPE_NODE | "Node" => Ok(Type::Node),
            TYPE_WAY | "Way" => Ok(Type::Way),
            TYPE_RELATION | "Relation" => Ok(Type::Relation),
            TYPE_CHANGESET | "Changeset" => Ok(Type::Changeset),
            TYPE_NOTE | "Note" => Ok(Type::Note),
            TYPE_USER | "User" => Ok(Type::User),
            TYPE_BOUNDS | "Bounds" => Ok(Type::Bounds),
            _ => Err(format!("unknown type: {}", s)),
        }
    }
}

/// NodeID is the primary key of a node
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct NodeID(pub i64);

impl NodeID {
    /// Returns the object id for this node id
    pub fn object_id(self, version: i32) -> ObjectID {
        ObjectID(self.element_id(version).0)
    }

    /// Returns the feature id for this node id
    pub fn feature_id(self) -> FeatureID {
        FeatureID(NODE_MASK | (((self.0 << VERSION_BITS) as i64) & REF_VERSION_MASK))
    }

    /// Returns the element id for this node id with the given version
    pub fn element_id(self, version: i32) -> ElementID {
        self.feature_id().element_id(version)
    }
}

impl fmt::Display for NodeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// WayID is the primary key of a way
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct WayID(pub i64);

impl WayID {
    /// Returns the object id for this way id
    pub fn object_id(self, version: i32) -> ObjectID {
        ObjectID(self.element_id(version).0)
    }

    /// Returns the feature id for this way id
    pub fn feature_id(self) -> FeatureID {
        FeatureID(WAY_MASK | ((self.0 << VERSION_BITS) as i64))
    }

    /// Returns the element id for this way id with the given version
    pub fn element_id(self, version: i32) -> ElementID {
        self.feature_id().element_id(version)
    }
}

impl fmt::Display for WayID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// RelationID is the primary key of a relation
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct RelationID(pub i64);

impl RelationID {
    /// Returns the object id for this relation id
    pub fn object_id(self, version: i32) -> ObjectID {
        ObjectID(self.element_id(version).0)
    }

    /// Returns the feature id for this relation id
    pub fn feature_id(self) -> FeatureID {
        FeatureID(RELATION_MASK | (((self.0 << VERSION_BITS) as i64) & REF_VERSION_MASK))
    }

    /// Returns the element id for this relation id with the given version
    pub fn element_id(self, version: i32) -> ElementID {
        self.feature_id().element_id(version)
    }
}

impl fmt::Display for RelationID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// ChangesetID is the primary key for an osm changeset
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ChangesetID(pub i64);

impl ChangesetID {
    /// Returns the object id for this changeset id
    pub fn object_id(self) -> ObjectID {
        ObjectID(CHANGESET_MASK | (((self.0 << VERSION_BITS) as i64) & REF_VERSION_MASK))
    }
}

impl fmt::Display for ChangesetID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// NoteID is the unique identifier for an osm note
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct NoteID(pub i64);

impl NoteID {
    /// Returns the object id for this note id
    pub fn object_id(self) -> ObjectID {
        ObjectID(NOTE_MASK | (((self.0 << VERSION_BITS) as i64) & REF_VERSION_MASK))
    }
}

impl fmt::Display for NoteID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// UserID is the primary key for a user
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct UserID(pub i64);

impl UserID {
    /// Returns the object id for this user id
    pub fn object_id(self) -> ObjectID {
        ObjectID(USER_MASK | (((self.0 << VERSION_BITS) as i64) & REF_VERSION_MASK))
    }
}

impl fmt::Display for UserID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// ObjectID encodes the type and ref of an osm object
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectID(pub i64);

impl ObjectID {
    /// Returns the Type of the object
    pub fn type_(&self) -> Type {
        match self.0 & TYPE_MASK {
            NODE_MASK => Type::Node,
            WAY_MASK => Type::Way,
            RELATION_MASK => Type::Relation,
            CHANGESET_MASK => Type::Changeset,
            NOTE_MASK => Type::Note,
            USER_MASK => Type::User,
            BOUNDS_MASK => Type::Bounds,
            _ => panic!("unknown type"),
        }
    }

    /// Returns the ID reference for the object
    pub fn ref_(&self) -> i64 {
        // Handle negative ids correctly
        (((self.0 & REF_MASK) >> VERSION_BITS) << TYPE_VERSION_BITS) >> TYPE_VERSION_BITS
    }

    /// Returns the version of the object
    pub fn version(&self) -> i32 {
        (self.0 & VERSION_MASK) as i32
    }
}

impl fmt::Display for ObjectID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.version() == 0 {
            write!(f, "{}/{}:-", self.type_(), self.ref_())
        } else {
            write!(f, "{}/{}:{}", self.type_(), self.ref_(), self.version())
        }
    }
}

impl FromStr for ObjectID {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("invalid element id: {}", s));
        }

        let parts2: Vec<&str> = parts[1].split(':').collect();
        if parts2.is_empty() || parts2.len() > 2 {
            return Err(format!("invalid element id: {}", s));
        }

        let ref_id: i64 = parts2[0]
            .parse()
            .map_err(|e| format!("invalid element id: {}: {}", s, e))?;

        let version = if parts2.len() == 2 && parts2[1] != "-" {
            parts2[1]
                .parse()
                .map_err(|e| format!("invalid element id: {}: {}", s, e))?
        } else {
            0
        };

        let type_: Type = parts[0].parse()?;
        type_.object_id(ref_id, version)
    }
}

/// ElementID is a unique key for an osm element
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementID(pub i64);

impl ElementID {
    /// Returns the Type for the element
    pub fn type_(&self) -> Type {
        match self.0 & TYPE_MASK {
            NODE_MASK => Type::Node,
            WAY_MASK => Type::Way,
            RELATION_MASK => Type::Relation,
            _ => panic!("unknown type"),
        }
    }

    /// Returns the ID reference for the element
    pub fn ref_(&self) -> i64 {
        // Handle negative ids correctly
        (((self.0 & REF_MASK) >> VERSION_BITS) << TYPE_VERSION_BITS) >> TYPE_VERSION_BITS
    }

    /// Returns the version of the element
    pub fn version(&self) -> i32 {
        (self.0 & VERSION_MASK) as i32
    }

    /// Returns the ObjectID for this element
    pub fn object_id(&self) -> ObjectID {
        ObjectID(self.0)
    }

    /// Returns the FeatureID for this element (removing version)
    pub fn feature_id(&self) -> FeatureID {
        FeatureID(self.0 & FEATURE_MASK)
    }

    /// Returns the id as a NodeID. Panics if not a node.
    pub fn node_id(&self) -> NodeID {
        if self.0 & NODE_MASK != NODE_MASK {
            panic!("not a node: {:?}", self);
        }
        NodeID(self.ref_())
    }

    /// Returns the id as a WayID. Panics if not a way.
    pub fn way_id(&self) -> WayID {
        if self.0 & WAY_MASK != WAY_MASK {
            panic!("not a way: {:?}", self);
        }
        WayID(self.ref_())
    }

    /// Returns the id as a RelationID. Panics if not a relation.
    pub fn relation_id(&self) -> RelationID {
        if self.0 & RELATION_MASK != RELATION_MASK {
            panic!("not a relation: {:?}", self);
        }
        RelationID(self.ref_())
    }
}

impl fmt::Display for ElementID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.version() == 0 {
            write!(f, "{}/{}:-", self.type_(), self.ref_())
        } else {
            write!(f, "{}/{}:{}", self.type_(), self.ref_(), self.version())
        }
    }
}

impl FromStr for ElementID {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("invalid element id: {}", s));
        }

        let parts2: Vec<&str> = parts[1].split(':').collect();
        if parts2.len() != 1 && parts2.len() != 2 {
            return Err(format!("invalid element id: {}", s));
        }

        let ref_id: i64 = parts2[0]
            .parse()
            .map_err(|e| format!("invalid element id: {}: {}", s, e))?;

        let version = if parts2.len() == 2 && parts2[1] != "-" {
            parts2[1]
                .parse()
                .map_err(|e| format!("invalid element id: {}: {}", s, e))?
        } else {
            0
        };

        let type_: Type = parts[0].parse()?;
        let fid = type_.feature_id(ref_id)?;
        Ok(fid.element_id(version))
    }
}

/// FeatureID is an identifier for a feature in OSM (all versions of a given element)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureID(pub i64);

impl FeatureID {
    /// Returns the Type of the feature
    pub fn type_(&self) -> Type {
        match self.0 & TYPE_MASK {
            NODE_MASK => Type::Node,
            WAY_MASK => Type::Way,
            RELATION_MASK => Type::Relation,
            _ => Type::Node, // Default, returns empty string in Go
        }
    }

    /// Returns the ID reference for the feature
    pub fn ref_(&self) -> i64 {
        // Handle negative ids correctly
        (((self.0 & REF_MASK) >> VERSION_BITS) << TYPE_VERSION_BITS) >> TYPE_VERSION_BITS
    }

    /// Returns the ObjectID for this feature with the given version
    pub fn object_id(&self, version: i32) -> ObjectID {
        ObjectID(self.element_id(version).0)
    }

    /// Returns the ElementID for this feature with the given version
    pub fn element_id(&self, version: i32) -> ElementID {
        ElementID(self.0 | (VERSION_MASK & (version as i64)))
    }

    /// Returns the id as a NodeID. Panics if not a node.
    pub fn node_id(&self) -> NodeID {
        if self.0 & NODE_MASK != NODE_MASK {
            panic!("not a node: {:?}", self);
        }
        NodeID(self.ref_())
    }

    /// Returns the id as a WayID. Panics if not a way.
    pub fn way_id(&self) -> WayID {
        if self.0 & WAY_MASK != WAY_MASK {
            panic!("not a way: {:?}", self);
        }
        WayID(self.ref_())
    }

    /// Returns the id as a RelationID. Panics if not a relation.
    pub fn relation_id(&self) -> RelationID {
        if self.0 & RELATION_MASK != RELATION_MASK {
            panic!("not a relation: {:?}", self);
        }
        RelationID(self.ref_())
    }
}

impl fmt::Display for FeatureID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self.0 & TYPE_MASK {
            NODE_MASK => TYPE_NODE,
            WAY_MASK => TYPE_WAY,
            RELATION_MASK => TYPE_RELATION,
            _ => "unknown",
        };
        write!(f, "{}/{}", t, self.ref_())
    }
}

impl FromStr for FeatureID {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("invalid feature id: {}", s));
        }

        let ref_id: i64 = parts[1]
            .parse()
            .map_err(|e| format!("invalid feature id: {}: {}", s, e))?;

        let type_: Type = parts[0].parse()?;
        type_.feature_id(ref_id)
    }
}

/// A collection of ElementIDs with helper functions
#[derive(Debug, Clone, Default)]
pub struct ElementIDs(pub Vec<ElementID>);

impl ElementIDs {
    /// Creates a new empty ElementIDs collection
    pub fn new() -> Self {
        ElementIDs(Vec::new())
    }

    /// Creates ElementIDs from a vector
    pub fn from_vec(v: Vec<ElementID>) -> Self {
        ElementIDs(v)
    }

    /// Returns counts of nodes, ways and relations
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut nodes = 0;
        let mut ways = 0;
        let mut relations = 0;
        for id in &self.0 {
            match id.type_() {
                Type::Node => nodes += 1,
                Type::Way => ways += 1,
                Type::Relation => relations += 1,
                _ => {}
            }
        }
        (nodes, ways, relations)
    }

    /// Sorts the IDs by type, then by id
    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| {
            let type_a = a.0 & TYPE_MASK;
            let type_b = b.0 & TYPE_MASK;
            if type_a != type_b {
                return type_a.cmp(&type_b);
            }
            let ref_a = (a.0 << TYPE_VERSION_BITS) >> TYPE_VERSION_BITS;
            let ref_b = (b.0 << TYPE_VERSION_BITS) >> TYPE_VERSION_BITS;
            ref_a.cmp(&ref_b)
        });
    }
}

/// A collection of FeatureIDs with helper functions
#[derive(Debug, Clone, Default)]
pub struct FeatureIDs(pub Vec<FeatureID>);

impl FeatureIDs {
    /// Creates a new empty FeatureIDs collection
    pub fn new() -> Self {
        FeatureIDs(Vec::new())
    }

    /// Creates FeatureIDs from a vector
    pub fn from_vec(v: Vec<FeatureID>) -> Self {
        FeatureIDs(v)
    }

    /// Returns counts of nodes, ways and relations
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut nodes = 0;
        let mut ways = 0;
        let mut relations = 0;
        for id in &self.0 {
            match id.type_() {
                Type::Node => nodes += 1,
                Type::Way => ways += 1,
                Type::Relation => relations += 1,
                _ => {}
            }
        }
        (nodes, ways, relations)
    }

    /// Sorts the IDs by type, then by ref
    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| {
            let type_a = a.0 & TYPE_MASK;
            let type_b = b.0 & TYPE_MASK;
            if type_a != type_b {
                return type_a.cmp(&type_b);
            }
            a.ref_().cmp(&b.ref_())
        });
    }
}

/// A collection of ObjectIDs with helper functions
#[derive(Debug, Clone, Default)]
pub struct ObjectIDs(pub Vec<ObjectID>);

impl ObjectIDs {
    /// Creates a new empty ObjectIDs collection
    pub fn new() -> Self {
        ObjectIDs(Vec::new())
    }

    /// Creates ObjectIDs from a vector
    pub fn from_vec(v: Vec<ObjectID>) -> Self {
        ObjectIDs(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let id = NodeID(123);
        let fid = id.feature_id();
        assert_eq!(fid.type_(), Type::Node);
        assert_eq!(fid.ref_(), 123);
    }

    #[test]
    fn test_way_id() {
        let id = WayID(456);
        let fid = id.feature_id();
        assert_eq!(fid.type_(), Type::Way);
        assert_eq!(fid.ref_(), 456);
    }

    #[test]
    fn test_relation_id() {
        let id = RelationID(789);
        let fid = id.feature_id();
        assert_eq!(fid.type_(), Type::Relation);
        assert_eq!(fid.ref_(), 789);
    }

    #[test]
    fn test_element_id_string() {
        let id = NodeID(123).element_id(5);
        assert_eq!(id.to_string(), "node/123:5");
    }

    #[test]
    fn test_feature_id_string() {
        let id = WayID(456).feature_id();
        assert_eq!(id.to_string(), "way/456");
    }

    #[test]
    fn test_parse_element_id() {
        let id: ElementID = "node/123:5".parse().unwrap();
        assert_eq!(id.type_(), Type::Node);
        assert_eq!(id.ref_(), 123);
        assert_eq!(id.version(), 5);
    }

    #[test]
    fn test_parse_feature_id() {
        let id: FeatureID = "way/456".parse().unwrap();
        assert_eq!(id.type_(), Type::Way);
        assert_eq!(id.ref_(), 456);
    }
}
