//! OSM PBF (Protocol Buffer Binary Format) parser
//!
//! This module provides a parser for reading OSM PBF files, which are the most
//! efficient format for storing and transmitting OSM data.
//!
//! PBF files consist of a sequence of file blocks:
//! - Header block (OSMHeader) with metadata about the file
//! - Data blocks (OSMData) containing nodes, ways, and relations
//!
//! # Example
//!
//! ```rust,ignore
//! use std::fs::File;
//! use std::io::BufReader;
//! // Import Scanner from the crate's pbf module
//! use osm::pbf::Scanner; // or use crate::pbf::Scanner within this crate
//!
//! let file = File::open("planet.osm.pbf")?;
//! let reader = BufReader::new(file);
//! let mut scanner = Scanner::new(reader, 4);
//!
//! let header = scanner.header()?;
//! println!("Writing program: {:?}", header.writing_program);
//!
//! while scanner.scan() {
//!     let obj = scanner.object();
//!     // process object...
//! }
//!
//! if let Some(err) = scanner.err() {
//!     eprintln!("Error: {:?}", err);
//! }
//! ```

use std::io::{Read, BufReader};
use std::sync::atomic::{AtomicI64, Ordering};
use chrono::{DateTime, Utc, TimeZone};
use flate2::read::ZlibDecoder;
use prost::Message;

use crate::{
    Bounds, Node, NodeID, Way, WayID, Relation, RelationID,
    ChangesetID, UserID, Tags, Tag, WayNode, WayNodes, Members, Member, Type,
};

// Constants
const MAX_BLOB_HEADER_SIZE: u32 = 64 * 1024;
const MAX_BLOB_SIZE: u32 = 32 * 1024 * 1024;

const OSM_HEADER_TYPE: &str = "OSMHeader";
const OSM_DATA_TYPE: &str = "OSMData";

/// Parse capabilities supported by this parser
const PARSE_CAPABILITIES: &[&str] = &[
    "OsmSchema-V0.6",
    "DenseNodes",
    "HistoricalInformation",
];

/// PBF parsing error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("Blob header size >= 64KB")]
    BlobHeaderTooLarge,

    #[error("Blob size >= 32MB")]
    BlobTooLarge,

    #[error("Unknown blob data type")]
    UnknownBlobData,

    #[error("Parser does not have capability: {0}")]
    MissingCapability(String),

    #[error("Unexpected file block type: {0}")]
    UnexpectedBlockType(String),

    #[error("Dense node missing ids")]
    DenseNodeMissingIds,

    #[error("Dense node missing latitudes")]
    DenseNodeMissingLats,

    #[error("Dense node missing longitudes")]
    DenseNodeMissingLons,

    #[error("Scanner closed by user")]
    ScannerClosed,

    #[error("No header found")]
    NoHeader,
}

/// Result type for PBF operations
pub type Result<T> = std::result::Result<T, Error>;

/// Filter function type for nodes
pub type NodeFilter = Box<dyn Fn(&Node) -> bool + Send + Sync>;
/// Filter function type for ways
pub type WayFilter = Box<dyn Fn(&Way) -> bool + Send + Sync>;
/// Filter function type for relations
pub type RelationFilter = Box<dyn Fn(&Relation) -> bool + Send + Sync>;

/// Header contains the contents of the header in the PBF file
#[derive(Debug, Clone, Default)]
pub struct Header {
    /// Bounding box of the data
    pub bounds: Option<Bounds>,
    /// Features required to parse the file
    pub required_features: Vec<String>,
    /// Optional features used by the file
    pub optional_features: Vec<String>,
    /// Program that wrote the file
    pub writing_program: Option<String>,
    /// Source of the data
    pub source: Option<String>,
    /// Replication timestamp
    pub replication_timestamp: Option<DateTime<Utc>>,
    /// Replication sequence number
    pub replication_seq_num: u64,
    /// Replication base URL
    pub replication_base_url: Option<String>,
}

/// Scanner provides a convenient interface for reading PBF data
///
/// This is the main entry point for parsing PBF files. It provides
/// streaming access to all OSM objects in the file.
pub struct Scanner<R: Read> {
    /// Skip node elements
    pub skip_nodes: bool,
    /// Skip way elements
    pub skip_ways: bool,
    /// Skip relation elements
    pub skip_relations: bool,

    /// Filter function for nodes
    pub filter_node: Option<NodeFilter>,
    /// Filter function for ways
    pub filter_way: Option<WayFilter>,
    /// Filter function for relations
    pub filter_relation: Option<RelationFilter>,

    closed: bool,
    decoder: Decoder<R>,
    started: bool,
    procs: usize,
    next: Option<OsmObject>,
    err: Option<Error>,
}

/// OSM object enum for heterogeneous collections
#[derive(Debug, Clone)]
pub enum OsmObject {
    Node(Node),
    Way(Way),
    Relation(Relation),
}

impl<R: Read> Scanner<R> {
    /// Create a new scanner to read from the given reader
    ///
    /// `procs` indicates the number of parallel decoders (for future parallel implementation)
    pub fn new(reader: R, procs: usize) -> Self {
        Self {
            skip_nodes: false,
            skip_ways: false,
            skip_relations: false,
            filter_node: None,
            filter_way: None,
            filter_relation: None,
            closed: false,
            decoder: Decoder::new(reader),
            started: false,
            procs: procs.max(1),
            next: None,
            err: None,
        }
    }

    /// Returns the number of bytes that have been fully scanned
    pub fn fully_scanned_bytes(&self) -> i64 {
        self.decoder.current_offset.load(Ordering::SeqCst)
    }

    /// Returns the previous fully scanned bytes value
    pub fn previous_fully_scanned_bytes(&self) -> i64 {
        self.decoder.previous_offset.load(Ordering::SeqCst)
    }

    /// Close the scanner
    pub fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }

    /// Returns the PBF file header
    pub fn header(&mut self) -> Result<&Header> {
        if !self.started {
            self.started = true;
            if let Err(e) = self.decoder.start(self.procs, self.skip_nodes, self.skip_ways, self.skip_relations) {
                self.err = Some(e);
            }
        }

        if let Some(ref e) = self.err {
            return Err(match e {
                Error::Io(io) => Error::Io(std::io::Error::new(io.kind(), io.to_string())),
                Error::Decode(d) => Error::Decode(prost::DecodeError::new(d.to_string())),
                _ => Error::NoHeader,
            });
        }

        self.decoder.header.as_ref().ok_or(Error::NoHeader)
    }

    /// Scan advances the scanner to the next element
    ///
    /// Returns true if there's a new element available, false if the scan
    /// is complete or an error occurred.
    pub fn scan(&mut self) -> bool {
        if !self.started {
            self.started = true;
            if let Err(e) = self.decoder.start(self.procs, self.skip_nodes, self.skip_ways, self.skip_relations) {
                self.err = Some(e);
                return false;
            }
        }

        if self.err.is_some() || self.closed {
            return false;
        }

        match self.decoder.next(self.skip_nodes, self.skip_ways, self.skip_relations,
            &self.filter_node, &self.filter_way, &self.filter_relation) {
            Ok(Some(obj)) => {
                self.next = Some(obj);
                true
            }
            Ok(None) => false,
            Err(e) => {
                if matches!(e, Error::Io(ref io) if io.kind() == std::io::ErrorKind::UnexpectedEof) {
                    self.next = None;
                    return false;
                }
                self.err = Some(e);
                false
            }
        }
    }

    /// Returns the current object
    pub fn object(&self) -> Option<&OsmObject> {
        self.next.as_ref()
    }

    /// Returns the last error encountered
    pub fn err(&self) -> Option<&Error> {
        self.err.as_ref()
    }
}

/// Internal decoder for PBF files
struct Decoder<R: Read> {
    reader: BufReader<R>,
    header: Option<Header>,
    bytes_read: i64,
    current_offset: AtomicI64,
    previous_offset: AtomicI64,

    // Current decoding state
    string_table: Vec<String>,
    granularity: i32,
    lat_offset: i64,
    lon_offset: i64,
    date_granularity: i32,

    // Object buffer for current block
    objects: Vec<OsmObject>,
    object_index: usize,
}

impl<R: Read> Decoder<R> {
    fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            header: None,
            bytes_read: 0,
            current_offset: AtomicI64::new(0),
            previous_offset: AtomicI64::new(0),
            string_table: Vec::new(),
            granularity: 100,
            lat_offset: 0,
            lon_offset: 0,
            date_granularity: 1000,
            objects: Vec::new(),
            object_index: 0,
        }
    }

    fn start(&mut self, _procs: usize, skip_nodes: bool, skip_ways: bool, skip_relations: bool) -> Result<()> {
        // Read the first block, which should be the header
        let (blob_header, blob) = self.read_file_block()?;

        if blob_header.r#type == OSM_HEADER_TYPE {
            self.header = Some(decode_osm_header(&blob)?);
        } else {
            // First block is data, decode it
            let data = get_data(&blob)?;
            self.decode_primitive_block(&data, skip_nodes, skip_ways, skip_relations)?;
        }

        Ok(())
    }

    fn next(
        &mut self,
        skip_nodes: bool,
        skip_ways: bool,
        skip_relations: bool,
        filter_node: &Option<NodeFilter>,
        filter_way: &Option<WayFilter>,
        filter_relation: &Option<RelationFilter>,
    ) -> Result<Option<OsmObject>> {
        loop {
            // Return objects from current buffer
            while self.object_index < self.objects.len() {
                let obj = self.objects[self.object_index].clone();
                self.object_index += 1;

                // Apply filters
                let should_include = match &obj {
                    OsmObject::Node(n) => {
                        !skip_nodes && filter_node.as_ref().is_none_or(|f| f(n))
                    }
                    OsmObject::Way(w) => {
                        !skip_ways && filter_way.as_ref().is_none_or(|f| f(w))
                    }
                    OsmObject::Relation(r) => {
                        !skip_relations && filter_relation.as_ref().is_none_or(|f| f(r))
                    }
                };

                if should_include {
                    return Ok(Some(obj));
                }
            }

            // Need more data
            self.previous_offset.store(self.current_offset.load(Ordering::SeqCst), Ordering::SeqCst);
            self.current_offset.store(self.bytes_read, Ordering::SeqCst);

            let (blob_header, blob) = match self.read_file_block() {
                Ok(b) => b,
                Err(Error::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            };

            if blob_header.r#type != OSM_DATA_TYPE {
                return Err(Error::UnexpectedBlockType(blob_header.r#type));
            }

            let data = get_data(&blob)?;
            self.decode_primitive_block(&data, skip_nodes, skip_ways, skip_relations)?;
        }
    }

    fn read_file_block(&mut self) -> Result<(BlobHeader, Blob)> {
        // Read blob header size (4 bytes, big endian)
        let mut size_buf = [0u8; 4];
        self.reader.read_exact(&mut size_buf)?;
        let blob_header_size = u32::from_be_bytes(size_buf);

        if blob_header_size >= MAX_BLOB_HEADER_SIZE {
            return Err(Error::BlobHeaderTooLarge);
        }

        // Read blob header
        let mut header_buf = vec![0u8; blob_header_size as usize];
        self.reader.read_exact(&mut header_buf)?;
        let blob_header = BlobHeader::decode(&header_buf[..])?;

        if blob_header.datasize as u32 >= MAX_BLOB_SIZE {
            return Err(Error::BlobTooLarge);
        }

        // Read blob
        let mut blob_buf = vec![0u8; blob_header.datasize as usize];
        self.reader.read_exact(&mut blob_buf)?;
        let blob = Blob::decode(&blob_buf[..])?;

        self.bytes_read += 4 + blob_header_size as i64 + blob_header.datasize as i64;

        Ok((blob_header, blob))
    }

    fn decode_primitive_block(&mut self, data: &[u8], skip_nodes: bool, skip_ways: bool, skip_relations: bool) -> Result<()> {
        let pb = PrimitiveBlock::decode(data)?;

        // Reset state
        self.objects.clear();
        self.object_index = 0;

        // Read string table
        self.string_table = pb.stringtable
            .map(|st| st.s.into_iter().map(|b| String::from_utf8_lossy(&b).to_string()).collect())
            .unwrap_or_default();

        // Read granularities
        self.granularity = pb.granularity.unwrap_or(100);
        self.lat_offset = pb.lat_offset.unwrap_or(0);
        self.lon_offset = pb.lon_offset.unwrap_or(0);
        self.date_granularity = pb.date_granularity.unwrap_or(1000);

        // Decode primitive groups
        for group in pb.primitivegroup {
            self.decode_primitive_group(group, skip_nodes, skip_ways, skip_relations)?;
        }

        Ok(())
    }

    fn decode_primitive_group(&mut self, group: PrimitiveGroup, skip_nodes: bool, skip_ways: bool, skip_relations: bool) -> Result<()> {
        // Dense nodes
        if let Some(dense) = group.dense {
            if !skip_nodes {
                self.decode_dense_nodes(dense)?;
            }
        }

        // Regular nodes (rare, usually dense nodes are used)
        for node in group.nodes {
            if !skip_nodes {
                self.decode_node(node)?;
            }
        }

        // Ways
        for way in group.ways {
            if !skip_ways {
                self.decode_way(way)?;
            }
        }

        // Relations
        for relation in group.relations {
            if !skip_relations {
                self.decode_relation(relation)?;
            }
        }

        Ok(())
    }

    fn decode_dense_nodes(&mut self, dense: DenseNodes) -> Result<()> {
        let ids = dense.id;
        if ids.is_empty() {
            return Err(Error::DenseNodeMissingIds);
        }

        let lats = dense.lat;
        if lats.is_empty() {
            return Err(Error::DenseNodeMissingLats);
        }

        let lons = dense.lon;
        if lons.is_empty() {
            return Err(Error::DenseNodeMissingLons);
        }

        // Dense info (optional)
        let info = dense.denseinfo;
        let versions = info.as_ref().map(|i| &i.version);
        let timestamps = info.as_ref().map(|i| &i.timestamp);
        let changesets = info.as_ref().map(|i| &i.changeset);
        let uids = info.as_ref().map(|i| &i.uid);
        let usids = info.as_ref().map(|i| &i.user_sid);
        let visibles = info.as_ref().map(|i| &i.visible);

        let keys_vals = dense.keys_vals;

        let granularity = self.granularity as i64;
        let date_granularity = self.date_granularity as i64;
        let lat_offset = self.lat_offset;
        let lon_offset = self.lon_offset;

        // Delta decoding state
        let mut id: i64 = 0;
        let mut lat: i64 = 0;
        let mut lon: i64 = 0;
        let mut timestamp: i64 = 0;
        let mut changeset: i64 = 0;
        let mut uid: i32 = 0;
        let mut usid: i32 = 0;
        let mut kv_index = 0;

        for i in 0..ids.len() {
            id += ids[i];
            lat += lats[i];
            lon += lons[i];

            let mut node = Node {
                id: NodeID(id),
                lat: 1e-9 * (lat_offset + granularity * lat) as f64,
                lon: 1e-9 * (lon_offset + granularity * lon) as f64,
                visible: true,
                ..Default::default()
            };

            // Version
            if let Some(vers) = versions {
                if i < vers.len() {
                    node.version = vers[i];
                }
            }

            // Timestamp
            if let Some(times) = timestamps {
                if i < times.len() {
                    timestamp += times[i];
                    let millisec = timestamp * date_granularity;
                    if let Some(ts) = Utc.timestamp_millis_opt(millisec).single() {
                        node.timestamp = ts;
                    }
                }
            }

            // Changeset
            if let Some(cs) = changesets {
                if i < cs.len() {
                    changeset += cs[i];
                    node.changeset = ChangesetID(changeset);
                }
            }

            // User ID
            if let Some(u) = uids {
                if i < u.len() {
                    uid += u[i];
                    node.uid = UserID(uid as i64);
                }
            }

            // User string ID
            if let Some(us) = usids {
                if i < us.len() {
                    usid += us[i];
                    if (usid as usize) < self.string_table.len() {
                        node.user = self.string_table[usid as usize].clone();
                    }
                }
            }

            // Visible
            if let Some(vis) = visibles {
                if i < vis.len() {
                    node.visible = vis[i];
                }
            }

            // Tags (key-value pairs delimited by 0)
            let mut tags = Vec::new();
            while kv_index < keys_vals.len() {
                let k = keys_vals[kv_index];
                if k == 0 {
                    kv_index += 1;
                    break;
                }
                kv_index += 1;
                if kv_index < keys_vals.len() {
                    let v = keys_vals[kv_index] as usize;
                    kv_index += 1;
                    let key = self.string_table.get(k as usize).cloned().unwrap_or_default();
                    let value = self.string_table.get(v).cloned().unwrap_or_default();
                    tags.push(Tag::new(&key, &value));
                }
            }
            node.tags = Tags::from_vec(tags);

            self.objects.push(OsmObject::Node(node));
        }

        Ok(())
    }

    fn decode_node(&mut self, pbf_node: PbfNode) -> Result<()> {
        let granularity = self.granularity as i64;
        let lat_offset = self.lat_offset;
        let lon_offset = self.lon_offset;
        let date_granularity = self.date_granularity as i64;

        let mut node = Node {
            id: NodeID(pbf_node.id),
            lat: 1e-9 * (lat_offset + granularity * pbf_node.lat) as f64,
            lon: 1e-9 * (lon_offset + granularity * pbf_node.lon) as f64,
            visible: true,
            ..Default::default()
        };

        // Info
        if let Some(info) = pbf_node.info {
            node.version = info.version.unwrap_or(-1);
            if let Some(ts) = info.timestamp {
                let millisec = ts * date_granularity;
                if let Some(t) = Utc.timestamp_millis_opt(millisec).single() {
                    node.timestamp = t;
                }
            }
            if let Some(cs) = info.changeset {
                node.changeset = ChangesetID(cs);
            }
            if let Some(u) = info.uid {
                node.uid = UserID(u as i64);
            }
            if let Some(usid) = info.user_sid {
                if (usid as usize) < self.string_table.len() {
                    node.user = self.string_table[usid as usize].clone();
                }
            }
            if let Some(v) = info.visible {
                node.visible = v;
            }
        }

        // Tags
        let tags: Vec<Tag> = pbf_node.keys.iter().zip(pbf_node.vals.iter())
            .map(|(k, v)| {
                let key = self.string_table.get(*k as usize).cloned().unwrap_or_default();
                let value = self.string_table.get(*v as usize).cloned().unwrap_or_default();
                Tag::new(&key, &value)
            })
            .collect();
        node.tags = Tags::from_vec(tags);

        self.objects.push(OsmObject::Node(node));
        Ok(())
    }

    fn decode_way(&mut self, pbf_way: PbfWay) -> Result<()> {
        let granularity = self.granularity as i64;
        let lat_offset = self.lat_offset;
        let lon_offset = self.lon_offset;
        let date_granularity = self.date_granularity as i64;

        let mut way = Way {
            id: WayID(pbf_way.id),
            visible: true,
            ..Default::default()
        };

        // Info
        if let Some(info) = pbf_way.info {
            way.version = info.version.unwrap_or(-1);
            if let Some(ts) = info.timestamp {
                let millisec = ts * date_granularity;
                if let Some(t) = Utc.timestamp_millis_opt(millisec).single() {
                    way.timestamp = t;
                }
            }
            if let Some(cs) = info.changeset {
                way.changeset = ChangesetID(cs);
            }
            if let Some(u) = info.uid {
                way.uid = UserID(u as i64);
            }
            if let Some(usid) = info.user_sid {
                if (usid as usize) < self.string_table.len() {
                    way.user = self.string_table[usid as usize].clone();
                }
            }
            if let Some(v) = info.visible {
                way.visible = v;
            }
        }

        // Tags
        let tags: Vec<Tag> = pbf_way.keys.iter().zip(pbf_way.vals.iter())
            .map(|(k, v)| {
                let key = self.string_table.get(*k as usize).cloned().unwrap_or_default();
                let value = self.string_table.get(*v as usize).cloned().unwrap_or_default();
                Tag::new(&key, &value)
            })
            .collect();
        way.tags = Tags::from_vec(tags);

        // Node refs (delta encoded)
        let mut nodes = Vec::with_capacity(pbf_way.refs.len());
        let mut ref_id: i64 = 0;
        let mut lat: i64 = 0;
        let mut lon: i64 = 0;

        for i in 0..pbf_way.refs.len() {
            ref_id += pbf_way.refs[i];

            let mut wn = WayNode {
                id: NodeID(ref_id),
                ..Default::default()
            };

            // Optional lat/lon (LocationsOnWays feature)
            if i < pbf_way.lat.len() {
                lat += pbf_way.lat[i];
                wn.lat = 1e-9 * (lat_offset + granularity * lat) as f64;
            }
            if i < pbf_way.lon.len() {
                lon += pbf_way.lon[i];
                wn.lon = 1e-9 * (lon_offset + granularity * lon) as f64;
            }

            nodes.push(wn);
        }
        way.nodes = WayNodes(nodes);

        self.objects.push(OsmObject::Way(way));
        Ok(())
    }

    fn decode_relation(&mut self, pbf_rel: PbfRelation) -> Result<()> {
        let date_granularity = self.date_granularity as i64;

        let mut relation = Relation {
            id: RelationID(pbf_rel.id),
            visible: true,
            ..Default::default()
        };

        // Info
        if let Some(info) = pbf_rel.info {
            relation.version = info.version.unwrap_or(-1);
            if let Some(ts) = info.timestamp {
                let millisec = ts * date_granularity;
                if let Some(t) = Utc.timestamp_millis_opt(millisec).single() {
                    relation.timestamp = t;
                }
            }
            if let Some(cs) = info.changeset {
                relation.changeset = ChangesetID(cs);
            }
            if let Some(u) = info.uid {
                relation.uid = UserID(u as i64);
            }
            if let Some(usid) = info.user_sid {
                if (usid as usize) < self.string_table.len() {
                    relation.user = self.string_table[usid as usize].clone();
                }
            }
            if let Some(v) = info.visible {
                relation.visible = v;
            }
        }

        // Tags
        let tags: Vec<Tag> = pbf_rel.keys.iter().zip(pbf_rel.vals.iter())
            .map(|(k, v)| {
                let key = self.string_table.get(*k as usize).cloned().unwrap_or_default();
                let value = self.string_table.get(*v as usize).cloned().unwrap_or_default();
                Tag::new(&key, &value)
            })
            .collect();
        relation.tags = Tags::from_vec(tags);

        // Members (delta encoded member IDs)
        let mut members = Vec::with_capacity(pbf_rel.types.len());
        let mut mem_id: i64 = 0;

        for i in 0..pbf_rel.types.len() {
            if i < pbf_rel.memids.len() {
                mem_id += pbf_rel.memids[i];
            }

            let role = if i < pbf_rel.roles_sid.len() {
                self.string_table.get(pbf_rel.roles_sid[i] as usize)
                    .cloned()
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let member_type = match pbf_rel.types[i] {
                0 => Type::Node,
                1 => Type::Way,
                2 => Type::Relation,
                _ => Type::Node,
            };

            members.push(Member {
                member_type,
                ref_: mem_id,
                role,
                ..Default::default()
            });
        }
        relation.members = Members(members);

        self.objects.push(OsmObject::Relation(relation));
        Ok(())
    }
}

// Helper function to decompress blob data
fn get_data(blob: &Blob) -> Result<Vec<u8>> {
    if let Some(ref raw) = blob.raw {
        return Ok(raw.clone());
    }

    if let Some(ref zlib_data) = blob.zlib_data {
        let mut decoder = ZlibDecoder::new(&zlib_data[..]);
        let mut decompressed = Vec::with_capacity(blob.raw_size.unwrap_or(0) as usize);
        decoder.read_to_end(&mut decompressed)?;
        return Ok(decompressed);
    }

    Err(Error::UnknownBlobData)
}

// Helper function to decode OSM header
fn decode_osm_header(blob: &Blob) -> Result<Header> {
    let data = get_data(blob)?;
    let header_block = HeaderBlock::decode(&data[..])?;

    // Check capabilities
    for feature in &header_block.required_features {
        if !PARSE_CAPABILITIES.contains(&feature.as_str()) {
            return Err(Error::MissingCapability(feature.clone()));
        }
    }

    let mut header = Header {
        required_features: header_block.required_features,
        optional_features: header_block.optional_features,
        writing_program: header_block.writingprogram,
        source: header_block.source,
        replication_base_url: header_block.osmosis_replication_base_url,
        replication_seq_num: header_block.osmosis_replication_sequence_number.unwrap_or(0) as u64,
        ..Default::default()
    };

    // Timestamp
    if let Some(ts) = header_block.osmosis_replication_timestamp {
        header.replication_timestamp = Utc.timestamp_opt(ts, 0).single();
    }

    // Bounding box
    if let Some(bbox) = header_block.bbox {
        header.bounds = Some(Bounds {
            min_lon: 1e-9 * bbox.left as f64,
            max_lon: 1e-9 * bbox.right as f64,
            min_lat: 1e-9 * bbox.bottom as f64,
            max_lat: 1e-9 * bbox.top as f64,
        });
    }

    Ok(header)
}

// Protocol buffer message definitions
// These mirror the .proto definitions from the Go implementation

#[derive(Clone, PartialEq, Message)]
pub struct Blob {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub raw: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "2")]
    pub raw_size: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub zlib_data: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub lzma_data: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct BlobHeader {
    #[prost(string, tag = "1")]
    pub r#type: String,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub indexdata: Option<Vec<u8>>,
    #[prost(int32, tag = "3")]
    pub datasize: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct HeaderBlock {
    #[prost(message, optional, tag = "1")]
    pub bbox: Option<HeaderBBox>,
    #[prost(string, repeated, tag = "4")]
    pub required_features: Vec<String>,
    #[prost(string, repeated, tag = "5")]
    pub optional_features: Vec<String>,
    #[prost(string, optional, tag = "16")]
    pub writingprogram: Option<String>,
    #[prost(string, optional, tag = "17")]
    pub source: Option<String>,
    #[prost(int64, optional, tag = "32")]
    pub osmosis_replication_timestamp: Option<i64>,
    #[prost(int64, optional, tag = "33")]
    pub osmosis_replication_sequence_number: Option<i64>,
    #[prost(string, optional, tag = "34")]
    pub osmosis_replication_base_url: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct HeaderBBox {
    #[prost(sint64, tag = "1")]
    pub left: i64,
    #[prost(sint64, tag = "2")]
    pub right: i64,
    #[prost(sint64, tag = "3")]
    pub top: i64,
    #[prost(sint64, tag = "4")]
    pub bottom: i64,
}

#[derive(Clone, PartialEq, Message)]
pub struct PrimitiveBlock {
    #[prost(message, optional, tag = "1")]
    pub stringtable: Option<StringTable>,
    #[prost(message, repeated, tag = "2")]
    pub primitivegroup: Vec<PrimitiveGroup>,
    #[prost(int32, optional, tag = "17")]
    pub granularity: Option<i32>,
    #[prost(int64, optional, tag = "19")]
    pub lat_offset: Option<i64>,
    #[prost(int64, optional, tag = "20")]
    pub lon_offset: Option<i64>,
    #[prost(int32, optional, tag = "18")]
    pub date_granularity: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StringTable {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub s: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PrimitiveGroup {
    #[prost(message, repeated, tag = "1")]
    pub nodes: Vec<PbfNode>,
    #[prost(message, optional, tag = "2")]
    pub dense: Option<DenseNodes>,
    #[prost(message, repeated, tag = "3")]
    pub ways: Vec<PbfWay>,
    #[prost(message, repeated, tag = "4")]
    pub relations: Vec<PbfRelation>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbfNode {
    #[prost(sint64, tag = "1")]
    pub id: i64,
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    pub keys: Vec<u32>,
    #[prost(uint32, repeated, packed = "true", tag = "3")]
    pub vals: Vec<u32>,
    #[prost(message, optional, tag = "4")]
    pub info: Option<Info>,
    #[prost(sint64, tag = "8")]
    pub lat: i64,
    #[prost(sint64, tag = "9")]
    pub lon: i64,
}

#[derive(Clone, PartialEq, Message)]
pub struct DenseNodes {
    #[prost(sint64, repeated, packed = "true", tag = "1")]
    pub id: Vec<i64>,
    #[prost(message, optional, tag = "5")]
    pub denseinfo: Option<DenseInfo>,
    #[prost(sint64, repeated, packed = "true", tag = "8")]
    pub lat: Vec<i64>,
    #[prost(sint64, repeated, packed = "true", tag = "9")]
    pub lon: Vec<i64>,
    #[prost(int32, repeated, packed = "true", tag = "10")]
    pub keys_vals: Vec<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct DenseInfo {
    #[prost(int32, repeated, packed = "true", tag = "1")]
    pub version: Vec<i32>,
    #[prost(sint64, repeated, packed = "true", tag = "2")]
    pub timestamp: Vec<i64>,
    #[prost(sint64, repeated, packed = "true", tag = "3")]
    pub changeset: Vec<i64>,
    #[prost(sint32, repeated, packed = "true", tag = "4")]
    pub uid: Vec<i32>,
    #[prost(sint32, repeated, packed = "true", tag = "5")]
    pub user_sid: Vec<i32>,
    #[prost(bool, repeated, packed = "true", tag = "6")]
    pub visible: Vec<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Info {
    #[prost(int32, optional, tag = "1")]
    pub version: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    pub timestamp: Option<i64>,
    #[prost(int64, optional, tag = "3")]
    pub changeset: Option<i64>,
    #[prost(int32, optional, tag = "4")]
    pub uid: Option<i32>,
    #[prost(uint32, optional, tag = "5")]
    pub user_sid: Option<u32>,
    #[prost(bool, optional, tag = "6")]
    pub visible: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbfWay {
    #[prost(int64, tag = "1")]
    pub id: i64,
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    pub keys: Vec<u32>,
    #[prost(uint32, repeated, packed = "true", tag = "3")]
    pub vals: Vec<u32>,
    #[prost(message, optional, tag = "4")]
    pub info: Option<Info>,
    #[prost(sint64, repeated, packed = "true", tag = "8")]
    pub refs: Vec<i64>,
    #[prost(sint64, repeated, packed = "true", tag = "9")]
    pub lat: Vec<i64>,
    #[prost(sint64, repeated, packed = "true", tag = "10")]
    pub lon: Vec<i64>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbfRelation {
    #[prost(int64, tag = "1")]
    pub id: i64,
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    pub keys: Vec<u32>,
    #[prost(uint32, repeated, packed = "true", tag = "3")]
    pub vals: Vec<u32>,
    #[prost(message, optional, tag = "4")]
    pub info: Option<Info>,
    #[prost(int32, repeated, packed = "true", tag = "8")]
    pub roles_sid: Vec<i32>,
    #[prost(sint64, repeated, packed = "true", tag = "9")]
    pub memids: Vec<i64>,
    #[prost(int32, repeated, packed = "true", tag = "10")]
    pub types: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_capabilities() {
        assert!(PARSE_CAPABILITIES.contains(&"OsmSchema-V0.6"));
        assert!(PARSE_CAPABILITIES.contains(&"DenseNodes"));
        assert!(PARSE_CAPABILITIES.contains(&"HistoricalInformation"));
    }

    #[test]
    fn test_error_types() {
        let err = Error::BlobHeaderTooLarge;
        assert!(err.to_string().contains("64KB"));

        let err = Error::BlobTooLarge;
        assert!(err.to_string().contains("32MB"));
    }

    #[test]
    fn test_blob_decode() {
        // Test raw blob
        let blob = Blob {
            raw: Some(vec![1, 2, 3, 4]),
            raw_size: None,
            zlib_data: None,
            lzma_data: None,
        };
        let data = get_data(&blob).unwrap();
        assert_eq!(data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_zlib_blob_decode() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let compressed = encoder.finish().unwrap();

        let blob = Blob {
            raw: None,
            raw_size: Some(original.len() as i32),
            zlib_data: Some(compressed),
            lzma_data: None,
        };
        let data = get_data(&blob).unwrap();
        assert_eq!(data, original);
    }

    #[test]
    fn test_header_default() {
        let header = Header::default();
        assert!(header.bounds.is_none());
        assert!(header.required_features.is_empty());
        assert!(header.optional_features.is_empty());
        assert!(header.writing_program.is_none());
        assert_eq!(header.replication_seq_num, 0);
    }

    #[test]
    fn test_osm_object_enum() {
        let node = Node {
            id: NodeID(1),
            lat: 0.0,
            lon: 0.0,
            ..Default::default()
        };
        let obj = OsmObject::Node(node);

        match obj {
            OsmObject::Node(n) => assert_eq!(n.id, NodeID(1)),
            _ => panic!("Expected Node"),
        }
    }

    #[test]
    fn test_scanner_close() {
        let data: &[u8] = &[];
        let mut scanner = Scanner::new(data, 1);
        assert!(!scanner.closed);
        scanner.close().unwrap();
        assert!(scanner.closed);
    }

    #[test]
    fn test_blob_header_struct() {
        let header = BlobHeader {
            r#type: "OSMHeader".to_string(),
            indexdata: None,
            datasize: 1000,
        };
        assert_eq!(header.r#type, "OSMHeader");
        assert_eq!(header.datasize, 1000);
    }

    #[test]
    fn test_header_bbox_struct() {
        let bbox = HeaderBBox {
            left: -180_000_000_000,
            right: 180_000_000_000,
            top: 90_000_000_000,
            bottom: -90_000_000_000,
        };
        assert_eq!(bbox.left, -180_000_000_000);
        assert_eq!(bbox.right, 180_000_000_000);
    }

    #[test]
    fn test_dense_info_struct() {
        let info = DenseInfo {
            version: vec![1, 2, 3],
            timestamp: vec![100, 200, 300],
            changeset: vec![1, 2, 3],
            uid: vec![10, 20, 30],
            user_sid: vec![0, 1, 2],
            visible: vec![true, true, false],
        };
        assert_eq!(info.version.len(), 3);
        assert!(!info.visible[2]);
    }

    #[test]
    fn test_primitive_block_defaults() {
        let pb = PrimitiveBlock {
            stringtable: None,
            primitivegroup: vec![],
            granularity: None,
            lat_offset: None,
            lon_offset: None,
            date_granularity: None,
        };
        assert!(pb.stringtable.is_none());
        assert!(pb.primitivegroup.is_empty());
    }

    #[test]
    fn test_string_table() {
        let st = StringTable {
            s: vec![
                b"".to_vec(),
                b"highway".to_vec(),
                b"residential".to_vec(),
            ],
        };
        assert_eq!(st.s.len(), 3);
        assert_eq!(String::from_utf8_lossy(&st.s[1]), "highway");
    }
}
