use std::time::Duration;

use chrono::Utc;
use lettre::{AsyncTransport, Message, message::header::ContentType};
use sqlx::{FromRow, MySql, QueryBuilder, Transaction};
use uuid::Uuid;
use v2board_domain::smtp::SmtpSettings;

use crate::{batch::finish_item_batch, metrics::record_worker_metric, state::WorkerState};

pub(crate) const JOB_NAME: &str = "mail_outbox";
const MAIL_OUTBOX_BATCH_SIZE: i64 = 10;
const MAIL_OUTBOX_LEASE_SECS: i64 = 15 * 60;
const MAIL_OUTBOX_MAX_ATTEMPTS: u32 = 8;
const MAIL_OUTBOX_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MAIL_OUTBOX_ERROR_INTERVAL: Duration = Duration::from_secs(2);
const MAIL_OUTBOX_SMTP_TIMEOUT: Duration = Duration::from_secs(60);
const MAIL_OUTBOX_CLAIM_SQL: &str = r#"
SELECT item.id, item.batch_key, batch.sender, batch.template_name, item.recipient,
       batch.subject, batch.body, item.message_id, item.attempt_count
FROM v2_mail_outbox AS item
JOIN v2_mail_outbox_batch AS batch ON batch.batch_key = item.batch_key
WHERE item.failed_at IS NULL
  AND item.available_at <= ?
  AND (item.lease_expires_at IS NULL OR item.lease_expires_at <= ?)
  AND item.attempt_count < ?
ORDER BY item.id
LIMIT ?
FOR UPDATE SKIP LOCKED
"#;
const MAIL_OUTBOX_ACK_SQL: &str = r#"
DELETE FROM v2_mail_outbox
WHERE id = ? AND lease_token = ? AND failed_at IS NULL
"#;
const MAIL_OUTBOX_LOG_SQL: &str = r#"
INSERT INTO v2_mail_log
    (email, subject, template_name, error, created_at, updated_at)
VALUES (?, ?, ?, ?, ?, ?)
"#;
const MAIL_OUTBOX_FAILURE_SQL: &str = r#"
UPDATE v2_mail_outbox
SET attempt_count = ?, available_at = ?, failed_at = ?, last_error = ?,
    lease_token = NULL, lease_expires_at = NULL, updated_at = ?
WHERE id = ? AND lease_token = ? AND failed_at IS NULL
"#;
const MAIL_OUTBOX_CLEAR_ENVELOPE_SQL: &str = r#"
UPDATE v2_mail_outbox_batch AS batch
SET sender = NULL, subject = NULL, body = NULL, updated_at = ?
WHERE batch.batch_key = ?
  AND NOT EXISTS (
      SELECT 1
      FROM v2_mail_outbox AS item
      WHERE item.batch_key = batch.batch_key
        AND item.failed_at IS NULL
  )
"#;

#[derive(Debug, Clone, FromRow)]
struct MailOutboxItem {
    id: u64,
    batch_key: String,
    sender: Option<String>,
    template_name: Option<String>,
    recipient: String,
    subject: Option<String>,
    body: Option<String>,
    message_id: String,
    attempt_count: u32,
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
    loop {
        if *shutdown.borrow() {
            return Ok(());
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

async fn run_mail_outbox_batch(state: &WorkerState) -> anyhow::Result<usize> {
    let Some(batch) = claim_mail_outbox_batch(state).await? else {
        return Ok(0);
    };
    let state = state.snapshot_config_for_job().await;
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
    let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_mail_outbox SET lease_token = ");
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
    tx: &mut Transaction<'_, MySql>,
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

async fn lock_mail_batch(tx: &mut Transaction<'_, MySql>, batch_key: &str) -> anyhow::Result<()> {
    let found = sqlx::query_scalar::<_, String>(
        "SELECT batch_key FROM v2_mail_outbox_batch WHERE batch_key = ? FOR UPDATE",
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
    tx: &mut Transaction<'_, MySql>,
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

fn mail_outbox_backoff_seconds(attempt_count: u32) -> i64 {
    let exponent = attempt_count.saturating_sub(1).min(10);
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
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("JOIN v2_mail_outbox_batch"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("ORDER BY item.id"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("lease_expires_at"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("attempt_count < ?"));
    }

    #[test]
    fn mail_outbox_terminal_cleanup_deletes_success_and_scrubs_envelope() {
        assert!(MAIL_OUTBOX_ACK_SQL.trim_start().starts_with("DELETE"));
        assert!(MAIL_OUTBOX_ACK_SQL.contains("lease_token = ?"));
        assert!(MAIL_OUTBOX_FAILURE_SQL.contains("lease_token = ?"));
        assert!(MAIL_OUTBOX_LOG_SQL.contains("INSERT INTO v2_mail_log"));
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
        assert_eq!(mail_outbox_backoff_seconds(u32::MAX), 60 * 60);
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
        let migration = include_str!("../../../migrations/0006_durable_mail_outbox.sql");
        assert!(migration.contains("CREATE TABLE `v2_mail_outbox_batch`"));
        assert!(migration.contains("CREATE TABLE `v2_mail_outbox`"));
        assert!(migration.contains("uniq_mail_outbox_batch_recipient"));
        assert!(migration.contains("`message_id` varchar(255) NOT NULL"));
        assert!(migration.contains("`lease_expires_at` bigint DEFAULT NULL"));
        assert!(migration.contains("idx_mail_outbox_claim"));
        assert_eq!(migration.matches("`body` mediumtext").count(), 1);
        assert_eq!(migration.matches("`subject` mediumtext").count(), 1);
        assert_eq!(migration.matches("`template_name` varchar(255)").count(), 1);
        assert!(migration.contains("user/kind/business-day reminder occurrences"));
        assert!(migration.contains("replace pre-send Redis cooldowns"));
    }
}
