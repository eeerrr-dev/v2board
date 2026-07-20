use super::*;
use v2board_domain_model::{
    MoneyMinor, PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices,
};

const PLAN_USER_LOCK_PAGE_SIZE: i64 = 500;
const PLAN_FORCE_UPDATE_MAX_USERS: usize = 10_000;

pub(super) fn plan_sort_ids(ids: &[i64]) -> Result<Vec<i32>, ApiError> {
    let mut unique = HashSet::new();
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        let id = i32::try_from(*id)
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| validation_error("ids", "plan ids must be positive 32-bit integers"))?;
        if !unique.insert(id) {
            return Err(validation_error(
                "ids",
                "plan ids must not contain duplicates",
            ));
        }
        normalized.push(id);
    }
    Ok(normalized)
}

/// Application command for creating a plan. HTTP deserialization and OpenAPI
/// metadata belong to the API adapter, not this use-case boundary.
#[derive(Debug, Clone)]
pub struct PlanCreateCommand {
    pub name: String,
    pub group_id: i64,
    pub transfer_enable: i64,
    pub device_limit: Option<i64>,
    pub speed_limit: Option<i64>,
    pub capacity_limit: Option<i64>,
    pub content: Option<String>,
    pub prices: PlanPrices,
    pub reset_traffic_method: Option<i64>,
}

/// Application command for PATCH-like plan changes. The nested option keeps
/// the use-case distinction between retain, clear, and set without importing
/// a transport serializer into the application layer.
#[derive(Debug, Clone, Default)]
pub struct PlanPatchCommand {
    pub name: Option<String>,
    pub group_id: Option<i64>,
    pub transfer_enable: Option<i64>,
    pub device_limit: Option<Option<i64>>,
    pub speed_limit: Option<Option<i64>>,
    pub capacity_limit: Option<Option<i64>>,
    pub content: Option<Option<String>>,
    pub prices: PlanPriceUpdates,
    pub reset_traffic_method: Option<Option<i64>>,
    pub show: Option<bool>,
    pub renew: Option<bool>,
    pub force_update: Option<bool>,
}

/// Application projection. Epoch seconds are converted to RFC 3339 only at
/// the HTTP boundary so this type remains independent of a wire format.
#[derive(Debug, Clone)]
pub struct AdminPlanView {
    pub id: i32,
    pub group_id: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub name: String,
    pub speed_limit: Option<i32>,
    pub show: bool,
    pub sort: Option<i32>,
    pub renew: bool,
    pub content: Option<String>,
    pub prices: PlanPrices,
    pub reset_traffic_method: Option<i16>,
    pub capacity_limit: Option<i32>,
    pub count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

async fn set_plan_price(
    tx: &mut DbTransaction<'_>,
    plan_id: i32,
    period: PlanPricePeriod,
    amount_minor: Option<MoneyMinor>,
) -> Result<(), ApiError> {
    v2board_db::plan::set_plan_price(tx, plan_id, period, amount_minor).await?;
    Ok(())
}

async fn lock_server_group_for_share(
    tx: &mut DbTransaction<'_>,
    group_id: i64,
) -> Result<(), ApiError> {
    let exists: Option<i32> =
        sqlx::query_scalar("SELECT id FROM server_group WHERE id = $1 LIMIT 1 FOR SHARE")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    if exists.is_none() {
        return Err(Problem::new(Code::ServerGroupNotFound).into());
    }
    Ok(())
}

async fn lock_plan_users_for_update(
    tx: &mut DbTransaction<'_>,
    plan_id: i64,
) -> Result<Vec<i64>, ApiError> {
    let mut after_id = 0_i64;
    let mut locked = Vec::new();
    loop {
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id
            FROM users
            WHERE plan_id = $1 AND id > $2
            ORDER BY id
            LIMIT $3
            FOR UPDATE
            "#,
        )
        .bind(plan_id)
        .bind(after_id)
        .bind(PLAN_USER_LOCK_PAGE_SIZE)
        .fetch_all(&mut **tx)
        .await?;
        let Some(last_id) = ids.last().copied() else {
            return Ok(locked);
        };
        if locked.len().saturating_add(ids.len()) > PLAN_FORCE_UPDATE_MAX_USERS {
            return Err(Problem::new(Code::PlanForceUpdateLimitExceeded).into());
        }
        locked.extend(ids);
        after_id = last_id;
    }
}

async fn plan_user_ids_after_parent_lock(
    tx: &mut DbTransaction<'_>,
    plan_id: i64,
) -> Result<Vec<i64>, ApiError> {
    // One extra row distinguishes an over-limit late-binding set without
    // retaining an unbounded vector. The caller already owns the parent plan
    // lock, so no new binding can commit while this snapshot is read.
    Ok(
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE plan_id = $1 ORDER BY id LIMIT $2",
        )
        .bind(plan_id)
        .bind(i64::try_from(PLAN_FORCE_UPDATE_MAX_USERS).unwrap_or(i64::MAX) + 1)
        .fetch_all(&mut **tx)
        .await?,
    )
}

pub(super) fn plan_create_validation(body: &PlanCreateCommand) -> Result<(), ApiError> {
    if body.name.trim().is_empty() {
        return Err(validation_error("name", "name cannot be empty"));
    }
    nonnegative_i32("transfer_enable", body.transfer_enable)?;
    for (field, value) in [
        ("device_limit", body.device_limit),
        ("speed_limit", body.speed_limit),
        ("capacity_limit", body.capacity_limit),
    ] {
        optional_nonnegative_i32(field, value)?;
    }
    optional_reset_traffic_method("reset_traffic_method", body.reset_traffic_method)?;
    Ok(())
}

pub(super) fn plan_in_use_problem(reference: v2board_db::plan::PlanReferenceKind) -> Problem {
    let detail = match reference {
        v2board_db::plan::PlanReferenceKind::Order => "该订阅下存在订单无法删除",
        v2board_db::plan::PlanReferenceKind::User => "该订阅下存在用户无法删除",
        v2board_db::plan::PlanReferenceKind::GiftCard => "该订阅仍被礼品卡使用，无法删除",
    };
    Problem::new(Code::PlanInUse).with_detail(detail)
}

pub(super) fn plan_reference_for_constraint(
    constraint: Option<&str>,
) -> Option<v2board_db::plan::PlanReferenceKind> {
    match constraint {
        Some("orders_referenced_plan_id_fkey") => Some(v2board_db::plan::PlanReferenceKind::Order),
        Some("users_plan_id_fkey") => Some(v2board_db::plan::PlanReferenceKind::User),
        Some("gift_card_plan_id_fkey") => Some(v2board_db::plan::PlanReferenceKind::GiftCard),
        _ => None,
    }
}

fn map_plan_delete_error(error: sqlx::Error) -> ApiError {
    let Some(database_error) = error.as_database_error() else {
        return ApiError::Database(error);
    };
    if database_error.code().as_deref() != Some("23503") {
        return ApiError::Database(error);
    }
    match plan_reference_for_constraint(database_error.constraint()) {
        Some(reference) => plan_in_use_problem(reference).into(),
        // The parent has no other restrictive child relation today. Keep an
        // unknown future FK failure typed and non-leaky instead of exposing a
        // database error through this business endpoint.
        None => Problem::new(Code::PlanInUse).into(),
    }
}

pub(super) fn plan_patch_validation(body: &PlanPatchCommand) -> Result<(), ApiError> {
    if let Some(name) = &body.name
        && name.trim().is_empty()
    {
        return Err(validation_error("name", "name cannot be empty"));
    }
    if let Some(transfer_enable) = body.transfer_enable {
        nonnegative_i32("transfer_enable", transfer_enable)?;
    }
    for (field, value) in [
        ("device_limit", &body.device_limit),
        ("speed_limit", &body.speed_limit),
        ("capacity_limit", &body.capacity_limit),
    ] {
        if let Some(update) = value {
            optional_nonnegative_i32(field, *update)?;
        }
    }
    if let Some(update) = &body.reset_traffic_method {
        optional_reset_traffic_method("reset_traffic_method", *update)?;
    }
    Ok(())
}

impl AdminService {
    /// GET `plans` (§6.2): bare array — every plan, shown and hidden, with
    /// its active-user `count`, in the operator sort order.
    pub async fn plans_list(&self) -> Result<Vec<AdminPlanView>, ApiError> {
        let plans = v2board_db::plan::fetch_all_plans(&self.db).await?;
        let counts = v2board_db::plan::count_active_users_by_plan(&self.db).await?;
        Ok(plans
            .into_iter()
            .map(|plan| {
                let count = counts.get(&plan.id).copied().unwrap_or_default();
                let prices = plan.prices()?;
                Ok(AdminPlanView {
                    id: plan.id,
                    group_id: plan.group_id,
                    transfer_enable: plan.transfer_enable,
                    device_limit: plan.device_limit,
                    name: plan.name,
                    speed_limit: plan.speed_limit,
                    show: plan.show,
                    sort: plan.sort,
                    renew: plan.renew,
                    content: plan.content,
                    prices,
                    reset_traffic_method: plan.reset_traffic_method,
                    capacity_limit: plan.capacity_limit,
                    count,
                    created_at: plan.created_at,
                    updated_at: plan.updated_at,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?)
    }

    /// POST `plans` (§6.2) → the new id (a 201 `{id}` on the wire).
    pub async fn plan_create(&self, body: &PlanCreateCommand) -> Result<i32, ApiError> {
        plan_create_validation(body)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        // Group writers use group -> user -> plan ordering. The shared parent
        // lock makes a concurrent group drop wait before the plan is created.
        lock_server_group_for_share(&mut tx, body.group_id).await?;
        let id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO plan (
                group_id, transfer_enable, device_limit, name, speed_limit,
                content, reset_traffic_method, capacity_limit, created_at, updated_at
            )
            VALUES (
                CAST($1::BIGINT AS INTEGER), $2, CAST($3::BIGINT AS INTEGER), $4,
                CAST($5::BIGINT AS INTEGER), $6, CAST($7::BIGINT AS SMALLINT),
                CAST($8::BIGINT AS INTEGER), $9, $10
            )
            RETURNING id
            "#,
        )
        .bind(body.group_id)
        .bind(body.transfer_enable)
        .bind(body.device_limit)
        .bind(&body.name)
        .bind(body.speed_limit)
        .bind(&body.content)
        .bind(body.reset_traffic_method)
        .bind(body.capacity_limit)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        for (period, amount_minor) in body.prices.iter() {
            if amount_minor.is_some() {
                set_plan_price(&mut tx, id, period, amount_minor).await?;
            }
        }
        tx.commit().await?;
        Ok(id)
    }

    /// PATCH `plans/{id}` (§6.2): §4.4 partial update over the PlanSave
    /// field set plus the merged show/renew toggles. `force_update` locks
    /// and repropagates the **final** plan limits (post-patch values, with
    /// untouched columns read from the current row) to every subscribed
    /// user, preserving the legacy group -> user -> plan lock ordering.
    pub async fn plan_patch(&self, id: i64, body: &PlanPatchCommand) -> Result<(), ApiError> {
        plan_patch_validation(body)?;
        let force_update = body.force_update.unwrap_or(false);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        // This optimistic read identifies the parent group that must be locked
        // before users and the plan. Values used for propagation are re-read
        // only after the plan lock has actually been granted below.
        #[derive(FromRow)]
        struct CurrentPlan {
            group_id: i32,
            transfer_enable: i64,
            device_limit: Option<i32>,
            speed_limit: Option<i32>,
        }
        let current = sqlx::query_as::<_, CurrentPlan>(
            "SELECT group_id, transfer_enable, device_limit, speed_limit \
             FROM plan WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::PlanNotFound)))?;
        let target_group = body.group_id.unwrap_or(i64::from(current.group_id));
        if body.group_id.is_some() || force_update {
            // Group writers use group -> user -> plan ordering. The shared
            // parent lock makes a concurrent group drop wait before either
            // the plan or its users can be changed.
            lock_server_group_for_share(&mut tx, target_group).await?;
        }
        let locked_user_ids = if force_update {
            // Order lifecycle writers take user before plan. Acquire every
            // affected user in primary-key pages before the plan row so the
            // force propagation cannot invert that order or materialize an
            // unbounded id list.
            Some(lock_plan_users_for_update(&mut tx, id).await?)
        } else {
            None
        };
        let locked_current = sqlx::query_as::<_, CurrentPlan>(
            "SELECT group_id, transfer_enable, device_limit, speed_limit \
             FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::PlanNotFound)))?;
        if force_update && body.group_id.is_none() && locked_current.group_id != current.group_id {
            // Another PATCH moved the plan after our optimistic parent read.
            // Continuing would either propagate the wrong group or acquire a
            // group lock after user/plan locks and invert the global order.
            return Err(Problem::new(Code::PlanUpdateConflict).into());
        }
        if let Some(locked_user_ids) = locked_user_ids {
            // A creator or planless account can take the old plan version's
            // shared lock after the first user scan and commit its binding
            // before this exclusive parent lock is granted. Those rows were
            // not locked in child -> parent order. Updating them now would
            // invert the order and can deadlock with a concurrent user writer,
            // so fail the optimistic force attempt and let a retry lock the
            // authoritative set from the start.
            let current_user_ids = plan_user_ids_after_parent_lock(&mut tx, id).await?;
            if current_user_ids.len() > PLAN_FORCE_UPDATE_MAX_USERS {
                return Err(Problem::new(Code::PlanForceUpdateLimitExceeded).into());
            }
            if current_user_ids != locked_user_ids {
                return Err(Problem::new(Code::PlanUpdateConflict).into());
            }
        }

        let mut values: Vec<(&str, AdminSqlValue)> = Vec::new();
        if let Some(name) = &body.name {
            values.push(("name", AdminSqlValue::Text(name.clone())));
        }
        if let Some(group_id) = body.group_id {
            values.push(("group_id", AdminSqlValue::Integer(group_id)));
        }
        if let Some(transfer_enable) = body.transfer_enable {
            values.push(("transfer_enable", AdminSqlValue::Integer(transfer_enable)));
        }
        for (column, field) in [
            ("device_limit", &body.device_limit),
            ("speed_limit", &body.speed_limit),
            ("capacity_limit", &body.capacity_limit),
            ("reset_traffic_method", &body.reset_traffic_method),
        ] {
            if let Some(update) = field {
                values.push((
                    column,
                    update.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
                ));
            }
        }
        if let Some(content) = &body.content {
            values.push((
                "content",
                content
                    .clone()
                    .map_or(AdminSqlValue::TextNull, AdminSqlValue::Text),
            ));
        }
        if let Some(show) = body.show {
            values.push(("show", AdminSqlValue::Boolean(show)));
        }
        if let Some(renew) = body.renew {
            values.push(("renew", AdminSqlValue::Boolean(renew)));
        }
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE plan SET ");
        for (column, value) in &values {
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
            builder.push(", ");
        }
        builder.push("\"updated_at\" = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&mut *tx).await?;

        let plan_id = i32::try_from(id)
            .map_err(|_| validation_error("id", "plan id is outside the supported range"))?;
        for (period, update) in body.prices.iter() {
            match update {
                PlanPriceUpdate::Retain => {}
                PlanPriceUpdate::Clear => set_plan_price(&mut tx, plan_id, period, None).await?,
                PlanPriceUpdate::Set(amount_minor) => {
                    set_plan_price(&mut tx, plan_id, period, Some(amount_minor)).await?
                }
            }
        }

        if force_update {
            let transfer_enable_bytes = checked_gib_bytes(
                body.transfer_enable
                    .unwrap_or(locked_current.transfer_enable),
                "transfer_enable",
            )?;
            let device_limit = match &body.device_limit {
                Some(update) => *update,
                None => locked_current.device_limit.map(i64::from),
            };
            let speed_limit = match &body.speed_limit {
                Some(update) => *update,
                None => locked_current.speed_limit.map(i64::from),
            };
            sqlx::query(
                r#"
                UPDATE users
                SET group_id = CAST($1::BIGINT AS INTEGER), transfer_enable = $2,
                    device_limit = CAST($3::BIGINT AS INTEGER),
                    speed_limit = CAST($4::BIGINT AS INTEGER), updated_at = $5
                WHERE plan_id = $6::BIGINT
                "#,
            )
            .bind(target_group)
            .bind(transfer_enable_bytes)
            .bind(device_limit)
            .bind(speed_limit)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// DELETE `plans/{id}` (§6.2): rejects deletion while any order, user,
    /// or gift card still references the plan (400 `plan_in_use`, with the
    /// blocking dependency in `detail` per §3.4); a missing id is 404
    /// `plan_not_found`. One locking transaction, as the legacy drop.
    pub async fn plan_delete(&self, id: i64) -> Result<(), ApiError> {
        let id = i32::try_from(id).map_err(|_| ApiError::from(Problem::new(Code::PlanNotFound)))?;
        let mut tx = self.db.begin().await?;
        if let Some(reference) =
            v2board_db::plan::find_plan_reference_for_update(&mut tx, id).await?
        {
            return Err(plan_in_use_problem(reference).into());
        }
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(Problem::new(Code::PlanNotFound).into());
        }
        // The preflight can race with a child writer that acquired the plan's
        // FK key-share lock just before our parent lock. Once FOR UPDATE is
        // granted that writer has committed or rolled back, and this fresh
        // READ COMMITTED check is authoritative. It deliberately does not
        // lock child rows after the parent, which would invert the global
        // child -> plan order and permit a deadlock.
        if let Some(reference) =
            v2board_db::plan::find_plan_reference_after_parent_lock(&mut tx, id).await?
        {
            return Err(plan_in_use_problem(reference).into());
        }
        let deleted = sqlx::query("DELETE FROM plan WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_plan_delete_error)?;
        if deleted.rows_affected() != 1 {
            return Err(Problem::new(Code::PlanNotFound).into());
        }
        tx.commit().await.map_err(map_plan_delete_error)?;
        Ok(())
    }

    /// POST `plans/sort` (§6.2): JSON `{ids}` full resequencing; empty 204.
    pub async fn plans_sort(&self, ids: &[i64]) -> Result<(), ApiError> {
        let ids = plan_sort_ids(ids)?;
        match v2board_db::plan::sort_plans_exact(&self.db, &ids).await {
            Ok(()) => Ok(()),
            Err(v2board_db::plan::SortPlansError::PlanSetChanged) => {
                Err(Problem::new(Code::PlanUpdateConflict).into())
            }
            Err(v2board_db::plan::SortPlansError::Database(error)) => {
                Err(ApiError::Database(error))
            }
        }
    }
}
