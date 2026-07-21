use sqlx::{FromRow, PgPool, Postgres, Transaction};
use v2board_application::{
    RepositoryError,
    order_jobs::{
        CommissionClaim, CommissionOrder, CommissionRepository, RenewalClaim, RenewalRepository,
        RenewalSnapshot, RenewalWrite, RepositoryResult,
    },
};
use v2board_domain_model::{
    CommissionInviter, CommissionPayout, CommissionState, OrderKind, OrderPeriod, OrderState,
    PlanPricePeriod,
};

const COMMISSION_READY_SQL: &str = r#"
WITH candidates AS (
    SELECT id
    FROM orders
    WHERE commission_status = 0
      AND invite_user_id IS NOT NULL
      AND status IN (3, 4)
      AND updated_at <= $2
    ORDER BY id
    LIMIT $3
    FOR UPDATE SKIP LOCKED
)
UPDATE orders AS target
SET commission_status = 1, updated_at = $1
FROM candidates
WHERE target.id = candidates.id AND target.commission_status = 0
"#;

const COMMISSION_CLAIM_SQL: &str = r#"
SELECT id, invite_user_id, user_id, trade_no, total_amount, commission_balance,
       actual_commission_balance
FROM orders
WHERE commission_status = 1
  AND invite_user_id IS NOT NULL
  AND id > $1
ORDER BY id
LIMIT 1
FOR UPDATE SKIP LOCKED
"#;

const RENEWAL_CANDIDATE_SQL: &str = r#"
SELECT id
FROM users
WHERE auto_renewal <> 0
  AND plan_id IS NOT NULL
  AND expired_at IS NOT NULL
  AND expired_at > $1
  AND expired_at < $2
  AND id > $3
ORDER BY id
LIMIT $4
"#;

const RENEWAL_LOCKED_USER_SQL: &str = r#"
SELECT id, balance, plan_id, expired_at
FROM users
WHERE id = $1
  AND auto_renewal <> 0
  AND plan_id IS NOT NULL
  AND expired_at IS NOT NULL
  AND expired_at > $2
  AND expired_at < $3
LIMIT 1
FOR UPDATE
"#;

#[derive(Clone, Debug)]
pub struct PostgresOrderJobsRepository {
    pool: PgPool,
}

impl PostgresOrderJobsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

pub struct PostgresCommissionClaim<'a> {
    tx: Transaction<'a, Postgres>,
    order: CommissionOrder,
}

impl CommissionClaim for PostgresCommissionClaim<'_> {
    fn order(&self) -> &CommissionOrder {
        &self.order
    }

    async fn inviter_chain(
        &mut self,
        start_id: i64,
        max_depth: usize,
    ) -> RepositoryResult<Vec<CommissionInviter>> {
        let mut chain = Vec::with_capacity(max_depth);
        let mut cursor = Some(start_id);
        for _ in 0..max_depth {
            let Some(id) = cursor else {
                break;
            };
            if chain
                .iter()
                .any(|inviter: &CommissionInviter| inviter.id == id)
            {
                break;
            }
            let row = sqlx::query_as::<_, (i64, Option<i64>)>(
                "SELECT id, invite_user_id FROM users WHERE id = $1 LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(|error| repository_error("load commission inviter chain", error))?;
            let Some((id, inviter_id)) = row else {
                break;
            };
            chain.push(CommissionInviter { id, inviter_id });
            cursor = inviter_id;
        }
        Ok(chain)
    }

    async fn settle(
        &mut self,
        payouts: &[CommissionPayout],
        credit_account_balance: bool,
        actual_commission_balance: i32,
        now: i64,
    ) -> RepositoryResult<()> {
        for payout in payouts {
            let (balance, commission_balance) = sqlx::query_as::<_, (i32, i32)>(
                "SELECT balance, commission_balance FROM users WHERE id = $1 LIMIT 1 FOR UPDATE",
            )
            .bind(payout.inviter_id)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(|error| repository_error("lock commission recipient", error))?
            .ok_or_else(|| {
                repository_error("lock commission recipient", "recipient no longer exists")
            })?;
            let amount = payout.amount.get();
            if credit_account_balance {
                let next = balance.checked_add(amount).ok_or_else(|| {
                    repository_error("credit commission", "account balance exceeds integer range")
                })?;
                sqlx::query("UPDATE users SET balance = $1, updated_at = $2 WHERE id = $3")
                    .bind(next)
                    .bind(now)
                    .bind(payout.inviter_id)
                    .execute(&mut *self.tx)
                    .await
                    .map_err(|error| repository_error("credit commission balance", error))?;
            } else {
                let next = commission_balance.checked_add(amount).ok_or_else(|| {
                    repository_error(
                        "credit commission",
                        "commission balance exceeds integer range",
                    )
                })?;
                sqlx::query(
                    "UPDATE users SET commission_balance = $1, updated_at = $2 WHERE id = $3",
                )
                .bind(next)
                .bind(now)
                .bind(payout.inviter_id)
                .execute(&mut *self.tx)
                .await
                .map_err(|error| repository_error("credit commission balance", error))?;
            }
            sqlx::query(
                r#"
                INSERT INTO commission_log
                    (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(payout.inviter_id)
            .bind(self.order.user_id)
            .bind(&self.order.trade_no)
            .bind(self.order.total_amount)
            .bind(amount)
            .bind(now)
            .bind(now)
            .execute(&mut *self.tx)
            .await
            .map_err(|error| repository_error("record commission payout", error))?;
        }
        let completed = sqlx::query(
            r#"
            UPDATE orders
            SET commission_status = $1, actual_commission_balance = $2, updated_at = $3
            WHERE id = $4 AND commission_status = $5
            "#,
        )
        .bind(CommissionState::Paid.code())
        .bind(actual_commission_balance)
        .bind(now)
        .bind(self.order.id)
        .bind(CommissionState::Processing.code())
        .execute(&mut *self.tx)
        .await
        .map_err(|error| repository_error("complete commission payout", error))?;
        if completed.rows_affected() != 1 {
            return Err(repository_error(
                "complete commission payout",
                "commission order claim was lost",
            ));
        }
        Ok(())
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.tx
            .commit()
            .await
            .map_err(|error| repository_error("commit commission payout", error))
    }
}

impl CommissionRepository for PostgresOrderJobsRepository {
    type Claim<'a> = PostgresCommissionClaim<'a>;

    async fn mark_ready(&self, now: i64, cutoff: i64, limit: i64) -> RepositoryResult<u64> {
        sqlx::query(COMMISSION_READY_SQL)
            .bind(now)
            .bind(cutoff)
            .bind(limit)
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| repository_error("mark commissions ready", error))
    }

    async fn claim_after(&self, after_id: i64) -> RepositoryResult<Option<Self::Claim<'_>>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin commission claim", error))?;
        let order = sqlx::query_as::<_, CommissionOrderRow>(COMMISSION_CLAIM_SQL)
            .bind(after_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| repository_error("claim commission order", error))?;
        Ok(order.map(|order| PostgresCommissionClaim {
            tx,
            order: order.application(),
        }))
    }
}

#[derive(Debug, Clone, FromRow)]
struct CommissionOrderRow {
    id: i64,
    invite_user_id: i64,
    user_id: i64,
    trade_no: String,
    total_amount: i32,
    commission_balance: i32,
    actual_commission_balance: Option<i32>,
}

impl CommissionOrderRow {
    fn application(self) -> CommissionOrder {
        CommissionOrder {
            id: self.id,
            invite_user_id: self.invite_user_id,
            user_id: self.user_id,
            trade_no: self.trade_no,
            total_amount: self.total_amount,
            commission_balance: self.commission_balance,
            actual_commission_balance: self.actual_commission_balance,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct RenewalUserRow {
    id: i64,
    balance: i32,
    plan_id: i32,
    expired_at: i64,
}

pub struct PostgresRenewalClaim<'a> {
    tx: Transaction<'a, Postgres>,
    snapshot: RenewalSnapshot,
}

impl RenewalClaim for PostgresRenewalClaim<'_> {
    fn snapshot(&self) -> &RenewalSnapshot {
        &self.snapshot
    }

    async fn disable(&mut self, now: i64) -> RepositoryResult<()> {
        sqlx::query(
            "UPDATE users SET auto_renewal = 0, updated_at = $1 WHERE id = $2 AND auto_renewal <> 0",
        )
        .bind(now)
        .bind(self.snapshot.user_id)
        .execute(&mut *self.tx)
        .await
        .map(|_| ())
        .map_err(|error| repository_error("disable auto renewal", error))
    }

    async fn renew(&mut self, write: RenewalWrite) -> RepositoryResult<()> {
        let updated = sqlx::query(
            r#"
            UPDATE users
            SET balance = balance - $1, expired_at = $2, updated_at = $3
            WHERE id = $4
              AND auto_renewal <> 0
              AND plan_id = $5
              AND expired_at = $6
              AND balance >= $7
              AND expired_at > $8
            "#,
        )
        .bind(write.debit)
        .bind(write.expired_at)
        .bind(write.now)
        .bind(self.snapshot.user_id)
        .bind(self.snapshot.plan_id)
        .bind(self.snapshot.expired_at)
        .bind(write.debit)
        .bind(write.now)
        .execute(&mut *self.tx)
        .await
        .map_err(|error| repository_error("debit renewal", error))?;
        if updated.rows_affected() != 1 {
            return Err(repository_error(
                "debit renewal",
                "auto-renew user changed while its row lock was held",
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO orders
                (user_id, plan_id, "type", period, trade_no, total_amount, balance_amount, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, 0, $6, $7, $8, $9)
            "#,
        )
        .bind(self.snapshot.user_id)
        .bind(self.snapshot.plan_id)
        .bind(OrderKind::Renewal.code())
        .bind(order_period_storage(write.period))
        .bind(write.trade_no)
        .bind(write.debit)
        .bind(OrderState::Completed.code())
        .bind(write.now)
        .bind(write.now)
        .execute(&mut *self.tx)
        .await
        .map(|_| ())
        .map_err(|error| repository_error("create renewal order", error))
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.tx
            .commit()
            .await
            .map_err(|error| repository_error("commit renewal", error))
    }
}

impl RenewalRepository for PostgresOrderJobsRepository {
    type Claim<'a> = PostgresRenewalClaim<'a>;

    async fn candidates(
        &self,
        after_id: i64,
        now: i64,
        renewal_before: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<i64>> {
        sqlx::query_scalar(RENEWAL_CANDIDATE_SQL)
            .bind(now)
            .bind(renewal_before)
            .bind(after_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("list renewal candidates", error))
    }

    async fn claim(
        &self,
        user_id: i64,
        now: i64,
        renewal_before: i64,
    ) -> RepositoryResult<Option<Self::Claim<'_>>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin renewal claim", error))?;
        // Preserve the global order -> user -> plan lock sequence.
        let latest_period = sqlx::query_scalar::<_, String>(
            r#"
            SELECT period
            FROM orders
            WHERE user_id = $1
              AND period NOT IN ('reset_price', 'onetime_price', 'deposit')
              AND status = 3
            ORDER BY created_at DESC
            LIMIT 1
            FOR SHARE
            "#,
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("load renewal period", error))?;
        let user = sqlx::query_as::<_, RenewalUserRow>(RENEWAL_LOCKED_USER_SQL)
            .bind(user_id)
            .bind(now)
            .bind(renewal_before)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| repository_error("claim renewal user", error))?;
        let Some(user) = user else {
            return Ok(None);
        };
        let plan = crate::plan::find_renewal_plan_for_share(&mut tx, user.plan_id)
            .await
            .map_err(|error| repository_error("load renewal plan", error))?;
        let period = latest_period.as_deref().and_then(order_period_from_storage);
        let price = match (period, plan.as_ref()) {
            (Some(OrderPeriod::Plan(period)), Some(plan)) => plan.recurring_price(period),
            _ => None,
        };
        let snapshot = RenewalSnapshot {
            user_id: user.id,
            balance: user.balance,
            plan_id: user.plan_id,
            expired_at: user.expired_at,
            period,
            price,
            plan_allows_renewal: plan.is_some_and(|plan| plan.renew),
        };
        Ok(Some(PostgresRenewalClaim { tx, snapshot }))
    }
}

fn order_period_from_storage(period: &str) -> Option<OrderPeriod> {
    match period {
        "month_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Month)),
        "quarter_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Quarter)),
        "half_year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::HalfYear)),
        "year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Year)),
        "two_year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::TwoYear)),
        "three_year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::ThreeYear)),
        "onetime_price" => Some(OrderPeriod::Plan(PlanPricePeriod::OneTime)),
        "reset_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Reset)),
        "deposit" => Some(OrderPeriod::Deposit),
        _ => None,
    }
}

const fn order_period_storage(period: OrderPeriod) -> &'static str {
    match period {
        OrderPeriod::Plan(PlanPricePeriod::Month) => "month_price",
        OrderPeriod::Plan(PlanPricePeriod::Quarter) => "quarter_price",
        OrderPeriod::Plan(PlanPricePeriod::HalfYear) => "half_year_price",
        OrderPeriod::Plan(PlanPricePeriod::Year) => "year_price",
        OrderPeriod::Plan(PlanPricePeriod::TwoYear) => "two_year_price",
        OrderPeriod::Plan(PlanPricePeriod::ThreeYear) => "three_year_price",
        OrderPeriod::Plan(PlanPricePeriod::OneTime) => "onetime_price",
        OrderPeriod::Plan(PlanPricePeriod::Reset) => "reset_price",
        OrderPeriod::Deposit => "deposit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_claims_remain_bounded_ordered_and_non_blocking() {
        assert!(COMMISSION_READY_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(COMMISSION_CLAIM_SQL.contains("ORDER BY id"));
        assert!(COMMISSION_CLAIM_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(RENEWAL_CANDIDATE_SQL.contains("id > $3"));
        assert!(RENEWAL_CANDIDATE_SQL.contains("ORDER BY id"));
        assert!(RENEWAL_CANDIDATE_SQL.contains("LIMIT $4"));
    }

    #[test]
    fn renewal_period_storage_mapping_is_closed_and_round_trips() {
        for period in [
            OrderPeriod::Plan(PlanPricePeriod::Month),
            OrderPeriod::Plan(PlanPricePeriod::Quarter),
            OrderPeriod::Plan(PlanPricePeriod::HalfYear),
            OrderPeriod::Plan(PlanPricePeriod::Year),
            OrderPeriod::Plan(PlanPricePeriod::TwoYear),
            OrderPeriod::Plan(PlanPricePeriod::ThreeYear),
            OrderPeriod::Plan(PlanPricePeriod::OneTime),
            OrderPeriod::Plan(PlanPricePeriod::Reset),
            OrderPeriod::Deposit,
        ] {
            assert_eq!(
                order_period_from_storage(order_period_storage(period)),
                Some(period)
            );
        }
    }
}
