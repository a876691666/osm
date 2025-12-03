//! Core OSM types module
//!
//! This module contains the fundamental types used in OpenStreetMap data:
//! - Node: A point with coordinates
//! - Way: An ordered list of nodes
//! - Relation: A group of elements with roles
//! - Changeset: Metadata about a set of changes
//! - Note: Map notes/comments
//! - User: OSM user information

mod bounds;
mod changeset;
mod ids;
mod node;
mod note;
mod relation;
mod tag;
mod update;
mod user;
mod way;

pub use bounds::*;
pub use changeset::*;
pub use ids::*;
pub use node::*;
pub use note::*;
pub use relation::*;
pub use tag::*;
pub use update::*;
pub use user::*;
pub use way::*;
