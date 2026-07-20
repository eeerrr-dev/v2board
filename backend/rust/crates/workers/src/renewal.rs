use chrono::{Months, TimeZone, Utc};
use sqlx::{FromRow, Postgres, Transaction};
use v2board_config::app_timezone;
use v2board_db::plan::{RenewalPlanRow, find_renewal_plan_for_share};
use v2board_domain_model::{
    MoneyMinor, NonNegativeMoneyMinor, OrderKind, OrderPeriod, OrderState, PlanPricePeriod,
    RenewalDecision, RenewalRequest, decide_renewal,
};

use crate::{batch::finish_item_batch, state::WorkerState, time::timestamp_after};

const RENEWAL_CANDIDATE_PAGE_SIZE: i64 = 250;
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
const RENEWAL_UPDATE_SQL: &str = r#"
UPDATE users
SET balance = balance - $1, expired_at = $2, updated_at = $3
WHERE id = $4
  AND auto_renewal <> 0
  AND plan_id = $5
  AND expired_at = $6
  AND balance >= $7
  AND expired_at > $8
"#;

#[derive(Debug, Clone, FromRow)]
struct RenewalUserRow {
    id: i64,
    balance: i32,
    plan_id: i32,
    expired_at: i64,
}

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let renewal_before = timestamp_after(now, 2 * 86_400);
    let mut after_id = 0_i64;
    let mut total = 0_usize;
    let mut failed = 0_usize;
    let mut first_error = None;

    loop {
        let user_ids = sqlx::query_scalar::<_, i64>(RENEWAL_CANDIDATE_SQL)
            .bind(now)
            .bind(renewal_before)
            .bind(after_id)
            .bind(RENEWAL_CANDIDATE_PAGE_SIZE)
            .fetch_all(&state.db)
            .await?;
        let Some(last_id) = user_ids.last().copied() else {
            break;
        };
        for user_id in user_ids {
            total += 1;
            if let Err(error) = renew_user(state, user_id).await {
                tracing::warn!(user_id, ?error, "auto renewal failed");
                failed += 1;
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
        after_id = last_id;
    }
    finish_item_batch("auto renewals", total, failed, first_error)
}

async fn renew_user(state: &WorkerState, user_id: i64) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    // Keep the global order lifecycle lock order: order/range first, then the
    // owning user, then the plan.  Cancellation and settlement take the same
    // order -> user sequence, so renewal cannot form the former user -> order
    // deadlock cycle with them.
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
    .await?;
    let Some(user) = sqlx::query_as::<_, RenewalUserRow>(RENEWAL_LOCKED_USER_SQL)
        .bind(user_id)
        .bind(now)
        .bind(timestamp_after(now, 2 * 86_400))
        .fetch_optional(&mut *tx)
        .await?
    else {
        tx.rollback().await?;
        return Ok(());
    };
    let Some(latest_period) = latest_period else {
        disable_auto_renewal(&mut tx, user.id, now).await?;
        tx.commit().await?;
        return Ok(());
    };
    let Some(plan) = find_renewal_plan_for_share(&mut tx, user.plan_id).await? else {
        disable_auto_renewal(&mut tx, user.id, now).await?;
        tx.commit().await?;
        return Ok(());
    };
    let decision_now = Utc::now().timestamp();
    if user.expired_at <= decision_now {
        disable_auto_renewal(&mut tx, user.id, decision_now).await?;
        tx.commit().await?;
        return Ok(());
    }
    let Some(terms) = renewal_terms(&user, &plan, &latest_period, decision_now) else {
        disable_auto_renewal(&mut tx, user.id, decision_now).await?;
        tx.commit().await?;
        return Ok(());
    };

    let trade_no = v2board_domain::order::generate_order_no();
    let updated = sqlx::query(RENEWAL_UPDATE_SQL)
        .bind(terms.price)
        .bind(terms.expired_at)
        .bind(decision_now)
        .bind(user.id)
        .bind(user.plan_id)
        .bind(user.expired_at)
        .bind(terms.price)
        .bind(decision_now)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() != 1 {
        anyhow::bail!("auto-renew user changed while its row lock was held");
    }
    sqlx::query(
        r#"
        INSERT INTO orders
            (user_id, plan_id, "type", period, trade_no, total_amount, balance_amount, status, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, 0, $6, $7, $8, $9)
        "#,
    )
    .bind(user.id)
    .bind(plan.id)
    .bind(OrderKind::Renewal.code())
    .bind(latest_period)
    .bind(trade_no)
    .bind(terms.price)
    .bind(OrderState::Completed.code())
    .bind(decision_now)
    .bind(decision_now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RenewalTerms {
    price: i32,
    expired_at: i64,
}

fn renewal_terms(
    user: &RenewalUserRow,
    plan: &RenewalPlanRow,
    period: &str,
    now: i64,
) -> Option<RenewalTerms> {
    let period = order_period_from_storage(period)?;
    let balance = NonNegativeMoneyMinor::new(user.balance).ok()?;
    let decision = decide_renewal(RenewalRequest {
        now,
        current_expiry: user.expired_at,
        balance,
        plan_allows_renewal: plan.renew,
        period,
        plan_price: raw_renewal_price(plan, period).map(MoneyMinor::from_i32),
    });
    match decision {
        RenewalDecision::Renew {
            debit,
            extension_base,
            months,
        } => Some(RenewalTerms {
            price: debit.get(),
            expired_at: add_period(extension_base, months)?,
        }),
        RenewalDecision::Disable(_) => None,
    }
}

/// The plan's price (in cents) for a recurring renewal period. Returns `Some(0)`
/// when the column is NULL or zero — Laravel's CheckRenewal compares
/// `balance < $plan[$period]` where a NULL price coerces to 0, so a free/unpriced
/// period auto-renews at no cost rather than disabling auto-renewal. Only a period
/// that is not a recognized recurring key yields `None` (cannot be renewed).
#[cfg(test)]
fn renewal_price(plan: &RenewalPlanRow, period: &str) -> Option<i32> {
    let period = order_period_from_storage(period)?;
    period.recurring_months()?;
    Some(raw_renewal_price(plan, period).unwrap_or(0))
}

fn raw_renewal_price(plan: &RenewalPlanRow, period: OrderPeriod) -> Option<i32> {
    match period.plan_period()? {
        PlanPricePeriod::Month => plan.month_price,
        PlanPricePeriod::Quarter => plan.quarter_price,
        PlanPricePeriod::HalfYear => plan.half_year_price,
        PlanPricePeriod::Year => plan.year_price,
        PlanPricePeriod::TwoYear => plan.two_year_price,
        PlanPricePeriod::ThreeYear => plan.three_year_price,
        PlanPricePeriod::OneTime | PlanPricePeriod::Reset => None,
    }
}

async fn disable_auto_renewal(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    now: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE users SET auto_renewal = 0, updated_at = $1 WHERE id = $2 AND auto_renewal <> 0",
    )
    .bind(now)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn add_period(timestamp: i64, months: u32) -> Option<i64> {
    app_timezone()
        .timestamp_opt(timestamp, 0)
        .single()?
        .checked_add_months(Months::new(months))
        .map(|date| date.timestamp())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renewal_price_treats_null_or_zero_as_free_not_disable() {
        let plan = RenewalPlanRow {
            id: 1,
            renew: true,
            month_price: None,      // unpriced period
            quarter_price: Some(0), // explicitly free
            half_year_price: Some(1000),
            year_price: None,
            two_year_price: None,
            three_year_price: None,
        };
        // Laravel free-renews (balance < NULL/0 is false) instead of disabling.
        assert_eq!(renewal_price(&plan, "month_price"), Some(0));
        assert_eq!(renewal_price(&plan, "quarter_price"), Some(0));
        assert_eq!(renewal_price(&plan, "half_year_price"), Some(1000));
        // A non-recurring period cannot be auto-renewed.
        assert_eq!(renewal_price(&plan, "reset_price"), None);
        assert_eq!(renewal_price(&plan, "onetime_price"), None);
    }

    #[test]
    fn renewal_adapter_disables_at_the_exact_expiry_boundary() {
        let now = 1_700_000_000;
        let user = RenewalUserRow {
            id: 7,
            balance: 1_000,
            plan_id: 3,
            expired_at: now,
        };
        let plan = RenewalPlanRow {
            id: 3,
            renew: true,
            month_price: Some(1_000),
            quarter_price: None,
            half_year_price: None,
            year_price: None,
            two_year_price: None,
            three_year_price: None,
        };
        assert_eq!(renewal_terms(&user, &plan, "month_price", now), None);
    }

    #[test]
    fn renewal_adapter_turns_a_null_recurring_price_into_a_free_renewal() {
        let now = 1_700_000_000;
        let user = RenewalUserRow {
            id: 7,
            balance: 0,
            plan_id: 3,
            expired_at: now + 86_400,
        };
        let plan = RenewalPlanRow {
            id: 3,
            renew: true,
            month_price: None,
            quarter_price: None,
            half_year_price: None,
            year_price: None,
            two_year_price: None,
            three_year_price: None,
        };
        let terms = renewal_terms(&user, &plan, "month_price", now)
            .expect("a NULL recurring price keeps the established free-renewal policy");
        assert_eq!(terms.price, 0);
        assert_eq!(terms.expired_at, add_period(user.expired_at, 1).unwrap());
    }

    #[test]
    fn renewal_terms_never_allow_negative_balance_or_negative_price() {
        let user = RenewalUserRow {
            id: 7,
            balance: 999,
            plan_id: 3,
            expired_at: Utc::now().timestamp() + 86_400,
        };
        let plan = RenewalPlanRow {
            id: 3,
            renew: true,
            month_price: Some(1_000),
            quarter_price: Some(-1),
            half_year_price: None,
            year_price: None,
            two_year_price: None,
            three_year_price: None,
        };
        let now = Utc::now().timestamp();
        assert_eq!(renewal_terms(&user, &plan, "month_price", now), None);
        assert_eq!(renewal_terms(&user, &plan, "quarter_price", now), None);

        let funded = RenewalUserRow {
            balance: 1_000,
            ..user
        };
        assert_eq!(
            renewal_terms(&funded, &plan, "month_price", now).map(|terms| terms.price),
            Some(1_000)
        );
        let disabled = RenewalPlanRow {
            renew: false,
            ..plan
        };
        assert_eq!(renewal_terms(&funded, &disabled, "month_price", now), None);
    }
}
