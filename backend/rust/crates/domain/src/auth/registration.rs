use chrono::Utc;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::Deserialize;
use v2board_compat::ApiError;
use v2board_config::duration_minutes_to_seconds;
use v2board_db::DbTransaction;

use super::validation::{validate_email, validate_password};
use super::{AuthData, AuthService, cache_key, legacy_guid};

pub(super) const MAX_INVITE_CODE_BYTES: usize = 255;
pub(super) const MAX_EMAIL_CODE_BYTES: usize = 64;
pub(super) const MAX_RECAPTCHA_DATA_BYTES: usize = 4096;

pub(super) const RESERVE_REGISTRATION_SLOT_SCRIPT: &str = r#"
local now = tonumber(ARGV[1])
local expires_at = tonumber(ARGV[2])
local limit = tonumber(ARGV[3])
local token = ARGV[4]

redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', now)
if redis.call('ZCARD', KEYS[1]) >= limit then
    return 0
end

redis.call('ZADD', KEYS[1], 'NX', expires_at, token)
redis.call('EXPIREAT', KEYS[1], expires_at)
return 1
"#;

pub(super) const RELEASE_REGISTRATION_SLOT_SCRIPT: &str = r#"
local removed = redis.call('ZREM', KEYS[1], ARGV[1])
if redis.call('ZCARD', KEYS[1]) == 0 then
    redis.call('DEL', KEYS[1])
end
return removed
"#;

struct RegistrationLimitReservation {
    key: String,
    token: String,
}

#[derive(Clone, Deserialize)]
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
        validate_registration_auxiliary_inputs(&input)?;

        let reservation = self.reserve_registration_slot(ip.as_deref()).await?;
        let registration: Result<i64, ApiError> = async {
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

            // Do not next-key lock a missing email before the shared invite
            // row. Concurrent registrations for different addresses otherwise
            // hold compatible email gaps, then one waits on the invite while
            // the invite holder waits for the other's insert intention (a deadlock).
            // The unique email index is the authoritative race-free check; an
            // insert conflict rolls this transaction's invite consumption back.
            let invite_user_id = self
                .consume_invite_code(&mut tx, input.invite_code.as_deref())
                .await?;
            let trial = self.trial_plan(&mut tx).await?;

            let user_id = sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO users (
                    invite_user_id, email, password, uuid, token, transfer_enable, device_limit,
                    group_id, plan_id, speed_limit, expired_at, last_login_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                RETURNING id
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
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| {
                if is_email_unique_violation(&error) {
                    ApiError::legacy("Email already exists")
                } else {
                    ApiError::Database(error)
                }
            })?;
            tx.commit().await?;
            Ok(user_id)
        }
        .await;
        let user_id = match registration {
            Ok(user_id) => user_id,
            Err(error) => {
                if let Some(reservation) = reservation.as_ref() {
                    self.release_registration_slot(reservation).await;
                }
                return Err(error);
            }
        };

        self.auth_data_for_user(user_id, Some(0), ip, user_agent, true)
            .await
    }

    pub async fn passport_pv(&self, invite_code: Option<&str>) -> Result<bool, ApiError> {
        if let Some(invite_code) = invite_code.map(str::trim).filter(|value| !value.is_empty()) {
            if invite_code.len() > MAX_INVITE_CODE_BYTES {
                return Err(ApiError::validation_field(
                    "invite_code",
                    "Invalid invitation code",
                ));
            }
            sqlx::query(
                "UPDATE invite_code SET pv = pv + 1, updated_at = $1 \
                 WHERE lower(code) = lower($2)",
            )
            .bind(Utc::now().timestamp())
            .bind(invite_code)
            .execute(&self.db)
            .await?;
        }
        Ok(true)
    }

    async fn consume_invite_code(
        &self,
        tx: &mut DbTransaction<'_>,
        invite_code: Option<&str>,
    ) -> Result<Option<i64>, ApiError> {
        let Some(code) = invite_code.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(None);
        };
        let row = sqlx::query_as::<_, InviteCodeRow>(
            "SELECT id, user_id FROM invite_code \
             WHERE lower(code) = lower($1) AND status = 0 LIMIT 1 FOR UPDATE",
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
            let result = sqlx::query(
                "UPDATE invite_code SET status = 1, updated_at = $1 WHERE id = $2 AND status = 0",
            )
            .bind(Utc::now().timestamp())
            .bind(row.id)
            .execute(&mut **tx)
            .await?;
            if result.rows_affected() != 1 {
                return Err(ApiError::legacy("Invalid invitation code"));
            }
        }
        Ok(Some(row.user_id))
    }

    async fn reserve_registration_slot(
        &self,
        ip: Option<&str>,
    ) -> Result<Option<RegistrationLimitReservation>, ApiError> {
        let Some(ip) = ip.filter(|_| self.config.register_limit_by_ip_enable) else {
            return Ok(None);
        };
        let key = cache_key("REGISTER_IP_RATE_LIMIT_V2", ip);
        let token = legacy_guid(false);
        let now = Utc::now().timestamp();
        let ttl = i64::try_from(duration_minutes_to_seconds(
            self.config.register_limit_expire,
        ))
        .unwrap_or(i64::MAX)
        .max(1);
        let expires_at = now.saturating_add(ttl);
        let mut conn = self.redis.clone();
        let reserved = redis::Script::new(RESERVE_REGISTRATION_SLOT_SCRIPT)
            .key(&key)
            .arg(now)
            .arg(expires_at)
            .arg(self.config.register_limit_count)
            .arg(&token)
            .invoke_async::<i64>(&mut conn)
            .await?;
        if reserved != 1 {
            return Err(ApiError::legacy(format!(
                "Register frequently, please try again after {} minute",
                self.config.register_limit_expire
            )));
        }
        Ok(Some(RegistrationLimitReservation { key, token }))
    }

    async fn release_registration_slot(&self, reservation: &RegistrationLimitReservation) {
        let mut conn = self.redis.clone();
        if let Err(error) = redis::Script::new(RELEASE_REGISTRATION_SLOT_SCRIPT)
            .key(&reservation.key)
            .arg(&reservation.token)
            .invoke_async::<i64>(&mut conn)
            .await
        {
            tracing::warn!(
                ?error,
                key = %reservation.key,
                "failed to release registration IP limiter reservation"
            );
        }
    }

    async fn trial_plan(&self, tx: &mut DbTransaction<'_>) -> Result<TrialPlan, ApiError> {
        if self.config.try_out_plan_id <= 0 {
            return Ok(TrialPlan::default());
        }
        let Some(plan) = sqlx::query_as::<_, TrialPlanRow>(
            r#"
            SELECT id, group_id, transfer_enable, device_limit, speed_limit
            FROM plan
            WHERE id = $1
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

pub(super) fn validate_registration_auxiliary_inputs(
    input: &RegisterInput,
) -> Result<(), ApiError> {
    if input
        .invite_code
        .as_deref()
        .is_some_and(|value| value.len() > MAX_INVITE_CODE_BYTES)
    {
        return Err(ApiError::validation_field(
            "invite_code",
            "Invalid invitation code",
        ));
    }
    if input
        .email_code
        .as_deref()
        .is_some_and(|value| value.len() > MAX_EMAIL_CODE_BYTES)
    {
        return Err(ApiError::validation_field(
            "email_code",
            "Incorrect email verification code",
        ));
    }
    if input
        .recaptcha_data
        .as_deref()
        .is_some_and(|value| value.len() > MAX_RECAPTCHA_DATA_BYTES)
    {
        return Err(ApiError::validation_field(
            "recaptcha_data",
            "Invalid code is incorrect",
        ));
    }
    Ok(())
}

fn is_email_unique_violation(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|error| {
        error.is_unique_violation()
            && matches!(
                error.constraint(),
                Some("uniq_user_email" | "uniq_user_email_canonical")
            )
    })
}

#[derive(Debug, sqlx::FromRow)]
struct InviteCodeRow {
    id: i32,
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

pub(super) fn checked_trial_expired_at(now: i64, try_out_hour: Decimal) -> Result<i64, ApiError> {
    if try_out_hour.is_sign_negative() {
        return Err(ApiError::legacy("Trial plan duration must not be negative"));
    }
    let hours = if try_out_hour.is_zero() {
        Decimal::ONE
    } else {
        try_out_hour
    };
    let seconds = hours
        .checked_mul(Decimal::from(3_600))
        .ok_or_else(|| ApiError::legacy("Trial plan duration exceeds the supported range"))?
        .trunc()
        .to_i64()
        .ok_or_else(|| ApiError::legacy("Trial plan duration exceeds the supported range"))?;
    now.checked_add(seconds)
        .ok_or_else(|| ApiError::legacy("Trial plan expiry exceeds the supported range"))
}
