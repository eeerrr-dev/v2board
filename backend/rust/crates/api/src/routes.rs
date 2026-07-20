use std::time::Duration;

use axum::{
    Router,
    extract::Request,
    handler::Handler,
    http::{HeaderMap, HeaderName, HeaderValue, Method, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{MethodFilter, get, on, post},
};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveHeadersLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::RequestBodyDeadlineLayer;
use tower_http::trace::TraceLayer;
use v2board_api_contract::{HttpMethod, OperationSurface, operation};
use v2board_config::AppConfig;

use crate::{
    localization::language_middleware,
    runtime::{
        AppState, enforce_https_middleware, request_timeout_middleware,
        trusted_client_ip_middleware,
    },
};

pub(crate) const X_REQUEST_ID: &str = "x-request-id";
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024 * 1024;

pub(crate) fn bind_internal_operation<H, T>(
    router: Router<AppState>,
    expected_surface: OperationSurface,
    id: &'static str,
    handler: H,
) -> Router<AppState>
where
    H: Handler<T, AppState>,
    T: 'static,
{
    let operation = operation(id).unwrap_or_else(|| panic!("unregistered internal operation {id}"));
    assert_eq!(
        operation.surface, expected_surface,
        "internal operation {id} was mounted into the wrong surface"
    );
    let filter = match operation.method {
        HttpMethod::Get => MethodFilter::GET,
        HttpMethod::Post => MethodFilter::POST,
        HttpMethod::Put => MethodFilter::PUT,
        HttpMethod::Patch => MethodFilter::PATCH,
        HttpMethod::Delete => MethodFilter::DELETE,
    };
    let path = operation.runtime_path();
    router.route(path.as_ref(), on(filter, handler))
}

/// Preserve cross-origin API clients while keeping authentication explicit in
/// the Authorization header. The API has no cookie-auth contract, so enabling
/// ambient credentials would only broaden future CSRF exposure.
fn cors_layer(state: AppState) -> CorsLayer {
    let cors = CorsLayer::new()
        // docs/api-dialect.md §4.2: the modern dialect uses resource-shaped
        // methods (DELETE /auth/session today; PATCH/PUT as families migrate).
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::HEAD,
        ])
        // No `Origin` entry: browsers forbid it in Access-Control-Request-Headers, so
        // advertising it (a verbatim transplant of legacy CORS.php) was inert.
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::AUTHORIZATION,
            HeaderName::from_static(X_REQUEST_ID),
            HeaderName::from_static("x-v2board-step-up"),
            HeaderName::from_static("idempotency-key"),
        ])
        .expose_headers([HeaderName::from_static(X_REQUEST_ID)])
        // 7200s is Chromium's preflight cache cap; the legacy 10080 was a
        // minutes literal shipped as seconds.
        .max_age(Duration::from_secs(7200));
    cors.allow_origin(AllowOrigin::predicate(move |origin, _request| {
        cors_origin_allowed(&state.config_snapshot().cors_allowed_origins, origin)
    }))
}

fn cors_origin_allowed(allowed_origins: &[String], origin: &HeaderValue) -> bool {
    allowed_origins
        .iter()
        .any(|allowed| allowed.as_bytes() == origin.as_bytes())
}

define_internal_operation_router! {
    pub(crate) fn fixed_internal_router;
    pub(crate) const FIXED_INTERNAL_OPERATION_IDS;
    {
        "public.config" [Public] => crate::client::public_config,
        "public.invite-views.create" [Public] => crate::auth::public_invite_views,
        "auth.register" [Auth] => crate::auth::register,
        "auth.login" [Auth] => crate::auth::login,
        "auth.quick-login" [Auth] => crate::auth::quick_login,
        "auth.token-login" [Auth] => crate::auth::token_login,
        "auth.password-reset" [Auth] => crate::auth::password_reset,
        "auth.step-up" [Auth] => crate::auth::step_up,
        "auth.quick-login-url" [Auth] => crate::auth::quick_login_url,
        "auth.email-codes" [Auth] => crate::auth::email_codes,
        "auth.session.get" [Auth] => crate::auth::session_get,
        "auth.session.delete" [Auth] => crate::auth::session_delete,
        "user.profile.get" [User] => crate::user::user_profile,
        "user.profile.update" [User] => crate::user::user_profile_update,
        "user.password.update" [User] => crate::user::user_password_update,
        "user.stats.get" [User] => crate::user::user_stats,
        "user.sessions.list" [User] => crate::user::user_sessions,
        "user.sessions.delete" [User] => crate::user::user_session_delete,
        "user.commission-transfers.create" [User] => crate::user::commission_transfer_create,
        "user.gift-card-redemptions.create" [User] => crate::user::gift_card_redemption_create,
        "user.telegram-binding.delete" [User] => crate::user::user_telegram_binding_delete,
        "user.telegram-bot.get" [User] => crate::user::telegram_bot,
        "user.config.get" [User] => crate::user::user_config,
        "user.subscription.get" [User] => crate::user::user_subscription,
        "user.subscription.new-period" [User] => crate::user::subscription_new_period,
        "user.subscription.reset-token" [User] => crate::user::subscription_reset_token,
        "user.servers.list" [User] => crate::user::user_servers,
        "user.traffic-logs.list" [User] => crate::user::user_traffic_logs,
        "user.plans.get" [User] => crate::commerce::plan_detail,
        "user.plans.list" [User] => crate::commerce::plans_list,
        "user.orders.create" [User] => crate::commerce::order_create,
        "user.orders.list" [User] => crate::commerce::orders_list,
        "user.orders.get" [User] => crate::commerce::order_detail,
        "user.orders.status" [User] => crate::commerce::order_status,
        "user.orders.cancel" [User] => crate::commerce::order_cancel,
        "user.orders.checkout" [User] => crate::commerce::order_checkout,
        "user.orders.stripe-intent" [User] => crate::commerce::order_stripe_intent,
        "user.payment-methods.list" [User] => crate::commerce::payment_methods,
        "user.coupons.check" [User] => crate::commerce::coupon_check,
        "user.invite-codes.create" [User] => crate::user::invite_code_create,
        "user.invite.get" [User] => crate::user::invite_get,
        "user.commissions.list" [User] => crate::user::commissions_list,
        "user.tickets.get" [User] => crate::ticket::ticket_detail,
        "user.tickets.list" [User] => crate::ticket::tickets_list,
        "user.tickets.create" [User] => crate::ticket::ticket_create,
        "user.tickets.replies.create" [User] => crate::ticket::ticket_reply_create,
        "user.tickets.close" [User] => crate::ticket::ticket_close,
        "user.withdrawal-tickets.create" [User] => crate::ticket::withdrawal_ticket_create,
        "user.knowledge.get" [User] => crate::user::knowledge_detail,
        "user.knowledge.list" [User] => crate::user::knowledge_list,
        "user.knowledge-categories.list" [User] => crate::user::knowledge_categories,
        "user.notices.list" [User] => crate::user::user_notices,
    }
}

pub(super) fn build_app(state: AppState, config: &AppConfig) -> Router {
    let user_assets = ServeDir::new(config.runtime_paths.frontend.join("current/user"))
        .append_index_html_on_directories(false)
        .fallback(
            ServeDir::new(config.runtime_paths.frontend.join("previous/user"))
                .append_index_html_on_directories(false),
        );
    let admin_assets = ServeDir::new(config.runtime_paths.frontend.join("current/admin"))
        .append_index_html_on_directories(false)
        .fallback(
            ServeDir::new(config.runtime_paths.frontend.join("previous/admin"))
                .append_index_html_on_directories(false),
        );
    let request_id_header = HeaderName::from_static(X_REQUEST_ID);
    let middleware_state = state.clone();
    let https_state = state.clone();
    let timeout_state = state.clone();
    let cors_state = state.clone();
    let http_metrics_state = state.clone();
    let rate_limit_state = state.clone();
    let request_timeout = Duration::from_secs(config.api_request_timeout_seconds);

    Router::new()
        .nest_service("/assets/user", user_assets)
        .nest_service("/assets/admin", admin_assets)
        .route("/healthz", get(crate::runtime::healthz))
        .route("/readyz", get(crate::runtime::readyz))
        .route("/metrics", get(crate::metrics::metrics))
        .route("/robots.txt", get(crate::frontend::robots_txt))
        // All modern internal fixed-prefix routes derive method and path from
        // the Rust-owned operation registry. Frozen external routes remain
        // explicit below because their legacy wire contract is intentionally
        // outside the modern internal OpenAPI document.
        .merge(fixed_internal_router())
        .route(
            "/api/v1/guest/payment/notify/{method}/{uuid}",
            get(crate::client::payment_notify).post(crate::client::payment_notify),
        )
        .route(
            "/api/v1/guest/telegram/webhook",
            post(crate::telegram::telegram_webhook),
        )
        .route(
            "/api/v1/client/subscribe",
            get(crate::client::client_subscribe),
        )
        .route(
            "/api/v1/client/app/getConfig",
            get(crate::client::client_app_config),
        )
        .route(
            "/api/v1/client/app/getVersion",
            get(crate::client::client_app_version),
        )
        // The admin API has no boot-time literal route: every method under
        // the live `/api/v1/{secure_path}/` prefix re-dispatches through
        // `dynamic_fallback` into the nested method-aware admin router
        // (docs/api-dialect.md §6 preamble), so a runtime `secure_path` save
        // takes effect without a restart.
        // The §6.9 staff namespace keeps its fixed prefix, so it nests as a
        // boot-time method-aware router (unlike the admin prefix above).
        .nest_service("/api/v1/staff", crate::admin::staff_router(state.clone()))
        .route(
            "/api/v1/server/{class}/{action}",
            get(crate::server_api::server_v1).post(crate::server_api::server_v1),
        )
        .route(
            "/api/v2/server/config",
            get(crate::server_api::server_v2_config).post(crate::server_api::server_v2_config),
        )
        // route_layer runs only after routing succeeded, so the bounded
        // MatchedPath template is available for per-route counters; fallback
        // dispatch (admin API, SPA documents) is covered by the family
        // histogram in http_metrics_middleware instead.
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::metrics::route_metrics_middleware,
        ))
        .fallback(crate::fallback::dynamic_fallback)
        .with_state(state)
        .layer(middleware::from_fn(cache_static_assets))
        .layer(middleware::from_fn(security_response_headers))
        .layer(middleware::from_fn(language_middleware))
        .layer(CompressionLayer::new())
        .layer(cors_layer(cors_state))
        .layer(middleware::from_fn_with_state(
            timeout_state,
            request_timeout_middleware,
        ))
        .layer(RequestBodyDeadlineLayer::new(request_timeout))
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES))
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request| {
                let request_id = request
                    .headers()
                    .get(X_REQUEST_ID)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("missing");
                let client_ip = request
                    .extensions()
                    .get::<crate::runtime::ClientIp>()
                    .map(|client_ip| client_ip.0.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let span = tracing::info_span!(
                    "http.request",
                    method = %request.method(),
                    path = request.uri().path(),
                    request_id = request_id,
                    client_ip = client_ip,
                );
                // W3C trace context: adopt an incoming `traceparent` as the
                // remote parent only when OTLP export is on; the disabled
                // default stays a plain local span with zero extra work.
                if crate::runtime::otel_enabled() {
                    use tracing_opentelemetry::OpenTelemetrySpanExt;
                    let parent = opentelemetry::global::get_text_map_propagator(|propagator| {
                        propagator.extract(&crate::runtime::HeaderCarrier(request.headers()))
                    });
                    // Fails only for a closed/disabled span; nothing to do.
                    let _ = span.set_parent(parent);
                }
                span
            }),
        )
        // Inner to trusted_client_ip_middleware so the resolved ClientIp
        // extension is available; only the unauthenticated internal families
        // are counted (crate::rate_limit module docs).
        .layer(middleware::from_fn_with_state(
            rate_limit_state,
            crate::rate_limit::http_rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            https_state,
            enforce_https_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            middleware_state,
            trusted_client_ip_middleware,
        ))
        .layer(SetSensitiveHeadersLayer::new([
            header::AUTHORIZATION,
            header::COOKIE,
            header::SET_COOKIE,
            HeaderName::from_static("cf-connecting-ip"),
            HeaderName::from_static("x-telegram-bot-api-secret-token"),
            HeaderName::from_static("x-v2board-server-token"),
            HeaderName::from_static("x-v2board-step-up"),
        ]))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(middleware::from_fn(sanitize_request_id))
        // Outermost so the status-class counters see every response,
        // including ones short-circuited by the layers below.
        .layer(middleware::from_fn_with_state(
            http_metrics_state,
            crate::metrics::http_metrics_middleware,
        ))
}

async fn security_response_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    apply_security_response_headers(&path, response.headers_mut());
    response
}

fn apply_security_response_headers(path: &str, headers: &mut HeaderMap) {
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    // Baseline frame isolation for API and asset responses. HTML documents
    // carry the full §10.5 policy set by `frontend::render`, which must win:
    // only fill the header when the handler has not already claimed it.
    headers
        .entry(header::CONTENT_SECURITY_POLICY)
        .or_insert(HeaderValue::from_static("frame-ancestors 'self'"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    if path.starts_with("/api/") {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        );
        headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
    }
}

async fn sanitize_request_id(mut request: Request, next: Next) -> Response {
    if !valid_request_id_header(request.headers()) {
        request.headers_mut().remove(X_REQUEST_ID);
    }
    next.run(request).await
}

fn valid_request_id_header(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(X_REQUEST_ID).iter();
    match (values.next(), values.next()) {
        (None, _) => true,
        (Some(value), None) => value.to_str().ok().is_some_and(valid_request_id),
        (Some(_), Some(_)) => false,
    }
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

async fn cache_static_assets(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let asset = path
        .strip_prefix("/assets/user/")
        .or_else(|| path.strip_prefix("/assets/admin/"));
    if asset.is_some_and(|asset| !is_content_hashed_asset(asset)) {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    let immutable = asset.is_some();
    let mut response = next.run(request).await;
    if immutable && response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            "public, max-age=31536000, immutable"
                .parse()
                .expect("cache-control value is valid"),
        );
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            "nosniff".parse().expect("nosniff value is valid"),
        );
    }
    response
}

/// Runtime gate for `/assets/{user,admin}/*` names. The build-time grammar in
/// frontend/scripts/deploy-contract.mjs (`hashedAssetNamePattern`) is a strict
/// subset of this parse; keep it that way so a build-certified release can
/// never contain a runtime-unservable filename.
fn is_content_hashed_asset(path: &str) -> bool {
    if path.is_empty() || path.contains('/') || path.contains('\\') {
        return false;
    }
    let Some((stem, extension)) = path.rsplit_once('.') else {
        return false;
    };
    !extension.is_empty()
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        && stem.match_indices('-').any(|(separator, _)| {
            let name = &stem[..separator];
            let hash = &stem[separator + 1..];
            !name.is_empty()
                && hash.len() >= 8
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, net::SocketAddr};

    use axum::{
        body::{Body, to_bytes},
        extract::{ConnectInfo, Request},
        http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    };
    use tower::ServiceExt as _;
    use v2board_config::AppConfig;

    use super::{
        X_REQUEST_ID, apply_security_response_headers, build_app, cors_origin_allowed,
        is_content_hashed_asset, valid_request_id, valid_request_id_header,
    };
    use crate::runtime::AppState;

    const ALLOWED_ORIGIN: &str = "https://app.example.test";

    #[test]
    fn every_internal_registry_operation_is_bound_exactly_once() {
        // Construction itself is part of the assertion: Axum rejects
        // conflicting registrations for the same method/path.
        let _router = super::fixed_internal_router();
        let bound = super::FIXED_INTERNAL_OPERATION_IDS
            .iter()
            .chain(crate::admin::ADMIN_INTERNAL_OPERATION_IDS)
            .chain(crate::admin::STAFF_INTERNAL_OPERATION_IDS)
            .copied()
            .collect::<Vec<_>>();
        let unique = bound.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            bound.len(),
            "duplicate internal handler binding"
        );

        let expected = v2board_api_contract::INTERNAL_OPERATIONS
            .iter()
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(unique, expected);
    }

    /// Builds the real production router with service-free process dependencies:
    /// the PostgreSQL pool and the Redis connection manager are lazy, so every
    /// request below must be answered before any backing service is touched.
    fn cors_test_app() -> axum::Router {
        let mut config = AppConfig::from_api_env();
        config.cors_allowed_origins = vec![ALLOWED_ORIGIN.to_string()];
        // The loopback test peer is the client itself: no proxy resolution and
        // no HTTPS redirect may run before the CORS layer.
        config.trusted_proxy_cidrs = Vec::new();
        config.force_https = false;
        let state = AppState::service_free_test(config.clone());
        build_app(state, &config)
    }

    fn with_loopback_peer(mut request: Request) -> Request {
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))));
        request
    }

    fn header_str<'a>(headers: &'a HeaderMap, name: &HeaderName) -> &'a str {
        headers
            .get(name)
            .unwrap_or_else(|| panic!("missing header {name}"))
            .to_str()
            .expect("ascii header value")
    }

    #[tokio::test]
    async fn preflight_from_an_allowlisted_origin_is_answered_without_credentials() {
        let app = cors_test_app();
        let request = with_loopback_peer(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/auth/login")
                .header(header::ORIGIN, ALLOWED_ORIGIN)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "authorization, idempotency-key, x-v2board-step-up",
                )
                .body(Body::empty())
                .unwrap(),
        );
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers().clone();
        assert_eq!(
            headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            ALLOWED_ORIGIN
        );
        let allow_methods = header_str(&headers, &header::ACCESS_CONTROL_ALLOW_METHODS);
        // §4.2: the modern dialect's resource methods are advertised.
        for method in ["GET", "POST", "PATCH", "PUT", "DELETE"] {
            assert!(
                allow_methods.contains(method),
                "missing {method} in {allow_methods}"
            );
        }
        let allow_headers =
            header_str(&headers, &header::ACCESS_CONTROL_ALLOW_HEADERS).to_ascii_lowercase();
        assert!(allow_headers.contains("authorization"));
        assert!(allow_headers.contains("idempotency-key"));
        assert!(allow_headers.contains("x-v2board-step-up"));
        // Origin is a browser-forbidden request header: it can never appear in
        // Access-Control-Request-Headers, so it must not be advertised either.
        assert!(!allow_headers.split(',').any(|name| name.trim() == "origin"));
        assert_eq!(
            header_str(&headers, &header::ACCESS_CONTROL_MAX_AGE),
            "7200"
        );
        // Pins the db6bf00b regression: the API has no cookie-auth contract, so
        // the preflight must never grant ambient credentials.
        assert!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
        // The CORS layer must short-circuit the preflight instead of letting it
        // fall through to the router/fallback, which would produce a body.
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn preflight_from_a_disallowed_origin_gets_no_allow_origin() {
        let app = cors_test_app();
        let request = with_loopback_peer(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/auth/login")
                .header(header::ORIGIN, "https://evil.example.test")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        );
        let response = app.oneshot(request).await.unwrap();

        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
    }

    #[tokio::test]
    async fn simple_cross_origin_post_carries_allow_origin_on_the_localized_error() {
        let app = cors_test_app();
        let request = with_loopback_peer(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/login")
                .header(header::ORIGIN, ALLOWED_ORIGIN)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    "{\"email\":\"not-an-email\",\"password\":\"password123\"}",
                ))
                .unwrap(),
        );
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            ALLOWED_ORIGIN
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // The default-locale zh-CN detail proves the request traversed the
        // full middleware stack down to the handler's construction-time
        // localized validation problem (docs/api-dialect.md §3.1/§4.3).
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(body["detail"], "邮箱格式不正确");
        assert_eq!(body["errors"]["email"][0], "邮箱格式不正确");
    }

    #[test]
    fn public_asset_gate_accepts_only_flat_content_hashed_files() {
        // The accepted corpus is mirrored byte-for-byte by the build-time
        // grammar test (frontend/scripts/deploy-contract.test.mjs). The build
        // regex is a strict subset of this gate, so every name it certifies
        // must be accepted here.
        assert!(is_content_hashed_asset("index-Dp3_abcdef.js"));
        assert!(is_content_hashed_asset("asset-a1b2c3d4.woff2"));
        assert!(is_content_hashed_asset("logo.dark-a1b2c3d4.png"));
        assert!(is_content_hashed_asset(
            "roboto-v30-latin-regular-a1b2c3d4.woff2"
        ));
        assert!(!is_content_hashed_asset("index.html"));
        assert!(!is_content_hashed_asset("manifest.json"));
        assert!(!is_content_hashed_asset("umi.js"));
        assert!(!is_content_hashed_asset("nested/index-a1b2c3d4.js"));
        assert!(!is_content_hashed_asset("../index-a1b2c3d4.js"));
        // Dotted extension chains land the extra segment in the hash bytes
        // and are rejected; the build regex refuses to certify them too.
        assert!(!is_content_hashed_asset("chunk-abcdefgh.js.map"));
        assert!(!is_content_hashed_asset("asset-a1b2c3d4.js.LICENSE.txt"));
        assert!(!is_content_hashed_asset("index-abc.js"));
        assert!(!is_content_hashed_asset("index-abcdefgh."));
        assert!(!is_content_hashed_asset("-abcdefgh.js"));
    }

    #[test]
    fn request_ids_are_bounded_and_log_safe() {
        assert!(valid_request_id("018f8f3c-2d31-7dd1-a2f1.trace_9"));
        assert!(!valid_request_id(""));
        assert!(!valid_request_id("contains a space"));
        assert!(!valid_request_id("contains/query"));
        assert!(!valid_request_id(&"a".repeat(129)));
    }

    #[test]
    fn ambiguous_duplicate_request_ids_are_replaced() {
        let mut headers = HeaderMap::new();
        headers.append(X_REQUEST_ID, HeaderValue::from_static("first"));
        headers.append(X_REQUEST_ID, HeaderValue::from_static("second"));
        assert!(!valid_request_id_header(&headers));
    }

    #[test]
    fn cors_origin_match_uses_the_current_exact_allowlist() {
        let origin = HeaderValue::from_static("https://app.example.test");
        assert!(cors_origin_allowed(
            &["https://app.example.test".to_string()],
            &origin
        ));
        assert!(!cors_origin_allowed(
            &["https://other.example.test".to_string()],
            &origin
        ));
        assert!(!cors_origin_allowed(&[], &origin));
    }

    #[test]
    fn api_responses_are_never_cacheable_and_all_responses_are_hardened() {
        let mut headers = HeaderMap::new();
        apply_security_response_headers("/api/v1/auth/login", &mut headers);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store, max-age=0");
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
        assert_eq!(
            headers.get("content-security-policy").unwrap(),
            "frame-ancestors 'self'"
        );

        let mut asset_headers = HeaderMap::new();
        apply_security_response_headers("/assets/user/index-deadbeef.js", &mut asset_headers);
        assert!(asset_headers.get("cache-control").is_none());
        assert_eq!(asset_headers.get("x-frame-options").unwrap(), "SAMEORIGIN");

        // A handler-claimed CSP (the full §10.5 HTML document policy) must
        // survive the middleware baseline.
        let mut html_headers = HeaderMap::new();
        html_headers.insert(
            "content-security-policy",
            HeaderValue::from_static("default-src 'self'"),
        );
        apply_security_response_headers("/", &mut html_headers);
        assert_eq!(
            html_headers.get("content-security-policy").unwrap(),
            "default-src 'self'"
        );
    }
}
