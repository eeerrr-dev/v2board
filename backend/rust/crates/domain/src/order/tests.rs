use serde_json::{Value, json};

use super::*;

mod gateway;
mod lifecycle_math;

fn fixture_payment(method: &str, mut config: Value) -> PaymentForCheckout {
    if method.starts_with("Stripe")
        && let Some(config) = config.as_object_mut()
    {
        config
            .entry("currency".to_string())
            .or_insert_with(|| json!("cny"));
    }
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

#[test]
fn every_settlement_requires_the_current_payment_binding() {
    let expected = ExpectedPaymentBinding {
        payment_id: 5,
        user_id: None,
        callback_no: None,
    };
    assert!(payment_binding_matches(&expected, 9, Some(5), None, None));
    assert!(!payment_binding_matches(&expected, 9, Some(6), None, None));
    assert!(!payment_binding_matches(&expected, 9, None, None, None));
}

#[test]
fn stripe_credit_settlement_keeps_user_and_exact_intent_binding() {
    let expected = ExpectedPaymentBinding {
        payment_id: 5,
        user_id: Some(9),
        callback_no: Some("pi_current".to_string()),
    };
    let current = payment_identifier_hash("pi_current");
    let superseded = payment_identifier_hash("pi_superseded");
    assert!(payment_binding_matches(
        &expected,
        9,
        Some(5),
        Some("pi_current"),
        Some(current.as_slice()),
    ));
    assert!(!payment_binding_matches(
        &expected,
        10,
        Some(5),
        Some("pi_current"),
        Some(current.as_slice()),
    ));
    assert!(!payment_binding_matches(
        &expected,
        9,
        Some(5),
        Some("pi_superseded"),
        Some(superseded.as_slice()),
    ));
}

#[test]
fn settlement_amount_includes_the_locked_orders_handling_fee() {
    assert!(payment_amount_matches(1_000, Some(234), 1_234));
    assert!(payment_amount_matches(1_234, None, 1_234));
    assert!(!payment_amount_matches(1_000, Some(234), 1_233));
    assert!(!payment_amount_matches(i64::MAX, Some(1), i64::MIN));
}

#[test]
fn callback_replay_and_second_provider_transaction_are_distinct() {
    let first = payment_identifier_hash("provider-tx-1");
    assert!(is_ordinary_payment_replay(
        1,
        Some("provider-tx-1"),
        Some(first.as_slice()),
        "provider-tx-1"
    ));
    assert!(!is_ordinary_payment_replay(
        1,
        Some("provider-tx-1"),
        Some(first.as_slice()),
        "provider-tx-2"
    ));
    assert!(!is_ordinary_payment_replay(
        2,
        Some("provider-tx-1"),
        Some(first.as_slice()),
        "provider-tx-1"
    ));
    assert!(!is_ordinary_payment_replay(0, None, None, "provider-tx-1"));
    assert!(is_ordinary_payment_replay(
        3,
        Some("provider-tx-1"),
        None,
        "provider-tx-1"
    ));
    let stale = payment_identifier_hash("old-provider-tx");
    assert!(!is_ordinary_payment_replay(
        3,
        Some("provider-tx-1"),
        Some(stale.as_slice()),
        "provider-tx-1"
    ));
    let long = "A".repeat(300);
    let long_hash = payment_identifier_hash(&long);
    assert!(!is_ordinary_payment_replay(
        3,
        Some(&"A".repeat(255)),
        Some(long_hash.as_slice()),
        &"A".repeat(255),
    ));
    assert!(!is_ordinary_payment_replay(
        3,
        Some("provider-tx-2"),
        Some(stale.as_slice()),
        "provider-tx-1"
    ));
}

#[test]
fn provider_identifiers_keep_bounded_utf8_labels_and_full_hashes() {
    let raw = format!("prefix-🚀{}", "界".repeat(100));
    let label = bounded_payment_identifier(&raw);
    assert!(label.len() <= 255);
    assert!(label.contains("\\u{1F680}"));
    assert!(!label.contains('🚀'));
    assert_eq!(payment_identifier_hash(&raw).len(), 32);
    let (audit_label, audit_hash) = bounded_payment_audit_identity(&raw);
    assert_eq!(audit_label, label);
    assert_eq!(audit_hash.len(), 64);
    assert!(audit_hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let migration = include_str!("../../../../migrations-postgres/0001_initial.sql");
    assert!(migration.contains("callback_no_hash"));
    assert!(migration.contains("octet_length(callback_no_hash) = 32"));
}

#[test]
fn late_payment_notification_is_idempotent_after_the_ledger_upsert() {
    assert!(should_emit_late_payment_notice(true));
    assert!(!should_emit_late_payment_notice(false));
    let migration = include_str!("../../../../migrations-postgres/0001_initial.sql");
    assert!(migration.contains("uniq_payment_reconciliation_callback"));
    assert!(migration.contains("reason"));
    let implementation = include_str!("settlement.rs");
    assert!(implementation.contains("ON CONFLICT (payment_id, callback_no_hash) DO UPDATE"));
    assert!(implementation.contains("occurrence_count + 1"));
    assert!(implementation.contains("RETURNING occurrence_count = 1"));
}

#[test]
fn payable_amount_rejects_overflow_and_non_positive_fees() {
    assert_eq!(payable_amount_cents(1_000, Some(234)).unwrap(), 1_234);
    assert!(payable_amount_cents(i32::MAX, Some(1)).is_err());
    assert!(payable_amount_cents(100, Some(-100)).is_err());
}

#[test]
fn settlement_query_locks_binding_and_amount_in_one_row_snapshot() {
    for required in [
        "total_amount",
        "handling_amount",
        "user_id",
        "payment_id",
        "callback_no_hash",
        "FOR UPDATE",
    ] {
        assert!(PAYMENT_SETTLEMENT_ORDER_SQL.contains(required));
    }
}

#[test]
fn callback_lookup_does_not_filter_out_a_disabled_in_flight_gateway() {
    assert!(PAYMENT_NOTIFY_LOOKUP_SQL.contains("payment = $1 AND uuid = $2"));
    assert!(!PAYMENT_NOTIFY_LOOKUP_SQL.contains("enable ="));
}

#[test]
fn payment_binding_rejects_archived_driver_or_config_snapshots_without_a_global_exclusive_lock() {
    assert!(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL.contains("FOR SHARE"));
    assert!(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL.contains("archived_at IS NULL"));
    assert!(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL.contains("enable = 1"));
    let expected = json!({"secret": "old", "nested": {"enabled": true}});
    assert!(payment_config_snapshot_matches(
        Some(("Coinbase".to_string(), expected.to_string())),
        "Coinbase",
        &expected,
    ));
    assert!(!payment_config_snapshot_matches(
        Some(("BTCPay".to_string(), expected.to_string())),
        "Coinbase",
        &expected,
    ));
    assert!(!payment_config_snapshot_matches(
        Some(("Coinbase".to_string(), json!({"secret": "new"}).to_string(),)),
        "Coinbase",
        &expected,
    ));
    assert!(!payment_config_snapshot_matches(
        None, "Coinbase", &expected,
    ));
}

#[test]
fn unfinished_order_invariant_has_both_lock_and_database_guard() {
    assert!(USER_FOR_ORDER_SQL.contains("FOR UPDATE"));
    assert!(UNFINISHED_ORDER_FOR_UPDATE_SQL.ends_with("FOR UPDATE"));
    let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
    assert!(finalize.contains(UNFINISHED_ORDER_UNIQUE_KEY));
    assert!(finalize.contains("status"));
    assert!(finalize.contains("user_id"));
    assert!(finalize.contains("0, 1"));
}

#[test]
fn pending_payment_guards_have_a_selective_database_index() {
    let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
    assert!(finalize.contains("idx_order_payment_status"));
    assert!(finalize.contains("payment_id"));
    assert!(finalize.contains("status"));
}

#[test]
fn capacity_reservation_query_counts_only_unmaterialized_slot_consumers() {
    let sql = v2board_db::plan::PLAN_CAPACITY_USAGE_SQL;
    assert!(sql.contains("COUNT(DISTINCT pending_order.user_id)"));
    assert!(sql.contains("pending_order.status IN (0, 1)"));
    assert!(sql.contains("pending_order.type IN (1, 3)"));
    assert!(sql.contains("NOT EXISTS"));
    assert!(sql.contains("reserved_user.plan_id = pending_order.plan_id"));
    assert!(
        sql.contains("reserved_user.expired_at >= EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT")
    );
}
