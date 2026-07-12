use std::collections::HashMap;

use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PlanRow {
    pub id: i32,
    pub group_id: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub name: String,
    pub speed_limit: Option<i32>,
    pub show: i16,
    pub sort: Option<i32>,
    pub renew: i16,
    pub content: Option<String>,
    pub month_price: Option<i32>,
    pub quarter_price: Option<i32>,
    pub half_year_price: Option<i32>,
    pub year_price: Option<i32>,
    pub two_year_price: Option<i32>,
    pub three_year_price: Option<i32>,
    pub onetime_price: Option<i32>,
    pub reset_price: Option<i32>,
    pub reset_traffic_method: Option<i16>,
    pub capacity_limit: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn find_plan(pool: &PgPool, id: i32) -> Result<Option<PlanRow>, sqlx::Error> {
    sqlx::query_as::<_, PlanRow>(PLAN_SELECT_SQL)
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Locks the plan row that serializes every capacity-consuming path.
pub async fn find_plan_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanRow>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(PLAN_SELECT_SQL);
    query.push("FOR UPDATE");
    query
        .build_query_as::<PlanRow>()
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
}

pub const PLAN_CAPACITY_USAGE_SQL: &str = r#"
SELECT
    (
        SELECT COUNT(*)
        FROM v2_user AS active_user
        WHERE active_user.plan_id = $1
          AND (active_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT OR active_user.expired_at IS NULL)
    ) + (
        SELECT COUNT(DISTINCT pending_order.user_id)
        FROM v2_order AS pending_order
        WHERE pending_order.plan_id = $2
          AND pending_order.status IN (0, 1)
          AND pending_order.type IN (1, 3)
          AND NOT EXISTS (
              SELECT 1
              FROM v2_user AS reserved_user
              WHERE reserved_user.id = pending_order.user_id
                AND reserved_user.plan_id = pending_order.plan_id
                AND (
                    reserved_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT
                    OR reserved_user.expired_at IS NULL
                )
          )
    ) AS capacity_used
"#;

pub async fn capacity_usage_for_update(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: i32,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(PLAN_CAPACITY_USAGE_SQL)
        .bind(plan_id)
        .bind(plan_id)
        .fetch_one(&mut **tx)
        .await
}

pub async fn fetch_visible_plans(pool: &PgPool) -> Result<Vec<PlanRow>, sqlx::Error> {
    sqlx::query_as::<_, PlanRow>(
        r#"
        SELECT
            id,
            group_id,
            transfer_enable,
            device_limit,
            name,
            speed_limit,
            show,
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
        WHERE show = 1
        ORDER BY sort ASC NULLS FIRST
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn count_active_users_by_plan(pool: &PgPool) -> Result<HashMap<i32, i64>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PlanActiveCountRow>(
        r#"
        SELECT plan_id, COUNT(*) AS count
        FROM v2_user
        WHERE plan_id IS NOT NULL
          AND (expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT OR expired_at IS NULL)
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

/// Capacity consumption includes active subscribers plus pending/opening
/// new-plan orders that have not yet materialized as an active user. Completed
/// orders disappear from the reservation term and are represented by the user
/// row; cancelled orders disappear naturally.
pub async fn count_capacity_usage_by_plan(pool: &PgPool) -> Result<HashMap<i32, i64>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PlanActiveCountRow>(
        r#"
        SELECT plan_id, SUM(slot_count)::BIGINT AS count
        FROM (
            SELECT plan_id, COUNT(*) AS slot_count
            FROM v2_user
            WHERE plan_id IS NOT NULL
              AND (expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT OR expired_at IS NULL)
            GROUP BY plan_id

            UNION ALL

            SELECT pending_order.plan_id, COUNT(DISTINCT pending_order.user_id) AS slot_count
            FROM v2_order AS pending_order
            WHERE pending_order.status IN (0, 1)
              AND pending_order.type IN (1, 3)
              AND NOT EXISTS (
                  SELECT 1
                  FROM v2_user AS reserved_user
                  WHERE reserved_user.id = pending_order.user_id
                    AND reserved_user.plan_id = pending_order.plan_id
                    AND (
                        reserved_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT
                        OR reserved_user.expired_at IS NULL
                    )
              )
            GROUP BY pending_order.plan_id
        ) AS capacity_usage
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
    show,
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
WHERE id = $1
LIMIT 1
"#;
