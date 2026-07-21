use v2board_order_adapters::runtime_order_service;

use crate::{batch::finish_item_batch, state::WorkerState};

const ORDER_CANDIDATE_PAGE_SIZE: i64 = 250;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let service = runtime_order_service(state.db.clone(), state.config.clone());
    let mut after_id = 0_i64;
    let mut total = 0_usize;
    let mut failed = 0_usize;
    let mut first_error = None;

    loop {
        let candidates = service
            .pending_candidates(after_id, ORDER_CANDIDATE_PAGE_SIZE)
            .await
            .map_err(anyhow::Error::new)?;
        let Some(last_id) = candidates.last().map(|candidate| candidate.id) else {
            break;
        };
        for candidate in candidates {
            let trade_no = candidate.trade_no;
            total += 1;
            if let Err(error) = handle_order(state, &trade_no).await {
                tracing::error!(trade_no, ?error, "order handle failed");
                failed += 1;
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
        after_id = last_id;
    }
    finish_item_batch("orders", total, failed, first_error)
}

async fn handle_order(state: &WorkerState, trade_no: &str) -> anyhow::Result<()> {
    runtime_order_service(state.db.clone(), state.config.clone())
        .handle_pending_order(trade_no)
        .await
        .map_err(anyhow::Error::new)
}
