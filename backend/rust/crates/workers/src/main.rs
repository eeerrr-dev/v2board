//! v2board background worker.
//!
//! Scheduler side-effects (traffic accounting, order settlement, commission
//! payout, reminder mail, log/traffic resets, statistics) run in independent
//! Tokio/cron loops guarded by renewable distributed Redis leases. The same
//! runtime also drains the durable SQL mail outbox used by bulk, transactional,
//! and scheduled reminder mail; no second queue framework or worker runtime is
//! introduced.

mod analytics;
mod batch;
mod commission;
mod lease;
mod metrics;
mod orders;
mod outbox;
mod reminders;
mod renewal;
mod reset;
mod runtime;
mod scheduler;
mod state;
mod statistics;
mod tickets;
mod time;
mod traffic;
mod traffic_adapters;

use std::{env, sync::Arc};

use v2board_config::AppConfig;
use v2board_configuration_adapters::operator_config::{
    self, OperatorConfigConsumer, OperatorConfigError,
};
use v2board_mail_adapters::smtp::SmtpTransportCache;

use crate::state::WorkerState;

fn main() -> anyhow::Result<()> {
    // Telemetry must initialize before the tokio runtime exists: the OTLP
    // batch exporter constructs a blocking HTTP client, which panics inside
    // an async context. The guard flushes both exports on drop.
    let production =
        v2board_config::RuntimeEnvironment::parse(std::env::var("V2BOARD_ENV").ok().as_deref())
            .is_ok_and(v2board_config::RuntimeEnvironment::is_production);
    let _telemetry =
        v2board_telemetry::init_tracing("v2board-worker", "v2board_workers=info", production);
    run()
}

#[tokio::main]
async fn run() -> anyhow::Result<()> {
    let bootstrap = AppConfig::try_from_worker_boot_env()?;
    let db = v2board_db::connect_postgres(&bootstrap.database_url).await?;
    if !v2board_db::migrations_current(&db).await? {
        anyhow::bail!(
            "refusing to start worker commands against a PostgreSQL schema that is not exactly current"
        );
    }
    let installation_id = v2board_db::installation_id(&db).await?;
    let authority =
        match operator_config::load_active(&db, installation_id, &bootstrap.app_key).await {
            Ok(Some(authority)) => authority,
            Ok(None) => return Err(OperatorConfigError::MissingAuthority.into()),
            Err(error) => {
                if let Some((observed_revision, error_code)) = error.observed_rejection() {
                    let _ = operator_config::acknowledge(
                        &db,
                        installation_id,
                        OperatorConfigConsumer::Worker,
                        observed_revision,
                        None,
                        Some(error_code),
                    )
                    .await;
                }
                return Err(error.into());
            }
        };
    let config = Arc::new(bootstrap.with_operator_config(&authority.values, authority.revision)?);
    operator_config::acknowledge(
        &db,
        installation_id,
        OperatorConfigConsumer::Worker,
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
    let state = WorkerState::new(
        config,
        db,
        installation_id,
        redis,
        SmtpTransportCache::default(),
    );
    let args = env::args().skip(1).collect::<Vec<_>>();
    if !args.is_empty() {
        return scheduler::run_command(&args, &state).await;
    }

    runtime::run(state).await
}
