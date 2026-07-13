use chrono::{Datelike, NaiveDate};
use clickhouse::Row;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use uuid::Uuid;

use crate::{
    ACCOUNTED_EVENT_NAME, AccountedOutcome, AccountedTrafficEvent, ClaimedBatch,
    EventValidationError, IdentityKind, REPORTED_EVENT_NAME, ReportedTrafficEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionStatus {
    InsertedAndVerified,
    AlreadyPresentAndVerified,
}

#[derive(Debug, thiserror::Error)]
pub enum BatchProjectionError {
    #[error("ClickHouse request failed: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("analytics event failed validation: {0}")]
    Event(#[from] EventValidationError),
    #[error("analytics batch {batch_id} has unsupported event type {event_name}")]
    UnsupportedEvent { batch_id: Uuid, event_name: String },
    #[error("analytics batch {batch_id} payload does not match its PostgreSQL envelope")]
    PayloadConflict { batch_id: Uuid },
    #[error("analytics batch {batch_id} has partial, duplicate, or conflicting ClickHouse rows")]
    ProjectionConflict { batch_id: Uuid },
    #[error("analytics batch {batch_id} belongs to a different installation")]
    InstallationConflict { batch_id: Uuid },
    #[error("analytics event contains an invalid {field}")]
    InvalidField { field: &'static str },
    #[error("analytics batch {batch_id} daily aggregate exceeds UInt64 or UInt32")]
    AggregateOverflow { batch_id: Uuid },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Row, Serialize)]
struct ReportedRow {
    event_id: String,
    schema_major: u16,
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    report_key: String,
    payload_hash: String,
    identity_kind: String,
    user_id: u64,
    traffic_epoch: u64,
    server_id: u64,
    server_type: String,
    rate_text: String,
    rate_decimal_10_2: i64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
    accepted_at_unix: i64,
    #[serde(with = "clickhouse::serde::chrono::date")]
    accounting_date: NaiveDate,
    accounting_timezone: String,
    #[serde(with = "clickhouse::serde::uuid")]
    ingest_batch_id: Uuid,
    batch_row_number: u32,
    outbox_payload_sha256: String,
    table_generation: u32,
    ingested_at_unix: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Row, Serialize)]
struct AccountedRow {
    event_id: String,
    schema_major: u16,
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    report_key: String,
    payload_hash: String,
    identity_kind: String,
    user_id: u64,
    traffic_epoch: u64,
    server_id: u64,
    server_type: String,
    rate_text: String,
    rate_decimal_10_2: i64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
    accepted_at_unix: i64,
    #[serde(with = "clickhouse::serde::chrono::date")]
    accounting_date: NaiveDate,
    accounting_timezone: String,
    accounted_at_unix: i64,
    outcome: String,
    u_after: Option<u64>,
    d_after: Option<u64>,
    #[serde(with = "clickhouse::serde::uuid")]
    ingest_batch_id: Uuid,
    batch_row_number: u32,
    outbox_payload_sha256: String,
    table_generation: u32,
    ingested_at_unix: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Row, Serialize)]
struct ReportedDailyRow {
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    #[serde(with = "clickhouse::serde::chrono::date")]
    accounting_date: NaiveDate,
    user_id: u64,
    server_id: u64,
    server_type: String,
    rate_text: String,
    rate_decimal_10_2: i64,
    table_generation: u32,
    #[serde(with = "clickhouse::serde::uuid")]
    ingest_batch_id: Uuid,
    batch_aggregate_row_number: u32,
    event_count: u64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Row, Serialize)]
struct AccountedDailyRow {
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    #[serde(with = "clickhouse::serde::chrono::date")]
    accounting_date: NaiveDate,
    user_id: u64,
    server_id: u64,
    server_type: String,
    rate_text: String,
    rate_decimal_10_2: i64,
    outcome: String,
    table_generation: u32,
    #[serde(with = "clickhouse::serde::uuid")]
    ingest_batch_id: Uuid,
    batch_aggregate_row_number: u32,
    event_count: u64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReportedDailyKey {
    installation_id: Uuid,
    accounting_date: NaiveDate,
    user_id: u64,
    server_id: u64,
    server_type: String,
    rate_text: String,
    rate_decimal_10_2: i64,
    table_generation: u32,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AccountedDailyKey {
    common: ReportedDailyKey,
    outcome: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct DailySums {
    event_count: u64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
}

pub async fn project_or_verify_batch(
    client: &clickhouse::Client,
    batch: &ClaimedBatch,
    expected_installation_id: Uuid,
) -> Result<ProjectionStatus, BatchProjectionError> {
    match batch.event_name.as_str() {
        REPORTED_EVENT_NAME => {
            let rows = batch
                .rows
                .iter()
                .map(|record| reported_row(record, batch, expected_installation_id))
                .collect::<Result<Vec<_>, _>>()?;
            let daily_rows = reported_daily_rows(batch, &rows)?;
            let raw_present = verify_exact_projection(
                batch,
                &reported_verification_rows(client, batch).await?,
                &rows,
            )?;
            let daily_present = verify_exact_projection(
                batch,
                &reported_daily_verification_rows(client, batch).await?,
                &daily_rows,
            )?;
            if raw_present && daily_present {
                return Ok(ProjectionStatus::AlreadyPresentAndVerified);
            }
            if !raw_present {
                let mut insert = client
                    .insert::<ReportedRow>("v2_traffic_reported_v1")
                    .await?
                    .with_setting(
                        "insert_deduplication_token",
                        projection_token("reported-raw", batch.batch_id),
                    )
                    .with_setting("async_insert", "0")
                    .with_setting("wait_end_of_query", "1")
                    .with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)));
                for row in &rows {
                    insert.write(row).await?;
                }
                insert.end().await?;
            }
            if !daily_present {
                let mut insert = client
                    .insert::<ReportedDailyRow>("v2_traffic_reported_daily_v1")
                    .await?
                    .with_setting(
                        "insert_deduplication_token",
                        projection_token("reported-daily", batch.batch_id),
                    )
                    .with_setting("async_insert", "0")
                    .with_setting("wait_end_of_query", "1")
                    .with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)));
                for row in &daily_rows {
                    insert.write(row).await?;
                }
                insert.end().await?;
            }
            if !verify_exact_projection(
                batch,
                &reported_verification_rows(client, batch).await?,
                &rows,
            )? || !verify_exact_projection(
                batch,
                &reported_daily_verification_rows(client, batch).await?,
                &daily_rows,
            )? {
                return Err(projection_conflict(batch));
            }
            Ok(ProjectionStatus::InsertedAndVerified)
        }
        ACCOUNTED_EVENT_NAME => {
            let rows = batch
                .rows
                .iter()
                .map(|record| accounted_row(record, batch, expected_installation_id))
                .collect::<Result<Vec<_>, _>>()?;
            let daily_rows = accounted_daily_rows(batch, &rows)?;
            let raw_present = verify_exact_projection(
                batch,
                &accounted_verification_rows(client, batch).await?,
                &rows,
            )?;
            let daily_present = verify_exact_projection(
                batch,
                &accounted_daily_verification_rows(client, batch).await?,
                &daily_rows,
            )?;
            if raw_present && daily_present {
                return Ok(ProjectionStatus::AlreadyPresentAndVerified);
            }
            if !raw_present {
                let mut insert = client
                    .insert::<AccountedRow>("v2_traffic_accounted_v1")
                    .await?
                    .with_setting(
                        "insert_deduplication_token",
                        projection_token("accounted-raw", batch.batch_id),
                    )
                    .with_setting("async_insert", "0")
                    .with_setting("wait_end_of_query", "1")
                    .with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)));
                for row in &rows {
                    insert.write(row).await?;
                }
                insert.end().await?;
            }
            if !daily_present {
                let mut insert = client
                    .insert::<AccountedDailyRow>("v2_traffic_accounted_daily_v1")
                    .await?
                    .with_setting(
                        "insert_deduplication_token",
                        projection_token("accounted-daily", batch.batch_id),
                    )
                    .with_setting("async_insert", "0")
                    .with_setting("wait_end_of_query", "1")
                    .with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)));
                for row in &daily_rows {
                    insert.write(row).await?;
                }
                insert.end().await?;
            }
            if !verify_exact_projection(
                batch,
                &accounted_verification_rows(client, batch).await?,
                &rows,
            )? || !verify_exact_projection(
                batch,
                &accounted_daily_verification_rows(client, batch).await?,
                &daily_rows,
            )? {
                return Err(projection_conflict(batch));
            }
            Ok(ProjectionStatus::InsertedAndVerified)
        }
        _ => Err(BatchProjectionError::UnsupportedEvent {
            batch_id: batch.batch_id,
            event_name: batch.event_name.clone(),
        }),
    }
}

fn verify_exact_projection<T: PartialEq>(
    batch: &ClaimedBatch,
    stored: &[T],
    expected: &[T],
) -> Result<bool, BatchProjectionError> {
    if stored.is_empty() {
        return Ok(false);
    }
    if stored != expected {
        return Err(projection_conflict(batch));
    }
    Ok(true)
}

fn projection_conflict(batch: &ClaimedBatch) -> BatchProjectionError {
    BatchProjectionError::ProjectionConflict {
        batch_id: batch.batch_id,
    }
}

async fn reported_verification_rows(
    client: &clickhouse::Client,
    batch: &ClaimedBatch,
) -> Result<Vec<ReportedRow>, clickhouse::error::Error> {
    let (month_start, month_end) = verification_month_bounds(batch);
    client
        .query(
            "SELECT event_id, schema_major, installation_id, report_key, payload_hash, \
                    identity_kind, user_id, traffic_epoch, server_id, server_type, \
                    rate_text, rate_decimal_10_2, raw_u, raw_d, charged_u, charged_d, \
                    accepted_at_unix, accounting_date, accounting_timezone, ingest_batch_id, \
                    batch_row_number, outbox_payload_sha256, table_generation, ingested_at_unix \
             FROM v2_traffic_reported_v1 \
             WHERE table_generation = ? \
               AND accounting_date >= toDate(?) AND accounting_date < toDate(?) \
               AND ingest_batch_id = toUUID(?) \
             ORDER BY batch_row_number",
        )
        .bind(batch.table_generation)
        .bind(month_start)
        .bind(month_end)
        .bind(batch.batch_id.to_string())
        .fetch_all::<ReportedRow>()
        .await
}

async fn accounted_verification_rows(
    client: &clickhouse::Client,
    batch: &ClaimedBatch,
) -> Result<Vec<AccountedRow>, clickhouse::error::Error> {
    let (month_start, month_end) = verification_month_bounds(batch);
    client
        .query(
            "SELECT event_id, schema_major, installation_id, report_key, payload_hash, \
                    identity_kind, user_id, traffic_epoch, server_id, server_type, \
                    rate_text, rate_decimal_10_2, raw_u, raw_d, charged_u, charged_d, \
                    accepted_at_unix, accounting_date, accounting_timezone, accounted_at_unix, \
                    outcome, u_after, d_after, ingest_batch_id, batch_row_number, \
                    outbox_payload_sha256, table_generation, ingested_at_unix \
             FROM v2_traffic_accounted_v1 \
             WHERE table_generation = ? \
               AND accounting_date >= toDate(?) AND accounting_date < toDate(?) \
               AND ingest_batch_id = toUUID(?) \
             ORDER BY batch_row_number",
        )
        .bind(batch.table_generation)
        .bind(month_start)
        .bind(month_end)
        .bind(batch.batch_id.to_string())
        .fetch_all::<AccountedRow>()
        .await
}

async fn reported_daily_verification_rows(
    client: &clickhouse::Client,
    batch: &ClaimedBatch,
) -> Result<Vec<ReportedDailyRow>, clickhouse::error::Error> {
    let (month_start, month_end) = verification_month_bounds(batch);
    client
        .query(
            "SELECT installation_id, accounting_date, user_id, server_id, server_type, \
                    rate_text, rate_decimal_10_2, table_generation, ingest_batch_id, \
                    batch_aggregate_row_number, event_count, raw_u, raw_d, charged_u, charged_d \
             FROM v2_traffic_reported_daily_v1 \
             WHERE table_generation = ? \
               AND accounting_date >= toDate(?) AND accounting_date < toDate(?) \
               AND ingest_batch_id = toUUID(?) \
             ORDER BY batch_aggregate_row_number",
        )
        .bind(batch.table_generation)
        .bind(month_start)
        .bind(month_end)
        .bind(batch.batch_id.to_string())
        .fetch_all::<ReportedDailyRow>()
        .await
}

async fn accounted_daily_verification_rows(
    client: &clickhouse::Client,
    batch: &ClaimedBatch,
) -> Result<Vec<AccountedDailyRow>, clickhouse::error::Error> {
    let (month_start, month_end) = verification_month_bounds(batch);
    client
        .query(
            "SELECT installation_id, accounting_date, user_id, server_id, server_type, \
                    rate_text, rate_decimal_10_2, outcome, table_generation, ingest_batch_id, \
                    batch_aggregate_row_number, event_count, raw_u, raw_d, charged_u, charged_d \
             FROM v2_traffic_accounted_daily_v1 \
             WHERE table_generation = ? \
               AND accounting_date >= toDate(?) AND accounting_date < toDate(?) \
               AND ingest_batch_id = toUUID(?) \
             ORDER BY batch_aggregate_row_number",
        )
        .bind(batch.table_generation)
        .bind(month_start)
        .bind(month_end)
        .bind(batch.batch_id.to_string())
        .fetch_all::<AccountedDailyRow>()
        .await
}

fn verification_month_bounds(batch: &ClaimedBatch) -> (String, String) {
    (
        batch.partition_month.format("%Y-%m-%d").to_string(),
        next_month(batch.partition_month)
            .format("%Y-%m-%d")
            .to_string(),
    )
}

fn next_month(month: NaiveDate) -> NaiveDate {
    let (year, month_number) = if month.month() == 12 {
        (month.year() + 1, 1)
    } else {
        (month.year(), month.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month_number, 1).unwrap_or(NaiveDate::MAX)
}

fn reported_row(
    record: &crate::OutboxRecord,
    batch: &ClaimedBatch,
    expected_installation_id: Uuid,
) -> Result<ReportedRow, BatchProjectionError> {
    let event: ReportedTrafficEvent = serde_json::from_value(record.event.payload.clone())
        .map_err(|_| BatchProjectionError::PayloadConflict {
            batch_id: batch.batch_id,
        })?;
    if event.clone().into_outbox()? != record.event {
        return Err(BatchProjectionError::PayloadConflict {
            batch_id: batch.batch_id,
        });
    }
    let core = event.core;
    let installation_id = parse_uuid(&core.installation_id, "installation_id")?;
    if installation_id != expected_installation_id {
        return Err(BatchProjectionError::InstallationConflict {
            batch_id: batch.batch_id,
        });
    }
    Ok(ReportedRow {
        event_id: event.event_id,
        schema_major: u16::try_from(event.schema_major).map_err(|_| invalid("schema_major"))?,
        installation_id,
        report_key: core.report_key,
        payload_hash: core.payload_hash,
        identity_kind: identity_kind(core.identity_kind).into(),
        user_id: parse_u64(&core.user_id, "user_id")?,
        traffic_epoch: parse_u64(&core.traffic_epoch, "traffic_epoch")?,
        server_id: parse_u64(&core.server_id, "server_id")?,
        server_type: core.server_type,
        rate_text: core.rate_text,
        rate_decimal_10_2: decimal_10_2(&core.rate_decimal_10_2)?,
        raw_u: parse_u64(&core.raw_u, "raw_u")?,
        raw_d: parse_u64(&core.raw_d, "raw_d")?,
        charged_u: parse_u64(&core.charged_u, "charged_u")?,
        charged_d: parse_u64(&core.charged_d, "charged_d")?,
        accepted_at_unix: core.accepted_at,
        accounting_date: parse_date(&core.accounting_date)?,
        accounting_timezone: core.accounting_timezone,
        ingest_batch_id: batch.batch_id,
        batch_row_number: record.batch_row_number,
        outbox_payload_sha256: record.event.payload_sha256.clone(),
        table_generation: u32::try_from(batch.table_generation)
            .map_err(|_| invalid("table_generation"))?,
        ingested_at_unix: batch.created_at,
    })
}

fn accounted_row(
    record: &crate::OutboxRecord,
    batch: &ClaimedBatch,
    expected_installation_id: Uuid,
) -> Result<AccountedRow, BatchProjectionError> {
    let event: AccountedTrafficEvent = serde_json::from_value(record.event.payload.clone())
        .map_err(|_| BatchProjectionError::PayloadConflict {
            batch_id: batch.batch_id,
        })?;
    if event.clone().into_outbox()? != record.event {
        return Err(BatchProjectionError::PayloadConflict {
            batch_id: batch.batch_id,
        });
    }
    let core = event.core;
    let installation_id = parse_uuid(&core.installation_id, "installation_id")?;
    if installation_id != expected_installation_id {
        return Err(BatchProjectionError::InstallationConflict {
            batch_id: batch.batch_id,
        });
    }
    Ok(AccountedRow {
        event_id: event.event_id,
        schema_major: u16::try_from(event.schema_major).map_err(|_| invalid("schema_major"))?,
        installation_id,
        report_key: core.report_key,
        payload_hash: core.payload_hash,
        identity_kind: identity_kind(core.identity_kind).into(),
        user_id: parse_u64(&core.user_id, "user_id")?,
        traffic_epoch: parse_u64(&core.traffic_epoch, "traffic_epoch")?,
        server_id: parse_u64(&core.server_id, "server_id")?,
        server_type: core.server_type,
        rate_text: core.rate_text,
        rate_decimal_10_2: decimal_10_2(&core.rate_decimal_10_2)?,
        raw_u: parse_u64(&core.raw_u, "raw_u")?,
        raw_d: parse_u64(&core.raw_d, "raw_d")?,
        charged_u: parse_u64(&core.charged_u, "charged_u")?,
        charged_d: parse_u64(&core.charged_d, "charged_d")?,
        accepted_at_unix: core.accepted_at,
        accounting_date: parse_date(&core.accounting_date)?,
        accounting_timezone: core.accounting_timezone,
        accounted_at_unix: event.accounted_at,
        outcome: outcome(event.outcome).into(),
        u_after: event
            .u_after
            .as_deref()
            .map(|value| parse_u64(value, "u_after"))
            .transpose()?,
        d_after: event
            .d_after
            .as_deref()
            .map(|value| parse_u64(value, "d_after"))
            .transpose()?,
        ingest_batch_id: batch.batch_id,
        batch_row_number: record.batch_row_number,
        outbox_payload_sha256: record.event.payload_sha256.clone(),
        table_generation: u32::try_from(batch.table_generation)
            .map_err(|_| invalid("table_generation"))?,
        ingested_at_unix: batch.created_at,
    })
}

fn reported_daily_rows(
    batch: &ClaimedBatch,
    rows: &[ReportedRow],
) -> Result<Vec<ReportedDailyRow>, BatchProjectionError> {
    let mut grouped = BTreeMap::<ReportedDailyKey, DailySums>::new();
    for row in rows {
        let key = ReportedDailyKey {
            installation_id: row.installation_id,
            accounting_date: row.accounting_date,
            user_id: row.user_id,
            server_id: row.server_id,
            server_type: row.server_type.clone(),
            rate_text: row.rate_text.clone(),
            rate_decimal_10_2: row.rate_decimal_10_2,
            table_generation: row.table_generation,
        };
        add_daily_sums(
            grouped.entry(key).or_default(),
            row.raw_u,
            row.raw_d,
            row.charged_u,
            row.charged_d,
            batch.batch_id,
        )?;
    }
    grouped
        .into_iter()
        .enumerate()
        .map(|(index, (key, sums))| {
            Ok(ReportedDailyRow {
                installation_id: key.installation_id,
                accounting_date: key.accounting_date,
                user_id: key.user_id,
                server_id: key.server_id,
                server_type: key.server_type,
                rate_text: key.rate_text,
                rate_decimal_10_2: key.rate_decimal_10_2,
                table_generation: key.table_generation,
                ingest_batch_id: batch.batch_id,
                batch_aggregate_row_number: u32::try_from(index)
                    .map_err(|_| aggregate_overflow(batch.batch_id))?,
                event_count: sums.event_count,
                raw_u: sums.raw_u,
                raw_d: sums.raw_d,
                charged_u: sums.charged_u,
                charged_d: sums.charged_d,
            })
        })
        .collect()
}

fn accounted_daily_rows(
    batch: &ClaimedBatch,
    rows: &[AccountedRow],
) -> Result<Vec<AccountedDailyRow>, BatchProjectionError> {
    let mut grouped = BTreeMap::<AccountedDailyKey, DailySums>::new();
    for row in rows {
        let key = AccountedDailyKey {
            common: ReportedDailyKey {
                installation_id: row.installation_id,
                accounting_date: row.accounting_date,
                user_id: row.user_id,
                server_id: row.server_id,
                server_type: row.server_type.clone(),
                rate_text: row.rate_text.clone(),
                rate_decimal_10_2: row.rate_decimal_10_2,
                table_generation: row.table_generation,
            },
            outcome: row.outcome.clone(),
        };
        add_daily_sums(
            grouped.entry(key).or_default(),
            row.raw_u,
            row.raw_d,
            row.charged_u,
            row.charged_d,
            batch.batch_id,
        )?;
    }
    grouped
        .into_iter()
        .enumerate()
        .map(|(index, (key, sums))| {
            Ok(AccountedDailyRow {
                installation_id: key.common.installation_id,
                accounting_date: key.common.accounting_date,
                user_id: key.common.user_id,
                server_id: key.common.server_id,
                server_type: key.common.server_type,
                rate_text: key.common.rate_text,
                rate_decimal_10_2: key.common.rate_decimal_10_2,
                outcome: key.outcome,
                table_generation: key.common.table_generation,
                ingest_batch_id: batch.batch_id,
                batch_aggregate_row_number: u32::try_from(index)
                    .map_err(|_| aggregate_overflow(batch.batch_id))?,
                event_count: sums.event_count,
                raw_u: sums.raw_u,
                raw_d: sums.raw_d,
                charged_u: sums.charged_u,
                charged_d: sums.charged_d,
            })
        })
        .collect()
}

fn add_daily_sums(
    sums: &mut DailySums,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
    batch_id: Uuid,
) -> Result<(), BatchProjectionError> {
    sums.event_count = sums
        .event_count
        .checked_add(1)
        .ok_or_else(|| aggregate_overflow(batch_id))?;
    sums.raw_u = sums
        .raw_u
        .checked_add(raw_u)
        .ok_or_else(|| aggregate_overflow(batch_id))?;
    sums.raw_d = sums
        .raw_d
        .checked_add(raw_d)
        .ok_or_else(|| aggregate_overflow(batch_id))?;
    sums.charged_u = sums
        .charged_u
        .checked_add(charged_u)
        .ok_or_else(|| aggregate_overflow(batch_id))?;
    sums.charged_d = sums
        .charged_d
        .checked_add(charged_d)
        .ok_or_else(|| aggregate_overflow(batch_id))?;
    Ok(())
}

fn aggregate_overflow(batch_id: Uuid) -> BatchProjectionError {
    BatchProjectionError::AggregateOverflow { batch_id }
}

fn projection_token(kind: &str, batch_id: Uuid) -> String {
    format!("v2board.analytics.{kind}.v1.{batch_id}")
}

fn parse_u64(value: &str, field: &'static str) -> Result<u64, BatchProjectionError> {
    value.parse().map_err(|_| invalid(field))
}

fn parse_uuid(value: &str, field: &'static str) -> Result<Uuid, BatchProjectionError> {
    Uuid::parse_str(value).map_err(|_| invalid(field))
}

fn parse_date(value: &str) -> Result<NaiveDate, BatchProjectionError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| invalid("accounting_date"))
}

fn decimal_10_2(value: &str) -> Result<i64, BatchProjectionError> {
    let mut decimal = value
        .parse::<Decimal>()
        .map_err(|_| invalid("rate_decimal_10_2"))?;
    if decimal.scale() > 2 {
        return Err(invalid("rate_decimal_10_2"));
    }
    decimal.rescale(2);
    i64::try_from(decimal.mantissa()).map_err(|_| invalid("rate_decimal_10_2"))
}

fn identity_kind(value: IdentityKind) -> &'static str {
    match value {
        IdentityKind::Explicit => "explicit",
        IdentityKind::Implicit => "implicit",
    }
}

fn outcome(value: AccountedOutcome) -> &'static str {
    match value {
        AccountedOutcome::Applied => "applied",
        AccountedOutcome::StaleEpoch => "stale_epoch",
        AccountedOutcome::MissingUser => "missing_user",
    }
}

fn invalid(field: &'static str) -> BatchProjectionError {
    BatchProjectionError::InvalidField { field }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clickhouse_decimal_uses_scaled_integer_without_float_rounding() {
        assert_eq!(decimal_10_2("1.26").unwrap(), 126);
        assert_eq!(decimal_10_2("0").unwrap(), 0);
        assert!(decimal_10_2("1.255").is_err());
    }

    #[test]
    fn verification_month_range_rolls_over_years() {
        assert_eq!(
            next_month(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
            NaiveDate::from_ymd_opt(2026, 8, 1).unwrap()
        );
        assert_eq!(
            next_month(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()),
            NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()
        );
    }

    #[test]
    fn daily_projection_groups_within_batch_and_checks_sums() {
        let batch_id = Uuid::from_u128(7);
        let date = NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
        let batch = ClaimedBatch {
            batch_id,
            event_name: REPORTED_EVENT_NAME.to_string(),
            schema_major: 1,
            partition_month: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            table_generation: 1,
            content_sha256: "a".repeat(64),
            insert_settings_sha256: "b".repeat(64),
            lease_owner: Uuid::from_u128(8),
            lease_expires_at: 1,
            created_at: 1,
            state: crate::DeliveryBatchState::Publishing,
            rows: Vec::new(),
        };
        let row = |event_id: &str, raw_u, raw_d, charged_u, charged_d| ReportedRow {
            event_id: event_id.to_string(),
            schema_major: 1,
            installation_id: Uuid::from_u128(9),
            report_key: event_id.to_string(),
            payload_hash: "c".repeat(64),
            identity_kind: "explicit".to_string(),
            user_id: 1,
            traffic_epoch: 1,
            server_id: 2,
            server_type: "test".to_string(),
            rate_text: "1.00".to_string(),
            rate_decimal_10_2: 100,
            raw_u,
            raw_d,
            charged_u,
            charged_d,
            accepted_at_unix: 1,
            accounting_date: date,
            accounting_timezone: "UTC".to_string(),
            ingest_batch_id: batch_id,
            batch_row_number: 0,
            outbox_payload_sha256: "d".repeat(64),
            table_generation: 1,
            ingested_at_unix: 1,
        };
        let rows = reported_daily_rows(
            &batch,
            &[row("one", 10, 20, 30, 40), row("two", 1, 2, 3, 4)],
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_count, 2);
        assert_eq!(rows[0].raw_u, 11);
        assert_eq!(rows[0].raw_d, 22);
        assert_eq!(rows[0].charged_u, 33);
        assert_eq!(rows[0].charged_d, 44);
        assert_ne!(
            projection_token("reported-raw", batch_id),
            projection_token("reported-daily", batch_id)
        );
    }
}
