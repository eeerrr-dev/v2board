use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use v2board_domain_model::{MoneyMinor, PlanPricePeriod, PlanPrices};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanReferenceKind {
    Order,
    User,
    GiftCard,
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
) -> Result<Option<PlanReferenceKind>, sqlx::Error> {
    find_plan_reference(tx, id, true).await
}

pub async fn find_plan_reference_after_parent_lock(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
) -> Result<Option<PlanReferenceKind>, sqlx::Error> {
    find_plan_reference(tx, id, false).await
}

async fn find_plan_reference(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
    lock_child: bool,
) -> Result<Option<PlanReferenceKind>, sqlx::Error> {
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
        return Ok(Some(PlanReferenceKind::Order));
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
        return Ok(Some(PlanReferenceKind::User));
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
    Ok(gift_card.map(|_| PlanReferenceKind::GiftCard))
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
