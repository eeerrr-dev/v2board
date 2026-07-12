use chrono::Utc;
use redis::AsyncCommands;

use crate::state::WorkerState;

pub(crate) async fn mark_scheduler_alive(state: &WorkerState) -> anyhow::Result<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: () = conn
        .set("SCHEDULE_LAST_CHECK_AT_", Utc::now().timestamp())
        .await?;
    Ok(())
}

pub(crate) async fn record_worker_metric(
    state: &WorkerState,
    name: &str,
    success: bool,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: () = conn.hincr("RUST_WORKER_JOBS_TOTAL", name, 1).await?;
    let _: () = conn.hset("RUST_WORKER_LAST_RUN_AT", name, now).await?;
    if success {
        let _: () = conn.hset("RUST_WORKER_LAST_SUCCESS_AT", name, now).await?;
    } else {
        let _: () = conn.hincr("RUST_WORKER_JOBS_FAILED", name, 1).await?;
        let _: () = conn.hset("RUST_WORKER_LAST_FAILURE_AT", name, now).await?;
    }
    Ok(())
}
