use chrono::{Months, TimeZone, Utc};
use sqlx::{FromRow, Postgres, Transaction};
use v2board_config::app_timezone;

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

#[derive(Debug, Clone, FromRow)]
struct RenewalPlanRow {
    id: i32,
    renew: i16,
    month_price: Option<i32>,
    quarter_price: Option<i32>,
    half_year_price: Option<i32>,
    year_price: Option<i32>,
    two_year_price: Option<i32>,
    three_year_price: Option<i32>,
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
    let Some(plan) = sqlx::query_as::<_, RenewalPlanRow>(
        r#"
        SELECT id, renew, month_price, quarter_price, half_year_price, year_price,
               two_year_price, three_year_price
        FROM plan
        WHERE id = $1
        LIMIT 1
        FOR SHARE
        "#,
    )
    .bind(user.plan_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
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
    let Some(terms) = renewal_terms(&user, &plan, &latest_period) else {
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
        VALUES ($1, $2, 2, $3, $4, 0, $5, 3, $6, $7)
        "#,
    )
    .bind(user.id)
    .bind(plan.id)
    .bind(latest_period)
    .bind(trade_no)
    .bind(terms.price)
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
) -> Option<RenewalTerms> {
    if plan.renew == 0 {
        return None;
    }
    let price = renewal_price(plan, period)?;
    if price < 0 || user.balance.checked_sub(price)? < 0 {
        return None;
    }
    Some(RenewalTerms {
        price,
        expired_at: add_period(user.expired_at, period)?,
    })
}

/// The plan's price (in cents) for a recurring renewal period. Returns `Some(0)`
/// when the column is NULL or zero — Laravel's CheckRenewal compares
/// `balance < $plan[$period]` where a NULL price coerces to 0, so a free/unpriced
/// period auto-renews at no cost rather than disabling auto-renewal. Only a period
/// that is not a recognized recurring key yields `None` (cannot be renewed).
fn renewal_price(plan: &RenewalPlanRow, period: &str) -> Option<i32> {
    match period {
        "month_price" => Some(plan.month_price.unwrap_or(0)),
        "quarter_price" => Some(plan.quarter_price.unwrap_or(0)),
        "half_year_price" => Some(plan.half_year_price.unwrap_or(0)),
        "year_price" => Some(plan.year_price.unwrap_or(0)),
        "two_year_price" => Some(plan.two_year_price.unwrap_or(0)),
        "three_year_price" => Some(plan.three_year_price.unwrap_or(0)),
        _ => None,
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

fn add_period(timestamp: i64, period: &str) -> Option<i64> {
    let months = match period {
        "month_price" => 1,
        "quarter_price" => 3,
        "half_year_price" => 6,
        "year_price" => 12,
        "two_year_price" => 24,
        "three_year_price" => 36,
        _ => return None,
    };
    let base = if timestamp < Utc::now().timestamp() {
        Utc::now().timestamp()
    } else {
        timestamp
    };
    app_timezone()
        .timestamp_opt(base, 0)
        .single()?
        .checked_add_months(Months::new(months))
        .map(|date| date.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renewal_price_treats_null_or_zero_as_free_not_disable() {
        let plan = RenewalPlanRow {
            id: 1,
            renew: 1,
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
    fn renewal_terms_never_allow_negative_balance_or_negative_price() {
        let user = RenewalUserRow {
            id: 7,
            balance: 999,
            plan_id: 3,
            expired_at: Utc::now().timestamp() + 86_400,
        };
        let plan = RenewalPlanRow {
            id: 3,
            renew: 1,
            month_price: Some(1_000),
            quarter_price: Some(-1),
            half_year_price: None,
            year_price: None,
            two_year_price: None,
            three_year_price: None,
        };
        assert_eq!(renewal_terms(&user, &plan, "month_price"), None);
        assert_eq!(renewal_terms(&user, &plan, "quarter_price"), None);

        let funded = RenewalUserRow {
            balance: 1_000,
            ..user
        };
        assert_eq!(
            renewal_terms(&funded, &plan, "month_price").map(|terms| terms.price),
            Some(1_000)
        );
        let disabled = RenewalPlanRow { renew: 0, ..plan };
        assert_eq!(renewal_terms(&funded, &disabled, "month_price"), None);
    }

    #[test]
    fn renewal_sql_rechecks_locked_state_and_guards_the_deduction() {
        assert!(RENEWAL_CANDIDATE_SQL.trim_start().starts_with("SELECT id"));
        assert!(RENEWAL_CANDIDATE_SQL.contains("id > $3"));
        assert!(RENEWAL_CANDIDATE_SQL.contains("ORDER BY id"));
        assert!(RENEWAL_CANDIDATE_SQL.contains("LIMIT $4"));
        assert!(RENEWAL_LOCKED_USER_SQL.contains("FOR UPDATE"));
        assert!(RENEWAL_LOCKED_USER_SQL.contains("auto_renewal <> 0"));
        assert!(RENEWAL_LOCKED_USER_SQL.contains("expired_at > $2"));
        assert!(RENEWAL_UPDATE_SQL.contains("plan_id = $5"));
        assert!(RENEWAL_UPDATE_SQL.contains("expired_at = $6"));
        assert!(RENEWAL_UPDATE_SQL.contains("balance >= $7"));
        assert!(RENEWAL_UPDATE_SQL.contains("expired_at > $8"));

        let source = include_str!("renewal.rs");
        let function = &source[source.find("async fn renew_user").unwrap()..];
        assert!(
            function.find("SELECT period").unwrap()
                < function.find("RENEWAL_LOCKED_USER_SQL").unwrap()
        );
    }
}
