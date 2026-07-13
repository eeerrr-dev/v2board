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

use std::{env, sync::Arc};

use v2board_config::AppConfig;
use v2board_domain::{
    operator_config::{self, OperatorConfigConsumer, OperatorConfigError},
    smtp::SmtpTransportCache,
};

use crate::state::WorkerState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    runtime::init_tracing();

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
