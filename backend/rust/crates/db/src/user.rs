use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, FromRow)]
pub struct UserAuthRow {
    pub id: i64,
    pub email: String,
    pub password: String,
    pub password_algo: Option<String>,
    pub password_salt: Option<String>,
    pub token: String,
    pub banned: i8,
    pub is_admin: i8,
    pub is_staff: i8,
}

#[derive(Debug, Clone, FromRow)]
struct RawUserInfoRow {
    pub email: String,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub banned: i8,
    pub auto_renewal: Option<i8>,
    pub remind_expire: Option<i8>,
    pub remind_traffic: Option<i8>,
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
    pub banned: i8,
    pub auto_renewal: Option<i8>,
    pub remind_expire: Option<i8>,
    pub remind_traffic: Option<i8>,
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
    pub banned: i8,
    pub u: i64,
    pub d: i64,
    pub transfer_enable: i64,
    pub expired_at: Option<i64>,
    pub commission_balance: i32,
}

pub async fn find_user_for_auth(
    pool: &MySqlPool,
    email: &str,
) -> Result<Option<UserAuthRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAuthRow>(
        r#"
        SELECT id, email, password, password_algo, password_salt, token, banned, is_admin, is_staff
        FROM v2_user
        WHERE email = ?
        LIMIT 1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_for_auth_by_id(
    pool: &MySqlPool,
    id: i64,
) -> Result<Option<UserAuthRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAuthRow>(
        r#"
        SELECT id, email, password, password_algo, password_salt, token, banned, is_admin, is_staff
        FROM v2_user
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_info(pool: &MySqlPool, id: i64) -> Result<Option<UserInfoRow>, sqlx::Error> {
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
        FROM v2_user
        WHERE id = ?
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
    pool: &MySqlPool,
    id: i64,
) -> Result<Option<UserSubscribeRow>, sqlx::Error> {
    sqlx::query_as::<_, UserSubscribeRow>(
        r#"
        SELECT plan_id, token, expired_at, u, d, transfer_enable, device_limit, email, uuid
        FROM v2_user
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_access(
    pool: &MySqlPool,
    id: i64,
) -> Result<Option<UserAccessRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAccessRow>(
        r#"
        SELECT id, token, uuid, group_id, plan_id, banned, u, d, transfer_enable, expired_at, commission_balance
        FROM v2_user
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_access_by_token(
    pool: &MySqlPool,
    token: &str,
) -> Result<Option<UserAccessRow>, sqlx::Error> {
    sqlx::query_as::<_, UserAccessRow>(
        r#"
        SELECT id, token, uuid, group_id, plan_id, banned, u, d, transfer_enable, expired_at, commission_balance
        FROM v2_user
        WHERE token = ?
        LIMIT 1
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn update_preferences(
    pool: &MySqlPool,
    id: i64,
    auto_renewal: Option<i8>,
    remind_expire: Option<i8>,
    remind_traffic: Option<i8>,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE v2_user
        SET
            auto_renewal = COALESCE(?, auto_renewal),
            remind_expire = COALESCE(?, remind_expire),
            remind_traffic = COALESCE(?, remind_traffic),
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(auto_renewal)
    .bind(remind_expire)
    .bind(remind_traffic)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_telegram_id(pool: &MySqlPool, id: i64, now: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE v2_user SET telegram_id = NULL, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_security(
    pool: &MySqlPool,
    id: i64,
    uuid: &str,
    token: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE v2_user SET uuid = ?, token = ?, updated_at = ? WHERE id = ?")
        .bind(uuid)
        .bind(token)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_password(
    pool: &MySqlPool,
    id: i64,
    password_hash: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE v2_user
        SET password = ?, password_algo = NULL, password_salt = NULL, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(password_hash)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn count_pending_orders(pool: &MySqlPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_order WHERE status = 0 AND user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

pub async fn count_pending_tickets(pool: &MySqlPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_ticket WHERE status = 0 AND user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

pub async fn count_invited_users(pool: &MySqlPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE invite_user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(count)
}
