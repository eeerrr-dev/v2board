//! Wire-to-application resolution for the admin filter DSL
//! (docs/api-dialect.md §7.1): the one place that turns the transport
//! `AdminFilterClause` array into validated, resource-typed
//! `FilterClause<F>`s — shared by every admin list/bulk-mutation endpoint
//! (users, orders, system/audit logs).
//!
//! Field parsing (`F::parse`), value-type coercion (driven by
//! `F::kind()`), and the operator/value validity gate
//! (`ColumnKind::accepts`) all happen here, immediately on the way in, so
//! every consumer gets the same 422 `validation_failed` behavior for the
//! same malformed input.

use v2board_api_contract::admin_business::{
    AdminFilterClause, AdminFilterNumber, AdminFilterOperator, AdminFilterScalar, AdminFilterValue,
};
use v2board_application::filter_dsl::{
    ColumnKind, FilterClause, FilterField, FilterOperator, FilterValue,
};
use v2board_compat::{ApiError, Problem};

fn filter_error(detail: impl Into<String>) -> ApiError {
    Problem::validation_field("filter", detail).into()
}

fn filter_number(value: AdminFilterNumber) -> Result<i64, ApiError> {
    match value {
        AdminFilterNumber::Integer(value) => Ok(value),
        AdminFilterNumber::Unsigned(value) => i64::try_from(value)
            .map_err(|_| filter_error("filter integer is outside the supported range")),
        AdminFilterNumber::Decimal(_) => Err(filter_error("filter value must be an integer")),
    }
}

fn timestamp_filter(field_name: &str, value: String) -> Result<i64, ApiError> {
    chrono::DateTime::parse_from_rfc3339(&value)
        .map(|instant| instant.timestamp())
        .map_err(|_| filter_error(format!("{field_name} requires an RFC 3339 timestamp value")))
}

/// Resolves one scalar wire value against `field`'s column kind. Every
/// scalar — bare or inside an `in` array — goes through this single
/// coercion, so (for example) a timestamp column only ever accepts an RFC
/// 3339 string, never a raw epoch integer, on every DSL consumer alike.
fn resolve_scalar<F: FilterField>(
    field: F,
    value: AdminFilterScalar,
) -> Result<FilterValue, ApiError> {
    match (field.kind(), value) {
        (ColumnKind::Boolean, AdminFilterScalar::Bool(value)) => Ok(FilterValue::Boolean(value)),
        (ColumnKind::Integer, AdminFilterScalar::Number(value)) => {
            filter_number(value).map(FilterValue::Integer)
        }
        (ColumnKind::Timestamp, AdminFilterScalar::String(value)) => {
            timestamp_filter(field.name(), value).map(FilterValue::Integer)
        }
        (ColumnKind::Text | ColumnKind::Email, AdminFilterScalar::String(value)) => {
            Ok(FilterValue::Text(value))
        }
        _ => Err(filter_error(format!(
            "filter value type does not match {}",
            field.name()
        ))),
    }
}

fn resolve_value<F: FilterField>(
    field: F,
    value: AdminFilterValue,
) -> Result<FilterValue, ApiError> {
    match value {
        AdminFilterValue::Null => Ok(FilterValue::Null),
        AdminFilterValue::Bool(value) => resolve_scalar(field, AdminFilterScalar::Bool(value)),
        AdminFilterValue::Number(value) => resolve_scalar(field, AdminFilterScalar::Number(value)),
        AdminFilterValue::String(value) => resolve_scalar(field, AdminFilterScalar::String(value)),
        AdminFilterValue::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| resolve_scalar(field, value))
                .collect::<Result<Vec<_>, _>>()?;
            match field.kind() {
                ColumnKind::Boolean => values
                    .into_iter()
                    .map(|value| match value {
                        FilterValue::Boolean(value) => Ok(value),
                        _ => unreachable!("resolve_scalar returns a boolean for boolean columns"),
                    })
                    .collect::<Result<Vec<_>, ApiError>>()
                    .map(FilterValue::Booleans),
                ColumnKind::Integer | ColumnKind::Timestamp => values
                    .into_iter()
                    .map(|value| match value {
                        FilterValue::Integer(value) => Ok(value),
                        _ => unreachable!(
                            "resolve_scalar returns an integer for number-like columns"
                        ),
                    })
                    .collect::<Result<Vec<_>, ApiError>>()
                    .map(FilterValue::Integers),
                ColumnKind::Text | ColumnKind::Email => values
                    .into_iter()
                    .map(|value| match value {
                        FilterValue::Text(value) => Ok(value),
                        _ => unreachable!("resolve_scalar returns text for text-like columns"),
                    })
                    .collect::<Result<Vec<_>, ApiError>>()
                    .map(FilterValue::Texts),
            }
        }
    }
}

const fn resolve_operator(value: AdminFilterOperator) -> FilterOperator {
    match value {
        AdminFilterOperator::Eq => FilterOperator::Eq,
        AdminFilterOperator::Neq => FilterOperator::Neq,
        AdminFilterOperator::Like => FilterOperator::Like,
        AdminFilterOperator::Gt => FilterOperator::Gt,
        AdminFilterOperator::Gte => FilterOperator::Gte,
        AdminFilterOperator::Lt => FilterOperator::Lt,
        AdminFilterOperator::Lte => FilterOperator::Lte,
        AdminFilterOperator::In => FilterOperator::In,
    }
}

/// Resolves one wire clause into a validated resource-typed filter clause:
/// parses `field` against `F`'s whitelist, coerces `value` to its column
/// kind, and rejects an operator/value combination the column kind does
/// not accept (§7.1) — the one type-check + validity gate every admin
/// filterable endpoint runs on the way in.
pub(super) fn resolve_filter_clause<F: FilterField>(
    clause: AdminFilterClause,
) -> Result<FilterClause<F>, ApiError> {
    let field = F::parse(&clause.field)
        .ok_or_else(|| filter_error(format!("field {} is not filterable", clause.field)))?;
    let value = resolve_value(field, clause.value)?;
    let operator = resolve_operator(clause.op);
    if !field.kind().accepts(operator, &value) {
        return Err(filter_error(format!(
            "operator/value combination is invalid for {}",
            field.name()
        )));
    }
    Ok(FilterClause {
        field,
        operator,
        value,
    })
}

/// Resolves an already-decoded wire clause array (e.g. a JSON request body)
/// into validated resource-typed clauses.
pub(super) fn resolve_filter_clauses<F: FilterField>(
    clauses: Vec<AdminFilterClause>,
) -> Result<Vec<FilterClause<F>>, ApiError> {
    clauses.into_iter().map(resolve_filter_clause).collect()
}

/// Parses the `filter=` query-string JSON array and resolves it, or an
/// empty clause list when the parameter is absent — the shared `GET` entry
/// point for every DSL consumer.
pub(super) fn parse_filter_query<F: FilterField>(
    raw: Option<&str>,
) -> Result<Vec<FilterClause<F>>, ApiError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let clauses = serde_json::from_str::<Vec<AdminFilterClause>>(raw)
        .map_err(|error| filter_error(format!("filter must be a JSON clause array: {error}")))?;
    resolve_filter_clauses(clauses)
}

#[cfg(test)]
mod tests {
    use v2board_application::admin_user::UserFilterField;

    use super::*;

    #[test]
    fn unknown_field_and_type_confusion_are_rejected() {
        assert!(
            resolve_filter_clause::<UserFilterField>(AdminFilterClause {
                field: "raw_sql".into(),
                op: AdminFilterOperator::Eq,
                value: AdminFilterValue::Number(AdminFilterNumber::Integer(1)),
            })
            .is_err()
        );
        assert!(
            resolve_filter_clause::<UserFilterField>(AdminFilterClause {
                field: "banned".into(),
                op: AdminFilterOperator::Like,
                value: AdminFilterValue::String("1".into()),
            })
            .is_err()
        );
    }

    #[test]
    fn timestamp_columns_require_an_rfc3339_string_not_a_raw_epoch_integer() {
        assert!(
            resolve_filter_clause::<UserFilterField>(AdminFilterClause {
                field: "created_at".into(),
                op: AdminFilterOperator::Eq,
                value: AdminFilterValue::Number(AdminFilterNumber::Integer(1_700_000_000)),
            })
            .is_err()
        );
        let clause = resolve_filter_clause::<UserFilterField>(AdminFilterClause {
            field: "created_at".into(),
            op: AdminFilterOperator::Gte,
            value: AdminFilterValue::String("2025-01-02T03:04:05Z".into()),
        })
        .expect("RFC 3339 timestamps resolve");
        assert_eq!(clause.value, FilterValue::Integer(1_735_787_045));
    }

    #[test]
    fn empty_in_arrays_are_rejected() {
        assert!(
            resolve_filter_clause::<UserFilterField>(AdminFilterClause {
                field: "id".into(),
                op: AdminFilterOperator::In,
                value: AdminFilterValue::Array(Vec::new()),
            })
            .is_err()
        );
    }
}
