//! Container types for OSM data
//!
//! This module contains the container types used to hold OSM data:
//! - OSM: The main container returned by API/parsed from files
//! - Change: Used by the replication API
//! - Diff: Corresponds to Overpass Augmented Diffs

mod osm;
mod change;
mod diff;

pub use osm::*;
pub use change::*;
pub use diff::*;
