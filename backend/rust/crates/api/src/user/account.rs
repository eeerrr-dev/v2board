//! User account family — modern dialect (docs/api-dialect.md §5.3, §9.1,
//! §9.4, Appendix A §W5): bare success bodies on modern value types
//! (RFC 3339 timestamps, boolean flags), §4.4 double-Option PATCH semantics,
//! path-borne session identity, and problem+json failures.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use v2board_compat::{
    Code, Problem,
    json::{double_option, rfc3339, rfc3339_option},
};

use crate::{
    auth::require_user, dialect::DialectJson, dialect::problem_from, locale::request_locale,
    runtime::AppState,
};

/// Bare GET /user/profile body (§5.3, W5): the legacy 0/1 flags are booleans
/// (§4.1; a NULL preference reads as `false`) and the epoch timestamps are
/// RFC 3339 instants (§4.5). Money stays integer cents.
#[derive(Debug, Serialize)]
pub(crate) struct UserProfileBody {
    pub(crate) email: String,
    pub(crate) transfer_enable: i64,
    pub(crate) device_limit: Option<i32>,
    #[serde(with = "rfc3339_option")]
    pub(crate) last_login_at: Option<i64>,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    pub(crate) banned: bool,
    pub(crate) auto_renewal: bool,
    pub(crate) remind_expire: bool,
    pub(crate) remind_traffic: bool,
    #[serde(with = "rfc3339_option")]
    pub(crate) expired_at: Option<i64>,
    pub(crate) balance: i32,
    pub(crate) commission_balance: i32,
    pub(crate) plan_id: Option<i32>,
    pub(crate) discount: Option<i32>,
    pub(crate) commission_rate: Option<i32>,
    pub(crate) telegram_id: Option<i64>,
    pub(crate) uuid: String,
    pub(crate) avatar_url: String,
}

impl From<v2board_db::user::UserInfoRow> for UserProfileBody {
    fn from(row: v2board_db::user::UserInfoRow) -> Self {
        Self {
            email: row.email,
            transfer_enable: row.transfer_enable,
            device_limit: row.device_limit,
            last_login_at: row.last_login_at,
            created_at: row.created_at,
            banned: row.banned != 0,
            auto_renewal: row.auto_renewal.unwrap_or(0) != 0,
            remind_expire: row.remind_expire.unwrap_or(0) != 0,
            remind_traffic: row.remind_traffic.unwrap_or(0) != 0,
            expired_at: row.expired_at,
            balance: row.balance,
            commission_balance: row.commission_balance,
            plan_id: row.plan_id,
            discount: row.discount,
            commission_rate: row.commission_rate,
            telegram_id: row.telegram_id,
            uuid: row.uuid,
            avatar_url: row.avatar_url,
        }
    }
}

/// GET /user/profile — bare profile (§5.3).
pub(crate) async fn user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserProfileBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let info = v2board_db::user::find_user_info(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::SessionExpired, locale))?;
    Ok(Json(UserProfileBody::from(info)))
}

/// PATCH /user/profile request (§5.3): boolean preference flags on §4.4
/// double-Option semantics — absent retains, null clears, a value sets.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UserProfilePatch {
    #[serde(default, with = "double_option")]
    auto_renewal: Option<Option<bool>>,
    #[serde(default, with = "double_option")]
    remind_expire: Option<Option<bool>>,
    #[serde(default, with = "double_option")]
    remind_traffic: Option<Option<bool>>,
}

fn preference_write(field: Option<Option<bool>>) -> Option<Option<i16>> {
    field.map(|value| value.map(i16::from))
}

/// PATCH /user/profile — 204 on success (§5.3, §4.4).
pub(crate) async fn user_profile_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<UserProfilePatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    v2board_db::user::update_preferences(
        &state.db,
        user.id,
        preference_write(payload.auto_renewal),
        preference_write(payload.remind_expire),
        preference_write(payload.remind_traffic),
        Utc::now().timestamp(),
    )
    .await
    .map_err(|error| problem_from(error.into(), locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordUpdateRequest {
    old_password: String,
    new_password: String,
}

/// PUT /user/password — 204 on success (§5.3). Success still invalidates
/// every session; the client redirect to /login is the Tier-1 outcome.
pub(crate) async fn user_password_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<PasswordUpdateRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    state
        .auth_service()
        .change_password(user.id, &payload.old_password, &payload.new_password)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// One GET /user/sessions entry (§5.3/§9.4): the legacy map key is
/// `session_id`, `login_at` is RFC 3339, and the constant-`""` `auth_data`
/// filler died with the map shape.
#[derive(Debug, Serialize)]
pub(crate) struct SessionBody {
    pub(crate) session_id: String,
    pub(crate) ip: String,
    pub(crate) ua: String,
    #[serde(with = "rfc3339")]
    pub(crate) login_at: i64,
    pub(crate) current: bool,
}

impl From<v2board_domain::auth::UserSession> for SessionBody {
    fn from(session: v2board_domain::auth::UserSession) -> Self {
        Self {
            session_id: session.session_id,
            ip: session.ip,
            ua: session.ua,
            login_at: session.login_at,
            current: session.current,
        }
    }
}

/// GET /user/sessions — bare array (§5.3).
pub(crate) async fn user_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionBody>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let sessions = state
        .auth_service()
        .sessions(user.id, Some(&user.session_id))
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(sessions.into_iter().map(SessionBody::from).collect()))
}

/// DELETE /user/sessions/{session_id} — 204 (§5.3). Removal of an unknown or
/// already-revoked session stays an idempotent success, exactly like the
/// legacy boolean-true response.
pub(crate) async fn user_session_delete(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    state
        .auth_service()
        .remove_session(user.id, &session_id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /user/telegram-binding — 204 (§5.3; the legacy GET-with-side-effect
/// became a DELETE).
pub(crate) async fn user_telegram_binding_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let updated = v2board_db::user::clear_telegram_id(&state.db, user.id, Utc::now().timestamp())
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    if !updated {
        return Err(Problem::localized(Code::TelegramUnbindFailed, locale));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Bare GET /user/config body (docs/api-dialect.md §5.3, W3): flags are
/// booleans, `withdraw_methods` is always a real array, and the commission
/// distribution rates are numbers (the legacy string-vs-number split dies,
/// §4.1).
#[derive(Debug, Serialize)]
pub(crate) struct UserConfig {
    pub(crate) is_telegram: bool,
    pub(crate) telegram_discuss_link: Option<String>,
    pub(crate) withdraw_methods: Vec<String>,
    pub(crate) withdraw_close: bool,
    pub(crate) currency: String,
    pub(crate) currency_symbol: String,
    pub(crate) commission_distribution_enable: bool,
    pub(crate) commission_distribution_l1: Option<f64>,
    pub(crate) commission_distribution_l2: Option<f64>,
    pub(crate) commission_distribution_l3: Option<f64>,
}

/// Legacy admin config stores distribution rates as free-form strings; the
/// modern wire carries numbers, and an unset or non-numeric value is null.
fn distribution_rate(value: Option<&str>) -> Option<f64> {
    value.and_then(|raw| raw.trim().parse::<f64>().ok())
}

pub(crate) async fn user_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserConfig>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let config = state.config_snapshot();
    Ok(Json(UserConfig {
        is_telegram: config.telegram_bot_enable,
        telegram_discuss_link: config.telegram_discuss_link.clone(),
        withdraw_methods: config.commission_withdraw_method.clone(),
        withdraw_close: config.withdraw_close_enable,
        currency: config.currency.clone(),
        currency_symbol: config.currency_symbol.clone(),
        commission_distribution_enable: config.commission_distribution_enable,
        commission_distribution_l1: distribution_rate(config.commission_distribution_l1.as_deref()),
        commission_distribution_l2: distribution_rate(config.commission_distribution_l2.as_deref()),
        commission_distribution_l3: distribution_rate(config.commission_distribution_l3.as_deref()),
    }))
}
