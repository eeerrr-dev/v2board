//! User commerce family — modern dialect (docs/api-dialect.md §5.5, §9.3,
//! §9.4, Appendix A §W4): bare success bodies on modern value types
//! (RFC 3339 timestamps, boolean flags, numeric `handling_fee_percent`),
//! path-borne `trade_no`/plan ids, the discriminated create-order request
//! union, the §9.3 checkout result union, and problem+json failures.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use v2board_compat::{
    Code, Problem,
    json::{rfc3339, rfc3339_option},
};
use v2board_db::{
    DbPool,
    coupon::CouponRow,
    order::{OrderPlan, OrderRow},
    payment::PaymentMethodRow,
    plan::PlanRow,
};
use v2board_domain_model::PlanPricePeriod;

use crate::{
    auth::require_user, dialect::DialectJson, dialect::problem_from, locale::request_locale,
    runtime::AppState,
};

/// A handler-constructed problem with a legacy-derived custom detail, pushed
/// through [`problem_from`] so the detail catalog-localizes exactly like the
/// retired response-rewrite middleware (§3.4).
fn commerce_problem(
    code: Code,
    detail: impl Into<std::borrow::Cow<'static, str>>,
    locale: &str,
) -> Problem {
    problem_from(Problem::new(code).with_detail(detail).into(), locale)
}

/// One plan (§5.5) on modern value types: boolean `show`/`renew` (§4.1) and
/// RFC 3339 timestamps (§4.5). Sold-out `capacity_limit` representation is
/// unchanged (spec-owned edge case).
#[derive(Debug, Serialize)]
pub(crate) struct PlanBody {
    pub(crate) id: i32,
    pub(crate) group_id: i32,
    pub(crate) transfer_enable: i64,
    pub(crate) device_limit: Option<i32>,
    pub(crate) name: String,
    pub(crate) speed_limit: Option<i32>,
    pub(crate) show: bool,
    pub(crate) sort: Option<i32>,
    pub(crate) renew: bool,
    pub(crate) content: Option<String>,
    pub(crate) month_price: Option<i32>,
    pub(crate) quarter_price: Option<i32>,
    pub(crate) half_year_price: Option<i32>,
    pub(crate) year_price: Option<i32>,
    pub(crate) two_year_price: Option<i32>,
    pub(crate) three_year_price: Option<i32>,
    pub(crate) onetime_price: Option<i32>,
    pub(crate) reset_price: Option<i32>,
    pub(crate) reset_traffic_method: Option<i16>,
    pub(crate) capacity_limit: Option<i32>,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<PlanRow> for PlanBody {
    fn from(row: PlanRow) -> Self {
        Self {
            id: row.id,
            group_id: row.group_id,
            transfer_enable: row.transfer_enable,
            device_limit: row.device_limit,
            name: row.name,
            speed_limit: row.speed_limit,
            show: row.show,
            sort: row.sort,
            renew: row.renew,
            content: row.content,
            month_price: row.month_price,
            quarter_price: row.quarter_price,
            half_year_price: row.half_year_price,
            year_price: row.year_price,
            two_year_price: row.two_year_price,
            three_year_price: row.three_year_price,
            onetime_price: row.onetime_price,
            reset_price: row.reset_price,
            reset_traffic_method: row.reset_traffic_method,
            capacity_limit: row.capacity_limit,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// The order's `plan` payload: a full modern plan, or the minimal
/// `{id: 0, name: "deposit"}` marker on deposit orders (untagged, kept from
/// the legacy anchor shape).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum OrderPlanBody {
    Full(Box<PlanBody>),
    Deposit { id: i32, name: &'static str },
}

impl From<OrderPlan> for OrderPlanBody {
    fn from(plan: OrderPlan) -> Self {
        match plan {
            OrderPlan::Full(plan) => Self::Full(Box::new(PlanBody::from(*plan))),
            OrderPlan::Deposit(deposit) => Self::Deposit {
                id: deposit.id,
                name: deposit.name,
            },
        }
    }
}

/// One order (§5.5) on modern value types. `status`/`type`/
/// `commission_status` stay numeric (true enums, §4.1); money stays integer
/// cents; `paid_at` is a nullable RFC 3339 instant.
#[derive(Debug, Serialize)]
pub(crate) struct OrderBody {
    pub(crate) trade_no: String,
    pub(crate) callback_no: Option<String>,
    pub(crate) plan_id: i32,
    pub(crate) coupon_id: Option<i32>,
    pub(crate) payment_id: Option<i32>,
    pub(crate) r#type: i32,
    pub(crate) period: String,
    pub(crate) total_amount: i32,
    pub(crate) handling_amount: Option<i32>,
    pub(crate) discount_amount: Option<i32>,
    pub(crate) surplus_amount: Option<i32>,
    pub(crate) refund_amount: Option<i32>,
    pub(crate) balance_amount: Option<i32>,
    pub(crate) surplus_order_ids: Option<Vec<i64>>,
    pub(crate) status: i16,
    pub(crate) commission_status: i16,
    pub(crate) commission_balance: i32,
    pub(crate) actual_commission_balance: Option<i32>,
    pub(crate) invite_user_id: Option<i64>,
    #[serde(with = "rfc3339_option")]
    pub(crate) paid_at: Option<i64>,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) plan: Option<OrderPlanBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) try_out_plan_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) surplus_orders: Option<Vec<OrderBody>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bounus: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) get_amount: Option<i32>,
}

impl From<OrderRow> for OrderBody {
    fn from(row: OrderRow) -> Self {
        Self {
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
            surplus_order_ids: row.surplus_order_ids,
            status: row.status,
            commission_status: row.commission_status,
            commission_balance: row.commission_balance,
            actual_commission_balance: row.actual_commission_balance,
            invite_user_id: row.invite_user_id,
            paid_at: row.paid_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            plan: row.plan.map(OrderPlanBody::from),
            try_out_plan_id: row.try_out_plan_id,
            surplus_orders: row
                .surplus_orders
                .map(|orders| orders.into_iter().map(OrderBody::from).collect()),
            bounus: row.bounus,
            get_amount: row.get_amount,
        }
    }
}

/// One enabled payment method (§5.5): `handling_fee_percent` becomes a JSON
/// number (§4.1; was Eloquent's decimal string).
#[derive(Debug, Serialize)]
pub(crate) struct PaymentMethodBody {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) payment: String,
    pub(crate) icon: Option<String>,
    pub(crate) handling_fee_fixed: Option<i32>,
    pub(crate) handling_fee_percent: Option<f64>,
}

impl From<PaymentMethodRow> for PaymentMethodBody {
    fn from(row: PaymentMethodRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            payment: row.payment,
            icon: row.icon,
            // The column is NUMERIC(5,2)::text, so the string form always
            // parses; a hypothetical non-numeric value degrades to null
            // rather than failing the whole method list.
            handling_fee_percent: row
                .handling_fee_percent
                .as_deref()
                .and_then(|value| value.parse::<f64>().ok()),
            handling_fee_fixed: row.handling_fee_fixed,
        }
    }
}

/// The bare coupon body for POST /user/coupons/check (§5.5): boolean `show`
/// (§4.1; coupon `type` stays a numeric enum) and RFC 3339 windows (§4.5).
#[derive(Debug, Serialize)]
pub(crate) struct CouponBody {
    pub(crate) id: i32,
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) r#type: i16,
    pub(crate) value: i32,
    pub(crate) show: bool,
    pub(crate) limit_use: Option<i32>,
    pub(crate) limit_use_with_user: Option<i32>,
    pub(crate) limit_plan_ids: Option<Vec<i32>>,
    pub(crate) limit_period: Option<Vec<String>>,
    #[serde(with = "rfc3339")]
    pub(crate) started_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) ended_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<CouponRow> for CouponBody {
    fn from(row: CouponRow) -> Self {
        Self {
            id: row.id,
            code: row.code,
            name: row.name,
            r#type: row.r#type,
            value: row.value,
            show: row.show != 0,
            limit_use: row.limit_use,
            limit_use_with_user: row.limit_use_with_user,
            limit_plan_ids: row.limit_plan_ids,
            limit_period: row.limit_period,
            started_at: row.started_at,
            ended_at: row.ended_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// GET /user/plans — bare array of visible plans with the legacy remaining-
/// capacity rewrite (§5.5).
pub(crate) async fn plans_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PlanBody>>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let counts = v2board_db::plan::count_capacity_usage_by_plan(&state.db)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    let mut plans = v2board_db::plan::fetch_visible_plans(&state.db)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    for plan in &mut plans {
        if let Some(capacity_limit) = plan.capacity_limit
            && let Some(count) = counts.get(&plan.id)
        {
            let remaining = i64::from(capacity_limit)
                .checked_sub(*count)
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| {
                    tracing::error!(plan_id = plan.id, "plan capacity usage is outside range");
                    Problem::localized(Code::InternalError, locale)
                })?;
            plan.capacity_limit = Some(remaining);
        }
    }
    Ok(Json(plans.into_iter().map(PlanBody::from).collect()))
}

/// GET /user/plans/{id} — bare plan; a path-identified miss (including the
/// legacy hidden-plan availability rules) is 404 plan_not_found (§3.4).
pub(crate) async fn plan_detail(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<PlanBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let subscribe = v2board_db::user::find_user_subscribe(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::UserNotRegistered, locale))?;
    let plan = v2board_db::plan::find_plan(&state.db, id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::PlanNotFound, locale))?;
    let hidden_plan = !plan.show;
    let unavailable_hidden_plan =
        hidden_plan && (!plan.renew || subscribe.plan_id != Some(plan.id));
    if unavailable_hidden_plan {
        return Err(Problem::localized(Code::PlanNotFound, locale));
    }
    Ok(Json(PlanBody::from(plan)))
}

/// The §5.5 create-order request union: internally tagged on `kind`, both
/// arms `deny_unknown_fields`. The deposit arm replaces the legacy
/// `plan_id: 0` + `period: "deposit"` sentinel; `deposit_amount` stays
/// integer cents.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum CreateOrderRequest {
    Plan(PlanOrderRequest),
    Deposit(DepositOrderRequest),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanOrderRequest {
    pub(crate) plan_id: i32,
    pub(crate) period: String,
    /// §5.5 empty-coupon rule (Tier-1): the client omits the field entirely
    /// when no coupon is applied — never sends `""`.
    #[serde(default)]
    pub(crate) coupon_code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DepositOrderRequest {
    pub(crate) deposit_amount: i32,
}

/// §9.4: POST /user/orders answers `{"trade_no": …}` with 201 instead of the
/// legacy bare string.
#[derive(Debug, Serialize)]
pub(crate) struct CreatedOrder {
    pub(crate) trade_no: String,
}

/// POST /user/orders — create from the discriminated union (§5.5), 201 with
/// the created identity.
pub(crate) async fn order_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CreateOrderRequest>,
) -> Result<(StatusCode, Json<CreatedOrder>), Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let input = match request {
        CreateOrderRequest::Plan(plan) => {
            let period = plan_price_period_from_wire(&plan.period).ok_or_else(|| {
                commerce_problem(Code::PlanPeriodUnavailable, "Wrong plan period", locale)
            })?;
            v2board_domain::order::SaveOrderInput::Plan {
                plan_id: plan.plan_id,
                period,
                coupon_code: plan.coupon_code,
            }
        }
        CreateOrderRequest::Deposit(deposit) => v2board_domain::order::SaveOrderInput::Deposit {
            deposit_amount: deposit.deposit_amount,
        },
    };
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let trade_no = service
        .save(user.id, input)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedOrder { trade_no })))
}

fn plan_price_period_from_wire(period: &str) -> Option<PlanPricePeriod> {
    match period {
        "month_price" => Some(PlanPricePeriod::Month),
        "quarter_price" => Some(PlanPricePeriod::Quarter),
        "half_year_price" => Some(PlanPricePeriod::HalfYear),
        "year_price" => Some(PlanPricePeriod::Year),
        "two_year_price" => Some(PlanPricePeriod::TwoYear),
        "three_year_price" => Some(PlanPricePeriod::ThreeYear),
        "onetime_price" => Some(PlanPricePeriod::OneTime),
        "reset_price" => Some(PlanPricePeriod::Reset),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct OrdersListQuery {
    status: Option<i16>,
}

/// GET /user/orders?status= — bare array (§5.5).
pub(crate) async fn orders_list(
    State(state): State<AppState>,
    Query(query): Query<OrdersListQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<OrderBody>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let orders = v2board_db::order::fetch_user_orders(&state.db, user.id, query.status)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(orders.into_iter().map(OrderBody::from).collect()))
}

/// GET /user/orders/{trade_no} — bare order; a miss is 404 order_not_found
/// (§3.4: every modern order route carries `trade_no` in the path).
pub(crate) async fn order_detail(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let config = state.config_snapshot();
    let mut order =
        v2board_db::order::find_user_order(&state.db, user.id, &trade_no, config.try_out_plan_id)
            .await
            .map_err(|error| problem_from(error.into(), locale))?
            .ok_or_else(|| Problem::localized(Code::OrderNotFound, locale))?;
    if order.plan_id != 0 && order.plan.is_none() {
        return Err(Problem::localized(Code::PlanUnavailable, locale));
    }
    // Deposit orders (plan_id == 0) advertise the reward tier: `bounus` and
    // `get_amount = total_amount + bounus` (OrderController::detail). The db
    // layer is config-free, so the real tier lookup happens here.
    if order.plan_id == 0 {
        let bonus = config.deposit_bonus(order.total_amount);
        let get_amount = order.total_amount.checked_add(bonus).ok_or_else(|| {
            commerce_problem(
                Code::PaymentAmountOutOfRange,
                "Deposit principal and bonus exceed the supported cents range",
                locale,
            )
        })?;
        order.bounus = Some(bonus);
        order.get_amount = Some(get_amount);
    }
    Ok(Json(OrderBody::from(order)))
}

/// §9.4: GET /user/orders/{trade_no}/status answers `{"status": n}` instead
/// of the legacy bare number. The 3 s polling cadence stays client-side.
#[derive(Debug, Serialize)]
pub(crate) struct OrderStatusBody {
    pub(crate) status: i16,
}

/// GET /user/orders/{trade_no}/status — bare `{status}` (§5.5/§9.4).
pub(crate) async fn order_status(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderStatusBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let status = v2board_db::order::find_order_status(&state.db, user.id, &trade_no)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| commerce_problem(Code::OrderNotFound, "Order does not exist", locale))?;
    Ok(Json(OrderStatusBody { status }))
}

/// POST /user/orders/{trade_no}/cancel — `{trade_no}` body → path segment,
/// boolean body → 204/problem (§5.5).
pub(crate) async fn order_cancel(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let not_pending = || {
        commerce_problem(
            Code::OrderNotPending,
            "You can only cancel pending orders",
            locale,
        )
    };
    let candidate = v2board_db::order::find_cancel_candidate(&state.db, user.id, &trade_no)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| commerce_problem(Code::OrderNotFound, "Order does not exist", locale))?;
    if candidate.status != 0 {
        return Err(not_pending());
    }
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    if !service
        .cancel_stripe_intent_binding(candidate.payment_id, candidate.callback_no.as_deref())
        .await
        .map_err(|error| problem_from(error, locale))?
    {
        return Err(not_pending());
    }
    let cancelled = v2board_db::order::cancel_pending_order(
        &state.db,
        user.id,
        &trade_no,
        candidate.balance_amount,
        candidate.payment_id,
        candidate.callback_no.as_deref(),
        Utc::now().timestamp(),
    )
    .await
    .map_err(|error| match error {
        v2board_db::order::CancelPendingOrderError::Database(error) => {
            problem_from(error.into(), locale)
        }
        v2board_db::order::CancelPendingOrderError::BalanceOverflow => commerce_problem(
            Code::BalanceOutOfRange,
            "Order balance refund exceeds the supported balance range",
            locale,
        ),
        v2board_db::order::CancelPendingOrderError::UserNotFound => {
            Problem::localized(Code::UserNotRegistered, locale)
        }
    })?;
    if !cancelled {
        // The row changed under us (paid/cancelled concurrently): no longer a
        // pending order.
        return Err(not_pending());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// The §5.5 checkout / stripe-intent request body. `method_id` replaces the
/// legacy `method` name; it stays optional so a zero-total order settles
/// without a gateway selection exactly like the legacy anchor.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckoutRequest {
    #[serde(default)]
    pub(crate) method_id: Option<i32>,
}

/// The §9.3 checkout discriminated union.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum CheckoutOutcome {
    QrCode { payload: String },
    Redirect { url: String },
    Settled,
}

impl CheckoutOutcome {
    /// Map the legacy gateway envelope onto the union; anything the table in
    /// §9.3 does not cover is gateway misbehavior — 400
    /// payment_gateway_unsupported.
    fn from_gateway_result(
        result: v2board_domain::order::CheckoutResult,
        locale: &str,
    ) -> Result<Self, Problem> {
        let v2board_domain::order::CheckoutResult { r#type, data } = result;
        match (r#type, data) {
            (0, serde_json::Value::String(payload)) => Ok(Self::QrCode { payload }),
            (1, serde_json::Value::String(url)) => Ok(Self::Redirect { url }),
            (-1, _) => Ok(Self::Settled),
            (r#type, data) => {
                tracing::error!(
                    gateway_type = r#type,
                    ?data,
                    "unmappable checkout gateway result"
                );
                Err(Problem::localized(Code::PaymentGatewayUnsupported, locale))
            }
        }
    }
}

/// POST /user/orders/{trade_no}/checkout — the §9.3 union.
pub(crate) async fn order_checkout(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CheckoutRequest>,
) -> Result<Json<CheckoutOutcome>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let result = service
        .checkout(
            user.id,
            v2board_domain::order::CheckoutOrderInput {
                trade_no,
                method: request.method_id,
            },
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(CheckoutOutcome::from_gateway_result(result, locale)?))
}

/// POST /user/orders/{trade_no}/stripe-intent — bare
/// `{public_key, client_secret, amount, currency}` (§5.5). The Stripe
/// external payloads behind it are byte-frozen (§2).
pub(crate) async fn order_stripe_intent(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CheckoutRequest>,
) -> Result<Json<v2board_domain::order::StripePaymentIntentResult>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let intent = service
        .prepare_stripe_intent(
            user.id,
            v2board_domain::order::CheckoutOrderInput {
                trade_no,
                method: request.method_id,
            },
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(intent))
}

/// GET /user/payment-methods — bare array (§5.5).
pub(crate) async fn payment_methods(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PaymentMethodBody>>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let methods = v2board_db::payment::fetch_enabled_payment_methods(&state.db)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(
        methods.into_iter().map(PaymentMethodBody::from).collect(),
    ))
}

/// POST /user/coupons/check — read-shaped action kept as POST (§5.5), JSON
/// `{code, plan_id}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CouponCheckRequest {
    pub(crate) code: String,
    #[serde(default)]
    pub(crate) plan_id: Option<i32>,
}

pub(crate) async fn coupon_check(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CouponCheckRequest>,
) -> Result<Json<CouponBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    if request.code.trim().is_empty() {
        return Err(commerce_problem(
            Code::CouponInvalid,
            "Coupon cannot be empty",
            locale,
        ));
    }
    let coupon = v2board_db::coupon::find_coupon(&state.db, &request.code)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::CouponInvalid, locale))?;
    validate_coupon_for_check(&state.db, user.id, &coupon, request.plan_id)
        .await
        .map_err(|problem| problem_from(problem.into(), locale))?;
    Ok(Json(CouponBody::from(coupon)))
}

async fn validate_coupon_for_check(
    db: &DbPool,
    user_id: i64,
    coupon: &CouponRow,
    plan_id: Option<i32>,
) -> Result<(), Problem> {
    if coupon.show == 0 {
        return Err(Problem::new(Code::CouponInvalid));
    }
    if matches!(coupon.limit_use, Some(limit_use) if limit_use <= 0) {
        return Err(Problem::new(Code::CouponUnavailable));
    }
    let now = Utc::now().timestamp();
    if now < coupon.started_at {
        return Err(Problem::new(Code::CouponNotStarted));
    }
    if now > coupon.ended_at {
        return Err(Problem::new(Code::CouponExpired));
    }
    if let (Some(plan_id), Some(limit_plan_ids)) = (plan_id, coupon.limit_plan_ids.as_ref())
        && !limit_plan_ids.contains(&plan_id)
    {
        return Err(Problem::new(Code::CouponNotApplicable)
            .with_detail("The coupon code cannot be used for this subscription"));
    }
    if let Some(limit) = coupon.limit_use_with_user {
        let used = v2board_db::coupon::count_user_coupon_uses(db, coupon.id, user_id)
            .await
            .map_err(|error| {
                tracing::error!(?error, "coupon usage count failed");
                Problem::new(Code::InternalError)
            })?;
        if used >= i64::from(limit) {
            return Err(Problem::new(Code::CouponNotApplicable)
                .with_detail(format!("The coupon can only be used {limit} per person")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn create_order_request_parses_both_arms() {
        let plan: CreateOrderRequest = serde_json::from_value(json!({
            "kind": "plan",
            "plan_id": 2,
            "period": "month_price",
            "coupon_code": "SAVE10",
        }))
        .unwrap();
        match plan {
            CreateOrderRequest::Plan(plan) => {
                assert_eq!(plan.plan_id, 2);
                assert_eq!(plan.period, "month_price");
                assert_eq!(plan.coupon_code.as_deref(), Some("SAVE10"));
            }
            CreateOrderRequest::Deposit(_) => panic!("parsed the wrong arm"),
        }

        let deposit: CreateOrderRequest = serde_json::from_value(json!({
            "kind": "deposit",
            "deposit_amount": 500,
        }))
        .unwrap();
        match deposit {
            CreateOrderRequest::Deposit(deposit) => assert_eq!(deposit.deposit_amount, 500),
            CreateOrderRequest::Plan(_) => panic!("parsed the wrong arm"),
        }
    }

    #[test]
    fn plan_order_periods_are_decoded_at_the_http_boundary() {
        for (wire, expected) in [
            ("month_price", PlanPricePeriod::Month),
            ("quarter_price", PlanPricePeriod::Quarter),
            ("half_year_price", PlanPricePeriod::HalfYear),
            ("year_price", PlanPricePeriod::Year),
            ("two_year_price", PlanPricePeriod::TwoYear),
            ("three_year_price", PlanPricePeriod::ThreeYear),
            ("onetime_price", PlanPricePeriod::OneTime),
            ("reset_price", PlanPricePeriod::Reset),
        ] {
            assert_eq!(plan_price_period_from_wire(wire), Some(expected));
        }
        assert_eq!(plan_price_period_from_wire("deposit"), None);
        assert_eq!(plan_price_period_from_wire("monthly"), None);
    }

    #[test]
    fn create_order_request_rejects_unknown_fields_and_sentinels() {
        // Both arms are deny_unknown_fields (§5.5): typos and the retired
        // legacy sentinel fields are 422-shaped rejections, not silent drops.
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "kind": "plan",
                "plan_id": 2,
                "period": "month_price",
                "deposit_amount": 500,
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "kind": "deposit",
                "deposit_amount": 500,
                "period": "deposit",
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "plan_id": 2,
                "period": "month_price",
            }))
            .is_err(),
            "a body without the kind tag must not parse"
        );
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "kind": "deposit",
            }))
            .is_err(),
            "the deposit arm requires deposit_amount"
        );
    }

    #[test]
    fn checkout_request_is_deny_unknown_fields_with_optional_method() {
        let empty: CheckoutRequest = serde_json::from_value(json!({})).unwrap();
        assert_eq!(empty.method_id, None);
        let chosen: CheckoutRequest = serde_json::from_value(json!({ "method_id": 3 })).unwrap();
        assert_eq!(chosen.method_id, Some(3));
        assert!(serde_json::from_value::<CheckoutRequest>(json!({ "method": 3 })).is_err());
    }

    #[test]
    fn checkout_outcome_serializes_the_spec_union() {
        assert_eq!(
            serde_json::to_value(CheckoutOutcome::QrCode {
                payload: "qr-payload".to_string(),
            })
            .unwrap(),
            json!({ "kind": "qr_code", "payload": "qr-payload" })
        );
        assert_eq!(
            serde_json::to_value(CheckoutOutcome::Redirect {
                url: "https://gateway.example/pay/1".to_string(),
            })
            .unwrap(),
            json!({ "kind": "redirect", "url": "https://gateway.example/pay/1" })
        );
        assert_eq!(
            serde_json::to_value(CheckoutOutcome::Settled).unwrap(),
            json!({ "kind": "settled" })
        );
    }

    #[test]
    fn checkout_outcome_maps_the_legacy_gateway_envelope() {
        let qr = CheckoutOutcome::from_gateway_result(
            v2board_domain::order::CheckoutResult {
                r#type: 0,
                data: json!("qr-payload"),
            },
            "en-US",
        )
        .unwrap();
        assert!(matches!(qr, CheckoutOutcome::QrCode { .. }));
        let settled = CheckoutOutcome::from_gateway_result(
            v2board_domain::order::CheckoutResult {
                r#type: -1,
                data: json!(true),
            },
            "en-US",
        )
        .unwrap();
        assert!(matches!(settled, CheckoutOutcome::Settled));
        // §9.3: unknown type / non-string data is 400 payment_gateway_unsupported.
        let unknown = CheckoutOutcome::from_gateway_result(
            v2board_domain::order::CheckoutResult {
                r#type: 7,
                data: json!(true),
            },
            "en-US",
        )
        .unwrap_err();
        assert_eq!(unknown.code(), Code::PaymentGatewayUnsupported);
        let non_string = CheckoutOutcome::from_gateway_result(
            v2board_domain::order::CheckoutResult {
                r#type: 0,
                data: json!(true),
            },
            "en-US",
        )
        .unwrap_err();
        assert_eq!(non_string.code(), Code::PaymentGatewayUnsupported);
    }

    #[test]
    fn payment_method_percent_becomes_a_number() {
        let body = PaymentMethodBody::from(PaymentMethodRow {
            id: 1,
            name: "EPay".to_string(),
            payment: "EPay".to_string(),
            icon: None,
            handling_fee_fixed: Some(20),
            handling_fee_percent: Some("0.50".to_string()),
        });
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["handling_fee_percent"], json!(0.5));
        assert_eq!(value["handling_fee_fixed"], json!(20));
    }

    #[test]
    fn order_body_serializes_modern_value_types() {
        let body = OrderBody::from(OrderRow {
            trade_no: "trade-1".to_string(),
            callback_no: None,
            plan_id: 0,
            coupon_id: None,
            payment_id: None,
            r#type: 9,
            period: "deposit".to_string(),
            total_amount: 500,
            handling_amount: None,
            discount_amount: None,
            surplus_amount: None,
            refund_amount: None,
            balance_amount: None,
            surplus_order_ids: None,
            status: 0,
            commission_status: 0,
            commission_balance: 0,
            actual_commission_balance: None,
            invite_user_id: None,
            paid_at: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            plan: Some(OrderPlan::Deposit(v2board_db::order::DepositPlan {
                id: 0,
                name: "deposit",
            })),
            try_out_plan_id: None,
            surplus_orders: None,
            bounus: Some(50),
            get_amount: Some(550),
        });
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["status"], json!(0));
        assert_eq!(value["paid_at"], json!(null));
        assert_eq!(value["created_at"], json!("2023-11-14T22:13:20Z"));
        assert_eq!(value["plan"], json!({ "id": 0, "name": "deposit" }));
        assert_eq!(value["bounus"], json!(50));
        assert!(value.get("surplus_orders").is_none());
        assert!(value.get("try_out_plan_id").is_none());
    }

    #[test]
    fn coupon_body_folds_show_flag_and_windows() {
        let body = CouponBody::from(CouponRow {
            id: 1,
            code: "SAVE10".to_string(),
            name: "Save".to_string(),
            r#type: 2,
            value: 10,
            show: 1,
            limit_use: Some(5),
            limit_use_with_user: None,
            limit_plan_ids: None,
            limit_period: None,
            started_at: 1_700_000_000,
            ended_at: 1_700_086_400,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        });
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["show"], json!(true));
        assert_eq!(value["type"], json!(2));
        assert_eq!(value["started_at"], json!("2023-11-14T22:13:20Z"));
        assert_eq!(value["ended_at"], json!("2023-11-15T22:13:20Z"));
    }
}
