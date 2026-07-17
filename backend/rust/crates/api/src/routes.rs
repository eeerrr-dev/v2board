use std::time::Duration;

use axum::{
    Router,
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, Method, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveHeadersLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::RequestBodyDeadlineLayer;
use tower_http::trace::TraceLayer;
use v2board_config::AppConfig;

use crate::{
    localization::language_middleware,
    runtime::{
        AppState, enforce_https_middleware, request_timeout_middleware,
        trusted_client_ip_middleware,
    },
};

const X_REQUEST_ID: &str = "x-request-id";
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Preserve cross-origin API clients while keeping authentication explicit in
/// the Authorization header. The API has no cookie-auth contract, so enabling
/// ambient credentials would only broaden future CSRF exposure.
fn cors_layer(state: AppState) -> CorsLayer {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS, Method::HEAD])
        // No `Origin` entry: browsers forbid it in Access-Control-Request-Headers, so
        // advertising it (a verbatim transplant of legacy CORS.php) was inert.
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::AUTHORIZATION,
            HeaderName::from_static(X_REQUEST_ID),
            HeaderName::from_static("x-v2board-step-up"),
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

pub(super) fn build_app(state: AppState, config: &AppConfig) -> Router {
    let admin_api_route = config.admin_api_route();
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
    let request_timeout = Duration::from_secs(config.api_request_timeout_seconds);

    Router::new()
        .nest_service("/assets/user", user_assets)
        .nest_service("/assets/admin", admin_assets)
        .route("/healthz", get(crate::runtime::healthz))
        .route("/readyz", get(crate::runtime::readyz))
        .route(
            "/api/v1/guest/comm/config",
            get(crate::client::guest_config),
        )
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
        .route(
            "/api/v1/passport/auth/register",
            post(crate::auth::register),
        )
        .route("/api/v1/passport/auth/login", post(crate::auth::login))
        .route(
            "/api/v1/passport/auth/token2Login",
            get(crate::auth::token2_login),
        )
        .route(
            "/api/v1/passport/auth/forget",
            post(crate::auth::forget_password),
        )
        .route(
            "/api/v1/passport/auth/stepUp",
            post(crate::auth::privileged_step_up),
        )
        .route(
            "/api/v1/passport/auth/getQuickLoginUrl",
            post(crate::auth::passport_quick_login_url),
        )
        .route(
            "/api/v1/passport/comm/sendEmailVerify",
            post(crate::auth::send_email_verify),
        )
        .route("/api/v1/passport/comm/pv", post(crate::auth::passport_pv))
        .route("/api/v1/user/info", get(crate::user::user_info))
        .route("/api/v1/user/checkLogin", get(crate::user::check_login))
        .route("/api/v1/user/logout", post(crate::user::logout))
        .route("/api/v1/user/getStat", get(crate::user::user_stat))
        .route(
            "/api/v1/user/getSubscribe",
            get(crate::user::user_subscribe),
        )
        .route("/api/v1/user/newPeriod", post(crate::user::user_new_period))
        .route(
            "/api/v1/user/redeemgiftcard",
            post(crate::user::redeem_giftcard),
        )
        .route("/api/v1/user/update", post(crate::user::user_update))
        .route(
            "/api/v1/user/changePassword",
            post(crate::user::change_password),
        )
        .route(
            "/api/v1/user/resetSecurity",
            get(crate::user::reset_security),
        )
        .route(
            "/api/v1/user/unbindTelegram",
            get(crate::user::unbind_telegram),
        )
        .route("/api/v1/user/transfer", post(crate::user::user_transfer))
        .route(
            "/api/v1/user/getQuickLoginUrl",
            post(crate::user::user_quick_login_url),
        )
        .route(
            "/api/v1/user/getActiveSession",
            get(crate::user::active_sessions),
        )
        .route(
            "/api/v1/user/removeActiveSession",
            post(crate::user::remove_active_session),
        )
        .route("/api/v1/user/plan/fetch", get(crate::user::user_plan_fetch))
        .route("/api/v1/user/order/save", post(crate::commerce::order_save))
        .route(
            "/api/v1/user/order/checkout",
            post(crate::commerce::order_checkout),
        )
        .route(
            "/api/v1/user/order/stripe/intent",
            post(crate::commerce::stripe_payment_intent),
        )
        .route(
            "/api/v1/user/order/fetch",
            get(crate::commerce::order_fetch),
        )
        .route(
            "/api/v1/user/order/detail",
            get(crate::commerce::order_detail),
        )
        .route(
            "/api/v1/user/order/check",
            get(crate::commerce::order_check),
        )
        .route(
            "/api/v1/user/order/cancel",
            post(crate::commerce::order_cancel),
        )
        .route(
            "/api/v1/user/order/getPaymentMethod",
            get(crate::commerce::order_payment_methods),
        )
        .route("/api/v1/user/invite/save", get(crate::user::invite_save))
        .route("/api/v1/user/invite/fetch", get(crate::user::invite_fetch))
        .route(
            "/api/v1/user/invite/details",
            get(crate::user::invite_details),
        )
        .route(
            "/api/v1/user/ticket/fetch",
            get(crate::ticket::ticket_fetch),
        )
        .route("/api/v1/user/ticket/save", post(crate::ticket::ticket_save))
        .route(
            "/api/v1/user/ticket/reply",
            post(crate::ticket::ticket_reply),
        )
        .route(
            "/api/v1/user/ticket/close",
            post(crate::ticket::ticket_close),
        )
        .route(
            "/api/v1/user/ticket/withdraw",
            post(crate::ticket::ticket_withdraw),
        )
        .route("/api/v1/user/server/fetch", get(crate::user::server_fetch))
        .route(
            "/api/v1/user/coupon/check",
            post(crate::commerce::coupon_check),
        )
        .route(
            "/api/v1/user/knowledge/fetch",
            get(crate::user::knowledge_fetch),
        )
        .route(
            "/api/v1/user/knowledge/getCategory",
            get(crate::user::knowledge_categories),
        )
        .route(
            "/api/v1/user/notice/fetch",
            get(crate::user::user_notice_fetch),
        )
        .route(
            "/api/v1/user/telegram/getBotInfo",
            get(crate::user::telegram_bot_info),
        )
        .route(
            "/api/v1/user/comm/config",
            get(crate::user::user_comm_config),
        )
        .route(
            "/api/v1/user/stat/getTrafficLog",
            get(crate::user::user_traffic_logs),
        )
        .route(
            &admin_api_route,
            get(crate::admin::admin_get).post(crate::admin::admin_post),
        )
        .route(
            "/api/v1/staff/{*staff_path}",
            get(crate::admin::staff_get).post(crate::admin::staff_post),
        )
        .route(
            "/api/v1/server/{class}/{action}",
            get(crate::server_api::server_v1).post(crate::server_api::server_v1),
        )
        .route(
            "/api/v2/server/config",
            get(crate::server_api::server_v2_config).post(crate::server_api::server_v2_config),
        )
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
                tracing::info_span!(
                    "http.request",
                    method = %request.method(),
                    path = request.uri().path(),
                    request_id = request_id,
                    client_ip = client_ip,
                )
            }),
        )
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
    // This CSP directive is additive to X-Frame-Options and deliberately does
    // not constrain script/style sources, so operator-provided custom HTML
    // remains compatible while modern browsers still enforce frame isolation.
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("frame-ancestors 'self'"),
    );
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
    use axum::http::{HeaderMap, HeaderValue};

    use super::{
        X_REQUEST_ID, apply_security_response_headers, cors_origin_allowed,
        is_content_hashed_asset, valid_request_id, valid_request_id_header,
    };

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
        apply_security_response_headers("/api/v1/passport/auth/login", &mut headers);
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
    }
}
