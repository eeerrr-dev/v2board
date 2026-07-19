//! The modern-dialect auth family (docs/api-dialect.md §5.2, Appendix A §W2)
//! plus the shared session extractors every internal route authenticates
//! through.
//!
//! Two cross-cutting flips live here because this middleware is shared
//! (§4.2, §3.2): `select_auth_data` requires and strips the
//! `Authorization: Bearer ` scheme on internal routes, and a missing/expired/
//! invalid session is a **401** `session_expired` problem (with
//! `WWW-Authenticate`) on every internal route — including families whose
//! success/business bodies are still legacy. `permission_denied` and
//! `step_up_required` stay 403 and must never tear a session down.

use axum::{
    Json,
    body::Body,
    extract::{Extension, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use v2board_compat::{ApiError, Code, Problem};
use v2board_domain::auth::{AuthUser, EmailVerifyInput, ForgetInput, RegisterInput};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::{AppState, ClientIp},
};

const MAX_AUTH_DATA_BYTES: usize = 4096;
const MAX_REDIRECT_BYTES: usize = 2048;
const MAX_TEMP_TOKEN_BYTES: usize = 256;
const MAX_USER_AGENT_BYTES: usize = 512;
const STEP_UP_HEADER: &str = "x-v2board-step-up";
/// §4.2 — the only accepted Authorization scheme on internal routes.
const BEARER_SCHEME: &str = "Bearer ";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginRequest {
    email: String,
    password: String,
    /// Required (as a second phase) only for privileged accounts with an
    /// enabled TOTP factor: absent → 401 `mfa_code_required`, wrong →
    /// 401 `mfa_code_invalid`.
    #[serde(default)]
    totp_code: Option<String>,
}

/// POST /auth/login — bare `{is_admin: bool, auth_data}` (§5.2).
pub(crate) async fn login(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<LoginRequest>,
) -> Result<Json<v2board_domain::auth::AuthData>, Problem> {
    let locale = request_locale(&headers);
    let auth = state.auth_service();
    let user_agent = bounded_user_agent(&headers);
    let ip = Some(client_ip.to_string());
    let data = auth
        .login(
            &payload.email,
            &payload.password,
            payload.totp_code.as_deref(),
            ip,
            user_agent,
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(data))
}

/// POST /auth/register — 201 with the same bare auth body (§1, §5.2).
pub(crate) async fn register(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<RegisterInput>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let auth = state.auth_service();
    let user_agent = bounded_user_agent(&headers);
    let data = auth
        .register(payload, Some(client_ip.to_string()), user_agent)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(data)).into_response())
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuickLoginQuery {
    token: Option<String>,
    redirect: Option<String>,
}

/// GET /auth/quick-login?token= — browser-facing 302 to the path-style
/// `{app_url}/login?verify=…&redirect=…` (§5.2, §10.4). The legacy
/// GET-with-side-effect `?verify=` exchange moved to POST /auth/token-login,
/// and the legacy "neither param → empty 200" branch dies (422).
pub(crate) async fn quick_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<QuickLoginQuery>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let Some(token) = query.token.as_deref().filter(|value| !value.is_empty()) else {
        return Err(Problem::validation_field(
            "token",
            "The token field is required",
        ));
    };
    if token.len() > MAX_TEMP_TOKEN_BYTES {
        return Err(Problem::new(Code::InvalidParameter).with_detail("Token is too long"));
    }
    if query
        .redirect
        .as_deref()
        .is_some_and(|value| value.len() > MAX_REDIRECT_BYTES)
    {
        return Err(Problem::new(Code::InvalidParameter).with_detail("Redirect is too long"));
    }
    let auth = state.auth_service();
    let url = auth.login_redirect_url(token, query.redirect.as_deref());
    // 302 Found, matching the legacy Laravel `redirect()->to(...)` semantics
    // the emailed links already rely on.
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::FOUND;
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&url).map_err(|_| Problem::localized(Code::InternalError, locale))?,
    );
    Ok(response)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TokenLoginRequest {
    verify: String,
}

/// POST /auth/token-login — the SPA one-time `?verify=` exchange; a dead or
/// malformed token is 400 `invalid_token` (§5.2, §3.4).
pub(crate) async fn token_login(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<TokenLoginRequest>,
) -> Result<Json<v2board_domain::auth::AuthData>, Problem> {
    let locale = request_locale(&headers);
    let verify = payload.verify.trim();
    if verify.is_empty() {
        return Err(Problem::validation_field(
            "verify",
            "The verify field is required",
        ));
    }
    if verify.len() > MAX_TEMP_TOKEN_BYTES {
        return Err(Problem::new(Code::InvalidParameter).with_detail("Token is too long"));
    }
    let auth = state.auth_service();
    let user_agent = bounded_user_agent(&headers);
    let data = auth
        .token_login(verify, Some(client_ip.to_string()), user_agent)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(data))
}

/// POST /auth/password-reset — empty success (204).
pub(crate) async fn password_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<ForgetInput>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let auth = state.auth_service();
    auth.forget(payload)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StepUpRequest {
    password: String,
}

/// Bare step-up grant body (§5.2): `{step_up_token, expires_in}`.
#[derive(Serialize)]
pub(crate) struct StepUpGrant {
    pub(crate) step_up_token: String,
    pub(crate) expires_in: u64,
}

/// POST /auth/step-up — re-verify the privileged password; the grant rides
/// subsequent requests as `x-v2board-step-up` (§4.2).
pub(crate) async fn step_up(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<StepUpRequest>,
) -> Result<Json<StepUpGrant>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    if user.is_admin == 0 && user.is_staff == 0 {
        return Err(Problem::localized(Code::PermissionDenied, locale));
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
        .await
        .map_err(|error| problem_from(error, locale))?;
    let expires_in = state.config_snapshot().privileged_step_up_ttl_seconds;
    Ok(Json(StepUpGrant {
        step_up_token: token,
        expires_in,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QuickLoginUrlRequest {
    redirect: Option<String>,
}

/// Bare minted-URL body (§5.2, §9.4): `{url}`.
#[derive(Serialize)]
pub(crate) struct QuickLoginUrl {
    pub(crate) url: String,
}

/// POST /auth/quick-login-url — consolidates the duplicate legacy
/// `/passport/auth/getQuickLoginUrl` + `/user/getQuickLoginUrl` pair (§5.2).
pub(crate) async fn quick_login_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<QuickLoginUrlRequest>,
) -> Result<Json<QuickLoginUrl>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let auth = state.auth_service();
    let url = auth
        .quick_login_url(user.id, payload.redirect.as_deref())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(QuickLoginUrl { url }))
}

/// POST /auth/email-codes — empty success (204); `isforget: 0/1` became
/// `is_forget: bool` (§4.1, §5.2).
pub(crate) async fn email_codes(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<EmailVerifyInput>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let auth = state.auth_service();
    auth.send_email_verify(payload, Some(client_ip.to_string()))
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Bare session-probe body (§5.2): `{is_login: bool, is_admin?: bool}`.
#[derive(Debug, Serialize)]
pub(crate) struct SessionState {
    pub(crate) is_login: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_admin: Option<bool>,
}

/// GET /auth/session — the checkLogin successor. A dead or absent bearer is
/// data (`{is_login: false}`), not an error; infrastructure failures still
/// surface as 500s.
pub(crate) async fn session_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionState>, Problem> {
    let locale = request_locale(&headers);
    match require_user(&state, &headers).await {
        Ok(user) => Ok(Json(SessionState {
            is_login: true,
            is_admin: (user.is_admin != 0).then_some(true),
        })),
        Err(error) if error.is_session_expired() => Ok(Json(SessionState {
            is_login: false,
            is_admin: None,
        })),
        Err(error) => Err(problem_from(error, locale)),
    }
}

/// DELETE /auth/session — explicit sign-out (204). A dead or absent bearer
/// stays a successful no-op so the client's fire-and-forget teardown is
/// idempotent (§5.2).
pub(crate) async fn session_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    if let Some(auth_data) = select_auth_data(&headers) {
        let auth = state.auth_service();
        auth.logout(&auth_data)
            .await
            .map_err(|error| problem_from(error, locale))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InviteViewRequest {
    invite_code: String,
}

/// POST /public/invite-views — unauthenticated invite-view telemetry
/// (docs/api-dialect.md §5.1, W3): JSON `{invite_code}`, 204 on success
/// (the legacy `{data: true}` body dies).
pub(crate) async fn public_invite_views(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<InviteViewRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let auth = state.auth_service();
    auth.passport_pv(Some(payload.invite_code.as_str()))
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Authenticate the shared session extractor path. Missing, malformed, or
/// dead credentials are the global 401 `session_expired` problem (§3.2) with
/// the RFC 6750 challenge: bare `Bearer` when the request carried no
/// `Authorization` header at all, `error="invalid_token"` otherwise.
pub(crate) async fn require_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthUser, ApiError> {
    let locale = request_locale(headers);
    let Some(auth_data) = select_auth_data(headers) else {
        let mut problem = Problem::localized(Code::SessionExpired, locale);
        if !headers.contains_key(axum::http::header::AUTHORIZATION) {
            problem = problem.missing_credentials();
        }
        return Err(problem.into());
    };
    let auth = state.auth_service();
    auth.user_from_auth_data(&auth_data)
        .await
        .map_err(|error| error.relocalize_problem(locale))
}

/// The Authorization header is the only accepted bearer transport, and since
/// W2 the value must carry the `Bearer ` scheme (§4.2) — the bare legacy
/// header value is no longer accepted on internal routes. The stored
/// localStorage value stays the raw token; the scheme lives on the wire only.
/// URL `?auth_data=` tokens (a legacy PHP vestige) remain deliberately
/// unsupported: query strings outlive the request in referrers and durable
/// edge logs, while headers stay out of them.
pub(crate) fn select_auth_data(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    if value.len() > BEARER_SCHEME.len() + MAX_AUTH_DATA_BYTES {
        return None;
    }
    let (scheme, token) = value.split_at_checked(BEARER_SCHEME.len())?;
    if !scheme.eq_ignore_ascii_case(BEARER_SCHEME) || token.is_empty() {
        return None;
    }
    Some(token.to_owned())
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
) -> Result<AuthUser, ApiError> {
    let user = require_user(state, headers).await?;
    if user.is_admin == 0 {
        return Err(Problem::localized(Code::PermissionDenied, request_locale(headers)).into());
    }
    Ok(user)
}

pub(crate) async fn require_staff(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthUser, ApiError> {
    let user = require_user(state, headers).await?;
    if user.is_staff == 0 {
        return Err(Problem::localized(Code::PermissionDenied, request_locale(headers)).into());
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
    let locale = request_locale(headers);
    let step_up_required = || ApiError::from(Problem::localized(Code::StepUpRequired, locale));
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
        return Err(step_up_required());
    }
    let config = state.config_snapshot();
    if recent_password_authentication(
        user,
        chrono::Utc::now().timestamp(),
        config.privileged_step_up_ttl_seconds,
    ) {
        return Ok(());
    }
    Err(step_up_required())
}

fn recent_password_authentication(user: &AuthUser, now: i64, ttl_seconds: u64) -> bool {
    user.password_authenticated
        && user.authenticated_at <= now
        && now.saturating_sub(user.authenticated_at) <= ttl_seconds as i64
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::{
        body::{Body, to_bytes},
        extract::{ConnectInfo, Request},
        http::{HeaderMap, StatusCode, header},
    };
    use tower::ServiceExt as _;
    use v2board_config::AppConfig;

    use super::{
        MAX_USER_AGENT_BYTES, bounded_user_agent, recent_password_authentication, select_auth_data,
    };
    use v2board_domain::auth::AuthUser;

    fn with_loopback_peer(mut request: Request) -> Request {
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))));
        request
    }

    fn service_free_app() -> axum::Router {
        let mut config = AppConfig::from_api_env();
        config.trusted_proxy_cidrs = Vec::new();
        config.force_https = false;
        let state = crate::runtime::AppState::service_free_test(config.clone());
        crate::routes::build_app(state, &config)
    }

    /// The W2 global flips on the shared extractor (docs/api-dialect.md §4.2,
    /// §3.2): only a `Bearer `-schemed Authorization header is a credential,
    /// and every unauthenticated internal request is a 401 `session_expired`
    /// problem with the RFC 6750 challenge — exact header bytes pinned here.
    #[tokio::test]
    async fn bearer_authorization_is_the_only_authentication_source() {
        let app = service_free_app();

        // No credentials at all: 401 problem+json with the bare challenge.
        let no_credentials = Request::builder()
            .uri("/api/v1/user/profile?auth_data=url-query-token")
            .body(Body::empty())
            .unwrap();
        let response = app
            .clone()
            .oneshot(with_loopback_peer(no_credentials))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Bearer",
            "a credential-less 401 must carry the bare Bearer challenge"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "session_expired");
        assert_eq!(body["status"], 401);
        // Default locale stays zh-CN (§4.3).
        assert_eq!(body["detail"], "未登录或登陆已过期");

        // A raw legacy header value (no scheme) is no longer accepted: it
        // must fail closed before any session lookup, with the
        // `invalid_token` challenge because credentials were presented.
        let raw_token = Request::builder()
            .uri("/api/v1/user/profile")
            .header(header::AUTHORIZATION, "raw-legacy-token")
            .header(header::ACCEPT_LANGUAGE, "en-US")
            .body(Body::empty())
            .unwrap();
        let response = app
            .clone()
            .oneshot(with_loopback_peer(raw_token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Bearer error=\"invalid_token\""
        );
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "session_expired");
        // Accept-Language resolved the detail at construction time (§4.3).
        assert_eq!(
            body["detail"],
            "Your session has expired, please sign in again"
        );

        // The same request with a Bearer credential passes selection and
        // reaches the (unreachable) session store: authentication is sourced
        // from the schemed header alone.
        let bearer_token = Request::builder()
            .uri("/api/v1/user/profile?auth_data=url-query-token")
            .header(header::AUTHORIZATION, "Bearer header-token")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(with_loopback_peer(bearer_token)).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "the Bearer credential must be what reaches the session store"
        );
    }

    /// §5.2: a dead or absent bearer keeps DELETE /auth/session a successful
    /// no-op — the fire-and-forget client teardown must stay idempotent.
    #[tokio::test]
    async fn session_delete_without_credentials_is_a_no_op_204() {
        let app = service_free_app();
        let request = Request::builder()
            .method("DELETE")
            .uri("/api/v1/auth/session")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(with_loopback_peer(request)).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    /// §5.2: the session probe reports a missing session as data, not an
    /// error — no teardown-triggering 401 from the probe itself.
    #[tokio::test]
    async fn session_probe_reports_absent_credentials_as_logged_out() {
        let app = service_free_app();
        let request = Request::builder()
            .uri("/api/v1/auth/session")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(with_loopback_peer(request)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), b"{\"is_login\":false}");
    }

    /// §5.2: the legacy neither-param token2Login "empty 200" branch dies —
    /// GET /auth/quick-login without a token is a 422 validation problem.
    #[tokio::test]
    async fn quick_login_without_a_token_is_a_validation_problem() {
        let app = service_free_app();
        let request = Request::builder()
            .uri("/api/v1/auth/quick-login")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(with_loopback_peer(request)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(body["errors"]["token"][0], "The token field is required");
    }

    /// §4.1/§4.4: modern auth requests are JSON with `deny_unknown_fields` —
    /// a form-encoded legacy body is a 400 `invalid_parameter` problem and a
    /// typo'd field is a 422 `validation_failed` problem.
    #[tokio::test]
    async fn login_rejects_legacy_form_bodies_and_unknown_fields_as_problems() {
        let app = service_free_app();
        let form = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("email=a@b.c&password=password123"))
            .unwrap();
        let response = app.clone().oneshot(with_loopback_peer(form)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "invalid_parameter");

        let typo = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                "{\"email\":\"a@b.c\",\"password\":\"password123\",\"pasword\":\"x\"}",
            ))
            .unwrap();
        let response = app.oneshot(with_loopback_peer(typo)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "validation_failed");
    }

    #[test]
    fn select_auth_data_requires_and_strips_the_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer header-token".parse().unwrap(),
        );
        assert_eq!(select_auth_data(&headers).as_deref(), Some("header-token"));

        // RFC 9110: the auth scheme is case-insensitive.
        headers.insert(
            header::AUTHORIZATION,
            "bearer header-token".parse().unwrap(),
        );
        assert_eq!(select_auth_data(&headers).as_deref(), Some("header-token"));

        // The bare legacy value (no scheme) is no longer a credential (§4.2).
        headers.insert(header::AUTHORIZATION, "header-token".parse().unwrap());
        assert_eq!(select_auth_data(&headers), None);

        headers.insert(header::AUTHORIZATION, "Bearer ".parse().unwrap());
        assert_eq!(
            select_auth_data(&headers),
            None,
            "an empty token is not a credential"
        );

        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", "x".repeat(super::MAX_AUTH_DATA_BYTES + 1))
                .parse()
                .unwrap(),
        );
        assert_eq!(
            select_auth_data(&headers),
            None,
            "an oversized Authorization header must fail closed"
        );
        headers.remove(header::AUTHORIZATION);
        assert_eq!(select_auth_data(&headers), None);
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
