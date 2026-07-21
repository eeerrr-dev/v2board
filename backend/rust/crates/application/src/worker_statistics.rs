//! Daily statistics aggregation use case over a persistence port.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatisticsWindow {
    pub start_at: i64,
    pub end_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawDailyStatistics {
    pub order_count: i64,
    pub order_total: String,
    pub commission_count: i64,
    pub commission_total: String,
    pub paid_count: i64,
    pub paid_total: String,
    pub register_count: i64,
    pub invite_count: i64,
    pub transfer_used_total: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyStatisticsRecord {
    pub record_at: i64,
    pub order_count: i32,
    pub order_total: i64,
    pub commission_count: i32,
    pub commission_total: i64,
    pub paid_count: i32,
    pub paid_total: i64,
    pub register_count: i32,
    pub invite_count: i32,
    pub transfer_used_total: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum StatisticsWorkerError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("statistics window must have a positive duration")]
    InvalidWindow,
    #[error("{metric} aggregate is not a valid integer")]
    InvalidAggregate { metric: &'static str },
    #[error("{metric} aggregate exceeds the supported range")]
    AggregateOutOfRange { metric: &'static str },
}

#[allow(async_fn_in_trait)]
pub trait StatisticsWorkerRepository: Send + Sync {
    async fn aggregate(&self, window: StatisticsWindow) -> RepositoryResult<RawDailyStatistics>;
    async fn upsert(&self, record: &DailyStatisticsRecord) -> RepositoryResult<()>;
}

#[derive(Clone, Debug)]
pub struct StatisticsWorkerService<R> {
    repository: R,
}

impl<R> StatisticsWorkerService<R>
where
    R: StatisticsWorkerRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn run(
        &self,
        window: StatisticsWindow,
        now: i64,
    ) -> Result<DailyStatisticsRecord, StatisticsWorkerError> {
        if window.start_at >= window.end_at {
            return Err(StatisticsWorkerError::InvalidWindow);
        }
        let raw = self.repository.aggregate(window).await?;
        let record = DailyStatisticsRecord {
            record_at: window.start_at,
            order_count: exact_i32_count(raw.order_count, "order count")?,
            order_total: exact_i64_aggregate(&raw.order_total, "order total")?,
            commission_count: exact_i32_count(raw.commission_count, "commission count")?,
            commission_total: exact_i64_aggregate(&raw.commission_total, "commission total")?,
            paid_count: exact_i32_count(raw.paid_count, "paid order count")?,
            paid_total: exact_i64_aggregate(&raw.paid_total, "paid total")?,
            register_count: exact_i32_count(raw.register_count, "registration count")?,
            invite_count: exact_i32_count(raw.invite_count, "invite count")?,
            transfer_used_total: exact_i64_aggregate(&raw.transfer_used_total, "traffic total")?,
            created_at: now,
            updated_at: now,
        };
        self.repository.upsert(&record).await?;
        Ok(record)
    }
}

fn exact_i64_aggregate(value: &str, metric: &'static str) -> Result<i64, StatisticsWorkerError> {
    let exact = value
        .parse::<i128>()
        .map_err(|_| StatisticsWorkerError::InvalidAggregate { metric })?;
    i64::try_from(exact).map_err(|_| StatisticsWorkerError::AggregateOutOfRange { metric })
}

fn exact_i32_count(value: i64, metric: &'static str) -> Result<i32, StatisticsWorkerError> {
    i32::try_from(value).map_err(|_| StatisticsWorkerError::AggregateOutOfRange { metric })
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
    struct FakeRepository {
        raw: RawDailyStatistics,
        stored: Arc<Mutex<Option<DailyStatisticsRecord>>>,
    }

    impl StatisticsWorkerRepository for FakeRepository {
        async fn aggregate(&self, _: StatisticsWindow) -> RepositoryResult<RawDailyStatistics> {
            Ok(self.raw.clone())
        }

        async fn upsert(&self, record: &DailyStatisticsRecord) -> RepositoryResult<()> {
            *self.stored.lock().unwrap() = Some(record.clone());
            Ok(())
        }
    }

    fn raw() -> RawDailyStatistics {
        RawDailyStatistics {
            order_count: 2,
            order_total: "30".to_string(),
            commission_count: 1,
            commission_total: "4".to_string(),
            paid_count: 1,
            paid_total: "20".to_string(),
            register_count: 3,
            invite_count: 1,
            transfer_used_total: "50".to_string(),
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
    fn exact_aggregates_are_validated_before_the_record_is_persisted() {
        let stored = Arc::new(Mutex::new(None));
        let repository = FakeRepository {
            raw: raw(),
            stored: stored.clone(),
        };
        let record = run(StatisticsWorkerService::new(repository).run(
            StatisticsWindow {
                start_at: 100,
                end_at: 200,
            },
            300,
        ))
        .unwrap();
        assert_eq!((record.order_count, record.order_total), (2, 30));
        assert_eq!(stored.lock().unwrap().as_ref(), Some(&record));
    }

    #[test]
    fn aggregate_overflow_is_rejected_without_an_upsert() {
        let stored = Arc::new(Mutex::new(None));
        let mut value = raw();
        value.order_total = "9223372036854775808".to_string();
        let error = run(StatisticsWorkerService::new(FakeRepository {
            raw: value,
            stored: stored.clone(),
        })
        .run(
            StatisticsWindow {
                start_at: 100,
                end_at: 200,
            },
            300,
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            StatisticsWorkerError::AggregateOutOfRange {
                metric: "order total"
            }
        ));
        assert!(stored.lock().unwrap().is_none());
    }
}
