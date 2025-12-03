//! OSM API client module for interacting with the OSM v0.6 API.
//!
//! This module provides an interface to fetch nodes, ways, relations, changesets,
//! notes, and users from the OpenStreetMap API.

use std::time::Duration;

use reqwest::{Client, StatusCode};
use thiserror::Error;

use crate::{
    Bounds, Change, Changeset, ChangesetID, Node, NodeID, Note, NoteID, OSM, Relation,
    RelationID, User, UserID, Way, WayID,
};

/// Base URL for the OSM API
pub const BASE_URL: &str = "https://api.openstreetmap.org/api/0.6";

/// Error types for API operations
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found at {url}")]
    NotFound { url: String },

    #[error("forbidden at {url}")]
    Forbidden { url: String },

    #[error("gone at {url}")]
    Gone { url: String },

    #[error("uri too long at {url}")]
    RequestURITooLong { url: String },

    #[error("unexpected status code {code} for url {url}")]
    UnexpectedStatusCode { code: u16, url: String },

    #[error("wrong number of elements, expected {expected}, got {got}")]
    WrongCount { expected: usize, got: usize },

    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("xml parse error: {0}")]
    XmlParse(#[from] quick_xml::DeError),

    #[error("invalid option: {0}")]
    InvalidOption(String),
}

/// Result type for API operations
pub type ApiResult<T> = Result<T, ApiError>;

/// Check if an error is a not found error
pub fn not_found(err: &ApiError) -> bool {
    matches!(err, ApiError::NotFound { .. })
}

/// Options for feature requests
#[derive(Debug, Clone, Default)]
pub struct FeatureOptions {
    /// Request features at a specific time (osm.fyi extension)
    pub at: Option<chrono::DateTime<chrono::Utc>>,
}

impl FeatureOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn at(mut self, time: chrono::DateTime<chrono::Utc>) -> Self {
        self.at = Some(time);
        self
    }

    fn to_params(&self) -> String {
        let mut params = Vec::new();
        if let Some(t) = &self.at {
            params.push(format!("at={}", t.format("%Y-%m-%dT%H:%M:%SZ")));
        }
        params.join("&")
    }
}

/// Options for notes requests
#[derive(Debug, Clone, Default)]
pub struct NotesOptions {
    /// Limit number of results (1-10000, default 100)
    pub limit: Option<i32>,
    /// Max days closed (-1 for all, 0 for open only, default 7)
    pub max_days_closed: Option<i32>,
}

impl NotesOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn limit(mut self, n: i32) -> ApiResult<Self> {
        if !(1..=10000).contains(&n) {
            return Err(ApiError::InvalidOption(
                "limit must be between 1 and 10000".to_string(),
            ));
        }
        self.limit = Some(n);
        Ok(self)
    }

    pub fn max_days_closed(mut self, n: i32) -> Self {
        self.max_days_closed = Some(n);
        self
    }

    fn to_params(&self) -> Vec<String> {
        let mut params = Vec::new();
        if let Some(n) = self.limit {
            params.push(format!("limit={}", n));
        }
        if let Some(n) = self.max_days_closed {
            params.push(format!("closed={}", n));
        }
        params
    }
}

/// Datasource for making API requests
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
                .timeout(Duration::from_secs(360)) // 6 minutes like Go version
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

    async fn get_from_api<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> ApiResult<T> {
        let resp = self.client.get(url).send().await?;

        match resp.status() {
            StatusCode::OK => {
                let text = resp.text().await?;
                let result: T = quick_xml::de::from_str(&text)?;
                Ok(result)
            }
            StatusCode::NOT_FOUND => Err(ApiError::NotFound {
                url: url.to_string(),
            }),
            StatusCode::FORBIDDEN => Err(ApiError::Forbidden {
                url: url.to_string(),
            }),
            StatusCode::GONE => Err(ApiError::Gone {
                url: url.to_string(),
            }),
            StatusCode::URI_TOO_LONG => Err(ApiError::RequestURITooLong {
                url: url.to_string(),
            }),
            status => Err(ApiError::UnexpectedStatusCode {
                code: status.as_u16(),
                url: url.to_string(),
            }),
        }
    }

    /// Check if an error is not found
    pub fn not_found(&self, err: &ApiError) -> bool {
        not_found(err)
    }

    // ========== Node Methods ==========

    /// Get the latest version of a node
    pub async fn node(&self, id: NodeID, opts: Option<FeatureOptions>) -> ApiResult<Node> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/node/{}?{}", self.base_url(), id.0, params);

        let osm: OSM = self.get_from_api(&url).await?;
        if osm.nodes.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.nodes.len(),
            });
        }
        Ok(osm.nodes.into_iter().next().unwrap())
    }

    /// Get multiple nodes by IDs
    pub async fn nodes(
        &self,
        ids: &[NodeID],
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Vec<Node>> {
        let params = opts.unwrap_or_default().to_params();
        let ids_str: Vec<String> = ids.iter().map(|id| id.0.to_string()).collect();
        let mut url = format!(
            "{}/nodes?nodes={}",
            self.base_url(),
            ids_str.join(",")
        );
        if !params.is_empty() {
            url.push('&');
            url.push_str(&params);
        }

        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.nodes)
    }

    /// Get a specific version of a node
    pub async fn node_version(&self, id: NodeID, version: i32) -> ApiResult<Node> {
        let url = format!("{}/node/{}/{}", self.base_url(), id.0, version);

        let osm: OSM = self.get_from_api(&url).await?;
        if osm.nodes.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.nodes.len(),
            });
        }
        Ok(osm.nodes.into_iter().next().unwrap())
    }

    /// Get all versions of a node
    pub async fn node_history(&self, id: NodeID) -> ApiResult<Vec<Node>> {
        let url = format!("{}/node/{}/history", self.base_url(), id.0);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.nodes)
    }

    /// Get all ways containing a node
    pub async fn node_ways(
        &self,
        id: NodeID,
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Vec<Way>> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/node/{}/ways?{}", self.base_url(), id.0, params);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.ways)
    }

    /// Get all relations containing a node
    pub async fn node_relations(
        &self,
        id: NodeID,
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Vec<Relation>> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/node/{}/relations?{}", self.base_url(), id.0, params);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.relations)
    }

    // ========== Way Methods ==========

    /// Get the latest version of a way
    pub async fn way(&self, id: WayID, opts: Option<FeatureOptions>) -> ApiResult<Way> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/way/{}?{}", self.base_url(), id.0, params);

        let osm: OSM = self.get_from_api(&url).await?;
        if osm.ways.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.ways.len(),
            });
        }
        Ok(osm.ways.into_iter().next().unwrap())
    }

    /// Get multiple ways by IDs
    pub async fn ways(&self, ids: &[WayID], opts: Option<FeatureOptions>) -> ApiResult<Vec<Way>> {
        let params = opts.unwrap_or_default().to_params();
        let ids_str: Vec<String> = ids.iter().map(|id| id.0.to_string()).collect();
        let mut url = format!("{}/ways?ways={}", self.base_url(), ids_str.join(","));
        if !params.is_empty() {
            url.push('&');
            url.push_str(&params);
        }

        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.ways)
    }

    /// Get a specific version of a way
    pub async fn way_version(&self, id: WayID, version: i32) -> ApiResult<Way> {
        let url = format!("{}/way/{}/{}", self.base_url(), id.0, version);

        let osm: OSM = self.get_from_api(&url).await?;
        if osm.ways.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.ways.len(),
            });
        }
        Ok(osm.ways.into_iter().next().unwrap())
    }

    /// Get all versions of a way
    pub async fn way_history(&self, id: WayID) -> ApiResult<Vec<Way>> {
        let url = format!("{}/way/{}/history", self.base_url(), id.0);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.ways)
    }

    /// Get all relations containing a way
    pub async fn way_relations(
        &self,
        id: WayID,
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Vec<Relation>> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/way/{}/relations?{}", self.base_url(), id.0, params);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.relations)
    }

    /// Get a way with all its nodes
    pub async fn way_full(&self, id: WayID, opts: Option<FeatureOptions>) -> ApiResult<OSM> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/way/{}/full?{}", self.base_url(), id.0, params);
        self.get_from_api(&url).await
    }

    // ========== Relation Methods ==========

    /// Get the latest version of a relation
    pub async fn relation(
        &self,
        id: RelationID,
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Relation> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/relation/{}?{}", self.base_url(), id.0, params);

        let osm: OSM = self.get_from_api(&url).await?;
        if osm.relations.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.relations.len(),
            });
        }
        Ok(osm.relations.into_iter().next().unwrap())
    }

    /// Get multiple relations by IDs
    pub async fn relations(
        &self,
        ids: &[RelationID],
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Vec<Relation>> {
        let params = opts.unwrap_or_default().to_params();
        let ids_str: Vec<String> = ids.iter().map(|id| id.0.to_string()).collect();
        let mut url = format!(
            "{}/relations?relations={}",
            self.base_url(),
            ids_str.join(",")
        );
        if !params.is_empty() {
            url.push('&');
            url.push_str(&params);
        }

        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.relations)
    }

    /// Get a specific version of a relation
    pub async fn relation_version(&self, id: RelationID, version: i32) -> ApiResult<Relation> {
        let url = format!("{}/relation/{}/{}", self.base_url(), id.0, version);

        let osm: OSM = self.get_from_api(&url).await?;
        if osm.relations.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.relations.len(),
            });
        }
        Ok(osm.relations.into_iter().next().unwrap())
    }

    /// Get all versions of a relation
    pub async fn relation_history(&self, id: RelationID) -> ApiResult<Vec<Relation>> {
        let url = format!("{}/relation/{}/history", self.base_url(), id.0);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.relations)
    }

    /// Get all relations containing a relation
    pub async fn relation_relations(
        &self,
        id: RelationID,
        opts: Option<FeatureOptions>,
    ) -> ApiResult<Vec<Relation>> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/relation/{}/relations?{}", self.base_url(), id.0, params);
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.relations)
    }

    /// Get a relation with all its members
    pub async fn relation_full(
        &self,
        id: RelationID,
        opts: Option<FeatureOptions>,
    ) -> ApiResult<OSM> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!("{}/relation/{}/full?{}", self.base_url(), id.0, params);
        self.get_from_api(&url).await
    }

    // ========== Changeset Methods ==========

    /// Get a changeset
    pub async fn changeset(&self, id: ChangesetID) -> ApiResult<Changeset> {
        let url = format!("{}/changeset/{}", self.base_url(), id.0);
        self.get_changeset(&url).await
    }

    /// Get a changeset with its discussion
    pub async fn changeset_with_discussion(&self, id: ChangesetID) -> ApiResult<Changeset> {
        let url = format!(
            "{}/changeset/{}?include_discussion=true",
            self.base_url(),
            id.0
        );
        self.get_changeset(&url).await
    }

    async fn get_changeset(&self, url: &str) -> ApiResult<Changeset> {
        let osm: OSM = self.get_from_api(url).await?;
        if osm.changesets.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.changesets.len(),
            });
        }
        Ok(osm.changesets.into_iter().next().unwrap())
    }

    /// Download the full osmchange for a changeset
    pub async fn changeset_download(&self, id: ChangesetID) -> ApiResult<Change> {
        let url = format!("{}/changeset/{}/download", self.base_url(), id.0);
        self.get_from_api(&url).await
    }

    // ========== Note Methods ==========

    /// Get a note
    pub async fn note(&self, id: NoteID) -> ApiResult<Note> {
        let url = format!("{}/notes/{}", self.base_url(), id.0);
        let osm: OSM = self.get_from_api(&url).await?;
        if osm.notes.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.notes.len(),
            });
        }
        Ok(osm.notes.into_iter().next().unwrap())
    }

    /// Get notes in a bounding box
    pub async fn notes(
        &self,
        bounds: &Bounds,
        opts: Option<NotesOptions>,
    ) -> ApiResult<Vec<Note>> {
        let mut params = vec![format!(
            "bbox={},{},{},{}",
            bounds.min_lon, bounds.min_lat, bounds.max_lon, bounds.max_lat
        )];
        if let Some(o) = opts {
            params.extend(o.to_params());
        }
        let url = format!("{}/notes?{}", self.base_url(), params.join("&"));
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.notes)
    }

    /// Search notes by text
    pub async fn notes_search(
        &self,
        query: &str,
        opts: Option<NotesOptions>,
    ) -> ApiResult<Vec<Note>> {
        let mut params = vec![format!("q={}", urlencoding::encode(query))];
        if let Some(o) = opts {
            params.extend(o.to_params());
        }
        let url = format!("{}/notes/search?{}", self.base_url(), params.join("&"));
        let osm: OSM = self.get_from_api(&url).await?;
        Ok(osm.notes)
    }

    // ========== User Methods ==========

    /// Get a user
    pub async fn user(&self, id: UserID) -> ApiResult<User> {
        let url = format!("{}/user/{}", self.base_url(), id.0);
        let osm: OSM = self.get_from_api(&url).await?;
        if osm.users.len() != 1 {
            return Err(ApiError::WrongCount {
                expected: 1,
                got: osm.users.len(),
            });
        }
        Ok(osm.users.into_iter().next().unwrap())
    }

    // ========== Map Methods ==========

    /// Get all elements in a bounding box
    pub async fn map(&self, bounds: &Bounds, opts: Option<FeatureOptions>) -> ApiResult<OSM> {
        let params = opts.unwrap_or_default().to_params();
        let url = format!(
            "{}/map?bbox={},{},{},{}&{}",
            self.base_url(),
            bounds.min_lon,
            bounds.min_lat,
            bounds.max_lon,
            bounds.max_lat,
            params
        );
        self.get_from_api(&url).await
    }
}

/// Default datasource for package-level convenience functions
pub static DEFAULT_DATASOURCE: std::sync::LazyLock<Datasource> =
    std::sync::LazyLock::new(Datasource::new);

// ========== Package-level convenience functions ==========

/// Get a node using the default datasource
pub async fn node(id: NodeID, opts: Option<FeatureOptions>) -> ApiResult<Node> {
    DEFAULT_DATASOURCE.node(id, opts).await
}

/// Get multiple nodes using the default datasource
pub async fn nodes(ids: &[NodeID], opts: Option<FeatureOptions>) -> ApiResult<Vec<Node>> {
    DEFAULT_DATASOURCE.nodes(ids, opts).await
}

/// Get a specific version of a node using the default datasource
pub async fn node_version(id: NodeID, version: i32) -> ApiResult<Node> {
    DEFAULT_DATASOURCE.node_version(id, version).await
}

/// Get all versions of a node using the default datasource
pub async fn node_history(id: NodeID) -> ApiResult<Vec<Node>> {
    DEFAULT_DATASOURCE.node_history(id).await
}

/// Get a way using the default datasource
pub async fn way(id: WayID, opts: Option<FeatureOptions>) -> ApiResult<Way> {
    DEFAULT_DATASOURCE.way(id, opts).await
}

/// Get multiple ways using the default datasource
pub async fn ways(ids: &[WayID], opts: Option<FeatureOptions>) -> ApiResult<Vec<Way>> {
    DEFAULT_DATASOURCE.ways(ids, opts).await
}

/// Get a specific version of a way using the default datasource
pub async fn way_version(id: WayID, version: i32) -> ApiResult<Way> {
    DEFAULT_DATASOURCE.way_version(id, version).await
}

/// Get all versions of a way using the default datasource
pub async fn way_history(id: WayID) -> ApiResult<Vec<Way>> {
    DEFAULT_DATASOURCE.way_history(id).await
}

/// Get a relation using the default datasource
pub async fn relation(id: RelationID, opts: Option<FeatureOptions>) -> ApiResult<Relation> {
    DEFAULT_DATASOURCE.relation(id, opts).await
}

/// Get multiple relations using the default datasource
pub async fn relations(
    ids: &[RelationID],
    opts: Option<FeatureOptions>,
) -> ApiResult<Vec<Relation>> {
    DEFAULT_DATASOURCE.relations(ids, opts).await
}

/// Get a specific version of a relation using the default datasource
pub async fn relation_version(id: RelationID, version: i32) -> ApiResult<Relation> {
    DEFAULT_DATASOURCE.relation_version(id, version).await
}

/// Get all versions of a relation using the default datasource
pub async fn relation_history(id: RelationID) -> ApiResult<Vec<Relation>> {
    DEFAULT_DATASOURCE.relation_history(id).await
}

/// Get a changeset using the default datasource
pub async fn changeset(id: ChangesetID) -> ApiResult<Changeset> {
    DEFAULT_DATASOURCE.changeset(id).await
}

/// Download changeset using the default datasource
pub async fn changeset_download(id: ChangesetID) -> ApiResult<Change> {
    DEFAULT_DATASOURCE.changeset_download(id).await
}

/// Get a note using the default datasource
pub async fn note(id: NoteID) -> ApiResult<Note> {
    DEFAULT_DATASOURCE.note(id).await
}

/// Get a user using the default datasource
pub async fn user(id: UserID) -> ApiResult<User> {
    DEFAULT_DATASOURCE.user(id).await
}

/// Get all elements in a bounding box using the default datasource
pub async fn map(bounds: &Bounds, opts: Option<FeatureOptions>) -> ApiResult<OSM> {
    DEFAULT_DATASOURCE.map(bounds, opts).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_options() {
        let opts = FeatureOptions::new();
        assert!(opts.at.is_none());
        assert!(opts.to_params().is_empty());
    }

    #[test]
    fn test_notes_options() {
        let opts = NotesOptions::new();
        assert!(opts.limit.is_none());
        assert!(opts.max_days_closed.is_none());
    }

    #[test]
    fn test_notes_options_limit() {
        let opts = NotesOptions::new().limit(100).unwrap();
        assert_eq!(opts.limit, Some(100));

        // Invalid limits should fail
        assert!(NotesOptions::new().limit(0).is_err());
        assert!(NotesOptions::new().limit(10001).is_err());
    }

    #[test]
    fn test_datasource_default() {
        let ds = Datasource::new();
        assert_eq!(ds.base_url, BASE_URL);
    }

    #[test]
    fn test_datasource_custom_url() {
        let ds = Datasource::new().with_base_url("http://localhost:8080");
        assert_eq!(ds.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_not_found() {
        let err = ApiError::NotFound {
            url: "test".to_string(),
        };
        assert!(not_found(&err));

        let err = ApiError::Forbidden {
            url: "test".to_string(),
        };
        assert!(!not_found(&err));
    }
}
