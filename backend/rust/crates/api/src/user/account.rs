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
pub(crate) use v2board_api_contract::user::{
    UserConfig, UserProfile as UserProfileBody, UserSession as SessionBody,
};
use v2board_api_contract::{
    time::Rfc3339Timestamp,
    user::{PasswordUpdateRequest, UserProfilePatch},
};
use v2board_application::account::{AccountError, AccountProfile, PreferenceChanges};
use v2board_application::auth::UserSession;
use v2board_compat::{ApiError, Code, Problem};

use crate::{
    auth::{auth_error, require_user},
    dialect::DialectJson,
    dialect::problem_from,
    locale::request_locale,
    runtime::AppState,
};

fn account_error(error: AccountError) -> ApiError {
    match error {
        AccountError::NotFound => Problem::new(Code::SessionExpired).into(),
        AccountError::TelegramUnbindFailed => Problem::new(Code::TelegramUnbindFailed).into(),
        AccountError::Repository(error) => ApiError::internal(error.to_string()),
    }
}

pub(crate) fn user_profile_body(row: AccountProfile) -> UserProfileBody {
    UserProfileBody {
        email: row.email,
        transfer_enable: row.transfer_enable,
        device_limit: row.device_limit,
        last_login_at: row.last_login_at.map(Rfc3339Timestamp::from_epoch_seconds),
        created_at: Rfc3339Timestamp::from_epoch_seconds(row.created_at),
        banned: row.banned,
        auto_renewal: row.auto_renewal,
        remind_expire: row.remind_expire,
        remind_traffic: row.remind_traffic,
        expired_at: row.expired_at.map(Rfc3339Timestamp::from_epoch_seconds),
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

/// GET /user/profile — bare profile (§5.3).
pub(crate) async fn user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserProfileBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let info = state
        .account_service()
        .profile(user.id)
        .await
        .map_err(account_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(user_profile_body(info)))
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
    state
        .account_service()
        .update_preferences(
            user.id,
            PreferenceChanges {
                auto_renewal: payload.auto_renewal,
                remind_expire: payload.remind_expire,
                remind_traffic: payload.remind_traffic,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(account_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
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
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

fn session_body(session: UserSession) -> SessionBody {
    SessionBody {
        session_id: session.session_id,
        ip: session.ip,
        ua: session.ua,
        login_at: Rfc3339Timestamp::from_epoch_seconds(session.login_at),
        current: session.current,
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
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(sessions.into_iter().map(session_body).collect()))
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
        .map_err(auth_error)
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
    state
        .account_service()
        .unbind_telegram(user.id, Utc::now().timestamp())
        .await
        .map_err(account_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
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
