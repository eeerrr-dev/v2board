use chrono::{Datelike, TimeZone, Utc};
use v2board_application::worker_statistics::{StatisticsWindow, StatisticsWorkerService};
use v2board_config::{app_now, app_timezone};
use v2board_db::worker_statistics::PostgresStatisticsWorkerRepository;

use crate::{state::WorkerState, time::timestamp_before};

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let end_at = today_start_timestamp();
    let start_at = timestamp_before(end_at, 86_400);
    StatisticsWorkerService::new(PostgresStatisticsWorkerRepository::new(state.db.clone()))
        .run(
            StatisticsWindow { start_at, end_at },
            Utc::now().timestamp(),
        )
        .await?;
    Ok(())
}

fn today_start_timestamp() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}
