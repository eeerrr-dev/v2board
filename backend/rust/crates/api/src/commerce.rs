use axum::{
    Json,
    extract::{Form, Query, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};
use v2board_db::DbPool;

use crate::{
    auth::{AuthQuery, require_user},
    runtime::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct OrderFetchQuery {
    status: Option<i16>,
    auth_data: Option<String>,
}

pub(crate) async fn order_fetch(
    State(state): State<AppState>,
    Query(query): Query<OrderFetchQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::order::OrderRow>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let orders = v2board_db::order::fetch_user_orders(&state.db, user.id, query.status).await?;
    Ok(legacy_data(orders))
}

pub(crate) async fn order_save(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<v2board_domain::order::SaveOrderInput>,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let trade_no = service.save(user.id, payload).await?;
    Ok(legacy_data(trade_no))
}

#[derive(Debug, Serialize)]
pub(crate) struct CheckoutEnvelope {
    r#type: i16,
    data: serde_json::Value,
}

pub(crate) async fn order_checkout(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<v2board_domain::order::CheckoutOrderInput>,
) -> Result<Json<CheckoutEnvelope>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let result = service.checkout(user.id, payload).await?;
    Ok(Json(CheckoutEnvelope {
        r#type: result.r#type,
        data: result.data,
    }))
}

pub(crate) async fn stripe_payment_intent(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<v2board_domain::order::CheckoutOrderInput>,
) -> Result<Json<LegacyEnvelope<v2board_domain::order::StripePaymentIntentResult>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let intent = service.prepare_stripe_intent(user.id, payload).await?;
    Ok(legacy_data(intent))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TradeNoQuery {
    trade_no: String,
    auth_data: Option<String>,
}

pub(crate) async fn order_detail(
    State(state): State<AppState>,
    Query(query): Query<TradeNoQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::order::OrderRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let config = state.config_snapshot();
    let mut order = v2board_db::order::find_user_order(
        &state.db,
        user.id,
        &query.trade_no,
        config.try_out_plan_id,
    )
    .await?
    .ok_or_else(|| ApiError::legacy("Order does not exist or has been paid"))?;
    if order.plan_id != 0 && order.plan.is_none() {
        return Err(ApiError::legacy("Subscription plan does not exist"));
    }
    // Deposit orders (plan_id == 0) advertise the reward tier: `bounus` and
    // `get_amount = total_amount + bounus` (OrderController::detail). The db layer is config-free,
    // so the real tier lookup happens here.
    if order.plan_id == 0 {
        let bonus = config.deposit_bonus(order.total_amount);
        let get_amount = order.total_amount.checked_add(bonus).ok_or_else(|| {
            ApiError::legacy("Deposit principal and bonus exceed the supported cents range")
        })?;
        order.bounus = Some(bonus);
        order.get_amount = Some(get_amount);
    }
    Ok(legacy_data(order))
}

pub(crate) async fn order_check(
    State(state): State<AppState>,
    Query(query): Query<TradeNoQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<i16>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let status = v2board_db::order::find_order_status(&state.db, user.id, &query.trade_no)
        .await?
        .ok_or_else(|| ApiError::legacy("Order does not exist"))?;
    Ok(legacy_data(status))
}

pub(crate) async fn order_payment_methods(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::payment::PaymentMethodRow>>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    let methods = v2board_db::payment::fetch_enabled_payment_methods(&state.db).await?;
    Ok(legacy_data(methods))
}

#[derive(Debug, Deserialize)]
pub(crate) struct OrderCancelRequest {
    trade_no: String,
}

pub(crate) async fn order_cancel(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<OrderCancelRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if payload.trade_no.trim().is_empty() {
        return Err(ApiError::legacy("Invalid parameter"));
    }
    let candidate = v2board_db::order::find_cancel_candidate(&state.db, user.id, &payload.trade_no)
        .await?
        .ok_or_else(|| ApiError::legacy("Order does not exist"))?;
    if candidate.status != 0 {
        return Err(ApiError::legacy("You can only cancel pending orders"));
    }
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    if !service
        .cancel_stripe_intent_binding(candidate.payment_id, candidate.callback_no.as_deref())
        .await?
    {
        return Err(ApiError::legacy("You can only cancel pending orders"));
    }
    let cancelled = v2board_db::order::cancel_pending_order(
        &state.db,
        user.id,
        &payload.trade_no,
        candidate.balance_amount,
        candidate.payment_id,
        candidate.callback_no.as_deref(),
        Utc::now().timestamp(),
    )
    .await
    .map_err(|error| match error {
        v2board_db::order::CancelPendingOrderError::Database(error) => ApiError::Database(error),
        v2board_db::order::CancelPendingOrderError::BalanceOverflow => {
            ApiError::legacy("Order balance refund exceeds the supported balance range")
        }
        v2board_db::order::CancelPendingOrderError::UserNotFound => {
            ApiError::legacy("The user does not exist")
        }
    })?;
    if !cancelled {
        return Err(ApiError::legacy("Cancel failed"));
    }
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
pub(crate) struct CouponCheckRequest {
    code: String,
    plan_id: Option<i32>,
}

pub(crate) async fn coupon_check(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<CouponCheckRequest>,
) -> Result<Json<LegacyEnvelope<v2board_db::coupon::CouponRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if payload.code.trim().is_empty() {
        return Err(ApiError::legacy("Coupon cannot be empty"));
    }
    let coupon = v2board_db::coupon::find_coupon(&state.db, &payload.code)
        .await?
        .ok_or_else(|| ApiError::legacy("Invalid coupon"))?;
    validate_coupon_for_check(&state.db, user.id, &coupon, payload.plan_id).await?;
    Ok(legacy_data(coupon))
}

async fn validate_coupon_for_check(
    db: &DbPool,
    user_id: i64,
    coupon: &v2board_db::coupon::CouponRow,
    plan_id: Option<i32>,
) -> Result<(), ApiError> {
    if coupon.show == 0 {
        return Err(ApiError::legacy("Invalid coupon"));
    }
    if matches!(coupon.limit_use, Some(limit_use) if limit_use <= 0) {
        return Err(ApiError::legacy("This coupon is no longer available"));
    }
    let now = Utc::now().timestamp();
    if now < coupon.started_at {
        return Err(ApiError::legacy("This coupon has not yet started"));
    }
    if now > coupon.ended_at {
        return Err(ApiError::legacy("This coupon has expired"));
    }
    if let (Some(plan_id), Some(limit_plan_ids)) = (plan_id, coupon.limit_plan_ids.as_ref())
        && !limit_plan_ids.contains(&plan_id)
    {
        return Err(ApiError::legacy(
            "The coupon code cannot be used for this subscription",
        ));
    }
    if let Some(limit) = coupon.limit_use_with_user {
        let used = v2board_db::coupon::count_user_coupon_uses(db, coupon.id, user_id).await?;
        if used >= i64::from(limit) {
            return Err(ApiError::legacy(format!(
                "The coupon can only be used {limit} per person"
            )));
        }
    }
    Ok(())
}
