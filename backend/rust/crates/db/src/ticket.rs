use serde::Serialize;
use sqlx::{FromRow, MySql, MySqlPool, Transaction};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TicketRow {
    pub id: i32,
    pub user_id: i64,
    pub subject: String,
    pub level: i8,
    pub status: i8,
    pub reply_status: i8,
    pub last_reply_user_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TicketMessageRow {
    pub id: i32,
    pub user_id: i64,
    pub ticket_id: i32,
    pub message: String,
    pub is_me: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketDetailRow {
    pub id: i32,
    pub user_id: i64,
    pub subject: String,
    pub level: i8,
    pub status: i8,
    pub reply_status: i8,
    pub last_reply_user_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub message: Vec<TicketMessageRow>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TicketStatusRow {
    pub id: i32,
    pub user_id: i64,
    pub subject: String,
    pub status: i8,
}

#[derive(Debug, Clone, FromRow)]
pub struct LastTicketMessageRow {
    pub user_id: i64,
}

pub async fn fetch_tickets(pool: &MySqlPool, user_id: i64) -> Result<Vec<TicketRow>, sqlx::Error> {
    sqlx::query_as::<_, TicketRow>(
        r#"
        SELECT
            t.id,
            t.user_id,
            t.subject,
            t.level,
            t.status,
            t.reply_status,
            (
                SELECT tm.user_id
                FROM v2_ticket_message tm
                WHERE tm.ticket_id = t.id
                ORDER BY tm.id DESC
                LIMIT 1
            ) AS last_reply_user_id,
            t.created_at,
            t.updated_at
        FROM v2_ticket t
        WHERE t.user_id = ?
        ORDER BY t.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn fetch_ticket_detail(
    pool: &MySqlPool,
    user_id: i64,
    ticket_id: i32,
) -> Result<Option<TicketDetailRow>, sqlx::Error> {
    let Some(ticket) = sqlx::query_as::<_, TicketRow>(
        r#"
        SELECT
            t.id,
            t.user_id,
            t.subject,
            t.level,
            t.status,
            t.reply_status,
            (
                SELECT tm.user_id
                FROM v2_ticket_message tm
                WHERE tm.ticket_id = t.id
                ORDER BY tm.id DESC
                LIMIT 1
            ) AS last_reply_user_id,
            t.created_at,
            t.updated_at
        FROM v2_ticket t
        WHERE t.id = ? AND t.user_id = ?
        LIMIT 1
        "#,
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let messages = sqlx::query_as::<_, TicketMessageRow>(
        r#"
        SELECT
            id,
            user_id,
            ticket_id,
            message,
            user_id = ? AS is_me,
            created_at,
            updated_at
        FROM v2_ticket_message
        WHERE ticket_id = ?
        ORDER BY id ASC
        "#,
    )
    .bind(user_id)
    .bind(ticket_id)
    .fetch_all(pool)
    .await?;
    Ok(Some(TicketDetailRow {
        id: ticket.id,
        user_id: ticket.user_id,
        subject: ticket.subject,
        level: ticket.level,
        status: ticket.status,
        reply_status: ticket.reply_status,
        last_reply_user_id: ticket.last_reply_user_id,
        created_at: ticket.created_at,
        updated_at: ticket.updated_at,
        message: messages,
    }))
}

pub async fn create_ticket(
    pool: &MySqlPool,
    user_id: i64,
    subject: &str,
    level: i8,
    message: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let ticket_id = insert_ticket(&mut tx, user_id, subject, level, now).await?;
    insert_message(&mut tx, user_id, ticket_id, message, now).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_withdraw_ticket(
    pool: &MySqlPool,
    user_id: i64,
    withdraw_method: &str,
    withdraw_account: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let subject = "[Commission Withdrawal Request] This ticket is opened by the system";
    let message =
        format!("Withdrawal method：{withdraw_method}\r\nWithdrawal account：{withdraw_account}");
    create_ticket(pool, user_id, subject, 2, &message, now).await
}

pub async fn count_open_tickets(pool: &MySqlPool, user_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM v2_ticket WHERE status = 0 AND user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

pub async fn count_paid_orders(pool: &MySqlPool, user_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM v2_order WHERE user_id = ? AND status IN (3, 4)")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

pub async fn find_ticket_for_reply(
    pool: &MySqlPool,
    user_id: i64,
    ticket_id: i32,
) -> Result<Option<TicketStatusRow>, sqlx::Error> {
    sqlx::query_as::<_, TicketStatusRow>(
        "SELECT id, user_id, subject, status FROM v2_ticket WHERE id = ? AND user_id = ? LIMIT 1",
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_last_message(
    pool: &MySqlPool,
    ticket_id: i32,
) -> Result<Option<LastTicketMessageRow>, sqlx::Error> {
    sqlx::query_as::<_, LastTicketMessageRow>(
        "SELECT user_id FROM v2_ticket_message WHERE ticket_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(ticket_id)
    .fetch_optional(pool)
    .await
}

pub async fn reply_ticket(
    pool: &MySqlPool,
    ticket_id: i32,
    user_id: i64,
    message: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    insert_message(&mut tx, user_id, ticket_id, message, now).await?;
    sqlx::query("UPDATE v2_ticket SET reply_status = 0, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn close_ticket(
    pool: &MySqlPool,
    user_id: i64,
    ticket_id: i32,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("UPDATE v2_ticket SET status = 1, updated_at = ? WHERE id = ? AND user_id = ?")
            .bind(now)
            .bind(ticket_id)
            .bind(user_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

async fn insert_ticket(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
    subject: &str,
    level: i8,
    now: i64,
) -> Result<i32, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO v2_ticket (user_id, subject, level, status, reply_status, created_at, updated_at)
        VALUES (?, ?, ?, 0, 0, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(subject)
    .bind(level)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id() as i32)
}

async fn insert_message(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
    ticket_id: i32,
    message: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO v2_ticket_message (user_id, ticket_id, message, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(ticket_id)
    .bind(message)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
