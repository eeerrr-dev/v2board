use std::{
    io,
    sync::Arc,
    sync::atomic::{AtomicBool, AtomicI64, Ordering},
    time::Duration as StdDuration,
};

use arc_swap::ArcSwap;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
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

mod ingress;

pub(crate) use self::ingress::{
    ClientIp, enforce_https_middleware, host_matches_app_url, request_timeout_middleware,
    trusted_client_ip_middleware,
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
    pub(crate) http_metrics: Arc<crate::metrics::HttpMetrics>,
    pub(crate) http_rate_limiter: Arc<crate::rate_limit::HttpRateLimiter>,
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
            http_metrics: Arc::new(crate::metrics::HttpMetrics::default()),
            http_rate_limiter: Arc::new(crate::rate_limit::HttpRateLimiter::from_env()),
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

    pub(crate) fn operator_config_acknowledged(&self) -> bool {
        self.pending_operator_ack.load(Ordering::Acquire) == 0
    }

    pub(crate) fn operator_config_authority_healthy(&self) -> bool {
        self.operator_authority_healthy.load(Ordering::Acquire)
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

/// `v2board-api reset-admin-totp <email>` — the operator escape hatch for a
/// privileged account locked out of its TOTP factor. Removes the factor
/// (pending or enabled); the account's password remains untouched.
pub(crate) async fn reset_admin_totp(db: &DbPool, email: &str) -> anyhow::Result<()> {
    let email = email.trim();
    if email.is_empty() {
        anyhow::bail!("usage: v2board-api reset-admin-totp <email>");
    }
    let user_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM users \
         WHERE lower(btrim(email)) = lower(btrim($1)) AND (is_admin = 1 OR is_staff = 1) LIMIT 1",
    )
    .bind(email)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("privileged account not found: {email}"))?;
    if v2board_db::admin_mfa::reset(db, user_id).await? == 0 {
        println!("no TOTP factor was configured for {email}; nothing to remove");
    } else {
        println!("TOTP factor removed for {email}");
    }
    Ok(())
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

/// Process-lifetime telemetry state. Holding it keeps the Sentry client
/// alive; dropping it at the end of `main` flushes queued Sentry events and
/// shuts down the OTLP tracer provider so buffered spans export.
pub(crate) struct TelemetryGuard {
    _sentry: Option<sentry::ClientInitGuard>,
    otel: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.otel.take()
            && let Err(error) = provider.shutdown()
        {
            eprintln!("OTLP tracer provider shutdown did not flush cleanly: {error}");
        }
    }
}

static OTEL_ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether OTLP span export was initialized for this process. Gates the
/// per-request W3C trace-context extraction so the disabled default costs
/// nothing.
pub(crate) fn otel_enabled() -> bool {
    OTEL_ENABLED.load(Ordering::Relaxed)
}

/// Adapts request headers to the OTel propagation API for `traceparent`/
/// `tracestate` extraction.
pub(crate) struct HeaderCarrier<'a>(pub(crate) &'a axum::http::HeaderMap);

impl opentelemetry::propagation::Extractor for HeaderCarrier<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|key| key.as_str()).collect()
    }
}

/// Initializes tracing plus optional Sentry error reporting and optional OTLP
/// span export. Must run before the tokio runtime starts: the OTLP batch
/// exporter constructs a blocking HTTP client, which panics inside one. The
/// caller holds the returned guard for the process lifetime. Both exports are
/// entirely off unless their env variable is set (`V2BOARD_SENTRY_DSN`,
/// `V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT`); invalid values warn and stay off
/// rather than failing the service.
pub(crate) fn init_tracing() -> TelemetryGuard {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("v2board_api=info,tower_http=info"));
    let production =
        v2board_config::RuntimeEnvironment::parse(std::env::var("V2BOARD_ENV").ok().as_deref())
            .is_ok_and(v2board_config::RuntimeEnvironment::is_production);
    let dsn = parse_sentry_dsn(std::env::var("V2BOARD_SENTRY_DSN").ok());
    let sentry_guard = dsn.as_ref().ok().and_then(Option::as_ref).map(|dsn| {
        sentry::init(sentry::ClientOptions {
            dsn: Some(dsn.clone()),
            release: sentry::release_name!(),
            environment: Some(if production { "production" } else { "local" }.into()),
            attach_stacktrace: true,
            ..Default::default()
        })
    });
    let otel = init_otel("v2board-api");
    let otel_provider = otel.as_ref().ok().and_then(Option::as_ref);
    // ERROR events become Sentry events and WARN/INFO become breadcrumbs
    // (the sentry-tracing default); without a client the layer is absent.
    let sentry_enabled = sentry_guard.is_some();
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
            .with(sentry_enabled.then(sentry::integrations::tracing::layer))
            .with(otel_provider.map(|provider| {
                use opentelemetry::trace::TracerProvider as _;
                tracing_opentelemetry::layer().with_tracer(provider.tracer("v2board-api"))
            }))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(sentry_enabled.then(sentry::integrations::tracing::layer))
            .with(otel_provider.map(|provider| {
                use opentelemetry::trace::TracerProvider as _;
                tracing_opentelemetry::layer().with_tracer(provider.tracer("v2board-api"))
            }))
            .init();
    }
    if let Err(error) = &dsn {
        tracing::warn!(%error, "V2BOARD_SENTRY_DSN is invalid; error reporting is disabled");
    }
    let otel = match otel {
        Ok(provider) => provider,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT is invalid; span export is disabled"
            );
            None
        }
    };
    if otel.is_some() {
        OTEL_ENABLED.store(true, Ordering::Relaxed);
    }
    TelemetryGuard {
        _sentry: sentry_guard,
        otel,
    }
}

/// `Ok(None)` when the endpoint variable is absent or blank (export off).
/// When set, the W3C trace-context propagator becomes the process global so
/// incoming `traceparent` headers join the exported trace.
fn init_otel(
    service_name: &'static str,
) -> Result<Option<opentelemetry_sdk::trace::SdkTracerProvider>, String> {
    let Some(endpoint) = std::env::var("V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let traces_endpoint = normalize_otlp_traces_endpoint(&endpoint)?;
    let exporter = {
        use opentelemetry_otlp::WithExportConfig;
        opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
            .with_endpoint(traces_endpoint)
            .build()
            .map_err(|error| error.to_string())?
    };
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name)
                .build(),
        )
        .build();
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    Ok(Some(provider))
}

/// Standard OTLP ergonomics: the operator supplies the base endpoint
/// (e.g. `http://127.0.0.1:4318`) and the traces signal path is appended,
/// unless a full signal URL was already given.
fn normalize_otlp_traces_endpoint(endpoint: &str) -> Result<String, String> {
    let url = url::Url::parse(endpoint)
        .map_err(|error| format!("endpoint is not a valid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("endpoint must be an http(s) URL".to_owned());
    }
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/v1/traces") {
        Ok(trimmed.to_owned())
    } else {
        Ok(format!("{trimmed}/v1/traces"))
    }
}

/// `Ok(None)` when the variable is absent or blank (reporting off); `Err`
/// preserves the parse failure so it can be logged once tracing is up.
fn parse_sentry_dsn(
    raw: Option<String>,
) -> Result<Option<sentry::types::Dsn>, sentry::types::ParseDsnError> {
    match raw.as_deref().map(str::trim).filter(|raw| !raw.is_empty()) {
        Some(raw) => raw.parse().map(Some),
        None => Ok(None),
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
    use super::{
        ActivationSource, activation_source, finish_applied_operator_activation,
        normalize_otlp_traces_endpoint, parse_sentry_dsn,
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
    fn otlp_endpoints_normalize_to_the_traces_signal_url() {
        assert_eq!(
            normalize_otlp_traces_endpoint("http://127.0.0.1:4318"),
            Ok("http://127.0.0.1:4318/v1/traces".to_owned())
        );
        assert_eq!(
            normalize_otlp_traces_endpoint("https://otel.example.com/"),
            Ok("https://otel.example.com/v1/traces".to_owned())
        );
        assert_eq!(
            normalize_otlp_traces_endpoint("http://127.0.0.1:4318/v1/traces"),
            Ok("http://127.0.0.1:4318/v1/traces".to_owned())
        );
        assert!(normalize_otlp_traces_endpoint("not a url").is_err());
        assert!(normalize_otlp_traces_endpoint("grpc://127.0.0.1:4317").is_err());
    }

    #[test]
    fn sentry_reporting_is_off_without_a_dsn_and_on_parse_failure() {
        assert!(matches!(parse_sentry_dsn(None), Ok(None)));
        assert!(matches!(parse_sentry_dsn(Some(String::new())), Ok(None)));
        assert!(matches!(
            parse_sentry_dsn(Some("   ".to_string())),
            Ok(None)
        ));
        assert!(parse_sentry_dsn(Some("not a dsn".to_string())).is_err());
        let parsed = parse_sentry_dsn(Some(
            "https://f00d@o111111.ingest.sentry.io/2222".to_string(),
        ));
        assert!(matches!(parsed, Ok(Some(_))));
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
}
