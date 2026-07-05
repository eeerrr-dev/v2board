use std::collections::HashMap;

use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PlanRow {
    pub id: i32,
    pub group_id: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub name: String,
    pub speed_limit: Option<i32>,
    pub show: i8,
    pub sort: Option<i32>,
    pub renew: i8,
    pub content: Option<String>,
    pub month_price: Option<i32>,
    pub quarter_price: Option<i32>,
    pub half_year_price: Option<i32>,
    pub year_price: Option<i32>,
    pub two_year_price: Option<i32>,
    pub three_year_price: Option<i32>,
    pub onetime_price: Option<i32>,
    pub reset_price: Option<i32>,
    pub reset_traffic_method: Option<i8>,
    pub capacity_limit: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn find_plan(pool: &MySqlPool, id: i32) -> Result<Option<PlanRow>, sqlx::Error> {
    sqlx::query_as::<_, PlanRow>(PLAN_SELECT_SQL)
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn fetch_visible_plans(pool: &MySqlPool) -> Result<Vec<PlanRow>, sqlx::Error> {
    sqlx::query_as::<_, PlanRow>(
        r#"
        SELECT
            id,
            group_id,
            transfer_enable,
            device_limit,
            name,
            speed_limit,
            `show`,
            sort,
            renew,
            content,
            month_price,
            quarter_price,
            half_year_price,
            year_price,
            two_year_price,
            three_year_price,
            onetime_price,
            reset_price,
            reset_traffic_method,
            capacity_limit,
            created_at,
            updated_at
        FROM v2_plan
        WHERE `show` = 1
        ORDER BY sort ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn count_active_users_by_plan(
    pool: &MySqlPool,
) -> Result<HashMap<i32, i64>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PlanActiveCountRow>(
        r#"
        SELECT plan_id, COUNT(*) AS count
        FROM v2_user
        WHERE plan_id IS NOT NULL
          AND (expired_at >= UNIX_TIMESTAMP() OR expired_at IS NULL)
        GROUP BY plan_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.plan_id, row.count))
        .collect())
}

#[derive(Debug, FromRow)]
struct PlanActiveCountRow {
    plan_id: i32,
    count: i64,
}

const PLAN_SELECT_SQL: &str = r#"
SELECT
    id,
    group_id,
    transfer_enable,
    device_limit,
    name,
    speed_limit,
    `show`,
    sort,
    renew,
    content,
    month_price,
    quarter_price,
    half_year_price,
    year_price,
    two_year_price,
    three_year_price,
    onetime_price,
    reset_price,
    reset_traffic_method,
    capacity_limit,
    created_at,
    updated_at
FROM v2_plan
WHERE id = ?
LIMIT 1
"#;
