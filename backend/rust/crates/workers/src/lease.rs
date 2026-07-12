use std::{future::Future, time::Duration};

use uuid::Uuid;

use crate::state::WorkerState;

pub(crate) const SCHEDULER_LOCK_TTL_SECS: u64 = 900;
const SCHEDULER_LOCK_RENEW_SECS: u64 = 300;
const SCHEDULER_LOCK_IO_TIMEOUT: Duration = Duration::from_secs(5);

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
    let acquired: Option<String> = tokio::time::timeout(SCHEDULER_LOCK_IO_TIMEOUT, async {
        let mut conn = state.redis.get_multiplexed_async_connection().await?;
        redis::cmd("SET")
            .arg(&key)
            .arg(&token)
            .arg("NX")
            .arg("EX")
            .arg(SCHEDULER_LOCK_TTL_SECS)
            .query_async(&mut conn)
            .await
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out acquiring scheduler lock `{key}`"))??;
    Ok(acquired.map(|_| SchedulerLock { key, token }))
}

async fn renew_scheduler_lock(
    state: &WorkerState,
    scheduler_lock: &SchedulerLock,
) -> anyhow::Result<bool> {
    let renewed: i64 = tokio::time::timeout(SCHEDULER_LOCK_IO_TIMEOUT, async {
        let mut conn = state.redis.get_multiplexed_async_connection().await?;
        redis::Script::new(
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
        .await
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out renewing scheduler lock `{}`", scheduler_lock.key))??;
    Ok(renewed == 1)
}

pub(crate) async fn release_scheduler_lock(
    state: &WorkerState,
    scheduler_lock: SchedulerLock,
) -> anyhow::Result<()> {
    let key = scheduler_lock.key;
    let token = scheduler_lock.token;
    let _: i64 = tokio::time::timeout(SCHEDULER_LOCK_IO_TIMEOUT, async {
        let mut conn = state.redis.get_multiplexed_async_connection().await?;
        redis::Script::new(
            r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            end
            return 0
            "#,
        )
        .key(&key)
        .arg(token)
        .invoke_async(&mut conn)
        .await
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out releasing scheduler lock `{key}`"))??;
    Ok(())
}

pub(crate) async fn run_with_lease<F>(
    task: F,
    state: &WorkerState,
    scheduler_lock: &SchedulerLock,
) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    tokio::pin!(task);
    loop {
        tokio::select! {
            result = &mut task => return result,
            _ = tokio::time::sleep(Duration::from_secs(SCHEDULER_LOCK_RENEW_SECS)) => {
                match renew_scheduler_lock(state, scheduler_lock).await {
                    Ok(true) => {}
                    Ok(false) => {
                        anyhow::bail!("scheduled job lost its distributed lease");
                    }
                    Err(error) => {
                        return Err(anyhow::anyhow!(
                            "failed to renew scheduled job lease: {error}"
                        ));
                    }
                }
            }
        }
    }
}
