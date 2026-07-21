use v2board_config::app_now;
use v2board_mail_adapters::worker::reminder_service;

use crate::state::WorkerState;

/// Scheduler inbound adapter. Paging, idempotency, eligibility SQL, rendering,
/// and durable enqueueing live behind the application mail use case and ports.
pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let now = app_now();
    let business_day = now.date_naive().to_string();
    let report = reminder_service(state.db.clone(), state.config.clone())
        .run(now.timestamp(), &business_day)
        .await?;
    for failure in &report.preparation_failures {
        tracing::warn!(
            kind = ?failure.kind,
            detail = failure.detail,
            "reminder mail configuration is invalid"
        );
    }
    tracing::info!(
        enqueued = report.enqueued,
        existing = report.existing,
        skipped = report.skipped,
        %business_day,
        "durable reminder occurrences prepared"
    );
    Ok(())
}
