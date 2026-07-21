//! Worker reminder and durable-mail relay use cases over explicit outbound ports.
//!
//! Scheduling, cancellation, metrics, PostgreSQL leasing, SMTP, rendering, and
//! runtime configuration remain outer-adapter concerns. This module owns the
//! deterministic paging, retry, acknowledgement, and retention orchestration.

use std::fmt::Display;

const REMINDER_PAGE_SIZE: usize = 500;
const MAX_RETRY_BACKOFF_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderKind {
    Expire,
    Traffic,
}

impl ReminderKind {
    pub const ALL: [Self; 2] = [Self::Expire, Self::Traffic];

    pub const fn template_name(self) -> &'static str {
        match self {
            Self::Expire => "remindExpire",
            Self::Traffic => "remindTraffic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderEnvelope {
    pub sender: String,
    pub template_name: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("reminder envelope is unavailable: {detail}")]
pub struct ReminderPreparationError {
    detail: String,
}

impl ReminderPreparationError {
    pub fn new(error: impl Display) -> Self {
        Self {
            detail: error.to_string(),
        }
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{operation} failed: {detail}")]
pub struct MailWorkerPortError {
    operation: &'static str,
    detail: String,
}

impl MailWorkerPortError {
    pub fn new(operation: &'static str, error: impl Display) -> Self {
        Self {
            operation,
            detail: error.to_string(),
        }
    }

    pub const fn operation(&self) -> &'static str {
        self.operation
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

pub trait ReminderRenderer: Send + Sync {
    fn prepare(&self, kind: ReminderKind) -> Result<ReminderEnvelope, ReminderPreparationError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderPageCommand<'a> {
    pub kind: ReminderKind,
    pub envelope: &'a ReminderEnvelope,
    pub now: i64,
    pub business_day: &'a str,
    pub after_user_id: i64,
    pub limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReminderPageOutcome {
    /// Number of candidate rows consumed, including invalid recipients.
    pub selected: usize,
    /// Last consumed user id. `None` is the end-of-stream sentinel.
    pub last_user_id: Option<i64>,
    pub enqueued: usize,
    pub existing: usize,
    pub skipped: usize,
}

#[allow(async_fn_in_trait)]
pub trait ReminderRepository: Send + Sync {
    async fn enqueue_page(
        &self,
        command: ReminderPageCommand<'_>,
    ) -> Result<ReminderPageOutcome, MailWorkerPortError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderPreparationFailure {
    pub kind: ReminderKind,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReminderRunReport {
    pub enqueued: usize,
    pub existing: usize,
    pub skipped: usize,
    pub preparation_failures: Vec<ReminderPreparationFailure>,
}

#[derive(Debug, thiserror::Error)]
pub enum MailWorkerError {
    #[error(transparent)]
    Port(#[from] MailWorkerPortError),
    #[error("worker mail invariant failed: {0}")]
    Invariant(String),
    #[error("{failed} of {total} mail outbox items failed; first error: {first_error}")]
    PartialDelivery {
        total: usize,
        failed: usize,
        first_error: String,
    },
}

pub struct ReminderService<R, E> {
    repository: R,
    renderer: E,
}

impl<R, E> ReminderService<R, E>
where
    R: ReminderRepository,
    E: ReminderRenderer,
{
    pub const fn new(repository: R, renderer: E) -> Self {
        Self {
            repository,
            renderer,
        }
    }

    pub async fn run(
        &self,
        now: i64,
        business_day: &str,
    ) -> Result<ReminderRunReport, MailWorkerError> {
        let mut report = ReminderRunReport::default();
        for kind in ReminderKind::ALL {
            let envelope = match self.renderer.prepare(kind) {
                Ok(envelope) => envelope,
                Err(error) => {
                    report.skipped += 1;
                    report
                        .preparation_failures
                        .push(ReminderPreparationFailure {
                            kind,
                            detail: error.detail().to_string(),
                        });
                    continue;
                }
            };
            let mut after_user_id = 0_i64;
            loop {
                let page = self
                    .repository
                    .enqueue_page(ReminderPageCommand {
                        kind,
                        envelope: &envelope,
                        now,
                        business_day,
                        after_user_id,
                        limit: REMINDER_PAGE_SIZE,
                    })
                    .await?;
                if page.selected > REMINDER_PAGE_SIZE {
                    return Err(MailWorkerError::Invariant(format!(
                        "reminder repository returned {} rows for a {} row page",
                        page.selected, REMINDER_PAGE_SIZE
                    )));
                }
                report.enqueued += page.enqueued;
                report.existing += page.existing;
                report.skipped += page.skipped;
                let Some(last_user_id) = page.last_user_id else {
                    if page.selected != 0 {
                        return Err(MailWorkerError::Invariant(
                            "non-empty reminder page omitted its cursor".to_string(),
                        ));
                    }
                    break;
                };
                if page.selected == 0 || last_user_id <= after_user_id {
                    return Err(MailWorkerError::Invariant(
                        "reminder repository did not advance its cursor".to_string(),
                    ));
                }
                after_user_id = last_user_id;
                if page.selected < REMINDER_PAGE_SIZE {
                    break;
                }
            }
        }
        Ok(report)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedMailItem {
    pub id: i64,
    pub batch_key: String,
    pub sender: Option<String>,
    pub template_name: Option<String>,
    pub recipient: String,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub message_id: String,
    pub attempt_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedMailBatch {
    pub lease_token: String,
    pub items: Vec<ClaimedMailItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailOutboxPolicy {
    pub batch_size: usize,
    pub lease_seconds: i64,
    pub max_attempts: i32,
}

impl Default for MailOutboxPolicy {
    fn default() -> Self {
        Self {
            batch_size: 10,
            lease_seconds: 15 * 60,
            max_attempts: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailFailure {
    pub attempt_count: i32,
    pub available_at: i64,
    pub failed_at: Option<i64>,
    pub last_error: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub mail_retention_seconds: i64,
    pub idempotency_retention_seconds: i64,
    pub batch_size: usize,
    pub max_batches_per_table: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            mail_retention_seconds: 90 * 86_400,
            idempotency_retention_seconds: 90 * 86_400,
            batch_size: 1_000,
            max_batches_per_table: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionCleanup {
    pub mail_before: i64,
    pub idempotency_before: i64,
    pub batch_size: usize,
    pub max_batches_per_table: usize,
}

#[allow(async_fn_in_trait)]
pub trait MailOutboxRepository: Send + Sync {
    async fn claim(
        &self,
        now: i64,
        policy: MailOutboxPolicy,
    ) -> Result<Option<ClaimedMailBatch>, MailWorkerPortError>;

    async fn acknowledge(
        &self,
        lease_token: &str,
        item: &ClaimedMailItem,
        now: i64,
    ) -> Result<(), MailWorkerPortError>;

    async fn record_failure(
        &self,
        lease_token: &str,
        item: &ClaimedMailItem,
        failure: &MailFailure,
        now: i64,
    ) -> Result<(), MailWorkerPortError>;

    async fn cleanup(&self, cleanup: RetentionCleanup) -> Result<u64, MailWorkerPortError>;
}

#[allow(async_fn_in_trait)]
pub trait MailDelivery: Send + Sync {
    async fn deliver(&self, item: &ClaimedMailItem) -> Result<(), MailWorkerPortError>;
}

pub struct MailOutboxService<R, D> {
    repository: R,
    delivery: D,
    policy: MailOutboxPolicy,
}

impl<R, D> MailOutboxService<R, D>
where
    R: MailOutboxRepository,
    D: MailDelivery,
{
    pub fn new(repository: R, delivery: D, policy: MailOutboxPolicy) -> Self {
        Self {
            repository,
            delivery,
            policy,
        }
    }

    pub async fn deliver_batch(&self, now: i64) -> Result<usize, MailWorkerError> {
        validate_outbox_policy(self.policy)?;
        let Some(batch) = self.repository.claim(now, self.policy).await? else {
            return Ok(0);
        };
        let total = batch.items.len();
        let mut failed = 0_usize;
        let mut first_error = None;
        for item in batch.items {
            match self.delivery.deliver(&item).await {
                Ok(()) => {
                    if let Err(error) = self
                        .repository
                        .acknowledge(&batch.lease_token, &item, now)
                        .await
                    {
                        failed += 1;
                        first_error.get_or_insert_with(|| error.to_string());
                    }
                }
                Err(delivery_error) => {
                    let failure = failure_for(&item, self.policy, now, &delivery_error)?;
                    let persistence_error = self
                        .repository
                        .record_failure(&batch.lease_token, &item, &failure, now)
                        .await
                        .err();
                    failed += 1;
                    first_error.get_or_insert_with(|| match persistence_error {
                        Some(error) => format!(
                            "{delivery_error}; additionally could not persist the failure: {error}"
                        ),
                        None => delivery_error.to_string(),
                    });
                }
            }
        }
        if failed == 0 {
            Ok(total)
        } else {
            Err(MailWorkerError::PartialDelivery {
                total,
                failed,
                first_error: first_error.unwrap_or_else(|| "unknown error".to_string()),
            })
        }
    }

    pub async fn cleanup(&self, now: i64, policy: RetentionPolicy) -> Result<u64, MailWorkerError> {
        if policy.mail_retention_seconds <= 0
            || policy.idempotency_retention_seconds <= 0
            || policy.batch_size == 0
            || policy.max_batches_per_table == 0
        {
            return Err(MailWorkerError::Invariant(
                "retention policy values must be positive".to_string(),
            ));
        }
        let mail_before = now
            .checked_sub(policy.mail_retention_seconds)
            .ok_or_else(|| MailWorkerError::Invariant("mail retention cutoff underflow".into()))?;
        let idempotency_before = now
            .checked_sub(policy.idempotency_retention_seconds)
            .ok_or_else(|| {
                MailWorkerError::Invariant("idempotency retention cutoff underflow".into())
            })?;
        self.repository
            .cleanup(RetentionCleanup {
                mail_before,
                idempotency_before,
                batch_size: policy.batch_size,
                max_batches_per_table: policy.max_batches_per_table,
            })
            .await
            .map_err(Into::into)
    }
}

fn validate_outbox_policy(policy: MailOutboxPolicy) -> Result<(), MailWorkerError> {
    if policy.batch_size == 0 || policy.lease_seconds <= 0 || policy.max_attempts <= 0 {
        return Err(MailWorkerError::Invariant(
            "mail outbox policy values must be positive".to_string(),
        ));
    }
    Ok(())
}

fn failure_for(
    item: &ClaimedMailItem,
    policy: MailOutboxPolicy,
    now: i64,
    error: &MailWorkerPortError,
) -> Result<MailFailure, MailWorkerError> {
    let attempt_count = item
        .attempt_count
        .checked_add(1)
        .ok_or_else(|| MailWorkerError::Invariant("mail attempt counter overflow".into()))?;
    let terminal = attempt_count >= policy.max_attempts;
    let available_at = now
        .checked_add(retry_backoff_seconds(attempt_count))
        .ok_or_else(|| MailWorkerError::Invariant("mail retry timestamp overflow".into()))?;
    Ok(MailFailure {
        attempt_count,
        available_at,
        failed_at: terminal.then_some(now),
        last_error: error.to_string().chars().take(4096).collect(),
    })
}

fn retry_backoff_seconds(attempt_count: i32) -> i64 {
    let exponent = u32::try_from(attempt_count.saturating_sub(1).min(10)).unwrap_or_default();
    (5_i64.saturating_mul(1_i64 << exponent)).min(MAX_RETRY_BACKOFF_SECONDS)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        future::Future,
        pin::pin,
        sync::Mutex,
        task::{Context, Poll, Waker},
    };

    use super::*;

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    struct FakeRenderer;

    impl ReminderRenderer for FakeRenderer {
        fn prepare(
            &self,
            kind: ReminderKind,
        ) -> Result<ReminderEnvelope, ReminderPreparationError> {
            if kind == ReminderKind::Traffic {
                return Err(ReminderPreparationError::new("sender missing"));
            }
            Ok(ReminderEnvelope {
                sender: "Board <sender@example.test>".into(),
                template_name: "mail.default.remindExpire".into(),
                subject: "Expiry".into(),
                body: "Body".into(),
            })
        }
    }

    struct FakeReminderRepository {
        pages: Mutex<VecDeque<ReminderPageOutcome>>,
        cursors: Mutex<Vec<i64>>,
    }

    impl ReminderRepository for FakeReminderRepository {
        async fn enqueue_page(
            &self,
            command: ReminderPageCommand<'_>,
        ) -> Result<ReminderPageOutcome, MailWorkerPortError> {
            self.cursors.lock().unwrap().push(command.after_user_id);
            Ok(self.pages.lock().unwrap().pop_front().unwrap_or_default())
        }
    }

    #[test]
    fn reminder_use_case_pages_once_and_treats_invalid_configuration_as_skipped() {
        let repository = FakeReminderRepository {
            pages: Mutex::new(VecDeque::from([ReminderPageOutcome {
                selected: 2,
                last_user_id: Some(20),
                enqueued: 1,
                existing: 1,
                skipped: 0,
            }])),
            cursors: Mutex::new(Vec::new()),
        };
        let service = ReminderService::new(repository, FakeRenderer);
        let report = block_on(service.run(100, "2026-07-20")).unwrap();
        assert_eq!(
            (report.enqueued, report.existing, report.skipped),
            (1, 1, 1)
        );
        assert_eq!(report.preparation_failures.len(), 1);
        assert_eq!(report.preparation_failures[0].kind, ReminderKind::Traffic);
        assert_eq!(*service.repository.cursors.lock().unwrap(), vec![0]);
    }

    #[derive(Default)]
    struct FakeOutboxRepository {
        batch: Mutex<Option<ClaimedMailBatch>>,
        acknowledgements: Mutex<Vec<i64>>,
        failures: Mutex<Vec<(i64, MailFailure)>>,
        cleanups: Mutex<Vec<RetentionCleanup>>,
    }

    impl MailOutboxRepository for FakeOutboxRepository {
        async fn claim(
            &self,
            _now: i64,
            _policy: MailOutboxPolicy,
        ) -> Result<Option<ClaimedMailBatch>, MailWorkerPortError> {
            Ok(self.batch.lock().unwrap().take())
        }

        async fn acknowledge(
            &self,
            _lease_token: &str,
            item: &ClaimedMailItem,
            _now: i64,
        ) -> Result<(), MailWorkerPortError> {
            self.acknowledgements.lock().unwrap().push(item.id);
            Ok(())
        }

        async fn record_failure(
            &self,
            _lease_token: &str,
            item: &ClaimedMailItem,
            failure: &MailFailure,
            _now: i64,
        ) -> Result<(), MailWorkerPortError> {
            self.failures
                .lock()
                .unwrap()
                .push((item.id, failure.clone()));
            Ok(())
        }

        async fn cleanup(&self, cleanup: RetentionCleanup) -> Result<u64, MailWorkerPortError> {
            self.cleanups.lock().unwrap().push(cleanup);
            Ok(7)
        }
    }

    struct FakeDelivery;

    impl MailDelivery for FakeDelivery {
        async fn deliver(&self, item: &ClaimedMailItem) -> Result<(), MailWorkerPortError> {
            if item.id == 2 {
                Err(MailWorkerPortError::new("deliver mail", "rejected"))
            } else {
                Ok(())
            }
        }
    }

    fn item(id: i64, attempt_count: i32) -> ClaimedMailItem {
        ClaimedMailItem {
            id,
            batch_key: format!("batch-{id}"),
            sender: Some("Board <sender@example.test>".into()),
            template_name: Some("mail.default.notify".into()),
            recipient: format!("user-{id}@example.test"),
            subject: Some("Subject".into()),
            body: Some("Body".into()),
            message_id: format!("<{id}@mail.v2board.local>"),
            attempt_count,
        }
    }

    #[test]
    fn outbox_use_case_acknowledges_success_and_persists_terminal_failure() {
        let repository = FakeOutboxRepository {
            batch: Mutex::new(Some(ClaimedMailBatch {
                lease_token: "lease".into(),
                items: vec![item(1, 0), item(2, 7)],
            })),
            ..FakeOutboxRepository::default()
        };
        let service = MailOutboxService::new(repository, FakeDelivery, MailOutboxPolicy::default());
        let error = block_on(service.deliver_batch(1_000)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("1 of 2 mail outbox items failed")
        );
        assert_eq!(
            *service.repository.acknowledgements.lock().unwrap(),
            vec![1]
        );
        let failures = service.repository.failures.lock().unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].0, 2);
        assert_eq!(failures[0].1.attempt_count, 8);
        assert_eq!(failures[0].1.failed_at, Some(1_000));
        assert_eq!(failures[0].1.available_at, 1_640);
    }

    #[test]
    fn retention_use_case_computes_cutoffs_before_calling_the_adapter() {
        let repository = FakeOutboxRepository::default();
        let service = MailOutboxService::new(repository, FakeDelivery, MailOutboxPolicy::default());
        let policy = RetentionPolicy {
            mail_retention_seconds: 100,
            idempotency_retention_seconds: 200,
            batch_size: 25,
            max_batches_per_table: 3,
        };
        assert_eq!(block_on(service.cleanup(1_000, policy)).unwrap(), 7);
        assert_eq!(
            *service.repository.cleanups.lock().unwrap(),
            vec![RetentionCleanup {
                mail_before: 900,
                idempotency_before: 800,
                batch_size: 25,
                max_batches_per_table: 3,
            }]
        );
    }

    #[test]
    fn retry_backoff_is_bounded() {
        assert_eq!(retry_backoff_seconds(1), 5);
        assert_eq!(retry_backoff_seconds(2), 10);
        assert_eq!(retry_backoff_seconds(i32::MAX), 60 * 60);
    }
}
