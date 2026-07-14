use std::time::Duration;

use chrono::Utc;
use redis::AsyncCommands;
use v2board_analytics::AnalyticsAdmissionSnapshot;

use crate::state::WorkerState;

const METRICS_REDIS_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) async fn mark_scheduler_alive(state: &WorkerState) -> anyhow::Result<()> {
    let mut conn = tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        state.redis.get_multiplexed_async_connection(),
    )
    .await??;
    tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        conn.set::<_, _, ()>(
            state.redis_key("SCHEDULE_LAST_CHECK_AT_"),
            Utc::now().timestamp(),
        ),
    )
    .await??;
    Ok(())
}

pub(crate) async fn record_worker_loop_heartbeat(
    state: &WorkerState,
    name: &str,
) -> anyhow::Result<()> {
    let mut conn = tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        state.redis.get_multiplexed_async_connection(),
    )
    .await??;
    tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        conn.hset::<_, _, _, ()>(
            state.redis_key("RUST_WORKER_LOOP_HEARTBEAT_AT"),
            name,
            Utc::now().timestamp(),
        ),
    )
    .await??;
    Ok(())
}

pub(crate) async fn record_worker_metric(
    state: &WorkerState,
    name: &str,
    success: bool,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let mut conn = tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        state.redis.get_multiplexed_async_connection(),
    )
    .await??;
    let mut pipeline = redis::pipe();
    let jobs_total = state.redis_key("RUST_WORKER_JOBS_TOTAL");
    let last_run = state.redis_key("RUST_WORKER_LAST_RUN_AT");
    let last_success = state.redis_key("RUST_WORKER_LAST_SUCCESS_AT");
    let jobs_failed = state.redis_key("RUST_WORKER_JOBS_FAILED");
    let last_failure = state.redis_key("RUST_WORKER_LAST_FAILURE_AT");
    pipeline
        .hincr(&jobs_total, name, 1)
        .ignore()
        .hset(&last_run, name, now)
        .ignore();
    if success {
        pipeline.hset(&last_success, name, now).ignore();
    } else {
        pipeline
            .hincr(&jobs_failed, name, 1)
            .ignore()
            .hset(&last_failure, name, now)
            .ignore();
    }
    tokio::time::timeout(METRICS_REDIS_TIMEOUT, pipeline.query_async::<()>(&mut conn)).await??;
    Ok(())
}

pub(crate) async fn record_analytics_admission_metrics(
    state: &WorkerState,
    snapshot: &AnalyticsAdmissionSnapshot,
) -> anyhow::Result<()> {
    let mut conn = tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        state.redis.get_multiplexed_async_connection(),
    )
    .await??;
    let oldest_age = snapshot
        .oldest_pending_age_seconds
        .map_or_else(|| "none".to_owned(), |value| value.to_string());
    let analytics_key = state.redis_key("RUST_ANALYTICS_ADMISSION");
    let mut pipeline = redis::pipe();
    pipeline
        .hset(&analytics_key, "observed", "true")
        .ignore()
        .hset(&analytics_key, "observed_at", Utc::now().timestamp())
        .ignore()
        .hdel(&analytics_key, "observation_error")
        .ignore()
        .hset(
            &analytics_key,
            "installation_id",
            snapshot.installation_id.to_string(),
        )
        .ignore()
        .hset(&analytics_key, "policy_sha256", &snapshot.policy_sha256)
        .ignore()
        .hset(
            &analytics_key,
            "pressure_state",
            snapshot.pressure_state.as_str(),
        )
        .ignore()
        .hset(
            &analytics_key,
            "sample_fresh",
            snapshot.sample_fresh.to_string(),
        )
        .ignore()
        .hset(&analytics_key, "sampled_at", snapshot.sampled_at)
        .ignore()
        .hset(&analytics_key, "generation", snapshot.generation)
        .ignore()
        .hset(&analytics_key, "pending_rows", snapshot.pending_rows)
        .ignore()
        .hset(
            &analytics_key,
            "accounted_pending_rows",
            snapshot.accounted_pending_rows,
        )
        .ignore()
        .hset(&analytics_key, "oldest_pending_age_seconds", oldest_age)
        .ignore()
        .hset(
            &analytics_key,
            "relation_heap_bytes",
            snapshot.relation_heap_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "relation_index_bytes",
            snapshot.relation_index_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "relation_toast_bytes",
            snapshot.relation_toast_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "relation_total_bytes",
            snapshot.relation_total_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "accounted_relation_bytes",
            snapshot.accounted_relation_bytes,
        )
        .ignore()
        .hset(&analytics_key, "database_bytes", snapshot.database_bytes)
        .ignore()
        .hset(
            &analytics_key,
            "capacity_headroom_bytes",
            snapshot.capacity_headroom_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "database_capacity_bytes",
            snapshot.database_capacity_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "soft_pending_rows",
            snapshot.soft_pending_rows,
        )
        .ignore()
        .hset(
            &analytics_key,
            "hard_pending_rows",
            snapshot.hard_pending_rows,
        )
        .ignore()
        .hset(
            &analytics_key,
            "soft_relation_bytes",
            snapshot.soft_relation_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "hard_relation_bytes",
            snapshot.hard_relation_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "soft_oldest_age_seconds",
            snapshot.soft_oldest_age_seconds,
        )
        .ignore()
        .hset(
            &analytics_key,
            "hard_oldest_age_seconds",
            snapshot.hard_oldest_age_seconds,
        )
        .ignore()
        .hset(
            &analytics_key,
            "hard_min_headroom_bytes",
            snapshot.hard_min_headroom_bytes,
        )
        .ignore()
        .hset(
            &analytics_key,
            "soft_max_new_rows_per_second",
            snapshot.soft_max_new_rows_per_second,
        )
        .ignore()
        .hset(
            &analytics_key,
            "sample_interval_seconds",
            snapshot.sample_interval_seconds,
        )
        .ignore()
        .hset(
            &analytics_key,
            "stale_after_seconds",
            snapshot.stale_after_seconds,
        )
        .ignore()
        .hset(
            &analytics_key,
            "last_transition_reason",
            &snapshot.last_transition_reason,
        )
        .ignore();
    tokio::time::timeout(METRICS_REDIS_TIMEOUT, pipeline.query_async::<()>(&mut conn)).await??;
    Ok(())
}

pub(crate) async fn record_analytics_admission_failure(
    state: &WorkerState,
    reason: &str,
) -> anyhow::Result<()> {
    let mut conn = tokio::time::timeout(
        METRICS_REDIS_TIMEOUT,
        state.redis.get_multiplexed_async_connection(),
    )
    .await??;
    let analytics_key = state.redis_key("RUST_ANALYTICS_ADMISSION");
    let mut pipeline = redis::pipe();
    pipeline
        .hset(&analytics_key, "observed", "false")
        .ignore()
        .hset(&analytics_key, "observed_at", Utc::now().timestamp())
        .ignore()
        .hset(&analytics_key, "observation_error", reason)
        .ignore();
    tokio::time::timeout(METRICS_REDIS_TIMEOUT, pipeline.query_async::<()>(&mut conn)).await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_metrics_use_a_bounded_redis_deadline() {
        assert!(METRICS_REDIS_TIMEOUT <= Duration::from_secs(5));
        assert!(!METRICS_REDIS_TIMEOUT.is_zero());
    }
}
