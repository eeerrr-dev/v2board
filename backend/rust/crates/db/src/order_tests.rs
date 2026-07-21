use crate::plan::PlanRow;
use rust_decimal::Decimal;
use serde_json::json;
use v2board_domain_model::OrderKind;

use crate::{
    order_lifecycle::*,
    order_runtime::{DraftOrder, GIB, OrderForCheckout, PaymentForCheckout, UserForOrder},
};

fn fixture_payment(method: &str, config: serde_json::Value) -> PaymentForCheckout {
    PaymentForCheckout {
        id: 1,
        payment: method.to_string(),
        enable: 1,
        uuid: "payment-uuid".to_string(),
        config: config.to_string(),
        notify_domain: None,
        handling_fee_fixed: None,
        handling_fee_percent: None,
    }
}

fn draft_fixture(total_amount: i32) -> DraftOrder {
    DraftOrder {
        user_id: 1,
        plan_id: 1,
        coupon_id: None,
        kind: 1,
        period: "month_price".to_string(),
        trade_no: "test".to_string(),
        total_amount: Decimal::from(total_amount),
        discount_amount: None,
        surplus_amount: None,
        refund_amount: None,
        balance_amount: None,
        surplus_order_ids: None,
        invite_user_id: None,
        commission_balance: Decimal::ZERO,
    }
}

#[test]
fn order_event_reset_decodes_known_kinds_and_keeps_unknown_codes_as_noops() {
    assert!(order_event_resets_traffic(
        OrderKind::NewSubscription.code(),
        1,
        0,
        0
    ));
    assert!(order_event_resets_traffic(
        OrderKind::Renewal.code(),
        0,
        1,
        0
    ));
    assert!(order_event_resets_traffic(
        OrderKind::PlanChange.code(),
        0,
        0,
        1
    ));
    assert!(!order_event_resets_traffic(
        OrderKind::TrafficReset.code(),
        1,
        1,
        1
    ));
    assert!(!order_event_resets_traffic(
        OrderKind::BalanceDeposit.code(),
        1,
        1,
        1
    ));
    for unknown in [i32::MIN, -1, 0, 5, 8, i32::MAX] {
        assert!(!order_event_resets_traffic(unknown, 1, 1, 1));
    }
}

#[test]
fn vip_and_coupon_discount_defer_rounding_to_persist() {
    // total=1990, coupon 33% (656.7) then VIP 15% (298.5). Laravel defers
    // persistence rounding:
    // discount_amount = 656.7 + 298.5 = 955.2 -> persist 955; total = 1990 -
    // 955.2 = 1034.8 -> persist 1035. Rounding each portion first would drift to
    // 956 / 1034, so the rounding must be deferred to persist time.
    let mut draft = draft_fixture(1990);
    draft.discount_amount = Some(Decimal::from(1990) * percent(33));
    apply_vip_discount(Some(15), &mut draft);
    assert_eq!(round_cents(draft.discount_amount.unwrap()).unwrap(), 955);
    assert_eq!(round_cents(draft.total_amount).unwrap(), 1035);
}

#[test]
fn no_coupon_no_vip_leaves_total_untouched_and_discount_null() {
    // Laravel's setVipDiscount leaves discount_amount NULL and total unchanged
    // when neither a coupon nor a VIP discount applies.
    let mut draft = draft_fixture(1990);
    apply_vip_discount(None, &mut draft);
    assert!(draft.discount_amount.is_none());
    assert_eq!(round_cents(draft.total_amount).unwrap(), 1990);
}

#[test]
fn monetary_boundaries_keep_deferred_half_away_rounding() {
    assert_eq!(round_cents(Decimal::new(5, 1)).unwrap(), 1);
    assert_eq!(round_cents(Decimal::new(-5, 1)).unwrap(), -1);
    assert!(round_cents(Decimal::MAX).is_err());

    let mut payment = fixture_payment("Epay", json!({}));
    payment.handling_fee_percent = Some(Decimal::new(125, 2));
    assert_eq!(
        calculate_handling_amount_cents(200, &payment).unwrap(),
        Some(3)
    );
    payment.handling_fee_fixed = Some(3);
    assert_eq!(
        calculate_handling_amount_cents(199, &payment).unwrap(),
        Some(5)
    );
}

#[test]
fn handling_fee_math_rejects_negative_legacy_configuration_and_overflow() {
    let mut payment = fixture_payment("Epay", json!({}));
    payment.handling_fee_fixed = Some(-1);
    assert!(calculate_handling_amount_cents(1_000, &payment).is_err());

    payment.handling_fee_fixed = Some(0);
    payment.handling_fee_percent = Some(Decimal::new(-1, 0));
    assert!(calculate_handling_amount_cents(1_000, &payment).is_err());

    payment.handling_fee_percent = Some(Decimal::MAX);
    assert!(calculate_handling_amount_cents(i32::MAX, &payment).is_err());
}

#[test]
fn commission_is_eligible_mirrors_setinvite_switch() {
    // type 0: gated by first-time config AND whether the buyer already ordered.
    assert!(commission_is_eligible(0, false, true)); // gating off -> always pay
    assert!(commission_is_eligible(0, true, false)); // gating on, first order
    assert!(!commission_is_eligible(0, true, true)); // gating on, repeat buyer
    // type 1: always pay regardless of history.
    assert!(commission_is_eligible(1, true, true));
    // type 2: first order only.
    assert!(commission_is_eligible(2, true, false));
    assert!(!commission_is_eligible(2, true, true));
    // unrecognized type: never pay.
    assert!(!commission_is_eligible(9, false, false));
}

#[test]
fn commission_amount_prefers_inviter_rate_then_global_default() {
    // Per-inviter rate wins when set.
    assert_eq!(
        commission_amount(Decimal::from(10_000), Some(25), 10),
        Decimal::from(2_500)
    );
    // Zero/None rate falls back to the global invite_commission default.
    assert_eq!(
        commission_amount(Decimal::from(10_000), Some(0), 10),
        Decimal::from(1_000)
    );
    assert_eq!(
        commission_amount(Decimal::from(10_000), None, 10),
        Decimal::from(1_000)
    );
    // Commission math stays unrounded here (a fractional-cent result survives);
    // insert_order rounds once at persist.
    assert_eq!(
        commission_amount(Decimal::from(333), Some(10), 10),
        Decimal::new(333, 1)
    );
}

#[test]
fn commission_amount_cents_rounds_exactly_and_rejects_overflow() {
    assert_eq!(
        v2board_domain_model::order_commission_amount(5, Some(10), 0).unwrap(),
        1
    );
    assert_eq!(
        v2board_domain_model::order_commission_amount(-5, Some(10), 0).unwrap(),
        -1
    );
    assert_eq!(
        v2board_domain_model::order_commission_amount(333, Some(10), 0).unwrap(),
        33
    );
    assert_eq!(
        v2board_domain_model::order_commission_amount(10_000, Some(0), 10).unwrap(),
        1_000
    );
    assert_eq!(
        v2board_domain_model::order_commission_amount(10_000, Some(-10), 0).unwrap(),
        -1_000
    );
    assert!(v2board_domain_model::order_commission_amount(i64::MAX, Some(100), 0).is_err());
}

// --- Order-open grant math (the paid -> plan/traffic/expiry side effect).
//     These pure functions decide expiry extension and reset-on-renew, which
//     Laravel's OrderService::buyByPeriod/buyByOneTime own; a regression here
//     would silently mis-grant subscriptions on every paid order. `now` is
//     injected so the assertions are deterministic and timezone-robust. ---

const GRANT_NOW: i64 = 1_700_000_000;

fn plan_grant_fixture() -> PlanRow {
    PlanRow {
        id: 7,
        group_id: 3,
        transfer_enable: 100, // GiB, multiplied by GIB inside the grant fns
        device_limit: Some(3),
        name: "Pro".to_string(),
        speed_limit: Some(1000),
        show: true,
        sort: Some(1),
        renew: true,
        content: None,
        month_price: Some(1000),
        quarter_price: Some(2700),
        half_year_price: None,
        year_price: Some(10000),
        two_year_price: None,
        three_year_price: None,
        onetime_price: Some(5000),
        reset_price: Some(500),
        reset_traffic_method: None,
        capacity_limit: None,
        created_at: 0,
        updated_at: 0,
    }
}

fn order_grant_fixture(order_type: i32, period: &str) -> OrderForCheckout {
    OrderForCheckout {
        id: 100,
        user_id: 1,
        plan_id: 7,
        kind: order_type,
        period: period.to_string(),
        trade_no: "T-GRANT".to_string(),
        total_amount: 1000,
        refund_amount: None,
        surplus_order_ids: None,
    }
}

fn user_grant_fixture() -> UserForOrder {
    UserForOrder {
        id: 1,
        invite_user_id: None,
        balance: 0,
        discount: None,
        commission_type: 0,
        commission_rate: None,
        traffic_epoch: 0,
        u: 9 * GIB,
        d: GIB,
        transfer_enable: 50 * GIB,
        device_limit: Some(1),
        banned: 0,
        group_id: Some(1),
        plan_id: Some(1),
        speed_limit: Some(100),
        expired_at: None,
    }
}

#[test]
fn new_order_forces_reset_and_grants_full_plan() {
    // type 1 (new) always resets traffic even if the user still had time left,
    // and grants the plan's transfer/device/group.
    let mut user = user_grant_fixture();
    user.expired_at = Some(GRANT_NOW + 86_400 * 40);
    let plan = plan_grant_fixture();
    buy_by_period(
        &mut user,
        &order_grant_fixture(1, "month_price"),
        &plan,
        "month_price",
        GRANT_NOW,
    )
    .unwrap();
    assert_eq!(user.u, 0);
    assert_eq!(user.d, 0);
    assert_eq!(user.transfer_enable, 100 * GIB);
    assert_eq!(user.device_limit, Some(3));
    assert_eq!(user.plan_id, Some(7));
    assert_eq!(user.group_id, Some(3));
    // Extended one month from the still-future expiry, not from now.
    assert_eq!(
        user.expired_at,
        Some(add_months(GRANT_NOW + 86_400 * 40, 1))
    );
}

#[test]
fn renew_preserves_traffic_when_not_same_month_day() {
    // type 2 (renew) with an expiry ~40 days out keeps used traffic and just
    // extends the period.
    let mut user = user_grant_fixture();
    user.expired_at = Some(GRANT_NOW + 86_400 * 40);
    buy_by_period(
        &mut user,
        &order_grant_fixture(2, "month_price"),
        &plan_grant_fixture(),
        "month_price",
        GRANT_NOW,
    )
    .unwrap();
    assert_eq!(user.u, 9 * GIB);
    assert_eq!(user.d, GIB);
    assert_eq!(user.transfer_enable, 100 * GIB);
    assert_eq!(
        user.expired_at,
        Some(add_months(GRANT_NOW + 86_400 * 40, 1))
    );
}

#[test]
fn renew_resets_traffic_on_same_month_day() {
    // Renewing on the exact expiry day (same month/day) resets traffic, per
    // OrderService::buyByPeriod's Carbon isSameDay branch.
    let mut user = user_grant_fixture();
    user.expired_at = Some(GRANT_NOW);
    buy_by_period(
        &mut user,
        &order_grant_fixture(2, "month_price"),
        &plan_grant_fixture(),
        "month_price",
        GRANT_NOW,
    )
    .unwrap();
    assert_eq!(user.u, 0);
    assert_eq!(user.d, 0);
    assert_eq!(user.expired_at, Some(add_months(GRANT_NOW, 1)));
}

#[test]
fn change_order_restarts_period_from_now_and_keeps_traffic() {
    // type 3 (change plan) drops the old expiry to now, then extends from now;
    // traffic is not reset by buy_by_period itself.
    let mut user = user_grant_fixture();
    user.expired_at = Some(GRANT_NOW + 86_400 * 100);
    buy_by_period(
        &mut user,
        &order_grant_fixture(3, "month_price"),
        &plan_grant_fixture(),
        "month_price",
        GRANT_NOW,
    )
    .unwrap();
    assert_eq!(user.u, 9 * GIB);
    assert_eq!(user.d, GIB);
    assert_eq!(user.expired_at, Some(add_months(GRANT_NOW, 1)));
}

#[test]
fn one_time_absorbs_leftover_traffic_when_no_expiry() {
    // buyByOneTime folds the user's unused traffic into the new allowance when
    // they have no active expiry and no surplus orders were consumed.
    let mut user = user_grant_fixture();
    user.expired_at = None;
    user.transfer_enable = 50 * GIB;
    user.u = 5 * GIB;
    user.d = 3 * GIB; // 42 GiB unused
    buy_by_one_time(&mut user, &plan_grant_fixture(), false).unwrap();
    assert_eq!(user.transfer_enable, 100 * GIB + 42 * GIB);
    assert_eq!(user.u, 0);
    assert_eq!(user.d, 0);
    assert_eq!(user.expired_at, None);
    assert_eq!(user.plan_id, Some(7));
}

#[test]
fn one_time_ignores_leftover_when_surplus_orders_consumed() {
    let mut user = user_grant_fixture();
    user.expired_at = None;
    user.transfer_enable = 50 * GIB;
    user.u = 5 * GIB;
    user.d = 3 * GIB;
    buy_by_one_time(&mut user, &plan_grant_fixture(), true).unwrap();
    assert_eq!(user.transfer_enable, 100 * GIB);
}

#[test]
fn plan_orders_reject_negative_prices_but_keep_zero_price_valid() {
    let mut plan = plan_grant_fixture();
    plan.month_price = Some(0);
    assert_eq!(
        crate::order_lifecycle::purchasable_period_price(&plan, "month_price").unwrap(),
        0
    );

    plan.month_price = Some(-1);
    assert!(crate::order_lifecycle::purchasable_period_price(&plan, "month_price").is_err());
    assert!(crate::order_lifecycle::purchasable_period_price(&plan, "half_year_price").is_err());
}

#[test]
fn grant_math_rejects_invalid_plan_traffic_without_mutating_the_user() {
    let mut user = user_grant_fixture();
    let original_transfer = user.transfer_enable;
    let original_u = user.u;
    let original_d = user.d;
    let mut plan = plan_grant_fixture();
    plan.transfer_enable = -1;

    assert!(
        buy_by_period(
            &mut user,
            &order_grant_fixture(1, "month_price"),
            &plan,
            "month_price",
            GRANT_NOW,
        )
        .is_err()
    );
    assert_eq!(user.transfer_enable, original_transfer);
    assert_eq!((user.u, user.d), (original_u, original_d));

    plan.transfer_enable = i64::MAX / GIB + 1;
    assert!(buy_by_one_time(&mut user, &plan, false).is_err());
    assert_eq!(user.transfer_enable, original_transfer);
    assert_eq!((user.u, user.d), (original_u, original_d));
}

#[test]
fn one_time_grant_rejects_used_leftover_and_allowance_overflow() {
    let plan = plan_grant_fixture();

    let mut used_overflow = user_grant_fixture();
    used_overflow.u = i64::MAX;
    used_overflow.d = 1;
    assert!(buy_by_one_time(&mut used_overflow, &plan, false).is_err());
    assert_eq!((used_overflow.u, used_overflow.d), (i64::MAX, 1));

    let mut leftover_overflow = user_grant_fixture();
    leftover_overflow.transfer_enable = i64::MAX;
    leftover_overflow.u = -1;
    leftover_overflow.d = 0;
    assert!(buy_by_one_time(&mut leftover_overflow, &plan, false).is_err());
    assert_eq!(leftover_overflow.u, -1);

    let mut addition_overflow = user_grant_fixture();
    addition_overflow.transfer_enable = GIB;
    addition_overflow.u = 0;
    addition_overflow.d = 0;
    let mut largest_plan = plan_grant_fixture();
    largest_plan.transfer_enable = i64::MAX / GIB;
    assert!(buy_by_one_time(&mut addition_overflow, &largest_plan, false).is_err());
    assert_eq!(addition_overflow.transfer_enable, GIB);
    assert_eq!((addition_overflow.u, addition_overflow.d), (0, 0));
}

#[test]
fn surplus_unused_traffic_math_is_checked_at_i64_boundaries() {
    let mut user = user_grant_fixture();
    user.transfer_enable = 50 * GIB;
    user.u = 5 * GIB;
    user.d = 3 * GIB;
    assert_eq!(
        crate::order_lifecycle::checked_unused_traffic(&user).unwrap(),
        42 * GIB
    );

    user.u = i64::MAX;
    user.d = 1;
    assert!(crate::order_lifecycle::checked_unused_traffic(&user).is_err());

    user.transfer_enable = i64::MAX;
    user.u = -1;
    user.d = 0;
    assert!(crate::order_lifecycle::checked_unused_traffic(&user).is_err());
}

#[test]
fn surplus_aggregate_and_duration_math_rejects_boundary_overflow() {
    assert_eq!(
        crate::order_lifecycle::checked_order_month_sum(12, 24).unwrap(),
        36
    );
    assert!(crate::order_lifecycle::checked_order_month_sum(u32::MAX, 1).is_err());

    assert_eq!(
        crate::order_lifecycle::checked_order_amount_sum(10, 100, Some(20), Some(30), Some(5))
            .unwrap(),
        155
    );
    assert!(
        crate::order_lifecycle::checked_order_amount_sum(i64::MAX, 1, None, None, None).is_err()
    );
    assert!(
        crate::order_lifecycle::checked_order_amount_sum(i64::MIN, 0, None, None, Some(1)).is_err()
    );

    assert_eq!(
        crate::order_lifecycle::checked_surplus_seconds(1_000, 400).unwrap(),
        600
    );
    assert!(crate::order_lifecycle::checked_surplus_seconds(i64::MAX, i64::MIN).is_err());
    assert!(crate::order_lifecycle::checked_surplus_seconds(i64::MIN, i64::MAX).is_err());

    assert_eq!(
        crate::order_lifecycle::checked_surplus_add_months(GRANT_NOW, 1).unwrap(),
        add_months(GRANT_NOW, 1)
    );
    assert!(crate::order_lifecycle::checked_surplus_add_months(GRANT_NOW, u32::MAX).is_err());

    assert!(crate::order_lifecycle::checked_surplus_mul(Decimal::MAX, Decimal::from(2)).is_err());
    assert!(crate::order_lifecycle::checked_surplus_add(Decimal::MAX, Decimal::from(1)).is_err());
    assert!(crate::order_lifecycle::checked_surplus_div(Decimal::from(1), Decimal::ZERO).is_err());
}

#[test]
fn add_period_time_floors_past_base_to_now_and_passes_through_unknown_period() {
    // A base in the past is clamped to now before adding the period.
    assert_eq!(
        add_period_time("month_price", GRANT_NOW - 999_999, GRANT_NOW),
        add_months(GRANT_NOW, 1)
    );
    // Non-calendar periods (e.g. deposit) return the clamped base unchanged.
    assert_eq!(
        add_period_time("deposit", GRANT_NOW - 5, GRANT_NOW),
        GRANT_NOW
    );
    assert_eq!(
        add_period_time("deposit", GRANT_NOW + 100, GRANT_NOW),
        GRANT_NOW + 100
    );
}

#[test]
fn capacity_slot_decision_treats_pending_reservations_as_consumed() {
    assert!(crate::order_lifecycle::capacity_has_slot(2, 1));
    assert!(!crate::order_lifecycle::capacity_has_slot(2, 2));
    assert!(!crate::order_lifecycle::capacity_has_slot(2, 3));
    assert!(!crate::order_lifecycle::capacity_has_slot(-1, 0));
}

#[test]
fn cent_addition_rejects_deposit_and_refund_overflow() {
    assert_eq!(
        crate::order_lifecycle::checked_add_cents(100, 25, "overflow").unwrap(),
        125
    );
    assert!(crate::order_lifecycle::checked_add_cents(i32::MAX, 1, "overflow").is_err());
}
