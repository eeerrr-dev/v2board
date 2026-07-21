//! PostgreSQL transaction adapter for gift-card redemption.

use sqlx::{FromRow, PgPool, Postgres, Transaction};
use v2board_application::{
    RepositoryError,
    giftcard::{
        GiftCardPlan, GiftCardPlanCapacity, GiftCardRedemptionTransaction, GiftCardRepository,
        RepositoryResult,
    },
};
use v2board_domain_model::{GiftCardRedemptionMutation, GiftCardSnapshot, GiftCardUserSnapshot};

#[derive(Clone, Debug)]
pub struct PostgresGiftCardRepository {
    pool: PgPool,
}

impl PostgresGiftCardRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub struct PostgresGiftCardRedemption<'a> {
    transaction: Transaction<'a, Postgres>,
}

#[derive(FromRow)]
struct GiftCardRecord {
    id: i32,
    r#type: i16,
    value: Option<i32>,
    plan_id: Option<i32>,
    limit_use: Option<i32>,
    started_at: i64,
    ended_at: i64,
}

#[derive(FromRow)]
struct GiftCardUserRecord {
    id: i64,
    balance: i32,
    expired_at: Option<i64>,
    transfer_enable: i64,
    traffic_epoch: i64,
    u: i64,
    d: i64,
    plan_id: Option<i32>,
}

#[derive(FromRow)]
struct GiftCardPlanRecord {
    id: i32,
    group_id: i32,
    transfer_enable: i64,
    device_limit: Option<i32>,
    capacity_limit: Option<i32>,
}

const UNFINISHED_ORDER_RANGE_SQL: &str = r#"
SELECT id
FROM orders
WHERE user_id = $1 AND status IN (0, 1)
LIMIT 1
FOR UPDATE
"#;

const PLAN_CAPACITY_USAGE_SQL: &str = r#"
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

impl GiftCardRedemptionTransaction for PostgresGiftCardRedemption<'_> {
    async fn lock_unfinished_order_range(&mut self, user_id: i64) -> RepositoryResult<()> {
        let _: Option<i64> = sqlx::query_scalar(UNFINISHED_ORDER_RANGE_SQL)
            .bind(user_id)
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|error| repository_error("lock gift-card unfinished-order range", error))?;
        Ok(())
    }

    async fn lock_user(&mut self, user_id: i64) -> RepositoryResult<Option<GiftCardUserSnapshot>> {
        sqlx::query_as::<_, GiftCardUserRecord>(
            r#"
            SELECT id, balance, expired_at, transfer_enable, traffic_epoch, u, d, plan_id
            FROM users
            WHERE id = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(user_id)
        .fetch_optional(&mut *self.transaction)
        .await
        .map(|row| {
            row.map(|row| GiftCardUserSnapshot {
                id: row.id,
                balance: row.balance,
                expires_at: row.expired_at,
                transfer_enable: row.transfer_enable,
                traffic_epoch: row.traffic_epoch,
                uploaded: row.u,
                downloaded: row.d,
                plan_id: row.plan_id,
            })
        })
        .map_err(|error| repository_error("lock gift-card user", error))
    }

    async fn lock_giftcard(&mut self, code: &str) -> RepositoryResult<Option<GiftCardSnapshot>> {
        sqlx::query_as::<_, GiftCardRecord>(
            r#"
            SELECT id, "type", value, plan_id, limit_use, started_at, ended_at
            FROM gift_card
            WHERE lower(code) = lower($1)
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(code)
        .fetch_optional(&mut *self.transaction)
        .await
        .map(|row| {
            row.map(|row| GiftCardSnapshot {
                id: row.id,
                kind_code: row.r#type,
                value: row.value,
                plan_id: row.plan_id,
                remaining_uses: row.limit_use,
                starts_at: row.started_at,
                ends_at: row.ended_at,
            })
        })
        .map_err(|error| repository_error("lock gift card", error))
    }

    async fn already_redeemed(&mut self, giftcard_id: i32, user_id: i64) -> RepositoryResult<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM gift_card_redemption WHERE giftcard_id = $1 AND user_id = $2)",
        )
        .bind(giftcard_id)
        .bind(user_id)
        .fetch_one(&mut *self.transaction)
        .await
        .map_err(|error| repository_error("check prior gift-card redemption", error))
    }

    async fn lock_plan(&mut self, plan_id: i32) -> RepositoryResult<Option<GiftCardPlan>> {
        let plan = sqlx::query_as::<_, GiftCardPlanRecord>(
            r#"
            SELECT id, group_id, transfer_enable, device_limit, capacity_limit
            FROM plan
            WHERE id = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(plan_id)
        .fetch_optional(&mut *self.transaction)
        .await
        .map_err(|error| repository_error("lock gift-card plan", error))?;
        Ok(plan.map(|plan| GiftCardPlan {
            id: plan.id,
            group_id: plan.group_id,
            transfer_gib: plan.transfer_enable,
            device_limit: plan.device_limit,
            capacity_limit: plan.capacity_limit,
        }))
    }

    async fn plan_capacity_facts(
        &mut self,
        plan_id: i32,
        user_id: i64,
    ) -> RepositoryResult<GiftCardPlanCapacity> {
        let has_existing_reservation = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM orders
                WHERE user_id = $1 AND plan_id = $2
                  AND status IN (0, 1) AND "type" IN (1, 3)
            )
            "#,
        )
        .bind(user_id)
        .bind(plan_id)
        .fetch_one(&mut *self.transaction)
        .await
        .map_err(|error| repository_error("check gift-card plan reservation", error))?;
        let capacity_used: i64 = sqlx::query_scalar(PLAN_CAPACITY_USAGE_SQL)
            .bind(plan_id)
            .bind(plan_id)
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|error| repository_error("count gift-card plan capacity", error))?;
        Ok(GiftCardPlanCapacity {
            used: capacity_used,
            has_existing_reservation,
        })
    }

    async fn persist(&mut self, mutation: GiftCardRedemptionMutation) -> RepositoryResult<()> {
        let now = mutation.redeemed_at;
        let plan_binding = mutation.plan_binding;
        let updated = sqlx::query(
            r#"
            UPDATE users
            SET balance = $1, expired_at = $2, transfer_enable = $3, traffic_epoch = $4,
                u = $5, d = $6, plan_id = $7, group_id = COALESCE($8, group_id),
                device_limit = CASE WHEN $9 <> 0 THEN $10 ELSE device_limit END,
                updated_at = $11
            WHERE id = $12
            "#,
        )
        .bind(mutation.balance)
        .bind(mutation.expires_at)
        .bind(mutation.transfer_enable)
        .bind(mutation.traffic_epoch)
        .bind(mutation.uploaded)
        .bind(mutation.downloaded)
        .bind(mutation.plan_id)
        .bind(plan_binding.map(|binding| binding.group_id))
        .bind(i32::from(plan_binding.is_some()))
        .bind(plan_binding.and_then(|binding| binding.device_limit))
        .bind(now)
        .bind(mutation.user_id)
        .execute(&mut *self.transaction)
        .await
        .map_err(|error| repository_error("apply gift-card user mutation", error))?;
        if updated.rows_affected() != 1 {
            return Err(repository_error(
                "apply gift-card user mutation",
                "locked gift-card user disappeared",
            ));
        }
        sqlx::query(
            "INSERT INTO gift_card_redemption (giftcard_id, user_id, created_at) VALUES ($1, $2, $3)",
        )
        .bind(mutation.giftcard_id)
        .bind(mutation.user_id)
        .bind(now)
        .execute(&mut *self.transaction)
        .await
        .map_err(|error| repository_error("record gift-card redemption", error))?;
        let updated =
            sqlx::query("UPDATE gift_card SET limit_use = $1, updated_at = $2 WHERE id = $3")
                .bind(mutation.remaining_uses)
                .bind(now)
                .bind(mutation.giftcard_id)
                .execute(&mut *self.transaction)
                .await
                .map_err(|error| repository_error("consume gift-card use", error))?;
        if updated.rows_affected() != 1 {
            return Err(repository_error(
                "consume gift-card use",
                "locked gift card disappeared",
            ));
        }
        Ok(())
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit gift-card redemption", error))
    }
}

impl GiftCardRepository for PostgresGiftCardRepository {
    type Redemption<'a> = PostgresGiftCardRedemption<'a>;

    async fn begin_redemption(&self) -> RepositoryResult<Self::Redemption<'_>> {
        self.pool
            .begin()
            .await
            .map(|transaction| PostgresGiftCardRedemption { transaction })
            .map_err(|error| repository_error("begin gift-card redemption", error))
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}
