//! Diff type for OSM augmented diffs

use serde::{Deserialize, Serialize};

use super::OSM;
use crate::types::Changeset;

/// Diff represents a difference of osm data with old and new data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Diff {
    pub actions: Vec<Action>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changesets: Vec<Changeset>,
}

impl Diff {
    /// Creates a new empty Diff
    pub fn new() -> Self {
        Diff::default()
    }
}

/// Actions is a set of diff actions
pub type Actions = Vec<Action>;

/// Action is an explicit create, modify or delete action with
/// old and new data if applicable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(rename = "type")]
    pub action_type: ActionType,
    /// For Create: contains the new element
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osm: Option<OSM>,
    /// For Modify/Delete: contains the old element
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<OSM>,
    /// For Modify/Delete: contains the new element
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<OSM>,
}

impl Default for Action {
    fn default() -> Self {
        Action {
            action_type: ActionType::Create,
            osm: None,
            old: None,
            new: None,
        }
    }
}

/// ActionType is a strong type for the different diff actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    #[default]
    Create,
    Modify,
    Delete,
}

impl ActionType {
    /// Returns the string representation of the action type
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::Create => "create",
            ActionType::Modify => "modify",
            ActionType::Delete => "delete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_as_str() {
        assert_eq!(ActionType::Create.as_str(), "create");
        assert_eq!(ActionType::Modify.as_str(), "modify");
        assert_eq!(ActionType::Delete.as_str(), "delete");
    }
}
