use super::*;
use rust_decimal::{RoundingStrategy, prelude::ToPrimitive};
use tokio::task::JoinSet;

const USER_DELETE_SQL_BATCH_SIZE: usize = 500;
const USER_MUTATION_PAGE_SIZE: i64 = 500;
const USER_BULK_MAX_ROWS: usize = 10_000;
const USER_CSV_PAGE_SIZE: i64 = 500;
const USER_CSV_MAX_ROWS: usize = 50_000;
const GENERATED_USER_MAX_ROWS: usize = 1_000;
const SESSION_CLEANUP_CONCURRENCY: usize = 8;

/// Attaches the method-aware `subscribe_url` onto fetched user rows through
/// the shared minter (`Helper::getSubscribeUrl` in the tail of
/// UserController::fetch, Admin/UserController.php:103). Under
/// show_subscribe_method 1 each row reuses its cached `otp_` token via one
/// mint-script round-trip on the one shared connection.
pub(super) async fn attach_subscribe_urls<C>(
    config: &AppConfig,
    redis_keys: &RedisKeyspace,
    conn: &mut Option<C>,
    users: &mut [Value],
) -> Result<(), ApiError>
where
    C: redis::aio::ConnectionLike + Send,
{
    for user in users.iter_mut() {
        let Some(object) = user.as_object_mut() else {
            continue;
        };
        let Some(token) = object
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let user_id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
        let url = crate::subscribe_link::subscribe_url_for_user(
            config, redis_keys, conn, user_id, &token,
        )
        .await?;
        object.insert("subscribe_url".to_string(), json!(url));
    }
    Ok(())
}

/// The one SELECT behind every admin-facing user projection (`user_fetch`,
/// `user_detail`, `staff_user_detail`). Callers append their own `AND …`
/// filter clauses after the `WHERE 1 = 1` anchor. Credential-verification
/// columns (`password`, `password_algo`, `password_salt`) and the unconsumed
/// `last_login_ip` are deliberately absent: they never leave the database
/// through an admin response.
const ADMIN_USER_SELECT: &str = "\
    SELECT u.id, u.email, u.balance, u.commission_balance, u.transfer_enable, \
           u.device_limit, u.u, u.d, u.plan_id, p.name AS plan_name, u.group_id, \
           u.expired_at, u.uuid, u.token, u.banned, u.is_admin, u.is_staff, \
           u.invite_user_id, u.discount, u.commission_type, u.commission_rate, \
           u.t, u.speed_limit, u.auto_renewal, u.remind_expire, u.remind_traffic, \
           u.remarks, u.telegram_id, u.last_login_at, u.created_at, u.updated_at \
    FROM users u \
    LEFT JOIN plan p ON p.id = u.plan_id \
    WHERE 1 = 1";

/// One typed row of the shared admin user projection. `into_value` is the
/// producer-side contract: it emits exactly the key set the admin frontend
/// consumes, with `password` blanked and `subscribe_url`/`alive_ip`/`ips`
/// carrying the pre-enrichment defaults that `enrich_users` may later
/// overwrite in place.
#[derive(Debug, sqlx::FromRow)]
pub(super) struct AdminUserRecord {
    id: i64,
    email: String,
    balance: i32,
    commission_balance: i32,
    transfer_enable: i64,
    device_limit: Option<i32>,
    u: i64,
    d: i64,
    plan_id: Option<i32>,
    plan_name: Option<String>,
    group_id: Option<i32>,
    expired_at: Option<i64>,
    uuid: String,
    token: String,
    banned: i16,
    is_admin: i16,
    is_staff: i16,
    invite_user_id: Option<i64>,
    discount: Option<i32>,
    commission_type: i16,
    commission_rate: Option<i32>,
    t: i64,
    speed_limit: Option<i32>,
    auto_renewal: Option<i16>,
    remind_expire: Option<i16>,
    remind_traffic: Option<i16>,
    remarks: Option<String>,
    telegram_id: Option<i64>,
    last_login_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl AdminUserRecord {
    fn into_value(self) -> Value {
        // u and d are non-negative by check constraint, so the sum always
        // fits u64 (the legacy SQL used NUMERIC(65,0) for the same reason).
        let total_used = u64::try_from(self.u)
            .unwrap_or_default()
            .saturating_add(u64::try_from(self.d).unwrap_or_default());
        json!({
            "id": self.id,
            "email": self.email,
            "password": "",
            "balance": self.balance,
            "commission_balance": self.commission_balance,
            "transfer_enable": self.transfer_enable,
            "device_limit": self.device_limit,
            "u": self.u,
            "d": self.d,
            "total_used": total_used,
            "alive_ip": 0,
            "ips": "",
            "plan_id": self.plan_id,
            "plan_name": self.plan_name,
            "group_id": self.group_id,
            "expired_at": self.expired_at,
            "uuid": self.uuid,
            "token": self.token,
            "subscribe_url": "",
            "banned": self.banned,
            "is_admin": self.is_admin,
            "is_staff": self.is_staff,
            "invite_user_id": self.invite_user_id,
            "discount": self.discount,
            "commission_type": self.commission_type,
            "commission_rate": self.commission_rate,
            "t": self.t,
            "speed_limit": self.speed_limit,
            "auto_renewal": self.auto_renewal,
            "remind_expire": self.remind_expire,
            "remind_traffic": self.remind_traffic,
            "remarks": self.remarks,
            "telegram_id": self.telegram_id,
            "last_login_at": self.last_login_at,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

/// Attaches the `invite_user` object only when the inviter row still exists,
/// matching the legacy jsonb `CASE WHEN i.id IS NULL THEN '{}'` merge: an
/// absent or dangling inviter omits the key entirely rather than emitting null.
fn attach_invite_user(value: &mut Value, inviter: Option<(i64, String)>) {
    let Some((id, email)) = inviter else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "invite_user".to_string(),
            json!({ "id": id, "email": email }),
        );
    }
}

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
                let predicate = if op == "like" {
                    "email ILIKE $1".to_string()
                } else if matches!(op, "=" | "<>") {
                    format!("lower(btrim(email)) {op} lower(btrim($1))")
                } else {
                    format!("email {op} $1")
                };
                let invite_id: Option<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
                    "SELECT id FROM users WHERE {predicate} LIMIT 1"
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
                value: if op == "like" || !user_column_is_numeric(column) {
                    FilterBind::Text(value)
                } else {
                    FilterBind::Int(value.trim().parse().unwrap_or_default())
                },
            });
        }
        Ok(clauses)
    }

    /// Returns one primary-key page of users matching the request filter. Bulk
    /// mutations advance this cursor after every bounded database/cache batch
    /// instead of retaining the whole account table in memory.
    async fn filtered_user_id_page(
        &self,
        clauses: &[UserFilterClause],
        staff_scoped: bool,
        after_id: i64,
    ) -> Result<Vec<i64>, ApiError> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT u.id FROM users u WHERE 1 = 1");
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        push_user_where(&mut builder, clauses);
        builder.push(" AND u.id > ");
        builder.push_bind(after_id);
        builder.push(" ORDER BY u.id LIMIT ");
        builder.push_bind(USER_MUTATION_PAGE_SIZE);
        Ok(builder
            .build_query_scalar::<i64>()
            .fetch_all(&self.db)
            .await?)
    }

    async fn filtered_user_ids_bounded(
        &self,
        clauses: &[UserFilterClause],
        staff_scoped: bool,
    ) -> Result<Vec<i64>, ApiError> {
        let mut ids = Vec::new();
        let mut after_id = 0_i64;
        loop {
            let page = self
                .filtered_user_id_page(clauses, staff_scoped, after_id)
                .await?;
            let Some(last_id) = page.last().copied() else {
                break;
            };
            if ids.len().saturating_add(page.len()) > USER_BULK_MAX_ROWS {
                return Err(ApiError::business(
                    "单次最多批量操作 10000 个用户，请缩小筛选范围",
                ));
            }
            ids.extend(page);
            after_id = last_id;
        }
        Ok(ids)
    }

    /// Fetches one recipient page through the caller's transaction. The cursor
    /// and all outbox inserts share that transaction, preserving the atomic
    /// audience snapshot without an unbounded email vector.
    pub(super) async fn filtered_user_email_page_in_tx(
        &self,
        clauses: &[UserFilterClause],
        staff_scoped: bool,
        after_id: i64,
        tx: &mut DbTransaction<'_>,
    ) -> Result<Vec<(i64, String)>, ApiError> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT u.id, u.email FROM users u WHERE 1 = 1");
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        push_user_where(&mut builder, clauses);
        builder.push(" AND u.id > ");
        builder.push_bind(after_id);
        builder.push(" ORDER BY u.id LIMIT ");
        builder.push_bind(USER_MUTATION_PAGE_SIZE);
        Ok(builder
            .build_query_as::<(i64, String)>()
            .fetch_all(&mut **tx)
            .await?)
    }

    /// One shared Redis connection for method-1 one-time subscribe-token
    /// minting (one mint-script round-trip per row); methods 0/2 never touch
    /// Redis, exactly like Helper::getSubscribeUrl, which only reads the
    /// `otp_` cache under method 1 (Admin/UserController.php:103,197,275).
    async fn subscribe_mint_connection(
        &self,
    ) -> Result<Option<redis::aio::MultiplexedConnection>, ApiError> {
        if self.config.show_subscribe_method != 1 {
            return Ok(None);
        }
        Ok(Some(self.redis.get_multiplexed_async_connection().await?))
    }

    /// Adds `subscribe_url` and the `alive_ip` / `ips` device stats onto fetched
    /// user rows. Ports the tail of UserController::fetch (:88-105); the alive-IP
    /// cache read is best-effort so a Redis outage does not fail the listing.
    async fn enrich_users(&self, users: &mut [Value]) -> Result<(), ApiError> {
        if users.is_empty() {
            return Ok(());
        }
        let mut mint_conn = self.subscribe_mint_connection().await?;
        attach_subscribe_urls(&self.config, &self.redis_keys, &mut mint_conn, users).await?;
        let mut cache_rows = Vec::with_capacity(users.len());
        for (index, user) in users.iter().enumerate() {
            let Some(object) = user.as_object() else {
                continue;
            };
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
        for cache_rows in cache_rows.chunks(REDIS_MGET_BATCH_SIZE) {
            let keys = cache_rows
                .iter()
                .map(|(_, id)| self.redis_key(&format!("ALIVE_IP_USER_{id}")))
                .collect::<Vec<_>>();
            let cached = match conn.mget::<_, Vec<Option<String>>>(&keys).await {
                Ok(cached) => cached,
                Err(error) => {
                    tracing::warn!(?error, "admin user device-cache batch read failed");
                    return Ok(());
                }
            };
            for ((index, _), raw) in cache_rows.iter().copied().zip(cached) {
                if let Some(raw) = raw
                    && let Some(object) = users[index].as_object_mut()
                {
                    let (alive_ip, ips) = parse_alive_ip(&raw);
                    object.insert("alive_ip".to_string(), json!(alive_ip));
                    object.insert("ips".to_string(), json!(ips));
                }
            }
        }
        Ok(())
    }

    /// Deletes both session metadata and hashed opaque-token mappings.
    /// Best-effort: the durable database epoch remains authoritative if Redis is unavailable.
    async fn remove_user_sessions(&self, user_id: i64) {
        if let Err(error) =
            crate::auth::remove_user_sessions_from_client(&self.redis, &self.redis_keys, user_id)
                .await
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
                let redis_keys = self.redis_keys.clone();
                let user_id = *user_id;
                tasks.spawn(async move {
                    let result =
                        crate::auth::remove_user_sessions_from_client(&redis, &redis_keys, user_id)
                            .await;
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
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
        )
        .bind(email)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::business("管理员不存在"))
    }

    /// Set-based cascade shared by delUser and allDel. Every chunk follows the same table order
    /// and sorted id order, preserving a stable lock acquisition order under concurrent deletes.
    async fn delete_users_cascade(
        &self,
        tx: &mut DbTransaction<'_>,
        user_ids: &[i64],
    ) -> Result<(), ApiError> {
        for user_ids in user_ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut orders = QueryBuilder::<Postgres>::new("DELETE FROM orders WHERE user_id IN (");
            push_id_binds(&mut orders, user_ids);
            orders.push(")");
            orders.build().execute(&mut **tx).await?;

            let mut invites =
                QueryBuilder::<Postgres>::new("DELETE FROM invite_code WHERE user_id IN (");
            push_id_binds(&mut invites, user_ids);
            invites.push(")");
            invites.build().execute(&mut **tx).await?;

            let mut messages = QueryBuilder::<Postgres>::new(
                "DELETE FROM ticket_message tm USING ticket t WHERE t.id = tm.ticket_id AND t.user_id IN (",
            );
            push_id_binds(&mut messages, user_ids);
            messages.push(")");
            messages.build().execute(&mut **tx).await?;

            let mut tickets =
                QueryBuilder::<Postgres>::new("DELETE FROM ticket WHERE user_id IN (");
            push_id_binds(&mut tickets, user_ids);
            tickets.push(")");
            tickets.build().execute(&mut **tx).await?;

            let mut referrals = QueryBuilder::<Postgres>::new(
                "UPDATE users SET invite_user_id = NULL WHERE invite_user_id IN (",
            );
            push_id_binds(&mut referrals, user_ids);
            referrals.push(")");
            referrals.build().execute(&mut **tx).await?;

            let mut users = QueryBuilder::<Postgres>::new("DELETE FROM users WHERE id IN (");
            push_id_binds(&mut users, user_ids);
            users.push(")");
            users.build().execute(&mut **tx).await?;
        }
        Ok(())
    }

    async fn lock_users_for_update(
        tx: &mut DbTransaction<'_>,
        user_ids: &[i64],
    ) -> Result<usize, ApiError> {
        let mut found = 0_usize;
        for user_ids in user_ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Postgres>::new("SELECT id FROM users WHERE id IN (");
            push_id_binds(&mut builder, user_ids);
            builder.push(") ORDER BY id FOR UPDATE");
            found += builder
                .build_query_scalar::<i64>()
                .fetch_all(&mut **tx)
                .await?
                .len();
        }
        Ok(found)
    }

    pub(super) async fn user_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let clauses = self.user_filter_clauses(params).await?;
        let (sort_expr, direction) = user_sort(params);

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM users u WHERE 1 = 1");
        push_user_where(&mut count_builder, &clauses);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(ADMIN_USER_SELECT);
        push_user_where(&mut builder, &clauses);
        // sort_expr and direction are whitelisted by user_sort, so this raw push is safe.
        let nulls = if direction == "ASC" {
            "NULLS FIRST"
        } else {
            "NULLS LAST"
        };
        builder.push(format!(" ORDER BY {sort_expr} {direction} {nulls} LIMIT "));
        builder.push_bind(pagination.limit);
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        let rows = builder
            .build_query_as::<AdminUserRecord>()
            .fetch_all(&self.db)
            .await?;
        let mut data: Vec<Value> = rows.into_iter().map(AdminUserRecord::into_value).collect();
        self.enrich_users(&mut data).await?;
        Ok(AdminOutput::Page { data, total })
    }

    /// Fetches one row of the shared projection by id, optionally restricted
    /// to non-admin/non-staff targets for the staff surface.
    async fn admin_user_record(
        &self,
        id: i64,
        staff_scoped: bool,
    ) -> Result<Option<AdminUserRecord>, ApiError> {
        let mut builder = QueryBuilder::<Postgres>::new(ADMIN_USER_SELECT);
        builder.push(" AND u.id = ");
        builder.push_bind(id);
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        builder.push(" LIMIT 1");
        Ok(builder
            .build_query_as::<AdminUserRecord>()
            .fetch_optional(&self.db)
            .await?)
    }

    pub(super) async fn user_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let row = self
            .admin_user_record(id, false)
            .await?
            .ok_or_else(|| ApiError::business("用户不存在"))?;
        let inviter: Option<(i64, String)> = match row.invite_user_id {
            Some(invite_user_id) => {
                sqlx::query_as("SELECT id, email FROM users WHERE id = $1 LIMIT 1")
                    .bind(invite_user_id)
                    .fetch_optional(&self.db)
                    .await?
            }
            None => None,
        };
        let mut value = row.into_value();
        attach_invite_user(&mut value, inviter);
        Ok(AdminOutput::Data(value))
    }

    pub(super) async fn staff_user_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let row = self
            .admin_user_record(id, true)
            .await?
            .ok_or_else(|| ApiError::business("用户不存在"))?;
        Ok(AdminOutput::Data(row.into_value()))
    }

    pub(super) async fn user_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::update (laravel .../Admin/UserController.php:125-172).
        let id = required_i64(params, "id")?;
        let current_email: String =
            sqlx::query_scalar("SELECT email FROM users WHERE id = $1 LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::business("用户不存在"))?;
        let email = required_string(params, "email")?;
        if email != current_email {
            let taken: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
            )
            .bind(&email)
            .fetch_optional(&self.db)
            .await?;
            if taken.is_some() {
                return Err(ApiError::business("邮箱已被使用"));
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
        let mut group_id = AdminSqlValue::IntegerNull;
        if params.contains_key("plan_id") {
            if let Some(plan_id) = optional_i64(params, "plan_id") {
                let plan_group: Option<i32> =
                    sqlx::query_scalar("SELECT group_id FROM plan WHERE id = $1 LIMIT 1")
                        .bind(plan_id)
                        .fetch_optional(&self.db)
                        .await?
                        .ok_or_else(|| ApiError::business("订阅计划不存在"))?;
                group_id = plan_group
                    .map(|value| AdminSqlValue::Integer(i64::from(value)))
                    .unwrap_or(AdminSqlValue::IntegerNull);
                values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
            } else {
                values.push(("plan_id", AdminSqlValue::IntegerNull));
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
                if let Some(invite_id) = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
                )
                .bind(invite_email)
                .fetch_optional(&self.db)
                .await?
                {
                    values.push(("invite_user_id", AdminSqlValue::Integer(invite_id)));
                }
            }
            None => values.push(("invite_user_id", AdminSqlValue::IntegerNull)),
        }

        let password_changed = params
            .get("password")
            .is_some_and(|value| !value.is_empty());
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = self.password_kdf.hash(password).await?;
            values.push(("password", AdminSqlValue::Text(hash)));
            values.push(("password_algo", AdminSqlValue::TextNull));
        }

        // Any privilege assignment invalidates sessions issued under the old
        // role. A newly promoted staff/admin user must not keep a 30-day user
        // session, and a demoted account must lose privileged step-up tokens.
        let role_changed = params.contains_key("is_admin") || params.contains_key("is_staff");
        let revokes_sessions =
            password_changed || optional_i64(params, "banned") == Some(1) || role_changed;
        let resets_traffic = params.contains_key("u") || params.contains_key("d");

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE users SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
        }
        if revokes_sessions {
            builder.push(", \"session_epoch\" = \"session_epoch\" + 1");
        }
        if resets_traffic {
            builder.push(", \"traffic_epoch\" = \"traffic_epoch\" + 1");
        }
        builder.push(", \"updated_at\" = ");
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
            "SELECT email FROM users WHERE id = $1 AND is_admin = 0 AND is_staff = 0 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::business("用户不存在"))?;
        let email = required_string(params, "email")?;
        if email != current_email {
            let taken: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
            )
            .bind(&email)
            .fetch_optional(&self.db)
            .await?;
            if taken.is_some() {
                return Err(ApiError::business("邮箱已被使用"));
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
                let plan_group: Option<i32> =
                    sqlx::query_scalar("SELECT group_id FROM plan WHERE id = $1 LIMIT 1")
                        .bind(plan_id)
                        .fetch_optional(&self.db)
                        .await?
                        .ok_or_else(|| ApiError::business("订阅计划不存在"))?;
                values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
                values.push((
                    "group_id",
                    plan_group
                        .map(|value| AdminSqlValue::Integer(i64::from(value)))
                        .unwrap_or(AdminSqlValue::IntegerNull),
                ));
            } else {
                values.push(("plan_id", AdminSqlValue::IntegerNull));
            }
        }
        let password_changed = params
            .get("password")
            .is_some_and(|value| !value.is_empty());
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = self.password_kdf.hash(password).await?;
            values.push(("password", AdminSqlValue::Text(hash)));
            values.push(("password_algo", AdminSqlValue::TextNull));
        }
        let revokes_sessions = password_changed || optional_i64(params, "banned") == Some(1);
        let resets_traffic = params.contains_key("u") || params.contains_key("d");

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE users SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
        }
        if revokes_sessions {
            builder.push(", \"session_epoch\" = \"session_epoch\" + 1");
        }
        if resets_traffic {
            builder.push(", \"traffic_epoch\" = \"traffic_epoch\" + 1");
        }
        builder.push(", \"updated_at\" = ");
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
        let clauses = self.user_filter_clauses(params).await?;
        let ids = self.filtered_user_ids_bounded(&clauses, true).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut tx = self.db.begin().await?;
        for ids in ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Postgres>::new(
                "UPDATE users SET banned = 1, session_epoch = session_epoch + 1, updated_at = ",
            );
            builder.push_bind(Utc::now().timestamp());
            builder.push(" WHERE id IN (");
            push_id_binds(&mut builder, ids);
            builder.push(")");
            builder.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.remove_user_sessions_bounded(&ids).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn user_reset_secret(&self, id: i64) -> Result<AdminOutput, ApiError> {
        sqlx::query("UPDATE users SET token = $1, uuid = $2, updated_at = $3 WHERE id = $4")
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
            "SELECT id::BIGINT, group_id::BIGINT, transfer_enable, device_limit::BIGINT \
             FROM plan WHERE id = $1::BIGINT LIMIT 1",
        )
        .bind(plan_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::business("订阅计划不存在"))?;
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
        // These values originate from INTEGER columns, but `generate_plan`
        // exposes i64 for legacy arithmetic. Convert back to exact PostgreSQL
        // bind types before INSERT (including QueryBuilder's batched path).
        let plan_id_db = plan_id
            .map(i32::try_from)
            .transpose()
            .map_err(|_| ApiError::internal("stored plan id exceeds PostgreSQL INTEGER"))?;
        let group_id_db = group_id
            .map(i32::try_from)
            .transpose()
            .map_err(|_| ApiError::internal("stored group id exceeds PostgreSQL INTEGER"))?;
        let device_limit_db = device_limit
            .map(i32::try_from)
            .transpose()
            .map_err(|_| ApiError::internal("stored device limit exceeds PostgreSQL INTEGER"))?;

        // Single generation returns JSON; the CSV path is multiGenerate only.
        if let Some(prefix) = optional_string(params, "email_prefix") {
            let suffix = required_string(params, "email_suffix")?;
            let email = format!("{prefix}@{suffix}");
            let exists: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
            )
            .bind(&email)
            .fetch_optional(&self.db)
            .await?;
            if exists.is_some() {
                return Err(ApiError::business("邮箱已存在于系统中"));
            }
            let password_plain = params
                .get("password")
                .filter(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| email.clone());
            let hash = self.password_kdf.hash(&password_plain).await?;
            sqlx::query(
                r#"
                INSERT INTO users (
                    email, plan_id, group_id, transfer_enable, device_limit, expired_at,
                    uuid, token, password, password_algo, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)
                "#,
            )
            .bind(&email)
            .bind(plan_id_db)
            .bind(group_id_db)
            .bind(transfer_enable)
            .bind(device_limit_db)
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
        if count > GENERATED_USER_MAX_ROWS {
            return Err(ApiError::validation_field(
                "generate_count",
                "单次最多生成 1000 个用户",
            ));
        }
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
        let mut insert = QueryBuilder::<Postgres>::new(
            r#"
            INSERT INTO users (
                email, plan_id, group_id, transfer_enable, device_limit, expired_at,
                uuid, token, password, password_algo, created_at, updated_at
            )
            "#,
        );
        insert.push_values(&prepared, |mut row, (_, email, _, uuid, token, hash)| {
            row.push_bind(email)
                .push_bind(plan_id_db)
                .push_bind(group_id_db)
                .push_bind(transfer_enable)
                .push_bind(device_limit_db)
                .push_bind(expired_at)
                .push_bind(uuid)
                .push_bind(token)
                .push_bind(hash)
                .push_bind(Option::<String>::None)
                .push_bind(now)
                .push_bind(now);
        });
        // The method-2 subscribe token embeds the user id, so recover the
        // generated ids alongside the batch-unique random tokens.
        insert.push(" RETURNING id, token");
        let inserted: Vec<(i64, String)> = insert
            .build_query_as::<(i64, String)>()
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        let token_ids = inserted
            .into_iter()
            .map(|(id, token)| (token, id))
            .collect::<HashMap<_, _>>();

        let create_date = local_datetime(now);
        let expire = expired_at
            .map(local_datetime)
            .unwrap_or_else(|| "长期有效".to_string());
        let mut mint_conn = self.subscribe_mint_connection().await?;
        let mut rows = Vec::with_capacity(prepared.len());
        for (_, email, password_plain, uuid, token, _) in prepared {
            let user_id = token_ids.get(&token).copied().ok_or_else(|| {
                ApiError::internal("generated user row is missing its inserted id")
            })?;
            let url = crate::subscribe_link::subscribe_url_for_user(
                &self.config,
                &self.redis_keys,
                &mut mint_conn,
                user_id,
                &token,
            )
            .await?;
            rows.push(vec![
                email,
                password_plain,
                expire.clone(),
                uuid,
                create_date.clone(),
                url,
            ]);
        }
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
        let mut csv = CsvExportWriter::new(
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
            true,
        )?;
        let mut after_id = 0_i64;
        let mut exported = 0_usize;
        let mut mint_conn = self.subscribe_mint_connection().await?;
        loop {
            let mut builder = QueryBuilder::<Postgres>::new(
                "SELECT u.id AS id, u.email AS email, u.balance AS balance, \
                 u.commission_balance AS commission_balance, u.transfer_enable AS transfer_enable, \
                 u.u AS u, u.d AS d, u.device_limit AS device_limit, u.expired_at AS expired_at, \
                 p.name AS plan_name, u.token AS token \
                 FROM users u LEFT JOIN plan p ON p.id = u.plan_id WHERE 1 = 1",
            );
            push_user_where(&mut builder, &clauses);
            builder.push(" AND u.id > ");
            builder.push_bind(after_id);
            builder.push(" ORDER BY u.id ASC LIMIT ");
            builder.push_bind(USER_CSV_PAGE_SIZE);
            let rows = builder
                .build_query_as::<UserDumpRow>()
                .fetch_all(&self.db)
                .await?;
            let Some(last_id) = rows.last().map(|row| row.id) else {
                break;
            };
            exported = exported
                .checked_add(rows.len())
                .ok_or_else(|| ApiError::business("导出用户数量超出支持范围，请缩小筛选范围"))?;
            if exported > USER_CSV_MAX_ROWS {
                return Err(ApiError::business(
                    "单次最多导出 50000 个用户，请缩小筛选范围",
                ));
            }
            for row in rows {
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
                let url = crate::subscribe_link::subscribe_url_for_user(
                    &self.config,
                    &self.redis_keys,
                    &mut mint_conn,
                    row.id,
                    &row.token,
                )
                .await?;
                csv.write_row(vec![
                    row.email,
                    balance.to_string(),
                    commission.to_string(),
                    transfer.to_string(),
                    device,
                    not_use.to_string(),
                    expire,
                    plan,
                    url,
                ])?;
            }
            after_id = last_id;
        }
        let body = csv.finish()?;
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
            return Err(ApiError::business("Invalid user flag"));
        }
        let clauses = self.user_filter_clauses(params).await?;
        let ids = self.filtered_user_ids_bounded(&clauses, false).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut tx = self.db.begin().await?;
        for ids in ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Postgres>::new("UPDATE users SET banned = CAST(");
            builder.push_bind(value);
            builder.push("::BIGINT AS SMALLINT), session_epoch = session_epoch + 1");
            builder.push(", updated_at = ");
            builder.push_bind(Utc::now().timestamp());
            builder.push(" WHERE id IN (");
            push_id_binds(&mut builder, ids);
            builder.push(")");
            builder.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
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
        let clauses = self.user_filter_clauses(params).await?;
        let ids = sorted_unique_user_ids(self.filtered_user_ids_bounded(&clauses, false).await?);
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut tx = self.db.begin().await?;
        if Self::lock_user_orders_and_find_pending_stripe(&mut tx, &ids).await? {
            return Err(ApiError::business(
                "所选用户仍有待支付的 Stripe 订单，请先取消订单",
            ));
        }
        Self::lock_users_for_update(&mut tx, &ids).await?;
        self.delete_users_cascade(&mut tx, &ids).await?;
        tx.commit().await?;
        self.remove_user_sessions_bounded(&ids).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn del_user(&self, id: i64) -> Result<AdminOutput, ApiError> {
        // Ports UserController::delUser (:361-391): single-user cascade delete.
        let mut tx = self.db.begin().await?;
        if Self::lock_user_orders_and_find_pending_stripe(&mut tx, &[id]).await? {
            return Err(ApiError::business(
                "该用户仍有待支付的 Stripe 订单，请先取消订单",
            ));
        }
        if Self::lock_users_for_update(&mut tx, &[id]).await? != 1 {
            return Err(ApiError::business("用户不存在"));
        }
        self.delete_users_cascade(&mut tx, &[id]).await?;
        tx.commit().await?;
        self.remove_user_sessions_bounded(&[id]).await;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn lock_user_orders_and_find_pending_stripe(
        tx: &mut DbTransaction<'_>,
        user_ids: &[i64],
    ) -> Result<bool, ApiError> {
        if user_ids.is_empty() {
            return Ok(false);
        }
        for user_ids in user_ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut after_order_id = 0_i64;
            loop {
                let mut builder = QueryBuilder::<Postgres>::new(
                    "SELECT id, status, callback_no FROM orders WHERE user_id IN (",
                );
                push_id_binds(&mut builder, user_ids);
                builder.push(") AND id > ");
                builder.push_bind(after_order_id);
                builder.push(" ORDER BY id LIMIT ");
                builder.push_bind(USER_MUTATION_PAGE_SIZE);
                builder.push(" FOR UPDATE");
                let rows = builder
                    .build_query_as::<(i64, i16, Option<String>)>()
                    .fetch_all(&mut **tx)
                    .await?;
                let Some(last_id) = rows.last().map(|(id, _, _)| *id) else {
                    break;
                };
                if rows.iter().any(|(_, status, callback_no)| {
                    *status == 0
                        && callback_no
                            .as_deref()
                            .is_some_and(|callback_no| callback_no.starts_with("pi_"))
                }) {
                    return Ok(true);
                }
                after_order_id = last_id;
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
        sqlx::query("UPDATE users SET invite_user_id = $1, updated_at = $2 WHERE id = $3")
            .bind(invite_user_id)
            .bind(Utc::now().timestamp())
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }
}

fn push_id_binds(builder: &mut QueryBuilder<Postgres>, user_ids: &[i64]) {
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

    fn sample_admin_user_record() -> AdminUserRecord {
        AdminUserRecord {
            id: 7,
            email: "admin-user@example.test".to_string(),
            balance: 1200,
            commission_balance: 340,
            transfer_enable: 107_374_182_400,
            device_limit: Some(3),
            u: 1_073_741_824,
            d: 2_147_483_648,
            plan_id: Some(2),
            plan_name: Some("Pro".to_string()),
            group_id: Some(1),
            expired_at: Some(1_893_456_000),
            uuid: "uuid-7".to_string(),
            token: "token-7".to_string(),
            banned: 0,
            is_admin: 0,
            is_staff: 0,
            invite_user_id: Some(1),
            discount: None,
            commission_type: 0,
            commission_rate: None,
            t: 1_700_000_000,
            speed_limit: None,
            auto_renewal: Some(0),
            remind_expire: Some(1),
            remind_traffic: Some(1),
            remarks: None,
            telegram_id: None,
            last_login_at: Some(1_700_000_000),
            created_at: 1_690_000_000,
            updated_at: 1_700_000_000,
        }
    }

    #[test]
    fn admin_user_projection_serializes_the_exact_contract_key_set() {
        let value = sample_admin_user_record().into_value();
        let object = value.as_object().unwrap();
        let keys: Vec<&str> = object.keys().map(String::as_str).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            vec![
                "alive_ip",
                "auto_renewal",
                "balance",
                "banned",
                "commission_balance",
                "commission_rate",
                "commission_type",
                "created_at",
                "d",
                "device_limit",
                "discount",
                "email",
                "expired_at",
                "group_id",
                "id",
                "invite_user_id",
                "ips",
                "is_admin",
                "is_staff",
                "last_login_at",
                "password",
                "plan_id",
                "plan_name",
                "remarks",
                "remind_expire",
                "remind_traffic",
                "speed_limit",
                "subscribe_url",
                "t",
                "telegram_id",
                "token",
                "total_used",
                "transfer_enable",
                "u",
                "updated_at",
                "uuid",
            ]
        );
        // Credential-verification columns never leave the database.
        for leaked in ["password_algo", "password_salt", "last_login_ip"] {
            assert!(!object.contains_key(leaked), "leaked key: {leaked}");
        }
        assert_eq!(value["password"], json!(""));
        assert_eq!(value["subscribe_url"], json!(""));
        assert_eq!(value["alive_ip"], json!(0));
        assert_eq!(value["ips"], json!(""));
        assert_eq!(value["total_used"], json!(3_221_225_472_u64));
    }

    #[test]
    fn invite_user_is_attached_only_when_the_inviter_exists() {
        let mut absent = sample_admin_user_record().into_value();
        attach_invite_user(&mut absent, None);
        assert!(absent.get("invite_user").is_none());

        let mut present = sample_admin_user_record().into_value();
        attach_invite_user(&mut present, Some((1, "inviter@example.test".to_string())));
        assert_eq!(
            present["invite_user"],
            json!({ "id": 1, "email": "inviter@example.test" })
        );
    }

    #[test]
    fn shared_user_projection_is_the_only_admin_user_select() {
        let source = include_str!("users.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        // The three read paths must not regrow private per-endpoint projections.
        assert_eq!(production.matches("jsonb_build_object").count(), 0);
        assert_eq!(production.matches("ADMIN_USER_SELECT").count(), 3);
    }

    #[test]
    fn user_deletion_is_set_based_and_cache_cleanup_is_post_commit() {
        let source = include_str!("users.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for sql in [
            "DELETE FROM orders WHERE user_id IN (",
            "DELETE FROM invite_code WHERE user_id IN (",
            "t.user_id IN (",
            "DELETE FROM ticket WHERE user_id IN (",
            "WHERE invite_user_id IN (",
            "DELETE FROM users WHERE id IN (",
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
