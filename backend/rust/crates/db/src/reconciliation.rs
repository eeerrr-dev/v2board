use serde_json::json;
use sqlx::{FromRow, PgPool};
use v2board_application::{
    RepositoryError,
    reconciliation::{
        PaymentReconciliation, ReconciliationPage, ReconciliationQuery, ReconciliationRepository,
        RepositoryResult, ResolutionFilter, ResolveReconciliationOutcome,
    },
};

#[derive(Clone)]
pub struct PostgresReconciliationRepository {
    pool: PgPool,
}

impl PostgresReconciliationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ReconciliationRow {
    id: i64,
    payment_id: i32,
    payment_name: String,
    payment_archived_at: Option<i64>,
    provider: String,
    trade_no: String,
    trade_no_hash: String,
    callback_no: String,
    callback_no_hash: String,
    reason: String,
    order_status: i16,
    expected_amount: i64,
    settled_amount: Option<i64>,
    occurrence_count: i32,
    first_seen_at: i64,
    last_seen_at: i64,
    resolved_at: Option<i64>,
    resolution: Option<String>,
}

impl From<ReconciliationRow> for PaymentReconciliation {
    fn from(row: ReconciliationRow) -> Self {
        Self {
            id: row.id,
            payment_id: row.payment_id,
            payment_name: row.payment_name,
            payment_archived_at: row.payment_archived_at,
            provider: row.provider,
            trade_no: row.trade_no,
            trade_no_hash: row.trade_no_hash,
            callback_no: row.callback_no,
            callback_no_hash: row.callback_no_hash,
            reason: row.reason,
            order_status: row.order_status,
            expected_amount: row.expected_amount,
            settled_amount: row.settled_amount,
            occurrence_count: row.occurrence_count,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
            resolved_at: row.resolved_at,
            resolution: row.resolution,
        }
    }
}

impl ReconciliationRepository for PostgresReconciliationRepository {
    async fn list(&self, query: ReconciliationQuery) -> RepositoryResult<ReconciliationPage> {
        let resolution = match query.resolution {
            ResolutionFilter::Open => 0_i16,
            ResolutionFilter::Resolved => 1_i16,
            ResolutionFilter::All => 2_i16,
        };
        let trade_no_hash = query.trade_no_hash.map(|hash| hash.to_vec());
        let callback_no_hash = query.callback_no_hash.map(|hash| hash.to_vec());
        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM payment_reconciliation r
            WHERE (
                $1::SMALLINT = 2
                OR ($1::SMALLINT = 0 AND r.resolved_at IS NULL)
                OR ($1::SMALLINT = 1 AND r.resolved_at IS NOT NULL)
            )
              AND ($2::INTEGER IS NULL OR r.payment_id = $2)
              AND ($3::TEXT IS NULL OR r.reason = $3)
              AND ($4::BYTEA IS NULL OR r.trade_no_hash = $4)
              AND ($5::BYTEA IS NULL OR r.callback_no_hash = $5)
            "#,
        )
        .bind(resolution)
        .bind(query.payment_id)
        .bind(query.reason.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .fetch_one(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("count payment reconciliations", error))?;

        let items = sqlx::query_as::<_, ReconciliationRow>(
            r#"
            SELECT
                r.id, r.payment_id, p.name AS payment_name,
                p.archived_at AS payment_archived_at, r.provider, r.trade_no,
                encode(r.trade_no_hash, 'hex') AS trade_no_hash,
                r.callback_no,
                encode(r.callback_no_hash, 'hex') AS callback_no_hash,
                r.reason, r.order_status, r.expected_amount, r.settled_amount,
                r.occurrence_count, r.first_seen_at, r.last_seen_at,
                r.resolved_at, r.resolution
            FROM payment_reconciliation r
            JOIN payment_method p ON p.id = r.payment_id
            WHERE (
                $1::SMALLINT = 2
                OR ($1::SMALLINT = 0 AND r.resolved_at IS NULL)
                OR ($1::SMALLINT = 1 AND r.resolved_at IS NOT NULL)
            )
              AND ($2::INTEGER IS NULL OR r.payment_id = $2)
              AND ($3::TEXT IS NULL OR r.reason = $3)
              AND ($4::BYTEA IS NULL OR r.trade_no_hash = $4)
              AND ($5::BYTEA IS NULL OR r.callback_no_hash = $5)
            ORDER BY (r.resolved_at IS NOT NULL) ASC, r.first_seen_at DESC, r.id DESC
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(resolution)
        .bind(query.payment_id)
        .bind(query.reason.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(query.limit)
        .bind(query.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("list payment reconciliations", error))?
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(ReconciliationPage { items, total })
    }

    async fn resolve(
        &self,
        id: i64,
        actor: &str,
        note: &str,
        resolved_at: i64,
    ) -> RepositoryResult<ResolveReconciliationOutcome> {
        let resolution = serde_json::to_string(&json!({ "actor": actor, "note": note }))
            .map_err(|error| RepositoryError::new("encode reconciliation resolution", error))?;
        if resolution.len() > 255 {
            return Ok(ResolveReconciliationOutcome::EncodedResolutionTooLong);
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RepositoryError::new("begin reconciliation resolution", error))?;
        let current = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
            r#"
            SELECT resolved_at, resolution
            FROM payment_reconciliation
            WHERE id = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::new("lock payment reconciliation", error))?;
        let Some((current_resolved_at, current_resolution)) = current else {
            return Ok(ResolveReconciliationOutcome::NotFound);
        };
        if current_resolved_at.is_some() {
            if current_resolution.as_deref() == Some(&resolution) {
                transaction.commit().await.map_err(|error| {
                    RepositoryError::new("commit idempotent reconciliation resolution", error)
                })?;
                return Ok(ResolveReconciliationOutcome::AlreadyResolvedIdentically);
            }
            return Ok(ResolveReconciliationOutcome::AlreadyProcessed);
        }
        let result = sqlx::query(
            r#"
            UPDATE payment_reconciliation
            SET resolved_at = $1, resolution = $2
            WHERE id = $3 AND resolved_at IS NULL
            "#,
        )
        .bind(resolved_at)
        .bind(resolution)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::new("resolve payment reconciliation", error))?;
        if result.rows_affected() != 1 {
            return Ok(ResolveReconciliationOutcome::AlreadyProcessed);
        }
        transaction
            .commit()
            .await
            .map_err(|error| RepositoryError::new("commit reconciliation resolution", error))?;
        Ok(ResolveReconciliationOutcome::Resolved)
    }
}
