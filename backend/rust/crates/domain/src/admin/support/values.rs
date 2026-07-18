use super::*;

#[derive(Debug, Clone)]
pub(in super::super) enum AdminSqlValue {
    TextNull,
    IntegerNull,
    Integer(i64),
    Text(String),
    Json(Option<Value>),
}

#[derive(Clone, Copy)]
enum AdminIntegerColumnType {
    SmallInt,
    Integer,
    BigInt,
}

/// `AdminSqlValue` deliberately keeps request integers as `i64`: validation and
/// compatibility logic operate on Laravel-style integers, while the PostgreSQL
/// schema uses a mixture of SMALLINT, INTEGER, and BIGINT. PostgreSQL does not
/// implicitly narrow a BIGINT bind for INSERT/UPDATE assignment, so every
/// dynamic integer assignment must carry the target column's exact SQL type.
fn admin_integer_column_type(column: &str) -> AdminIntegerColumnType {
    match column {
        "allow_insecure"
        | "auto_renewal"
        | "banned"
        | "commission_type"
        | "disable_sni"
        | "insecure"
        | "is_admin"
        | "is_staff"
        | "remind_expire"
        | "remind_traffic"
        | "renew"
        | "reset_traffic_method"
        | "show"
        | "tls"
        | "type"
        | "zero_rtt_handshake" => AdminIntegerColumnType::SmallInt,
        "balance"
        | "capacity_limit"
        | "commission_balance"
        | "commission_rate"
        | "device_limit"
        | "discount"
        | "down_mbps"
        | "group_id"
        | "half_year_price"
        | "limit_use"
        | "limit_use_with_user"
        | "month_price"
        | "onetime_price"
        | "parent_id"
        | "plan_id"
        | "port"
        | "quarter_price"
        | "reset_price"
        | "server_port"
        | "sort"
        | "speed_limit"
        | "three_year_price"
        | "two_year_price"
        | "up_mbps"
        | "value"
        | "version"
        | "year_price" => AdminIntegerColumnType::Integer,
        _ => AdminIntegerColumnType::BigInt,
    }
}

fn push_admin_integer_value(
    separated: &mut sqlx::query_builder::Separated<'_, Postgres, &str>,
    column: &str,
    value: Option<i64>,
) {
    let cast = match admin_integer_column_type(column) {
        AdminIntegerColumnType::SmallInt => " AS SMALLINT)",
        AdminIntegerColumnType::Integer => " AS INTEGER)",
        AdminIntegerColumnType::BigInt => " AS BIGINT)",
    };
    separated.push("CAST(");
    separated.push_bind_unseparated(value);
    separated.push_unseparated(cast);
}

fn push_admin_integer_bind(builder: &mut QueryBuilder<Postgres>, column: &str, value: Option<i64>) {
    let cast = match admin_integer_column_type(column) {
        AdminIntegerColumnType::SmallInt => " AS SMALLINT)",
        AdminIntegerColumnType::Integer => " AS INTEGER)",
        AdminIntegerColumnType::BigInt => " AS BIGINT)",
    };
    builder.push("CAST(");
    builder.push_bind(value);
    builder.push(cast);
}

pub(in super::super) fn push_admin_sql_value(
    separated: &mut sqlx::query_builder::Separated<'_, Postgres, &str>,
    column: &str,
    value: &AdminSqlValue,
) {
    match value {
        AdminSqlValue::TextNull => {
            separated.push_bind(Option::<String>::None);
        }
        AdminSqlValue::IntegerNull => {
            push_admin_integer_value(separated, column, None);
        }
        AdminSqlValue::Integer(value) => {
            push_admin_integer_value(separated, column, Some(*value));
        }
        AdminSqlValue::Text(value) => {
            separated.push_bind(value.clone());
        }
        AdminSqlValue::Json(value) => {
            separated.push_bind(value.clone().map(Json));
        }
    }
}

pub(in super::super) fn push_admin_sql_bind(
    builder: &mut QueryBuilder<Postgres>,
    column: &str,
    value: &AdminSqlValue,
) {
    match value {
        AdminSqlValue::TextNull => {
            builder.push_bind(Option::<String>::None);
        }
        AdminSqlValue::IntegerNull => {
            push_admin_integer_bind(builder, column, None);
        }
        AdminSqlValue::Integer(value) => {
            push_admin_integer_bind(builder, column, Some(*value));
        }
        AdminSqlValue::Text(value) => {
            builder.push_bind(value.clone());
        }
        AdminSqlValue::Json(value) => {
            builder.push_bind(value.clone().map(Json));
        }
    }
}

pub(in super::super) fn text_value(value: String) -> AdminSqlValue {
    AdminSqlValue::Text(value)
}

pub(in super::super) fn optional_text(value: Option<String>) -> AdminSqlValue {
    value
        .filter(|value| !value.trim().is_empty() && !value.eq_ignore_ascii_case("null"))
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::TextNull)
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
        .unwrap_or(AdminSqlValue::IntegerNull)
}

/// Builds a Laravel-style 422 validation error for a single field: the message
/// doubles as the top-level message and the field's first error.
pub(in super::super) fn validation_error(field: &str, message: &str) -> ApiError {
    ApiError::validation_field(field, message)
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
    json_array_string(params, key)?.ok_or_else(|| ApiError::business("参数有误"))
}

pub(in super::super) fn optional_json_array_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    AdminSqlValue::Json(
        json_array_string(params, key)
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_str(&value).ok()),
    )
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
    AdminSqlValue::Json(optional_json_value(params, key))
}

pub(in super::super) fn optional_decoded_json_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    let Some(value) = optional_string(params, key) else {
        return optional_json_text_value(params, key);
    };
    AdminSqlValue::Json(serde_json::from_str::<Value>(&value).ok())
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
    AdminSqlValue::Json(Some(value))
}

#[cfg(test)]
mod postgres_integer_bind_tests {
    use super::*;

    #[test]
    fn dynamic_values_cast_request_i64_to_exact_postgres_column_types() {
        let mut builder = QueryBuilder::<Postgres>::new("VALUES (");
        {
            let mut values = builder.separated(", ");
            push_admin_sql_value(&mut values, "show", &AdminSqlValue::Integer(1));
            push_admin_sql_value(&mut values, "plan_id", &AdminSqlValue::Integer(2));
            push_admin_sql_value(&mut values, "expired_at", &AdminSqlValue::IntegerNull);
        }
        builder.push(")");
        assert_eq!(
            builder.sql(),
            "VALUES (CAST($1 AS SMALLINT), CAST($2 AS INTEGER), CAST($3 AS BIGINT))"
        );
    }

    #[test]
    fn dynamic_assignments_use_the_same_exact_integer_casts() {
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE users SET banned = ");
        push_admin_sql_bind(&mut builder, "banned", &AdminSqlValue::Integer(1));
        builder.push(", balance = ");
        push_admin_sql_bind(&mut builder, "balance", &AdminSqlValue::Integer(10));
        assert_eq!(
            builder.sql(),
            "UPDATE users SET banned = CAST($1 AS SMALLINT), balance = CAST($2 AS INTEGER)"
        );
    }
}
