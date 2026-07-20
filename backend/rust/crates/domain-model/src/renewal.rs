use crate::{MoneyMinor, NonNegativeMoneyMinor, OrderPeriod};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenewalRequest {
    pub now: i64,
    pub current_expiry: i64,
    pub balance: NonNegativeMoneyMinor,
    pub plan_allows_renewal: bool,
    pub period: OrderPeriod,
    /// `None` deliberately means a free renewal. This preserves the established
    /// billing rule for an unpriced recurring period without hiding it in a DB
    /// row mapper.
    pub plan_price: Option<MoneyMinor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenewalDisableReason {
    PlanDisabled,
    SubscriptionExpired,
    PeriodNotRecurring,
    NegativePrice,
    InsufficientBalance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenewalDecision {
    Renew {
        debit: NonNegativeMoneyMinor,
        extension_base: i64,
        months: u32,
    },
    Disable(RenewalDisableReason),
}

pub fn decide_renewal(request: RenewalRequest) -> RenewalDecision {
    if !request.plan_allows_renewal {
        return RenewalDecision::Disable(RenewalDisableReason::PlanDisabled);
    }
    if request.current_expiry <= request.now {
        return RenewalDecision::Disable(RenewalDisableReason::SubscriptionExpired);
    }
    let Some(months) = request.period.recurring_months() else {
        return RenewalDecision::Disable(RenewalDisableReason::PeriodNotRecurring);
    };
    let price = match request.plan_price {
        Some(price) => match NonNegativeMoneyMinor::new(price.get()) {
            Ok(price) => price,
            Err(_) => return RenewalDecision::Disable(RenewalDisableReason::NegativePrice),
        },
        None => NonNegativeMoneyMinor::ZERO,
    };
    if request.balance.checked_sub(price).is_none() {
        return RenewalDecision::Disable(RenewalDisableReason::InsufficientBalance);
    }
    RenewalDecision::Renew {
        debit: price,
        extension_base: request.current_expiry.max(request.now),
        months,
    }
}

#[cfg(test)]
mod tests {
    use crate::PlanPricePeriod;

    use super::*;

    fn request() -> RenewalRequest {
        RenewalRequest {
            now: 100,
            current_expiry: 200,
            balance: NonNegativeMoneyMinor::new(1_000).unwrap(),
            plan_allows_renewal: true,
            period: OrderPeriod::Plan(PlanPricePeriod::Month),
            plan_price: Some(MoneyMinor::from_i32(1_000)),
        }
    }

    #[test]
    fn renewal_is_decided_without_clock_or_database_access() {
        assert_eq!(
            decide_renewal(request()),
            RenewalDecision::Renew {
                debit: NonNegativeMoneyMinor::new(1_000).unwrap(),
                extension_base: 200,
                months: 1,
            }
        );
    }

    #[test]
    fn null_price_is_explicitly_a_free_renewal() {
        assert!(matches!(
            decide_renewal(RenewalRequest {
                plan_price: None,
                ..request()
            }),
            RenewalDecision::Renew { debit, .. } if debit == NonNegativeMoneyMinor::ZERO
        ));
    }

    #[test]
    fn renewal_rejects_invalid_or_unaffordable_terms() {
        assert_eq!(
            decide_renewal(RenewalRequest {
                plan_price: Some(MoneyMinor::from_i32(-1)),
                ..request()
            }),
            RenewalDecision::Disable(RenewalDisableReason::NegativePrice)
        );
        assert_eq!(
            decide_renewal(RenewalRequest {
                balance: NonNegativeMoneyMinor::new(999).unwrap(),
                ..request()
            }),
            RenewalDecision::Disable(RenewalDisableReason::InsufficientBalance)
        );
        assert_eq!(
            decide_renewal(RenewalRequest {
                period: OrderPeriod::Plan(PlanPricePeriod::Reset),
                ..request()
            }),
            RenewalDecision::Disable(RenewalDisableReason::PeriodNotRecurring)
        );
    }
}
