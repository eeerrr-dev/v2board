use v2board_domain::order::OrderService;

use crate::{batch::finish_item_batch, state::WorkerState};

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let trade_nos = sqlx::query_scalar::<_, String>(
        "SELECT trade_no FROM v2_order WHERE status IN (0, 1) ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await?;
    let total = trade_nos.len();
    let mut failed = 0_usize;
    let mut first_error = None;
    for trade_no in trade_nos {
        if let Err(error) = handle_order(state, &trade_no).await {
            tracing::error!(trade_no, ?error, "order handle failed");
            failed += 1;
            first_error.get_or_insert_with(|| error.to_string());
        }
    }
    finish_item_batch("orders", total, failed, first_error)
}

async fn handle_order(state: &WorkerState, trade_no: &str) -> anyhow::Result<()> {
    OrderService::new(state.db.clone(), state.config.clone())
        .handle_pending_order(trade_no)
        .await
        .map_err(anyhow::Error::new)
}
