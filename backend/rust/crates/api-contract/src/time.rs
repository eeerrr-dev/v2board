//! Wire-level RFC 3339 timestamp backed by an epoch second in application
//! code. PostgreSQL/application adapters may change independently of this
//! stable JSON representation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, ToSchema)]
#[schema(value_type = String, format = DateTime)]
pub struct Rfc3339Timestamp(i64);

impl Rfc3339Timestamp {
    #[must_use]
    pub const fn from_epoch_seconds(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn epoch_seconds(self) -> i64 {
        self.0
    }
}

impl Serialize for Rfc3339Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DateTime::<Utc>::from_timestamp(self.0, 0)
            .ok_or_else(|| serde::ser::Error::custom("timestamp is outside the RFC 3339 range"))?
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Rfc3339Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&value)
            .map(|value| Self(value.timestamp()))
            .map_err(D::Error::custom)
    }
}
