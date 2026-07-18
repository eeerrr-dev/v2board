use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method},
    response::{IntoResponse, Response},
};
use v2board_compat::{ApiError, Code, Problem};

use crate::{
    admin::dispatch_admin,
    client::{ClientSubscribeQuery, client_subscribe_response},
    frontend,
    route_paths::{custom_subscribe_route_path, normalize_request_path},
    runtime::AppState,
};

/// Subtree HTML fallback (docs/api-dialect.md §10.2). Precedence:
///
/// 1. Operator `subscribe_path` alias (GET/HEAD) — reserved, never HTML.
/// 2. Live-prefix admin API dispatch under `/api/v1/{admin_path}/` for
///    **every** method (GET/POST/PATCH/PUT/DELETE) — resolved per request
///    into the nested method-aware admin router so a runtime `secure_path`
///    change keeps working without a process restart (§6 preamble); rule 4's
///    404 applies only after this dispatch has declined the path.
/// 3. `/api/*` never falls back to HTML: unknown API paths 404 in their
///    namespace's dialect — the legacy `{message}` body under the frozen §2
///    external prefixes, problem+json `endpoint_not_found` everywhere else
///    (the internal migration completed with W14).
/// 4. GET/HEAD: reserved roots (`/assets/*`, `/healthz`, `/readyz`) → modern
///    404; `/{admin_path}` and `/{admin_path}/*` → admin `index.html`; every
///    other path → user `index.html` (safe_mode Host gate inside `render`).
/// 5. Everything else (non-GET/HEAD unmatched) → 404 problem+json
///    `endpoint_not_found`.
pub(crate) async fn dynamic_fallback(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let method = request.method().clone();
    let path = normalize_request_path(request.uri().path());
    let config = state.config_snapshot();

    if matches!(method, Method::GET | Method::HEAD)
        && custom_subscribe_route_path(&config.subscribe_path)
            .as_deref()
            .is_some_and(|subscribe_path| normalize_request_path(subscribe_path) == path)
    {
        let query = serde_urlencoded::from_str::<ClientSubscribeQuery>(
            request.uri().query().unwrap_or_default(),
        )
        .map_err(|_| ApiError::bad_request("Invalid subscribe query"))?;
        return client_subscribe_response(&state, query, headers).await;
    }

    let admin_prefix = format!("/api/v1/{}/", config.admin_path());
    if let Some(admin_path) = path.strip_prefix(&admin_prefix) {
        // §6 preamble: all methods re-dispatch into the nested method-aware
        // admin router; unmatched paths get its problem+json 404
        // `endpoint_not_found` fallback (§10.2 rule 1) — the legacy string
        // dispatch died with W14.
        let admin_path = admin_path.to_string();
        return dispatch_admin(&state, &path, &admin_path, request).await;
    }

    if path == "/api" || path.starts_with("/api/") {
        // §10.2 rule 1: per-namespace dialect 404. Only the frozen §2
        // external prefixes keep the legacy `{message}` body.
        if is_frozen_external_api_path(&path) {
            return Err(ApiError::not_found("Not Found"));
        }
        return Ok(Problem::new(Code::EndpointNotFound).into_response());
    }

    if matches!(method, Method::GET | Method::HEAD) && !is_reserved_non_html_path(&path) {
        let admin_root = format!("/{}", config.admin_path());
        let app = if path == admin_root || path.starts_with(&format!("{admin_root}/")) {
            frontend::FrontendApp::Admin
        } else {
            frontend::FrontendApp::User
        };
        return Ok(frontend::render(&config, app, &method, &headers).await);
    }

    Ok(Problem::new(Code::EndpointNotFound).into_response())
}

/// §10.2 rule 1 roots that must never serve SPA HTML. `/api/*` and the
/// subscribe alias are handled before the HTML branch; `/assets/{user,admin}`
/// and `/healthz`/`/readyz` are normally routed, so this only catches strays
/// such as `/assets`, `/assets/other`, or a trailing-slash probe.
fn is_reserved_non_html_path(path: &str) -> bool {
    path == "/assets" || path.starts_with("/assets/") || path == "/healthz" || path == "/readyz"
}

/// The byte-frozen §2 external namespaces: unknown paths under these
/// prefixes keep the legacy `{message}` 404 because their error bodies are
/// pinned for external consumers (subscription clients, node agents,
/// payment providers, Telegram).
fn is_frozen_external_api_path(path: &str) -> bool {
    path.starts_with("/api/v1/client/")
        || path.starts_with("/api/v1/server/")
        || path.starts_with("/api/v2/server/")
        || path.starts_with("/api/v1/guest/payment/notify/")
        || path == "/api/v1/guest/telegram/webhook"
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};

    use axum::{
        body::{Body, to_bytes},
        extract::{ConnectInfo, Request},
        http::{Method, StatusCode, header},
    };
    use tower::ServiceExt as _;
    use uuid::Uuid;
    use v2board_config::AppConfig;

    use crate::routes::build_app;
    use crate::runtime::AppState;

    /// Minimal deploy tree for `frontend::render`: `current/{user,admin}` with
    /// an index carrying exactly one runtime-config token.
    fn write_frontend_release(root: &Path) {
        for app in ["user", "admin"] {
            let dir = root.join("current").join(app);
            std::fs::create_dir_all(&dir).expect("create frontend release dir");
            std::fs::write(
                dir.join("index.html"),
                format!(
                    "<!doctype html><html><head><script type=\"application/json\" \
                     id=\"runtime-config\">__V2BOARD_RUNTIME_CONFIG__</script></head>\
                     <body>{app}-shell</body></html>"
                ),
            )
            .expect("write frontend index");
        }
    }

    fn temp_frontend_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("v2board-fallback-test-{}", Uuid::new_v4()));
        write_frontend_release(&root);
        root
    }

    fn fallback_test_config(frontend_root: &Path) -> AppConfig {
        let mut config = AppConfig::from_api_env();
        config.runtime_paths.frontend = frontend_root.to_path_buf();
        config.secure_path = Some("boot-admin1".to_string());
        config.frontend_admin_path = None;
        // The loopback test peer is the client itself: no proxy resolution and
        // no HTTPS redirect may run before the fallback.
        config.trusted_proxy_cidrs = Vec::new();
        config.force_https = false;
        config.safe_mode_enable = false;
        config
    }

    fn fallback_test_app(config: &AppConfig) -> (axum::Router, AppState) {
        let state = AppState::service_free_test(config.clone());
        (build_app(state.clone(), config), state)
    }

    fn with_loopback_peer(mut request: Request) -> Request {
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))));
        request
    }

    fn request(method: Method, uri: &str) -> Request {
        with_loopback_peer(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
    }

    async fn body_string(response: axum::response::Response) -> String {
        let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn get_and_head_subtree_paths_fall_back_to_spa_html() {
        let root = temp_frontend_root();
        let config = fallback_test_config(&root);
        let (app, _state) = fallback_test_app(&config);

        for path in ["/", "/order/T123", "/no/such/page", "/dashboard/"] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "GET {path}");
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "text/html; charset=utf-8"
            );
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store, max-age=0"
            );
            // HTML documents carry the full §10.5 policy, not the middleware
            // frame-ancestors baseline.
            let csp = response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(csp.starts_with("default-src 'self'; script-src 'self' 'sha256-"));
            assert!(body_string(response).await.contains("user-shell"));
        }

        for path in ["/boot-admin1", "/boot-admin1/plans/7"] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "GET {path}");
            let body = body_string(response).await;
            assert!(body.contains("admin-shell"));
            assert!(body.contains("\"secure_path\":\"boot-admin1\""));
        }

        let response = app
            .clone()
            .oneshot(request(Method::HEAD, "/order/T123"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert!(body_string(response).await.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn reserved_roots_never_serve_spa_html() {
        let root = temp_frontend_root();
        let config = fallback_test_config(&root);
        let (app, _state) = fallback_test_app(&config);

        // Stray asset/probe paths that miss their routed services get the
        // modern 404, not an HTML fallback.
        for path in ["/assets", "/assets/other", "/healthz/", "/readyz/"] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "GET {path}");
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "application/problem+json"
            );
            assert!(body_string(response).await.contains("endpoint_not_found"));
        }

        // Unknown internal `/api/*` paths are the modern 404 since W14
        // closed the migration…
        for path in ["/api", "/api/v1/unknown", "/api/v9/thing"] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "GET {path}");
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "application/problem+json",
                "modern 404 for {path}"
            );
            assert!(body_string(response).await.contains("endpoint_not_found"));
        }

        // …while unknown paths under the frozen §2 external prefixes keep
        // the legacy `{message}` body forever.
        for path in [
            "/api/v1/client/unknown",
            "/api/v1/server/class/action/extra",
            "/api/v2/server/unknown",
            "/api/v1/guest/payment/notify/x/y/z",
        ] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "GET {path}");
            let body = body_string(response).await;
            assert!(body.contains("\"message\""), "legacy 404 body for {path}");
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn non_get_head_unmatched_requests_get_the_modern_404() {
        let root = temp_frontend_root();
        let config = fallback_test_config(&root);
        let (app, _state) = fallback_test_app(&config);

        for (method, path) in [
            (Method::DELETE, "/order/T1"),
            (Method::POST, "/dashboard"),
            (Method::PUT, "/boot-admin1/plans/7"),
        ] {
            let response = app
                .clone()
                .oneshot(request(method.clone(), path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "application/problem+json"
            );
            assert!(body_string(response).await.contains("endpoint_not_found"));
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn admin_dispatch_and_html_root_follow_the_live_prefix_without_restart() {
        let root = temp_frontend_root();
        let config = fallback_test_config(&root);
        let (app, state) = fallback_test_app(&config);

        // Boot prefix: the fallback re-dispatch reaches the modern admin
        // router (401 unauthenticated — the W2 session_expired flip — proves
        // it got past routing into the admin guard).
        let response = app
            .clone()
            .oneshot(request(Method::GET, "/api/v1/boot-admin1/config"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Simulate a runtime `secure_path` save: no rebuild, no restart.
        let mut live = config.clone();
        live.secure_path = Some("live-admin22".to_string());
        state.replace_config_for_test(live);

        // The new prefix is served through the fallback re-dispatch — for
        // every method (§6 preamble): PATCH must keep working across a
        // `secure_path` change without a process restart.
        for method in [Method::GET, Method::PATCH] {
            let response = app
                .clone()
                .oneshot(request(method.clone(), "/api/v1/live-admin22/config"))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{method}");
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "application/problem+json",
                "{method}"
            );
        }

        // The stale boot prefix is declined by the live-prefix check.
        let response = app
            .clone()
            .oneshot(request(Method::GET, "/api/v1/boot-admin1/config"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Paths without a modern admin route get the problem+json 404 from
        // the admin router's fallback (§10.2 rule 1) — since W14 there is no
        // legacy string dispatch left under the prefix.
        let response = app
            .clone()
            .oneshot(request(Method::DELETE, "/api/v1/live-admin22/plan/1"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(body_string(response).await.contains("endpoint_not_found"));

        // The HTML admin root follows the live prefix too; the stale root
        // becomes an ordinary user-SPA path.
        let response = app
            .clone()
            .oneshot(request(Method::GET, "/live-admin22"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("admin-shell"));
        let response = app
            .clone()
            .oneshot(request(Method::GET, "/boot-admin1"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("user-shell"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn safe_mode_gates_only_the_user_spa_fallback() {
        let root = temp_frontend_root();
        let mut config = fallback_test_config(&root);
        config.safe_mode_enable = true;
        config.app_url = Some("https://app.example.test".to_string());
        let (app, _state) = fallback_test_app(&config);

        let foreign = with_loopback_peer(
            Request::builder()
                .method(Method::GET)
                .uri("/dashboard")
                .header(header::HOST, "evil.example.test")
                .body(Body::empty())
                .unwrap(),
        );
        let response = app.clone().oneshot(foreign).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(body_string(response).await.is_empty());

        let canonical = with_loopback_peer(
            Request::builder()
                .method(Method::GET)
                .uri("/dashboard")
                .header(header::HOST, "app.example.test")
                .body(Body::empty())
                .unwrap(),
        );
        let response = app.clone().oneshot(canonical).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("user-shell"));

        let admin = with_loopback_peer(
            Request::builder()
                .method(Method::GET)
                .uri("/boot-admin1")
                .header(header::HOST, "evil.example.test")
                .body(Body::empty())
                .unwrap(),
        );
        let response = app.clone().oneshot(admin).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("admin-shell"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn subscribe_alias_is_reserved_and_never_html() {
        let root = temp_frontend_root();
        let mut config = fallback_test_config(&root);
        config.subscribe_path = "/sub-alias/xyz".to_string();
        let (app, _state) = fallback_test_app(&config);

        // GET on the alias enters the subscribe handler (deterministic
        // token-is-null rejection), not the SPA fallback.
        let response = app
            .clone()
            .oneshot(request(Method::GET, "/sub-alias/xyz"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(body_string(response).await.contains("token is null"));

        // Other methods on the alias are unmatched, reserved, and modern-404.
        let response = app
            .clone()
            .oneshot(request(Method::POST, "/sub-alias/xyz"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
