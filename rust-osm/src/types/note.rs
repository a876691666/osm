//! Note type for OSM data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{NoteID, ObjectID, Type, UserID};

/// Notes is a collection of notes with helpers
pub type Notes = Vec<Note>;

/// Note is information for other mappers dropped at a map location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    #[serde(rename = "type", default)]
    pub type_marker: NoteTypeMarker,
    pub id: NoteID,
    pub lat: f64,
    pub lon: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub close_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reopen_url: String,
    pub date_created: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "is_default_timestamp")]
    pub date_closed: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(default)]
    pub comments: Vec<NoteComment>,
}

fn is_default_timestamp(v: &Option<DateTime<Utc>>) -> bool {
    v.is_none()
}

/// Marker type to serialize "note" as the type field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoteTypeMarker;

impl Serialize for NoteTypeMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("note")
    }
}

impl<'de> Deserialize<'de> for NoteTypeMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = String::deserialize(deserializer)?;
        Ok(NoteTypeMarker)
    }
}

impl Default for Note {
    fn default() -> Self {
        Note {
            type_marker: NoteTypeMarker,
            id: NoteID(0),
            lat: 0.0,
            lon: 0.0,
            url: String::new(),
            comment_url: String::new(),
            close_url: String::new(),
            reopen_url: String::new(),
            date_created: DateTime::default(),
            date_closed: None,
            status: String::new(),
            comments: Vec::new(),
        }
    }
}

impl Note {
    /// Returns the object id of the note
    pub fn object_id(&self) -> ObjectID {
        self.id.object_id()
    }
}

/// Implementation of Object trait for Note
impl crate::Object for Note {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::Note
    }
}

/// NoteComment is a comment on a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteComment {
    pub date: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "is_zero_user_id")]
    pub uid: UserID,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_url: String,
    pub action: String,
    pub text: String,
    #[serde(default)]
    pub html: String,
}

fn is_zero_user_id(v: &UserID) -> bool {
    v.0 == 0
}

impl Default for NoteComment {
    fn default() -> Self {
        NoteComment {
            date: DateTime::default(),
            uid: UserID(0),
            user: String::new(),
            user_url: String::new(),
            action: String::new(),
            text: String::new(),
            html: String::new(),
        }
    }
}

/// NoteCommentAction are actions that a note comment took
pub mod note_comment_action {
    pub const OPENED: &str = "opened";
    pub const COMMENT: &str = "commented";
    pub const CLOSED: &str = "closed";
}

/// NoteStatus is the status of the note
pub mod note_status {
    pub const OPEN: &str = "open";
    pub const CLOSED: &str = "closed";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_object_id() {
        let note = Note {
            id: NoteID(123),
            ..Default::default()
        };
        let oid = note.object_id();
        assert_eq!(oid.type_(), Type::Note);
    }
}
