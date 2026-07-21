use std::collections::BTreeMap;

use sqlx::{PgPool, Postgres, QueryBuilder};
use v2board_application::{
    RepositoryError,
    statistics::{
        RepositoryResult, ServerRankSource, StatSeriesSource, StatisticsBoundaries,
        StatisticsBucket, StatisticsRepository, StatisticsSummary, UserRankSource, UserTraffic,
    },
};

const SERVER_TABLES: &[(&str, &str)] = &[
    ("shadowsocks", "server_shadowsocks"),
    ("vmess", "server_vmess"),
    ("trojan", "server_trojan"),
    ("tuic", "server_tuic"),
    ("hysteria", "server_hysteria"),
    ("vless", "server_vless"),
    ("anytls", "server_anytls"),
    ("v2node", "server_v2node"),
];

#[derive(Clone, Debug)]
pub struct PostgresStatisticsRepository {
    pool: PgPool,
}

impl PostgresStatisticsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn exact_sum(
        &self,
        statement: &'static str,
        start: i64,
        end: i64,
        operation: &'static str,
    ) -> RepositoryResult<i64> {
        let value: String = sqlx::query_scalar(statement)
            .bind(start)
            .bind(end)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error(operation, error))?;
        value
            .parse::<i64>()
            .map_err(|error| repository_error(operation, error))
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

impl StatisticsRepository for PostgresStatisticsRepository {
    async fn summary(
        &self,
        boundaries: StatisticsBoundaries,
    ) -> RepositoryResult<StatisticsSummary> {
        const INCOME: &str = "SELECT CAST(COALESCE(SUM(total_amount), 0) AS TEXT) FROM orders \
             WHERE created_at >= $1 AND created_at < $2 AND status NOT IN (0, 2)";
        const PAYOUT: &str = "SELECT CAST(COALESCE(SUM(get_amount), 0) AS TEXT) FROM commission_log \
             WHERE created_at >= $1 AND created_at < $2";

        let online_user = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE t >= $1")
            .bind(boundaries.now.saturating_sub(600))
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count online users", error))?;
        let month_income = self
            .exact_sum(INCOME, boundaries.month, boundaries.now, "sum month income")
            .await?;
        let day_income = self
            .exact_sum(INCOME, boundaries.today, boundaries.now, "sum day income")
            .await?;
        let last_month_income = self
            .exact_sum(
                INCOME,
                boundaries.previous_month,
                boundaries.month,
                "sum previous month income",
            )
            .await?;
        let month_register_total = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(boundaries.month)
        .bind(boundaries.now)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("count month registrations", error))?;
        let day_register_total = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(boundaries.today)
        .bind(boundaries.now)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("count day registrations", error))?;
        let ticket_pending_total =
            sqlx::query_scalar("SELECT COUNT(*) FROM ticket WHERE status = 0 AND reply_status = 0")
                .fetch_one(&self.pool)
                .await
                .map_err(|error| repository_error("count pending tickets", error))?;
        let commission_pending_total = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE commission_status = 0 AND invite_user_id IS NOT NULL \
             AND status NOT IN (0, 2) AND commission_balance > 0",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("count pending commissions", error))?;
        let payment_reconciliation_pending_total = sqlx::query_scalar(
            "SELECT COUNT(*) FROM payment_reconciliation WHERE resolved_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("count pending payment reconciliations", error))?;
        let pending_amount: String = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(COALESCE(settled_amount, expected_amount)), 0) AS TEXT) \
             FROM payment_reconciliation WHERE resolved_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("sum pending payment reconciliations", error))?;
        let payment_reconciliation_pending_amount = pending_amount
            .parse::<i64>()
            .map_err(|error| repository_error("decode pending payment reconciliations", error))?;
        let commission_month_payout = self
            .exact_sum(
                PAYOUT,
                boundaries.month,
                boundaries.now,
                "sum month payouts",
            )
            .await?;
        let commission_last_month_payout = self
            .exact_sum(
                PAYOUT,
                boundaries.previous_month,
                boundaries.month,
                "sum previous month payouts",
            )
            .await?;

        Ok(StatisticsSummary {
            online_user,
            month_income,
            month_register_total,
            day_register_total,
            ticket_pending_total,
            commission_pending_total,
            payment_reconciliation_pending_total,
            payment_reconciliation_pending_amount,
            day_income,
            last_month_income,
            commission_month_payout,
            commission_last_month_payout,
        })
    }

    async fn server_rank_sources(
        &self,
        start: i64,
        end: i64,
    ) -> RepositoryResult<(Vec<ServerRankSource>, BTreeMap<(String, i64), String>)> {
        let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
            "SELECT server_id::BIGINT, server_type, u, d FROM server_traffic \
             WHERE record_at >= $1 AND record_at < $2 AND record_type = 'd' \
             ORDER BY (CAST(u AS NUMERIC(30,0)) + CAST(d AS NUMERIC(30,0))) DESC LIMIT 15",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("load server rank sources", error))?;
        let mut names = BTreeMap::new();
        for (kind, table) in SERVER_TABLES {
            let node_rows: Vec<(i64, String)> = QueryBuilder::<Postgres>::new(format!(
                "SELECT id::BIGINT, name FROM {table} WHERE parent_id IS NULL"
            ))
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("load server rank names", error))?;
            for (id, name) in node_rows {
                names.insert(((*kind).to_string(), id), name);
            }
        }
        Ok((
            rows.into_iter()
                .map(
                    |(server_id, server_type, upload, download)| ServerRankSource {
                        server_id,
                        server_type,
                        upload,
                        download,
                    },
                )
                .collect(),
            names,
        ))
    }

    async fn user_rank_sources(
        &self,
        start: i64,
        end: i64,
    ) -> RepositoryResult<Vec<UserRankSource>> {
        let rows: Vec<(i64, f64, i64, i64, Option<String>)> = sqlx::query_as(
            "SELECT s.user_id, CAST(s.server_rate AS DOUBLE PRECISION), s.u, s.d, u.email \
             FROM user_traffic s LEFT JOIN users u ON u.id = s.user_id \
             WHERE s.record_at >= $1 AND s.record_at < $2 AND s.record_type = 'd' \
             ORDER BY (CAST(s.u AS NUMERIC(30,0)) + CAST(s.d AS NUMERIC(30,0))) DESC LIMIT 30",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("load user rank sources", error))?;
        Ok(rows
            .into_iter()
            .map(
                |(user_id, server_rate, upload, download, email)| UserRankSource {
                    user_id,
                    server_rate,
                    upload,
                    download,
                    email,
                },
            )
            .collect())
    }

    async fn series_sources(
        &self,
        bucket: StatisticsBucket,
    ) -> RepositoryResult<Vec<StatSeriesSource>> {
        let rows: Vec<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT record_at, register_count::BIGINT, paid_total, paid_count::BIGINT, \
                    commission_total, commission_count::BIGINT \
             FROM stat WHERE record_type = $1 ORDER BY record_at DESC LIMIT 31",
        )
        .bind(bucket.storage_code())
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("load statistic series", error))?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    recorded_at,
                    register_count,
                    paid_total,
                    paid_count,
                    commission_total,
                    commission_count,
                )| StatSeriesSource {
                    recorded_at,
                    register_count,
                    paid_total,
                    paid_count,
                    commission_total,
                    commission_count,
                },
            )
            .collect())
    }

    async fn user_traffic(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepositoryResult<(Vec<UserTraffic>, i64)> {
        let total = sqlx::query_scalar("SELECT COUNT(*) FROM user_traffic WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count user traffic", error))?;
        let rows: Vec<(i64, i64, i64, f64)> = sqlx::query_as(
            "SELECT record_at, u, d, CAST(server_rate AS DOUBLE PRECISION) \
             FROM user_traffic WHERE user_id = $1 \
             ORDER BY record_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("load user traffic", error))?;
        Ok((
            rows.into_iter()
                .map(|(recorded_at, upload, download, server_rate)| UserTraffic {
                    recorded_at,
                    upload,
                    download,
                    server_rate,
                })
                .collect(),
            total,
        ))
    }
}
