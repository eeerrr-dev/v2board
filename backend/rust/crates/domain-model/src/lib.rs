//! Pure business concepts shared by application services and adapters.
//!
//! This crate deliberately has no database, cache, HTTP, configuration, or
//! async-runtime dependencies. Infrastructure translates at its boundary;
//! business vocabulary and invariant checks live here.

mod commission;
mod money;
mod order;
mod plan;
mod renewal;
mod subscription;

pub use commission::{
    CommissionEligibility, CommissionInviter, CommissionPayout, commission_is_eligible,
    order_commission_amount, plan_commission_payouts,
};
pub use money::{MoneyMinor, MoneyMinorError, NonNegativeMoneyMinor};
pub use order::{CommissionState, OrderKind, OrderPeriod, OrderState};
pub use plan::{PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices};
pub use renewal::{RenewalDecision, RenewalDisableReason, RenewalRequest, decide_renewal};
pub use subscription::{
    CalendarDay, CalendarDayError, NewPeriodError, NewPeriodWindow, ScheduledTrafficResetPolicy,
    SubscriptionAvailability, TrafficResetFacts, TrafficResetMethod,
    checked_reset_subscription_expiry, scheduled_traffic_reset_due,
};
