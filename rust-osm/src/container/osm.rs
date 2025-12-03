//! OSM container type

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{
    Bounds, Changeset, ElementID, ElementIDs, FeatureID, FeatureIDs, Node, Note, ObjectID,
    Relation, Type, User, Way,
};

/// OSM represents the core osm data
/// designed to parse http://wiki.openstreetmap.org/wiki/OSM_XML
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OSM {
    /// Version can be string or number, stored as string for consistency
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generator: String,

    /// Copyright, Attribution and License contain information about data source
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub copyright: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribution: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ways: Vec<Way>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,

    /// Changesets will typically not be included with actual data
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changesets: Vec<Changeset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<User>,
}

/// ObjectWrapper is an enum for holding different OSM object types
#[derive(Debug, Clone)]
pub enum OsmObject {
    Node(Node),
    Way(Way),
    Relation(Relation),
    Changeset(Changeset),
    Note(Note),
    User(User),
    Bounds(Bounds),
}

impl OsmObject {
    /// Returns the ObjectID for this object
    pub fn object_id(&self) -> ObjectID {
        match self {
            OsmObject::Node(n) => crate::Object::object_id(n),
            OsmObject::Way(w) => crate::Object::object_id(w),
            OsmObject::Relation(r) => crate::Object::object_id(r),
            OsmObject::Changeset(c) => crate::Object::object_id(c),
            OsmObject::Note(n) => crate::Object::object_id(n),
            OsmObject::User(u) => crate::Object::object_id(u),
            OsmObject::Bounds(b) => crate::Object::object_id(b),
        }
    }

    /// Returns the Type for this object
    pub fn object_type(&self) -> Type {
        match self {
            OsmObject::Node(_) => Type::Node,
            OsmObject::Way(_) => Type::Way,
            OsmObject::Relation(_) => Type::Relation,
            OsmObject::Changeset(_) => Type::Changeset,
            OsmObject::Note(_) => Type::Note,
            OsmObject::User(_) => Type::User,
            OsmObject::Bounds(_) => Type::Bounds,
        }
    }
}

impl OSM {
    /// Creates a new empty OSM container
    pub fn new() -> Self {
        OSM::default()
    }

    /// Appends an object to the OSM container
    pub fn append(&mut self, obj: OsmObject) {
        match obj {
            OsmObject::Node(n) => self.nodes.push(n),
            OsmObject::Way(w) => self.ways.push(w),
            OsmObject::Relation(r) => self.relations.push(r),
            OsmObject::Changeset(c) => self.changesets.push(c),
            OsmObject::Note(n) => self.notes.push(n),
            OsmObject::User(u) => self.users.push(u),
            OsmObject::Bounds(b) => self.bounds = Some(b),
        }
    }

    /// Returns all nodes, ways and relations as Elements
    pub fn elements(&self) -> Vec<OsmElement> {
        let mut result = Vec::with_capacity(self.nodes.len() + self.ways.len() + self.relations.len());
        for n in &self.nodes {
            result.push(OsmElement::Node(n.clone()));
        }
        for w in &self.ways {
            result.push(OsmElement::Way(w.clone()));
        }
        for r in &self.relations {
            result.push(OsmElement::Relation(r.clone()));
        }
        result
    }

    /// Returns all objects including bounds, nodes, ways, relations, changesets, notes and users
    pub fn objects(&self) -> Vec<OsmObject> {
        let mut result = Vec::new();
        if let Some(b) = &self.bounds {
            result.push(OsmObject::Bounds(b.clone()));
        }
        for n in &self.nodes {
            result.push(OsmObject::Node(n.clone()));
        }
        for w in &self.ways {
            result.push(OsmObject::Way(w.clone()));
        }
        for r in &self.relations {
            result.push(OsmObject::Relation(r.clone()));
        }
        for c in &self.changesets {
            result.push(OsmObject::Changeset(c.clone()));
        }
        for u in &self.users {
            result.push(OsmObject::User(u.clone()));
        }
        for n in &self.notes {
            result.push(OsmObject::Note(n.clone()));
        }
        result
    }

    /// Returns the feature ids for all nodes, ways and relations
    pub fn feature_ids(&self) -> FeatureIDs {
        let mut result = Vec::with_capacity(self.nodes.len() + self.ways.len() + self.relations.len());
        for n in &self.nodes {
            result.push(n.id.feature_id());
        }
        for w in &self.ways {
            result.push(w.id.feature_id());
        }
        for r in &self.relations {
            result.push(r.id.feature_id());
        }
        FeatureIDs(result)
    }

    /// Returns the element ids for all nodes, ways and relations
    pub fn element_ids(&self) -> ElementIDs {
        let mut result = Vec::with_capacity(self.nodes.len() + self.ways.len() + self.relations.len());
        for n in &self.nodes {
            result.push(crate::Element::element_id(n));
        }
        for w in &self.ways {
            result.push(crate::Element::element_id(w));
        }
        for r in &self.relations {
            result.push(crate::Element::element_id(r));
        }
        ElementIDs(result)
    }
}

/// OsmElement is an enum for holding Node, Way, or Relation
#[derive(Debug, Clone)]
pub enum OsmElement {
    Node(Node),
    Way(Way),
    Relation(Relation),
}

impl OsmElement {
    /// Returns the ElementID for this element
    pub fn element_id(&self) -> ElementID {
        match self {
            OsmElement::Node(n) => crate::Element::element_id(n),
            OsmElement::Way(w) => crate::Element::element_id(w),
            OsmElement::Relation(r) => crate::Element::element_id(r),
        }
    }

    /// Returns the FeatureID for this element
    pub fn feature_id(&self) -> FeatureID {
        match self {
            OsmElement::Node(n) => crate::Element::feature_id(n),
            OsmElement::Way(w) => crate::Element::feature_id(w),
            OsmElement::Relation(r) => crate::Element::feature_id(r),
        }
    }

    /// Returns the Type for this element
    pub fn element_type(&self) -> Type {
        match self {
            OsmElement::Node(_) => Type::Node,
            OsmElement::Way(_) => Type::Way,
            OsmElement::Relation(_) => Type::Relation,
        }
    }
}

/// Custom JSON deserialization for OSM (handling array of elements)
impl OSM {
    /// Deserializes OSM from JSON format (like Overpass API response)
    pub fn from_json(data: &[u8]) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct JsonOSM {
            #[serde(default)]
            version: Value,
            #[serde(default)]
            generator: Option<String>,
            #[serde(default)]
            copyright: Option<String>,
            #[serde(default)]
            attribution: Option<String>,
            #[serde(default)]
            license: Option<String>,
            #[serde(default)]
            elements: Vec<Value>,
        }

        let json: JsonOSM = serde_json::from_slice(data)?;

        let mut osm = OSM {
            version: match json.version {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            },
            generator: json.generator.unwrap_or_default(),
            copyright: json.copyright.unwrap_or_default(),
            attribution: json.attribution.unwrap_or_default(),
            license: json.license.unwrap_or_default(),
            ..Default::default()
        };

        for elem in json.elements {
            let type_str = elem
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("");

            match type_str {
                "node" => {
                    if let Ok(n) = serde_json::from_value::<Node>(elem) {
                        osm.nodes.push(n);
                    }
                }
                "way" => {
                    if let Ok(w) = serde_json::from_value::<Way>(elem) {
                        osm.ways.push(w);
                    }
                }
                "relation" => {
                    if let Ok(r) = serde_json::from_value::<Relation>(elem) {
                        osm.relations.push(r);
                    }
                }
                "changeset" => {
                    if let Ok(c) = serde_json::from_value::<Changeset>(elem) {
                        osm.changesets.push(c);
                    }
                }
                "note" => {
                    if let Ok(n) = serde_json::from_value::<Note>(elem) {
                        osm.notes.push(n);
                    }
                }
                "user" => {
                    if let Ok(u) = serde_json::from_value::<User>(elem) {
                        osm.users.push(u);
                    }
                }
                _ => {}
            }
        }

        Ok(osm)
    }

    /// Serializes OSM to JSON format (like Overpass API response)
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        #[derive(Serialize)]
        struct JsonOSM<'a> {
            #[serde(skip_serializing_if = "str::is_empty")]
            version: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            generator: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            copyright: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            attribution: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            license: &'a str,
            elements: Vec<Value>,
        }

        let mut elements = Vec::new();

        for obj in self.objects() {
            let val = match obj {
                OsmObject::Node(n) => serde_json::to_value(n)?,
                OsmObject::Way(w) => serde_json::to_value(w)?,
                OsmObject::Relation(r) => serde_json::to_value(r)?,
                OsmObject::Changeset(c) => serde_json::to_value(c)?,
                OsmObject::Note(n) => serde_json::to_value(n)?,
                OsmObject::User(u) => serde_json::to_value(u)?,
                OsmObject::Bounds(_) => continue, // Bounds not in elements array
            };
            elements.push(val);
        }

        let json = JsonOSM {
            version: &self.version,
            generator: &self.generator,
            copyright: &self.copyright,
            attribution: &self.attribution,
            license: &self.license,
            elements,
        };

        serde_json::to_vec(&json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeID;

    #[test]
    fn test_osm_new() {
        let osm = OSM::new();
        assert!(osm.nodes.is_empty());
        assert!(osm.ways.is_empty());
        assert!(osm.relations.is_empty());
    }

    #[test]
    fn test_osm_append() {
        let mut osm = OSM::new();
        osm.append(OsmObject::Node(Node {
            id: NodeID(1),
            ..Default::default()
        }));
        assert_eq!(osm.nodes.len(), 1);
    }

    #[test]
    fn test_osm_feature_ids() {
        let mut osm = OSM::new();
        osm.nodes.push(Node {
            id: NodeID(1),
            ..Default::default()
        });
        osm.nodes.push(Node {
            id: NodeID(2),
            ..Default::default()
        });

        let fids = osm.feature_ids();
        assert_eq!(fids.0.len(), 2);
    }
}
