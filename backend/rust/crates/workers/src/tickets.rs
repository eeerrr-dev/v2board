use chrono::Utc;
use v2board_application::ticket::TicketMaintenance;
use v2board_db::ticket::PostgresTicketRepository;

use crate::state::WorkerState;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    TicketMaintenance::new(PostgresTicketRepository::new(state.db.clone()))
        .auto_close_answered(Utc::now().timestamp())
        .await?;
    Ok(())
}
