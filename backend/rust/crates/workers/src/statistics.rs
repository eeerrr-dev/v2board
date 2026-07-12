use chrono::{Datelike, TimeZone, Utc};
use v2board_config::{app_now, app_timezone};

use crate::{state::WorkerState, time::timestamp_before};

const STAT_ORDER_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(total_amount), 0) AS CHAR) FROM v2_order WHERE created_at >= ? AND created_at < ?";
const STAT_PAID_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(total_amount), 0) AS CHAR) FROM v2_order WHERE paid_at >= ? AND paid_at < ? AND status NOT IN (0, 2)";
const STAT_COMMISSION_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(get_amount), 0) AS CHAR) FROM v2_commission_log WHERE created_at >= ? AND created_at < ?";
const STAT_TRANSFER_TOTAL_SQL: &str = "SELECT CAST(COALESCE(SUM(u) + SUM(d), 0) AS CHAR) FROM v2_stat_server WHERE created_at >= ? AND created_at < ?";
const STAT_UPSERT_SQL: &str = r#"
INSERT INTO v2_stat
    (record_at, record_type, order_count, order_total, commission_count,
     commission_total, paid_count, paid_total, register_count, invite_count,
     transfer_used_total, created_at, updated_at)
VALUES (?, 'd', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) AS incoming
ON DUPLICATE KEY UPDATE
    order_count = incoming.order_count,
    order_total = incoming.order_total,
    commission_count = incoming.commission_count,
    commission_total = incoming.commission_total,
    paid_count = incoming.paid_count,
    paid_total = incoming.paid_total,
    register_count = incoming.register_count,
    invite_count = incoming.invite_count,
    transfer_used_total = incoming.transfer_used_total,
    updated_at = incoming.updated_at
"#;

fn exact_i64_aggregate(value: &str, metric: &str) -> anyhow::Result<i64> {
    let exact = value
        .parse::<i128>()
        .map_err(|_| anyhow::anyhow!("{metric} aggregate is not a valid integer"))?;
    i64::try_from(exact)
        .map_err(|_| anyhow::anyhow!("{metric} aggregate exceeds the supported range"))
}

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let end_at = today_start_timestamp();
    let start_at = timestamp_before(end_at, 86_400);
    let order_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_order WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let order_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_ORDER_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "order total",
    )?;
    let paid_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_order WHERE paid_at >= ? AND paid_at < ? AND status NOT IN (0, 2)",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let paid_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_PAID_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "paid total",
    )?;
    let commission_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_commission_log WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let commission_total = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(STAT_COMMISSION_TOTAL_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?,
        "commission total",
    )?;
    let register_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE created_at >= ? AND created_at < ?")
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?;
    let invite_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_user WHERE created_at >= ? AND created_at < ? AND invite_user_id IS NOT NULL",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
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
            assert!(sql.contains("AS CHAR"));
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
    }

    #[test]
    fn statistic_upsert_uses_mysql_new_row_alias() {
        assert!(STAT_UPSERT_SQL.contains("AS incoming"));
        assert!(STAT_UPSERT_SQL.contains("incoming.order_count"));
        assert!(!STAT_UPSERT_SQL.contains("VALUES(order_count)"));
    }
}
