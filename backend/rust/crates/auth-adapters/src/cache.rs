use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use v2board_application::{
    RepositoryError,
    auth::{
        AuthCache, EmailCodeScope, LimitedEmailCodeResult, RegistrationReservation,
        RepositoryResult, SessionIdentity, SessionMetadata, StoredSession,
    },
};
use v2board_config::RedisKeyspace;

const MAX_SESSION_METADATA_BYTES: usize = 256 * 1_024;
const MAX_SESSION_METADATA_ENTRIES: usize = 100;
const AUTH_SESSION_KEY_PREFIX: &str = "AUTH_SESSION_";
const AUTH_STEP_UP_KEY_PREFIX: &str = "AUTH_STEP_UP_";

#[derive(Clone)]
pub struct RedisAuthCache {
    redis: ConnectionManager,
    keys: RedisKeyspace,
}

impl RedisAuthCache {
    pub fn new(redis: ConnectionManager, installation_id: Uuid) -> Self {
        Self {
            redis,
            keys: RedisKeyspace::new(installation_id),
        }
    }

    fn key(&self, logical: &str) -> String {
        self.keys.key(logical)
    }
}

#[derive(Serialize, Deserialize)]
struct RedisSessionIdentity {
    id: i64,
    session: String,
    session_epoch: i64,
}

#[derive(Clone, Serialize, Deserialize)]
struct RedisSessionMetadata {
    ip: Option<String>,
    login_at: i64,
    ua: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
    #[serde(default)]
    password_authenticated: bool,
}

impl RedisSessionMetadata {
    fn from_application(metadata: &SessionMetadata, token_hash: String) -> Self {
        Self {
            ip: metadata.ip.clone(),
            login_at: metadata.login_at,
            ua: metadata.user_agent.clone(),
            token_hash: Some(token_hash),
            expires_at: metadata.expires_at,
            password_authenticated: metadata.password_authenticated,
        }
    }

    fn into_application(self) -> SessionMetadata {
        SessionMetadata {
            ip: self.ip,
            login_at: self.login_at,
            user_agent: self.ua,
            expires_at: self.expires_at,
            password_authenticated: self.password_authenticated,
        }
    }
}

impl From<RedisSessionIdentity> for SessionIdentity {
    fn from(identity: RedisSessionIdentity) -> Self {
        Self {
            user_id: identity.id,
            session_id: identity.session,
            session_epoch: identity.session_epoch,
        }
    }
}

impl From<&SessionIdentity> for RedisSessionIdentity {
    fn from(identity: &SessionIdentity) -> Self {
        Self {
            id: identity.user_id,
            session: identity.session_id.clone(),
            session_epoch: identity.session_epoch,
        }
    }
}

#[allow(async_fn_in_trait)]
impl AuthCache for RedisAuthCache {
    async fn reserve_login_attempt(
        &self,
        email: &str,
        client_ip: Option<&str>,
        account_limit: i64,
        ip_limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool> {
        let keys = login_limiter_keys(email, client_ip).map(|key| self.key(&key));
        let mut redis = self.redis.clone();
        redis::Script::new(RESERVE_LOGIN_ATTEMPT_SCRIPT)
            .key(&keys[0])
            .key(&keys[1])
            .key(&keys[2])
            .arg(account_limit)
            .arg(ip_limit)
            .arg(ttl_seconds)
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|reserved| reserved == 1)
            .map_err(|error| repository_error("reserve login limiter attempt", error))
    }

    async fn release_login_attempt(&self, email: &str, client_ip: Option<&str>) {
        let keys = login_limiter_keys(email, client_ip).map(|key| self.key(&key));
        let mut redis = self.redis.clone();
        if let Err(error) = redis::Script::new(RELEASE_LOGIN_ATTEMPT_SCRIPT)
            .key(&keys[0])
            .key(&keys[1])
            .key(&keys[2])
            .invoke_async::<i64>(&mut redis)
            .await
        {
            tracing::warn!(?error, "login limiter reservation cleanup failed");
        }
    }

    async fn reserve_registration_slot(
        &self,
        reservation: &RegistrationReservation,
        now: i64,
        expires_at: i64,
        limit: i64,
    ) -> RepositoryResult<bool> {
        let mut redis = self.redis.clone();
        redis::Script::new(RESERVE_REGISTRATION_SLOT_SCRIPT)
            .key(self.key(&cache_key(
                "REGISTER_IP_RATE_LIMIT_V2",
                &reservation.client_ip,
            )))
            .arg(now)
            .arg(expires_at)
            .arg(limit)
            .arg(&reservation.token)
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|reserved| reserved == 1)
            .map_err(|error| repository_error("reserve registration limiter slot", error))
    }

    async fn release_registration_slot(&self, reservation: &RegistrationReservation) {
        let key = self.key(&cache_key(
            "REGISTER_IP_RATE_LIMIT_V2",
            &reservation.client_ip,
        ));
        let mut redis = self.redis.clone();
        if let Err(error) = redis::Script::new(RELEASE_REGISTRATION_SLOT_SCRIPT)
            .key(key)
            .arg(&reservation.token)
            .invoke_async::<i64>(&mut redis)
            .await
        {
            tracing::warn!(?error, "failed to release registration limiter slot");
        }
    }

    async fn consume_email_code(
        &self,
        email: &str,
        code: &str,
        scope: EmailCodeScope,
        limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<LimitedEmailCodeResult> {
        let limiter = match scope {
            EmailCodeScope::Registration => "REGISTER_EMAIL_CODE_LIMIT",
            EmailCodeScope::PasswordReset => "FORGET_REQUEST_LIMIT",
        };
        let mut redis = self.redis.clone();
        redis::Script::new(CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT)
            .key(self.key(&cache_key("EMAIL_VERIFY_CODE", email)))
            .key(self.key(&cache_key(limiter, email)))
            .arg(code)
            .arg(limit.max(1))
            .arg(ttl_seconds.max(1))
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|result| match result {
                1 => LimitedEmailCodeResult::Consumed,
                -1 => LimitedEmailCodeResult::Limited,
                _ => LimitedEmailCodeResult::Incorrect,
            })
            .map_err(|error| repository_error("consume verification email code", error))
    }

    async fn increment_email_send_limit(
        &self,
        client_ip: &str,
        limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool> {
        let mut redis = self.redis.clone();
        redis::Script::new(CHECK_AND_INCREMENT_LIMIT_SCRIPT)
            .key(self.key(&cache_key("SEND_EMAIL_VERIFY_LIMIT", client_ip)))
            .arg(limit)
            .arg(ttl_seconds.max(1))
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|result| result == 1)
            .map_err(|error| repository_error("increment email-send limiter", error))
    }

    async fn reserve_email_code(
        &self,
        email: &str,
        code: &str,
        now: i64,
    ) -> RepositoryResult<bool> {
        let mut redis = self.redis.clone();
        redis::Script::new(RESERVE_EMAIL_CODE_SCRIPT)
            .key(self.key(&cache_key("EMAIL_VERIFY_CODE", email)))
            .key(self.key(&cache_key("LAST_SEND_EMAIL_VERIFY_TIMESTAMP", email)))
            .arg(code)
            .arg(now)
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|reserved| reserved == 1)
            .map_err(|error| repository_error("reserve verification email code", error))
    }

    async fn release_email_code(&self, email: &str, code: &str) {
        let mut redis = self.redis.clone();
        if let Err(error) = redis::Script::new(RELEASE_EMAIL_CODE_SCRIPT)
            .key(self.key(&cache_key("EMAIL_VERIFY_CODE", email)))
            .key(self.key(&cache_key("LAST_SEND_EMAIL_VERIFY_TIMESTAMP", email)))
            .arg(code)
            .invoke_async::<i64>(&mut redis)
            .await
        {
            tracing::warn!(?error, "failed to release verification email code");
        }
    }

    async fn put_temporary_token(
        &self,
        token: &str,
        user_id: i64,
        session_epoch: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<()> {
        let mut redis = self.redis.clone();
        redis
            .set_ex::<_, _, ()>(
                self.key(&cache_key("TEMP_TOKEN", token)),
                format!("{user_id}:{session_epoch}"),
                ttl_seconds,
            )
            .await
            .map_err(|error| repository_error("store temporary login token", error))
    }

    async fn take_temporary_token(&self, token: &str) -> RepositoryResult<Option<SessionIdentity>> {
        let mut redis = self.redis.clone();
        let value: Option<String> = redis::cmd("GETDEL")
            .arg(self.key(&cache_key("TEMP_TOKEN", token)))
            .query_async(&mut redis)
            .await
            .map_err(|error| repository_error("consume temporary login token", error))?;
        Ok(value.and_then(|value| {
            let (user_id, session_epoch) = value.split_once(':')?;
            Some(SessionIdentity {
                user_id: user_id.parse().ok()?,
                session_id: String::new(),
                session_epoch: session_epoch.parse().ok()?,
            })
        }))
    }

    async fn add_session(
        &self,
        identity: &SessionIdentity,
        metadata: &SessionMetadata,
        bearer: &str,
        ttl_seconds: u64,
        maximum_sessions: i64,
        now: i64,
    ) -> RepositoryResult<bool> {
        let bearer_hash = digest(bearer);
        let metadata = serde_json::to_string(&RedisSessionMetadata::from_application(
            metadata,
            bearer_hash,
        ))
        .map_err(|error| repository_error("encode session metadata", error))?;
        let user_id = identity.user_id;
        let session_id = identity.session_id.clone();
        let identity = serde_json::to_string(&RedisSessionIdentity::from(identity))
            .map_err(|error| repository_error("encode session identity", error))?;
        let mut redis = self.redis.clone();
        redis::Script::new(ADD_OPAQUE_SESSION_SCRIPT)
            .key(self.key(&user_sessions_key(user_id)))
            .key(self.key(&auth_session_key(bearer)))
            .key(self.key(&user_auth_keys_key(user_id)))
            .arg(session_id)
            .arg(metadata)
            .arg(identity)
            .arg(ttl_seconds)
            .arg(now)
            .arg(maximum_sessions)
            .arg(self.key(AUTH_SESSION_KEY_PREFIX))
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|inserted| inserted == 1)
            .map_err(|error| repository_error("store opaque login session", error))
    }

    async fn session_identity(&self, bearer: &str) -> RepositoryResult<Option<SessionIdentity>> {
        let mut redis = self.redis.clone();
        let value: Option<String> = redis
            .get(self.key(&auth_session_key(bearer)))
            .await
            .map_err(|error| repository_error("load opaque session identity", error))?;
        value
            .map(|value| {
                serde_json::from_str::<RedisSessionIdentity>(&value)
                    .map(SessionIdentity::from)
                    .map_err(|error| repository_error("decode opaque session identity", error))
            })
            .transpose()
    }

    async fn session_metadata(
        &self,
        user_id: i64,
        session_id: &str,
    ) -> RepositoryResult<Option<SessionMetadata>> {
        Ok(self
            .load_sessions(user_id)
            .await?
            .into_iter()
            .find(|session| session.session_id == session_id)
            .map(|session| session.metadata))
    }

    async fn sessions(&self, user_id: i64) -> RepositoryResult<Vec<StoredSession>> {
        self.load_sessions(user_id).await
    }

    async fn remove_session(&self, user_id: i64, session_id: &str) -> RepositoryResult<()> {
        let mut redis = self.redis.clone();
        redis::Script::new(REMOVE_SESSION_SCRIPT)
            .key(self.key(&user_sessions_key(user_id)))
            .key(self.key(&user_auth_keys_key(user_id)))
            .arg(session_id)
            .arg(self.key(AUTH_SESSION_KEY_PREFIX))
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|_| ())
            .map_err(|error| repository_error("remove login session", error))
    }

    async fn remove_all_sessions(&self, user_id: i64) -> RepositoryResult<()> {
        let mut redis = self.redis.clone();
        redis::Script::new(REMOVE_ALL_SESSIONS_SCRIPT)
            .key(self.key(&user_sessions_key(user_id)))
            .key(self.key(&user_auth_keys_key(user_id)))
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|_| ())
            .map_err(|error| repository_error("remove all login sessions", error))
    }

    async fn reserve_step_up_attempt(
        &self,
        user_id: i64,
        client_ip: Option<&str>,
        user_limit: i64,
        ip_limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool> {
        let keys = step_up_limiter_keys(user_id, client_ip).map(|key| self.key(&key));
        let mut redis = self.redis.clone();
        redis::Script::new(RESERVE_STEP_UP_ATTEMPT_SCRIPT)
            .key(&keys[0])
            .key(&keys[1])
            .arg(user_limit)
            .arg(ip_limit)
            .arg(ttl_seconds)
            .invoke_async::<i64>(&mut redis)
            .await
            .map(|reserved| reserved == 1)
            .map_err(|error| repository_error("reserve privileged step-up attempt", error))
    }

    async fn clear_step_up_attempts(&self, user_id: i64, client_ip: Option<&str>) {
        let keys = step_up_limiter_keys(user_id, client_ip).map(|key| self.key(&key));
        let mut redis = self.redis.clone();
        if let Err(error) = redis::cmd("DEL")
            .arg(&keys)
            .query_async::<i64>(&mut redis)
            .await
        {
            tracing::warn!(?error, "step-up limiter success cleanup failed");
        }
    }

    async fn put_step_up(
        &self,
        token: &str,
        user_id: i64,
        session_id: &str,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool> {
        let value = serde_json::to_string(&(user_id, session_id))
            .map_err(|error| repository_error("encode step-up identity", error))?;
        let mut redis = self.redis.clone();
        redis::cmd("SET")
            .arg(self.key(&step_up_key(token)))
            .arg(value)
            .arg("EX")
            .arg(ttl_seconds)
            .arg("NX")
            .query_async::<Option<String>>(&mut redis)
            .await
            .map(|inserted| inserted.is_some())
            .map_err(|error| repository_error("store privileged step-up token", error))
    }

    async fn step_up_identity(&self, token: &str) -> RepositoryResult<Option<(i64, String)>> {
        let mut redis = self.redis.clone();
        let value: Option<String> = redis
            .get(self.key(&step_up_key(token)))
            .await
            .map_err(|error| repository_error("load privileged step-up token", error))?;
        Ok(value.and_then(|value| serde_json::from_str(&value).ok()))
    }
}

impl RedisAuthCache {
    async fn load_sessions(&self, user_id: i64) -> RepositoryResult<Vec<StoredSession>> {
        let mut redis = self.redis.clone();
        let current: Option<String> = redis
            .get(self.key(&user_sessions_key(user_id)))
            .await
            .map_err(|error| repository_error("load account sessions", error))?;
        let Some(current) = current else {
            return Ok(Vec::new());
        };
        if current.len() > MAX_SESSION_METADATA_BYTES {
            return Err(repository_error(
                "decode account sessions",
                "session metadata exceeds its size limit",
            ));
        }
        let sessions = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&current)
            .map_err(|error| repository_error("decode account sessions", error))?;
        if sessions.len() > MAX_SESSION_METADATA_ENTRIES {
            return Err(repository_error(
                "decode account sessions",
                "session metadata exceeds its entry limit",
            ));
        }
        Ok(sessions
            .into_iter()
            .filter_map(|(session_id, value)| {
                serde_json::from_value::<RedisSessionMetadata>(value)
                    .ok()
                    .map(|metadata| StoredSession {
                        session_id,
                        metadata: metadata.into_application(),
                    })
            })
            .collect())
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

fn cache_key(prefix: &str, unique: &str) -> String {
    format!("{prefix}_{unique}")
}

fn digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn login_limiter_keys(email: &str, client_ip: Option<&str>) -> [String; 3] {
    let email = email.trim().to_ascii_lowercase();
    let client_ip = client_ip
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("no-client-ip");
    [
        format!("PASSWORD_ERROR_LIMIT_ACCOUNT_{}", digest(&email)),
        format!("PASSWORD_ERROR_LIMIT_IP_{}", digest(client_ip)),
        format!(
            "PASSWORD_ERROR_LIMIT_ACCOUNT_IP_{}",
            digest(&format!("{email}\0{client_ip}"))
        ),
    ]
}

fn user_sessions_key(user_id: i64) -> String {
    format!("USER_SESSIONS_{user_id}")
}

fn user_auth_keys_key(user_id: i64) -> String {
    format!("AUTH_USER_SESSION_KEYS_{user_id}")
}

fn auth_session_key(bearer: &str) -> String {
    format!("{AUTH_SESSION_KEY_PREFIX}{}", digest(bearer))
}

fn step_up_key(token: &str) -> String {
    format!("{AUTH_STEP_UP_KEY_PREFIX}{}", digest(token))
}

fn step_up_limiter_keys(user_id: i64, client_ip: Option<&str>) -> [String; 2] {
    [
        format!("AUTH_STEP_UP_LIMIT_USER_{user_id}"),
        format!(
            "AUTH_STEP_UP_LIMIT_IP_{}",
            digest(client_ip.unwrap_or("unknown"))
        ),
    ]
}

const RESERVE_LOGIN_ATTEMPT_SCRIPT: &str = r#"
local account_count = tonumber(redis.call('GET', KEYS[1]) or '0')
local ip_count = tonumber(redis.call('GET', KEYS[2]) or '0')
local account_ip_count = tonumber(redis.call('GET', KEYS[3]) or '0')
if account_count >= tonumber(ARGV[1]) or
   ip_count >= tonumber(ARGV[2]) or
   account_ip_count >= tonumber(ARGV[1]) then
    return 0
end
for index = 1, 3 do
    local count = redis.call('INCR', KEYS[index])
    if count == 1 then redis.call('EXPIRE', KEYS[index], ARGV[3]) end
end
return 1
"#;

const RELEASE_LOGIN_ATTEMPT_SCRIPT: &str = r#"
for index = 1, 3 do
    local count = tonumber(redis.call('GET', KEYS[index]) or '0')
    if count <= 1 then redis.call('DEL', KEYS[index]) else redis.call('DECR', KEYS[index]) end
end
return 1
"#;

const RESERVE_REGISTRATION_SLOT_SCRIPT: &str = r#"
local now = tonumber(ARGV[1])
local expires_at = tonumber(ARGV[2])
local limit = tonumber(ARGV[3])
local token = ARGV[4]
redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', now)
if redis.call('ZCARD', KEYS[1]) >= limit then return 0 end
redis.call('ZADD', KEYS[1], 'NX', expires_at, token)
redis.call('EXPIREAT', KEYS[1], expires_at)
return 1
"#;

const RELEASE_REGISTRATION_SLOT_SCRIPT: &str = r#"
local removed = redis.call('ZREM', KEYS[1], ARGV[1])
if redis.call('ZCARD', KEYS[1]) == 0 then redis.call('DEL', KEYS[1]) end
return removed
"#;

const CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT: &str = r#"
local current = tonumber(redis.call('GET', KEYS[2]) or '0')
if current >= tonumber(ARGV[2]) then return -1 end
if redis.call('GET', KEYS[1]) == ARGV[1] then redis.call('DEL', KEYS[1]); return 1 end
local value = redis.call('INCR', KEYS[2])
if value == 1 or redis.call('TTL', KEYS[2]) < 0 then redis.call('EXPIRE', KEYS[2], ARGV[3]) end
return 0
"#;

const CHECK_AND_INCREMENT_LIMIT_SCRIPT: &str = r#"
local current = tonumber(redis.call('GET', KEYS[1]) or '0')
if current >= tonumber(ARGV[1]) then return 0 end
local value = redis.call('INCR', KEYS[1])
if value == 1 or redis.call('TTL', KEYS[1]) < 0 then redis.call('EXPIRE', KEYS[1], ARGV[2]) end
return 1
"#;

const RESERVE_EMAIL_CODE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[2]) == 1 then return 0 end
redis.call('SET', KEYS[1], ARGV[1], 'EX', 300)
redis.call('SET', KEYS[2], ARGV[2], 'EX', 60)
return 1
"#;

const RELEASE_EMAIL_CODE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    redis.call('DEL', KEYS[1]); redis.call('DEL', KEYS[2]); return 1
end
return 0
"#;

const RESERVE_STEP_UP_ATTEMPT_SCRIPT: &str = r#"
local user_count = tonumber(redis.call('GET', KEYS[1]) or '0')
local ip_count = tonumber(redis.call('GET', KEYS[2]) or '0')
if user_count >= tonumber(ARGV[1]) or ip_count >= tonumber(ARGV[2]) then return 0 end
for index = 1, 2 do
    local count = redis.call('INCR', KEYS[index])
    if count == 1 then redis.call('EXPIRE', KEYS[index], ARGV[3]) end
end
return 1
"#;

const ADD_OPAQUE_SESSION_SCRIPT: &str = r#"
local inserted = redis.call('SET', KEYS[2], ARGV[3], 'EX', ARGV[4], 'NX')
if not inserted then return 0 end
local sessions = {}
local current = redis.call('GET', KEYS[1])
if current then
    local ok, decoded = pcall(cjson.decode, current)
    if ok and type(decoded) == 'table' then sessions = decoded end
end
local function remove_session(session_id, meta)
    if type(meta) == 'table' and meta['token_hash'] then
        local auth_key = ARGV[7] .. meta['token_hash']
        redis.call('DEL', auth_key)
        redis.call('SREM', KEYS[3], auth_key)
    end
    sessions[session_id] = nil
end
for session_id, meta in pairs(sessions) do
    if type(meta) == 'table' and meta['expires_at'] and tonumber(meta['expires_at']) <= tonumber(ARGV[5]) then
        remove_session(session_id, meta)
    end
end
sessions[ARGV[1]] = cjson.decode(ARGV[2])
local count = 0
for _ in pairs(sessions) do count = count + 1 end
while count > tonumber(ARGV[6]) do
    local oldest_id = nil
    local oldest_login = nil
    for session_id, meta in pairs(sessions) do
        if session_id ~= ARGV[1] then
            local login_at = 0
            if type(meta) == 'table' and meta['login_at'] then login_at = tonumber(meta['login_at']) or 0 end
            if oldest_id == nil or login_at < oldest_login or
               (login_at == oldest_login and session_id < oldest_id) then
                oldest_id = session_id
                oldest_login = login_at
            end
        end
    end
    if oldest_id == nil then break end
    remove_session(oldest_id, sessions[oldest_id])
    count = count - 1
end
redis.call('SET', KEYS[1], cjson.encode(sessions), 'EX', ARGV[4])
redis.call('SADD', KEYS[3], KEYS[2])
redis.call('EXPIRE', KEYS[3], ARGV[4])
return 1
"#;

const REMOVE_SESSION_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then return 0 end
local ok, sessions = pcall(cjson.decode, current)
if not ok or type(sessions) ~= 'table' then redis.call('DEL', KEYS[1]); return 0 end
local meta = sessions[ARGV[1]]
if type(meta) == 'table' and meta['token_hash'] then
    local auth_key = ARGV[2] .. meta['token_hash']
    redis.call('DEL', auth_key)
    redis.call('SREM', KEYS[2], auth_key)
end
sessions[ARGV[1]] = nil
if next(sessions) == nil then
    redis.call('DEL', KEYS[1]); redis.call('DEL', KEYS[2])
else
    redis.call('SET', KEYS[1], cjson.encode(sessions), 'KEEPTTL')
end
return 1
"#;

const REMOVE_ALL_SESSIONS_SCRIPT: &str = r#"
local auth_keys = redis.call('SMEMBERS', KEYS[2])
for _, auth_key in ipairs(auth_keys) do redis.call('DEL', auth_key) end
redis.call('DEL', KEYS[1]); redis.call('DEL', KEYS[2])
return #auth_keys
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_keys_normalize_accounts_and_never_embed_pii() {
        let first = login_limiter_keys(" User@Example.Test ", Some("203.0.113.7"));
        let second = login_limiter_keys("user@example.test", Some("203.0.113.7"));
        assert_eq!(first, second);
        assert!(first.iter().all(|key| !key.contains("example.test")));
        assert!(first.iter().all(|key| !key.contains("203.0.113.7")));
        assert_ne!(first[0], first[2]);
    }

    #[test]
    fn opaque_credentials_are_represented_only_by_digests_in_redis_keys() {
        let bearer = "opaque-session-credential";
        let session_key = auth_session_key(bearer);
        let step_up = step_up_key(bearer);
        assert!(session_key.starts_with(AUTH_SESSION_KEY_PREFIX));
        assert!(step_up.starts_with(AUTH_STEP_UP_KEY_PREFIX));
        assert!(!session_key.contains(bearer));
        assert!(!step_up.contains(bearer));
        assert_eq!(session_key.len(), AUTH_SESSION_KEY_PREFIX.len() + 64);
    }

    #[test]
    fn login_reservation_and_release_update_all_three_dimensions_atomically() {
        for fragment in [
            "account_count >= tonumber(ARGV[1])",
            "ip_count >= tonumber(ARGV[2])",
            "redis.call('INCR', KEYS[index])",
            "redis.call('EXPIRE', KEYS[index], ARGV[3])",
        ] {
            assert!(RESERVE_LOGIN_ATTEMPT_SCRIPT.contains(fragment));
        }
        assert!(RELEASE_LOGIN_ATTEMPT_SCRIPT.contains("for index = 1, 3 do"));
        assert!(RELEASE_LOGIN_ATTEMPT_SCRIPT.contains("redis.call('DECR', KEYS[index])"));
    }

    #[test]
    fn verification_codes_have_a_shared_failure_ceiling_and_one_time_consumption() {
        let limit_check = CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT
            .find("current >=")
            .unwrap();
        let code_check = CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT
            .find("GET', KEYS[1]")
            .unwrap();
        let failure_increment = CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT
            .find("INCR', KEYS[2]")
            .unwrap();
        assert!(limit_check < code_check && code_check < failure_increment);
        assert!(CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT.contains("DEL', KEYS[1]"));
        assert!(CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT.contains("EXPIRE', KEYS[2]"));
    }

    #[test]
    fn session_scripts_bind_reverse_mapping_eviction_and_cardinality() {
        assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("'NX'"));
        assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("'EX', ARGV[4]"));
        assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("while count > tonumber(ARGV[6])"));
        assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("redis.call('SREM', KEYS[3], auth_key)"));
        assert!(REMOVE_SESSION_SCRIPT.contains("local auth_key = ARGV[2] .. meta['token_hash']"));
        assert!(REMOVE_SESSION_SCRIPT.contains("redis.call('DEL', auth_key)"));
        assert!(REMOVE_ALL_SESSIONS_SCRIPT.contains("redis.call('DEL', auth_key)"));
    }

    #[test]
    fn cache_metadata_does_not_leak_the_bearer_into_application_models() {
        let application = SessionMetadata {
            ip: Some("203.0.113.7".to_string()),
            login_at: 1_000,
            user_agent: Some("browser".to_string()),
            expires_at: Some(2_000),
            password_authenticated: true,
        };
        let stored = RedisSessionMetadata::from_application(&application, "bearer-digest".into());
        assert_eq!(stored.token_hash.as_deref(), Some("bearer-digest"));
        assert_eq!(stored.into_application(), application);
    }
}
