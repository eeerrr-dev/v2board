use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;
use v2board_analytics::{
    BatchProjectionError, ClaimedBatch, ClickHouseMigrationError, OutboxError,
    bind_clickhouse_installation, claim_delivery_batch, clickhouse_client, mark_batch_published,
    outbox_backlog, project_or_verify_batch, quarantine_batch, release_batch_for_retry,
    verify_clickhouse_runtime_ready,
};

use crate::{metrics::record_worker_metric, state::WorkerState};

pub(crate) const JOB_NAME: &str = "analytics_outbox";

const MAX_BATCH_ROWS: i64 = 10_000;
const LEASE_SECONDS: i64 = 300;
const IDLE_INTERVAL: Duration = Duration::from_secs(1);
const BACKLOG_OBSERVATION_INTERVAL: Duration = Duration::from_secs(300);
const DATABASE_OPERATION_TIMEOUT: Duration = Duration::from_secs(15);
const SCHEMA_READINESS_TIMEOUT: Duration = Duration::from_secs(60);
// `project_or_verify_batch` caps each insert's final acknowledgement at 90s.
// Keep the complete projection/verification budget below the PostgreSQL lease
// so a second relay cannot reclaim a batch while this relay still writes it.
const CLICKHOUSE_OPERATION_TIMEOUT: Duration = Duration::from_secs(240);
const RETRY_BASE_DELAY: Duration = Duration::from_secs(1);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionFailureDisposition {
    Retry,
    Quarantine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutboxFailureSeverity {
    Transient,
    Integrity,
}

#[derive(Debug)]
struct RetryBackoff {
    consecutive_failures: u32,
    lease_owner: Uuid,
}

impl RetryBackoff {
    fn new(lease_owner: Uuid) -> Self {
        Self {
            consecutive_failures: 0,
            lease_owner,
        }
    }

    fn reset(&mut self) {
        self.consecutive_failures = 0;
    }

    fn next_delay(&mut self) -> Duration {
        let attempt = self.consecutive_failures;
        let shift = attempt.min(6);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let nominal = RETRY_BASE_DELAY
            .saturating_mul(1_u32 << shift)
            .min(RETRY_MAX_DELAY);
        jittered_delay(self.lease_owner, attempt, nominal)
    }
}

pub(crate) async fn run_loop(
    state: WorkerState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let writer = state
        .config
        .clickhouse_writer
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("worker runtime config is missing ClickHouse writer"))?;
    let client = clickhouse_client(
        &writer.url,
        &writer.database,
        &writer.username,
        writer.password.as_deref(),
    );
    let lease_owner = Uuid::new_v4();
    let mut backoff = RetryBackoff::new(lease_owner);
    let mut next_backlog_observation = tokio::time::Instant::now();
    let mut schema_ready = false;

    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= next_backlog_observation {
            observe_backlog_best_effort(&state);
            next_backlog_observation = tokio::time::Instant::now() + BACKLOG_OBSERVATION_INTERVAL;
        }
        if !schema_ready {
            match tokio::time::timeout(
                SCHEMA_READINESS_TIMEOUT,
                bind_clickhouse_installation(
                    &client,
                    state.installation_id,
                    Utc::now().timestamp(),
                ),
            )
            .await
            {
                Ok(Ok(())) => {
                    tracing::info!(
                        job = JOB_NAME,
                        "ClickHouse schema and installation binding ready"
                    );
                    schema_ready = true;
                    backoff.reset();
                }
                Ok(Err(error)) => {
                    log_schema_readiness_error("startup/bind", &error);
                    record_metric_best_effort(&state, false);
                    if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                        return Ok(());
                    }
                    continue;
                }
                Err(_) => {
                    tracing::warn!(
                        job = JOB_NAME,
                        "ClickHouse schema and installation binding check timed out"
                    );
                    record_metric_best_effort(&state, false);
                    if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                        return Ok(());
                    }
                    continue;
                }
            }
        }
        let claim = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
                continue;
            }
            result = tokio::time::timeout(
                DATABASE_OPERATION_TIMEOUT,
                claim_delivery_batch(
                    &state.db,
                    lease_owner,
                    Utc::now().timestamp(),
                    LEASE_SECONDS,
                    MAX_BATCH_ROWS,
                ),
            ) => result,
        };

        let batch = match claim {
            Ok(Ok(Some(batch))) => batch,
            Ok(Ok(None)) => {
                backoff.reset();
                if wait_or_shutdown(&mut shutdown, IDLE_INTERVAL).await {
                    return Ok(());
                }
                continue;
            }
            Ok(Err(error)) => {
                log_outbox_error("claim", None, &error);
                if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                    return Ok(());
                }
                continue;
            }
            Err(_) => {
                tracing::warn!(job = JOB_NAME, "analytics PostgreSQL batch claim timed out");
                if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                    return Ok(());
                }
                continue;
            }
        };

        match tokio::time::timeout(
            SCHEMA_READINESS_TIMEOUT,
            verify_clickhouse_runtime_ready(&client, state.installation_id),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log_schema_readiness_error("pre-projection", &error);
                record_metric_best_effort(&state, false);
                release_for_retry(
                    &state,
                    &batch,
                    "ClickHouse schema or installation binding is not ready",
                )
                .await;
                schema_ready = false;
                if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                    return Ok(());
                }
                continue;
            }
            Err(_) => {
                tracing::warn!(
                    job = JOB_NAME,
                    batch_id = %batch.batch_id,
                    "ClickHouse pre-projection readiness check timed out"
                );
                record_metric_best_effort(&state, false);
                release_for_retry(
                    &state,
                    &batch,
                    "ClickHouse pre-projection readiness check timed out",
                )
                .await;
                schema_ready = false;
                if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                    return Ok(());
                }
                continue;
            }
        }

        let projected = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    release_for_retry(
                        &state,
                        &batch,
                        "analytics worker shut down before projection completed",
                    ).await;
                    return Ok(());
                }
                continue;
            }
            result = tokio::time::timeout(
                CLICKHOUSE_OPERATION_TIMEOUT,
                project_or_verify_batch(&client, &batch, state.installation_id),
            ) => result,
        };

        match projected {
            Ok(Ok(status)) => {
                let published_at = Utc::now().timestamp();
                match tokio::time::timeout(
                    DATABASE_OPERATION_TIMEOUT,
                    mark_batch_published(&state.db, &batch, published_at),
                )
                .await
                {
                    Ok(Ok(())) => {
                        tracing::info!(
                            job = JOB_NAME,
                            batch_id = %batch.batch_id,
                            row_count = batch.rows.len(),
                            ?status,
                            "analytics batch published"
                        );
                        record_metric_best_effort(&state, true);
                        backoff.reset();
                    }
                    Ok(Err(error)) => {
                        log_outbox_error("mark published", Some(batch.batch_id), &error);
                        record_metric_best_effort(&state, false);
                        match &error {
                            OutboxError::Database(_) => {
                                release_for_retry(&state, &batch, &error.to_string()).await;
                            }
                            OutboxError::ManifestConflict { .. } => {
                                quarantine_integrity_failure(&state, &batch, &error.to_string())
                                    .await;
                            }
                            // A lease may legitimately have been reclaimed or
                            // acknowledged by another relay. Do not overwrite
                            // its owner/state with a stale handle.
                            OutboxError::LeaseLost { .. }
                            | OutboxError::EventConflict { .. }
                            | OutboxError::InvalidBatchSize
                            | OutboxError::InvalidLease
                            | OutboxError::InvalidPartitionMonth
                            | OutboxError::RowCountOverflow
                            | OutboxError::InvalidPruneLimit => {}
                        }
                        if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                            return Ok(());
                        }
                    }
                    Err(_) => {
                        tracing::error!(
                            job = JOB_NAME,
                            batch_id = %batch.batch_id,
                            "analytics published-state transaction timed out"
                        );
                        record_metric_best_effort(&state, false);
                        release_for_retry(
                            &state,
                            &batch,
                            "analytics published-state transaction timed out",
                        )
                        .await;
                        if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                            return Ok(());
                        }
                    }
                }
            }
            Ok(Err(error)) => match projection_failure_disposition(&error) {
                ProjectionFailureDisposition::Retry => {
                    tracing::warn!(
                        job = JOB_NAME,
                        batch_id = %batch.batch_id,
                        ?error,
                        "analytics ClickHouse projection failed; releasing batch for retry"
                    );
                    record_metric_best_effort(&state, false);
                    release_for_retry(&state, &batch, &error.to_string()).await;
                    if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                        return Ok(());
                    }
                }
                ProjectionFailureDisposition::Quarantine => {
                    tracing::error!(
                        job = JOB_NAME,
                        analytics_integrity_incident = true,
                        batch_id = %batch.batch_id,
                        ?error,
                        "analytics batch failed immutable projection validation"
                    );
                    record_metric_best_effort(&state, false);
                    quarantine_integrity_failure(&state, &batch, &error.to_string()).await;
                    backoff.reset();
                }
            },
            Err(_) => {
                tracing::warn!(
                    job = JOB_NAME,
                    batch_id = %batch.batch_id,
                    "analytics ClickHouse projection timed out; releasing batch for retry"
                );
                record_metric_best_effort(&state, false);
                release_for_retry(&state, &batch, "analytics ClickHouse projection timed out")
                    .await;
                if wait_or_shutdown(&mut shutdown, backoff.next_delay()).await {
                    return Ok(());
                }
            }
        }
    }
}

fn jittered_delay(lease_owner: Uuid, attempt: u32, nominal: Duration) -> Duration {
    // Deterministic 80%-100% jitter spreads replicas after a shared ClickHouse
    // outage while preserving the exponential upper bound. The process-unique
    // lease owner and failure attempt make retries stable without another RNG.
    let nominal_millis = u64::try_from(nominal.as_millis()).unwrap_or(u64::MAX);
    let lower_millis = nominal_millis.saturating_mul(4) / 5;
    let span = nominal_millis.saturating_sub(lower_millis);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in lease_owner.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in attempt.to_be_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Duration::from_millis(lower_millis.saturating_add(hash % span.saturating_add(1)))
        .min(RETRY_MAX_DELAY)
}

async fn release_for_retry(state: &WorkerState, batch: &ClaimedBatch, reason: &str) {
    match tokio::time::timeout(
        DATABASE_OPERATION_TIMEOUT,
        release_batch_for_retry(&state.db, batch, reason),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log_outbox_error("release for retry", Some(batch.batch_id), &error),
        Err(_) => tracing::error!(
            job = JOB_NAME,
            batch_id = %batch.batch_id,
            "analytics batch release timed out; its lease must expire before recovery"
        ),
    }
}

async fn quarantine_integrity_failure(state: &WorkerState, batch: &ClaimedBatch, reason: &str) {
    match tokio::time::timeout(
        DATABASE_OPERATION_TIMEOUT,
        quarantine_batch(&state.db, batch, Utc::now().timestamp(), reason),
    )
    .await
    {
        Ok(Ok(())) => tracing::error!(
            job = JOB_NAME,
            analytics_integrity_incident = true,
            batch_id = %batch.batch_id,
            "analytics batch quarantined"
        ),
        Ok(Err(error)) => log_outbox_error("quarantine", Some(batch.batch_id), &error),
        Err(_) => tracing::error!(
            job = JOB_NAME,
            analytics_integrity_incident = true,
            batch_id = %batch.batch_id,
            "analytics quarantine transaction timed out"
        ),
    }
}

fn projection_failure_disposition(error: &BatchProjectionError) -> ProjectionFailureDisposition {
    match error {
        BatchProjectionError::ClickHouse(_) => ProjectionFailureDisposition::Retry,
        BatchProjectionError::Event(_)
        | BatchProjectionError::UnsupportedEvent { .. }
        | BatchProjectionError::PayloadConflict { .. }
        | BatchProjectionError::ProjectionConflict { .. }
        | BatchProjectionError::InstallationConflict { .. }
        | BatchProjectionError::InvalidField { .. } => ProjectionFailureDisposition::Quarantine,
    }
}

fn log_schema_readiness_error(operation: &str, error: &ClickHouseMigrationError) {
    match error {
        ClickHouseMigrationError::ClickHouse(_) => tracing::warn!(
            job = JOB_NAME,
            operation,
            ?error,
            "ClickHouse runtime readiness is temporarily unavailable"
        ),
        _ => tracing::error!(
            job = JOB_NAME,
            operation,
            analytics_integrity_incident = true,
            ?error,
            "ClickHouse schema lineage or installation binding is unsafe"
        ),
    }
}

fn outbox_failure_severity(error: &OutboxError) -> OutboxFailureSeverity {
    match error {
        OutboxError::Database(_) | OutboxError::LeaseLost { .. } => {
            OutboxFailureSeverity::Transient
        }
        OutboxError::EventConflict { .. }
        | OutboxError::InvalidBatchSize
        | OutboxError::InvalidLease
        | OutboxError::InvalidPartitionMonth
        | OutboxError::ManifestConflict { .. }
        | OutboxError::RowCountOverflow
        | OutboxError::InvalidPruneLimit => OutboxFailureSeverity::Integrity,
    }
}

fn record_metric_best_effort(state: &WorkerState, success: bool) {
    let state = state.clone();
    drop(tokio::spawn(async move {
        if let Err(error) = record_worker_metric(&state, JOB_NAME, success).await {
            tracing::warn!(
                job = JOB_NAME,
                ?error,
                "failed to record analytics relay metric"
            );
        }
    }));
}

fn observe_backlog_best_effort(state: &WorkerState) {
    let state = state.clone();
    drop(tokio::spawn(async move {
        match tokio::time::timeout(DATABASE_OPERATION_TIMEOUT, outbox_backlog(&state.db)).await {
            Ok(Ok(backlog)) => {
                let oldest_age_seconds = backlog
                    .oldest_pending_created_at
                    .map(|created_at| Utc::now().timestamp().saturating_sub(created_at));
                tracing::info!(
                    job = JOB_NAME,
                    pending_rows = backlog.pending_rows,
                    ?oldest_age_seconds,
                    "analytics PostgreSQL outbox backlog"
                );
            }
            Ok(Err(error)) => log_outbox_error("observe backlog", None, &error),
            Err(_) => tracing::warn!(
                job = JOB_NAME,
                "analytics PostgreSQL backlog observation timed out"
            ),
        }
    }));
}

fn log_outbox_error(operation: &str, batch_id: Option<Uuid>, error: &OutboxError) {
    match outbox_failure_severity(error) {
        OutboxFailureSeverity::Transient => tracing::warn!(
            job = JOB_NAME,
            operation,
            ?batch_id,
            ?error,
            "analytics PostgreSQL operation failed"
        ),
        OutboxFailureSeverity::Integrity => tracing::error!(
            job = JOB_NAME,
            analytics_integrity_incident = true,
            operation,
            ?batch_id,
            ?error,
            "analytics outbox lease or manifest invariant failed"
        ),
    }
}

async fn wait_or_shutdown(
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
    delay: Duration,
) -> bool {
    if *shutdown.borrow() {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_is_exponential_and_bounded() {
        let mut backoff = RetryBackoff::new(Uuid::from_u128(1));
        let delays = (0..9)
            .map(|_| backoff.next_delay().as_millis())
            .collect::<Vec<_>>();
        for (delay, upper) in delays.into_iter().zip([
            1_000_u128, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000, 60_000,
        ]) {
            assert!((upper * 4 / 5..=upper).contains(&delay));
        }
        backoff.reset();
        assert!((800..=1_000).contains(&backoff.next_delay().as_millis()));
        assert_ne!(
            jittered_delay(Uuid::from_u128(1), 3, Duration::from_secs(8)),
            jittered_delay(Uuid::from_u128(2), 3, Duration::from_secs(8))
        );
    }

    #[test]
    fn projection_integrity_failures_are_never_retried() {
        let batch_id = Uuid::nil();
        for error in [
            BatchProjectionError::UnsupportedEvent {
                batch_id,
                event_name: "future.event.v2".to_owned(),
            },
            BatchProjectionError::PayloadConflict { batch_id },
            BatchProjectionError::ProjectionConflict { batch_id },
            BatchProjectionError::InstallationConflict { batch_id },
            BatchProjectionError::InvalidField { field: "user_id" },
        ] {
            assert_eq!(
                projection_failure_disposition(&error),
                ProjectionFailureDisposition::Quarantine
            );
        }
    }

    #[test]
    fn relay_limits_fit_the_analytics_outbox_contract() {
        assert_eq!(MAX_BATCH_ROWS, 10_000);
        assert!((1..=3_600).contains(&LEASE_SECONDS));
        let projection = include_str!("../../analytics/src/projection.rs");
        assert_eq!(
            projection
                .matches(
                    ".with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)))"
                )
                .count(),
            2
        );
        assert!(CLICKHOUSE_OPERATION_TIMEOUT.as_secs() < LEASE_SECONDS as u64);
        assert!(90 < CLICKHOUSE_OPERATION_TIMEOUT.as_secs());
        assert!(BACKLOG_OBSERVATION_INTERVAL >= Duration::from_secs(60));
    }
}
