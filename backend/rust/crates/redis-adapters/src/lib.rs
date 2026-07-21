use thiserror::Error;
use v2board_config::RuntimeEnvironment;

#[derive(Debug, Error)]
pub enum RedisRuntimeError {
    #[error("Redis runtime probe failed: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Redis returned an unexpected PING response")]
    UnexpectedPing,
    #[error("production Redis maxmemory-policy must be noeviction, found {0:?}")]
    EvictionPolicy(Option<String>),
}

/// Fixed-window flood reservation: INCR the caller's fully-qualified window
/// key and report whether the count stayed within `limit`. `EXPIRE NX` pins
/// the TTL to the window's first write, so the key self-expires and holds no
/// durable state. Callers own key naming and the denial error shape.
pub async fn reserve_fixed_window_slot(
    connection: &mut redis::aio::ConnectionManager,
    key: &str,
    limit: i64,
    window_seconds: i64,
) -> Result<bool, redis::RedisError> {
    let (count, _ttl_was_set): (i64, i64) = redis::pipe()
        .atomic()
        .cmd("INCR")
        .arg(key)
        .cmd("EXPIRE")
        .arg(key)
        .arg(window_seconds)
        .arg("NX")
        .query_async(connection)
        .await?;
    Ok(count <= limit)
}

/// Verify the disposable Redis runtime without treating it as a durable store.
///
/// Production deliberately checks the `maxmemory_policy` field from `INFO
/// memory`. A lease or session key silently evicted under memory pressure
/// changes security and scheduler behavior, so an unverifiable policy is a
/// startup failure rather than an optimistic assumption. Runtime principals do
/// not need access to the broader `CONFIG GET` surface.
pub async fn verify_redis_runtime(
    client: &redis::Client,
    environment: RuntimeEnvironment,
) -> Result<(), RedisRuntimeError> {
    let mut connection = client.get_multiplexed_async_connection().await?;
    let pong: String = redis::cmd("PING").query_async(&mut connection).await?;
    if pong != "PONG" {
        return Err(RedisRuntimeError::UnexpectedPing);
    }
    if !environment.is_production() {
        return Ok(());
    }

    let memory: String = redis::cmd("INFO")
        .arg("memory")
        .query_async(&mut connection)
        .await?;
    require_noeviction(&memory)
}

fn require_noeviction(memory: &str) -> Result<(), RedisRuntimeError> {
    let policy = memory
        .lines()
        .find_map(|line| {
            let (key, value) = line.trim_end_matches('\r').split_once(':')?;
            (key == "maxmemory_policy").then_some(value)
        })
        .map(|value| value.trim().to_ascii_lowercase());
    if policy.as_deref() != Some("noeviction") {
        return Err(RedisRuntimeError::EvictionPolicy(policy));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_policy_error_does_not_accept_missing_or_approximate_values() {
        assert!(require_noeviction("").is_err());
        assert!(
            require_noeviction("# Memory\r\nused_memory:123\r\nmaxmemory_policy:allkeys-lru\r\n")
                .is_err()
        );
        assert!(
            require_noeviction("# Memory\r\nused_memory:123\r\nmaxmemory_policy: NOEVICTION \r\n")
                .is_ok()
        );
        assert!(require_noeviction("maxmemory-policy:noeviction\r\n").is_err());
    }
}
