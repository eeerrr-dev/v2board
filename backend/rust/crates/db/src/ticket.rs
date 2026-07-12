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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketCreateOutcome {
    Created,
    OpenTicketExists,
    PaidOrderRequired,
    UserNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserTicketReplyOutcome {
    Replied,
    NotFound,
    Closed,
    AwaitingOperator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorReplyTarget {
    pub id: i32,
    pub user_id: i64,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorReplyTargetOutcome {
    Locked(OperatorReplyTarget),
    NotFound,
    OtherOpenTicketExists,
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
    require_paid_order: bool,
) -> Result<TicketCreateOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    if !lock_user(&mut tx, user_id).await? {
        tx.rollback().await?;
        return Ok(TicketCreateOutcome::UserNotFound);
    }
    if open_ticket_exists(&mut tx, user_id, None).await? {
        tx.rollback().await?;
        return Ok(TicketCreateOutcome::OpenTicketExists);
    }
    if require_paid_order && count_paid_orders_in_tx(&mut tx, user_id).await? == 0 {
        tx.rollback().await?;
        return Ok(TicketCreateOutcome::PaidOrderRequired);
    }
    let ticket_id = match insert_ticket(&mut tx, user_id, subject, level, now).await {
        Ok(ticket_id) => ticket_id,
        Err(error) if is_unique_violation(&error) => {
            tx.rollback().await?;
            return Ok(TicketCreateOutcome::OpenTicketExists);
        }
        Err(error) => return Err(error),
    };
    insert_message(&mut tx, user_id, ticket_id, message, now).await?;
    tx.commit().await?;
    Ok(TicketCreateOutcome::Created)
}

pub async fn create_withdraw_ticket(
    pool: &MySqlPool,
    user_id: i64,
    withdraw_method: &str,
    withdraw_account: &str,
    now: i64,
) -> Result<TicketCreateOutcome, sqlx::Error> {
    let subject = "[Commission Withdrawal Request] This ticket is opened by the system";
    let message =
        format!("Withdrawal method：{withdraw_method}\r\nWithdrawal account：{withdraw_account}");
    create_ticket(pool, user_id, subject, 2, &message, now, false).await
}

async fn count_paid_orders_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM v2_order WHERE user_id = ? AND status IN (3, 4)")
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await
}

pub async fn reply_ticket(
    pool: &MySqlPool,
    ticket_id: i32,
    user_id: i64,
    message: &str,
    now: i64,
) -> Result<UserTicketReplyOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    if !lock_user(&mut tx, user_id).await? {
        tx.rollback().await?;
        return Ok(UserTicketReplyOutcome::NotFound);
    }
    let ticket = sqlx::query_as::<_, (i32, i8)>(
        "SELECT id, status FROM v2_ticket WHERE id = ? AND user_id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((_, status)) = ticket else {
        tx.rollback().await?;
        return Ok(UserTicketReplyOutcome::NotFound);
    };
    if status != 0 {
        tx.rollback().await?;
        return Ok(UserTicketReplyOutcome::Closed);
    }
    let last_user_id = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM v2_ticket_message WHERE ticket_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(ticket_id)
    .fetch_optional(&mut *tx)
    .await?;
    if last_user_id == Some(user_id) {
        tx.rollback().await?;
        return Ok(UserTicketReplyOutcome::AwaitingOperator);
    }
    insert_message(&mut tx, user_id, ticket_id, message, now).await?;
    let updated = sqlx::query(
        "UPDATE v2_ticket SET reply_status = 0, updated_at = ? WHERE id = ? AND user_id = ? AND status = 0",
    )
        .bind(now)
        .bind(ticket_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() != 1 {
        return Err(sqlx::Error::Protocol(
            "ticket state changed while applying a user reply".to_string(),
        ));
    }
    tx.commit().await?;
    Ok(UserTicketReplyOutcome::Replied)
}

pub async fn close_ticket(
    pool: &MySqlPool,
    user_id: i64,
    ticket_id: i32,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    if !lock_user(&mut tx, user_id).await? {
        tx.rollback().await?;
        return Ok(false);
    }
    let exists = sqlx::query_scalar::<_, i32>(
        "SELECT id FROM v2_ticket WHERE id = ? AND user_id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    if exists.is_none() {
        tx.rollback().await?;
        return Ok(false);
    }
    let result = sqlx::query(
        "UPDATE v2_ticket SET status = 1, updated_at = ? WHERE id = ? AND user_id = ? AND status = 0",
    )
    .bind(now)
    .bind(ticket_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

pub async fn lock_operator_reply_target(
    tx: &mut Transaction<'_, MySql>,
    ticket_id: i32,
) -> Result<OperatorReplyTargetOutcome, sqlx::Error> {
    let Some(user_id) =
        sqlx::query_scalar::<_, i64>("SELECT user_id FROM v2_ticket WHERE id = ? LIMIT 1")
            .bind(ticket_id)
            .fetch_optional(&mut **tx)
            .await?
    else {
        return Ok(OperatorReplyTargetOutcome::NotFound);
    };
    if !lock_user(tx, user_id).await? {
        return Ok(OperatorReplyTargetOutcome::NotFound);
    }
    let target = sqlx::query_as::<_, (i32, i64, String)>(
        "SELECT id, user_id, subject FROM v2_ticket WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(ticket_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((id, locked_user_id, subject)) = target else {
        return Ok(OperatorReplyTargetOutcome::NotFound);
    };
    if locked_user_id != user_id {
        return Err(sqlx::Error::Protocol(
            "ticket owner changed while locking an operator reply".to_string(),
        ));
    }
    if open_ticket_exists(tx, user_id, Some(ticket_id)).await? {
        return Ok(OperatorReplyTargetOutcome::OtherOpenTicketExists);
    }
    Ok(OperatorReplyTargetOutcome::Locked(OperatorReplyTarget {
        id,
        user_id,
        subject,
    }))
}

pub async fn apply_operator_reply(
    tx: &mut Transaction<'_, MySql>,
    target: &OperatorReplyTarget,
    operator_id: i64,
    message: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    insert_message(tx, operator_id, target.id, message, now).await?;
    let reply_status = i8::from(operator_id != target.user_id);
    let result = sqlx::query(
        "UPDATE v2_ticket SET status = 0, reply_status = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(reply_status)
    .bind(now)
    .bind(target.id)
    .bind(target.user_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(sqlx::Error::Protocol(
            "ticket state changed while applying an operator reply".to_string(),
        ));
    }
    Ok(())
}

pub async fn reply_ticket_as_operator(
    pool: &MySqlPool,
    ticket_id: i32,
    operator_id: i64,
    message: &str,
    now: i64,
) -> Result<OperatorReplyTargetOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let target = lock_operator_reply_target(&mut tx, ticket_id).await?;
    let OperatorReplyTargetOutcome::Locked(ref locked) = target else {
        tx.rollback().await?;
        return Ok(target);
    };
    apply_operator_reply(&mut tx, locked, operator_id, message, now).await?;
    tx.commit().await?;
    Ok(target)
}

pub async fn close_ticket_as_operator(
    pool: &MySqlPool,
    ticket_id: i32,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(user_id) =
        sqlx::query_scalar::<_, i64>("SELECT user_id FROM v2_ticket WHERE id = ? LIMIT 1")
            .bind(ticket_id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        tx.rollback().await?;
        return Ok(false);
    };
    if !lock_user(&mut tx, user_id).await? {
        tx.rollback().await?;
        return Ok(false);
    }
    let exists = sqlx::query_scalar::<_, i32>(
        "SELECT id FROM v2_ticket WHERE id = ? AND user_id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    if exists.is_none() {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "UPDATE v2_ticket SET status = 1, updated_at = ? WHERE id = ? AND user_id = ? AND status = 0",
    )
    .bind(now)
    .bind(ticket_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

async fn lock_user(tx: &mut Transaction<'_, MySql>, user_id: i64) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some(),
    )
}

async fn open_ticket_exists(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
    excluding_ticket_id: Option<i32>,
) -> Result<bool, sqlx::Error> {
    let id = match excluding_ticket_id {
        Some(ticket_id) => {
            sqlx::query_scalar::<_, i32>(
                "SELECT id FROM v2_ticket WHERE user_id = ? AND status = 0 AND id <> ? LIMIT 1",
            )
            .bind(user_id)
            .bind(ticket_id)
            .fetch_optional(&mut **tx)
            .await?
        }
        None => {
            sqlx::query_scalar::<_, i32>(
                "SELECT id FROM v2_ticket WHERE user_id = ? AND status = 0 LIMIT 1",
            )
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    Ok(id.is_some())
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
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

#[cfg(test)]
mod tests {
    #[test]
    fn ticket_state_machine_has_database_and_transaction_guards() {
        let source = include_str!("ticket.rs");
        assert!(source.contains("SELECT id FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE"));
        assert!(source.contains("WHERE id = ? AND user_id = ? LIMIT 1 FOR UPDATE"));
        assert!(source.contains("AND status = 0"));
        assert!(source.contains("rows_affected() != 1"));
        assert!(source.contains("OtherOpenTicketExists"));

        let preflight = include_str!("../../../migrations/0010_business_invariants.sql");
        let ticket_migration = include_str!("../../../migrations/0018_ticket_open_invariants.sql");
        let message_migration = include_str!("../../../migrations/0019_ticket_message_index.sql");
        assert!(preflight.contains("user has multiple open tickets"));
        assert!(ticket_migration.contains("GENERATED ALWAYS AS"));
        assert!(ticket_migration.contains("uniq_ticket_open_user"));
        assert!(message_migration.contains("idx_ticket_message_ticket_id_id"));
    }
}
