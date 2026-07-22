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
use serde_json::json;
use uuid::Uuid;
use v2board_analytics::{AnalyticsPressureState, analytics_admission_snapshot};
use v2board_application::{
    account::AccountService,
    admin_order::{AdminOrderService, AssignOrderPolicy},
    admin_user::AdminUserService,
    audit::AuditService,
    content::ContentService,
    giftcard::GiftCardService,
    invite::InviteService,
    logs::LogService,
    operator_access::{OperatorMfaResetOutcome, OperatorPasswordResetOutcome},
    payment::PaymentService,
    plan::PlanService,
    promotion::PromotionService,
    reconciliation::ReconciliationService,
    server_management::ServerManagementService,
    server_runtime::ServerRuntimeService,
    service_usage::ServiceUsageService,
    statistics::StatisticsService,
    subscription::{SubscriptionAccessService, SubscriptionService},
    system_monitoring::SystemMonitoringService,
    telegram::{TelegramPolicy, TelegramService},
    ticket::{TicketPolicy, TicketService},
};
use v2board_auth_adapters::{
    PasswordKdf, RuntimeAuthService, RuntimeOperatorAccessService, runtime_auth_service,
};
use v2board_compat::ApiError;
use v2board_config::{AppConfig, RedisKeyspace};
use v2board_configuration_adapters::operator_config::{self, OperatorConfigConsumer};
use v2board_configuration_adapters::{RuntimeConfigurationService, runtime_configuration_service};
use v2board_db::{
    DbPool, account::PostgresAccountRepository, admin_order::PostgresAdminOrderRepository,
    admin_payment::PostgresPaymentRepository, admin_server::PostgresServerManagementRepository,
    admin_user::PostgresAdminUserRepository, audit::PostgresAuditRepository,
    content::PostgresContentRepository, coupon::PostgresPromotionRepository,
    giftcard::PostgresGiftCardRepository, invite::PostgresInviteRepository,
    logs::PostgresLogRepository, migrations_current, plan::PostgresPlanRepository,
    reconciliation::PostgresReconciliationRepository,
    server_runtime::PostgresServerRuntimeRepository, service_usage::PostgresServiceUsageRepository,
    statistics::PostgresStatisticsRepository, subscription::PostgresSubscriptionRepository,
    telegram::PostgresTelegramRepository, ticket::PostgresTicketRepository,
};
use v2board_domain_model::TicketCreationPolicy;
use v2board_mail_adapters::smtp::SmtpTransportCache;
use v2board_order_adapters::{
    RuntimeOrderService, TimestampOrderNumberGenerator, runtime_order_service,
};
use v2board_payment_adapters::{EncryptedPaymentSecurity, Sha256ReconciliationIdentityHasher};
use v2board_server_adapters::HmacNodeCredentials;

use crate::{
    admin_order_adapters::{RuntimeAdminOrderLifecycle, RuntimeOrderNumberGenerator},
    admin_user_adapters::RuntimeAdminUserExternal,
    server_management_adapters::{OpenSslServerCredentials, RedisAdminServerPresence},
    server_runtime_adapters::RedisServerRuntimeCache,
    service_usage_adapters::RedisServerPresence,
    statistics_adapters::{ConfiguredStatisticsCalendar, RedisWorkerMetrics},
    subscription_adapters::{ConfiguredResetCalendar, RedisSubscriptionAccess},
    telegram_adapters::RuntimeTelegramExternal,
    ticket_adapters::{RedisTicketAdmission, TicketEmailNotifications},
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

    pub(crate) fn auth_service(&self) -> RuntimeAuthService {
        runtime_auth_service(
            self.db.clone(),
            self.auth_redis.clone(),
            self.installation_id,
            self.config_snapshot(),
            self.http.clone(),
            self.password_kdf.clone(),
            self.smtp.clone(),
        )
    }

    pub(crate) fn configuration_service(&self) -> RuntimeConfigurationService {
        runtime_configuration_service(
            self.db.clone(),
            self.installation_id,
            self.config_snapshot(),
            self.http.clone(),
            self.smtp.clone(),
        )
    }

    pub(crate) fn audit_service(&self) -> AuditService<PostgresAuditRepository> {
        AuditService::new(PostgresAuditRepository::new(self.db.clone()))
    }

    pub(crate) fn content_service(&self) -> ContentService<PostgresContentRepository> {
        ContentService::new(PostgresContentRepository::new(self.db.clone()))
    }

    pub(crate) fn account_service(&self) -> AccountService<PostgresAccountRepository> {
        AccountService::new(PostgresAccountRepository::new(self.db.clone()))
    }

    pub(crate) fn invite_service(
        &self,
    ) -> InviteService<PostgresInviteRepository, TimestampOrderNumberGenerator> {
        InviteService::new(
            PostgresInviteRepository::new(self.db.clone()),
            TimestampOrderNumberGenerator,
        )
    }

    pub(crate) fn plan_service(&self) -> PlanService<PostgresPlanRepository> {
        PlanService::new(PostgresPlanRepository::new(self.db.clone()))
    }

    pub(crate) fn payment_service(
        &self,
    ) -> PaymentService<PostgresPaymentRepository, EncryptedPaymentSecurity> {
        let config = self.config_snapshot();
        PaymentService::new(
            PostgresPaymentRepository::new(self.db.clone()),
            EncryptedPaymentSecurity::new(config.app_key.clone()),
            config.app_url.clone(),
        )
    }

    pub(crate) fn order_service(&self) -> RuntimeOrderService {
        runtime_order_service(self.db.clone(), self.config_snapshot())
    }

    pub(crate) fn admin_order_service(
        &self,
    ) -> AdminOrderService<
        PostgresAdminOrderRepository,
        RuntimeAdminOrderLifecycle,
        RuntimeOrderNumberGenerator,
    > {
        let config = self.config_snapshot();
        AdminOrderService::new(
            PostgresAdminOrderRepository::new(self.db.clone()),
            RuntimeAdminOrderLifecycle::new(self.db.clone(), config.clone()),
            RuntimeOrderNumberGenerator,
            AssignOrderPolicy {
                default_commission_rate: config.invite_commission,
                commission_first_time_enable: config.commission_first_time_enable,
            },
        )
    }

    pub(crate) fn admin_user_service(
        &self,
    ) -> AdminUserService<PostgresAdminUserRepository, RuntimeAdminUserExternal> {
        AdminUserService::new(
            PostgresAdminUserRepository::new(self.db.clone()),
            RuntimeAdminUserExternal::new(
                self.redis.clone(),
                self.redis_keys.clone(),
                self.config_snapshot(),
                self.password_kdf.clone(),
            ),
        )
    }

    pub(crate) fn reconciliation_service(
        &self,
    ) -> ReconciliationService<PostgresReconciliationRepository, Sha256ReconciliationIdentityHasher>
    {
        ReconciliationService::new(
            PostgresReconciliationRepository::new(self.db.clone()),
            Sha256ReconciliationIdentityHasher,
        )
    }

    pub(crate) fn promotion_service(&self) -> PromotionService<PostgresPromotionRepository> {
        PromotionService::new(PostgresPromotionRepository::new(self.db.clone()))
    }

    pub(crate) fn server_management_service(
        &self,
    ) -> ServerManagementService<
        PostgresServerManagementRepository,
        RedisAdminServerPresence,
        OpenSslServerCredentials,
    > {
        let config = self.config_snapshot();
        let install_api_host = config
            .server_api_url
            .clone()
            .or_else(|| config.app_url.clone())
            .unwrap_or_default();
        ServerManagementService::new(
            PostgresServerManagementRepository::new(self.db.clone()),
            RedisAdminServerPresence::new(self.redis.clone(), self.redis_keys.clone()),
            OpenSslServerCredentials::new(
                config.server_token.clone().unwrap_or_default(),
                install_api_host,
            ),
        )
    }

    pub(crate) fn giftcard_service(&self) -> GiftCardService<PostgresGiftCardRepository> {
        GiftCardService::new(PostgresGiftCardRepository::new(self.db.clone()))
    }

    pub(crate) fn server_runtime_service(
        &self,
    ) -> ServerRuntimeService<
        PostgresServerRuntimeRepository,
        RedisServerRuntimeCache,
        HmacNodeCredentials,
    > {
        ServerRuntimeService::new(
            PostgresServerRuntimeRepository::new(self.db.clone()),
            RedisServerRuntimeCache::new(self.redis.clone(), self.redis_keys.clone()),
            HmacNodeCredentials::new(
                self.config_snapshot()
                    .server_token
                    .clone()
                    .unwrap_or_default(),
            ),
        )
    }

    pub(crate) fn service_usage_service(
        &self,
    ) -> ServiceUsageService<PostgresServiceUsageRepository, RedisServerPresence> {
        ServiceUsageService::new(
            PostgresServiceUsageRepository::new(self.db.clone()),
            RedisServerPresence::new(self.redis.clone(), self.redis_keys.clone()),
        )
    }

    pub(crate) fn subscription_service(
        &self,
    ) -> SubscriptionService<PostgresSubscriptionRepository, ConfiguredResetCalendar> {
        SubscriptionService::new(
            PostgresSubscriptionRepository::new(self.db.clone()),
            ConfiguredResetCalendar,
        )
    }

    pub(crate) fn subscription_access_service(
        &self,
    ) -> SubscriptionAccessService<PostgresSubscriptionRepository, RedisSubscriptionAccess> {
        SubscriptionAccessService::new(
            PostgresSubscriptionRepository::new(self.db.clone()),
            RedisSubscriptionAccess::new(
                self.auth_redis.clone(),
                self.redis_keys.clone(),
                self.config_snapshot(),
            ),
        )
    }

    pub(crate) fn statistics_service(
        &self,
    ) -> StatisticsService<PostgresStatisticsRepository, ConfiguredStatisticsCalendar> {
        StatisticsService::new(
            PostgresStatisticsRepository::new(self.db.clone()),
            ConfiguredStatisticsCalendar,
        )
    }

    pub(crate) fn log_service(&self) -> LogService<PostgresLogRepository> {
        LogService::new(PostgresLogRepository::new(self.db.clone()))
    }

    pub(crate) fn system_monitoring_service(&self) -> SystemMonitoringService<RedisWorkerMetrics> {
        SystemMonitoringService::new(RedisWorkerMetrics::new(
            self.redis.clone(),
            self.redis_keys.clone(),
        ))
    }

    pub(crate) fn ticket_service(
        &self,
    ) -> TicketService<PostgresTicketRepository, RedisTicketAdmission, TicketEmailNotifications>
    {
        let config = self.config_snapshot();
        TicketService::new(
            PostgresTicketRepository::new(self.db.clone()),
            RedisTicketAdmission::new(self.auth_redis.clone(), self.redis_keys.clone()),
            TicketEmailNotifications::new(
                self.redis.clone(),
                self.redis_keys.clone(),
                config.clone(),
            ),
            TicketPolicy {
                creation: TicketCreationPolicy::from(config.ticket_status),
                withdrawal_closed: config.withdraw_close_enable,
                withdrawal_methods: config.commission_withdraw_method.clone(),
                withdrawal_minimum_mantissa: config.commission_withdraw_limit.mantissa(),
                withdrawal_minimum_scale: config.commission_withdraw_limit.scale(),
                withdrawal_minimum_display: config.commission_withdraw_limit.to_string(),
            },
        )
    }

    pub(crate) fn telegram_service(
        &self,
        bot_token: String,
    ) -> TelegramService<PostgresTelegramRepository, RuntimeTelegramExternal> {
        let config = self.config_snapshot();
        TelegramService::new(
            PostgresTelegramRepository::new(self.db.clone()),
            RuntimeTelegramExternal::new(self.clone(), bot_token),
            TelegramPolicy {
                app_name: config.app_name.clone(),
                app_url: config.app_url.clone().unwrap_or_default(),
                notifications_enabled: config.telegram_bot_enable,
            },
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
    /// `Ok` reports only whether the PostgreSQL-active revision entered
    /// ArcSwap; a failed acknowledgement does not turn an applied revision into
    /// an activation failure. A malformed committed snapshot without a positive
    /// revision is a typed internal error, never a process panic.
    pub(crate) async fn activate_operator_config(
        &self,
        config: AppConfig,
    ) -> Result<bool, ApiError> {
        let _guard = self.config_reload.lock().await;
        let incoming_revision = config
            .operator_revision()
            .filter(|revision| *revision > 0)
            .ok_or_else(|| {
                ApiError::internal("committed operator configuration revision is missing")
            })?;
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
                    return Ok(false);
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
                    return Ok(false);
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
                return Ok(false);
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
                        return Ok(false);
                    }
                    Err(error) => {
                        self.operator_authority_healthy
                            .store(false, Ordering::Release);
                        tracing::error!(?error, "operator validation task failed");
                        return Ok(false);
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
            return Ok(true);
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
        Ok(finish_applied_operator_activation(
            &self.pending_operator_ack,
            active_revision,
            acknowledgement,
        ))
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
pub(crate) async fn reset_admin_totp(
    service: &RuntimeOperatorAccessService,
    email: &str,
) -> anyhow::Result<()> {
    let email = email.trim();
    if email.is_empty() {
        anyhow::bail!("usage: v2board-api reset-admin-totp <email>");
    }
    match service.reset_mfa(email).await? {
        OperatorMfaResetOutcome::AccountNotFound => {
            anyhow::bail!("privileged account not found: {email}")
        }
        OperatorMfaResetOutcome::NoFactorConfigured => {
            println!("no TOTP factor was configured for {email}; nothing to remove");
        }
        OperatorMfaResetOutcome::Reset => println!("TOTP factor removed for {email}"),
    }
    Ok(())
}

pub(crate) async fn reset_admin_password(
    service: &RuntimeOperatorAccessService,
    email: &str,
    password: Option<String>,
) -> anyhow::Result<()> {
    let email = email.trim();
    if email.is_empty() {
        anyhow::bail!(
            "usage: provide the v2board-new-password systemd credential (or V2BOARD_NEW_PASSWORD_FILE) and run v2board-api reset-admin-password <email>"
        );
    }
    match service.reset_password(email, password.as_deref()).await? {
        OperatorPasswordResetOutcome::AccountNotFound => {
            anyhow::bail!("admin account not found: {email}")
        }
        OperatorPasswordResetOutcome::Updated {
            user_id,
            session_cleanup_error: Some(error),
        } => {
            tracing::warn!(
                user_id,
                ?error,
                "administrator password changed but cached sessions could not be removed"
            );
        }
        OperatorPasswordResetOutcome::Updated {
            session_cleanup_error: None,
            ..
        } => {}
    }
    println!("administrator password updated: {email}");
    Ok(())
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
    if let Err(error) =
        v2board_config::systemd_notify("STOPPING=1\nSTATUS=Draining HTTP connections")
    {
        tracing::warn!(?error, "failed to notify systemd of shutdown");
    }
    tracing::info!("shutdown signal received; draining HTTP connections");
}

#[cfg(test)]
mod tests {
    use super::{ActivationSource, activation_source, finish_applied_operator_activation};
    use std::sync::atomic::{AtomicI64, Ordering};
    use v2board_configuration_adapters::operator_config::OperatorConfigError;

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
}
