//! v2board background worker.
//!
//! Scheduler side-effects (traffic accounting, order settlement, commission
//! payout, reminder mail, log/traffic resets, statistics) run in independent
//! Tokio/cron loops guarded by renewable distributed Redis leases. The same
//! runtime also drains the durable SQL mail outbox used by bulk, transactional,
//! and scheduled reminder mail; no second queue framework or worker runtime is
//! introduced.

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
use v2board_domain::smtp::SmtpTransportCache;

use crate::state::WorkerState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    runtime::init_tracing();

    let config = Arc::new(AppConfig::from_env());
    let db = v2board_db::connect_mysql(&config.database_url).await?;
    let redis = redis::Client::open(config.redis_url.clone())?;
    let state = WorkerState::new(config, db, redis, SmtpTransportCache::default());
    let args = env::args().skip(1).collect::<Vec<_>>();
    if !args.is_empty() {
        return scheduler::run_command(&args, &state).await;
    }

    runtime::run(state).await
}
