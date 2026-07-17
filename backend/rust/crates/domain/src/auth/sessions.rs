use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_db as db;

use super::{AuthService, cache_key, legacy_guid, validation::validate_password};

const MAX_SESSION_METADATA_BYTES: usize = 256 * 1024;
const MAX_SESSION_METADATA_ENTRIES: usize = 100;
const MAX_SESSION_IP_BYTES: usize = 64;
const MAX_SESSION_USER_AGENT_BYTES: usize = 512;
const AUTH_STEP_UP_KEY_PREFIX: &str = "AUTH_STEP_UP_";
const STEP_UP_LIMIT_USER_PREFIX: &str = "AUTH_STEP_UP_LIMIT_USER_";
const STEP_UP_LIMIT_IP_PREFIX: &str = "AUTH_STEP_UP_LIMIT_IP_";

pub(super) const RESERVE_STEP_UP_ATTEMPT_SCRIPT: &str = r#"
local user_count = tonumber(redis.call('GET', KEYS[1]) or '0')
local ip_count = tonumber(redis.call('GET', KEYS[2]) or '0')
if user_count >= tonumber(ARGV[1]) or ip_count >= tonumber(ARGV[2]) then
    return 0
end
for index = 1, 2 do
    local count = redis.call('INCR', KEYS[index])
    if count == 1 then
        redis.call('EXPIRE', KEYS[index], ARGV[3])
    end
end
return 1
"#;

#[derive(Serialize)]
pub struct AuthData {
    pub token: String,
    pub is_admin: i16,
    pub auth_data: String,
}

#[derive(Serialize, Deserialize)]
struct OpaqueSessionIdentity {
    id: i64,
    session: String,
    session_epoch: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionMeta {
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

#[derive(Clone, Serialize)]
pub struct AuthUser {
    pub id: i64,
    pub email: String,
    pub is_admin: i16,
    pub is_staff: i16,
    pub session_id: String,
    pub authenticated_at: i64,
    pub password_authenticated: bool,
}

impl AuthService {
    pub async fn quick_login_url(
        &self,
        user_id: i64,
        redirect: Option<&str>,
    ) -> Result<String, ApiError> {
        let code = legacy_guid(false);
        let key = self.redis_key(&cache_key("TEMP_TOKEN", &code));
        let session_epoch: i64 = sqlx::query_scalar(
            "SELECT session_epoch FROM users WHERE id = $1 AND banned = 0 LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
        let mut conn = self.redis.clone();
        conn.set_ex::<_, _, ()>(key, format!("{user_id}:{session_epoch}"), 60)
            .await?;
        Ok(self.login_redirect_url(&code, redirect))
    }

    pub async fn token_login(
        &self,
        verify: &str,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, ApiError> {
        let key = self.redis_key(&cache_key("TEMP_TOKEN", verify));
        let mut conn = self.redis.clone();
        let token_value: Option<String> = redis::cmd("GETDEL")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        let token_value = token_value.ok_or_else(|| ApiError::legacy("Token error"))?;
        let (user_id, session_epoch) =
            parse_temp_token(&token_value).ok_or_else(|| ApiError::legacy("Token error"))?;
        self.auth_data_for_user(user_id, Some(session_epoch), ip, user_agent, false)
            .await
    }

    pub(super) async fn auth_data_for_user(
        &self,
        user_id: i64,
        expected_session_epoch: Option<i64>,
        ip: Option<String>,
        user_agent: Option<String>,
        password_authenticated: bool,
    ) -> Result<AuthData, ApiError> {
        self.auth_data_for_user_inner(
            user_id,
            expected_session_epoch,
            ip,
            user_agent,
            password_authenticated,
        )
        .await
    }

    pub fn login_redirect_url(&self, token: &str, redirect: Option<&str>) -> String {
        let redirect = redirect
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("dashboard");
        let path = format!("/#/login?verify={token}&redirect={redirect}");
        if let Some(app_url) = self
            .config
            .app_url
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            // Raw concatenation, matching AuthController::token2Login's
            // `config('v2board.app_url') . $redirect`. Laravel does not strip a
            // trailing slash, so neither do we — an operator's configured app_url is
            // emitted verbatim into this backend-generated redirect/email link.
            format!("{app_url}{path}")
        } else {
            path
        }
    }

    async fn auth_data_for_user_inner(
        &self,
        user_id: i64,
        expected_session_epoch: Option<i64>,
        ip: Option<String>,
        user_agent: Option<String>,
        password_authenticated: bool,
    ) -> Result<AuthData, ApiError> {
        let user = db::user::find_user_for_auth_by_id(&self.db, user_id)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
        if user.banned != 0 {
            return Err(ApiError::legacy("Your account has been suspended"));
        }
        if expected_session_epoch.is_some_and(|expected| expected != user.session_epoch) {
            return Err(ApiError::unauthorized());
        }
        // Laravel `generateAuthData` does not write `last_login_at`; only registration sets it
        // once (Rust seeds it in the register INSERT). Do not touch it on login/token2Login.
        let now = Utc::now().timestamp();

        let session = Uuid::new_v4().simple().to_string();
        let session_ttl_seconds = session_ttl_seconds(
            self.config.auth_session_ttl_seconds,
            self.config.privileged_auth_session_ttl_seconds,
            user.is_admin,
            user.is_staff,
        );
        let expires_at = now.saturating_add(session_ttl_seconds as i64);
        let ip = ip.map(|value| truncate_utf8(value, MAX_SESSION_IP_BYTES));
        let user_agent = user_agent.map(|value| truncate_utf8(value, MAX_SESSION_USER_AGENT_BYTES));
        let mut auth_data = None;
        for _ in 0..3 {
            let candidate = generate_auth_token()?;
            let token_hash = auth_token_hash(&candidate);
            let inserted = self
                .add_opaque_session(
                    user.id,
                    &session,
                    user.session_epoch,
                    SessionMeta {
                        ip: ip.clone(),
                        login_at: now,
                        ua: user_agent.clone(),
                        token_hash: Some(token_hash),
                        expires_at: Some(expires_at),
                        password_authenticated,
                    },
                    &candidate,
                    session_ttl_seconds,
                )
                .await?;
            if inserted {
                auth_data = Some(candidate);
                break;
            }
        }
        let auth_data = auth_data
            .ok_or_else(|| ApiError::internal("could not allocate a unique session token"))?;

        Ok(AuthData {
            token: user.token,
            is_admin: user.is_admin,
            auth_data,
        })
    }

    pub async fn user_from_auth_data(&self, auth_data: &str) -> Result<AuthUser, ApiError> {
        if auth_data.is_empty() || auth_data.len() > 4096 {
            return Err(ApiError::unauthorized());
        }
        let identity = self
            .opaque_session_identity(auth_data)
            .await?
            .ok_or_else(ApiError::unauthorized)?;
        let user = db::user::find_user_for_auth_by_id(&self.db, identity.id).await?;
        let Some(user) = user else {
            return Err(ApiError::unauthorized());
        };
        if user.banned != 0 || user.session_epoch != identity.session_epoch {
            return Err(ApiError::unauthorized());
        }
        let Some(session_meta) = self.session_meta(identity.id, &identity.session).await? else {
            return Err(ApiError::unauthorized());
        };
        Ok(AuthUser {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin,
            is_staff: user.is_staff,
            session_id: identity.session,
            authenticated_at: session_meta.login_at,
            password_authenticated: session_meta.password_authenticated,
        })
    }

    pub async fn sessions(
        &self,
        user_id: i64,
        current_session_id: Option<&str>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
        let sessions = self.load_sessions(user_id).await?;
        let now = Utc::now().timestamp();
        let mut visible = serde_json::Map::new();
        for (session_id, value) in sessions {
            let Ok(meta) = serde_json::from_value::<SessionMeta>(value) else {
                continue;
            };
            if meta.expires_at.is_some_and(|expires_at| expires_at <= now) {
                continue;
            }
            visible.insert(
                session_id.clone(),
                serde_json::json!({
                    "ip": meta.ip.unwrap_or_default(),
                    "login_at": meta.login_at,
                    "ua": meta.ua.unwrap_or_default(),
                    // Keep the established response field without leaking any session bearer.
                    "auth_data": "",
                    "current": current_session_id == Some(session_id.as_str()),
                }),
            );
        }
        Ok(visible)
    }

    /// Revokes the session behind the opaque bearer an explicit sign-out
    /// presents. A bearer that no longer resolves — already revoked, expired,
    /// or never issued — is a successful no-op so repeated logout calls stay
    /// idempotent.
    pub async fn logout(&self, auth_data: &str) -> Result<bool, ApiError> {
        if auth_data.is_empty() || auth_data.len() > 4096 {
            return Ok(false);
        }
        let Some(identity) = self.opaque_session_identity(auth_data).await? else {
            return Ok(false);
        };
        self.remove_session(identity.id, &identity.session).await
    }

    pub async fn remove_session(&self, user_id: i64, session_id: &str) -> Result<bool, ApiError> {
        let sessions_key = self.redis_key(&user_sessions_key(user_id));
        let auth_keys_key = self.redis_key(&user_auth_keys_key(user_id));
        let mut conn = self.redis.clone();
        redis::Script::new(REMOVE_SESSION_SCRIPT)
            .key(sessions_key)
            .key(auth_keys_key)
            .arg(session_id)
            .arg(self.redis_key(AUTH_SESSION_KEY_PREFIX))
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(true)
    }

    pub async fn remove_all_sessions(&self, user_id: i64) -> Result<bool, ApiError> {
        let sessions_key = self.redis_key(&user_sessions_key(user_id));
        let auth_keys_key = self.redis_key(&user_auth_keys_key(user_id));
        let mut conn = self.redis.clone();
        redis::Script::new(REMOVE_ALL_SESSIONS_SCRIPT)
            .key(sessions_key)
            .key(auth_keys_key)
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(true)
    }

    /// Re-verifies a privileged user's password and issues a short-lived token
    /// bound to the currently authenticated session. Deployments can enable the
    /// corresponding mutation gate after their admin client has learned to send
    /// the returned token in `x-v2board-step-up`.
    pub async fn create_privileged_step_up(
        &self,
        user_id: i64,
        session_id: &str,
        password: &str,
        client_ip: Option<&str>,
    ) -> Result<String, ApiError> {
        validate_password(password)?;
        let limiter_keys = step_up_limiter_keys(user_id, client_ip).map(|key| self.redis_key(&key));
        let mut limiter_conn = self.redis.clone();
        let reserved = redis::Script::new(RESERVE_STEP_UP_ATTEMPT_SCRIPT)
            .key(&limiter_keys[0])
            .key(&limiter_keys[1])
            .arg(self.config.privileged_step_up_max_attempts)
            .arg(
                self.config
                    .privileged_step_up_max_attempts
                    .saturating_mul(5),
            )
            .arg(self.config.privileged_step_up_attempt_window_seconds)
            .invoke_async::<i64>(&mut limiter_conn)
            .await?;
        if reserved != 1 {
            return Err(ApiError::legacy(
                "Too many password verification attempts; try again later",
            ));
        }
        let user = db::user::find_user_for_auth_by_id(&self.db, user_id)
            .await?
            .ok_or_else(ApiError::unauthorized)?;
        if user.banned != 0 || (user.is_admin == 0 && user.is_staff == 0) {
            return Err(ApiError::unauthorized());
        }
        if !self
            .password_kdf
            .verify(
                user.password_algo.as_deref(),
                user.password_salt.as_deref(),
                password,
                &user.password,
            )
            .await?
        {
            return Err(ApiError::legacy("Incorrect email or password"));
        }
        if !self.check_session(user_id, session_id).await? {
            return Err(ApiError::unauthorized());
        }
        let mut limiter_conn = self.redis.clone();
        if let Err(error) = redis::cmd("DEL")
            .arg(&limiter_keys)
            .query_async::<i64>(&mut limiter_conn)
            .await
        {
            tracing::warn!(?error, "step-up limiter success cleanup failed");
        }

        for _ in 0..3 {
            let token = generate_auth_token()?;
            let key = self.redis_key(&step_up_key(&token));
            let value = serde_json::to_string(&(user_id, session_id))
                .map_err(|_| ApiError::internal("step-up identity encode error"))?;
            let mut conn = self.redis.clone();
            let inserted: Option<String> = redis::cmd("SET")
                .arg(key)
                .arg(value)
                .arg("EX")
                .arg(self.config.privileged_step_up_ttl_seconds)
                .arg("NX")
                .query_async(&mut conn)
                .await?;
            if inserted.is_some() {
                return Ok(token);
            }
        }
        Err(ApiError::internal("could not allocate a step-up token"))
    }

    pub async fn verify_privileged_step_up(
        &self,
        user_id: i64,
        session_id: &str,
        token: &str,
    ) -> Result<bool, ApiError> {
        if token.is_empty() || token.len() > 256 {
            return Ok(false);
        }
        let mut conn = self.redis.clone();
        let value: Option<String> = conn.get(self.redis_key(&step_up_key(token))).await?;
        let Some(value) = value else {
            return Ok(false);
        };
        let Ok((bound_user_id, bound_session_id)) = serde_json::from_str::<(i64, String)>(&value)
        else {
            return Ok(false);
        };
        Ok(bound_user_id == user_id && bound_session_id == session_id)
    }

    async fn add_opaque_session(
        &self,
        user_id: i64,
        session_id: &str,
        session_epoch: i64,
        meta: SessionMeta,
        auth_data: &str,
        ttl_seconds: u64,
    ) -> Result<bool, ApiError> {
        let sessions_key = self.redis_key(&user_sessions_key(user_id));
        let auth_keys_key = self.redis_key(&user_auth_keys_key(user_id));
        let auth_key = self.redis_key(&auth_session_key(auth_data));
        let meta = serde_json::to_string(&meta)
            .map_err(|error| ApiError::internal(format!("session encode error: {error}")))?;
        let identity = serde_json::to_string(&OpaqueSessionIdentity {
            id: user_id,
            session: session_id.to_string(),
            session_epoch,
        })
        .map_err(|error| ApiError::internal(format!("session identity encode error: {error}")))?;
        let mut conn = self.redis.clone();
        let inserted = redis::Script::new(ADD_OPAQUE_SESSION_SCRIPT)
            .key(sessions_key)
            .key(auth_key)
            .key(auth_keys_key)
            .arg(session_id)
            .arg(meta)
            .arg(identity)
            .arg(ttl_seconds)
            .arg(Utc::now().timestamp())
            .arg(self.config.auth_session_max_per_user)
            .arg(self.redis_key(AUTH_SESSION_KEY_PREFIX))
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(inserted == 1)
    }

    async fn opaque_session_identity(
        &self,
        auth_data: &str,
    ) -> Result<Option<OpaqueSessionIdentity>, ApiError> {
        let mut conn = self.redis.clone();
        let value: Option<String> = conn
            .get(self.redis_key(&auth_session_key(auth_data)))
            .await?;
        match value {
            Some(value) => serde_json::from_str(&value).map(Some).map_err(|error| {
                ApiError::internal(format!("session identity decode error: {error}"))
            }),
            None => Ok(None),
        }
    }

    async fn check_session(&self, user_id: i64, session_id: &str) -> Result<bool, ApiError> {
        Ok(self.session_meta(user_id, session_id).await?.is_some())
    }

    async fn session_meta(
        &self,
        user_id: i64,
        session_id: &str,
    ) -> Result<Option<SessionMeta>, ApiError> {
        let sessions = self.load_sessions(user_id).await?;
        let Some(value) = sessions.get(session_id).cloned() else {
            return Ok(None);
        };
        let meta =
            serde_json::from_value::<SessionMeta>(value).map_err(|_| ApiError::unauthorized())?;
        if meta
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now().timestamp())
        {
            return Ok(None);
        }
        Ok(Some(meta))
    }

    async fn load_sessions(
        &self,
        user_id: i64,
    ) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
        let key = self.redis_key(&user_sessions_key(user_id));
        let mut conn = self.redis.clone();
        let current: Option<String> = conn.get(key).await?;
        decode_session_metadata(current.as_deref())
    }
}

pub(super) fn decode_session_metadata(
    value: Option<&str>,
) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
    match value {
        Some(value) => {
            if value.len() > MAX_SESSION_METADATA_BYTES {
                return Err(ApiError::internal(
                    "session metadata exceeds its size limit",
                ));
            }
            let sessions =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(value).map_err(
                    |error| ApiError::internal(format!("session metadata decode error: {error}")),
                )?;
            if sessions.len() > MAX_SESSION_METADATA_ENTRIES {
                return Err(ApiError::internal(
                    "session metadata exceeds its entry limit",
                ));
            }
            Ok(sessions)
        }
        None => Ok(serde_json::Map::new()),
    }
}

pub(super) fn truncate_utf8(mut value: String, maximum_bytes: usize) -> String {
    if value.len() <= maximum_bytes {
        return value;
    }
    let mut boundary = maximum_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value
}

pub(super) fn session_ttl_seconds(
    ordinary: u64,
    privileged: u64,
    is_admin: i16,
    is_staff: i16,
) -> u64 {
    if is_admin != 0 || is_staff != 0 {
        privileged
    } else {
        ordinary
    }
}

// These helpers return logical names only. AuthService binds every name to the
// immutable installation keyspace before it reaches Redis; the empty pre-release
// Redis has no legacy prefix or dual-read contract to preserve.
fn user_sessions_key(user_id: i64) -> String {
    format!("USER_SESSIONS_{user_id}")
}

pub(super) const AUTH_SESSION_KEY_PREFIX: &str = "AUTH_SESSION_";

fn user_auth_keys_key(user_id: i64) -> String {
    format!("AUTH_USER_SESSION_KEYS_{user_id}")
}

fn auth_token_hash(auth_data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(auth_data.as_bytes());
    hex::encode(hasher.finalize())
}

fn step_up_key(token: &str) -> String {
    format!("{AUTH_STEP_UP_KEY_PREFIX}{}", auth_token_hash(token))
}

pub(super) fn step_up_limiter_keys(user_id: i64, client_ip: Option<&str>) -> [String; 2] {
    let ip_hash = auth_token_hash(client_ip.unwrap_or("unknown"));
    [
        format!("{STEP_UP_LIMIT_USER_PREFIX}{user_id}"),
        format!("{STEP_UP_LIMIT_IP_PREFIX}{ip_hash}"),
    ]
}

pub(super) fn auth_session_key(auth_data: &str) -> String {
    format!("{AUTH_SESSION_KEY_PREFIX}{}", auth_token_hash(auth_data))
}

pub(super) fn generate_auth_token() -> Result<String, ApiError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| ApiError::internal(format!("session entropy error: {error}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(super) fn parse_temp_token(value: &str) -> Option<(i64, i64)> {
    let (user_id, session_epoch) = value.split_once(':')?;
    Some((user_id.parse().ok()?, session_epoch.parse().ok()?))
}

pub(super) const ADD_OPAQUE_SESSION_SCRIPT: &str = r#"
local inserted = redis.call('SET', KEYS[2], ARGV[3], 'EX', ARGV[4], 'NX')
if not inserted then
    return 0
end
local sessions = {}
local current = redis.call('GET', KEYS[1])
if current then
    local ok, decoded = pcall(cjson.decode, current)
    if ok and type(decoded) == 'table' then
        sessions = decoded
    end
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
for _ in pairs(sessions) do
    count = count + 1
end
while count > tonumber(ARGV[6]) do
    local oldest_id = nil
    local oldest_login = nil
    for session_id, meta in pairs(sessions) do
        if session_id ~= ARGV[1] then
            local login_at = 0
            if type(meta) == 'table' and meta['login_at'] then
                login_at = tonumber(meta['login_at']) or 0
            end
            if oldest_id == nil or login_at < oldest_login or
                (login_at == oldest_login and session_id < oldest_id) then
                oldest_id = session_id
                oldest_login = login_at
            end
        end
    end
    if oldest_id == nil then
        break
    end
    remove_session(oldest_id, sessions[oldest_id])
    count = count - 1
end
redis.call('SET', KEYS[1], cjson.encode(sessions), 'EX', ARGV[4])
redis.call('SADD', KEYS[3], KEYS[2])
redis.call('EXPIRE', KEYS[3], ARGV[4])
return 1
"#;

pub(super) const REMOVE_SESSION_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then
    return 0
end
local ok, sessions = pcall(cjson.decode, current)
if not ok or type(sessions) ~= 'table' then
    redis.call('DEL', KEYS[1])
    return 0
end
local meta = sessions[ARGV[1]]
if type(meta) == 'table' and meta['token_hash'] then
    local auth_key = ARGV[2] .. meta['token_hash']
    redis.call('DEL', auth_key)
    redis.call('SREM', KEYS[2], auth_key)
end
sessions[ARGV[1]] = nil
if next(sessions) == nil then
    redis.call('DEL', KEYS[1])
    redis.call('DEL', KEYS[2])
else
    redis.call('SET', KEYS[1], cjson.encode(sessions), 'KEEPTTL')
end
return 1
"#;

const REMOVE_ALL_SESSIONS_SCRIPT: &str = r#"
local auth_keys = redis.call('SMEMBERS', KEYS[2])
for _, auth_key in ipairs(auth_keys) do
    redis.call('DEL', auth_key)
end
redis.call('DEL', KEYS[1])
redis.call('DEL', KEYS[2])
return #auth_keys
"#;

/// Best-effort cache cleanup for admin/staff revocation paths. The database session epoch remains
/// authoritative; this removes the hashed opaque-token reverse mappings immediately as well as
/// the user-visible session metadata.
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
