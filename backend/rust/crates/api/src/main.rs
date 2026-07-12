use std::net::SocketAddr;

use v2board_config::AppConfig;
use v2board_db::{connect_postgres, migrate_postgres, migrations_current};
use v2board_domain::{auth::PasswordKdf, smtp::SmtpTransportCache};

mod admin;
mod auth;
mod cli;
mod client;
mod codec;
mod commerce;
mod fallback;
mod frontend;
mod i18n;
mod json_value;
mod localization;
mod request_params;
mod route_paths;
mod routes;
mod runtime;
mod server_api;
mod subscription;
mod telegram;
mod ticket;
mod user;
mod validation;

use runtime::{AppState, build_http_client, init_tracing, reset_admin_password, shutdown_signal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    init_tracing();
    let config = AppConfig::try_from_api_env()?;
    let database_url = if matches!(&command, cli::Command::Migrate) {
        migration_database_url(&config, migration_database_secret)?
    } else {
        config.database_url.clone()
    };
    let db = connect_postgres(&database_url).await?;
    if matches!(&command, cli::Command::Migrate) {
        migrate_postgres(&db, config.environment.is_production()).await?;
        println!("database migration lineage verified and any permitted migrations applied");
        return Ok(());
    }
    if !migrations_current(&db).await? {
        anyhow::bail!(
            "refusing to start API commands against a PostgreSQL schema that is not exactly current"
        );
    }
    let password_kdf = PasswordKdf::new(config.password_kdf_max_parallel);
    if let cli::Command::ResetAdminPassword { email } = command {
        return reset_admin_password(
            &db,
            &config,
            &password_kdf,
            &email,
            admin_password_secret,
        )
        .await;
    }
    let installation_id = v2board_db::installation_id(&db).await?;
    let redis = redis::Client::open(config.redis_url.clone())?;
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
    let server_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    config_reloader.abort();
    let _ = config_reloader.await;
    server_result?;

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
