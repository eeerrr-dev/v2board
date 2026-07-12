use chrono::Utc;
use redis::AsyncCommands;
use serde::Deserialize;
use sqlx::{MySql, Transaction};
use v2board_compat::ApiError;
use v2board_config::duration_minutes_to_seconds;

use super::validation::{validate_email, validate_password};
use super::{AuthData, AuthService, cache_key, legacy_guid};

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub invite_code: Option<String>,
    pub email_code: Option<String>,
    pub recaptcha_data: Option<String>,
}

impl AuthService {
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
            let mut conn = self.redis.clone();
            let count = conn.get::<_, Option<i64>>(&key).await?.unwrap_or(0);
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
        if self.config.email_verify
            && !self
                .consume_email_code(&cache_email, input.email_code.as_deref())
                .await?
        {
            return Err(ApiError::legacy("Incorrect email verification code"));
        }

        let password_hash = self.password_kdf.hash(&input.password).await?;
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

        if let Some(ip) = ip.as_deref()
            && self.config.register_limit_by_ip_enable
        {
            let key = cache_key("REGISTER_IP_RATE_LIMIT", ip);
            if let Err(error) = self
                .increment_counter_with_ttl(
                    &key,
                    duration_minutes_to_seconds(self.config.register_limit_expire),
                )
                .await
            {
                tracing::warn!(
                    ?error,
                    user_id,
                    ip,
                    "registration IP limiter update failed after committed account creation"
                );
            }
        }

        self.auth_data_for_user(user_id, Some(0), ip, user_agent)
            .await
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
        let now = Utc::now().timestamp();
        Ok(TrialPlan {
            transfer_enable: checked_trial_transfer_bytes(plan.transfer_enable)?,
            device_limit: plan.device_limit,
            group_id: Some(plan.group_id),
            plan_id: Some(plan.id),
            speed_limit: plan.speed_limit,
            expired_at: Some(checked_trial_expired_at(now, self.config.try_out_hour)?),
        })
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

pub(super) fn checked_trial_transfer_bytes(transfer_gib: i64) -> Result<i64, ApiError> {
    if transfer_gib < 0 {
        return Err(ApiError::legacy(
            "Trial plan traffic allowance must not be negative",
        ));
    }
    transfer_gib
        .checked_mul(1_073_741_824)
        .ok_or_else(|| ApiError::legacy("Trial plan traffic allowance exceeds the supported range"))
}

pub(super) fn checked_trial_expired_at(now: i64, try_out_hour: i64) -> Result<i64, ApiError> {
    if try_out_hour < 0 {
        return Err(ApiError::legacy("Trial plan duration must not be negative"));
    }
    let seconds = try_out_hour
        .max(1)
        .checked_mul(3_600)
        .ok_or_else(|| ApiError::legacy("Trial plan duration exceeds the supported range"))?;
    now.checked_add(seconds)
        .ok_or_else(|| ApiError::legacy("Trial plan expiry exceeds the supported range"))
}
