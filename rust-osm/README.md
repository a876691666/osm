# osm-rs

A general purpose library for reading, writing and working with OpenStreetMap data in Rust.

This is a Rust port of the Go library [paulmach/osm](https://github.com/paulmach/osm).

## Features

- Core OSM types: Node, Way, Relation, Changeset, Note, User
- Container types: OSM, Change, Diff
- ID types: NodeID, WayID, RelationID, FeatureID, ElementID, ObjectID
- Polygon detection for ways and relations
- History datasource for tracking element versions
- JSON serialization/deserialization (Overpass API compatible)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
osm = "0.1"
```

## Example

```rust
use osm::{Node, NodeID, Tags, Tag};

let node = Node {
    id: NodeID(123),
    lat: 51.5074,
    lon: -0.1278,
    tags: Tags::from_vec(vec![
        Tag::new("name", "London"),
        Tag::new("place", "city"),
    ]),
    ..Default::default()
};

assert_eq!(node.tags.find("name"), "London");
```

## Working with Ways

```rust
use osm::{Way, WayID, WayNode, WayNodes, NodeID, Tags, Tag};

let way = Way {
    id: WayID(456),
    nodes: WayNodes(vec![
        WayNode { id: NodeID(1), ..Default::default() },
        WayNode { id: NodeID(2), ..Default::default() },
        WayNode { id: NodeID(3), ..Default::default() },
        WayNode { id: NodeID(1), ..Default::default() }, // closed
    ]),
    tags: Tags::from_vec(vec![
        Tag::new("building", "yes"),
    ]),
    ..Default::default()
};

// Check if it's a polygon
assert!(way.is_polygon());
```

## Working with the OSM Container

```rust
use osm::{OSM, Node, NodeID, OsmObject};

let mut osm = OSM::new();
osm.append(OsmObject::Node(Node {
    id: NodeID(1),
    lat: 51.5,
    lon: -0.1,
    ..Default::default()
}));

// Get all objects
let objects = osm.objects();
```

## Types

### Core Types

| Type | Description |
|------|-------------|
| `Node` | An OSM point with coordinates |
| `Way` | An ordered list of nodes |
| `Relation` | A group of elements with roles |
| `Changeset` | Metadata about a set of changes |
| `Note` | Map notes/comments |
| `User` | OSM user information |

### ID Types

| Type | Description |
|------|-------------|
| `NodeID` | Primary key of a node |
| `WayID` | Primary key of a way |
| `RelationID` | Primary key of a relation |
| `FeatureID` | Identifies all versions of an element |
| `ElementID` | Identifies a specific version of an element |
| `ObjectID` | Identifies any OSM object |

### Container Types

| Type | Description |
|------|-------------|
| `OSM` | Container for OSM data (nodes, ways, relations, etc.) |
| `Change` | Structure for changeset data (create, modify, delete) |
| `Diff` | Augmented diff with old/new data |

## License

MIT License
