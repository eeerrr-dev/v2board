//! User commerce family — modern dialect (docs/api-dialect.md §5.5, §9.3,
//! §9.4, Appendix A §W4): bare success bodies on modern value types
//! (RFC 3339 timestamps, boolean flags, numeric `handling_fee_percent`),
//! path-borne `trade_no`/plan ids, the discriminated create-order request
//! union, the §9.3 checkout result union, and problem+json failures.

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
pub(crate) use v2board_api_contract::{
    common::CreatedTradeNo as CreatedOrder,
    user::UserPlan as PlanBody,
    user_commerce::{
        CheckoutOutcome, Coupon as CouponBody, OrderPlan as OrderPlanBody,
        OrderStatus as OrderStatusBody, PaymentMethod as PaymentMethodBody, UserOrder as OrderBody,
    },
};
use v2board_api_contract::{
    time::Rfc3339Timestamp,
    user_commerce::{
        CheckoutRequest, CouponCheckRequest, CreateOrderRequest, DepositOrderPlan,
        StripeIntentRequest, StripePaymentIntent,
    },
};
use v2board_application::{
    auth::AuthUser,
    order::{
        AvailablePaymentMethod, CheckoutOutcome as ApplicationCheckoutOutcome, OrderError,
        OrderFailure, SaveOrderInput, StripePaymentIntent as ApplicationStripePaymentIntent,
        UserOrder as ApplicationUserOrder, UserOrderPlan as ApplicationUserOrderPlan,
    },
    plan::Plan as ApplicationPlan,
    promotion::PromotionError,
};
use v2board_compat::{ApiError, Code, Problem};
use v2board_domain_model::{Coupon, CouponRuleViolation, MoneyMinor, PlanPricePeriod};

use crate::{
    auth::require_privileged_step_up, dialect::DialectJson, dialect::problem_from,
    locale::request_locale, runtime::AppState,
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

pub(crate) fn plan_body(plan: ApplicationPlan) -> PlanBody {
    let price = |period| plan.prices.get(period).map(MoneyMinor::get);
    PlanBody {
        id: plan.id,
        group_id: plan.group_id,
        transfer_enable: plan.transfer_enable,
        device_limit: plan.device_limit,
        name: plan.name,
        speed_limit: plan.speed_limit,
        show: plan.show,
        sort: plan.sort,
        renew: plan.renew,
        content: plan.content,
        month_price: price(PlanPricePeriod::Month),
        quarter_price: price(PlanPricePeriod::Quarter),
        half_year_price: price(PlanPricePeriod::HalfYear),
        year_price: price(PlanPricePeriod::Year),
        two_year_price: price(PlanPricePeriod::TwoYear),
        three_year_price: price(PlanPricePeriod::ThreeYear),
        onetime_price: price(PlanPricePeriod::OneTime),
        reset_price: price(PlanPricePeriod::Reset),
        reset_traffic_method: plan.reset_traffic_method,
        capacity_limit: plan.capacity_limit,
        created_at: Rfc3339Timestamp::from_epoch_seconds(plan.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(plan.updated_at),
    }
}

fn order_plan_body(plan: ApplicationUserOrderPlan) -> OrderPlanBody {
    match plan {
        ApplicationUserOrderPlan::Full(plan) => OrderPlanBody::Full(Box::new(plan_body(*plan))),
        ApplicationUserOrderPlan::Deposit { id, name } => {
            OrderPlanBody::Deposit(DepositOrderPlan { id, name })
        }
    }
}

pub(crate) fn order_body(row: ApplicationUserOrder) -> OrderBody {
    OrderBody {
        trade_no: row.trade_no,
        callback_no: row.callback_no,
        plan_id: row.plan_id,
        coupon_id: row.coupon_id,
        payment_id: row.payment_id,
        r#type: row.kind,
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
        paid_at: row.paid_at.map(Rfc3339Timestamp::from_epoch_seconds),
        created_at: Rfc3339Timestamp::from_epoch_seconds(row.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(row.updated_at),
        plan: row.plan.map(order_plan_body),
        try_out_plan_id: row.try_out_plan_id,
        surplus_orders: row
            .surplus_orders
            .map(|orders| orders.into_iter().map(order_body).collect()),
        bounus: row.bonus,
        get_amount: row.get_amount,
    }
}

fn order_problem(error: OrderError, locale: &str) -> Problem {
    let code = match error.failure() {
        Some(OrderFailure::UserNotRegistered) => Code::UserNotRegistered,
        Some(OrderFailure::PendingOrderExists) => Code::PendingOrderExists,
        Some(OrderFailure::PlanNotFound) => Code::PlanNotFound,
        Some(OrderFailure::PlanUnavailable) => Code::PlanUnavailable,
        Some(OrderFailure::PlanPeriodUnavailable) => Code::PlanPeriodUnavailable,
        Some(OrderFailure::PlanSoldOut) => Code::PlanSoldOut,
        Some(OrderFailure::PlanChangeDisabled) => Code::PlanChangeDisabled,
        Some(OrderFailure::RenewalNotAllowed) => Code::RenewalNotAllowed,
        Some(OrderFailure::CouponInvalid) => Code::CouponInvalid,
        Some(OrderFailure::CouponUnavailable) => Code::CouponUnavailable,
        Some(OrderFailure::CouponNotStarted) => Code::CouponNotStarted,
        Some(OrderFailure::CouponExpired) => Code::CouponExpired,
        Some(OrderFailure::CouponNotApplicable) => Code::CouponNotApplicable,
        Some(OrderFailure::CouponExhausted) => Code::CouponExhausted,
        Some(OrderFailure::InsufficientBalance) => Code::InsufficientBalance,
        Some(OrderFailure::SubscriptionValueOutOfRange) => Code::SubscriptionValueOutOfRange,
        Some(OrderFailure::HandlingFeeOutOfRange) => Code::HandlingFeeOutOfRange,
        Some(OrderFailure::PaymentAmountOutOfRange) => Code::PaymentAmountOutOfRange,
        Some(OrderFailure::PaymentMethodUnavailable) => Code::PaymentMethodUnavailable,
        Some(OrderFailure::PaymentConfigInvalid) => Code::PaymentConfigInvalid,
        Some(OrderFailure::PaymentGatewayUnsupported) => Code::PaymentGatewayUnsupported,
        Some(OrderFailure::StripeBindingInvalid) => Code::StripeBindingInvalid,
        Some(OrderFailure::OrderNotFound) => Code::OrderNotFound,
        Some(OrderFailure::OrderNotPending) => Code::OrderNotPending,
        Some(OrderFailure::OrderUpdateFailed) => Code::OrderUpdateFailed,
        None => {
            tracing::error!(?error, "order application service failed");
            Code::InternalError
        }
    };
    Problem::localized(code, locale)
}

pub(crate) fn payment_method_body(row: AvailablePaymentMethod) -> PaymentMethodBody {
    PaymentMethodBody {
        id: row.id,
        name: row.name,
        payment: row.provider,
        icon: row.icon,
        handling_fee_percent: row
            .handling_fee_percent
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite()),
        handling_fee_fixed: row.handling_fee_fixed,
    }
}

pub(crate) fn coupon_body(row: Coupon) -> CouponBody {
    CouponBody {
        id: row.id,
        code: row.code,
        name: row.name,
        r#type: row.kind_code,
        value: row.value,
        show: row.visible,
        limit_use: row.remaining_uses,
        limit_use_with_user: row.per_user_limit,
        limit_plan_ids: row.plan_ids,
        limit_period: row.periods,
        started_at: Rfc3339Timestamp::from_epoch_seconds(row.starts_at),
        ended_at: Rfc3339Timestamp::from_epoch_seconds(row.ends_at),
        created_at: Rfc3339Timestamp::from_epoch_seconds(row.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(row.updated_at),
    }
}

/// GET /user/plans — bare array of visible plans with the legacy remaining-
/// capacity rewrite (§5.5).
pub(crate) async fn plans_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PlanBody>>, Problem> {
    let locale = request_locale(&headers);
    let plans = state
        .order_service()
        .catalog_plans()
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(plans.into_iter().map(plan_body).collect()))
}

/// GET /user/plans/{id} — bare plan; a path-identified miss (including the
/// legacy hidden-plan availability rules) is 404 plan_not_found (§3.4).
pub(crate) async fn plan_detail(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<PlanBody>, Problem> {
    let locale = request_locale(&headers);
    let plan = state
        .order_service()
        .catalog_plan(user.id, id)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(plan_body(plan)))
}

/// POST /user/orders — create from the discriminated union (§5.5), 201 with
/// the created identity.
pub(crate) async fn order_create(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CreateOrderRequest>,
) -> Result<(StatusCode, Json<CreatedOrder>), Problem> {
    let locale = request_locale(&headers);
    let input = match request {
        CreateOrderRequest::Plan(plan) => {
            let period = plan_price_period_from_wire(&plan.period).ok_or_else(|| {
                commerce_problem(Code::PlanPeriodUnavailable, "Wrong plan period", locale)
            })?;
            SaveOrderInput::Plan {
                plan_id: plan.plan_id,
                period,
                coupon_code: plan.coupon_code,
            }
        }
        CreateOrderRequest::Deposit(deposit) => SaveOrderInput::Deposit {
            deposit_amount: deposit.deposit_amount,
        },
    };
    let trade_no = state
        .order_service()
        .save(user.id, input)
        .await
        .map_err(|error| order_problem(error, locale))?;
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
    Extension(user): Extension<AuthUser>,
    Query(query): Query<OrdersListQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<OrderBody>>, Problem> {
    let locale = request_locale(&headers);
    let orders = state
        .order_service()
        .orders(user.id, query.status)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(orders.into_iter().map(order_body).collect()))
}

/// GET /user/orders/{trade_no} — bare order; a miss is 404 order_not_found
/// (§3.4: every modern order route carries `trade_no` in the path).
pub(crate) async fn order_detail(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderBody>, Problem> {
    let locale = request_locale(&headers);
    let order = state
        .order_service()
        .order(user.id, &trade_no)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(order_body(order)))
}

/// GET /user/orders/{trade_no}/status — bare `{status}` (§5.5/§9.4).
pub(crate) async fn order_status(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderStatusBody>, Problem> {
    let locale = request_locale(&headers);
    let status = state
        .order_service()
        .status(user.id, &trade_no)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(OrderStatusBody { status }))
}

/// POST /user/orders/{trade_no}/cancel — `{trade_no}` body → path segment,
/// boolean body → 204/problem (§5.5).
pub(crate) async fn order_cancel(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .order_service()
        .cancel(user.id, trade_no)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Map the provider-neutral domain result onto the closed checkout wire union.
fn checkout_outcome(result: ApplicationCheckoutOutcome) -> CheckoutOutcome {
    match result {
        ApplicationCheckoutOutcome::QrCode(payload) => CheckoutOutcome::QrCode { payload },
        ApplicationCheckoutOutcome::Redirect(url) => CheckoutOutcome::Redirect { url },
        ApplicationCheckoutOutcome::Settled => CheckoutOutcome::Settled,
    }
}

pub(crate) fn stripe_payment_intent(intent: ApplicationStripePaymentIntent) -> StripePaymentIntent {
    StripePaymentIntent {
        public_key: intent.public_key,
        client_secret: intent.client_secret,
        amount: intent.amount,
        currency: intent.currency,
    }
}

/// POST /user/orders/{trade_no}/checkout — the §9.3 union. Money-initiating
/// (settles from balance/gift-card funds or hands off to an external
/// gateway), so it carries the same step-up gate as privileged admin
/// mutations: a recent password authentication or a valid `x-v2board-step-up`
/// token, else 403 `step_up_required` (never a session teardown).
pub(crate) async fn order_checkout(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CheckoutRequest>,
) -> Result<Json<CheckoutOutcome>, Problem> {
    let locale = request_locale(&headers);
    require_privileged_step_up(&state, &headers, &user)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let result = state
        .order_service()
        .checkout(user.id, trade_no, request.method_id)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(checkout_outcome(result)))
}

/// POST /user/orders/{trade_no}/stripe-intent — bare
/// `{public_key, client_secret, amount, currency}` (§5.5). The Stripe
/// external payloads behind it are byte-frozen (§2). Creating a real Stripe
/// PaymentIntent is money-initiating, so it carries the same step-up gate as
/// `order_checkout`.
pub(crate) async fn order_stripe_intent(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<StripeIntentRequest>,
) -> Result<Json<StripePaymentIntent>, Problem> {
    let locale = request_locale(&headers);
    require_privileged_step_up(&state, &headers, &user)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let intent = state
        .order_service()
        .prepare_stripe_intent(user.id, trade_no, request.method_id)
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(stripe_payment_intent(intent)))
}

/// GET /user/payment-methods — bare array (§5.5).
pub(crate) async fn payment_methods(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PaymentMethodBody>>, Problem> {
    let locale = request_locale(&headers);
    let methods = state
        .order_service()
        .payment_methods()
        .await
        .map_err(|error| order_problem(error, locale))?;
    Ok(Json(methods.into_iter().map(payment_method_body).collect()))
}

pub(crate) async fn coupon_check(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(request): DialectJson<CouponCheckRequest>,
) -> Result<Json<CouponBody>, Problem> {
    let locale = request_locale(&headers);
    let coupon = state
        .promotion_service()
        .check_coupon(
            user.id,
            &request.code,
            request.plan_id,
            Utc::now().timestamp(),
        )
        .await
        .map_err(|error| coupon_problem(error, locale))?;
    Ok(Json(coupon_body(coupon)))
}

fn coupon_problem(error: PromotionError, locale: &str) -> Problem {
    let problem = match error {
        PromotionError::CouponCodeEmpty => {
            Problem::new(Code::CouponInvalid).with_detail("Coupon cannot be empty")
        }
        PromotionError::CouponInvalid => Problem::new(Code::CouponInvalid),
        PromotionError::CouponRule(CouponRuleViolation::InvalidDiscount) => {
            Problem::new(Code::CouponInvalid).with_detail("Invalid coupon discount value")
        }
        PromotionError::CouponRule(CouponRuleViolation::Hidden) => {
            Problem::new(Code::CouponInvalid)
        }
        PromotionError::CouponRule(CouponRuleViolation::Unavailable) => {
            Problem::new(Code::CouponUnavailable)
        }
        PromotionError::CouponRule(CouponRuleViolation::NotStarted) => {
            Problem::new(Code::CouponNotStarted)
        }
        PromotionError::CouponRule(CouponRuleViolation::Expired) => {
            Problem::new(Code::CouponExpired)
        }
        PromotionError::CouponRule(CouponRuleViolation::PlanNotApplicable) => {
            Problem::new(Code::CouponNotApplicable)
                .with_detail("The coupon code cannot be used for this subscription")
        }
        PromotionError::CouponRule(CouponRuleViolation::PeriodNotApplicable) => {
            Problem::new(Code::CouponNotApplicable)
                .with_detail("The coupon code cannot be used for this period")
        }
        PromotionError::CouponRule(CouponRuleViolation::UserLimitExceeded(limit)) => {
            Problem::new(Code::CouponNotApplicable)
                .with_detail(format!("The coupon can only be used {limit} per person"))
        }
        PromotionError::Repository(error) => {
            return problem_from(ApiError::internal(error.to_string()), locale);
        }
        other => {
            tracing::error!(?other, "unexpected coupon-check application error");
            return Problem::localized(Code::InternalError, locale);
        }
    };
    problem_from(problem.into(), locale)
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
    fn checkout_outcome_maps_the_application_union() {
        let qr = checkout_outcome(ApplicationCheckoutOutcome::QrCode("qr-payload".to_string()));
        assert!(matches!(qr, CheckoutOutcome::QrCode { .. }));
        let settled = checkout_outcome(ApplicationCheckoutOutcome::Settled);
        assert!(matches!(settled, CheckoutOutcome::Settled));
    }

    #[test]
    fn payment_method_percent_becomes_a_number() {
        let body = payment_method_body(AvailablePaymentMethod {
            id: 1,
            name: "EPay".to_string(),
            provider: "EPay".to_string(),
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
        let body = order_body(ApplicationUserOrder {
            trade_no: "trade-1".to_string(),
            callback_no: None,
            plan_id: 0,
            coupon_id: None,
            payment_id: None,
            kind: 9,
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
            plan: Some(ApplicationUserOrderPlan::Deposit {
                id: 0,
                name: "deposit".to_string(),
            }),
            try_out_plan_id: None,
            surplus_orders: None,
            bonus: Some(50),
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
        let body = coupon_body(Coupon {
            id: 1,
            code: "SAVE10".to_string(),
            name: "Save".to_string(),
            kind_code: 2,
            value: 10,
            visible: true,
            remaining_uses: Some(5),
            per_user_limit: None,
            plan_ids: None,
            periods: None,
            starts_at: 1_700_000_000,
            ends_at: 1_700_086_400,
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
