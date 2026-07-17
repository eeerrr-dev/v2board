use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    sync::atomic::{AtomicBool, AtomicI64, Ordering},
    time::Duration as StdDuration,
};

use arc_swap::ArcSwap;
use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, uri::Authority},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use v2board_analytics::{AnalyticsPressureState, analytics_admission_snapshot};
use v2board_config::{AppConfig, RedisKeyspace};
use v2board_db::{DbPool, migrations_current};
use v2board_domain::{
    admin::AdminService,
    auth::{AuthService, PasswordKdf},
    operator_config::{self, OperatorConfigConsumer},
    smtp::SmtpTransportCache,
};

#[derive(Clone)]
pub(crate) struct AppState {
    config: Arc<ArcSwap<AppConfig>>,
    config_reload: Arc<tokio::sync::Mutex<()>>,
    pending_operator_ack: Arc<AtomicI64>,
    operator_authority_healthy: Arc<AtomicBool>,
    pub(crate) db: DbPool,
    pub(crate) installation_id: Uuid,
    pub(crate) redis_keys: RedisKeyspace,
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
            pending_operator_ack: Arc::new(AtomicI64::new(0)),
            operator_authority_healthy: Arc::new(AtomicBool::new(true)),
            db,
            installation_id,
            redis_keys: RedisKeyspace::new(installation_id),
            redis,
            auth_redis,
            http,
            password_kdf,
            smtp,
        }
    }

    /// Builds an [`AppState`] whose PostgreSQL pool and Redis connections are
    /// lazy handles to a closed loopback port: tests can drive the real router
    /// and prove which requests are answered (or fail) before any backing
    /// service is reachable.
    #[cfg(test)]
    pub(crate) fn service_free_test(config: AppConfig) -> Self {
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:9/unused")
            .expect("lazy postgres pool");
        let redis = redis::Client::open("redis://127.0.0.1:9/").expect("redis client");
        let auth_redis = redis::aio::ConnectionManager::new_lazy_with_config(
            redis.clone(),
            // Fail fast: a refused connection must surface as an immediate
            // command error instead of a reconnect backoff.
            redis::aio::ConnectionManagerConfig::new().set_number_of_retries(0),
        )
        .expect("lazy redis connection manager");
        Self::new(
            config,
            db,
            Uuid::new_v4(),
            redis,
            auth_redis,
            reqwest::Client::new(),
            PasswordKdf::new(1),
            SmtpTransportCache::default(),
        )
    }

    /// Swaps the live config snapshot without the operator-authority dance so
    /// router tests can prove the per-request dynamic-prefix behavior
    /// (docs/api-dialect.md §10.2 rule 4) across a `secure_path` change.
    #[cfg(test)]
    pub(crate) fn replace_config_for_test(&self, config: AppConfig) {
        self.config.store(Arc::new(config));
    }

    pub(crate) fn auth_service(&self) -> AuthService {
        AuthService::new(
            self.db.clone(),
            self.auth_redis.clone(),
            self.installation_id,
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
            self.installation_id,
            config,
            self.http.clone(),
            self.password_kdf.clone(),
            self.smtp.clone(),
        )
    }

    pub(crate) fn config_snapshot(&self) -> Arc<AppConfig> {
        self.config.load_full()
    }

    pub(crate) fn redis_key(&self, logical_key: &str) -> String {
        self.redis_keys.key(logical_key)
    }

    pub(crate) async fn reload_config(&self) -> io::Result<Arc<AppConfig>> {
        // Serialize reloads so a slower poll cannot overwrite the synchronous
        // post-save reload with an older snapshot it started reading earlier.
        let _guard = self.config_reload.lock().await;
        let current = self.config_snapshot();
        let snapshot =
            match operator_config::load_active(&self.db, self.installation_id, &current.app_key)
                .await
            {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => {
                    self.operator_authority_healthy
                        .store(false, Ordering::Release);
                    return Err(io::Error::other(
                        "operator configuration authority is missing",
                    ));
                }
                Err(error) => {
                    self.operator_authority_healthy
                        .store(false, Ordering::Release);
                    if let Some((observed_revision, error_code)) = error.observed_rejection()
                        && current
                            .operator_revision()
                            .is_none_or(|revision| revision < observed_revision)
                        && let Err(ack_error) = operator_config::acknowledge(
                            &self.db,
                            self.installation_id,
                            OperatorConfigConsumer::Api,
                            observed_revision,
                            current.operator_revision(),
                            Some(error_code),
                        )
                        .await
                    {
                        tracing::error!(
                            ?ack_error,
                            observed_revision,
                            "failed to acknowledge rejected API operator configuration"
                        );
                    }
                    return Err(io::Error::other(error.to_string()));
                }
            };
        if current.operator_revision() == Some(snapshot.revision) {
            if self.pending_operator_ack.load(Ordering::Acquire) == snapshot.revision {
                operator_config::acknowledge(
                    &self.db,
                    self.installation_id,
                    OperatorConfigConsumer::Api,
                    snapshot.revision,
                    Some(snapshot.revision),
                    None,
                )
                .await
                .map_err(|error| io::Error::other(error.to_string()))?;
                self.pending_operator_ack.store(0, Ordering::Release);
            }
            self.operator_authority_healthy
                .store(true, Ordering::Release);
            return Ok(current);
        }
        let observed_revision = snapshot.revision;
        let reload_base = current.clone();
        let values = snapshot.values;
        let config = match tokio::task::spawn_blocking(move || {
            reload_base.with_operator_config(&values, observed_revision)
        })
        .await
        .map_err(|error| io::Error::other(error.to_string()))?
        {
            Ok(config) => Arc::new(config),
            Err(error) => {
                self.operator_authority_healthy
                    .store(false, Ordering::Release);
                let _ = operator_config::acknowledge(
                    &self.db,
                    self.installation_id,
                    OperatorConfigConsumer::Api,
                    observed_revision,
                    current.operator_revision(),
                    Some("typed_validation_failed"),
                )
                .await;
                return Err(error);
            }
        };
        self.config.store(config.clone());
        self.operator_authority_healthy
            .store(true, Ordering::Release);
        if let Err(error) = operator_config::acknowledge(
            &self.db,
            self.installation_id,
            OperatorConfigConsumer::Api,
            observed_revision,
            Some(observed_revision),
            None,
        )
        .await
        {
            self.pending_operator_ack
                .store(observed_revision, Ordering::Release);
            return Err(io::Error::other(error.to_string()));
        }
        self.pending_operator_ack.store(0, Ordering::Release);
        Ok(config)
    }

    /// Applies the already-validated committed revision before acknowledging it.
    /// If the ACK connection fails after commit, the active ArcSwap snapshot is
    /// retained (it matches PostgreSQL), readiness is degraded, and the normal
    /// poller retries only that pending acknowledgement until it succeeds. The
    /// return value reports only whether the PostgreSQL-active revision entered
    /// ArcSwap; a failed acknowledgement does not turn an applied revision into
    /// an activation failure.
    pub(crate) async fn activate_operator_config(&self, config: AppConfig) -> bool {
        let _guard = self.config_reload.lock().await;
        let incoming_revision = config
            .operator_revision()
            .expect("committed operator config has a revision");
        let current = self.config_snapshot();
        let authority =
            match operator_config::load_active(&self.db, self.installation_id, &current.app_key)
                .await
            {
                Ok(Some(authority)) => authority,
                Ok(None) => {
                    self.operator_authority_healthy
                        .store(false, Ordering::Release);
                    tracing::error!("operator configuration authority disappeared after save");
                    return false;
                }
                Err(error) => {
                    self.operator_authority_healthy
                        .store(false, Ordering::Release);
                    if let Some((observed_revision, error_code)) = error.observed_rejection()
                        && current
                            .operator_revision()
                            .is_none_or(|revision| revision < observed_revision)
                    {
                        let _ = operator_config::acknowledge(
                            &self.db,
                            self.installation_id,
                            OperatorConfigConsumer::Api,
                            observed_revision,
                            current.operator_revision(),
                            Some(error_code),
                        )
                        .await;
                    }
                    tracing::error!(
                        ?error,
                        "failed to authenticate active operator configuration"
                    );
                    return false;
                }
            };
        let active_revision = authority.revision;
        let source = match activation_source(
            current.operator_revision(),
            incoming_revision,
            active_revision,
        ) {
            Ok(source) => source,
            Err(()) => {
                self.operator_authority_healthy
                    .store(false, Ordering::Release);
                tracing::error!(
                    memory_revision = current.operator_revision(),
                    active_revision,
                    "database operator revision moved backwards"
                );
                return false;
            }
        };
        let target = match source {
            ActivationSource::Current => current.clone(),
            ActivationSource::Incoming => Arc::new(config),
            ActivationSource::Authority => {
                let reload_base = current.clone();
                let values = authority.values;
                match tokio::task::spawn_blocking(move || {
                    reload_base.with_operator_config(&values, active_revision)
                })
                .await
                {
                    Ok(Ok(config)) => Arc::new(config),
                    Ok(Err(error)) => {
                        self.operator_authority_healthy
                            .store(false, Ordering::Release);
                        let _ = operator_config::acknowledge(
                            &self.db,
                            self.installation_id,
                            OperatorConfigConsumer::Api,
                            active_revision,
                            current.operator_revision(),
                            Some("typed_validation_failed"),
                        )
                        .await;
                        tracing::error!(
                            ?error,
                            active_revision,
                            "rejected superseding operator configuration"
                        );
                        return false;
                    }
                    Err(error) => {
                        self.operator_authority_healthy
                            .store(false, Ordering::Release);
                        tracing::error!(?error, "operator validation task failed");
                        return false;
                    }
                }
            }
        };
        if target.operator_revision() != current.operator_revision() {
            self.config.store(target);
        }
        self.operator_authority_healthy
            .store(true, Ordering::Release);
        if source == ActivationSource::Current
            && self.pending_operator_ack.load(Ordering::Acquire) != active_revision
        {
            return true;
        }
        let acknowledgement = operator_config::acknowledge(
            &self.db,
            self.installation_id,
            OperatorConfigConsumer::Api,
            active_revision,
            Some(active_revision),
            None,
        )
        .await;
        finish_applied_operator_activation(
            &self.pending_operator_ack,
            active_revision,
            acknowledgement,
        )
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

fn finish_applied_operator_activation(
    pending_operator_ack: &AtomicI64,
    active_revision: i64,
    acknowledgement: Result<(), operator_config::OperatorConfigError>,
) -> bool {
    match acknowledgement {
        Ok(()) => pending_operator_ack.store(0, Ordering::Release),
        Err(error) => {
            pending_operator_ack.store(active_revision, Ordering::Release);
            tracing::error!(
                ?error,
                active_revision,
                "operator config is active but its API acknowledgement is pending retry"
            );
        }
    }
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActivationSource {
    Current,
    Incoming,
    Authority,
}

fn activation_source(
    current: Option<i64>,
    incoming: i64,
    authority: i64,
) -> Result<ActivationSource, ()> {
    let current = current.unwrap_or(0);
    if authority < current {
        Err(())
    } else if authority == current {
        Ok(ActivationSource::Current)
    } else if authority == incoming {
        Ok(ActivationSource::Incoming)
    } else {
        Ok(ActivationSource::Authority)
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
            anyhow::anyhow!(
                "the one-shot administrator password must contain at least 8 characters"
            )
        })?;
    let password_hash = password_kdf
        .hash(&password)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut tx = db.begin().await?;
    let user_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM users \
         WHERE lower(btrim(email)) = lower(btrim($1)) AND is_admin = 1 LIMIT 1 FOR UPDATE",
    )
    .bind(email)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow::anyhow!("admin account not found: {email}"))?;
    let result = sqlx::query(
        r#"
        UPDATE users
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
            let redis_keys = RedisKeyspace::new(v2board_db::installation_id(db).await?);
            if let Err(error) =
                v2board_domain::auth::remove_user_sessions_from_client(&redis, &redis_keys, user_id)
                    .await
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
    let analytics_admission =
        tokio::time::timeout(deadline, analytics_admission_snapshot(&state.db));
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
    let (db, redis, analytics_admission, user_index, admin_index) =
        tokio::join!(db, redis, analytics_admission, user_index, admin_index);
    let db = db.is_ok_and(|result| result.is_ok_and(|current| current));
    let redis = redis.is_ok_and(|result| result.as_deref() == Ok("PONG"));
    let frontend = user_index.is_ok_and(|metadata| metadata.is_file())
        && admin_index.is_ok_and(|metadata| metadata.is_file());
    let operator_config_acknowledged = state.pending_operator_ack.load(Ordering::Acquire) == 0;
    let operator_config_authority_healthy =
        state.operator_authority_healthy.load(Ordering::Acquire);
    let analytics_admission = match analytics_admission {
        Ok(Ok(snapshot)) => json!({
            "observed": true,
            "policy_sha256": snapshot.policy_sha256,
            "installation_id": snapshot.installation_id.to_string(),
            "pressure_state": snapshot.pressure_state.as_str(),
            "traffic_writes_available": snapshot.sample_fresh
                && snapshot.pressure_state != AnalyticsPressureState::HardStop,
            "sample_fresh": snapshot.sample_fresh,
            "sample_age_seconds": snapshot.sample_age_seconds,
            "generation": snapshot.generation,
            "pending_rows": snapshot.pending_rows,
            "accounted_pending_rows": snapshot.accounted_pending_rows,
            "oldest_pending_age_seconds": snapshot.oldest_pending_age_seconds,
            "relation_heap_bytes": snapshot.relation_heap_bytes,
            "relation_index_bytes": snapshot.relation_index_bytes,
            "relation_toast_bytes": snapshot.relation_toast_bytes,
            "relation_total_bytes": snapshot.relation_total_bytes,
            "accounted_relation_bytes": snapshot.accounted_relation_bytes,
            "database_bytes": snapshot.database_bytes,
            "capacity_headroom_bytes": snapshot.capacity_headroom_bytes,
            "last_transition_reason": snapshot.last_transition_reason,
            "thresholds": {
                "pending_rows": {
                    "recovery": snapshot.recovery_pending_rows,
                    "soft": snapshot.soft_pending_rows,
                    "hard": snapshot.hard_pending_rows,
                },
                "relation_bytes": {
                    "recovery": snapshot.recovery_relation_bytes,
                    "soft": snapshot.soft_relation_bytes,
                    "hard": snapshot.hard_relation_bytes,
                },
                "oldest_age_seconds": {
                    "recovery": snapshot.recovery_oldest_age_seconds,
                    "soft": snapshot.soft_oldest_age_seconds,
                    "hard": snapshot.hard_oldest_age_seconds,
                },
                "database_capacity_bytes": snapshot.database_capacity_bytes,
                "minimum_headroom_bytes": {
                    "hard": snapshot.hard_min_headroom_bytes,
                    "soft": snapshot.soft_min_headroom_bytes,
                    "recovery": snapshot.recovery_min_headroom_bytes,
                },
                "event_reservation_bytes": snapshot.event_reservation_bytes,
                "soft_max_new_rows_per_second": snapshot.soft_max_new_rows_per_second,
                "sample_interval_seconds": snapshot.sample_interval_seconds,
                "stale_after_seconds": snapshot.stale_after_seconds,
            },
        }),
        Ok(Err(error)) => {
            tracing::warn!(?error, "analytics admission readiness observation failed");
            json!({ "observed": false, "error": "unavailable" })
        }
        Err(_) => json!({ "observed": false, "error": "timeout" }),
    };
    let ok = db
        && redis
        && frontend
        && operator_config_acknowledged
        && operator_config_authority_healthy;
    let body = Json(json!({
        "ok": ok,
        "checks": {
            "database": db,
            "redis": redis,
            "frontend": frontend,
            "operator_config_acknowledged": operator_config_acknowledged,
            "operator_config_authority_healthy": operator_config_authority_healthy,
            "analytics_admission": analytics_admission,
        }
    }));
    if ok {
        body.into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, body).into_response()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClientIp(pub(crate) IpAddr);

const CF_CONNECTING_IP: &str = "cf-connecting-ip";
const PRODUCTION_PROBE_HOST: &str = "127.0.0.1:8080";
const FORWARDING_METADATA_HEADERS: [&str; 10] = [
    CF_CONNECTING_IP,
    "cf-connecting-ipv6",
    "cf-pseudo-ipv4",
    "forwarded",
    "true-client-ip",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-port",
    "x-forwarded-proto",
    "x-real-ip",
];

pub(crate) async fn trusted_client_ip_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    let config = state.config_snapshot();
    let peer_ip = peer.ip();
    let client_ip = if https_enforcement_exempt_path(request.uri().path()) {
        match direct_probe_client_ip(peer_ip, request.headers(), &config.trusted_proxy_cidrs) {
            Ok(client_ip) => client_ip,
            Err(status) => return status.into_response(),
        }
    } else {
        match public_client_ip(peer_ip, request.headers(), &config.trusted_proxy_cidrs) {
            Ok(client_ip) => client_ip,
            Err(status) => return status.into_response(),
        }
    };
    strip_forwarding_metadata(request.headers_mut());
    request.extensions_mut().insert(ClientIp(client_ip));
    next.run(request).await
}

/// Enforces the fixed public Cloudflare HTTPS origin without consulting generic
/// forwarding headers. A request is externally canonical only when the local
/// cloudflared peer is trusted and its preserved Host matches the configured
/// HTTPS application authority. Redirects never reflect an untrusted Host.
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
    let https = request_uses_canonical_https(
        peer.ip(),
        request.headers(),
        &config.trusted_proxy_cidrs,
        config.app_url.as_deref(),
    );
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

fn direct_probe_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Result<IpAddr, StatusCode> {
    if trusted_proxies.is_empty() {
        return Ok(peer);
    }
    let direct_host =
        strict_single_header(headers, header::HOST.as_str()) == Some(PRODUCTION_PROBE_HOST);
    if peer.is_loopback() && direct_host && !headers.contains_key(CF_CONNECTING_IP) {
        Ok(peer)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

fn request_uses_canonical_https(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
    app_url: Option<&str>,
) -> bool {
    if !trusted_proxy_peer(peer, trusted_proxies) {
        return false;
    }
    let Some(app_url) = app_url else {
        return false;
    };
    if !reqwest::Url::parse(app_url).is_ok_and(|url| url.scheme() == "https") {
        return false;
    }
    strict_single_header(headers, header::HOST.as_str())
        .is_some_and(|request_host| host_matches_app_url(app_url, request_host))
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

fn public_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Result<IpAddr, StatusCode> {
    resolve_client_ip(peer, headers, trusted_proxies).ok_or(StatusCode::BAD_REQUEST)
}

fn resolve_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<IpAddr> {
    if trusted_proxies.is_empty() {
        return Some(peer);
    }
    if !trusted_proxy_peer(peer, trusted_proxies) {
        return None;
    }
    let value = strict_single_header(headers, CF_CONNECTING_IP)?;
    if value.is_empty() || value.trim() != value {
        return None;
    }
    value.parse().ok()
}

fn trusted_proxy_peer(peer: IpAddr, trusted_proxies: &[ipnet::IpNet]) -> bool {
    trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
}

fn strict_single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    value.to_str().ok()
}

fn strip_forwarding_metadata(headers: &mut HeaderMap) {
    for name in FORWARDING_METADATA_HEADERS {
        headers.remove(name);
    }
}

pub(crate) fn host_matches_app_url(app_url: &str, request_host: &str) -> bool {
    let Ok(expected) = reqwest::Url::parse(app_url) else {
        return false;
    };
    let Some(expected_host) = expected.host_str() else {
        return false;
    };
    let Ok(actual) = request_host.parse::<Authority>() else {
        return false;
    };
    if !actual.host().eq_ignore_ascii_case(expected_host) {
        return false;
    }

    match (expected.port(), expected.port_or_known_default()) {
        (Some(expected_port), _) => actual.port_u16() == Some(expected_port),
        (None, Some(default_port)) => actual.port_u16().is_none_or(|port| port == default_port),
        (None, None) => actual.port_u16().is_none(),
    }
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

    use axum::http::{HeaderMap, HeaderValue, header};

    use super::{
        ActivationSource, CF_CONNECTING_IP, FORWARDING_METADATA_HEADERS, PRODUCTION_PROBE_HOST,
        activation_source, direct_probe_client_ip, finish_applied_operator_activation,
        https_enforcement_exempt_path, public_client_ip, request_timeout_exempt,
        request_uses_canonical_https, resolve_client_ip, strip_forwarding_metadata,
    };
    use std::sync::atomic::{AtomicI64, Ordering};
    use v2board_domain::operator_config::OperatorConfigError;

    #[test]
    fn operator_activation_uses_the_database_active_revision_monotonically() {
        assert_eq!(
            activation_source(None, 1, 1),
            Ok(ActivationSource::Incoming)
        );
        assert_eq!(
            activation_source(Some(7), 7, 7),
            Ok(ActivationSource::Current)
        );
        assert_eq!(activation_source(Some(8), 7, 7), Err(()));
        assert_eq!(
            activation_source(Some(1), 2, 3),
            Ok(ActivationSource::Authority),
            "a delayed revision-2 response must apply and acknowledge DB-active revision 3"
        );
    }

    #[test]
    fn operator_activation_ack_failure_stays_applied_and_marks_readiness_pending() {
        let pending = AtomicI64::new(0);
        assert!(finish_applied_operator_activation(
            &pending,
            9,
            Err(OperatorConfigError::Invalid(
                "acknowledgement unavailable".to_string()
            )),
        ));
        assert_eq!(pending.load(Ordering::Acquire), 9);

        assert!(finish_applied_operator_activation(&pending, 9, Ok(())));
        assert_eq!(pending.load(Ordering::Acquire), 0);
    }

    fn proxy_networks(values: &[&str]) -> Vec<ipnet::IpNet> {
        values.iter().map(|value| value.parse().unwrap()).collect()
    }

    #[test]
    fn configured_proxy_mode_rejects_untrusted_peers_and_local_mode_uses_the_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(CF_CONNECTING_IP, "192.0.2.10".parse().unwrap());
        headers.insert("forwarded", "for=192.0.2.11;proto=https".parse().unwrap());
        headers.insert("x-forwarded-for", "198.51.100.7".parse().unwrap());
        let peer = "203.0.113.10".parse().unwrap();
        assert_eq!(
            public_client_ip(peer, &headers, &proxy_networks(&["10.0.0.0/8"])),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
        assert_eq!(public_client_ip(peer, &headers, &[]), Ok(peer));

        let alternate_loopback = "127.0.0.2".parse().unwrap();
        assert_eq!(
            public_client_ip(
                alternate_loopback,
                &headers,
                &proxy_networks(&["127.0.0.1/32"]),
            ),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn trusted_cloudflared_uses_one_strict_cf_connecting_ip_value() {
        let mut headers = HeaderMap::new();
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);

        headers.insert(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        assert_eq!(
            public_client_ip(peer, &headers, &trusted),
            Ok("198.51.100.7".parse::<IpAddr>().unwrap())
        );
        headers.insert(CF_CONNECTING_IP, "2001:db8::42".parse().unwrap());
        assert_eq!(
            public_client_ip(peer, &headers, &trusted),
            Ok("2001:db8::42".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn trusted_cloudflared_rejects_missing_malformed_or_ambiguous_client_ip() {
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);
        assert_eq!(
            public_client_ip(peer, &HeaderMap::new(), &trusted),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );

        for value in [
            "",
            " 198.51.100.7",
            "198.51.100.7 ",
            "198.51.100.7, 192.0.2.10",
            "198.51.100.7:443",
            "unknown",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                CF_CONNECTING_IP,
                HeaderValue::from_bytes(value.as_bytes()).unwrap(),
            );
            assert_eq!(
                public_client_ip(peer, &headers, &trusted),
                Err(axum::http::StatusCode::BAD_REQUEST),
                "value={value:?}"
            );
        }

        let mut duplicate = HeaderMap::new();
        duplicate.append(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        duplicate.append(CF_CONNECTING_IP, "192.0.2.10".parse().unwrap());
        assert_eq!(
            public_client_ip(peer, &duplicate, &trusted),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn generic_forwarding_headers_do_not_change_client_ip_or_canonical_https() {
        let mut canonical_headers = HeaderMap::new();
        canonical_headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        let mut headers = canonical_headers.clone();
        headers.insert("forwarded", "for=192.0.2.10;proto=https".parse().unwrap());
        headers.insert("x-forwarded-for", "192.0.2.11".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);

        assert_eq!(resolve_client_ip(peer, &headers, &trusted), None);
        assert_eq!(
            request_uses_canonical_https(
                peer,
                &headers,
                &trusted,
                Some("https://panel.example.com"),
            ),
            request_uses_canonical_https(
                peer,
                &canonical_headers,
                &trusted,
                Some("https://panel.example.com"),
            )
        );

        headers.insert(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        assert_eq!(
            resolve_client_ip(peer, &headers, &trusted),
            Some("198.51.100.7".parse().unwrap())
        );
    }

    #[test]
    fn canonical_https_requires_the_trusted_peer_and_exact_app_url_host() {
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "PANEL.example.com:443".parse().unwrap());
        assert!(request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));
        assert!(!request_uses_canonical_https(
            "127.0.0.2".parse().unwrap(),
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));
        assert!(!request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("http://panel.example.com"),
        ));

        headers.insert(header::HOST, "attacker.example".parse().unwrap());
        assert!(!request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));

        headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        headers.append(header::HOST, "panel.example.com".parse().unwrap());
        assert!(!request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));
    }

    #[test]
    fn production_probes_require_the_exact_direct_loopback_origin() {
        let loopback = "127.0.0.1".parse().unwrap();
        let remote = "192.0.2.10".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);
        let mut headers = HeaderMap::new();

        assert_eq!(direct_probe_client_ip(remote, &headers, &[]), Ok(remote));
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.insert(header::HOST, PRODUCTION_PROBE_HOST.parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Ok(loopback)
        );
        assert_eq!(
            direct_probe_client_ip(remote, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.insert(header::HOST, PRODUCTION_PROBE_HOST.parse().unwrap());
        headers.insert(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.remove(CF_CONNECTING_IP);
        headers.append(header::HOST, PRODUCTION_PROBE_HOST.parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );
    }

    #[test]
    fn parsed_forwarding_metadata_is_not_exposed_to_handlers() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        for name in FORWARDING_METADATA_HEADERS {
            headers.insert(name, "198.51.100.7".parse().unwrap());
        }
        strip_forwarding_metadata(&mut headers);
        assert_eq!(headers.get(header::HOST).unwrap(), "panel.example.com");
        for name in FORWARDING_METADATA_HEADERS {
            assert!(!headers.contains_key(name), "header={name}");
        }
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
        assert!(!request_timeout_exempt("/api/v1/user/orders", "admin"));
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
