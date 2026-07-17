use std::sync::OnceLock;

use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use v2board_compat::{ApiError, Code, Problem};
use v2board_config::duration_minutes_to_seconds;
use v2board_db as db;

use super::password::password_needs_rehash;
use super::validation::{
    normalize_email, validate_change_password, validate_email, validate_forget, validate_password,
};
use super::verification::LimitedEmailCodeResult;
use super::{AuthData, AuthService, cache_key, legacy_guid};

static DUMMY_PASSWORD_HASH: OnceLock<String> = OnceLock::new();

pub(super) const RESERVE_LOGIN_ATTEMPT_SCRIPT: &str = r#"
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
    if count == 1 then
        redis.call('EXPIRE', KEYS[index], ARGV[3])
    end
end
return 1
"#;

pub(super) const RELEASE_LOGIN_ATTEMPT_SCRIPT: &str = r#"
for index = 1, 3 do
    local count = tonumber(redis.call('GET', KEYS[index]) or '0')
    if count <= 1 then
        redis.call('DEL', KEYS[index])
    else
        redis.call('DECR', KEYS[index])
    end
end
return 1
"#;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgetInput {
    pub email: String,
    pub email_code: String,
    pub password: String,
}

impl AuthService {
    pub async fn login(
        &self,
        email: &str,
        password: &str,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, ApiError> {
        // Laravel `AuthLogin` FormRequest validates email (required|email:strict) and
        // password (required|min:8) with 422 field errors before the controller body runs,
        // i.e. before the password-error rate limiter is touched.
        validate_email(email)?;
        validate_password(password)?;
        let email = normalize_email(email);

        let password_error_limit = if self.config.password_limit_enable {
            let keys = login_limiter_keys(&email, ip.as_deref()).map(|key| self.redis_key(&key));
            if !self.reserve_login_attempt(&keys).await? {
                return Err(Problem::new(Code::PasswordAttemptsRateLimited)
                    .with_detail(format!(
                        "There are too many password errors, please try again after {} minutes.",
                        self.config.password_limit_expire
                    ))
                    .into());
            }
            Some(keys)
        } else {
            None
        };

        let user = match db::user::find_user_for_auth(&self.db, &email).await {
            Ok(user) => user,
            Err(error) => {
                self.release_login_attempt(password_error_limit.as_ref())
                    .await;
                return Err(error.into());
            }
        };
        let Some(user) = user else {
            if let Err(error) = self.verify_dummy_password(password).await {
                self.release_login_attempt(password_error_limit.as_ref())
                    .await;
                return Err(error);
            }
            return Err(Problem::new(Code::InvalidCredentials).into());
        };

        let password_matches = match self
            .password_kdf
            .verify(
                user.password_algo.as_deref(),
                user.password_salt.as_deref(),
                password,
                &user.password,
            )
            .await
        {
            Ok(password_matches) => password_matches,
            Err(error) => {
                self.release_login_attempt(password_error_limit.as_ref())
                    .await;
                return Err(error);
            }
        };
        if !password_matches {
            return Err(Problem::new(Code::InvalidCredentials).into());
        }

        // The reservation represents only a password failure. Release it once
        // the password is proven correct, while retaining prior failed counts.
        self.release_login_attempt(password_error_limit.as_ref())
            .await;

        if user.banned != 0 {
            return Err(Problem::new(Code::AccountSuspended).into());
        }

        if password_needs_rehash(user.password_algo.as_deref(), &user.password) {
            let upgraded_hash = self.password_kdf.hash(password).await?;
            // Compare-and-set makes a concurrent password reset authoritative. Failure to win
            // that race is not a login error; its newly incremented session epoch will reject
            // the token before it can be used.
            db::user::rehash_password(
                &self.db,
                user.id,
                &user.password,
                &upgraded_hash,
                Utc::now().timestamp(),
            )
            .await?;
        }

        self.auth_data_for_user(user.id, Some(user.session_epoch), ip, user_agent, true)
            .await
    }

    async fn reserve_login_attempt(&self, keys: &[String; 3]) -> Result<bool, ApiError> {
        let mut conn = self.redis.clone();
        let account_limit = self.config.password_limit_count.max(1);
        let ip_limit = account_limit.saturating_mul(10);
        let reserved = redis::Script::new(RESERVE_LOGIN_ATTEMPT_SCRIPT)
            .key(&keys[0])
            .key(&keys[1])
            .key(&keys[2])
            .arg(account_limit)
            .arg(ip_limit)
            .arg(duration_minutes_to_seconds(
                self.config.password_limit_expire,
            ))
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(reserved == 1)
    }

    async fn release_login_attempt(&self, keys: Option<&[String; 3]>) {
        let Some(keys) = keys else {
            return;
        };
        let mut conn = self.redis.clone();
        if let Err(error) = redis::Script::new(RELEASE_LOGIN_ATTEMPT_SCRIPT)
            .key(&keys[0])
            .key(&keys[1])
            .key(&keys[2])
            .invoke_async::<i64>(&mut conn)
            .await
        {
            tracing::warn!(?error, "login limiter reservation cleanup failed");
        }
    }

    async fn verify_dummy_password(&self, password: &str) -> Result<(), ApiError> {
        let hash = if let Some(hash) = DUMMY_PASSWORD_HASH.get() {
            hash.clone()
        } else {
            let candidate = self
                .password_kdf
                .hash("v2board-dummy-password-not-an-account")
                .await?;
            let _ = DUMMY_PASSWORD_HASH.set(candidate);
            DUMMY_PASSWORD_HASH
                .get()
                .expect("dummy password hash was initialized")
                .clone()
        };
        let _ = self
            .password_kdf
            .verify(None, None, password, &hash)
            .await?;
        Ok(())
    }

    pub async fn forget(&self, input: ForgetInput) -> Result<bool, ApiError> {
        // Laravel `AuthForget` FormRequest validates email (email:strict, max 64),
        // password (min 8, max 64 — character counts) and email_code (digits:6) with 422
        // field errors before the controller body runs.
        validate_forget(&input.email, &input.password, &input.email_code)?;
        // Laravel lowercases only the cache-key email (`strtolower(trim($email))`) for
        // FORGET_REQUEST_LIMIT / EMAIL_VERIFY_CODE and passes the original spelling to
        // the user lookup. The legacy utf8mb4_unicode_ci column still made that lookup
        // case-insensitive; the PostgreSQL repository preserves the same identity rule.
        let email = input.email.trim();
        let cache_email = email.to_ascii_lowercase();
        let limit_key = self.redis_key(&cache_key("FORGET_REQUEST_LIMIT", &cache_email));
        match self
            .consume_email_code_with_failure_limit(
                &cache_email,
                &input.email_code,
                &limit_key,
                3,
                300,
            )
            .await?
        {
            LimitedEmailCodeResult::Consumed => {}
            LimitedEmailCodeResult::Incorrect => {
                return Err(Problem::new(Code::InvalidEmailCode).into());
            }
            LimitedEmailCodeResult::Limited => {
                return Err(Problem::new(Code::PasswordResetFailed).into());
            }
        }
        let user = db::user::find_user_for_auth(&self.db, email)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::EmailNotRegistered)))?;
        let password_hash = self.password_kdf.hash(&input.password).await?;
        let updated =
            db::user::update_password(&self.db, user.id, &password_hash, Utc::now().timestamp())
                .await?;
        if !updated {
            return Err(Problem::new(Code::PasswordResetFailed).into());
        }
        // The database epoch is the durable revocation mechanism. Redis cleanup is cache hygiene
        // only and must not turn an already committed password reset into an apparent failure.
        if let Err(error) = self.remove_all_sessions(user.id).await {
            tracing::warn!(
                ?error,
                user_id = user.id,
                "password-reset session cache cleanup failed after durable revocation"
            );
        }
        Ok(true)
    }

    pub async fn change_password(
        &self,
        user_id: i64,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), ApiError> {
        validate_change_password(old_password, new_password)?;

        let user = db::user::find_user_for_auth_by_id(&self.db, user_id)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
        if !self
            .password_kdf
            .verify(
                user.password_algo.as_deref(),
                user.password_salt.as_deref(),
                old_password,
                &user.password,
            )
            .await?
        {
            return Err(Problem::new(Code::OldPasswordIncorrect).into());
        }

        let password_hash = self.password_kdf.hash(new_password).await?;
        let updated = db::user::change_password_if_current(
            &self.db,
            user_id,
            &user.password,
            user.session_epoch,
            &password_hash,
            Utc::now().timestamp(),
        )
        .await?;
        if !updated {
            return Err(ApiError::business("Save failed"));
        }
        if let Err(error) = self.remove_all_sessions(user_id).await {
            tracing::warn!(
                ?error,
                user_id,
                "password-change session cache cleanup failed after durable revocation"
            );
        }
        Ok(())
    }

    pub async fn reset_security(&self, user_id: i64) -> Result<String, ApiError> {
        let uuid = Uuid::new_v4().hyphenated().to_string();
        let token = legacy_guid(false);
        let updated =
            db::user::update_security(&self.db, user_id, &uuid, &token, Utc::now().timestamp())
                .await?;
        if !updated {
            // Legacy "Reset failed" — shares password_reset_failed with the
            // forget-password flow (docs/api-dialect.md §3.4 registry).
            return Err(Problem::new(Code::PasswordResetFailed).into());
        }
        // Method-aware minting (Helper::getSubscribeUrl): under
        // show_subscribe_method 1/2 the returned URL must carry the rotating
        // token, never the freshly rotated permanent one.
        crate::subscribe_link::subscribe_url_for_user(
            &self.config,
            &self.redis_keys,
            &mut Some(self.redis.clone()),
            user_id,
            &token,
        )
        .await
    }
}

pub(super) fn login_limiter_keys(email: &str, ip: Option<&str>) -> [String; 3] {
    let email = normalize_email(email);
    let ip = ip
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("no-client-ip");
    [
        format!("PASSWORD_ERROR_LIMIT_ACCOUNT_{}", digest_key(&email)),
        format!("PASSWORD_ERROR_LIMIT_IP_{}", digest_key(ip)),
        format!(
            "PASSWORD_ERROR_LIMIT_ACCOUNT_IP_{}",
            digest_key(&format!("{email}\0{ip}"))
        ),
    ]
}

fn digest_key(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}
