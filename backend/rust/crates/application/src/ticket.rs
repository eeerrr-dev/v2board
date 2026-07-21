//! Ticket use cases and outbound ports.
//!
//! HTTP problems, PostgreSQL transactions, Redis commands, runtime config and
//! mail rendering are outer-adapter concerns. This module owns the workflow,
//! business outcomes and the order in which policy checks occur.

use v2board_domain_model::{
    TicketCreationPolicy, TicketInputViolation, TicketLevel, TicketReplyStatus, TicketStatus,
    commission_balance_meets_minimum, validate_operator_ticket_message,
    validate_ticket_create_input, validate_ticket_message, validate_withdrawal_input,
};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

pub const TICKET_CREATE_LIMIT: i64 = 10;
pub const TICKET_CREATE_WINDOW_SECONDS: i64 = 3_600;
pub const TICKET_REPLY_LIMIT: i64 = 20;
pub const TICKET_REPLY_WINDOW_SECONDS: i64 = 300;
pub const AUTO_CLOSE_AFTER_SECONDS: i64 = 86_400;
pub const AUTO_CLOSE_BATCH_SIZE: i64 = 1_000;
pub const AUTO_CLOSE_MAX_BATCHES: usize = 20;
pub const WITHDRAWAL_TICKET_SUBJECT: &str =
    "[Commission Withdrawal Request] This ticket is opened by the system";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ticket {
    pub id: i64,
    pub user_id: i64,
    pub subject: String,
    pub level: TicketLevel,
    pub status: TicketStatus,
    pub reply_status: TicketReplyStatus,
    pub last_reply_user_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketMessage {
    pub id: i64,
    pub user_id: i64,
    pub ticket_id: i64,
    pub message: String,
    pub is_me: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketDetail {
    pub ticket: Ticket,
    pub messages: Vec<TicketMessage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketPage {
    pub items: Vec<Ticket>,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorTicketOrder {
    UpdatedAt,
    CreatedAt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorTicketListQuery {
    pub limit: i64,
    pub offset: i64,
    pub status: Option<i64>,
    pub reply_statuses: Vec<i64>,
    pub email: Option<String>,
    pub order: OperatorTicketOrder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewTicket {
    pub user_id: i64,
    pub subject: String,
    pub level: TicketLevel,
    pub message: String,
    pub created_at: i64,
    pub require_paid_order: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketCreateOutcome {
    Created(i64),
    OpenTicketExists,
    PaidOrderRequired,
    UserNotFound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserTicketReply {
    pub ticket_id: i64,
    pub user_id: i64,
    pub message: String,
    pub replied_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserTicketReplyOutcome {
    Replied,
    NotFound,
    Closed,
    AwaitingOperator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorReplyTarget {
    pub ticket_id: i64,
    pub user_id: i64,
    pub subject: String,
    pub recipient_email: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableMailDelivery {
    pub batch_key: String,
    pub payload_hash: String,
    pub actor: String,
    pub recipient: String,
    pub message_id: String,
    pub sender: String,
    pub template_name: String,
    pub subject: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationReservation {
    pub key: String,
    pub token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedTicketNotification {
    pub delivery: DurableMailDelivery,
    pub reservation: NotificationReservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorTicketReply {
    pub ticket_id: i64,
    pub expected_user_id: i64,
    pub operator_id: i64,
    pub message: String,
    pub replied_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorTicketReplyOutcome {
    Replied,
    NotFound,
    OtherOpenTicketExists,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketWriteKind {
    Create,
    Reply,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketAdmissionRequest {
    pub user_id: i64,
    pub kind: TicketWriteKind,
    pub limit: i64,
    pub window_seconds: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketPolicy {
    pub creation: TicketCreationPolicy,
    pub withdrawal_closed: bool,
    pub withdrawal_methods: Vec<String>,
    pub withdrawal_minimum_mantissa: i128,
    pub withdrawal_minimum_scale: u32,
    pub withdrawal_minimum_display: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateTicketInput {
    pub subject: String,
    pub level: i16,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateWithdrawalTicketInput {
    pub method: String,
    pub account: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorIdentity<'a> {
    Email(&'a str),
    UserId(i64),
}

#[derive(Debug, thiserror::Error)]
pub enum TicketError {
    #[error("invalid ticket field {field}: {detail}")]
    Validation {
        field: &'static str,
        detail: &'static str,
    },
    #[error("ticket not found")]
    NotFound,
    #[error("ticket owner is not registered")]
    UserNotRegistered,
    #[error("the user already has an unresolved ticket")]
    UnresolvedTicketExists,
    #[error("ticket requires a plan: {detail}")]
    RequiresPlan { detail: &'static str },
    #[error("ticket state does not allow the operation: {detail}")]
    InvalidState { detail: &'static str },
    #[error("withdrawal method is unsupported")]
    WithdrawMethodUnsupported { detail: Option<&'static str> },
    #[error("withdrawal balance is below {minimum}")]
    WithdrawBelowMinimum { minimum: String },
    #[error("ticket write limit exceeded")]
    RateLimited,
    #[error("ticket invariant failed: {0}")]
    Invariant(&'static str),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait TicketRepository: Send + Sync {
    async fn list_for_user(&self, user_id: i64) -> RepositoryResult<Vec<Ticket>>;
    async fn find_for_user(
        &self,
        user_id: i64,
        ticket_id: i64,
    ) -> RepositoryResult<Option<TicketDetail>>;
    async fn create(&self, ticket: NewTicket) -> RepositoryResult<TicketCreateOutcome>;
    async fn reply_as_user(
        &self,
        reply: UserTicketReply,
    ) -> RepositoryResult<UserTicketReplyOutcome>;
    async fn close_as_user(
        &self,
        user_id: i64,
        ticket_id: i64,
        closed_at: i64,
    ) -> RepositoryResult<bool>;
    async fn commission_balance(&self, user_id: i64) -> RepositoryResult<Option<i64>>;

    async fn list_for_operator(
        &self,
        query: OperatorTicketListQuery,
    ) -> RepositoryResult<TicketPage>;
    async fn find_for_operator(&self, ticket_id: i64) -> RepositoryResult<Option<TicketDetail>>;
    async fn operator_id_by_email(&self, email: &str) -> RepositoryResult<Option<i64>>;
    async fn operator_reply_target(
        &self,
        ticket_id: i64,
    ) -> RepositoryResult<Option<OperatorReplyTarget>>;
    async fn reply_as_operator(
        &self,
        reply: OperatorTicketReply,
        notification: Option<&DurableMailDelivery>,
    ) -> RepositoryResult<OperatorTicketReplyOutcome>;
    async fn close_as_operator(&self, ticket_id: i64, closed_at: i64) -> RepositoryResult<bool>;

    async fn auto_close_batch(&self, now: i64, cutoff: i64, limit: i64) -> RepositoryResult<u64>;
}

#[allow(async_fn_in_trait)]
pub trait TicketWriteAdmission: Send + Sync {
    async fn reserve(&self, request: TicketAdmissionRequest) -> RepositoryResult<bool>;
}

#[allow(async_fn_in_trait)]
pub trait TicketReplyNotifications: Send + Sync {
    /// Notification preparation and its Redis gate are deliberately best
    /// effort. `None` must not prevent the business reply.
    async fn prepare(
        &self,
        target: &OperatorReplyTarget,
        message: &str,
    ) -> Option<PreparedTicketNotification>;
    async fn release(&self, reservation: &NotificationReservation);
}

#[derive(Clone, Debug)]
pub struct TicketService<R, A, N> {
    repository: R,
    admission: A,
    notifications: N,
    policy: TicketPolicy,
}

impl<R, A, N> TicketService<R, A, N>
where
    R: TicketRepository,
    A: TicketWriteAdmission,
    N: TicketReplyNotifications,
{
    pub fn new(repository: R, admission: A, notifications: N, policy: TicketPolicy) -> Self {
        Self {
            repository,
            admission,
            notifications,
            policy,
        }
    }

    pub async fn user_tickets(&self, user_id: i64) -> Result<Vec<Ticket>, TicketError> {
        Ok(self.repository.list_for_user(user_id).await?)
    }

    pub async fn user_ticket(
        &self,
        user_id: i64,
        ticket_id: i64,
    ) -> Result<TicketDetail, TicketError> {
        self.repository
            .find_for_user(user_id, ticket_id)
            .await?
            .ok_or(TicketError::NotFound)
    }

    pub async fn create_ticket(
        &self,
        user_id: i64,
        input: CreateTicketInput,
        now: i64,
    ) -> Result<i64, TicketError> {
        let (subject, level, message) =
            validate_ticket_create_input(&input.subject, input.level, &input.message)
                .map_err(ticket_input_error)?;
        let require_paid_order = match self.policy.creation {
            TicketCreationPolicy::Open => false,
            TicketCreationPolicy::PaidOrderRequired => true,
            TicketCreationPolicy::PlanRejected => {
                return Err(TicketError::RequiresPlan {
                    detail: "当前套餐不允许发起工单",
                });
            }
            TicketCreationPolicy::InvalidState => {
                return Err(TicketError::InvalidState {
                    detail: "未知的工单状态",
                });
            }
        };
        self.reserve(user_id, TicketWriteKind::Create).await?;
        self.created_id(
            self.repository
                .create(NewTicket {
                    user_id,
                    subject: subject.to_string(),
                    level,
                    message: message.to_string(),
                    created_at: now,
                    require_paid_order,
                })
                .await?,
        )
    }

    pub async fn reply_as_user(
        &self,
        user_id: i64,
        ticket_id: i64,
        message: String,
        now: i64,
    ) -> Result<(), TicketError> {
        let message = validate_ticket_message(&message)
            .map_err(ticket_input_error)?
            .to_string();
        self.reserve(user_id, TicketWriteKind::Reply).await?;
        match self
            .repository
            .reply_as_user(UserTicketReply {
                ticket_id,
                user_id,
                message,
                replied_at: now,
            })
            .await?
        {
            UserTicketReplyOutcome::Replied => Ok(()),
            UserTicketReplyOutcome::NotFound => Err(TicketError::NotFound),
            UserTicketReplyOutcome::Closed => Err(TicketError::InvalidState {
                detail: "The ticket is closed and cannot be replied",
            }),
            UserTicketReplyOutcome::AwaitingOperator => Err(TicketError::InvalidState {
                detail: "Please wait for the technical enginneer to reply",
            }),
        }
    }

    pub async fn close_as_user(
        &self,
        user_id: i64,
        ticket_id: i64,
        now: i64,
    ) -> Result<(), TicketError> {
        if self
            .repository
            .close_as_user(user_id, ticket_id, now)
            .await?
        {
            Ok(())
        } else {
            Err(TicketError::NotFound)
        }
    }

    pub async fn create_withdrawal_ticket(
        &self,
        user_id: i64,
        input: CreateWithdrawalTicketInput,
        now: i64,
    ) -> Result<i64, TicketError> {
        let (method, _account, message) =
            validate_withdrawal_input(&input.method, &input.account).map_err(ticket_input_error)?;
        if self.policy.withdrawal_closed {
            return Err(TicketError::WithdrawMethodUnsupported {
                detail: Some("Unsupported withdrawal"),
            });
        }
        if !self
            .policy
            .withdrawal_methods
            .iter()
            .any(|allowed| allowed == method)
        {
            return Err(TicketError::WithdrawMethodUnsupported { detail: None });
        }
        let balance = self
            .repository
            .commission_balance(user_id)
            .await?
            .ok_or(TicketError::UserNotRegistered)?;
        if !commission_balance_meets_minimum(
            balance,
            self.policy.withdrawal_minimum_mantissa,
            self.policy.withdrawal_minimum_scale,
        ) {
            return Err(TicketError::WithdrawBelowMinimum {
                minimum: self.policy.withdrawal_minimum_display.clone(),
            });
        }
        self.reserve(user_id, TicketWriteKind::Create).await?;
        let outcome = self
            .repository
            .create(NewTicket {
                user_id,
                subject: WITHDRAWAL_TICKET_SUBJECT.to_string(),
                level: TicketLevel::High,
                message,
                created_at: now,
                require_paid_order: false,
            })
            .await?;
        if outcome == TicketCreateOutcome::PaidOrderRequired {
            return Err(TicketError::Invariant(
                "withdrawal ticket unexpectedly required a paid order",
            ));
        }
        self.created_id(outcome)
    }

    pub async fn operator_tickets(
        &self,
        query: OperatorTicketListQuery,
    ) -> Result<TicketPage, TicketError> {
        Ok(self.repository.list_for_operator(query).await?)
    }

    pub async fn operator_ticket(&self, ticket_id: i64) -> Result<TicketDetail, TicketError> {
        self.repository
            .find_for_operator(ticket_id)
            .await?
            .ok_or(TicketError::NotFound)
    }

    /// Admin/staff replies preserve the existing atomic boundary: if the
    /// best-effort notification gate was acquired, the durable outbox row is
    /// committed in the same transaction as the reply. A failed transaction
    /// releases only the reservation token it owns.
    pub async fn reply_as_operator(
        &self,
        ticket_id: i64,
        identity: OperatorIdentity<'_>,
        message: String,
        now: i64,
        notify_owner: bool,
    ) -> Result<(), TicketError> {
        validate_operator_ticket_message(&message).map_err(operator_input_error)?;
        let operator_id =
            match identity {
                OperatorIdentity::UserId(id) => id,
                OperatorIdentity::Email(email) => {
                    self.repository.operator_id_by_email(email).await?.ok_or(
                        TicketError::Invariant("acting operator account no longer exists"),
                    )?
                }
            };
        let target = self
            .repository
            .operator_reply_target(ticket_id)
            .await?
            .ok_or(TicketError::NotFound)?;
        let prepared = if notify_owner {
            self.notifications.prepare(&target, &message).await
        } else {
            None
        };
        let result = self
            .repository
            .reply_as_operator(
                OperatorTicketReply {
                    ticket_id,
                    expected_user_id: target.user_id,
                    operator_id,
                    message,
                    replied_at: now,
                },
                prepared.as_ref().map(|value| &value.delivery),
            )
            .await;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                if let Some(prepared) = prepared.as_ref() {
                    self.notifications.release(&prepared.reservation).await;
                }
                return Err(error.into());
            }
        };
        match outcome {
            OperatorTicketReplyOutcome::Replied => Ok(()),
            OperatorTicketReplyOutcome::NotFound => {
                if let Some(prepared) = prepared.as_ref() {
                    self.notifications.release(&prepared.reservation).await;
                }
                Err(TicketError::NotFound)
            }
            OperatorTicketReplyOutcome::OtherOpenTicketExists => {
                if let Some(prepared) = prepared.as_ref() {
                    self.notifications.release(&prepared.reservation).await;
                }
                Err(TicketError::UnresolvedTicketExists)
            }
        }
    }

    pub async fn close_as_operator(&self, ticket_id: i64, now: i64) -> Result<(), TicketError> {
        // The existing admin/staff contract is idempotent: a missing resource
        // still produces 204, unlike the user-owned close endpoint.
        self.repository.close_as_operator(ticket_id, now).await?;
        Ok(())
    }

    async fn reserve(&self, user_id: i64, kind: TicketWriteKind) -> Result<(), TicketError> {
        let (limit, window_seconds) = match kind {
            TicketWriteKind::Create => (TICKET_CREATE_LIMIT, TICKET_CREATE_WINDOW_SECONDS),
            TicketWriteKind::Reply => (TICKET_REPLY_LIMIT, TICKET_REPLY_WINDOW_SECONDS),
        };
        if self
            .admission
            .reserve(TicketAdmissionRequest {
                user_id,
                kind,
                limit,
                window_seconds,
            })
            .await?
        {
            Ok(())
        } else {
            Err(TicketError::RateLimited)
        }
    }

    fn created_id(&self, outcome: TicketCreateOutcome) -> Result<i64, TicketError> {
        match outcome {
            TicketCreateOutcome::Created(id) => Ok(id),
            TicketCreateOutcome::OpenTicketExists => Err(TicketError::UnresolvedTicketExists),
            TicketCreateOutcome::PaidOrderRequired => Err(TicketError::RequiresPlan {
                detail: "请先购买套餐",
            }),
            TicketCreateOutcome::UserNotFound => Err(TicketError::UserNotRegistered),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TicketMaintenance<R> {
    repository: R,
}

impl<R> TicketMaintenance<R>
where
    R: TicketRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn auto_close_answered(&self, now: i64) -> Result<u64, TicketError> {
        let cutoff = now.saturating_sub(AUTO_CLOSE_AFTER_SECONDS);
        let mut total = 0_u64;
        for _ in 0..AUTO_CLOSE_MAX_BATCHES {
            let closed = self
                .repository
                .auto_close_batch(now, cutoff, AUTO_CLOSE_BATCH_SIZE)
                .await?;
            total = total.saturating_add(closed);
            if closed < AUTO_CLOSE_BATCH_SIZE as u64 {
                break;
            }
        }
        Ok(total)
    }
}

fn ticket_input_error(violation: TicketInputViolation) -> TicketError {
    let (field, detail) = match violation {
        TicketInputViolation::EmptySubject => ("subject", "Ticket subject cannot be empty"),
        TicketInputViolation::SubjectTooLong => ("subject", "Ticket subject is too long"),
        TicketInputViolation::InvalidLevel => ("level", "Incorrect ticket level format"),
        TicketInputViolation::EmptyMessage => ("message", "Message cannot be empty"),
        TicketInputViolation::MessageTooLong => ("message", "Message is too long"),
        TicketInputViolation::EmptyWithdrawMethod => {
            ("withdraw_method", "The withdrawal method cannot be empty")
        }
        TicketInputViolation::WithdrawMethodTooLong => {
            ("withdraw_method", "The withdrawal method is too long")
        }
        TicketInputViolation::EmptyWithdrawAccount => {
            ("withdraw_account", "The withdrawal account cannot be empty")
        }
        TicketInputViolation::WithdrawMessageTooLong => ("withdraw_account", "Message is too long"),
    };
    TicketError::Validation { field, detail }
}

fn operator_input_error(_: TicketInputViolation) -> TicketError {
    TicketError::Validation {
        field: "message",
        detail: "工单回复内容过长",
    }
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
    struct FakeState {
        create_outcome: Option<TicketCreateOutcome>,
        reply_outcome: Option<UserTicketReplyOutcome>,
        operator_outcome: Option<OperatorTicketReplyOutcome>,
        balance: Option<i64>,
        created: Option<NewTicket>,
        admissions: Vec<TicketAdmissionRequest>,
        operator_delivery: Option<DurableMailDelivery>,
        released: Vec<NotificationReservation>,
    }

    #[derive(Clone, Default)]
    struct FakePorts(Arc<Mutex<FakeState>>);

    impl TicketRepository for FakePorts {
        async fn list_for_user(&self, _: i64) -> RepositoryResult<Vec<Ticket>> {
            Ok(vec![])
        }
        async fn find_for_user(&self, _: i64, _: i64) -> RepositoryResult<Option<TicketDetail>> {
            Ok(None)
        }
        async fn create(&self, ticket: NewTicket) -> RepositoryResult<TicketCreateOutcome> {
            let mut state = self.0.lock().unwrap();
            state.created = Some(ticket);
            Ok(state
                .create_outcome
                .unwrap_or(TicketCreateOutcome::Created(7)))
        }
        async fn reply_as_user(
            &self,
            _: UserTicketReply,
        ) -> RepositoryResult<UserTicketReplyOutcome> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .reply_outcome
                .unwrap_or(UserTicketReplyOutcome::Replied))
        }
        async fn close_as_user(&self, _: i64, _: i64, _: i64) -> RepositoryResult<bool> {
            Ok(true)
        }
        async fn commission_balance(&self, _: i64) -> RepositoryResult<Option<i64>> {
            Ok(self.0.lock().unwrap().balance)
        }
        async fn list_for_operator(
            &self,
            _: OperatorTicketListQuery,
        ) -> RepositoryResult<TicketPage> {
            Ok(TicketPage {
                items: vec![],
                total: 0,
            })
        }
        async fn find_for_operator(&self, _: i64) -> RepositoryResult<Option<TicketDetail>> {
            Ok(None)
        }
        async fn operator_id_by_email(&self, _: &str) -> RepositoryResult<Option<i64>> {
            Ok(Some(99))
        }
        async fn operator_reply_target(
            &self,
            ticket_id: i64,
        ) -> RepositoryResult<Option<OperatorReplyTarget>> {
            Ok(Some(OperatorReplyTarget {
                ticket_id,
                user_id: 8,
                subject: "Help".into(),
                recipient_email: "user@example.test".into(),
            }))
        }
        async fn reply_as_operator(
            &self,
            _: OperatorTicketReply,
            notification: Option<&DurableMailDelivery>,
        ) -> RepositoryResult<OperatorTicketReplyOutcome> {
            let mut state = self.0.lock().unwrap();
            state.operator_delivery = notification.cloned();
            Ok(state
                .operator_outcome
                .unwrap_or(OperatorTicketReplyOutcome::Replied))
        }
        async fn close_as_operator(&self, _: i64, _: i64) -> RepositoryResult<bool> {
            Ok(false)
        }
        async fn auto_close_batch(&self, _: i64, _: i64, _: i64) -> RepositoryResult<u64> {
            Ok(0)
        }
    }

    impl TicketWriteAdmission for FakePorts {
        async fn reserve(&self, request: TicketAdmissionRequest) -> RepositoryResult<bool> {
            self.0.lock().unwrap().admissions.push(request);
            Ok(true)
        }
    }

    impl TicketReplyNotifications for FakePorts {
        async fn prepare(
            &self,
            target: &OperatorReplyTarget,
            _: &str,
        ) -> Option<PreparedTicketNotification> {
            Some(PreparedTicketNotification {
                delivery: DurableMailDelivery {
                    batch_key: "batch".into(),
                    payload_hash: "hash".into(),
                    actor: format!("ticket:{}", target.user_id),
                    recipient: target.recipient_email.clone(),
                    message_id: "message".into(),
                    sender: "sender".into(),
                    template_name: "notify".into(),
                    subject: "reply".into(),
                    body: "body".into(),
                },
                reservation: NotificationReservation {
                    key: "gate".into(),
                    token: "token".into(),
                },
            })
        }
        async fn release(&self, reservation: &NotificationReservation) {
            self.0.lock().unwrap().released.push(reservation.clone());
        }
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

    fn policy() -> TicketPolicy {
        TicketPolicy {
            creation: TicketCreationPolicy::Open,
            withdrawal_closed: false,
            withdrawal_methods: vec!["bank".into()],
            withdrawal_minimum_mantissa: 1005,
            withdrawal_minimum_scale: 2,
            withdrawal_minimum_display: "10.05".into(),
        }
    }

    #[test]
    fn create_policy_and_admission_are_application_owned() {
        let ports = FakePorts::default();
        let id = block_on(
            TicketService::new(ports.clone(), ports.clone(), ports.clone(), policy())
                .create_ticket(
                    4,
                    CreateTicketInput {
                        subject: "  Help  ".into(),
                        level: 1,
                        message: "  Details  ".into(),
                    },
                    42,
                ),
        )
        .unwrap();
        assert_eq!(id, 7);
        let state = ports.0.lock().unwrap();
        assert_eq!(state.created.as_ref().unwrap().subject, "Help");
        assert_eq!(state.created.as_ref().unwrap().message, "Details");
        assert_eq!(
            state.admissions,
            vec![TicketAdmissionRequest {
                user_id: 4,
                kind: TicketWriteKind::Create,
                limit: 10,
                window_seconds: 3_600
            }]
        );
    }

    #[test]
    fn withdrawal_checks_exact_minimum_before_reserving_a_write() {
        let ports = FakePorts::default();
        ports.0.lock().unwrap().balance = Some(1_004);
        let error = block_on(
            TicketService::new(ports.clone(), ports.clone(), ports.clone(), policy())
                .create_withdrawal_ticket(
                    4,
                    CreateWithdrawalTicketInput {
                        method: "bank".into(),
                        account: "123".into(),
                    },
                    42,
                ),
        )
        .unwrap_err();
        assert!(matches!(error, TicketError::WithdrawBelowMinimum { .. }));
        assert!(ports.0.lock().unwrap().admissions.is_empty());
    }

    #[test]
    fn operator_reply_carries_mail_into_the_reply_transaction() {
        let ports = FakePorts::default();
        block_on(
            TicketService::new(ports.clone(), ports.clone(), ports.clone(), policy())
                .reply_as_operator(
                    7,
                    OperatorIdentity::Email("admin@example.test"),
                    "reply".into(),
                    50,
                    true,
                ),
        )
        .unwrap();
        let state = ports.0.lock().unwrap();
        assert_eq!(state.operator_delivery.as_ref().unwrap().batch_key, "batch");
        assert!(state.released.is_empty());
    }

    #[test]
    fn rejected_operator_reply_releases_only_its_notification_reservation() {
        let ports = FakePorts::default();
        ports.0.lock().unwrap().operator_outcome =
            Some(OperatorTicketReplyOutcome::OtherOpenTicketExists);
        let error = block_on(
            TicketService::new(ports.clone(), ports.clone(), ports.clone(), policy())
                .reply_as_operator(7, OperatorIdentity::UserId(99), "reply".into(), 50, true),
        )
        .unwrap_err();
        assert!(matches!(error, TicketError::UnresolvedTicketExists));
        assert_eq!(ports.0.lock().unwrap().released.len(), 1);
    }
}
