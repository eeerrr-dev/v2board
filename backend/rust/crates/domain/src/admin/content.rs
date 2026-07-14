use super::*;

pub(super) fn validate_ticket_message_length(message: &str) -> Result<(), ApiError> {
    if message.len() > 65_535 {
        return Err(validation_error("message", "工单回复内容过长"));
    }
    Ok(())
}

const GENERATED_CODE_MAX_ROWS: usize = 1_000;

#[derive(Clone, Copy)]
enum GeneratedCodeTable {
    Coupon,
    Giftcard,
}

fn unique_random_codes(count: usize, length: usize) -> Vec<String> {
    let mut codes = HashSet::with_capacity(count);
    while codes.len() < count {
        codes.insert(random_char(length));
    }
    codes.into_iter().collect()
}

async fn insert_generated_codes(
    tx: &mut DbTransaction<'_>,
    table: GeneratedCodeTable,
    field_values: &[(&'static str, AdminSqlValue)],
    codes: &[String],
    now: i64,
) -> Result<(), ApiError> {
    if codes.is_empty() {
        return Ok(());
    }
    let mut builder = match table {
        GeneratedCodeTable::Coupon => QueryBuilder::<Postgres>::new("INSERT INTO coupon ("),
        GeneratedCodeTable::Giftcard => QueryBuilder::<Postgres>::new("INSERT INTO gift_card ("),
    };
    let mut columns = builder.separated(", ");
    for (column, _) in field_values {
        columns.push(format!("\"{column}\""));
    }
    if matches!(table, GeneratedCodeTable::Coupon) {
        columns.push("\"show\"");
    }
    columns.push("\"code\"");
    columns.push("\"created_at\"");
    columns.push("\"updated_at\"");
    builder.push(") ");
    builder.push_values(codes, |mut row, code| {
        for (column, value) in field_values {
            push_admin_sql_value(&mut row, column, value);
        }
        if matches!(table, GeneratedCodeTable::Coupon) {
            row.push_bind(1_i16);
        }
        row.push_bind(code).push_bind(now).push_bind(now);
    });
    builder.build().execute(&mut **tx).await?;
    Ok(())
}

async fn insert_unique_generated_code_batch(
    tx: &mut DbTransaction<'_>,
    table: GeneratedCodeTable,
    field_values: &[(&'static str, AdminSqlValue)],
    count: usize,
    length: usize,
    now: i64,
) -> Result<Vec<String>, ApiError> {
    for _ in 0..8 {
        let codes = unique_random_codes(count, length);
        match insert_generated_codes(tx, table, field_values, &codes, now).await {
            Ok(()) => return Ok(codes),
            Err(ApiError::Database(error)) if is_unique_violation(&error) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(ApiError::internal(
        "could not allocate a collision-free generated code batch",
    ))
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
}

fn duplicate_code_error(error: ApiError, field: &str, message: &str) -> ApiError {
    match error {
        ApiError::Database(error) if is_unique_violation(&error) => {
            ApiError::validation_field(field, message)
        }
        error => error,
    }
}

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
    pub(super) async fn notice_fetch(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_as::<_, NoticeRaw>(
            "SELECT id, title, content, img_url, tags::text AS tags, \"show\", created_at, updated_at FROM notice ORDER BY id DESC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(
            rows.into_iter().map(NoticeDto::from).collect::<Vec<_>>()
        )))
    }

    pub(super) async fn notice_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        let tags = json_array_string(params, "tags")?
            .map(|value| serde_json::from_str::<Value>(&value))
            .transpose()
            .map_err(|_| ApiError::validation_field("tags", "公告标签格式不正确"))?
            .map(Json);
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                "UPDATE notice SET title = $1, content = $2, img_url = $3, tags = $4, updated_at = $5 WHERE id = $6",
            )
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "content")?)
            .bind(params.get("img_url"))
            .bind(tags)
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO notice (title, content, img_url, tags, \"show\", created_at, updated_at) VALUES ($1, $2, $3, $4, 1, $5, $6)",
            )
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "content")?)
            .bind(params.get("img_url"))
            .bind(tags)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn notice_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let id = required_i64(params, "id")?;
        let mut values = Vec::new();
        if let Some(title) = optional_string(params, "title") {
            values.push(("title", AdminSqlValue::Text(title)));
        }
        if let Some(content) = optional_string(params, "content") {
            values.push(("content", AdminSqlValue::Text(content)));
        }
        if params.contains_key("img_url") {
            values.push(("img_url", optional_text_value(params, "img_url")));
        }
        if params
            .keys()
            .any(|key| key == "tags" || key.starts_with("tags["))
        {
            values.push(("tags", optional_json_array_text_value(params, "tags")));
        }
        if let Some(show) = optional_i64(params, "show") {
            values.push(("show", AdminSqlValue::Integer(show)));
        }
        if values.is_empty() {
            return self
                .toggle("notice", "show", id, ApiError::legacy("公告不存在"))
                .await;
        }

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE notice SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
        }
        builder.push(", \"updated_at\" = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        let result = builder.build().execute(&self.db).await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("公告不存在"));
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn knowledge_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        if let Some(id) = optional_i64(params, "id") {
            let value = fetch_json_one(
                &self.db,
                r#"
                SELECT jsonb_build_object(
                    'id', id, 'language', language, 'category', category, 'title', title,
                    'body', body, 'sort', sort, 'show', "show", 'created_at', created_at,
                    'updated_at', updated_at
                )
                FROM knowledge
                WHERE id = $1
                LIMIT 1
                "#,
                id,
            )
            .await?
            .ok_or_else(|| ApiError::legacy("知识不存在"))?;
            return Ok(AdminOutput::Data(value));
        }
        Ok(AdminOutput::Data(json!(
            fetch_json_list(
                &self.db,
                r#"
            SELECT jsonb_build_object(
                'id', id, 'category', category, 'title', title, 'sort', sort, 'show', "show",
                'updated_at', updated_at
            )
            FROM knowledge
            ORDER BY sort ASC NULLS FIRST
            "#
            )
            .await?
        )))
    }

    pub(super) async fn knowledge_categories(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT category FROM knowledge ORDER BY category ASC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(rows)))
    }

    pub(super) async fn knowledge_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // KnowledgeSave only validates category/language/title/body, so create/update
        // never touch `show` or `sort`: create leaves the DB defaults (show = 0,
        // sort = NULL) and update leaves those columns as-is.
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                r#"
                UPDATE knowledge
                SET language = $1, category = $2, title = $3, body = $4, updated_at = $5
                WHERE id = $6
                "#,
            )
            .bind(required_string(params, "language")?)
            .bind(required_string(params, "category")?)
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "body")?)
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO knowledge (language, category, title, body, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(required_string(params, "language")?)
            .bind(required_string(params, "category")?)
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "body")?)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn ticket_fetch(
        &self,
        params: &HashMap<String, String>,
        staff: bool,
    ) -> Result<AdminOutput, ApiError> {
        if let Some(id) = optional_i64(params, "id") {
            let ticket = fetch_json_one(
                &self.db,
                r#"
                SELECT jsonb_build_object(
                    'id', id, 'user_id', user_id, 'subject', subject, 'level', level,
                    'status', status, 'reply_status', reply_status,
                    'last_reply_user_id', (
                        SELECT user_id FROM ticket_message WHERE ticket_id = ticket.id ORDER BY id DESC LIMIT 1
                    ),
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM ticket
                WHERE id = $1
                LIMIT 1
                "#,
                id,
            )
            .await?
            .ok_or_else(|| ApiError::legacy("工单不存在"))?;
            // is_me marks messages whose author is NOT the ticket owner, i.e. an
            // admin/staff reply (TicketController::fetch :22-30).
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
            let mut ticket = ticket.as_object().cloned().unwrap_or_default();
            ticket.insert("message".to_string(), json!(messages));
            return Ok(AdminOutput::Data(Value::Object(ticket)));
        }

        // Admin list honors the status / reply_status[] / email filters (:37-48)
        // and orders by updated_at. Staff\TicketController::fetch only filters by
        // status and orders by created_at.
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

        let status = params
            .get("status")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<i64>().ok());
        // Staff has no reply_status / email filters.
        let reply_statuses: Vec<i64> = if staff {
            Vec::new()
        } else {
            json_array_param(params, "reply_status")
                .iter()
                .filter_map(Value::as_i64)
                .collect()
        };
        // email present + user found → scope to that user; present-but-unknown or
        // absent → no scope, matching the Laravel `if ($user)` guard.
        let user_id = if !staff && params.contains_key("email") {
            let email = params.get("email").cloned().unwrap_or_default();
            sqlx::query_scalar::<_, i64>(
                "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
            )
            .bind(email)
            .fetch_optional(&self.db)
            .await?
        } else {
            None
        };

        let pagination = page(params)?;
        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ticket WHERE 1 = 1");
        apply_filters(&mut count_builder, status, &reply_statuses, user_id);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(
            r#"
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
            "#,
        );
        apply_filters(&mut builder, status, &reply_statuses, user_id);
        let order_column = if staff { "created_at" } else { "updated_at" };
        builder.push(format!(" ORDER BY {order_column} DESC LIMIT "));
        builder.push_bind(pagination.limit);
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let data = rows.into_iter().map(|row| row.0).collect();
        Ok(AdminOutput::Page { data, total })
    }

    pub(super) async fn ticket_reply(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports TicketService::replyByAdmin (:34-61): records the reply under the
        // acting admin, reopens the ticket (status = 0), sets reply_status based
        // on authorship, and notifies the owner by email (deduped 30 min).
        let id = required_i64(params, "id")?;
        let ticket_id = id;
        let message = required_string(params, "message")?;
        validate_ticket_message_length(&message)?;
        let admin_id = self.current_admin_id(params).await?;
        let (ticket_user_id, subject): (i64, String) =
            sqlx::query_as("SELECT user_id, subject FROM ticket WHERE id = $1 LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::legacy("工单不存在"))?;
        let prepared_notification = self
            .prepare_ticket_reply_notification(ticket_user_id, &subject, &message)
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
                        return Err(ApiError::legacy("工单不存在"));
                    }
                    v2board_db::ticket::OperatorReplyTargetOutcome::OtherOpenTicketExists => {
                        return Err(ApiError::legacy(
                            "用户存在其他未解决工单，无法重新打开该工单",
                        ));
                    }
                };
            if target.user_id != ticket_user_id {
                return Err(ApiError::internal(
                    "ticket owner changed while preparing an admin reply",
                ));
            }
            v2board_db::ticket::apply_operator_reply(&mut tx, &target, admin_id, &message, now)
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
        Ok(AdminOutput::Data(json!(true)))
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

    pub(super) async fn ticket_close(&self, id: i64) -> Result<AdminOutput, ApiError> {
        v2board_db::ticket::close_ticket_as_operator(&self.db, id, Utc::now().timestamp()).await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn coupon_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM coupon")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
            &self.db,
            &format!(
                r#"
            SELECT jsonb_build_object(
                'id', id, 'code', code, 'name', name, 'type', type, 'value', value,
                'show', "show", 'limit_use', limit_use, 'limit_use_with_user', limit_use_with_user,
                'limit_plan_ids', CAST(limit_plan_ids AS JSONB), 'limit_period', CAST(limit_period AS JSONB),
                'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at, 'updated_at', updated_at
            )
            FROM coupon
            {}
            LIMIT $1 OFFSET $2
            "#,
                admin_sort_clause(params)
            ),
            pagination.limit,
            pagination.offset,
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    pub(super) async fn giftcard_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gift_card")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
            &self.db,
            &format!(
                r#"
            SELECT jsonb_build_object(
                'id', id, 'code', code, 'name', name, 'type', type, 'value', value,
                'plan_id', plan_id, 'limit_use', limit_use,
                'used_user_ids', COALESCE(
                    (
                        SELECT jsonb_agg(redemption.user_id)
                        FROM gift_card_redemption AS redemption
                        WHERE redemption.giftcard_id = gift_card.id
                    ),
                    '[]'::jsonb
                ),
                'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at, 'updated_at', updated_at
            )
            FROM gift_card
            {}
            LIMIT $1 OFFSET $2
            "#,
                admin_sort_clause(params)
            ),
            pagination.limit,
            pagination.offset,
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    pub(super) async fn coupon_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports CouponController::generate / multiGenerate. A generate_count marks
        // the CSV batch path; otherwise this is a single create (or update by id).
        // The CouponGenerate FormRequest validates every path before the method body.
        coupon_generate_validation(params)?;
        let now = Utc::now().timestamp();
        if let Some(count) = optional_i64(params, "generate_count").filter(|count| *count > 0) {
            let field_values = coupon_field_values(params);
            let count = usize::try_from(count)
                .map_err(|_| ApiError::validation_field("generate_count", "生成数量格式有误"))?;
            if count > GENERATED_CODE_MAX_ROWS {
                return Err(ApiError::validation_field(
                    "generate_count",
                    "单次最多生成 1000 张优惠券",
                ));
            }
            let mut tx = self.db.begin().await?;
            let codes = insert_unique_generated_code_batch(
                &mut tx,
                GeneratedCodeTable::Coupon,
                &field_values,
                count,
                8,
                now,
            )
            .await?;
            tx.commit().await?;

            let coupon_type = optional_i64(params, "type").unwrap_or_default();
            let value = optional_i64(params, "value").unwrap_or_default();
            let type_label = match coupon_type {
                1 => "金额",
                2 => "比例",
                _ => "",
            };
            let value_display = match coupon_type {
                1 => (value as f64 / 100.0).to_string(),
                2 => value.to_string(),
                _ => String::new(),
            };
            let name = optional_string(params, "name").unwrap_or_default();
            let start = local_datetime(optional_i64(params, "started_at").unwrap_or_default());
            let end = local_datetime(optional_i64(params, "ended_at").unwrap_or_default());
            let limit_use = optional_i64(params, "limit_use")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "不限制".to_string());
            let limit_plan_ids = joined_array_display(params, "limit_plan_ids");
            let create = local_datetime(now);
            let rows = codes.into_iter().map(|code| {
                vec![
                    name.clone(),
                    type_label.to_string(),
                    value_display.clone(),
                    start.clone(),
                    end.clone(),
                    limit_use.clone(),
                    limit_plan_ids.clone(),
                    code,
                    create.clone(),
                ]
            });
            let body = csv_export(
                &[
                    "名称",
                    "类型",
                    "金额或比例",
                    "开始时间",
                    "结束时间",
                    "可用次数",
                    "可用于订阅",
                    "券码",
                    "生成时间",
                ],
                rows,
                false,
            )?;
            return Ok(AdminOutput::Csv {
                filename: "coupon.csv".to_string(),
                body,
            });
        }

        let mut values = coupon_field_values(params);
        if let Some(id) = optional_i64(params, "id") {
            if let Some(code) = optional_string(params, "code") {
                values.push(("code", AdminSqlValue::Text(code)));
            }
            self.update_row("coupon", id, &values, now)
                .await
                .map_err(|error| duplicate_code_error(error, "code", "优惠码已存在"))?;
        } else if let Some(code) = optional_string(params, "code") {
            values.push(("code", AdminSqlValue::Text(code)));
            self.insert_row("coupon", &values, now)
                .await
                .map_err(|error| duplicate_code_error(error, "code", "优惠码已存在"))?;
        } else {
            self.insert_generated_single_code("coupon", &values, 8, now)
                .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn giftcard_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports GiftcardController::generate / multiGenerate. Codes are 16 chars
        // and, in the batch path, retried until unique.
        // The GiftcardGenerate FormRequest validates every path before the body.
        giftcard_generate_validation(params)?;
        let now = Utc::now().timestamp();
        if let Some(count) = optional_i64(params, "generate_count").filter(|count| *count > 0) {
            let field_values = giftcard_field_values(params);
            let count = usize::try_from(count)
                .map_err(|_| ApiError::validation_field("generate_count", "生成数量格式有误"))?;
            if count > GENERATED_CODE_MAX_ROWS {
                return Err(ApiError::validation_field(
                    "generate_count",
                    "单次最多生成 1000 张礼品卡",
                ));
            }
            let mut tx = self.db.begin().await?;
            let codes = insert_unique_generated_code_batch(
                &mut tx,
                GeneratedCodeTable::Giftcard,
                &field_values,
                count,
                16,
                now,
            )
            .await?;
            tx.commit().await?;

            let card_type = optional_i64(params, "type").unwrap_or_default();
            let value = optional_i64(params, "value").unwrap_or_default();
            let type_label = match card_type {
                1 => "金额",
                2 => "时长",
                3 => "流量",
                4 => "重置",
                5 => "套餐",
                _ => "",
            };
            let value_display = match card_type {
                1 => format!("{:.2}", value as f64 / 100.0),
                2 | 5 => format!("{value}天"),
                3 => format!("{value}GB"),
                4 => "-".to_string(),
                _ => String::new(),
            };
            let name = optional_string(params, "name").unwrap_or_default();
            let start = local_datetime(optional_i64(params, "started_at").unwrap_or_default());
            let end = local_datetime(optional_i64(params, "ended_at").unwrap_or_default());
            let limit_use = optional_i64(params, "limit_use")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "不限制".to_string());
            let create = local_datetime(now);
            let rows = codes.into_iter().map(|code| {
                vec![
                    name.clone(),
                    type_label.to_string(),
                    value_display.clone(),
                    start.clone(),
                    end.clone(),
                    limit_use.clone(),
                    code,
                    create.clone(),
                ]
            });
            let body = csv_export(
                &[
                    "名称",
                    "类型",
                    "数值",
                    "开始时间",
                    "结束时间",
                    "可用次数",
                    "礼品卡卡密",
                    "生成时间",
                ],
                rows,
                false,
            )?;
            return Ok(AdminOutput::Csv {
                filename: "giftcard.csv".to_string(),
                body,
            });
        }

        let mut values = giftcard_field_values(params);
        if let Some(id) = optional_i64(params, "id") {
            let exists: Option<i32> =
                sqlx::query_scalar("SELECT id FROM gift_card WHERE id = $1 LIMIT 1")
                    .bind(id)
                    .fetch_optional(&self.db)
                    .await?;
            if exists.is_none() {
                return Err(ApiError::not_found("礼品卡不存在"));
            }
            if let Some(code) = optional_string(params, "code") {
                values.push(("code", AdminSqlValue::Text(code)));
            }
            self.update_row("gift_card", id, &values, now)
                .await
                .map_err(|error| duplicate_code_error(error, "code", "礼品卡卡密已存在"))?;
        } else if let Some(code) = optional_string(params, "code") {
            values.push(("code", AdminSqlValue::Text(code)));
            self.insert_row("gift_card", &values, now)
                .await
                .map_err(|error| duplicate_code_error(error, "code", "礼品卡卡密已存在"))?;
        } else {
            self.insert_generated_single_code("gift_card", &values, 16, now)
                .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Builds and runs a dynamic `INSERT ... (created_at, updated_at)` for the
    /// given whitelisted column/value pairs. Table names are compile-time
    /// literals, so the interpolation is injection-safe.
    async fn insert_row(
        &self,
        table: &str,
        values: &[(&str, AdminSqlValue)],
        now: i64,
    ) -> Result<(), ApiError> {
        let mut builder = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
        let mut columns = builder.separated(", ");
        for (column, _) in values {
            columns.push(format!("\"{column}\""));
        }
        columns.push("\"created_at\"");
        columns.push("\"updated_at\"");
        builder.push(") VALUES (");
        let mut placeholders = builder.separated(", ");
        for (column, value) in values {
            push_admin_sql_value(&mut placeholders, column, value);
        }
        placeholders.push_bind(now);
        placeholders.push_bind(now);
        builder.push(")");
        builder.build().execute(&self.db).await?;
        Ok(())
    }

    async fn insert_generated_single_code(
        &self,
        table: &str,
        values: &[(&str, AdminSqlValue)],
        length: usize,
        now: i64,
    ) -> Result<(), ApiError> {
        for _ in 0..8 {
            let mut candidate = values.to_vec();
            candidate.push(("code", AdminSqlValue::Text(random_char(length))));
            match self.insert_row(table, &candidate, now).await {
                Ok(()) => return Ok(()),
                Err(ApiError::Database(error)) if is_unique_violation(&error) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(ApiError::internal(
            "could not allocate a collision-free generated code",
        ))
    }

    /// Builds and runs a dynamic `UPDATE ... SET ..., updated_at WHERE id = ?`.
    async fn update_row(
        &self,
        table: &str,
        id: i64,
        values: &[(&str, AdminSqlValue)],
        now: i64,
    ) -> Result<(), ApiError> {
        let mut builder = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET "));
        let mut first = true;
        for (column, value) in values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
        }
        if !first {
            builder.push(", ");
        }
        builder.push("\"updated_at\" = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&self.db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod generated_code_tests {
    use super::*;

    #[test]
    fn bulk_code_generation_is_unique_before_the_single_insert() {
        let coupons = unique_random_codes(500, 8);
        assert_eq!(coupons.len(), 500);
        assert!(coupons.iter().all(|code| code.len() == 8));
        assert_eq!(coupons.iter().collect::<HashSet<_>>().len(), 500);

        let source = include_str!("content.rs");
        assert!(source.contains("builder.push_values(codes"));
        assert!(source.contains("insert_unique_generated_code_batch"));
        assert!(source.contains("is_unique_violation"));
        let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
        assert!(finalize.contains("uniq_coupon_code_canonical"));
        assert!(finalize.contains("uniq_gift_card_code_canonical"));
    }
}
