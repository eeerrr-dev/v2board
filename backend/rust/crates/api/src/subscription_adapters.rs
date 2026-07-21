use std::sync::Arc;

use chrono::{Datelike, Duration, TimeZone};
use redis::AsyncCommands;
use v2board_application::{
    RepositoryError,
    subscription::{
        RepositoryResult, ResetCalendar, SubscriptionAccessExternal, resolve_reset_method,
    },
};
use v2board_compat::constant_time_bytes_eq;
use v2board_config::{
    AppConfig, RedisKeyspace, app_now, app_timezone, duration_minutes_to_seconds,
};
use v2board_domain_model::TrafficResetMethod;
use v2board_subscription_adapters::{hmac_sha1_hex, subscribe_url_for_user, totp_counter_bytes};

use crate::codec::base64_decode_url_safe;

const CONSUME_SUBSCRIBE_TOKEN_SCRIPT: &str = r#"
local user_token = redis.call('GET', KEYS[1])
if not user_token then
    return false
end
redis.call('DEL', KEYS[1])
redis.call('DEL', ARGV[1] .. user_token)
return user_token
"#;

#[derive(Clone)]
pub(crate) struct RedisSubscriptionAccess {
    redis: redis::aio::ConnectionManager,
    keys: RedisKeyspace,
    config: Arc<AppConfig>,
}

impl RedisSubscriptionAccess {
    pub(crate) const fn new(
        redis: redis::aio::ConnectionManager,
        keys: RedisKeyspace,
        config: Arc<AppConfig>,
    ) -> Self {
        Self {
            redis,
            keys,
            config,
        }
    }

    fn key(&self, logical: &str) -> String {
        self.keys.key(logical)
    }
}

impl SubscriptionAccessExternal for RedisSubscriptionAccess {
    async fn consume_one_time_token(&self, presented: &str) -> RepositoryResult<Option<String>> {
        let mut redis = self.redis.clone();
        redis::Script::new(CONSUME_SUBSCRIBE_TOKEN_SCRIPT)
            .key(self.key(&format!("otpn_{presented}")))
            .arg(self.key("otp_"))
            .invoke_async(&mut redis)
            .await
            .map_err(|error| subscription_access_error("consume one-time token", error))
    }

    async fn cached_time_token(&self, presented: &str) -> RepositoryResult<Option<String>> {
        let mut redis = self.redis.clone();
        redis
            .get(self.key(&format!("totp_{presented}")))
            .await
            .map_err(|error| subscription_access_error("read time-token cache", error))
    }

    fn time_token_user_id(&self, presented: &str) -> Option<i64> {
        time_token_parts(presented).map(|(user_id, _)| user_id)
    }

    fn time_token_matches(&self, presented: &str, user_id: i64, permanent_token: &str) -> bool {
        let Some((candidate_user_id, candidate_hash)) = time_token_parts(presented) else {
            return false;
        };
        if candidate_user_id != user_id {
            return false;
        }
        hmac_sha1_hex(
            permanent_token.as_bytes(),
            &totp_counter_bytes(&self.config),
        )
        .is_ok_and(|expected| {
            constant_time_bytes_eq(expected.as_bytes(), candidate_hash.as_bytes())
        })
    }

    async fn cache_time_token(
        &self,
        presented: &str,
        permanent_token: &str,
    ) -> RepositoryResult<()> {
        let mut redis = self.redis.clone();
        let ttl = duration_minutes_to_seconds(self.config.show_subscribe_expire);
        redis
            .set_ex(self.key(&format!("totp_{presented}")), permanent_token, ttl)
            .await
            .map_err(|error| subscription_access_error("cache time token", error))
    }

    async fn alive_ip(&self, user_id: i64) -> RepositoryResult<i64> {
        let mut redis = self.redis.clone();
        let current: Option<String> = redis
            .get(self.key(&format!("ALIVE_IP_USER_{user_id}")))
            .await
            .map_err(|error| subscription_access_error("read alive IP projection", error))?;
        Ok(current
            .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
            .and_then(|value| value.get("alive_ip").and_then(serde_json::Value::as_i64))
            .unwrap_or(0))
    }

    async fn subscribe_url(&self, user_id: i64, permanent_token: &str) -> RepositoryResult<String> {
        let mut redis = Some(self.redis.clone());
        subscribe_url_for_user(
            &self.config,
            &self.keys,
            &mut redis,
            user_id,
            permanent_token,
        )
        .await
        .map_err(|error| subscription_access_error("mint subscription URL", error))
    }
}

fn time_token_parts(presented: &str) -> Option<(i64, String)> {
    let decoded = String::from_utf8(base64_decode_url_safe(presented)?).ok()?;
    let (user_id, hash) = decoded.split_once(':')?;
    if user_id.is_empty() || hash.is_empty() {
        return None;
    }
    Some((user_id.parse().ok()?, hash.to_string()))
}

fn subscription_access_error(
    operation: &'static str,
    error: impl std::fmt::Display,
) -> RepositoryError {
    RepositoryError::new(operation, error)
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ConfiguredResetCalendar;

impl ResetCalendar for ConfiguredResetCalendar {
    fn days_until_reset(&self, method: TrafficResetMethod, expired_at: i64) -> Option<i64> {
        match method {
            TrafficResetMethod::MonthStart => Some(reset_day_by_month_first_day()),
            TrafficResetMethod::ExpiryDay => Some(reset_day_by_expire_day(expired_at)),
            TrafficResetMethod::Never => None,
            TrafficResetMethod::YearStart => days_until_year_first_day(),
            TrafficResetMethod::ExpiryAnniversary => days_until_year_expire_day(expired_at),
        }
    }
}

pub(crate) fn reset_day(
    expired_at: Option<i64>,
    plan_reset_method: Option<Option<i16>>,
    default_method: i32,
    now: i64,
) -> Option<i64> {
    let expired_at = expired_at.filter(|expired_at| *expired_at > now)?;
    let plan_reset_method = plan_reset_method?;
    let method = resolve_reset_method(plan_reset_method, default_method)?;
    ConfiguredResetCalendar.days_until_reset(method, expired_at)
}

pub(crate) fn reset_day_by_month_first_day() -> i64 {
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
