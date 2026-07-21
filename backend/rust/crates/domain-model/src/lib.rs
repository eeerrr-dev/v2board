//! Pure business concepts shared by application services and adapters.
//!
//! This crate deliberately has no database, cache, HTTP, configuration, or
//! async-runtime dependencies. Infrastructure translates at its boundary;
//! business vocabulary and invariant checks live here.

mod admin_permission;
mod commission;
mod content;
mod coupon;
mod giftcard;
mod money;
mod order;
mod plan;
mod renewal;
mod server;
mod subscription;
mod ticket;

pub use admin_permission::{
    ADMIN_PERMISSION_FAMILIES, AdminPathAccess, admin_path_access, is_registered_permission,
    staff_permissions_allow,
};
pub use commission::{
    CommissionEligibility, CommissionInviter, CommissionPayout, commission_is_eligible,
    order_commission_amount, plan_commission_payouts,
};
pub use content::{
    ContentVisibility, KnowledgeAccess, KnowledgeTemplateValues, render_knowledge_body,
};
pub use coupon::{Coupon, CouponKind, CouponRuleViolation, CouponUseContext, validate_coupon};
pub use giftcard::{
    GiftCardKind, GiftCardPlanSnapshot, GiftCardRedemptionMutation, GiftCardRuleViolation,
    GiftCardSnapshot, GiftCardUserSnapshot, PlanBindingMutation, PreparedGiftCardRedemption,
    checked_add_cents, checked_add_giftcard_days, checked_gib_bytes, giftcard_plan_has_capacity,
    prepare_gift_card_redemption, validate_gift_card_window_and_limit,
};
pub use money::{MoneyMinor, MoneyMinorError, NonNegativeMoneyMinor};
pub use order::{CommissionState, OrderKind, OrderPeriod, OrderState};
pub use plan::{
    PLAN_FORCE_UPDATE_MAX_USERS, PlanInputViolation, PlanPricePeriod, PlanPriceUpdate,
    PlanPriceUpdates, PlanPrices, normalize_plan_sort_ids, plan_transfer_bytes,
    validate_plan_capacity_limit, validate_plan_device_limit, validate_plan_name,
    validate_plan_reset_traffic_method, validate_plan_speed_limit, validate_plan_transfer_enable,
};
pub use renewal::{RenewalDecision, RenewalDisableReason, RenewalRequest, decide_renewal};
pub use server::{
    ServerInputViolation, ServerKind, ServerRouteAction, canonical_server_group_ids,
    filter_server_route_matches, server_available_status, validate_server_port,
    validate_server_route_matches,
};
pub use subscription::{
    CalendarDay, CalendarDayError, NewPeriodError, NewPeriodWindow, ScheduledTrafficResetPolicy,
    SubscriptionAvailability, TrafficResetFacts, TrafficResetMethod,
    checked_reset_subscription_expiry, scheduled_traffic_reset_due,
};
pub use ticket::{
    TicketCreationPolicy, TicketInputViolation, TicketLevel, TicketReplyStatus, TicketStatus,
    commission_balance_meets_minimum, validate_operator_ticket_message,
    validate_ticket_create_input, validate_ticket_message, validate_ticket_subject,
    validate_withdrawal_input,
};
