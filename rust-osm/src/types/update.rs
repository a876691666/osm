//! Update type for OSM data changes

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::ChangesetID;

/// CommitInfoStart is the start time when we know committed at information.
/// Any update.Timestamp >= this date is a committed at time.
pub fn commit_info_start() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2012, 9, 12, 9, 30, 3).unwrap()
}

/// An Update is a change to children of a way or relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub index: usize,
    pub version: i32,
    /// Timestamp is the committed at time if time > CommitInfoStart
    /// or the element timestamp if before that date
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "is_zero_changeset_id")]
    pub changeset: ChangesetID,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lat: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lon: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverse: bool,
}

fn is_zero_changeset_id(v: &ChangesetID) -> bool {
    v.0 == 0
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

impl Default for Update {
    fn default() -> Self {
        Update {
            index: 0,
            version: 0,
            timestamp: DateTime::default(),
            changeset: ChangesetID(0),
            lat: 0.0,
            lon: 0.0,
            reverse: false,
        }
    }
}

/// Updates are collections of updates
#[derive(Debug, Clone, Default)]
pub struct Updates(pub Vec<Update>);

impl Updates {
    /// Creates an empty Updates collection
    pub fn new() -> Self {
        Updates(Vec::new())
    }

    /// Returns the subset of updates taking place up to and on the given time
    pub fn up_to(&self, t: DateTime<Utc>) -> Updates {
        let result: Vec<Update> = self
            .0
            .iter()
            .filter(|u| u.timestamp <= t)
            .cloned()
            .collect();
        Updates(result)
    }

    /// Sorts updates by timestamp in ascending order
    pub fn sort_by_timestamp(&mut self) {
        self.0.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Sorts updates by index in ascending order
    pub fn sort_by_index(&mut self) {
        self.0.sort_by(|a, b| {
            if a.index != b.index {
                a.index.cmp(&b.index)
            } else {
                a.timestamp.cmp(&b.timestamp)
            }
        });
    }

    /// Returns the number of updates
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Updates {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Updates {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let updates = Vec::deserialize(deserializer)?;
        Ok(Updates(updates))
    }
}

impl From<Vec<Update>> for Updates {
    fn from(v: Vec<Update>) -> Self {
        Updates(v)
    }
}

/// Error returned when applying an update to an object
/// and the update index is out of range
#[derive(Debug, Clone, Error)]
pub enum UpdateError {
    #[error("osm: index {index} is out of range")]
    IndexOutOfRange { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_updates_up_to() {
        let t1 = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2020, 6, 1, 0, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap();

        let updates = Updates(vec![
            Update {
                index: 0,
                timestamp: t1,
                ..Default::default()
            },
            Update {
                index: 1,
                timestamp: t2,
                ..Default::default()
            },
            Update {
                index: 2,
                timestamp: t3,
                ..Default::default()
            },
        ]);

        let filtered = updates.up_to(Utc.with_ymd_and_hms(2020, 7, 1, 0, 0, 0).unwrap());
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_updates_sort_by_timestamp() {
        let t1 = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2019, 1, 1, 0, 0, 0).unwrap();

        let mut updates = Updates(vec![
            Update {
                index: 0,
                timestamp: t1,
                ..Default::default()
            },
            Update {
                index: 1,
                timestamp: t2,
                ..Default::default()
            },
        ]);

        updates.sort_by_timestamp();
        assert_eq!(updates.0[0].index, 1); // t2 (2019) should come first
    }
}
