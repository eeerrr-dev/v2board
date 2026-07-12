use chrono::Utc;
use v2board_config::AppConfig;

use super::{
    giftcard::{
        GIFTCARD_FOR_UPDATE_SQL, GIFTCARD_USER_ORDER_RANGE_SQL, checked_add_cents,
        checked_add_giftcard_days, checked_gib_bytes, giftcard_plan_has_capacity,
    },
    invite::{checked_pagination_values, checked_transfer_balances, validate_pagination},
    subscription::{checked_reset_subscription_expiry, reset_day, reset_day_by_month_first_day},
};

fn reset_day_plan_fixture(reset_traffic_method: Option<i16>) -> v2board_db::plan::PlanRow {
    v2board_db::plan::PlanRow {
        id: 1,
        group_id: 1,
        transfer_enable: 0,
        device_limit: None,
        name: "p".to_string(),
        speed_limit: None,
        show: 1,
        sort: None,
        renew: 1,
        content: None,
        month_price: None,
        quarter_price: None,
        half_year_price: None,
        year_price: None,
        two_year_price: None,
        three_year_price: None,
        onetime_price: None,
        reset_price: None,
        reset_traffic_method,
        capacity_limit: None,
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn reset_day_returns_none_for_plan_less_user_ignoring_config_default() {
    let mut config = AppConfig::from_api_env();
    // A month-first-day default would otherwise compute a non-null day.
    config.reset_traffic_method = 0;
    let future = Utc::now().timestamp() + 30 * 86_400;

    // Plan-less: getResetDay returns null at the `plan_id === NULL` guard, never
    // the config default.
    assert_eq!(reset_day(Some(future), None, &config), None);
    // A resolved plan whose own method is NULL still uses the config default.
    let null_method = reset_day_plan_fixture(None);
    assert_eq!(
        reset_day(Some(future), Some(&null_method), &config),
        Some(reset_day_by_month_first_day())
    );
    // method 2 (no reset) is null even with a plan.
    let no_reset = reset_day_plan_fixture(Some(2));
    assert_eq!(reset_day(Some(future), Some(&no_reset), &config), None);
    // Missing / past expiry is null.
    assert_eq!(reset_day(None, Some(&null_method), &config), None);
    assert_eq!(reset_day(Some(1), Some(&null_method), &config), None);
}

#[test]
fn cents_addition_rejects_balance_overflow() {
    assert_eq!(checked_add_cents(10, 20, "overflow").unwrap(), 30);
    assert!(checked_add_cents(i32::MAX, 1, "overflow").is_err());
}

#[test]
fn reset_subscription_expiry_math_rejects_timestamp_overflow() {
    assert_eq!(
        checked_reset_subscription_expiry(100 * 86_400, 30, 30, 0).unwrap(),
        Some(70 * 86_400)
    );
    assert_eq!(
        checked_reset_subscription_expiry(31 * 86_400, 30, 30, 0).unwrap(),
        None
    );
    assert!(checked_reset_subscription_expiry(i64::MAX, 30, 30, i64::MIN).is_err());
    // An already-expired timestamp safely short-circuits before a reset is applied; the huge
    // reset duration is irrelevant on this branch and is not itself a timestamp overflow.
    assert_eq!(
        checked_reset_subscription_expiry(i64::MIN, i64::MAX, 30, 0).unwrap(),
        None
    );
    // A future expiry reaches the reset calculation and rejects the unrepresentable duration.
    assert!(checked_reset_subscription_expiry(i64::MAX, i64::MAX, 30, 0).is_err());
    assert!(checked_reset_subscription_expiry(100, 1, i64::MAX, 0).is_err());
}

#[test]
fn commission_transfer_checks_both_balance_columns() {
    assert_eq!(checked_transfer_balances(100, 200, 25).unwrap(), (75, 225));
    assert!(checked_transfer_balances(10, 200, 25).is_err());
    assert!(checked_transfer_balances(100, i32::MAX, 1).is_err());
    assert!(checked_transfer_balances(100, 200, -1).is_err());
}

#[test]
fn giftcard_redemption_serializes_on_the_card_row() {
    assert!(GIFTCARD_FOR_UPDATE_SQL.trim_end().ends_with("FOR UPDATE"));
    assert!(GIFTCARD_FOR_UPDATE_SQL.contains("lower(code) = lower($1)"));
    let migration = include_str!("../../../../migrations-postgres/0001_initial.sql");
    assert!(migration.contains("uniq_giftcard_code_canonical"));
}

#[test]
fn giftcard_mutations_lock_the_order_range_before_the_user() {
    assert!(GIFTCARD_USER_ORDER_RANGE_SQL.contains("status IN (0, 1)"));
    assert!(
        GIFTCARD_USER_ORDER_RANGE_SQL
            .trim_end()
            .ends_with("FOR UPDATE")
    );
    let source = include_str!("giftcard.rs");
    let range_lock = source
        .find("sqlx::query_scalar(GIFTCARD_USER_ORDER_RANGE_SQL)")
        .unwrap();
    let user_lock = source
        .find("FROM v2_user WHERE id = $1 LIMIT 1 FOR UPDATE")
        .unwrap();
    assert!(range_lock < user_lock);
}

#[test]
fn giftcard_units_reject_negative_values_and_integer_overflow() {
    assert_eq!(checked_gib_bytes(2).unwrap(), 2_147_483_648);
    assert!(checked_gib_bytes(-1).is_err());
    assert!(checked_gib_bytes(i64::MAX).is_err());
    assert_eq!(checked_add_giftcard_days(1_000, 2).unwrap(), 173_800);
    assert!(checked_add_giftcard_days(1_000, -1).is_err());
    assert!(checked_add_giftcard_days(i64::MAX, 1).is_err());
}

#[test]
fn plan_giftcards_consume_capacity_but_can_materialize_an_existing_reservation() {
    assert!(giftcard_plan_has_capacity(2, 1, false));
    assert!(!giftcard_plan_has_capacity(2, 2, false));
    assert!(giftcard_plan_has_capacity(2, 2, true));
    assert!(!giftcard_plan_has_capacity(-1, 0, false));
}

#[test]
fn pagination_is_bounded_and_never_overflows_the_offset() {
    assert_eq!(validate_pagination(None, None).unwrap(), (10, 0));
    assert_eq!(
        validate_pagination(Some("2"), Some("25")).unwrap(),
        (25, 25)
    );
    assert!(validate_pagination(Some("0"), Some("10")).is_err());
    assert!(validate_pagination(Some("1"), Some("101")).is_err());
    assert!(validate_pagination(Some("not-an-integer"), Some("10")).is_err());
    assert!(validate_pagination(Some("9223372036854775807"), Some("100")).is_err());
    assert!(checked_pagination_values(i64::MAX, 100).is_err());
}
