//! Method-aware subscribe-link minting shared by every surface that hands out
//! a subscribe URL (`Helper::getSubscribeUrl`, Helper.php:100-143): the user
//! panel and knowledge body, the Surge/Surfboard `$subs_link` substitution,
//! reset-security, and the admin user listing/CSV exports
//! (Admin/UserController.php:103,197,275). Routing every mint site through
//! here keeps `show_subscribe_method` 1/2 from leaking the permanent token in
//! a URL that the subscribe resolution would then reject.

use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use redis::aio::ConnectionLike;
use sha1::Sha1;
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::{AppConfig, RedisKeyspace, duration_minutes_to_seconds};

/// Method 1 mint script: `Cache::add("otp_{token}")` semantics. A fresh
/// 24-byte url-safe token is stored together with the reverse
/// `otpn_{newtoken}` mapping the subscribe resolution consumes. The
/// SET-if-absent check inside one script mirrors `Cache::add`, so a concurrent
/// generator that loses the race reuses the winner's token; either way the
/// mint costs exactly one Redis round-trip. The consume side lives in
/// `api::user::subscription::CONSUME_SUBSCRIBE_TOKEN_SCRIPT`.
const MINT_SUBSCRIBE_TOKEN_SCRIPT: &str = r#"
local existing = redis.call('GET', KEYS[1])
if existing and existing ~= '' then
    return existing
end
redis.call('SET', KEYS[1], ARGV[3], 'EX', ARGV[4])
redis.call('SET', ARGV[1] .. ARGV[3], ARGV[2], 'EX', ARGV[4])
return ARGV[3]
"#;

/// Mirror `Helper::getSubscribeUrl`: derive the method-specific token and
/// render the full subscribe URL for it.
pub async fn subscribe_url_for_user<C>(
    config: &AppConfig,
    redis_keys: &RedisKeyspace,
    conn: &mut Option<C>,
    user_id: i64,
    token: &str,
) -> Result<String, ApiError>
where
    C: ConnectionLike + Send,
{
    let method_token = method_subscribe_token(config, redis_keys, conn, user_id, token).await?;
    Ok(config.subscribe_url_for_token(&method_token))
}

/// Derives the `show_subscribe_method`-specific token so the generated URL
/// resolves back through the client subscribe resolution. Method 0 keeps the
/// raw token byte-identically and never touches Redis; method 1 mints/reuses
/// the cached `otp_` one-time token (one Redis round-trip per call); method 2
/// derives the time-stepped `{id}:{hmac}` token purely.
pub async fn method_subscribe_token<C>(
    config: &AppConfig,
    redis_keys: &RedisKeyspace,
    conn: &mut Option<C>,
    user_id: i64,
    token: &str,
) -> Result<String, ApiError>
where
    C: ConnectionLike + Send,
{
    match config.show_subscribe_method {
        1 => {
            let conn = conn.as_mut().ok_or_else(|| {
                ApiError::internal("one-time subscribe-token minting requires a Redis connection")
            })?;
            one_time_subscribe_token(redis_keys, conn, token).await
        }
        2 => totp_subscribe_token(config, user_id, token),
        _ => Ok(token.to_string()),
    }
}

async fn one_time_subscribe_token<C>(
    redis_keys: &RedisKeyspace,
    conn: &mut C,
    token: &str,
) -> Result<String, ApiError>
where
    C: ConnectionLike + Send,
{
    let mut raw = [0_u8; 24];
    raw[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    raw[16..].copy_from_slice(&Uuid::new_v4().as_bytes()[..8]);
    let new_token = base64_encode_url_safe(&raw);
    Ok(redis::Script::new(MINT_SUBSCRIBE_TOKEN_SCRIPT)
        .key(redis_keys.key(&format!("otp_{token}")))
        .arg(redis_keys.key("otpn_"))
        .arg(token)
        .arg(&new_token)
        .arg(86_400)
        .invoke_async(conn)
        .await?)
}

/// Method 2 token: `base64url("{user_id}:{hmac_sha1(counterBytes, token)}")`,
/// derived purely so it stays in lock-step with the resolution side for the
/// same time window.
pub fn totp_subscribe_token(
    config: &AppConfig,
    user_id: i64,
    token: &str,
) -> Result<String, ApiError> {
    let hash = hmac_sha1_hex(token.as_bytes(), &totp_counter_bytes(config))?;
    Ok(base64_encode_url_safe(
        format!("{user_id}:{hash}").as_bytes(),
    ))
}

/// The method-2 HOTP-style counter for the current `show_subscribe_expire`
/// window, packed exactly like Laravel's `pack('N*', 0) . pack('N*', counter)`.
pub fn totp_counter_bytes(config: &AppConfig) -> [u8; 8] {
    let timestep = duration_minutes_to_seconds(config.show_subscribe_expire);
    let counter = Utc::now().timestamp().max(0) as u64 / timestep;
    let mut counter_bytes = [0_u8; 8];
    counter_bytes[4..].copy_from_slice(&(counter as u32).to_be_bytes());
    counter_bytes
}

pub fn hmac_sha1_hex(key: &[u8], message: &[u8]) -> Result<String, ApiError> {
    type HmacSha1 = Hmac<Sha1>;
    let mut mac =
        HmacSha1::new_from_slice(key).map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(message);
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn base64_encode_url_safe(bytes: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::VecDeque;

    /// A scripted Redis connection: records every packed command and replays
    /// queued replies, so mint paths can be exercised without a server.
    pub(crate) struct MockRedis {
        pub(crate) commands: Vec<Vec<Vec<u8>>>,
        pub(crate) replies: VecDeque<redis::Value>,
    }

    impl MockRedis {
        pub(crate) fn new(replies: impl IntoIterator<Item = redis::Value>) -> Self {
            Self {
                commands: Vec::new(),
                replies: replies.into_iter().collect(),
            }
        }

        pub(crate) fn command_args(&self, index: usize) -> Vec<String> {
            self.commands[index]
                .iter()
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect()
        }
    }

    impl redis::aio::ConnectionLike for MockRedis {
        fn req_packed_command<'a>(
            &'a mut self,
            cmd: &'a redis::Cmd,
        ) -> redis::RedisFuture<'a, redis::Value> {
            self.commands.push(
                cmd.args_iter()
                    .map(|arg| match arg {
                        redis::Arg::Simple(bytes) => bytes.to_vec(),
                        _ => b"<cursor>".to_vec(),
                    })
                    .collect(),
            );
            let reply = self.replies.pop_front().unwrap_or(redis::Value::Nil);
            Box::pin(async move { Ok(reply) })
        }

        fn req_packed_commands<'a>(
            &'a mut self,
            _cmd: &'a redis::Pipeline,
            _offset: usize,
            _count: usize,
        ) -> redis::RedisFuture<'a, Vec<redis::Value>> {
            Box::pin(async move { Ok(Vec::new()) })
        }

        fn get_db(&self) -> i64 {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::MockRedis;
    use super::*;
    use serde_json::Map;
    use std::path::PathBuf;
    use v2board_config::RuntimePaths;

    fn config_with_method(method: i32) -> AppConfig {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let mut config =
            AppConfig::try_from_api_config_map(Map::new(), paths).expect("subscribe test config");
        // Pin the fields the assertions depend on; the container environment
        // may inject its own app_url/subscribe_url defaults.
        config.app_url = Some("https://panel.example".to_string());
        config.subscribe_url = Some("https://sub.example".to_string());
        config.show_subscribe_method = method;
        config
    }

    fn keyspace() -> RedisKeyspace {
        RedisKeyspace::new(uuid::Uuid::nil())
    }

    #[tokio::test]
    async fn method_zero_keeps_the_raw_token_and_never_touches_redis() {
        let config = config_with_method(0);
        // No connection at all proves method 0 cannot issue Redis commands.
        let mut conn: Option<MockRedis> = None;
        let url = subscribe_url_for_user(&config, &keyspace(), &mut conn, 7, "raw-token")
            .await
            .expect("method 0 url");
        assert_eq!(url, config.subscribe_url_for_token("raw-token"));
        assert!(url.contains("token=raw-token"));
    }

    #[tokio::test]
    async fn method_one_mints_or_reuses_the_cached_otp_token_in_one_round_trip() {
        let config = config_with_method(1);
        let mut conn = Some(MockRedis::new([redis::Value::BulkString(
            b"cached-otp-token".to_vec(),
        )]));
        let url = subscribe_url_for_user(&config, &keyspace(), &mut conn, 7, "raw-token")
            .await
            .expect("method 1 url");
        assert_eq!(url, config.subscribe_url_for_token("cached-otp-token"));
        assert!(!url.contains("raw-token"));

        let conn = conn.expect("mock connection");
        // Exactly one Redis round-trip: the EVALSHA of the mint script.
        assert_eq!(conn.commands.len(), 1);
        let args = conn.command_args(0);
        assert_eq!(args[0], "EVALSHA");
        let keyspace = keyspace();
        assert!(args.contains(&keyspace.key("otp_raw-token")));
        assert!(args.contains(&keyspace.key("otpn_")));
        assert!(args.contains(&"raw-token".to_string()));
        assert!(args.contains(&"86400".to_string()));
    }

    #[tokio::test]
    async fn method_one_without_a_connection_is_an_internal_error() {
        let config = config_with_method(1);
        let mut conn: Option<MockRedis> = None;
        let error = subscribe_url_for_user(&config, &keyspace(), &mut conn, 7, "raw-token")
            .await
            .expect_err("method 1 requires Redis");
        assert!(matches!(error, ApiError::Internal(_)));
    }

    #[tokio::test]
    async fn method_two_derives_the_time_stepped_id_hmac_token() {
        let config = config_with_method(2);
        let mut conn: Option<MockRedis> = None;
        // Retry across an (unlikely) timestep boundary so the expectation and
        // the minted token always share one counter window.
        loop {
            let before = totp_counter_bytes(&config);
            let url = subscribe_url_for_user(&config, &keyspace(), &mut conn, 42, "raw-token")
                .await
                .expect("method 2 url");
            let after = totp_counter_bytes(&config);
            if before != after {
                continue;
            }
            let hash = hmac_sha1_hex(b"raw-token", &before).expect("hmac");
            let expected = base64_encode_url_safe(format!("42:{hash}").as_bytes());
            assert_eq!(url, config.subscribe_url_for_token(&expected));
            assert!(!url.contains("raw-token"));
            break;
        }
    }

    #[test]
    fn totp_counter_is_packed_like_the_laravel_pair_of_n_words() {
        let config = config_with_method(2);
        let bytes = totp_counter_bytes(&config);
        assert_eq!(&bytes[..4], &[0, 0, 0, 0]);
    }
}
