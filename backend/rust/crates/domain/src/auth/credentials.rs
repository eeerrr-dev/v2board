use chrono::Utc;
use redis::AsyncCommands;
use serde::Deserialize;
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::duration_minutes_to_seconds;
use v2board_db as db;

use super::password::password_needs_rehash;
use super::validation::{
    validate_change_password, validate_email, validate_forget, validate_password,
};
use super::{AuthData, AuthService, cache_key, legacy_guid};

#[derive(Debug, Clone, Deserialize)]
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

        let password_error_limit = if self.config.password_limit_enable {
            let key = cache_key("PASSWORD_ERROR_LIMIT", email);
            let mut conn = self.redis.clone();
            let count = conn.get::<_, Option<i64>>(&key).await?.unwrap_or(0);
            if count >= self.config.password_limit_count {
                return Err(ApiError::legacy(format!(
                    "There are too many password errors, please try again after {} minutes.",
                    self.config.password_limit_expire
                )));
            }
            Some(key)
        } else {
            None
        };

        let user = db::user::find_user_for_auth(&self.db, email)
            .await?
            .ok_or_else(|| ApiError::legacy("Incorrect email or password"))?;

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
            if let Some(key) = password_error_limit {
                self.increment_counter_with_ttl(
                    &key,
                    duration_minutes_to_seconds(self.config.password_limit_expire),
                )
                .await?;
            }
            return Err(ApiError::legacy("Incorrect email or password"));
        }

        if user.banned != 0 {
            return Err(ApiError::legacy("Your account has been suspended"));
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

        self.auth_data_for_user(user.id, Some(user.session_epoch), ip, user_agent)
            .await
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
        let mut conn = self.redis.clone();
        let count = conn.get::<_, Option<i64>>(&limit_key).await?.unwrap_or(0);
        if count >= 3 {
            return Err(ApiError::legacy("Reset failed, Please try again later"));
        }
        if !self
            .consume_email_code(&cache_email, Some(input.email_code.as_str()))
            .await?
        {
            self.increment_counter_with_ttl(&limit_key, 300).await?;
            return Err(ApiError::legacy("Incorrect email verification code"));
        }
        let user = db::user::find_user_for_auth(&self.db, email)
            .await?
            .ok_or_else(|| ApiError::legacy("This email is not registered in the system"))?;
        let password_hash = self.password_kdf.hash(&input.password).await?;
        let updated =
            db::user::update_password(&self.db, user.id, &password_hash, Utc::now().timestamp())
                .await?;
        if !updated {
            return Err(ApiError::legacy("Reset failed"));
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
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
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
            return Err(ApiError::legacy("The old password is wrong"));
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
            return Err(ApiError::legacy("Save failed"));
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
            return Err(ApiError::legacy("Reset failed"));
        }
        Ok(self.config.subscribe_url_for_token(&token))
    }
}
