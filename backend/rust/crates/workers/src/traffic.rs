use v2board_application::worker_traffic::{TrafficDrainPolicy, TrafficWorkerService};
use v2board_db::worker_traffic::PostgresTrafficAccountingRepository;

use crate::{
    state::WorkerState,
    traffic_adapters::{RedisTrafficResetBarrier, SystemWorkerClock, TRAFFIC_RESET_LOCK_KEY},
};

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let service = TrafficWorkerService::new(
        PostgresTrafficAccountingRepository::new(state.db.clone()),
        RedisTrafficResetBarrier::new(state.redis.clone(), state.redis_key(TRAFFIC_RESET_LOCK_KEY)),
        SystemWorkerClock::default(),
        TrafficDrainPolicy::default(),
    );
    let outcome = service.run(&state.installation_id.to_string()).await?;
    if outcome.stale_items > 0 {
        tracing::info!(
            stale_items = outcome.stale_items,
            "discarded traffic from an earlier quota epoch"
        );
    }
    if outcome.missing_users > 0 {
        tracing::warn!(
            missing_users = outcome.missing_users,
            "recorded traffic items whose user no longer exists"
        );
    }
    if outcome.exhausted {
        tracing::warn!(
            processed = outcome.processed,
            "traffic drain reached its per-tick safety budget"
        );
    }
    Ok(())
}
