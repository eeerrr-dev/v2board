//! Administrative order use cases and outbound ports.
//!
//! HTTP filter JSON, PostgreSQL query construction, provider calls, runtime
//! configuration, and RFC problem rendering stay in outer adapters. This
//! module owns the closed query vocabulary and order orchestration policy.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminOrder {
    pub id: i64,
    pub invite_user_id: Option<i64>,
    pub user_id: i64,
    pub plan_id: i32,
    pub coupon_id: Option<i32>,
    pub kind: i32,
    pub period: String,
    pub trade_no: String,
    pub callback_no: Option<String>,
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
    pub payment_id: Option<i32>,
    pub paid_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminOrderListItem {
    pub order: AdminOrder,
    pub email: String,
    pub plan_name: Option<String>,
    pub open_reconciliation_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminCommissionLog {
    pub id: i64,
    pub invite_user_id: i64,
    pub user_id: i64,
    pub trade_no: String,
    pub order_amount: i32,
    pub get_amount: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminOrderReconciliation {
    pub id: i64,
    pub payment_id: i32,
    pub provider: String,
    pub trade_no: String,
    pub trade_no_hash: String,
    pub callback_no: String,
    pub callback_no_hash: String,
    pub reason: String,
    pub order_status: i16,
    pub expected_amount: i64,
    pub settled_amount: Option<i64>,
    pub occurrence_count: i32,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub resolved_at: Option<i64>,
    pub resolution: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminOrderDetail {
    pub order: AdminOrder,
    pub commission_log: Vec<AdminCommissionLog>,
    pub payment_reconciliations: Vec<AdminOrderReconciliation>,
    pub surplus_orders: Option<Vec<AdminOrder>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminOrderPage {
    pub items: Vec<AdminOrderListItem>,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderFieldKind {
    Integer,
    Text,
    Timestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderField {
    Id,
    InviteUserId,
    UserId,
    PlanId,
    CouponId,
    PaymentId,
    Type,
    Period,
    TradeNo,
    CallbackNo,
    TotalAmount,
    HandlingAmount,
    DiscountAmount,
    SurplusAmount,
    RefundAmount,
    BalanceAmount,
    Status,
    CommissionStatus,
    CommissionBalance,
    ActualCommissionBalance,
    PaidAt,
    CreatedAt,
    UpdatedAt,
}

impl OrderField {
    pub fn from_name(value: &str) -> Option<Self> {
        Some(match value {
            "id" => Self::Id,
            "invite_user_id" => Self::InviteUserId,
            "user_id" => Self::UserId,
            "plan_id" => Self::PlanId,
            "coupon_id" => Self::CouponId,
            "payment_id" => Self::PaymentId,
            "type" => Self::Type,
            "period" => Self::Period,
            "trade_no" => Self::TradeNo,
            "callback_no" => Self::CallbackNo,
            "total_amount" => Self::TotalAmount,
            "handling_amount" => Self::HandlingAmount,
            "discount_amount" => Self::DiscountAmount,
            "surplus_amount" => Self::SurplusAmount,
            "refund_amount" => Self::RefundAmount,
            "balance_amount" => Self::BalanceAmount,
            "status" => Self::Status,
            "commission_status" => Self::CommissionStatus,
            "commission_balance" => Self::CommissionBalance,
            "actual_commission_balance" => Self::ActualCommissionBalance,
            "paid_at" => Self::PaidAt,
            "created_at" => Self::CreatedAt,
            "updated_at" => Self::UpdatedAt,
            _ => return None,
        })
    }

    pub const fn kind(self) -> OrderFieldKind {
        match self {
            Self::Period | Self::TradeNo | Self::CallbackNo => OrderFieldKind::Text,
            Self::PaidAt | Self::CreatedAt | Self::UpdatedAt => OrderFieldKind::Timestamp,
            _ => OrderFieldKind::Integer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Comparison {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderPredicate {
    IsNull {
        field: OrderField,
        negated: bool,
    },
    CompareInteger {
        field: OrderField,
        comparison: Comparison,
        value: i64,
    },
    CompareText {
        field: OrderField,
        comparison: Comparison,
        value: String,
    },
    ContainsInteger {
        field: OrderField,
        escaped_pattern: String,
    },
    ContainsText {
        field: OrderField,
        escaped_pattern: String,
    },
    InInteger {
        field: OrderField,
        values: Vec<i64>,
    },
    InText {
        field: OrderField,
        values: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderSort {
    pub field: OrderField,
    pub direction: SortDirection,
}

impl Default for OrderSort {
    fn default() -> Self {
        Self {
            field: OrderField::CreatedAt,
            direction: SortDirection::Descending,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminOrderQuery {
    pub predicates: Vec<OrderPredicate>,
    pub sort: OrderSort,
    pub commission_only: bool,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderPatch {
    Status(i16),
    CommissionStatus(i16),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignOrderInput {
    pub email: String,
    pub plan_id: i64,
    pub period: String,
    pub total_amount: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignOrderPolicy {
    pub default_commission_rate: i32,
    pub commission_first_time_enable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignOrderCommand {
    pub email: String,
    pub plan_id: i32,
    pub period: String,
    pub total_amount: i32,
    pub trade_no: String,
    pub now: i64,
    pub policy: AssignOrderPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingOrderBinding {
    pub trade_no: String,
    pub payment_id: Option<i32>,
    pub callback_no: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchOrderOutcome {
    Updated,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignOrderOutcome {
    Created,
    UserNotRegistered,
    PlanUnavailable,
    UnfinishedOrder,
    UpdateConflict,
    AmountOutOfRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PendingOrderOutcome {
    Pending(PendingOrderBinding),
    NotFound,
    NotPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOrderOutcome {
    Cancelled,
    NotPending,
    UpdateFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminOrderInputViolation {
    EmptyPeriod,
    PlanIdOutOfRange,
    TotalAmountOutOfRange,
    InvalidStatus,
    InvalidCommissionStatus,
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("order lifecycle adapter failed during {operation}: {message}")]
pub struct AdminOrderLifecycleError {
    failure: AdminOrderLifecycleFailure,
    operation: &'static str,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminOrderLifecycleFailure {
    NotFound,
    NotPending,
    UpdateFailed,
    Other,
}

impl AdminOrderLifecycleError {
    pub fn new(operation: &'static str, message: impl ToString) -> Self {
        Self {
            failure: AdminOrderLifecycleFailure::Other,
            operation,
            message: message.to_string(),
        }
    }

    pub fn classified(
        failure: AdminOrderLifecycleFailure,
        operation: &'static str,
        message: impl ToString,
    ) -> Self {
        Self {
            failure,
            operation,
            message: message.to_string(),
        }
    }

    pub const fn failure(&self) -> AdminOrderLifecycleFailure {
        self.failure
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AdminOrderError {
    #[error("invalid admin order input: {0:?}")]
    InvalidInput(AdminOrderInputViolation),
    #[error("order not found")]
    NotFound,
    #[error("order is not pending")]
    NotPending,
    #[error("user is not registered")]
    UserNotRegistered,
    #[error("plan is unavailable")]
    PlanUnavailable,
    #[error("user already has an unfinished order")]
    AssignConflict,
    #[error("order update conflicted")]
    UpdateConflict,
    #[error("order update failed")]
    UpdateFailed,
    #[error(transparent)]
    Lifecycle(#[from] AdminOrderLifecycleError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait AdminOrderRepository: Send + Sync {
    async fn list(&self, query: AdminOrderQuery) -> RepositoryResult<AdminOrderPage>;
    async fn detail(&self, trade_no: &str) -> RepositoryResult<Option<AdminOrderDetail>>;
    async fn patch(
        &self,
        trade_no: &str,
        patch: OrderPatch,
        now: i64,
    ) -> RepositoryResult<PatchOrderOutcome>;
    async fn pending_binding(&self, trade_no: &str) -> RepositoryResult<PendingOrderOutcome>;
    async fn cancel_pending(
        &self,
        binding: &PendingOrderBinding,
        now: i64,
    ) -> RepositoryResult<CancelOrderOutcome>;
    async fn assign(&self, command: AssignOrderCommand) -> RepositoryResult<AssignOrderOutcome>;
}

#[allow(async_fn_in_trait)]
pub trait AdminOrderLifecycle: Send + Sync {
    async fn mark_paid(&self, trade_no: &str) -> Result<(), AdminOrderLifecycleError>;
    async fn cancel_payment_binding(
        &self,
        payment_id: Option<i32>,
        callback_no: Option<&str>,
    ) -> Result<bool, AdminOrderLifecycleError>;
}

pub trait OrderNumberGenerator: Send + Sync {
    fn generate(&self) -> String;
}

pub struct AdminOrderService<R, L, G> {
    repository: R,
    lifecycle: L,
    generator: G,
    policy: AssignOrderPolicy,
}

impl<R, L, G> AdminOrderService<R, L, G>
where
    R: AdminOrderRepository,
    L: AdminOrderLifecycle,
    G: OrderNumberGenerator,
{
    pub fn new(repository: R, lifecycle: L, generator: G, policy: AssignOrderPolicy) -> Self {
        Self {
            repository,
            lifecycle,
            generator,
            policy,
        }
    }

    pub async fn orders(&self, query: AdminOrderQuery) -> Result<AdminOrderPage, AdminOrderError> {
        Ok(self.repository.list(query).await?)
    }

    pub async fn order(&self, trade_no: &str) -> Result<AdminOrderDetail, AdminOrderError> {
        self.repository
            .detail(trade_no)
            .await?
            .ok_or(AdminOrderError::NotFound)
    }

    pub async fn patch(
        &self,
        trade_no: &str,
        patch: OrderPatch,
        now: i64,
    ) -> Result<(), AdminOrderError> {
        match patch {
            OrderPatch::Status(status) if !(0..=3).contains(&status) => {
                return Err(AdminOrderError::InvalidInput(
                    AdminOrderInputViolation::InvalidStatus,
                ));
            }
            OrderPatch::CommissionStatus(status) if !matches!(status, 0 | 1 | 3) => {
                return Err(AdminOrderError::InvalidInput(
                    AdminOrderInputViolation::InvalidCommissionStatus,
                ));
            }
            _ => {}
        }
        match self.repository.patch(trade_no, patch, now).await? {
            PatchOrderOutcome::Updated => Ok(()),
            PatchOrderOutcome::NotFound => Err(AdminOrderError::NotFound),
        }
    }

    pub async fn mark_paid(&self, trade_no: &str) -> Result<(), AdminOrderError> {
        self.lifecycle
            .mark_paid(trade_no)
            .await
            .map_err(classify_lifecycle_error)
    }

    pub async fn cancel(&self, trade_no: &str, now: i64) -> Result<(), AdminOrderError> {
        let binding = match self.repository.pending_binding(trade_no).await? {
            PendingOrderOutcome::Pending(binding) => binding,
            PendingOrderOutcome::NotFound => return Err(AdminOrderError::NotFound),
            PendingOrderOutcome::NotPending => return Err(AdminOrderError::NotPending),
        };
        let cancelled = self
            .lifecycle
            .cancel_payment_binding(binding.payment_id, binding.callback_no.as_deref())
            .await
            .map_err(classify_lifecycle_error)?;
        if !cancelled {
            return Err(AdminOrderError::NotPending);
        }
        match self.repository.cancel_pending(&binding, now).await? {
            CancelOrderOutcome::Cancelled => Ok(()),
            CancelOrderOutcome::NotPending => Err(AdminOrderError::NotPending),
            CancelOrderOutcome::UpdateFailed => Err(AdminOrderError::UpdateFailed),
        }
    }

    pub async fn assign(
        &self,
        input: AssignOrderInput,
        now: i64,
    ) -> Result<String, AdminOrderError> {
        let period = input.period.trim();
        if period.is_empty() {
            return Err(AdminOrderError::InvalidInput(
                AdminOrderInputViolation::EmptyPeriod,
            ));
        }
        let plan_id = i32::try_from(input.plan_id).map_err(|_| {
            AdminOrderError::InvalidInput(AdminOrderInputViolation::PlanIdOutOfRange)
        })?;
        let total_amount = i32::try_from(input.total_amount.unwrap_or_default())
            .ok()
            .filter(|value| *value >= 0)
            .ok_or(AdminOrderError::InvalidInput(
                AdminOrderInputViolation::TotalAmountOutOfRange,
            ))?;
        let trade_no = self.generator.generate();
        let outcome = self
            .repository
            .assign(AssignOrderCommand {
                email: input.email,
                plan_id,
                period: period.to_string(),
                total_amount,
                trade_no: trade_no.clone(),
                now,
                policy: self.policy,
            })
            .await?;
        match outcome {
            AssignOrderOutcome::Created => Ok(trade_no),
            AssignOrderOutcome::UserNotRegistered => Err(AdminOrderError::UserNotRegistered),
            AssignOrderOutcome::PlanUnavailable => Err(AdminOrderError::PlanUnavailable),
            AssignOrderOutcome::UnfinishedOrder => Err(AdminOrderError::AssignConflict),
            AssignOrderOutcome::UpdateConflict => Err(AdminOrderError::UpdateConflict),
            AssignOrderOutcome::AmountOutOfRange => Err(AdminOrderError::InvalidInput(
                AdminOrderInputViolation::TotalAmountOutOfRange,
            )),
        }
    }
}

fn classify_lifecycle_error(error: AdminOrderLifecycleError) -> AdminOrderError {
    match error.failure() {
        AdminOrderLifecycleFailure::NotFound => AdminOrderError::NotFound,
        AdminOrderLifecycleFailure::NotPending => AdminOrderError::NotPending,
        AdminOrderLifecycleFailure::UpdateFailed => AdminOrderError::UpdateFailed,
        AdminOrderLifecycleFailure::Other => AdminOrderError::Lifecycle(error),
    }
}

#[must_use]
pub fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Default)]
    struct State {
        assignment: Option<AssignOrderCommand>,
        patch: Option<OrderPatch>,
        cancel_calls: usize,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<State>>);

    impl AdminOrderRepository for FakeRepository {
        async fn list(&self, _query: AdminOrderQuery) -> RepositoryResult<AdminOrderPage> {
            Ok(AdminOrderPage {
                items: Vec::new(),
                total: 0,
            })
        }

        async fn detail(&self, _trade_no: &str) -> RepositoryResult<Option<AdminOrderDetail>> {
            Ok(None)
        }

        async fn patch(
            &self,
            _trade_no: &str,
            patch: OrderPatch,
            _now: i64,
        ) -> RepositoryResult<PatchOrderOutcome> {
            self.0.lock().unwrap().patch = Some(patch);
            Ok(PatchOrderOutcome::Updated)
        }

        async fn pending_binding(&self, trade_no: &str) -> RepositoryResult<PendingOrderOutcome> {
            Ok(PendingOrderOutcome::Pending(PendingOrderBinding {
                trade_no: trade_no.to_string(),
                payment_id: Some(7),
                callback_no: Some("pi_1".to_string()),
            }))
        }

        async fn cancel_pending(
            &self,
            _binding: &PendingOrderBinding,
            _now: i64,
        ) -> RepositoryResult<CancelOrderOutcome> {
            self.0.lock().unwrap().cancel_calls += 1;
            Ok(CancelOrderOutcome::Cancelled)
        }

        async fn assign(
            &self,
            command: AssignOrderCommand,
        ) -> RepositoryResult<AssignOrderOutcome> {
            self.0.lock().unwrap().assignment = Some(command);
            Ok(AssignOrderOutcome::Created)
        }
    }

    #[derive(Clone, Copy)]
    struct FakeLifecycle(bool);

    impl AdminOrderLifecycle for FakeLifecycle {
        async fn mark_paid(&self, _trade_no: &str) -> Result<(), AdminOrderLifecycleError> {
            Ok(())
        }

        async fn cancel_payment_binding(
            &self,
            _payment_id: Option<i32>,
            _callback_no: Option<&str>,
        ) -> Result<bool, AdminOrderLifecycleError> {
            Ok(self.0)
        }
    }

    struct FixedGenerator;

    impl OrderNumberGenerator for FixedGenerator {
        fn generate(&self) -> String {
            "fixed-trade".to_string()
        }
    }

    fn service(
        repository: FakeRepository,
        lifecycle: FakeLifecycle,
    ) -> AdminOrderService<FakeRepository, FakeLifecycle, FixedGenerator> {
        AdminOrderService::new(
            repository,
            lifecycle,
            FixedGenerator,
            AssignOrderPolicy {
                default_commission_rate: 12,
                commission_first_time_enable: true,
            },
        )
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn assign_validates_and_builds_a_closed_command() {
        let repository = FakeRepository::default();
        let trade_no = block_on(service(repository.clone(), FakeLifecycle(true)).assign(
            AssignOrderInput {
                email: "person@example.test".to_string(),
                plan_id: 3,
                period: " month_price ".to_string(),
                total_amount: Some(500),
            },
            20,
        ))
        .unwrap();
        assert_eq!(trade_no, "fixed-trade");
        let state = repository.0.lock().unwrap();
        let command = state.assignment.as_ref().unwrap();
        assert_eq!(command.period, "month_price");
        assert_eq!(command.total_amount, 500);
        assert_eq!(command.policy.default_commission_rate, 12);
    }

    #[test]
    fn patch_rejects_invalid_state_before_the_repository() {
        let repository = FakeRepository::default();
        assert!(matches!(
            block_on(service(repository.clone(), FakeLifecycle(true)).patch(
                "trade",
                OrderPatch::Status(4),
                1,
            )),
            Err(AdminOrderError::InvalidInput(
                AdminOrderInputViolation::InvalidStatus
            ))
        ));
        assert!(repository.0.lock().unwrap().patch.is_none());
    }

    #[test]
    fn provider_cancellation_must_succeed_before_the_atomic_refund() {
        let repository = FakeRepository::default();
        assert!(matches!(
            block_on(service(repository.clone(), FakeLifecycle(false)).cancel("trade", 1)),
            Err(AdminOrderError::NotPending)
        ));
        assert_eq!(repository.0.lock().unwrap().cancel_calls, 0);
        block_on(service(repository.clone(), FakeLifecycle(true)).cancel("trade", 1)).unwrap();
        assert_eq!(repository.0.lock().unwrap().cancel_calls, 1);
    }

    #[test]
    fn field_vocabulary_and_like_escaping_are_closed() {
        assert_eq!(OrderField::from_name("trade_no"), Some(OrderField::TradeNo));
        assert_eq!(OrderField::TradeNo.kind(), OrderFieldKind::Text);
        assert_eq!(OrderField::PaidAt.kind(), OrderFieldKind::Timestamp);
        assert!(OrderField::from_name("raw_sql").is_none());
        assert_eq!(escape_like_pattern(r"a%b_c\d"), r"a\%b\_c\\d");
    }
}
