use std::time::Duration;

use chrono::Utc;
use redis::AsyncCommands;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_metrics_use_a_bounded_redis_deadline() {
        assert!(METRICS_REDIS_TIMEOUT <= Duration::from_secs(5));
        assert!(!METRICS_REDIS_TIMEOUT.is_zero());
    }
}
