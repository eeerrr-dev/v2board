use chrono::Utc;
use v2board_order_adapters::{commission_run, runtime_commission_service};

use crate::state::WorkerState;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let command = commission_run(&state.config, Utc::now().timestamp());
    let outcome = runtime_commission_service(state.db.clone())
        .run(&command)
        .await?;
    for failure in outcome.failures {
        tracing::error!(
            order_id = failure.id,
            trade_no = failure.reference,
            detail = failure.detail,
            "commission payment failed"
        );
    }
    Ok(())
}
