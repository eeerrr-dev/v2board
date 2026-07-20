use std::collections::HashMap;

use chrono::Utc;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use sqlx::{FromRow, Postgres, Transaction};
use v2board_config::AppConfig;
use v2board_domain_model::{
    CommissionInviter, CommissionState, NonNegativeMoneyMinor,
    plan_commission_payouts as decide_commission_payouts,
};

use crate::{state::WorkerState, time::timestamp_before};

const COMMISSION_AUTO_CHECK_BATCH_SIZE: i64 = 1_000;
const COMMISSION_AUTO_CHECK_MAX_BATCHES: usize = 20;
const COMMISSION_MAX_PAYOUTS_PER_TICK: usize = 10_000;
const COMMISSION_AUTO_CHECK_SQL: &str = r#"
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

#[derive(Debug, Clone, FromRow)]
struct InviterRow {
    id: i64,
    invite_user_id: Option<i64>,
}

pub(crate) async fn run(state: &WorkerState) -> anyhow::Result<()> {
    if state.config.commission_auto_check_enable {
        let now = Utc::now().timestamp();
        let cutoff = timestamp_before(now, 3 * 86_400);
        for _ in 0..COMMISSION_AUTO_CHECK_MAX_BATCHES {
            let marked = sqlx::query(COMMISSION_AUTO_CHECK_SQL)
                .bind(now)
                .bind(cutoff)
                .bind(COMMISSION_AUTO_CHECK_BATCH_SIZE)
                .execute(&state.db)
                .await?
                .rows_affected();
            if marked < COMMISSION_AUTO_CHECK_BATCH_SIZE as u64 {
                break;
            }
        }
    }

    let mut after_id = 0_i64;
    let mut processed = 0_usize;
    while processed < COMMISSION_MAX_PAYOUTS_PER_TICK {
        let mut tx = state.db.begin().await?;
        let order = sqlx::query_as::<_, CommissionOrderRow>(COMMISSION_CLAIM_SQL)
            .bind(after_id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(order) = order else {
            tx.commit().await?;
            break;
        };
        after_id = order.id;
        processed += 1;
        if let Err(error) = pay_commission_order_in_tx(state, &mut tx, &order).await {
            tx.rollback().await?;
            tracing::error!(
                trade_no = order.trade_no,
                ?error,
                "commission payment failed"
            );
            // Do not let one corrupt invite chain starve all later ready orders in
            // this run. Its status remains 1 and the next scheduled run retries it.
            continue;
        }
        tx.commit().await?;
    }
    Ok(())
}

async fn pay_commission_order_in_tx(
    state: &WorkerState,
    tx: &mut Transaction<'_, Postgres>,
    order: &CommissionOrderRow,
) -> anyhow::Result<()> {
    let shares = commission_shares(&state.config);

    // Prefetch the invite chain (bounded by the number of share levels) so the payout
    // walk itself is decided by the pure `plan_commission_payouts` port of payHandle.
    // The linear chain is a superset of the inviters the walk can reach, because the
    // pointer only advances after a real payout.
    let mut chain: HashMap<i64, InviterRow> = HashMap::new();
    let mut cursor = Some(order.invite_user_id);
    for _ in 0..shares.len() {
        let Some(id) = cursor else {
            break;
        };
        if chain.contains_key(&id) {
            break;
        }
        let Some(row) = sqlx::query_as::<_, InviterRow>(
            "SELECT id, invite_user_id FROM users WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        else {
            break;
        };
        cursor = row.invite_user_id;
        chain.insert(id, row);
    }

    let commission_pool = NonNegativeMoneyMinor::new(order.commission_balance)
        .map_err(|error| anyhow::anyhow!("invalid commission pool: {error}"))?;
    let payouts = decide_commission_payouts(&shares, commission_pool, order.invite_user_id, |id| {
        chain.get(&id).map(|inviter| CommissionInviter {
            id: inviter.id,
            inviter_id: inviter.invite_user_id,
        })
    });

    let mut actual_commission_balance = order.actual_commission_balance.unwrap_or_default();
    for payout in &payouts {
        let payout_amount = payout.amount.get();
        let next_actual_commission_balance =
            checked_commission_total(actual_commission_balance, payout_amount).ok_or_else(
                || anyhow::anyhow!("actual commission balance exceeds supported cents"),
            )?;
        let now = Utc::now().timestamp();
        let (balance, commission_balance): (i32, i32) = sqlx::query_as(
            "SELECT balance, commission_balance FROM users WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(payout.inviter_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("commission recipient no longer exists"))?;
        if state.config.withdraw_close_enable {
            let balance = checked_commission_total(balance, payout_amount)
                .ok_or_else(|| anyhow::anyhow!("recipient balance exceeds supported cents"))?;
            sqlx::query("UPDATE users SET balance = $1, updated_at = $2 WHERE id = $3")
                .bind(balance)
                .bind(now)
                .bind(payout.inviter_id)
                .execute(&mut **tx)
                .await?;
        } else {
            let commission_balance = checked_commission_total(commission_balance, payout_amount)
                .ok_or_else(|| {
                    anyhow::anyhow!("recipient commission balance exceeds supported cents")
                })?;
            sqlx::query("UPDATE users SET commission_balance = $1, updated_at = $2 WHERE id = $3")
                .bind(commission_balance)
                .bind(now)
                .bind(payout.inviter_id)
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query(
            r#"
            INSERT INTO commission_log
                (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(payout.inviter_id)
        .bind(order.user_id)
        .bind(&order.trade_no)
        .bind(order.total_amount)
        .bind(payout_amount)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
        actual_commission_balance = next_actual_commission_balance;
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
    .bind(Utc::now().timestamp())
    .bind(order.id)
    .bind(CommissionState::Processing.code())
    .execute(&mut **tx)
    .await?;
    if completed.rows_affected() != 1 {
        anyhow::bail!("commission order claim was lost");
    }
    Ok(())
}

fn checked_commission_total(current: i32, payout: i32) -> Option<i32> {
    // The native schema and the one-shot importer intentionally preserve a
    // signed historical account balance.  The domain decision guarantees the
    // payout itself is non-negative; applying it must therefore accept a
    // negative starting balance while still rejecting INTEGER overflow.
    NonNegativeMoneyMinor::new(payout).ok()?;
    current.checked_add(payout)
}

fn commission_shares(config: &AppConfig) -> Vec<i32> {
    if !config.commission_distribution_enable {
        return vec![100];
    }
    // CheckCommission.php:85-93 builds a fixed 3-level array with `(int)` casts, so an
    // unset/NULL/non-numeric level becomes 0 while still occupying its slot. Dropping it
    // (as a filter would) would shift later shares onto the wrong inviter.
    vec![
        parse_share(config.commission_distribution_l1.as_deref()),
        parse_share(config.commission_distribution_l2.as_deref()),
        parse_share(config.commission_distribution_l3.as_deref()),
    ]
}

fn parse_share(value: Option<&str>) -> i32 {
    value
        .map(str::trim)
        .and_then(|value| value.parse::<Decimal>().ok())
        .and_then(|value| value.trunc().to_i32())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use v2board_domain_model::CommissionPayout;

    use super::*;

    #[test]
    fn commission_claim_is_ordered_locked_and_non_blocking_across_workers() {
        assert!(COMMISSION_AUTO_CHECK_SQL.contains("WITH candidates AS"));
        assert!(COMMISSION_AUTO_CHECK_SQL.contains("LIMIT $3"));
        assert!(COMMISSION_AUTO_CHECK_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(COMMISSION_CLAIM_SQL.contains("ORDER BY id"));
        assert!(COMMISSION_CLAIM_SQL.contains("FOR UPDATE SKIP LOCKED"));
        let finalize = include_str!("../../../migrations-postgres/0002_import_finalize.sql");
        assert!(
            finalize.contains("CREATE INDEX idx_commission_claim ON orders(commission_status, id)")
        );
    }

    fn inviter(id: i64, invited_by: Option<i64>) -> CommissionInviter {
        CommissionInviter {
            id,
            inviter_id: invited_by,
        }
    }

    fn three_level_chain() -> HashMap<i64, CommissionInviter> {
        // 1 (invited by 2) -> 2 (invited by 3) -> 3 (top of the chain).
        [
            (1, inviter(1, Some(2))),
            (2, inviter(2, Some(3))),
            (3, inviter(3, None)),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn commission_zero_share_does_not_advance_invite_chain() {
        let chain = three_level_chain();
        // shares [0, 50, 0]: level 0 pays nobody but must NOT advance the pointer, so the
        // direct inviter (id 1) is the one paid at level 1's 50% share. Level 2's 0 share
        // again does not advance. Mirrors CheckCommission::payHandle.
        let payouts = decide_commission_payouts(
            &[0, 50, 0],
            NonNegativeMoneyMinor::new(100).unwrap(),
            1,
            |id| chain.get(&id).copied(),
        );
        assert_eq!(
            payouts,
            vec![CommissionPayout {
                inviter_id: 1,
                amount: NonNegativeMoneyMinor::new(50).unwrap(),
            }]
        );
    }

    #[test]
    fn commission_positive_shares_walk_up_the_chain() {
        let chain = three_level_chain();
        let payouts = decide_commission_payouts(
            &[50, 30, 20],
            NonNegativeMoneyMinor::new(100).unwrap(),
            1,
            |id| chain.get(&id).copied(),
        );
        assert_eq!(
            payouts,
            vec![
                CommissionPayout {
                    inviter_id: 1,
                    amount: NonNegativeMoneyMinor::new(50).unwrap(),
                },
                CommissionPayout {
                    inviter_id: 2,
                    amount: NonNegativeMoneyMinor::new(30).unwrap(),
                },
                CommissionPayout {
                    inviter_id: 3,
                    amount: NonNegativeMoneyMinor::new(20).unwrap(),
                },
            ]
        );
    }

    #[test]
    fn commission_single_full_share_pays_direct_inviter() {
        // The distribution-disabled path produces shares = [100].
        let chain = three_level_chain();
        let payouts =
            decide_commission_payouts(&[100], NonNegativeMoneyMinor::new(250).unwrap(), 1, |id| {
                chain.get(&id).copied()
            });
        assert_eq!(
            payouts,
            vec![CommissionPayout {
                inviter_id: 1,
                amount: NonNegativeMoneyMinor::new(250).unwrap(),
            }]
        );
    }

    #[test]
    fn commission_share_rounds_half_away_without_floats() {
        let chain = three_level_chain();
        assert_eq!(
            decide_commission_payouts(&[50], NonNegativeMoneyMinor::new(1).unwrap(), 1, |id| chain
                .get(&id)
                .copied()),
            vec![CommissionPayout {
                inviter_id: 1,
                amount: NonNegativeMoneyMinor::new(1).unwrap(),
            }]
        );
    }

    #[test]
    fn commission_total_accepts_signed_current_and_rejects_invalid_payout_or_overflow() {
        assert_eq!(checked_commission_total(100, 20), Some(120));
        assert_eq!(checked_commission_total(-100, 20), Some(-80));
        assert_eq!(checked_commission_total(i32::MIN, 1), Some(i32::MIN + 1));
        assert_eq!(checked_commission_total(i32::MAX, 1), None);
        assert_eq!(checked_commission_total(i32::MIN, -1), None);
    }

    #[test]
    fn parse_share_coerces_missing_and_non_numeric_to_zero() {
        // Matches PHP `(int)config(...)`: NULL/absent and unparseable become 0, and the
        // slot is preserved so later levels are not shifted.
        assert_eq!(parse_share(None), 0);
        assert_eq!(parse_share(Some("")), 0);
        assert_eq!(parse_share(Some("abc")), 0);
        assert_eq!(parse_share(Some(" 40 ")), 40);
        assert_eq!(parse_share(Some("50.5")), 50);
        assert_eq!(parse_share(Some("-50.5")), -50);
        assert_eq!(parse_share(Some("9999999999999999999999999999")), 0);
    }
}
