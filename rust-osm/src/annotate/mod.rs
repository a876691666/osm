//! Annotate module for computing and annotating OSM element changes.
//!
//! This module provides functionality to:
//! - Convert OSM Change to Diff with previous version information
//! - Annotate Ways with node history (lat/lon/changeset data)
//! - Annotate Relations with member history
//!
//! The AnnotateError process uses a HistoryDatasourcer to look up previous
//! versions of elements and compute the changes.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    Action, ActionType, Change, ChangesetID, Diff, FeatureID, Node, NodeID, OSM, Relation,
    RelationID, Update, Updates, Way, WayID,
};

/// Error types for annotate operations
#[derive(Debug, Error)]
pub enum AnnotateError {
    #[error("no visible child found for {0}")]
    NoVisibleChild(FeatureID),

    #[error("datasource error: {0}")]
    Datasource(String),

    #[error("element not found: {0}")]
    NotFound(FeatureID),
}

/// Result type for annotate operations
pub type AnnotateResult<T> = Result<T, AnnotateError>;

/// Options for annotation operations
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// If true, missing children will not cause errors
    pub ignore_missing_children: bool,
    /// Threshold for determining if updates are "close enough" in time
    pub threshold: Option<std::time::Duration>,
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ignore_missing_children(mut self, v: bool) -> Self {
        self.ignore_missing_children = v;
        self
    }

    pub fn threshold(mut self, d: std::time::Duration) -> Self {
        self.threshold = Some(d);
        self
    }
}

/// Trait for providing history data for OSM elements.
/// Implementations should provide methods to fetch all versions of elements.
pub trait HistoryDatasourcer: Send + Sync {
    /// Get all versions of a node
    fn node_history(&self, id: NodeID) -> AnnotateResult<Vec<Node>>;

    /// Get all versions of a way
    fn way_history(&self, id: WayID) -> AnnotateResult<Vec<Way>>;

    /// Get all versions of a relation
    fn relation_history(&self, id: RelationID) -> AnnotateResult<Vec<Relation>>;

    /// Check if an error indicates element not found
    fn not_found(&self, err: &AnnotateError) -> bool;
}

/// Child element information used during annotation
#[derive(Debug, Clone)]
pub struct Child {
    pub version: i32,
    pub changeset: ChangesetID,
    pub lat: f64,
    pub lon: f64,
    pub timestamp: Option<DateTime<Utc>>,
    pub way: Option<Way>,
}

/// Convert a Change into a Diff by looking up previous versions of elements.
///
/// This function takes an OSM Change (from replication API) and converts it
/// to a Diff by determining what the previous state was for each element.
///
/// # Arguments
/// * `change` - The change to convert
/// * `ds` - A datasource for looking up element history
/// * `opts` - Options for the conversion
///
/// # Returns
/// A Diff containing Actions with both old and new states
pub fn change_to_diff<D: HistoryDatasourcer>(
    change: &Change,
    ds: &D,
    opts: &Options,
) -> AnnotateResult<Diff> {
    let mut actions = Vec::new();

    // Creates are all "new" things
    if let Some(create) = &change.create {
        for n in &create.nodes {
            let mut node = n.clone();
            node.visible = true;
            actions.push(Action {
                action_type: ActionType::Create,
                osm: Some(OSM {
                    nodes: vec![node],
                    ..Default::default()
                }),
                old: None,
                new: None,
            });
        }

        for w in &create.ways {
            let mut way = w.clone();
            way.visible = true;
            actions.push(Action {
                action_type: ActionType::Create,
                osm: Some(OSM {
                    ways: vec![way],
                    ..Default::default()
                }),
                old: None,
                new: None,
            });
        }

        for r in &create.relations {
            let mut rel = r.clone();
            rel.visible = true;
            actions.push(Action {
                action_type: ActionType::Create,
                osm: Some(OSM {
                    relations: vec![rel],
                    ..Default::default()
                }),
                old: None,
                new: None,
            });
        }
    }

    // Modify
    actions = add_update(
        actions,
        change.modify.as_ref(),
        ActionType::Modify,
        ds,
        opts.ignore_missing_children,
    )?;

    // Delete
    actions = add_update(
        actions,
        change.delete.as_ref(),
        ActionType::Delete,
        ds,
        opts.ignore_missing_children,
    )?;

    Ok(Diff {
        actions,
        changesets: Vec::new(),
    })
}

fn add_update<D: HistoryDatasourcer>(
    mut actions: Vec<Action>,
    osm_opt: Option<&OSM>,
    action_type: ActionType,
    ds: &D,
    ignore_missing: bool,
) -> AnnotateResult<Vec<Action>> {
    let osm = match osm_opt {
        Some(o) => o,
        None => return Ok(actions),
    };

    let current_visible = action_type != ActionType::Delete;

    for n in &osm.nodes {
        let old = find_previous_node(n, ds, ignore_missing)?;

        if old.is_none() {
            let mut node = n.clone();
            node.visible = true;
            actions.push(Action {
                action_type: ActionType::Create,
                osm: Some(OSM {
                    nodes: vec![node],
                    ..Default::default()
                }),
                old: None,
                new: None,
            });
            continue;
        }

        let mut node = n.clone();
        node.visible = current_visible;
        actions.push(Action {
            action_type,
            osm: None,
            old: Some(OSM {
                nodes: vec![old.unwrap()],
                ..Default::default()
            }),
            new: Some(OSM {
                nodes: vec![node],
                ..Default::default()
            }),
        });
    }

    for w in &osm.ways {
        let old = find_previous_way(w, ds, ignore_missing)?;

        if old.is_none() {
            let mut way = w.clone();
            way.visible = true;
            actions.push(Action {
                action_type: ActionType::Create,
                osm: Some(OSM {
                    ways: vec![way],
                    ..Default::default()
                }),
                old: None,
                new: None,
            });
            continue;
        }

        let mut way = w.clone();
        way.visible = current_visible;
        actions.push(Action {
            action_type,
            osm: None,
            old: Some(OSM {
                ways: vec![old.unwrap()],
                ..Default::default()
            }),
            new: Some(OSM {
                ways: vec![way],
                ..Default::default()
            }),
        });
    }

    for r in &osm.relations {
        let old = find_previous_relation(r, ds, ignore_missing)?;

        if old.is_none() {
            let mut rel = r.clone();
            rel.visible = true;
            actions.push(Action {
                action_type: ActionType::Create,
                osm: Some(OSM {
                    relations: vec![rel],
                    ..Default::default()
                }),
                old: None,
                new: None,
            });
            continue;
        }

        let mut rel = r.clone();
        rel.visible = current_visible;
        actions.push(Action {
            action_type,
            osm: None,
            old: Some(OSM {
                relations: vec![old.unwrap()],
                ..Default::default()
            }),
            new: Some(OSM {
                relations: vec![rel],
                ..Default::default()
            }),
        });
    }

    Ok(actions)
}

fn find_previous_node<D: HistoryDatasourcer>(
    n: &Node,
    ds: &D,
    ignore_missing: bool,
) -> AnnotateResult<Option<Node>> {
    let nodes = match ds.node_history(n.id) {
        Ok(nodes) => nodes,
        Err(e) => {
            if ignore_missing && ds.not_found(&e) {
                return Ok(None);
            }
            return Err(e);
        }
    };

    let mut best: Option<&Node> = None;
    for node in &nodes {
        if node.version < n.version
            && (best.is_none() || node.version > best.unwrap().version) {
                best = Some(node);
            }
    }

    if best.is_none() && !ignore_missing {
        return Err(AnnotateError::NoVisibleChild(n.feature_id()));
    }

    Ok(best.cloned())
}

fn find_previous_way<D: HistoryDatasourcer>(
    w: &Way,
    ds: &D,
    ignore_missing: bool,
) -> AnnotateResult<Option<Way>> {
    let ways = match ds.way_history(w.id) {
        Ok(ways) => ways,
        Err(e) => {
            if ignore_missing && ds.not_found(&e) {
                return Ok(None);
            }
            return Err(e);
        }
    };

    let mut best: Option<&Way> = None;
    for way in &ways {
        if way.version < w.version
            && (best.is_none() || way.version > best.unwrap().version) {
                best = Some(way);
            }
    }

    if best.is_none() && !ignore_missing {
        return Err(AnnotateError::NoVisibleChild(w.feature_id()));
    }

    Ok(best.cloned())
}

fn find_previous_relation<D: HistoryDatasourcer>(
    r: &Relation,
    ds: &D,
    ignore_missing: bool,
) -> AnnotateResult<Option<Relation>> {
    let relations = match ds.relation_history(r.id) {
        Ok(rels) => rels,
        Err(e) => {
            if ignore_missing && ds.not_found(&e) {
                return Ok(None);
            }
            return Err(e);
        }
    };

    let mut best: Option<&Relation> = None;
    for rel in &relations {
        if rel.version < r.version
            && (best.is_none() || rel.version > best.unwrap().version) {
                best = Some(rel);
            }
    }

    if best.is_none() && !ignore_missing {
        return Err(AnnotateError::NoVisibleChild(r.feature_id()));
    }

    Ok(best.cloned())
}

/// Annotate ways with node history information.
///
/// This function updates the way nodes with their historical lat/lon/changeset data
/// that was valid at the time the way was committed.
///
/// # Arguments
/// * `ways` - The ways to annotate (modified in place via return)
/// * `ds` - A datasource for looking up node history
/// * `opts` - Options for the annotation
///
/// # Returns
/// The annotated ways with updates computed
pub fn annotate_ways<D: HistoryDatasourcer>(
    ways: &[Way],
    ds: &D,
    opts: &Options,
) -> AnnotateResult<Vec<Way>> {
    let threshold = opts
        .threshold
        .unwrap_or(std::time::Duration::from_secs(60));

    let mut result = Vec::with_capacity(ways.len());

    for way in ways {
        let mut annotated_way = way.clone();
        let mut updates: Vec<Update> = Vec::new();

        // Get the committed time for the way
        let committed = way.committed.unwrap_or(way.timestamp);

        // Build a map of node histories
        let mut node_children: HashMap<NodeID, Vec<Child>> = HashMap::new();

        for wn in &way.nodes.0 {
            if node_children.contains_key(&wn.id) {
                continue;
            }

            let history = match ds.node_history(wn.id) {
                Ok(h) => h,
                Err(e) => {
                    if opts.ignore_missing_children && ds.not_found(&e) {
                        continue;
                    }
                    return Err(e);
                }
            };

            let children: Vec<Child> = history
                .iter()
                .map(|n| Child {
                    version: n.version,
                    changeset: n.changeset,
                    lat: n.lat,
                    lon: n.lon,
                    timestamp: Some(n.timestamp),
                    way: None,
                })
                .collect();

            node_children.insert(wn.id, children);
        }

        // Annotate each node in the way
        for (idx, wn) in annotated_way.nodes.0.iter_mut().enumerate() {
            if let Some(children) = node_children.get(&wn.id) {
                // Find the child that was valid at the committed time
                let child = find_child_at_time(children, committed, threshold);

                if let Some(c) = child {
                    wn.version = c.version;
                    wn.changeset = c.changeset;
                    wn.lat = c.lat;
                    wn.lon = c.lon;

                    // Check if this represents an update
                    if idx > 0 {
                        if let Some(prev_child) =
                            find_child_at_time(children, committed, threshold)
                        {
                            if prev_child.version != c.version {
                                updates.push(Update {
                                    index: idx,
                                    version: c.version,
                                    changeset: c.changeset,
                                    timestamp: c.timestamp.unwrap_or(committed),
                                    lat: c.lat,
                                    lon: c.lon,
                                    reverse: false,
                                });
                            }
                        }
                    }
                }
            }
        }

        annotated_way.updates = if updates.is_empty() {
            Updates::new()
        } else {
            Updates::from(updates)
        };

        result.push(annotated_way);
    }

    Ok(result)
}

/// Annotate relations with member history information.
///
/// This function updates the relation members with their historical version/changeset data.
///
/// # Arguments
/// * `relations` - The relations to annotate
/// * `ds` - A datasource for looking up element history
/// * `opts` - Options for the annotation
///
/// # Returns
/// The annotated relations with updates computed
pub fn annotate_relations<D: HistoryDatasourcer>(
    relations: &[Relation],
    ds: &D,
    opts: &Options,
) -> AnnotateResult<Vec<Relation>> {
    let threshold = opts
        .threshold
        .unwrap_or(std::time::Duration::from_secs(60));

    let mut result = Vec::with_capacity(relations.len());

    for relation in relations {
        let mut annotated = relation.clone();
        let committed = relation.committed.unwrap_or(relation.timestamp);

        // Annotate each member
        for member in &mut annotated.members.0 {
            let children: Vec<Child> = match member.member_type {
                crate::Type::Node => {
                    let id = NodeID(member.ref_);
                    match ds.node_history(id) {
                        Ok(nodes) => nodes
                            .iter()
                            .map(|n| Child {
                                version: n.version,
                                changeset: n.changeset,
                                lat: n.lat,
                                lon: n.lon,
                                timestamp: Some(n.timestamp),
                                way: None,
                            })
                            .collect(),
                        Err(e) => {
                            if opts.ignore_missing_children && ds.not_found(&e) {
                                continue;
                            }
                            return Err(e);
                        }
                    }
                }
                crate::Type::Way => {
                    let id = WayID(member.ref_);
                    match ds.way_history(id) {
                        Ok(ways) => ways
                            .iter()
                            .map(|w| Child {
                                version: w.version,
                                changeset: w.changeset,
                                lat: 0.0,
                                lon: 0.0,
                                timestamp: Some(w.timestamp),
                                way: Some(w.clone()),
                            })
                            .collect(),
                        Err(e) => {
                            if opts.ignore_missing_children && ds.not_found(&e) {
                                continue;
                            }
                            return Err(e);
                        }
                    }
                }
                crate::Type::Relation => {
                    let id = RelationID(member.ref_);
                    match ds.relation_history(id) {
                        Ok(rels) => rels
                            .iter()
                            .map(|r| Child {
                                version: r.version,
                                changeset: r.changeset,
                                lat: 0.0,
                                lon: 0.0,
                                timestamp: Some(r.timestamp),
                                way: None,
                            })
                            .collect(),
                        Err(e) => {
                            if opts.ignore_missing_children && ds.not_found(&e) {
                                continue;
                            }
                            return Err(e);
                        }
                    }
                }
                _ => continue,
            };

            if let Some(child) = find_child_at_time(&children, committed, threshold) {
                member.version = child.version;
                member.changeset = child.changeset;
                member.lat = child.lat;
                member.lon = child.lon;
            }
        }

        result.push(annotated);
    }

    Ok(result)
}

/// Find the child element that was valid at a given time
fn find_child_at_time(
    children: &[Child],
    at: DateTime<Utc>,
    threshold: std::time::Duration,
) -> Option<Child> {
    let threshold_chrono = chrono::Duration::from_std(threshold).unwrap_or(chrono::Duration::zero());

    // First, try to find an exact match (child timestamp <= at)
    let mut best: Option<&Child> = None;
    for child in children {
        if let Some(ts) = child.timestamp {
            if ts <= at
                && (best.is_none() || ts > best.unwrap().timestamp.unwrap()) {
                    best = Some(child);
                }
        }
    }

    // If no exact match, find the closest one within threshold
    if best.is_none() {
        for child in children {
            if let Some(ts) = child.timestamp {
                let diff = if ts > at { ts - at } else { at - ts };
                if diff <= threshold_chrono {
                    if best.is_none() {
                        best = Some(child);
                    } else if let Some(best_ts) = best.unwrap().timestamp {
                        let best_diff = if best_ts > at {
                            best_ts - at
                        } else {
                            at - best_ts
                        };
                        if diff < best_diff {
                            best = Some(child);
                        }
                    }
                }
            }
        }
    }

    best.cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tags;

    struct MockDatasource {
        nodes: HashMap<NodeID, Vec<Node>>,
        ways: HashMap<WayID, Vec<Way>>,
        relations: HashMap<RelationID, Vec<Relation>>,
    }

    impl MockDatasource {
        fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                ways: HashMap::new(),
                relations: HashMap::new(),
            }
        }
    }

    impl HistoryDatasourcer for MockDatasource {
        fn node_history(&self, id: NodeID) -> AnnotateResult<Vec<Node>> {
            self.nodes
                .get(&id)
                .cloned()
                .ok_or(AnnotateError::NotFound(id.feature_id()))
        }

        fn way_history(&self, id: WayID) -> AnnotateResult<Vec<Way>> {
            self.ways
                .get(&id)
                .cloned()
                .ok_or(AnnotateError::NotFound(id.feature_id()))
        }

        fn relation_history(&self, id: RelationID) -> AnnotateResult<Vec<Relation>> {
            self.relations
                .get(&id)
                .cloned()
                .ok_or(AnnotateError::NotFound(id.feature_id()))
        }

        fn not_found(&self, err: &AnnotateError) -> bool {
            matches!(err, AnnotateError::NotFound(_))
        }
    }

    #[test]
    fn test_options() {
        let opts = Options::new()
            .ignore_missing_children(true)
            .threshold(std::time::Duration::from_secs(120));

        assert!(opts.ignore_missing_children);
        assert_eq!(opts.threshold, Some(std::time::Duration::from_secs(120)));
    }

    #[test]
    fn test_change_to_diff_creates() {
        let ds = MockDatasource::new();
        let opts = Options::new();

        let change = Change {
            create: Some(OSM {
                nodes: vec![Node {
                    id: NodeID(1),
                    version: 1,
                    lat: 1.0,
                    lon: 2.0,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            modify: None,
            delete: None,
            ..Default::default()
        };

        let diff = change_to_diff(&change, &ds, &opts).unwrap();

        assert_eq!(diff.actions.len(), 1);
        assert_eq!(diff.actions[0].action_type, ActionType::Create);
    }

    #[test]
    fn test_find_previous_node() {
        let mut ds = MockDatasource::new();

        ds.nodes.insert(
            NodeID(1),
            vec![
                Node {
                    id: NodeID(1),
                    version: 1,
                    lat: 1.0,
                    lon: 1.0,
                    tags: Tags::default(),
                    ..Default::default()
                },
                Node {
                    id: NodeID(1),
                    version: 2,
                    lat: 2.0,
                    lon: 2.0,
                    tags: Tags::default(),
                    ..Default::default()
                },
            ],
        );

        let current = Node {
            id: NodeID(1),
            version: 3,
            ..Default::default()
        };

        let prev = find_previous_node(&current, &ds, false).unwrap();
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().version, 2);
    }

    #[test]
    fn test_find_child_at_time() {
        let now = Utc::now();
        let earlier = now - chrono::Duration::hours(1);
        let much_earlier = now - chrono::Duration::hours(2);

        let children = vec![
            Child {
                version: 1,
                changeset: ChangesetID(100),
                lat: 1.0,
                lon: 1.0,
                timestamp: Some(much_earlier),
                way: None,
            },
            Child {
                version: 2,
                changeset: ChangesetID(200),
                lat: 2.0,
                lon: 2.0,
                timestamp: Some(earlier),
                way: None,
            },
        ];

        let threshold = std::time::Duration::from_secs(60);

        // Should find version 2 as it's the latest before "now"
        let found = find_child_at_time(&children, now, threshold);
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, 2);

        // Should find version 1 as the latest before "earlier"
        let found = find_child_at_time(&children, much_earlier, threshold);
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, 1);
    }
}
