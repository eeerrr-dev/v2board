use axum::{Json, extract::State, http::HeaderMap};
use chrono::{Datelike, Duration, TimeZone, Utc};
use redis::AsyncCommands;
use serde::Serialize;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};
use v2board_config::{AppConfig, app_now, app_timezone, duration_minutes_to_seconds};
use v2board_domain::subscribe_link::{hmac_sha1_hex, totp_counter_bytes};

use crate::{
    auth::require_user, codec::base64_decode_url_safe, runtime::AppState, validation::forbidden,
};

#[derive(Debug, Serialize)]
pub(crate) struct SubscribeInfo {
    pub(crate) plan_id: Option<i32>,
    pub(crate) token: String,
    pub(crate) expired_at: Option<i64>,
    pub(crate) u: i64,
    pub(crate) d: i64,
    pub(crate) transfer_enable: i64,
    pub(crate) device_limit: Option<i32>,
    pub(crate) email: String,
    pub(crate) uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) plan: Option<v2board_db::plan::PlanRow>,
    pub(crate) alive_ip: i64,
    pub(crate) subscribe_url: String,
    pub(crate) reset_day: Option<i64>,
    pub(crate) allow_new_period: i32,
}

pub(crate) async fn user_subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<SubscribeInfo>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let subscribe = v2board_db::user::find_user_subscribe(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::business("The user does not exist"))?;
    let plan = match subscribe.plan_id {
        Some(plan_id) => Some(
            v2board_db::plan::find_plan(&state.db, plan_id)
                .await?
                .ok_or_else(|| ApiError::business("Subscription plan does not exist"))?,
        ),
        None => None,
    };
    let alive_ip = alive_ip(&state, user.id).await?;
    let config = state.config_snapshot();
    let reset_day = reset_day(subscribe.expired_at, plan.as_ref(), &config);
    let subscribe_url = subscribe_url_for_user(&state, user.id, &subscribe.token).await?;

    Ok(legacy_data(SubscribeInfo {
        plan_id: subscribe.plan_id,
        token: subscribe.token,
        expired_at: subscribe.expired_at,
        u: subscribe.u,
        d: subscribe.d,
        transfer_enable: subscribe.transfer_enable,
        device_limit: subscribe.device_limit,
        email: subscribe.email,
        uuid: subscribe.uuid,
        plan,
        alive_ip,
        subscribe_url,
        reset_day,
        allow_new_period: config.allow_new_period,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct UserPeriodRow {
    plan_id: Option<i32>,
    transfer_enable: i64,
    u: i64,
    d: i64,
    expired_at: Option<i64>,
}

pub(crate) async fn user_new_period(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let config = state.config_snapshot();
    if config.allow_new_period == 0 {
        return Err(ApiError::business("Renewal is not allowed"));
    }
    let mut tx = state.db.begin().await?;
    let row = sqlx::query_as::<_, UserPeriodRow>(
        r#"
        SELECT u.plan_id, u.transfer_enable, u.u, u.d, u.expired_at
        FROM users u
        WHERE u.id = $1
        LIMIT 1
        FOR UPDATE OF u
        "#,
    )
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::business("The user does not exist"))?;
    let used = row
        .u
        .checked_add(row.d)
        .ok_or_else(|| ApiError::internal("user traffic exceeds the supported range"))?;
    if row.transfer_enable > used {
        return Err(ApiError::business(
            "You have not used up your traffic, you cannot renew your subscription",
        ));
    }
    // A plan-less user cannot renew: both getResetDay and getResetPeriod return null
    // at `if ($user->plan_id === NULL) return null;`, and UserController::newPeriod
    // turns either null into abort(500, 'You do not allow to renew the subscription').
    let plan_id = row
        .plan_id
        .ok_or_else(|| ApiError::business("You do not allow to renew the subscription"))?;
    // PostgreSQL cannot lock the nullable side of an outer join. Preserve the
    // subscription writer lock order explicitly: user first, then the existing
    // plan whose reset method controls this mutation.
    let plan_reset_method = sqlx::query_scalar::<_, Option<i16>>(
        "SELECT reset_traffic_method FROM plan WHERE id = $1 FOR SHARE",
    )
    .bind(plan_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    let expired_at = row
        .expired_at
        .filter(|expired_at| *expired_at > Utc::now().timestamp())
        .ok_or_else(|| ApiError::business("You do not allow to renew the subscription"))?;
    let mut reset_day = reset_day_by_method(expired_at, plan_reset_method, &config)
        .ok_or_else(|| ApiError::business("You do not allow to renew the subscription"))?;
    let mut period = reset_period_by_method(plan_reset_method, &config)
        .ok_or_else(|| ApiError::business("You do not allow to renew the subscription"))?;
    match period {
        1 => {
            reset_day = 30;
            period = 30;
        }
        30 => {}
        12 => {
            reset_day = 365;
            period = 365;
        }
        365 => {}
        _ => return Err(ApiError::business("Invalid reset period")),
    }
    if reset_day <= 0 {
        reset_day = period;
    }
    if let Some(next_expired_at) =
        checked_reset_subscription_expiry(expired_at, reset_day, period, Utc::now().timestamp())?
    {
        let updated = sqlx::query(
            "UPDATE users SET expired_at = $1, traffic_epoch = traffic_epoch + 1, \
             u = 0, d = 0, updated_at = $2 WHERE id = $3",
        )
        .bind(next_expired_at)
        .bind(Utc::now().timestamp())
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ApiError::internal(
                "subscription period update lost its user row",
            ));
        }
        tx.commit().await?;
        Ok(legacy_data(true))
    } else {
        Err(ApiError::business(
            "You do not have enough time to renew your subscription",
        ))
    }
}

pub(super) fn checked_reset_subscription_expiry(
    expired_at: i64,
    reset_day: i64,
    period: i64,
    now: i64,
) -> Result<Option<i64>, ApiError> {
    if reset_day < 0 || period < 0 {
        return Err(ApiError::business("Invalid reset period"));
    }
    let threshold = period
        .checked_add(1)
        .and_then(|days| days.checked_mul(86_400))
        .ok_or_else(|| ApiError::business("Reset period exceeds the supported range"))?;
    let remaining = expired_at
        .checked_sub(now)
        .ok_or_else(|| ApiError::business("Subscription expiry exceeds the supported range"))?;
    if threshold >= remaining {
        return Ok(None);
    }
    let reset_seconds = reset_day
        .checked_mul(86_400)
        .ok_or_else(|| ApiError::business("Reset period exceeds the supported range"))?;
    expired_at
        .checked_sub(reset_seconds)
        .map(Some)
        .ok_or_else(|| ApiError::business("Subscription expiry exceeds the supported range"))
}

pub(crate) async fn resolve_subscribe_token(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    match state.config_snapshot().show_subscribe_method {
        0 => Ok(token.to_string()),
        1 => resolve_one_time_subscribe_token(state, token).await,
        2 => resolve_totp_subscribe_token(state, token).await,
        _ => Ok(token.to_string()),
    }
}

async fn resolve_one_time_subscribe_token(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    let mut conn = state.auth_redis.clone();
    redis::Script::new(CONSUME_SUBSCRIBE_TOKEN_SCRIPT)
        .key(state.redis_key(&format!("otpn_{token}")))
        .arg(state.redis_key("otp_"))
        .invoke_async::<Option<String>>(&mut conn)
        .await?
        .ok_or_else(|| forbidden("token is error"))
}

pub(crate) async fn resolve_totp_subscribe_token(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    let cache_key = state.redis_key(&format!("totp_{token}"));
    let mut conn = state.auth_redis.clone();
    if let Some(user_token) = conn.get::<_, Option<String>>(&cache_key).await? {
        return Ok(user_token);
    }

    let decoded = base64_decode_url_safe(token).ok_or_else(|| forbidden("token is error"))?;
    let decoded = String::from_utf8(decoded).map_err(|_| forbidden("token is error"))?;
    let (user_id, client_hash) = decoded
        .split_once(':')
        .ok_or_else(|| forbidden("token is error"))?;
    if user_id.is_empty() || client_hash.is_empty() {
        return Err(forbidden("token is error"));
    }
    let user_id = user_id
        .parse::<i64>()
        .map_err(|_| forbidden("token is error"))?;
    let user = v2board_db::user::find_user_access(&state.db, user_id)
        .await?
        .ok_or_else(|| forbidden("token is error"))?;

    let config = state.config_snapshot();
    let timestep = duration_minutes_to_seconds(config.show_subscribe_expire);
    let expected = hmac_sha1_hex(user.token.as_bytes(), &totp_counter_bytes(&config))?;
    if client_hash != expected {
        return Err(forbidden("token is error"));
    }

    let _: () = conn.set_ex(cache_key, &user.token, timestep).await?;
    Ok(user.token)
}

/// Mirror `Helper::getSubscribeUrl` through the shared domain minter
/// (`v2board_domain::subscribe_link`): derive the method-specific token so the
/// generated URL resolves back through [`resolve_subscribe_token`].
pub(crate) async fn subscribe_url_for_user(
    state: &AppState,
    user_id: i64,
    token: &str,
) -> Result<String, ApiError> {
    let config = state.config_snapshot();
    v2board_domain::subscribe_link::subscribe_url_for_user(
        &config,
        &state.redis_keys,
        &mut Some(state.auth_redis.clone()),
        user_id,
        token,
    )
    .await
}

/// Consume side of the method-1 one-time token pair; the mint side is
/// `v2board_domain::subscribe_link::MINT_SUBSCRIBE_TOKEN_SCRIPT`.
const CONSUME_SUBSCRIBE_TOKEN_SCRIPT: &str = r#"
local user_token = redis.call('GET', KEYS[1])
if not user_token then
    return false
end
redis.call('DEL', KEYS[1])
redis.call('DEL', ARGV[1] .. user_token)
return user_token
"#;

async fn alive_ip(state: &AppState, user_id: i64) -> Result<i64, ApiError> {
    let key = state.redis_key(&format!("ALIVE_IP_USER_{user_id}"));
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let current: Option<String> = conn.get(key).await?;
    let Some(current) = current else {
        return Ok(0);
    };
    Ok(serde_json::from_str::<serde_json::Value>(&current)
        .ok()
        .and_then(|value| value.get("alive_ip").and_then(|alive| alive.as_i64()))
        .unwrap_or(0))
}

pub(crate) fn reset_day(
    expired_at: Option<i64>,
    plan: Option<&v2board_db::plan::PlanRow>,
    config: &AppConfig,
) -> Option<i64> {
    let expired_at = expired_at?;
    // A plan-less user has no reset schedule: UserService::getResetDay returns null
    // at `if ($user->plan_id === NULL) return null;` before any method lookup, so a
    // None plan must NOT fall back to the config default. A resolved plan whose own
    // reset_traffic_method is NULL still uses the config default (the `unwrap_or`
    // below), mirroring the `=== NULL` switch arm.
    let plan = plan?;
    if expired_at <= Utc::now().timestamp() {
        return None;
    }
    let method = plan
        .reset_traffic_method
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method);

    match method {
        0 => Some(reset_day_by_month_first_day()),
        1 => Some(reset_day_by_expire_day(expired_at)),
        2 => None,
        3 => days_until_year_first_day(),
        4 => days_until_year_expire_day(expired_at),
        _ => None,
    }
}

fn reset_day_by_method(
    expired_at: i64,
    plan_reset_method: Option<i16>,
    config: &AppConfig,
) -> Option<i64> {
    if expired_at <= Utc::now().timestamp() {
        return None;
    }
    match plan_reset_method
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method)
    {
        0 => Some(reset_day_by_month_first_day()),
        1 => Some(reset_day_by_expire_day(expired_at)),
        2 => None,
        3 => days_until_year_first_day(),
        4 => days_until_year_expire_day(expired_at),
        _ => None,
    }
}

fn reset_period_by_method(plan_reset_method: Option<i16>, config: &AppConfig) -> Option<i64> {
    match plan_reset_method
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method)
    {
        0 => Some(1),
        1 => Some(30),
        2 => None,
        3 => Some(12),
        4 => Some(365),
        _ => None,
    }
}

pub(super) fn reset_day_by_month_first_day() -> i64 {
    let today = app_now().date_naive();
    i64::from(last_day_of_current_month() - today.day())
}

fn reset_day_by_expire_day(expired_at: i64) -> i64 {
    let today = app_now().date_naive();
    let expire_day = app_timezone()
        .timestamp_opt(expired_at, 0)
        .single()
        .map(|date| date.day())
        .unwrap_or(today.day());
    let today_day = today.day();
    let last_day = last_day_of_current_month();

    if expire_day >= today_day && expire_day >= last_day {
        return i64::from(last_day - today_day);
    }
    if expire_day >= today_day {
        return i64::from(expire_day - today_day);
    }
    i64::from(last_day - today_day + expire_day)
}

fn days_until_year_first_day() -> Option<i64> {
    let now = app_now();
    let next_year = app_timezone()
        .with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0)
        .single()?;
    Some((next_year.timestamp() - now.timestamp()) / 86_400)
}

fn days_until_year_expire_day(expired_at: i64) -> Option<i64> {
    let now = app_now();
    let timezone = app_timezone();
    let expired = timezone.timestamp_opt(expired_at, 0).single()?;
    let this_year = timezone
        .with_ymd_and_hms(now.year(), expired.month(), expired.day(), 0, 0, 0)
        .single();
    let target = match this_year {
        Some(target) if target > now => target,
        _ => timezone
            .with_ymd_and_hms(now.year() + 1, expired.month(), expired.day(), 0, 0, 0)
            .single()?,
    };
    Some((target.timestamp() - now.timestamp()) / 86_400)
}

fn last_day_of_current_month() -> u32 {
    let today = app_now().date_naive();
    let (year, month) = if today.month() == 12 {
        (today.year() + 1, 1)
    } else {
        (today.year(), today.month() + 1)
    };
    let first_next_month = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
    (first_next_month - Duration::days(1)).day()
}

pub(crate) fn user_is_available(user: &v2board_db::user::UserAccessRow) -> bool {
    let unexpired = user
        .expired_at
        .map(|expired_at| expired_at > Utc::now().timestamp())
        .unwrap_or(true);
    user.banned == 0 && user.transfer_enable > 0 && unexpired
}
