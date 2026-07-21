//! PostgreSQL adapter for the ticket application port.

use sqlx::{AssertSqlSafe, FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use v2board_application::{
    RepositoryError,
    ticket::{
        DurableMailDelivery, NewTicket, OperatorReplyTarget, OperatorTicketListQuery,
        OperatorTicketOrder, OperatorTicketReply, OperatorTicketReplyOutcome,
        Ticket as ApplicationTicket, TicketCreateOutcome, TicketDetail, TicketMessage, TicketPage,
        TicketRepository, UserTicketReply, UserTicketReplyOutcome,
    },
};
use v2board_domain_model::{TicketLevel, TicketReplyStatus, TicketStatus};

#[derive(Debug, FromRow)]
struct TicketRecord {
    id: i64,
    user_id: i64,
    subject: String,
    level: i16,
    status: i16,
    reply_status: i16,
    last_reply_user_id: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, FromRow)]
struct TicketMessageRecord {
    id: i64,
    user_id: i64,
    ticket_id: i64,
    message: String,
    is_me: bool,
    created_at: i64,
    updated_at: i64,
}

#[derive(Clone, Debug)]
pub struct PostgresTicketRepository {
    pool: PgPool,
}

impl PostgresTicketRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const TICKET_PROJECTION: &str = r#"
    SELECT
        t.id, t.user_id, t.subject, t.level, t.status, t.reply_status,
        (
            SELECT tm.user_id
            FROM ticket_message tm
            WHERE tm.ticket_id = t.id
            ORDER BY tm.id DESC
            LIMIT 1
        ) AS last_reply_user_id,
        t.created_at, t.updated_at
    FROM ticket t
"#;

const AUTO_CLOSE_TICKETS_SQL: &str = r#"
    WITH candidates AS (
        SELECT t.id
        FROM ticket AS t
        WHERE t.status = 0
          AND t.updated_at <= $2
          AND t.reply_status = 1
          AND COALESCE((
              SELECT tm.user_id
              FROM ticket_message tm
              WHERE tm.ticket_id = t.id
              ORDER BY tm.id DESC
              LIMIT 1
          ), 0) <> t.user_id
        ORDER BY t.updated_at, t.id
        LIMIT $3
        FOR UPDATE SKIP LOCKED
    )
    UPDATE ticket AS t
    SET status = 1, updated_at = $1
    FROM candidates
    WHERE t.id = candidates.id AND t.status = 0
"#;

#[allow(async_fn_in_trait)]
impl TicketRepository for PostgresTicketRepository {
    async fn list_for_user(&self, user_id: i64) -> Result<Vec<ApplicationTicket>, RepositoryError> {
        let rows = sqlx::query_as::<_, TicketRecord>(AssertSqlSafe(format!(
            "{TICKET_PROJECTION} WHERE t.user_id = $1 ORDER BY t.created_at DESC"
        )))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.list_for_user", error))?;
        rows.into_iter().map(decode_ticket).collect()
    }

    async fn find_for_user(
        &self,
        user_id: i64,
        ticket_id: i64,
    ) -> Result<Option<TicketDetail>, RepositoryError> {
        let row = sqlx::query_as::<_, TicketRecord>(AssertSqlSafe(format!(
            "{TICKET_PROJECTION} WHERE t.id = $1 AND t.user_id = $2 LIMIT 1"
        )))
        .bind(ticket_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.find_for_user", error))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let messages = sqlx::query_as::<_, TicketMessageRecord>(
            r#"
            SELECT id, user_id, ticket_id, message, user_id = $1 AS is_me, created_at, updated_at
            FROM ticket_message
            WHERE ticket_id = $2
            ORDER BY id ASC
            "#,
        )
        .bind(user_id)
        .bind(ticket_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.find_for_user.messages", error))?;
        Ok(Some(TicketDetail {
            ticket: decode_ticket(row)?,
            messages: messages.into_iter().map(decode_message).collect(),
        }))
    }

    async fn create(&self, ticket: NewTicket) -> Result<TicketCreateOutcome, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("ticket.create.begin", error))?;
        if !lock_user(&mut tx, ticket.user_id)
            .await
            .map_err(|error| repository_error("ticket.create.lock_user", error))?
        {
            return Ok(TicketCreateOutcome::UserNotFound);
        }
        if open_ticket_exists(&mut tx, ticket.user_id, None)
            .await
            .map_err(|error| repository_error("ticket.create.open_guard", error))?
        {
            return Ok(TicketCreateOutcome::OpenTicketExists);
        }
        if ticket.require_paid_order
            && count_paid_orders(&mut tx, ticket.user_id)
                .await
                .map_err(|error| repository_error("ticket.create.paid_order", error))?
                == 0
        {
            return Ok(TicketCreateOutcome::PaidOrderRequired);
        }
        let ticket_id = match insert_ticket(&mut tx, &ticket).await {
            Ok(ticket_id) => ticket_id,
            Err(error) if is_unique_violation(&error) => {
                return Ok(TicketCreateOutcome::OpenTicketExists);
            }
            Err(error) => return Err(repository_error("ticket.create.insert", error)),
        };
        insert_message(
            &mut tx,
            ticket.user_id,
            ticket_id,
            &ticket.message,
            ticket.created_at,
        )
        .await
        .map_err(|error| repository_error("ticket.create.message", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("ticket.create.commit", error))?;
        Ok(TicketCreateOutcome::Created(ticket_id))
    }

    async fn reply_as_user(
        &self,
        reply: UserTicketReply,
    ) -> Result<UserTicketReplyOutcome, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("ticket.user_reply.begin", error))?;
        if !lock_user(&mut tx, reply.user_id)
            .await
            .map_err(|error| repository_error("ticket.user_reply.lock_user", error))?
        {
            return Ok(UserTicketReplyOutcome::NotFound);
        }
        let ticket = sqlx::query_as::<_, (i64, i16)>(
            "SELECT id, status FROM ticket WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE",
        )
        .bind(reply.ticket_id)
        .bind(reply.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.user_reply.lock_ticket", error))?;
        let Some((_, status)) = ticket else {
            return Ok(UserTicketReplyOutcome::NotFound);
        };
        if status != TicketStatus::Open.code() {
            return Ok(UserTicketReplyOutcome::Closed);
        }
        let last_user_id = sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM ticket_message WHERE ticket_id = $1 ORDER BY id DESC LIMIT 1",
        )
        .bind(reply.ticket_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.user_reply.last_author", error))?;
        if last_user_id == Some(reply.user_id) {
            return Ok(UserTicketReplyOutcome::AwaitingOperator);
        }
        insert_message(
            &mut tx,
            reply.user_id,
            reply.ticket_id,
            &reply.message,
            reply.replied_at,
        )
        .await
        .map_err(|error| repository_error("ticket.user_reply.message", error))?;
        let updated = sqlx::query(
            "UPDATE ticket SET reply_status = 0, updated_at = $1 WHERE id = $2 AND user_id = $3 AND status = 0",
        )
        .bind(reply.replied_at)
        .bind(reply.ticket_id)
        .bind(reply.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.user_reply.update", error))?;
        if updated.rows_affected() != 1 {
            return Err(repository_error(
                "ticket.user_reply.update",
                "ticket state changed while applying a user reply",
            ));
        }
        tx.commit()
            .await
            .map_err(|error| repository_error("ticket.user_reply.commit", error))?;
        Ok(UserTicketReplyOutcome::Replied)
    }

    async fn close_as_user(
        &self,
        user_id: i64,
        ticket_id: i64,
        closed_at: i64,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("ticket.user_close.begin", error))?;
        if !lock_user(&mut tx, user_id)
            .await
            .map_err(|error| repository_error("ticket.user_close.lock_user", error))?
        {
            return Ok(false);
        }
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM ticket WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE",
        )
        .bind(ticket_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.user_close.lock_ticket", error))?;
        if exists.is_none() {
            return Ok(false);
        }
        let result = sqlx::query(
            "UPDATE ticket SET status = 1, updated_at = $1 WHERE id = $2 AND user_id = $3 AND status = 0",
        )
        .bind(closed_at)
        .bind(ticket_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.user_close.update", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("ticket.user_close.commit", error))?;
        Ok(result.rows_affected() == 1)
    }

    async fn commission_balance(&self, user_id: i64) -> Result<Option<i64>, RepositoryError> {
        let balance = sqlx::query_scalar::<_, i32>(
            "SELECT commission_balance FROM users WHERE id = $1 LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.commission_balance", error))?;
        Ok(balance.map(i64::from))
    }

    async fn list_for_operator(
        &self,
        query: OperatorTicketListQuery,
    ) -> Result<TicketPage, RepositoryError> {
        let user_id = match query.email.as_deref() {
            Some(email) => sqlx::query_scalar::<_, i64>(
                "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
            )
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| repository_error("ticket.operator_list.email", error))?,
            None => None,
        };
        let apply_filters = |builder: &mut QueryBuilder<Postgres>| {
            if let Some(status) = query.status {
                builder.push(" AND t.status = ").push_bind(status);
            }
            if !query.reply_statuses.is_empty() {
                builder.push(" AND t.reply_status IN (");
                let mut values = builder.separated(", ");
                for value in &query.reply_statuses {
                    values.push_bind(*value);
                }
                values.push_unseparated(")");
            }
            if let Some(user_id) = user_id {
                builder.push(" AND t.user_id = ").push_bind(user_id);
            }
        };

        let mut count = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ticket t WHERE 1 = 1");
        apply_filters(&mut count);
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("ticket.operator_list.count", error))?;

        let mut rows = QueryBuilder::<Postgres>::new(TICKET_PROJECTION);
        rows.push(" WHERE 1 = 1");
        apply_filters(&mut rows);
        match query.order {
            OperatorTicketOrder::UpdatedAt => rows.push(" ORDER BY t.updated_at DESC"),
            OperatorTicketOrder::CreatedAt => rows.push(" ORDER BY t.created_at DESC"),
        };
        rows.push(" LIMIT ")
            .push_bind(query.limit)
            .push(" OFFSET ")
            .push_bind(query.offset);
        let items = rows
            .build_query_as::<TicketRecord>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("ticket.operator_list.rows", error))?
            .into_iter()
            .map(decode_ticket)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TicketPage { items, total })
    }

    async fn find_for_operator(
        &self,
        ticket_id: i64,
    ) -> Result<Option<TicketDetail>, RepositoryError> {
        let row = sqlx::query_as::<_, TicketRecord>(AssertSqlSafe(format!(
            "{TICKET_PROJECTION} WHERE t.id = $1 LIMIT 1"
        )))
        .bind(ticket_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.operator_detail", error))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let messages = sqlx::query_as::<_, TicketMessageRecord>(
            r#"
            SELECT tm.id, tm.user_id, tm.ticket_id, tm.message,
                   tm.user_id <> t.user_id AS is_me,
                   tm.created_at, tm.updated_at
            FROM ticket_message tm
            JOIN ticket t ON t.id = tm.ticket_id
            WHERE tm.ticket_id = $1
            ORDER BY tm.id ASC
            "#,
        )
        .bind(ticket_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.operator_detail.messages", error))?;
        Ok(Some(TicketDetail {
            ticket: decode_ticket(row)?,
            messages: messages.into_iter().map(decode_message).collect(),
        }))
    }

    async fn operator_id_by_email(&self, email: &str) -> Result<Option<i64>, RepositoryError> {
        sqlx::query_scalar(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.operator_id", error))
    }

    async fn operator_reply_target(
        &self,
        ticket_id: i64,
    ) -> Result<Option<OperatorReplyTarget>, RepositoryError> {
        let row = sqlx::query_as::<_, (i64, i64, String, String)>(
            r#"
            SELECT t.id, t.user_id, t.subject, u.email
            FROM ticket t
            JOIN users u ON u.id = t.user_id
            WHERE t.id = $1
            LIMIT 1
            "#,
        )
        .bind(ticket_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("ticket.operator_reply.target", error))?;
        Ok(row.map(
            |(ticket_id, user_id, subject, recipient_email)| OperatorReplyTarget {
                ticket_id,
                user_id,
                subject,
                recipient_email,
            },
        ))
    }

    async fn reply_as_operator(
        &self,
        reply: OperatorTicketReply,
        notification: Option<&DurableMailDelivery>,
    ) -> Result<OperatorTicketReplyOutcome, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("ticket.operator_reply.begin", error))?;
        let Some(user_id) =
            sqlx::query_scalar::<_, i64>("SELECT user_id FROM ticket WHERE id = $1 LIMIT 1")
                .bind(reply.ticket_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| repository_error("ticket.operator_reply.owner", error))?
        else {
            return Ok(OperatorTicketReplyOutcome::NotFound);
        };
        if user_id != reply.expected_user_id {
            return Err(repository_error(
                "ticket.operator_reply.owner",
                "ticket owner changed while preparing an operator reply",
            ));
        }
        if !lock_user(&mut tx, user_id)
            .await
            .map_err(|error| repository_error("ticket.operator_reply.lock_user", error))?
        {
            return Ok(OperatorTicketReplyOutcome::NotFound);
        }
        let target = sqlx::query_as::<_, (i64, i64)>(
            "SELECT id, user_id FROM ticket WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(reply.ticket_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.operator_reply.lock_ticket", error))?;
        let Some((_, locked_user_id)) = target else {
            return Ok(OperatorTicketReplyOutcome::NotFound);
        };
        if locked_user_id != user_id {
            return Err(repository_error(
                "ticket.operator_reply.lock_ticket",
                "ticket owner changed while locking an operator reply",
            ));
        }
        if open_ticket_exists(&mut tx, user_id, Some(reply.ticket_id))
            .await
            .map_err(|error| repository_error("ticket.operator_reply.open_guard", error))?
        {
            return Ok(OperatorTicketReplyOutcome::OtherOpenTicketExists);
        }
        insert_message(
            &mut tx,
            reply.operator_id,
            reply.ticket_id,
            &reply.message,
            reply.replied_at,
        )
        .await
        .map_err(|error| repository_error("ticket.operator_reply.message", error))?;
        let reply_status = i16::from(reply.operator_id != user_id);
        let updated = sqlx::query(
            "UPDATE ticket SET status = 0, reply_status = $1, updated_at = $2 WHERE id = $3 AND user_id = $4",
        )
        .bind(reply_status)
        .bind(reply.replied_at)
        .bind(reply.ticket_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.operator_reply.update", error))?;
        if updated.rows_affected() != 1 {
            return Err(repository_error(
                "ticket.operator_reply.update",
                "ticket state changed while applying an operator reply",
            ));
        }
        if let Some(delivery) = notification {
            enqueue_mail(&mut tx, delivery, reply.replied_at).await?;
        }
        tx.commit()
            .await
            .map_err(|error| repository_error("ticket.operator_reply.commit", error))?;
        Ok(OperatorTicketReplyOutcome::Replied)
    }

    async fn close_as_operator(
        &self,
        ticket_id: i64,
        closed_at: i64,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("ticket.operator_close.begin", error))?;
        let Some(user_id) =
            sqlx::query_scalar::<_, i64>("SELECT user_id FROM ticket WHERE id = $1 LIMIT 1")
                .bind(ticket_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| repository_error("ticket.operator_close.owner", error))?
        else {
            return Ok(false);
        };
        if !lock_user(&mut tx, user_id)
            .await
            .map_err(|error| repository_error("ticket.operator_close.lock_user", error))?
        {
            return Ok(false);
        }
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM ticket WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE",
        )
        .bind(ticket_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.operator_close.lock_ticket", error))?;
        if exists.is_none() {
            return Ok(false);
        }
        sqlx::query(
            "UPDATE ticket SET status = 1, updated_at = $1 WHERE id = $2 AND user_id = $3 AND status = 0",
        )
        .bind(closed_at)
        .bind(ticket_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| repository_error("ticket.operator_close.update", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("ticket.operator_close.commit", error))?;
        Ok(true)
    }

    async fn auto_close_batch(
        &self,
        now: i64,
        cutoff: i64,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        Ok(sqlx::query(AUTO_CLOSE_TICKETS_SQL)
            .bind(now)
            .bind(cutoff)
            .bind(limit)
            .execute(&self.pool)
            .await
            .map_err(|error| repository_error("ticket.auto_close", error))?
            .rows_affected())
    }
}

fn decode_ticket(row: TicketRecord) -> Result<ApplicationTicket, RepositoryError> {
    Ok(ApplicationTicket {
        id: row.id,
        user_id: row.user_id,
        subject: row.subject,
        level: TicketLevel::try_from(row.level)
            .map_err(|_| repository_error("ticket.decode", "invalid ticket level"))?,
        status: TicketStatus::try_from(row.status)
            .map_err(|_| repository_error("ticket.decode", "invalid ticket status"))?,
        reply_status: TicketReplyStatus::try_from(row.reply_status)
            .map_err(|_| repository_error("ticket.decode", "invalid ticket reply status"))?,
        last_reply_user_id: row.last_reply_user_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn decode_message(row: TicketMessageRecord) -> TicketMessage {
    TicketMessage {
        id: row.id,
        user_id: row.user_id,
        ticket_id: row.ticket_id,
        message: row.message,
        is_me: row.is_me,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

async fn lock_user(tx: &mut Transaction<'_, Postgres>, user_id: i64) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some(),
    )
}

async fn count_paid_orders(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE user_id = $1 AND status IN (3, 4)")
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await
}

async fn open_ticket_exists(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    excluding_ticket_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let id = match excluding_ticket_id {
        Some(ticket_id) => {
            sqlx::query_scalar::<_, i64>(
                "SELECT id FROM ticket WHERE user_id = $1 AND status = 0 AND id <> $2 LIMIT 1",
            )
            .bind(user_id)
            .bind(ticket_id)
            .fetch_optional(&mut **tx)
            .await?
        }
        None => {
            sqlx::query_scalar::<_, i64>(
                "SELECT id FROM ticket WHERE user_id = $1 AND status = 0 LIMIT 1",
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
    tx: &mut Transaction<'_, Postgres>,
    ticket: &NewTicket,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO ticket (user_id, subject, level, status, reply_status, created_at, updated_at)
        VALUES ($1, $2, $3, 0, 0, $4, $5)
        RETURNING id
        "#,
    )
    .bind(ticket.user_id)
    .bind(&ticket.subject)
    .bind(ticket.level.code())
    .bind(ticket.created_at)
    .bind(ticket.created_at)
    .fetch_one(&mut **tx)
    .await
}

async fn insert_message(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    ticket_id: i64,
    message: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO ticket_message (user_id, ticket_id, message, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
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

async fn enqueue_mail(
    tx: &mut Transaction<'_, Postgres>,
    delivery: &DurableMailDelivery,
    now: i64,
) -> Result<(), RepositoryError> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO mail_outbox_batch
            (batch_key, payload_hash, actor, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (batch_key) DO NOTHING
        "#,
    )
    .bind(&delivery.batch_key)
    .bind(&delivery.payload_hash)
    .bind(&delivery.actor)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| repository_error("ticket.notification.reserve_batch", error))?
    .rows_affected()
        == 1;
    if !inserted {
        let existing_hash: String = sqlx::query_scalar(
            "SELECT payload_hash FROM mail_outbox_batch WHERE batch_key = $1 FOR UPDATE",
        )
        .bind(&delivery.batch_key)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| repository_error("ticket.notification.batch_hash", error))?;
        if existing_hash != delivery.payload_hash {
            return Err(repository_error(
                "ticket.notification.batch_hash",
                "mail idempotency key was reused with a different payload",
            ));
        }
        return Ok(());
    }
    let updated = sqlx::query(
        r#"
        UPDATE mail_outbox_batch
        SET sender = $1, template_name = $2, subject = $3, body = $4, updated_at = $5
        WHERE batch_key = $6
        "#,
    )
    .bind(&delivery.sender)
    .bind(&delivery.template_name)
    .bind(&delivery.subject)
    .bind(&delivery.body)
    .bind(now)
    .bind(&delivery.batch_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| repository_error("ticket.notification.envelope", error))?;
    if updated.rows_affected() != 1 {
        return Err(repository_error(
            "ticket.notification.envelope",
            "mail outbox batch envelope was lost",
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO mail_outbox
            (batch_key, recipient, message_id, attempt_count, available_at, created_at, updated_at)
        VALUES ($1, $2, $3, 0, $4, $5, $6)
        "#,
    )
    .bind(&delivery.batch_key)
    .bind(&delivery.recipient)
    .bind(&delivery.message_id)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| repository_error("ticket.notification.item", error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AUTO_CLOSE_TICKETS_SQL;

    #[test]
    fn auto_close_rechecks_complete_state_under_the_update_lock() {
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("WITH candidates AS"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.status = 0"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.updated_at <= $2"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("t.reply_status = 1"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("tm.ticket_id = t.id"));
        assert!(AUTO_CLOSE_TICKETS_SQL.contains("FOR UPDATE SKIP LOCKED"));
    }
}
