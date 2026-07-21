//! Worker-health reporting independent of Redis and HTTP projection types.

use std::collections::{BTreeMap, BTreeSet};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkerSnapshot {
    pub schedule_last_seen_at: Option<i64>,
    pub totals: BTreeMap<String, i64>,
    pub failed: BTreeMap<String, i64>,
    pub last_run_at: BTreeMap<String, i64>,
    pub last_success_at: BTreeMap<String, i64>,
    pub last_failure_at: BTreeMap<String, i64>,
}

impl WorkerSnapshot {
    fn total_jobs(&self) -> i64 {
        self.totals.values().copied().fold(0, i64::saturating_add)
    }

    fn failed_jobs(&self) -> i64 {
        self.failed.values().copied().fold(0, i64::saturating_add)
    }

    fn last_seen_at(&self) -> Option<i64> {
        self.schedule_last_seen_at
            .into_iter()
            .chain(self.last_run_at.values().copied())
            .max()
    }

    fn job_names(&self) -> Vec<String> {
        self.totals
            .keys()
            .chain(self.failed.keys())
            .chain(self.last_run_at.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn running(&self, now: i64) -> bool {
        self.last_seen_at()
            .is_some_and(|last_seen| timestamp_is_recent(now, last_seen, 180))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIdentity {
    pub log_level: String,
    pub backend_version: String,
    pub frontend_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemStatus {
    pub schedule: bool,
    pub worker: bool,
    pub schedule_last_seen_at: Option<i64>,
    pub runtime: RuntimeIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueStats {
    pub failed_jobs: i64,
    pub jobs_per_minute: usize,
    pub recent_jobs: i64,
    pub worker_running: bool,
    pub queue_with_max_throughput: Option<String>,
    pub last_run_at: BTreeMap<String, i64>,
    pub last_success_at: BTreeMap<String, i64>,
    pub last_failure_at: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueWorkload {
    pub name: String,
    pub processes: i64,
    pub recent_jobs: i64,
    pub failed_jobs: i64,
    pub last_run_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_failure_at: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueMaster {
    pub running: bool,
    pub supervisors: Vec<String>,
    pub last_seen_at: Option<i64>,
    pub schedule_last_seen_at: Option<i64>,
}

#[allow(async_fn_in_trait)]
pub trait WorkerMetricsRepository: Send + Sync {
    async fn snapshot(&self) -> RepositoryResult<WorkerSnapshot>;
}

#[derive(Clone, Debug)]
pub struct SystemMonitoringService<R> {
    repository: R,
}

impl<R> SystemMonitoringService<R>
where
    R: WorkerMetricsRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn status(
        &self,
        now: i64,
        runtime: RuntimeIdentity,
    ) -> RepositoryResult<SystemStatus> {
        let snapshot = self.repository.snapshot().await?;
        Ok(SystemStatus {
            schedule: snapshot
                .schedule_last_seen_at
                .is_some_and(|seen| timestamp_is_recent(now, seen, 180)),
            worker: snapshot.running(now),
            schedule_last_seen_at: snapshot.schedule_last_seen_at,
            runtime,
        })
    }

    pub async fn queue_stats(&self, now: i64) -> RepositoryResult<QueueStats> {
        let snapshot = self.repository.snapshot().await?;
        let queue_with_max_throughput = snapshot
            .totals
            .iter()
            .max_by_key(|(_, value)| *value)
            .map(|(name, _)| name.clone());
        Ok(QueueStats {
            failed_jobs: snapshot.failed_jobs(),
            jobs_per_minute: snapshot
                .last_run_at
                .values()
                .filter(|seen| timestamp_is_recent(now, **seen, 60))
                .count(),
            recent_jobs: snapshot.total_jobs(),
            worker_running: snapshot.running(now),
            queue_with_max_throughput,
            last_run_at: snapshot.last_run_at,
            last_success_at: snapshot.last_success_at,
            last_failure_at: snapshot.last_failure_at,
        })
    }

    pub async fn workload(&self, now: i64) -> RepositoryResult<Vec<QueueWorkload>> {
        let snapshot = self.repository.snapshot().await?;
        Ok(snapshot
            .job_names()
            .into_iter()
            .map(|name| {
                let last_run_at = snapshot.last_run_at.get(&name).copied();
                QueueWorkload {
                    processes: i64::from(
                        last_run_at.is_some_and(|seen| timestamp_is_recent(now, seen, 180)),
                    ),
                    recent_jobs: snapshot.totals.get(&name).copied().unwrap_or_default(),
                    failed_jobs: snapshot.failed.get(&name).copied().unwrap_or_default(),
                    last_success_at: snapshot.last_success_at.get(&name).copied(),
                    last_failure_at: snapshot.last_failure_at.get(&name).copied(),
                    name,
                    last_run_at,
                }
            })
            .collect())
    }

    pub async fn masters(&self, now: i64) -> RepositoryResult<Vec<QueueMaster>> {
        let snapshot = self.repository.snapshot().await?;
        Ok(vec![QueueMaster {
            running: snapshot.running(now),
            supervisors: snapshot.job_names(),
            last_seen_at: snapshot.last_seen_at(),
            schedule_last_seen_at: snapshot.schedule_last_seen_at,
        }])
    }
}

fn timestamp_is_recent(now: i64, last_seen: i64, seconds: i64) -> bool {
    now.saturating_sub(last_seen) <= seconds
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

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<WorkerSnapshot>>);

    impl WorkerMetricsRepository for FakeRepository {
        async fn snapshot(&self) -> RepositoryResult<WorkerSnapshot> {
            Ok(self.0.lock().unwrap().clone())
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
    fn counters_saturate_and_recency_is_overflow_safe() {
        let repository = FakeRepository::default();
        *repository.0.lock().unwrap() = WorkerSnapshot {
            schedule_last_seen_at: Some(i64::MIN),
            totals: BTreeMap::from([("a".to_string(), i64::MAX), ("b".to_string(), 1)]),
            failed: BTreeMap::from([("a".to_string(), i64::MIN), ("b".to_string(), -1)]),
            ..WorkerSnapshot::default()
        };
        let stats = run(SystemMonitoringService::new(repository).queue_stats(i64::MAX)).unwrap();
        assert_eq!(stats.recent_jobs, i64::MAX);
        assert_eq!(stats.failed_jobs, i64::MIN);
        assert!(!stats.worker_running);
    }

    #[test]
    fn workload_uses_union_of_known_job_names() {
        let repository = FakeRepository::default();
        *repository.0.lock().unwrap() = WorkerSnapshot {
            failed: BTreeMap::from([("failed-only".to_string(), 2)]),
            last_run_at: BTreeMap::from([("active".to_string(), 90)]),
            ..WorkerSnapshot::default()
        };
        let rows = run(SystemMonitoringService::new(repository).workload(100)).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
            ["active", "failed-only"]
        );
        assert_eq!(rows[0].processes, 1);
    }
}
