use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration as StdDuration,
};

use arc_swap::ArcSwap;
use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use v2board_config::AppConfig;
use v2board_db::{DbPool, migrations_current};
use v2board_domain::{
    admin::AdminService,
    auth::{AuthService, PasswordKdf},
    smtp::SmtpTransportCache,
};

#[derive(Clone)]
pub(crate) struct AppState {
    config: Arc<ArcSwap<AppConfig>>,
    config_reload: Arc<tokio::sync::Mutex<()>>,
    pub(crate) db: DbPool,
    pub(crate) installation_id: Uuid,
    pub(crate) redis: redis::Client,
    pub(crate) auth_redis: redis::aio::ConnectionManager,
    pub(crate) http: reqwest::Client,
    password_kdf: PasswordKdf,
    smtp: SmtpTransportCache,
}

impl AppState {
    // Keep the process-owned dependencies explicit at the one composition root;
    // collapsing unrelated pools/clients into an opaque tuple would make startup
    // wiring and principal separation harder to audit.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        config: AppConfig,
        db: DbPool,
        installation_id: Uuid,
        redis: redis::Client,
        auth_redis: redis::aio::ConnectionManager,
        http: reqwest::Client,
        password_kdf: PasswordKdf,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
            config_reload: Arc::new(tokio::sync::Mutex::new(())),
            db,
            installation_id,
            redis,
            auth_redis,
            http,
            password_kdf,
            smtp,
        }
    }

    pub(crate) fn auth_service(&self) -> AuthService {
        AuthService::new(
            self.db.clone(),
            self.auth_redis.clone(),
            self.config_snapshot(),
            self.http.clone(),
            self.password_kdf.clone(),
            self.smtp.clone(),
        )
    }

    pub(crate) fn admin_service(&self, config: Arc<AppConfig>) -> AdminService {
        AdminService::new(
            self.db.clone(),
            self.redis.clone(),
            config,
            self.http.clone(),
            self.password_kdf.clone(),
            self.smtp.clone(),
        )
    }

    pub(crate) fn config_snapshot(&self) -> Arc<AppConfig> {
        self.config.load_full()
    }

    pub(crate) async fn reload_config(&self) -> io::Result<Arc<AppConfig>> {
        // Serialize reloads so a slower poll cannot overwrite the synchronous
        // post-save reload with an older snapshot it started reading earlier.
        let _guard = self.config_reload.lock().await;
        let current = self.config_snapshot();
        let config = Arc::new(
            tokio::task::spawn_blocking(move || current.reload())
                .await
                .map_err(|error| io::Error::other(error.to_string()))??,
        );
        self.config.store(config.clone());
        Ok(config)
    }

    pub(crate) fn spawn_config_reloader(&self) -> tokio::task::JoinHandle<()> {
        let state = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(StdDuration::from_secs(2));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut last_error = None::<String>;
            loop {
                interval.tick().await;
                match state.reload_config().await {
                    Ok(_) => {
                        if last_error.take().is_some() {
                            tracing::info!("runtime configuration reload recovered");
                        }
                    }
                    Err(error) => {
                        let message = error.to_string();
                        if last_error.as_deref() != Some(message.as_str()) {
                            tracing::warn!(?error, "keeping last-known-good runtime configuration");
                        }
                        last_error = Some(message);
                    }
                }
            }
        })
    }
}

pub(crate) fn build_http_client(config: &AppConfig) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(config.http_connect_timeout_seconds))
        .timeout(StdDuration::from_secs(config.http_request_timeout_seconds))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(5))
        .tcp_keepalive(StdDuration::from_secs(60))
        .user_agent(format!("v2board-rust/{}", env!("CARGO_PKG_VERSION")))
        .build()?)
}

pub(crate) async fn reset_admin_password(
    db: &DbPool,
    config: &AppConfig,
    password_kdf: &PasswordKdf,
    email: &str,
    password: Option<String>,
) -> anyhow::Result<()> {
    let email = email.trim();
    if email.is_empty() {
        anyhow::bail!(
            "usage: provide the v2board-new-password systemd credential (or V2BOARD_NEW_PASSWORD_FILE) and run v2board-api reset-admin-password <email>"
        );
    }
    let password = password
        .filter(|password| password.chars().count() >= 8)
        .ok_or_else(|| {
            anyhow::anyhow!("the one-shot administrator password must contain at least 8 characters")
        })?;
    let password_hash = password_kdf
        .hash(&password)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut tx = db.begin().await?;
    let user_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM v2_user \
         WHERE lower(btrim(email)) = lower(btrim($1)) AND is_admin = 1 LIMIT 1 FOR UPDATE",
    )
    .bind(email)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow::anyhow!("admin account not found: {email}"))?;
    let result = sqlx::query(
        r#"
        UPDATE v2_user
        SET password = $1, password_algo = NULL, password_salt = NULL,
            session_epoch = session_epoch + 1, updated_at = $2
        WHERE id = $3 AND is_admin = 1
        "#,
    )
    .bind(password_hash)
    .bind(Utc::now().timestamp())
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() != 1 {
        anyhow::bail!("admin account not found: {email}");
    }
    tx.commit().await?;

    match redis::Client::open(config.redis_url.clone()) {
        Ok(redis) => {
            if let Err(error) =
                v2board_domain::auth::remove_user_sessions_from_client(&redis, user_id).await
            {
                tracing::warn!(
                    user_id,
                    ?error,
                    "administrator password changed but cached sessions could not be removed"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                user_id,
                ?error,
                "administrator password changed but Redis could not be opened for session cleanup"
            );
        }
    }
    println!("administrator password updated: {email}");
    Ok(())
}

pub(crate) fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("v2board_api=info,tower_http=info"));
    let production =
        v2board_config::RuntimeEnvironment::parse(std::env::var("V2BOARD_ENV").ok().as_deref())
            .is_ok_and(v2board_config::RuntimeEnvironment::is_production);
    if production {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

pub(crate) async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

pub(crate) async fn readyz(State(state): State<AppState>) -> Response {
    let deadline = StdDuration::from_secs(3);
    let db = async {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&state.db)
            .await?;
        migrations_current(&state.db).await
    };
    let db = tokio::time::timeout(deadline, db);
    let redis = async {
        let mut connection = state.auth_redis.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
    };
    let redis = tokio::time::timeout(deadline, redis);
    let config = state.config_snapshot();
    let user_index = tokio::fs::metadata(
        config
            .runtime_paths
            .frontend
            .join("current/user/index.html"),
    );
    let admin_index = tokio::fs::metadata(
        config
            .runtime_paths
            .frontend
            .join("current/admin/index.html"),
    );
    let (db, redis, user_index, admin_index) = tokio::join!(db, redis, user_index, admin_index);
    let db = db.is_ok_and(|result| result.is_ok_and(|current| current));
    let redis = redis.is_ok_and(|result| result.as_deref() == Ok("PONG"));
    let frontend = user_index.is_ok_and(|metadata| metadata.is_file())
        && admin_index.is_ok_and(|metadata| metadata.is_file());
    let ok = db && redis && frontend;
    let body = Json(json!({
        "ok": ok,
        "checks": { "database": db, "redis": redis, "frontend": frontend }
    }));
    if ok {
        body.into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, body).into_response()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClientIp(pub(crate) IpAddr);

pub(crate) async fn trusted_client_ip_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    let config = state.config_snapshot();
    let client_ip = resolve_client_ip(peer.ip(), request.headers(), &config.trusted_proxy_cidrs);
    request.extensions_mut().insert(ClientIp(client_ip));
    next.run(request).await
}

/// Enforces HTTPS without trusting forwarding headers from arbitrary clients.
/// Only a directly connected peer inside `trusted_proxy_cidrs` may describe the
/// original transport. Redirects use the configured canonical app URL rather
/// than the untrusted Host header.
pub(crate) async fn enforce_https_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if https_enforcement_exempt_path(request.uri().path()) {
        return next.run(request).await;
    }
    let config = state.config_snapshot();
    let https = request_uses_https(peer.ip(), request.headers(), &config.trusted_proxy_cidrs);
    if config.force_https && !https {
        let Some(location) = https_redirect_location(&config, request.uri()) else {
            tracing::error!("force_https app_url could not be converted to a redirect URL");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
        response.headers_mut().insert(header::LOCATION, location);
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        );
        return response;
    }

    let mut response = next.run(request).await;
    if https {
        response.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

fn https_enforcement_exempt_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz")
}

fn request_uses_https(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[ipnet::IpNet]) -> bool {
    if !trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
    {
        return false;
    }
    if headers.contains_key("forwarded") {
        return forwarded_proto(headers).is_some_and(|proto| proto.eq_ignore_ascii_case("https"));
    }
    x_forwarded_proto(headers).is_some_and(|proto| proto.eq_ignore_ascii_case("https"))
}

fn forwarded_proto(headers: &HeaderMap) -> Option<&str> {
    let mut nearest = None;
    for value in headers.get_all("forwarded") {
        let value = value.to_str().ok()?;
        for element in value.split(',') {
            let proto = element.split(';').find_map(|parameter| {
                let (name, value) = parameter.trim().split_once('=')?;
                name.trim()
                    .eq_ignore_ascii_case("proto")
                    .then_some(value.trim().trim_matches('"'))
            })?;
            if !matches!(proto.to_ascii_lowercase().as_str(), "http" | "https") {
                return None;
            }
            nearest = Some(proto);
        }
    }
    nearest
}

fn x_forwarded_proto(headers: &HeaderMap) -> Option<&str> {
    let mut nearest = None;
    for value in headers.get_all("x-forwarded-proto") {
        let value = value.to_str().ok()?;
        for proto in value.split(',').map(str::trim) {
            if !matches!(proto.to_ascii_lowercase().as_str(), "http" | "https") {
                return None;
            }
            nearest = Some(proto);
        }
    }
    nearest
}

fn https_redirect_location(config: &AppConfig, uri: &axum::http::Uri) -> Option<HeaderValue> {
    let mut target = reqwest::Url::parse(config.app_url.as_deref()?).ok()?;
    if target.scheme() != "https" || target.host_str().is_none() {
        return None;
    }
    target.set_path(uri.path());
    target.set_query(uri.query());
    target.set_fragment(None);
    HeaderValue::from_str(target.as_str()).ok()
}

pub(crate) async fn request_timeout_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let config = state.config_snapshot();
    if request_timeout_exempt(request.uri().path(), &config.admin_path()) {
        return next.run(request).await;
    }
    let timeout = StdDuration::from_secs(config.api_request_timeout_seconds);
    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(json!({ "message": "Request timeout" })),
        )
            .into_response(),
    }
}

fn request_timeout_exempt(path: &str, admin_path: &str) -> bool {
    path == format!("/api/v1/{admin_path}/config/testSendMail")
}

fn resolve_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> IpAddr {
    if !trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
    {
        return peer;
    }
    forwarded_chain(headers)
        .or_else(|| x_forwarded_for_chain(headers))
        .map(|chain| nearest_untrusted_hop(peer, &chain, trusted_proxies))
        .unwrap_or(peer)
}

fn nearest_untrusted_hop(
    peer: IpAddr,
    chain: &[IpAddr],
    trusted_proxies: &[ipnet::IpNet],
) -> IpAddr {
    let mut current = peer;
    for hop in chain.iter().rev() {
        if !trusted_proxies
            .iter()
            .any(|network| network.contains(&current))
        {
            break;
        }
        current = *hop;
    }
    current
}

fn forwarded_chain(headers: &HeaderMap) -> Option<Vec<IpAddr>> {
    let mut chain = Vec::new();
    for value in headers.get_all("forwarded") {
        let value = value.to_str().ok()?;
        for element in value.split(',') {
            let node = element.split(';').find_map(|parameter| {
                let (name, value) = parameter.trim().split_once('=')?;
                name.trim()
                    .eq_ignore_ascii_case("for")
                    .then_some(value.trim())
            })?;
            chain.push(parse_forwarded_node(node)?);
        }
    }
    (!chain.is_empty()).then_some(chain)
}

fn x_forwarded_for_chain(headers: &HeaderMap) -> Option<Vec<IpAddr>> {
    let mut chain = Vec::new();
    for value in headers.get_all("x-forwarded-for") {
        let value = value.to_str().ok()?;
        for node in value.split(',') {
            chain.push(parse_forwarded_node(node.trim())?);
        }
    }
    (!chain.is_empty()).then_some(chain)
}

fn parse_forwarded_node(value: &str) -> Option<IpAddr> {
    let value = value.trim().trim_matches('"');
    if value.eq_ignore_ascii_case("unknown") || value.starts_with('_') {
        return None;
    }
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|address| address.ip()))
        .or_else(|| value.strip_prefix('[')?.strip_suffix(']')?.parse().ok())
}

pub(crate) async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(?error, "failed to install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => tracing::error!(?error, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received; draining HTTP connections");
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use axum::http::HeaderMap;

    use super::{
        https_enforcement_exempt_path, request_timeout_exempt, request_uses_https,
        resolve_client_ip,
    };

    fn proxy_networks(values: &[&str]) -> Vec<ipnet::IpNet> {
        values.iter().map(|value| value.parse().unwrap()).collect()
    }

    #[test]
    fn untrusted_peers_cannot_spoof_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.7".parse().unwrap());
        let peer = "203.0.113.10".parse().unwrap();
        assert_eq!(
            resolve_client_ip(peer, &headers, &proxy_networks(&["10.0.0.0/8"])),
            peer
        );
    }

    #[test]
    fn trusted_proxy_walk_stops_at_the_nearest_untrusted_hop() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "192.0.2.99, 198.51.100.7, 10.0.0.2".parse().unwrap(),
        );
        let client = resolve_client_ip(
            "10.0.0.1".parse().unwrap(),
            &headers,
            &proxy_networks(&["10.0.0.0/8"]),
        );
        assert_eq!(client, "198.51.100.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn forwarded_header_supports_quoted_ipv6_with_a_port() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "forwarded",
            "for=\"[2001:db8::42]:443\";proto=https, for=10.0.0.2"
                .parse()
                .unwrap(),
        );
        let client = resolve_client_ip(
            "10.0.0.1".parse().unwrap(),
            &headers,
            &proxy_networks(&["10.0.0.0/8"]),
        );
        assert_eq!(client, "2001:db8::42".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn malformed_forwarding_chain_falls_back_to_the_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.7, unknown".parse().unwrap());
        let peer = "10.0.0.1".parse().unwrap();
        assert_eq!(
            resolve_client_ip(peer, &headers, &proxy_networks(&["10.0.0.0/8"])),
            peer
        );
    }

    #[test]
    fn forwarded_transport_is_honored_only_from_a_trusted_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert!(!request_uses_https(
            "203.0.113.10".parse().unwrap(),
            &headers,
            &proxy_networks(&["10.0.0.0/8"]),
        ));
        assert!(request_uses_https(
            "10.0.0.1".parse().unwrap(),
            &headers,
            &proxy_networks(&["10.0.0.0/8"]),
        ));

        headers.insert(
            "forwarded",
            "for=198.51.100.7;proto=http, for=10.0.0.2;proto=https"
                .parse()
                .unwrap(),
        );
        assert!(request_uses_https(
            "10.0.0.1".parse().unwrap(),
            &headers,
            &proxy_networks(&["10.0.0.0/8"]),
        ));
        headers.insert("forwarded", "for=10.0.0.2;proto=ftp".parse().unwrap());
        assert!(!request_uses_https(
            "10.0.0.1".parse().unwrap(),
            &headers,
            &proxy_networks(&["10.0.0.0/8"]),
        ));
    }

    #[test]
    fn only_the_synchronous_mail_probe_is_exempt_from_request_deadlines() {
        assert!(!request_timeout_exempt(
            "/api/v1/admin/user/sendMail",
            "admin"
        ));
        assert!(request_timeout_exempt(
            "/api/v1/admin/config/testSendMail",
            "admin"
        ));
        assert!(!request_timeout_exempt(
            "/api/v1/staff/user/sendMail",
            "admin"
        ));
        assert!(!request_timeout_exempt(
            "/api/v1/not-admin/user/sendMail",
            "admin"
        ));
        assert!(!request_timeout_exempt("/api/v1/user/order/save", "admin"));
        assert!(!request_timeout_exempt("/api/v1/user/info", "admin"));
    }

    #[test]
    fn internal_health_probes_are_the_only_https_enforcement_exceptions() {
        assert!(https_enforcement_exempt_path("/healthz"));
        assert!(https_enforcement_exempt_path("/readyz"));
        assert!(!https_enforcement_exempt_path("/"));
        assert!(!https_enforcement_exempt_path("/api/v1/user/info"));
    }
}
