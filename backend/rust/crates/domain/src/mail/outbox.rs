//! Shared durable-mail outbox identities, validation, and transactional enqueueing.
//!
//! Request handlers and background jobs both use this module so every durable mail
//! producer writes the same batch/item schema and derives the same stable message IDs.

use std::collections::{HashMap, HashSet};

use lettre::{
    Message,
    message::{Mailbox, header::ContentType},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;
use v2board_db::DbTransaction;

const MAIL_OUTBOX_ENQUEUE_CHUNK_SIZE: usize = 100;
const MAIL_OUTBOX_ADDRESS_MAX_CHARS: usize = 512;
const MAIL_OUTBOX_TEMPLATE_MAX_CHARS: usize = 255;
// Preserve the legacy payload ceiling even though PostgreSQL TEXT is larger.
const LEGACY_MAIL_BODY_MAX_BYTES: usize = 16_777_215;

#[derive(Debug, Clone)]
pub struct PreparedMailEnvelope {
    pub sender: String,
    pub template_name: String,
    pub subject: String,
    pub body: String,
}

/// One independently idempotent single-recipient application occurrence.
/// Scheduler reminders use one occurrence per user/kind/business day.
#[derive(Debug, Clone)]
pub struct MailOutboxOccurrence {
    pub batch_key: String,
    pub recipient: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MailOutboxOccurrenceResult {
    pub inserted: usize,
    pub existing: usize,
    pub duplicate_inputs: usize,
    pub conflicting_batch_keys: Vec<String>,
}

#[derive(Debug, Clone)]
struct ValidatedMailOutboxOccurrence {
    batch_key: String,
    payload_hash: String,
    recipient: String,
    message_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MailOutboxError {
    #[error("mail outbox database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("mail idempotency key was reused with a different payload")]
    IdempotencyConflict,
    #[error("email sender is invalid")]
    InvalidSender,
    #[error("email recipient is invalid")]
    InvalidRecipient,
    #[error("email content is invalid")]
    InvalidContent,
    #[error("mail outbox batch envelope was lost")]
    BatchLost,
}

/// Hashes a producer scope plus its stable mutation identity into the batch PK.
pub fn mail_batch_key(actor: &str, idempotency_key: &str) -> String {
    sha256_hex(format!("{actor}\0{idempotency_key}").as_bytes())
}

/// Hashes a canonical, serializable producer payload for idempotency conflict checks.
pub fn mail_payload_hash<T: Serialize + ?Sized>(payload: &T) -> String {
    sha256_hex(&serde_json::to_vec(payload).expect("mail payload is always JSON serializable"))
}

pub fn prepared_mail_payload_hash(
    envelope: &PreparedMailEnvelope,
    recipients: &[String],
) -> String {
    mail_payload_hash(&(
        &envelope.sender,
        &envelope.template_name,
        &envelope.subject,
        &envelope.body,
        recipients,
    ))
}

/// The stable Message-ID narrows SMTP's unavoidable accept-before-ack uncertainty.
pub fn mail_message_id(batch_key: &str, recipient: &str) -> String {
    let digest = sha256_hex(format!("{batch_key}\0{}", recipient.to_ascii_lowercase()).as_bytes());
    format!("<{digest}@mail.v2board.local>")
}

pub fn validate_mail_sender(sender: &str) -> Result<(), MailOutboxError> {
    if sender.chars().count() > MAIL_OUTBOX_ADDRESS_MAX_CHARS {
        return Err(MailOutboxError::InvalidSender);
    }
    sender
        .parse::<Mailbox>()
        .map(|_| ())
        .map_err(|_| MailOutboxError::InvalidSender)
}

pub fn validate_mail_recipient(recipient: &str) -> Result<(), MailOutboxError> {
    if recipient.chars().count() > MAIL_OUTBOX_ADDRESS_MAX_CHARS {
        return Err(MailOutboxError::InvalidRecipient);
    }
    recipient
        .parse::<Mailbox>()
        .map(|_| ())
        .map_err(|_| MailOutboxError::InvalidRecipient)
}

/// Validates the complete RFC message envelope/header and returns its stable Message-ID.
pub fn validate_prepared_mail_delivery(
    batch_key: &str,
    envelope: &PreparedMailEnvelope,
    recipient: &str,
) -> Result<String, MailOutboxError> {
    validate_mail_sender(&envelope.sender)?;
    validate_mail_recipient(recipient)?;
    if envelope.template_name.chars().count() > MAIL_OUTBOX_TEMPLATE_MAX_CHARS
        || envelope.subject.len() > LEGACY_MAIL_BODY_MAX_BYTES
        || envelope.body.len() > LEGACY_MAIL_BODY_MAX_BYTES
    {
        return Err(MailOutboxError::InvalidContent);
    }
    let message_id = mail_message_id(batch_key, recipient);
    let sender = envelope
        .sender
        .parse::<Mailbox>()
        .map_err(|_| MailOutboxError::InvalidSender)?;
    let recipient = recipient
        .parse::<Mailbox>()
        .map_err(|_| MailOutboxError::InvalidRecipient)?;
    Message::builder()
        .from(sender)
        .to(recipient)
        .subject(&envelope.subject)
        .message_id(Some(message_id.clone()))
        .header(ContentType::TEXT_HTML)
        .body(String::new())
        .map_err(|_| MailOutboxError::InvalidContent)?;
    Ok(message_id)
}

/// Reserves a durable batch identity inside the caller's transaction.
///
/// `Ok(false)` means the identical payload was already reserved or committed. A
/// payload mismatch is explicit so request paths can reject key reuse while cron
/// producers can warn and retain the first durable occurrence.
pub async fn reserve_mail_outbox_batch(
    tx: &mut DbTransaction<'_>,
    batch_key: &str,
    payload_hash: &str,
    actor: &str,
    now: i64,
) -> Result<bool, MailOutboxError> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO mail_outbox_batch
            (batch_key, payload_hash, actor, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (batch_key) DO NOTHING
        "#,
    )
    .bind(batch_key)
    .bind(payload_hash)
    .bind(actor)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1;
    if inserted {
        return Ok(true);
    }
    let existing_hash: String = sqlx::query_scalar(
        "SELECT payload_hash FROM mail_outbox_batch WHERE batch_key = $1 FOR UPDATE",
    )
    .bind(batch_key)
    .fetch_one(&mut **tx)
    .await?;
    if existing_hash != payload_hash {
        return Err(MailOutboxError::IdempotencyConflict);
    }
    Ok(false)
}

/// Stores one envelope on the reserved batch and one durable item per recipient.
/// All RFC validation finishes before the first write, so callers can skip a bad
/// reminder recipient without leaving a partial/empty reminder occurrence.
pub async fn enqueue_prepared_mail(
    tx: &mut DbTransaction<'_>,
    batch_key: &str,
    envelope: &PreparedMailEnvelope,
    recipients: &[String],
    now: i64,
) -> Result<(), MailOutboxError> {
    if recipients.is_empty() {
        return Ok(());
    }
    let mut deliveries = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        let message_id = validate_prepared_mail_delivery(batch_key, envelope, recipient)?;
        deliveries.push((recipient.as_str(), message_id));
    }

    let updated = sqlx::query(
        r#"
        UPDATE mail_outbox_batch
        SET sender = $1, template_name = $2, subject = $3, body = $4, updated_at = $5
        WHERE batch_key = $6
        "#,
    )
    .bind(&envelope.sender)
    .bind(&envelope.template_name)
    .bind(&envelope.subject)
    .bind(&envelope.body)
    .bind(now)
    .bind(batch_key)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(MailOutboxError::BatchLost);
    }
    for chunk in deliveries.chunks(100) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO mail_outbox (batch_key, recipient, message_id, attempt_count, available_at, created_at, updated_at) ",
        );
        builder.push_values(chunk, |mut row, (recipient, message_id)| {
            row.push_bind(batch_key)
                .push_bind(*recipient)
                .push_bind(message_id)
                .push_bind(0_i32)
                .push_bind(now)
                .push_bind(now)
                .push_bind(now);
        });
        builder.build().execute(&mut **tx).await?;
    }
    Ok(())
}

/// Enqueues many independently idempotent single-recipient occurrences without
/// per-recipient SQL. Existing batches are never given a new item: that preserves
/// the durable success tombstone after its delivered item has been deleted.
///
/// Every recipient and RFC header is validated before the first write. A unique
/// transaction-local actor marker atomically claims new batch rows through a
/// no-op upsert; the following locked read distinguishes rows claimed by this
/// call from pre-existing success/pending/failure tombstones. Only claimed rows
/// receive items, so a concurrent caller cannot resurrect a completed delivery.
pub async fn enqueue_mail_outbox_occurrences(
    tx: &mut DbTransaction<'_>,
    actor: &str,
    envelope: &PreparedMailEnvelope,
    occurrences: &[MailOutboxOccurrence],
    now: i64,
) -> Result<MailOutboxOccurrenceResult, MailOutboxError> {
    if actor.chars().count() > MAIL_OUTBOX_ADDRESS_MAX_CHARS {
        return Err(MailOutboxError::InvalidContent);
    }
    let mut validated = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let message_id = validate_prepared_mail_delivery(
            &occurrence.batch_key,
            envelope,
            &occurrence.recipient,
        )?;
        let payload_hash =
            prepared_mail_payload_hash(envelope, std::slice::from_ref(&occurrence.recipient));
        validated.push(ValidatedMailOutboxOccurrence {
            batch_key: occurrence.batch_key.clone(),
            payload_hash,
            recipient: occurrence.recipient.clone(),
            message_id,
        });
    }

    let (validated, duplicate_inputs, input_conflicts) =
        normalize_mail_outbox_occurrences(validated);
    let mut result = MailOutboxOccurrenceResult {
        duplicate_inputs,
        conflicting_batch_keys: input_conflicts,
        ..MailOutboxOccurrenceResult::default()
    };
    if validated.is_empty() {
        return Ok(result);
    }

    let claim_actor = format!("outbox-enqueue:{}", Uuid::new_v4());
    for chunk in validated.chunks(MAIL_OUTBOX_ENQUEUE_CHUNK_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO mail_outbox_batch (batch_key, payload_hash, actor, sender, template_name, subject, body, created_at, updated_at) ",
        );
        builder.push_values(chunk, |mut row, occurrence| {
            row.push_bind(&occurrence.batch_key)
                .push_bind(&occurrence.payload_hash)
                .push_bind(&claim_actor)
                .push_bind(&envelope.sender)
                .push_bind(&envelope.template_name)
                .push_bind(&envelope.subject)
                .push_bind(&envelope.body)
                .push_bind(now)
                .push_bind(now);
        });
        builder.push(" ON CONFLICT (batch_key) DO NOTHING");
        builder.build().execute(&mut **tx).await?;
    }

    let mut claimed_rows = HashMap::with_capacity(validated.len());
    for chunk in validated.chunks(MAIL_OUTBOX_ENQUEUE_CHUNK_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT batch_key, payload_hash, actor FROM mail_outbox_batch WHERE batch_key IN (",
        );
        let mut keys = builder.separated(", ");
        for occurrence in chunk {
            keys.push_bind(&occurrence.batch_key);
        }
        keys.push_unseparated(") FOR UPDATE");
        let rows = builder
            .build_query_as::<(String, String, String)>()
            .fetch_all(&mut **tx)
            .await?;
        claimed_rows.extend(
            rows.into_iter()
                .map(|(batch_key, payload_hash, actor)| (batch_key, (payload_hash, actor))),
        );
    }
    let fresh = partition_claimed_mail_outbox_occurrences(
        validated,
        &claimed_rows,
        &claim_actor,
        &mut result,
    )?;

    for chunk in fresh.chunks(MAIL_OUTBOX_ENQUEUE_CHUNK_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE mail_outbox_batch SET actor = ");
        builder
            .push_bind(actor)
            .push(" WHERE actor = ")
            .push_bind(&claim_actor)
            .push(" AND batch_key IN (");
        let mut keys = builder.separated(", ");
        for occurrence in chunk {
            keys.push_bind(&occurrence.batch_key);
        }
        keys.push_unseparated(")");
        let updated = builder.build().execute(&mut **tx).await?;
        if updated.rows_affected() != chunk.len() as u64 {
            return Err(MailOutboxError::BatchLost);
        }
    }
    for chunk in fresh.chunks(MAIL_OUTBOX_ENQUEUE_CHUNK_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO mail_outbox (batch_key, recipient, message_id, attempt_count, available_at, created_at, updated_at) ",
        );
        builder.push_values(chunk, |mut row, occurrence| {
            row.push_bind(&occurrence.batch_key)
                .push_bind(&occurrence.recipient)
                .push_bind(&occurrence.message_id)
                .push_bind(0_i32)
                .push_bind(now)
                .push_bind(now)
                .push_bind(now);
        });
        builder.build().execute(&mut **tx).await?;
    }
    result.inserted = fresh.len();
    Ok(result)
}

fn normalize_mail_outbox_occurrences(
    occurrences: Vec<ValidatedMailOutboxOccurrence>,
) -> (Vec<ValidatedMailOutboxOccurrence>, usize, Vec<String>) {
    let mut positions = HashMap::<String, usize>::with_capacity(occurrences.len());
    let mut normalized =
        Vec::<Option<ValidatedMailOutboxOccurrence>>::with_capacity(occurrences.len());
    let mut duplicate_counts = Vec::<usize>::with_capacity(occurrences.len());
    let mut conflicting = HashSet::with_capacity(occurrences.len());
    let mut conflicting_batch_keys = Vec::new();
    for occurrence in occurrences {
        if conflicting.contains(&occurrence.batch_key) {
            continue;
        }
        if let Some(index) = positions.get(&occurrence.batch_key).copied() {
            let existing = normalized[index]
                .as_ref()
                .expect("non-conflicting occurrence position is populated");
            if existing.payload_hash == occurrence.payload_hash {
                duplicate_counts[index] += 1;
            } else {
                normalized[index] = None;
                duplicate_counts[index] = 0;
                conflicting.insert(occurrence.batch_key.clone());
                conflicting_batch_keys.push(occurrence.batch_key);
            }
            continue;
        }
        positions.insert(occurrence.batch_key.clone(), normalized.len());
        normalized.push(Some(occurrence));
        duplicate_counts.push(0);
    }
    let duplicate_inputs = duplicate_counts.into_iter().sum();
    (
        normalized.into_iter().flatten().collect(),
        duplicate_inputs,
        conflicting_batch_keys,
    )
}

fn partition_claimed_mail_outbox_occurrences(
    occurrences: Vec<ValidatedMailOutboxOccurrence>,
    claimed_rows: &HashMap<String, (String, String)>,
    claim_actor: &str,
    result: &mut MailOutboxOccurrenceResult,
) -> Result<Vec<ValidatedMailOutboxOccurrence>, MailOutboxError> {
    let mut fresh = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let Some((payload_hash, row_actor)) = claimed_rows.get(&occurrence.batch_key) else {
            return Err(MailOutboxError::BatchLost);
        };
        if row_actor == claim_actor {
            if payload_hash != &occurrence.payload_hash {
                return Err(MailOutboxError::BatchLost);
            }
            fresh.push(occurrence);
        } else if payload_hash == &occurrence.payload_hash {
            result.existing += 1;
        } else {
            result.conflicting_batch_keys.push(occurrence.batch_key);
        }
    }
    Ok(fresh)
}

fn sha256_hex(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> PreparedMailEnvelope {
        PreparedMailEnvelope {
            sender: "Site <sender@example.test>".to_string(),
            template_name: "mail.default.notify".to_string(),
            subject: "Subject".to_string(),
            body: "Body".to_string(),
        }
    }

    #[test]
    fn identities_are_stable_scoped_and_recipient_specific() {
        let batch_key = mail_batch_key("admin:one@example.test", "retry-key");
        assert_eq!(batch_key.len(), 64);
        assert_eq!(
            batch_key,
            mail_batch_key("admin:one@example.test", "retry-key")
        );
        assert_ne!(
            batch_key,
            mail_batch_key("staff:one@example.test", "retry-key")
        );

        let message_id = mail_message_id(&batch_key, "USER@example.test");
        assert_eq!(message_id, mail_message_id(&batch_key, "user@example.test"));
        assert_ne!(
            message_id,
            mail_message_id(&batch_key, "other@example.test")
        );
        assert!(message_id.starts_with('<'));
        assert!(message_id.ends_with("@mail.v2board.local>"));
    }

    #[test]
    fn prepared_payload_identity_covers_envelope_and_recipient() {
        let envelope = envelope();
        let first = prepared_mail_payload_hash(&envelope, &["one@example.test".to_string()]);
        assert_eq!(
            first,
            prepared_mail_payload_hash(&envelope, &["one@example.test".to_string()])
        );
        assert_ne!(
            first,
            prepared_mail_payload_hash(&envelope, &["two@example.test".to_string()])
        );
    }

    #[test]
    fn complete_delivery_validation_rejects_each_invalid_header_role() {
        let envelope = envelope();
        let batch_key = mail_batch_key("test", "one");
        assert!(validate_prepared_mail_delivery(&batch_key, &envelope, "to@example.test").is_ok());
        assert!(matches!(
            validate_prepared_mail_delivery(&batch_key, &envelope, "not an address"),
            Err(MailOutboxError::InvalidRecipient)
        ));
        let mut bad_template = envelope.clone();
        bad_template.template_name = "x".repeat(MAIL_OUTBOX_TEMPLATE_MAX_CHARS + 1);
        assert!(matches!(
            validate_prepared_mail_delivery(&batch_key, &bad_template, "to@example.test"),
            Err(MailOutboxError::InvalidContent)
        ));
        let mut bad_sender = envelope;
        bad_sender.sender = "not an address".to_string();
        assert!(matches!(
            validate_prepared_mail_delivery(&batch_key, &bad_sender, "to@example.test"),
            Err(MailOutboxError::InvalidSender)
        ));
    }

    fn validated(batch_key: &str, payload_hash: &str) -> ValidatedMailOutboxOccurrence {
        ValidatedMailOutboxOccurrence {
            batch_key: batch_key.to_string(),
            payload_hash: payload_hash.to_string(),
            recipient: format!("{batch_key}@example.test"),
            message_id: format!("<{batch_key}@mail.v2board.local>"),
        }
    }

    #[test]
    fn bulk_occurrence_input_duplicates_are_collapsed_or_conflicted_before_sql() {
        let same = validated("same", "payload");
        let (normalized, duplicate_inputs, conflicts) =
            normalize_mail_outbox_occurrences(vec![same.clone(), same]);
        assert_eq!(normalized.len(), 1);
        assert_eq!(duplicate_inputs, 1);
        assert!(conflicts.is_empty());

        let (normalized, duplicate_inputs, conflicts) = normalize_mail_outbox_occurrences(vec![
            validated("conflict", "first"),
            validated("conflict", "second"),
            validated("conflict", "first"),
        ]);
        assert!(normalized.is_empty());
        assert_eq!(duplicate_inputs, 0);
        assert_eq!(conflicts, vec!["conflict"]);
    }

    #[test]
    fn only_rows_owned_by_this_atomic_claim_receive_new_items() {
        let claim_actor = "outbox-enqueue:claim";
        let occurrences = vec![
            validated("fresh", "fresh-payload"),
            validated("completed", "completed-payload"),
            validated("conflict", "requested-payload"),
        ];
        let claimed_rows = HashMap::from([
            (
                "fresh".to_string(),
                ("fresh-payload".to_string(), claim_actor.to_string()),
            ),
            (
                "completed".to_string(),
                (
                    "completed-payload".to_string(),
                    "original-actor".to_string(),
                ),
            ),
            (
                "conflict".to_string(),
                ("stored-payload".to_string(), "original-actor".to_string()),
            ),
        ]);
        let mut result = MailOutboxOccurrenceResult::default();
        let fresh = partition_claimed_mail_outbox_occurrences(
            occurrences,
            &claimed_rows,
            claim_actor,
            &mut result,
        )
        .unwrap();
        assert_eq!(
            fresh
                .into_iter()
                .map(|occurrence| occurrence.batch_key)
                .collect::<Vec<_>>(),
            vec!["fresh"]
        );
        assert_eq!(result.existing, 1);
        assert_eq!(result.conflicting_batch_keys, vec!["conflict"]);
    }
}
