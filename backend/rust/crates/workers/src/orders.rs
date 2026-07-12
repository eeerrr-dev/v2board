use v2board_domain::order::OrderService;

use crate::{batch::finish_item_batch, state::WorkerState};

const ORDER_CANDIDATE_PAGE_SIZE: i64 = 250;
const ORDER_CANDIDATE_SQL: &str = r#"
SELECT id, trade_no
FROM v2_order
WHERE status IN (0, 1) AND id > $1
ORDER BY id
LIMIT $2
"#;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let mut after_id = 0_i64;
    let mut total = 0_usize;
    let mut failed = 0_usize;
    let mut first_error = None;

    loop {
        let candidates = sqlx::query_as::<_, (i64, String)>(ORDER_CANDIDATE_SQL)
            .bind(after_id)
            .bind(ORDER_CANDIDATE_PAGE_SIZE)
            .fetch_all(&state.db)
            .await?;
        let Some(last_id) = candidates.last().map(|(id, _)| *id) else {
            break;
        };
        for (_, trade_no) in candidates {
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
    OrderService::new(state.db.clone(), state.config.clone())
        .handle_pending_order(trade_no)
        .await
        .map_err(anyhow::Error::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfinished_orders_are_scanned_with_a_bounded_primary_key_cursor() {
        assert!(ORDER_CANDIDATE_SQL.contains("id > $1"));
        assert!(ORDER_CANDIDATE_SQL.contains("ORDER BY id"));
        assert!(ORDER_CANDIDATE_SQL.contains("LIMIT $2"));
        assert_eq!(ORDER_CANDIDATE_PAGE_SIZE, 250);
    }
}
