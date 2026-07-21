//! Serde helpers for PATCH fields where absence means retain and JSON `null`
//! either means clear or is rejected, according to the field contract.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// PATCH field whose absence means retain but whose explicit JSON value must
/// be non-null. `Option<T>` cannot model this: Serde aliases both an absent
/// property and an explicit `null` to `None`, silently turning an invalid clear
/// into a successful no-op. The containing field uses `#[serde(default)]`, so
/// absence produces `Retain`; when present this deserializer delegates directly
/// to `T`, which rejects `null` for strings, numbers and booleans.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum NonNull<T> {
    #[default]
    Retain,
    Set(T),
}

impl<'de, T> Deserialize<'de> for NonNull<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Set)
    }
}

impl<T> NonNull<T> {
    #[must_use]
    pub const fn is_retain(&self) -> bool {
        matches!(self, Self::Retain)
    }

    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Retain => None,
            Self::Set(value) => Some(value),
        }
    }
}

impl<T> Serialize for NonNull<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Retain => serializer.serialize_none(),
            Self::Set(value) => value.serialize(serializer),
        }
    }
}

pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

pub fn serialize<S, T>(value: &Option<Option<T>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    match value {
        None | Some(None) => serializer.serialize_none(),
        Some(Some(value)) => value.serialize(serializer),
    }
}
