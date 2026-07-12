use chrono::Utc;
use sqlx::{MySql, QueryBuilder};

use crate::{state::WorkerState, time::timestamp_before};

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let ticket_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT t.id
        FROM v2_ticket t
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
        "#,
    )
    .bind(timestamp_before(now, 86_400))
    .fetch_all(&state.db)
    .await?;
    if ticket_ids.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_ticket SET status = 1, updated_at = ");
    builder.push_bind(now);
    builder.push(" WHERE id IN (");
    {
        let mut separated = builder.separated(", ");
        for id in ticket_ids {
            separated.push_bind(id);
        }
    }
    builder.push(")");
    builder.build().execute(&state.db).await?;
    Ok(())
}
