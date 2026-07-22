//! Shared admin filter DSL (docs/api-dialect.md §7.1): the closed
//! operator/value vocabulary, a per-resource column-kind registry, and the
//! one generic validity check every filterable admin resource (users,
//! orders, system/audit logs) is built on.
//!
//! SQL rendering stays an outer-adapter concern (`v2board-db` owns the
//! generic translator that turns a validated `FilterClause<F>` slice into
//! parameterized SQL); this module only knows the closed vocabulary and
//! which operator/value combinations are legal for a column kind.

/// Closed operator vocabulary (§7.1), shared by every filterable resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterOperator {
    Eq,
    Neq,
    Like,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
}

/// Closed, already-typed value vocabulary a filter clause can bind. Scalar
/// and array variants mirror the wire's untagged `AdminFilterValue`, once
/// the clause's column kind has resolved which shape applies (RFC 3339
/// strings become epoch-seconds integers before reaching this type).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilterValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Text(String),
    Booleans(Vec<bool>),
    Integers(Vec<i64>),
    Texts(Vec<String>),
}

/// Storage/comparison kind for a filterable column. Governs both which
/// operator/value combinations are valid (`accepts`) and how the SQL
/// adapter renders the comparison (integer `::text` cast for `like`, the
/// trimmed/lowercased email comparison).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnKind {
    Boolean,
    Integer,
    /// Integer-typed but never `like`-searchable; the wire's RFC 3339
    /// string is resolved to epoch seconds ahead of this type.
    Timestamp,
    Text,
    /// Text compared with the legacy trimmed/lowercased semantics (email
    /// equality only; docs/api-dialect.md §7.1).
    Email,
}

impl ColumnKind {
    const fn is_textlike(self) -> bool {
        matches!(self, Self::Text | Self::Email)
    }

    const fn is_numberlike(self) -> bool {
        matches!(self, Self::Integer | Self::Timestamp)
    }

    /// True when `operator`/`value` is a legal combination for this column
    /// kind. Every SQL adapter can trust a clause that passes this check;
    /// the generic translator falls back to `unreachable!` on the rest.
    #[must_use]
    pub fn accepts(self, operator: FilterOperator, value: &FilterValue) -> bool {
        match operator {
            FilterOperator::Eq | FilterOperator::Neq => match value {
                FilterValue::Null => true,
                FilterValue::Boolean(_) => matches!(self, Self::Boolean),
                FilterValue::Integer(_) => self.is_numberlike(),
                FilterValue::Text(_) => self.is_textlike(),
                FilterValue::Booleans(_) | FilterValue::Integers(_) | FilterValue::Texts(_) => {
                    false
                }
            },
            FilterOperator::Like => {
                matches!(value, FilterValue::Text(_))
                    && (self.is_textlike() || matches!(self, Self::Integer))
            }
            FilterOperator::Gt | FilterOperator::Gte | FilterOperator::Lt | FilterOperator::Lte => {
                matches!(value, FilterValue::Integer(_)) && self.is_numberlike()
            }
            FilterOperator::In => match value {
                FilterValue::Booleans(values) => {
                    !values.is_empty() && matches!(self, Self::Boolean)
                }
                FilterValue::Integers(values) => !values.is_empty() && self.is_numberlike(),
                FilterValue::Texts(values) => !values.is_empty() && self.is_textlike(),
                FilterValue::Null | FilterValue::Boolean(_) | FilterValue::Integer(_) => false,
                FilterValue::Text(_) => false,
            },
        }
    }
}

/// A closed field vocabulary for one filterable resource: parses the wire
/// `field` string, reports its canonical name back, and its column kind —
/// the §7.1 "column-type registry" entry for that field. SQL column
/// expressions are a separate, adapter-owned mapping (`v2board-db`).
pub trait FilterField: Copy + Eq {
    fn parse(name: &str) -> Option<Self>;
    fn name(self) -> &'static str;
    fn kind(self) -> ColumnKind;
}

/// One AND-combined `{field, operator, value}` clause against a whitelisted
/// column, generic over the resource's closed field enum `F`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterClause<F> {
    pub field: F,
    pub operator: FilterOperator,
    pub value: FilterValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterViolation {
    pub field: &'static str,
    pub message: String,
}

/// Validates a closed filter vocabulary before any SQL adapter sees it —
/// the one check every resource's list/export/bulk-mutation entry point
/// runs ahead of its repository, regardless of what the transport layer
/// already checked on the way in.
pub fn validate_filters<F: FilterField>(
    filters: &[FilterClause<F>],
) -> Result<(), FilterViolation> {
    for filter in filters {
        if !filter.field.kind().accepts(filter.operator, &filter.value) {
            return Err(FilterViolation {
                field: "filter",
                message: format!(
                    "operator/value combination is invalid for {}",
                    filter.field.name()
                ),
            });
        }
    }
    Ok(())
}

/// Wraps `value` in the shared `%...%` `ILIKE` wildcard escape (§7.1):
/// literal `%`, `_`, and `\` in `value` are escaped ahead of binding so
/// they match themselves rather than acting as SQL wildcards.
#[must_use]
pub fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('%');
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('%');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_literal_escaping_wraps_and_escapes_every_sql_wildcard() {
        assert_eq!(escape_like_pattern("50%_a\\b"), "%50\\%\\_a\\\\b%");
    }

    #[test]
    fn null_is_always_a_valid_eq_neq_value_regardless_of_kind() {
        for kind in [
            ColumnKind::Boolean,
            ColumnKind::Integer,
            ColumnKind::Timestamp,
            ColumnKind::Text,
            ColumnKind::Email,
        ] {
            assert!(kind.accepts(FilterOperator::Eq, &FilterValue::Null));
            assert!(kind.accepts(FilterOperator::Neq, &FilterValue::Null));
        }
    }

    #[test]
    fn like_is_closed_to_text_email_and_integer_kinds() {
        let value = FilterValue::Text("x".into());
        assert!(ColumnKind::Text.accepts(FilterOperator::Like, &value));
        assert!(ColumnKind::Email.accepts(FilterOperator::Like, &value));
        assert!(ColumnKind::Integer.accepts(FilterOperator::Like, &value));
        assert!(!ColumnKind::Timestamp.accepts(FilterOperator::Like, &value));
        assert!(!ColumnKind::Boolean.accepts(FilterOperator::Like, &value));
    }

    #[test]
    fn range_operators_are_closed_to_number_like_kinds() {
        let value = FilterValue::Integer(1);
        for operator in [
            FilterOperator::Gt,
            FilterOperator::Gte,
            FilterOperator::Lt,
            FilterOperator::Lte,
        ] {
            assert!(ColumnKind::Integer.accepts(operator, &value));
            assert!(ColumnKind::Timestamp.accepts(operator, &value));
            assert!(!ColumnKind::Text.accepts(operator, &value));
            assert!(!ColumnKind::Email.accepts(operator, &value));
            assert!(!ColumnKind::Boolean.accepts(operator, &value));
        }
    }

    #[test]
    fn in_requires_a_non_empty_array_matching_the_column_kind() {
        assert!(
            ColumnKind::Text.accepts(FilterOperator::In, &FilterValue::Texts(vec!["a".into()]))
        );
        assert!(!ColumnKind::Text.accepts(FilterOperator::In, &FilterValue::Texts(Vec::new())));
        assert!(
            !ColumnKind::Integer.accepts(FilterOperator::In, &FilterValue::Texts(vec!["a".into()]))
        );
    }
}
