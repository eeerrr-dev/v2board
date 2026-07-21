use std::net::SocketAddr;

use v2board_auth_adapters::{PasswordKdf, runtime_operator_access_service};
use v2board_config::AppConfig;
use v2board_configuration_adapters::operator_config::{self, OperatorConfigConsumer};
use v2board_db::{connect_postgres, migrate_postgres, migrations_current};
use v2board_mail_adapters::smtp::SmtpTransportCache;

/// Define one Axum subrouter from canonical internal-operation ids. The macro
/// deliberately accepts no method or path: both come from
/// `v2board_api_contract::INTERNAL_OPERATIONS`, and the generated id slice lets
/// the coverage test prove every registry entry is bound exactly once.
macro_rules! define_internal_operation_router {
    (
        $visibility:vis fn $router_name:ident;
        $ids_visibility:vis const $ids_name:ident;
        { $( $id:literal [$surface:ident] => $handler:path ),+ $(,)? }
    ) => {
        #[cfg(test)]
        $ids_visibility const $ids_name: &[&str] = &[$($id),+];

        $visibility fn $router_name() -> axum::Router<crate::runtime::AppState> {
            let router = axum::Router::new();
            $(
                let router = crate::routes::bind_internal_operation(
                    router,
                    v2board_api_contract::OperationSurface::$surface,
                    $id,
                    $handler,
                );
            )+
            router
        }
    };
}

mod admin;
mod admin_order_adapters;
mod admin_user_adapters;
mod audit;
mod auth;
mod cli;
mod client;
mod codec;
mod commerce;
mod dialect;
mod fallback;
mod frontend;
#[cfg(test)]
mod golden_wire;
mod i18n;
mod json_value;
mod locale;
mod localization;
mod metrics;
mod rate_limit;
mod request_params;
mod route_paths;
mod routes;
mod runtime;
mod server_api;
mod server_management_adapters;
mod server_runtime_adapters;
mod service_usage_adapters;
mod statistics_adapters;
mod subscription;
mod subscription_adapters;
mod telegram;
mod telegram_adapters;
mod ticket;
mod ticket_adapters;
mod user;
mod validation;

use runtime::{
    AppState, build_http_client, init_tracing, reset_admin_password, reset_admin_totp,
    shutdown_signal,
};

fn main() -> anyhow::Result<()> {
    // Telemetry must initialize before the tokio runtime exists: the OTLP
    // batch exporter constructs a blocking HTTP client, which panics inside
    // an async context. The guard flushes both exports on drop.
    let _telemetry = init_tracing();
    run()
}

#[tokio::main]
async fn run() -> anyhow::Result<()> {
    let command = cli::parse()?;
    if matches!(&command, cli::Command::Help) {
        cli::print_help();
        return Ok(());
    }
    let migration_database_secret = v2board_config::one_shot_secret(
        "V2BOARD_MIGRATION_DATABASE_URL",
        "V2BOARD_MIGRATION_DATABASE_URL_FILE",
        "v2board-migration-database-url",
    )?;
    let admin_password_secret = v2board_config::one_shot_secret(
        "V2BOARD_NEW_PASSWORD",
        "V2BOARD_NEW_PASSWORD_FILE",
        "v2board-new-password",
    )?;
    if !matches!(&command, cli::Command::Migrate) && migration_database_secret.is_some() {
        anyhow::bail!(
            "the one-shot migration database credential is forbidden for this API command"
        );
    }
    if !matches!(&command, cli::Command::ResetAdminPassword { .. })
        && admin_password_secret.is_some()
    {
        anyhow::bail!(
            "the one-shot administrator password credential is forbidden for this API command"
        );
    }

    let config = AppConfig::try_from_api_boot_env()?;
    let database_url = if matches!(&command, cli::Command::Migrate) {
        migration_database_url(&config, migration_database_secret)?
    } else {
        config.database_url.clone()
    };
    let db = connect_postgres(&database_url).await?;
    if matches!(&command, cli::Command::Migrate) {
        migrate_postgres(&db, config.environment.is_production()).await?;
        if !config.environment.is_production() {
            install_local_analytics_admission(&db).await?;
        }
        println!("database migration lineage verified and any permitted migrations applied");
        return Ok(());
    }
    if !migrations_current(&db).await? {
        anyhow::bail!(
            "refusing to start API commands against a PostgreSQL schema that is not exactly current"
        );
    }
    let password_kdf = PasswordKdf::new(config.password_kdf_max_parallel);
    let installation_id = v2board_db::installation_id(&db).await?;
    if let cli::Command::ResetAdminPassword { email } = command {
        let service = runtime_operator_access_service(
            db,
            password_kdf,
            config.redis_url.clone(),
            installation_id,
        );
        return reset_admin_password(&service, &email, admin_password_secret).await;
    }
    if let cli::Command::ResetAdminTotp { email } = command {
        let service = runtime_operator_access_service(
            db,
            password_kdf,
            config.redis_url.clone(),
            installation_id,
        );
        return reset_admin_totp(&service, &email).await;
    }
    let authority_result = if config.environment.is_production() {
        operator_config::load_active(&db, installation_id, &config.app_key)
            .await
            .and_then(|authority| {
                authority.ok_or(operator_config::OperatorConfigError::MissingAuthority)
            })
    } else {
        // Local development has an explicit, disposable bootstrap path so a
        // fresh `make reset` remains self-contained. Production authority is
        // seeded during initial database preparation before either long-lived role starts.
        operator_config::ensure_authority(&db, installation_id, &config).await
    };
    let authority = match authority_result {
        Ok(authority) => authority,
        Err(error) => {
            if let Some((observed_revision, error_code)) = error.observed_rejection() {
                let _ = operator_config::acknowledge(
                    &db,
                    installation_id,
                    OperatorConfigConsumer::Api,
                    observed_revision,
                    None,
                    Some(error_code),
                )
                .await;
            }
            return Err(error.into());
        }
    };
    let config = config.with_operator_config(&authority.values, authority.revision)?;
    operator_config::acknowledge(
        &db,
        installation_id,
        OperatorConfigConsumer::Api,
        authority.revision,
        Some(authority.revision),
        None,
    )
    .await?;
    let redis = redis::Client::open(config.redis_url.clone())?;
    tokio::time::timeout(
        std::time::Duration::from_secs(config.http_connect_timeout_seconds),
        v2board_redis_adapters::verify_redis_runtime(&redis, config.environment),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out verifying the Redis runtime policy"))??;
    let auth_redis = tokio::time::timeout(
        std::time::Duration::from_secs(config.http_connect_timeout_seconds),
        redis::aio::ConnectionManager::new(redis.clone()),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out establishing the Redis connection manager"))??;
    let http = build_http_client(&config)?;
    let smtp = SmtpTransportCache::default();
    let state = AppState::new(
        config.clone(),
        db,
        installation_id,
        redis,
        auth_redis,
        http,
        password_kdf,
        smtp,
    );
    let config_reloader = state.spawn_config_reloader();
    let app = routes::build_app(state, &config);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(bind_addr = %config.bind_addr, "v2board rust api listening");
    v2board_config::systemd_notify("READY=1\nSTATUS=HTTP listener is accepting connections")?;
    // Unlike the worker (whose watchdog is gated on dependency health so systemd
    // restarts it away from a wedged dependency), the API pings unconditionally:
    // dependency outages are surfaced through /readyz and 5xx responses, where a
    // restart would only add flapping. The watchdog exists to catch a hung or
    // deadlocked event loop.
    let watchdog = tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = v2board_config::systemd_notify("WATCHDOG=1") {
                tracing::warn!(?error, "failed to ping the systemd watchdog");
            }
        }
    });
    let server_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    watchdog.abort();
    let _ = watchdog.await;
    config_reloader.abort();
    let _ = config_reloader.await;
    server_result?;

    Ok(())
}

async fn install_local_analytics_admission(db: &sqlx::PgPool) -> anyhow::Result<()> {
    let installation_id = v2board_db::installation_id(db).await?;
    let now: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint")
            .fetch_one(db)
            .await?;
    let gib = 1024_u64 * 1024 * 1024;
    let policy = v2board_analytics::AnalyticsAdmissionPolicy {
        recovery_pending_rows: 750_000,
        soft_pending_rows: 1_000_000,
        hard_pending_rows: 2_000_000,
        recovery_relation_bytes: 3 * gib,
        soft_relation_bytes: 4 * gib,
        hard_relation_bytes: 8 * gib,
        recovery_oldest_age_seconds: 120,
        soft_oldest_age_seconds: 300,
        hard_oldest_age_seconds: 1_800,
        database_capacity_bytes: 64 * gib,
        hard_min_headroom_bytes: 8 * gib,
        soft_min_headroom_bytes: 16 * gib,
        recovery_min_headroom_bytes: 20 * gib,
        event_reservation_bytes: 4_096,
        soft_max_new_rows_per_second: 100_000,
        sample_interval_seconds: 1,
        stale_after_seconds: 10,
        capacity_evidence: "local Docker PostgreSQL volume development budget".to_owned(),
    };
    v2board_analytics::install_analytics_admission_policy(db, installation_id, &policy, now)
        .await?;
    v2board_analytics::refresh_analytics_admission(db).await?;
    Ok(())
}

fn migration_database_url(
    config: &AppConfig,
    migration_database_secret: Option<String>,
) -> anyhow::Result<String> {
    match migration_database_secret {
        Some(value) => {
            let migration = url::Url::parse(&value)
                .map_err(|_| anyhow::anyhow!("V2BOARD_MIGRATION_DATABASE_URL is invalid"))?;
            v2board_config::validate_postgres_connection_query(
                &migration,
                config.environment.is_production(),
            )?;
            let runtime = url::Url::parse(&config.database_url)
                .map_err(|_| anyhow::anyhow!("database_url is invalid"))?;
            let migration_database = v2board_config::postgres_database_name(&migration)?;
            let runtime_database = v2board_config::postgres_database_name(&runtime)?;
            if !matches!(migration.scheme(), "postgres" | "postgresql")
                || migration.host_str().map(str::to_ascii_lowercase)
                    != runtime.host_str().map(str::to_ascii_lowercase)
                || migration.port().unwrap_or(5432) != runtime.port().unwrap_or(5432)
                || migration_database != runtime_database
            {
                anyhow::bail!(
                    "V2BOARD_MIGRATION_DATABASE_URL must target the configured PostgreSQL database"
                );
            }
            let migration_principal = v2board_config::postgres_principal_name(&migration)?;
            let runtime_principal = v2board_config::postgres_principal_name(&runtime)?;
            if config.environment.is_production() && migration_principal.is_empty() {
                anyhow::bail!(
                    "V2BOARD_MIGRATION_DATABASE_URL must name an explicit PostgreSQL principal"
                );
            }
            if config.environment.is_production()
                && (migration_principal == runtime_principal
                    || migration_principal == config.peer_database_principal)
            {
                anyhow::bail!(
                    "V2BOARD_MIGRATION_DATABASE_URL must use a principal distinct from API and declared worker"
                );
            }
            Ok(value)
        }
        _ if config.environment.is_production() => anyhow::bail!(
            "V2BOARD_MIGRATION_DATABASE_URL is required for production migrations; \
             migration credentials must not be retained as the API database_url"
        ),
        _ => Ok(config.database_url.clone()),
    }
}
