use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Canonical modern pagination envelope. Pagination metadata is never
/// optional: every paginated internal response carries both fields.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
}

impl<T> Page<T> {
    #[must_use]
    pub const fn new(items: Vec<T>, total: i64) -> Self {
        Self { items, total }
    }
}

/// Created identifiers backed by PostgreSQL `INTEGER` columns.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedInt32Id {
    pub id: i32,
}

/// Created identifiers backed by PostgreSQL `BIGINT` columns.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedInt64Id {
    pub id: i64,
}

/// Order creation result. Trade numbers are opaque wire identifiers.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedTradeNo {
    pub trade_no: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_total_is_always_present() {
        let value = serde_json::to_value(Page::new(vec![1_i32, 2], 2)).expect("page JSON");
        assert_eq!(value, serde_json::json!({ "items": [1, 2], "total": 2 }));
    }
}
