use axum::{
    Json,
    extract::{Form, Query, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};

use crate::{
    auth::{AuthQuery, require_user},
    runtime::AppState,
};

pub(crate) async fn user_info(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::user::UserInfoRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let info = v2board_db::user::find_user_info(&state.db, user.id)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(legacy_data(info))
}

#[derive(Debug, Serialize)]
pub(crate) struct CheckLoginResult {
    is_login: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_admin: Option<bool>,
}

pub(crate) async fn check_login(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<CheckLoginResult>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    Ok(legacy_data(CheckLoginResult {
        is_login: true,
        is_admin: (user.is_admin != 0).then_some(true),
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserUpdateRequest {
    auto_renewal: Option<i8>,
    remind_expire: Option<i8>,
    remind_traffic: Option<i8>,
}

pub(crate) async fn user_update(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<UserUpdateRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
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
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<ChangePasswordRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = state.auth_service();
    auth.change_password(user.id, &payload.old_password, &payload.new_password)
        .await?;
    Ok(legacy_data(true))
}

pub(crate) async fn reset_security(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = state.auth_service();
    let subscribe_url = auth.reset_security(user.id).await?;
    Ok(legacy_data(subscribe_url))
}

pub(crate) async fn unbind_telegram(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let updated =
        v2board_db::user::clear_telegram_id(&state.db, user.id, Utc::now().timestamp()).await?;
    if !updated {
        return Err(ApiError::legacy("Unbind telegram failed"));
    }
    Ok(legacy_data(true))
}

pub(crate) async fn active_sessions(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<serde_json::Map<String, serde_json::Value>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
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
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<RemoveActiveSessionRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = state.auth_service();
    let removed = auth.remove_session(user.id, &payload.session_id).await?;
    Ok(legacy_data(removed))
}

#[derive(Debug, Serialize)]
pub(crate) struct UserCommConfig {
    is_telegram: i32,
    telegram_discuss_link: Option<String>,
    withdraw_methods: Vec<String>,
    withdraw_close: i32,
    currency: String,
    currency_symbol: String,
    commission_distribution_enable: i32,
    commission_distribution_l1: Option<String>,
    commission_distribution_l2: Option<String>,
    commission_distribution_l3: Option<String>,
}

pub(crate) async fn user_comm_config(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<UserCommConfig>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    let config = state.config_snapshot();
    Ok(legacy_data(UserCommConfig {
        is_telegram: config.telegram_bot_enable as i32,
        telegram_discuss_link: config.telegram_discuss_link.clone(),
        withdraw_methods: config.commission_withdraw_method.clone(),
        withdraw_close: config.withdraw_close_enable as i32,
        currency: config.currency.clone(),
        currency_symbol: config.currency_symbol.clone(),
        commission_distribution_enable: config.commission_distribution_enable as i32,
        commission_distribution_l1: config.commission_distribution_l1.clone(),
        commission_distribution_l2: config.commission_distribution_l2.clone(),
        commission_distribution_l3: config.commission_distribution_l3.clone(),
    }))
}

fn validate_binary(field: &str, value: Option<i8>) -> Result<(), ApiError> {
    match value {
        Some(0 | 1) | None => Ok(()),
        Some(_) => Err(ApiError::bad_request(format!("{field} must be 0 or 1"))),
    }
}
