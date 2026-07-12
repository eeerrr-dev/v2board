use chrono::Utc;

use crate::{state::WorkerState, time::timestamp_before};

const AUTO_CLOSE_TICKETS_SQL: &str = r#"
    UPDATE v2_ticket AS t
    SET t.status = 1, t.updated_at = ?
    WHERE t.status = 0
      AND t.updated_at <= ?
      AND t.reply_status = 1
      AND COALESCE((
          SELECT tm.user_id
          FROM v2_ticket_message tm
          WHERE tm.ticket_id = t.id
          ORDER BY tm.id DESC
          LIMIT 1
      ), 0) <> t.user_id
"#;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(AUTO_CLOSE_TICKETS_SQL)
        .bind(now)
        .bind(timestamp_before(now, 86_400))
        .execute(&state.db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AUTO_CLOSE_TICKETS_SQL;

    #[test]
    fn auto_close_rechecks_the_complete_state_in_the_update_statement() {
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.status = 0"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.updated_at <= ?"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.reply_status = 1"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("tm.ticket_id = t.id"));
        assert!(!AUTO_CLOSE_TICKETS_SQL.contains("WHERE id IN"));
    }
}
