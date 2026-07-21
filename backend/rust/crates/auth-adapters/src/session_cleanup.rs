fn user_sessions_key(user_id: i64) -> String {
    format!("USER_SESSIONS_{user_id}")
}

fn user_auth_keys_key(user_id: i64) -> String {
    format!("AUTH_USER_SESSION_KEYS_{user_id}")
}

const REMOVE_ALL_SESSIONS_SCRIPT: &str = r#"
local auth_keys = redis.call('SMEMBERS', KEYS[2])
for _, auth_key in ipairs(auth_keys) do
    redis.call('DEL', auth_key)
end
redis.call('DEL', KEYS[1])
redis.call('DEL', KEYS[2])
return #auth_keys
"#;

/// Best-effort cache cleanup for legacy admin/staff revocation paths. The database session epoch
/// remains authoritative; this immediately removes hashed opaque-token reverse mappings and the
/// user-visible session metadata.
pub async fn remove_user_sessions_from_client(
    redis: &redis::Client,
    redis_keys: &v2board_config::RedisKeyspace,
    user_id: i64,
) -> Result<(), redis::RedisError> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    redis::Script::new(REMOVE_ALL_SESSIONS_SCRIPT)
        .key(redis_keys.key(&user_sessions_key(user_id)))
        .key(redis_keys.key(&user_auth_keys_key(user_id)))
        .invoke_async::<i64>(&mut conn)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::REMOVE_ALL_SESSIONS_SCRIPT;

    #[test]
    fn cleanup_removes_reverse_mappings_before_user_indexes() {
        assert!(REMOVE_ALL_SESSIONS_SCRIPT.contains("SMEMBERS"));
        assert!(REMOVE_ALL_SESSIONS_SCRIPT.contains("redis.call('DEL', auth_key)"));
        assert!(REMOVE_ALL_SESSIONS_SCRIPT.contains("redis.call('DEL', KEYS[1])"));
        assert!(REMOVE_ALL_SESSIONS_SCRIPT.contains("redis.call('DEL', KEYS[2])"));
    }
}
