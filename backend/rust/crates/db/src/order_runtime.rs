use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use v2board_application::order::{
    AvailablePaymentMethod, BindCheckoutCommand, BindStripeCommand, CancelCandidate,
    CancelOrderCommand, CancelOrderOutcome, CheckoutBindingOutcome, CheckoutPreparation,
    CreateOrderCommand, FulfillmentPolicy, ManualPaymentCandidate, ManualPaymentOutcome,
    OrderFailure, OrderPortError, OrderRepository, PaymentBinding, PaymentMethod,
    PaymentSettlementNotices, PaymentSnapshotVerifier, PendingOrderCandidate, PendingOrderSnapshot,
    PortResult, SettlePaymentCommand, StripePreparation, UserOrder, UserOrderPlan, UserPlanLookup,
};

use crate::order::{CancelPendingOrderError, OrderPlan, OrderRow};

pub(super) const GIB: i64 = 1_073_741_824;
pub(super) const UNFINISHED_ORDER_UNIQUE_KEY: &str = "uniq_unfinished_order_per_user";

#[derive(Clone, Copy, Debug)]
pub(super) enum Code {
    UserNotRegistered,
    PendingOrderExists,
    PlanUnavailable,
    PlanPeriodUnavailable,
    PlanSoldOut,
    PlanChangeDisabled,
    RenewalNotAllowed,
    CouponInvalid,
    CouponUnavailable,
    CouponNotStarted,
    CouponExpired,
    CouponNotApplicable,
    CouponExhausted,
    InsufficientBalance,
    SubscriptionValueOutOfRange,
    HandlingFeeOutOfRange,
    PaymentAmountOutOfRange,
    PaymentMethodUnavailable,
    OrderNotFound,
}

impl From<Code> for OrderFailure {
    fn from(code: Code) -> Self {
        match code {
            Code::UserNotRegistered => Self::UserNotRegistered,
            Code::PendingOrderExists => Self::PendingOrderExists,
            Code::PlanUnavailable => Self::PlanUnavailable,
            Code::PlanPeriodUnavailable => Self::PlanPeriodUnavailable,
            Code::PlanSoldOut => Self::PlanSoldOut,
            Code::PlanChangeDisabled => Self::PlanChangeDisabled,
            Code::RenewalNotAllowed => Self::RenewalNotAllowed,
            Code::CouponInvalid => Self::CouponInvalid,
            Code::CouponUnavailable => Self::CouponUnavailable,
            Code::CouponNotStarted => Self::CouponNotStarted,
            Code::CouponExpired => Self::CouponExpired,
            Code::CouponNotApplicable => Self::CouponNotApplicable,
            Code::CouponExhausted => Self::CouponExhausted,
            Code::InsufficientBalance => Self::InsufficientBalance,
            Code::SubscriptionValueOutOfRange => Self::SubscriptionValueOutOfRange,
            Code::HandlingFeeOutOfRange => Self::HandlingFeeOutOfRange,
            Code::PaymentAmountOutOfRange => Self::PaymentAmountOutOfRange,
            Code::PaymentMethodUnavailable => Self::PaymentMethodUnavailable,
            Code::OrderNotFound => Self::OrderNotFound,
        }
    }
}

pub(super) struct Problem {
    code: Code,
    detail: Option<String>,
}

impl Problem {
    pub(super) const fn new(code: Code) -> Self {
        Self { code, detail: None }
    }

    pub(super) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum PersistenceError {
    #[error("order policy rejected persistence: {failure:?}: {detail}")]
    Business {
        failure: OrderFailure,
        detail: String,
    },
    #[error("order database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("order persistence invariant failed: {0}")]
    Internal(String),
    #[error(transparent)]
    Port(#[from] OrderPortError),
}

pub(super) type ApiError = PersistenceError;
pub(super) type PersistenceResult<T> = Result<T, PersistenceError>;

impl ApiError {
    pub(super) fn internal(detail: impl Into<String>) -> Self {
        Self::Internal(detail.into())
    }

    pub(super) fn legacy(detail: impl Into<String>) -> Self {
        Self::Internal(detail.into())
    }

    pub(super) fn into_port(self, operation: &'static str) -> OrderPortError {
        match self {
            Self::Business { failure, detail } => {
                OrderPortError::business(operation, failure, detail)
            }
            Self::Database(error) => OrderPortError::infrastructure(operation, error),
            Self::Internal(detail) => OrderPortError::infrastructure(operation, detail),
            Self::Port(error) => error,
        }
    }
}

impl From<Problem> for ApiError {
    fn from(problem: Problem) -> Self {
        Self::Business {
            failure: problem.code.into(),
            detail: problem
                .detail
                .unwrap_or_else(|| format!("{:?}", problem.code)),
        }
    }
}

#[derive(Clone)]
pub struct PostgresOrderRepository<V> {
    pub(super) pool: PgPool,
    pub(super) verifier: V,
}

impl<V> PostgresOrderRepository<V>
where
    V: PaymentSnapshotVerifier,
{
    pub fn new(pool: PgPool, verifier: V) -> Self {
        Self { pool, verifier }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DraftOrder {
    pub(super) user_id: i64,
    pub(super) plan_id: i32,
    pub(super) coupon_id: Option<i32>,
    pub(super) kind: i32,
    pub(super) period: String,
    pub(super) trade_no: String,
    pub(super) total_amount: Decimal,
    pub(super) discount_amount: Option<Decimal>,
    pub(super) surplus_amount: Option<Decimal>,
    pub(super) refund_amount: Option<Decimal>,
    pub(super) balance_amount: Option<Decimal>,
    pub(super) surplus_order_ids: Option<Vec<i64>>,
    pub(super) invite_user_id: Option<i64>,
    pub(super) commission_balance: Decimal,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct UserForOrder {
    pub(super) id: i64,
    pub(super) invite_user_id: Option<i64>,
    pub(super) balance: i32,
    pub(super) discount: Option<i32>,
    pub(super) commission_type: i16,
    pub(super) commission_rate: Option<i32>,
    pub(super) traffic_epoch: i64,
    pub(super) u: i64,
    pub(super) d: i64,
    pub(super) transfer_enable: i64,
    pub(super) device_limit: Option<i32>,
    pub(super) banned: i16,
    pub(super) group_id: Option<i32>,
    pub(super) plan_id: Option<i32>,
    pub(super) speed_limit: Option<i32>,
    pub(super) expired_at: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct SurplusOrderRow {
    pub(super) id: i64,
    pub(super) period: String,
    pub(super) total_amount: i32,
    pub(super) balance_amount: Option<i32>,
    pub(super) surplus_amount: Option<i32>,
    pub(super) refund_amount: Option<i32>,
    pub(super) created_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct OrderForCheckout {
    pub(super) id: i64,
    pub(super) user_id: i64,
    pub(super) plan_id: i32,
    pub(super) kind: i32,
    pub(super) period: String,
    pub(super) trade_no: String,
    pub(super) total_amount: i32,
    pub(super) refund_amount: Option<i32>,
    pub(super) surplus_order_ids: Option<String>,
}

#[derive(Clone, FromRow)]
pub(super) struct PaymentForCheckout {
    pub(super) id: i32,
    pub(super) payment: String,
    pub(super) enable: i16,
    pub(super) uuid: String,
    pub(super) config: String,
    pub(super) notify_domain: Option<String>,
    pub(super) handling_fee_fixed: Option<i32>,
    pub(super) handling_fee_percent: Option<Decimal>,
}

impl PaymentForCheckout {
    pub(super) fn application(&self) -> PaymentMethod {
        PaymentMethod {
            id: self.id,
            provider: self.payment.clone(),
            enabled: self.enable == 1,
            uuid: self.uuid.clone(),
            sealed_config: self.config.clone(),
            notify_domain: self.notify_domain.clone(),
            handling_fee_fixed: self.handling_fee_fixed,
            handling_fee_percent: self
                .handling_fee_percent
                .map(|value| value.normalize().to_string()),
        }
    }
}

pub(super) fn infrastructure(
    operation: &'static str,
    error: impl std::fmt::Display,
) -> OrderPortError {
    OrderPortError::infrastructure(operation, error)
}

pub(super) fn business(
    operation: &'static str,
    failure: OrderFailure,
    detail: impl Into<String>,
) -> OrderPortError {
    OrderPortError::business(operation, failure, detail)
}

pub(super) fn payment_identifier_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

pub(super) fn bounded_payment_identifier(value: &str) -> String {
    const MAX_BYTES: usize = 255;
    let mut bounded = String::with_capacity(value.len().min(MAX_BYTES));
    for character in value.chars() {
        if character.len_utf8() == 4 {
            let escaped = format!("\\u{{{:X}}}", u32::from(character));
            if bounded.len() + escaped.len() > MAX_BYTES {
                break;
            }
            bounded.push_str(&escaped);
        } else {
            let mut bytes = [0_u8; 4];
            let encoded = character.encode_utf8(&mut bytes);
            if bounded.len() + encoded.len() > MAX_BYTES {
                break;
            }
            bounded.push_str(encoded);
        }
    }
    bounded
}

fn application_order(row: OrderRow) -> PersistenceResult<UserOrder> {
    let plan = match row.plan {
        Some(OrderPlan::Full(plan)) => {
            Some(UserOrderPlan::Full(Box::new(application_plan(*plan)?)))
        }
        Some(OrderPlan::Deposit(plan)) => Some(UserOrderPlan::Deposit {
            id: plan.id,
            name: plan.name.to_string(),
        }),
        None => None,
    };
    let surplus_orders = row
        .surplus_orders
        .map(|orders| {
            orders
                .into_iter()
                .map(application_order)
                .collect::<PersistenceResult<Vec<_>>>()
        })
        .transpose()?;
    Ok(UserOrder {
        trade_no: row.trade_no,
        callback_no: row.callback_no,
        plan_id: row.plan_id,
        coupon_id: row.coupon_id,
        payment_id: row.payment_id,
        kind: row.r#type,
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
        plan,
        try_out_plan_id: row.try_out_plan_id,
        surplus_orders,
        bonus: row.bounus,
        get_amount: row.get_amount,
    })
}

fn application_plan(
    row: crate::plan::PlanRow,
) -> PersistenceResult<v2board_application::plan::Plan> {
    application_plan_with_count(row, 0)
}

fn application_plan_with_count(
    row: crate::plan::PlanRow,
    count: i64,
) -> PersistenceResult<v2board_application::plan::Plan> {
    let prices = row.prices()?;
    Ok(v2board_application::plan::Plan {
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
        prices,
        reset_traffic_method: row.reset_traffic_method,
        capacity_limit: row.capacity_limit,
        count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

impl<V> OrderRepository for PostgresOrderRepository<V>
where
    V: PaymentSnapshotVerifier,
{
    async fn visible_plans(&self) -> PortResult<Vec<v2board_application::plan::Plan>> {
        let counts = crate::plan::count_capacity_usage_by_plan(&self.pool)
            .await
            .map_err(|error| infrastructure("count visible plan capacity", error))?;
        crate::plan::fetch_visible_plans(&self.pool)
            .await
            .map_err(|error| infrastructure("list visible plans", error))?
            .into_iter()
            .map(|row| {
                let count = counts.get(&row.id).copied().unwrap_or_default();
                application_plan_with_count(row, count)
                    .map_err(|error| error.into_port("map visible plan"))
            })
            .collect()
    }

    async fn user_plan(&self, user_id: i64, plan_id: i32) -> PortResult<UserPlanLookup> {
        let Some(subscription) = crate::user::find_user_subscribe(&self.pool, user_id)
            .await
            .map_err(|error| infrastructure("load user plan subscription", error))?
        else {
            return Ok(UserPlanLookup::UserNotRegistered);
        };
        let Some(row) = crate::plan::find_plan(&self.pool, plan_id)
            .await
            .map_err(|error| infrastructure("load user plan", error))?
        else {
            return Ok(UserPlanLookup::PlanNotFound);
        };
        Ok(UserPlanLookup::Found {
            plan: application_plan(row).map_err(|error| error.into_port("map user plan"))?,
            current_plan_id: subscription.plan_id,
        })
    }

    async fn available_payment_methods(&self) -> PortResult<Vec<AvailablePaymentMethod>> {
        Ok(crate::payment::fetch_enabled_payment_methods(&self.pool)
            .await
            .map_err(|error| infrastructure("list payment methods", error))?
            .into_iter()
            .map(|row| AvailablePaymentMethod {
                id: row.id,
                name: row.name,
                provider: row.payment,
                icon: row.icon,
                handling_fee_fixed: row.handling_fee_fixed,
                handling_fee_percent: row.handling_fee_percent,
            })
            .collect())
    }

    async fn pending_order_candidates(
        &self,
        after_id: i64,
        limit: i64,
    ) -> PortResult<Vec<PendingOrderCandidate>> {
        sqlx::query_as::<_, (i64, String)>(
            r#"
            SELECT id, trade_no
            FROM orders
            WHERE status IN (0, 1) AND id > $1
            ORDER BY id
            LIMIT $2
            "#,
        )
        .bind(after_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, trade_no)| PendingOrderCandidate { id, trade_no })
                .collect()
        })
        .map_err(|error| infrastructure("list pending order candidates", error))
    }

    async fn create_order(&self, command: CreateOrderCommand) -> PortResult<()> {
        self.create_order_inner(command)
            .await
            .map_err(|error| error.into_port("create order"))
    }

    async fn prepare_checkout(
        &self,
        user_id: i64,
        trade_no: &str,
        method_id: Option<i32>,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PortResult<CheckoutPreparation> {
        self.prepare_checkout_inner(user_id, trade_no, method_id, now, fulfillment)
            .await
            .map_err(|error| error.into_port("prepare checkout"))
    }

    async fn bind_checkout(
        &self,
        command: BindCheckoutCommand,
    ) -> PortResult<CheckoutBindingOutcome> {
        self.bind_checkout_inner(command)
            .await
            .map_err(|error| error.into_port("bind checkout"))
    }

    async fn prepare_stripe(
        &self,
        user_id: i64,
        trade_no: &str,
        method_id: i32,
    ) -> PortResult<StripePreparation> {
        self.prepare_stripe_inner(user_id, trade_no, method_id)
            .await
            .map_err(|error| error.into_port("prepare Stripe intent"))
    }

    async fn bind_stripe(&self, command: BindStripeCommand) -> PortResult<CheckoutBindingOutcome> {
        self.bind_stripe_inner(command)
            .await
            .map_err(|error| error.into_port("bind Stripe intent"))
    }

    async fn payment_binding_material(&self, payment_id: i32) -> PortResult<Option<PaymentMethod>> {
        self.payment_binding_material_inner(payment_id)
            .await
            .map_err(|error| error.into_port("load payment binding"))
    }

    async fn cancel_candidate(
        &self,
        user_id: i64,
        trade_no: &str,
    ) -> PortResult<Option<CancelCandidate>> {
        crate::order::find_cancel_candidate(&self.pool, user_id, trade_no)
            .await
            .map(|candidate| {
                candidate.map(|candidate| CancelCandidate {
                    status: candidate.status,
                    balance_amount: candidate.balance_amount,
                    binding: PaymentBinding {
                        payment_id: candidate.payment_id,
                        callback_no: candidate.callback_no,
                    },
                })
            })
            .map_err(|error| infrastructure("load cancellation candidate", error))
    }

    async fn cancel_order(&self, command: CancelOrderCommand) -> PortResult<CancelOrderOutcome> {
        let cancelled = crate::order::cancel_pending_order(
            &self.pool,
            command.user_id,
            &command.trade_no,
            command.candidate.balance_amount,
            command.candidate.binding.payment_id,
            command.candidate.binding.callback_no.as_deref(),
            command.now,
        )
        .await
        .map_err(|error| match error {
            CancelPendingOrderError::Database(error) => infrastructure("cancel order", error),
            CancelPendingOrderError::BalanceOverflow | CancelPendingOrderError::UserNotFound => {
                business(
                    "cancel order",
                    OrderFailure::OrderUpdateFailed,
                    error.to_string(),
                )
            }
        })?;
        Ok(if cancelled {
            CancelOrderOutcome::Cancelled
        } else {
            CancelOrderOutcome::Changed
        })
    }

    async fn payment_for_notification(
        &self,
        provider: &str,
        uuid: &str,
    ) -> PortResult<Option<PaymentMethod>> {
        self.payment_for_notification_inner(provider, uuid)
            .await
            .map_err(|error| error.into_port("load notification payment"))
    }

    async fn settle_payment(
        &self,
        command: SettlePaymentCommand,
    ) -> PortResult<PaymentSettlementNotices> {
        self.settle_payment_inner(command)
            .await
            .map_err(|error| error.into_port("settle payment"))
    }

    async fn manual_payment_candidate(
        &self,
        trade_no: &str,
    ) -> PortResult<Option<ManualPaymentCandidate>> {
        self.manual_payment_candidate_inner(trade_no)
            .await
            .map_err(|error| error.into_port("load manual settlement candidate"))
    }

    async fn settle_manually(
        &self,
        trade_no: &str,
        expected: ManualPaymentCandidate,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PortResult<ManualPaymentOutcome> {
        self.settle_manually_inner(trade_no, expected, now, fulfillment)
            .await
            .map_err(|error| error.into_port("settle order manually"))
    }

    async fn pending_order(&self, trade_no: &str) -> PortResult<Option<PendingOrderSnapshot>> {
        self.pending_order_inner(trade_no)
            .await
            .map_err(|error| error.into_port("load pending order"))
    }

    async fn process_pending_order(
        &self,
        trade_no: &str,
        expected: PendingOrderSnapshot,
        expire_pending: bool,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PortResult<()> {
        self.process_pending_order_inner(trade_no, expected, expire_pending, now, fulfillment)
            .await
            .map_err(|error| error.into_port("process pending order"))
    }

    async fn user_orders(&self, user_id: i64, status: Option<i16>) -> PortResult<Vec<UserOrder>> {
        let rows = crate::order::fetch_user_orders(&self.pool, user_id, status)
            .await
            .map_err(|error| infrastructure("list user orders", error))?;
        rows.into_iter()
            .map(application_order)
            .collect::<PersistenceResult<Vec<_>>>()
            .map_err(|error| error.into_port("map user orders"))
    }

    async fn user_order(
        &self,
        user_id: i64,
        trade_no: &str,
        try_out_plan_id: i32,
    ) -> PortResult<Option<UserOrder>> {
        crate::order::find_user_order(&self.pool, user_id, trade_no, try_out_plan_id)
            .await
            .map_err(|error| infrastructure("load user order", error))?
            .map(application_order)
            .transpose()
            .map_err(|error| error.into_port("map user order"))
    }

    async fn user_order_status(&self, user_id: i64, trade_no: &str) -> PortResult<Option<i16>> {
        crate::order::find_order_status(&self.pool, user_id, trade_no)
            .await
            .map_err(|error| infrastructure("load user order status", error))
    }
}
