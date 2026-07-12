use std::{collections::BTreeMap, time::Duration};

use chrono::Utc;
use redis::AsyncCommands;
use sqlx::{FromRow, MySql, QueryBuilder, Transaction};

use crate::{reset::TRAFFIC_RESET_LOCK_KEY, state::WorkerState};

#[derive(FromRow)]
struct DurableTrafficItem {
    user_id: i64,
    traffic_epoch: i64,
    u: i64,
    d: i64,
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

const TRAFFIC_SQL_BATCH_SIZE: usize = 250;
const TRAFFIC_DRAIN_BUDGET: Duration = Duration::from_secs(45);
const TRAFFIC_MAX_REPORTS_PER_TICK: usize = 10_000;

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let reset_in_progress = tokio::time::timeout(Duration::from_secs(5), async {
        let mut conn = state.redis.get_multiplexed_async_connection().await?;
        conn.exists::<_, bool>(TRAFFIC_RESET_LOCK_KEY).await
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
        let report_key = sqlx::query_scalar::<_, String>(
            r#"
            SELECT report_key
            FROM v2_server_traffic_report
            WHERE applied_at IS NULL
            ORDER BY created_at, report_key
            LIMIT 1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(report_key) = report_key else {
            tx.commit().await?;
            break;
        };
        let items = sqlx::query_as::<_, DurableTrafficItem>(
            r#"
            SELECT user_id, traffic_epoch, u, d
            FROM v2_server_traffic_report_item
            WHERE report_key = ?
            ORDER BY user_id
            "#,
        )
        .bind(&report_key)
        .fetch_all(&mut *tx)
        .await?;
        let stale_items = apply_traffic_items(&mut tx, &items).await?;
        if is_internal_traffic_report_key(&report_key) {
            // Implicit reports have a fresh unguessable key for every
            // upload, so their applied header cannot deduplicate a replay. Drop
            // it with its FK-cascaded items in the same accounting transaction.
            let deleted = sqlx::query(
                "DELETE FROM v2_server_traffic_report WHERE report_key = ? AND applied_at IS NULL",
            )
            .bind(&report_key)
            .execute(&mut *tx)
            .await?;
            if deleted.rows_affected() != 1 {
                anyhow::bail!("durable internal traffic report claim was lost");
            }
        } else {
            let now = Utc::now().timestamp();
            let updated = sqlx::query(
                r#"
                UPDATE v2_server_traffic_report
                SET applied_at = ?, updated_at = ?
                WHERE report_key = ? AND applied_at IS NULL
                "#,
            )
            .bind(now)
            .bind(now)
            .bind(&report_key)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() != 1 {
                anyhow::bail!("durable traffic report claim was lost");
            }
            // Explicit keys are replay identities. Retain the applied header,
            // but payload rows are no longer needed after the atomic commit.
            sqlx::query("DELETE FROM v2_server_traffic_report_item WHERE report_key = ?")
                .bind(&report_key)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        processed += 1;
        if stale_items > 0 {
            tracing::info!(
                report_key,
                stale_items,
                "discarded traffic from an earlier quota epoch"
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
    tx: &mut Transaction<'_, MySql>,
    items: &[DurableTrafficItem],
) -> anyhow::Result<usize> {
    let aggregated = aggregate_traffic_items(items)?;
    if aggregated.is_empty() {
        return Ok(0);
    }

    // Every worker acquires user locks in ascending id order, including across chunks. Each
    // related row is locked once before any update, preventing opposite report order from
    // creating a deadlock cycle and keeping the report transaction all-or-nothing.
    let mut locked = BTreeMap::new();
    for chunk in aggregated.chunks(TRAFFIC_SQL_BATCH_SIZE) {
        let mut builder =
            QueryBuilder::<MySql>::new("SELECT id, traffic_epoch, u, d FROM v2_user WHERE id IN (");
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
    let mut stale_items = 0_usize;
    for item in aggregated {
        let Some(current) = locked.get(&item.user_id) else {
            continue;
        };
        if current.traffic_epoch != item.traffic_epoch {
            stale_items += 1;
            continue;
        }
        let (u, d) = checked_traffic_totals(current.u, current.d, item.u, item.d)
            .ok_or_else(|| anyhow::anyhow!("user traffic exceeds the supported range"))?;
        updates.push(TrafficUpdate {
            user_id: item.user_id,
            u,
            d,
        });
    }

    let now = Utc::now().timestamp();
    for chunk in updates.chunks(TRAFFIC_SQL_BATCH_SIZE) {
        update_traffic_chunk(tx, chunk, now).await?;
    }
    Ok(stale_items)
}

fn aggregate_traffic_items(
    items: &[DurableTrafficItem],
) -> anyhow::Result<Vec<DurableTrafficItem>> {
    let mut aggregated = BTreeMap::<i64, (i64, i64, i64)>::new();
    for item in items {
        let totals = aggregated
            .entry(item.user_id)
            .or_insert((item.traffic_epoch, 0, 0));
        if totals.0 != item.traffic_epoch {
            anyhow::bail!("traffic report contains multiple quota epochs for one user");
        }
        let (u, d) = checked_traffic_totals(totals.1, totals.2, item.u, item.d)
            .ok_or_else(|| anyhow::anyhow!("traffic report exceeds the supported range"))?;
        totals.1 = u;
        totals.2 = d;
    }
    Ok(aggregated
        .into_iter()
        .map(|(user_id, (traffic_epoch, u, d))| DurableTrafficItem {
            user_id,
            traffic_epoch,
            u,
            d,
        })
        .collect())
}

async fn update_traffic_chunk(
    tx: &mut Transaction<'_, MySql>,
    updates: &[TrafficUpdate],
    now: i64,
) -> Result<(), sqlx::Error> {
    if updates.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET u = CASE id ");
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
    builder.build().execute(&mut **tx).await?;
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
                u: 1,
                d: 2,
            },
            DurableTrafficItem {
                user_id: 2,
                traffic_epoch: 7,
                u: 3,
                d: 4,
            },
            DurableTrafficItem {
                user_id: 9,
                traffic_epoch: 4,
                u: 5,
                d: 6,
            },
        ])
        .unwrap();
        assert_eq!(
            aggregated
                .iter()
                .map(|item| (item.user_id, item.u, item.d))
                .collect::<Vec<_>>(),
            vec![(2, 3, 4), (9, 6, 8)]
        );
        assert!(
            aggregate_traffic_items(&[
                DurableTrafficItem {
                    user_id: 1,
                    traffic_epoch: 0,
                    u: i64::MAX,
                    d: 0,
                },
                DurableTrafficItem {
                    user_id: 1,
                    traffic_epoch: 0,
                    u: 1,
                    d: 0,
                },
            ])
            .is_err()
        );
    }

    #[test]
    fn traffic_apply_uses_ordered_batch_locking_and_case_updates() {
        let source = include_str!("traffic.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.contains(") ORDER BY id FOR UPDATE"));
        assert!(production.contains("UPDATE v2_user SET u = CASE id"));
        assert!(!production.contains("WHERE id = ? LIMIT 1 FOR UPDATE"));
    }

    #[test]
    fn traffic_migration_contains_durable_report_ledgers() {
        let migration = include_str!("../../../migrations/0003_worker_idempotency.sql");
        assert!(migration.contains("CREATE TABLE `v2_server_traffic_report`"));
        assert!(migration.contains("CREATE TABLE `v2_server_traffic_report_item`"));
    }

    #[test]
    fn only_internal_traffic_keys_are_ephemeral_after_apply() {
        assert!(is_internal_traffic_report_key(&format!(
            "i-{}",
            "a".repeat(62)
        )));
        assert!(!is_internal_traffic_report_key(&"a".repeat(64)));
        let migration = include_str!("../../../migrations/0004_traffic_report_sha256.sql");
        assert!(migration.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn traffic_epoch_migration_fences_reset_periods() {
        let migration = include_str!("../../../migrations/0009_traffic_quota_epoch.sql");
        assert!(migration.contains("`traffic_epoch` bigint NOT NULL DEFAULT 0"));
        let item_migration = include_str!("../../../migrations/0013_traffic_report_epoch.sql");
        assert!(item_migration.contains("`traffic_epoch` bigint NOT NULL DEFAULT 0"));
        let source = include_str!("traffic.rs");
        assert!(source.contains("current.traffic_epoch != item.traffic_epoch"));
    }
}
