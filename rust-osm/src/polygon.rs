//! Polygon detection for OSM ways and relations
//!
//! This module provides functionality to determine if an OSM way or relation
//! should be considered a closed polygon area.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::types::{Relation, Way};

/// Polygon returns true if the way should be considered a closed polygon area.
/// OpenStreetMap doesn't have an intrinsic area data type. The algorithm used
/// here considers a set of heuristics to determine what is most likely an area.
/// The heuristics can be found here,
/// https://wiki.openstreetmap.org/wiki/Overpass_turbo/Polygon_Features
/// and are used by osmtogeojson and overpass turbo.
pub fn way_is_polygon(way: &Way) -> bool {
    if way.nodes.0.len() <= 3 {
        // need more than 3 nodes to be a polygon since first/last is repeated.
        return false;
    }

    if way.nodes.0.first().map(|n| n.id) != way.nodes.0.last().map(|n| n.id) {
        // must be closed
        return false;
    }

    let area = way.tags.find("area");
    if area == "no" {
        return false;
    } else if !area.is_empty() {
        return true;
    }

    for c in POLY_CONDITIONS.iter() {
        let v = way.tags.find(c.key);
        if v.is_empty() || v == "no" {
            continue;
        }

        match c.condition {
            ConditionType::All => return true,
            ConditionType::Whitelist => {
                if c.values.binary_search(&v.as_str()).is_ok() {
                    return true;
                }
            }
            ConditionType::Blacklist => {
                if c.values.binary_search(&v.as_str()).is_err() {
                    return true;
                }
            }
        }
    }

    false
}

/// Polygon returns true if the relation is of type multipolygon or boundary.
pub fn relation_is_polygon(relation: &Relation) -> bool {
    let t = relation.tags.find("type");
    t == "multipolygon" || t == "boundary"
}

// Extend Way with polygon method
impl Way {
    /// Returns true if this way should be considered a closed polygon area
    pub fn is_polygon(&self) -> bool {
        way_is_polygon(self)
    }
}

// Extend Relation with polygon method
impl Relation {
    /// Returns true if this relation is of type multipolygon or boundary
    pub fn is_polygon(&self) -> bool {
        relation_is_polygon(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PolyCondition {
    key: String,
    #[serde(rename = "polygon")]
    condition: ConditionType,
    #[serde(default)]
    values: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ConditionType {
    All,
    Blacklist,
    Whitelist,
}

/// Polygon conditions parsed and cached at initialization
static POLY_CONDITIONS: Lazy<Vec<ParsedPolyCondition>> = Lazy::new(|| {
    let conditions: Vec<PolyCondition> = serde_json::from_str(POLYGON_JSON).unwrap();
    conditions
        .into_iter()
        .map(|c| {
            let mut values: Vec<&'static str> = c
                .values
                .iter()
                .map(|s| -> &'static str { Box::leak(s.clone().into_boxed_str()) })
                .collect();
            values.sort();
            ParsedPolyCondition {
                key: Box::leak(c.key.into_boxed_str()),
                condition: c.condition,
                values,
            }
        })
        .collect()
});

struct ParsedPolyCondition {
    key: &'static str,
    condition: ConditionType,
    values: Vec<&'static str>,
}

/// polygonJSON holds advanced conditions for when an osm way is a polygon.
/// Sourced from: https://wiki.openstreetmap.org/wiki/Overpass_turbo/Polygon_Features
/// Also used by node lib: https://github.com/tyrasd/osmtogeojson
const POLYGON_JSON: &str = r#"
[
    {
        "key": "building",
        "polygon": "all"
    },
    {
        "key": "highway",
        "polygon": "whitelist",
        "values": [
            "services",
            "rest_area",
            "escape",
            "elevator"
        ]
    },
    {
        "key": "natural",
        "polygon": "blacklist",
        "values": [
            "coastline",
            "cliff",
            "ridge",
            "arete",
            "tree_row"
        ]
    },
    {
        "key": "landuse",
        "polygon": "all"
    },
    {
        "key": "waterway",
        "polygon": "whitelist",
        "values": [
            "riverbank",
            "dock",
            "boatyard",
            "dam"
        ]
    },
    {
        "key": "amenity",
        "polygon": "all"
    },
    {
        "key": "leisure",
        "polygon": "all"
    },
    {
        "key": "barrier",
        "polygon": "whitelist",
        "values": [
            "city_wall",
            "ditch",
            "hedge",
            "retaining_wall",
            "wall",
            "spikes"
        ]
    },
    {
        "key": "railway",
        "polygon": "whitelist",
        "values": [
            "station",
            "turntable",
            "roundhouse",
            "platform"
        ]
    },
    {
        "key": "boundary",
        "polygon": "all"
    },
    {
        "key": "man_made",
        "polygon": "blacklist",
        "values": [
            "cutline",
            "embankment",
            "pipeline"
        ]
    },
    {
        "key": "power",
        "polygon": "whitelist",
        "values": [
            "plant",
            "substation",
            "generator",
            "transformer"
        ]
    },
    {
        "key": "place",
        "polygon": "all"
    },
    {
        "key": "shop",
        "polygon": "all"
    },
    {
        "key": "aeroway",
        "polygon": "blacklist",
        "values": [
            "taxiway"
        ]
    },
    {
        "key": "tourism",
        "polygon": "all"
    },
    {
        "key": "historic",
        "polygon": "all"
    },
    {
        "key": "public_transport",
        "polygon": "all"
    },
    {
        "key": "office",
        "polygon": "all"
    },
    {
        "key": "building:part",
        "polygon": "all"
    },
    {
        "key": "military",
        "polygon": "all"
    },
    {
        "key": "ruins",
        "polygon": "all"
    },
    {
        "key": "area:highway",
        "polygon": "all"
    },
    {
        "key": "craft",
        "polygon": "all"
    },
    {
        "key": "golf",
        "polygon": "all"
    },
    {
        "key": "indoor",
        "polygon": "all"
    }
]
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NodeID, Tag, Tags, WayNode, WayNodes};

    fn make_closed_way(tags: Tags) -> Way {
        Way {
            nodes: WayNodes(vec![
                WayNode {
                    id: NodeID(1),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(2),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(3),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(1),
                    ..Default::default()
                },
            ]),
            tags,
            ..Default::default()
        }
    }

    #[test]
    fn test_way_not_closed() {
        let way = Way {
            nodes: WayNodes(vec![
                WayNode {
                    id: NodeID(1),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(2),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(3),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        assert!(!way.is_polygon());
    }

    #[test]
    fn test_way_too_few_nodes() {
        let way = Way {
            nodes: WayNodes(vec![
                WayNode {
                    id: NodeID(1),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(2),
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(1),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        assert!(!way.is_polygon());
    }

    #[test]
    fn test_way_area_no() {
        let way = make_closed_way(Tags::from_vec(vec![Tag::new("area", "no")]));
        assert!(!way.is_polygon());
    }

    #[test]
    fn test_way_area_yes() {
        let way = make_closed_way(Tags::from_vec(vec![Tag::new("area", "yes")]));
        assert!(way.is_polygon());
    }

    #[test]
    fn test_way_building() {
        let way = make_closed_way(Tags::from_vec(vec![Tag::new("building", "yes")]));
        assert!(way.is_polygon());
    }

    #[test]
    fn test_way_highway_services() {
        let way = make_closed_way(Tags::from_vec(vec![Tag::new("highway", "services")]));
        assert!(way.is_polygon());
    }

    #[test]
    fn test_way_highway_primary() {
        let way = make_closed_way(Tags::from_vec(vec![Tag::new("highway", "primary")]));
        assert!(!way.is_polygon());
    }

    #[test]
    fn test_relation_multipolygon() {
        let relation = Relation {
            tags: Tags::from_vec(vec![Tag::new("type", "multipolygon")]),
            ..Default::default()
        };
        assert!(relation.is_polygon());
    }

    #[test]
    fn test_relation_boundary() {
        let relation = Relation {
            tags: Tags::from_vec(vec![Tag::new("type", "boundary")]),
            ..Default::default()
        };
        assert!(relation.is_polygon());
    }

    #[test]
    fn test_relation_not_polygon() {
        let relation = Relation {
            tags: Tags::from_vec(vec![Tag::new("type", "route")]),
            ..Default::default()
        };
        assert!(!relation.is_polygon());
    }
}
