use super::*;
use rust_decimal::{RoundingStrategy, prelude::ToPrimitive};
use tokio::task::JoinSet;

const USER_DELETE_SQL_BATCH_SIZE: usize = 500;
const SESSION_CLEANUP_CONCURRENCY: usize = 8;

pub(super) fn decimal_gib_filter_bytes(value: &str) -> Result<i64, ApiError> {
    value
        .trim()
        .parse::<Decimal>()
        .ok()
        .and_then(|value| value.checked_mul(Decimal::from(GIB)))
        .map(|value| value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero))
        .and_then(|value| value.to_i64())
        .ok_or_else(|| {
            ApiError::validation_field("filter", "Traffic filter is outside the supported range")
        })
}

impl AdminService {
    /// Reconstructs the `filter[]` array into injection-safe WHERE clauses.
    /// Ports UserController::filter (laravel .../Admin/UserController.php:36-62):
    /// `模糊` → LIKE %value%, `d`/`transfer_enable` scaled by GiB, `invite_by_email`
    /// resolved to invite_user_id (0 when not found), and `plan_id == 'null'` → IS NULL.
    /// Unknown columns/operators are dropped rather than interpolated (unlike the
    /// Laravel builder, which trusts the raw request key).
    pub(super) async fn user_filter_clauses(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<Vec<UserFilterClause>, ApiError> {
        let mut clauses = Vec::new();
        for entry in collect_filter_entries(params) {
            let Some(key) = entry.get("key").map(String::as_str) else {
                continue;
            };
            let mut condition = entry
                .get("condition")
                .map(String::as_str)
                .unwrap_or("=")
                .to_string();
            let mut value = entry.get("value").cloned().unwrap_or_default();
            if condition == "模糊" {
                condition = "like".to_string();
                value = format!("%{value}%");
            }
            if key == "d" || key == "transfer_enable" {
                let scaled = decimal_gib_filter_bytes(&value)?;
                let (Some(column), Some(op)) = (user_column(key), user_filter_operator(&condition))
                else {
                    continue;
                };
                clauses.push(UserFilterClause::Compare {
                    column,
                    op,
                    value: FilterBind::Int(scaled),
                });
                continue;
            }
            if key == "invite_by_email" {
                let op = user_filter_operator(&condition).unwrap_or("=");
                let invite_id: Option<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
                    "SELECT id FROM v2_user WHERE email {op} ? LIMIT 1"
                )))
                .bind(&value)
                .fetch_optional(&self.db)
                .await?;
                clauses.push(UserFilterClause::Compare {
                    column: "invite_user_id",
                    op: "=",
                    value: FilterBind::Int(invite_id.unwrap_or(0)),
                });
                continue;
            }
            if key == "plan_id" && value == "null" {
                clauses.push(UserFilterClause::IsNull { column: "plan_id" });
                continue;
            }
            let (Some(column), Some(op)) = (user_column(key), user_filter_operator(&condition))
            else {
                continue;
            };
            clauses.push(UserFilterClause::Compare {
                column,
                op,
                value: FilterBind::Text(value),
            });
        }
        Ok(clauses)
    }

    /// Returns the ids of the users matching the request `filter[]` (used by ban
    /// and allDel to stay scoped, like UserController::ban/allDel).
    async fn filtered_user_ids(
        &self,
        params: &HashMap<String, String>,
        staff_scoped: bool,
    ) -> Result<Vec<i64>, ApiError> {
        let clauses = self.user_filter_clauses(params).await?;
        let mut builder = QueryBuilder::<MySql>::new("SELECT u.id FROM v2_user u WHERE 1 = 1");
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        push_user_where(&mut builder, &clauses);
        let ids = builder
            .build_query_scalar::<i64>()
            .fetch_all(&self.db)
            .await?;
        Ok(ids)
    }

    /// Fetches every user matching the admin list filter through the caller's
    /// transaction, so the selected recipients and every outbox insert commit
    /// as one unit. Staff remains scoped away from admin/staff recipients.
    pub(super) async fn filtered_user_emails_in_tx(
        &self,
        clauses: &[UserFilterClause],
        staff_scoped: bool,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<Vec<String>, ApiError> {
        let mut builder = QueryBuilder::<MySql>::new("SELECT u.email FROM v2_user u WHERE 1 = 1");
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        push_user_where(&mut builder, clauses);
        Ok(builder
            .build_query_scalar::<String>()
            .fetch_all(&mut **tx)
            .await?)
    }

    /// Adds `subscribe_url` and the `alive_ip` / `ips` device stats onto fetched
    /// user rows. Ports the tail of UserController::fetch (:88-105); the alive-IP
    /// cache read is best-effort so a Redis outage does not fail the listing.
    async fn enrich_users(&self, users: &mut [Value]) -> Result<(), ApiError> {
        if users.is_empty() {
            return Ok(());
        }
        let mut cache_rows = Vec::with_capacity(users.len());
        for (index, user) in users.iter_mut().enumerate() {
            let Some(object) = user.as_object_mut() else {
                continue;
            };
            if let Some(token) = object.get("token").and_then(Value::as_str) {
                let url = self.config.subscribe_url_for_token(token);
                object.insert("subscribe_url".to_string(), json!(url));
            }
            let id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
            cache_rows.push((index, id));
        }
        if cache_rows.is_empty() {
            return Ok(());
        }

        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(?error, "admin user device-cache connection unavailable");
                return Ok(());
            }
        };
        let keys = cache_rows
            .iter()
            .map(|(_, id)| format!("ALIVE_IP_USER_{id}"))
            .collect::<Vec<_>>();
        let cached = match conn.mget::<_, Vec<Option<String>>>(&keys).await {
            Ok(cached) => cached,
            Err(error) => {
                tracing::warn!(?error, "admin user device-cache batch read failed");
                return Ok(());
            }
        };
        for ((index, _), raw) in cache_rows.into_iter().zip(cached) {
            if let Some(raw) = raw
                && let Some(object) = users[index].as_object_mut()
            {
                let (alive_ip, ips) = parse_alive_ip(&raw);
                object.insert("alive_ip".to_string(), json!(alive_ip));
                object.insert("ips".to_string(), json!(ips));
            }
        }
        Ok(())
    }

    /// Deletes both session metadata and hashed opaque-token mappings.
    /// Best-effort: the durable database epoch remains authoritative if Redis is unavailable.
    async fn remove_user_sessions(&self, user_id: i64) {
        if let Err(error) =
            crate::auth::remove_user_sessions_from_client(&self.redis, user_id).await
        {
            tracing::warn!(
                ?error,
                user_id,
                "admin session cache cleanup failed after durable revocation"
            );
        }
    }

    /// Reclaims Redis session metadata after the durable database mutation commits. Cleanup is
    /// bounded and best-effort: neither Redis failures nor task failures can flip a committed
    /// admin mutation into an apparent database failure.
    async fn remove_user_sessions_bounded(&self, user_ids: &[i64]) {
        for chunk in user_ids.chunks(SESSION_CLEANUP_CONCURRENCY) {
            let mut tasks = JoinSet::new();
            for user_id in chunk {
                let redis = self.redis.clone();
                let user_id = *user_id;
                tasks.spawn(async move {
                    let result =
                        crate::auth::remove_user_sessions_from_client(&redis, user_id).await;
                    (user_id, result)
                });
            }
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok((_user_id, Ok(()))) => {}
                    Ok((user_id, Err(error))) => tracing::warn!(
                        ?error,
                        user_id,
                        "admin session cache cleanup failed after durable user mutation"
                    ),
                    Err(error) => tracing::warn!(
                        ?error,
                        "admin session cache cleanup task failed after durable user mutation"
                    ),
                }
            }
        }
    }

    /// Resolves the acting admin's user id from the `_admin_email` the router
    /// injects (main.rs adds only the email, not the id).
    pub(super) async fn current_admin_id(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<i64, ApiError> {
        let email = required_string(params, "_admin_email")?;
        sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
            .bind(email)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("管理员不存在"))
    }

    /// Set-based cascade shared by delUser and allDel. Every chunk follows the same table order
    /// and sorted id order, preserving a stable lock acquisition order under concurrent deletes.
    async fn delete_users_cascade(
        &self,
        tx: &mut sqlx::Transaction<'_, MySql>,
        user_ids: &[i64],
    ) -> Result<(), ApiError> {
        for user_ids in user_ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut orders = QueryBuilder::<MySql>::new("DELETE FROM v2_order WHERE user_id IN (");
            push_id_binds(&mut orders, user_ids);
            orders.push(")");
            orders.build().execute(&mut **tx).await?;

            let mut invites =
                QueryBuilder::<MySql>::new("DELETE FROM v2_invite_code WHERE user_id IN (");
            push_id_binds(&mut invites, user_ids);
            invites.push(")");
            invites.build().execute(&mut **tx).await?;

            let mut messages = QueryBuilder::<MySql>::new(
                "DELETE tm FROM v2_ticket_message tm INNER JOIN v2_ticket t ON t.id = tm.ticket_id WHERE t.user_id IN (",
            );
            push_id_binds(&mut messages, user_ids);
            messages.push(")");
            messages.build().execute(&mut **tx).await?;

            let mut tickets =
                QueryBuilder::<MySql>::new("DELETE FROM v2_ticket WHERE user_id IN (");
            push_id_binds(&mut tickets, user_ids);
            tickets.push(")");
            tickets.build().execute(&mut **tx).await?;

            let mut referrals = QueryBuilder::<MySql>::new(
                "UPDATE v2_user SET invite_user_id = NULL WHERE invite_user_id IN (",
            );
            push_id_binds(&mut referrals, user_ids);
            referrals.push(")");
            referrals.build().execute(&mut **tx).await?;

            let mut users = QueryBuilder::<MySql>::new("DELETE FROM v2_user WHERE id IN (");
            push_id_binds(&mut users, user_ids);
            users.push(")");
            users.build().execute(&mut **tx).await?;
        }
        Ok(())
    }

    pub(super) async fn user_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let clauses = self.user_filter_clauses(params).await?;
        let (sort_expr, direction) = user_sort(params);

        let mut count_builder =
            QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM v2_user u WHERE 1 = 1");
        push_user_where(&mut count_builder, &clauses);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<MySql>::new(
            r#"
            SELECT JSON_OBJECT(
                'id', u.id, 'email', u.email, 'password', '', 'balance', u.balance,
                'commission_balance', u.commission_balance, 'transfer_enable', u.transfer_enable,
                'device_limit', u.device_limit, 'u', u.u, 'd', u.d,
                'total_used', CAST(u.u AS DECIMAL(65,0)) + CAST(u.d AS DECIMAL(65,0)),
                'alive_ip', 0, 'ips', '', 'plan_id', u.plan_id, 'plan_name', p.name,
                'group_id', u.group_id, 'expired_at', u.expired_at, 'uuid', u.uuid,
                'token', u.token, 'subscribe_url', '', 'banned', u.banned,
                'is_admin', u.is_admin, 'is_staff', u.is_staff,
                'invite_user_id', u.invite_user_id, 'discount', u.discount,
                'commission_type', u.commission_type, 'commission_rate', u.commission_rate,
                't', u.t, 'speed_limit', u.speed_limit, 'auto_renewal', u.auto_renewal,
                'remind_expire', u.remind_expire, 'remind_traffic', u.remind_traffic,
                'remarks', u.remarks, 'last_login_ip', u.last_login_ip,
                'password_algo', u.password_algo, 'password_salt', u.password_salt,
                'telegram_id', u.telegram_id,
                'last_login_at', u.last_login_at, 'created_at', u.created_at, 'updated_at', u.updated_at
            )
            FROM v2_user u
            LEFT JOIN v2_plan p ON p.id = u.plan_id
            WHERE 1 = 1
            "#,
        );
        push_user_where(&mut builder, &clauses);
        // sort_expr and direction are whitelisted by user_sort, so this raw push is safe.
        builder.push(format!(" ORDER BY {sort_expr} {direction} LIMIT "));
        builder.push_bind(pagination.limit);
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let mut data: Vec<Value> = rows.into_iter().map(|row| row.0).collect();
        self.enrich_users(&mut data).await?;
        Ok(AdminOutput::Page { data, total })
    }

    pub(super) async fn user_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let value = fetch_json_one(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', u.id, 'email', u.email, 'password', '', 'balance', u.balance,
                'commission_balance', u.commission_balance, 'transfer_enable', u.transfer_enable,
                'device_limit', u.device_limit, 'u', u.u, 'd', u.d,
                'total_used', CAST(u.u AS DECIMAL(65,0)) + CAST(u.d AS DECIMAL(65,0)),
                'alive_ip', 0, 'ips', '', 'plan_id', u.plan_id, 'plan_name', p.name,
                'group_id', u.group_id, 'expired_at', u.expired_at, 'uuid', u.uuid,
                'token', u.token, 'subscribe_url', '', 'banned', u.banned,
                'is_admin', u.is_admin, 'is_staff', u.is_staff,
                'invite_user_id', u.invite_user_id, 'discount', u.discount,
                'commission_type', u.commission_type, 'commission_rate', u.commission_rate,
                't', u.t, 'speed_limit', u.speed_limit, 'auto_renewal', u.auto_renewal,
                'remind_expire', u.remind_expire, 'remind_traffic', u.remind_traffic,
                'remarks', u.remarks, 'last_login_ip', u.last_login_ip,
                'password_algo', u.password_algo, 'password_salt', u.password_salt,
                'telegram_id', u.telegram_id,
                'last_login_at', u.last_login_at, 'created_at', u.created_at, 'updated_at', u.updated_at,
                'invite_user', IF(i.id IS NULL, NULL, JSON_OBJECT('id', i.id, 'email', i.email))
            )
            FROM v2_user u
            LEFT JOIN v2_plan p ON p.id = u.plan_id
            LEFT JOIN v2_user i ON i.id = u.invite_user_id
            WHERE u.id = ?
            LIMIT 1
            "#,
            id,
        )
        .await?
        .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        Ok(AdminOutput::Data(value))
    }

    pub(super) async fn staff_user_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let value = fetch_json_one(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', u.id, 'email', u.email, 'password', '', 'balance', u.balance,
                'commission_balance', u.commission_balance, 'transfer_enable', u.transfer_enable,
                'device_limit', u.device_limit, 'u', u.u, 'd', u.d,
                'total_used', CAST(u.u AS DECIMAL(65,0)) + CAST(u.d AS DECIMAL(65,0)),
                'alive_ip', 0, 'ips', '', 'plan_id', u.plan_id, 'plan_name', p.name,
                'group_id', u.group_id, 'expired_at', u.expired_at, 'uuid', u.uuid,
                'token', u.token, 'subscribe_url', '', 'banned', u.banned,
                'is_admin', u.is_admin, 'is_staff', u.is_staff,
                'invite_user_id', u.invite_user_id, 'discount', u.discount,
                'commission_type', u.commission_type, 'commission_rate', u.commission_rate,
                't', u.t, 'speed_limit', u.speed_limit, 'auto_renewal', u.auto_renewal,
                'remind_expire', u.remind_expire, 'remind_traffic', u.remind_traffic,
                'remarks', u.remarks, 'last_login_ip', u.last_login_ip,
                'password_algo', u.password_algo, 'password_salt', u.password_salt,
                'telegram_id', u.telegram_id,
                'last_login_at', u.last_login_at, 'created_at', u.created_at, 'updated_at', u.updated_at
            )
            FROM v2_user u
            LEFT JOIN v2_plan p ON p.id = u.plan_id
            WHERE u.id = ? AND u.is_admin = 0 AND u.is_staff = 0
            LIMIT 1
            "#,
            id,
        )
        .await?
        .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        Ok(AdminOutput::Data(value))
    }

    pub(super) async fn user_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::update (laravel .../Admin/UserController.php:125-172).
        let id = required_i64(params, "id")?;
        let current_email: String =
            sqlx::query_scalar("SELECT email FROM v2_user WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        let email = required_string(params, "email")?;
        if email != current_email {
            let taken: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                    .bind(&email)
                    .fetch_optional(&self.db)
                    .await?;
            if taken.is_some() {
                return Err(ApiError::legacy("邮箱已被使用"));
            }
        }

        let mut values: Vec<(&str, AdminSqlValue)> = vec![("email", AdminSqlValue::Text(email))];
        // transfer_enable is stored as-is: the admin UI already sends bytes, so the
        // previous `* GIB` double-scaled it. Laravel's update() stores the raw value.
        for key in [
            "transfer_enable",
            "device_limit",
            "expired_at",
            "banned",
            "commission_rate",
            "discount",
            "is_admin",
            "is_staff",
            "u",
            "d",
            "balance",
            "commission_type",
            "commission_balance",
            "speed_limit",
        ] {
            if params.contains_key(key) {
                values.push((key, optional_int_or_null_value(params, key)));
            }
        }
        if params.contains_key("remarks") {
            values.push(("remarks", optional_text_value(params, "remarks")));
        }

        // plan_id drives group_id (:145-153): a set plan_id resolves group_id from
        // the plan, otherwise group_id is reset to NULL.
        let mut group_id = AdminSqlValue::Null;
        if params.contains_key("plan_id") {
            if let Some(plan_id) = optional_i64(params, "plan_id") {
                let plan_group: Option<i64> =
                    sqlx::query_scalar("SELECT group_id FROM v2_plan WHERE id = ? LIMIT 1")
                        .bind(plan_id)
                        .fetch_optional(&self.db)
                        .await?
                        .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?;
                group_id = plan_group
                    .map(AdminSqlValue::Integer)
                    .unwrap_or(AdminSqlValue::Null);
                values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
            } else {
                values.push(("plan_id", AdminSqlValue::Null));
            }
        }
        values.push(("group_id", group_id));

        // invite_user_email → invite_user_id (:155-162). A present-but-unknown
        // email leaves invite_user_id untouched; an absent email resets it to NULL.
        match params
            .get("invite_user_email")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            Some(invite_email) => {
                if let Some(invite_id) =
                    sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                        .bind(invite_email)
                        .fetch_optional(&self.db)
                        .await?
                {
                    values.push(("invite_user_id", AdminSqlValue::Integer(invite_id)));
                }
            }
            None => values.push(("invite_user_id", AdminSqlValue::Null)),
        }

        let password_changed = params
            .get("password")
            .is_some_and(|value| !value.is_empty());
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = self.password_kdf.hash(password).await?;
            values.push(("password", AdminSqlValue::Text(hash)));
            values.push(("password_algo", AdminSqlValue::Null));
        }

        let revokes_sessions = password_changed || optional_i64(params, "banned") == Some(1);

        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("`{column}` = "));
            push_admin_sql_bind(&mut builder, value);
        }
        if revokes_sessions {
            builder.push(", `session_epoch` = `session_epoch` + 1");
        }
        builder.push(", `updated_at` = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&self.db).await?;
        if revokes_sessions {
            self.remove_user_sessions(id).await;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn staff_user_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports Staff\UserController::update. Staff cannot touch speed_limit,
        // is_admin, is_staff, commission_type or remarks (Staff\UserUpdate rules),
        // and — unlike Laravel's unscoped find — the target stays restricted to
        // non-admin/non-staff users.
        let id = required_i64(params, "id")?;
        let current_email: String = sqlx::query_scalar(
            "SELECT email FROM v2_user WHERE id = ? AND is_admin = 0 AND is_staff = 0 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        let email = required_string(params, "email")?;
        if email != current_email {
            let taken: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                    .bind(&email)
                    .fetch_optional(&self.db)
                    .await?;
            if taken.is_some() {
                return Err(ApiError::legacy("邮箱已被使用"));
            }
        }

        let mut values: Vec<(&str, AdminSqlValue)> = vec![("email", AdminSqlValue::Text(email))];
        for key in [
            "transfer_enable",
            "device_limit",
            "expired_at",
            "banned",
            "commission_rate",
            "discount",
            "u",
            "d",
            "balance",
            "commission_balance",
        ] {
            if params.contains_key(key) {
                values.push((key, optional_int_or_null_value(params, key)));
            }
        }
        // Staff update only sets group_id when a real plan_id is supplied.
        if params.contains_key("plan_id") {
            if let Some(plan_id) = optional_i64(params, "plan_id") {
                let plan_group: Option<i64> =
                    sqlx::query_scalar("SELECT group_id FROM v2_plan WHERE id = ? LIMIT 1")
                        .bind(plan_id)
                        .fetch_optional(&self.db)
                        .await?
                        .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?;
                values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
                values.push((
                    "group_id",
                    plan_group
                        .map(AdminSqlValue::Integer)
                        .unwrap_or(AdminSqlValue::Null),
                ));
            } else {
                values.push(("plan_id", AdminSqlValue::Null));
            }
        }
        let password_changed = params
            .get("password")
            .is_some_and(|value| !value.is_empty());
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = self.password_kdf.hash(password).await?;
            values.push(("password", AdminSqlValue::Text(hash)));
            values.push(("password_algo", AdminSqlValue::Null));
        }
        let revokes_sessions = password_changed || optional_i64(params, "banned") == Some(1);

        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("`{column}` = "));
            push_admin_sql_bind(&mut builder, value);
        }
        if revokes_sessions {
            builder.push(", `session_epoch` = `session_epoch` + 1");
        }
        builder.push(", `updated_at` = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.push(" AND is_admin = 0 AND is_staff = 0");
        builder.build().execute(&self.db).await?;
        if revokes_sessions {
            self.remove_user_sessions(id).await;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn staff_send_mail_to_users(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        self.enqueue_mail_to_users(params, true).await
    }

    pub(super) async fn staff_user_bulk_ban(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Staff safety remains scoped to non-admin/non-staff users. The durable epoch check is
        // deliberately stronger than the retired implementation's Redis-only admin revocation.
        let ids = self.filtered_user_ids(params, true).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut builder = QueryBuilder::<MySql>::new(
            "UPDATE v2_user SET banned = 1, session_epoch = session_epoch + 1, updated_at = ",
        );
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        builder.push(")");
        builder.build().execute(&self.db).await?;
        self.remove_user_sessions_bounded(&ids).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn user_reset_secret(&self, id: i64) -> Result<AdminOutput, ApiError> {
        sqlx::query("UPDATE v2_user SET token = ?, uuid = ?, updated_at = ? WHERE id = ?")
            .bind(random_token())
            .bind(Uuid::new_v4().to_string())
            .bind(Utc::now().timestamp())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Resolves the plan referenced by a generate request into
    /// `(id, group_id, transfer_enable_bytes, device_limit)`. Ports the
    /// `Plan::find` guard shared by generate() and multiGenerate().
    async fn generate_plan(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<Option<(i64, Option<i64>, i64, Option<i64>)>, ApiError> {
        let Some(plan_id) = optional_i64(params, "plan_id") else {
            return Ok(None);
        };
        let row: (i64, Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT id, group_id, transfer_enable, device_limit FROM v2_plan WHERE id = ? LIMIT 1",
        )
        .bind(plan_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?;
        Ok(Some((
            row.0,
            row.1,
            checked_gib_bytes(row.2.unwrap_or_default(), "transfer_enable")?,
            row.3,
        )))
    }

    pub(super) async fn user_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::generate (:204-236) and multiGenerate (:238-279).
        // The UserGenerate FormRequest validates before the method body: it makes
        // email_suffix required and integer-checks generate_count/expired_at/plan_id.
        user_generate_validation(params)?;
        let now = Utc::now().timestamp();
        let plan = self.generate_plan(params).await?;
        let (plan_id, group_id, transfer_enable, device_limit) = match plan {
            Some((id, group_id, transfer_enable, device_limit)) => {
                (Some(id), group_id, transfer_enable, device_limit)
            }
            None => (None, None, 0, None),
        };

        // Single generation returns JSON; the CSV path is multiGenerate only.
        if let Some(prefix) = optional_string(params, "email_prefix") {
            let suffix = required_string(params, "email_suffix")?;
            let email = format!("{prefix}@{suffix}");
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                    .bind(&email)
                    .fetch_optional(&self.db)
                    .await?;
            if exists.is_some() {
                return Err(ApiError::legacy("邮箱已存在于系统中"));
            }
            let password_plain = params
                .get("password")
                .filter(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| email.clone());
            let hash = self.password_kdf.hash(&password_plain).await?;
            sqlx::query(
                r#"
                INSERT INTO v2_user (
                    email, plan_id, group_id, transfer_enable, device_limit, expired_at,
                    uuid, token, password, password_algo, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
                "#,
            )
            .bind(&email)
            .bind(plan_id)
            .bind(group_id)
            .bind(transfer_enable)
            .bind(device_limit)
            .bind(optional_i64(params, "expired_at"))
            .bind(Uuid::new_v4().to_string())
            .bind(random_token())
            .bind(&hash)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
            return Ok(AdminOutput::Data(json!(true)));
        }

        let count = optional_i64(params, "generate_count").unwrap_or_default();
        if count <= 0 {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let count = usize::try_from(count)
            .map_err(|_| ApiError::validation_field("generate_count", "生成数量格式有误"))?;
        let suffix = required_string(params, "email_suffix")?;
        let input_password = params
            .get("password")
            .filter(|value| !value.is_empty())
            .cloned();
        let expired_at = optional_i64(params, "expired_at");
        let mut jobs = JoinSet::new();
        let mut emails = HashSet::with_capacity(count);
        while emails.len() < count {
            emails.insert(format!("{}@{}", random_char(6), suffix));
        }
        for (index, email) in emails.into_iter().enumerate() {
            let password_plain = input_password.clone().unwrap_or_else(|| email.clone());
            let uuid = Uuid::new_v4().to_string();
            let token = random_token();
            let password_kdf = self.password_kdf.clone();
            jobs.spawn(async move {
                let hash = password_kdf.hash(&password_plain).await?;
                Ok::<_, ApiError>((index, email, password_plain, uuid, token, hash))
            });
        }
        let mut prepared = Vec::with_capacity(count);
        while let Some(result) = jobs.join_next().await {
            prepared.push(result.map_err(|error| {
                ApiError::internal(format!("password generation task failed: {error}"))
            })??);
        }
        prepared.sort_unstable_by_key(|(index, ..)| *index);

        let mut tx = self.db.begin().await?;
        let mut insert = QueryBuilder::<MySql>::new(
            r#"
            INSERT INTO v2_user (
                email, plan_id, group_id, transfer_enable, device_limit, expired_at,
                uuid, token, password, password_algo, created_at, updated_at
            )
            "#,
        );
        insert.push_values(&prepared, |mut row, (_, email, _, uuid, token, hash)| {
            row.push_bind(email)
                .push_bind(plan_id)
                .push_bind(group_id)
                .push_bind(transfer_enable)
                .push_bind(device_limit)
                .push_bind(expired_at)
                .push_bind(uuid)
                .push_bind(token)
                .push_bind(hash)
                .push_bind(Option::<String>::None)
                .push_bind(now)
                .push_bind(now);
        });
        insert.build().execute(&mut *tx).await?;
        tx.commit().await?;

        let create_date = local_datetime(now);
        let expire = expired_at
            .map(local_datetime)
            .unwrap_or_else(|| "长期有效".to_string());
        let rows = prepared
            .into_iter()
            .map(|(_, email, password_plain, uuid, token, _)| {
                vec![
                    email,
                    password_plain,
                    expire.clone(),
                    uuid,
                    create_date.clone(),
                    self.config.subscribe_url_for_token(&token),
                ]
            });
        let body = csv_export(
            &["账号", "密码", "过期时间", "UUID", "创建时间", "订阅地址"],
            rows,
            false,
        )?;
        Ok(AdminOutput::Csv {
            filename: "users.csv".to_string(),
            body,
        })
    }

    pub(super) async fn user_dump_csv(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::dumpCSV (:174-202). device_limit is emitted for
        // real here — the Laravel row reads a `devce_limit` typo that always
        // produced an empty column.
        let clauses = self.user_filter_clauses(params).await?;
        let mut builder = QueryBuilder::<MySql>::new(
            "SELECT u.email AS email, u.balance AS balance, \
             u.commission_balance AS commission_balance, u.transfer_enable AS transfer_enable, \
             u.u AS u, u.d AS d, u.device_limit AS device_limit, u.expired_at AS expired_at, \
             p.name AS plan_name, u.token AS token \
             FROM v2_user u LEFT JOIN v2_plan p ON p.id = u.plan_id WHERE 1 = 1",
        );
        push_user_where(&mut builder, &clauses);
        builder.push(" ORDER BY u.id ASC");
        let rows = builder
            .build_query_as::<UserDumpRow>()
            .fetch_all(&self.db)
            .await?;

        let rows = rows.into_iter().map(|row| {
            let expire = row
                .expired_at
                .map(local_datetime)
                .unwrap_or_else(|| "长期有效".to_string());
            let balance = row.balance as f64 / 100.0;
            let commission = row.commission_balance as f64 / 100.0;
            let transfer = if row.transfer_enable != 0 {
                row.transfer_enable as f64 / GIB as f64
            } else {
                0.0
            };
            let device = row
                .device_limit
                .map(|value| value.to_string())
                .unwrap_or_default();
            let used = i128::from(row.u) + i128::from(row.d);
            let not_use = (i128::from(row.transfer_enable) - used) as f64 / GIB as f64;
            let plan = row.plan_name.unwrap_or_else(|| "无订阅".to_string());
            let url = self.config.subscribe_url_for_token(&row.token);
            vec![
                row.email,
                balance.to_string(),
                commission.to_string(),
                transfer.to_string(),
                device,
                not_use.to_string(),
                expire,
                plan,
                url,
            ]
        });
        let body = csv_export(
            &[
                "邮箱",
                "余额",
                "推广佣金",
                "总流量",
                "设备数限制",
                "剩余流量",
                "套餐到期时间",
                "订阅计划",
                "订阅地址",
            ],
            rows,
            true,
        )?;
        Ok(AdminOutput::Csv {
            filename: "users.csv".to_string(),
            body,
        })
    }

    pub(super) async fn user_bulk_flag(
        &self,
        params: &HashMap<String, String>,
        column: &str,
        value: i64,
    ) -> Result<AdminOutput, ApiError> {
        // The database epoch is authoritative; Redis deletion after the update only reclaims the
        // cached session metadata and may safely fail without leaving a banned session usable.
        if column != "banned" {
            return Err(ApiError::legacy("Invalid user flag"));
        }
        let ids = self.filtered_user_ids(params, false).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET banned = ");
        builder.push_bind(value);
        builder.push(", session_epoch = session_epoch + 1");
        builder.push(", updated_at = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        builder.push(")");
        builder.build().execute(&self.db).await?;
        self.remove_user_sessions_bounded(&ids).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn user_bulk_delete(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::allDel (:328-359): scoped to the request filter,
        // cascading orders / invite codes / tickets and detaching referrals for
        // each user inside a single transaction, then deleting the matched users.
        let ids = sorted_unique_user_ids(self.filtered_user_ids(params, false).await?);
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut tx = self.db.begin().await?;
        if Self::users_have_pending_stripe_intents_in_tx(&mut tx, &ids).await? {
            return Err(ApiError::legacy(
                "所选用户仍有待支付的 Stripe 订单，请先取消订单",
            ));
        }
        self.delete_users_cascade(&mut tx, &ids).await?;
        tx.commit().await?;
        self.remove_user_sessions_bounded(&ids).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn del_user(&self, id: i64) -> Result<AdminOutput, ApiError> {
        // Ports UserController::delUser (:361-391): single-user cascade delete.
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM v2_user WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("用户不存在"));
        }
        let mut tx = self.db.begin().await?;
        if Self::users_have_pending_stripe_intents_in_tx(&mut tx, &[id]).await? {
            return Err(ApiError::legacy(
                "该用户仍有待支付的 Stripe 订单，请先取消订单",
            ));
        }
        self.delete_users_cascade(&mut tx, &[id]).await?;
        tx.commit().await?;
        self.remove_user_sessions_bounded(&[id]).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn users_have_pending_stripe_intents_in_tx(
        tx: &mut sqlx::Transaction<'_, MySql>,
        user_ids: &[i64],
    ) -> Result<bool, ApiError> {
        if user_ids.is_empty() {
            return Ok(false);
        }
        for user_ids in user_ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut builder = QueryBuilder::<MySql>::new(
                "SELECT callback_no FROM v2_order WHERE status = 0 AND user_id IN (",
            );
            push_id_binds(&mut builder, user_ids);
            builder.push(") ORDER BY user_id, id FOR UPDATE");
            let callback_numbers = builder
                .build_query_scalar::<Option<String>>()
                .fetch_all(&mut **tx)
                .await?;
            if callback_numbers
                .iter()
                .flatten()
                .any(|callback_no| callback_no.starts_with("pi_"))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) async fn user_set_invite(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let user_id = required_i64(params, "user_id")?;
        let invite_user_id = optional_i64(params, "invite_user_id");
        sqlx::query("UPDATE v2_user SET invite_user_id = ?, updated_at = ? WHERE id = ?")
            .bind(invite_user_id)
            .bind(Utc::now().timestamp())
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }
}

fn push_id_binds(builder: &mut QueryBuilder<MySql>, user_ids: &[i64]) {
    let mut separated = builder.separated(", ");
    for user_id in user_ids {
        separated.push_bind(*user_id);
    }
}

fn sorted_unique_user_ids(mut user_ids: Vec<i64>) -> Vec<i64> {
    user_ids.sort_unstable();
    user_ids.dedup();
    user_ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_ids_are_sorted_and_deduplicated_for_stable_locking() {
        assert_eq!(sorted_unique_user_ids(vec![9, 2, 9, 4]), vec![2, 4, 9]);
    }

    #[test]
    fn user_deletion_is_set_based_and_cache_cleanup_is_post_commit() {
        let source = include_str!("users.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for sql in [
            "DELETE FROM v2_order WHERE user_id IN (",
            "DELETE FROM v2_invite_code WHERE user_id IN (",
            "WHERE t.user_id IN (",
            "DELETE FROM v2_ticket WHERE user_id IN (",
            "WHERE invite_user_id IN (",
            "DELETE FROM v2_user WHERE id IN (",
        ] {
            assert!(production.contains(sql), "missing set-based cascade: {sql}");
        }
        assert!(!production.contains("delete_user_cascade("));

        let bulk_start = production
            .find("pub(super) async fn user_bulk_delete")
            .unwrap();
        let single_start = production.find("pub(super) async fn del_user").unwrap();
        let bulk = &production[bulk_start..single_start];
        assert!(
            bulk.find("tx.commit().await?").unwrap()
                < bulk.find("remove_user_sessions_bounded").unwrap()
        );

        let single = &production[single_start..];
        assert!(
            single.find("tx.commit().await?").unwrap()
                < single.find("remove_user_sessions_bounded").unwrap()
        );
        assert!(production.contains("SESSION_CLEANUP_CONCURRENCY: usize = 8"));

        let flag_start = production
            .find("pub(super) async fn user_bulk_flag")
            .unwrap();
        let flag = &production[flag_start..bulk_start];
        assert!(flag.contains("remove_user_sessions_bounded(&ids)"));
        assert!(!flag.contains("for id in ids"));
    }

    #[test]
    fn multi_user_generation_hashes_before_transaction_and_inserts_as_one_batch() {
        let source = include_str!("users.rs");
        let generate_start = source.find("pub(super) async fn user_generate").unwrap();
        let generate_end = source[generate_start..]
            .find("pub(super) async fn user_export")
            .map(|offset| generate_start + offset)
            .unwrap_or(source.len());
        let generate = &source[generate_start..generate_end];
        let jobs = generate.find("let mut jobs = JoinSet::new()").unwrap();
        let joined = generate
            .find("while let Some(result) = jobs.join_next()")
            .unwrap();
        let transaction = generate.find("self.db.begin()").unwrap();
        assert!(jobs < joined);
        assert!(joined < transaction);
        assert!(generate.contains("insert.push_values(&prepared"));
        assert!(!generate[jobs..transaction].contains(".execute(&mut *tx)"));
    }
}
