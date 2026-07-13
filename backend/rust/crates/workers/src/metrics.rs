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
        conn.set::<_, _, ()>("SCHEDULE_LAST_CHECK_AT_", Utc::now().timestamp()),
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
            "RUST_WORKER_LOOP_HEARTBEAT_AT",
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
    pipeline
        .hincr("RUST_WORKER_JOBS_TOTAL", name, 1)
        .ignore()
        .hset("RUST_WORKER_LAST_RUN_AT", name, now)
        .ignore();
    if success {
        pipeline
            .hset("RUST_WORKER_LAST_SUCCESS_AT", name, now)
            .ignore();
    } else {
        pipeline
            .hincr("RUST_WORKER_JOBS_FAILED", name, 1)
            .ignore()
            .hset("RUST_WORKER_LAST_FAILURE_AT", name, now)
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
    let mut pipeline = redis::pipe();
    pipeline
        .hset("RUST_ANALYTICS_ADMISSION", "observed", "true")
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "observed_at",
            Utc::now().timestamp(),
        )
        .ignore()
        .hdel("RUST_ANALYTICS_ADMISSION", "observation_error")
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "installation_id",
            snapshot.installation_id.to_string(),
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "policy_sha256",
            &snapshot.policy_sha256,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "pressure_state",
            snapshot.pressure_state.as_str(),
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "sample_fresh",
            snapshot.sample_fresh.to_string(),
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "sampled_at",
            snapshot.sampled_at,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "generation",
            snapshot.generation,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "pending_rows",
            snapshot.pending_rows,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "accounted_pending_rows",
            snapshot.accounted_pending_rows,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "oldest_pending_age_seconds",
            oldest_age,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "relation_heap_bytes",
            snapshot.relation_heap_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "relation_index_bytes",
            snapshot.relation_index_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "relation_toast_bytes",
            snapshot.relation_toast_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "relation_total_bytes",
            snapshot.relation_total_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "accounted_relation_bytes",
            snapshot.accounted_relation_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "database_bytes",
            snapshot.database_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "capacity_headroom_bytes",
            snapshot.capacity_headroom_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "database_capacity_bytes",
            snapshot.database_capacity_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "soft_pending_rows",
            snapshot.soft_pending_rows,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "hard_pending_rows",
            snapshot.hard_pending_rows,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "soft_relation_bytes",
            snapshot.soft_relation_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "hard_relation_bytes",
            snapshot.hard_relation_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "soft_oldest_age_seconds",
            snapshot.soft_oldest_age_seconds,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "hard_oldest_age_seconds",
            snapshot.hard_oldest_age_seconds,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "hard_min_headroom_bytes",
            snapshot.hard_min_headroom_bytes,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "soft_max_new_rows_per_second",
            snapshot.soft_max_new_rows_per_second,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "sample_interval_seconds",
            snapshot.sample_interval_seconds,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "stale_after_seconds",
            snapshot.stale_after_seconds,
        )
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
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
    let mut pipeline = redis::pipe();
    pipeline
        .hset("RUST_ANALYTICS_ADMISSION", "observed", "false")
        .ignore()
        .hset(
            "RUST_ANALYTICS_ADMISSION",
            "observed_at",
            Utc::now().timestamp(),
        )
        .ignore()
        .hset("RUST_ANALYTICS_ADMISSION", "observation_error", reason)
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
