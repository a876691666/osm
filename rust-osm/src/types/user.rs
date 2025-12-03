//! User type for OSM data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{ObjectID, Type, UserID};

/// Users is a collection of users with helpers
pub type Users = Vec<User>;

/// A User is a registered OSM user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "type", default)]
    pub type_marker: UserTypeMarker,
    pub id: UserID,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub img: UserImage,
    #[serde(default)]
    pub changesets: UserChangesets,
    #[serde(default)]
    pub traces: UserTraces,
    #[serde(default)]
    pub home: UserHome,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub blocks: UserBlocks,
    #[serde(default)]
    pub messages: UserMessages,
    pub created_at: DateTime<Utc>,
}

/// Marker type to serialize "user" as the type field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UserTypeMarker;

impl Serialize for UserTypeMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("user")
    }
}

impl<'de> Deserialize<'de> for UserTypeMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = String::deserialize(deserializer)?;
        Ok(UserTypeMarker)
    }
}

impl Default for User {
    fn default() -> Self {
        User {
            type_marker: UserTypeMarker,
            id: UserID(0),
            name: String::new(),
            description: String::new(),
            img: UserImage::default(),
            changesets: UserChangesets::default(),
            traces: UserTraces::default(),
            home: UserHome::default(),
            languages: Vec::new(),
            blocks: UserBlocks::default(),
            messages: UserMessages::default(),
            created_at: DateTime::default(),
        }
    }
}

impl User {
    /// Returns the object id of the user
    pub fn object_id(&self) -> ObjectID {
        self.id.object_id()
    }
}

/// Implementation of Object trait for User
impl crate::Object for User {
    fn object_id(&self) -> ObjectID {
        self.object_id()
    }

    fn object_type(&self) -> Type {
        Type::User
    }
}

/// User image info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserImage {
    #[serde(default)]
    pub href: String,
}

/// User changesets count
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserChangesets {
    #[serde(default)]
    pub count: i32,
}

/// User traces count
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserTraces {
    #[serde(default)]
    pub count: i32,
}

/// User home location
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserHome {
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
    #[serde(default)]
    pub zoom: i32,
}

/// User blocks info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserBlocks {
    #[serde(default)]
    pub received: UserBlocksReceived,
}

/// User blocks received info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserBlocksReceived {
    #[serde(default)]
    pub count: i32,
    #[serde(default)]
    pub active: i32,
}

/// User messages info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserMessages {
    #[serde(default)]
    pub received: UserMessagesReceived,
    #[serde(default)]
    pub sent: UserMessagesSent,
}

/// User messages received info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserMessagesReceived {
    #[serde(default)]
    pub count: i32,
    #[serde(default)]
    pub unread: i32,
}

/// User messages sent info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserMessagesSent {
    #[serde(default)]
    pub count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_object_id() {
        let user = User {
            id: UserID(123),
            ..Default::default()
        };
        let oid = user.object_id();
        assert_eq!(oid.type_(), Type::User);
    }
}
