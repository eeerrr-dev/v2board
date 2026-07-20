use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Number, Value};

use crate::mysql_import_policy::{
    LegacyOrderBinding, LegacyOrderDisposition, classify_legacy_order,
    is_legacy_stripe_payment_driver,
};

use super::mappings::{
    AddedValue, ColumnRule, JsonOutput, JsonShape, TableMapping, TransformColumn,
    mapping_for_source,
};
use super::values::{
    CanonicalJson, CanonicalRow, CanonicalValue, ConverterError, LegacyGiftcardRedemptionRow,
    SourceRow, SourceValue,
};

pub(super) fn transform_row(
    mapping: &TableMapping,
    source: &SourceRow,
) -> Result<CanonicalRow, ConverterError> {
    for column in source.keys() {
        if !mapping_has_source_column(mapping, column) {
            return Err(ConverterError::UnconsumedColumn {
                table: mapping.source.to_string(),
                column: column.clone(),
            });
        }
    }
    let mut target = BTreeMap::new();
    for column in mapping.direct_columns {
        let value = required_source_value(mapping, source, column)?;
        target.insert((*column).to_string(), direct_value(value));
    }
    for column in mapping.transformed_columns {
        let value = required_source_value(mapping, source, column.source)?;
        let value = apply_rule(mapping, column, value)?;
        target.insert(column.target.to_string(), value);
    }
    for column in mapping.added_columns {
        let value = match column.value {
            AddedValue::Null => CanonicalValue::Null,
            AddedValue::I64(value) => CanonicalValue::I64(value),
        };
        target.insert(column.target.to_string(), value);
    }
    // Consumed columns must still be present in the source row so a partial
    // SELECT cannot accidentally look complete.
    for column in mapping.consumed_source_columns {
        required_source_value(mapping, source, column.source)?;
    }
    Ok(target)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MysqlImportRowDisposition {
    Discard,
    Retain(CanonicalRow),
}

/// The only public base-row conversion entry point. Orders are checked against
/// the complete source payment index before the fixed Stripe loss policy is
/// applied: Stripe payment rows and status 0/1 Stripe orders are omitted, while
/// status 2/3/4 Stripe history is detached from provider fields.
pub fn transform_mysql_import_row(
    mapping: &TableMapping,
    source: &SourceRow,
    known_payment_ids: &BTreeSet<i32>,
    stripe_payment_ids: &BTreeSet<i32>,
) -> Result<MysqlImportRowDisposition, ConverterError> {
    match mapping.source {
        "v2_payment" => {
            let driver = required_source_value(mapping, source, "payment")?;
            let SourceValue::Text(driver) = driver else {
                return Err(ConverterError::TypeMismatch {
                    table: mapping.source.to_string(),
                    column: "payment".to_string(),
                    actual: source_kind(driver),
                });
            };
            if is_legacy_stripe_payment_driver(driver) {
                Ok(MysqlImportRowDisposition::Discard)
            } else {
                transform_row(mapping, source).map(MysqlImportRowDisposition::Retain)
            }
        }
        "v2_order" => {
            let status = source_i16(mapping, source, "status")?;
            let payment_id = source_optional_i32(mapping, source, "payment_id")?;
            let callback_no = source_optional_text(mapping, source, "callback_no")?;
            match classify_legacy_order(
                LegacyOrderBinding {
                    status,
                    payment_id,
                    callback_no,
                },
                known_payment_ids,
                stripe_payment_ids,
            )? {
                LegacyOrderDisposition::DiscardUnfinishedStripe => {
                    Ok(MysqlImportRowDisposition::Discard)
                }
                LegacyOrderDisposition::RetainUnchanged(_) => {
                    transform_row(mapping, source).map(MysqlImportRowDisposition::Retain)
                }
                LegacyOrderDisposition::RetainScrubbedStripe(_) => {
                    let mut row = transform_row(mapping, source)?;
                    row.insert("payment_id".to_string(), CanonicalValue::Null);
                    row.insert("callback_no".to_string(), CanonicalValue::Null);
                    row.insert("callback_no_hash".to_string(), CanonicalValue::Null);
                    Ok(MysqlImportRowDisposition::Retain(row))
                }
            }
        }
        _ => transform_row(mapping, source).map(MysqlImportRowDisposition::Retain),
    }
}

fn source_i16(
    mapping: &TableMapping,
    source: &SourceRow,
    column: &str,
) -> Result<i16, ConverterError> {
    let value = required_source_value(mapping, source, column)?;
    let converted = match value {
        SourceValue::I64(value) => i16::try_from(*value).ok(),
        SourceValue::U64(value) => i16::try_from(*value).ok(),
        _ => None,
    };
    converted.ok_or_else(|| ConverterError::TypeMismatch {
        table: mapping.source.to_string(),
        column: column.to_string(),
        actual: source_kind(value),
    })
}

fn source_optional_i32(
    mapping: &TableMapping,
    source: &SourceRow,
    column: &str,
) -> Result<Option<i32>, ConverterError> {
    let value = required_source_value(mapping, source, column)?;
    match value {
        SourceValue::Null => Ok(None),
        SourceValue::I64(value) => {
            i32::try_from(*value)
                .map(Some)
                .map_err(|_| ConverterError::TypeMismatch {
                    table: mapping.source.to_string(),
                    column: column.to_string(),
                    actual: "i64",
                })
        }
        SourceValue::U64(value) => {
            i32::try_from(*value)
                .map(Some)
                .map_err(|_| ConverterError::TypeMismatch {
                    table: mapping.source.to_string(),
                    column: column.to_string(),
                    actual: "u64",
                })
        }
        other => Err(ConverterError::TypeMismatch {
            table: mapping.source.to_string(),
            column: column.to_string(),
            actual: source_kind(other),
        }),
    }
}

fn source_optional_text(
    mapping: &TableMapping,
    source: &SourceRow,
    column: &str,
) -> Result<Option<String>, ConverterError> {
    let value = required_source_value(mapping, source, column)?;
    match value {
        SourceValue::Null => Ok(None),
        SourceValue::Text(value) => Ok(Some(value.clone())),
        other => Err(ConverterError::TypeMismatch {
            table: mapping.source.to_string(),
            column: column.to_string(),
            actual: source_kind(other),
        }),
    }
}

fn mapping_has_source_column(mapping: &TableMapping, name: &str) -> bool {
    mapping.direct_columns.contains(&name)
        || mapping
            .transformed_columns
            .iter()
            .any(|column| column.source == name)
        || mapping
            .consumed_source_columns
            .iter()
            .any(|column| column.source == name)
}

pub(super) fn source_columns_in_order(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.source),
        )
        .chain(
            mapping
                .consumed_source_columns
                .iter()
                .map(|column| column.source),
        )
        .collect()
}

fn required_source_value<'a>(
    mapping: &TableMapping,
    source: &'a SourceRow,
    column: &str,
) -> Result<&'a SourceValue, ConverterError> {
    source
        .get(column)
        .ok_or_else(|| ConverterError::MissingColumn {
            table: mapping.source.to_string(),
            column: column.to_string(),
        })
}

fn direct_value(value: &SourceValue) -> CanonicalValue {
    match value {
        SourceValue::Null => CanonicalValue::Null,
        SourceValue::I64(value) => CanonicalValue::I64(*value),
        SourceValue::U64(value) => CanonicalValue::U64(*value),
        SourceValue::Decimal(value) => CanonicalValue::Decimal(value.clone()),
        SourceValue::Text(value) => CanonicalValue::Text(value.clone()),
        SourceValue::Bytes(value) => CanonicalValue::Bytes(value.clone()),
    }
}

fn apply_rule(
    mapping: &TableMapping,
    column: &TransformColumn,
    source: &SourceValue,
) -> Result<CanonicalValue, ConverterError> {
    if matches!(source, SourceValue::Null) {
        return Ok(CanonicalValue::Null);
    }
    match column.rule {
        ColumnRule::Boolean01 => match source {
            SourceValue::I64(0) | SourceValue::U64(0) => Ok(CanonicalValue::Bool(false)),
            SourceValue::I64(1) | SourceValue::U64(1) => Ok(CanonicalValue::Bool(true)),
            SourceValue::I64(_) | SourceValue::U64(_) => Err(ConverterError::InvalidBoolean {
                table: mapping.source.to_string(),
                column: column.source.to_string(),
            }),
            other => type_mismatch(mapping, column, source_kind(other)),
        },
        ColumnRule::ExactDecimal => {
            let text = match source {
                SourceValue::Decimal(value) | SourceValue::Text(value) => value,
                other => return type_mismatch(mapping, column, source_kind(other)),
            };
            let normalized =
                normalize_decimal(text).ok_or_else(|| ConverterError::InvalidDecimal {
                    table: mapping.source.to_string(),
                    column: column.source.to_string(),
                })?;
            Ok(CanonicalValue::Decimal(normalized))
        }
        ColumnRule::Json(shape) => {
            let text = source_text(mapping, column, source)?;
            let value = parse_canonical_json(mapping, column, text)?;
            if !json_shape_matches(&value, shape) {
                return Err(ConverterError::InvalidJson {
                    table: mapping.source.to_string(),
                    column: column.source.to_string(),
                    message: format!("top-level value is not {shape:?}"),
                });
            }
            Ok(CanonicalValue::Json(value))
        }
        ColumnRule::PositiveIdArray {
            maximum,
            require_non_empty,
            output,
        } => {
            let text = source_text(mapping, column, source)?;
            let value = normalize_id_array(mapping, column, text, maximum, require_non_empty)?;
            match output {
                JsonOutput::Json => CanonicalJson::from_serde_value(&value)
                    .map(CanonicalValue::Json)
                    .map_err(|message| ConverterError::InvalidJson {
                        table: mapping.source.to_string(),
                        column: column.source.to_string(),
                        message,
                    }),
                JsonOutput::CanonicalText => Ok(CanonicalValue::Text(value.to_string())),
            }
        }
    }
}

fn source_text<'a>(
    mapping: &TableMapping,
    column: &TransformColumn,
    source: &'a SourceValue,
) -> Result<&'a str, ConverterError> {
    match source {
        SourceValue::Text(value) => Ok(value),
        other => type_mismatch(mapping, column, source_kind(other)),
    }
}

fn type_mismatch<T>(
    mapping: &TableMapping,
    column: &TransformColumn,
    actual: &'static str,
) -> Result<T, ConverterError> {
    Err(ConverterError::TypeMismatch {
        table: mapping.source.to_string(),
        column: column.source.to_string(),
        actual,
    })
}

fn source_kind(value: &SourceValue) -> &'static str {
    match value {
        SourceValue::Null => "null",
        SourceValue::I64(_) => "i64",
        SourceValue::U64(_) => "u64",
        SourceValue::Decimal(_) => "decimal",
        SourceValue::Text(_) => "text",
        SourceValue::Bytes(_) => "bytes",
    }
}

fn parse_canonical_json(
    mapping: &TableMapping,
    column: &TransformColumn,
    text: &str,
) -> Result<CanonicalJson, ConverterError> {
    CanonicalJson::parse(text).map_err(|message| ConverterError::InvalidJson {
        table: mapping.source.to_string(),
        column: column.source.to_string(),
        message,
    })
}

fn json_shape_matches(value: &CanonicalJson, shape: JsonShape) -> bool {
    match shape {
        JsonShape::Any => true,
        JsonShape::Array => value.is_array(),
    }
}

pub(super) fn normalize_id_array(
    mapping: &TableMapping,
    column: &TransformColumn,
    text: &str,
    maximum: u64,
    require_non_empty: bool,
) -> Result<Value, ConverterError> {
    let value: Value = serde_json::from_str(text).map_err(|error| ConverterError::InvalidJson {
        table: mapping.source.to_string(),
        column: column.source.to_string(),
        message: error.to_string(),
    })?;
    let members = value
        .as_array()
        .ok_or_else(|| ConverterError::InvalidJson {
            table: mapping.source.to_string(),
            column: column.source.to_string(),
            message: "top-level value is not an array".to_string(),
        })?;
    if require_non_empty && members.is_empty() {
        return Err(ConverterError::InvalidJson {
            table: mapping.source.to_string(),
            column: column.source.to_string(),
            message: "array must not be empty".to_string(),
        });
    }
    let mut normalized = Vec::with_capacity(members.len());
    for (index, member) in members.iter().enumerate() {
        let id = match member {
            Value::Number(number) => number.as_u64(),
            Value::String(text) if canonical_positive_decimal(text) => text.parse::<u64>().ok(),
            _ => None,
        }
        .filter(|id| *id > 0 && *id <= maximum)
        .ok_or_else(|| ConverterError::InvalidIdArray {
            table: mapping.source.to_string(),
            column: column.source.to_string(),
            index,
        })?;
        normalized.push(Value::Number(Number::from(id)));
    }
    Ok(Value::Array(normalized))
}

fn canonical_positive_decimal(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && (b'1'..=b'9').contains(&bytes[0])
        && bytes[1..].iter().all(u8::is_ascii_digit)
}

pub(super) fn normalize_decimal(value: &str) -> Option<String> {
    if value.is_empty()
        || value.starts_with('+')
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    if unsigned.is_empty() || unsigned.starts_with('+') {
        return None;
    }
    let split = unsigned.split_once('.');
    let (integer, fraction) = split.unwrap_or((unsigned, ""));
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || unsigned.matches('.').count() > 1
        || split.is_some_and(|_| fraction.is_empty())
    {
        return None;
    }
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.trim_end_matches('0');
    let is_zero = integer == "0" && fraction.is_empty();
    let sign = if negative && !is_zero { "-" } else { "" };
    if fraction.is_empty() {
        Some(format!("{sign}{integer}"))
    } else {
        Some(format!("{sign}{integer}.{fraction}"))
    }
}

/// Expands legacy set-valued redemption ids without inventing a historical
/// timestamp. The PostgreSQL baseline explicitly reserves `(0,
/// "legacy_unknown")` for this representation. Output is sorted and deduped;
/// malformed or missing users fail closed.
pub fn expand_giftcard_redemptions(
    giftcard_id: i32,
    used_user_ids: &SourceValue,
) -> Result<Vec<LegacyGiftcardRedemptionRow>, ConverterError> {
    if matches!(used_user_ids, SourceValue::Null) {
        return Ok(Vec::new());
    }
    let mapping = mapping_for_source("v2_giftcard")
        .ok_or_else(|| ConverterError::UnknownTable("v2_giftcard".to_string()))?;
    let column = TransformColumn {
        source: "used_user_ids",
        target: "user_id",
        rule: ColumnRule::PositiveIdArray {
            maximum: i64::MAX as u64,
            require_non_empty: false,
            output: JsonOutput::Json,
        },
        source_referenced_table: Some("v2_user"),
        referenced_target_table: Some("users"),
    };
    let text = source_text(mapping, &column, used_user_ids)?;
    let normalized = normalize_id_array(mapping, &column, text, i64::MAX as u64, false)?;
    let members = normalized.as_array().ok_or_else(|| {
        ConverterError::Registry("id-array normalizer returned a non-array".into())
    })?;
    let ids = members
        .iter()
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                ConverterError::Registry("id-array normalizer returned a non-u64".into())
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut rows = Vec::with_capacity(ids.len());
    for user_id in &ids {
        rows.push(LegacyGiftcardRedemptionRow {
            giftcard_id,
            user_id: i64::try_from(*user_id).map_err(|_| {
                ConverterError::Registry("id-array normalizer exceeded i64::MAX".into())
            })?,
            created_at: 0,
            created_at_provenance: "legacy_unknown".to_string(),
        });
    }
    Ok(rows)
}
