//! Pagination for the modern internal dialect (docs/api-dialect.md §8):
//! `page`/`per_page` request parsing and the `{items, total}` response
//! envelope. Replaces `current`/`pageSize`/`page_size` and the legacy
//! `{data, total}` page envelope on internal routes as each family migrates
//! (consumed from W2 on, docs/api-dialect.md Appendix A). Non-paginated lists
//! stay bare arrays — never wrap them in `items`.

use axum::Json;
use serde::Serialize;

use crate::problem::Problem;

/// §8: `per_page` hard cap.
pub const MAX_PER_PAGE: i64 = 100;

/// A validated `page`/`per_page` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    /// 1-based page number.
    pub page: i64,
    pub per_page: i64,
}

impl Pagination {
    /// Resolve raw query values per §8: `page` defaults to 1 and must be
    /// ≥ 1; `per_page` defaults to the endpoint's legacy default (10 unless
    /// the route table notes otherwise) and must be within 1..=100.
    /// Out-of-range values are a 422 `validation_failed` problem, matching
    /// the legacy explicit pagination validation.
    pub fn resolve(
        page: Option<i64>,
        per_page: Option<i64>,
        default_per_page: i64,
    ) -> Result<Self, Problem> {
        debug_assert!(
            (1..=MAX_PER_PAGE).contains(&default_per_page),
            "endpoint default_per_page must itself be a valid per_page"
        );
        let page = page.unwrap_or(1);
        if page < 1 {
            return Err(Problem::validation_field(
                "page",
                "The page must be at least 1",
            ));
        }
        let per_page = per_page.unwrap_or(default_per_page);
        if !(1..=MAX_PER_PAGE).contains(&per_page) {
            return Err(Problem::validation_field(
                "per_page",
                "The per_page must be between 1 and 100",
            ));
        }
        Ok(Self { page, per_page })
    }

    /// Row offset for SQL `OFFSET`; saturates instead of overflowing on
    /// adversarially large pages.
    pub fn offset(self) -> i64 {
        (self.page - 1).saturating_mul(self.per_page)
    }

    /// Row limit for SQL `LIMIT`.
    pub fn limit(self) -> i64 {
        self.per_page
    }
}

/// §8 paginated response envelope: `{"items": [...], "total": <i64>}`.
#[derive(Debug, Serialize)]
pub struct Page<T>
where
    T: Serialize,
{
    pub items: Vec<T>,
    pub total: i64,
}

/// Build the §8 page envelope response.
pub fn page<T>(items: Vec<T>, total: i64) -> Json<Page<T>>
where
    T: Serialize,
{
    Json(Page { items, total })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;
    use crate::problem::Code;

    #[test]
    fn defaults_apply_when_params_are_absent() {
        let pagination = Pagination::resolve(None, None, 10).unwrap();
        assert_eq!(
            pagination,
            Pagination {
                page: 1,
                per_page: 10
            }
        );

        let pagination = Pagination::resolve(None, None, 5).unwrap();
        assert_eq!(pagination.per_page, 5);
    }

    #[test]
    fn explicit_values_within_range_are_kept() {
        let pagination = Pagination::resolve(Some(3), Some(MAX_PER_PAGE), 10).unwrap();
        assert_eq!(
            pagination,
            Pagination {
                page: 3,
                per_page: 100
            }
        );
    }

    #[test]
    fn page_below_one_is_a_validation_problem() {
        for page in [0, -1] {
            let problem = Pagination::resolve(Some(page), None, 10).unwrap_err();
            assert_eq!(problem.code(), Code::ValidationFailed);
            assert_eq!(problem.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert!(problem.errors().is_some_and(|bag| bag.contains_key("page")));
        }
    }

    #[test]
    fn per_page_out_of_range_is_a_validation_problem() {
        for per_page in [0, -1, MAX_PER_PAGE + 1] {
            let problem = Pagination::resolve(None, Some(per_page), 10).unwrap_err();
            assert_eq!(problem.code(), Code::ValidationFailed);
            assert!(
                problem
                    .errors()
                    .is_some_and(|bag| bag.contains_key("per_page"))
            );
        }
    }

    #[test]
    fn offset_and_limit_translate_to_sql_windows() {
        let pagination = Pagination::resolve(Some(3), Some(10), 10).unwrap();
        assert_eq!(pagination.offset(), 20);
        assert_eq!(pagination.limit(), 10);

        let first = Pagination::resolve(None, None, 10).unwrap();
        assert_eq!(first.offset(), 0);
    }

    #[test]
    fn offset_saturates_instead_of_overflowing() {
        let pagination = Pagination::resolve(Some(i64::MAX), Some(100), 10).unwrap();
        assert_eq!(pagination.offset(), i64::MAX);
    }

    #[test]
    fn page_envelope_serializes_items_and_total() {
        let Json(envelope) = page(vec![1, 2], 5);
        assert_eq!(
            serde_json::to_string(&envelope).unwrap(),
            "{\"items\":[1,2],\"total\":5}"
        );
    }

    #[test]
    fn empty_page_keeps_an_empty_items_array() {
        let Json(envelope) = page(Vec::<i64>::new(), 0);
        assert_eq!(
            serde_json::to_string(&envelope).unwrap(),
            "{\"items\":[],\"total\":0}"
        );
    }
}
