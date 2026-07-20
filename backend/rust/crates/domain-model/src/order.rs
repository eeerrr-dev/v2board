use crate::PlanPricePeriod;

/// The period attached to an order. Deposits are intentionally separate from
/// plan prices instead of being represented by another magic string.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OrderPeriod {
    Plan(PlanPricePeriod),
    Deposit,
}

impl OrderPeriod {
    pub const fn plan_period(self) -> Option<PlanPricePeriod> {
        match self {
            Self::Plan(period) => Some(period),
            Self::Deposit => None,
        }
    }

    pub const fn recurring_months(self) -> Option<u32> {
        match self {
            Self::Plan(period) => period.recurring_months(),
            Self::Deposit => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OrderKind {
    NewSubscription,
    Renewal,
    PlanChange,
    TrafficReset,
    BalanceDeposit,
}

impl OrderKind {
    /// The stable business code exposed on order wires and retained by native
    /// persistence. It is not a database implementation detail.
    pub const ALL: [Self; 5] = [
        Self::NewSubscription,
        Self::Renewal,
        Self::PlanChange,
        Self::TrafficReset,
        Self::BalanceDeposit,
    ];

    pub const fn code(self) -> i32 {
        match self {
            Self::NewSubscription => 1,
            Self::Renewal => 2,
            Self::PlanChange => 3,
            Self::TrafficReset => 4,
            Self::BalanceDeposit => 9,
        }
    }

    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(Self::NewSubscription),
            2 => Some(Self::Renewal),
            3 => Some(Self::PlanChange),
            4 => Some(Self::TrafficReset),
            9 => Some(Self::BalanceDeposit),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OrderState {
    Pending,
    /// Payment is accepted and fulfilment is still in progress (`status = 1`,
    /// shown as “activating” by the admin surface).
    Activating,
    Cancelled,
    Completed,
    Credited,
}

impl OrderState {
    /// The stable business code shared by the order API and native storage.
    pub const ALL: [Self; 5] = [
        Self::Pending,
        Self::Activating,
        Self::Cancelled,
        Self::Completed,
        Self::Credited,
    ];

    pub const fn code(self) -> i16 {
        match self {
            Self::Pending => 0,
            Self::Activating => 1,
            Self::Cancelled => 2,
            Self::Completed => 3,
            Self::Credited => 4,
        }
    }

    pub const fn from_code(code: i16) -> Option<Self> {
        match code {
            0 => Some(Self::Pending),
            1 => Some(Self::Activating),
            2 => Some(Self::Cancelled),
            3 => Some(Self::Completed),
            4 => Some(Self::Credited),
            _ => None,
        }
    }

    pub const fn can_customer_cancel(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn can_settle(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn is_fulfilled(self) -> bool {
        matches!(self, Self::Completed | Self::Credited)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommissionState {
    Pending,
    Processing,
    Paid,
    Rejected,
}

impl CommissionState {
    /// The stable business code shared by commission APIs and native storage.
    pub const ALL: [Self; 4] = [Self::Pending, Self::Processing, Self::Paid, Self::Rejected];

    pub const fn code(self) -> i16 {
        match self {
            Self::Pending => 0,
            Self::Processing => 1,
            Self::Paid => 2,
            Self::Rejected => 3,
        }
    }

    pub const fn from_code(code: i16) -> Option<Self> {
        match code {
            0 => Some(Self::Pending),
            1 => Some(Self::Processing),
            2 => Some(Self::Paid),
            3 => Some(Self::Rejected),
            _ => None,
        }
    }

    pub const fn can_begin_payment(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn can_complete_payment(self) -> bool {
        matches!(self, Self::Processing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_period_does_not_conflate_deposit_and_plan_prices() {
        assert_eq!(OrderPeriod::Deposit.plan_period(), None);
        assert_eq!(
            OrderPeriod::Plan(PlanPricePeriod::Quarter).recurring_months(),
            Some(3)
        );
        assert_eq!(
            OrderPeriod::Plan(PlanPricePeriod::Reset).recurring_months(),
            None
        );
    }

    #[test]
    fn state_capabilities_are_explicit() {
        assert!(OrderState::Pending.can_customer_cancel());
        assert!(!OrderState::Activating.can_customer_cancel());
        assert!(OrderState::Completed.is_fulfilled());
        assert!(CommissionState::Pending.can_begin_payment());
        assert!(!CommissionState::Paid.can_begin_payment());
    }

    #[test]
    fn stable_business_codes_are_exhaustive_and_round_trip() {
        assert_eq!(OrderKind::ALL.map(OrderKind::code), [1_i32, 2, 3, 4, 9]);
        for kind in OrderKind::ALL {
            assert_eq!(OrderKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(OrderKind::from_code(0), None);
        assert_eq!(OrderKind::from_code(5), None);

        assert_eq!(OrderState::ALL.map(OrderState::code), [0_i16, 1, 2, 3, 4]);
        for state in OrderState::ALL {
            assert_eq!(OrderState::from_code(state.code()), Some(state));
        }
        assert_eq!(OrderState::from_code(-1), None);
        assert_eq!(OrderState::from_code(5), None);

        assert_eq!(
            CommissionState::ALL.map(CommissionState::code),
            [0_i16, 1, 2, 3]
        );
        for state in CommissionState::ALL {
            assert_eq!(CommissionState::from_code(state.code()), Some(state));
        }
        assert_eq!(CommissionState::from_code(-1), None);
        assert_eq!(CommissionState::from_code(4), None);
    }
}
