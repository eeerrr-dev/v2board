use sqlx::{FromRow, PgPool};
use v2board_application::{
    RepositoryError,
    worker_statistics::{
        DailyStatisticsRecord, RawDailyStatistics, RepositoryResult, StatisticsWindow,
        StatisticsWorkerRepository,
    },
};

const STAT_AGGREGATE_SQL: &str = r#"
SELECT
    (SELECT COUNT(*) FROM orders WHERE created_at >= $1 AND created_at < $2) AS order_count,
    (SELECT CAST(COALESCE(SUM(total_amount), 0) AS TEXT)
       FROM orders WHERE created_at >= $1 AND created_at < $2) AS order_total,
    (SELECT COUNT(*) FROM commission_log WHERE created_at >= $1 AND created_at < $2)
        AS commission_count,
    (SELECT CAST(COALESCE(SUM(get_amount), 0) AS TEXT)
       FROM commission_log WHERE created_at >= $1 AND created_at < $2) AS commission_total,
    (SELECT COUNT(*) FROM orders
       WHERE paid_at >= $1 AND paid_at < $2 AND status NOT IN (0, 2)) AS paid_count,
    (SELECT CAST(COALESCE(SUM(total_amount), 0) AS TEXT) FROM orders
       WHERE paid_at >= $1 AND paid_at < $2 AND status NOT IN (0, 2)) AS paid_total,
    (SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2) AS register_count,
    (SELECT COUNT(*) FROM users
       WHERE created_at >= $1 AND created_at < $2 AND invite_user_id IS NOT NULL) AS invite_count,
    (SELECT CAST(COALESCE(SUM(u) + SUM(d), 0) AS TEXT)
       FROM server_traffic WHERE created_at >= $1 AND created_at < $2) AS transfer_used_total
"#;

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

#[derive(Clone, Debug)]
pub struct PostgresStatisticsWorkerRepository {
    pool: PgPool,
}

impl PostgresStatisticsWorkerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct RawDailyStatisticsRow {
    order_count: i64,
    order_total: String,
    commission_count: i64,
    commission_total: String,
    paid_count: i64,
    paid_total: String,
    register_count: i64,
    invite_count: i64,
    transfer_used_total: String,
}

impl StatisticsWorkerRepository for PostgresStatisticsWorkerRepository {
    async fn aggregate(&self, window: StatisticsWindow) -> RepositoryResult<RawDailyStatistics> {
        sqlx::query_as::<_, RawDailyStatisticsRow>(STAT_AGGREGATE_SQL)
            .bind(window.start_at)
            .bind(window.end_at)
            .fetch_one(&self.pool)
            .await
            .map(|row| RawDailyStatistics {
                order_count: row.order_count,
                order_total: row.order_total,
                commission_count: row.commission_count,
                commission_total: row.commission_total,
                paid_count: row.paid_count,
                paid_total: row.paid_total,
                register_count: row.register_count,
                invite_count: row.invite_count,
                transfer_used_total: row.transfer_used_total,
            })
            .map_err(|error| RepositoryError::new("aggregate daily worker statistics", error))
    }

    async fn upsert(&self, record: &DailyStatisticsRecord) -> RepositoryResult<()> {
        sqlx::query(STAT_UPSERT_SQL)
            .bind(record.record_at)
            .bind(record.order_count)
            .bind(record.order_total)
            .bind(record.commission_count)
            .bind(record.commission_total)
            .bind(record.paid_count)
            .bind(record.paid_total)
            .bind(record.register_count)
            .bind(record.invite_count)
            .bind(record.transfer_used_total.to_string())
            .bind(record.created_at)
            .bind(record.updated_at)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| RepositoryError::new("upsert daily worker statistics", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_sql_preserves_exact_postgres_text_values() {
        assert_eq!(STAT_AGGREGATE_SQL.matches("AS TEXT").count(), 4);
        assert!(!STAT_AGGREGATE_SQL.contains("AS SIGNED"));
        assert!(STAT_UPSERT_SQL.contains("ON CONFLICT (record_at)"));
        assert!(STAT_UPSERT_SQL.contains("EXCLUDED.order_count"));
    }
}
