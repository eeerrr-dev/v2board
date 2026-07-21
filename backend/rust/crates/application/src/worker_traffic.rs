//! Bounded durable-traffic accounting orchestration for the background worker.
//!
//! PostgreSQL row locking, analytics outbox writes, Redis, and wall-clock
//! implementations are outbound adapters. This use case owns the reset
//! barrier, bounded drain policy, and observable accounting summary.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficDrainPolicy {
    pub max_reports: usize,
    pub budget_millis: u64,
}

impl Default for TrafficDrainPolicy {
    fn default() -> Self {
        Self {
            max_reports: 10_000,
            budget_millis: 45_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedTrafficReport {
    pub report_key: String,
    pub stale_items: usize,
    pub missing_users: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrafficDrainOutcome {
    pub processed: usize,
    pub stale_items: usize,
    pub missing_users: usize,
    pub skipped_for_reset: bool,
    pub exhausted: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum TrafficWorkerError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("traffic drain policy values must be positive")]
    InvalidPolicy,
    #[error("traffic drain counters exceed the supported range")]
    CounterOverflow,
}

#[allow(async_fn_in_trait)]
pub trait TrafficAccountingRepository: Send + Sync {
    /// Atomically claims and applies one report. The implementation must keep
    /// the user updates, accounted analytics events, and report acknowledgement
    /// in the same transaction.
    async fn apply_next(
        &self,
        installation_id: &str,
        accounted_at: i64,
    ) -> RepositoryResult<Option<AppliedTrafficReport>>;
}

#[allow(async_fn_in_trait)]
pub trait TrafficResetBarrier: Send + Sync {
    async fn reset_in_progress(&self) -> RepositoryResult<bool>;
}

pub trait WorkerClock: Send + Sync {
    fn unix_timestamp(&self) -> i64;
    fn monotonic_millis(&self) -> u64;
}

#[derive(Clone, Debug)]
pub struct TrafficWorkerService<R, B, C> {
    repository: R,
    barrier: B,
    clock: C,
    policy: TrafficDrainPolicy,
}

impl<R, B, C> TrafficWorkerService<R, B, C>
where
    R: TrafficAccountingRepository,
    B: TrafficResetBarrier,
    C: WorkerClock,
{
    pub const fn new(repository: R, barrier: B, clock: C, policy: TrafficDrainPolicy) -> Self {
        Self {
            repository,
            barrier,
            clock,
            policy,
        }
    }

    pub async fn run(
        &self,
        installation_id: &str,
    ) -> Result<TrafficDrainOutcome, TrafficWorkerError> {
        if self.policy.max_reports == 0 || self.policy.budget_millis == 0 {
            return Err(TrafficWorkerError::InvalidPolicy);
        }
        if self.barrier.reset_in_progress().await? {
            return Ok(TrafficDrainOutcome {
                skipped_for_reset: true,
                ..TrafficDrainOutcome::default()
            });
        }

        let deadline = self
            .clock
            .monotonic_millis()
            .saturating_add(self.policy.budget_millis);
        let mut outcome = TrafficDrainOutcome::default();
        let mut source_empty = false;
        while outcome.processed < self.policy.max_reports
            && self.clock.monotonic_millis() < deadline
        {
            let Some(applied) = self
                .repository
                .apply_next(installation_id, self.clock.unix_timestamp())
                .await?
            else {
                source_empty = true;
                break;
            };
            outcome.processed = outcome
                .processed
                .checked_add(1)
                .ok_or(TrafficWorkerError::CounterOverflow)?;
            outcome.stale_items = outcome
                .stale_items
                .checked_add(applied.stale_items)
                .ok_or(TrafficWorkerError::CounterOverflow)?;
            outcome.missing_users = outcome
                .missing_users
                .checked_add(applied.missing_users)
                .ok_or(TrafficWorkerError::CounterOverflow)?;
        }
        outcome.exhausted = !source_empty
            && (outcome.processed == self.policy.max_reports
                || self.clock.monotonic_millis() >= deadline);
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<VecDeque<AppliedTrafficReport>>>);

    impl TrafficAccountingRepository for FakeRepository {
        async fn apply_next(
            &self,
            _: &str,
            _: i64,
        ) -> RepositoryResult<Option<AppliedTrafficReport>> {
            Ok(self.0.lock().unwrap().pop_front())
        }
    }

    #[derive(Clone, Copy)]
    struct FakeBarrier(bool);

    impl TrafficResetBarrier for FakeBarrier {
        async fn reset_in_progress(&self) -> RepositoryResult<bool> {
            Ok(self.0)
        }
    }

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl WorkerClock for FixedClock {
        fn unix_timestamp(&self) -> i64 {
            1_700_000_000
        }

        fn monotonic_millis(&self) -> u64 {
            10
        }
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
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
    fn reset_barrier_short_circuits_without_claiming_reports() {
        let repository = FakeRepository::default();
        repository
            .0
            .lock()
            .unwrap()
            .push_back(AppliedTrafficReport {
                report_key: "report".to_string(),
                stale_items: 0,
                missing_users: 0,
            });
        let outcome = run(TrafficWorkerService::new(
            repository.clone(),
            FakeBarrier(true),
            FixedClock,
            TrafficDrainPolicy::default(),
        )
        .run("installation"))
        .unwrap();
        assert!(outcome.skipped_for_reset);
        assert_eq!(repository.0.lock().unwrap().len(), 1);
    }

    #[test]
    fn drain_aggregates_repository_outcomes_until_the_source_is_empty() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().extend([
            AppliedTrafficReport {
                report_key: "first".to_string(),
                stale_items: 2,
                missing_users: 0,
            },
            AppliedTrafficReport {
                report_key: "second".to_string(),
                stale_items: 0,
                missing_users: 1,
            },
        ]);
        let outcome = run(TrafficWorkerService::new(
            repository,
            FakeBarrier(false),
            FixedClock,
            TrafficDrainPolicy::default(),
        )
        .run("installation"))
        .unwrap();
        assert_eq!(outcome.processed, 2);
        assert_eq!((outcome.stale_items, outcome.missing_users), (2, 1));
        assert!(!outcome.exhausted);
    }
}
