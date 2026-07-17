use chrono::Utc;
use v2board_config::AppConfig;

use super::{
    giftcard::{
        GIFTCARD_FOR_UPDATE_SQL, GIFTCARD_USER_ORDER_RANGE_SQL, checked_add_cents,
        checked_add_giftcard_days, checked_gib_bytes, giftcard_plan_has_capacity,
    },
    invite::{checked_pagination_values, checked_transfer_balances, validate_pagination},
    stats::server_body,
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

fn server_row_fixture() -> v2board_db::server::AvailableServerRow {
    v2board_db::server::AvailableServerRow {
        id: 1,
        parent_id: None,
        group_id: vec![1],
        route_id: None,
        name: "Node".to_string(),
        rate: "1.5".to_string(),
        r#type: "shadowsocks".to_string(),
        host: "node.example.test".to_string(),
        port: serde_json::Value::from(443_i64),
        cache_key: "shadowsocks-1-0-1".to_string(),
        last_check_at: Some(1_700_000_000),
        is_online: 1,
        tags: None,
        sort: None,
        extra: serde_json::Value::Null,
    }
}

/// docs/api-dialect.md §5.4 (W6): the modern server row is numeric
/// `rate`/`port` and boolean `is_online`; the legacy free-text VARCHAR
/// columns must never leak a non-numeric string onto the wire.
#[test]
fn server_body_enforces_the_numeric_wire_contract() {
    let body = server_body(server_row_fixture()).unwrap();
    assert_eq!(body.rate, 1.5);
    assert_eq!(body.port, 443);
    assert!(body.is_online);
    assert_eq!(body.last_check_at, Some(1_700_000_000));

    // String-typed but numeric legacy port values still convert.
    let mut string_port = server_row_fixture();
    string_port.port = serde_json::Value::String(" 8443 ".to_string());
    string_port.is_online = 0;
    let body = server_body(string_port).unwrap();
    assert_eq!(body.port, 8443);
    assert!(!body.is_online);

    // Non-numeric operator values are internal errors, not string fallbacks.
    let mut bad_rate = server_row_fixture();
    bad_rate.rate = "fast".to_string();
    assert!(server_body(bad_rate).is_err());
    let mut nan_rate = server_row_fixture();
    nan_rate.rate = "NaN".to_string();
    assert!(server_body(nan_rate).is_err());
    let mut bad_port = server_row_fixture();
    bad_port.port = serde_json::Value::String("443,8443".to_string());
    assert!(server_body(bad_port).is_err());
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
    let baseline = include_str!("../../../../migrations-postgres/0001_initial.sql");
    let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
    assert!(finalize.contains("uniq_gift_card_code_canonical"));
    assert!(baseline.contains("created_at_provenance = 'legacy_unknown'"));
    assert!(baseline.contains("CREATE TABLE analytics_admission_policy"));
    assert!(baseline.contains("CREATE TABLE analytics_admission_state"));
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
        .find("FROM users WHERE id = $1 LIMIT 1 FOR UPDATE")
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

/// Deterministic business-rule failures on frontend-only routes are HTTP 400
/// with the same `{message}` body shape as the legacy 500, so the frontend
/// retry policy (>=500 is transient) never retries them. `legacy()` keeps the
/// byte-identical 500 contract for the external namespaces.
#[tokio::test]
async fn business_rule_failures_are_400_with_the_legacy_message_body() {
    use axum::response::IntoResponse as _;

    let response =
        v2board_compat::ApiError::business("Current product is sold out").into_response();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "message": "Current product is sold out" })
    );

    let response = v2board_compat::ApiError::legacy("fail").into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, serde_json::json!({ "message": "fail" }));
}
