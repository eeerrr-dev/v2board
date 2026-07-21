//! PostgreSQL, runtime configuration, and SMTP adapters for worker mail ports.

use std::{collections::HashMap, sync::Arc, time::Duration};

use lettre::{AsyncTransport, Message, message::header::ContentType};
use sqlx::{FromRow, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;
use v2board_application::worker_mail::{
    ClaimedMailBatch, ClaimedMailItem, MailDelivery, MailFailure, MailOutboxPolicy,
    MailOutboxRepository, MailOutboxService, MailWorkerPortError, ReminderEnvelope, ReminderKind,
    ReminderPageCommand, ReminderPageOutcome, ReminderPreparationError, ReminderRenderer,
    ReminderRepository, ReminderService, RetentionCleanup, RetentionPolicy,
};
use v2board_config::AppConfig;
use v2board_db::DbPool;

use crate::{
    mail::{
        outbox::{
            MailOutboxOccurrence, PreparedMailEnvelope, enqueue_mail_outbox_occurrences,
            mail_batch_key, validate_mail_sender, validate_prepared_mail_delivery,
        },
        render_reminder,
    },
    smtp::{SmtpSettings, SmtpTransportCache},
};

const EXPIRE_REMINDER_CANDIDATES_SQL: &str = r#"
SELECT id, email
FROM users
WHERE COALESCE(remind_expire, 0) <> 0
  AND expired_at IS NOT NULL
  AND expired_at > $1
  AND CAST(expired_at AS DECIMAL(65, 0)) - 86400
      < CAST($2 AS DECIMAL(65, 0))
  AND id > $3
ORDER BY id
LIMIT $4
"#;
const TRAFFIC_REMINDER_CANDIDATES_SQL: &str = r#"
SELECT id, email
FROM users
WHERE COALESCE(remind_traffic, 0) <> 0
  AND (expired_at IS NULL OR expired_at >= $1)
  AND (CAST(u AS DECIMAL(65, 0)) + CAST(d AS DECIMAL(65, 0))) > 0
  AND CAST(transfer_enable AS DECIMAL(65, 0)) > 0
  AND (CAST(u AS DECIMAL(65, 0)) + CAST(d AS DECIMAL(65, 0))) * 100
      >= CAST(transfer_enable AS DECIMAL(65, 0)) * 95
  AND (CAST(u AS DECIMAL(65, 0)) + CAST(d AS DECIMAL(65, 0)))
      < CAST(transfer_enable AS DECIMAL(65, 0))
  AND id > $2
ORDER BY id
LIMIT $3
"#;
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

const DEFAULT_MAIL_RETENTION_DAYS: u64 = 90;
const DEFAULT_IDEMPOTENCY_RETENTION_DAYS: u64 = 90;
const DEFAULT_CLEANUP_INTERVAL_SECONDS: u64 = 6 * 60 * 60;
const SMTP_DELIVERY_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, FromRow)]
struct ReminderUserRow {
    id: i64,
    email: String,
}

#[derive(Clone)]
pub struct PostgresReminderRepository {
    db: DbPool,
}

impl PostgresReminderRepository {
    pub const fn new(db: DbPool) -> Self {
        Self { db }
    }
}

impl ReminderRepository for PostgresReminderRepository {
    async fn enqueue_page(
        &self,
        command: ReminderPageCommand<'_>,
    ) -> Result<ReminderPageOutcome, MailWorkerPortError> {
        let limit = i64::try_from(command.limit)
            .map_err(|error| port("select reminder candidates", error))?;
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| port("begin reminder transaction", error))?;
        let users = select_reminder_candidates(
            &mut tx,
            command.kind,
            command.now,
            command.after_user_id,
            limit,
        )
        .await?;
        let selected = users.len();
        let last_user_id = users.last().map(|user| user.id);
        if users.is_empty() {
            tx.commit()
                .await
                .map_err(|error| port("commit reminder transaction", error))?;
            return Ok(ReminderPageOutcome::default());
        }

        let actor = reminder_actor(command.kind);
        let envelope = PreparedMailEnvelope {
            sender: command.envelope.sender.clone(),
            template_name: command.envelope.template_name.clone(),
            subject: command.envelope.subject.clone(),
            body: command.envelope.body.clone(),
        };
        let mut occurrences = Vec::with_capacity(users.len());
        let mut user_ids_by_batch = HashMap::with_capacity(users.len());
        let mut skipped = 0_usize;
        for user in users {
            let batch_key =
                mail_batch_key(&actor, &format!("{}:{}", user.id, command.business_day));
            if let Err(error) = validate_prepared_mail_delivery(&batch_key, &envelope, &user.email)
            {
                tracing::warn!(
                    user_id = user.id,
                    kind = ?command.kind,
                    ?error,
                    "reminder mail recipient is invalid"
                );
                skipped += 1;
                continue;
            }
            user_ids_by_batch.insert(batch_key.clone(), user.id);
            occurrences.push(MailOutboxOccurrence {
                batch_key,
                recipient: user.email,
            });
        }
        let result =
            enqueue_mail_outbox_occurrences(&mut tx, &actor, &envelope, &occurrences, command.now)
                .await
                .map_err(|error| port("enqueue reminder occurrences", error))?;
        tx.commit()
            .await
            .map_err(|error| port("commit reminder transaction", error))?;
        for batch_key in &result.conflicting_batch_keys {
            tracing::warn!(
                user_id = ?user_ids_by_batch.get(batch_key),
                kind = ?command.kind,
                batch_key,
                "reminder occurrence already exists with a different payload"
            );
        }
        Ok(ReminderPageOutcome {
            selected,
            last_user_id,
            enqueued: result.inserted,
            existing: result.existing + result.duplicate_inputs,
            skipped: skipped + result.conflicting_batch_keys.len(),
        })
    }
}

async fn select_reminder_candidates(
    tx: &mut Transaction<'_, Postgres>,
    kind: ReminderKind,
    now: i64,
    after_user_id: i64,
    limit: i64,
) -> Result<Vec<ReminderUserRow>, MailWorkerPortError> {
    match kind {
        ReminderKind::Expire => {
            sqlx::query_as::<_, ReminderUserRow>(EXPIRE_REMINDER_CANDIDATES_SQL)
                .bind(now)
                .bind(now)
                .bind(after_user_id)
                .bind(limit)
                .fetch_all(&mut **tx)
                .await
                .map_err(|error| port("select expiry reminder candidates", error))
        }
        ReminderKind::Traffic => {
            sqlx::query_as::<_, ReminderUserRow>(TRAFFIC_REMINDER_CANDIDATES_SQL)
                .bind(now)
                .bind(after_user_id)
                .bind(limit)
                .fetch_all(&mut **tx)
                .await
                .map_err(|error| port("select traffic reminder candidates", error))
        }
    }
}

#[derive(Clone)]
pub struct RuntimeReminderRenderer {
    config: Arc<AppConfig>,
}

impl RuntimeReminderRenderer {
    pub const fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }
}

impl ReminderRenderer for RuntimeReminderRenderer {
    fn prepare(&self, kind: ReminderKind) -> Result<ReminderEnvelope, ReminderPreparationError> {
        let from = self
            .config
            .email_from_address
            .as_deref()
            .or(self.config.email_username.as_deref())
            .filter(|from| !from.trim().is_empty())
            .ok_or_else(|| ReminderPreparationError::new("email sender is missing"))?;
        let sender = format!("{} <{}>", self.config.app_name, from);
        validate_mail_sender(&sender).map_err(ReminderPreparationError::new)?;
        let subject = match kind {
            ReminderKind::Expire => {
                format!("The service in {} is about to expire", self.config.app_name)
            }
            ReminderKind::Traffic => format!(
                "The traffic usage in {} has reached 95%",
                self.config.app_name
            ),
        };
        let envelope = ReminderEnvelope {
            sender,
            template_name: format!(
                "mail.{}.{}",
                self.config.email_template.as_deref().unwrap_or("default"),
                kind.template_name()
            ),
            subject,
            body: render_reminder(
                kind,
                &self.config.app_name,
                self.config.app_url.as_deref().unwrap_or_default(),
            ),
        };
        let validation_batch = mail_batch_key("worker:reminder:validation", kind.template_name());
        validate_prepared_mail_delivery(
            &validation_batch,
            &PreparedMailEnvelope {
                sender: envelope.sender.clone(),
                template_name: envelope.template_name.clone(),
                subject: envelope.subject.clone(),
                body: envelope.body.clone(),
            },
            "mail-validator@v2board.local",
        )
        .map_err(ReminderPreparationError::new)?;
        Ok(envelope)
    }
}

#[derive(Debug, Clone, FromRow)]
struct MailOutboxRow {
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

impl From<MailOutboxRow> for ClaimedMailItem {
    fn from(item: MailOutboxRow) -> Self {
        Self {
            id: item.id,
            batch_key: item.batch_key,
            sender: item.sender,
            template_name: item.template_name,
            recipient: item.recipient,
            subject: item.subject,
            body: item.body,
            message_id: item.message_id,
            attempt_count: item.attempt_count,
        }
    }
}

#[derive(Clone)]
pub struct PostgresMailOutboxRepository {
    db: DbPool,
}

impl PostgresMailOutboxRepository {
    pub const fn new(db: DbPool) -> Self {
        Self { db }
    }
}

impl MailOutboxRepository for PostgresMailOutboxRepository {
    async fn claim(
        &self,
        now: i64,
        policy: MailOutboxPolicy,
    ) -> Result<Option<ClaimedMailBatch>, MailWorkerPortError> {
        let limit = i64::try_from(policy.batch_size)
            .map_err(|error| port("claim mail outbox batch", error))?;
        let lease_expires_at = now
            .checked_add(policy.lease_seconds)
            .ok_or_else(|| port("claim mail outbox batch", "lease timestamp overflow"))?;
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| port("begin mail outbox claim", error))?;
        let rows = sqlx::query_as::<_, MailOutboxRow>(MAIL_OUTBOX_CLAIM_SQL)
            .bind(now)
            .bind(now)
            .bind(policy.max_attempts)
            .bind(limit)
            .fetch_all(&mut *tx)
            .await
            .map_err(|error| port("select mail outbox claim", error))?;
        if rows.is_empty() {
            tx.commit()
                .await
                .map_err(|error| port("commit empty mail outbox claim", error))?;
            return Ok(None);
        }
        let lease_token = Uuid::new_v4().to_string();
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE mail_outbox SET lease_token = ");
        builder
            .push_bind(&lease_token)
            .push(", lease_expires_at = ")
            .push_bind(lease_expires_at)
            .push(", updated_at = ")
            .push_bind(now)
            .push(" WHERE id IN (");
        let mut ids = builder.separated(", ");
        for item in &rows {
            ids.push_bind(item.id);
        }
        ids.push_unseparated(")");
        let claimed = builder
            .build()
            .execute(&mut *tx)
            .await
            .map_err(|error| port("update mail outbox leases", error))?;
        if claimed.rows_affected() != rows.len() as u64 {
            return Err(port(
                "claim mail outbox batch",
                "claim changed an unexpected number of rows",
            ));
        }
        tx.commit()
            .await
            .map_err(|error| port("commit mail outbox claim", error))?;
        Ok(Some(ClaimedMailBatch {
            lease_token,
            items: rows.into_iter().map(Into::into).collect(),
        }))
    }

    async fn acknowledge(
        &self,
        lease_token: &str,
        item: &ClaimedMailItem,
        now: i64,
    ) -> Result<(), MailWorkerPortError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| port("begin mail acknowledgement", error))?;
        lock_mail_batch(&mut tx, &item.batch_key).await?;
        let deleted = sqlx::query(MAIL_OUTBOX_ACK_SQL)
            .bind(item.id)
            .bind(lease_token)
            .execute(&mut *tx)
            .await
            .map_err(|error| port("acknowledge mail delivery", error))?;
        if deleted.rows_affected() != 1 {
            return Err(port("acknowledge mail delivery", "delivery lease was lost"));
        }
        insert_mail_log(&mut tx, item, None, now).await?;
        clear_completed_batch_envelope(&mut tx, &item.batch_key, now).await?;
        tx.commit()
            .await
            .map_err(|error| port("commit mail acknowledgement", error))
    }

    async fn record_failure(
        &self,
        lease_token: &str,
        item: &ClaimedMailItem,
        failure: &MailFailure,
        now: i64,
    ) -> Result<(), MailWorkerPortError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| port("begin mail failure recording", error))?;
        lock_mail_batch(&mut tx, &item.batch_key).await?;
        let updated = sqlx::query(MAIL_OUTBOX_FAILURE_SQL)
            .bind(failure.attempt_count)
            .bind(failure.available_at)
            .bind(failure.failed_at)
            .bind(&failure.last_error)
            .bind(now)
            .bind(item.id)
            .bind(lease_token)
            .execute(&mut *tx)
            .await
            .map_err(|error| port("record mail delivery failure", error))?;
        if updated.rows_affected() != 1 {
            return Err(port(
                "record mail delivery failure",
                "delivery lease was lost",
            ));
        }
        if failure.failed_at.is_some() {
            insert_mail_log(&mut tx, item, Some(&failure.last_error), now).await?;
            clear_completed_batch_envelope(&mut tx, &item.batch_key, now).await?;
        }
        tx.commit()
            .await
            .map_err(|error| port("commit mail failure recording", error))
    }

    async fn cleanup(&self, cleanup: RetentionCleanup) -> Result<u64, MailWorkerPortError> {
        let limit = i64::try_from(cleanup.batch_size)
            .map_err(|error| port("clean retained worker state", error))?;
        let mut deleted = 0_u64;
        for (sql, cutoff) in [
            (MAIL_OUTBOX_RETENTION_SQL, cleanup.mail_before),
            (MAIL_OUTBOX_BATCH_RETENTION_SQL, cleanup.mail_before),
            (MAIL_LOG_RETENTION_SQL, cleanup.mail_before),
            (TRAFFIC_REPORT_RETENTION_SQL, cleanup.idempotency_before),
        ] {
            for _ in 0..cleanup.max_batches_per_table {
                let affected = sqlx::query(sql)
                    .bind(cutoff)
                    .bind(limit)
                    .execute(&self.db)
                    .await
                    .map_err(|error| port("clean retained worker state", error))?
                    .rows_affected();
                deleted = deleted.saturating_add(affected);
                if affected < limit as u64 {
                    break;
                }
            }
        }
        Ok(deleted)
    }
}

async fn lock_mail_batch(
    tx: &mut Transaction<'_, Postgres>,
    batch_key: &str,
) -> Result<(), MailWorkerPortError> {
    let found = sqlx::query_scalar::<_, String>(
        "SELECT batch_key FROM mail_outbox_batch WHERE batch_key = $1 FOR UPDATE",
    )
    .bind(batch_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| port("lock mail outbox batch", error))?;
    if found.is_none() {
        return Err(port("lock mail outbox batch", "batch was lost"));
    }
    Ok(())
}

async fn insert_mail_log(
    tx: &mut Transaction<'_, Postgres>,
    item: &ClaimedMailItem,
    error: Option<&str>,
    now: i64,
) -> Result<(), MailWorkerPortError> {
    let subject = truncate_log_field(item.subject.as_deref(), "");
    let template_name = truncate_log_field(item.template_name.as_deref(), "mail.default.notify");
    sqlx::query(MAIL_OUTBOX_LOG_SQL)
        .bind(&item.recipient)
        .bind(subject)
        .bind(template_name)
        .bind(error)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await
        .map_err(|error| port("insert mail delivery log", error))?;
    Ok(())
}

async fn clear_completed_batch_envelope(
    tx: &mut Transaction<'_, Postgres>,
    batch_key: &str,
    now: i64,
) -> Result<(), MailWorkerPortError> {
    sqlx::query(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL)
        .bind(now)
        .bind(batch_key)
        .execute(&mut **tx)
        .await
        .map_err(|error| port("clear completed mail envelope", error))?;
    Ok(())
}

fn truncate_log_field(value: Option<&str>, fallback: &str) -> String {
    value.unwrap_or(fallback).chars().take(255).collect()
}

#[derive(Clone)]
pub struct SmtpMailDelivery {
    config: Arc<AppConfig>,
    smtp: SmtpTransportCache,
}

impl SmtpMailDelivery {
    pub const fn new(config: Arc<AppConfig>, smtp: SmtpTransportCache) -> Self {
        Self { config, smtp }
    }
}

impl MailDelivery for SmtpMailDelivery {
    async fn deliver(&self, item: &ClaimedMailItem) -> Result<(), MailWorkerPortError> {
        let settings =
            SmtpSettings::load(&self.config).map_err(|error| port("load SMTP settings", error))?;
        let sender = item
            .sender
            .as_deref()
            .ok_or_else(|| port("build SMTP message", "mail outbox sender is missing"))?;
        let subject = item
            .subject
            .as_deref()
            .ok_or_else(|| port("build SMTP message", "mail outbox subject is missing"))?;
        let body = item
            .body
            .as_deref()
            .ok_or_else(|| port("build SMTP message", "mail outbox body is missing"))?;
        let message = Message::builder()
            .from(
                sender
                    .parse()
                    .map_err(|error| port("parse SMTP sender", error))?,
            )
            .to(item
                .recipient
                .parse()
                .map_err(|error| port("parse SMTP recipient", error))?)
            .subject(subject)
            .message_id(Some(item.message_id.clone()))
            .header(ContentType::TEXT_HTML)
            .body(body.to_string())
            .map_err(|error| port("build SMTP message", error))?;
        let transport = self
            .smtp
            .transport(&settings)
            .map_err(|error| port("build SMTP transport", error))?;
        tokio::time::timeout(SMTP_DELIVERY_TIMEOUT, transport.send(message))
            .await
            .map_err(|_| port("deliver SMTP message", "SMTP delivery timed out"))?
            .map_err(|error| port("deliver SMTP message", error))?;
        Ok(())
    }
}

pub type RuntimeReminderService =
    ReminderService<PostgresReminderRepository, RuntimeReminderRenderer>;
pub type RuntimeMailOutboxService =
    MailOutboxService<PostgresMailOutboxRepository, SmtpMailDelivery>;

pub fn reminder_service(db: DbPool, config: Arc<AppConfig>) -> RuntimeReminderService {
    ReminderService::new(
        PostgresReminderRepository::new(db),
        RuntimeReminderRenderer::new(config),
    )
}

pub fn mail_outbox_service(
    db: DbPool,
    config: Arc<AppConfig>,
    smtp: SmtpTransportCache,
) -> RuntimeMailOutboxService {
    MailOutboxService::new(
        PostgresMailOutboxRepository::new(db),
        SmtpMailDelivery::new(config, smtp),
        MailOutboxPolicy::default(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerRetentionConfig {
    pub policy: RetentionPolicy,
    pub cleanup_interval: Duration,
}

impl WorkerRetentionConfig {
    pub fn from_env() -> Result<Self, MailWorkerPortError> {
        let mail_days = parse_bounded_env(
            "V2BOARD_MAIL_RETENTION_DAYS",
            DEFAULT_MAIL_RETENTION_DAYS,
            1,
            3_650,
        )?;
        let idempotency_days = parse_bounded_env(
            "V2BOARD_IDEMPOTENCY_RETENTION_DAYS",
            DEFAULT_IDEMPOTENCY_RETENTION_DAYS,
            1,
            3_650,
        )?;
        let cleanup_interval = parse_bounded_env(
            "V2BOARD_WORKER_CLEANUP_INTERVAL_SECONDS",
            DEFAULT_CLEANUP_INTERVAL_SECONDS,
            60,
            7 * 86_400,
        )?;
        Ok(Self {
            policy: RetentionPolicy {
                mail_retention_seconds: days_to_seconds(mail_days)?,
                idempotency_retention_seconds: days_to_seconds(idempotency_days)?,
                ..RetentionPolicy::default()
            },
            cleanup_interval: Duration::from_secs(cleanup_interval),
        })
    }
}

fn parse_bounded_env(
    name: &'static str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, MailWorkerPortError> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| port("parse worker retention configuration", "value is not UTF-8"))?;
    parse_bounded_value(name, raw, minimum, maximum)
}

fn parse_bounded_value(
    name: &'static str,
    raw: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, MailWorkerPortError> {
    let value = raw.parse::<u64>().map_err(|_| {
        port(
            "parse worker retention configuration",
            format!("{name} must be an integer"),
        )
    })?;
    if !(minimum..=maximum).contains(&value) {
        return Err(port(
            "parse worker retention configuration",
            format!("{name} must be between {minimum} and {maximum}"),
        ));
    }
    Ok(value)
}

fn days_to_seconds(days: u64) -> Result<i64, MailWorkerPortError> {
    days.checked_mul(86_400)
        .and_then(|seconds| i64::try_from(seconds).ok())
        .ok_or_else(|| {
            port(
                "parse worker retention configuration",
                "retention duration overflow",
            )
        })
}

fn reminder_actor(kind: ReminderKind) -> String {
    format!("worker:reminder:{}", kind.template_name())
}

fn port(operation: &'static str, error: impl std::fmt::Display) -> MailWorkerPortError {
    MailWorkerPortError::new(operation, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_queries_are_keyset_paged_and_use_wide_math() {
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("DECIMAL(65, 0)"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("id > $3"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("LIMIT $4"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("DECIMAL(65, 0)"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("* 95"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("id > $2"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("LIMIT $3"));
    }

    #[test]
    fn claim_and_terminal_mutations_are_leased_and_scrub_envelopes() {
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(MAIL_OUTBOX_CLAIM_SQL.contains("attempt_count < $3"));
        assert!(MAIL_OUTBOX_ACK_SQL.contains("lease_token = $2"));
        assert!(MAIL_OUTBOX_FAILURE_SQL.contains("lease_token = $7"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("sender = NULL"));
        assert!(MAIL_OUTBOX_CLEAR_ENVELOPE_SQL.contains("NOT EXISTS"));
    }

    #[test]
    fn retention_configuration_is_strict_and_bounded() {
        assert_eq!(parse_bounded_value("test", "90", 1, 3_650).unwrap(), 90);
        assert!(parse_bounded_value("test", "0", 1, 3_650).is_err());
        assert!(parse_bounded_value("test", "3651", 1, 3_650).is_err());
        assert!(parse_bounded_value("test", "many", 1, 3_650).is_err());
        assert_eq!(days_to_seconds(90).unwrap(), 7_776_000);
    }

    #[test]
    fn mail_log_fields_fit_the_schema() {
        assert_eq!(
            truncate_log_field(None, "mail.default.notify"),
            "mail.default.notify"
        );
        assert_eq!(
            truncate_log_field(Some(&"界".repeat(300)), "")
                .chars()
                .count(),
            255
        );
    }

    #[test]
    fn retention_queries_only_remove_terminal_rows_in_bounded_batches() {
        assert!(MAIL_OUTBOX_RETENTION_SQL.contains("failed_at IS NOT NULL"));
        assert!(MAIL_OUTBOX_BATCH_RETENTION_SQL.contains("NOT EXISTS"));
        assert!(TRAFFIC_REPORT_RETENTION_SQL.contains("applied_at IS NOT NULL"));
        assert!(MAIL_OUTBOX_RETENTION_SQL.contains("LIMIT $2"));
    }
}
