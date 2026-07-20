use crate::{MoneyMinorError, NonNegativeMoneyMinor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommissionEligibility {
    ConfigurableFirstPurchase,
    Always,
    FirstPurchaseOnly,
}

pub const fn commission_is_eligible(
    policy: CommissionEligibility,
    first_purchase_only: bool,
    buyer_has_completed_order: bool,
) -> bool {
    match policy {
        CommissionEligibility::ConfigurableFirstPurchase => {
            !first_purchase_only || !buyer_has_completed_order
        }
        CommissionEligibility::Always => true,
        CommissionEligibility::FirstPurchaseOnly => !buyer_has_completed_order,
    }
}

/// Compute the commission attached to an order in integer minor units.
/// Fractional units round halfway away from zero, matching the persistence
/// contract, while the intermediate uses `i128` so overflow is detected.
pub fn order_commission_amount(
    order_total: i64,
    inviter_rate: Option<i32>,
    default_rate: i32,
) -> Result<i32, MoneyMinorError> {
    let rate = inviter_rate
        .filter(|rate| *rate != 0)
        .unwrap_or(default_rate);
    let numerator = i128::from(order_total) * i128::from(rate);
    let mut amount = numerator / 100;
    let remainder = numerator % 100;
    if remainder.unsigned_abs() * 2 >= 100 {
        amount += numerator.signum();
    }
    i32::try_from(amount).map_err(|_| {
        MoneyMinorError::OutOfRange(if amount.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommissionInviter {
    pub id: i64,
    pub inviter_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommissionPayout {
    pub inviter_id: i64,
    pub amount: NonNegativeMoneyMinor,
}

/// Plan the established multi-level payout walk. A zero/negative share keeps
/// the cursor on the same inviter; only a real payout advances it.
pub fn plan_commission_payouts<F>(
    shares: &[i32],
    commission_pool: NonNegativeMoneyMinor,
    first_inviter: i64,
    mut lookup: F,
) -> Vec<CommissionPayout>
where
    F: FnMut(i64) -> Option<CommissionInviter>,
{
    let mut inviter_id = Some(first_inviter);
    let mut payouts = Vec::new();
    for &share in shares {
        let Some(current) = inviter_id else {
            break;
        };
        let Some(inviter) = lookup(current) else {
            break;
        };
        if share <= 0 {
            continue;
        }
        let numerator = i64::from(commission_pool.get()) * i64::from(share);
        if numerator == 0 {
            continue;
        }
        let mut amount = numerator / 100;
        let remainder = numerator % 100;
        if remainder.unsigned_abs() * 2 >= 100 {
            amount += numerator.signum();
        }
        // Preserve the established saturation policy at the amount-column
        // boundary. The subsequent checked account total can still reject it.
        let amount = i32::try_from(amount).unwrap_or(i32::MAX);
        payouts.push(CommissionPayout {
            inviter_id: inviter.id,
            amount: NonNegativeMoneyMinor::new(amount)
                .expect("positive share of a non-negative pool is non-negative"),
        });
        inviter_id = inviter.inviter_id;
    }
    payouts
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn chain() -> BTreeMap<i64, CommissionInviter> {
        [
            (
                1,
                CommissionInviter {
                    id: 1,
                    inviter_id: Some(2),
                },
            ),
            (
                2,
                CommissionInviter {
                    id: 2,
                    inviter_id: Some(3),
                },
            ),
            (
                3,
                CommissionInviter {
                    id: 3,
                    inviter_id: None,
                },
            ),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn eligibility_policies_are_named_and_exhaustive() {
        assert!(commission_is_eligible(
            CommissionEligibility::ConfigurableFirstPurchase,
            false,
            true
        ));
        assert!(!commission_is_eligible(
            CommissionEligibility::FirstPurchaseOnly,
            true,
            true
        ));
        assert!(commission_is_eligible(
            CommissionEligibility::Always,
            true,
            true
        ));
    }

    #[test]
    fn order_commission_rounds_half_away_and_checks_range() {
        assert_eq!(order_commission_amount(5, Some(10), 0).unwrap(), 1);
        assert_eq!(order_commission_amount(-5, Some(10), 0).unwrap(), -1);
        assert_eq!(order_commission_amount(10_000, Some(0), 10).unwrap(), 1_000);
        assert!(order_commission_amount(i64::MAX, Some(100), 0).is_err());
    }

    #[test]
    fn payout_walk_preserves_zero_share_cursor_semantics() {
        let chain = chain();
        let payouts = plan_commission_payouts(
            &[0, 50, 20],
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
                    amount: NonNegativeMoneyMinor::new(20).unwrap(),
                },
            ]
        );
    }
}
