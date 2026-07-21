use std::time::Duration;

use v2board_config::app_now;
use v2board_mail_adapters::worker::{WorkerRetentionConfig, mail_outbox_service};

use crate::{metrics::record_worker_metric, state::WorkerState};

pub(crate) const JOB_NAME: &str = "mail_outbox";
const MAIL_OUTBOX_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MAIL_OUTBOX_ERROR_INTERVAL: Duration = Duration::from_secs(2);

/// Long-running scheduler/metrics/cancellation adapter. Durable state changes,
/// delivery policy, and SMTP are delegated through the application service.
pub(crate) async fn run_loop(
    state: WorkerState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let retention = WorkerRetentionConfig::from_env()?;
    let mut next_cleanup = tokio::time::Instant::now();
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= next_cleanup {
            let service =
                mail_outbox_service(state.db.clone(), state.config.clone(), state.smtp.clone());
            let cleanup_delay = match service
                .cleanup(app_now().timestamp(), retention.policy)
                .await
            {
                Ok(deleted) if deleted > 0 => {
                    tracing::info!(deleted, "cleaned retained worker state");
                    retention.cleanup_interval
                }
                Ok(_) => retention.cleanup_interval,
                Err(error) => {
                    tracing::warn!(?error, "worker state retention cleanup failed");
                    MAIL_OUTBOX_ERROR_INTERVAL
                }
            };
            next_cleanup = tokio::time::Instant::now() + cleanup_delay;
        }

        // Authority health is a precondition for claiming durable work. A
        // refreshed snapshot is bound to the delivery adapter for this batch.
        let result = async {
            let batch_state = state.snapshot_config_for_job().await?;
            mail_outbox_service(
                batch_state.db.clone(),
                batch_state.config.clone(),
                batch_state.smtp.clone(),
            )
            .deliver_batch(app_now().timestamp())
            .await
            .map_err(anyhow::Error::from)
        }
        .await;
        let delay = match result {
            Ok(0) => MAIL_OUTBOX_POLL_INTERVAL,
            Ok(count) => {
                if let Err(error) = record_worker_metric(&state, JOB_NAME, true).await {
                    tracing::warn!(
                        job = JOB_NAME,
                        ?error,
                        "failed to record mail outbox metric"
                    );
                }
                tracing::info!(count, "mail outbox batch delivered");
                Duration::ZERO
            }
            Err(error) => {
                tracing::error!(job = JOB_NAME, ?error, "mail outbox batch failed");
                let _ = record_worker_metric(&state, JOB_NAME, false).await;
                MAIL_OUTBOX_ERROR_INTERVAL
            }
        };
        if delay.is_zero() {
            tokio::task::yield_now().await;
            continue;
        }
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
        }
    }
}
