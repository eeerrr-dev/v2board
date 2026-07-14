use std::{future::Future, time::Duration};

use uuid::Uuid;

use crate::state::WorkerState;

pub(crate) const SCHEDULER_LOCK_TTL_SECS: u64 = 30;
const SCHEDULER_LOCK_RENEW_SECS: u64 = 2;
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
    let key = state.redis_key(&format!("RUST_SCHEDULER_LOCK_{name}"));
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
    run_with_lease_renewal(task, Duration::from_secs(SCHEDULER_LOCK_RENEW_SECS), || {
        renew_scheduler_lock(state, scheduler_lock)
    })
    .await
}

async fn run_with_lease_renewal<F, R, RFut>(
    task: F,
    renew_interval: Duration,
    mut renew: R,
) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
    R: FnMut() -> RFut,
    RFut: Future<Output = anyhow::Result<bool>>,
{
    tokio::pin!(task);
    let mut renewal = tokio::time::interval(renew_interval);
    renewal.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // `interval` ticks immediately. The acquisition itself established
    // ownership, so wait one full interval before the first renewal.
    renewal.tick().await;
    loop {
        tokio::select! {
            result = &mut task => return result,
            _ = renewal.tick() => {
                match renew().await {
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    struct DropSignal(Arc<AtomicBool>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[tokio::test]
    async fn losing_lease_cancels_the_in_flight_job_future() {
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = dropped.clone();
        let task = async move {
            let _drop_signal = DropSignal(task_dropped);
            std::future::pending::<()>().await;
            Ok(())
        };
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_with_lease_renewal(task, Duration::from_millis(1), || async { Ok(false) }),
        )
        .await
        .expect("lease-loss monitor must finish");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("lost its distributed lease")
        );
        assert!(dropped.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn renewal_error_cancels_the_in_flight_job_future() {
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_with_lease_renewal(
                std::future::pending::<anyhow::Result<()>>(),
                Duration::from_millis(1),
                || async { anyhow::bail!("redis unavailable") },
            ),
        )
        .await
        .expect("renewal failure monitor must finish");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("failed to renew scheduled job lease")
        );
    }
}
