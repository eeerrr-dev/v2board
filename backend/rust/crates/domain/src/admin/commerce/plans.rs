use super::*;
use serde::Deserialize;
use v2board_compat::json::{double_option, rfc3339};

const PLAN_USER_LOCK_PAGE_SIZE: i64 = 500;
const PLAN_FORCE_UPDATE_MAX_USERS: usize = 10_000;

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
) -> Result<(), ApiError> {
    let mut after_id = 0_i64;
    let mut locked = 0_usize;
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
            return Ok(());
        };
        locked = locked.saturating_add(ids.len());
        if locked > PLAN_FORCE_UPDATE_MAX_USERS {
            return Err(Problem::new(Code::PlanForceUpdateLimitExceeded).into());
        }
        after_id = last_id;
    }
}

/// One admin plan row (§6.2 `GET plans`): the legacy field set plus the
/// active-user `count`, on modern value types — bool flags, §4.5 RFC 3339
/// timestamps. Prices stay integer cents; `transfer_enable` stays the
/// operator-facing GiB figure.
#[derive(Debug, Serialize)]
pub struct AdminPlanItem {
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
    pub month_price: Option<i32>,
    pub quarter_price: Option<i32>,
    pub half_year_price: Option<i32>,
    pub year_price: Option<i32>,
    pub two_year_price: Option<i32>,
    pub three_year_price: Option<i32>,
    pub onetime_price: Option<i32>,
    pub reset_price: Option<i32>,
    pub reset_traffic_method: Option<i16>,
    pub capacity_limit: Option<i32>,
    pub count: i64,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

/// POST `plans` (§6.2): the legacy PlanSave field set as a JSON body.
/// Creates keep the DB defaults PlanSave never touched (`show` = 0,
/// `renew` = 1, `sort` = NULL).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCreate {
    pub name: String,
    pub group_id: i64,
    pub transfer_enable: i64,
    #[serde(default)]
    pub device_limit: Option<i64>,
    #[serde(default)]
    pub speed_limit: Option<i64>,
    #[serde(default)]
    pub capacity_limit: Option<i64>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub month_price: Option<i64>,
    #[serde(default)]
    pub quarter_price: Option<i64>,
    #[serde(default)]
    pub half_year_price: Option<i64>,
    #[serde(default)]
    pub year_price: Option<i64>,
    #[serde(default)]
    pub two_year_price: Option<i64>,
    #[serde(default)]
    pub three_year_price: Option<i64>,
    #[serde(default)]
    pub onetime_price: Option<i64>,
    #[serde(default)]
    pub reset_price: Option<i64>,
    #[serde(default)]
    pub reset_traffic_method: Option<i64>,
}

/// PATCH `plans/{id}` (§6.2): §4.4 partial update merging the legacy
/// `plan/update` show/renew toggles; `force_update` stays a body flag that
/// propagates the final plan limits to every subscribed user.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub group_id: Option<i64>,
    #[serde(default)]
    pub transfer_enable: Option<i64>,
    #[serde(default, with = "double_option")]
    pub device_limit: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub speed_limit: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub capacity_limit: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub content: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub month_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub quarter_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub half_year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub two_year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub three_year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub onetime_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub reset_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub reset_traffic_method: Option<Option<i64>>,
    #[serde(default)]
    pub show: Option<bool>,
    #[serde(default)]
    pub renew: Option<bool>,
    #[serde(default)]
    pub force_update: Option<bool>,
}

fn plan_create_validation(body: &PlanCreate) -> Result<(), ApiError> {
    if body.name.trim().is_empty() {
        return Err(validation_error("name", "name cannot be empty"));
    }
    nonnegative_i32("transfer_enable", body.transfer_enable)?;
    for (field, value) in [
        ("device_limit", body.device_limit),
        ("speed_limit", body.speed_limit),
        ("capacity_limit", body.capacity_limit),
        ("month_price", body.month_price),
        ("quarter_price", body.quarter_price),
        ("half_year_price", body.half_year_price),
        ("year_price", body.year_price),
        ("two_year_price", body.two_year_price),
        ("three_year_price", body.three_year_price),
        ("onetime_price", body.onetime_price),
        ("reset_price", body.reset_price),
    ] {
        optional_nonnegative_i32(field, value)?;
    }
    optional_smallint("reset_traffic_method", body.reset_traffic_method)?;
    Ok(())
}

pub(super) fn plan_patch_validation(body: &PlanPatch) -> Result<(), ApiError> {
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
        ("month_price", &body.month_price),
        ("quarter_price", &body.quarter_price),
        ("half_year_price", &body.half_year_price),
        ("year_price", &body.year_price),
        ("two_year_price", &body.two_year_price),
        ("three_year_price", &body.three_year_price),
        ("onetime_price", &body.onetime_price),
        ("reset_price", &body.reset_price),
    ] {
        if let Some(update) = value {
            optional_nonnegative_i32(field, *update)?;
        }
    }
    if let Some(update) = &body.reset_traffic_method {
        optional_smallint("reset_traffic_method", *update)?;
    }
    Ok(())
}

impl AdminService {
    /// GET `plans` (§6.2): bare array — every plan, shown and hidden, with
    /// its active-user `count`, in the operator sort order.
    pub async fn plans_list(&self) -> Result<Vec<AdminPlanItem>, ApiError> {
        let plans = sqlx::query_as::<_, v2board_db::plan::PlanRow>(
            r#"
            SELECT id, group_id, transfer_enable, device_limit, name, speed_limit, "show", sort,
                   renew, content, month_price, quarter_price, half_year_price, year_price,
                   two_year_price, three_year_price, onetime_price, reset_price,
                   reset_traffic_method, capacity_limit, created_at, updated_at
            FROM plan
            ORDER BY sort ASC NULLS FIRST, id ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let counts = v2board_db::plan::count_active_users_by_plan(&self.db).await?;
        Ok(plans
            .into_iter()
            .map(|plan| {
                let count = counts.get(&plan.id).copied().unwrap_or_default();
                AdminPlanItem {
                    id: plan.id,
                    group_id: plan.group_id,
                    transfer_enable: plan.transfer_enable,
                    device_limit: plan.device_limit,
                    name: plan.name,
                    speed_limit: plan.speed_limit,
                    show: plan.show != 0,
                    sort: plan.sort,
                    renew: plan.renew != 0,
                    content: plan.content,
                    month_price: plan.month_price,
                    quarter_price: plan.quarter_price,
                    half_year_price: plan.half_year_price,
                    year_price: plan.year_price,
                    two_year_price: plan.two_year_price,
                    three_year_price: plan.three_year_price,
                    onetime_price: plan.onetime_price,
                    reset_price: plan.reset_price,
                    reset_traffic_method: plan.reset_traffic_method,
                    capacity_limit: plan.capacity_limit,
                    count,
                    created_at: plan.created_at,
                    updated_at: plan.updated_at,
                }
            })
            .collect())
    }

    /// POST `plans` (§6.2) → the new id (a 201 `{id}` on the wire).
    pub async fn plan_create(&self, body: &PlanCreate) -> Result<i32, ApiError> {
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
                content, month_price, quarter_price, half_year_price, year_price,
                two_year_price, three_year_price, onetime_price, reset_price,
                reset_traffic_method, capacity_limit, created_at, updated_at
            )
            VALUES (
                CAST($1::BIGINT AS INTEGER), $2, CAST($3::BIGINT AS INTEGER), $4,
                CAST($5::BIGINT AS INTEGER), $6, CAST($7::BIGINT AS INTEGER),
                CAST($8::BIGINT AS INTEGER), CAST($9::BIGINT AS INTEGER),
                CAST($10::BIGINT AS INTEGER), CAST($11::BIGINT AS INTEGER),
                CAST($12::BIGINT AS INTEGER), CAST($13::BIGINT AS INTEGER),
                CAST($14::BIGINT AS INTEGER), CAST($15::BIGINT AS SMALLINT),
                CAST($16::BIGINT AS INTEGER), $17, $18
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
        .bind(body.month_price)
        .bind(body.quarter_price)
        .bind(body.half_year_price)
        .bind(body.year_price)
        .bind(body.two_year_price)
        .bind(body.three_year_price)
        .bind(body.onetime_price)
        .bind(body.reset_price)
        .bind(body.reset_traffic_method)
        .bind(body.capacity_limit)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// PATCH `plans/{id}` (§6.2): §4.4 partial update over the PlanSave
    /// field set plus the merged show/renew toggles. `force_update` locks
    /// and repropagates the **final** plan limits (post-patch values, with
    /// untouched columns read from the current row) to every subscribed
    /// user, preserving the legacy group -> user -> plan lock ordering.
    pub async fn plan_patch(&self, id: i64, body: &PlanPatch) -> Result<(), ApiError> {
        plan_patch_validation(body)?;
        let force_update = body.force_update.unwrap_or(false);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        // The current values feed the group lock and the force propagation
        // when the body leaves them untouched. The plain read is safe: the
        // plan row itself is locked FOR UPDATE below before any write.
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
        if force_update {
            // Order lifecycle writers take user before plan. Acquire every
            // affected user in primary-key pages before the plan row so the
            // force propagation cannot invert that order or materialize an
            // unbounded id list.
            lock_plan_users_for_update(&mut tx, id).await?;
        }
        let locked: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if locked.is_none() {
            return Err(Problem::new(Code::PlanNotFound).into());
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
            ("month_price", &body.month_price),
            ("quarter_price", &body.quarter_price),
            ("half_year_price", &body.half_year_price),
            ("year_price", &body.year_price),
            ("two_year_price", &body.two_year_price),
            ("three_year_price", &body.three_year_price),
            ("onetime_price", &body.onetime_price),
            ("reset_price", &body.reset_price),
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
            values.push(("show", AdminSqlValue::Integer(i64::from(show))));
        }
        if let Some(renew) = body.renew {
            values.push(("renew", AdminSqlValue::Integer(i64::from(renew))));
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

        if force_update {
            let transfer_enable_bytes = checked_gib_bytes(
                body.transfer_enable.unwrap_or(current.transfer_enable),
                "transfer_enable",
            )?;
            let device_limit = match &body.device_limit {
                Some(update) => *update,
                None => current.device_limit.map(i64::from),
            };
            let speed_limit = match &body.speed_limit {
                Some(update) => *update,
                None => current.speed_limit.map(i64::from),
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
        let mut tx = self.db.begin().await?;
        let has_order: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM orders WHERE referenced_plan_id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if has_order.is_some() {
            return Err(Problem::new(Code::PlanInUse)
                .with_detail("该订阅下存在订单无法删除")
                .into());
        }
        let has_user: Option<i64> =
            sqlx::query_scalar("SELECT id FROM users WHERE plan_id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if has_user.is_some() {
            return Err(Problem::new(Code::PlanInUse)
                .with_detail("该订阅下存在用户无法删除")
                .into());
        }
        let has_giftcard: Option<i32> =
            sqlx::query_scalar("SELECT id FROM gift_card WHERE plan_id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if has_giftcard.is_some() {
            return Err(Problem::new(Code::PlanInUse)
                .with_detail("该订阅仍被礼品卡使用，无法删除")
                .into());
        }
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(Problem::new(Code::PlanNotFound).into());
        }
        let deleted = sqlx::query("DELETE FROM plan WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(Problem::new(Code::PlanNotFound).into());
        }
        tx.commit().await?;
        Ok(())
    }

    /// POST `plans/sort` (§6.2): JSON `{ids}` full resequencing; empty 204.
    pub async fn plans_sort(&self, ids: &[i64]) -> Result<(), ApiError> {
        self.sort_ids("plan", ids).await
    }
}
