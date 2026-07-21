use sqlx::{FromRow, PgPool};
use v2board_application::{
    RepositoryError,
    telegram::{
        BindTelegramOutcome, RepositoryResult, TelegramRepository, TelegramUser,
        UnbindTelegramOutcome,
    },
};

#[derive(Clone)]
pub struct PostgresTelegramRepository {
    pool: PgPool,
}

impl PostgresTelegramRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct TelegramUserRow {
    id: i64,
    email: String,
    is_admin: i16,
    is_staff: i16,
    u: i64,
    d: i64,
    transfer_enable: i64,
    banned: i16,
    expired_at: Option<i64>,
}

impl From<TelegramUserRow> for TelegramUser {
    fn from(row: TelegramUserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            is_admin: row.is_admin != 0,
            is_staff: row.is_staff != 0,
            uploaded: row.u,
            downloaded: row.d,
            transfer_enable: row.transfer_enable,
            banned: row.banned != 0,
            expired_at: row.expired_at,
        }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

impl TelegramRepository for PostgresTelegramRepository {
    async fn user_by_telegram_id(
        &self,
        telegram_id: i64,
    ) -> RepositoryResult<Option<TelegramUser>> {
        sqlx::query_as::<_, TelegramUserRow>(
            r#"
            SELECT id, email, is_admin, is_staff, u, d, transfer_enable, banned, expired_at
            FROM users
            WHERE telegram_id = $1
            LIMIT 1
            "#,
        )
        .bind(telegram_id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| repository_error("find Telegram user", error))
    }

    async fn bind_telegram(
        &self,
        token: &str,
        telegram_id: i64,
        updated_at: i64,
    ) -> RepositoryResult<BindTelegramOutcome> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin Telegram binding", error))?;
        let row = sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT id, telegram_id FROM users WHERE token = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(token)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| repository_error("lock Telegram binding user", error))?;
        let Some((user_id, existing_telegram_id)) = row else {
            return Ok(BindTelegramOutcome::UserNotFound);
        };
        if existing_telegram_id.is_some() {
            return Ok(BindTelegramOutcome::AlreadyBound);
        }
        sqlx::query("UPDATE users SET telegram_id = $1, updated_at = $2 WHERE id = $3")
            .bind(telegram_id)
            .bind(updated_at)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| repository_error("bind Telegram user", error))?;
        transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit Telegram binding", error))?;
        Ok(BindTelegramOutcome::Bound)
    }

    async fn unbind_telegram(
        &self,
        telegram_id: i64,
        updated_at: i64,
    ) -> RepositoryResult<UnbindTelegramOutcome> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin Telegram unbind", error))?;
        let user_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE telegram_id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(telegram_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| repository_error("lock Telegram user for unbind", error))?;
        let Some(user_id) = user_id else {
            return Ok(UnbindTelegramOutcome::UserNotFound);
        };
        sqlx::query("UPDATE users SET telegram_id = NULL, updated_at = $1 WHERE id = $2")
            .bind(updated_at)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| repository_error("unbind Telegram user", error))?;
        transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit Telegram unbind", error))?;
        Ok(UnbindTelegramOutcome::Unbound)
    }

    async fn admin_recipients(&self, include_staff: bool) -> RepositoryResult<Vec<i64>> {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT telegram_id
            FROM users
            WHERE telegram_id IS NOT NULL
              AND (is_admin = 1 OR ($1 AND is_staff = 1))
            ORDER BY id
            "#,
        )
        .bind(include_staff)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("list Telegram admin recipients", error))
    }
}
