use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use sqlx::{FromRow, MySql, MySqlPool, QueryBuilder};

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
    pub plan: Option<OrderPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub try_out_plan_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus_orders: Option<Vec<OrderRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounus: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get_amount: Option<i32>,
}

/// The order's `plan` payload. A real order carries the full `PlanRow`; a deposit order
/// (`plan_id == 0`) carries only `{id:0, name:"deposit"}`, matching Laravel's
/// OrderController::detail. Serialized untagged so the emitted JSON is the inner shape.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum OrderPlan {
    Full(Box<PlanRow>),
    Deposit(DepositPlan),
}

#[derive(Debug, Clone, Serialize)]
pub struct DepositPlan {
    pub id: i32,
    pub name: &'static str,
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
    pub payment_id: Option<i32>,
    pub callback_no: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CancelPendingOrderError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("the order balance refund exceeds the supported balance range")]
    BalanceOverflow,
    #[error("the order owner no longer exists")]
    UserNotFound,
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

    let plan_ids = rows
        .iter()
        .filter_map(|row| (row.plan_id != 0).then_some(row.plan_id))
        .collect::<Vec<_>>();
    let plans = fetch_order_plans(pool, &plan_ids).await?;
    let mut orders = Vec::with_capacity(rows.len());
    for row in rows {
        let plan = plans.get(&row.plan_id).cloned();
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
        // Deposit order: Laravel emits a minimal `{id:0, name:'deposit'}` plan and reserves the
        // reward fields. The tier amount depends on config, so the API layer fills the real
        // `bounus`/`get_amount`.
        let total_amount = row.total_amount;
        let mut order = to_order(row, None);
        order.plan = Some(OrderPlan::Deposit(DepositPlan {
            id: 0,
            name: "deposit",
        }));
        order.bounus = Some(0);
        order.get_amount = Some(total_amount);
        return Ok(Some(order));
    }

    let plan = find_plan(pool, row.plan_id).await?;
    let surplus_order_ids = parse_surplus_order_ids(row.surplus_order_ids.as_deref());
    let mut order = to_order(row, plan);
    order.try_out_plan_id = Some(try_out_plan_id);
    if let Some(ids) = surplus_order_ids {
        let raw_orders = fetch_raw_user_orders_by_ids(pool, user_id, &ids).await?;
        let plan_ids = raw_orders
            .values()
            .filter_map(|row| (row.plan_id != 0).then_some(row.plan_id))
            .collect::<Vec<_>>();
        let plans = fetch_order_plans(pool, &plan_ids).await?;
        let mut surplus_orders = Vec::new();
        for raw in values_in_requested_order(&ids, &raw_orders) {
            let plan = if raw.plan_id == 0 {
                Some(deposit_plan())
            } else {
                plans.get(&raw.plan_id).cloned()
            };
            surplus_orders.push(to_order(raw, plan));
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
        "SELECT status, balance_amount, payment_id, callback_no FROM v2_order WHERE user_id = ? AND trade_no = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(trade_no)
    .fetch_optional(pool)
    .await
    .map(|row| {
        row.map(|row| CancelCandidate {
            status: row.status,
            balance_amount: row.balance_amount,
            payment_id: row.payment_id,
            callback_no: row.callback_no,
        })
    })
}

pub async fn cancel_pending_order(
    pool: &MySqlPool,
    user_id: i64,
    trade_no: &str,
    balance_amount: Option<i32>,
    expected_payment_id: Option<i32>,
    expected_callback_no: Option<&str>,
    now: i64,
) -> Result<bool, CancelPendingOrderError> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE v2_order SET status = 2, updated_at = ?
        WHERE user_id = ? AND trade_no = ? AND status = 0
          AND payment_id <=> ? AND callback_no <=> ?
        "#,
    )
    .bind(now)
    .bind(user_id)
    .bind(trade_no)
    .bind(expected_payment_id)
    .bind(expected_callback_no)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }

    if let Some(balance_amount) = balance_amount.filter(|amount| *amount > 0) {
        let current_balance: i32 =
            sqlx::query_scalar("SELECT balance FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CancelPendingOrderError::UserNotFound)?;
        let new_balance = current_balance
            .checked_add(balance_amount)
            .ok_or(CancelPendingOrderError::BalanceOverflow)?;
        sqlx::query("UPDATE v2_user SET balance = ?, updated_at = ? WHERE id = ?")
            .bind(new_balance)
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
    payment_id: Option<i32>,
    callback_no: Option<String>,
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

async fn fetch_raw_user_orders_by_ids(
    pool: &MySqlPool,
    user_id: i64,
    ids: &[i64],
) -> Result<HashMap<i64, RawOrderRow>, sqlx::Error> {
    let ids = ids.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = HashMap::with_capacity(ids.len());
    let ids = ids.into_iter().collect::<Vec<_>>();
    for chunk in ids.chunks(500) {
        let mut builder = QueryBuilder::<MySql>::new(ORDER_FIND_BY_IDS_SQL);
        builder.push_bind(user_id);
        builder.push(" AND id IN (");
        let mut separated = builder.separated(", ");
        for id in chunk {
            separated.push_bind(*id);
        }
        builder.push(")");
        for row in builder
            .build_query_as::<RawOrderRow>()
            .fetch_all(pool)
            .await?
        {
            rows.insert(row.id, row);
        }
    }
    Ok(rows)
}

async fn fetch_order_plans(
    pool: &MySqlPool,
    plan_ids: &[i32],
) -> Result<HashMap<i32, PlanRow>, sqlx::Error> {
    let plan_ids = plan_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut plans = HashMap::with_capacity(plan_ids.len());
    let plan_ids = plan_ids.into_iter().collect::<Vec<_>>();
    for chunk in plan_ids.chunks(500) {
        let mut builder = QueryBuilder::<MySql>::new(PLAN_FIND_BY_IDS_SQL);
        let mut separated = builder.separated(", ");
        for plan_id in chunk {
            separated.push_bind(*plan_id);
        }
        builder.push(")");
        for plan in builder.build_query_as::<PlanRow>().fetch_all(pool).await? {
            plans.insert(plan.id, plan);
        }
    }
    Ok(plans)
}

fn values_in_requested_order<T: Clone>(ids: &[i64], values: &HashMap<i64, T>) -> Vec<T> {
    ids.iter()
        .filter_map(|id| values.get(id).cloned())
        .collect()
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
        plan: plan.map(|plan| OrderPlan::Full(Box::new(plan))),
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

const ORDER_FIND_BY_IDS_SQL: &str = r#"
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
WHERE user_id =
"#;

const PLAN_FIND_BY_IDS_SQL: &str = r#"
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
WHERE id IN (
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_plan_serializes_as_minimal_object() {
        let plan = OrderPlan::Deposit(DepositPlan {
            id: 0,
            name: "deposit",
        });
        let value = serde_json::to_value(&plan).unwrap();
        assert_eq!(value, serde_json::json!({ "id": 0, "name": "deposit" }));
    }

    #[test]
    fn full_plan_serializes_untagged_with_all_fields() {
        let value = serde_json::to_value(OrderPlan::Full(Box::new(deposit_plan()))).unwrap();
        // Untagged: the variant wrapper is dropped and the full `PlanRow` fields are emitted,
        // so keys absent from the minimal deposit shape (e.g. `group_id`) are present.
        assert_eq!(value["id"], serde_json::json!(0));
        assert_eq!(value["name"], serde_json::json!("deposit"));
        assert!(value.get("group_id").is_some());
        assert!(value.get("transfer_enable").is_some());
    }

    #[test]
    fn related_order_reads_use_bounded_batch_queries() {
        assert!(ORDER_FIND_BY_IDS_SQL.contains("WHERE user_id ="));
        assert!(PLAN_FIND_BY_IDS_SQL.contains("id IN ("));
        let source = include_str!("order.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.contains("builder.push(\" AND id IN (\")"));
        assert!(production.contains("ids.chunks(500)"));
        assert!(production.contains("plan_ids.chunks(500)"));
        assert!(!production.contains("find_raw_user_order_by_id"));
    }

    #[test]
    fn batched_surplus_orders_keep_requested_order_duplicates_and_missing_behavior() {
        let values = HashMap::from([(2, "two"), (7, "seven")]);
        assert_eq!(
            values_in_requested_order(&[7, 99, 2, 7], &values),
            vec!["seven", "two", "seven"]
        );
    }
}
