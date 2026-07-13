use std::collections::HashMap;

use chrono::NaiveDate;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::AnalyticsEvent;
use crate::admission::{AnalyticsAdmissionError, admit_analytics_rows, release_terminal_rows};

const INSERT_SETTINGS: &str = concat!(
    "async_insert=0\n",
    "wait_end_of_query=1\n",
    "insert_deduplication_token=batch_id\n",
    "non_replicated_deduplication_window=10000\n",
    "clickhouse_lts=26.3\n",
    "single_event_table=1\n",
    "single_partition_month=1\n",
    "stable_outbox_order=1\n",
);
const ENQUEUE_CHUNK_ROWS: usize = 1_000;
const MAX_ENQUEUE_ROWS: usize = 100_000;
const MAX_PRUNE_ROWS: i64 = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryBatchState {
    Ready,
    Publishing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxRecord {
    pub outbox_id: i64,
    pub batch_row_number: u32,
    pub event: AnalyticsEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedBatch {
    pub batch_id: Uuid,
    pub event_name: String,
    pub schema_major: i16,
    pub partition_month: NaiveDate,
    pub table_generation: i32,
    pub content_sha256: String,
    pub insert_settings_sha256: String,
    pub lease_owner: Uuid,
    pub lease_expires_at: i64,
    pub created_at: i64,
    pub state: DeliveryBatchState,
    pub rows: Vec<OutboxRecord>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PruneResult {
    pub outbox_rows: u64,
    pub delivery_batches: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutboxBacklog {
    pub pending_rows: i64,
    pub oldest_pending_created_at: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error(transparent)]
    Admission(#[from] AnalyticsAdmissionError),
    #[error("analytics outbox database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("analytics event id {event_id} already exists with different immutable content")]
    EventConflict { event_id: String },
    #[error("analytics batch size must be between 1 and 100000")]
    InvalidBatchSize,
    #[error("analytics lease duration must be between 1 and 3600 seconds")]
    InvalidLease,
    #[error("analytics outbox contains an invalid partition month")]
    InvalidPartitionMonth,
    #[error("analytics batch {batch_id} manifest is inconsistent with its rows")]
    ManifestConflict { batch_id: Uuid },
    #[error("analytics batch {batch_id} lease is no longer owned by this relay")]
    LeaseLost { batch_id: Uuid },
    #[error("analytics batch row count exceeds the supported range")]
    RowCountOverflow,
    #[error("analytics outbox prune limit must be between 1 and 100000")]
    InvalidPruneLimit,
}

#[derive(Debug, FromRow)]
struct BatchRow {
    batch_id: Uuid,
    event_name: String,
    schema_major: i16,
    partition_month: NaiveDate,
    table_generation: i32,
    content_sha256: String,
    insert_settings_sha256: String,
    row_count: i32,
    lease_expires_at: i64,
    created_at: i64,
    state: String,
}

#[derive(Debug, FromRow)]
struct EventRow {
    outbox_id: i64,
    event_id: String,
    event_name: String,
    schema_major: i16,
    report_key: String,
    partition_month: NaiveDate,
    occurred_at: i64,
    payload: serde_json::Value,
    payload_sha256: String,
    batch_row_number: i32,
}

#[derive(Debug, FromRow)]
struct UnclaimedRow {
    outbox_id: i64,
    event_id: String,
    payload_sha256: String,
}

#[derive(Debug, FromRow)]
struct ExistingEventRow {
    event_id: String,
    event_name: String,
    schema_major: i16,
    report_key: String,
    partition_month: NaiveDate,
    occurred_at: i64,
    payload: serde_json::Value,
    payload_sha256: String,
    table_generation: i32,
}

#[derive(Debug, FromRow)]
struct BacklogRow {
    pending_rows: i64,
    oldest_pending_created_at: Option<i64>,
}

/// Insert an immutable event in the caller's existing PostgreSQL transaction.
/// A deterministic id may be retried, but it may never be reused for different
/// content.
pub async fn enqueue_event(
    tx: &mut Transaction<'_, Postgres>,
    event: &AnalyticsEvent,
    created_at: i64,
) -> Result<(), OutboxError> {
    enqueue_events(tx, std::slice::from_ref(event), created_at).await
}

/// Insert immutable events in bounded multi-value statements, then verify
/// every identity (including rows skipped by `ON CONFLICT`) against its full
/// PostgreSQL envelope. This keeps a large traffic report to a small, fixed
/// number of round trips without weakening deterministic retry conflicts.
pub async fn enqueue_events(
    tx: &mut Transaction<'_, Postgres>,
    events: &[AnalyticsEvent],
    created_at: i64,
) -> Result<(), OutboxError> {
    if events.len() > MAX_ENQUEUE_ROWS {
        return Err(OutboxError::InvalidBatchSize);
    }
    if events.is_empty() {
        return Ok(());
    }

    let mut positions = HashMap::<&str, usize>::with_capacity(events.len());
    let mut unique = Vec::<(&AnalyticsEvent, NaiveDate)>::with_capacity(events.len());
    for event in events {
        let partition_month = NaiveDate::parse_from_str(&event.partition_month, "%Y-%m-%d")
            .map_err(|_| OutboxError::InvalidPartitionMonth)?;
        if let Some(position) = positions.get(event.event_id.as_str()) {
            let (existing, existing_month) = unique[*position];
            if !same_immutable_event(existing, existing_month, event, partition_month) {
                return Err(OutboxError::EventConflict {
                    event_id: event.event_id.clone(),
                });
            }
        } else {
            positions.insert(event.event_id.as_str(), unique.len());
            unique.push((event, partition_month));
        }
    }

    let mut inserted_rows = 0_u64;
    for chunk in unique.chunks(ENQUEUE_CHUNK_ROWS) {
        let mut insert = QueryBuilder::<Postgres>::new(
            "INSERT INTO v2_analytics_outbox \
             (event_id, event_name, schema_major, report_key, partition_month, \
              occurred_at, payload, payload_sha256, table_generation, created_at) ",
        );
        insert.push_values(chunk, |mut row, (event, partition_month)| {
            row.push_bind(&event.event_id)
                .push_bind(&event.event_name)
                .push_bind(event.schema_major)
                .push_bind(&event.report_key)
                .push_bind(*partition_month)
                .push_bind(event.occurred_at)
                .push_bind(&event.payload)
                .push_bind(&event.payload_sha256)
                .push_bind(1_i32)
                .push_bind(created_at);
        });
        insert.push(" ON CONFLICT (event_id) DO NOTHING");
        inserted_rows = inserted_rows
            .checked_add(insert.build().execute(&mut **tx).await?.rows_affected())
            .ok_or(OutboxError::RowCountOverflow)?;

        let ids = chunk
            .iter()
            .map(|(event, _)| event.event_id.clone())
            .collect::<Vec<_>>();
        let stored = sqlx::query_as::<_, ExistingEventRow>(
            r#"
            SELECT event_id, event_name, schema_major, report_key,
                   partition_month, occurred_at, payload, payload_sha256,
                   table_generation
            FROM v2_analytics_outbox
            WHERE event_id = ANY($1)
            FOR SHARE
            "#,
        )
        .bind(&ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|row| (row.event_id.clone(), row))
        .collect::<HashMap<_, _>>();

        for (event, partition_month) in chunk {
            let matches = stored.get(&event.event_id).is_some_and(|row| {
                row.event_name == event.event_name
                    && row.schema_major == event.schema_major
                    && row.report_key == event.report_key
                    && row.partition_month == *partition_month
                    && row.occurred_at == event.occurred_at
                    && row.payload == event.payload
                    && row.payload_sha256 == event.payload_sha256
                    && row.table_generation == 1
            });
            if !matches {
                return Err(OutboxError::EventConflict {
                    event_id: event.event_id.clone(),
                });
            }
        }
    }
    let inserted_rows =
        usize::try_from(inserted_rows).map_err(|_| OutboxError::RowCountOverflow)?;
    admit_analytics_rows(tx, inserted_rows, created_at).await?;
    Ok(())
}

fn same_immutable_event(
    first: &AnalyticsEvent,
    first_month: NaiveDate,
    second: &AnalyticsEvent,
    second_month: NaiveDate,
) -> bool {
    first.event_name == second.event_name
        && first.schema_major == second.schema_major
        && first.report_key == second.report_key
        && first_month == second_month
        && first.occurred_at == second.occurred_at
        && first.payload == second.payload
        && first.payload_sha256 == second.payload_sha256
}

/// Claim one immutable, single-table, single-month delivery batch.
pub async fn claim_delivery_batch(
    pool: &sqlx::PgPool,
    lease_owner: Uuid,
    now_unix: i64,
    lease_seconds: i64,
    max_rows: i64,
) -> Result<Option<ClaimedBatch>, OutboxError> {
    if !(1..=100_000).contains(&max_rows) {
        return Err(OutboxError::InvalidBatchSize);
    }
    if !(1..=3_600).contains(&lease_seconds) {
        return Err(OutboxError::InvalidLease);
    }
    let lease_expires_at = now_unix.saturating_add(lease_seconds);
    let mut tx = pool.begin().await?;

    if let Some(batch) =
        claim_existing_batch(&mut tx, lease_owner, now_unix, lease_expires_at).await?
    {
        tx.commit().await?;
        return Ok(Some(batch));
    }

    let seed = sqlx::query_as::<_, (String, i16, NaiveDate, i32)>(
        r#"
        SELECT event_name, schema_major, partition_month, table_generation
        FROM v2_analytics_outbox
        WHERE published_at IS NULL
          AND quarantined_at IS NULL
          AND delivery_batch_id IS NULL
        ORDER BY outbox_id
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some((event_name, schema_major, partition_month, table_generation)) = seed else {
        tx.commit().await?;
        return Ok(None);
    };

    let rows = sqlx::query_as::<_, UnclaimedRow>(
        r#"
        SELECT outbox_id, event_id, payload_sha256
        FROM v2_analytics_outbox
        WHERE published_at IS NULL
          AND quarantined_at IS NULL
          AND delivery_batch_id IS NULL
          AND event_name = $1
          AND schema_major = $2
          AND partition_month = $3
          AND table_generation = $4
        ORDER BY outbox_id
        LIMIT $5
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(&event_name)
    .bind(schema_major)
    .bind(partition_month)
    .bind(table_generation)
    .bind(max_rows)
    .fetch_all(&mut *tx)
    .await?;
    if rows.is_empty() {
        tx.commit().await?;
        return Ok(None);
    }

    let batch_id = Uuid::new_v4();
    let content_sha256 = batch_content_hash(rows.iter().enumerate().map(|(index, row)| {
        (
            index as u32,
            row.event_id.as_str(),
            row.payload_sha256.as_str(),
        )
    }));
    let insert_settings_sha256 = settings_hash();
    let row_count = i32::try_from(rows.len()).map_err(|_| OutboxError::RowCountOverflow)?;
    sqlx::query(
        r#"
        INSERT INTO v2_analytics_delivery_batch
            (batch_id, event_name, schema_major, partition_month, table_generation,
             row_count, content_sha256, insert_settings_sha256, state,
             lease_owner, lease_expires_at, attempt_count, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'publishing', $9, $10, 1, $11)
        "#,
    )
    .bind(batch_id)
    .bind(&event_name)
    .bind(schema_major)
    .bind(partition_month)
    .bind(table_generation)
    .bind(row_count)
    .bind(&content_sha256)
    .bind(&insert_settings_sha256)
    .bind(lease_owner)
    .bind(lease_expires_at)
    .bind(now_unix)
    .execute(&mut *tx)
    .await?;

    let ids = rows.iter().map(|row| row.outbox_id).collect::<Vec<_>>();
    let row_numbers = (0..row_count).collect::<Vec<_>>();
    let assigned = sqlx::query(
        r#"
        UPDATE v2_analytics_outbox AS outbox
        SET delivery_batch_id = $1, batch_row_number = assignments.row_number
        FROM UNNEST($2::BIGINT[], $3::INTEGER[])
             AS assignments(outbox_id, row_number)
        WHERE outbox.outbox_id = assignments.outbox_id
          AND outbox.delivery_batch_id IS NULL
          AND outbox.published_at IS NULL
          AND outbox.quarantined_at IS NULL
        "#,
    )
    .bind(batch_id)
    .bind(&ids)
    .bind(&row_numbers)
    .execute(&mut *tx)
    .await?;
    if assigned.rows_affected() != rows.len() as u64 {
        return Err(OutboxError::ManifestConflict { batch_id });
    }

    let batch = load_claimed_batch(&mut tx, batch_id, lease_owner).await?;
    tx.commit().await?;
    Ok(Some(batch))
}

async fn claim_existing_batch(
    tx: &mut Transaction<'_, Postgres>,
    lease_owner: Uuid,
    now_unix: i64,
    lease_expires_at: i64,
) -> Result<Option<ClaimedBatch>, OutboxError> {
    let batch_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT batch_id
        FROM v2_analytics_delivery_batch
        WHERE published_at IS NULL
          AND quarantined_at IS NULL
          AND state IN ('ready', 'publishing')
          AND (lease_expires_at IS NULL OR lease_expires_at < $1)
        ORDER BY created_at, batch_id
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(now_unix)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(batch_id) = batch_id else {
        return Ok(None);
    };
    sqlx::query(
        r#"
        UPDATE v2_analytics_delivery_batch
        SET state = 'publishing', lease_owner = $1, lease_expires_at = $2,
            attempt_count = attempt_count + 1
        WHERE batch_id = $3
        "#,
    )
    .bind(lease_owner)
    .bind(lease_expires_at)
    .bind(batch_id)
    .execute(&mut **tx)
    .await?;
    load_claimed_batch(tx, batch_id, lease_owner)
        .await
        .map(Some)
}

async fn load_claimed_batch(
    tx: &mut Transaction<'_, Postgres>,
    batch_id: Uuid,
    lease_owner: Uuid,
) -> Result<ClaimedBatch, OutboxError> {
    let batch = sqlx::query_as::<_, BatchRow>(
        r#"
        SELECT batch_id, event_name, schema_major, partition_month,
               table_generation, content_sha256, insert_settings_sha256, row_count,
               lease_expires_at, created_at, state
        FROM v2_analytics_delivery_batch
        WHERE batch_id = $1 AND lease_owner = $2
        "#,
    )
    .bind(batch_id)
    .bind(lease_owner)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(OutboxError::LeaseLost { batch_id })?;
    let event_rows = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT outbox_id, event_id, event_name, schema_major, report_key,
               partition_month, occurred_at, payload, payload_sha256,
               batch_row_number
        FROM v2_analytics_outbox
        WHERE delivery_batch_id = $1
        ORDER BY batch_row_number
        "#,
    )
    .bind(batch_id)
    .fetch_all(&mut **tx)
    .await?;
    let rows = event_rows
        .into_iter()
        .map(|row| {
            let batch_row_number = u32::try_from(row.batch_row_number)
                .map_err(|_| OutboxError::ManifestConflict { batch_id })?;
            Ok(OutboxRecord {
                outbox_id: row.outbox_id,
                batch_row_number,
                event: AnalyticsEvent {
                    event_id: row.event_id,
                    event_name: row.event_name,
                    schema_major: row.schema_major,
                    report_key: row.report_key,
                    partition_month: row.partition_month.format("%Y-%m-%d").to_string(),
                    occurred_at: row.occurred_at,
                    payload: row.payload,
                    payload_sha256: row.payload_sha256,
                },
            })
        })
        .collect::<Result<Vec<_>, OutboxError>>()?;
    let actual_hash = batch_content_hash(rows.iter().map(|row| {
        (
            row.batch_row_number,
            row.event.event_id.as_str(),
            row.event.payload_sha256.as_str(),
        )
    }));
    if rows.is_empty()
        || usize::try_from(batch.row_count).ok() != Some(rows.len())
        || rows
            .iter()
            .enumerate()
            .any(|(expected, row)| row.batch_row_number != expected as u32)
        || actual_hash != batch.content_sha256
        || batch.insert_settings_sha256 != settings_hash()
        || rows.iter().any(|row| {
            row.event.event_name != batch.event_name
                || row.event.schema_major != batch.schema_major
                || row.event.partition_month != batch.partition_month.format("%Y-%m-%d").to_string()
        })
    {
        return Err(OutboxError::ManifestConflict { batch_id });
    }
    let state = match batch.state.as_str() {
        "ready" => DeliveryBatchState::Ready,
        "publishing" => DeliveryBatchState::Publishing,
        _ => return Err(OutboxError::ManifestConflict { batch_id }),
    };
    Ok(ClaimedBatch {
        batch_id: batch.batch_id,
        event_name: batch.event_name,
        schema_major: batch.schema_major,
        partition_month: batch.partition_month,
        table_generation: batch.table_generation,
        content_sha256: batch.content_sha256,
        insert_settings_sha256: batch.insert_settings_sha256,
        lease_owner,
        lease_expires_at: batch.lease_expires_at,
        created_at: batch.created_at,
        state,
        rows,
    })
}

pub async fn mark_batch_published(
    pool: &sqlx::PgPool,
    batch: &ClaimedBatch,
    published_at: i64,
) -> Result<(), OutboxError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        r#"
        UPDATE v2_analytics_delivery_batch
        SET state = 'published', published_at = $1,
            lease_owner = NULL, lease_expires_at = NULL, last_error = NULL
        WHERE batch_id = $2 AND lease_owner = $3
          AND state = 'publishing' AND published_at IS NULL
          AND quarantined_at IS NULL
        "#,
    )
    .bind(published_at)
    .bind(batch.batch_id)
    .bind(batch.lease_owner)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(OutboxError::LeaseLost {
            batch_id: batch.batch_id,
        });
    }
    let events = sqlx::query(
        r#"
        UPDATE v2_analytics_outbox
        SET published_at = $1
        WHERE delivery_batch_id = $2 AND published_at IS NULL
          AND quarantined_at IS NULL
        "#,
    )
    .bind(published_at)
    .bind(batch.batch_id)
    .execute(&mut *tx)
    .await?;
    if events.rows_affected() != batch.rows.len() as u64 {
        return Err(OutboxError::ManifestConflict {
            batch_id: batch.batch_id,
        });
    }
    release_terminal_rows(&mut tx, batch.rows.len()).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn release_batch_for_retry(
    pool: &sqlx::PgPool,
    batch: &ClaimedBatch,
    error: &str,
) -> Result<(), OutboxError> {
    let updated = sqlx::query(
        r#"
        UPDATE v2_analytics_delivery_batch
        SET state = 'ready', lease_owner = NULL, lease_expires_at = NULL,
            last_error = LEFT($1, 2000)
        WHERE batch_id = $2 AND lease_owner = $3
          AND state = 'publishing' AND published_at IS NULL
          AND quarantined_at IS NULL
        "#,
    )
    .bind(error)
    .bind(batch.batch_id)
    .bind(batch.lease_owner)
    .execute(pool)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(OutboxError::LeaseLost {
            batch_id: batch.batch_id,
        });
    }
    Ok(())
}

pub async fn quarantine_batch(
    pool: &sqlx::PgPool,
    batch: &ClaimedBatch,
    quarantined_at: i64,
    reason: &str,
) -> Result<(), OutboxError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        r#"
        UPDATE v2_analytics_delivery_batch
        SET state = 'quarantined', quarantined_at = $1,
            quarantine_reason = LEFT($2, 2000),
            lease_owner = NULL, lease_expires_at = NULL
        WHERE batch_id = $3 AND lease_owner = $4
          AND published_at IS NULL AND quarantined_at IS NULL
        "#,
    )
    .bind(quarantined_at)
    .bind(reason)
    .bind(batch.batch_id)
    .bind(batch.lease_owner)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(OutboxError::LeaseLost {
            batch_id: batch.batch_id,
        });
    }
    let events = sqlx::query(
        r#"
        UPDATE v2_analytics_outbox
        SET quarantined_at = $1
        WHERE delivery_batch_id = $2 AND published_at IS NULL
        "#,
    )
    .bind(quarantined_at)
    .bind(batch.batch_id)
    .execute(&mut *tx)
    .await?;
    if events.rows_affected() != batch.rows.len() as u64 {
        return Err(OutboxError::ManifestConflict {
            batch_id: batch.batch_id,
        });
    }
    release_terminal_rows(&mut tx, batch.rows.len()).await?;
    tx.commit().await?;
    Ok(())
}

/// Return the exact pending backlog. Callers should sample this on a bounded
/// interval: PostgreSQL must scan the pending partial index to produce an exact
/// count, so this is observability rather than a hot-path operation.
pub async fn outbox_backlog(pool: &sqlx::PgPool) -> Result<OutboxBacklog, OutboxError> {
    let row = sqlx::query_as::<_, BacklogRow>(
        r#"
        SELECT COUNT(*)::BIGINT AS pending_rows,
               MIN(created_at) AS oldest_pending_created_at
        FROM v2_analytics_outbox
        WHERE published_at IS NULL AND quarantined_at IS NULL
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(OutboxBacklog {
        pending_rows: row.pending_rows,
        oldest_pending_created_at: row.oldest_pending_created_at,
    })
}

/// Delete a bounded slice of terminal, successfully published history older
/// than `published_before`. Pending and quarantined events are never eligible.
/// Delivery batches are removed only after their final outbox reference is
/// gone, preserving the PostgreSQL foreign-key direction under partial runs.
///
/// The caller must choose a cutoff older than every supported producer retry
/// and replay window. Once an event identity is pruned, PostgreSQL no longer
/// acts as its permanent idempotency tombstone.
pub async fn prune_published_outbox(
    pool: &sqlx::PgPool,
    published_before: i64,
    max_rows: i64,
) -> Result<PruneResult, OutboxError> {
    if !(1..=MAX_PRUNE_ROWS).contains(&max_rows) {
        return Err(OutboxError::InvalidPruneLimit);
    }
    let mut tx = pool.begin().await?;
    let deleted_outbox = sqlx::query(
        r#"
        WITH candidates AS (
            SELECT outbox_id
            FROM v2_analytics_outbox
            WHERE published_at IS NOT NULL
              AND published_at < $1
              AND quarantined_at IS NULL
            ORDER BY published_at, outbox_id
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        DELETE FROM v2_analytics_outbox AS outbox
        USING candidates
        WHERE outbox.outbox_id = candidates.outbox_id
          AND outbox.published_at IS NOT NULL
          AND outbox.published_at < $1
          AND outbox.quarantined_at IS NULL
        "#,
    )
    .bind(published_before)
    .bind(max_rows)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let deleted_batches = sqlx::query(
        r#"
        WITH candidates AS (
            SELECT batch.batch_id
            FROM v2_analytics_delivery_batch AS batch
            WHERE batch.state = 'published'
              AND batch.published_at IS NOT NULL
              AND batch.published_at < $1
              AND NOT EXISTS (
                  SELECT 1
                  FROM v2_analytics_outbox AS outbox
                  WHERE outbox.delivery_batch_id = batch.batch_id
              )
            ORDER BY batch.published_at, batch.batch_id
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        DELETE FROM v2_analytics_delivery_batch AS batch
        USING candidates
        WHERE batch.batch_id = candidates.batch_id
          AND batch.state = 'published'
          AND batch.published_at IS NOT NULL
          AND batch.published_at < $1
          AND NOT EXISTS (
              SELECT 1
              FROM v2_analytics_outbox AS outbox
              WHERE outbox.delivery_batch_id = batch.batch_id
          )
        "#,
    )
    .bind(published_before)
    .bind(max_rows)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(PruneResult {
        outbox_rows: deleted_outbox,
        delivery_batches: deleted_batches,
    })
}

fn batch_content_hash<'a>(rows: impl IntoIterator<Item = (u32, &'a str, &'a str)>) -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.analytics.delivery-batch.v1\0");
    for (row_number, event_id, payload_sha256) in rows {
        digest.update(row_number.to_be_bytes());
        for value in [event_id.as_bytes(), payload_sha256.as_bytes()] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
    }
    hex::encode(digest.finalize())
}

fn settings_hash() -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.analytics.insert-settings.v1\0");
    digest.update((INSERT_SETTINGS.len() as u64).to_be_bytes());
    digest.update(INSERT_SETTINGS.as_bytes());
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_hash_binds_order_and_content() {
        let first = batch_content_hash([(0, "a", "1"), (1, "b", "2")]);
        assert_eq!(first, batch_content_hash([(0, "a", "1"), (1, "b", "2")]));
        assert_ne!(first, batch_content_hash([(0, "b", "2"), (1, "a", "1")]));
        assert_ne!(first, batch_content_hash([(0, "a", "1"), (1, "b", "3")]));
    }
}
