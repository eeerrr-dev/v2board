//! PostgreSQL adapters for authentication and registration transactions.

use sqlx::{FromRow, PgPool, Postgres, Transaction};
use v2board_application::{
    RepositoryError,
    auth::{
        AuthAccount, AuthRepository, InsertAuthAccountOutcome, InviteCodeRecord, MfaRecord,
        NewAuthAccount, RegistrationTransaction, RepositoryResult, SealedMfaSecret,
        TrialPlanRecord,
    },
};

#[derive(Clone, Debug)]
pub struct PostgresAuthRepository {
    pool: PgPool,
}

impl PostgresAuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub struct PostgresRegistration<'a> {
    transaction: Transaction<'a, Postgres>,
}

#[derive(FromRow)]
struct InviteCodeRow {
    id: i32,
    user_id: i64,
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

fn account(row: crate::user::UserAuthRow) -> AuthAccount {
    AuthAccount {
        id: row.id,
        email: row.email,
        password_hash: row.password,
        password_algo: row.password_algo,
        password_salt: row.password_salt,
        session_epoch: row.session_epoch,
        banned: row.banned != 0,
        is_admin: row.is_admin,
        is_staff: row.is_staff,
        admin_permissions: row.admin_permissions.0,
    }
}

impl RegistrationTransaction for PostgresRegistration<'_> {
    async fn lock_invite_code(&mut self, code: &str) -> RepositoryResult<Option<InviteCodeRecord>> {
        sqlx::query_as::<_, InviteCodeRow>(
            "SELECT id, user_id FROM invite_code \
             WHERE lower(code) = lower($1) AND status = 0 LIMIT 1 FOR UPDATE",
        )
        .bind(code)
        .fetch_optional(&mut *self.transaction)
        .await
        .map(|row| {
            row.map(|row| InviteCodeRecord {
                id: row.id,
                user_id: row.user_id,
            })
        })
        .map_err(|error| repository_error("lock registration invite code", error))
    }

    async fn consume_invite_code(&mut self, id: i32, updated_at: i64) -> RepositoryResult<bool> {
        sqlx::query(
            "UPDATE invite_code SET status = 1, updated_at = $1 WHERE id = $2 AND status = 0",
        )
        .bind(updated_at)
        .bind(id)
        .execute(&mut *self.transaction)
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| repository_error("consume registration invite code", error))
    }

    async fn lock_trial_plan(&mut self, plan_id: i32) -> RepositoryResult<Option<TrialPlanRecord>> {
        crate::plan::find_plan_binding_for_share(&mut self.transaction, plan_id)
            .await
            .map(|plan| {
                plan.map(|plan| TrialPlanRecord {
                    id: plan.id,
                    group_id: plan.group_id,
                    transfer_gib: plan.transfer_enable,
                    device_limit: plan.device_limit,
                    speed_limit: plan.speed_limit,
                })
            })
            .map_err(|error| repository_error("lock registration trial plan", error))
    }

    async fn insert_account(
        &mut self,
        account: NewAuthAccount,
    ) -> RepositoryResult<InsertAuthAccountOutcome> {
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO users (
                invite_user_id, email, password, uuid, token, transfer_enable, device_limit,
                group_id, plan_id, speed_limit, expired_at, last_login_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(account.invite_user_id)
        .bind(account.email)
        .bind(account.password_hash)
        .bind(account.uuid)
        .bind(account.token)
        .bind(account.transfer_enable)
        .bind(account.device_limit)
        .bind(account.group_id)
        .bind(account.plan_id)
        .bind(account.speed_limit)
        .bind(account.expired_at)
        .bind(account.created_at)
        .bind(account.created_at)
        .bind(account.created_at)
        .fetch_one(&mut *self.transaction)
        .await;
        match result {
            Ok(user_id) => Ok(InsertAuthAccountOutcome::Inserted(user_id)),
            Err(error) if is_email_unique_violation(&error) => {
                Ok(InsertAuthAccountOutcome::EmailAlreadyRegistered)
            }
            Err(error) => Err(repository_error("insert registered account", error)),
        }
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit account registration", error))
    }
}

impl AuthRepository for PostgresAuthRepository {
    type Registration<'a> = PostgresRegistration<'a>;

    async fn begin_registration(&self) -> RepositoryResult<Self::Registration<'_>> {
        self.pool
            .begin()
            .await
            .map(|transaction| PostgresRegistration { transaction })
            .map_err(|error| repository_error("begin account registration", error))
    }

    async fn find_account_by_email(&self, email: &str) -> RepositoryResult<Option<AuthAccount>> {
        crate::user::find_user_for_auth(&self.pool, email)
            .await
            .map(|row| row.map(account))
            .map_err(|error| repository_error("find authentication account by email", error))
    }

    async fn find_account_by_id(&self, user_id: i64) -> RepositoryResult<Option<AuthAccount>> {
        crate::user::find_user_for_auth_by_id(&self.pool, user_id)
            .await
            .map(|row| row.map(account))
            .map_err(|error| repository_error("find authentication account by id", error))
    }

    async fn active_session_epoch(&self, user_id: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar("SELECT session_epoch FROM users WHERE id = $1 AND banned = 0 LIMIT 1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| repository_error("find active account session epoch", error))
    }

    async fn rehash_password(
        &self,
        user_id: i64,
        expected_hash: &str,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<()> {
        crate::user::rehash_password(
            &self.pool,
            user_id,
            expected_hash,
            password_hash,
            updated_at,
        )
        .await
        .map(|_| ())
        .map_err(|error| repository_error("rehash authenticated password", error))
    }

    async fn update_password(
        &self,
        user_id: i64,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::user::update_password(&self.pool, user_id, password_hash, updated_at)
            .await
            .map_err(|error| repository_error("update account password", error))
    }

    async fn change_password_if_current(
        &self,
        user_id: i64,
        expected_hash: &str,
        expected_session_epoch: i64,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::user::change_password_if_current(
            &self.pool,
            user_id,
            expected_hash,
            expected_session_epoch,
            password_hash,
            updated_at,
        )
        .await
        .map_err(|error| repository_error("change account password", error))
    }

    async fn update_security(
        &self,
        user_id: i64,
        uuid: &str,
        token: &str,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::user::update_security(&self.pool, user_id, uuid, token, updated_at)
            .await
            .map_err(|error| repository_error("rotate account security identifiers", error))
    }

    async fn increment_invite_view(&self, code: &str, updated_at: i64) -> RepositoryResult<()> {
        sqlx::query(
            "UPDATE invite_code SET pv = pv + 1, updated_at = $1 WHERE lower(code) = lower($2)",
        )
        .bind(updated_at)
        .bind(code)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|error| repository_error("increment invite-code view", error))
    }

    async fn find_mfa(&self, user_id: i64) -> RepositoryResult<Option<MfaRecord>> {
        crate::admin_mfa::find(&self.pool, user_id)
            .await
            .map(|row| {
                row.map(|row| MfaRecord {
                    secret_nonce: row.secret_nonce,
                    secret_ciphertext: row.secret_ciphertext,
                    secret_tag: row.secret_tag,
                    enabled_at: row.enabled_at,
                    last_step: row.last_step,
                })
            })
            .map_err(|error| repository_error("find account mfa", error))
    }

    async fn upsert_pending_mfa(
        &self,
        user_id: i64,
        secret: &SealedMfaSecret,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::admin_mfa::upsert_pending(
            &self.pool,
            user_id,
            &secret.secret_nonce,
            &secret.secret_ciphertext,
            &secret.secret_tag,
            updated_at,
        )
        .await
        .map(|rows| rows == 1)
        .map_err(|error| repository_error("store pending account mfa", error))
    }

    async fn enable_mfa(
        &self,
        user_id: i64,
        accepted_step: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::admin_mfa::enable(&self.pool, user_id, accepted_step, updated_at)
            .await
            .map(|rows| rows == 1)
            .map_err(|error| repository_error("enable account mfa", error))
    }

    async fn consume_mfa_step(
        &self,
        user_id: i64,
        accepted_step: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::admin_mfa::consume_step(&self.pool, user_id, accepted_step, updated_at)
            .await
            .map(|rows| rows == 1)
            .map_err(|error| repository_error("consume account mfa step", error))
    }

    async fn disable_mfa(&self, user_id: i64, accepted_step: i64) -> RepositoryResult<bool> {
        crate::admin_mfa::disable(&self.pool, user_id, accepted_step)
            .await
            .map(|rows| rows == 1)
            .map_err(|error| repository_error("disable account mfa", error))
    }
}

fn is_email_unique_violation(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|error| {
        error.is_unique_violation()
            && matches!(error.constraint(), Some("uniq_user_email_canonical"))
    })
}
