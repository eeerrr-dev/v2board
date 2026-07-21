//! PostgreSQL adapter for the append-only privileged mutation audit trail.

use v2board_application::{
    RepositoryError,
    audit::{AuditRepository, PrivilegedMutationAudit, RepositoryResult},
};

use crate::DbPool;

#[derive(Clone)]
pub struct PostgresAuditRepository {
    pool: DbPool,
}

impl PostgresAuditRepository {
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl AuditRepository for PostgresAuditRepository {
    async fn append(&self, record: PrivilegedMutationAudit) -> RepositoryResult<()> {
        sqlx::query(
            "INSERT INTO audit_log \
             (actor_id, actor_email, session_id, surface, method, path, status_code, client_ip, request_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(record.actor_id)
        .bind(record.actor_email)
        .bind(record.session_id)
        .bind(record.surface)
        .bind(record.method)
        .bind(record.path)
        .bind(record.status_code)
        .bind(record.client_ip)
        .bind(record.request_id)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("append privileged mutation audit", error))?;
        Ok(())
    }
}
