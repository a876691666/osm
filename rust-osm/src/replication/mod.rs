//! OSM Replication module for fetching OSM planet replication data.
//!
//! This module provides access to the OSM planet replication feeds including
//! minute, hour, day, and changeset replication streams.

use std::io::Read;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use flate2::read::GzDecoder;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Change, Changeset, OSM};

/// Base URL for the OSM planet server
pub const BASE_URL: &str = "https://planet.osm.org";

/// Error types for replication operations
#[derive(Debug, Error)]
pub enum ReplicationError {
    #[error("unexpected status code {code} for url {url}")]
    UnexpectedStatusCode { code: u16, url: String },

    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("xml parse error: {0}")]
    XmlParse(#[from] quick_xml::DeError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Result type for replication operations
pub type ReplicationResult<T> = Result<T, ReplicationError>;

/// Check if an error is a not found error
pub fn not_found(err: &ReplicationError) -> bool {
    matches!(
        err,
        ReplicationError::UnexpectedStatusCode { code: 404, .. }
    )
}

/// State information for a replication sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub seq_num: u64,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_max: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_max_queried: Option<i64>,
}

/// Sequence number type marker trait
pub trait SeqNum: Clone + Copy {
    fn dir() -> &'static str;
    fn value(&self) -> u64;
}

/// Minute sequence number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MinuteSeqNum(pub u64);

impl SeqNum for MinuteSeqNum {
    fn dir() -> &'static str {
        "minute"
    }
    fn value(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for MinuteSeqNum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "minute/{}", self.0)
    }
}

/// Hour sequence number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HourSeqNum(pub u64);

impl SeqNum for HourSeqNum {
    fn dir() -> &'static str {
        "hour"
    }
    fn value(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for HourSeqNum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hour/{}", self.0)
    }
}

/// Day sequence number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DaySeqNum(pub u64);

impl SeqNum for DaySeqNum {
    fn dir() -> &'static str {
        "day"
    }
    fn value(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for DaySeqNum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "day/{}", self.0)
    }
}

/// Changeset sequence number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChangesetSeqNum(pub u64);

impl std::fmt::Display for ChangesetSeqNum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "changeset/{}", self.0)
    }
}

/// Starting sequence numbers
pub const MINUTE_SEQ_START: MinuteSeqNum = MinuteSeqNum(4);
pub const HOUR_SEQ_START: HourSeqNum = HourSeqNum(11);
pub const DAY_SEQ_START: DaySeqNum = DaySeqNum(1);

/// Datasource for replication data requests
#[derive(Debug, Clone)]
pub struct Datasource {
    pub base_url: String,
    client: Client,
}

impl Default for Datasource {
    fn default() -> Self {
        Self::new()
    }
}

impl Datasource {
    /// Create a new datasource with default settings
    pub fn new() -> Self {
        Self {
            base_url: BASE_URL.to_string(),
            client: Client::builder()
                .timeout(Duration::from_secs(30 * 60)) // 30 minutes
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create a new datasource with a custom client
    pub fn with_client(client: Client) -> Self {
        Self {
            base_url: BASE_URL.to_string(),
            client,
        }
    }

    /// Create a new datasource with a custom base URL
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn base_seq_url<S: SeqNum>(&self, n: S) -> String {
        let num = n.value();
        format!(
            "{}/replication/{}/{:03}/{:03}/{:03}",
            self.base_url(),
            S::dir(),
            num / 1_000_000,
            (num % 1_000_000) / 1000,
            num % 1000
        )
    }

    fn base_changeset_url(&self, n: ChangesetSeqNum) -> String {
        let num = n.0;
        format!(
            "{}/replication/changesets/{:03}/{:03}/{:03}",
            self.base_url(),
            num / 1_000_000,
            (num % 1_000_000) / 1000,
            num % 1000
        )
    }

    // ========== Minute State Methods ==========

    /// Get the current minute replication state
    pub async fn current_minute_state(&self) -> ReplicationResult<(MinuteSeqNum, State)> {
        let state = self.minute_state(MinuteSeqNum(0)).await?;
        Ok((MinuteSeqNum(state.seq_num), state))
    }

    /// Get the state for a specific minute sequence
    pub async fn minute_state(&self, n: MinuteSeqNum) -> ReplicationResult<State> {
        self.fetch_state(n).await
    }

    /// Get the change diff for a given minute
    pub async fn minute(&self, n: MinuteSeqNum) -> ReplicationResult<Change> {
        let url = format!("{}.osc.gz", self.base_seq_url(n));
        self.fetch_interval_data(&url).await
    }

    // ========== Hour State Methods ==========

    /// Get the current hour replication state
    pub async fn current_hour_state(&self) -> ReplicationResult<(HourSeqNum, State)> {
        let state = self.hour_state(HourSeqNum(0)).await?;
        Ok((HourSeqNum(state.seq_num), state))
    }

    /// Get the state for a specific hour sequence
    pub async fn hour_state(&self, n: HourSeqNum) -> ReplicationResult<State> {
        self.fetch_state(n).await
    }

    /// Get the change diff for a given hour
    pub async fn hour(&self, n: HourSeqNum) -> ReplicationResult<Change> {
        let url = format!("{}.osc.gz", self.base_seq_url(n));
        self.fetch_interval_data(&url).await
    }

    // ========== Day State Methods ==========

    /// Get the current day replication state
    pub async fn current_day_state(&self) -> ReplicationResult<(DaySeqNum, State)> {
        let state = self.day_state(DaySeqNum(0)).await?;
        Ok((DaySeqNum(state.seq_num), state))
    }

    /// Get the state for a specific day sequence
    pub async fn day_state(&self, n: DaySeqNum) -> ReplicationResult<State> {
        self.fetch_state(n).await
    }

    /// Get the change diff for a given day
    pub async fn day(&self, n: DaySeqNum) -> ReplicationResult<Change> {
        let url = format!("{}.osc.gz", self.base_seq_url(n));
        self.fetch_interval_data(&url).await
    }

    // ========== Changeset State Methods ==========

    /// Get the current changeset replication state
    pub async fn current_changeset_state(&self) -> ReplicationResult<(ChangesetSeqNum, State)> {
        let state = self.fetch_changeset_state(ChangesetSeqNum(0)).await?;
        Ok((ChangesetSeqNum(state.seq_num), state))
    }

    /// Get the state for a specific changeset sequence
    pub async fn changeset_state(&self, n: ChangesetSeqNum) -> ReplicationResult<State> {
        self.fetch_changeset_state(n).await
    }

    /// Get changesets for a given sequence number
    pub async fn changesets(&self, n: ChangesetSeqNum) -> ReplicationResult<Vec<Changeset>> {
        let url = format!("{}.osm.gz", self.base_changeset_url(n));

        let resp = self.client.get(&url).send().await?;

        if resp.status() != StatusCode::OK {
            return Err(ReplicationError::UnexpectedStatusCode {
                code: resp.status().as_u16(),
                url,
            });
        }

        let bytes = resp.bytes().await?;
        let mut gz = GzDecoder::new(&bytes[..]);
        let mut xml_data = String::new();
        gz.read_to_string(&mut xml_data)?;

        let osm: OSM = quick_xml::de::from_str(&xml_data)?;
        Ok(osm.changesets)
    }

    // ========== Internal Methods ==========

    async fn fetch_state<S: SeqNum>(&self, n: S) -> ReplicationResult<State> {
        let url = if n.value() != 0 {
            format!("{}.state.txt", self.base_seq_url(n))
        } else {
            format!("{}/replication/{}/state.txt", self.base_url(), S::dir())
        };

        let resp = self.client.get(&url).send().await?;

        if resp.status() != StatusCode::OK {
            return Err(ReplicationError::UnexpectedStatusCode {
                code: resp.status().as_u16(),
                url,
            });
        }

        let data = resp.text().await?;
        decode_interval_state(&data)
    }

    async fn fetch_changeset_state(&self, n: ChangesetSeqNum) -> ReplicationResult<State> {
        let url = if n.0 != 0 {
            format!("{}.state.txt", self.base_changeset_url(n))
        } else {
            format!("{}/replication/changesets/state.yaml", self.base_url())
        };

        let resp = self.client.get(&url).send().await?;

        if resp.status() != StatusCode::OK {
            return Err(ReplicationError::UnexpectedStatusCode {
                code: resp.status().as_u16(),
                url,
            });
        }

        let data = resp.text().await?;
        let mut state = decode_changeset_state(&data)?;

        // Handle the off-by-one error in changeset state files
        if n.0 == 0 {
            state.seq_num += 1;
        } else {
            state.seq_num = n.0;
        }

        Ok(state)
    }

    async fn fetch_interval_data(&self, url: &str) -> ReplicationResult<Change> {
        let resp = self.client.get(url).send().await?;

        if resp.status() != StatusCode::OK {
            return Err(ReplicationError::UnexpectedStatusCode {
                code: resp.status().as_u16(),
                url: url.to_string(),
            });
        }

        let bytes = resp.bytes().await?;
        let mut gz = GzDecoder::new(&bytes[..]);
        let mut xml_data = String::new();
        gz.read_to_string(&mut xml_data)?;

        let change: Change = quick_xml::de::from_str(&xml_data)?;
        Ok(change)
    }

    // ========== Search Methods ==========

    /// Find the minute state at a given timestamp
    pub async fn minute_state_at(
        &self,
        timestamp: DateTime<Utc>,
    ) -> ReplicationResult<(MinuteSeqNum, State)> {
        let state = self
            .search_timestamp::<MinuteSeqNum>(1, timestamp)
            .await?;
        Ok((MinuteSeqNum(state.seq_num), state))
    }

    /// Find the hour state at a given timestamp
    pub async fn hour_state_at(
        &self,
        timestamp: DateTime<Utc>,
    ) -> ReplicationResult<(HourSeqNum, State)> {
        let state = self.search_timestamp::<HourSeqNum>(1, timestamp).await?;
        Ok((HourSeqNum(state.seq_num), state))
    }

    /// Find the day state at a given timestamp
    pub async fn day_state_at(
        &self,
        timestamp: DateTime<Utc>,
    ) -> ReplicationResult<(DaySeqNum, State)> {
        let state = self.search_timestamp::<DaySeqNum>(1, timestamp).await?;
        Ok((DaySeqNum(state.seq_num), state))
    }

    async fn search_timestamp<S>(
        &self,
        min: u64,
        timestamp: DateTime<Utc>,
    ) -> ReplicationResult<State>
    where
        S: SeqNum + From<u64>,
    {
        // Get current state
        let upper = self.fetch_state(S::from(0)).await?;

        if timestamp > upper.timestamp {
            return Ok(upper);
        }

        // Get lower bound state
        let lower = match self.fetch_state(S::from(min)).await {
            Ok(s) => s,
            Err(e) if not_found(&e) => {
                // Binary search for a valid lower bound
                return self.find_in_range::<S>(min, upper.seq_num, timestamp).await;
            }
            Err(e) => return Err(e),
        };

        if lower.seq_num + 1 >= upper.seq_num {
            return Ok(lower);
        }

        self.find_in_range::<S>(lower.seq_num, upper.seq_num, timestamp)
            .await
    }

    async fn find_in_range<S>(
        &self,
        mut lower: u64,
        mut upper: u64,
        timestamp: DateTime<Utc>,
    ) -> ReplicationResult<State>
    where
        S: SeqNum + From<u64>,
    {
        let mut lower_state: Option<State> = None;
        let mut upper_state: Option<State> = None;

        while lower + 1 < upper {
            let mid = (lower + upper) / 2;

            match self.fetch_state(S::from(mid)).await {
                Ok(state) => {
                    if timestamp > state.timestamp {
                        lower = state.seq_num;
                        lower_state = Some(state);
                    } else {
                        upper = state.seq_num;
                        upper_state = Some(state);
                    }
                }
                Err(e) if not_found(&e) => {
                    // Try searching in lower half first
                    let new_mid = (lower + mid) / 2;
                    if new_mid > lower {
                        upper = mid;
                        continue;
                    }
                    // Otherwise try upper half
                    lower = mid;
                }
                Err(e) => return Err(e),
            }
        }

        // Return upper state if available, otherwise fetch it
        if let Some(state) = upper_state {
            Ok(state)
        } else if let Some(state) = lower_state {
            Ok(state)
        } else {
            self.fetch_state(S::from(upper)).await
        }
    }
}

impl From<u64> for MinuteSeqNum {
    fn from(v: u64) -> Self {
        MinuteSeqNum(v)
    }
}

impl From<u64> for HourSeqNum {
    fn from(v: u64) -> Self {
        HourSeqNum(v)
    }
}

impl From<u64> for DaySeqNum {
    fn from(v: u64) -> Self {
        DaySeqNum(v)
    }
}

/// Time formats used in state files
const TIME_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S%.f Z",
    "%Y-%m-%d %H:%M:%S%.f +00:00",
    "%Y-%m-%dT%H\\:%M\\:%SZ",
    "%Y-%m-%dT%H:%M:%SZ",
];

fn decode_time(s: &str) -> Result<DateTime<Utc>, ReplicationError> {
    for format in TIME_FORMATS {
        // Try parsing with timezone
        if let Ok(dt) = DateTime::parse_from_str(s, format) {
            return Ok(dt.with_timezone(&Utc));
        }
        // Try parsing as naive and assume UTC
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, format) {
            return Ok(Utc.from_utc_datetime(&dt));
        }
    }

    // Handle escaped colons
    let unescaped = s.replace("\\:", ":");
    for format in TIME_FORMATS {
        if let Ok(dt) = DateTime::parse_from_str(&unescaped, format) {
            return Ok(dt.with_timezone(&Utc));
        }
        if let Ok(dt) = NaiveDateTime::parse_from_str(&unescaped, format) {
            return Ok(Utc.from_utc_datetime(&dt));
        }
    }

    Err(ReplicationError::Parse(format!(
        "unable to parse time: {}",
        s
    )))
}

fn decode_interval_state(data: &str) -> ReplicationResult<State> {
    let mut state = State {
        seq_num: 0,
        timestamp: Utc::now(),
        txn_max: None,
        txn_max_queried: None,
    };

    for line in data.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            match key.trim() {
                "sequenceNumber" => {
                    state.seq_num = value
                        .parse()
                        .map_err(|_| ReplicationError::Parse(format!("invalid seq: {}", value)))?;
                }
                "timestamp" => {
                    state.timestamp = decode_time(value)?;
                }
                "txnMax" => {
                    state.txn_max = Some(value.parse().map_err(|_| {
                        ReplicationError::Parse(format!("invalid txnMax: {}", value))
                    })?);
                }
                "txnMaxQueried" => {
                    state.txn_max_queried = Some(value.parse().map_err(|_| {
                        ReplicationError::Parse(format!("invalid txnMaxQueried: {}", value))
                    })?);
                }
                _ => {}
            }
        }
    }

    Ok(state)
}

fn decode_changeset_state(data: &str) -> ReplicationResult<State> {
    // YAML format:
    // ---
    // last_run: 2016-07-02 22:46:01.422137422 +00:00
    // sequence: 1912325

    let mut state = State {
        seq_num: 0,
        timestamp: Utc::now(),
        txn_max: None,
        txn_max_queried: None,
    };

    for line in data.lines() {
        let line = line.trim();
        if line.starts_with("last_run:") {
            let time_str = line.trim_start_matches("last_run:").trim();
            state.timestamp = decode_time(time_str)?;
        } else if line.starts_with("sequence:") {
            let seq_str = line.trim_start_matches("sequence:").trim();
            state.seq_num = seq_str
                .parse()
                .map_err(|_| ReplicationError::Parse(format!("invalid sequence: {}", seq_str)))?;
        }
    }

    Ok(state)
}

/// Default datasource for package-level convenience functions
pub static DEFAULT_DATASOURCE: std::sync::LazyLock<Datasource> =
    std::sync::LazyLock::new(Datasource::new);

// ========== Package-level convenience functions ==========

/// Get the current minute state
pub async fn current_minute_state() -> ReplicationResult<(MinuteSeqNum, State)> {
    DEFAULT_DATASOURCE.current_minute_state().await
}

/// Get the state for a specific minute
pub async fn minute_state(n: MinuteSeqNum) -> ReplicationResult<State> {
    DEFAULT_DATASOURCE.minute_state(n).await
}

/// Get the change diff for a given minute
pub async fn minute(n: MinuteSeqNum) -> ReplicationResult<Change> {
    DEFAULT_DATASOURCE.minute(n).await
}

/// Get the current hour state
pub async fn current_hour_state() -> ReplicationResult<(HourSeqNum, State)> {
    DEFAULT_DATASOURCE.current_hour_state().await
}

/// Get the state for a specific hour
pub async fn hour_state(n: HourSeqNum) -> ReplicationResult<State> {
    DEFAULT_DATASOURCE.hour_state(n).await
}

/// Get the change diff for a given hour
pub async fn hour(n: HourSeqNum) -> ReplicationResult<Change> {
    DEFAULT_DATASOURCE.hour(n).await
}

/// Get the current day state
pub async fn current_day_state() -> ReplicationResult<(DaySeqNum, State)> {
    DEFAULT_DATASOURCE.current_day_state().await
}

/// Get the state for a specific day
pub async fn day_state(n: DaySeqNum) -> ReplicationResult<State> {
    DEFAULT_DATASOURCE.day_state(n).await
}

/// Get the change diff for a given day
pub async fn day(n: DaySeqNum) -> ReplicationResult<Change> {
    DEFAULT_DATASOURCE.day(n).await
}

/// Get the current changeset state
pub async fn current_changeset_state() -> ReplicationResult<(ChangesetSeqNum, State)> {
    DEFAULT_DATASOURCE.current_changeset_state().await
}

/// Get the state for a specific changeset sequence
pub async fn changeset_state(n: ChangesetSeqNum) -> ReplicationResult<State> {
    DEFAULT_DATASOURCE.changeset_state(n).await
}

/// Get changesets for a given sequence number
pub async fn changesets(n: ChangesetSeqNum) -> ReplicationResult<Vec<Changeset>> {
    DEFAULT_DATASOURCE.changesets(n).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seq_num_display() {
        assert_eq!(MinuteSeqNum(12345).to_string(), "minute/12345");
        assert_eq!(HourSeqNum(123).to_string(), "hour/123");
        assert_eq!(DaySeqNum(45).to_string(), "day/45");
        assert_eq!(ChangesetSeqNum(999).to_string(), "changeset/999");
    }

    #[test]
    fn test_base_seq_url() {
        let ds = Datasource::new();
        assert_eq!(
            ds.base_seq_url(MinuteSeqNum(2010580)),
            "https://planet.osm.org/replication/minute/002/010/580"
        );
        assert_eq!(
            ds.base_seq_url(HourSeqNum(1234)),
            "https://planet.osm.org/replication/hour/000/001/234"
        );
    }

    #[test]
    fn test_decode_interval_state() {
        let data = r#"#Sat Jul 16 06:14:03 UTC 2016
txnMaxQueried=836439235
sequenceNumber=2010580
timestamp=2016-07-16T06\:14\:02Z
txnReadyList=
txnMax=836439235
txnActiveList=836439008"#;

        let state = decode_interval_state(data).unwrap();
        assert_eq!(state.seq_num, 2010580);
        assert_eq!(state.txn_max, Some(836439235));
        assert_eq!(state.txn_max_queried, Some(836439235));
    }

    #[test]
    fn test_decode_changeset_state() {
        let data = r#"---
last_run: 2016-07-02 22:46:01.422137422 +00:00
sequence: 1912325"#;

        let state = decode_changeset_state(data).unwrap();
        assert_eq!(state.seq_num, 1912325);
    }

    #[test]
    fn test_not_found() {
        let err = ReplicationError::UnexpectedStatusCode {
            code: 404,
            url: "test".to_string(),
        };
        assert!(not_found(&err));

        let err = ReplicationError::UnexpectedStatusCode {
            code: 500,
            url: "test".to_string(),
        };
        assert!(!not_found(&err));
    }

    #[test]
    fn test_datasource_default() {
        let ds = Datasource::new();
        assert_eq!(ds.base_url, BASE_URL);
    }
}
