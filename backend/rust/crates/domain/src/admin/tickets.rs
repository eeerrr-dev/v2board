use super::*;
use v2board_compat::Pagination;

pub(super) fn validate_ticket_message_length(message: &str) -> Result<(), ApiError> {
    if message.len() > 65_535 {
        return Err(validation_error("message", "工单回复内容过长"));
    }
    Ok(())
}

/// The shared §6.5/§6.9 ticket row projection (list and detail): the legacy
/// key set with the computed `last_reply_user_id`. Timestamps convert to
/// RFC 3339 (§4.5) after fetch.
const ADMIN_TICKET_ROW_SELECT: &str = r#"
    SELECT jsonb_build_object(
        'id', id, 'user_id', user_id, 'subject', subject, 'level', level,
        'status', status, 'reply_status', reply_status,
        'last_reply_user_id', (
            SELECT user_id FROM ticket_message WHERE ticket_id = ticket.id ORDER BY id DESC LIMIT 1
        ),
        'created_at', created_at, 'updated_at', updated_at
    )
    FROM ticket
    WHERE 1 = 1
"#;

const TICKET_NOTIFICATION_GATE_TTL_SECONDS: u64 = 1800;
pub(super) const TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
end
return 0
"#;

struct TicketNotificationGate {
    key: String,
    token: String,
}

struct TicketReplyNotification {
    email: String,
    envelope: PreparedMailEnvelope,
    gate: TicketNotificationGate,
}

impl AdminService {
    /// GET `tickets` (docs/api-dialect.md §6.5, W14) plus the §6.9 staff
    /// mirror: §8 `{items,total}` pagination over the shared row projection
    /// with RFC 3339 timestamps (§4.5).
    ///
    /// The admin list honors the dedicated `status` / repeatable
    /// `reply_status` / `email` filters (never the §7 DSL) and orders by
    /// `updated_at`; the staff mirror only filters by `status` and orders by
    /// `created_at`. Email scoping keeps the legacy outcome: present + known
    /// user → scope to that user; present-but-unknown or absent → no scope
    /// (the Laravel `if ($user)` guard).
    pub async fn tickets_list(
        &self,
        pagination: Pagination,
        status: Option<i64>,
        reply_statuses: &[i64],
        email: Option<&str>,
        staff: bool,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        fn apply_filters(
            builder: &mut QueryBuilder<Postgres>,
            status: Option<i64>,
            reply_statuses: &[i64],
            user_id: Option<i64>,
        ) {
            if let Some(status) = status {
                builder.push(" AND status = ");
                builder.push_bind(status);
            }
            if !reply_statuses.is_empty() {
                builder.push(" AND reply_status IN (");
                let mut separated = builder.separated(", ");
                for value in reply_statuses {
                    separated.push_bind(*value);
                }
                builder.push(")");
            }
            if let Some(user_id) = user_id {
                builder.push(" AND user_id = ");
                builder.push_bind(user_id);
            }
        }

        // Staff has no reply_status / email filters.
        let reply_statuses = if staff { &[][..] } else { reply_statuses };
        let user_id = match email.filter(|_| !staff) {
            Some(email) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
                )
                .bind(email)
                .fetch_optional(&self.db)
                .await?
            }
            None => None,
        };

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ticket WHERE 1 = 1");
        apply_filters(&mut count_builder, status, reply_statuses, user_id);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(ADMIN_TICKET_ROW_SELECT);
        apply_filters(&mut builder, status, reply_statuses, user_id);
        let order_column = if staff { "created_at" } else { "updated_at" };
        builder.push(format!(" ORDER BY {order_column} DESC LIMIT "));
        builder.push_bind(pagination.limit());
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset());
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let items = rows
            .into_iter()
            .map(|row| statistics::epoch_fields_to_rfc3339(row.0, &["created_at", "updated_at"]))
            .collect();
        Ok((items, total))
    }

    /// GET `tickets/{id}` (§6.5, W14) plus the §6.9 staff mirror: the bare
    /// row with the ordered `message[]` thread. `is_me` semantics are
    /// unchanged — true marks messages whose author is NOT the ticket owner,
    /// i.e. an admin/staff reply (TicketController::fetch :22-30).
    pub async fn ticket_detail(&self, id: i64) -> Result<Value, ApiError> {
        let ticket = fetch_json_one(
            &self.db,
            &format!("{ADMIN_TICKET_ROW_SELECT} AND id = $1 LIMIT 1"),
            id,
        )
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::TicketNotFound)))?;
        let messages = fetch_json_list_bind(
            &self.db,
            r#"
            SELECT jsonb_build_object(
                'id', id, 'user_id', user_id, 'ticket_id', ticket_id, 'message', message,
                'is_me', user_id <> (
                    SELECT user_id FROM ticket WHERE id = ticket_message.ticket_id
                ),
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM ticket_message
            WHERE ticket_id = $1
            ORDER BY id ASC
            "#,
            id,
        )
        .await?;
        let messages: Vec<Value> = messages
            .into_iter()
            .map(|row| statistics::epoch_fields_to_rfc3339(row, &["created_at", "updated_at"]))
            .collect();
        let mut ticket = statistics::epoch_fields_to_rfc3339(ticket, &["created_at", "updated_at"])
            .as_object()
            .cloned()
            .unwrap_or_default();
        ticket.insert("message".to_string(), json!(messages));
        Ok(Value::Object(ticket))
    }

    /// POST `tickets/{id}/replies` (§6.5, W14) plus the §6.9 staff mirror:
    /// empty on success; `ticket_not_found` (404) and
    /// `unresolved_ticket_exists` (400) replace the legacy business errors.
    pub async fn ticket_reply(
        &self,
        ticket_id: i64,
        message: &str,
        operator_email: &str,
    ) -> Result<(), ApiError> {
        // Ports TicketService::replyByAdmin (:34-61): records the reply under the
        // acting admin, reopens the ticket (status = 0), sets reply_status based
        // on authorship, and notifies the owner by email (deduped 30 min).
        let id = ticket_id;
        validate_ticket_message_length(message)?;
        let admin_id = self.current_admin_id(operator_email).await?;
        let (ticket_user_id, subject): (i64, String) =
            sqlx::query_as("SELECT user_id, subject FROM ticket WHERE id = $1 LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::from(Problem::new(Code::TicketNotFound)))?;
        let prepared_notification = self
            .prepare_ticket_reply_notification(ticket_user_id, &subject, message)
            .await;
        let notification = if let Some((email, envelope)) = prepared_notification {
            self.reserve_ticket_notification_gate(ticket_user_id)
                .await
                .map(|gate| TicketReplyNotification {
                    email,
                    envelope,
                    gate,
                })
        } else {
            None
        };
        let now = Utc::now().timestamp();
        let transaction_result: Result<(), ApiError> = async {
            let mut tx = self.db.begin().await?;
            let target =
                match v2board_db::ticket::lock_operator_reply_target(&mut tx, ticket_id).await? {
                    v2board_db::ticket::OperatorReplyTargetOutcome::Locked(target) => target,
                    v2board_db::ticket::OperatorReplyTargetOutcome::NotFound => {
                        return Err(Problem::new(Code::TicketNotFound).into());
                    }
                    v2board_db::ticket::OperatorReplyTargetOutcome::OtherOpenTicketExists => {
                        // Default detail relocalizes per §4.3 (the W8 user
                        // path already uses the registry default).
                        return Err(Problem::new(Code::UnresolvedTicketExists).into());
                    }
                };
            if target.user_id != ticket_user_id {
                return Err(ApiError::internal(
                    "ticket owner changed while preparing an admin reply",
                ));
            }
            v2board_db::ticket::apply_operator_reply(&mut tx, &target, admin_id, message, now)
                .await?;
            if let Some(notification) = notification.as_ref() {
                let recipients = vec![notification.email.clone()];
                let actor = format!("ticket:{ticket_user_id}");
                let batch_key = mail_batch_key(&actor, &Uuid::new_v4().to_string());
                let payload_hash = prepared_mail_payload_hash(&notification.envelope, &recipients);
                if reserve_mail_outbox_batch(&mut tx, &batch_key, &payload_hash, &actor, now)
                    .await
                    .map_err(mail_outbox_api_error)?
                {
                    enqueue_prepared_mail(
                        &mut tx,
                        &batch_key,
                        &notification.envelope,
                        &recipients,
                        now,
                    )
                    .await
                    .map_err(mail_outbox_api_error)?;
                }
            }
            tx.commit().await?;
            Ok(())
        }
        .await;
        if let Err(error) = transaction_result {
            if let Some(notification) = notification.as_ref() {
                self.release_ticket_notification_gate(&notification.gate)
                    .await;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Prepares the recipient and envelope before reserving the Redis admission
    /// gate. Recipient and mail-configuration failures remain best-effort so a
    /// ticket reply succeeds without suppressing a later valid notification.
    async fn prepare_ticket_reply_notification(
        &self,
        user_id: i64,
        subject: &str,
        message: &str,
    ) -> Option<(String, PreparedMailEnvelope)> {
        let email: Option<String> =
            match sqlx::query_scalar("SELECT email FROM users WHERE id = $1 LIMIT 1")
                .bind(user_id)
                .fetch_optional(&self.db)
                .await
            {
                Ok(email) => email,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        user_id,
                        "ticket reply notification user lookup failed"
                    );
                    return None;
                }
            };
        let email = email?;
        if let Err(error) = validate_mail_recipient(&email) {
            tracing::warn!(
                ?error,
                user_id,
                "ticket reply notification recipient invalid"
            );
            return None;
        }
        let subject_line = format!("您在{}的工单得到了回复", self.config.app_name);
        let content = format!("主题：{subject}\r\n回复内容：{message}");
        match self.prepare_notify_mail(&subject_line, &content) {
            Ok(envelope) => Some((email, envelope)),
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id,
                    "ticket reply notification envelope invalid"
                );
                None
            }
        }
    }

    async fn reserve_ticket_notification_gate(
        &self,
        user_id: i64,
    ) -> Option<TicketNotificationGate> {
        let key = self.redis_key(&format!("ticket_sendEmailNotify_{user_id}"));
        let token = Uuid::new_v4().to_string();
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id,
                    "ticket reply notification Redis unavailable"
                );
                return None;
            }
        };
        let acquired: Result<Option<String>, redis::RedisError> = redis::cmd("SET")
            .arg(&key)
            .arg(&token)
            .arg("NX")
            .arg("EX")
            .arg(TICKET_NOTIFICATION_GATE_TTL_SECONDS)
            .query_async(&mut conn)
            .await;
        match acquired {
            Ok(Some(_)) => Some(TicketNotificationGate { key, token }),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id,
                    "ticket reply notification reservation failed"
                );
                None
            }
        }
    }

    async fn release_ticket_notification_gate(&self, gate: &TicketNotificationGate) {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    key = %gate.key,
                    "ticket reply notification reservation release failed"
                );
                return;
            }
        };
        let released: Result<i64, redis::RedisError> =
            redis::Script::new(TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT)
                .key(&gate.key)
                .arg(&gate.token)
                .invoke_async(&mut conn)
                .await;
        match released {
            Ok(1) => {}
            Ok(_) => tracing::warn!(
                key = %gate.key,
                "ticket reply notification reservation ownership changed before release"
            ),
            Err(error) => tracing::warn!(
                ?error,
                key = %gate.key,
                "ticket reply notification reservation release failed"
            ),
        }
    }

    /// POST `tickets/{id}/close` (§6.5, W14) plus the §6.9 staff mirror:
    /// empty on success.
    pub async fn ticket_close(&self, id: i64) -> Result<(), ApiError> {
        v2board_db::ticket::close_ticket_as_operator(&self.db, id, Utc::now().timestamp()).await?;
        Ok(())
    }
}
