use sqlx::PgPool;
use v2board_application::{
    RepositoryError,
    operator_access::{OperatorAccessRepository, OperatorMfaResetOutcome, RepositoryResult},
};

#[derive(Clone, Debug)]
pub struct PostgresOperatorAccessRepository {
    pool: PgPool,
}

impl PostgresOperatorAccessRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

impl OperatorAccessRepository for PostgresOperatorAccessRepository {
    async fn reset_privileged_mfa(&self, email: &str) -> RepositoryResult<OperatorMfaResetOutcome> {
        let (account_exists, factor_deleted): (bool, bool) = sqlx::query_as(
            r#"
            WITH privileged AS MATERIALIZED (
                SELECT id
                FROM users
                WHERE lower(btrim(email)) = lower(btrim($1))
                  AND (is_admin = 1 OR is_staff = 1)
                LIMIT 1
            ), deleted AS (
                DELETE FROM admin_mfa
                WHERE user_id = (SELECT id FROM privileged)
                RETURNING user_id
            )
            SELECT
                EXISTS(SELECT 1 FROM privileged),
                EXISTS(SELECT 1 FROM deleted)
            "#,
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("reset privileged operator MFA", error))?;
        Ok(match (account_exists, factor_deleted) {
            (false, _) => OperatorMfaResetOutcome::AccountNotFound,
            (true, false) => OperatorMfaResetOutcome::NoFactorConfigured,
            (true, true) => OperatorMfaResetOutcome::Reset,
        })
    }

    async fn replace_admin_password(
        &self,
        email: &str,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar(
            r#"
            UPDATE users
            SET password = $1,
                password_algo = NULL,
                password_salt = NULL,
                session_epoch = session_epoch + 1,
                updated_at = $2
            WHERE id = (
                SELECT id
                FROM users
                WHERE lower(btrim(email)) = lower(btrim($3))
                  AND is_admin = 1
                LIMIT 1
            )
            RETURNING id
            "#,
        )
        .bind(password_hash)
        .bind(updated_at)
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("replace operator administrator password", error))
    }
}
