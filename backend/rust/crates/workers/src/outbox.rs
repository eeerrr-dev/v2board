use std::time::Duration;

use chrono::Utc;
use lettre::{AsyncTransport, Message, message::header::ContentType};
use sqlx::{FromRow, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;
use v2board_domain::smtp::SmtpSettings;

use crate::{batch::finish_item_batch, metrics::record_worker_metric, state::WorkerState};

pub(crate) const JOB_NAME: &str = "mail_outbox";
const MAIL_OUTBOX_BATCH_SIZE: i64 = 10;
const MAIL_OUTBOX_LEASE_SECS: i64 = 15 * 60;
const MAIL_OUTBOX_MAX_ATTEMPTS: i32 = 8;
const MAIL_OUTBOX_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MAIL_OUTBOX_ERROR_INTERVAL: Duration = Duration::from_secs(2);
const MAIL_OUTBOX_SMTP_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_MAIL_RETENTION_DAYS: u64 = 90;
const DEFAULT_IDEMPOTENCY_RETENTION_DAYS: u64 = 90;
const DEFAULT_CLEANUP_INTERVAL_SECONDS: u64 = 6 * 60 * 60;
const CLEANUP_BATCH_SIZE: i64 = 1_000;
const CLEANUP_MAX_BATCHES_PER_TABLE: usize = 10;
const MAIL_OUTBOX_CLAIM_SQL: &str = r#"
SELECT item.id, item.batch_key, batch.sender, batch.template_name, item.recipient,
       batch.subject, batch.body, item.message_id, item.attempt_count
FROM mail_outbox AS item
JOIN mail_outbox_batch AS batch ON batch.batch_key = item.batch_key
WHERE item.failed_at IS NULL
  AND item.available_at <= $1
  AND (item.lease_expires_at IS NULL OR item.lease_expires_at <= $2)
  AND item.attempt_count < $3
ORDER BY item.id
LIMIT $4
FOR UPDATE SKIP LOCKED
"#;
const MAIL_OUTBOX_ACK_SQL: &str = r#"
DELETE FROM mail_outbox
WHERE id = $1 AND lease_token = $2 AND failed_at IS NULL
"#;
const MAIL_OUTBOX_LOG_SQL: &str = r#"
INSERT INTO mail_log
    (email, subject, template_name, error, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6)
"#;
const MAIL_OUTBOX_FAILURE_SQL: &str = r#"
UPDATE mail_outbox
SET attempt_count = $1, available_at = $2, failed_at = $3, last_error = $4,
    lease_token = NULL, lease_expires_at = NULL, updated_at = $5
WHERE id = $6 AND lease_token = $7 AND failed_at IS NULL
"#;
const MAIL_OUTBOX_CLEAR_ENVELOPE_SQL: &str = r#"
UPDATE mail_outbox_batch AS batch
SET sender = NULL, subject = NULL, body = NULL, updated_at = $1
WHERE batch.batch_key = $2
  AND NOT EXISTS (
      SELECT 1
      FROM mail_outbox AS item
      WHERE item.batch_key = batch.batch_key
        AND item.failed_at IS NULL
  )
"#;
const MAIL_OUTBOX_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id
    FROM mail_outbox
    WHERE failed_at IS NOT NULL AND failed_at < $1
    ORDER BY failed_at, id
    LIMIT $2
)
DELETE FROM mail_outbox AS item
USING doomed
WHERE item.id = doomed.id
"#;
const MAIL_OUTBOX_BATCH_RETENTION_SQL: &str = r#"
WITH doomed AS (
  SELECT batch.batch_key
  FROM mail_outbox_batch AS batch
  WHERE batch.sender IS NULL
  AND batch.subject IS NULL
  AND batch.body IS NULL
  AND batch.updated_at < $1
  AND NOT EXISTS (
      SELECT 1 FROM mail_outbox AS item
      WHERE item.batch_key = batch.batch_key
  )
  ORDER BY batch.updated_at, batch.batch_key
  LIMIT $2
)
DELETE FROM mail_outbox_batch AS batch
USING doomed
WHERE batch.batch_key = doomed.batch_key
"#;
const MAIL_LOG_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id
    FROM mail_log
    WHERE created_at < $1
    ORDER BY created_at, id
    LIMIT $2
)
DELETE FROM mail_log AS log
USING doomed
WHERE log.id = doomed.id
"#;
const TRAFFIC_REPORT_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT report_key
    FROM server_traffic_report
    WHERE applied_at IS NOT NULL AND applied_at < $1
    ORDER BY applied_at, report_key
    LIMIT $2
)
DELETE FROM server_traffic_report AS report
USING doomed
WHERE report.report_key = doomed.report_key
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetentionConfig {
    mail_retention: Duration,
    idempotency_retention: Duration,
    cleanup_interval: Duration,
}

impl RetentionConfig {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            mail_retention: days(parse_bounded_env(
                "V2BOARD_MAIL_RETENTION_DAYS",
                DEFAULT_MAIL_RETENTION_DAYS,
                1,
                3_650,
            )?)?,
            idempotency_retention: days(parse_bounded_env(
                "V2BOARD_IDEMPOTENCY_RETENTION_DAYS",
                DEFAULT_IDEMPOTENCY_RETENTION_DAYS,
                1,
                3_650,
            )?)?,
            cleanup_interval: Duration::from_secs(parse_bounded_env(
                "V2BOARD_WORKER_CLEANUP_INTERVAL_SECONDS",
                DEFAULT_CLEANUP_INTERVAL_SECONDS,
                60,
                7 * 86_400,
            )?),
        })
    }
}

#[derive(Debug, Clone, FromRow)]
struct MailOutboxItem {
    id: i64,
    batch_key: String,
    sender: Option<String>,
    template_name: Option<String>,
    recipient: String,
    subject: Option<String>,
    body: Option<String>,
    message_id: String,
    attempt_count: i32,
}

#[derive(Debug)]
struct ClaimedMailBatch {
    lease_token: String,
    items: Vec<MailOutboxItem>,
}

pub(crate) async fn run_loop(
    state: WorkerState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let retention = RetentionConfig::from_env()?;
    let mut next_cleanup = tokio::time::Instant::now();
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= next_cleanup {
            let cleanup_delay = match cleanup_retained_state(&state, retention).await {
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
        let delay = match run_mail_outbox_batch(&state).await {
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

async fn cleanup_retained_state(
    state: &WorkerState,
    retention: RetentionConfig,
) -> anyhow::Result<u64> {
    let now = Utc::now().timestamp();
    let mail_cutoff = retention_cutoff(now, retention.mail_retention)?;
    let idempotency_cutoff = retention_cutoff(now, retention.idempotency_retention)?;
    let mut deleted = 0_u64;
    for (sql, cutoff) in [
        (MAIL_OUTBOX_RETENTION_SQL, mail_cutoff),
        (MAIL_OUTBOX_BATCH_RETENTION_SQL, mail_cutoff),
        (MAIL_LOG_RETENTION_SQL, mail_cutoff),
        (TRAFFIC_REPORT_RETENTION_SQL, idempotency_cutoff),
    ] {
        for _ in 0..CLEANUP_MAX_BATCHES_PER_TABLE {
            let affected = sqlx::query(sql)
                .bind(cutoff)
                .bind(CLEANUP_BATCH_SIZE)
                .execute(&state.db)
                .await?
                .rows_affected();
            deleted = deleted.saturating_add(affected);
            if affected < CLEANUP_BATCH_SIZE as u64 {
                break;
            }
        }
    }
    Ok(deleted)
}

fn retention_cutoff(now: i64, retention: Duration) -> anyhow::Result<i64> {
    let seconds = i64::try_from(retention.as_secs())
        .map_err(|_| anyhow::anyhow!("retention duration is too large"))?;
    now.checked_sub(seconds)
        .ok_or_else(|| anyhow::anyhow!("retention cutoff underflow"))
}

fn days(value: u64) -> anyhow::Result<Duration> {
    value
        .checked_mul(86_400)
        .map(Duration::from_secs)
        .ok_or_else(|| anyhow::anyhow!("retention days overflow"))
}

fn parse_bounded_env(name: &str, default: u64, minimum: u64, maximum: u64) -> anyhow::Result<u64> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{name} must be valid UTF-8"))?;
    parse_bounded_value(name, raw, minimum, maximum)
}

fn parse_bounded_value(name: &str, raw: &str, minimum: u64, maximum: u64) -> anyhow::Result<u64> {
    let value = raw
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        anyhow::bail!("{name} must be between {minimum} and {maximum}");
    }
    Ok(value)
}

async fn run_mail_outbox_batch(state: &WorkerState) -> anyhow::Result<usize> {
    // Authority health is a precondition for touching durable outbox state.
    // Claiming first would mutate lease/attempt metadata even though no SMTP
    // side effect is permitted while the active revision cannot be applied.
    let state = state.snapshot_config_for_job().await?;
    let Some(batch) = claim_mail_outbox_batch(&state).await? else {
        return Ok(0);
    };
    let total = batch.items.len();
    let mut failed = 0_usize;
    let mut first_error = None;
    for item in batch.items {
        match send_mail_outbox_item(&state, &item).await {
            Ok(()) => {
                if let Err(error) = mark_mail_outbox_sent(&state, &batch.lease_token, &item).await {
                    tracing::error!(
                        outbox_id = item.id,
                        message_id = item.message_id,
                        ?error,
                        "mail was accepted but its outbox acknowledgement failed"
                    );
                    failed += 1;
                    first_error.get_or_insert_with(|| error.to_string());
                }
            }
            Err(error) => {
                let error_message = error.to_string();
                tracing::warn!(
                    outbox_id = item.id,
                    recipient = item.recipient,
                    message_id = item.message_id,
                    attempt = item.attempt_count + 1,
                    ?error,
                    "mail outbox delivery failed"
                );
                if let Err(update_error) =
                    record_mail_outbox_failure(&state, &batch.lease_token, &item, &error_message)
                        .await
                {
                    tracing::error!(
                        outbox_id = item.id,
                        ?update_error,
                        "failed to persist mail outbox delivery error"
                    );
                }
                failed += 1;
                first_error.get_or_insert(error_message);
            }
        }
    }
    finish_item_batch("mail outbox items", total, failed, first_error)?;
    Ok(total)
}

async fn claim_mail_outbox_batch(state: &WorkerState) -> anyhow::Result<Option<ClaimedMailBatch>> {
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    let items = sqlx::query_as::<_, MailOutboxItem>(MAIL_OUTBOX_CLAIM_SQL)
        .bind(now)
        .bind(now)
        .bind(MAIL_OUTBOX_MAX_ATTEMPTS)
        .bind(MAIL_OUTBOX_BATCH_SIZE)
        .fetch_all(&mut *tx)
        .await?;
    if items.is_empty() {
        tx.commit().await?;
        return Ok(None);
    }

    let lease_token = Uuid::new_v4().to_string();
    let lease_expires_at = checked_mail_outbox_timestamp(now, MAIL_OUTBOX_LEASE_SECS)?;
    let mut builder = QueryBuilder::<Postgres>::new("UPDATE mail_outbox SET lease_token = ");
    builder
        .push_bind(&lease_token)
        .push(", lease_expires_at = ")
        .push_bind(lease_expires_at)
        .push(", updated_at = ")
        .push_bind(now)
        .push(" WHERE id IN (");
    let mut ids = builder.separated(", ");
    for item in &items {
        ids.push_bind(item.id);
    }
    ids.push_unseparated(")");
    let claimed = builder.build().execute(&mut *tx).await?;
    if claimed.rows_affected() != items.len() as u64 {
        anyhow::bail!("mail outbox claim changed an unexpected number of rows");
    }
    tx.commit().await?;
    Ok(Some(ClaimedMailBatch { lease_token, items }))
}

async fn send_mail_outbox_item(state: &WorkerState, item: &MailOutboxItem) -> anyhow::Result<()> {
    let settings = SmtpSettings::load(&state.config)?;
    let sender = item
        .sender
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("mail outbox batch sender is missing"))?;
    let subject = item
        .subject
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("mail outbox batch subject is missing"))?;
    let body = item
        .body
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("mail outbox batch body is missing"))?;
    let message = Message::builder()
        .from(sender.parse()?)
        .to(item.recipient.parse()?)
        .subject(subject)
        .message_id(Some(item.message_id.clone()))
        .header(ContentType::TEXT_HTML)
        .body(body.to_string())?;
    let transport = state.smtp.transport(&settings)?;
    let delivery = transport.send(message);
    tokio::time::timeout(MAIL_OUTBOX_SMTP_TIMEOUT, delivery)
        .await
        .map_err(|_| anyhow::anyhow!("SMTP delivery timed out"))??;
    Ok(())
}

async fn mark_mail_outbox_sent(
    state: &WorkerState,
    lease_token: &str,
    item: &MailOutboxItem,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    lock_mail_batch(&mut tx, &item.batch_key).await?;
    let deleted = sqlx::query(MAIL_OUTBOX_ACK_SQL)
        .bind(item.id)
        .bind(lease_token)
        .execute(&mut *tx)
        .await?;
    if deleted.rows_affected() != 1 {
        anyhow::bail!("mail outbox delivery lease was lost before acknowledgement");
    }
    insert_mail_outbox_log(&mut tx, item, None, now).await?;
    clear_completed_mail_batch_envelope(&mut tx, &item.batch_key, now).await?;
    tx.commit().await?;
    Ok(())
}

async fn record_mail_outbox_failure(
    state: &WorkerState,
    lease_token: &str,
    item: &MailOutboxItem,
    error: &str,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let attempt_count = item.attempt_count + 1;
    let terminal = attempt_count >= MAIL_OUTBOX_MAX_ATTEMPTS;
    let available_at =
        checked_mail_outbox_timestamp(now, mail_outbox_backoff_seconds(attempt_count))?;
    let last_error = error.chars().take(4096).collect::<String>();
    let failed_at = terminal.then_some(now);
    let mut tx = state.db.begin().await?;
    lock_mail_batch(&mut tx, &item.batch_key).await?;
    let updated = sqlx::query(MAIL_OUTBOX_FAILURE_SQL)
        .bind(attempt_count)
        .bind(available_at)
        .bind(failed_at)
        .bind(&last_error)
        .bind(now)
        .bind(item.id)
        .bind(lease_token)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() != 1 {
        anyhow::bail!("mail outbox delivery lease was lost before failure recording");
    }
    if terminal {
        insert_mail_outbox_log(&mut tx, item, Some(&last_error), now).await?;
        clear_completed_mail_batch_envelope(&mut tx, &item.batch_key, now).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn insert_mail_outbox_log(
    tx: &mut Transaction<'_, Postgres>,
    item: &MailOutboxItem,
    error: Option<&str>,
    now: i64,
) -> anyhow::Result<()> {
    let subject = truncate_mail_log_field(item.subject.as_deref(), "");
    let template_name =
        truncate_mail_log_field(item.template_name.as_deref(), "mail.default.notify");
    sqlx::query(MAIL_OUTBOX_LOG_SQL)
        .bind(&item.recipient)
        .bind(subject)
        .bind(template_name)
        .bind(error)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn truncate_mail_log_field(value: Option<&str>, fallback: &str) -> String {
    value.unwrap_or(fallback).chars().take(255).collect()
}

async fn lock_mail_batch(
    tx: &mut Transaction<'_, Postgres>,
    batch_key: &str,
) -> anyhow::Result<()> {
    let found = sqlx::query_scalar::<_, String>(
        "SELECT batch_key FROM mail_outbox_batch WHERE batch_key = $1 FOR UPDATE",
    )
    .bind(batch_key)
    .fetch_optional(&mut **tx)
    .await?;
    if found.is_none() {
        anyhow::bail!("mail outbox batch was lost");
    }
    Ok(())
}

async fn clear_completed_mail_batch_envelope(
    tx: &mut Transaction<'_, Postgres>,
    batch_key: &str,
    now: i64,
) -> anyhow::Result<()> {
    sqlx::query(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL)
        .bind(now)
        .bind(batch_key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn mail_outbox_backoff_seconds(attempt_count: i32) -> i64 {
    let exponent = u32::try_from(attempt_count.saturating_sub(1).min(10)).unwrap_or_default();
    (5_i64.saturating_mul(1_i64 << exponent)).min(60 * 60)
}

fn checked_mail_outbox_timestamp(now: i64, delay_seconds: i64) -> anyhow::Result<i64> {
    now.checked_add(delay_seconds)
        .ok_or_else(|| anyhow::anyhow!("mail outbox timestamp overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_outbox_claim_is_batched_leased_and_non_blocking() {
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("JOIN mail_outbox_batch"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("ORDER BY item.id"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("lease_expires_at"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("attempt_count < $3"));
    }

    #[test]
    fn operator_authority_is_checked_before_outbox_claim_mutates_a_lease() {
        let source = include_str!("outbox.rs");
        let start = source
            .find("async fn run_mail_outbox_batch")
            .expect("outbox batch runner");
        let body = &source[start..];
        let authority = body
            .find("snapshot_config_for_job().await?")
            .expect("authority precondition");
        let claim = body
            .find("claim_mail_outbox_batch(&state).await?")
            .expect("durable claim");
        assert!(authority < claim);
    }

    #[test]
    fn mail_outbox_terminal_cleanup_deletes_success_and_scrubs_envelope() {
        assert!(MAIL_OUTBOX_ACK_SQL.trim_start().starts_with("DELETE"));
        assert!(MAIL_OUTBOX_ACK_SQL.contains("lease_token = $2"));
        assert!(MAIL_OUTBOX_FAILURE_SQL.contains("lease_token = $7"));
        assert!(MAIL_OUTBOX_LOG_SQL.contains("INSERT INTO mail_log"));
        assert!(MAIL_OUTBOX_LOG_SQL.contains("template_name"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("sender = NULL"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("subject = NULL"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("body = NULL"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("NOT EXISTS"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("item.failed_at IS NULL"));
    }

    #[test]
    fn mail_outbox_log_fields_fit_the_legacy_schema() {
        assert_eq!(
            truncate_mail_log_field(None, "mail.default.notify"),
            "mail.default.notify"
        );
        assert_eq!(
            truncate_mail_log_field(Some(&"界".repeat(300)), "")
                .chars()
                .count(),
            255
        );
    }

    #[test]
    fn mail_outbox_retry_backoff_is_bounded() {
        assert_eq!(mail_outbox_backoff_seconds(1), 5);
        assert_eq!(mail_outbox_backoff_seconds(2), 10);
        assert!(mail_outbox_backoff_seconds(7) > mail_outbox_backoff_seconds(6));
        assert_eq!(mail_outbox_backoff_seconds(i32::MAX), 60 * 60);
    }

    #[test]
    fn mail_outbox_deadlines_reject_timestamp_overflow() {
        assert_eq!(
            checked_mail_outbox_timestamp(i64::MAX - 5, 5).unwrap(),
            i64::MAX
        );
        assert!(checked_mail_outbox_timestamp(i64::MAX, 1).is_err());
        assert_eq!(
            checked_mail_outbox_timestamp(i64::MIN, 1).unwrap(),
            i64::MIN + 1
        );
    }

    #[test]
    fn mail_outbox_migration_has_durable_identity_and_delivery_state() {
        let migration = include_str!("../../../migrations-postgres/0001_initial.sql");
        assert!(migration.contains("CREATE TABLE mail_outbox_batch"));
        assert!(migration.contains("CREATE TABLE mail_outbox"));
        assert!(migration.contains("uniq_mail_outbox_batch_recipient"));
        assert!(migration.contains("message_id VARCHAR(255) NOT NULL"));
        assert!(migration.contains("lease_expires_at BIGINT"));
        assert!(migration.contains("idx_mail_outbox_claim"));
        let batch = migration
            .split_once("CREATE TABLE mail_outbox_batch (")
            .and_then(|(_, rest)| rest.split_once("CREATE TABLE mail_outbox ("))
            .map(|(batch, _)| batch)
            .expect("mail outbox batch table must precede the recipient table");
        assert_eq!(batch.matches("body TEXT").count(), 1);
        assert_eq!(batch.matches("subject TEXT").count(), 1);
        assert_eq!(batch.matches("template_name VARCHAR(255)").count(), 1);
    }

    #[test]
    fn retention_cleanup_never_deletes_pending_work() {
        assert!(MAIL_OUTBOX_RETENTION_SQL.contains("failed_at IS NOT NULL"));
        assert!(MAIL_OUTBOX_BATCH_RETENTION_SQL.contains("NOT EXISTS"));
        assert!(TRAFFIC_REPORT_RETENTION_SQL.contains("applied_at IS NOT NULL"));
        assert!(MAIL_OUTBOX_RETENTION_SQL.contains("LIMIT $2"));
        assert!(MAIL_OUTBOX_BATCH_RETENTION_SQL.contains("LIMIT $2"));
        assert!(TRAFFIC_REPORT_RETENTION_SQL.contains("LIMIT $2"));
    }

    #[test]
    fn retention_configuration_is_strict_and_bounded() {
        assert_eq!(parse_bounded_value("test", "90", 1, 3_650).unwrap(), 90);
        assert!(parse_bounded_value("test", "0", 1, 3_650).is_err());
        assert!(parse_bounded_value("test", "3651", 1, 3_650).is_err());
        assert!(parse_bounded_value("test", "many", 1, 3_650).is_err());
        assert_eq!(
            retention_cutoff(100_000, Duration::from_secs(1)).unwrap(),
            99_999
        );
        assert!(retention_cutoff(i64::MIN, Duration::from_secs(1)).is_err());
    }
}
