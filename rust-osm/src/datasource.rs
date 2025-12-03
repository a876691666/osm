//! Datasource for OSM history data
//!
//! Provides a way to store and retrieve OSM element history by feature ID.

use std::collections::HashMap;

use crate::types::{Node, NodeID, Nodes, Relation, RelationID, Relations, Way, WayID, Ways};

/// Error returned when a feature is not found
#[derive(Debug, Clone, thiserror::Error)]
#[error("osm: feature not found")]
pub struct NotFoundError;

/// A HistoryDatasourcer defines an interface to osm history data
pub trait HistoryDatasourcer {
    fn node_history(&self, id: NodeID) -> Result<Nodes, NotFoundError>;
    fn way_history(&self, id: WayID) -> Result<Ways, NotFoundError>;
    fn relation_history(&self, id: RelationID) -> Result<Relations, NotFoundError>;
    fn not_found(&self, err: &NotFoundError) -> bool;
}

/// A HistoryDatasource wraps maps to implement the HistoryDatasourcer interface
#[derive(Debug, Clone, Default)]
pub struct HistoryDatasource {
    pub nodes: HashMap<NodeID, Vec<Node>>,
    pub ways: HashMap<WayID, Vec<Way>>,
    pub relations: HashMap<RelationID, Vec<Relation>>,
}

impl HistoryDatasource {
    /// Creates a new empty HistoryDatasource
    pub fn new() -> Self {
        HistoryDatasource::default()
    }

    /// Adds nodes, ways, and relations from an OSM container
    pub fn add(&mut self, osm: &crate::container::OSM, visible: Option<bool>) {
        for n in &osm.nodes {
            let mut node = n.clone();
            if let Some(v) = visible {
                node.visible = v;
            }
            self.nodes.entry(n.id).or_default().push(node);
        }

        for w in &osm.ways {
            let mut way = w.clone();
            if let Some(v) = visible {
                way.visible = v;
            }
            self.ways.entry(w.id).or_default().push(way);
        }

        for r in &osm.relations {
            let mut relation = r.clone();
            if let Some(v) = visible {
                relation.visible = v;
            }
            self.relations.entry(r.id).or_default().push(relation);
        }
    }

    /// Returns the history for the given node id
    pub fn node_history(&self, id: NodeID) -> Result<Nodes, NotFoundError> {
        self.nodes
            .get(&id)
            .map(|v| Nodes(v.clone()))
            .ok_or(NotFoundError)
    }

    /// Returns the history for the given way id
    pub fn way_history(&self, id: WayID) -> Result<Ways, NotFoundError> {
        self.ways
            .get(&id)
            .map(|v| Ways(v.clone()))
            .ok_or(NotFoundError)
    }

    /// Returns the history for the given relation id
    pub fn relation_history(&self, id: RelationID) -> Result<Relations, NotFoundError> {
        self.relations
            .get(&id)
            .map(|v| Relations(v.clone()))
            .ok_or(NotFoundError)
    }

    /// Returns true if the error is a not found error
    pub fn not_found(&self, _err: &NotFoundError) -> bool {
        true
    }
}

impl HistoryDatasourcer for HistoryDatasource {
    fn node_history(&self, id: NodeID) -> Result<Nodes, NotFoundError> {
        self.node_history(id)
    }

    fn way_history(&self, id: WayID) -> Result<Ways, NotFoundError> {
        self.way_history(id)
    }

    fn relation_history(&self, id: RelationID) -> Result<Relations, NotFoundError> {
        self.relation_history(id)
    }

    fn not_found(&self, err: &NotFoundError) -> bool {
        self.not_found(err)
    }
}

// Extend OSM to provide history datasource
impl crate::container::OSM {
    /// Converts the OSM object to a datasource accessible by feature id
    pub fn history_datasource(&self) -> HistoryDatasource {
        let mut ds = HistoryDatasource::new();
        ds.add(self, None);
        ds
    }
}

// Extend Change to provide history datasource
impl crate::container::Change {
    /// Converts the change object to a datasource accessible by feature id.
    /// All the creates, modifies and deletes will be added in that order.
    pub fn history_datasource(&self) -> HistoryDatasource {
        let mut ds = HistoryDatasource::new();

        if let Some(create) = &self.create {
            ds.add(create, Some(true));
        }
        if let Some(modify) = &self.modify {
            ds.add(modify, Some(true));
        }
        if let Some(delete) = &self.delete {
            ds.add(delete, Some(false));
        }

        ds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::OSM;

    #[test]
    fn test_history_datasource_new() {
        let ds = HistoryDatasource::new();
        assert!(ds.nodes.is_empty());
        assert!(ds.ways.is_empty());
        assert!(ds.relations.is_empty());
    }

    #[test]
    fn test_history_datasource_add() {
        let mut osm = OSM::new();
        osm.nodes.push(Node {
            id: NodeID(1),
            version: 1,
            ..Default::default()
        });
        osm.nodes.push(Node {
            id: NodeID(1),
            version: 2,
            ..Default::default()
        });

        let ds = osm.history_datasource();
        let history = ds.node_history(NodeID(1)).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_history_datasource_not_found() {
        let ds = HistoryDatasource::new();
        let result = ds.node_history(NodeID(999));
        assert!(result.is_err());
    }
}
