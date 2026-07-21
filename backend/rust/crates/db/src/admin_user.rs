use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction, types::Json};
use v2board_application::{
    RepositoryError,
    admin_user::{
        AdminInviter, AdminUser, AdminUserChanges, AdminUserDetailRecord, AdminUserListRequest,
        AdminUserRepository, BanUsersOutcome, CreateUsersCommand, CreateUsersOutcome,
        CreatedAccount, DeleteUsersOutcome, RepositoryResult, RepositoryUserPage,
        SetInviterOutcome, StaffUserChanges, UserExportPage, UserExportRow, UserFilterClause,
        UserFilterField, UserFilterOperator, UserFilterValue, UserSecret, UserSortField,
        UserUpdateOutcome,
    },
};

const USER_DELETE_SQL_BATCH_SIZE: usize = 500;

const ADMIN_USER_SELECT: &str = "\
    SELECT u.id, u.email, u.balance, u.commission_balance, u.transfer_enable, \
           u.device_limit, u.u, u.d, u.plan_id, p.name AS plan_name, u.group_id, \
           u.expired_at, u.uuid, u.token, u.banned, u.is_admin, u.is_staff, \
           u.admin_permissions, u.invite_user_id, u.discount, u.commission_type, \
           u.commission_rate, u.speed_limit, u.auto_renewal, u.remind_expire, \
           u.remind_traffic, u.remarks, u.telegram_id, u.last_login_at, \
           u.created_at, u.updated_at \
    FROM users u LEFT JOIN plan p ON p.id = u.plan_id WHERE 1 = 1";

#[derive(Clone)]
pub struct PostgresAdminUserRepository {
    pool: PgPool,
}

impl PostgresAdminUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct AdminUserRow {
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
    admin_permissions: Json<Vec<String>>,
    invite_user_id: Option<i64>,
    discount: Option<i32>,
    commission_type: i16,
    commission_rate: Option<i32>,
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

impl From<AdminUserRow> for AdminUser {
    fn from(row: AdminUserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            balance: row.balance,
            commission_balance: row.commission_balance,
            transfer_enable: row.transfer_enable,
            device_limit: row.device_limit,
            uploaded: row.u,
            downloaded: row.d,
            plan_id: row.plan_id,
            plan_name: row.plan_name,
            group_id: row.group_id,
            expired_at: row.expired_at,
            uuid: row.uuid,
            token: row.token,
            banned: row.banned != 0,
            is_admin: row.is_admin != 0,
            is_staff: row.is_staff != 0,
            admin_permissions: row.admin_permissions.0,
            invite_user_id: row.invite_user_id,
            discount: row.discount,
            commission_type: row.commission_type,
            commission_rate: row.commission_rate,
            speed_limit: row.speed_limit,
            auto_renewal: row.auto_renewal.map(|value| value != 0),
            remind_expire: row.remind_expire.map(|value| value != 0),
            remind_traffic: row.remind_traffic.map(|value| value != 0),
            remarks: row.remarks,
            telegram_id: row.telegram_id,
            last_login_at: row.last_login_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(FromRow)]
struct UserExportDbRow {
    id: i64,
    email: String,
    balance: i32,
    commission_balance: i32,
    transfer_enable: i64,
    u: i64,
    d: i64,
    device_limit: Option<i32>,
    expired_at: Option<i64>,
    plan_name: Option<String>,
    token: String,
}

impl From<UserExportDbRow> for UserExportRow {
    fn from(row: UserExportDbRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            balance: row.balance,
            commission_balance: row.commission_balance,
            transfer_enable: row.transfer_enable,
            uploaded: row.u,
            downloaded: row.d,
            device_limit: row.device_limit,
            expired_at: row.expired_at,
            plan_name: row.plan_name,
            token: row.token,
        }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

const fn filter_expression(field: UserFilterField) -> &'static str {
    match field {
        UserFilterField::Id => "u.id",
        UserFilterField::Email => "u.email",
        UserFilterField::TelegramId => "u.telegram_id",
        UserFilterField::Balance => "u.balance",
        UserFilterField::Discount => "u.discount",
        UserFilterField::CommissionType => "u.commission_type",
        UserFilterField::CommissionRate => "u.commission_rate",
        UserFilterField::CommissionBalance => "u.commission_balance",
        UserFilterField::LastTrafficResetAt => "u.t",
        UserFilterField::Uploaded => "u.u",
        UserFilterField::Downloaded => "u.d",
        UserFilterField::TransferEnable => "u.transfer_enable",
        UserFilterField::DeviceLimit => "u.device_limit",
        UserFilterField::Banned => "(u.banned <> 0)",
        UserFilterField::IsAdmin => "(u.is_admin <> 0)",
        UserFilterField::IsStaff => "(u.is_staff <> 0)",
        UserFilterField::LastLoginAt => "u.last_login_at",
        UserFilterField::Uuid => "u.uuid",
        UserFilterField::GroupId => "u.group_id",
        UserFilterField::PlanId => "u.plan_id",
        UserFilterField::SpeedLimit => "u.speed_limit",
        UserFilterField::Token => "u.token",
        UserFilterField::ExpiredAt => "u.expired_at",
        UserFilterField::Remarks => "u.remarks",
        UserFilterField::InviteUserId => "u.invite_user_id",
        UserFilterField::CreatedAt => "u.created_at",
        UserFilterField::UpdatedAt => "u.updated_at",
    }
}

const fn sort_expression(field: UserSortField) -> &'static str {
    match field {
        UserSortField::Field(field) => match field {
            UserFilterField::Banned => "u.banned",
            UserFilterField::IsAdmin => "u.is_admin",
            UserFilterField::IsStaff => "u.is_staff",
            _ => filter_expression(field),
        },
        UserSortField::TotalUsed => "(CAST(u.u AS NUMERIC(65,0)) + CAST(u.d AS NUMERIC(65,0)))",
    }
}

fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('%');
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('%');
    escaped
}

/// Appends a validated closed user-filter set to a PostgreSQL query.
/// Column expressions are code-owned and every request value is bound.
pub fn push_user_filters(builder: &mut QueryBuilder<Postgres>, filters: &[UserFilterClause]) {
    for filter in filters {
        let expression = filter_expression(filter.field);
        builder.push(" AND ");
        match (filter.operator, &filter.value) {
            (UserFilterOperator::Eq, UserFilterValue::Null) => {
                builder.push(expression).push(" IS NULL");
            }
            (UserFilterOperator::Neq, UserFilterValue::Null) => {
                builder.push(expression).push(" IS NOT NULL");
            }
            (operator @ (UserFilterOperator::Eq | UserFilterOperator::Neq), value) => {
                let comparison = if operator == UserFilterOperator::Eq {
                    " = "
                } else {
                    " <> "
                };
                if filter.field == UserFilterField::Email {
                    builder.push("lower(btrim(").push(expression).push("))");
                    builder.push(comparison).push("lower(btrim(");
                    if let UserFilterValue::Text(value) = value {
                        builder.push_bind(value.clone());
                    }
                    builder.push("))");
                } else {
                    builder.push(expression).push(comparison);
                    push_scalar_bind(builder, value);
                }
            }
            (UserFilterOperator::Like, UserFilterValue::Text(value)) => {
                builder.push(expression);
                if matches!(
                    filter.field.kind(),
                    v2board_application::admin_user::UserColumnKind::Integer
                ) {
                    builder.push("::text");
                }
                builder.push(" ILIKE ").push_bind(escape_like(value));
            }
            (operator, UserFilterValue::Integer(value))
                if matches!(
                    operator,
                    UserFilterOperator::Gt
                        | UserFilterOperator::Gte
                        | UserFilterOperator::Lt
                        | UserFilterOperator::Lte
                ) =>
            {
                builder.push(expression).push(match operator {
                    UserFilterOperator::Gt => " > ",
                    UserFilterOperator::Gte => " >= ",
                    UserFilterOperator::Lt => " < ",
                    UserFilterOperator::Lte => " <= ",
                    _ => unreachable!(),
                });
                builder.push_bind(*value);
            }
            (UserFilterOperator::In, UserFilterValue::Integers(values)) => {
                builder
                    .push(expression)
                    .push(" = ANY(")
                    .push_bind(values.clone())
                    .push(")");
            }
            (UserFilterOperator::In, UserFilterValue::Booleans(values)) => {
                builder
                    .push(expression)
                    .push(" = ANY(")
                    .push_bind(values.clone())
                    .push(")");
            }
            (UserFilterOperator::In, UserFilterValue::Texts(values)) => {
                if filter.field == UserFilterField::Email {
                    let values = values
                        .iter()
                        .map(|value| value.trim().to_lowercase())
                        .collect::<Vec<_>>();
                    builder
                        .push("lower(btrim(")
                        .push(expression)
                        .push(")) = ANY(")
                        .push_bind(values)
                        .push(")");
                } else {
                    builder
                        .push(expression)
                        .push(" = ANY(")
                        .push_bind(values.clone())
                        .push(")");
                }
            }
            _ => unreachable!("application validation rejects invalid admin-user filters"),
        }
    }
}

fn push_scalar_bind(builder: &mut QueryBuilder<Postgres>, value: &UserFilterValue) {
    match value {
        UserFilterValue::Boolean(value) => {
            builder.push_bind(*value);
        }
        UserFilterValue::Integer(value) => {
            builder.push_bind(*value);
        }
        UserFilterValue::Text(value) => {
            builder.push_bind(value.clone());
        }
        _ => unreachable!("application validation guarantees scalar values"),
    }
}

fn unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == "23505")
}

fn plan_bytes(gibibytes: i64) -> Option<i64> {
    gibibytes.checked_mul(1_073_741_824)
}

impl AdminUserRepository for PostgresAdminUserRepository {
    async fn list(&self, request: &AdminUserListRequest) -> RepositoryResult<RepositoryUserPage> {
        let mut count = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM users u WHERE 1 = 1");
        push_user_filters(&mut count, &request.filters);
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count admin users", error))?;

        let mut query = QueryBuilder::<Postgres>::new(ADMIN_USER_SELECT);
        push_user_filters(&mut query, &request.filters);
        query
            .push(" ORDER BY ")
            .push(sort_expression(request.sort.field));
        if request.sort.descending {
            query.push(" DESC NULLS LAST");
        } else {
            query.push(" ASC NULLS FIRST");
        }
        query
            .push(", u.id DESC LIMIT ")
            .push_bind(request.limit)
            .push(" OFFSET ")
            .push_bind(request.offset);
        let items = query
            .build_query_as::<AdminUserRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("list admin users", error))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(RepositoryUserPage { items, total })
    }

    async fn detail(
        &self,
        user_id: i64,
        staff_scoped: bool,
    ) -> RepositoryResult<Option<AdminUserDetailRecord>> {
        let mut query = QueryBuilder::<Postgres>::new(ADMIN_USER_SELECT);
        query.push(" AND u.id = ").push_bind(user_id);
        if staff_scoped {
            query.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        query.push(" LIMIT 1");
        let Some(row) = query
            .build_query_as::<AdminUserRow>()
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| repository_error("find admin user", error))?
        else {
            return Ok(None);
        };
        let inviter = match row.invite_user_id {
            Some(inviter_id) if !staff_scoped => sqlx::query_as::<_, (i64, String)>(
                "SELECT id, email FROM users WHERE id = $1 LIMIT 1",
            )
            .bind(inviter_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| repository_error("find admin user inviter", error))?
            .map(|(id, email)| AdminInviter { id, email }),
            _ => None,
        };
        Ok(Some(AdminUserDetailRecord {
            user: row.into(),
            inviter,
        }))
    }

    async fn update_admin(
        &self,
        user_id: i64,
        changes: AdminUserChanges,
    ) -> RepositoryResult<UserUpdateOutcome> {
        update_admin_user(&self.pool, user_id, changes).await
    }

    async fn update_staff(
        &self,
        user_id: i64,
        changes: StaffUserChanges,
    ) -> RepositoryResult<UserUpdateOutcome> {
        update_staff_user(&self.pool, user_id, changes).await
    }

    async fn create_users(
        &self,
        command: CreateUsersCommand,
    ) -> RepositoryResult<CreateUsersOutcome> {
        if command.accounts.is_empty() {
            return Ok(CreateUsersOutcome::Created(Vec::new()));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin admin user create", error))?;
        let (plan_id, group_id, transfer_enable, device_limit) = match command.plan_id {
            Some(plan_id) => {
                let Some(plan) = crate::plan::find_plan_binding_for_share(&mut tx, plan_id)
                    .await
                    .map_err(|error| repository_error("lock generated-user plan", error))?
                else {
                    return Ok(CreateUsersOutcome::PlanUnavailable);
                };
                let Some(transfer_enable) = plan_bytes(plan.transfer_enable) else {
                    return Ok(CreateUsersOutcome::PlanUnavailable);
                };
                (
                    Some(plan.id),
                    Some(plan.group_id),
                    transfer_enable,
                    plan.device_limit,
                )
            }
            None => (None, None, 0, None),
        };
        let expired_at = command.expired_at;
        let created_at = command.created_at;
        let mut insert = QueryBuilder::<Postgres>::new(
            "INSERT INTO users (email, plan_id, group_id, transfer_enable, device_limit, \
             expired_at, uuid, token, password, password_algo, created_at, updated_at) ",
        );
        insert.push_values(&command.accounts, |mut row, account| {
            row.push_bind(account.email.clone())
                .push_bind(plan_id)
                .push_bind(group_id)
                .push_bind(transfer_enable)
                .push_bind(device_limit)
                .push_bind(expired_at)
                .push_bind(account.uuid.clone())
                .push_bind(account.token.clone())
                .push_bind(account.password_hash.clone())
                .push_bind(Option::<String>::None)
                .push_bind(created_at)
                .push_bind(created_at);
        });
        insert.push(" RETURNING id, token");
        let inserted = match insert
            .build_query_as::<(i64, String)>()
            .fetch_all(&mut *tx)
            .await
        {
            Ok(inserted) => inserted,
            Err(error) if unique_violation(&error) => {
                return Ok(CreateUsersOutcome::EmailAlreadyRegistered);
            }
            Err(error) => return Err(repository_error("insert admin users", error)),
        };
        tx.commit()
            .await
            .map_err(|error| repository_error("commit admin user create", error))?;
        Ok(CreateUsersOutcome::Created(
            inserted
                .into_iter()
                .map(|(id, token)| CreatedAccount { id, token })
                .collect(),
        ))
    }

    async fn reset_secret(
        &self,
        user_id: i64,
        secret: UserSecret,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        let result =
            sqlx::query("UPDATE users SET token = $1, uuid = $2, updated_at = $3 WHERE id = $4")
                .bind(secret.token)
                .bind(secret.uuid)
                .bind(updated_at)
                .bind(user_id)
                .execute(&self.pool)
                .await
                .map_err(|error| repository_error("reset admin user secret", error))?;
        Ok(result.rows_affected() == 1)
    }

    async fn set_inviter(
        &self,
        user_id: i64,
        inviter_email: Option<&str>,
        updated_at: i64,
    ) -> RepositoryResult<SetInviterOutcome> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin set user inviter", error))?;
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| repository_error("lock inviter target", error))?;
        if exists.is_none() {
            return Ok(SetInviterOutcome::UserNotFound);
        }
        let inviter_id = match inviter_email {
            Some(email) => {
                let inviter_id = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
                )
                .bind(email)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| repository_error("resolve user inviter", error))?;
                let Some(inviter_id) = inviter_id else {
                    return Ok(SetInviterOutcome::InviterNotFound);
                };
                Some(inviter_id)
            }
            None => None,
        };
        sqlx::query("UPDATE users SET invite_user_id = $1, updated_at = $2 WHERE id = $3")
            .bind(inviter_id)
            .bind(updated_at)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| repository_error("set user inviter", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("commit set user inviter", error))?;
        Ok(SetInviterOutcome::Updated)
    }

    async fn export_page(
        &self,
        filters: &[UserFilterClause],
        after_id: i64,
        limit: i64,
    ) -> RepositoryResult<UserExportPage> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT u.id, u.email, u.balance, u.commission_balance, u.transfer_enable, \
             u.u, u.d, u.device_limit, u.expired_at, p.name AS plan_name, u.token \
             FROM users u LEFT JOIN plan p ON p.id = u.plan_id WHERE 1 = 1",
        );
        push_user_filters(&mut query, filters);
        query
            .push(" AND u.id > ")
            .push_bind(after_id)
            .push(" ORDER BY u.id ASC LIMIT ")
            .push_bind(limit);
        let items = query
            .build_query_as::<UserExportDbRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("export admin users page", error))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(UserExportPage { items })
    }

    async fn ban_users(
        &self,
        filters: &[UserFilterClause],
        staff_scoped: bool,
        maximum: usize,
        updated_at: i64,
    ) -> RepositoryResult<BanUsersOutcome> {
        let ids = filtered_user_ids(&self.pool, filters, staff_scoped, maximum).await?;
        let Some(ids) = ids else {
            return Ok(BanUsersOutcome::TooMany);
        };
        if ids.is_empty() {
            return Ok(BanUsersOutcome::Banned(ids));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin bulk user ban", error))?;
        for chunk in ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
            let mut update = QueryBuilder::<Postgres>::new(
                "UPDATE users SET banned = 1, session_epoch = session_epoch + 1, updated_at = ",
            );
            update.push_bind(updated_at).push(" WHERE id IN (");
            push_id_binds(&mut update, chunk);
            update.push(")");
            update
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|error| repository_error("bulk ban admin users", error))?;
        }
        tx.commit()
            .await
            .map_err(|error| repository_error("commit bulk user ban", error))?;
        Ok(BanUsersOutcome::Banned(ids))
    }

    async fn delete_users(
        &self,
        filters: &[UserFilterClause],
        maximum: usize,
    ) -> RepositoryResult<DeleteUsersOutcome> {
        let Some(mut ids) = filtered_user_ids(&self.pool, filters, false, maximum).await? else {
            return Ok(DeleteUsersOutcome::TooMany);
        };
        ids.sort_unstable();
        ids.dedup();
        delete_user_ids(&self.pool, ids, false).await
    }

    async fn delete_user(&self, user_id: i64) -> RepositoryResult<DeleteUsersOutcome> {
        delete_user_ids(&self.pool, vec![user_id], true).await
    }
}

async fn update_admin_user(
    pool: &PgPool,
    user_id: i64,
    changes: AdminUserChanges,
) -> RepositoryResult<UserUpdateOutcome> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| repository_error("begin admin user update", error))?;
    let Some(current_email) =
        sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1 LIMIT 1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| repository_error("lock admin user", error))?
    else {
        return Ok(UserUpdateOutcome::UserNotFound);
    };
    if let Some(email) = &changes.email
        && email != &current_email
    {
        let taken = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) AND id <> $2 LIMIT 1",
        )
        .bind(email)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("check admin user email", error))?;
        if taken.is_some() {
            return Ok(UserUpdateOutcome::EmailAlreadyRegistered);
        }
    }
    let plan_binding = match changes.plan_id {
        Some(Some(plan_id)) => {
            let Some(plan) = crate::plan::find_plan_binding_for_share(&mut tx, plan_id)
                .await
                .map_err(|error| repository_error("lock admin user plan", error))?
            else {
                return Ok(UserUpdateOutcome::PlanNotFound);
            };
            Some((Some(plan.id), Some(plan.group_id)))
        }
        Some(None) => Some((None, None)),
        None => None,
    };

    let mut update = QueryBuilder::<Postgres>::new("UPDATE users SET updated_at = ");
    update.push_bind(changes.updated_at);
    if let Some(email) = changes.email {
        update.push(", email = ").push_bind(email);
    }
    if let Some(password_hash) = changes.password_hash {
        update
            .push(", password = ")
            .push_bind(password_hash)
            .push(", password_algo = NULL");
    }
    if let Some(value) = changes.transfer_enable {
        update.push(", transfer_enable = ").push_bind(value);
    }
    if let Some(value) = changes.uploaded {
        update.push(", u = ").push_bind(value);
    }
    if let Some(value) = changes.downloaded {
        update.push(", d = ").push_bind(value);
    }
    if let Some(value) = changes.balance {
        update.push(", balance = ").push_bind(value);
    }
    if let Some(value) = changes.commission_balance {
        update.push(", commission_balance = ").push_bind(value);
    }
    if let Some(value) = changes.commission_type {
        update.push(", commission_type = ").push_bind(value);
    }
    if let Some(value) = changes.banned {
        update.push(", banned = ").push_bind(i16::from(value));
    }
    if let Some(value) = changes.is_admin {
        update.push(", is_admin = ").push_bind(i16::from(value));
    }
    if let Some(value) = changes.is_staff {
        update.push(", is_staff = ").push_bind(i16::from(value));
    }
    if let Some(value) = changes.admin_permissions {
        update.push(", admin_permissions = ").push_bind(Json(value));
    }
    push_nullable_i32(&mut update, "device_limit", changes.device_limit);
    push_nullable_i32(&mut update, "commission_rate", changes.commission_rate);
    push_nullable_i32(&mut update, "discount", changes.discount);
    push_nullable_i32(&mut update, "speed_limit", changes.speed_limit);
    push_nullable_i64(&mut update, "expired_at", changes.expired_at);
    if let Some(value) = changes.remarks {
        update.push(", remarks = ").push_bind(value);
    }
    if let Some((plan_id, group_id)) = plan_binding {
        update
            .push(", plan_id = ")
            .push_bind(plan_id)
            .push(", group_id = ")
            .push_bind(group_id);
    }
    if changes.revoke_sessions {
        update.push(", session_epoch = session_epoch + 1");
    }
    if changes.reset_traffic_epoch {
        update.push(", traffic_epoch = traffic_epoch + 1");
    }
    update.push(" WHERE id = ").push_bind(user_id);
    if let Err(error) = update.build().execute(&mut *tx).await {
        if unique_violation(&error) {
            return Ok(UserUpdateOutcome::EmailAlreadyRegistered);
        }
        return Err(repository_error("update admin user", error));
    }
    tx.commit()
        .await
        .map_err(|error| repository_error("commit admin user update", error))?;
    Ok(UserUpdateOutcome::Updated)
}

async fn update_staff_user(
    pool: &PgPool,
    user_id: i64,
    changes: StaffUserChanges,
) -> RepositoryResult<UserUpdateOutcome> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| repository_error("begin staff user update", error))?;
    let Some(current_email) = sqlx::query_scalar::<_, String>(
        "SELECT email FROM users WHERE id = $1 AND is_admin = 0 AND is_staff = 0 LIMIT 1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| repository_error("lock staff-scoped user", error))?
    else {
        return Ok(UserUpdateOutcome::UserNotFound);
    };
    if let Some(email) = &changes.email
        && email != &current_email
    {
        let taken = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) AND id <> $2 LIMIT 1",
        )
        .bind(email)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("check staff user email", error))?;
        if taken.is_some() {
            return Ok(UserUpdateOutcome::EmailAlreadyRegistered);
        }
    }
    // Staff clearing a plan intentionally leaves the historical group binding untouched.
    let plan_binding = match changes.plan_id {
        Some(Some(plan_id)) => {
            let Some(plan) = crate::plan::find_plan_binding_for_share(&mut tx, plan_id)
                .await
                .map_err(|error| repository_error("lock staff user plan", error))?
            else {
                return Ok(UserUpdateOutcome::PlanNotFound);
            };
            Some((Some(plan.id), Some(plan.group_id)))
        }
        Some(None) => Some((None, None)),
        None => None,
    };

    let mut update = QueryBuilder::<Postgres>::new("UPDATE users SET updated_at = ");
    update.push_bind(changes.updated_at);
    if let Some(email) = changes.email {
        update.push(", email = ").push_bind(email);
    }
    if let Some(password_hash) = changes.password_hash {
        update
            .push(", password = ")
            .push_bind(password_hash)
            .push(", password_algo = NULL");
    }
    if let Some(value) = changes.transfer_enable {
        update.push(", transfer_enable = ").push_bind(value);
    }
    if let Some(value) = changes.uploaded {
        update.push(", u = ").push_bind(value);
    }
    if let Some(value) = changes.downloaded {
        update.push(", d = ").push_bind(value);
    }
    if let Some(value) = changes.balance {
        update.push(", balance = ").push_bind(value);
    }
    if let Some(value) = changes.commission_balance {
        update.push(", commission_balance = ").push_bind(value);
    }
    if let Some(value) = changes.banned {
        update.push(", banned = ").push_bind(i16::from(value));
    }
    push_nullable_i32(&mut update, "device_limit", changes.device_limit);
    push_nullable_i32(&mut update, "commission_rate", changes.commission_rate);
    push_nullable_i32(&mut update, "discount", changes.discount);
    push_nullable_i64(&mut update, "expired_at", changes.expired_at);
    if let Some((plan_id, group_id)) = plan_binding {
        update.push(", plan_id = ").push_bind(plan_id);
        if group_id.is_some() {
            update.push(", group_id = ").push_bind(group_id);
        }
    }
    if changes.revoke_sessions {
        update.push(", session_epoch = session_epoch + 1");
    }
    if changes.reset_traffic_epoch {
        update.push(", traffic_epoch = traffic_epoch + 1");
    }
    update
        .push(" WHERE id = ")
        .push_bind(user_id)
        .push(" AND is_admin = 0 AND is_staff = 0");
    if let Err(error) = update.build().execute(&mut *tx).await {
        if unique_violation(&error) {
            return Ok(UserUpdateOutcome::EmailAlreadyRegistered);
        }
        return Err(repository_error("update staff-scoped user", error));
    }
    tx.commit()
        .await
        .map_err(|error| repository_error("commit staff user update", error))?;
    Ok(UserUpdateOutcome::Updated)
}

fn push_nullable_i32(
    update: &mut QueryBuilder<Postgres>,
    column: &'static str,
    value: Option<Option<i32>>,
) {
    if let Some(value) = value {
        update.push(", ").push(column).push(" = ").push_bind(value);
    }
}

fn push_nullable_i64(
    update: &mut QueryBuilder<Postgres>,
    column: &'static str,
    value: Option<Option<i64>>,
) {
    if let Some(value) = value {
        update.push(", ").push(column).push(" = ").push_bind(value);
    }
}

async fn filtered_user_ids(
    pool: &PgPool,
    filters: &[UserFilterClause],
    staff_scoped: bool,
    maximum: usize,
) -> RepositoryResult<Option<Vec<i64>>> {
    let maximum_plus_one = maximum.saturating_add(1);
    let limit = i64::try_from(maximum_plus_one)
        .map_err(|error| repository_error("bound admin user mutation", error))?;
    let mut query = QueryBuilder::<Postgres>::new("SELECT u.id FROM users u WHERE 1 = 1");
    if staff_scoped {
        query.push(" AND u.is_admin = 0 AND u.is_staff = 0");
    }
    push_user_filters(&mut query, filters);
    query.push(" ORDER BY u.id ASC LIMIT ").push_bind(limit);
    let ids = query
        .build_query_scalar::<i64>()
        .fetch_all(pool)
        .await
        .map_err(|error| repository_error("select bounded admin users", error))?;
    if ids.len() > maximum {
        Ok(None)
    } else {
        Ok(Some(ids))
    }
}

async fn delete_user_ids(
    pool: &PgPool,
    ids: Vec<i64>,
    require_all: bool,
) -> RepositoryResult<DeleteUsersOutcome> {
    if ids.is_empty() {
        return Ok(DeleteUsersOutcome::Deleted(ids));
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| repository_error("begin admin user delete", error))?;
    if lock_orders_and_find_pending_stripe(&mut tx, &ids).await? {
        return Ok(DeleteUsersOutcome::PendingStripeOrder);
    }
    let found = lock_users(&mut tx, &ids).await?;
    if require_all && found != ids.len() {
        return Ok(DeleteUsersOutcome::UserNotFound);
    }
    delete_users_cascade(&mut tx, &ids).await?;
    tx.commit()
        .await
        .map_err(|error| repository_error("commit admin user delete", error))?;
    Ok(DeleteUsersOutcome::Deleted(ids))
}

async fn lock_orders_and_find_pending_stripe(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[i64],
) -> RepositoryResult<bool> {
    for chunk in ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
        let mut after_id = 0_i64;
        loop {
            let mut query = QueryBuilder::<Postgres>::new(
                "SELECT id, status, callback_no FROM orders WHERE user_id IN (",
            );
            push_id_binds(&mut query, chunk);
            query
                .push(") AND id > ")
                .push_bind(after_id)
                .push(" ORDER BY id LIMIT 500 FOR UPDATE");
            let rows = query
                .build_query_as::<(i64, i16, Option<String>)>()
                .fetch_all(&mut **tx)
                .await
                .map_err(|error| repository_error("lock deleted-user orders", error))?;
            let Some(last_id) = rows.last().map(|row| row.0) else {
                break;
            };
            if rows.iter().any(|(_, status, callback)| {
                *status == 0
                    && callback
                        .as_deref()
                        .is_some_and(|value| value.starts_with("pi_"))
            }) {
                return Ok(true);
            }
            after_id = last_id;
        }
    }
    Ok(false)
}

async fn lock_users(tx: &mut Transaction<'_, Postgres>, ids: &[i64]) -> RepositoryResult<usize> {
    let mut found = 0_usize;
    for chunk in ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
        let mut query = QueryBuilder::<Postgres>::new("SELECT id FROM users WHERE id IN (");
        push_id_binds(&mut query, chunk);
        query.push(") ORDER BY id FOR UPDATE");
        found += query
            .build_query_scalar::<i64>()
            .fetch_all(&mut **tx)
            .await
            .map_err(|error| repository_error("lock deleted admin users", error))?
            .len();
    }
    Ok(found)
}

async fn delete_users_cascade(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[i64],
) -> RepositoryResult<()> {
    for chunk in ids.chunks(USER_DELETE_SQL_BATCH_SIZE) {
        execute_id_mutation(tx, "DELETE FROM orders WHERE user_id IN (", chunk).await?;
        execute_id_mutation(tx, "DELETE FROM invite_code WHERE user_id IN (", chunk).await?;

        let mut messages = QueryBuilder::<Postgres>::new(
            "DELETE FROM ticket_message tm USING ticket t \
             WHERE t.id = tm.ticket_id AND t.user_id IN (",
        );
        push_id_binds(&mut messages, chunk);
        messages.push(")");
        messages
            .build()
            .execute(&mut **tx)
            .await
            .map_err(|error| repository_error("delete user ticket messages", error))?;

        execute_id_mutation(tx, "DELETE FROM ticket WHERE user_id IN (", chunk).await?;
        execute_id_mutation(
            tx,
            "UPDATE users SET invite_user_id = NULL WHERE invite_user_id IN (",
            chunk,
        )
        .await?;
        execute_id_mutation(tx, "DELETE FROM users WHERE id IN (", chunk).await?;
    }
    Ok(())
}

async fn execute_id_mutation(
    tx: &mut Transaction<'_, Postgres>,
    prefix: &'static str,
    ids: &[i64],
) -> RepositoryResult<()> {
    let mut query = QueryBuilder::<Postgres>::new(prefix);
    push_id_binds(&mut query, ids);
    query.push(")");
    query
        .build()
        .execute(&mut **tx)
        .await
        .map_err(|error| repository_error("cascade deleted admin users", error))?;
    Ok(())
}

fn push_id_binds(builder: &mut QueryBuilder<Postgres>, ids: &[i64]) {
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(*id);
    }
}
