//! Admin filter & sort DSL (docs/api-dialect.md §7). Ships in W9 with its
//! first consumer (`GET system/logs`); W11/W12 reuse it for orders/users.
//!
//! Query-borne filters arrive as one URL-encoded JSON clause array in the
//! `filter` parameter; body-borne filters (§6.6 bulk actions) carry the same
//! clause array unencoded. Clauses are AND-combined. Every failure mode —
//! unparsable JSON, unknown `field`, unknown `op`, type-mismatched `value` —
//! is a 422 `validation_failed` problem with `errors: {"filter": [reason]}`.
//! The SQL builder only ever binds values; column expressions come from the
//! per-endpoint whitelist, never from the request.

use serde::{Deserialize, Serialize};
use serde_json::Number;
use sqlx::{Postgres, QueryBuilder};
use v2board_compat::{ApiError, Problem};

/// One clause of the §7.1 filter DSL: `{"field": ..., "op": ..., "value": ...}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilterClause {
    pub field: String,
    pub op: FilterOp,
    pub value: FilterValue,
}

/// The closed §7.1 operator vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterOp {
    Eq,
    Neq,
    Like,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
}

/// The bounded §7.1 value domain: scalars, null, or an array of scalars.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FilterValue {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<ScalarFilterValue>),
}

/// Array elements for the `in` operator — scalars only, never null/nested.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ScalarFilterValue {
    Bool(bool),
    Number(Number),
    String(String),
}

/// Column type driving `value` coercion for a whitelisted filter field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    Text,
    /// Text with the legacy trimmed/lowercased equality comparison (§7.1 `eq`).
    Email,
    Integer,
    Boolean,
    /// Stored epoch seconds; request values are RFC 3339 strings (§7.1).
    Timestamp,
}

/// One whitelisted filter field: the public `field` name, the SQL lvalue it
/// resolves to, and the column type for value coercion.
#[derive(Debug, Clone, Copy)]
pub struct FilterColumn {
    pub field: &'static str,
    pub expr: &'static str,
    pub kind: ColumnKind,
}

/// One whitelisted `sort_by` field and the SQL expression it orders by.
#[derive(Debug, Clone, Copy)]
pub struct SortColumn {
    pub field: &'static str,
    pub expr: &'static str,
}

/// A validated §7.2 sort: whitelisted expression plus direction.
#[derive(Debug, Clone, Copy)]
pub struct SortSpec {
    pub expr: &'static str,
    pub descending: bool,
}

impl SortSpec {
    /// `ORDER BY` clause body with the legacy NULLS pinning (MySQL orders
    /// NULL first for ASC and last for DESC; PostgreSQL defaults are the
    /// reverse, so the list contract is pinned explicitly).
    pub fn order_by(&self) -> String {
        if self.descending {
            format!("{} DESC NULLS LAST", self.expr)
        } else {
            format!("{} ASC NULLS FIRST", self.expr)
        }
    }
}

/// A whitelist-resolved, type-coerced clause ready for SQL building.
#[derive(Debug, Clone)]
pub enum ResolvedFilter {
    IsNull {
        expr: &'static str,
        negated: bool,
    },
    CompareInt {
        expr: &'static str,
        op: &'static str,
        value: i64,
    },
    CompareText {
        expr: &'static str,
        op: &'static str,
        value: String,
        email: bool,
    },
    CompareBool {
        expr: &'static str,
        op: &'static str,
        value: bool,
    },
    Like {
        expr: &'static str,
        pattern: String,
        cast_text: bool,
    },
    InInt {
        expr: &'static str,
        values: Vec<i64>,
    },
    InText {
        expr: &'static str,
        values: Vec<String>,
        email: bool,
    },
    InBool {
        expr: &'static str,
        values: Vec<bool>,
    },
}

fn filter_error(reason: impl Into<String>) -> ApiError {
    Problem::validation_field("filter", reason).into()
}

/// Parses the query-borne `filter` parameter: a JSON array of clauses.
/// Unparsable JSON or a non-clause shape is the §7.1 422.
pub fn parse_filter_param(raw: &str) -> Result<Vec<FilterClause>, ApiError> {
    serde_json::from_str::<Vec<FilterClause>>(raw)
        .map_err(|error| filter_error(format!("filter must be a JSON clause array: {error}")))
}

/// Resolves parsed clauses against an endpoint whitelist, coercing each
/// `value` to its column type (§7.1 second-pass validation).
pub fn resolve_filters(
    clauses: &[FilterClause],
    columns: &[FilterColumn],
) -> Result<Vec<ResolvedFilter>, ApiError> {
    clauses
        .iter()
        .map(|clause| resolve_clause(clause, columns))
        .collect()
}

/// Resolves the §7.2 `sort_by`/`sort_dir` pair. Defaults: `created_at`,
/// `desc`. Invalid values are 422s (the legacy silent fallback is retired).
pub fn resolve_sort(
    sort_by: Option<&str>,
    sort_dir: Option<&str>,
    columns: &[SortColumn],
) -> Result<SortSpec, ApiError> {
    debug_assert!(
        columns.iter().any(|column| column.field == "created_at"),
        "every sort whitelist must contain the default created_at"
    );
    let field = sort_by.unwrap_or("created_at");
    let expr = columns
        .iter()
        .find(|column| column.field == field)
        .map(|column| column.expr)
        .ok_or_else(|| {
            ApiError::from(Problem::validation_field(
                "sort_by",
                format!("sort_by field {field} is not sortable"),
            ))
        })?;
    let descending = match sort_dir {
        None | Some("desc") => true,
        Some("asc") => false,
        Some(other) => {
            return Err(Problem::validation_field(
                "sort_dir",
                format!("sort_dir must be asc or desc, got {other}"),
            )
            .into());
        }
    };
    Ok(SortSpec { expr, descending })
}

/// Escapes `%`, `_`, and `\` so a `like` value matches itself literally
/// (§7.1 recorded divergence from the legacy unescaped `%{value}%` bind).
pub fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    for ch in value.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Appends ` AND <clause>` for every resolved filter, binding all values.
pub fn push_filter_where(builder: &mut QueryBuilder<Postgres>, filters: &[ResolvedFilter]) {
    for filter in filters {
        builder.push(" AND ");
        match filter {
            ResolvedFilter::IsNull { expr, negated } => {
                builder.push(format!(
                    "{expr} IS {}NULL",
                    if *negated { "NOT " } else { "" }
                ));
            }
            ResolvedFilter::CompareInt { expr, op, value } => {
                builder.push(format!("{expr} {op} "));
                builder.push_bind(*value);
            }
            ResolvedFilter::CompareBool { expr, op, value } => {
                builder.push(format!("{expr} {op} "));
                builder.push_bind(*value);
            }
            ResolvedFilter::CompareText {
                expr,
                op,
                value,
                email,
            } => {
                if *email {
                    builder.push(format!("lower(btrim({expr})) {op} lower(btrim("));
                    builder.push_bind(value.clone());
                    builder.push("))");
                } else {
                    builder.push(format!("{expr} {op} "));
                    builder.push_bind(value.clone());
                }
            }
            ResolvedFilter::Like {
                expr,
                pattern,
                cast_text,
            } => {
                let cast = if *cast_text { "::text" } else { "" };
                builder.push(format!("{expr}{cast} ILIKE "));
                builder.push_bind(pattern.clone());
            }
            ResolvedFilter::InInt { expr, values } => {
                builder.push(format!("{expr} = ANY("));
                builder.push_bind(values.clone());
                builder.push(")");
            }
            ResolvedFilter::InBool { expr, values } => {
                builder.push(format!("{expr} = ANY("));
                builder.push_bind(values.clone());
                builder.push(")");
            }
            ResolvedFilter::InText {
                expr,
                values,
                email,
            } => {
                if *email {
                    let values = values
                        .iter()
                        .map(|value| value.trim().to_lowercase())
                        .collect::<Vec<_>>();
                    builder.push(format!("lower(btrim({expr})) = ANY("));
                    builder.push_bind(values);
                } else {
                    builder.push(format!("{expr} = ANY("));
                    builder.push_bind(values.clone());
                }
                builder.push(")");
            }
        }
    }
}

fn resolve_clause(
    clause: &FilterClause,
    columns: &[FilterColumn],
) -> Result<ResolvedFilter, ApiError> {
    let column = columns
        .iter()
        .find(|column| column.field == clause.field)
        .ok_or_else(|| filter_error(format!("field {} is not filterable", clause.field)))?;
    let field = column.field;
    let expr = column.expr;
    match clause.op {
        FilterOp::Eq | FilterOp::Neq => {
            let negated = clause.op == FilterOp::Neq;
            let op = if negated { "<>" } else { "=" };
            match &clause.value {
                FilterValue::Null => Ok(ResolvedFilter::IsNull { expr, negated }),
                value => resolve_equality(field, expr, op, column.kind, value),
            }
        }
        FilterOp::Like => {
            let FilterValue::String(value) = &clause.value else {
                return Err(filter_error(format!(
                    "like on {field} requires a string value"
                )));
            };
            // Integer columns keep the legacy `::text` substring search;
            // boolean/timestamp columns have no meaningful substring form.
            let cast_text = match column.kind {
                ColumnKind::Text | ColumnKind::Email => false,
                ColumnKind::Integer => true,
                ColumnKind::Boolean | ColumnKind::Timestamp => {
                    return Err(filter_error(format!("like is not supported on {field}")));
                }
            };
            Ok(ResolvedFilter::Like {
                expr,
                pattern: format!("%{}%", escape_like_pattern(value)),
                cast_text,
            })
        }
        FilterOp::Gt | FilterOp::Gte | FilterOp::Lt | FilterOp::Lte => {
            let op = match clause.op {
                FilterOp::Gt => ">",
                FilterOp::Gte => ">=",
                FilterOp::Lt => "<",
                _ => "<=",
            };
            let value = match (column.kind, &clause.value) {
                (ColumnKind::Integer, FilterValue::Number(number)) => integer_value(field, number)?,
                (ColumnKind::Timestamp, FilterValue::String(value)) => {
                    timestamp_value(field, value)?
                }
                _ => {
                    return Err(filter_error(format!(
                        "{} on {field} requires a {} value",
                        op_name(clause.op),
                        range_value_name(column.kind, field)?
                    )));
                }
            };
            Ok(ResolvedFilter::CompareInt { expr, op, value })
        }
        FilterOp::In => {
            let FilterValue::Array(values) = &clause.value else {
                return Err(filter_error(format!(
                    "in on {field} requires a non-empty array value"
                )));
            };
            if values.is_empty() {
                return Err(filter_error(format!(
                    "in on {field} requires a non-empty array value"
                )));
            }
            resolve_in(field, expr, column.kind, values)
        }
    }
}

fn resolve_equality(
    field: &str,
    expr: &'static str,
    op: &'static str,
    kind: ColumnKind,
    value: &FilterValue,
) -> Result<ResolvedFilter, ApiError> {
    match (kind, value) {
        (ColumnKind::Integer, FilterValue::Number(number)) => Ok(ResolvedFilter::CompareInt {
            expr,
            op,
            value: integer_value(field, number)?,
        }),
        (ColumnKind::Timestamp, FilterValue::String(value)) => Ok(ResolvedFilter::CompareInt {
            expr,
            op,
            value: timestamp_value(field, value)?,
        }),
        (ColumnKind::Boolean, FilterValue::Bool(value)) => Ok(ResolvedFilter::CompareBool {
            expr,
            op,
            value: *value,
        }),
        (ColumnKind::Text, FilterValue::String(value)) => Ok(ResolvedFilter::CompareText {
            expr,
            op,
            value: value.clone(),
            email: false,
        }),
        (ColumnKind::Email, FilterValue::String(value)) => Ok(ResolvedFilter::CompareText {
            expr,
            op,
            value: value.clone(),
            email: true,
        }),
        _ => Err(filter_error(format!(
            "{} on {field} requires a {} value",
            if op == "=" { "eq" } else { "neq" },
            scalar_value_name(kind)
        ))),
    }
}

fn resolve_in(
    field: &str,
    expr: &'static str,
    kind: ColumnKind,
    values: &[ScalarFilterValue],
) -> Result<ResolvedFilter, ApiError> {
    match kind {
        ColumnKind::Integer => {
            let values = values
                .iter()
                .map(|value| match value {
                    ScalarFilterValue::Number(number) => integer_value(field, number),
                    _ => Err(filter_error(format!(
                        "in on {field} requires number array elements"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResolvedFilter::InInt { expr, values })
        }
        ColumnKind::Timestamp => {
            let values = values
                .iter()
                .map(|value| match value {
                    ScalarFilterValue::String(value) => timestamp_value(field, value),
                    _ => Err(filter_error(format!(
                        "in on {field} requires RFC 3339 array elements"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResolvedFilter::InInt { expr, values })
        }
        ColumnKind::Boolean => {
            let values = values
                .iter()
                .map(|value| match value {
                    ScalarFilterValue::Bool(value) => Ok(*value),
                    _ => Err(filter_error(format!(
                        "in on {field} requires boolean array elements"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResolvedFilter::InBool { expr, values })
        }
        ColumnKind::Text | ColumnKind::Email => {
            let strings = values
                .iter()
                .map(|value| match value {
                    ScalarFilterValue::String(value) => Ok(value.clone()),
                    _ => Err(filter_error(format!(
                        "in on {field} requires string array elements"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResolvedFilter::InText {
                expr,
                values: strings,
                email: kind == ColumnKind::Email,
            })
        }
    }
}

fn integer_value(field: &str, number: &Number) -> Result<i64, ApiError> {
    number
        .as_i64()
        .ok_or_else(|| filter_error(format!("{field} requires an integer value")))
}

fn timestamp_value(field: &str, value: &str) -> Result<i64, ApiError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|instant| instant.timestamp())
        .map_err(|_| filter_error(format!("{field} requires an RFC 3339 timestamp value")))
}

fn op_name(op: FilterOp) -> &'static str {
    match op {
        FilterOp::Eq => "eq",
        FilterOp::Neq => "neq",
        FilterOp::Like => "like",
        FilterOp::Gt => "gt",
        FilterOp::Gte => "gte",
        FilterOp::Lt => "lt",
        FilterOp::Lte => "lte",
        FilterOp::In => "in",
    }
}

fn range_value_name(kind: ColumnKind, field: &str) -> Result<&'static str, ApiError> {
    match kind {
        ColumnKind::Integer => Ok("number"),
        ColumnKind::Timestamp => Ok("RFC 3339 timestamp"),
        _ => Err(filter_error(format!(
            "range comparison is not supported on {field}"
        ))),
    }
}

fn scalar_value_name(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Text | ColumnKind::Email => "string",
        ColumnKind::Integer => "number",
        ColumnKind::Boolean => "boolean",
        ColumnKind::Timestamp => "RFC 3339 timestamp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2board_compat::Code;

    const COLUMNS: &[FilterColumn] = &[
        FilterColumn {
            field: "email",
            expr: "u.email",
            kind: ColumnKind::Email,
        },
        FilterColumn {
            field: "level",
            expr: "level",
            kind: ColumnKind::Text,
        },
        FilterColumn {
            field: "plan_id",
            expr: "u.plan_id",
            kind: ColumnKind::Integer,
        },
        FilterColumn {
            field: "banned",
            expr: "u.banned",
            kind: ColumnKind::Boolean,
        },
        FilterColumn {
            field: "created_at",
            expr: "u.created_at",
            kind: ColumnKind::Timestamp,
        },
    ];
    const SORTS: &[SortColumn] = &[
        SortColumn {
            field: "created_at",
            expr: "u.created_at",
        },
        SortColumn {
            field: "level",
            expr: "level",
        },
    ];

    fn clauses(raw: &str) -> Vec<FilterClause> {
        parse_filter_param(raw).expect("parse filter fixture")
    }

    fn filter_reason(error: ApiError) -> String {
        // DSL failures are §7.1 422 `validation_failed` problems carrying
        // `errors: {"filter": [reason]}`.
        let ApiError::Problem(problem) = error else {
            panic!("expected a validation problem, got {error:?}");
        };
        assert_eq!(problem.code(), Code::ValidationFailed);
        problem
            .errors()
            .and_then(|errors| errors.get("filter"))
            .and_then(|messages| messages.first())
            .expect("errors bag carries the filter reason")
            .clone()
    }

    fn built_sql(filters: &[ResolvedFilter]) -> String {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT 1 FROM t WHERE 1 = 1");
        push_filter_where(&mut builder, filters);
        builder.sql().as_str().to_string()
    }

    #[test]
    fn whitelist_rejects_unknown_fields_ops_and_shapes() {
        // Unknown field.
        let error = resolve_filters(
            &clauses(r#"[{"field":"password","op":"eq","value":"x"}]"#),
            COLUMNS,
        )
        .unwrap_err();
        assert!(filter_reason(error).contains("password"));

        // Unknown op fails at parse time (closed serde enum).
        assert!(parse_filter_param(r#"[{"field":"email","op":"regex","value":"x"}]"#).is_err());
        // Unknown clause keys are 422s, not silent retains.
        assert!(parse_filter_param(r#"[{"field":"email","op":"eq","value":"x","x":1}]"#).is_err());
        // Unparsable JSON.
        assert!(parse_filter_param("not-json").is_err());
        // The legacy 'null' string sentinel is dead: it is just a string now.
        let resolved = resolve_filters(
            &clauses(r#"[{"field":"email","op":"eq","value":"null"}]"#),
            COLUMNS,
        )
        .unwrap();
        assert!(matches!(
            &resolved[0],
            ResolvedFilter::CompareText { value, .. } if value == "null"
        ));
    }

    #[test]
    fn value_types_are_coerced_per_column_kind() {
        let resolved = resolve_filters(
            &clauses(
                r#"[
                    {"field":"email","op":"eq","value":"  Golden@Example.Test "},
                    {"field":"plan_id","op":"eq","value":null},
                    {"field":"banned","op":"eq","value":true},
                    {"field":"plan_id","op":"in","value":[1,2,3]},
                    {"field":"created_at","op":"gte","value":"2023-11-14T22:13:20Z"}
                ]"#,
            ),
            COLUMNS,
        )
        .unwrap();
        assert!(matches!(
            &resolved[0],
            ResolvedFilter::CompareText {
                email: true,
                op: "=",
                ..
            }
        ));
        assert!(matches!(
            &resolved[1],
            ResolvedFilter::IsNull { negated: false, .. }
        ));
        assert!(matches!(
            &resolved[2],
            ResolvedFilter::CompareBool { value: true, .. }
        ));
        assert!(matches!(
            &resolved[3],
            ResolvedFilter::InInt { values, .. } if values == &vec![1, 2, 3]
        ));
        assert!(matches!(
            &resolved[4],
            ResolvedFilter::CompareInt {
                op: ">=",
                value: 1_700_000_000,
                ..
            }
        ));

        // Type mismatches are 422s.
        for raw in [
            r#"[{"field":"plan_id","op":"eq","value":"7"}]"#,
            r#"[{"field":"plan_id","op":"eq","value":1.5}]"#,
            r#"[{"field":"banned","op":"eq","value":1}]"#,
            r#"[{"field":"banned","op":"gt","value":1}]"#,
            r#"[{"field":"created_at","op":"lt","value":"yesterday"}]"#,
            r#"[{"field":"email","op":"in","value":[]}]"#,
            r#"[{"field":"plan_id","op":"in","value":["1"]}]"#,
            r#"[{"field":"email","op":"in","value":"x"}]"#,
        ] {
            assert!(
                resolve_filters(&clauses(raw), COLUMNS).is_err(),
                "expected 422 for {raw}"
            );
        }
    }

    #[test]
    fn like_escapes_wildcards_and_binds_a_literal_substring() {
        let resolved = resolve_filters(
            &clauses(r#"[{"field":"email","op":"like","value":"50%_a\\b"}]"#),
            COLUMNS,
        )
        .unwrap();
        let ResolvedFilter::Like {
            pattern, cast_text, ..
        } = &resolved[0]
        else {
            panic!("expected a like filter");
        };
        assert_eq!(pattern, "%50\\%\\_a\\\\b%");
        assert!(!cast_text);

        // Integer columns keep the legacy ::text substring search.
        let resolved = resolve_filters(
            &clauses(r#"[{"field":"plan_id","op":"like","value":"12"}]"#),
            COLUMNS,
        )
        .unwrap();
        assert!(matches!(
            &resolved[0],
            ResolvedFilter::Like {
                cast_text: true,
                ..
            }
        ));
        // No substring form for booleans/timestamps.
        assert!(
            resolve_filters(
                &clauses(r#"[{"field":"banned","op":"like","value":"1"}]"#),
                COLUMNS
            )
            .is_err()
        );
    }

    #[test]
    fn sql_builder_binds_every_value_and_never_interpolates() {
        let resolved = resolve_filters(
            &clauses(
                r#"[
                    {"field":"email","op":"eq","value":"a@b.test"},
                    {"field":"email","op":"like","value":"x"},
                    {"field":"plan_id","op":"neq","value":null},
                    {"field":"plan_id","op":"in","value":[1,2]},
                    {"field":"banned","op":"eq","value":false}
                ]"#,
            ),
            COLUMNS,
        )
        .unwrap();
        let sql = built_sql(&resolved);
        assert_eq!(
            sql,
            "SELECT 1 FROM t WHERE 1 = 1 AND lower(btrim(u.email)) = lower(btrim($1)) \
             AND u.email ILIKE $2 AND u.plan_id IS NOT NULL \
             AND u.plan_id = ANY($3) AND u.banned = $4"
        );
        assert!(!sql.contains("a@b.test"));
    }

    #[test]
    fn sort_defaults_and_rejects_unknown_values() {
        let sort = resolve_sort(None, None, SORTS).unwrap();
        assert_eq!(sort.expr, "u.created_at");
        assert!(sort.descending);
        assert_eq!(sort.order_by(), "u.created_at DESC NULLS LAST");

        let sort = resolve_sort(Some("level"), Some("asc"), SORTS).unwrap();
        assert_eq!(sort.order_by(), "level ASC NULLS FIRST");

        // Invalid values are 422s (legacy silently defaulted; §7.2 rejects).
        assert!(resolve_sort(Some("id"), None, SORTS).is_err());
        assert!(resolve_sort(None, Some("ASC"), SORTS).is_err());
        assert!(resolve_sort(None, Some("sideways"), SORTS).is_err());
    }
}
