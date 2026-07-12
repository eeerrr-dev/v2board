use std::net::SocketAddr;

use v2board_config::AppConfig;
use v2board_db::{connect_mysql, migrate_mysql};
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

    init_tracing();
    match &command {
        cli::Command::ProvisionValidate { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "valid": true,
                    "schema_version": spec.schema_version,
                    "operation_id": spec.operation_id,
                    "reference_commit": v2board_provision::LEGACY_REFERENCE_COMMIT,
                    "manifest_binding_hmac_sha256": spec.manifest_binding_hmac_sha256(),
                    "secrets_redacted": true
                }))?
            );
            return Ok(());
        }
        cli::Command::ProvisionInspect { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let inspection = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::Online,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
            if !inspection.passed() {
                anyhow::bail!(
                    "online provision compatibility inspection is blocked; see the JSON report"
                );
            }
            return Ok(());
        }
        cli::Command::ProvisionPlan { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let plan = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::FencedFinal,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
            if !plan.passed() {
                anyhow::bail!("fenced final provision plan is blocked; see the JSON report");
            }
            return Ok(());
        }
        cli::Command::Serve
        | cli::Command::Migrate
        | cli::Command::ResetAdminPassword { .. }
        | cli::Command::Help => {}
    }

    let config = AppConfig::try_from_env()?;
    let db = connect_mysql(&config.database_url).await?;
    if matches!(&command, cli::Command::Migrate) {
        migrate_mysql(&db).await?;
        println!("database migrations applied");
        return Ok(());
    }
    let password_kdf = PasswordKdf::new(config.password_kdf_max_parallel);
    if let cli::Command::ResetAdminPassword { email } = command {
        return reset_admin_password(&db, &config, &password_kdf, &email).await;
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
