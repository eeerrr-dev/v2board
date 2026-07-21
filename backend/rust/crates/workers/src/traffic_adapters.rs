use std::time::{Duration, Instant};

use chrono::Utc;
use redis::AsyncCommands;
use uuid::Uuid;
use v2board_application::{
    RepositoryError,
    worker_traffic::{RepositoryResult, TrafficResetBarrier, WorkerClock},
};

use crate::{
    lease::{SCHEDULER_LOCK_TTL_SECS, SchedulerLock},
    state::WorkerState,
};

pub(crate) const TRAFFIC_RESET_LOCK_KEY: &str = "traffic_reset_lock";
const TRAFFIC_UPDATE_SCHEDULER_LOCK_KEY: &str = "RUST_SCHEDULER_LOCK_traffic_update";
const RESET_LOCK_IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Redis-backed availability barrier used while the durable PostgreSQL quota
/// epoch reset is running. Keeping the Lua integration here leaves the reset
/// job itself as an inbound adapter over the maintenance application use case.
pub(crate) async fn acquire_traffic_reset_lock(
    state: &WorkerState,
) -> anyhow::Result<SchedulerLock> {
    let token = Uuid::new_v4().to_string();
    let reset_lock_key = state.redis_key(TRAFFIC_RESET_LOCK_KEY);
    let traffic_update_lock_key = state.redis_key(TRAFFIC_UPDATE_SCHEDULER_LOCK_KEY);
    loop {
        let acquired: i64 = tokio::time::timeout(RESET_LOCK_IO_TIMEOUT, async {
            let mut connection = state.redis.get_multiplexed_async_connection().await?;
            // This is an availability barrier; quota_epoch is the durable
            // correctness fence. Redis executes the two-key admission check
            // atomically so ordinary runs avoid producing stale reports.
            redis::Script::new(
                r#"
                if redis.call("EXISTS", KEYS[1]) == 1
                    or redis.call("EXISTS", KEYS[2]) == 1 then
                    return 0
                end
                local result = redis.call("SET", KEYS[1], ARGV[1], "NX", "EX", ARGV[2])
                if result then return 1 end
                return 0
                "#,
            )
            .key(&reset_lock_key)
            .key(&traffic_update_lock_key)
            .arg(&token)
            .arg(SCHEDULER_LOCK_TTL_SECS)
            .invoke_async(&mut connection)
            .await
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out acquiring traffic reset barrier"))??;
        if acquired == 1 {
            return Ok(SchedulerLock {
                key: reset_lock_key,
                token,
            });
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Clone)]
pub(crate) struct RedisTrafficResetBarrier {
    redis: redis::Client,
    key: String,
}

impl RedisTrafficResetBarrier {
    pub(crate) fn new(redis: redis::Client, key: String) -> Self {
        Self { redis, key }
    }
}

impl TrafficResetBarrier for RedisTrafficResetBarrier {
    async fn reset_in_progress(&self) -> RepositoryResult<bool> {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut connection = self.redis.get_multiplexed_async_connection().await?;
            connection.exists::<_, bool>(&self.key).await
        })
        .await
        .map_err(|error| RepositoryError::new("check traffic reset barrier timeout", error))?
        .map_err(|error| RepositoryError::new("check traffic reset barrier", error))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SystemWorkerClock {
    origin: Instant,
}

impl Default for SystemWorkerClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl WorkerClock for SystemWorkerClock {
    fn unix_timestamp(&self) -> i64 {
        Utc::now().timestamp()
    }

    fn monotonic_millis(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}
