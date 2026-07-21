use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use v2board_application::{
    RepositoryError,
    plan::{
        CreatePlanOutcome, DeletePlanOutcome, NewPlan, PatchPlanOutcome, Plan, PlanChanges,
        PlanReference, PlanRepository, RepositoryResult, SortPlansOutcome,
    },
};
use v2board_domain_model::{
    MoneyMinor, PLAN_FORCE_UPDATE_MAX_USERS, PlanPricePeriod, PlanPriceUpdate, PlanPrices,
    plan_transfer_bytes,
};

const PLAN_USER_LOCK_PAGE_SIZE: i64 = 500;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PlanRow {
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
    pub created_at: i64,
    pub updated_at: i64,
}

/// The plan-owned fields copied onto a user when an account is bound to a
/// plan. Callers must read this projection while holding the returned parent
/// row lock until the user write commits; otherwise a concurrent forced plan
/// update can leave the account with a stale group or limit snapshot.
#[derive(Debug, Clone, FromRow)]
pub struct PlanBindingRow {
    pub id: i32,
    pub group_id: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub speed_limit: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum SortPlansError {
    #[error("the submitted plan ids are not the current complete plan set")]
    PlanSetChanged,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl PlanRow {
    pub fn price(&self, period: PlanPricePeriod) -> Option<i32> {
        match period {
            PlanPricePeriod::Month => self.month_price,
            PlanPricePeriod::Quarter => self.quarter_price,
            PlanPricePeriod::HalfYear => self.half_year_price,
            PlanPricePeriod::Year => self.year_price,
            PlanPricePeriod::TwoYear => self.two_year_price,
            PlanPricePeriod::ThreeYear => self.three_year_price,
            PlanPricePeriod::OneTime => self.onetime_price,
            PlanPricePeriod::Reset => self.reset_price,
        }
    }

    /// Converts the persistence projection into the native period-keyed
    /// collection consumed by application services. The SQL projection is
    /// deliberately the last place that knows the historical wire field
    /// names.
    pub fn prices(&self) -> Result<PlanPrices, sqlx::Error> {
        let mut prices = PlanPrices::default();
        for period in PlanPricePeriod::ALL {
            let amount = self
                .price(period)
                .map(i64::from)
                .map(MoneyMinor::try_from)
                .transpose()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
            prices.set(period, amount);
        }
        Ok(prices)
    }
}

pub async fn find_plan(pool: &PgPool, id: i32) -> Result<Option<PlanRow>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(PLAN_PROJECTION_SQL);
    query.push(" WHERE p.id = ").push_bind(id).push(" LIMIT 1");
    query.build_query_as::<PlanRow>().fetch_optional(pool).await
}

/// Atomically replaces the ordering of the complete current plan set.
///
/// The table lock makes set validation and the one-statement resequence a
/// single serialization point: concurrent plan creates/deletes/reorders cannot
/// interleave or leave a partially updated order. An empty list succeeds only
/// when the current plan table is also empty.
pub async fn sort_plans_exact(pool: &PgPool, ids: &[i32]) -> Result<(), SortPlansError> {
    let mut tx = pool.begin().await?;
    sqlx::query("LOCK TABLE plan IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;

    let current_ids = sqlx::query_scalar::<_, i32>("SELECT id FROM plan ORDER BY id")
        .fetch_all(&mut *tx)
        .await?;
    let submitted = ids.iter().copied().collect::<BTreeSet<_>>();
    let current = current_ids.iter().copied().collect::<BTreeSet<_>>();
    if submitted.len() != ids.len() || submitted != current {
        return Err(SortPlansError::PlanSetChanged);
    }

    let updated = sqlx::query(
        r#"
        UPDATE plan AS target
        SET sort = ordered.ordinality::integer
        FROM unnest($1::integer[]) WITH ORDINALITY AS ordered(id, ordinality)
        WHERE target.id = ordered.id
        "#,
    )
    .bind(ids.to_vec())
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != u64::try_from(ids.len()).unwrap_or(u64::MAX) {
        return Err(SortPlansError::PlanSetChanged);
    }

    tx.commit().await?;
    Ok(())
}

pub async fn set_plan_price(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: i32,
    period: PlanPricePeriod,
    amount_minor: Option<MoneyMinor>,
) -> Result<(), sqlx::Error> {
    // Keep the normalized child write inseparable from the parent lock that
    // serializes plan projections. Existing callers may already hold it; a
    // repeated FOR UPDATE in the same transaction is cheap and makes future
    // direct callers safe by construction.
    let parent_exists =
        sqlx::query_scalar::<_, i32>("SELECT id FROM plan WHERE id = $1 FOR UPDATE")
            .bind(plan_id)
            .fetch_optional(&mut **tx)
            .await?;
    if parent_exists.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    match amount_minor {
        Some(amount_minor) => {
            sqlx::query(
                r#"
                INSERT INTO plan_price (plan_id, period, amount_minor)
                VALUES ($1, CAST($2 AS plan_price_period), $3)
                ON CONFLICT (plan_id, period)
                DO UPDATE SET amount_minor = EXCLUDED.amount_minor
                "#,
            )
            .bind(plan_id)
            .bind(native_plan_price_period(period))
            .bind(amount_minor.get())
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query(
                "DELETE FROM plan_price WHERE plan_id = $1 AND period = CAST($2 AS plan_price_period)",
            )
            .bind(plan_id)
            .bind(native_plan_price_period(period))
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

const fn native_plan_price_period(period: PlanPricePeriod) -> &'static str {
    match period {
        PlanPricePeriod::Month => "month",
        PlanPricePeriod::Quarter => "quarter",
        PlanPricePeriod::HalfYear => "half_year",
        PlanPricePeriod::Year => "year",
        PlanPricePeriod::TwoYear => "two_year",
        PlanPricePeriod::ThreeYear => "three_year",
        PlanPricePeriod::OneTime => "one_time",
        PlanPricePeriod::Reset => "reset",
    }
}

/// Locks the plan row that serializes every capacity-consuming path.
pub async fn find_plan_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanRow>, sqlx::Error> {
    let locked =
        sqlx::query_scalar::<_, i32>("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    if locked.is_none() {
        return Ok(None);
    }
    fetch_plan_in_transaction(tx, id).await
}

/// Locks and returns the plan-owned user-binding projection. PostgreSQL's
/// READ COMMITTED row-lock semantics re-evaluate a row after a lock wait, so
/// the returned values are the writer's committed values rather than the
/// version visible before the wait began.
pub async fn find_plan_binding_for_share(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanBindingRow>, sqlx::Error> {
    sqlx::query_as::<_, PlanBindingRow>(
        r#"
        SELECT id, group_id, transfer_enable, device_limit, speed_limit
        FROM plan
        WHERE id = $1
        LIMIT 1
        FOR SHARE
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
}

/// Finds the first dependency in the stable business-error precedence used by
/// plan deletion. The preflight form locks the child row before the parent,
/// matching user/order writers; the authoritative post-parent form is a plain
/// MVCC read so it cannot invert that order after the plan lock is held.
pub async fn find_plan_reference_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanReference>, sqlx::Error> {
    find_plan_reference(tx, id, true).await
}

pub async fn find_plan_reference_after_parent_lock(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanReference>, sqlx::Error> {
    find_plan_reference(tx, id, false).await
}

async fn find_plan_reference(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
    lock_child: bool,
) -> Result<Option<PlanReference>, sqlx::Error> {
    let order = if lock_child {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM orders WHERE referenced_plan_id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>("SELECT id FROM orders WHERE referenced_plan_id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
    };
    if order.is_some() {
        return Ok(Some(PlanReference::Order));
    }

    let user = if lock_child {
        sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE plan_id = $1 LIMIT 1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
    } else {
        sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE plan_id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
    };
    if user.is_some() {
        return Ok(Some(PlanReference::User));
    }

    let gift_card = if lock_child {
        sqlx::query_scalar::<_, i32>(
            "SELECT id FROM gift_card WHERE plan_id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        sqlx::query_scalar::<_, i32>("SELECT id FROM gift_card WHERE plan_id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
    };
    Ok(gift_card.map(|_| PlanReference::GiftCard))
}

/// Worker renewal reads the normalized price projection while sharing the same
/// repository boundary as API/order reads.
#[derive(Debug, Clone, FromRow)]
pub struct RenewalPlanRow {
    pub id: i32,
    pub renew: bool,
    pub month_price: Option<i32>,
    pub quarter_price: Option<i32>,
    pub half_year_price: Option<i32>,
    pub year_price: Option<i32>,
    pub two_year_price: Option<i32>,
    pub three_year_price: Option<i32>,
}

impl RenewalPlanRow {
    pub fn recurring_price(&self, period: PlanPricePeriod) -> Option<i32> {
        let price = match period {
            PlanPricePeriod::Month => self.month_price,
            PlanPricePeriod::Quarter => self.quarter_price,
            PlanPricePeriod::HalfYear => self.half_year_price,
            PlanPricePeriod::Year => self.year_price,
            PlanPricePeriod::TwoYear => self.two_year_price,
            PlanPricePeriod::ThreeYear => self.three_year_price,
            PlanPricePeriod::OneTime | PlanPricePeriod::Reset => return None,
        };
        Some(price.unwrap_or(0))
    }
}

pub async fn find_renewal_plan_for_share(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<RenewalPlanRow>, sqlx::Error> {
    let locked =
        sqlx::query_scalar::<_, i32>("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR SHARE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    if locked.is_none() {
        return Ok(None);
    }
    // The price relation is read in a second READ COMMITTED statement after
    // the parent lock has been granted. A waiter therefore observes the price
    // rows committed by the writer it waited for, rather than the stale child
    // snapshot taken before that wait.
    let mut query = QueryBuilder::<Postgres>::new(PLAN_PROJECTION_SQL);
    query.push(" WHERE p.id = ").push_bind(id).push(" LIMIT 1");
    query
        .build_query_as::<RenewalPlanRow>()
        .fetch_optional(&mut **tx)
        .await
}

async fn fetch_plan_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanRow>, sqlx::Error> {
    // Keep this as a distinct statement from the parent-row lock above. See
    // `find_renewal_plan_for_share` for the snapshot rationale.
    let mut query = QueryBuilder::<Postgres>::new(PLAN_PROJECTION_SQL);
    query.push(" WHERE p.id = ").push_bind(id).push(" LIMIT 1");
    query
        .build_query_as::<PlanRow>()
        .fetch_optional(&mut **tx)
        .await
}

pub const PLAN_CAPACITY_USAGE_SQL: &str = r#"
SELECT
    (
        SELECT COUNT(*)
        FROM users AS active_user
        WHERE active_user.plan_id = $1
          AND (active_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT OR active_user.expired_at IS NULL)
    ) + (
        SELECT COUNT(DISTINCT pending_order.user_id)
        FROM orders AS pending_order
        WHERE pending_order.plan_id = $2
          AND pending_order.status IN (0, 1)
          AND pending_order.type IN (1, 3)
          AND NOT EXISTS (
              SELECT 1
              FROM users AS reserved_user
              WHERE reserved_user.id = pending_order.user_id
                AND reserved_user.plan_id = pending_order.plan_id
                AND (
                    reserved_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT
                    OR reserved_user.expired_at IS NULL
                )
          )
    ) AS capacity_used
"#;

pub async fn capacity_usage_for_update(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: i32,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(PLAN_CAPACITY_USAGE_SQL)
        .bind(plan_id)
        .bind(plan_id)
        .fetch_one(&mut **tx)
        .await
}

pub async fn fetch_visible_plans(pool: &PgPool) -> Result<Vec<PlanRow>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(PLAN_PROJECTION_SQL);
    query.push(" WHERE p.show ORDER BY p.sort ASC NULLS FIRST, p.id ASC");
    query.build_query_as::<PlanRow>().fetch_all(pool).await
}

pub async fn fetch_all_plans(pool: &PgPool) -> Result<Vec<PlanRow>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(PLAN_PROJECTION_SQL);
    query.push(" ORDER BY p.sort ASC NULLS FIRST, p.id ASC");
    query.build_query_as::<PlanRow>().fetch_all(pool).await
}

pub async fn fetch_plans_by_ids(
    pool: &PgPool,
    ids: &[i32],
) -> Result<HashMap<i32, PlanRow>, sqlx::Error> {
    let ids = ids.iter().copied().collect::<BTreeSet<_>>();
    let mut plans = HashMap::with_capacity(ids.len());
    let ids = ids.into_iter().collect::<Vec<_>>();
    for chunk in ids.chunks(500) {
        let mut query = QueryBuilder::<Postgres>::new(PLAN_PROJECTION_SQL);
        query.push(" WHERE p.id IN (");
        let mut separated = query.separated(", ");
        for id in chunk {
            separated.push_bind(*id);
        }
        query.push(")");
        for plan in query.build_query_as::<PlanRow>().fetch_all(pool).await? {
            plans.insert(plan.id, plan);
        }
    }
    Ok(plans)
}

pub async fn count_active_users_by_plan(pool: &PgPool) -> Result<HashMap<i32, i64>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PlanActiveCountRow>(
        r#"
        SELECT plan_id, COUNT(*) AS count
        FROM users
        WHERE plan_id IS NOT NULL
          AND (expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT OR expired_at IS NULL)
        GROUP BY plan_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.plan_id, row.count))
        .collect())
}

/// Capacity consumption includes active subscribers plus pending/opening
/// new-plan orders that have not yet materialized as an active user. Completed
/// orders disappear from the reservation term and are represented by the user
/// row; cancelled orders disappear naturally.
pub async fn count_capacity_usage_by_plan(pool: &PgPool) -> Result<HashMap<i32, i64>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PlanActiveCountRow>(
        r#"
        SELECT plan_id, SUM(slot_count)::BIGINT AS count
        FROM (
            SELECT plan_id, COUNT(*) AS slot_count
            FROM users
            WHERE plan_id IS NOT NULL
              AND (expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT OR expired_at IS NULL)
            GROUP BY plan_id

            UNION ALL

            SELECT pending_order.plan_id, COUNT(DISTINCT pending_order.user_id) AS slot_count
            FROM orders AS pending_order
            WHERE pending_order.status IN (0, 1)
              AND pending_order.type IN (1, 3)
              AND NOT EXISTS (
                  SELECT 1
                  FROM users AS reserved_user
                  WHERE reserved_user.id = pending_order.user_id
                    AND reserved_user.plan_id = pending_order.plan_id
                    AND (
                        reserved_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT
                        OR reserved_user.expired_at IS NULL
                    )
              )
            GROUP BY pending_order.plan_id
        ) AS capacity_usage
        GROUP BY plan_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.plan_id, row.count))
        .collect())
}

#[derive(Debug, FromRow)]
struct PlanActiveCountRow {
    plan_id: i32,
    count: i64,
}

/// PostgreSQL adapter for the complete operator plan use-case boundary.
///
/// The application service owns validation and business outcome mapping;
/// this adapter owns every SQL statement, transaction, and lock-order rule.
#[derive(Clone, Debug)]
pub struct PostgresPlanRepository {
    pool: PgPool,
}

impl PostgresPlanRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

fn application_plan(row: PlanRow, count: i64) -> Result<Plan, sqlx::Error> {
    let prices = row.prices()?;
    Ok(Plan {
        id: row.id,
        group_id: row.group_id,
        transfer_enable: row.transfer_enable,
        device_limit: row.device_limit,
        name: row.name,
        speed_limit: row.speed_limit,
        show: row.show,
        sort: row.sort,
        renew: row.renew,
        content: row.content,
        prices,
        reset_traffic_method: row.reset_traffic_method,
        capacity_limit: row.capacity_limit,
        count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn lock_server_group_for_share(
    tx: &mut Transaction<'_, Postgres>,
    group_id: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "SELECT id FROM server_group WHERE id::BIGINT = $1 LIMIT 1 FOR SHARE",
    )
    .bind(group_id)
    .fetch_optional(&mut **tx)
    .await
    .map(|row| row.is_some())
}

enum LockedPlanUsers {
    Locked(Vec<i64>),
    LimitExceeded,
}

async fn lock_plan_users_for_update(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: i32,
) -> Result<LockedPlanUsers, sqlx::Error> {
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
            return Ok(LockedPlanUsers::Locked(locked));
        };
        if locked.len().saturating_add(ids.len()) > PLAN_FORCE_UPDATE_MAX_USERS {
            return Ok(LockedPlanUsers::LimitExceeded);
        }
        locked.extend(ids);
        after_id = last_id;
    }
}

async fn plan_user_ids_after_parent_lock(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: i32,
) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE plan_id = $1 ORDER BY id LIMIT $2")
        .bind(plan_id)
        .bind(i64::try_from(PLAN_FORCE_UPDATE_MAX_USERS).unwrap_or(i64::MAX) + 1)
        .fetch_all(&mut **tx)
        .await
}

#[derive(Debug, FromRow)]
struct CurrentPlan {
    group_id: i32,
    transfer_enable: i64,
    device_limit: Option<i32>,
    speed_limit: Option<i32>,
}

fn plan_reference_for_constraint(constraint: Option<&str>) -> PlanReference {
    match constraint {
        Some("orders_referenced_plan_id_fkey") => PlanReference::Order,
        Some("users_plan_id_fkey") => PlanReference::User,
        Some("gift_card_plan_id_fkey") => PlanReference::GiftCard,
        _ => PlanReference::Unknown,
    }
}

fn plan_delete_outcome(error: &sqlx::Error) -> Option<DeletePlanOutcome> {
    let database_error = error.as_database_error()?;
    (database_error.code().as_deref() == Some("23503")).then(|| {
        DeletePlanOutcome::InUse(plan_reference_for_constraint(database_error.constraint()))
    })
}

impl PlanRepository for PostgresPlanRepository {
    async fn list(&self) -> RepositoryResult<Vec<Plan>> {
        let rows = fetch_all_plans(&self.pool)
            .await
            .map_err(|error| repository_error("list plans", error))?;
        let counts = count_active_users_by_plan(&self.pool)
            .await
            .map_err(|error| repository_error("count active plan users", error))?;
        rows.into_iter()
            .map(|row| {
                let count = counts.get(&row.id).copied().unwrap_or_default();
                application_plan(row, count)
                    .map_err(|error| repository_error("decode plan prices", error))
            })
            .collect()
    }

    async fn create(&self, plan: NewPlan) -> RepositoryResult<CreatePlanOutcome> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin plan creation", error))?;
        if !lock_server_group_for_share(&mut tx, plan.input.group_id)
            .await
            .map_err(|error| repository_error("lock plan server group", error))?
        {
            return Ok(CreatePlanOutcome::ServerGroupNotFound);
        }
        let id = sqlx::query_scalar::<_, i32>(
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
        .bind(plan.input.group_id)
        .bind(plan.input.transfer_enable)
        .bind(plan.input.device_limit)
        .bind(&plan.input.name)
        .bind(plan.input.speed_limit)
        .bind(&plan.input.content)
        .bind(plan.input.reset_traffic_method)
        .bind(plan.input.capacity_limit)
        .bind(plan.created_at)
        .bind(plan.updated_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| repository_error("insert plan", error))?;
        for (period, amount_minor) in plan.input.prices.iter() {
            if amount_minor.is_some() {
                set_plan_price(&mut tx, id, period, amount_minor)
                    .await
                    .map_err(|error| repository_error("insert plan price", error))?;
            }
        }
        tx.commit()
            .await
            .map_err(|error| repository_error("commit plan creation", error))?;
        Ok(CreatePlanOutcome::Created(id))
    }

    async fn patch(&self, id: i32, changes: PlanChanges) -> RepositoryResult<PatchPlanOutcome> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin plan update", error))?;
        let current = sqlx::query_as::<_, CurrentPlan>(
            "SELECT group_id, transfer_enable, device_limit, speed_limit \
             FROM plan WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("read current plan", error))?;
        let Some(current) = current else {
            return Ok(PatchPlanOutcome::PlanNotFound);
        };

        let target_group = changes.group_id.unwrap_or(i64::from(current.group_id));
        if (changes.group_id.is_some() || changes.force_update)
            && !lock_server_group_for_share(&mut tx, target_group)
                .await
                .map_err(|error| repository_error("lock plan server group", error))?
        {
            return Ok(PatchPlanOutcome::ServerGroupNotFound);
        }

        let locked_user_ids = if changes.force_update {
            match lock_plan_users_for_update(&mut tx, id)
                .await
                .map_err(|error| repository_error("lock plan users", error))?
            {
                LockedPlanUsers::Locked(ids) => Some(ids),
                LockedPlanUsers::LimitExceeded => {
                    return Ok(PatchPlanOutcome::ForceUpdateLimitExceeded);
                }
            }
        } else {
            None
        };

        let locked_current = sqlx::query_as::<_, CurrentPlan>(
            "SELECT group_id, transfer_enable, device_limit, speed_limit \
             FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("lock current plan", error))?;
        let Some(locked_current) = locked_current else {
            return Ok(PatchPlanOutcome::PlanNotFound);
        };
        if changes.force_update
            && changes.group_id.is_none()
            && locked_current.group_id != current.group_id
        {
            return Ok(PatchPlanOutcome::UpdateConflict);
        }
        if let Some(locked_user_ids) = locked_user_ids {
            let current_user_ids = plan_user_ids_after_parent_lock(&mut tx, id)
                .await
                .map_err(|error| repository_error("verify locked plan users", error))?;
            if current_user_ids.len() > PLAN_FORCE_UPDATE_MAX_USERS {
                return Ok(PatchPlanOutcome::ForceUpdateLimitExceeded);
            }
            if current_user_ids != locked_user_ids {
                return Ok(PatchPlanOutcome::UpdateConflict);
            }
        }

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE plan SET ");
        {
            let mut assignments = builder.separated(", ");
            if let Some(name) = &changes.name {
                assignments
                    .push("\"name\" = ")
                    .push_bind_unseparated(name.clone());
            }
            if let Some(group_id) = changes.group_id {
                assignments
                    .push("\"group_id\" = CAST(")
                    .push_bind_unseparated(group_id)
                    .push_unseparated(" AS INTEGER)");
            }
            if let Some(transfer_enable) = changes.transfer_enable {
                assignments
                    .push("\"transfer_enable\" = ")
                    .push_bind_unseparated(transfer_enable);
            }
            for (column, update) in [
                ("device_limit", changes.device_limit),
                ("speed_limit", changes.speed_limit),
                ("capacity_limit", changes.capacity_limit),
            ] {
                if let Some(value) = update {
                    assignments
                        .push(format!("\"{column}\" = CAST("))
                        .push_bind_unseparated(value)
                        .push_unseparated(" AS INTEGER)");
                }
            }
            if let Some(content) = &changes.content {
                assignments
                    .push("\"content\" = ")
                    .push_bind_unseparated(content.clone());
            }
            if let Some(reset_traffic_method) = changes.reset_traffic_method {
                assignments
                    .push("\"reset_traffic_method\" = CAST(")
                    .push_bind_unseparated(reset_traffic_method)
                    .push_unseparated(" AS SMALLINT)");
            }
            if let Some(show) = changes.show {
                assignments.push("\"show\" = ").push_bind_unseparated(show);
            }
            if let Some(renew) = changes.renew {
                assignments
                    .push("\"renew\" = ")
                    .push_bind_unseparated(renew);
            }
            assignments
                .push("\"updated_at\" = ")
                .push_bind_unseparated(changes.updated_at);
        }
        builder.push(" WHERE id = ").push_bind(id);
        builder
            .build()
            .execute(&mut *tx)
            .await
            .map_err(|error| repository_error("update plan", error))?;

        for (period, update) in changes.prices.iter() {
            let amount = match update {
                PlanPriceUpdate::Retain => continue,
                PlanPriceUpdate::Clear => None,
                PlanPriceUpdate::Set(amount) => Some(amount),
            };
            set_plan_price(&mut tx, id, period, amount)
                .await
                .map_err(|error| repository_error("update plan price", error))?;
        }

        if changes.force_update {
            let transfer_enable = changes
                .transfer_enable
                .unwrap_or(locked_current.transfer_enable);
            let transfer_enable_bytes = plan_transfer_bytes(transfer_enable).map_err(|error| {
                repository_error("convert plan transfer allowance", format!("{error:?}"))
            })?;
            let device_limit = changes
                .device_limit
                .unwrap_or(locked_current.device_limit.map(i64::from));
            let speed_limit = changes
                .speed_limit
                .unwrap_or(locked_current.speed_limit.map(i64::from));
            sqlx::query(
                r#"
                UPDATE users
                SET group_id = CAST($1::BIGINT AS INTEGER), transfer_enable = $2,
                    device_limit = CAST($3::BIGINT AS INTEGER),
                    speed_limit = CAST($4::BIGINT AS INTEGER), updated_at = $5
                WHERE plan_id = $6
                "#,
            )
            .bind(target_group)
            .bind(transfer_enable_bytes)
            .bind(device_limit)
            .bind(speed_limit)
            .bind(changes.updated_at)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| repository_error("propagate plan limits", error))?;
        }

        tx.commit()
            .await
            .map_err(|error| repository_error("commit plan update", error))?;
        Ok(PatchPlanOutcome::Updated)
    }

    async fn delete(&self, id: i32) -> RepositoryResult<DeletePlanOutcome> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin plan deletion", error))?;
        if let Some(reference) = find_plan_reference_for_update(&mut tx, id)
            .await
            .map_err(|error| repository_error("preflight plan references", error))?
        {
            return Ok(DeletePlanOutcome::InUse(reference));
        }
        let exists =
            sqlx::query_scalar::<_, i32>("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| repository_error("lock plan for deletion", error))?;
        if exists.is_none() {
            return Ok(DeletePlanOutcome::PlanNotFound);
        }
        if let Some(reference) = find_plan_reference_after_parent_lock(&mut tx, id)
            .await
            .map_err(|error| repository_error("verify plan references", error))?
        {
            return Ok(DeletePlanOutcome::InUse(reference));
        }
        let deleted = match sqlx::query("DELETE FROM plan WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                if let Some(outcome) = plan_delete_outcome(&error) {
                    return Ok(outcome);
                }
                return Err(repository_error("delete plan", error));
            }
        };
        if deleted.rows_affected() != 1 {
            return Ok(DeletePlanOutcome::PlanNotFound);
        }
        if let Err(error) = tx.commit().await {
            if let Some(outcome) = plan_delete_outcome(&error) {
                return Ok(outcome);
            }
            return Err(repository_error("commit plan deletion", error));
        }
        Ok(DeletePlanOutcome::Deleted)
    }

    async fn sort_exact(&self, ids: &[i32]) -> RepositoryResult<SortPlansOutcome> {
        match sort_plans_exact(&self.pool, ids).await {
            Ok(()) => Ok(SortPlansOutcome::Sorted),
            Err(SortPlansError::PlanSetChanged) => Ok(SortPlansOutcome::PlanSetChanged),
            Err(SortPlansError::Database(error)) => Err(repository_error("sort plans", error)),
        }
    }
}

const PLAN_PROJECTION_SQL: &str = r#"
SELECT
    p.id, p.group_id, p.transfer_enable, p.device_limit, p.name,
    p.speed_limit, p.show, p.sort, p.renew, p.content,
    prices.month_price, prices.quarter_price, prices.half_year_price,
    prices.year_price, prices.two_year_price, prices.three_year_price,
    prices.onetime_price, prices.reset_price,
    p.reset_traffic_method, p.capacity_limit, p.created_at, p.updated_at
FROM plan p
LEFT JOIN LATERAL (
    SELECT
        MAX(amount_minor) FILTER (WHERE period = 'month') AS month_price,
        MAX(amount_minor) FILTER (WHERE period = 'quarter') AS quarter_price,
        MAX(amount_minor) FILTER (WHERE period = 'half_year') AS half_year_price,
        MAX(amount_minor) FILTER (WHERE period = 'year') AS year_price,
        MAX(amount_minor) FILTER (WHERE period = 'two_year') AS two_year_price,
        MAX(amount_minor) FILTER (WHERE period = 'three_year') AS three_year_price,
        MAX(amount_minor) FILTER (WHERE period = 'one_time') AS onetime_price,
        MAX(amount_minor) FILTER (WHERE period = 'reset') AS reset_price
    FROM plan_price
    WHERE plan_id = p.id
) prices ON TRUE
"#;
