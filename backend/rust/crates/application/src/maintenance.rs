//! Scheduled traffic-reset and retention use cases.
//!
//! Calendar conversion and persistence are outbound ports: the application
//! layer owns which accounts are reset and how bounded retention work is
//! orchestrated without depending on Chrono or SQLx.

use v2board_domain_model::{
    CalendarDay, ScheduledTrafficResetPolicy, TrafficResetFacts, TrafficResetMethod,
    scheduled_traffic_reset_due,
};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledResetCandidate {
    pub id: i64,
    pub expired_at: i64,
    pub reset_traffic_method: Option<i16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledTrafficResetRun {
    pub now_epoch: i64,
    pub now_day: CalendarDay,
    pub reset_key: String,
    pub default_method: i32,
    pub batch_size: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScheduledTrafficResetOutcome {
    pub examined: u64,
    pub reset: u64,
}

pub trait TrafficResetCalendar: Send + Sync {
    fn day_at(&self, timestamp: i64) -> Option<CalendarDay>;
}

#[allow(async_fn_in_trait)]
pub trait ScheduledTrafficResetBatch: Send {
    async fn lock_candidates(
        &mut self,
        after_id: i64,
        now_epoch: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<ScheduledResetCandidate>>;

    async fn apply_reset(
        &mut self,
        user_ids: &[i64],
        reset_key: &str,
        updated_at: i64,
    ) -> RepositoryResult<u64>;

    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait ScheduledTrafficResetRepository: Send + Sync {
    type Batch<'a>: ScheduledTrafficResetBatch
    where
        Self: 'a;

    async fn begin_batch(&self) -> RepositoryResult<Self::Batch<'_>>;
}

#[derive(Clone, Debug)]
pub struct ScheduledTrafficResetService<R, C> {
    repository: R,
    calendar: C,
}

impl<R, C> ScheduledTrafficResetService<R, C>
where
    R: ScheduledTrafficResetRepository,
    C: TrafficResetCalendar,
{
    pub const fn new(repository: R, calendar: C) -> Self {
        Self {
            repository,
            calendar,
        }
    }

    pub async fn run(
        &self,
        command: &ScheduledTrafficResetRun,
    ) -> RepositoryResult<ScheduledTrafficResetOutcome> {
        let mut outcome = ScheduledTrafficResetOutcome::default();
        let mut after_id = 0_i64;
        loop {
            let mut batch = self.repository.begin_batch().await?;
            let candidates = batch
                .lock_candidates(after_id, command.now_epoch, command.batch_size)
                .await?;
            let Some(last_id) = candidates.last().map(|candidate| candidate.id) else {
                batch.commit().await?;
                break;
            };
            outcome.examined = outcome
                .examined
                .saturating_add(u64::try_from(candidates.len()).unwrap_or(u64::MAX));
            let ids = candidates
                .iter()
                .filter(|candidate| scheduled_reset_is_due(candidate, command, &self.calendar))
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>();
            if !ids.is_empty() {
                outcome.reset = outcome.reset.saturating_add(
                    batch
                        .apply_reset(&ids, &command.reset_key, command.now_epoch)
                        .await?,
                );
            }
            batch.commit().await?;
            after_id = last_id;
        }
        Ok(outcome)
    }
}

pub fn scheduled_reset_is_due(
    candidate: &ScheduledResetCandidate,
    command: &ScheduledTrafficResetRun,
    calendar: &impl TrafficResetCalendar,
) -> bool {
    let policy = match candidate.reset_traffic_method {
        Some(method) => {
            traffic_reset_method(i32::from(method)).map(ScheduledTrafficResetPolicy::Explicit)
        }
        None => traffic_reset_method(command.default_method)
            .map(ScheduledTrafficResetPolicy::LegacyDefault),
    };
    let Some(policy) = policy else {
        return false;
    };
    let Some(expiry_day) = calendar.day_at(candidate.expired_at) else {
        return false;
    };
    scheduled_traffic_reset_due(
        policy,
        TrafficResetFacts {
            now: command.now_day,
            expiry: expiry_day,
            now_epoch: command.now_epoch,
            expiry_epoch: candidate.expired_at,
        },
    )
}

const fn traffic_reset_method(method: i32) -> Option<TrafficResetMethod> {
    match method {
        0 => Some(TrafficResetMethod::MonthStart),
        1 => Some(TrafficResetMethod::ExpiryDay),
        2 => Some(TrafficResetMethod::Never),
        3 => Some(TrafficResetMethod::YearStart),
        4 => Some(TrafficResetMethod::ExpiryAnniversary),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionDataset {
    UserTraffic,
    ServerTraffic,
    SystemLog,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionCutoff {
    pub dataset: RetentionDataset,
    pub before: i64,
}

#[allow(async_fn_in_trait)]
pub trait RetentionRepository: Send + Sync {
    async fn delete_batch(&self, cutoff: RetentionCutoff, batch_size: i64)
    -> RepositoryResult<u64>;
}

#[derive(Clone, Debug)]
pub struct RetentionService<R> {
    repository: R,
}

impl<R> RetentionService<R>
where
    R: RetentionRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn prune(
        &self,
        cutoffs: &[RetentionCutoff],
        batch_size: i64,
        max_batches_per_dataset: usize,
    ) -> RepositoryResult<u64> {
        let mut deleted_total = 0_u64;
        for cutoff in cutoffs {
            for _ in 0..max_batches_per_dataset {
                let deleted = self.repository.delete_batch(*cutoff, batch_size).await?;
                deleted_total = deleted_total.saturating_add(deleted);
                if deleted < u64::try_from(batch_size).unwrap_or(u64::MAX) {
                    break;
                }
            }
        }
        Ok(deleted_total)
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

    #[derive(Clone)]
    struct FixedCalendar;

    impl TrafficResetCalendar for FixedCalendar {
        fn day_at(&self, timestamp: i64) -> Option<CalendarDay> {
            match timestamp {
                500 => CalendarDay::new(3, 31, 31).ok(),
                600 => CalendarDay::new(4, 15, 30).ok(),
                _ => None,
            }
        }
    }

    #[derive(Clone, Default)]
    struct FakeResetRepository {
        rows: Arc<Vec<ScheduledResetCandidate>>,
        reset_ids: Arc<Mutex<Vec<i64>>>,
    }

    struct FakeResetBatch {
        rows: Arc<Vec<ScheduledResetCandidate>>,
        reset_ids: Arc<Mutex<Vec<i64>>>,
    }

    impl ScheduledTrafficResetBatch for FakeResetBatch {
        async fn lock_candidates(
            &mut self,
            after_id: i64,
            _now_epoch: i64,
            limit: i64,
        ) -> RepositoryResult<Vec<ScheduledResetCandidate>> {
            Ok(self
                .rows
                .iter()
                .copied()
                .filter(|row| row.id > after_id)
                .take(usize::try_from(limit).unwrap_or_default())
                .collect())
        }

        async fn apply_reset(
            &mut self,
            user_ids: &[i64],
            _reset_key: &str,
            _updated_at: i64,
        ) -> RepositoryResult<u64> {
            self.reset_ids.lock().unwrap().extend_from_slice(user_ids);
            Ok(u64::try_from(user_ids.len()).unwrap())
        }

        async fn commit(self) -> RepositoryResult<()> {
            Ok(())
        }
    }

    impl ScheduledTrafficResetRepository for FakeResetRepository {
        type Batch<'a> = FakeResetBatch;

        async fn begin_batch(&self) -> RepositoryResult<Self::Batch<'_>> {
            Ok(FakeResetBatch {
                rows: self.rows.clone(),
                reset_ids: self.reset_ids.clone(),
            })
        }
    }

    #[derive(Clone, Default)]
    struct FakeRetentionRepository(Arc<Mutex<Vec<(RetentionDataset, u64)>>>);

    impl RetentionRepository for FakeRetentionRepository {
        async fn delete_batch(
            &self,
            cutoff: RetentionCutoff,
            _batch_size: i64,
        ) -> RepositoryResult<u64> {
            let mut calls = self.0.lock().unwrap();
            let prior = calls
                .iter()
                .filter(|(dataset, _)| *dataset == cutoff.dataset)
                .count();
            let deleted = if prior == 0 { 5 } else { 2 };
            calls.push((cutoff.dataset, deleted));
            Ok(deleted)
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
    fn reset_service_owns_policy_and_batches_transactionally() {
        let repository = FakeResetRepository {
            rows: Arc::new(vec![
                ScheduledResetCandidate {
                    id: 1,
                    expired_at: 500,
                    reset_traffic_method: Some(1),
                },
                ScheduledResetCandidate {
                    id: 2,
                    expired_at: 600,
                    reset_traffic_method: Some(2),
                },
                ScheduledResetCandidate {
                    id: 3,
                    expired_at: 500,
                    reset_traffic_method: None,
                },
            ]),
            ..FakeResetRepository::default()
        };
        let command = ScheduledTrafficResetRun {
            now_epoch: -3_000_000,
            now_day: CalendarDay::new(3, 31, 31).unwrap(),
            reset_key: "2026-03-31".to_string(),
            default_method: 3,
            batch_size: 2,
        };
        let outcome =
            run(ScheduledTrafficResetService::new(repository.clone(), FixedCalendar).run(&command))
                .unwrap();
        assert_eq!(outcome.examined, 3);
        assert_eq!(outcome.reset, 2);
        assert_eq!(*repository.reset_ids.lock().unwrap(), vec![1, 3]);
    }

    #[test]
    fn retention_is_bounded_per_dataset() {
        let repository = FakeRetentionRepository::default();
        let deleted = run(RetentionService::new(repository.clone()).prune(
            &[
                RetentionCutoff {
                    dataset: RetentionDataset::UserTraffic,
                    before: 10,
                },
                RetentionCutoff {
                    dataset: RetentionDataset::SystemLog,
                    before: 20,
                },
            ],
            5,
            3,
        ))
        .unwrap();
        assert_eq!(deleted, 14);
        assert_eq!(repository.0.lock().unwrap().len(), 4);
    }
}
