use chrono::NaiveDate;
use clickhouse::Row;
use serde::Deserialize;
use uuid::Uuid;

use crate::{ClickHouseMigrationError, verify_clickhouse_runtime_ready};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppliedDailyTraffic {
    pub accounting_date: NaiveDate,
    pub event_count: u64,
    pub raw_u: u64,
    pub raw_d: u64,
    pub charged_u: u64,
    pub charged_d: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AnalyticsReadError {
    #[error("analytics reader requires a positive schema major")]
    InvalidSchemaMajor,
    #[error("analytics reader requires a positive user id")]
    InvalidUserId,
    #[error("analytics reader requires start_date < end_date_exclusive")]
    InvalidDateRange,
    #[error(transparent)]
    Schema(#[from] ClickHouseMigrationError),
    #[error("ClickHouse read failed: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
}

#[derive(Debug, Deserialize, Row)]
struct AppliedDailyTrafficRow {
    #[serde(with = "clickhouse::serde::chrono::date")]
    accounting_date: NaiveDate,
    event_count: u64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
}

/// Read authoritative-settlement analytics for one user from the immutable
/// batch aggregates. The schema-major predicate is deliberate: a future event
/// major must never be silently combined with v1 semantics.
///
/// The reader validates the installation binding before querying so pointing a
/// reader credential at another installation fails closed instead of returning
/// a plausible empty series.
pub async fn read_applied_daily_traffic(
    client: &clickhouse::Client,
    installation_id: Uuid,
    schema_major: u16,
    user_id: u64,
    start_date: NaiveDate,
    end_date_exclusive: NaiveDate,
) -> Result<Vec<AppliedDailyTraffic>, AnalyticsReadError> {
    validate_query(schema_major, user_id, start_date, end_date_exclusive)?;
    verify_clickhouse_runtime_ready(client, installation_id).await?;

    let rows = client
        .query(
            "SELECT accounting_date, sum(event_count) AS event_count, \
                    sum(raw_u) AS raw_u, sum(raw_d) AS raw_d, \
                    sum(charged_u) AS charged_u, sum(charged_d) AS charged_d \
             FROM traffic_accounted_daily \
             WHERE installation_id = toUUID(?) \
               AND schema_major = ? \
               AND user_id = ? \
               AND outcome = 'applied' \
               AND accounting_date >= toDate(?) \
               AND accounting_date < toDate(?) \
             GROUP BY accounting_date \
             ORDER BY accounting_date",
        )
        .bind(installation_id.to_string())
        .bind(schema_major)
        .bind(user_id)
        .bind(start_date.format("%Y-%m-%d").to_string())
        .bind(end_date_exclusive.format("%Y-%m-%d").to_string())
        .fetch_all::<AppliedDailyTrafficRow>()
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| AppliedDailyTraffic {
            accounting_date: row.accounting_date,
            event_count: row.event_count,
            raw_u: row.raw_u,
            raw_d: row.raw_d,
            charged_u: row.charged_u,
            charged_d: row.charged_d,
        })
        .collect())
}

fn validate_query(
    schema_major: u16,
    user_id: u64,
    start_date: NaiveDate,
    end_date_exclusive: NaiveDate,
) -> Result<(), AnalyticsReadError> {
    if schema_major == 0 {
        return Err(AnalyticsReadError::InvalidSchemaMajor);
    }
    if user_id == 0 {
        return Err(AnalyticsReadError::InvalidUserId);
    }
    if start_date >= end_date_exclusive {
        return Err(AnalyticsReadError::InvalidDateRange);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_rejects_ambiguous_or_empty_scopes() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();

        assert!(matches!(
            validate_query(0, 1, start, end),
            Err(AnalyticsReadError::InvalidSchemaMajor)
        ));
        assert!(matches!(
            validate_query(1, 0, start, end),
            Err(AnalyticsReadError::InvalidUserId)
        ));
        assert!(matches!(
            validate_query(1, 1, end, end),
            Err(AnalyticsReadError::InvalidDateRange)
        ));
        assert!(validate_query(1, 1, start, end).is_ok());
    }
}
