use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, Serialize)]
pub struct CouponRow {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub r#type: i16,
    pub value: i32,
    pub show: i16,
    pub limit_use: Option<i32>,
    pub limit_use_with_user: Option<i32>,
    pub limit_plan_ids: Option<Vec<i32>>,
    pub limit_period: Option<Vec<String>>,
    pub started_at: i64,
    pub ended_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
pub struct RawCouponRow {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub r#type: i16,
    pub value: i32,
    pub show: i16,
    pub limit_use: Option<i32>,
    pub limit_use_with_user: Option<i32>,
    pub limit_plan_ids: Option<String>,
    pub limit_period: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn find_coupon(pool: &PgPool, code: &str) -> Result<Option<CouponRow>, sqlx::Error> {
    sqlx::query_as::<_, RawCouponRow>(
        r#"
        SELECT
            id,
            code,
            name,
            type,
            value,
            show,
            limit_use,
            limit_use_with_user,
            limit_plan_ids::text AS limit_plan_ids,
            limit_period::text AS limit_period,
            started_at,
            ended_at,
            created_at,
            updated_at
        FROM coupon
        WHERE lower(code) = lower($1)
        LIMIT 1
        "#,
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(to_coupon))
}

pub async fn count_user_coupon_uses(
    pool: &PgPool,
    coupon_id: i32,
    user_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM orders
        WHERE coupon_id = $1 AND user_id = $2 AND status NOT IN (0, 2)
        "#,
    )
    .bind(coupon_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

fn to_coupon(row: RawCouponRow) -> CouponRow {
    CouponRow {
        id: row.id,
        code: row.code,
        name: row.name,
        r#type: row.r#type,
        value: row.value,
        show: row.show,
        limit_use: row.limit_use,
        limit_use_with_user: row.limit_use_with_user,
        limit_plan_ids: parse_i32_json_list(row.limit_plan_ids.as_deref()),
        limit_period: parse_string_json_list(row.limit_period.as_deref()),
        started_at: row.started_at,
        ended_at: row.ended_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn parse_i32_json_list(value: Option<&str>) -> Option<Vec<i32>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<serde_json::Value>>(value)
        .ok()
        .map(|items| {
            items
                .into_iter()
                .filter_map(|item| {
                    item.as_i64()
                        .and_then(|value| i32::try_from(value).ok())
                        .or_else(|| item.as_str().and_then(|value| value.parse::<i32>().ok()))
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn parse_string_json_list(value: Option<&str>) -> Option<Vec<String>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<String>>(value)
        .ok()
        .filter(|items| !items.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn coupon_lookup_preserves_legacy_case_insensitive_identity() {
        let source = include_str!("coupon.rs");
        assert!(source.contains("WHERE lower(code) = lower($1)"));
        let migration = include_str!("../../../migrations-postgres/0001_initial.sql");
        assert!(migration.contains("uniq_coupon_code_canonical"));
    }

    #[test]
    fn coupon_plan_scope_accepts_legacy_numeric_strings() {
        assert_eq!(
            super::parse_i32_json_list(Some(r#"["1",2,"invalid"]"#)),
            Some(vec![1, 2])
        );
    }
}
