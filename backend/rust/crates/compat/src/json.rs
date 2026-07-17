//! Serde helpers for the modern internal JSON dialect
//! (docs/api-dialect.md §4.1/§4.4/§4.5).
//!
//! Request structs on modern internal routes are plain `serde` structs
//! extracted with `axum::Json` and carry `#[serde(deny_unknown_fields)]` so
//! typos become validation errors instead of silent retains (§4.4). Clearable
//! fields on update-class (`PATCH`) endpoints use the [`double_option`]
//! recipe; epoch-second storage fields cross the API boundary as RFC 3339 UTC
//! strings via [`rfc3339`]/[`rfc3339_option`] (§4.5). Consumed by the family
//! waves from W2 on (docs/api-dialect.md Appendix A).

/// The §4.4 null-clear/absent-retain tri-state for update-class endpoints:
///
/// | JSON state | Rust value | Meaning |
/// | --- | --- | --- |
/// | field absent | `None` | retain current value |
/// | field `null` | `Some(None)` | clear (set NULL / disable) |
/// | field value | `Some(Some(v))` | set |
///
/// Recipe (every attribute is load-bearing):
///
/// ```ignore
/// #[serde(default, with = "double_option", skip_serializing_if = "Option::is_none")]
/// remark: Option<Option<String>>,
/// ```
pub mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }

    pub fn serialize<T, S>(value: &Option<Option<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        match value {
            Some(inner) => inner.serialize(serializer),
            // Reachable only when `skip_serializing_if = "Option::is_none"`
            // was omitted from the field; degrade to `null` rather than panic.
            None => serializer.serialize_none(),
        }
    }
}

/// §4.5: epoch-second storage integers serialize as RFC 3339 UTC strings
/// (`"2026-07-17T08:30:00Z"`). Deserialization accepts any valid RFC 3339
/// offset and normalizes to the UTC epoch; serialization always emits `Z`.
/// Storage stays epoch seconds — conversion happens only at this API serde
/// boundary.
pub mod rfc3339 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        super::parse_epoch(&value).map_err(serde::de::Error::custom)
    }

    pub fn serialize<S>(epoch_seconds: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let formatted = super::format_epoch(*epoch_seconds)
            .ok_or_else(|| serde::ser::Error::custom("timestamp is out of range"))?;
        serializer.serialize_str(&formatted)
    }
}

/// [`rfc3339`] for nullable timestamps (`expired_at: null` = never expires):
/// JSON `null` ⇄ `None`.
pub mod rfc3339_option {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| super::parse_epoch(&value).map_err(serde::de::Error::custom))
            .transpose()
    }

    pub fn serialize<S>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(epoch_seconds) => {
                let formatted = super::format_epoch(*epoch_seconds)
                    .ok_or_else(|| serde::ser::Error::custom("timestamp is out of range"))?;
                serializer.serialize_some(&formatted)
            }
            None => serializer.serialize_none(),
        }
    }
}

fn format_epoch(epoch_seconds: i64) -> Option<String> {
    Some(
        chrono::DateTime::from_timestamp(epoch_seconds, 0)?
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
}

fn parse_epoch(value: &str) -> Result<i64, String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|instant| instant.timestamp())
        .map_err(|error| format!("invalid RFC 3339 timestamp: {error}"))
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    /// A §4.4-shaped update request: `deny_unknown_fields` posture plus one
    /// clearable field using the full double-Option recipe.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct UpdateRequest {
        #[serde(
            default,
            with = "super::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        remark: Option<Option<String>>,
    }

    #[test]
    fn double_option_absent_means_retain() {
        let request: UpdateRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(request.remark, None);
        assert_eq!(serde_json::to_string(&request).unwrap(), "{}");
    }

    #[test]
    fn double_option_null_means_clear() {
        let request: UpdateRequest = serde_json::from_str("{\"remark\":null}").unwrap();
        assert_eq!(request.remark, Some(None));
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            "{\"remark\":null}"
        );
    }

    #[test]
    fn double_option_value_means_set() {
        let request: UpdateRequest = serde_json::from_str("{\"remark\":\"kept\"}").unwrap();
        assert_eq!(request.remark, Some(Some("kept".to_string())));
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            "{\"remark\":\"kept\"}"
        );
    }

    #[test]
    fn unknown_fields_are_rejected_not_silently_retained() {
        let error = serde_json::from_str::<UpdateRequest>("{\"remark\":\"x\",\"typo\":1}")
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field `typo`"), "{error}");
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Stamped {
        #[serde(with = "super::rfc3339")]
        created_at: i64,
        #[serde(with = "super::rfc3339_option")]
        expired_at: Option<i64>,
    }

    #[test]
    fn rfc3339_round_trips_through_the_spec_example() {
        let json = "{\"created_at\":\"2026-07-17T08:30:00Z\",\"expired_at\":null}";
        let stamped: Stamped = serde_json::from_str(json).unwrap();
        assert_eq!(stamped.expired_at, None);
        assert_eq!(serde_json::to_string(&stamped).unwrap(), json);
    }

    #[test]
    fn rfc3339_epoch_zero_is_unix_origin() {
        let stamped = Stamped {
            created_at: 0,
            expired_at: Some(0),
        };
        assert_eq!(
            serde_json::to_string(&stamped).unwrap(),
            "{\"created_at\":\"1970-01-01T00:00:00Z\",\"expired_at\":\"1970-01-01T00:00:00Z\"}"
        );
    }

    #[test]
    fn rfc3339_normalizes_offsets_to_utc_epoch() {
        const UTC: &str = "{\"created_at\":\"2026-07-17T08:30:00Z\",\"expired_at\":null}";
        const OFFSET: &str = "{\"created_at\":\"2026-07-17T16:30:00+08:00\",\"expired_at\":null}";
        let offset: Stamped = serde_json::from_str(OFFSET).unwrap();
        let utc: Stamped = serde_json::from_str(UTC).unwrap();
        assert_eq!(offset.created_at, utc.created_at);
        // Re-serialization always emits the `Z` form.
        assert_eq!(serde_json::to_string(&offset).unwrap(), UTC);
    }

    #[test]
    fn rfc3339_rejects_bare_epoch_integers_and_garbage() {
        assert!(
            serde_json::from_str::<Stamped>("{\"created_at\":1752741000,\"expired_at\":null}")
                .is_err()
        );
        assert!(
            serde_json::from_str::<Stamped>("{\"created_at\":\"yesterday\",\"expired_at\":null}")
                .is_err()
        );
    }
}
