use std::{collections::BTreeMap, time::Duration};

use chrono::{NaiveDate, Utc};
use redis::AsyncCommands;
use sqlx::{FromRow, Postgres, QueryBuilder, Transaction};
use v2board_analytics::{
    AccountedOutcome, AccountedTrafficEvent, AnalyticsEvent, IdentityKind, TrafficEventCore,
    enqueue_events,
};

use crate::{reset::TRAFFIC_RESET_LOCK_KEY, state::WorkerState};

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

const TRAFFIC_SQL_BATCH_SIZE: usize = 250;
const TRAFFIC_DRAIN_BUDGET: Duration = Duration::from_secs(45);
const TRAFFIC_MAX_REPORTS_PER_TICK: usize = 10_000;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let reset_in_progress = tokio::time::timeout(Duration::from_secs(5), async {
        let mut conn = state.redis.get_multiplexed_async_connection().await?;
        conn.exists::<_, bool>(state.redis_key(TRAFFIC_RESET_LOCK_KEY))
            .await
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out checking the traffic reset barrier"))??;
    if reset_in_progress {
        return Ok(());
    }
    apply_durable_traffic_reports(state).await
}

fn is_internal_traffic_report_key(report_key: &str) -> bool {
    report_key.starts_with("i-")
}

async fn apply_durable_traffic_reports(state: &WorkerState) -> anyhow::Result<()> {
    // Drain for a bounded wall-clock budget rather than the former hard limit of
    // 100 reports/minute. The item count within a report is independently
    // batched, so a burst cannot grow one SQL statement without bound.
    let deadline = tokio::time::Instant::now() + TRAFFIC_DRAIN_BUDGET;
    let mut processed = 0_usize;
    while processed < TRAFFIC_MAX_REPORTS_PER_TICK && tokio::time::Instant::now() < deadline {
        let mut tx = state.db.begin().await?;
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
        .fetch_optional(&mut *tx)
        .await?;
        let Some(report) = report else {
            tx.commit().await?;
            break;
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
        .fetch_all(&mut *tx)
        .await?;
        let accounting_results = apply_traffic_items(&mut tx, &items).await?;
        let accounted_at = Utc::now().timestamp();
        enqueue_accounted_events(state, &mut tx, &report, &accounting_results, accounted_at)
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
            // Implicit reports have a fresh unguessable key for every
            // upload, so their applied header cannot deduplicate a replay. Drop
            // it with its FK-cascaded items in the same accounting transaction.
            let deleted = sqlx::query(
                "DELETE FROM server_traffic_report WHERE report_key = $1 AND applied_at IS NULL",
            )
            .bind(&report.report_key)
            .execute(&mut *tx)
            .await?;
            if deleted.rows_affected() != 1 {
                anyhow::bail!("durable internal traffic report claim was lost");
            }
        } else {
            let now = Utc::now().timestamp();
            let updated = sqlx::query(
                r#"
                UPDATE server_traffic_report
                SET applied_at = $1, updated_at = $2
                WHERE report_key = $3 AND applied_at IS NULL
                "#,
            )
            .bind(now)
            .bind(now)
            .bind(&report.report_key)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() != 1 {
                anyhow::bail!("durable traffic report claim was lost");
            }
            // Explicit keys are replay identities. Retain the applied header,
            // but payload rows are no longer needed after the atomic commit.
            sqlx::query("DELETE FROM server_traffic_report_item WHERE report_key = $1")
                .bind(&report.report_key)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        processed += 1;
        if stale_items > 0 {
            tracing::info!(
                report_key = %report.report_key,
                stale_items,
                "discarded traffic from an earlier quota epoch"
            );
        }
        if missing_users > 0 {
            tracing::warn!(
                report_key = %report.report_key,
                missing_users,
                "recorded traffic items whose user no longer exists"
            );
        }
    }
    if processed == TRAFFIC_MAX_REPORTS_PER_TICK || tokio::time::Instant::now() >= deadline {
        tracing::warn!(
            processed,
            "traffic drain reached its per-tick safety budget"
        );
    }
    Ok(())
}

async fn apply_traffic_items(
    tx: &mut Transaction<'_, Postgres>,
    items: &[DurableTrafficItem],
) -> anyhow::Result<Vec<TrafficAccountingResult>> {
    let aggregated = aggregate_traffic_items(items)?;
    if aggregated.is_empty() {
        return Ok(Vec::new());
    }

    // Every worker acquires user locks in ascending id order, including across chunks. Each
    // related row is locked once before any update, preventing opposite report order from
    // creating a deadlock cycle and keeping the report transaction all-or-nothing.
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
            .fetch_all(&mut **tx)
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
            .ok_or_else(|| anyhow::anyhow!("user traffic exceeds the supported range"))?;
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

    let now = Utc::now().timestamp();
    for chunk in updates.chunks(TRAFFIC_SQL_BATCH_SIZE) {
        update_traffic_chunk(tx, chunk, now).await?;
    }
    Ok(results)
}

fn aggregate_traffic_items(
    items: &[DurableTrafficItem],
) -> anyhow::Result<Vec<DurableTrafficItem>> {
    let mut aggregated = BTreeMap::<i64, (i64, i64, i64, i64, i64)>::new();
    for item in items {
        let totals = aggregated
            .entry(item.user_id)
            .or_insert((item.traffic_epoch, 0, 0, 0, 0));
        if totals.0 != item.traffic_epoch {
            anyhow::bail!("traffic report contains multiple quota epochs for one user");
        }
        let (raw_u, raw_d) = checked_traffic_totals(totals.1, totals.2, item.raw_u, item.raw_d)
            .ok_or_else(|| anyhow::anyhow!("traffic report exceeds the supported range"))?;
        let (charged_u, charged_d) =
            checked_traffic_totals(totals.3, totals.4, item.charged_u, item.charged_d)
                .ok_or_else(|| anyhow::anyhow!("traffic report exceeds the supported range"))?;
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
    state: &WorkerState,
    tx: &mut Transaction<'_, Postgres>,
    report: &DurableTrafficReport,
    results: &[TrafficAccountingResult],
    accounted_at: i64,
) -> anyhow::Result<()> {
    let identity_kind = parse_identity_kind(&report.identity_kind)?;
    let key_is_implicit = is_internal_traffic_report_key(&report.report_key);
    if key_is_implicit != (identity_kind == IdentityKind::Implicit) {
        anyhow::bail!("traffic report identity kind does not match its report key");
    }
    let mut events = Vec::<AnalyticsEvent>::with_capacity(results.len());
    for result in results {
        let item = &result.item;
        let core = TrafficEventCore {
            installation_id: state.installation_id.to_string(),
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
            accounting_timezone: "Asia/Shanghai".to_owned(),
        };
        let event = AccountedTrafficEvent::new(
            core,
            accounted_at,
            result.outcome,
            result.u_after.map(|value| value.to_string()),
            result.d_after.map(|value| value.to_string()),
        )?
        .into_outbox()?;
        events.push(event);
    }
    enqueue_events(tx, &events, accounted_at).await?;
    Ok(())
}

fn parse_identity_kind(value: &str) -> anyhow::Result<IdentityKind> {
    match value {
        "explicit" => Ok(IdentityKind::Explicit),
        "implicit" => Ok(IdentityKind::Implicit),
        _ => anyhow::bail!("traffic report has an invalid identity kind"),
    }
}

async fn update_traffic_chunk(
    tx: &mut Transaction<'_, Postgres>,
    updates: &[TrafficUpdate],
    now: i64,
) -> Result<(), sqlx::Error> {
    if updates.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::<Postgres>::new("UPDATE users SET u = CASE id ");
    for update in updates {
        builder.push("WHEN ");
        builder.push_bind(update.user_id);
        builder.push(" THEN ");
        builder.push_bind(update.u);
        builder.push(" ");
    }
    builder.push("ELSE u END, d = CASE id ");
    for update in updates {
        builder.push("WHEN ");
        builder.push_bind(update.user_id);
        builder.push(" THEN ");
        builder.push_bind(update.d);
        builder.push(" ");
    }
    builder.push("ELSE d END, t = ");
    builder.push_bind(now);
    builder.push(", updated_at = ");
    builder.push_bind(now);
    builder.push(" WHERE id IN (");
    let mut separated = builder.separated(", ");
    for update in updates {
        separated.push_bind(update.user_id);
    }
    builder.push(")");
    let result = builder.build().execute(&mut **tx).await?;
    if result.rows_affected() != updates.len() as u64 {
        return Err(sqlx::Error::Protocol(
            "traffic user update count did not match the locked rows".to_owned(),
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
    fn durable_user_traffic_accumulation_rejects_i64_overflow() {
        assert_eq!(checked_traffic_totals(10, 20, 1, 2), Some((11, 22)));
        assert_eq!(checked_traffic_totals(i64::MAX, 0, 1, 0), None);
        assert_eq!(checked_traffic_totals(0, i64::MIN, 0, -1), None);
    }

    #[test]
    fn traffic_items_are_checked_and_aggregated_in_lock_order() {
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
                .map(|item| {
                    (
                        item.user_id,
                        item.raw_u,
                        item.raw_d,
                        item.charged_u,
                        item.charged_d,
                    )
                })
                .collect::<Vec<_>>(),
            vec![(2, 3, 4, 5, 6), (9, 6, 8, 10, 12)]
        );
        assert!(
            aggregate_traffic_items(&[
                DurableTrafficItem {
                    user_id: 1,
                    traffic_epoch: 0,
                    raw_u: i64::MAX,
                    raw_d: 0,
                    charged_u: i64::MAX,
                    charged_d: 0,
                },
                DurableTrafficItem {
                    user_id: 1,
                    traffic_epoch: 0,
                    raw_u: 1,
                    raw_d: 0,
                    charged_u: 1,
                    charged_d: 0,
                },
            ])
            .is_err()
        );
    }

    #[test]
    fn only_internal_traffic_keys_are_ephemeral_after_apply() {
        assert!(is_internal_traffic_report_key(&format!(
            "i-{}",
            "a".repeat(62)
        )));
        assert!(!is_internal_traffic_report_key(&"a".repeat(64)));
    }
}
