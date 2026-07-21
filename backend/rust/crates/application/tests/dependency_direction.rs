use std::{collections::BTreeSet, fs, path::Path};

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(directory: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).expect("read source directory") {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[test]
fn application_has_an_exact_inward_dependency_allowlist() {
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml")).expect("read Cargo.toml");
    let dependency_lines = manifest
        .split("[dependencies]")
        .nth(1)
        .expect("dependencies section")
        .split('\n')
        .take_while(|line| !line.trim_start().starts_with('['))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim()))
        .map(|name| name.strip_suffix(".workspace").unwrap_or(name).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        dependency_lines,
        BTreeSet::from(["thiserror".to_string(), "v2board-domain-model".to_string(),])
    );
    assert!(!manifest.contains("[dev-dependencies]"));
    assert!(!manifest.contains("[build-dependencies]"));
}

#[test]
fn application_source_cannot_import_transport_or_infrastructure() {
    let mut sources = Vec::new();
    rust_sources(&crate_root().join("src"), &mut sources);
    let forbidden = [
        "axum",
        "sqlx",
        "redis::",
        "reqwest",
        "lettre",
        "serde::",
        "serde_json",
        "tokio::",
        "v2board_compat",
        "v2board_config",
        "v2board_db",
        "ApiError",
        "AppConfig",
        "DbPool",
        "DbTransaction",
    ];
    for source in sources {
        let body = fs::read_to_string(&source).expect("read application source");
        for marker in forbidden {
            assert!(
                !body.contains(marker),
                "{} contains forbidden application dependency marker {marker}",
                source.display()
            );
        }
    }
}

#[test]
fn business_inbound_adapters_have_no_infrastructure_escape_hatch() {
    let crates = crate_root().parent().expect("application is under crates");
    for (relative, service) in [
        ("api/src/user/content.rs", ".content_service()"),
        ("api/src/auth.rs", ".auth_service()"),
        ("api/src/admin/content.rs", ".content_service()"),
        ("api/src/user/invite.rs", ".invite_service()"),
        ("api/src/user/account.rs", ".account_service()"),
        ("api/src/user/stats.rs", ".service_usage_service()"),
        ("api/src/user/giftcard.rs", ".giftcard_service()"),
        ("api/src/user/subscription.rs", ".subscription_service()"),
        ("api/src/admin/commerce.rs", ".plan_service()"),
        ("api/src/admin/commerce.rs", ".reconciliation_service()"),
        ("api/src/admin/commerce.rs", ".admin_order_service()"),
        ("api/src/admin/statistics.rs", ".statistics_service()"),
        ("api/src/admin/configuration.rs", ".configuration_service()"),
        ("api/src/admin/users.rs", ".admin_user_service()"),
        ("api/src/admin/servers.rs", ".server_management_service()"),
        ("api/src/admin/support.rs", ".ticket_service()"),
        ("api/src/ticket.rs", ".ticket_service()"),
        ("api/src/telegram.rs", ".telegram_service("),
        ("api/src/server_api/mod.rs", ".server_runtime_service()"),
        ("api/src/server_api/request.rs", ".server_runtime_service()"),
        ("api/src/server_api/config.rs", ".server_runtime_service()"),
        ("api/src/server_api/users.rs", ".server_runtime_service()"),
        ("api/src/server_api/traffic.rs", ".server_runtime_service()"),
        ("api/src/audit.rs", ".audit_service()"),
    ] {
        let source = crates.join(relative);
        let body = fs::read_to_string(&source).expect("read business inbound adapter");
        for marker in [
            "v2board_db",
            "sqlx::",
            "state.db",
            ".db.begin()",
            "state.http",
            "reqwest::",
            "redis::",
            ".auth_redis",
            "TimestampOrderNumberGenerator",
        ] {
            assert!(
                !body.contains(marker),
                "{} bypasses its application port through {marker}",
                source.display()
            );
        }
        assert!(
            body.contains(service),
            "{} must invoke its application service through {service}",
            source.display()
        );
    }

    for (relative, implementation) in [
        (
            "db/src/auth.rs",
            "impl AuthRepository for PostgresAuthRepository",
        ),
        (
            "db/src/content.rs",
            "impl ContentRepository for PostgresContentRepository",
        ),
        (
            "db/src/invite.rs",
            "impl InviteRepository for PostgresInviteRepository",
        ),
        (
            "db/src/account.rs",
            "impl AccountRepository for PostgresAccountRepository",
        ),
        (
            "db/src/service_usage.rs",
            "impl ServiceUsageRepository for PostgresServiceUsageRepository",
        ),
        (
            "db/src/ticket.rs",
            "impl TicketRepository for PostgresTicketRepository",
        ),
        (
            "db/src/giftcard.rs",
            "impl GiftCardRepository for PostgresGiftCardRepository",
        ),
        (
            "db/src/subscription.rs",
            "impl SubscriptionRepository for PostgresSubscriptionRepository",
        ),
        (
            "db/src/subscription.rs",
            "impl ClientSubscriptionRepository for PostgresSubscriptionRepository",
        ),
        (
            "db/src/plan.rs",
            "impl PlanRepository for PostgresPlanRepository",
        ),
        (
            "db/src/statistics.rs",
            "impl StatisticsRepository for PostgresStatisticsRepository",
        ),
        (
            "db/src/logs.rs",
            "impl LogRepository for PostgresLogRepository",
        ),
        (
            "db/src/admin_payment.rs",
            "impl PaymentRepository for PostgresPaymentRepository",
        ),
        (
            "db/src/reconciliation.rs",
            "impl ReconciliationRepository for PostgresReconciliationRepository",
        ),
        (
            "db/src/admin_order.rs",
            "impl AdminOrderRepository for PostgresAdminOrderRepository",
        ),
        (
            "db/src/admin_user.rs",
            "impl AdminUserRepository for PostgresAdminUserRepository",
        ),
        (
            "db/src/telegram.rs",
            "impl TelegramRepository for PostgresTelegramRepository",
        ),
        (
            "db/src/admin_server.rs",
            "impl ServerManagementRepository for PostgresServerManagementRepository",
        ),
        (
            "db/src/server_runtime.rs",
            "impl ServerRuntimeRepository for PostgresServerRuntimeRepository",
        ),
        (
            "db/src/audit.rs",
            "impl AuditRepository for PostgresAuditRepository",
        ),
    ] {
        let adapter = crates.join(relative);
        let body = fs::read_to_string(&adapter).expect("read postgres adapter");
        assert!(
            body.contains(implementation),
            "{} does not implement the expected application port",
            adapter.display()
        );
    }

    let client = fs::read_to_string(crates.join("api/src/client.rs"))
        .expect("read byte-frozen client inbound adapter");
    let client_subscription = client
        .split("pub(crate) async fn payment_notify")
        .next()
        .expect("client subscription section precedes payment notification");
    assert!(
        client_subscription.contains(".subscription_service()"),
        "client subscription endpoints must invoke the application subscription service"
    );
    for marker in ["v2board_db", "sqlx::", "state.db", "redis::"] {
        assert!(
            !client_subscription.contains(marker),
            "client subscription endpoints bypass application ports through {marker}"
        );
    }

    let server_runtime_adapter =
        fs::read_to_string(crates.join("api/src/server_runtime_adapters.rs"))
            .expect("read external server runtime cache adapter");
    assert!(
        server_runtime_adapter.contains("impl ServerRuntimeCache for RedisServerRuntimeCache"),
        "external server Redis integration must implement the application cache port"
    );
    assert!(
        !crates.join("api/src/server_api/repository.rs").exists(),
        "the SQL-backed server API repository escape hatch must be deleted"
    );
    assert!(
        !crates.join("domain/src/server_credentials.rs").exists(),
        "node credential derivation must live in the server adapter crate"
    );
    for relative in [
        "api/src/server_management_adapters.rs",
        "api/src/server_api/request.rs",
        "contract/src/production_invariants/access_control.rs",
    ] {
        let source = fs::read_to_string(crates.join(relative)).expect("read credential consumer");
        assert!(
            !source.contains("v2board_domain::server_credentials"),
            "{relative} still reaches into the retired domain credential helper"
        );
    }

    for (relative, implementation) in [
        (
            "auth-adapters/src/cache.rs",
            "impl AuthCache for RedisAuthCache",
        ),
        (
            "auth-adapters/src/external.rs",
            "impl AuthExternal for RuntimeAuthExternal",
        ),
        (
            "configuration-adapters/src/repository.rs",
            "impl ConfigurationRepository for RuntimeConfigurationRepository",
        ),
        (
            "configuration-adapters/src/external.rs",
            "impl ConfigurationExternal for RuntimeConfigurationExternal",
        ),
        (
            "configuration-adapters/src/bulk_mail.rs",
            "impl BulkMailRepository for PostgresBulkMailRepository",
        ),
    ] {
        let adapter = crates.join(relative);
        let body = fs::read_to_string(&adapter).expect("read authentication outer adapter");
        assert!(
            body.contains(implementation),
            "{} does not implement the expected authentication port",
            adapter.display()
        );
    }

    let subscription_handler = fs::read_to_string(crates.join("api/src/user/subscription.rs"))
        .expect("read subscription inbound adapter");
    assert!(
        subscription_handler.contains(".subscription_access_service()"),
        "subscription token and projection flows must use the application access service"
    );
    for marker in [
        "redis::",
        ".auth_redis",
        "state.redis",
        "reqwest::",
        "sqlx::",
    ] {
        assert!(
            !subscription_handler.contains(marker),
            "subscription inbound adapter bypasses its application ports through {marker}"
        );
    }
    let subscription_adapter = fs::read_to_string(crates.join("api/src/subscription_adapters.rs"))
        .expect("read subscription outer adapter");
    assert!(
        subscription_adapter
            .contains("impl SubscriptionAccessExternal for RedisSubscriptionAccess"),
        "subscription Redis/crypto integrations must implement the application external port"
    );

    assert!(
        !crates.join("domain/src/admin/content.rs").exists(),
        "the replaced SQL-backed legacy content service must be deleted"
    );
    assert!(
        !crates.join("domain/src/admin/tickets.rs").exists(),
        "the replaced SQL-backed legacy ticket service must be deleted"
    );
    assert!(
        !crates.join("domain/src/admin/commerce/plans.rs").exists(),
        "the replaced SQL-backed legacy plan service must be deleted"
    );
    assert!(
        !crates.join("domain/src/admin/statistics.rs").exists(),
        "the replaced SQL/Redis-backed reporting service must be deleted"
    );
    assert!(
        !crates.join("domain/src/admin/configuration.rs").exists(),
        "the replaced configuration orchestration service must be deleted"
    );
    assert!(
        !crates.join("domain/src/admin/users.rs").exists(),
        "the replaced SQL/Redis-backed admin-user service must be deleted"
    );
    let admin_user_adapter = fs::read_to_string(crates.join("api/src/admin_user_adapters.rs"))
        .expect("read admin-user outer adapter");
    assert!(
        admin_user_adapter.contains("impl AdminUserExternal for RuntimeAdminUserExternal"),
        "admin-user runtime integrations must implement the application external port"
    );
    let telegram_adapter = fs::read_to_string(crates.join("api/src/telegram_adapters.rs"))
        .expect("read Telegram outer adapter");
    assert!(
        telegram_adapter.contains("impl TelegramExternal for RuntimeTelegramExternal"),
        "Telegram runtime integrations must implement the application external port"
    );
    assert!(
        telegram_adapter.contains("v2board_http_adapters::bounded_bytes")
            && telegram_adapter.contains("v2board_http_adapters::bounded_json")
            && !telegram_adapter.contains("v2board_domain::http_response"),
        "Telegram HTTP boundaries must use the dedicated bounded HTTP adapter"
    );
    let telegram_handler =
        fs::read_to_string(crates.join("api/src/telegram.rs")).expect("read Telegram handler");
    for marker in [
        "sqlx::",
        "state.db",
        "redis::",
        ".auth_redis",
        "reqwest::",
        ".http",
    ] {
        assert!(
            !telegram_handler.contains(marker),
            "Telegram inbound adapter bypasses its application ports through {marker}"
        );
    }
    let user_content = fs::read_to_string(crates.join("api/src/user/content.rs"))
        .expect("read user content handler");
    assert!(
        user_content.contains(".telegram_service(") && user_content.contains(".bot_username()"),
        "the user Telegram-bot projection must invoke the Telegram application service"
    );
    for marker in ["state.http", "reqwest::", "bounded_json"] {
        assert!(
            !user_content.contains(marker),
            "the user Telegram-bot handler bypasses TelegramExternal through {marker}"
        );
    }
    for contract_marker in [
        "x-telegram-bot-api-secret-token",
        "1024 * 1024",
        "json!({ \"data\": true })",
    ] {
        assert!(
            telegram_handler.contains(contract_marker),
            "Telegram inbound adapter lost frozen webhook marker {contract_marker}"
        );
    }
    assert!(
        !crates.join("domain/src/admin.rs").exists(),
        "the final legacy AdminService/webhook helper module must be deleted"
    );
    assert!(
        !crates.join("config/src/telegram.rs").exists(),
        "Telegram integration secret derivation must not live in the typed config crate"
    );
    let telegram_configuration_adapter =
        fs::read_to_string(crates.join("configuration-adapters/src/telegram.rs"))
            .expect("read Telegram configuration adapter");
    assert!(
        telegram_configuration_adapter.contains("pub fn telegram_webhook_secret"),
        "Telegram webhook secret derivation must live at the integration adapter boundary"
    );
    for relative in ["api/src/admin/users.rs", "api/src/admin.rs"] {
        let source = fs::read_to_string(crates.join(relative)).expect("read admin-user handler");
        assert!(
            !source.contains(".admin_service("),
            "{relative} must not route through the removed AdminService"
        );
        for marker in ["sqlx::", "state.db", "redis::"] {
            assert!(
                !source.contains(marker),
                "{relative} bypasses the admin-user application ports through {marker}"
            );
        }
    }
    let api_configuration = fs::read_to_string(crates.join("api/src/admin/configuration.rs"))
        .expect("read configuration inbound adapter");
    assert!(
        !api_configuration.contains(".admin_service("),
        "configuration handlers must not route through the legacy AdminService"
    );
    for relative in ["api/src/admin/users.rs", "api/src/admin.rs"] {
        let source = fs::read_to_string(crates.join(relative)).expect("read mail inbound adapter");
        for legacy_call in [".users_mail(", ".staff_users_mail("] {
            assert!(
                !source.contains(legacy_call),
                "{relative} must not route bulk mail through {legacy_call}"
            );
        }
    }
    assert!(
        !crates
            .join("domain/src/admin/commerce/payments.rs")
            .exists(),
        "the replaced SQL-backed payment service must be deleted"
    );
    assert!(
        !crates
            .join("domain/src/admin/commerce/reconciliations.rs")
            .exists(),
        "the replaced SQL-backed reconciliation service must be deleted"
    );
    assert!(
        !crates.join("domain/src/admin/commerce.rs").exists()
            && !crates.join("domain/src/admin/commerce").exists(),
        "the replaced SQL-backed admin commerce service must be deleted"
    );

    let admin_commerce = fs::read_to_string(crates.join("api/src/admin/commerce.rs"))
        .expect("read admin commerce adapter");
    assert!(
        admin_commerce.contains(".payment_service()"),
        "admin payment handlers must invoke the application payment service"
    );
    assert!(
        admin_commerce.contains(".reconciliation_service()"),
        "admin reconciliation handlers must invoke the application reconciliation service"
    );
    assert!(
        admin_commerce.contains(".admin_order_service()"),
        "admin order handlers must invoke the application admin-order service"
    );
    for legacy_auth_source in [
        "credentials.rs",
        "mfa.rs",
        "registration.rs",
        "sessions.rs",
        "validation.rs",
        "verification.rs",
    ] {
        assert!(
            !crates
                .join("domain/src/auth")
                .join(legacy_auth_source)
                .exists(),
            "the replaced auth orchestration source {legacy_auth_source} must be deleted"
        );
    }

    assert!(
        !crates.join("domain/Cargo.toml").exists(),
        "the retired mixed-responsibility domain crate must be deleted"
    );
    for manifest in [
        "api/Cargo.toml",
        "workers/Cargo.toml",
        "contract/Cargo.toml",
    ] {
        let source = fs::read_to_string(crates.join(manifest)).expect("read runtime manifest");
        assert!(
            !source.contains("v2board-domain ="),
            "{manifest} still depends on the retired domain crate"
        );
    }

    let order_repository = fs::read_to_string(crates.join("db/src/order_runtime.rs"))
        .expect("read order repository adapter");
    assert!(
        order_repository.contains("impl<V> OrderRepository for PostgresOrderRepository<V>"),
        "PostgreSQL order persistence must implement the application port"
    );
    let payment_gateway = fs::read_to_string(crates.join("payment-adapters/src/gateway/mod.rs"))
        .expect("read payment gateway adapter");
    assert!(
        payment_gateway.contains("impl PaymentGateway for RuntimePaymentGateway")
            && payment_gateway.contains("impl PaymentSnapshotVerifier for RuntimePaymentGateway"),
        "runtime payment integration must implement both order payment ports"
    );
    let order_composition = fs::read_to_string(crates.join("order-adapters/src/lib.rs"))
        .expect("read order composition adapter");
    assert!(
        order_composition.contains("pub fn runtime_order_service")
            && order_composition.contains("impl OrderPolicy for ConfiguredOrderPolicy"),
        "order runtime composition must own infrastructure policy wiring"
    );
    for relative in [
        "api/src/commerce.rs",
        "api/src/client.rs",
        "api/src/admin_order_adapters.rs",
        "workers/src/orders.rs",
    ] {
        let source = fs::read_to_string(crates.join(relative)).expect("read order consumer");
        assert!(
            !source.contains("v2board_domain::order"),
            "{relative} still reaches into the retired order implementation"
        );
    }
    let commerce =
        fs::read_to_string(crates.join("api/src/commerce.rs")).expect("read commerce adapter");
    assert!(
        commerce.contains(".order_service()"),
        "user commerce order handlers must invoke the order application service"
    );

    for (relative, service) in [
        ("workers/src/reminders.rs", "reminder_service("),
        ("workers/src/outbox.rs", "mail_outbox_service("),
    ] {
        let source = fs::read_to_string(crates.join(relative)).expect("read worker mail adapter");
        assert!(
            source.contains(service),
            "{relative} must invoke the application-backed worker mail service"
        );
        for marker in [
            "sqlx::",
            ".db.begin()",
            "QueryBuilder",
            "Transaction<'_",
            "lettre::",
            "AsyncTransport",
        ] {
            assert!(
                !source.contains(marker),
                "{relative} bypasses worker mail ports through {marker}"
            );
        }
    }
    let worker_mail_adapter = fs::read_to_string(crates.join("mail-adapters/src/worker.rs"))
        .expect("read production worker mail adapters");
    for implementation in [
        "impl ReminderRepository for PostgresReminderRepository",
        "impl ReminderRenderer for RuntimeReminderRenderer",
        "impl MailOutboxRepository for PostgresMailOutboxRepository",
        "impl MailDelivery for SmtpMailDelivery",
    ] {
        assert!(
            worker_mail_adapter.contains(implementation),
            "mail adapter does not implement {implementation}"
        );
    }
}

#[test]
fn ticket_maintenance_worker_uses_the_application_port() {
    let crates = crate_root().parent().expect("application is under crates");
    let runner = fs::read_to_string(crates.join("workers/src/tickets.rs"))
        .expect("read ticket maintenance worker runner");
    assert!(
        runner.contains("TicketMaintenance::new"),
        "ticket maintenance worker must invoke the application use case"
    );
    for marker in [
        "sqlx::",
        "QueryBuilder",
        "Transaction<",
        ".db.begin()",
        "redis::",
        "lettre::",
        "reqwest::",
    ] {
        assert!(
            !runner.contains(marker),
            "ticket maintenance worker bypasses its application port through {marker}"
        );
    }
}

#[test]
fn traffic_and_statistics_worker_runners_use_application_ports() {
    let crates = crate_root().parent().expect("application is under crates");
    for (relative, service) in [
        ("workers/src/traffic.rs", "TrafficWorkerService::new"),
        ("workers/src/statistics.rs", "StatisticsWorkerService::new"),
    ] {
        let source = fs::read_to_string(crates.join(relative)).expect("read worker runner");
        assert!(
            source.contains(service),
            "{relative} must invoke its application use case through {service}"
        );
        for marker in [
            "sqlx::",
            "QueryBuilder",
            "Transaction<",
            "v2board_analytics",
        ] {
            assert!(
                !source.contains(marker),
                "{relative} bypasses its application port through {marker}"
            );
        }
    }

    let traffic_repository = fs::read_to_string(crates.join("db/src/worker_traffic.rs"))
        .expect("read traffic accounting PostgreSQL adapter");
    assert!(
        traffic_repository
            .contains("impl TrafficAccountingRepository for PostgresTrafficAccountingRepository")
    );
    assert!(
        traffic_repository.contains("enqueue_events(transaction, &events, accounted_at)")
            && traffic_repository.contains("transaction.commit().await?")
            && traffic_repository.contains("SET applied_at = $1, updated_at = $2"),
        "traffic user updates, accounted analytics events, and acknowledgement must remain in one PostgreSQL transaction"
    );

    let statistics_repository = fs::read_to_string(crates.join("db/src/worker_statistics.rs"))
        .expect("read statistics PostgreSQL adapter");
    assert!(
        statistics_repository
            .contains("impl StatisticsWorkerRepository for PostgresStatisticsWorkerRepository")
    );
    let traffic_barrier = fs::read_to_string(crates.join("workers/src/traffic_adapters.rs"))
        .expect("read traffic reset barrier adapter");
    assert!(traffic_barrier.contains("impl TrafficResetBarrier for RedisTrafficResetBarrier"));
}

#[test]
fn maintenance_worker_runner_uses_application_ports() {
    let crates = crate_root().parent().expect("application is under crates");
    let runner = fs::read_to_string(crates.join("workers/src/reset.rs"))
        .expect("read maintenance worker runner");
    for service in ["ScheduledTrafficResetService::new", "RetentionService::new"] {
        assert!(
            runner.contains(service),
            "maintenance worker must invoke the application use case through {service}"
        );
    }
    for marker in [
        "sqlx::",
        "QueryBuilder",
        "Transaction<",
        "state.db.begin()",
        "redis::",
        "state.redis",
    ] {
        assert!(
            !runner.contains(marker),
            "maintenance worker bypasses its application port through {marker}"
        );
    }

    let adapter = fs::read_to_string(crates.join("db/src/maintenance.rs"))
        .expect("read maintenance PostgreSQL adapter");
    for implementation in [
        "impl ScheduledTrafficResetBatch for PostgresScheduledTrafficResetBatch",
        "impl ScheduledTrafficResetRepository for PostgresMaintenanceRepository",
        "impl RetentionRepository for PostgresMaintenanceRepository",
    ] {
        assert!(
            adapter.contains(implementation),
            "maintenance adapter does not implement {implementation}"
        );
    }
}

#[test]
fn operator_recovery_cli_uses_application_ports() {
    let crates = crate_root().parent().expect("application is under crates");
    let runtime =
        fs::read_to_string(crates.join("api/src/runtime.rs")).expect("read API runtime adapter");
    let recovery = runtime
        .split("pub(crate) async fn reset_admin_totp")
        .nth(1)
        .expect("operator TOTP recovery function")
        .split("pub(crate) struct TelemetryGuard")
        .next()
        .expect("operator recovery section precedes telemetry");
    for service_call in ["service.reset_mfa(", "service.reset_password("] {
        assert!(
            recovery.contains(service_call),
            "operator recovery CLI must invoke {service_call}"
        );
    }
    for marker in [
        "sqlx::",
        ".begin()",
        "redis::Client",
        "v2board_db::admin_mfa",
    ] {
        assert!(
            !recovery.contains(marker),
            "operator recovery CLI bypasses its application port through {marker}"
        );
    }

    let repository = fs::read_to_string(crates.join("db/src/operator_access.rs"))
        .expect("read operator access PostgreSQL adapter");
    assert!(
        repository.contains("impl OperatorAccessRepository for PostgresOperatorAccessRepository")
    );
    let external = fs::read_to_string(crates.join("auth-adapters/src/operator_access.rs"))
        .expect("read operator access KDF/Redis adapter");
    assert!(external.contains("impl OperatorAccessExternal for RuntimeOperatorAccessExternal"));
}

#[test]
fn order_catalog_and_scheduled_order_runners_use_application_ports() {
    let crates = crate_root().parent().expect("application is under crates");
    for (relative, service) in [
        ("workers/src/orders.rs", "runtime_order_service("),
        ("workers/src/commission.rs", "runtime_commission_service("),
        ("workers/src/renewal.rs", "runtime_renewal_service("),
    ] {
        let runner = fs::read_to_string(crates.join(relative)).expect("read order worker runner");
        assert!(
            runner.contains(service),
            "{relative} must invoke its application-backed service through {service}"
        );
        for marker in [
            "sqlx::",
            "QueryBuilder",
            "Transaction<",
            ".db.begin()",
            "SELECT ",
            "INSERT ",
            "UPDATE ",
            "DELETE ",
        ] {
            assert!(
                !runner.contains(marker),
                "{relative} bypasses order application ports through {marker}"
            );
        }
    }

    let commerce =
        fs::read_to_string(crates.join("api/src/commerce.rs")).expect("read commerce handler");
    assert!(commerce.contains(".catalog_plans()"));
    assert!(commerce.contains(".catalog_plan("));
    assert!(commerce.contains(".payment_methods()"));
    for marker in [
        "v2board_db",
        "sqlx::",
        "PlanRow",
        "OrderRow",
        "PaymentMethodRow",
    ] {
        assert!(
            !commerce.contains(marker),
            "commerce handlers and tests bypass application projections through {marker}"
        );
    }

    let database = fs::read_to_string(crates.join("db/src/order_jobs.rs"))
        .expect("read scheduled order PostgreSQL adapters");
    for implementation in [
        "impl CommissionRepository for PostgresOrderJobsRepository",
        "impl RenewalRepository for PostgresOrderJobsRepository",
    ] {
        assert!(
            database.contains(implementation),
            "scheduled order adapter does not implement {implementation}"
        );
    }
    let order_database = fs::read_to_string(crates.join("db/src/order_runtime.rs"))
        .expect("read order PostgreSQL adapter");
    assert!(order_database.contains("impl<V> OrderRepository for PostgresOrderRepository<V>"));

    let composition = fs::read_to_string(crates.join("order-adapters/src/lib.rs"))
        .expect("read order runtime composition");
    for service in [
        "runtime_order_service",
        "runtime_commission_service",
        "runtime_renewal_service",
    ] {
        assert!(
            composition.contains(service),
            "order runtime composition is missing {service}"
        );
    }
}
