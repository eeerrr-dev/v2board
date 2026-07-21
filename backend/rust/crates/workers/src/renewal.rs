use chrono::Utc;
use v2board_order_adapters::{renewal_run, runtime_renewal_service};

use crate::{batch::finish_item_batch, state::WorkerState};

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let outcome = runtime_renewal_service(state.db.clone())
        .run(renewal_run(Utc::now().timestamp()))
        .await?;
    for failure in &outcome.failures {
        tracing::warn!(
            user_id = failure.id,
            detail = failure.detail,
            "auto renewal failed"
        );
    }
    finish_item_batch(
        "auto renewals",
        usize::try_from(outcome.examined).unwrap_or(usize::MAX),
        outcome.failures.len(),
        outcome
            .failures
            .first()
            .map(|failure| failure.detail.clone()),
    )
}
