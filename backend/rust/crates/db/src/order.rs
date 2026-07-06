use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

use crate::plan::{PlanRow, find_plan};

#[derive(Debug, Clone, Serialize)]
pub struct OrderRow {
    pub trade_no: String,
    pub callback_no: Option<String>,
    pub plan_id: i32,
    pub coupon_id: Option<i32>,
    pub payment_id: Option<i32>,
    pub r#type: i32,
    pub period: String,
    pub total_amount: i32,
    pub handling_amount: Option<i32>,
    pub discount_amount: Option<i32>,
    pub surplus_amount: Option<i32>,
    pub refund_amount: Option<i32>,
    pub balance_amount: Option<i32>,
    pub surplus_order_ids: Option<Vec<i64>>,
    pub status: i8,
    pub commission_status: i8,
    pub commission_balance: i32,
    pub actual_commission_balance: Option<i32>,
    pub invite_user_id: Option<i32>,
    pub paid_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub try_out_plan_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus_orders: Option<Vec<OrderRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounus: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get_amount: Option<i32>,
}

#[derive(Debug, Clone, FromRow)]
struct RawOrderRow {
    id: i64,
    invite_user_id: Option<i32>,
    user_id: i64,
    plan_id: i32,
    coupon_id: Option<i32>,
    payment_id: Option<i32>,
    r#type: i32,
    period: String,
    trade_no: String,
    callback_no: Option<String>,
    total_amount: i32,
    handling_amount: Option<i32>,
    discount_amount: Option<i32>,
    surplus_amount: Option<i32>,
    refund_amount: Option<i32>,
    balance_amount: Option<i32>,
    surplus_order_ids: Option<String>,
    status: i8,
    commission_status: i8,
    commission_balance: i32,
    actual_commission_balance: Option<i32>,
    paid_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct CancelCandidate {
    pub status: i8,
    pub balance_amount: Option<i32>,
}

pub async fn fetch_user_orders(
    pool: &MySqlPool,
    user_id: i64,
    status: Option<i8>,
) -> Result<Vec<OrderRow>, sqlx::Error> {
    let rows = match status {
        Some(status) => {
            sqlx::query_as::<_, RawOrderRow>(ORDER_FETCH_BY_STATUS_SQL)
                .bind(user_id)
                .bind(status)
                .fetch_all(pool)
                .await?
        }
        None => {
            sqlx::query_as::<_, RawOrderRow>(ORDER_FETCH_SQL)
                .bind(user_id)
                .fetch_all(pool)
                .await?
        }
    };

    let mut orders = Vec::with_capacity(rows.len());
    for row in rows {
        let plan = if row.plan_id == 0 {
            None
        } else {
            find_plan(pool, row.plan_id).await?
        };
        orders.push(to_order(row, plan));
    }
    Ok(orders)
}

pub async fn find_user_order(
    pool: &MySqlPool,
    user_id: i64,
    trade_no: &str,
    try_out_plan_id: i32,
) -> Result<Option<OrderRow>, sqlx::Error> {
    let Some(row) = find_raw_user_order(pool, user_id, trade_no).await? else {
        return Ok(None);
    };

    if row.plan_id == 0 {
        // Deposit order: synthesize the `deposit` plan and reserve the reward fields. The tier
        // amount depends on config, so the API layer fills the real `bounus`/`get_amount`.
        let total_amount = row.total_amount;
        let mut order = to_order(row, Some(deposit_plan()));
        order.bounus = Some(0);
        order.get_amount = Some(total_amount);
        return Ok(Some(order));
    }

    let plan = find_plan(pool, row.plan_id).await?;
    let surplus_order_ids = parse_surplus_order_ids(row.surplus_order_ids.as_deref());
    let mut order = to_order(row, plan);
    order.try_out_plan_id = Some(try_out_plan_id);
    if let Some(ids) = surplus_order_ids {
        let mut surplus_orders = Vec::new();
        for id in ids {
            if let Some(raw) = find_raw_user_order_by_id(pool, user_id, id).await? {
                let plan = if raw.plan_id == 0 {
                    Some(deposit_plan())
                } else {
                    find_plan(pool, raw.plan_id).await?
                };
                surplus_orders.push(to_order(raw, plan));
            }
        }
        if !surplus_orders.is_empty() {
            order.surplus_orders = Some(surplus_orders);
        }
    }
    Ok(Some(order))
}

pub async fn find_order_status(
    pool: &MySqlPool,
    user_id: i64,
    trade_no: &str,
) -> Result<Option<i8>, sqlx::Error> {
    sqlx::query_scalar("SELECT status FROM v2_order WHERE user_id = ? AND trade_no = ? LIMIT 1")
        .bind(user_id)
        .bind(trade_no)
        .fetch_optional(pool)
        .await
}

pub async fn find_cancel_candidate(
    pool: &MySqlPool,
    user_id: i64,
    trade_no: &str,
) -> Result<Option<CancelCandidate>, sqlx::Error> {
    sqlx::query_as::<_, CancelCandidateRow>(
        "SELECT status, balance_amount FROM v2_order WHERE user_id = ? AND trade_no = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(trade_no)
    .fetch_optional(pool)
    .await
    .map(|row| {
        row.map(|row| CancelCandidate {
            status: row.status,
            balance_amount: row.balance_amount,
        })
    })
}

pub async fn cancel_pending_order(
    pool: &MySqlPool,
    user_id: i64,
    trade_no: &str,
    balance_amount: Option<i32>,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE v2_order SET status = 2, updated_at = ? WHERE user_id = ? AND trade_no = ? AND status = 0",
    )
    .bind(now)
    .bind(user_id)
    .bind(trade_no)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }

    if let Some(balance_amount) = balance_amount.filter(|amount| *amount > 0) {
        sqlx::query("UPDATE v2_user SET balance = balance + ?, updated_at = ? WHERE id = ?")
            .bind(balance_amount)
            .bind(now)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(true)
}

#[derive(Debug, FromRow)]
struct CancelCandidateRow {
    status: i8,
    balance_amount: Option<i32>,
}

async fn find_raw_user_order(
    pool: &MySqlPool,
    user_id: i64,
    trade_no: &str,
) -> Result<Option<RawOrderRow>, sqlx::Error> {
    sqlx::query_as::<_, RawOrderRow>(ORDER_FIND_BY_TRADE_NO_SQL)
        .bind(user_id)
        .bind(trade_no)
        .fetch_optional(pool)
        .await
}

async fn find_raw_user_order_by_id(
    pool: &MySqlPool,
    user_id: i64,
    id: i64,
) -> Result<Option<RawOrderRow>, sqlx::Error> {
    sqlx::query_as::<_, RawOrderRow>(ORDER_FIND_BY_ID_SQL)
        .bind(user_id)
        .bind(id)
        .fetch_optional(pool)
        .await
}

fn to_order(row: RawOrderRow, plan: Option<PlanRow>) -> OrderRow {
    let surplus_order_ids = parse_surplus_order_ids(row.surplus_order_ids.as_deref());
    let _ = (row.id, row.user_id);
    OrderRow {
        trade_no: row.trade_no,
        callback_no: row.callback_no,
        plan_id: row.plan_id,
        coupon_id: row.coupon_id,
        payment_id: row.payment_id,
        r#type: row.r#type,
        period: row.period,
        total_amount: row.total_amount,
        handling_amount: row.handling_amount,
        discount_amount: row.discount_amount,
        surplus_amount: row.surplus_amount,
        refund_amount: row.refund_amount,
        balance_amount: row.balance_amount,
        surplus_order_ids,
        status: row.status,
        commission_status: row.commission_status,
        commission_balance: row.commission_balance,
        actual_commission_balance: row.actual_commission_balance,
        invite_user_id: row.invite_user_id,
        paid_at: row.paid_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        plan,
        try_out_plan_id: None,
        surplus_orders: None,
        bounus: None,
        get_amount: None,
    }
}

fn parse_surplus_order_ids(value: Option<&str>) -> Option<Vec<i64>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<i64>>(value)
        .ok()
        .filter(|items| !items.is_empty())
}

fn deposit_plan() -> PlanRow {
    PlanRow {
        id: 0,
        group_id: 0,
        transfer_enable: 0,
        device_limit: None,
        name: "deposit".to_string(),
        speed_limit: None,
        show: 0,
        sort: None,
        renew: 0,
        content: None,
        month_price: None,
        quarter_price: None,
        half_year_price: None,
        year_price: None,
        two_year_price: None,
        three_year_price: None,
        onetime_price: None,
        reset_price: None,
        reset_traffic_method: None,
        capacity_limit: None,
        created_at: 0,
        updated_at: 0,
    }
}

const ORDER_FETCH_BY_STATUS_SQL: &str = r#"
SELECT
    id,
    invite_user_id,
    user_id,
    plan_id,
    coupon_id,
    payment_id,
    `type`,
    period,
    trade_no,
    callback_no,
    total_amount,
    handling_amount,
    discount_amount,
    surplus_amount,
    refund_amount,
    balance_amount,
    surplus_order_ids,
    status,
    commission_status,
    commission_balance,
    actual_commission_balance,
    paid_at,
    created_at,
    updated_at
FROM v2_order
WHERE user_id = ? AND status = ?
ORDER BY created_at DESC
"#;

const ORDER_FETCH_SQL: &str = r#"
SELECT
    id,
    invite_user_id,
    user_id,
    plan_id,
    coupon_id,
    payment_id,
    `type`,
    period,
    trade_no,
    callback_no,
    total_amount,
    handling_amount,
    discount_amount,
    surplus_amount,
    refund_amount,
    balance_amount,
    surplus_order_ids,
    status,
    commission_status,
    commission_balance,
    actual_commission_balance,
    paid_at,
    created_at,
    updated_at
FROM v2_order
WHERE user_id = ?
ORDER BY created_at DESC
"#;

const ORDER_FIND_BY_TRADE_NO_SQL: &str = r#"
SELECT
    id,
    invite_user_id,
    user_id,
    plan_id,
    coupon_id,
    payment_id,
    `type`,
    period,
    trade_no,
    callback_no,
    total_amount,
    handling_amount,
    discount_amount,
    surplus_amount,
    refund_amount,
    balance_amount,
    surplus_order_ids,
    status,
    commission_status,
    commission_balance,
    actual_commission_balance,
    paid_at,
    created_at,
    updated_at
FROM v2_order
WHERE user_id = ? AND trade_no = ?
LIMIT 1
"#;

const ORDER_FIND_BY_ID_SQL: &str = r#"
SELECT
    id,
    invite_user_id,
    user_id,
    plan_id,
    coupon_id,
    payment_id,
    `type`,
    period,
    trade_no,
    callback_no,
    total_amount,
    handling_amount,
    discount_amount,
    surplus_amount,
    refund_amount,
    balance_amount,
    surplus_order_ids,
    status,
    commission_status,
    commission_balance,
    actual_commission_balance,
    paid_at,
    created_at,
    updated_at
FROM v2_order
WHERE user_id = ? AND id = ?
LIMIT 1
"#;
