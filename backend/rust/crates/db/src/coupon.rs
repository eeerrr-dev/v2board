use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, Serialize)]
pub struct CouponRow {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub r#type: i8,
    pub value: i32,
    pub show: i8,
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
    pub r#type: i8,
    pub value: i32,
    pub show: i8,
    pub limit_use: Option<i32>,
    pub limit_use_with_user: Option<i32>,
    pub limit_plan_ids: Option<String>,
    pub limit_period: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn find_coupon(pool: &MySqlPool, code: &str) -> Result<Option<CouponRow>, sqlx::Error> {
    sqlx::query_as::<_, RawCouponRow>(
        r#"
        SELECT
            id,
            code,
            name,
            `type`,
            value,
            `show`,
            limit_use,
            limit_use_with_user,
            limit_plan_ids,
            limit_period,
            started_at,
            ended_at,
            created_at,
            updated_at
        FROM v2_coupon
        WHERE code = ?
        LIMIT 1
        "#,
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(to_coupon))
}

pub async fn count_user_coupon_uses(
    pool: &MySqlPool,
    coupon_id: i32,
    user_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM v2_order
        WHERE coupon_id = ? AND user_id = ? AND status NOT IN (0, 2)
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
    serde_json::from_str::<Vec<i32>>(value)
        .ok()
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
