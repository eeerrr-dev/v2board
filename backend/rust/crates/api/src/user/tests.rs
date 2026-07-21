use chrono::Utc;
use v2board_application::service_usage::ServiceServer;
use v2board_domain_model::{
    checked_add_cents, checked_add_giftcard_days, checked_gib_bytes, giftcard_plan_has_capacity,
};

use crate::subscription_adapters::{reset_day, reset_day_by_month_first_day};

use super::{
    invite::checked_transfer_balances,
    stats::{ServerBody, server_body},
};

#[test]
fn reset_day_returns_none_for_plan_less_user_ignoring_config_default() {
    // A month-first-day default would otherwise compute a non-null day.
    let future = Utc::now().timestamp() + 30 * 86_400;

    // Plan-less: getResetDay returns null at the `plan_id === NULL` guard, never
    // the config default.
    assert_eq!(
        reset_day(Some(future), None, 0, Utc::now().timestamp()),
        None
    );
    // A resolved plan whose own method is NULL still uses the config default.
    assert_eq!(
        reset_day(Some(future), Some(None), 0, Utc::now().timestamp()),
        Some(reset_day_by_month_first_day())
    );
    // method 2 (no reset) is null even with a plan.
    assert_eq!(
        reset_day(Some(future), Some(Some(2)), 0, Utc::now().timestamp()),
        None
    );
    // Missing / past expiry is null.
    assert_eq!(reset_day(None, Some(None), 0, Utc::now().timestamp()), None);
    assert_eq!(
        reset_day(Some(1), Some(None), 0, Utc::now().timestamp()),
        None
    );
}

#[test]
fn reset_day_by_expire_day_clamps_to_the_short_month_end_under_a_frozen_clock() {
    use chrono::TimeZone;
    // Expiry anniversary on the 31st (Asia/Shanghai calendar).
    let expired_at = v2board_config::app_timezone()
        .with_ymd_and_hms(2026, 3, 31, 10, 0, 0)
        .single()
        .expect("valid Shanghai timestamp")
        .timestamp();

    // Frozen at UTC Feb 27 20:00 = Shanghai Feb 28: February has no 31st, so
    // the clamped reset day is today (0 days out) on the month's last day.
    {
        let _clock =
            v2board_config::freeze_time(Utc.with_ymd_and_hms(2026, 2, 27, 20, 0, 0).unwrap());
        assert_eq!(
            reset_day(
                Some(expired_at),
                Some(Some(1)),
                0,
                v2board_config::now_utc().timestamp(),
            ),
            Some(0)
        );
    }
    // Frozen mid-February: 8 days until the clamped Feb 28 reset.
    {
        let _clock =
            v2board_config::freeze_time(Utc.with_ymd_and_hms(2026, 2, 19, 20, 0, 0).unwrap());
        assert_eq!(
            reset_day(
                Some(expired_at),
                Some(Some(1)),
                0,
                v2board_config::now_utc().timestamp(),
            ),
            Some(8)
        );
    }
    // Frozen after the March anniversary date has passed (Shanghai Apr 1 —
    // but expiry itself is behind us now): expired subscriptions have no
    // reset day at all.
    let _clock = v2board_config::freeze_time(Utc.with_ymd_and_hms(2026, 3, 31, 16, 0, 0).unwrap());
    assert_eq!(
        reset_day(
            Some(expired_at),
            Some(Some(1)),
            0,
            v2board_config::now_utc().timestamp(),
        ),
        None
    );
}

fn server_row_fixture() -> ServiceServer {
    ServiceServer {
        id: 1,
        parent_id: None,
        group_ids: vec![1],
        route_ids: None,
        name: "Node".to_string(),
        rate: 1.5,
        kind: "shadowsocks".to_string(),
        host: "node.example.test".to_string(),
        port: 443,
        cache_key: "shadowsocks-1-0-1".to_string(),
        last_check_at: Some(1_700_000_000),
        online: true,
        tags: None,
        sort: None,
        extra_json: None,
    }
}

/// docs/api-dialect.md §5.4 (W6): the modern server row is numeric
/// `rate`/`port` and boolean `is_online`; the legacy free-text VARCHAR
/// columns must never leak a non-numeric string onto the wire.
#[test]
fn server_body_enforces_the_numeric_wire_contract() {
    let body = server_body(server_row_fixture()).unwrap();
    let ServerBody::Shadowsocks { server, extra } = body else {
        panic!("fixture must project to the shadowsocks variant");
    };
    assert!(extra.is_none());
    assert_eq!(server.rate, 1.5);
    assert_eq!(server.port, 443);
    assert!(server.is_online);
    assert_eq!(
        server.last_check_at,
        Some(v2board_api_contract::time::Rfc3339Timestamp::from_epoch_seconds(1_700_000_000,))
    );

    let mut offline = server_row_fixture();
    offline.online = false;
    let body = server_body(offline).unwrap();
    let ServerBody::Shadowsocks { server, .. } = body else {
        panic!("fixture must project to the shadowsocks variant");
    };
    assert!(!server.is_online);
}

#[test]
fn cents_addition_rejects_balance_overflow() {
    assert_eq!(checked_add_cents(10, 20).unwrap(), 30);
    assert!(checked_add_cents(i32::MAX, 1).is_err());
}

#[test]
fn commission_transfer_checks_both_balance_columns() {
    assert_eq!(checked_transfer_balances(100, 200, 25).unwrap(), (75, 225));
    assert!(checked_transfer_balances(10, 200, 25).is_err());
    assert!(checked_transfer_balances(100, i32::MAX, 1).is_err());
    assert!(checked_transfer_balances(100, 200, -1).is_err());
}

#[test]
fn giftcard_redemption_depends_on_the_canonical_code_index() {
    // The case-insensitive `lower(code)` gift-card redemption lookup depends on
    // the canonical unique index that forbids case-variant duplicate codes.
    let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
    assert!(finalize.contains("uniq_gift_card_code_canonical"));
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

/// `legacy()`/`bad_request()` keep the byte-identical `{message}` contract
/// for the frozen §2 external namespaces (payment notify's uniform 500
/// `fail`, notify body parsing 400s). The W14 teardown removed every
/// internal constructor of these bodies.
#[tokio::test]
async fn frozen_external_errors_keep_the_legacy_message_body() {
    use axum::response::IntoResponse as _;

    let response =
        v2board_compat::ApiError::bad_request("Invalid payment notify body").into_response();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "message": "Invalid payment notify body" })
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
