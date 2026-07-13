use std::collections::HashMap;

use chrono::NaiveDate;
use sqlx::{FromRow, Postgres, Transaction};
use v2board_config::{AppConfig, app_now};
use v2board_domain::mail::{
    ReminderKind,
    outbox::{
        MailOutboxError, MailOutboxOccurrence, PreparedMailEnvelope,
        enqueue_mail_outbox_occurrences, mail_batch_key, validate_mail_sender,
        validate_prepared_mail_delivery,
    },
    render_reminder,
};

use crate::state::WorkerState;

const REMINDER_PAGE_SIZE: i64 = 500;
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

#[derive(Debug, Clone, FromRow)]
struct ReminderUserRow {
    id: i64,
    email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReminderIdentity {
    batch_key: String,
}

#[derive(Debug, Default)]
struct ReminderRunStats {
    enqueued: usize,
    existing: usize,
    skipped: usize,
}

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let now = app_now();
    let timestamp = now.timestamp();
    let business_day = now.date_naive();
    let mut stats = ReminderRunStats::default();

    for kind in [ReminderKind::Expire, ReminderKind::Traffic] {
        let envelope = match prepare_reminder_envelope(&state.config, kind) {
            Ok(envelope) => envelope,
            Err(error) => {
                tracing::warn!(?kind, ?error, "reminder mail configuration is invalid");
                stats.skipped += 1;
                continue;
            }
        };
        let kind_stats =
            enqueue_reminder_kind(state, kind, &envelope, timestamp, business_day).await?;
        stats.enqueued += kind_stats.enqueued;
        stats.existing += kind_stats.existing;
        stats.skipped += kind_stats.skipped;
    }

    tracing::info!(
        enqueued = stats.enqueued,
        existing = stats.existing,
        skipped = stats.skipped,
        %business_day,
        "durable reminder occurrences prepared"
    );
    Ok(())
}

async fn enqueue_reminder_kind(
    state: &WorkerState,
    kind: ReminderKind,
    envelope: &PreparedMailEnvelope,
    now: i64,
    business_day: NaiveDate,
) -> anyhow::Result<ReminderRunStats> {
    let actor = reminder_actor(kind);
    let mut stats = ReminderRunStats::default();
    let mut after_user_id = 0_i64;
    loop {
        let mut tx = state.db.begin().await?;
        let users =
            select_reminder_candidates(&mut tx, kind, now, after_user_id, REMINDER_PAGE_SIZE)
                .await?;
        let Some(last_user_id) = users.last().map(|user| user.id) else {
            tx.commit().await?;
            break;
        };
        let page_len = users.len();
        let mut occurrences = Vec::with_capacity(users.len());
        let mut user_ids_by_batch = HashMap::with_capacity(users.len());
        for user in users {
            let identity = reminder_identity(user.id, kind, business_day);
            if let Err(error) =
                validate_prepared_mail_delivery(&identity.batch_key, envelope, &user.email)
            {
                tracing::warn!(
                    user_id = user.id,
                    ?kind,
                    ?error,
                    "reminder mail recipient is invalid"
                );
                stats.skipped += 1;
                continue;
            }
            user_ids_by_batch.insert(identity.batch_key.clone(), user.id);
            occurrences.push(MailOutboxOccurrence {
                batch_key: identity.batch_key,
                recipient: user.email,
            });
        }
        let result =
            enqueue_mail_outbox_occurrences(&mut tx, &actor, envelope, &occurrences, now).await?;
        tx.commit().await?;
        stats.enqueued += result.inserted;
        stats.existing += result.existing + result.duplicate_inputs;
        stats.skipped += result.conflicting_batch_keys.len();
        for batch_key in result.conflicting_batch_keys {
            tracing::warn!(
                user_id = ?user_ids_by_batch.get(&batch_key),
                ?kind,
                batch_key,
                "reminder occurrence already exists with a different payload"
            );
        }
        after_user_id = last_user_id;
        if page_len < REMINDER_PAGE_SIZE as usize {
            break;
        }
    }
    Ok(stats)
}

async fn select_reminder_candidates(
    tx: &mut Transaction<'_, Postgres>,
    kind: ReminderKind,
    now: i64,
    after_user_id: i64,
    limit: i64,
) -> anyhow::Result<Vec<ReminderUserRow>> {
    let users = match kind {
        ReminderKind::Expire => {
            sqlx::query_as::<_, ReminderUserRow>(EXPIRE_REMINDER_CANDIDATES_SQL)
                .bind(now)
                .bind(now)
                .bind(after_user_id)
                .bind(limit)
                .fetch_all(&mut **tx)
                .await?
        }
        ReminderKind::Traffic => {
            sqlx::query_as::<_, ReminderUserRow>(TRAFFIC_REMINDER_CANDIDATES_SQL)
                .bind(now)
                .bind(after_user_id)
                .bind(limit)
                .fetch_all(&mut **tx)
                .await?
        }
    };
    Ok(users)
}

fn prepare_reminder_envelope(
    config: &AppConfig,
    kind: ReminderKind,
) -> Result<PreparedMailEnvelope, MailOutboxError> {
    // Only the visible envelope is snapshotted. Relay credentials remain runtime
    // configuration so a queued reminder can recover after operators repair SMTP.
    let from = config
        .email_from_address
        .as_deref()
        .or(config.email_username.as_deref())
        .filter(|from| !from.trim().is_empty())
        .ok_or(MailOutboxError::InvalidSender)?;
    let sender = format!("{} <{}>", config.app_name, from);
    validate_mail_sender(&sender)?;
    let subject = match kind {
        ReminderKind::Expire => {
            format!("The service in {} is about to expire", config.app_name)
        }
        ReminderKind::Traffic => {
            format!("The traffic usage in {} has reached 95%", config.app_name)
        }
    };
    let envelope = PreparedMailEnvelope {
        sender,
        template_name: format!(
            "mail.{}.{}",
            config.email_template.as_deref().unwrap_or("default"),
            kind.template_name()
        ),
        subject,
        body: render_reminder(
            kind,
            &config.app_name,
            config.app_url.as_deref().unwrap_or_default(),
        ),
    };
    let validation_batch = mail_batch_key("worker:reminder:validation", kind.template_name());
    validate_prepared_mail_delivery(&validation_batch, &envelope, "mail-validator@v2board.local")?;
    Ok(envelope)
}

fn reminder_identity(
    user_id: i64,
    kind: ReminderKind,
    business_day: NaiveDate,
) -> ReminderIdentity {
    let actor = reminder_actor(kind);
    let occurrence = format!("{user_id}:{business_day}");
    let batch_key = mail_batch_key(&actor, &occurrence);
    ReminderIdentity { batch_key }
}

fn reminder_actor(kind: ReminderKind) -> String {
    format!("worker:reminder:{}", kind.template_name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2board_domain::mail::outbox::mail_message_id;

    #[test]
    fn reminder_queries_select_only_eligible_users_with_wide_math() {
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("remind_expire"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("expired_at > $1"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("- 86400"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("DECIMAL(65, 0)"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("id > $3"));
        assert!(EXPIRE_REMINDER_CANDIDATES_SQL.contains("LIMIT $4"));
        assert!(!EXPIRE_REMINDER_CANDIDATES_SQL.contains(" u,"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("remind_traffic"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("expired_at >= $1"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("DECIMAL(65, 0)"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("* 95"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("< CAST(transfer_enable"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("id > $2"));
        assert!(TRAFFIC_REMINDER_CANDIDATES_SQL.contains("LIMIT $3"));
    }

    #[test]
    fn reminder_identity_is_stable_per_user_kind_and_business_day() {
        let day = NaiveDate::from_ymd_opt(2026, 7, 11).unwrap();
        let first = reminder_identity(42, ReminderKind::Expire, day);
        let retry = reminder_identity(42, ReminderKind::Expire, day);
        assert_eq!(first, retry);
        assert_eq!(
            mail_message_id(&first.batch_key, "USER@example.test"),
            mail_message_id(&retry.batch_key, "user@example.test")
        );
        assert_ne!(first, reminder_identity(43, ReminderKind::Expire, day));
        assert_ne!(first, reminder_identity(42, ReminderKind::Traffic, day));
        assert_ne!(
            first,
            reminder_identity(42, ReminderKind::Expire, day.succ_opt().unwrap())
        );
    }

    #[test]
    fn reminders_only_enqueue_and_leave_delivery_to_the_outbox_worker() {
        let source = include_str!("reminders.rs");
        let bulk_enqueue = concat!("enqueue_mail_", "outbox_occurrences");
        assert_eq!(source.matches(bulk_enqueue).count(), 2);
        assert!(!source.contains(concat!("reserve_mail_", "outbox_batch")));
        assert!(!source.contains(concat!("enqueue_prepared_", "mail")));
        assert!(!source.contains(concat!("LAST_SEND_EMAIL_", "REMIND_TRAFFIC")));
        assert!(!source.contains(concat!("send_email", "_inner")));
        assert!(!source.contains(concat!("mail", "_log")));
        assert!(!source.contains(concat!("Async", "Transport")));
        assert!(!source.contains(concat!("tokio::time", "::sleep")));
        assert!(!source.contains(concat!("set", "_ex")));

        let loop_start = source
            .find(concat!("for user in ", "users {"))
            .expect("candidate validation loop");
        let bulk_start = source[loop_start..]
            .find(concat!("let result =", "\n            enqueue_mail_"))
            .map(|offset| loop_start + offset)
            .expect("bulk enqueue after candidate validation");
        assert!(!source[loop_start..bulk_start].contains(".await"));

        let page_start = source
            .find(concat!("async fn enqueue_", "reminder_kind"))
            .expect("paged reminder producer");
        let page_end = source[page_start..]
            .find(concat!("async fn select_", "reminder_candidates"))
            .map(|offset| page_start + offset)
            .expect("candidate selector after producer");
        let producer = &source[page_start..page_end];
        assert!(producer.contains(concat!("state.db.", "begin().await")));
        assert!(producer.contains(concat!("tx.", "commit().await")));
        assert!(producer.contains(concat!("after_user_id = ", "last_user_id")));
    }
}
