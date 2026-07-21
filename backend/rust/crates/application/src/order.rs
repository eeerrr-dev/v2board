//! User-order and payment-settlement use cases.
//!
//! The application layer owns sequencing across the transactional order
//! repository and the external payment gateway.  SQL row locks, encrypted
//! provider configuration, HTTP, wall-clock access, and wire serialization
//! remain in outer adapters.

use std::{collections::BTreeMap, fmt::Display};

use v2board_domain_model::PlanPricePeriod;

use crate::plan::Plan;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderFailure {
    UserNotRegistered,
    PendingOrderExists,
    PlanNotFound,
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
    PaymentConfigInvalid,
    PaymentGatewayUnsupported,
    StripeBindingInvalid,
    OrderNotFound,
    OrderNotPending,
    OrderUpdateFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{operation} failed: {message}")]
pub struct OrderPortError {
    operation: &'static str,
    message: String,
    failure: Option<OrderFailure>,
}

impl OrderPortError {
    pub fn infrastructure(operation: &'static str, error: impl Display) -> Self {
        Self {
            operation,
            message: error.to_string(),
            failure: None,
        }
    }

    pub fn business(
        operation: &'static str,
        failure: OrderFailure,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            message: detail.into(),
            failure: Some(failure),
        }
    }

    pub const fn operation(&self) -> &'static str {
        self.operation
    }

    pub const fn failure(&self) -> Option<OrderFailure> {
        self.failure
    }
}

pub type PortResult<T> = Result<T, OrderPortError>;

#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("order operation was rejected: {0:?}")]
    Business(OrderFailure),
    #[error(transparent)]
    Port(#[from] OrderPortError),
    #[error("payment notification failed")]
    PaymentNotificationFailed,
}

impl OrderError {
    pub const fn failure(&self) -> Option<OrderFailure> {
        match self {
            Self::Business(failure) => Some(*failure),
            Self::Port(error) => error.failure(),
            Self::PaymentNotificationFailed => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveOrderInput {
    Plan {
        plan_id: i32,
        period: PlanPricePeriod,
        coupon_code: Option<String>,
    },
    Deposit {
        deposit_amount: i32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOrderPolicy {
    pub plan_change_enabled: bool,
    pub surplus_enabled: bool,
    pub commission_first_time_enabled: bool,
    pub default_commission_rate: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FulfillmentPolicy {
    pub deposit_bonus: i32,
    pub new_order_event_id: i32,
    pub renewal_order_event_id: i32,
    pub change_order_event_id: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateOrderCommand {
    pub user_id: i64,
    pub input: SaveOrderInput,
    pub trade_no: String,
    pub now: i64,
    pub policy: CreateOrderPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentMethod {
    pub id: i32,
    pub provider: String,
    pub enabled: bool,
    pub uuid: String,
    /// Authenticated at-rest envelope. Only the payment adapter may open it.
    pub sealed_config: String,
    pub notify_domain: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    pub handling_fee_percent: Option<String>,
}

/// Public payment-method projection. Provider credentials and webhook routing
/// material deliberately never cross this application boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvailablePaymentMethod {
    pub id: i32,
    pub name: String,
    pub provider: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    pub handling_fee_percent: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserPlanLookup {
    UserNotRegistered,
    PlanNotFound,
    Found {
        plan: Plan,
        current_plan_id: Option<i32>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingOrderCandidate {
    pub id: i64,
    pub trade_no: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentBinding {
    pub payment_id: Option<i32>,
    pub callback_no: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayOrder {
    pub trade_no: String,
    pub total_amount: i32,
    pub user_id: i64,
    pub user_email: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckoutPreparation {
    Settled,
    Gateway {
        order_id: i64,
        payment: Box<PaymentMethod>,
        order: GatewayOrder,
        previous_binding: PaymentBinding,
        handling_amount: Option<i32>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckoutBindingOutcome {
    Bound,
    PaymentChanged,
    OrderChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindCheckoutCommand {
    pub order_id: i64,
    pub payment: PaymentMethod,
    pub previous_binding: PaymentBinding,
    pub handling_amount: Option<i32>,
    pub now: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StripePreparation {
    pub order_id: i64,
    pub payment: PaymentMethod,
    pub order: GatewayOrder,
    pub previous_binding: PaymentBinding,
    pub reusable_intent: Option<String>,
    pub handling_amount: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindStripeCommand {
    pub order_id: i64,
    pub payment: PaymentMethod,
    pub previous_binding: PaymentBinding,
    pub intent_id: String,
    pub handling_amount: Option<i32>,
    pub now: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckoutOutcome {
    QrCode(String),
    Redirect(String),
    Settled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StripePaymentIntent {
    pub public_key: String,
    pub client_secret: String,
    pub amount: i64,
    pub currency: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedStripeIntent {
    pub intent_id: String,
    pub response: StripePaymentIntent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelCandidate {
    pub status: i16,
    pub balance_amount: Option<i32>,
    pub binding: PaymentBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelOrderCommand {
    pub user_id: i64,
    pub trade_no: String,
    pub candidate: CancelCandidate,
    pub now: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOrderOutcome {
    Cancelled,
    Changed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PaymentNotifyInput {
    pub params: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPaymentNotification {
    pub trade_no: String,
    pub callback_no: String,
    pub custom_response: Option<String>,
    pub authenticated_user_id: Option<i64>,
    pub settled_amount_cents: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaymentVerification {
    Verified(VerifiedPaymentNotification),
    Ignored(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlePaymentCommand {
    pub payment: PaymentMethod,
    pub notification: VerifiedPaymentNotification,
    pub now: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaidOrderNotice {
    pub trade_no: String,
    pub total_amount: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatePaymentNotice {
    pub trade_no: String,
    pub trade_no_hash: String,
    pub callback_no: String,
    pub callback_no_hash: String,
    pub reason: String,
    pub order_status: i16,
    pub expected_amount: i64,
    pub settled_amount: Option<i64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PaymentSettlementNotices {
    pub paid: Option<PaidOrderNotice>,
    pub late: Option<LatePaymentNotice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentNotifyResponse {
    pub body: String,
    pub paid_notice: Option<PaidOrderNotice>,
    pub late_payment_notice: Option<LatePaymentNotice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManualPaymentCandidate {
    pub status: i16,
    pub binding: PaymentBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManualPaymentOutcome {
    Settled,
    NotFound,
    Changed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingOrderSnapshot {
    pub status: i16,
    pub created_at: i64,
    pub total_amount: i32,
    pub binding: PaymentBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserOrderPlan {
    Full(Box<Plan>),
    Deposit { id: i32, name: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserOrder {
    pub trade_no: String,
    pub callback_no: Option<String>,
    pub plan_id: i32,
    pub coupon_id: Option<i32>,
    pub payment_id: Option<i32>,
    pub kind: i32,
    pub period: String,
    pub total_amount: i32,
    pub handling_amount: Option<i32>,
    pub discount_amount: Option<i32>,
    pub surplus_amount: Option<i32>,
    pub refund_amount: Option<i32>,
    pub balance_amount: Option<i32>,
    pub surplus_order_ids: Option<Vec<i64>>,
    pub status: i16,
    pub commission_status: i16,
    pub commission_balance: i32,
    pub actual_commission_balance: Option<i32>,
    pub invite_user_id: Option<i64>,
    pub paid_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub plan: Option<UserOrderPlan>,
    pub try_out_plan_id: Option<i32>,
    pub surplus_orders: Option<Vec<UserOrder>>,
    pub bonus: Option<i32>,
    pub get_amount: Option<i32>,
}

#[allow(async_fn_in_trait)]
pub trait OrderRepository: Send + Sync {
    async fn visible_plans(&self) -> PortResult<Vec<Plan>>;
    async fn user_plan(&self, user_id: i64, plan_id: i32) -> PortResult<UserPlanLookup>;
    async fn available_payment_methods(&self) -> PortResult<Vec<AvailablePaymentMethod>>;
    async fn pending_order_candidates(
        &self,
        after_id: i64,
        limit: i64,
    ) -> PortResult<Vec<PendingOrderCandidate>>;
    async fn create_order(&self, command: CreateOrderCommand) -> PortResult<()>;
    async fn prepare_checkout(
        &self,
        user_id: i64,
        trade_no: &str,
        method_id: Option<i32>,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PortResult<CheckoutPreparation>;
    async fn bind_checkout(
        &self,
        command: BindCheckoutCommand,
    ) -> PortResult<CheckoutBindingOutcome>;
    async fn prepare_stripe(
        &self,
        user_id: i64,
        trade_no: &str,
        method_id: i32,
    ) -> PortResult<StripePreparation>;
    async fn bind_stripe(&self, command: BindStripeCommand) -> PortResult<CheckoutBindingOutcome>;
    async fn payment_binding_material(&self, payment_id: i32) -> PortResult<Option<PaymentMethod>>;
    async fn cancel_candidate(
        &self,
        user_id: i64,
        trade_no: &str,
    ) -> PortResult<Option<CancelCandidate>>;
    async fn cancel_order(&self, command: CancelOrderCommand) -> PortResult<CancelOrderOutcome>;
    async fn payment_for_notification(
        &self,
        provider: &str,
        uuid: &str,
    ) -> PortResult<Option<PaymentMethod>>;
    async fn settle_payment(
        &self,
        command: SettlePaymentCommand,
    ) -> PortResult<PaymentSettlementNotices>;
    async fn manual_payment_candidate(
        &self,
        trade_no: &str,
    ) -> PortResult<Option<ManualPaymentCandidate>>;
    async fn settle_manually(
        &self,
        trade_no: &str,
        expected: ManualPaymentCandidate,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PortResult<ManualPaymentOutcome>;
    async fn pending_order(&self, trade_no: &str) -> PortResult<Option<PendingOrderSnapshot>>;
    async fn process_pending_order(
        &self,
        trade_no: &str,
        expected: PendingOrderSnapshot,
        expire_pending: bool,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PortResult<()>;
    async fn user_orders(&self, user_id: i64, status: Option<i16>) -> PortResult<Vec<UserOrder>>;
    async fn user_order(
        &self,
        user_id: i64,
        trade_no: &str,
        try_out_plan_id: i32,
    ) -> PortResult<Option<UserOrder>>;
    async fn user_order_status(&self, user_id: i64, trade_no: &str) -> PortResult<Option<i16>>;
}

pub trait PaymentSnapshotVerifier: Clone + Send + Sync {
    fn equivalent(&self, expected: &PaymentMethod, current: &PaymentMethod) -> PortResult<bool>;
}

#[allow(async_fn_in_trait)]
pub trait PaymentGateway: PaymentSnapshotVerifier {
    async fn checkout(
        &self,
        payment: &PaymentMethod,
        order: &GatewayOrder,
    ) -> PortResult<CheckoutOutcome>;
    async fn prepare_stripe_intent(
        &self,
        payment: &PaymentMethod,
        order: &GatewayOrder,
        reusable_intent: Option<&str>,
    ) -> PortResult<PreparedStripeIntent>;
    async fn cancel_stripe_intent(
        &self,
        payment: Option<&PaymentMethod>,
        intent_id: Option<&str>,
    ) -> PortResult<bool>;
    async fn verify_notification(
        &self,
        payment: &PaymentMethod,
        input: &PaymentNotifyInput,
    ) -> PortResult<PaymentVerification>;
}

pub trait OrderClock: Clone + Send + Sync {
    fn now(&self) -> i64;
}

pub trait OrderNumberGenerator: Clone + Send + Sync {
    fn generate(&self) -> String;
}

pub trait OrderPolicy: Clone + Send + Sync {
    fn create_policy(&self) -> CreateOrderPolicy;
    fn fulfillment_policy(&self, total_amount: i32) -> FulfillmentPolicy;
    fn try_out_plan_id(&self) -> i32;
}

#[derive(Clone)]
pub struct OrderService<R, G, C, N, P> {
    repository: R,
    gateway: G,
    clock: C,
    numbers: N,
    policy: P,
}

impl<R, G, C, N, P> OrderService<R, G, C, N, P>
where
    R: OrderRepository,
    G: PaymentGateway,
    C: OrderClock,
    N: OrderNumberGenerator,
    P: OrderPolicy,
{
    pub const fn new(repository: R, gateway: G, clock: C, numbers: N, policy: P) -> Self {
        Self {
            repository,
            gateway,
            clock,
            numbers,
            policy,
        }
    }

    pub async fn save(&self, user_id: i64, input: SaveOrderInput) -> Result<String, OrderError> {
        let trade_no = self.numbers.generate();
        self.repository
            .create_order(CreateOrderCommand {
                user_id,
                input,
                trade_no: trade_no.clone(),
                now: self.clock.now(),
                policy: self.policy.create_policy(),
            })
            .await?;
        Ok(trade_no)
    }

    pub async fn catalog_plans(&self) -> Result<Vec<Plan>, OrderError> {
        let mut plans = self.repository.visible_plans().await?;
        for plan in &mut plans {
            if let Some(capacity) = plan.capacity_limit {
                let remaining = i64::from(capacity)
                    .checked_sub(plan.count)
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| {
                        OrderPortError::infrastructure(
                            "calculate visible plan capacity",
                            format!(
                                "plan {} capacity {capacity} and usage {} are outside range",
                                plan.id, plan.count
                            ),
                        )
                    })?;
                plan.capacity_limit = Some(remaining);
            }
        }
        Ok(plans)
    }

    pub async fn catalog_plan(&self, user_id: i64, plan_id: i32) -> Result<Plan, OrderError> {
        match self.repository.user_plan(user_id, plan_id).await? {
            UserPlanLookup::UserNotRegistered => {
                Err(OrderError::Business(OrderFailure::UserNotRegistered))
            }
            UserPlanLookup::PlanNotFound => Err(OrderError::Business(OrderFailure::PlanNotFound)),
            UserPlanLookup::Found {
                plan,
                current_plan_id,
            } if !plan.show && (!plan.renew || current_plan_id != Some(plan.id)) => {
                Err(OrderError::Business(OrderFailure::PlanNotFound))
            }
            UserPlanLookup::Found { plan, .. } => Ok(plan),
        }
    }

    pub async fn payment_methods(&self) -> Result<Vec<AvailablePaymentMethod>, OrderError> {
        Ok(self.repository.available_payment_methods().await?)
    }

    pub async fn pending_candidates(
        &self,
        after_id: i64,
        limit: i64,
    ) -> Result<Vec<PendingOrderCandidate>, OrderError> {
        Ok(self
            .repository
            .pending_order_candidates(after_id, limit)
            .await?)
    }

    pub async fn checkout(
        &self,
        user_id: i64,
        trade_no: String,
        method_id: Option<i32>,
    ) -> Result<CheckoutOutcome, OrderError> {
        let now = self.clock.now();
        let preparation = self
            .repository
            .prepare_checkout(
                user_id,
                &trade_no,
                method_id,
                now,
                self.policy.fulfillment_policy(0),
            )
            .await?;
        let CheckoutPreparation::Gateway {
            order_id,
            payment,
            order,
            previous_binding,
            handling_amount,
        } = preparation
        else {
            return Ok(CheckoutOutcome::Settled);
        };
        self.cancel_binding(&previous_binding).await?;
        match self
            .repository
            .bind_checkout(BindCheckoutCommand {
                order_id,
                payment: payment.as_ref().clone(),
                previous_binding,
                handling_amount,
                now: self.clock.now(),
            })
            .await?
        {
            CheckoutBindingOutcome::Bound => self
                .gateway
                .checkout(&payment, &order)
                .await
                .map_err(Into::into),
            CheckoutBindingOutcome::PaymentChanged => {
                Err(OrderError::Business(OrderFailure::PaymentConfigInvalid))
            }
            CheckoutBindingOutcome::OrderChanged => {
                Err(OrderError::Business(OrderFailure::OrderNotFound))
            }
        }
    }

    pub async fn prepare_stripe_intent(
        &self,
        user_id: i64,
        trade_no: String,
        method_id: i32,
    ) -> Result<StripePaymentIntent, OrderError> {
        let preparation = self
            .repository
            .prepare_stripe(user_id, &trade_no, method_id)
            .await?;
        if preparation.previous_binding.payment_id != Some(preparation.payment.id) {
            self.cancel_binding(&preparation.previous_binding).await?;
        }
        let prepared = self
            .gateway
            .prepare_stripe_intent(
                &preparation.payment,
                &preparation.order,
                preparation.reusable_intent.as_deref(),
            )
            .await?;
        let binding = self
            .repository
            .bind_stripe(BindStripeCommand {
                order_id: preparation.order_id,
                payment: preparation.payment.clone(),
                previous_binding: preparation.previous_binding,
                intent_id: prepared.intent_id.clone(),
                handling_amount: preparation.handling_amount,
                now: self.clock.now(),
            })
            .await?;
        if binding == CheckoutBindingOutcome::Bound {
            return Ok(prepared.response);
        }
        let cancelled = self
            .gateway
            .cancel_stripe_intent(Some(&preparation.payment), Some(&prepared.intent_id))
            .await?;
        if !cancelled {
            return Err(OrderError::Business(OrderFailure::StripeBindingInvalid));
        }
        Err(OrderError::Business(match binding {
            CheckoutBindingOutcome::PaymentChanged => OrderFailure::StripeBindingInvalid,
            CheckoutBindingOutcome::OrderChanged => OrderFailure::OrderNotFound,
            CheckoutBindingOutcome::Bound => unreachable!(),
        }))
    }

    pub async fn cancel(&self, user_id: i64, trade_no: String) -> Result<(), OrderError> {
        let candidate = self
            .repository
            .cancel_candidate(user_id, &trade_no)
            .await?
            .ok_or(OrderError::Business(OrderFailure::OrderNotFound))?;
        if candidate.status != 0 {
            return Err(OrderError::Business(OrderFailure::OrderNotPending));
        }
        if !self.cancel_binding(&candidate.binding).await? {
            return Err(OrderError::Business(OrderFailure::OrderNotPending));
        }
        match self
            .repository
            .cancel_order(CancelOrderCommand {
                user_id,
                trade_no,
                candidate,
                now: self.clock.now(),
            })
            .await?
        {
            CancelOrderOutcome::Cancelled => Ok(()),
            CancelOrderOutcome::Changed => Err(OrderError::Business(OrderFailure::OrderNotPending)),
        }
    }

    /// Cancels the provider-side payment represented by an already-read order
    /// binding. Administrative order workflows use this before their own
    /// compare-and-set transition, preserving the same network-before-write
    /// sequencing as user cancellation and expiry.
    pub async fn cancel_payment_binding(
        &self,
        payment_id: Option<i32>,
        callback_no: Option<&str>,
    ) -> Result<bool, OrderError> {
        self.cancel_binding(&PaymentBinding {
            payment_id,
            callback_no: callback_no.map(str::to_owned),
        })
        .await
    }

    pub async fn handle_payment_notify(
        &self,
        provider: &str,
        uuid: &str,
        input: PaymentNotifyInput,
    ) -> Result<PaymentNotifyResponse, OrderError> {
        self.run_payment_notify(provider, uuid, input)
            .await
            .map_err(|_| OrderError::PaymentNotificationFailed)
    }

    async fn run_payment_notify(
        &self,
        provider: &str,
        uuid: &str,
        input: PaymentNotifyInput,
    ) -> Result<PaymentNotifyResponse, OrderError> {
        let payment = self
            .repository
            .payment_for_notification(provider, uuid)
            .await?
            .ok_or(OrderError::Business(OrderFailure::PaymentMethodUnavailable))?;
        let verification = self.gateway.verify_notification(&payment, &input).await?;
        let PaymentVerification::Verified(notification) = verification else {
            let PaymentVerification::Ignored(body) = verification else {
                unreachable!()
            };
            return Ok(PaymentNotifyResponse {
                body,
                paid_notice: None,
                late_payment_notice: None,
            });
        };
        let response_body = notification
            .custom_response
            .clone()
            .unwrap_or_else(|| "success".to_string());
        let trade_no = notification.trade_no.clone();
        let notices = self
            .repository
            .settle_payment(SettlePaymentCommand {
                payment,
                notification,
                now: self.clock.now(),
            })
            .await?;
        if notices.paid.is_some()
            && let Ok(Some(snapshot)) = self.repository.pending_order(&trade_no).await
        {
            let _ = self
                .repository
                .process_pending_order(
                    &trade_no,
                    snapshot.clone(),
                    false,
                    self.clock.now(),
                    self.policy.fulfillment_policy(snapshot.total_amount),
                )
                .await;
        }
        Ok(PaymentNotifyResponse {
            body: response_body,
            paid_notice: notices.paid,
            late_payment_notice: notices.late,
        })
    }

    pub async fn paid_manually(&self, trade_no: &str) -> Result<(), OrderError> {
        let candidate = self
            .repository
            .manual_payment_candidate(trade_no)
            .await?
            .ok_or(OrderError::Business(OrderFailure::OrderNotFound))?;
        if candidate.status != 0 {
            return Err(OrderError::Business(OrderFailure::OrderNotPending));
        }
        if !self.cancel_binding(&candidate.binding).await? {
            return Err(OrderError::Business(OrderFailure::OrderNotPending));
        }
        let total_amount = self
            .repository
            .pending_order(trade_no)
            .await?
            .map_or(0, |snapshot| snapshot.total_amount);
        match self
            .repository
            .settle_manually(
                trade_no,
                candidate,
                self.clock.now(),
                self.policy.fulfillment_policy(total_amount),
            )
            .await?
        {
            ManualPaymentOutcome::Settled => Ok(()),
            ManualPaymentOutcome::NotFound => {
                Err(OrderError::Business(OrderFailure::OrderNotFound))
            }
            ManualPaymentOutcome::Changed => {
                Err(OrderError::Business(OrderFailure::OrderNotPending))
            }
        }
    }

    pub async fn handle_pending_order(&self, trade_no: &str) -> Result<(), OrderError> {
        let Some(snapshot) = self.repository.pending_order(trade_no).await? else {
            return Ok(());
        };
        let should_expire =
            snapshot.status == 0 && snapshot.created_at <= self.clock.now().saturating_sub(7_200);
        if should_expire && !self.cancel_binding(&snapshot.binding).await? {
            return Ok(());
        }
        self.repository
            .process_pending_order(
                trade_no,
                snapshot.clone(),
                should_expire,
                self.clock.now(),
                self.policy.fulfillment_policy(snapshot.total_amount),
            )
            .await?;
        Ok(())
    }

    pub async fn orders(
        &self,
        user_id: i64,
        status: Option<i16>,
    ) -> Result<Vec<UserOrder>, OrderError> {
        Ok(self.repository.user_orders(user_id, status).await?)
    }

    pub async fn order(&self, user_id: i64, trade_no: &str) -> Result<UserOrder, OrderError> {
        let mut order = self
            .repository
            .user_order(user_id, trade_no, self.policy.try_out_plan_id())
            .await?
            .ok_or(OrderError::Business(OrderFailure::OrderNotFound))?;
        if order.plan_id != 0 && order.plan.is_none() {
            return Err(OrderError::Business(OrderFailure::PlanUnavailable));
        }
        if order.plan_id == 0 {
            let bonus = self
                .policy
                .fulfillment_policy(order.total_amount)
                .deposit_bonus;
            order.bonus = Some(bonus);
            order.get_amount = order.total_amount.checked_add(bonus);
            if order.get_amount.is_none() {
                return Err(OrderError::Business(OrderFailure::PaymentAmountOutOfRange));
            }
        }
        Ok(order)
    }

    pub async fn status(&self, user_id: i64, trade_no: &str) -> Result<i16, OrderError> {
        self.repository
            .user_order_status(user_id, trade_no)
            .await?
            .ok_or(OrderError::Business(OrderFailure::OrderNotFound))
    }

    async fn cancel_binding(&self, binding: &PaymentBinding) -> Result<bool, OrderError> {
        let payment = match binding.payment_id {
            Some(payment_id) => self.repository.payment_binding_material(payment_id).await?,
            None => None,
        };
        Ok(self
            .gateway
            .cancel_stripe_intent(payment.as_ref(), binding.callback_no.as_deref())
            .await?)
    }
}

#[cfg(test)]
mod tests;
