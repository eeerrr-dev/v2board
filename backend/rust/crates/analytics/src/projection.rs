use chrono::{Datelike, NaiveDate};
use clickhouse::Row;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;
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
            let existing = reported_verification_rows(client, batch).await?;
            if verify_exact_projection(batch, &existing, &rows)? {
                return Ok(ProjectionStatus::AlreadyPresentAndVerified);
            }
            let mut insert = client
                .insert::<ReportedRow>("v2_traffic_reported_v1")
                .await?
                .with_setting("insert_deduplication_token", batch.batch_id.to_string())
                .with_setting("async_insert", "0")
                .with_setting("wait_end_of_query", "1")
                .with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)));
            for row in &rows {
                insert.write(row).await?;
            }
            insert.end().await?;
            let stored = reported_verification_rows(client, batch).await?;
            if !verify_exact_projection(batch, &stored, &rows)? {
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
            let existing = accounted_verification_rows(client, batch).await?;
            if verify_exact_projection(batch, &existing, &rows)? {
                return Ok(ProjectionStatus::AlreadyPresentAndVerified);
            }
            let mut insert = client
                .insert::<AccountedRow>("v2_traffic_accounted_v1")
                .await?
                .with_setting("insert_deduplication_token", batch.batch_id.to_string())
                .with_setting("async_insert", "0")
                .with_setting("wait_end_of_query", "1")
                .with_timeouts(Some(Duration::from_secs(30)), Some(Duration::from_secs(90)));
            for row in &rows {
                insert.write(row).await?;
            }
            insert.end().await?;
            let stored = accounted_verification_rows(client, batch).await?;
            if !verify_exact_projection(batch, &stored, &rows)? {
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
}
