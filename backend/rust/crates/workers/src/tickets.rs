use chrono::Utc;

use crate::{state::WorkerState, time::timestamp_before};

const AUTO_CLOSE_BATCH_SIZE: i64 = 1_000;
const AUTO_CLOSE_MAX_BATCHES: usize = 20;
const AUTO_CLOSE_TICKETS_SQL: &str = r#"
    WITH candidates AS (
        SELECT t.id
        FROM ticket AS t
        WHERE t.status = 0
          AND t.updated_at <= $2
          AND t.reply_status = 1
          AND COALESCE((
              SELECT tm.user_id
              FROM ticket_message tm
              WHERE tm.ticket_id = t.id
              ORDER BY tm.id DESC
              LIMIT 1
          ), 0) <> t.user_id
        ORDER BY t.updated_at, t.id
        LIMIT $3
        FOR UPDATE SKIP LOCKED
    )
    UPDATE ticket AS t
    SET status = 1, updated_at = $1
    FROM candidates
    WHERE t.id = candidates.id AND t.status = 0
"#;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let cutoff = timestamp_before(now, 86_400);
    for _ in 0..AUTO_CLOSE_MAX_BATCHES {
        let closed = sqlx::query(AUTO_CLOSE_TICKETS_SQL)
            .bind(now)
            .bind(cutoff)
            .bind(AUTO_CLOSE_BATCH_SIZE)
            .execute(&state.db)
            .await?
            .rows_affected();
        if closed < AUTO_CLOSE_BATCH_SIZE as u64 {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AUTO_CLOSE_TICKETS_SQL;

    #[test]
    fn auto_close_rechecks_the_complete_state_in_the_update_statement() {
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("WITH candidates AS"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.status = 0"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.updated_at <= $2"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.reply_status = 1"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("tm.ticket_id = t.id"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("LIMIT $3"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("FOR UPDATE SKIP LOCKED"));
    }
}
