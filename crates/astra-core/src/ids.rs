use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

uuid_id!(ResourceId);
uuid_id!(InstanceId);
uuid_id!(ViewInstanceId);

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(IdentifierError::Empty);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(D::Error::custom)
            }
        }
    };
}

string_id!(PointId);
string_id!(AspectId);
string_id!(ChartSlotId);
string_id!(PanelId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Revision(u64);

impl Revision {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u64) -> Result<Self, RevisionError> {
        if value == 0 {
            Err(RevisionError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Result<Self, RevisionError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(RevisionError::Overflow),
        }
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for Revision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u64::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    pub const V1: Self = Self(1);

    pub const fn new(value: u32) -> Result<Self, SchemaVersionError> {
        if value == 0 {
            Err(SchemaVersionError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    pub const fn from_unix_millis(value: i64) -> Self {
        Self(value)
    }

    pub const fn unix_millis(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ResourceRevisionRef {
    pub id: ResourceId,
    pub revision: Revision,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum IdentifierError {
    #[error("identifier must not be empty")]
    Empty,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RevisionError {
    #[error("revision zero is invalid")]
    Zero,
    #[error("revision overflow")]
    Overflow,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SchemaVersionError {
    #[error("schema version zero is invalid")]
    Zero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_identifiers_and_versions_reject_invalid_values() {
        assert!(serde_json::from_str::<PointId>(r#"" ""#).is_err());
        assert!(serde_json::from_str::<Revision>("0").is_err());
        assert!(serde_json::from_str::<SchemaVersion>("0").is_err());
    }
}
