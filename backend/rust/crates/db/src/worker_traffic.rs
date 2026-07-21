use std::collections::BTreeMap;

use chrono::NaiveDate;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use v2board_analytics::{
    AccountedOutcome, AccountedTrafficEvent, AnalyticsEvent, EventValidationError, IdentityKind,
    OutboxError, TrafficEventCore, enqueue_events,
};
use v2board_application::{
    RepositoryError,
    worker_traffic::{AppliedTrafficReport, RepositoryResult, TrafficAccountingRepository},
};

const TRAFFIC_SQL_BATCH_SIZE: usize = 250;

#[derive(Clone, Debug)]
pub struct PostgresTrafficAccountingRepository {
    pool: PgPool,
}

impl PostgresTrafficAccountingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, thiserror::Error)]
enum TrafficPersistenceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Event(#[from] EventValidationError),
    #[error(transparent)]
    Outbox(#[from] OutboxError),
    #[error("{0}")]
    Invariant(&'static str),
}

#[derive(Debug, FromRow)]
struct DurableTrafficReport {
    report_key: String,
    payload_hash: String,
    node_id: i32,
    node_type: String,
    rate_text: String,
    rate_decimal_10_2: String,
    identity_kind: String,
    accepted_at: i64,
    accounting_date: NaiveDate,
}

#[derive(Clone, Debug, FromRow, PartialEq, Eq)]
struct DurableTrafficItem {
    user_id: i64,
    traffic_epoch: i64,
    raw_u: i64,
    raw_d: i64,
    charged_u: i64,
    charged_d: i64,
}

#[derive(Debug, FromRow)]
struct LockedTrafficUser {
    id: i64,
    traffic_epoch: i64,
    u: i64,
    d: i64,
}

#[derive(Debug, PartialEq, Eq)]
struct TrafficUpdate {
    user_id: i64,
    u: i64,
    d: i64,
}

#[derive(Debug, PartialEq, Eq)]
struct TrafficAccountingResult {
    item: DurableTrafficItem,
    outcome: AccountedOutcome,
    u_after: Option<i64>,
    d_after: Option<i64>,
}

impl TrafficAccountingRepository for PostgresTrafficAccountingRepository {
    async fn apply_next(
        &self,
        installation_id: &str,
        accounted_at: i64,
    ) -> RepositoryResult<Option<AppliedTrafficReport>> {
        apply_next_report(&self.pool, installation_id, accounted_at)
            .await
            .map_err(|error| RepositoryError::new("apply durable traffic report", error))
    }
}

async fn apply_next_report(
    pool: &PgPool,
    installation_id: &str,
    accounted_at: i64,
) -> Result<Option<AppliedTrafficReport>, TrafficPersistenceError> {
    let mut transaction = pool.begin().await?;
    let report = sqlx::query_as::<_, DurableTrafficReport>(
        r#"
        SELECT report_key, payload_hash, node_id, node_type, rate_text,
               rate_decimal_10_2::text AS rate_decimal_10_2,
               identity_kind, accepted_at, accounting_date
        FROM server_traffic_report
        WHERE applied_at IS NULL
        ORDER BY created_at, report_key
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(report) = report else {
        transaction.commit().await?;
        return Ok(None);
    };
    let items = sqlx::query_as::<_, DurableTrafficItem>(
        r#"
        SELECT user_id, traffic_epoch, raw_u, raw_d, charged_u, charged_d
        FROM server_traffic_report_item
        WHERE report_key = $1
        ORDER BY user_id
        "#,
    )
    .bind(&report.report_key)
    .fetch_all(&mut *transaction)
    .await?;
    let accounting_results = apply_traffic_items(&mut transaction, &items, accounted_at).await?;
    enqueue_accounted_events(
        installation_id,
        &mut transaction,
        &report,
        &accounting_results,
        accounted_at,
    )
    .await?;
    let stale_items = accounting_results
        .iter()
        .filter(|result| result.outcome == AccountedOutcome::StaleEpoch)
        .count();
    let missing_users = accounting_results
        .iter()
        .filter(|result| result.outcome == AccountedOutcome::MissingUser)
        .count();

    if is_internal_traffic_report_key(&report.report_key) {
        let deleted = sqlx::query(
            "DELETE FROM server_traffic_report WHERE report_key = $1 AND applied_at IS NULL",
        )
        .bind(&report.report_key)
        .execute(&mut *transaction)
        .await?;
        if deleted.rows_affected() != 1 {
            return Err(TrafficPersistenceError::Invariant(
                "durable internal traffic report claim was lost",
            ));
        }
    } else {
        let updated = sqlx::query(
            r#"
            UPDATE server_traffic_report
            SET applied_at = $1, updated_at = $2
            WHERE report_key = $3 AND applied_at IS NULL
            "#,
        )
        .bind(accounted_at)
        .bind(accounted_at)
        .bind(&report.report_key)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(TrafficPersistenceError::Invariant(
                "durable traffic report claim was lost",
            ));
        }
        sqlx::query("DELETE FROM server_traffic_report_item WHERE report_key = $1")
            .bind(&report.report_key)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(Some(AppliedTrafficReport {
        report_key: report.report_key,
        stale_items,
        missing_users,
    }))
}

fn is_internal_traffic_report_key(report_key: &str) -> bool {
    report_key.starts_with("i-")
}

async fn apply_traffic_items(
    transaction: &mut Transaction<'_, Postgres>,
    items: &[DurableTrafficItem],
    accounted_at: i64,
) -> Result<Vec<TrafficAccountingResult>, TrafficPersistenceError> {
    let aggregated = aggregate_traffic_items(items)?;
    if aggregated.is_empty() {
        return Ok(Vec::new());
    }
    let mut locked = BTreeMap::new();
    for chunk in aggregated.chunks(TRAFFIC_SQL_BATCH_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, traffic_epoch, u, d FROM users WHERE id IN (",
        );
        let mut separated = builder.separated(", ");
        for item in chunk {
            separated.push_bind(item.user_id);
        }
        builder.push(") ORDER BY id FOR UPDATE");
        for user in builder
            .build_query_as::<LockedTrafficUser>()
            .fetch_all(&mut **transaction)
            .await?
        {
            locked.insert(user.id, user);
        }
    }

    let mut updates = Vec::with_capacity(locked.len());
    let mut results = Vec::with_capacity(aggregated.len());
    for item in aggregated {
        let Some(current) = locked.get(&item.user_id) else {
            results.push(TrafficAccountingResult {
                item,
                outcome: AccountedOutcome::MissingUser,
                u_after: None,
                d_after: None,
            });
            continue;
        };
        if current.traffic_epoch != item.traffic_epoch {
            results.push(TrafficAccountingResult {
                item,
                outcome: AccountedOutcome::StaleEpoch,
                u_after: None,
                d_after: None,
            });
            continue;
        }
        let (u, d) = checked_traffic_totals(current.u, current.d, item.charged_u, item.charged_d)
            .ok_or(TrafficPersistenceError::Invariant(
            "user traffic exceeds the supported range",
        ))?;
        updates.push(TrafficUpdate {
            user_id: item.user_id,
            u,
            d,
        });
        results.push(TrafficAccountingResult {
            item,
            outcome: AccountedOutcome::Applied,
            u_after: Some(u),
            d_after: Some(d),
        });
    }

    for chunk in updates.chunks(TRAFFIC_SQL_BATCH_SIZE) {
        update_traffic_chunk(transaction, chunk, accounted_at).await?;
    }
    Ok(results)
}

fn aggregate_traffic_items(
    items: &[DurableTrafficItem],
) -> Result<Vec<DurableTrafficItem>, TrafficPersistenceError> {
    let mut aggregated = BTreeMap::<i64, (i64, i64, i64, i64, i64)>::new();
    for item in items {
        let totals = aggregated
            .entry(item.user_id)
            .or_insert((item.traffic_epoch, 0, 0, 0, 0));
        if totals.0 != item.traffic_epoch {
            return Err(TrafficPersistenceError::Invariant(
                "traffic report contains multiple quota epochs for one user",
            ));
        }
        let (raw_u, raw_d) = checked_traffic_totals(totals.1, totals.2, item.raw_u, item.raw_d)
            .ok_or(TrafficPersistenceError::Invariant(
                "traffic report exceeds the supported range",
            ))?;
        let (charged_u, charged_d) =
            checked_traffic_totals(totals.3, totals.4, item.charged_u, item.charged_d).ok_or(
                TrafficPersistenceError::Invariant("traffic report exceeds the supported range"),
            )?;
        totals.1 = raw_u;
        totals.2 = raw_d;
        totals.3 = charged_u;
        totals.4 = charged_d;
    }
    Ok(aggregated
        .into_iter()
        .map(
            |(user_id, (traffic_epoch, raw_u, raw_d, charged_u, charged_d))| DurableTrafficItem {
                user_id,
                traffic_epoch,
                raw_u,
                raw_d,
                charged_u,
                charged_d,
            },
        )
        .collect())
}

async fn enqueue_accounted_events(
    installation_id: &str,
    transaction: &mut Transaction<'_, Postgres>,
    report: &DurableTrafficReport,
    results: &[TrafficAccountingResult],
    accounted_at: i64,
) -> Result<(), TrafficPersistenceError> {
    let identity_kind = parse_identity_kind(&report.identity_kind)?;
    if is_internal_traffic_report_key(&report.report_key)
        != (identity_kind == IdentityKind::Implicit)
    {
        return Err(TrafficPersistenceError::Invariant(
            "traffic report identity kind does not match its report key",
        ));
    }
    let mut events = Vec::<AnalyticsEvent>::with_capacity(results.len());
    for result in results {
        let item = &result.item;
        let core = TrafficEventCore {
            installation_id: installation_id.to_string(),
            report_key: report.report_key.clone(),
            payload_hash: report.payload_hash.clone(),
            identity_kind,
            user_id: item.user_id.to_string(),
            traffic_epoch: item.traffic_epoch.to_string(),
            server_id: report.node_id.to_string(),
            server_type: report.node_type.clone(),
            rate_text: report.rate_text.clone(),
            rate_decimal_10_2: report.rate_decimal_10_2.clone(),
            raw_u: item.raw_u.to_string(),
            raw_d: item.raw_d.to_string(),
            charged_u: item.charged_u.to_string(),
            charged_d: item.charged_d.to_string(),
            accepted_at: report.accepted_at,
            accounting_date: report.accounting_date.format("%Y-%m-%d").to_string(),
            accounting_timezone: "Asia/Shanghai".to_string(),
        };
        events.push(
            AccountedTrafficEvent::new(
                core,
                accounted_at,
                result.outcome,
                result.u_after.map(|value| value.to_string()),
                result.d_after.map(|value| value.to_string()),
            )?
            .into_outbox()?,
        );
    }
    enqueue_events(transaction, &events, accounted_at).await?;
    Ok(())
}

fn parse_identity_kind(value: &str) -> Result<IdentityKind, TrafficPersistenceError> {
    match value {
        "explicit" => Ok(IdentityKind::Explicit),
        "implicit" => Ok(IdentityKind::Implicit),
        _ => Err(TrafficPersistenceError::Invariant(
            "traffic report has an invalid identity kind",
        )),
    }
}

async fn update_traffic_chunk(
    transaction: &mut Transaction<'_, Postgres>,
    updates: &[TrafficUpdate],
    now: i64,
) -> Result<(), sqlx::Error> {
    if updates.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::<Postgres>::new("UPDATE users SET u = CASE id ");
    for update in updates {
        builder
            .push("WHEN ")
            .push_bind(update.user_id)
            .push(" THEN ")
            .push_bind(update.u)
            .push(" ");
    }
    builder.push("ELSE u END, d = CASE id ");
    for update in updates {
        builder
            .push("WHEN ")
            .push_bind(update.user_id)
            .push(" THEN ")
            .push_bind(update.d)
            .push(" ");
    }
    builder
        .push("ELSE d END, t = ")
        .push_bind(now)
        .push(", updated_at = ")
        .push_bind(now)
        .push(" WHERE id IN (");
    let mut separated = builder.separated(", ");
    for update in updates {
        separated.push_bind(update.user_id);
    }
    builder.push(")");
    let result = builder.build().execute(&mut **transaction).await?;
    if result.rows_affected() != updates.len() as u64 {
        return Err(sqlx::Error::Protocol(
            "traffic user update count did not match the locked rows".to_string(),
        ));
    }
    Ok(())
}

fn checked_traffic_totals(
    current_u: i64,
    current_d: i64,
    additional_u: i64,
    additional_d: i64,
) -> Option<(i64, i64)> {
    Some((
        current_u.checked_add(additional_u)?,
        current_d.checked_add(additional_d)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_items_are_checked_and_aggregated_in_lock_order() {
        let aggregated = aggregate_traffic_items(&[
            DurableTrafficItem {
                user_id: 9,
                traffic_epoch: 4,
                raw_u: 1,
                raw_d: 2,
                charged_u: 3,
                charged_d: 4,
            },
            DurableTrafficItem {
                user_id: 2,
                traffic_epoch: 7,
                raw_u: 3,
                raw_d: 4,
                charged_u: 5,
                charged_d: 6,
            },
            DurableTrafficItem {
                user_id: 9,
                traffic_epoch: 4,
                raw_u: 5,
                raw_d: 6,
                charged_u: 7,
                charged_d: 8,
            },
        ])
        .unwrap();
        assert_eq!(
            aggregated
                .iter()
                .map(|item| (item.user_id, item.charged_u, item.charged_d))
                .collect::<Vec<_>>(),
            vec![(2, 5, 6), (9, 10, 12)]
        );
        assert_eq!(checked_traffic_totals(i64::MAX, 0, 1, 0), None);
    }
}
