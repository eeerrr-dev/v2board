use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use v2board_application::{
    RepositoryError,
    maintenance::{
        RepositoryResult, RetentionCutoff, RetentionDataset, RetentionRepository,
        ScheduledResetCandidate, ScheduledTrafficResetBatch, ScheduledTrafficResetRepository,
    },
};

const USER_TRAFFIC_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id FROM user_traffic
    WHERE record_at < $1
    ORDER BY record_at, id
    LIMIT $2
)
DELETE FROM user_traffic AS target
USING doomed
WHERE target.id = doomed.id
"#;

const SERVER_TRAFFIC_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id FROM server_traffic
    WHERE record_at < $1
    ORDER BY record_at, id
    LIMIT $2
)
DELETE FROM server_traffic AS target
USING doomed
WHERE target.id = doomed.id
"#;

const SYSTEM_LOG_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id FROM system_log
    WHERE created_at < $1
    ORDER BY created_at, id
    LIMIT $2
)
DELETE FROM system_log AS target
USING doomed
WHERE target.id = doomed.id
"#;

#[derive(Clone, Debug)]
pub struct PostgresMaintenanceRepository {
    pool: PgPool,
}

impl PostgresMaintenanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub struct PostgresScheduledTrafficResetBatch<'a> {
    transaction: Transaction<'a, Postgres>,
}

#[derive(Debug, FromRow)]
struct ResetCandidateRow {
    id: i64,
    expired_at: i64,
    reset_traffic_method: Option<i16>,
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

impl ScheduledTrafficResetBatch for PostgresScheduledTrafficResetBatch<'_> {
    async fn lock_candidates(
        &mut self,
        after_id: i64,
        now_epoch: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<ScheduledResetCandidate>> {
        sqlx::query_as::<_, ResetCandidateRow>(
            r#"
            SELECT u.id, u.expired_at, p.reset_traffic_method
            FROM users u
            INNER JOIN plan p ON p.id = u.plan_id
            WHERE u.id > $1
              AND u.expired_at IS NOT NULL
              AND u.expired_at > $2
            ORDER BY u.id
            LIMIT $3
            FOR UPDATE
            "#,
        )
        .bind(after_id)
        .bind(now_epoch)
        .bind(limit)
        .fetch_all(&mut *self.transaction)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| ScheduledResetCandidate {
                    id: row.id,
                    expired_at: row.expired_at,
                    reset_traffic_method: row.reset_traffic_method,
                })
                .collect()
        })
        .map_err(|error| repository_error("lock scheduled traffic reset candidates", error))
    }

    async fn apply_reset(
        &mut self,
        user_ids: &[i64],
        reset_key: &str,
        updated_at: i64,
    ) -> RepositoryResult<u64> {
        if user_ids.is_empty() {
            return Ok(0);
        }
        let mut builder = QueryBuilder::<Postgres>::new(
            "UPDATE users SET traffic_epoch = traffic_epoch + 1, \
             u = 0, d = 0, scheduled_traffic_reset_key = ",
        );
        builder.push_bind(reset_key);
        builder.push(", updated_at = ");
        builder.push_bind(updated_at);
        builder.push(" WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for user_id in user_ids {
                separated.push_bind(user_id);
            }
        }
        builder
            .push(") AND (scheduled_traffic_reset_key IS NULL OR scheduled_traffic_reset_key <> ");
        builder.push_bind(reset_key);
        builder.push(")");
        builder
            .build()
            .execute(&mut *self.transaction)
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| repository_error("apply scheduled traffic resets", error))
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit scheduled traffic reset batch", error))
    }
}

impl ScheduledTrafficResetRepository for PostgresMaintenanceRepository {
    type Batch<'a> = PostgresScheduledTrafficResetBatch<'a>;

    async fn begin_batch(&self) -> RepositoryResult<Self::Batch<'_>> {
        self.pool
            .begin()
            .await
            .map(|transaction| PostgresScheduledTrafficResetBatch { transaction })
            .map_err(|error| repository_error("begin scheduled traffic reset batch", error))
    }
}

impl RetentionRepository for PostgresMaintenanceRepository {
    async fn delete_batch(
        &self,
        cutoff: RetentionCutoff,
        batch_size: i64,
    ) -> RepositoryResult<u64> {
        let statement = match cutoff.dataset {
            RetentionDataset::UserTraffic => USER_TRAFFIC_RETENTION_SQL,
            RetentionDataset::ServerTraffic => SERVER_TRAFFIC_RETENTION_SQL,
            RetentionDataset::SystemLog => SYSTEM_LOG_RETENTION_SQL,
        };
        sqlx::query(statement)
            .bind(cutoff.before)
            .bind(batch_size)
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| repository_error("delete retained maintenance rows", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_statements_are_index_ordered_and_bounded() {
        for statement in [
            USER_TRAFFIC_RETENTION_SQL,
            SERVER_TRAFFIC_RETENTION_SQL,
            SYSTEM_LOG_RETENTION_SQL,
        ] {
            assert!(statement.contains("WITH doomed AS"));
            assert!(statement.contains("ORDER BY"));
            assert!(statement.contains("LIMIT $2"));
            assert!(statement.contains("USING doomed"));
        }
    }
}
