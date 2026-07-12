use axum::{
    Json,
    body::Body,
    extract::{Extension, Form, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};
use v2board_domain::auth::{AuthUser, EmailVerifyInput, ForgetInput, RegisterInput};

use crate::{
    runtime::{AppState, ClientIp},
    validation::forbidden,
};

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    email: String,
    password: String,
}

pub(crate) async fn login(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    Form(payload): Form<LoginRequest>,
) -> Result<Json<LegacyEnvelope<v2board_domain::auth::AuthData>>, ApiError> {
    let auth = state.auth_service();
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let ip = Some(client_ip.to_string());
    let data = auth
        .login(&payload.email, &payload.password, ip, user_agent)
        .await?;
    Ok(legacy_data(data))
}

pub(crate) async fn register(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    Form(payload): Form<RegisterInput>,
) -> Result<Json<LegacyEnvelope<v2board_domain::auth::AuthData>>, ApiError> {
    let auth = state.auth_service();
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let data = auth
        .register(payload, Some(client_ip.to_string()), user_agent)
        .await?;
    Ok(legacy_data(data))
}

#[derive(Debug, Deserialize)]
pub(crate) struct Token2LoginQuery {
    token: Option<String>,
    verify: Option<String>,
    redirect: Option<String>,
}

pub(crate) async fn token2_login(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    Query(query): Query<Token2LoginQuery>,
) -> Result<Response, ApiError> {
    let auth = state.auth_service();
    if let Some(token) = query.token.as_deref().filter(|value| !value.is_empty()) {
        let url = auth.login_redirect_url(token, query.redirect.as_deref());
        // Laravel `redirect()->to($location)` issues a 302 Found (not a 307); match it so
        // the emitted Location is byte-identical.
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::FOUND;
        response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&url)
                .map_err(|_| ApiError::internal("invalid redirect location"))?,
        );
        return Ok(response);
    }
    if let Some(verify) = query.verify.as_deref().filter(|value| !value.is_empty()) {
        let user_agent = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let data = auth
            .token_login(verify, Some(client_ip.to_string()), user_agent)
            .await?;
        return Ok(legacy_data(data).into_response());
    }
    // Laravel token2Login has no `else`; a request with neither token nor verify falls
    // through to an empty 200 response.
    Ok(StatusCode::OK.into_response())
}

pub(crate) async fn forget_password(
    State(state): State<AppState>,
    Form(payload): Form<ForgetInput>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = state.auth_service();
    Ok(legacy_data(auth.forget(payload).await?))
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuickLoginRequest {
    auth_data: Option<String>,
    redirect: Option<String>,
}

pub(crate) async fn passport_quick_login_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<QuickLoginRequest>,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, payload.auth_data).await?;
    let auth = state.auth_service();
    let url = auth
        .quick_login_url(user.id, payload.redirect.as_deref())
        .await?;
    Ok(legacy_data(url))
}

pub(crate) async fn send_email_verify(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    Form(payload): Form<EmailVerifyInput>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = state.auth_service();
    Ok(legacy_data(
        auth.send_email_verify(payload, Some(client_ip.to_string()))
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub(crate) struct PassportPvRequest {
    invite_code: Option<String>,
}

pub(crate) async fn passport_pv(
    State(state): State<AppState>,
    Form(payload): Form<PassportPvRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = state.auth_service();
    Ok(legacy_data(
        auth.passport_pv(payload.invite_code.as_deref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub(crate) struct AuthQuery {
    pub(crate) auth_data: Option<String>,
}

pub(crate) async fn require_user(
    state: &AppState,
    headers: &HeaderMap,
    auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let auth_data = auth_data
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        })
        .ok_or_else(ApiError::unauthorized)?;
    let auth = state.auth_service();
    auth.user_from_auth_data(&auth_data).await
}

pub(crate) async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let user = require_user(state, headers, auth_data).await?;
    if user.is_admin == 0 {
        return Err(forbidden("Permission denied"));
    }
    Ok(user)
}

pub(crate) async fn require_staff(
    state: &AppState,
    headers: &HeaderMap,
    auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let user = require_user(state, headers, auth_data).await?;
    if user.is_staff == 0 {
        return Err(forbidden("Permission denied"));
    }
    Ok(user)
}
