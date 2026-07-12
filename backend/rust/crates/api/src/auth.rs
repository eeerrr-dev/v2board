use axum::{
    Json,
    body::Body,
    extract::{Extension, Form, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};
use v2board_domain::auth::{AuthUser, EmailVerifyInput, ForgetInput, RegisterInput};

use crate::{
    runtime::{AppState, ClientIp},
    validation::forbidden,
};

const MAX_AUTH_DATA_BYTES: usize = 4096;
const MAX_REDIRECT_BYTES: usize = 2048;
const MAX_TEMP_TOKEN_BYTES: usize = 256;
const MAX_USER_AGENT_BYTES: usize = 512;
const STEP_UP_HEADER: &str = "x-v2board-step-up";

#[derive(Deserialize)]
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
    let user_agent = bounded_user_agent(&headers);
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
    let user_agent = bounded_user_agent(&headers);
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
    if query
        .redirect
        .as_deref()
        .is_some_and(|value| value.len() > MAX_REDIRECT_BYTES)
    {
        return Err(ApiError::bad_request("Redirect is too long"));
    }
    if let Some(token) = query.token.as_deref().filter(|value| !value.is_empty()) {
        if token.len() > MAX_TEMP_TOKEN_BYTES {
            return Err(ApiError::bad_request("Token is too long"));
        }
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
        if verify.len() > MAX_TEMP_TOKEN_BYTES {
            return Err(ApiError::bad_request("Token is too long"));
        }
        let user_agent = bounded_user_agent(&headers);
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

#[derive(Deserialize)]
pub(crate) struct StepUpRequest {
    password: String,
    auth_data: Option<String>,
}

pub(crate) async fn privileged_step_up(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    Form(payload): Form<StepUpRequest>,
) -> Result<Json<LegacyEnvelope<Value>>, ApiError> {
    let user = require_user(&state, &headers, payload.auth_data).await?;
    if user.is_admin == 0 && user.is_staff == 0 {
        return Err(forbidden("Permission denied"));
    }
    let auth = state.auth_service();
    let client_ip = client_ip.to_string();
    let token = auth
        .create_privileged_step_up(
            user.id,
            &user.session_id,
            &payload.password,
            Some(&client_ip),
        )
        .await?;
    let expires_in = state.config_snapshot().privileged_step_up_ttl_seconds;
    Ok(legacy_data(json!({
        "step_up_token": token,
        "expires_in": expires_in,
    })))
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
    _auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let auth_data = select_auth_data(headers).ok_or_else(ApiError::unauthorized)?;
    let auth = state.auth_service();
    auth.user_from_auth_data(&auth_data).await
}

fn select_auth_data(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()
        .filter(|value| !value.is_empty() && value.len() <= MAX_AUTH_DATA_BYTES)
        .map(ToOwned::to_owned)
}

fn bounded_user_agent(headers: &HeaderMap) -> Option<String> {
    let value =
        std::str::from_utf8(headers.get(axum::http::header::USER_AGENT)?.as_bytes()).ok()?;
    if value.len() <= MAX_USER_AGENT_BYTES {
        return Some(value.to_string());
    }
    let mut boundary = MAX_USER_AGENT_BYTES;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    Some(value[..boundary].to_string())
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

pub(crate) async fn require_privileged_step_up(
    state: &AppState,
    headers: &HeaderMap,
    user: &AuthUser,
) -> Result<(), ApiError> {
    if !state.config_snapshot().privileged_step_up_enable {
        return Ok(());
    }
    let token = headers
        .get(STEP_UP_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(token) = token {
        if state
            .auth_service()
            .verify_privileged_step_up(user.id, &user.session_id, token)
            .await?
        {
            return Ok(());
        }
        return Err(forbidden("Recent password verification is required"));
    }
    let config = state.config_snapshot();
    if recent_password_authentication(
        user,
        chrono::Utc::now().timestamp(),
        config.privileged_step_up_ttl_seconds,
    ) {
        return Ok(());
    }
    Err(forbidden("Recent password verification is required"))
}

fn recent_password_authentication(user: &AuthUser, now: i64, ttl_seconds: u64) -> bool {
    user.password_authenticated
        && user.authenticated_at <= now
        && now.saturating_sub(user.authenticated_at) <= ttl_seconds as i64
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, header};

    use super::{
        AuthQuery, MAX_USER_AGENT_BYTES, bounded_user_agent, recent_password_authentication,
        select_auth_data,
    };
    use v2board_domain::auth::AuthUser;

    #[test]
    fn only_the_authorization_header_is_an_authentication_source() {
        let accepted_shape: AuthQuery =
            serde_urlencoded::from_str("auth_data=query-token").expect("accepted query shape");
        assert_eq!(accepted_shape.auth_data.as_deref(), Some("query-token"));

        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "header-token".parse().unwrap());
        assert_eq!(select_auth_data(&headers).as_deref(), Some("header-token"));
        headers.insert(
            header::AUTHORIZATION,
            "x".repeat(super::MAX_AUTH_DATA_BYTES + 1).parse().unwrap(),
        );
        assert_eq!(
            select_auth_data(&headers),
            None,
            "an invalid Authorization header must fail closed"
        );
        headers.remove(header::AUTHORIZATION);
        assert_eq!(
            select_auth_data(&headers),
            None,
            "accepted auth_data parameter shape is never an authentication source"
        );
    }

    #[test]
    fn user_agent_is_bounded_without_splitting_utf8() {
        let mut headers = HeaderMap::new();
        let user_agent = format!("{}中", "a".repeat(MAX_USER_AGENT_BYTES - 1));
        headers.insert(header::USER_AGENT, user_agent.parse().unwrap());
        let bounded = bounded_user_agent(&headers).unwrap();
        assert_eq!(bounded.len(), MAX_USER_AGENT_BYTES - 1);
        assert!(bounded.is_char_boundary(bounded.len()));
    }

    #[test]
    fn recent_password_login_satisfies_initial_step_up_window_only() {
        let mut user = AuthUser {
            id: 1,
            email: "admin@example.test".to_string(),
            is_admin: 1,
            is_staff: 0,
            session_id: "session".to_string(),
            authenticated_at: 1_000,
            password_authenticated: true,
        };
        assert!(recent_password_authentication(&user, 1_600, 600));
        assert!(!recent_password_authentication(&user, 1_601, 600));
        assert!(!recent_password_authentication(&user, 999, 600));
        user.password_authenticated = false;
        assert!(!recent_password_authentication(&user, 1_001, 600));
    }
}
