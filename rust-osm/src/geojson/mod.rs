//! OSM to GeoJSON conversion
//!
//! This module provides functionality to convert OSM data to GeoJSON format.

use std::collections::{HashMap, HashSet};

use geo_types::{Coord, LineString as GeoLineString};
use geojson::{Feature, FeatureCollection, Geometry, JsonObject, JsonValue, Value};

use crate::container::OSM;
use crate::mputil::{join, Orientation, Segment};
use crate::types::{FeatureID, Node, NodeID, Relation, RelationID, Tags, Type, Way, WayID};
use crate::Element;

/// Options for GeoJSON conversion
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// Omit setting the geojson feature.ID
    pub no_id: bool,
    /// Omit the meta (timestamp, user, changeset, etc) info
    pub no_meta: bool,
    /// Omit the list of relations an element is a member of
    pub no_relation_membership: bool,
    /// Include invalid polygons with nil outer ring
    pub include_invalid_polygons: bool,
}

impl ConvertOptions {
    /// Creates new default options
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets no_id option
    pub fn with_no_id(mut self, value: bool) -> Self {
        self.no_id = value;
        self
    }

    /// Sets no_meta option
    pub fn with_no_meta(mut self, value: bool) -> Self {
        self.no_meta = value;
        self
    }

    /// Sets no_relation_membership option
    pub fn with_no_relation_membership(mut self, value: bool) -> Self {
        self.no_relation_membership = value;
        self
    }

    /// Sets include_invalid_polygons option
    pub fn with_include_invalid_polygons(mut self, value: bool) -> Self {
        self.include_invalid_polygons = value;
        self
    }
}

/// Relation summary for embedding in features
#[derive(Debug, Clone)]
struct RelationSummary {
    id: RelationID,
    role: String,
    tags: HashMap<String, String>,
}

/// Context for conversion
struct ConvertContext<'a> {
    options: &'a ConvertOptions,
    osm: &'a OSM,
    skippable: HashSet<WayID>,
    relation_member: HashMap<FeatureID, Vec<RelationSummary>>,
    way_member: HashSet<NodeID>,
    node_map: Option<HashMap<NodeID, &'a Node>>,
    way_map: HashMap<WayID, &'a Way>,
}

impl<'a> ConvertContext<'a> {
    fn new(osm: &'a OSM, options: &'a ConvertOptions) -> Self {
        let mut ctx = ConvertContext {
            options,
            osm,
            skippable: HashSet::new(),
            relation_member: HashMap::new(),
            way_member: HashSet::new(),
            node_map: None,
            way_map: HashMap::with_capacity(osm.ways.len()),
        };

        // Build way map
        for w in &osm.ways {
            ctx.way_map.insert(w.id, w);
        }

        // Build way member set (nodes that are part of ways)
        for w in &osm.ways {
            for wn in &w.nodes.0 {
                ctx.way_member.insert(wn.id);
            }
        }

        // Build relation membership map
        for relation in &osm.relations {
            let mut tags: Option<HashMap<String, String>> = None;

            for m in &relation.members.0 {
                if options.no_relation_membership && m.member_type != Type::Node {
                    continue;
                }

                if m.member_type == Type::Way
                    && !ctx.way_map.contains_key(&WayID(m.ref_))
                {
                    continue;
                }

                let tags_map = tags.get_or_insert_with(|| relation.tags.map());

                let fid = m.feature_id();
                ctx.relation_member
                    .entry(fid)
                    .or_default()
                    .push(RelationSummary {
                        id: relation.id,
                        role: m.role.clone(),
                        tags: tags_map.clone(),
                    });
            }
        }

        ctx
    }

    fn get_node(&mut self, id: NodeID) -> Option<&'a Node> {
        if self.node_map.is_none() {
            let mut map = HashMap::with_capacity(self.osm.nodes.len());
            for n in &self.osm.nodes {
                map.insert(n.id, n);
            }
            self.node_map = Some(map);
        }

        self.node_map.as_ref().and_then(|m| m.get(&id).copied())
    }
}

/// Converts OSM data to a GeoJSON FeatureCollection
pub fn convert(osm: &OSM, options: &ConvertOptions) -> FeatureCollection {
    let mut ctx = ConvertContext::new(osm, options);
    let mut features = Vec::with_capacity(osm.relations.len() + osm.ways.len());

    // Process relations
    for relation in &osm.relations {
        let tt = relation.tags.find("type");
        if tt == "route" {
            if let Some(feature) = build_route_line_string(&mut ctx, relation) {
                features.push(feature);
            }
        } else if tt == "multipolygon" || tt == "boundary" {
            if let Some(feature) = build_polygon(&mut ctx, relation) {
                features.push(feature);
            }
        }
    }

    // Process ways
    for way in &osm.ways {
        if ctx.skippable.contains(&way.id) {
            continue;
        }

        if let Some(feature) = way_to_feature(&mut ctx, way) {
            features.push(feature);
        }
    }

    // Process nodes
    for node in &osm.nodes {
        // Skip if: member of a way, not a member of a relation, no interesting tags
        if ctx.way_member.contains(&node.id)
            && !ctx.relation_member.contains_key(&node.id.feature_id())
            && !has_interesting_tags(&node.tags, None)
        {
            continue;
        }

        if let Some(feature) = node_to_feature(&ctx, node) {
            features.push(feature);
        }
    }

    FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    }
}

fn node_to_feature(ctx: &ConvertContext, node: &Node) -> Option<Feature> {
    // Empty node check
    if node.lon == 0.0 && node.lat == 0.0 && node.version == 0 {
        return None;
    }

    let geometry = Geometry::new(Value::Point(vec![node.lon, node.lat]));

    let mut properties = JsonObject::new();
    properties.insert("id".to_string(), JsonValue::from(node.id.0));
    properties.insert("type".to_string(), JsonValue::from("node"));
    properties.insert("tags".to_string(), tags_to_json(&node.tags));

    add_meta_properties(ctx, &mut properties, node);

    let id = if ctx.options.no_id {
        None
    } else {
        Some(geojson::feature::Id::String(format!("node/{}", node.id.0)))
    };

    Some(Feature {
        bbox: None,
        geometry: Some(geometry),
        id,
        properties: Some(properties),
        foreign_members: None,
    })
}

fn way_to_line_string(ctx: &mut ConvertContext, way: &Way) -> (Vec<Vec<f64>>, bool) {
    let mut coords = Vec::with_capacity(way.nodes.0.len());
    let mut tainted = false;

    for wn in &way.nodes.0 {
        if wn.lon != 0.0 || wn.lat != 0.0 {
            coords.push(vec![wn.lon, wn.lat]);
        } else if let Some(n) = ctx.get_node(wn.id) {
            coords.push(vec![n.lon, n.lat]);
        } else {
            tainted = true;
        }
    }

    (coords, tainted)
}

fn way_to_feature(ctx: &mut ConvertContext, way: &Way) -> Option<Feature> {
    let (coords, tainted) = way_to_line_string(ctx, way);

    if coords.len() <= 1 {
        return None;
    }

    let geometry = if crate::polygon::way_is_polygon(way) && coords.len() >= 4 {
        // Closed polygon
        let mut ring = coords;
        // Ensure closed
        if ring.first() != ring.last() {
            if let Some(first) = ring.first().cloned() {
                ring.push(first);
            }
        }
        Geometry::new(Value::Polygon(vec![ring]))
    } else {
        Geometry::new(Value::LineString(coords))
    };

    let mut properties = JsonObject::new();
    properties.insert("id".to_string(), JsonValue::from(way.id.0));
    properties.insert("type".to_string(), JsonValue::from("way"));
    properties.insert("tags".to_string(), tags_to_json(&way.tags));

    if tainted {
        properties.insert("tainted".to_string(), JsonValue::from(true));
    }

    add_meta_properties(ctx, &mut properties, way);

    let id = if ctx.options.no_id {
        None
    } else {
        Some(geojson::feature::Id::String(format!("way/{}", way.id.0)))
    };

    Some(Feature {
        bbox: None,
        geometry: Some(geometry),
        id,
        properties: Some(properties),
        foreign_members: None,
    })
}

fn build_route_line_string(ctx: &mut ConvertContext, relation: &Relation) -> Option<Feature> {
    let mut lines: Vec<Segment> = Vec::new();
    let mut tainted = false;

    for m in &relation.members.0 {
        if m.member_type != Type::Way {
            continue;
        }

        let way = match ctx.way_map.get(&WayID(m.ref_)) {
            Some(w) => *w,
            None => {
                tainted = true;
                continue;
            }
        };

        if !has_interesting_tags(&way.tags, None) {
            ctx.skippable.insert(way.id);
        }

        let (coords, t) = way_to_line_string(ctx, way);
        if t {
            tainted = true;
        }

        if coords.is_empty() {
            continue;
        }

        let line_coords: Vec<Coord<f64>> = coords
            .iter()
            .map(|c| Coord { x: c[0], y: c[1] })
            .collect();
        let line = GeoLineString::new(line_coords);

        let orientation = match m.orientation {
            1 => Orientation::CCW,
            -1 => Orientation::CW,
            _ => Orientation::None,
        };

        lines.push(Segment::with_orientation(line, orientation));
    }

    if lines.is_empty() {
        return None;
    }

    let line_sections = join(lines);

    let geometry = if line_sections.len() == 1 {
        let ls = line_sections[0].line_string();
        let coords: Vec<Vec<f64>> = ls.0.iter().map(|c| vec![c.x, c.y]).collect();
        Geometry::new(Value::LineString(coords))
    } else {
        let mls: Vec<Vec<Vec<f64>>> = line_sections
            .iter()
            .map(|ms| {
                let ls = ms.line_string();
                ls.0.iter().map(|c| vec![c.x, c.y]).collect()
            })
            .collect();
        Geometry::new(Value::MultiLineString(mls))
    };

    let mut properties = JsonObject::new();
    properties.insert("id".to_string(), JsonValue::from(relation.id.0));
    properties.insert("type".to_string(), JsonValue::from("relation"));
    properties.insert("tags".to_string(), tags_to_json(&relation.tags));

    if tainted {
        properties.insert("tainted".to_string(), JsonValue::from(true));
    }

    add_meta_properties(ctx, &mut properties, relation);

    let id = if ctx.options.no_id {
        None
    } else {
        Some(geojson::feature::Id::String(format!(
            "relation/{}",
            relation.id.0
        )))
    };

    Some(Feature {
        bbox: None,
        geometry: Some(geometry),
        id,
        properties: Some(properties),
        foreign_members: None,
    })
}

fn build_polygon(ctx: &mut ConvertContext, relation: &Relation) -> Option<Feature> {
    let tags = relation.tags.map();

    let mut outer: Vec<Segment> = Vec::new();
    let mut inner: Vec<Segment> = Vec::new();
    let mut tainted = false;
    let mut outer_count = 0;

    for m in &relation.members.0 {
        if m.member_type != Type::Way {
            continue;
        }

        if m.role != "inner" && m.role != "outer" {
            continue;
        }

        if m.role == "outer" {
            outer_count += 1;
        }

        let way = match ctx.way_map.get(&WayID(m.ref_)) {
            Some(w) => *w,
            None => {
                tainted = true;
                continue;
            }
        };

        if m.role == "outer" {
            if !has_interesting_tags(&way.tags, Some(&tags)) {
                ctx.skippable.insert(way.id);
            }
        } else if !has_interesting_tags(&way.tags, None) {
            ctx.skippable.insert(way.id);
        }

        let (coords, t) = way_to_line_string(ctx, way);
        if t {
            tainted = true;
        }

        if coords.is_empty() {
            continue;
        }

        let line_coords: Vec<Coord<f64>> = coords
            .iter()
            .map(|c| Coord { x: c[0], y: c[1] })
            .collect();
        let line = GeoLineString::new(line_coords);

        let orientation = match m.orientation {
            1 => Orientation::CCW,
            -1 => Orientation::CW,
            _ => Orientation::None,
        };

        let mut segment = Segment::with_orientation(line, orientation);

        if m.role == "outer" {
            if segment.orientation == Orientation::CW {
                segment.reverse();
            }
            outer.push(segment);
        } else {
            if segment.orientation == Orientation::CCW {
                segment.reverse();
            }
            inner.push(segment);
        }
    }

    // Build the polygon geometry
    if outer.is_empty() && !ctx.options.include_invalid_polygons {
        return None;
    }

    let geometry = if outer.len() == 1 && outer_count == 1 {
        // Single outer ring case
        let outer_sections = join(outer);
        if outer_sections.is_empty() {
            return None;
        }

        let outer_ring = outer_sections[0].ring(Orientation::CCW);
        if outer_ring.len() < 4 {
            return None;
        }

        let mut polygon = vec![outer_ring.iter().map(|c| vec![c.x, c.y]).collect()];

        let inner_sections = join(inner);
        for is in inner_sections {
            let ring = is.ring(Orientation::CW);
            polygon.push(ring.iter().map(|c| vec![c.x, c.y]).collect());
        }

        Geometry::new(Value::Polygon(polygon))
    } else {
        // Multiple outer rings - multipolygon
        let outer_sections = join(outer);
        let mut polygons: Vec<Vec<Vec<Vec<f64>>>> = Vec::new();

        for os in outer_sections {
            let ring = os.ring(Orientation::CCW);
            if !ctx.options.include_invalid_polygons && ring.len() < 4 {
                continue;
            }

            let ring_coords: Vec<Vec<f64>> = ring.iter().map(|c| vec![c.x, c.y]).collect();
            polygons.push(vec![ring_coords]);
        }

        if polygons.is_empty() && !ctx.options.include_invalid_polygons {
            return None;
        }

        // Add inner rings to appropriate outer polygons
        let inner_sections = join(inner);
        for is in inner_sections {
            let ring = is.ring(Orientation::CW);
            let ring_coords: Vec<Vec<f64>> = ring.iter().map(|c| vec![c.x, c.y]).collect();

            // Find the outer polygon that contains this inner ring
            let mut added = false;
            for poly in &mut polygons {
                if !poly.is_empty() {
                    // Check if outer contains inner
                    let outer_coords: Vec<Coord<f64>> = poly[0]
                        .iter()
                        .map(|c| Coord { x: c[0], y: c[1] })
                        .collect();
                    let inner_coords: Vec<Coord<f64>> =
                        ring.iter().map(|c| Coord { x: c.x, y: c.y }).collect();

                    if crate::mputil::polygon_contains(&outer_coords, &inner_coords) {
                        poly.push(ring_coords.clone());
                        added = true;
                        break;
                    }
                }
            }

            if !added && ctx.options.include_invalid_polygons && !polygons.is_empty() {
                polygons[0].push(ring_coords);
            }
        }

        if polygons.len() == 1 {
            Geometry::new(Value::Polygon(polygons.remove(0)))
        } else {
            Geometry::new(Value::MultiPolygon(polygons))
        }
    };

    let mut properties = JsonObject::new();
    properties.insert("id".to_string(), JsonValue::from(relation.id.0));
    properties.insert("type".to_string(), JsonValue::from("relation"));
    properties.insert("tags".to_string(), tags_to_json(&relation.tags));

    if tainted {
        properties.insert("tainted".to_string(), JsonValue::from(true));
    }

    add_meta_properties(ctx, &mut properties, relation);

    let id = if ctx.options.no_id {
        None
    } else {
        Some(geojson::feature::Id::String(format!(
            "relation/{}",
            relation.id.0
        )))
    };

    Some(Feature {
        bbox: None,
        geometry: Some(geometry),
        id,
        properties: Some(properties),
        foreign_members: None,
    })
}

fn add_meta_properties<E: Element>(ctx: &ConvertContext, props: &mut JsonObject, element: &E) {
    // Add relation membership
    if !ctx.options.no_relation_membership {
        let fid = element.feature_id();
        if let Some(relations) = ctx.relation_member.get(&fid) {
            let rel_array: Vec<JsonValue> = relations
                .iter()
                .map(|r| {
                    let mut obj = JsonObject::new();
                    obj.insert("id".to_string(), JsonValue::from(r.id.0));
                    obj.insert("role".to_string(), JsonValue::from(r.role.clone()));

                    let tags_obj: JsonObject = r
                        .tags
                        .iter()
                        .map(|(k, v)| (k.clone(), JsonValue::from(v.clone())))
                        .collect();
                    obj.insert("tags".to_string(), JsonValue::Object(tags_obj));

                    JsonValue::Object(obj)
                })
                .collect();
            props.insert("relations".to_string(), JsonValue::Array(rel_array));
        } else {
            props.insert("relations".to_string(), JsonValue::Array(vec![]));
        }
    }

    if ctx.options.no_meta {
        return;
    }

    let tag_map = element.tag_map();
    let mut meta = JsonObject::new();

    // Add version, changeset, user, uid, timestamp from tags if available
    // This is a simplified version - full implementation would access element fields directly
    if let Some(v) = tag_map.get("version") {
        if let Ok(version) = v.parse::<i32>() {
            meta.insert("version".to_string(), JsonValue::from(version));
        }
    }

    props.insert("meta".to_string(), JsonValue::Object(meta));
}

fn tags_to_json(tags: &Tags) -> JsonValue {
    let map: JsonObject = tags.map().into_iter().map(|(k, v)| (k, JsonValue::from(v))).collect();
    JsonValue::Object(map)
}

/// Uninteresting tags that don't make a node/way "interesting"
static UNINTERESTING_TAGS: &[&str] = &[
    "source",
    "source_ref",
    "source:ref",
    "history",
    "attribution",
    "created_by",
    "tiger:cfcc",
    "tiger:county",
    "tiger:reviewed",
    "tiger:separated",
    "tiger:source",
    "tiger:tlid",
    "tiger:upload_uuid",
    "fixme",
    "FIXME",
];

fn has_interesting_tags(tags: &Tags, ignore: Option<&HashMap<String, String>>) -> bool {
    if tags.is_empty() {
        return false;
    }

    for tag in tags.0.iter() {
        let k = &tag.key;
        let v = &tag.value;

        if UNINTERESTING_TAGS.contains(&k.as_str()) {
            continue;
        }

        if let Some(ignore_map) = ignore {
            if ignore_map.get(k) == Some(&"true".to_string()) || ignore_map.get(k) == Some(v) {
                continue;
            }
        }

        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NodeID, Tag, WayNode, WayNodes};

    #[test]
    fn test_convert_empty() {
        let osm = OSM::new();
        let options = ConvertOptions::new();
        let fc = convert(&osm, &options);
        assert!(fc.features.is_empty());
    }

    #[test]
    fn test_convert_node() {
        let mut osm = OSM::new();
        osm.nodes.push(Node {
            id: NodeID(1),
            lat: 51.5,
            lon: -0.1,
            version: 1,
            tags: Tags::from_vec(vec![Tag::new("name", "Test")]),
            ..Default::default()
        });

        let options = ConvertOptions::new();
        let fc = convert(&osm, &options);

        assert_eq!(fc.features.len(), 1);
        let feature = &fc.features[0];

        if let Some(geojson::feature::Id::String(id)) = &feature.id {
            assert_eq!(id, "node/1");
        } else {
            panic!("Expected string ID");
        }
    }

    #[test]
    fn test_convert_way() {
        let mut osm = OSM::new();
        osm.ways.push(Way {
            id: WayID(1),
            version: 1,
            nodes: WayNodes(vec![
                WayNode {
                    id: NodeID(1),
                    lat: 51.5,
                    lon: -0.1,
                    ..Default::default()
                },
                WayNode {
                    id: NodeID(2),
                    lat: 51.6,
                    lon: -0.2,
                    ..Default::default()
                },
            ]),
            tags: Tags::from_vec(vec![Tag::new("highway", "primary")]),
            ..Default::default()
        });

        let options = ConvertOptions::new();
        let fc = convert(&osm, &options);

        assert_eq!(fc.features.len(), 1);
    }

    #[test]
    fn test_has_interesting_tags() {
        let tags = Tags::from_vec(vec![Tag::new("name", "Test")]);
        assert!(has_interesting_tags(&tags, None));

        let tags = Tags::from_vec(vec![Tag::new("source", "survey")]);
        assert!(!has_interesting_tags(&tags, None));

        let tags = Tags::from_vec(vec![]);
        assert!(!has_interesting_tags(&tags, None));
    }
}
