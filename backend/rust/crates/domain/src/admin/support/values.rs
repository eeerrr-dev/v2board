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

/// Builds a Laravel-style 422 validation error for a single field: the message
/// doubles as the top-level message and the field's first error.
pub(in super::super) fn validation_error(field: &str, message: &str) -> ApiError {
    ApiError::from(Problem::validation_field(field, message))
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
