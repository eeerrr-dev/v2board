use chrono::{Datelike, TimeZone, Utc};
use redis::AsyncCommands;
use v2board_application::statistics::{StatisticsBoundaries, StatisticsCalendar};
use v2board_application::{
    RepositoryError,
    system_monitoring::{RepositoryResult, WorkerMetricsRepository, WorkerSnapshot},
};
use v2board_config::{RedisKeyspace, app_now, app_timezone};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ConfiguredStatisticsCalendar;

impl StatisticsCalendar for ConfiguredStatisticsCalendar {
    fn boundaries(&self) -> StatisticsBoundaries {
        let now = app_now();
        let timezone = app_timezone();
        let today = timezone
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        let month = timezone
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        let (previous_year, previous_month) = if now.month() == 1 {
            (now.year() - 1, 12)
        } else {
            (now.year(), now.month() - 1)
        };
        let previous_month = timezone
            .with_ymd_and_hms(previous_year, previous_month, 1, 0, 0, 0)
            .single()
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        StatisticsBoundaries {
            now: now.timestamp(),
            today,
            yesterday: today.saturating_sub(86_400),
            month,
            previous_month,
        }
    }

    fn month_day(&self, timestamp: i64) -> String {
        app_timezone()
            .timestamp_opt(timestamp, 0)
            .single()
            .map(|value| value.format("%m-%d").to_string())
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub(crate) struct RedisWorkerMetrics {
    client: redis::Client,
    keyspace: RedisKeyspace,
}

impl RedisWorkerMetrics {
    pub(crate) const fn new(client: redis::Client, keyspace: RedisKeyspace) -> Self {
        Self { client, keyspace }
    }
}

impl WorkerMetricsRepository for RedisWorkerMetrics {
    async fn snapshot(&self) -> RepositoryResult<WorkerSnapshot> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| RepositoryError::new("connect worker metrics", error))?;
        let schedule_last_seen_at = connection
            .get(self.keyspace.key("SCHEDULE_LAST_CHECK_AT_"))
            .await
            .map_err(|error| RepositoryError::new("read scheduler heartbeat", error))?;
        let totals = connection
            .hgetall(self.keyspace.key("RUST_WORKER_JOBS_TOTAL"))
            .await
            .map_err(|error| RepositoryError::new("read worker totals", error))?;
        let failed = connection
            .hgetall(self.keyspace.key("RUST_WORKER_JOBS_FAILED"))
            .await
            .map_err(|error| RepositoryError::new("read worker failures", error))?;
        let last_run_at = connection
            .hgetall(self.keyspace.key("RUST_WORKER_LAST_RUN_AT"))
            .await
            .map_err(|error| RepositoryError::new("read worker last run", error))?;
        let last_success_at = connection
            .hgetall(self.keyspace.key("RUST_WORKER_LAST_SUCCESS_AT"))
            .await
            .map_err(|error| RepositoryError::new("read worker last success", error))?;
        let last_failure_at = connection
            .hgetall(self.keyspace.key("RUST_WORKER_LAST_FAILURE_AT"))
            .await
            .map_err(|error| RepositoryError::new("read worker last failure", error))?;
        Ok(WorkerSnapshot {
            schedule_last_seen_at,
            totals,
            failed,
            last_run_at,
            last_success_at,
            last_failure_at,
        })
    }
}
