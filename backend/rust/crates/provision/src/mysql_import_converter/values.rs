use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

use crate::mysql_import_policy::LegacyOrderPolicyError;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum SourceValue {
    Null,
    I64(i64),
    U64(u64),
    Decimal(String),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalValue {
    Null,
    I64(i64),
    U64(u64),
    Decimal(String),
    Text(String),
    Bytes(Vec<u8>),
    Json(CanonicalJson),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalJson(pub(super) ExactJsonValue);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExactJsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl CanonicalJson {
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw = serde_json::from_str::<Box<RawValue>>(text).map_err(|error| error.to_string())?;
        parse_exact_json_value(&raw).map(Self)
    }

    pub(super) fn from_serde_value(value: &Value) -> Result<Self, String> {
        let encoded = serde_json::to_string(value).map_err(|error| error.to_string())?;
        Self::parse(&encoded)
    }

    pub(super) fn is_array(&self) -> bool {
        matches!(self.0, ExactJsonValue::Array(_))
    }

    pub fn to_compact_json(&self) -> Result<String, serde_json::Error> {
        let mut encoded = String::new();
        write_exact_json_value(&mut encoded, &self.0)?;
        Ok(encoded)
    }

    pub fn contains_nul(&self) -> bool {
        exact_json_contains_nul(&self.0)
    }
}

fn parse_exact_json_value(raw: &RawValue) -> Result<ExactJsonValue, String> {
    let text = raw.get().trim();
    match text.as_bytes().first().copied() {
        Some(b'n') if text == "null" => Ok(ExactJsonValue::Null),
        Some(b't') if text == "true" => Ok(ExactJsonValue::Bool(true)),
        Some(b'f') if text == "false" => Ok(ExactJsonValue::Bool(false)),
        Some(b'\"') => serde_json::from_str(text)
            .map(ExactJsonValue::String)
            .map_err(|error| error.to_string()),
        Some(b'[') => serde_json::from_str::<Vec<Box<RawValue>>>(text)
            .map_err(|error| error.to_string())?
            .iter()
            .map(|value| parse_exact_json_value(value))
            .collect::<Result<Vec<_>, _>>()
            .map(ExactJsonValue::Array),
        Some(b'{') => serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(text)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|(key, value)| parse_exact_json_value(&value).map(|value| (key, value)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(ExactJsonValue::Object),
        Some(_) => canonical_json_number(text)
            .map(ExactJsonValue::Number)
            .ok_or_else(|| "number cannot be represented exactly".to_string()),
        None => Err("JSON value is empty".to_string()),
    }
}

fn write_exact_json_value(
    output: &mut String,
    value: &ExactJsonValue,
) -> Result<(), serde_json::Error> {
    match value {
        ExactJsonValue::Null => output.push_str("null"),
        ExactJsonValue::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        ExactJsonValue::Number(value) => output.push_str(value),
        ExactJsonValue::String(value) => output.push_str(&serde_json::to_string(value)?),
        ExactJsonValue::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_exact_json_value(output, value)?;
            }
            output.push(']');
        }
        ExactJsonValue::Object(values) => {
            output.push('{');
            for (index, (key, value)) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key)?);
                output.push(':');
                write_exact_json_value(output, value)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

fn exact_json_contains_nul(value: &ExactJsonValue) -> bool {
    match value {
        ExactJsonValue::String(value) => value.contains('\0'),
        ExactJsonValue::Array(values) => values.iter().any(exact_json_contains_nul),
        ExactJsonValue::Object(values) => values
            .iter()
            .any(|(key, value)| key.contains('\0') || exact_json_contains_nul(value)),
        ExactJsonValue::Null | ExactJsonValue::Bool(_) | ExactJsonValue::Number(_) => false,
    }
}

pub type SourceRow = BTreeMap<String, SourceValue>;
pub type CanonicalRow = BTreeMap<String, CanonicalValue>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegacyGiftcardRedemptionRow {
    pub giftcard_id: i32,
    pub user_id: i64,
    pub created_at: i64,
    pub created_at_provenance: String,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ConverterError {
    #[error("converter registry is invalid: {0}")]
    Registry(String),
    #[error("unknown mapping table {0}")]
    UnknownTable(String),
    #[error("source row for {table} is missing column {column}")]
    MissingColumn { table: String, column: String },
    #[error("source row for {table} contains unconsumed column {column}")]
    UnconsumedColumn { table: String, column: String },
    #[error("{table}.{column} has incompatible source type {actual}")]
    TypeMismatch {
        table: String,
        column: String,
        actual: &'static str,
    },
    #[error("{table}.{column} contains invalid JSON: {message}")]
    InvalidJson {
        table: String,
        column: String,
        message: String,
    },
    #[error("{table}.{column} contains an invalid exact decimal")]
    InvalidDecimal { table: String, column: String },
    #[error("{table}.{column} contains an invalid positive id at member {index}")]
    InvalidIdArray {
        table: String,
        column: String,
        index: usize,
    },
    #[error("{table}.{column} integer {value} cannot be represented by PostgreSQL BIGINT")]
    IntegerOutOfRange {
        table: String,
        column: String,
        value: u64,
    },
    #[error(transparent)]
    OrderPolicy(#[from] LegacyOrderPolicyError),
}

/// Canonicalizes one already-validated JSON number by exact base-10 value.
/// PostgreSQL JSONB stores JSON numbers as NUMERIC, so spelling differences
/// such as `1.2300e3` and `1230` must hash equally without passing through f64.
pub(super) fn canonical_json_number(value: &str) -> Option<String> {
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |unsigned| (true, unsigned));
    let (mantissa, explicit_exponent) = unsigned
        .split_once(['e', 'E'])
        .map_or((unsigned, "0"), |parts| parts);
    let explicit_exponent = explicit_exponent.parse::<i64>().ok()?;
    let (integer, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |parts| parts);
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let fraction_len = i64::try_from(fraction.len()).ok()?;
    let mut exponent = explicit_exponent.checked_sub(fraction_len)?;
    let mut digits = String::with_capacity(integer.len().checked_add(fraction.len())?);
    digits.push_str(integer);
    digits.push_str(fraction);
    let first_nonzero = digits.bytes().position(|byte| byte != b'0');
    let Some(first_nonzero) = first_nonzero else {
        return Some("0".to_string());
    };
    let significant = &digits[first_nonzero..];
    let trailing_zeroes = significant
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'0')
        .count();
    exponent = exponent.checked_add(i64::try_from(trailing_zeroes).ok()?)?;
    let significant = &significant[..significant.len() - trailing_zeroes];
    Some(format!(
        "{}{significant}e{exponent}",
        if negative { "-" } else { "" }
    ))
}
