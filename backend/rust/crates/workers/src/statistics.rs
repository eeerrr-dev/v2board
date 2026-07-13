use chrono::{Datelike, TimeZone, Utc};
use v2board_config::{app_now, app_timezone};

use crate::{state::WorkerState, time::timestamp_before};

const STAT_ORDER_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(total_amount), 0) AS TEXT) FROM orders WHERE created_at >= $1 AND created_at < $2";
const STAT_PAID_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(total_amount), 0) AS TEXT) FROM orders WHERE paid_at >= $1 AND paid_at < $2 AND status NOT IN (0, 2)";
const STAT_COMMISSION_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(get_amount), 0) AS TEXT) FROM commission_log WHERE created_at >= $1 AND created_at < $2";
const STAT_TRANSFER_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(u) + SUM(d), 0) AS TEXT) FROM stat_server WHERE created_at >= $1 AND created_at < $2";
const STAT_UPSERT_SQL: &str = r#"
INSERT INTO stat
    (record_at, record_type, order_count, order_total, commission_count,
     commission_total, paid_count, paid_total, register_count, invite_count,
     transfer_used_total, created_at, updated_at)
VALUES ($1, 'd', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
ON CONFLICT (record_at) DO UPDATE SET
    order_count = EXCLUDED.order_count,
    order_total = EXCLUDED.order_total,
    commission_count = EXCLUDED.commission_count,
    commission_total = EXCLUDED.commission_total,
    paid_count = EXCLUDED.paid_count,
    paid_total = EXCLUDED.paid_total,
    register_count = EXCLUDED.register_count,
    invite_count = EXCLUDED.invite_count,
    transfer_used_total = EXCLUDED.transfer_used_total,
    updated_at = EXCLUDED.updated_at
"#;

fn exact_i64_aggregate(value: &str, metric: &str) -> anyhow::Result<i64> {
    let exact = value
        .parse::<i128>()
        .map_err(|_| anyhow::anyhow!("{metric} aggregate is not a valid integer"))?;
    i64::try_from(exact)
        .map_err(|_| anyhow::anyhow!("{metric} aggregate exceeds the supported range"))
}

fn exact_i32_count(value: i64, metric: &str) -> anyhow::Result<i32> {
    i32::try_from(value).map_err(|_| anyhow::anyhow!("{metric} exceeds the supported range"))
}

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let end_at = today_start_timestamp();
    let start_at = timestamp_before(end_at, 86_400);
    let order_count = exact_i32_count(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(start_at)
        .bind(end_at)
        .fetch_one(&state.db)
        .await?,
        "order count",
    )?;
    let order_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_ORDER_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "order total",
    )?;
    let paid_count = exact_i32_count(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE paid_at >= $1 AND paid_at < $2 AND status NOT IN (0, 2)",
        )
        .bind(start_at)
        .bind(end_at)
        .fetch_one(&state.db)
        .await?,
        "paid order count",
    )?;
    let paid_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_PAID_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "paid total",
    )?;
    let commission_count = exact_i32_count(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM commission_log WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(start_at)
        .bind(end_at)
        .fetch_one(&state.db)
        .await?,
        "commission count",
    )?;
    let commission_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_COMMISSION_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "commission total",
    )?;
    let register_count = exact_i32_count(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(start_at)
        .bind(end_at)
        .fetch_one(&state.db)
        .await?,
        "registration count",
    )?;
    let invite_count = exact_i32_count(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2 AND invite_user_id IS NOT NULL",
        )
        .bind(start_at)
        .bind(end_at)
        .fetch_one(&state.db)
        .await?,
        "invite count",
    )?;
    let transfer_used_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_TRANSFER_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "traffic total",
    )?;
    sqlx::query(STAT_UPSERT_SQL)
        .bind(start_at)
        .bind(order_count)
        .bind(order_total)
        .bind(commission_count)
        .bind(commission_total)
        .bind(paid_count)
        .bind(paid_total)
        .bind(register_count)
        .bind(invite_count)
        .bind(transfer_used_total.to_string())
        .bind(Utc::now().timestamp())
        .bind(Utc::now().timestamp())
        .execute(&state.db)
        .await?;
    Ok(())
}

fn today_start_timestamp() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistic_aggregates_preserve_exact_values_and_reject_i64_overflow() {
        for sql in [
            STAT_ORDER_TOTAL_SQL,
            STAT_PAID_TOTAL_SQL,
            STAT_COMMISSION_TOTAL_SQL,
            STAT_TRANSFER_TOTAL_SQL,
        ] {
            assert!(sql.contains("AS TEXT"));
            assert!(!sql.contains("AS SIGNED"));
        }
        assert_eq!(exact_i64_aggregate("0", "test").unwrap(), 0);
        assert_eq!(
            exact_i64_aggregate("9223372036854775807", "test").unwrap(),
            i64::MAX
        );
        assert_eq!(
            exact_i64_aggregate("-9223372036854775808", "test").unwrap(),
            i64::MIN
        );
        assert!(exact_i64_aggregate("9223372036854775808", "test").is_err());
        assert!(exact_i64_aggregate("-9223372036854775809", "test").is_err());
        assert!(exact_i64_aggregate("not-a-number", "test").is_err());
        assert_eq!(
            exact_i32_count(i64::from(i32::MAX), "test").unwrap(),
            i32::MAX
        );
        assert!(exact_i32_count(i64::from(i32::MAX) + 1, "test").is_err());
    }

    #[test]
    fn statistic_upsert_uses_postgres_excluded_row() {
        assert!(STAT_UPSERT_SQL.contains("ON CONFLICT (record_at)"));
        assert!(STAT_UPSERT_SQL.contains("EXCLUDED.order_count"));
    }
}
