use axum::{
    Json,
    extract::{Form, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use v2board_compat::{ApiError, LegacyEnvelope, Problem, legacy_data};

use crate::{auth::require_user, dialect::problem_from, locale::request_locale, runtime::AppState};

pub(crate) async fn user_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::user::UserInfoRow>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let info = v2board_db::user::find_user_info(&state.db, user.id)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(legacy_data(info))
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserUpdateRequest {
    auto_renewal: Option<i16>,
    remind_expire: Option<i16>,
    remind_traffic: Option<i16>,
}

pub(crate) async fn user_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<UserUpdateRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    validate_binary("auto_renewal", payload.auto_renewal)?;
    validate_binary("remind_expire", payload.remind_expire)?;
    validate_binary("remind_traffic", payload.remind_traffic)?;

    v2board_db::user::update_preferences(
        &state.db,
        user.id,
        payload.auto_renewal,
        payload.remind_expire,
        payload.remind_traffic,
        Utc::now().timestamp(),
    )
    .await?;
    Ok(legacy_data(true))
}

#[derive(Deserialize)]
pub(crate) struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

pub(crate) async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<ChangePasswordRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let auth = state.auth_service();
    auth.change_password(user.id, &payload.old_password, &payload.new_password)
        .await?;
    Ok(legacy_data(true))
}

pub(crate) async fn reset_security(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let auth = state.auth_service();
    let subscribe_url = auth.reset_security(user.id).await?;
    Ok(legacy_data(subscribe_url))
}

pub(crate) async fn unbind_telegram(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let updated =
        v2board_db::user::clear_telegram_id(&state.db, user.id, Utc::now().timestamp()).await?;
    if !updated {
        return Err(ApiError::business("Unbind telegram failed"));
    }
    Ok(legacy_data(true))
}

pub(crate) async fn active_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<serde_json::Map<String, serde_json::Value>>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let auth = state.auth_service();
    let sessions = auth.sessions(user.id, Some(&user.session_id)).await?;
    Ok(legacy_data(sessions))
}

#[derive(Debug, Deserialize)]
pub(crate) struct RemoveActiveSessionRequest {
    session_id: String,
}

pub(crate) async fn remove_active_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<RemoveActiveSessionRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let auth = state.auth_service();
    let removed = auth.remove_session(user.id, &payload.session_id).await?;
    Ok(legacy_data(removed))
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

fn validate_binary(field: &str, value: Option<i16>) -> Result<(), ApiError> {
    match value {
        Some(0 | 1) | None => Ok(()),
        Some(_) => Err(ApiError::bad_request(format!("{field} must be 0 or 1"))),
    }
}
