//! Bounds type for OSM data

use geo_types::Rect;
use serde::{Deserialize, Serialize};

use super::{Node, ObjectID, Type};

/// Bounds are the bounds of osm data as defined in the xml file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    #[serde(rename = "minlat")]
    pub min_lat: f64,
    #[serde(rename = "maxlat")]
    pub max_lat: f64,
    #[serde(rename = "minlon")]
    pub min_lon: f64,
    #[serde(rename = "maxlon")]
    pub max_lon: f64,
}

impl Bounds {
    /// Creates a new Bounds
    pub fn new(min_lat: f64, max_lat: f64, min_lon: f64, max_lon: f64) -> Self {
        Bounds {
            min_lat,
            max_lat,
            min_lon,
            max_lon,
        }
    }

    /// Creates bounds from a tile index at a given zoom level
    pub fn from_tile(x: u32, y: u32, z: u32) -> Result<Self, String> {
        let max_index = 1u32 << z;
        if x >= max_index {
            return Err("osm: x index out of range for this zoom".to_string());
        }
        if y >= max_index {
            return Err("osm: y index out of range for this zoom".to_string());
        }

        let n = std::f64::consts::PI * (1.0 - 2.0 * (y as f64) / (max_index as f64));
        let max_lat = n.sinh().atan().to_degrees();

        let n = std::f64::consts::PI * (1.0 - 2.0 * ((y + 1) as f64) / (max_index as f64));
        let min_lat = n.sinh().atan().to_degrees();

        let min_lon = (x as f64) / (max_index as f64) * 360.0 - 180.0;
        let max_lon = ((x + 1) as f64) / (max_index as f64) * 360.0 - 180.0;

        Ok(Bounds {
            min_lat,
            max_lat,
            min_lon,
            max_lon,
        })
    }

    /// ContainsNode returns true if the node is within the bound.
    /// Uses inclusive intervals, ie. returns true if on the boundary.
    pub fn contains_node(&self, node: &Node) -> bool {
        if node.lat < self.min_lat || node.lat > self.max_lat {
            return false;
        }
        if node.lon < self.min_lon || node.lon > self.max_lon {
            return false;
        }
        true
    }

    /// Returns the ObjectID for the bounds (always returns bounds type with 0 id)
    pub fn object_id(&self) -> ObjectID {
        // Bounds mask value
        ObjectID(0x0800000000000000)
    }

    /// Converts bounds to a geo_types Rect
    pub fn to_rect(&self) -> Rect<f64> {
        use geo_types::Coord;
        Rect::new(
            Coord {
                x: self.min_lon,
                y: self.min_lat,
            },
            Coord {
                x: self.max_lon,
                y: self.max_lat,
            },
        )
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            min_lat: f64::MAX,
            max_lat: f64::MIN,
            min_lon: f64::MAX,
            max_lon: f64::MIN,
        }
    }
}

/// Implementation of Object trait for Bounds
impl crate::Object for Bounds {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::Bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_new() {
        let b = Bounds::new(-10.0, 10.0, -20.0, 20.0);
        assert_eq!(b.min_lat, -10.0);
        assert_eq!(b.max_lat, 10.0);
        assert_eq!(b.min_lon, -20.0);
        assert_eq!(b.max_lon, 20.0);
    }

    #[test]
    fn test_bounds_contains() {
        use super::super::NodeID;

        let b = Bounds::new(-10.0, 10.0, -20.0, 20.0);
        let node_inside = Node {
            id: NodeID(1),
            lat: 0.0,
            lon: 0.0,
            ..Default::default()
        };
        let node_outside = Node {
            id: NodeID(2),
            lat: 50.0,
            lon: 0.0,
            ..Default::default()
        };

        assert!(b.contains_node(&node_inside));
        assert!(!b.contains_node(&node_outside));
    }
}
