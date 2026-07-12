use std::net::SocketAddr;

use v2board_config::AppConfig;
use v2board_db::{connect_mysql, migrate_mysql};
use v2board_domain::{auth::PasswordKdf, smtp::SmtpTransportCache};

mod admin;
mod auth;
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
    let command = std::env::args().nth(1);
    if matches!(command.as_deref(), Some("--help" | "-h")) {
        println!(
            "v2board-api\n\nCommands:\n  migrate                       Apply database migrations and local seed\n  reset-admin-password <email>  Read the new password from V2BOARD_NEW_PASSWORD"
        );
        return Ok(());
    }
    if let Some(command) = command.as_deref()
        && !matches!(command, "migrate" | "reset-admin-password")
    {
        anyhow::bail!("unknown v2board-api command: {command}");
    }

    init_tracing();
    let config = AppConfig::try_from_env()?;
    let db = connect_mysql(&config.database_url).await?;
    if command.as_deref() == Some("migrate") {
        migrate_mysql(&db).await?;
        println!("database migrations applied");
        return Ok(());
    }
    let password_kdf = PasswordKdf::new(config.password_kdf_max_parallel);
    if command.as_deref() == Some("reset-admin-password") {
        return reset_admin_password(&db, &config, &password_kdf).await;
    }
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
