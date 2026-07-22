//! The one generic operator-to-SQL translator every admin filterable
//! resource (users, orders, system/audit logs) renders its `filter=` DSL
//! through (docs/api-dialect.md §7.1). Each resource still owns its own
//! closed field enum and `field -> SQL column expression` mapping — that
//! mapping is the adapter half of the column registry described there; the
//! `application` crate's `filter_dsl::ColumnKind` is the other half (value
//! type / allowed-operator half). This module only knows how to turn an
//! already-validated `FilterClause<F>` slice into parameterized SQL.

use sqlx::{Postgres, QueryBuilder};
use v2board_application::filter_dsl::{
    ColumnKind, FilterClause, FilterField, FilterOperator, FilterValue, escape_like_pattern,
};

/// Appends every clause in `filters` to `builder` as `AND`-combined,
/// parameterized SQL. `column_sql` resolves a field to its code-owned SQL
/// column expression (e.g. `u.email`, `(u.banned <> 0)`); every bound value
/// goes through `push_bind`, never string interpolation.
///
/// Callers must have already run `application::filter_dsl::validate_filters`
/// (directly or through a resource service) — invalid operator/value
/// combinations reach the `unreachable!` branches here, matching the
/// defense-in-depth this replaces.
pub fn push_filters<F: FilterField>(
    builder: &mut QueryBuilder<Postgres>,
    filters: &[FilterClause<F>],
    column_sql: impl Fn(F) -> &'static str,
) {
    for filter in filters {
        builder.push(" AND ");
        push_clause(
            builder,
            column_sql(filter.field),
            filter.field.kind(),
            filter,
        );
    }
}

fn push_clause<F: FilterField>(
    builder: &mut QueryBuilder<Postgres>,
    expression: &'static str,
    kind: ColumnKind,
    filter: &FilterClause<F>,
) {
    match (filter.operator, &filter.value) {
        (FilterOperator::Eq, FilterValue::Null) => {
            builder.push(expression).push(" IS NULL");
        }
        (FilterOperator::Neq, FilterValue::Null) => {
            builder.push(expression).push(" IS NOT NULL");
        }
        (operator @ (FilterOperator::Eq | FilterOperator::Neq), value) => {
            let comparison = if operator == FilterOperator::Eq {
                " = "
            } else {
                " <> "
            };
            if kind == ColumnKind::Email {
                builder.push("lower(btrim(").push(expression).push("))");
                builder.push(comparison).push("lower(btrim(");
                if let FilterValue::Text(value) = value {
                    builder.push_bind(value.clone());
                }
                builder.push("))");
            } else {
                builder.push(expression).push(comparison);
                push_scalar_bind(builder, value);
            }
        }
        (FilterOperator::Like, FilterValue::Text(value)) => {
            builder.push(expression);
            if kind == ColumnKind::Integer {
                builder.push("::text");
            }
            builder
                .push(" ILIKE ")
                .push_bind(escape_like_pattern(value));
        }
        (
            operator @ (FilterOperator::Gt
            | FilterOperator::Gte
            | FilterOperator::Lt
            | FilterOperator::Lte),
            FilterValue::Integer(value),
        ) => {
            builder.push(expression).push(match operator {
                FilterOperator::Gt => " > ",
                FilterOperator::Gte => " >= ",
                FilterOperator::Lt => " < ",
                FilterOperator::Lte => " <= ",
                _ => unreachable!("range operators are exhaustively listed above"),
            });
            builder.push_bind(*value);
        }
        (FilterOperator::In, FilterValue::Integers(values)) => {
            builder
                .push(expression)
                .push(" = ANY(")
                .push_bind(values.clone())
                .push(")");
        }
        (FilterOperator::In, FilterValue::Booleans(values)) => {
            builder
                .push(expression)
                .push(" = ANY(")
                .push_bind(values.clone())
                .push(")");
        }
        (FilterOperator::In, FilterValue::Texts(values)) => {
            if kind == ColumnKind::Email {
                let values = values
                    .iter()
                    .map(|value| value.trim().to_lowercase())
                    .collect::<Vec<_>>();
                builder
                    .push("lower(btrim(")
                    .push(expression)
                    .push(")) = ANY(")
                    .push_bind(values)
                    .push(")");
            } else {
                builder
                    .push(expression)
                    .push(" = ANY(")
                    .push_bind(values.clone())
                    .push(")");
            }
        }
        _ => unreachable!(
            "application validation rejects invalid admin filter operator/value combinations"
        ),
    }
}

fn push_scalar_bind(builder: &mut QueryBuilder<Postgres>, value: &FilterValue) {
    match value {
        FilterValue::Boolean(value) => {
            builder.push_bind(*value);
        }
        FilterValue::Integer(value) => {
            builder.push_bind(*value);
        }
        FilterValue::Text(value) => {
            builder.push_bind(value.clone());
        }
        _ => unreachable!("application validation guarantees a scalar eq/neq value"),
    }
}
