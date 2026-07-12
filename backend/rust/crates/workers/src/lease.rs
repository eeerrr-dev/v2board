use std::time::Duration;

use uuid::Uuid;

use crate::state::WorkerState;

pub(crate) const SCHEDULER_LOCK_TTL_SECS: u64 = 900;
const SCHEDULER_LOCK_RENEW_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub(crate) struct SchedulerLock {
    pub(crate) key: String,
    pub(crate) token: String,
}

pub(crate) async fn acquire_scheduler_lock(
    state: &WorkerState,
    name: &str,
) -> anyhow::Result<Option<SchedulerLock>> {
    let key = format!("RUST_SCHEDULER_LOCK_{name}");
    let token = Uuid::new_v4().to_string();
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("NX")
        .arg("EX")
        .arg(SCHEDULER_LOCK_TTL_SECS)
        .query_async(&mut conn)
        .await?;
    Ok(acquired.map(|_| SchedulerLock { key, token }))
}

async fn renew_scheduler_lock(
    state: &WorkerState,
    scheduler_lock: &SchedulerLock,
) -> anyhow::Result<bool> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let renewed: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("EXPIRE", KEYS[1], ARGV[2])
        end
        return 0
        "#,
    )
    .key(&scheduler_lock.key)
    .arg(&scheduler_lock.token)
    .arg(SCHEDULER_LOCK_TTL_SECS)
    .invoke_async(&mut conn)
    .await?;
    Ok(renewed == 1)
}

pub(crate) async fn release_scheduler_lock(
    state: &WorkerState,
    scheduler_lock: SchedulerLock,
) -> anyhow::Result<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        end
        return 0
        "#,
    )
    .key(scheduler_lock.key)
    .arg(scheduler_lock.token)
    .invoke_async(&mut conn)
    .await?;
    Ok(())
}

pub(crate) async fn run_task_with_lease(
    mut task_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
    state: &WorkerState,
    scheduler_lock: &SchedulerLock,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            result = &mut task_handle => {
                return result
                    .map_err(|error| anyhow::anyhow!("scheduled job panicked: {error}"))?;
            }
            _ = tokio::time::sleep(Duration::from_secs(SCHEDULER_LOCK_RENEW_SECS)) => {
                match renew_scheduler_lock(state, scheduler_lock).await {
                    Ok(true) => {}
                    Ok(false) => {
                        task_handle.abort();
                        let _ = task_handle.await;
                        anyhow::bail!("scheduled job lost its distributed lease");
                    }
                    Err(error) => {
                        task_handle.abort();
                        let _ = task_handle.await;
                        return Err(anyhow::anyhow!(
                            "failed to renew scheduled job lease: {error}"
                        ));
                    }
                }
            }
        }
    }
}
