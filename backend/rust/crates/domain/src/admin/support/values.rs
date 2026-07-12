use super::*;

#[derive(Debug, Clone)]
pub(in super::super) enum AdminSqlValue {
    Null,
    Integer(i64),
    Text(String),
}

pub(in super::super) fn push_admin_sql_value(
    separated: &mut sqlx::query_builder::Separated<'_, MySql, &str>,
    value: &AdminSqlValue,
) {
    match value {
        AdminSqlValue::Null => {
            separated.push_bind(Option::<String>::None);
        }
        AdminSqlValue::Integer(value) => {
            separated.push_bind(*value);
        }
        AdminSqlValue::Text(value) => {
            separated.push_bind(value.clone());
        }
    }
}

pub(in super::super) fn push_admin_sql_bind(
    builder: &mut QueryBuilder<MySql>,
    value: &AdminSqlValue,
) {
    match value {
        AdminSqlValue::Null => {
            builder.push_bind(Option::<String>::None);
        }
        AdminSqlValue::Integer(value) => {
            builder.push_bind(*value);
        }
        AdminSqlValue::Text(value) => {
            builder.push_bind(value.clone());
        }
    }
}

pub(in super::super) fn text_value(value: String) -> AdminSqlValue {
    AdminSqlValue::Text(value)
}

/// Validated coupon columns (excluding `code`) present in a generate request.
/// Mirrors CouponGenerate rules; used for both single create/update and the
/// per-row inserts of multiGenerate.
pub(in super::super) fn coupon_field_values(
    params: &HashMap<String, String>,
) -> Vec<(&'static str, AdminSqlValue)> {
    let mut values = Vec::new();
    if params.contains_key("name") {
        values.push(("name", optional_text_value(params, "name")));
    }
    for key in [
        "type",
        "value",
        "started_at",
        "ended_at",
        "limit_use",
        "limit_use_with_user",
    ] {
        if params.contains_key(key) {
            values.push((key, optional_int_or_null_value(params, key)));
        }
    }
    for key in ["limit_plan_ids", "limit_period"] {
        if params
            .keys()
            .any(|param| param == key || param.starts_with(&format!("{key}[")))
        {
            values.push((key, optional_json_array_text_value(params, key)));
        }
    }
    values
}

/// Validated giftcard columns (excluding `code`) present in a generate request.
pub(in super::super) fn giftcard_field_values(
    params: &HashMap<String, String>,
) -> Vec<(&'static str, AdminSqlValue)> {
    let mut values = Vec::new();
    if params.contains_key("name") {
        values.push(("name", optional_text_value(params, "name")));
    }
    for key in [
        "type",
        "value",
        "plan_id",
        "started_at",
        "ended_at",
        "limit_use",
    ] {
        if params.contains_key(key) {
            values.push((key, optional_int_or_null_value(params, key)));
        }
    }
    values
}

/// Joins a reconstructed array param with `/` for CSV display, or returns the
/// localized "unlimited" placeholder when the param was not supplied.
pub(in super::super) fn joined_array_display(
    params: &HashMap<String, String>,
    key: &str,
) -> String {
    let present = params
        .keys()
        .any(|param| param == key || param.starts_with(&format!("{key}[")));
    if !present {
        return "不限制".to_string();
    }
    json_array_param(params, key)
        .iter()
        .map(|value| match value {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(in super::super) fn optional_text(value: Option<String>) -> AdminSqlValue {
    value
        .filter(|value| !value.trim().is_empty() && !value.eq_ignore_ascii_case("null"))
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::Null)
}

pub(in super::super) fn optional_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    optional_text(optional_string(params, key))
}

pub(in super::super) fn optional_int_value(
    params: &HashMap<String, String>,
    key: &str,
    default: i64,
) -> AdminSqlValue {
    AdminSqlValue::Integer(optional_i64(params, key).unwrap_or(default))
}

pub(in super::super) fn optional_int_or_null_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    optional_i64(params, key)
        .map(AdminSqlValue::Integer)
        .unwrap_or(AdminSqlValue::Null)
}

/// Builds a Laravel-style 422 validation error for a single field: the message
/// doubles as the top-level message and the field's first error.
pub(in super::super) fn validation_error(field: &str, message: &str) -> ApiError {
    ApiError::validation(
        message,
        HashMap::from([(field.to_string(), vec![message.to_string()])]),
    )
}

/// A scalar request value trimmed of surrounding whitespace (Laravel's global
/// `TrimStrings` middleware), yielding `None` when the key is absent or the
/// value is empty after trimming. This is the presence test Laravel's
/// `required`/`nullable`/`integer` rules operate on — note it does NOT treat the
/// literal string `"null"` as empty (unlike `optional_string`), because Laravel
/// does not either.
pub(in super::super) fn present_value<'a>(
    params: &'a HashMap<String, String>,
    key: &str,
) -> Option<&'a str> {
    params
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// True when `key` (scalar or bracketed array) appears in the request params.
/// Mirrors Laravel's `required` presence check for nested inputs.
pub(in super::super) fn param_present(params: &HashMap<String, String>, key: &str) -> bool {
    params
        .keys()
        .any(|param| param == key || param.starts_with(&format!("{key}[")))
}

/// Approximates Laravel's `url` rule (filter_var FILTER_VALIDATE_URL): requires a
/// `scheme://host` shape with an alphabetic-led scheme and a non-empty host.
pub(in super::super) fn is_valid_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    let scheme_bytes = scheme.as_bytes();
    if scheme_bytes.is_empty()
        || !scheme_bytes[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty()
}

pub(in super::super) fn optional_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Option<String> {
    params
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
}

pub(in super::super) fn required_json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<String, ApiError> {
    json_array_string(params, key)?.ok_or_else(|| ApiError::legacy("参数有误"))
}

pub(in super::super) fn optional_json_array_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    json_array_string(params, key)
        .ok()
        .flatten()
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::Null)
}

pub(in super::super) fn json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<String>, ApiError> {
    if let Some(value) = optional_string(params, key) {
        if serde_json::from_str::<Value>(&value).is_ok() {
            return Ok(Some(value));
        }
        return Ok(Some(json_string(&Value::Array(vec![json_scalar(&value)]))));
    }
    let values = json_array_param(params, key);
    Ok((!values.is_empty()).then(|| json_string(&Value::Array(values))))
}

pub(in super::super) fn optional_json_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    optional_json_value(params, key)
        .map(json_value)
        .unwrap_or(AdminSqlValue::Null)
}

pub(in super::super) fn optional_decoded_json_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    let Some(value) = optional_string(params, key) else {
        return optional_json_text_value(params, key);
    };
    serde_json::from_str::<Value>(&value)
        .map(json_value)
        .unwrap_or(AdminSqlValue::Null)
}

pub(in super::super) fn optional_json_value(
    params: &HashMap<String, String>,
    key: &str,
) -> Option<Value> {
    if let Some(value) = optional_string(params, key)
        && let Ok(parsed) = serde_json::from_str::<Value>(&value)
    {
        return Some(parsed);
    }
    let value = nested_json(params, key);
    match &value {
        Value::Object(object) if object.is_empty() => None,
        _ => Some(value),
    }
}

pub(in super::super) fn json_value(value: Value) -> AdminSqlValue {
    AdminSqlValue::Text(json_string(&value))
}
