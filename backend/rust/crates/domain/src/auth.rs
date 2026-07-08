use std::path::Path;

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Transaction};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::AppConfig;
use v2board_db::{self as db, DbPool};

#[derive(Clone)]
pub struct AuthService {
    db: DbPool,
    redis: redis::Client,
    config: AppConfig,
}

#[derive(Debug, Serialize)]
pub struct AuthData {
    pub token: String,
    pub is_admin: i8,
    pub auth_data: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthClaims {
    id: i64,
    session: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    ip: Option<String>,
    login_at: i64,
    ua: Option<String>,
    auth_data: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub invite_code: Option<String>,
    pub email_code: Option<String>,
    pub recaptcha_data: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgetInput {
    pub email: String,
    pub email_code: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailVerifyInput {
    pub email: String,
    pub isforget: Option<i32>,
    pub recaptcha_data: Option<String>,
}

impl AuthService {
    pub fn new(db: DbPool, redis: redis::Client, config: AppConfig) -> Self {
        Self { db, redis, config }
    }

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

        let password_error_limit = if self.config.password_limit_enable {
            let key = cache_key("PASSWORD_ERROR_LIMIT", email);
            let mut conn = self.redis.get_multiplexed_async_connection().await?;
            let count = conn.get::<_, Option<i64>>(&key).await?.unwrap_or(0);
            if count >= self.config.password_limit_count {
                return Err(ApiError::legacy(format!(
                    "There are too many password errors, please try again after {} minutes.",
                    self.config.password_limit_expire
                )));
            }
            Some((key, count))
        } else {
            None
        };

        let user = db::user::find_user_for_auth(&self.db, email)
            .await?
            .ok_or_else(|| ApiError::legacy("Incorrect email or password"))?;

        if !verify_password(
            user.password_algo.as_deref(),
            user.password_salt.as_deref(),
            password,
            &user.password,
        ) {
            if let Some((key, count)) = password_error_limit {
                let mut conn = self.redis.get_multiplexed_async_connection().await?;
                conn.set_ex::<_, _, ()>(
                    key,
                    count + 1,
                    (self.config.password_limit_expire.max(1) * 60) as u64,
                )
                .await?;
            }
            return Err(ApiError::legacy("Incorrect email or password"));
        }

        if user.banned != 0 {
            return Err(ApiError::legacy("Your account has been suspended"));
        }

        self.auth_data_for_user(user.id, ip, user_agent).await
    }

    pub async fn register(
        &self,
        input: RegisterInput,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, ApiError> {
        // Laravel validates the AuthRegister FormRequest (email/password) before the controller
        // body, then runs the controller checks in this order: IP rate limit, recaptcha,
        // email whitelist/gmail, stop_register, invite_force, email verification code.
        validate_email(&input.email)?;
        validate_password(&input.password)?;
        // Laravel stores the TrimStrings-trimmed email in original case; the uniqueness lookup
        // (`User::where('email', ...)`) relies on the column collation. Only the
        // EMAIL_VERIFY_CODE cache key is lowercased (`strtolower(trim($email))`).
        let email = input.email.trim();
        let cache_email = email.to_ascii_lowercase();

        if let Some(ip) = ip.as_deref()
            && self.config.register_limit_by_ip_enable
        {
            let key = cache_key("REGISTER_IP_RATE_LIMIT", ip);
            let mut conn = self.redis.get_multiplexed_async_connection().await?;
            let count: i64 = conn.get(&key).await.unwrap_or(0);
            if count >= self.config.register_limit_count {
                return Err(ApiError::legacy(format!(
                    "Register frequently, please try again after {} minute",
                    self.config.register_limit_expire
                )));
            }
        }
        self.verify_recaptcha(input.recaptcha_data.as_deref())
            .await?;
        self.validate_register_email(email).await?;
        if self.config.stop_register {
            return Err(ApiError::legacy("Registration has closed"));
        }
        if self.config.invite_force
            && input
                .invite_code
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            return Err(ApiError::legacy(
                "You must use the invitation code to register",
            ));
        }
        if self.config.email_verify {
            self.verify_email_code(&cache_email, input.email_code.as_deref())
                .await?;
        }

        let password_hash = hash_password(&input.password)?;
        let uuid = legacy_guid(true);
        let token = legacy_guid(false);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;

        if email_exists_for_update(&mut tx, email).await? {
            return Err(ApiError::legacy("Email already exists"));
        }

        let invite_user_id = self
            .consume_invite_code(&mut tx, input.invite_code.as_deref())
            .await?;
        let trial = self.trial_plan(&mut tx).await?;

        let result = sqlx::query(
            r#"
            INSERT INTO v2_user (
                invite_user_id, email, password, uuid, token, transfer_enable, device_limit,
                group_id, plan_id, speed_limit, expired_at, last_login_at, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(invite_user_id)
        .bind(email)
        .bind(password_hash)
        .bind(uuid)
        .bind(token)
        .bind(trial.transfer_enable)
        .bind(trial.device_limit)
        .bind(trial.group_id)
        .bind(trial.plan_id)
        .bind(trial.speed_limit)
        .bind(trial.expired_at)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let user_id = result.last_insert_id() as i64;
        tx.commit().await?;

        if self.config.email_verify {
            let mut conn = self.redis.get_multiplexed_async_connection().await?;
            conn.del::<_, ()>(cache_key("EMAIL_VERIFY_CODE", &cache_email))
                .await?;
        }
        if let Some(ip) = ip.as_deref()
            && self.config.register_limit_by_ip_enable
        {
            let key = cache_key("REGISTER_IP_RATE_LIMIT", ip);
            let mut conn = self.redis.get_multiplexed_async_connection().await?;
            let count: i64 = conn.get(&key).await.unwrap_or(0);
            conn.set_ex::<_, _, ()>(
                key,
                count + 1,
                (self.config.register_limit_expire.max(1) * 60) as u64,
            )
            .await?;
        }

        self.auth_data_for_user(user_id, ip, user_agent).await
    }

    pub async fn forget(&self, input: ForgetInput) -> Result<bool, ApiError> {
        // Laravel `AuthForget` FormRequest validates email (email:strict, max 64),
        // password (min 8, max 64 — character counts) and email_code (digits:6) with 422
        // field errors before the controller body runs.
        validate_forget(&input.email, &input.password, &input.email_code)?;
        // Laravel lowercases only the cache-key email (`strtolower(trim($email))`) for
        // FORGET_REQUEST_LIMIT / EMAIL_VERIFY_CODE, but looks the user up with the raw,
        // original-case `$request->input('email')`. Registration now stores the trimmed
        // original-case email, so the reset lookup must match that case exactly.
        let email = input.email.trim();
        let cache_email = email.to_ascii_lowercase();
        let limit_key = cache_key("FORGET_REQUEST_LIMIT", &cache_email);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let count: i64 = conn.get(&limit_key).await.unwrap_or(0);
        if count >= 3 {
            return Err(ApiError::legacy("Reset failed, Please try again later"));
        }
        if self
            .verify_email_code(&cache_email, Some(input.email_code.as_str()))
            .await
            .is_err()
        {
            conn.set_ex::<_, _, ()>(&limit_key, count + 1, 300).await?;
            return Err(ApiError::legacy("Incorrect email verification code"));
        }
        let user = db::user::find_user_for_auth(&self.db, email)
            .await?
            .ok_or_else(|| ApiError::legacy("This email is not registered in the system"))?;
        let password_hash = hash_password(&input.password)?;
        let updated =
            db::user::update_password(&self.db, user.id, &password_hash, Utc::now().timestamp())
                .await?;
        if !updated {
            return Err(ApiError::legacy("Reset failed"));
        }
        conn.del::<_, ()>(cache_key("EMAIL_VERIFY_CODE", &cache_email))
            .await?;
        self.remove_all_sessions(user.id).await?;
        Ok(true)
    }

    pub async fn send_email_verify(
        &self,
        input: EmailVerifyInput,
        ip: Option<String>,
    ) -> Result<bool, ApiError> {
        // FormRequest validates `email => required|email:strict` (422) before the controller body,
        // which then runs: per-IP rate limit (429), recaptcha, whitelist/gmail, isforget, resend.
        validate_email(&input.email)?;
        // Laravel uses the raw, original-case `$email` for the whitelist check, the
        // `User::where('email', ...)` existence probe and the `SendEmailJob` recipient, and
        // only `strtolower(trim($email))` for the EMAIL_VERIFY_CODE / LAST_SEND cache keys.
        let email = input.email.trim();
        let cache_email = email.to_ascii_lowercase();

        // Laravel RateLimiter: 3 attempts per IP in a fixed 60s window, `abort(429)` on exceed
        // (CommController:33-36). INCR + expire-on-first mirrors `Cache::add` + `increment`.
        if let Some(ip) = ip.as_deref() {
            let key = cache_key("SEND_EMAIL_VERIFY_LIMIT", ip);
            let mut conn = self.redis.get_multiplexed_async_connection().await?;
            let attempts: i64 = conn.get::<_, Option<i64>>(&key).await?.unwrap_or(0);
            if attempts >= 3 {
                return Err(ApiError::too_many_requests(
                    "Too many requests, please try again later.",
                ));
            }
            let hits: i64 = conn.incr(&key, 1).await?;
            if hits == 1 {
                conn.expire::<_, ()>(&key, 60).await?;
            }
        }

        self.verify_recaptcha(input.recaptcha_data.as_deref())
            .await?;
        self.validate_register_email(email).await?;
        let exists = db::user::find_user_for_auth(&self.db, email)
            .await?
            .is_some();
        match input.isforget {
            Some(0) if exists => return Err(ApiError::legacy("This email is registered")),
            Some(1) if !exists => {
                return Err(ApiError::legacy(
                    "This email is not registered in the system",
                ));
            }
            _ => {}
        }
        let last_key = cache_key("LAST_SEND_EMAIL_VERIFY_TIMESTAMP", &cache_email);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let recently_sent: Option<i64> = conn.get(&last_key).await?;
        if recently_sent.is_some() {
            return Err(ApiError::legacy(
                "Email verification code has been sent, please request again later",
            ));
        }
        let code = six_digit_code();
        let subject = format!("{}Email verification code", self.config.app_name);
        let body = crate::mail::render_verify(
            &self.config.app_name,
            self.config.app_url.as_deref().unwrap_or_default(),
            &code,
        );
        self.send_mail(email, &subject, &body).await?;
        conn.set_ex::<_, _, ()>(cache_key("EMAIL_VERIFY_CODE", &cache_email), code, 300)
            .await?;
        conn.set_ex::<_, _, ()>(last_key, Utc::now().timestamp(), 60)
            .await?;
        Ok(true)
    }

    pub async fn passport_pv(&self, invite_code: Option<&str>) -> Result<bool, ApiError> {
        if let Some(invite_code) = invite_code.map(str::trim).filter(|value| !value.is_empty()) {
            sqlx::query("UPDATE v2_invite_code SET pv = pv + 1, updated_at = ? WHERE code = ?")
                .bind(Utc::now().timestamp())
                .bind(invite_code)
                .execute(&self.db)
                .await?;
        }
        Ok(true)
    }

    pub async fn quick_login_url(
        &self,
        user_id: i64,
        redirect: Option<&str>,
    ) -> Result<String, ApiError> {
        let code = legacy_guid(false);
        let key = cache_key("TEMP_TOKEN", &code);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        conn.set_ex::<_, _, ()>(key, user_id, 60).await?;
        Ok(self.login_redirect_url(&code, redirect))
    }

    pub async fn token_login(
        &self,
        verify: &str,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, ApiError> {
        let key = cache_key("TEMP_TOKEN", verify);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let user_id: Option<i64> = conn.get(&key).await?;
        let user_id = user_id.ok_or_else(|| ApiError::legacy("Token error"))?;
        conn.del::<_, ()>(&key).await?;
        self.auth_data_for_user(user_id, ip, user_agent).await
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

    async fn auth_data_for_user(
        &self,
        user_id: i64,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, ApiError> {
        let user = db::user::find_user_for_auth_by_id(&self.db, user_id)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
        if user.banned != 0 {
            return Err(ApiError::legacy("Your account has been suspended"));
        }
        // Laravel `generateAuthData` does not write `last_login_at`; only registration sets it
        // once (Rust seeds it in the register INSERT). Do not touch it on login/token2Login.
        let now = Utc::now().timestamp();

        let session = Uuid::new_v4().simple().to_string();
        let auth_data = self.encode_auth_data(user.id, &session)?;
        self.add_session(
            user.id,
            &session,
            SessionMeta {
                ip,
                login_at: now,
                ua: user_agent,
                auth_data: auth_data.clone(),
            },
        )
        .await?;

        Ok(AuthData {
            token: user.token,
            is_admin: user.is_admin,
            auth_data,
        })
    }

    pub async fn user_from_auth_data(&self, auth_data: &str) -> Result<AuthUser, ApiError> {
        let claims = self.decode_auth_data(auth_data)?;
        if !self.check_session(claims.id, &claims.session).await? {
            return Err(ApiError::unauthorized());
        }

        let user = db::user::find_user_for_auth_by_id(&self.db, claims.id).await?;
        let Some(user) = user else {
            return Err(ApiError::unauthorized());
        };
        Ok(AuthUser {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin,
            is_staff: user.is_staff,
        })
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
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
        if !verify_password(
            user.password_algo.as_deref(),
            user.password_salt.as_deref(),
            old_password,
            &user.password,
        ) {
            return Err(ApiError::legacy("The old password is wrong"));
        }

        let password_hash = hash_password(new_password)?;
        let updated =
            db::user::update_password(&self.db, user_id, &password_hash, Utc::now().timestamp())
                .await?;
        if !updated {
            return Err(ApiError::legacy("Save failed"));
        }
        self.remove_all_sessions(user_id).await?;
        Ok(())
    }

    pub async fn reset_security(&self, user_id: i64) -> Result<String, ApiError> {
        let uuid = Uuid::new_v4().hyphenated().to_string();
        let token = legacy_guid(false);
        let updated =
            db::user::update_security(&self.db, user_id, &uuid, &token, Utc::now().timestamp())
                .await?;
        if !updated {
            return Err(ApiError::legacy("Reset failed"));
        }
        Ok(self.config.subscribe_url_for_token(&token))
    }

    pub async fn sessions(
        &self,
        user_id: i64,
    ) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
        self.load_sessions(user_id).await
    }

    pub async fn remove_session(&self, user_id: i64, session_id: &str) -> Result<bool, ApiError> {
        let key = user_sessions_key(user_id);
        let mut sessions = self.load_sessions(user_id).await?;
        sessions.remove(session_id);
        let value = serde_json::Value::Object(sessions).to_string();
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        conn.set::<_, _, ()>(key, value).await?;
        Ok(true)
    }

    pub async fn remove_all_sessions(&self, user_id: i64) -> Result<bool, ApiError> {
        let key = user_sessions_key(user_id);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        conn.del::<_, ()>(key).await?;
        Ok(true)
    }

    fn encode_auth_data(&self, user_id: i64, session: &str) -> Result<String, ApiError> {
        let claims = AuthClaims {
            id: user_id,
            session: session.to_string(),
        };
        Ok(jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.config.app_key.as_bytes()),
        )?)
    }

    fn decode_auth_data(&self, token: &str) -> Result<AuthClaims, ApiError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.required_spec_claims.clear();
        let data = jsonwebtoken::decode::<AuthClaims>(
            token,
            &DecodingKey::from_secret(self.config.app_key.as_bytes()),
            &validation,
        )?;
        Ok(data.claims)
    }

    async fn add_session(
        &self,
        user_id: i64,
        session_id: &str,
        meta: SessionMeta,
    ) -> Result<(), ApiError> {
        let key = user_sessions_key(user_id);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let current: Option<String> = conn.get(&key).await?;
        let mut sessions: serde_json::Map<String, serde_json::Value> = current
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        sessions.insert(session_id.to_string(), serde_json::to_value(meta).unwrap());
        let value = serde_json::Value::Object(sessions).to_string();
        conn.set::<_, _, ()>(key, value).await?;
        Ok(())
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
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let current: Option<String> = conn.get(key).await?;
        Ok(current
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default())
    }

    async fn validate_register_email(&self, email: &str) -> Result<(), ApiError> {
        if self.config.email_whitelist_enable {
            let email = email.to_ascii_lowercase();
            let allowed = self.config.email_whitelist_suffix.iter().any(|suffix| {
                let suffix = suffix.trim().trim_start_matches('@').to_ascii_lowercase();
                !suffix.is_empty() && email.ends_with(&format!("@{suffix}"))
            });
            if !allowed {
                return Err(ApiError::legacy("Email suffix is not in the Whitelist"));
            }
        }
        if self.config.email_gmail_limit_enable
            && let Some(prefix) = email.split('@').next()
            && (prefix.contains('.') || prefix.contains('+'))
        {
            return Err(ApiError::legacy("Gmail alias is not supported"));
        }
        Ok(())
    }

    async fn verify_email_code(&self, email: &str, code: Option<&str>) -> Result<(), ApiError> {
        let Some(code) = code
            .map(str::trim)
            .filter(|value| value.len() == 6 && value.chars().all(|ch| ch.is_ascii_digit()))
        else {
            return Err(ApiError::legacy("Incorrect email verification code"));
        };
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let cached: Option<String> = conn.get(cache_key("EMAIL_VERIFY_CODE", email)).await?;
        if cached.as_deref() != Some(code) {
            return Err(ApiError::legacy("Incorrect email verification code"));
        }
        Ok(())
    }

    async fn verify_recaptcha(&self, token: Option<&str>) -> Result<(), ApiError> {
        if !self.config.recaptcha_enable {
            return Ok(());
        }
        let secret = self
            .config
            .recaptcha_key
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Invalid code is incorrect"))?;
        let response = token
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Invalid code is incorrect"))?;
        let request_body =
            serde_urlencoded::to_string([("secret", secret), ("response", response)])
                .map_err(|_| ApiError::legacy("Invalid code is incorrect"))?;
        let body: serde_json::Value = reqwest::Client::new()
            .post("https://www.google.com/recaptcha/api/siteverify")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(request_body)
            .send()
            .await
            .map_err(|_| ApiError::legacy("Invalid code is incorrect"))?
            .json()
            .await
            .map_err(|_| ApiError::legacy("Invalid code is incorrect"))?;
        if body
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(ApiError::legacy("Invalid code is incorrect"))
        }
    }

    async fn consume_invite_code(
        &self,
        tx: &mut Transaction<'_, MySql>,
        invite_code: Option<&str>,
    ) -> Result<Option<i64>, ApiError> {
        let Some(code) = invite_code.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(None);
        };
        let row = sqlx::query_as::<_, InviteCodeRow>(
            "SELECT id, user_id FROM v2_invite_code WHERE code = ? AND status = 0 LIMIT 1",
        )
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(row) = row else {
            if self.config.invite_force {
                return Err(ApiError::legacy("Invalid invitation code"));
            }
            return Ok(None);
        };
        if !self.config.invite_never_expire {
            sqlx::query("UPDATE v2_invite_code SET status = 1, updated_at = ? WHERE id = ?")
                .bind(Utc::now().timestamp())
                .bind(row.id)
                .execute(&mut **tx)
                .await?;
        }
        Ok(Some(row.user_id))
    }

    async fn trial_plan(&self, tx: &mut Transaction<'_, MySql>) -> Result<TrialPlan, ApiError> {
        if self.config.try_out_plan_id <= 0 {
            return Ok(TrialPlan::default());
        }
        let Some(plan) = sqlx::query_as::<_, TrialPlanRow>(
            r#"
            SELECT id, group_id, transfer_enable, device_limit, speed_limit
            FROM v2_plan
            WHERE id = ?
            LIMIT 1
            "#,
        )
        .bind(self.config.try_out_plan_id)
        .fetch_optional(&mut **tx)
        .await?
        else {
            return Ok(TrialPlan::default());
        };
        Ok(TrialPlan {
            transfer_enable: plan.transfer_enable * 1_073_741_824,
            device_limit: plan.device_limit,
            group_id: Some(plan.group_id),
            plan_id: Some(plan.id),
            speed_limit: plan.speed_limit,
            expired_at: Some(Utc::now().timestamp() + self.config.try_out_hour.max(1) * 3600),
        })
    }

    async fn send_mail(&self, to: &str, subject: &str, body: &str) -> Result<(), ApiError> {
        let settings = MailSettings::load(&self.config)?;
        let from = settings
            .from_address
            .clone()
            .or_else(|| settings.username.clone())
            .ok_or_else(|| ApiError::legacy("Email sender is not configured"))?;
        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|_| ApiError::legacy("Invalid email sender"))?,
            )
            .to(to
                .parse()
                .map_err(|_| ApiError::legacy("Invalid recipient email"))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body.to_string())
            .map_err(|error| ApiError::legacy(format!("Build mail failed: {error}")))?;
        let mut builder = if matches!(settings.encryption.as_deref(), Some("ssl" | "smtps")) {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)
        }
        .map_err(|error| ApiError::legacy(format!("SMTP config error: {error}")))?;
        if let Some(port) = settings.port {
            builder = builder.port(port);
        }
        if let (Some(username), Some(password)) = (settings.username, settings.password) {
            builder = builder.credentials(Credentials::new(username, password));
        }
        builder
            .build()
            .send(email)
            .await
            .map_err(|error| ApiError::legacy(format!("Send mail failed: {error}")))?;
        Ok(())
    }
}

async fn email_exists_for_update(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let id =
        sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE email = ? LIMIT 1 FOR UPDATE")
            .bind(email)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(id.is_some())
}

#[derive(Debug, sqlx::FromRow)]
struct InviteCodeRow {
    id: i64,
    user_id: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TrialPlanRow {
    id: i32,
    group_id: i32,
    transfer_enable: i64,
    device_limit: Option<i32>,
    speed_limit: Option<i32>,
}

#[derive(Debug, Default)]
struct TrialPlan {
    transfer_enable: i64,
    device_limit: Option<i32>,
    group_id: Option<i32>,
    plan_id: Option<i32>,
    speed_limit: Option<i32>,
    expired_at: Option<i64>,
}

struct MailSettings {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    encryption: Option<String>,
    from_address: Option<String>,
}

impl MailSettings {
    fn load(config: &AppConfig) -> Result<Self, ApiError> {
        let values = read_php_config(&config.runtime_paths.v2board_config);
        let host = config_string(&values, "email_host")
            .ok_or_else(|| ApiError::legacy("Email host is not configured"))?;
        Ok(Self {
            host,
            port: config_string(&values, "email_port").and_then(|value| value.parse().ok()),
            username: config_string(&values, "email_username"),
            password: config_string(&values, "email_password"),
            encryption: config_string(&values, "email_encryption")
                .map(|value| value.to_ascii_lowercase()),
            from_address: config_string(&values, "email_from_address"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub id: i64,
    pub email: String,
    pub is_admin: i8,
    pub is_staff: i8,
}

pub fn verify_password(
    algo: Option<&str>,
    salt: Option<&str>,
    password: &str,
    stored_hash: &str,
) -> bool {
    match algo {
        Some("md5") => format!("{:x}", md5::compute(password)) == stored_hash,
        Some("sha256") => {
            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            hex::encode(hasher.finalize()) == stored_hash
        }
        Some("md5salt") => {
            let salt = salt.unwrap_or_default();
            format!("{:x}", md5::compute(format!("{password}{salt}"))) == stored_hash
        }
        _ => verify_modern_password(password, stored_hash),
    }
}

fn verify_modern_password(password: &str, stored_hash: &str) -> bool {
    if stored_hash.starts_with("$argon2") {
        let Ok(parsed) = PasswordHash::new(stored_hash) else {
            return false;
        };
        return Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok();
    }

    let bcrypt_hash = stored_hash
        .strip_prefix("$2y$")
        .map(|rest| format!("$2b${rest}"))
        .unwrap_or_else(|| stored_hash.to_string());
    bcrypt::verify(password, &bcrypt_hash).unwrap_or(false)
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|error| ApiError::internal(format!("password hash error: {error}")))
}

/// Laravel `AuthRegister`/`CommSendEmailVerify` validate `email => required|email:strict`.
/// The FormRequest fires before the controller body, returning HTTP 422 with the field message.
fn validate_email(email: &str) -> Result<(), ApiError> {
    let email = email.trim();
    if email.is_empty() {
        return Err(ApiError::validation_field(
            "email",
            "Email can not be empty",
        ));
    }
    if !is_valid_email(email) {
        return Err(ApiError::validation_field(
            "email",
            "Email format is incorrect",
        ));
    }
    Ok(())
}

/// Laravel `AuthRegister` validates `password => required|min:8` (character count, not bytes).
fn validate_password(password: &str) -> Result<(), ApiError> {
    if password.is_empty() {
        return Err(ApiError::validation_field(
            "password",
            "Password can not be empty",
        ));
    }
    if password.chars().count() < 8 {
        return Err(ApiError::validation_field(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    Ok(())
}

/// Laravel `AuthForget` validates email (`required|string|email:strict|max:64`),
/// password (`required|string|min:8|max:64`) and email_code (`required|string|digits:6`).
/// Lengths are character counts (`mb_strlen`), not bytes. Fires before the controller body,
/// returning HTTP 422 with the field message.
fn validate_forget(email: &str, password: &str, email_code: &str) -> Result<(), ApiError> {
    validate_email(email)?;
    if email.trim().chars().count() > 64 {
        return Err(ApiError::validation_field(
            "email",
            "Email format is incorrect",
        ));
    }
    validate_password(password)?;
    if password.chars().count() > 64 {
        return Err(ApiError::validation_field(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    if email_code.trim().is_empty() {
        return Err(ApiError::validation_field(
            "email_code",
            "Email verification code cannot be empty",
        ));
    }
    if email_code.chars().count() != 6 || !email_code.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(ApiError::validation_field(
            "email_code",
            "Incorrect email verification code",
        ));
    }
    Ok(())
}

/// Laravel `UserChangePassword` validates old_password (`required`) and new_password
/// (`required|min:8`, character count not bytes). The FormRequest fires before the
/// controller body, returning HTTP 422 with the field message.
fn validate_change_password(old_password: &str, new_password: &str) -> Result<(), ApiError> {
    if old_password.is_empty() {
        return Err(ApiError::validation_field(
            "old_password",
            "Old password cannot be empty",
        ));
    }
    if new_password.is_empty() {
        return Err(ApiError::validation_field(
            "new_password",
            "New password cannot be empty",
        ));
    }
    if new_password.chars().count() < 8 {
        return Err(ApiError::validation_field(
            "new_password",
            "Password must be greater than 8 digits",
        ));
    }
    Ok(())
}

/// Structural `local@host` check — the practical subset of `email:strict` that avoids
/// false-rejecting any address Laravel's RFC validator would accept in real registrations.
fn is_valid_email(email: &str) -> bool {
    if email.chars().any(char::is_whitespace) {
        return false;
    }
    match email.split_once('@') {
        Some((local, host)) => !local.is_empty() && !host.is_empty() && !host.contains('@'),
        None => false,
    }
}

fn legacy_guid(format: bool) -> String {
    let uuid = Uuid::new_v4();
    if format {
        return uuid.hyphenated().to_string();
    }
    format!(
        "{:x}",
        md5::compute(format!("{}-{}", uuid.hyphenated(), Utc::now().timestamp()))
    )
}

// Laravel's `CacheKey::get()` returns the bare `KEY_unique` name; the Redis/cache
// key prefix (`REDIS_PREFIX` + `CACHE_PREFIX`) is applied by Laravel's cache layer, not
// here. The Rust backend reads and writes these keys consistently WITHOUT that prefix,
// which is intentional: the cutover is a cold switch where forced re-authentication is
// acceptable, so Rust does not need to read Laravel's still-live prefixed session/JWT
// entries. If a shared-Redis hot migration is ever required, prefix every key here with
// Laravel's effective prefix (or run Laravel with CACHE_PREFIX='' / REDIS_PREFIX='').
fn user_sessions_key(user_id: i64) -> String {
    format!("USER_SESSIONS_{user_id}")
}

fn cache_key(key: &str, unique: &str) -> String {
    format!("{key}_{unique}")
}

fn six_digit_code() -> String {
    let bytes = *Uuid::new_v4().as_bytes();
    let number = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 900_000 + 100_000;
    number.to_string()
}

fn read_php_config(path: impl AsRef<Path>) -> std::collections::HashMap<String, String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return std::collections::HashMap::new();
    };
    let mut values = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with('\'') || !line.contains("=>") {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once("=>") else {
            continue;
        };
        let key = raw_key.trim().trim_matches('\'');
        if key.is_empty() {
            continue;
        }
        if let Some(value) = parse_php_scalar(raw_value.trim().trim_end_matches(',')) {
            values.insert(key.to_string(), value);
        }
    }
    values
}

fn config_string(values: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    values
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

fn parse_php_scalar(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("null") {
        return None;
    }
    if raw.eq_ignore_ascii_case("true") {
        return Some("1".to_string());
    }
    if raw.eq_ignore_ascii_case("false") {
        return Some("0".to_string());
    }
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return Some(
            raw.trim_matches('\'')
                .replace("\\'", "'")
                .replace("\\\\", "\\"),
        );
    }
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return Some(
            raw.trim_matches('"')
                .replace("\\\"", "\"")
                .replace("\\\\", "\\"),
        );
    }
    raw.parse::<i64>().ok().map(|_| raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_email_accepts_structural_addresses_and_rejects_malformed() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("user@localhost"));
        assert!(!is_valid_email("notanemail"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("a@b@c"));
        assert!(!is_valid_email("user name@example.com"));
    }

    #[test]
    fn validate_email_reports_validation_error_with_laravel_messages() {
        assert!(validate_email("user@example.com").is_ok());
        let empty = validate_email("   ").unwrap_err();
        assert_eq!(empty.to_string(), "Email can not be empty");
        assert!(matches!(empty, ApiError::Validation { .. }));
        let malformed = validate_email("bad").unwrap_err();
        assert_eq!(malformed.to_string(), "Email format is incorrect");
        assert!(matches!(malformed, ApiError::Validation { .. }));
    }

    #[test]
    fn validate_password_counts_characters_not_bytes() {
        assert!(validate_password("password").is_ok());
        // Six multibyte characters (18 bytes): a byte-length check would pass, char count fails.
        assert_eq!(
            validate_password("七个中文密码").unwrap_err().to_string(),
            "Password must be greater than 8 digits"
        );
        assert_eq!(
            validate_password("").unwrap_err().to_string(),
            "Password can not be empty"
        );
    }

    #[test]
    fn validate_forget_mirrors_authforget_rules() {
        assert!(validate_forget("user@example.com", "password", "123456").is_ok());

        // email: required -> format -> max:64 (character count)
        let empty_email = validate_forget("", "password", "123456").unwrap_err();
        assert_eq!(empty_email.to_string(), "Email can not be empty");
        assert!(matches!(empty_email, ApiError::Validation { .. }));
        assert_eq!(
            validate_forget("bad", "password", "123456")
                .unwrap_err()
                .to_string(),
            "Email format is incorrect"
        );
        let long_email = format!("{}@example.com", "a".repeat(60)); // 72 chars > 64
        assert_eq!(
            validate_forget(&long_email, "password", "123456")
                .unwrap_err()
                .to_string(),
            "Email format is incorrect"
        );

        // password: min:8 and max:64 are character counts, not bytes
        assert_eq!(
            validate_forget("user@example.com", "七个中文密码", "123456")
                .unwrap_err()
                .to_string(),
            "Password must be greater than 8 digits"
        );
        assert_eq!(
            validate_forget("user@example.com", &"a".repeat(65), "123456")
                .unwrap_err()
                .to_string(),
            "Password must be greater than 8 digits"
        );

        // email_code: required (distinct message) then digits:6
        let empty_code = validate_forget("user@example.com", "password", "  ").unwrap_err();
        assert_eq!(
            empty_code.to_string(),
            "Email verification code cannot be empty"
        );
        assert!(matches!(empty_code, ApiError::Validation { .. }));
        assert_eq!(
            validate_forget("user@example.com", "password", "12345")
                .unwrap_err()
                .to_string(),
            "Incorrect email verification code"
        );
        assert_eq!(
            validate_forget("user@example.com", "password", "12345a")
                .unwrap_err()
                .to_string(),
            "Incorrect email verification code"
        );
    }

    #[test]
    fn validate_change_password_mirrors_userchangepassword_rules() {
        assert!(validate_change_password("old-secret", "new-secret").is_ok());

        // old_password required takes precedence over new_password rules.
        let empty_old = validate_change_password("", "short").unwrap_err();
        assert_eq!(empty_old.to_string(), "Old password cannot be empty");
        assert!(matches!(empty_old, ApiError::Validation { .. }));

        // new_password required reports its own message, not the min message.
        assert_eq!(
            validate_change_password("old-secret", "")
                .unwrap_err()
                .to_string(),
            "New password cannot be empty"
        );

        // min:8 counts characters (mb_strlen), so a 6-glyph multibyte password fails.
        assert_eq!(
            validate_change_password("old-secret", "七个中文密码")
                .unwrap_err()
                .to_string(),
            "Password must be greater than 8 digits"
        );
    }
}
