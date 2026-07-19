use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow)]
pub struct UserAuthRow {
    pub id: i64,
    pub email: String,
    pub password: String,
    pub password_algo: Option<String>,
    pub password_salt: Option<String>,
    pub session_epoch: i64,
    pub banned: i16,
    pub is_admin: i16,
    pub is_staff: i16,
}

#[derive(Debug, Clone, FromRow)]
struct RawUserInfoRow {
    pub email: String,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub banned: i16,
    pub auto_renewal: Option<i16>,
    pub remind_expire: Option<i16>,
    pub remind_traffic: Option<i16>,
    pub expired_at: Option<i64>,
    pub balance: i32,
    pub commission_balance: i32,
    pub plan_id: Option<i32>,
    pub discount: Option<i32>,
    pub commission_rate: Option<i32>,
    pub telegram_id: Option<i64>,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserInfoRow {
    pub email: String,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub banned: i16,
    pub auto_renewal: Option<i16>,
    pub remind_expire: Option<i16>,
    pub remind_traffic: Option<i16>,
    pub expired_at: Option<i64>,
    pub balance: i32,
    pub commission_balance: i32,
    pub plan_id: Option<i32>,
    pub discount: Option<i32>,
    pub commission_rate: Option<i32>,
    pub telegram_id: Option<i64>,
    pub uuid: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserSubscribeRow {
    pub plan_id: Option<i32>,
    pub token: String,
    pub expired_at: Option<i64>,
    pub u: i64,
    pub d: i64,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub email: String,
    pub uuid: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserAccessRow {
    pub id: i64,
    pub token: String,
    pub uuid: String,
    pub group_id: Option<i32>,
    pub plan_id: Option<i32>,
    pub banned: i16,
    pub u: i64,
    pub d: i64,
    pub transfer_enable: i64,
    pub expired_at: Option<i64>,
    pub commission_balance: i32,
}

pub async fn find_user_for_auth(
    pool: &PgPool,
    email: &str,
) -> Result<Option<UserAuthRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAuthRow>(
        r#"
        SELECT id, email, password, password_algo, password_salt, session_epoch, banned, is_admin, is_staff
        FROM users
        WHERE lower(btrim(email)) = lower(btrim($1))
        LIMIT 1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_for_auth_by_id(
    pool: &PgPool,
    id: i64,
) -> Result<Option<UserAuthRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAuthRow>(
        r#"
        SELECT id, email, password, password_algo, password_salt, session_epoch, banned, is_admin, is_staff
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_info(pool: &PgPool, id: i64) -> Result<Option<UserInfoRow>, sqlx::Error> {
    let raw = sqlx::query_as::<_, RawUserInfoRow>(
        r#"
        SELECT
            email,
            transfer_enable,
            device_limit,
            last_login_at,
            created_at,
            banned,
            auto_renewal,
            remind_expire,
            remind_traffic,
            expired_at,
            balance,
            commission_balance,
            plan_id,
            discount,
            commission_rate,
            telegram_id,
            uuid
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(raw.map(|raw| {
        let avatar_hash = format!("{:x}", md5::compute(raw.email.as_bytes()));
        UserInfoRow {
            email: raw.email,
            transfer_enable: raw.transfer_enable,
            device_limit: raw.device_limit,
            last_login_at: raw.last_login_at,
            created_at: raw.created_at,
            banned: raw.banned,
            auto_renewal: raw.auto_renewal,
            remind_expire: raw.remind_expire,
            remind_traffic: raw.remind_traffic,
            expired_at: raw.expired_at,
            balance: raw.balance,
            commission_balance: raw.commission_balance,
            plan_id: raw.plan_id,
            discount: raw.discount,
            commission_rate: raw.commission_rate,
            telegram_id: raw.telegram_id,
            uuid: raw.uuid,
            avatar_url: format!("https://cravatar.cn/avatar/{avatar_hash}?s=64&d=identicon"),
        }
    }))
}

pub async fn find_user_subscribe(
    pool: &PgPool,
    id: i64,
) -> Result<Option<UserSubscribeRow>, sqlx::Error> {
    sqlx::query_as::<_, UserSubscribeRow>(
        r#"
        SELECT plan_id, token, expired_at, u, d, transfer_enable, device_limit, email, uuid
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_access(
    pool: &PgPool,
    id: i64,
) -> Result<Option<UserAccessRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAccessRow>(
        r#"
        SELECT id, token, uuid, group_id, plan_id, banned, u, d, transfer_enable, expired_at, commission_balance
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_access_by_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<UserAccessRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAccessRow>(
        r#"
        SELECT id, token, uuid, group_id, plan_id, banned, u, d, transfer_enable, expired_at, commission_balance
        FROM users
        WHERE token = $1
        LIMIT 1
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

/// PATCH /user/profile preference write (docs/api-dialect.md §4.4): each
/// field is tri-state — outer `None` retains the current value, `Some(None)`
/// clears to NULL, `Some(Some(flag))` sets it. COALESCE cannot express the
/// clear arm, so a set-marker drives a CASE per column.
pub async fn update_preferences(
    pool: &PgPool,
    id: i64,
    auto_renewal: Option<Option<i16>>,
    remind_expire: Option<Option<i16>>,
    remind_traffic: Option<Option<i16>>,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE users
        SET
            auto_renewal = CASE WHEN $1 THEN $2 ELSE auto_renewal END,
            remind_expire = CASE WHEN $3 THEN $4 ELSE remind_expire END,
            remind_traffic = CASE WHEN $5 THEN $6 ELSE remind_traffic END,
            updated_at = $7
        WHERE id = $8
        "#,
    )
    .bind(auto_renewal.is_some())
    .bind(auto_renewal.flatten())
    .bind(remind_expire.is_some())
    .bind(remind_expire.flatten())
    .bind(remind_traffic.is_some())
    .bind(remind_traffic.flatten())
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_telegram_id(pool: &PgPool, id: i64, now: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE users SET telegram_id = NULL, updated_at = $1 WHERE id = $2")
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_security(
    pool: &PgPool,
    id: i64,
    uuid: &str,
    token: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("UPDATE users SET uuid = $1, token = $2, updated_at = $3 WHERE id = $4")
            .bind(uuid)
            .bind(token)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_password(
    pool: &PgPool,
    id: i64,
    password_hash: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET password = $1, password_algo = NULL, password_salt = NULL,
            session_epoch = session_epoch + 1, updated_at = $2
        WHERE id = $3
        "#,
    )
    .bind(password_hash)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn change_password_if_current(
    pool: &PgPool,
    id: i64,
    expected_hash: &str,
    expected_session_epoch: i64,
    password_hash: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET password = $1, password_algo = NULL, password_salt = NULL,
            session_epoch = session_epoch + 1, updated_at = $2
        WHERE id = $3 AND password = $4 AND session_epoch = $5
        "#,
    )
    .bind(password_hash)
    .bind(now)
    .bind(id)
    .bind(expected_hash)
    .bind(expected_session_epoch)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Upgrades a successfully verified legacy password without revoking the session that is
/// currently being created. The compare-and-set avoids overwriting a concurrent password reset.
pub async fn rehash_password(
    pool: &PgPool,
    id: i64,
    expected_hash: &str,
    password_hash: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET password = $1, password_algo = NULL, password_salt = NULL, updated_at = $2
        WHERE id = $3 AND password = $4
        "#,
    )
    .bind(password_hash)
    .bind(now)
    .bind(id)
    .bind(expected_hash)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn count_pending_orders(pool: &PgPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE status = 0 AND user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

pub async fn count_pending_tickets(pool: &PgPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ticket WHERE status = 0 AND user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

pub async fn count_invited_users(pool: &PgPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE invite_user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn authentication_lookup_uses_the_canonical_email_index() {
        // The case/whitespace-insensitive email authentication lookup depends on
        // the canonical unique index that forbids duplicate normalized emails.
        let finalize = include_str!("../../../migrations-postgres/0002_import_finalize.sql");
        assert!(finalize.contains(
            "CREATE UNIQUE INDEX uniq_user_email_canonical ON users((lower(btrim(email))))"
        ));
    }
}
