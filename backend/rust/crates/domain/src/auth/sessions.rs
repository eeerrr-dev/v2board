use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_db as db;

use super::{AuthService, cache_key, legacy_guid};

#[derive(Debug, Serialize)]
pub struct AuthData {
    pub token: String,
    pub is_admin: i8,
    pub auth_data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct AuthClaims {
    id: i64,
    session: String,
    /// Tokens issued before the auth-hardening migration deserialize as epoch zero. A password
    /// reset or ban increments the database value and therefore revokes those legacy tokens too.
    #[serde(default)]
    pub(super) session_epoch: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpaqueSessionIdentity {
    id: i64,
    session: String,
    session_epoch: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    ip: Option<String>,
    login_at: i64,
    ua: Option<String>,
    #[serde(default)]
    auth_data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub id: i64,
    pub email: String,
    pub is_admin: i8,
    pub is_staff: i8,
    pub session_id: String,
}

impl AuthService {
    pub async fn quick_login_url(
        &self,
        user_id: i64,
        redirect: Option<&str>,
    ) -> Result<String, ApiError> {
        let code = legacy_guid(false);
        let key = cache_key("TEMP_TOKEN", &code);
        let session_epoch: i64 = sqlx::query_scalar(
            "SELECT session_epoch FROM v2_user WHERE id = ? AND banned = 0 LIMIT 1",
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
        let key = cache_key("TEMP_TOKEN", verify);
        let mut conn = self.redis.clone();
        let token_value: Option<String> = redis::cmd("GETDEL")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        let token_value = token_value.ok_or_else(|| ApiError::legacy("Token error"))?;
        let (user_id, session_epoch) =
            parse_temp_token(&token_value).ok_or_else(|| ApiError::legacy("Token error"))?;
        self.auth_data_for_user(user_id, Some(session_epoch), ip, user_agent)
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

    pub(super) async fn auth_data_for_user(
        &self,
        user_id: i64,
        expected_session_epoch: Option<i64>,
        ip: Option<String>,
        user_agent: Option<String>,
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

        // Establish the compatibility deadline once. Redis SET NX means neither a restart nor a
        // later configuration reload can silently extend acceptance of pre-migration JWTs.
        self.legacy_jwt_deadline().await?;

        let session = Uuid::new_v4().simple().to_string();
        let expires_at = now.saturating_add(self.config.auth_session_ttl_seconds as i64);
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
                        // Never place a bearer credential in the user-visible session map. Old
                        // JWT records are read only during the bounded migration window.
                        auth_data: String::new(),
                        token_hash: Some(token_hash),
                        expires_at: Some(expires_at),
                    },
                    &candidate,
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
        let claims = if let Some(identity) = self.opaque_session_identity(auth_data).await? {
            AuthClaims {
                id: identity.id,
                session: identity.session,
                session_epoch: identity.session_epoch,
            }
        } else {
            if !looks_like_legacy_jwt(auth_data)
                || Utc::now().timestamp() >= self.legacy_jwt_deadline().await?
            {
                return Err(ApiError::unauthorized());
            }
            self.decode_legacy_auth_data(auth_data)?
        };
        let user = db::user::find_user_for_auth_by_id(&self.db, claims.id).await?;
        let Some(user) = user else {
            return Err(ApiError::unauthorized());
        };
        if user.banned != 0 || user.session_epoch != claims.session_epoch {
            return Err(ApiError::unauthorized());
        }
        if !self.check_session(claims.id, &claims.session).await? {
            return Err(ApiError::unauthorized());
        }
        Ok(AuthUser {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin,
            is_staff: user.is_staff,
            session_id: claims.session,
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

    pub async fn remove_session(&self, user_id: i64, session_id: &str) -> Result<bool, ApiError> {
        let sessions_key = user_sessions_key(user_id);
        let auth_keys_key = user_auth_keys_key(user_id);
        let mut conn = self.redis.clone();
        redis::Script::new(REMOVE_SESSION_SCRIPT)
            .key(sessions_key)
            .key(auth_keys_key)
            .arg(session_id)
            .arg(AUTH_SESSION_KEY_PREFIX)
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(true)
    }

    pub async fn remove_all_sessions(&self, user_id: i64) -> Result<bool, ApiError> {
        let sessions_key = user_sessions_key(user_id);
        let auth_keys_key = user_auth_keys_key(user_id);
        let mut conn = self.redis.clone();
        redis::Script::new(REMOVE_ALL_SESSIONS_SCRIPT)
            .key(sessions_key)
            .key(auth_keys_key)
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(true)
    }

    fn decode_legacy_auth_data(&self, token: &str) -> Result<AuthClaims, ApiError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.required_spec_claims.clear();
        let data = jsonwebtoken::decode::<AuthClaims>(
            token,
            &DecodingKey::from_secret(self.config.app_key.as_bytes()),
            &validation,
        )
        .map_err(|_| ApiError::unauthorized())?;
        Ok(data.claims)
    }

    async fn add_opaque_session(
        &self,
        user_id: i64,
        session_id: &str,
        session_epoch: i64,
        meta: SessionMeta,
        auth_data: &str,
    ) -> Result<bool, ApiError> {
        let sessions_key = user_sessions_key(user_id);
        let auth_keys_key = user_auth_keys_key(user_id);
        let auth_key = auth_session_key(auth_data);
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
            .arg(self.config.auth_session_ttl_seconds)
            .arg(Utc::now().timestamp())
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(inserted == 1)
    }

    async fn opaque_session_identity(
        &self,
        auth_data: &str,
    ) -> Result<Option<OpaqueSessionIdentity>, ApiError> {
        let mut conn = self.redis.clone();
        let value: Option<String> = conn.get(auth_session_key(auth_data)).await?;
        match value {
            Some(value) => serde_json::from_str(&value).map(Some).map_err(|error| {
                ApiError::internal(format!("session identity decode error: {error}"))
            }),
            None => Ok(None),
        }
    }

    async fn legacy_jwt_deadline(&self) -> Result<i64, ApiError> {
        let candidate = self.config.legacy_jwt_cutoff_unix.max(0);
        let mut conn = self.redis.clone();
        let _: Option<String> = redis::cmd("SET")
            .arg(LEGACY_JWT_DEADLINE_KEY)
            .arg(candidate)
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        let stored: i64 = conn.get(LEGACY_JWT_DEADLINE_KEY).await?;
        // Configuration can always shorten or disable a previously persisted migration window;
        // it can never extend one. Because `candidate` is an absolute Unix timestamp, losing
        // Redis state cannot move the cutoff into the future.
        Ok(effective_legacy_jwt_cutoff(stored, candidate))
    }

    async fn check_session(&self, user_id: i64, session_id: &str) -> Result<bool, ApiError> {
        let sessions = self.load_sessions(user_id).await?;
        Ok(sessions.contains_key(session_id))
    }

    async fn load_sessions(
        &self,
        user_id: i64,
    ) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
        let key = user_sessions_key(user_id);
        let mut conn = self.redis.clone();
        let current: Option<String> = conn.get(key).await?;
        decode_session_metadata(current.as_deref())
    }
}

pub(super) fn decode_session_metadata(
    value: Option<&str>,
) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
    match value {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| ApiError::internal(format!("session metadata decode error: {error}"))),
        None => Ok(serde_json::Map::new()),
    }
}

// Bare `KEY_unique` names are the native Redis contract. The read-only reference
// applied deployment-specific cache prefixes outside its key helper; those prefixes
// are not part of this runtime and are intentionally neither read nor dual-written.
fn user_sessions_key(user_id: i64) -> String {
    format!("USER_SESSIONS_{user_id}")
}

pub(super) const AUTH_SESSION_KEY_PREFIX: &str = "AUTH_SESSION_";
const LEGACY_JWT_DEADLINE_KEY: &str = "AUTH_LEGACY_JWT_DEADLINE";

fn user_auth_keys_key(user_id: i64) -> String {
    format!("AUTH_USER_SESSION_KEYS_{user_id}")
}

fn auth_token_hash(auth_data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(auth_data.as_bytes());
    hex::encode(hasher.finalize())
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

pub(super) fn looks_like_legacy_jwt(value: &str) -> bool {
    value.split('.').count() == 3
}

pub(super) fn effective_legacy_jwt_cutoff(stored: i64, configured: i64) -> i64 {
    stored.min(configured.max(0))
}

pub(super) fn parse_temp_token(value: &str) -> Option<(i64, i64)> {
    if let Some((user_id, session_epoch)) = value.split_once(':') {
        return Some((user_id.parse().ok()?, session_epoch.parse().ok()?));
    }
    // One-minute tokens created immediately before the migration stored only the user id. They
    // remain usable for epoch-zero users and are automatically invalid after any revocation.
    Some((value.parse().ok()?, 0))
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
for session_id, meta in pairs(sessions) do
    if type(meta) == 'table' and meta['expires_at'] and tonumber(meta['expires_at']) <= tonumber(ARGV[5]) then
        sessions[session_id] = nil
    end
end
sessions[ARGV[1]] = cjson.decode(ARGV[2])
redis.call('SET', KEYS[1], cjson.encode(sessions), 'EX', ARGV[4])
redis.call('SADD', KEYS[3], KEYS[2])
redis.call('EXPIRE', KEYS[3], ARGV[4])
return 1
"#;

const REMOVE_SESSION_SCRIPT: &str = r#"
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
    user_id: i64,
) -> Result<(), redis::RedisError> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    redis::Script::new(REMOVE_ALL_SESSIONS_SCRIPT)
        .key(user_sessions_key(user_id))
        .key(user_auth_keys_key(user_id))
        .invoke_async::<i64>(&mut conn)
        .await?;
    Ok(())
}
