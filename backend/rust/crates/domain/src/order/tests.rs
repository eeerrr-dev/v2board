use std::collections::BTreeMap;

use v2board_db::plan::PlanRow;

use super::payment_integrations::{
    CachedExchangeRate, EXCHANGE_RATE_FRESH_TTL_SECS, EXCHANGE_RATE_STALE_TTL_SECS,
    ExchangeRateCacheDecision, STRIPE_WEBHOOK_TOLERANCE_SECS, WechatUnifiedOrderResponse,
    add_stripe_settlement_metadata, alipay_f2f_notify, alipay_sign, bepusdt_notify,
    btcpay_invoice_settlement, btcpay_notify, canonical_query, coinbase_notify,
    coinpayments_notify, decimal_amount_cents, epay_notify, exchange_rate_cache_decision,
    form_query, hmac_sha256_hex, hmac_sha512_hex, mgate_notify, payment_http_client,
    reusable_stripe_credit_intent_matches, stripe_all_notify, stripe_checkout_notify,
    stripe_payment_amount, stripe_payment_intent_notify, stripe_source_charge_params,
    stripe_source_notify, url_origin, verify_legacy_md5_hex, wechat_pay_native_notify, wechat_sign,
    xml_from_params,
};
use super::*;

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

fn unwrap_verified(outcome: PaymentNotifyOutcome) -> VerifiedPaymentNotify {
    match outcome {
        PaymentNotifyOutcome::Verified(verified) => verified,
        PaymentNotifyOutcome::Ignored(body) => panic!("expected verified notify, got {body}"),
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
    let implementation = include_str!("../order.rs");
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
fn payment_provider_registry_matches_laravel_builtin_plugins() {
    assert_eq!(
        crate::payment_provider::payment_provider_codes(),
        vec![
            "AlipayF2F",
            "BEasyPaymentUSDT",
            "BTCPay",
            "CoinPayments",
            "Coinbase",
            "EPay",
            "MGate",
            "StripeALL",
            "StripeAlipay",
            "StripeCheckout",
            "StripeCredit",
            "StripeWepay",
            "WechatPayNative",
        ]
    );
}

#[test]
fn pay_and_notify_dispatch_cover_provider_registry() {
    for method in crate::payment_provider::payment_provider_codes() {
        assert!(
            require_payment_provider(method).is_ok(),
            "{method} is listed but checkout/notify dispatch cannot resolve it"
        );
    }
}

#[tokio::test]
async fn payment_http_client_rejects_plaintext_gateway_urls() {
    let result = payment_http_client("transport-test")
        .get("http://127.0.0.1:9/plaintext-is-forbidden")
        .send()
        .await;
    assert!(result.is_err());
}

#[test]
fn epay_notify_fixture_verifies_signature_and_extracts_trade_numbers() {
    let payment = fixture_payment("EPay", json!({ "key": "epay-secret" }));
    let mut signed = BTreeMap::from([
        ("money".to_string(), "12.34".to_string()),
        ("out_trade_no".to_string(), "T202607060001".to_string()),
        ("trade_no".to_string(), "EPAY-CALLBACK-1".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let sign = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), "epay-secret"))
    );
    signed.insert("sign".to_string(), sign);
    signed.insert("sign_type".to_string(), "MD5".to_string());
    let params = signed.into_iter().collect::<HashMap<_, _>>();

    let verified = unwrap_verified(epay_notify(&payment, &params).unwrap());

    assert_eq!(verified.trade_no, "T202607060001");
    assert_eq!(verified.callback_no, "EPAY-CALLBACK-1");
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn legacy_md5_signature_comparison_is_hex_strict_and_case_insensitive() {
    let digest = "00112233445566778899aabbccddeeff";
    assert!(verify_legacy_md5_hex(digest, digest));
    assert!(verify_legacy_md5_hex(
        digest,
        "00112233445566778899AABBCCDDEEFF"
    ));
    assert!(!verify_legacy_md5_hex(
        digest,
        "00112233445566778899aabbccddee00"
    ));
    assert!(!verify_legacy_md5_hex(digest, "not-hex"));
    assert!(!verify_legacy_md5_hex(digest, "0011"));
}

#[test]
fn mgate_notify_fixture_verifies_signature_and_extracts_trade_numbers() {
    let payment = fixture_payment("MGate", json!({ "mgate_app_secret": "mgate-secret" }));
    let mut signed = BTreeMap::from([
        ("out_trade_no".to_string(), "T202607060002".to_string()),
        ("status".to_string(), "success".to_string()),
        ("total_amount".to_string(), "1234".to_string()),
        ("trade_no".to_string(), "MGATE-CALLBACK-1".to_string()),
    ]);
    let sign = format!(
        "{:x}",
        md5::compute(format!(
            "{}{}",
            form_query(&signed).unwrap(),
            "mgate-secret"
        ))
    );
    signed.insert("sign".to_string(), sign);
    let params = signed.into_iter().collect::<HashMap<_, _>>();

    let verified = unwrap_verified(mgate_notify(&payment, &params).unwrap());

    assert_eq!(verified.trade_no, "T202607060002");
    assert_eq!(verified.callback_no, "MGATE-CALLBACK-1");
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn mgate_notify_rejects_a_signed_non_success_status() {
    let payment = fixture_payment("MGate", json!({ "mgate_app_secret": "mgate-secret" }));
    let mut signed = BTreeMap::from([
        ("out_trade_no".to_string(), "T202607060002".to_string()),
        ("status".to_string(), "failed".to_string()),
        ("total_amount".to_string(), "1234".to_string()),
        ("trade_no".to_string(), "MGATE-CALLBACK-2".to_string()),
    ]);
    let sign = format!(
        "{:x}",
        md5::compute(format!(
            "{}{}",
            form_query(&signed).unwrap(),
            "mgate-secret"
        ))
    );
    signed.insert("sign".to_string(), sign);
    let outcome = mgate_notify(&payment, &signed.into_iter().collect()).unwrap();
    assert!(matches!(outcome, PaymentNotifyOutcome::Ignored(_)));
}

#[test]
fn mgate_notify_keeps_statusless_legacy_callbacks_compatible() {
    let payment = fixture_payment("MGate", json!({ "mgate_app_secret": "mgate-secret" }));
    let mut signed = BTreeMap::from([
        ("out_trade_no".to_string(), "T202607060002".to_string()),
        ("total_amount".to_string(), "1234".to_string()),
        ("trade_no".to_string(), "MGATE-CALLBACK-LEGACY".to_string()),
    ]);
    let sign = format!(
        "{:x}",
        md5::compute(format!(
            "{}{}",
            form_query(&signed).unwrap(),
            "mgate-secret"
        ))
    );
    signed.insert("sign".to_string(), sign);
    let verified = unwrap_verified(mgate_notify(&payment, &signed.into_iter().collect()).unwrap());
    assert_eq!(verified.callback_no, "MGATE-CALLBACK-LEGACY");
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn bepusdt_notify_fixture_verifies_signature_and_returns_custom_ok_body() {
    let payment = fixture_payment(
        "BEasyPaymentUSDT",
        json!({ "bepusdt_apitoken": "bepusdt-secret" }),
    );
    let mut signed = BTreeMap::from([
        ("amount".to_string(), "12.34".to_string()),
        ("order_id".to_string(), "T202607060003".to_string()),
        ("status".to_string(), "2".to_string()),
        ("trade_id".to_string(), "BEPUSDT-CALLBACK-1".to_string()),
    ]);
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), "bepusdt-secret"))
    );
    signed.insert("signature".to_string(), signature);
    let params = signed.into_iter().collect::<HashMap<_, _>>();

    let verified = unwrap_verified(bepusdt_notify(&payment, &params).unwrap());

    assert_eq!(verified.trade_no, "T202607060003");
    assert_eq!(verified.callback_no, "BEPUSDT-CALLBACK-1");
    assert_eq!(verified.custom_result.as_deref(), Some("ok"));
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn stripe_checkout_notify_fixture_verifies_webhook_hmac() {
    let payment = fixture_payment(
        "StripeCheckout",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let body = json!({
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "payment_status": "paid",
                "client_reference_id": "T202607060004",
                "payment_intent": "pi_callback_1",
                "amount_total": 1234,
                "currency": "cny",
                "metadata": stripe_metadata_fixture("T202607060004", 1, 5678, 1234, "cny")
            }
        }
    })
    .to_string()
    .into_bytes();
    // Use a fresh timestamp so the webhook falls inside Stripe's 300s
    // replay tolerance regardless of when the suite runs.
    let timestamp = Utc::now().timestamp().to_string();
    let signature = stripe_test_signature(&timestamp, &body);
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1={signature}"),
        )]),
    };

    let verified = unwrap_verified(stripe_checkout_notify(&payment, &input).unwrap());

    assert_eq!(verified.trade_no, "T202607060004");
    assert_eq!(verified.callback_no, "pi_callback_1");
    assert_eq!(verified.authenticated_user_id, Some(9));
    assert_eq!(verified.settled_amount_cents, Some(5_678));
}

#[test]
fn stripe_checkout_notify_rejects_signed_amount_currency_or_binding_drift() {
    let payment = fixture_payment(
        "StripeCheckout",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let event = json!({
        "type": "checkout.session.completed",
        "data": { "object": {
            "payment_status": "paid",
            "client_reference_id": "T-CHECKOUT",
            "payment_intent": "pi_checkout",
            "amount_total": 1234,
            "currency": "cny",
            "metadata": stripe_metadata_fixture("T-CHECKOUT", 1, 5678, 1234, "cny")
        }}
    });
    for (path, value) in [
        ("/data/object/amount_total", json!(1)),
        ("/data/object/currency", json!("usd")),
        ("/data/object/metadata/expected_gateway_amount", json!("1")),
        ("/data/object/metadata/expected_amount", json!("1")),
        ("/data/object/metadata/expected_currency", json!("usd")),
        ("/data/object/metadata/payment_id", json!("2")),
        ("/data/object/metadata/user_id", json!("invalid")),
    ] {
        let mut drifted = event.clone();
        *drifted.pointer_mut(path).expect("fixture path") = value;
        assert!(stripe_checkout_notify(&payment, &stripe_signed_input(drifted)).is_err());
    }
    let mut coordinated_currency_drift = event.clone();
    coordinated_currency_drift["data"]["object"]["currency"] = json!("usd");
    coordinated_currency_drift["data"]["object"]["metadata"]["expected_currency"] = json!("usd");
    assert!(
        stripe_checkout_notify(&payment, &stripe_signed_input(coordinated_currency_drift),)
            .is_err()
    );
    let mut local_drift = event;
    local_drift["data"]["object"]["metadata"]["expected_local_amount"] = json!("1");
    let verified = unwrap_verified(
        stripe_checkout_notify(&payment, &stripe_signed_input(local_drift)).unwrap(),
    );
    assert_eq!(verified.settled_amount_cents, Some(1));
    assert!(!payment_amount_matches(
        5_678,
        None,
        verified.settled_amount_cents.unwrap(),
    ));
}

#[test]
fn stripe_checkout_notify_rejects_replayed_stale_webhook() {
    let payment = fixture_payment(
        "StripeCheckout",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let body = json!({
        "type": "checkout.session.completed",
        "data": { "object": {
            "payment_status": "paid",
            "client_reference_id": "T1",
            "payment_intent": "pi_1"
        } }
    })
    .to_string()
    .into_bytes();
    // A correctly-signed event whose timestamp is older than the tolerance
    // must be rejected as a replay.
    let stale = (Utc::now().timestamp() - STRIPE_WEBHOOK_TOLERANCE_SECS - 60).to_string();
    let signature = stripe_test_signature(&stale, &body);
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={stale},v1={signature}"),
        )]),
    };

    assert!(stripe_checkout_notify(&payment, &input).is_err());
}

#[test]
fn stripe_checkout_notify_rejects_a_future_dated_webhook() {
    let payment = fixture_payment(
        "StripeCheckout",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let body = json!({
        "type": "checkout.session.completed",
        "data": { "object": {
            "payment_status": "paid",
            "client_reference_id": "T1",
            "payment_intent": "pi_1"
        } }
    })
    .to_string()
    .into_bytes();
    let future = (Utc::now().timestamp() + STRIPE_WEBHOOK_TOLERANCE_SECS + 60).to_string();
    let signature = stripe_test_signature(&future, &body);
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={future},v1={signature}"),
        )]),
    };

    assert!(stripe_checkout_notify(&payment, &input).is_err());
}

fn stripe_test_signature(timestamp: &str, body: &[u8]) -> String {
    hmac_sha256_hex(
        b"whsec_test",
        format!("{timestamp}.")
            .as_bytes()
            .iter()
            .copied()
            .chain(body.iter().copied())
            .collect::<Vec<_>>()
            .as_slice(),
    )
    .unwrap()
}

fn stripe_metadata_fixture(
    trade_no: &str,
    payment_id: i32,
    local_amount: i64,
    gateway_amount: i64,
    currency: &str,
) -> Value {
    json!({
        "user_id": "9",
        "out_trade_no": trade_no,
        "payment_id": payment_id.to_string(),
        "expected_local_amount": local_amount.to_string(),
        "expected_gateway_amount": gateway_amount.to_string(),
        "expected_amount": gateway_amount.to_string(),
        "expected_currency": currency,
    })
}

fn stripe_signed_input(event: Value) -> PaymentNotifyInput {
    let body = event.to_string().into_bytes();
    let timestamp = Utc::now().timestamp().to_string();
    let signature = stripe_test_signature(&timestamp, &body);
    PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1={signature}"),
        )]),
    }
}

fn draft_fixture(total_amount: i32) -> DraftOrder {
    DraftOrder {
        user_id: 1,
        plan_id: 1,
        coupon_id: None,
        r#type: 1,
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
fn coupon_discount_values_fail_closed_for_legacy_invalid_rows() {
    assert!(super::lifecycle::validate_coupon_discount(1, 0).is_ok());
    assert!(super::lifecycle::validate_coupon_discount(1, i32::MAX).is_ok());
    assert!(super::lifecycle::validate_coupon_discount(1, -1).is_err());

    assert!(super::lifecycle::validate_coupon_discount(2, 0).is_ok());
    assert!(super::lifecycle::validate_coupon_discount(2, 100).is_ok());
    assert!(super::lifecycle::validate_coupon_discount(2, -1).is_err());
    assert!(super::lifecycle::validate_coupon_discount(2, 101).is_err());
    assert!(super::lifecycle::validate_coupon_discount(99, 10).is_err());
}

#[test]
fn coupon_lookup_preserves_legacy_case_insensitive_identity() {
    let source = include_str!("lifecycle.rs");
    assert!(source.contains("WHERE lower(code) = lower($1)"));
    let migration = include_str!("../../../../migrations-postgres/0001_initial.sql");
    assert!(migration.contains("uniq_coupon_code_canonical"));
}

#[test]
fn coupon_plan_scope_accepts_legacy_numeric_strings() {
    assert_eq!(
        super::lifecycle::parse_i32_json_list(Some(r#"["1",2,"invalid"]"#)),
        Some(vec![1, 2])
    );
}

#[test]
fn every_order_producer_can_reuse_the_bounded_trade_number_generator() {
    let trade_no = generate_order_no();
    assert_eq!(trade_no.len(), 25);
    assert!(trade_no.bytes().all(|byte| byte.is_ascii_digit()));
    assert!(trade_no.len() <= 36);
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
fn payment_url_and_wechat_xml_use_structured_codecs() {
    assert_eq!(
        url_origin("https://pay.example.com:8443/#/order/T1"),
        "https://pay.example.com:8443"
    );
    assert_eq!(url_origin("/#/order/T1"), "/#/order/T1");

    let params = BTreeMap::from([
        ("appid".to_string(), "wx-1".to_string()),
        ("body".to_string(), "A&B<C>".to_string()),
    ]);
    assert_eq!(
        xml_from_params(&params).unwrap(),
        "<xml><appid>wx-1</appid><body>A&amp;B&lt;C&gt;</body></xml>"
    );
    let response = quick_xml::de::from_str::<WechatUnifiedOrderResponse>(
        "<xml><return_code><![CDATA[SUCCESS]]></return_code><code_url>weixin://pay?a=1&amp;b=2</code_url></xml>",
    )
    .unwrap();
    assert_eq!(response.return_code, "SUCCESS");
    assert_eq!(response.code_url.as_deref(), Some("weixin://pay?a=1&b=2"));
}

fn unwrap_ignored(outcome: PaymentNotifyOutcome) -> String {
    match outcome {
        PaymentNotifyOutcome::Ignored(body) => body,
        PaymentNotifyOutcome::Verified(verified) => {
            panic!("expected ignored notify, got verified {verified:?}")
        }
    }
}

// A freshly generated RSA keypair (PKCS#8 private, SPKI public) for exercising
// the AlipayF2F RSA2 verify path without shipping a fixed private key.
fn alipay_test_keypair() -> (String, String) {
    let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
    let pkey = PKey::from_rsa(rsa).unwrap();
    let private_pem = String::from_utf8(pkey.private_key_to_pem_pkcs8().unwrap()).unwrap();
    let public_pem = String::from_utf8(pkey.public_key_to_pem().unwrap()).unwrap();
    (private_pem, public_pem)
}

// --- MD5 gateways: a tampered payload must be rejected (no forged callback
//     may mark an order paid). EPay/MGate reject hard; bepusdt soft-fails to
//     the Laravel 'cannot pass verification' body. ---

#[test]
fn epay_notify_rejects_tampered_payload() {
    let payment = fixture_payment("EPay", json!({ "key": "epay-secret" }));
    let mut signed = BTreeMap::from([
        ("money".to_string(), "12.34".to_string()),
        ("out_trade_no".to_string(), "T202607060001".to_string()),
        ("trade_no".to_string(), "EPAY-CALLBACK-1".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let sign = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), "epay-secret"))
    );
    signed.insert("sign".to_string(), sign);
    signed.insert("sign_type".to_string(), "MD5".to_string());
    // Attacker inflates the amount after the merchant signed the callback.
    signed.insert("money".to_string(), "9999.00".to_string());
    let params = signed.into_iter().collect::<HashMap<_, _>>();

    assert!(epay_notify(&payment, &params).is_err());
}

#[test]
fn epay_notify_rejects_blank_signature() {
    let payment = fixture_payment("EPay", json!({ "key": "epay-secret" }));
    let params = HashMap::from([
        ("out_trade_no".to_string(), "T202607060001".to_string()),
        ("trade_no".to_string(), "EPAY-CALLBACK-1".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
        ("sign".to_string(), String::new()),
    ]);

    assert!(epay_notify(&payment, &params).is_err());
}

#[test]
fn mgate_notify_rejects_tampered_payload() {
    let payment = fixture_payment("MGate", json!({ "mgate_app_secret": "mgate-secret" }));
    let mut signed = BTreeMap::from([
        ("out_trade_no".to_string(), "T202607060002".to_string()),
        ("trade_no".to_string(), "MGATE-CALLBACK-1".to_string()),
    ]);
    let sign = format!(
        "{:x}",
        md5::compute(format!(
            "{}{}",
            form_query(&signed).unwrap(),
            "mgate-secret"
        ))
    );
    signed.insert("sign".to_string(), sign);
    signed.insert("out_trade_no".to_string(), "T-INJECTED".to_string());
    let params = signed.into_iter().collect::<HashMap<_, _>>();

    assert!(mgate_notify(&payment, &params).is_err());
}

#[test]
fn bepusdt_notify_soft_fails_on_bad_signature() {
    let payment = fixture_payment(
        "BEasyPaymentUSDT",
        json!({ "bepusdt_apitoken": "bepusdt-secret" }),
    );
    let params = HashMap::from([
        ("order_id".to_string(), "T202607060003".to_string()),
        ("status".to_string(), "2".to_string()),
        ("trade_id".to_string(), "BEPUSDT-CALLBACK-1".to_string()),
        ("signature".to_string(), "deadbeef".to_string()),
    ]);

    // Laravel returns the body 'cannot pass verification' (HTTP 200) rather
    // than erroring, so a mismatched signature must not mark the order paid.
    assert_eq!(
        unwrap_ignored(bepusdt_notify(&payment, &params).unwrap()),
        "cannot pass verification"
    );
}

// --- Alipay F2F (RSA2): verify the real OpenSSL sign/verify round-trip and
//     that a tampered payload fails verification. ---

#[test]
fn alipay_f2f_notify_verifies_rsa_signed_callback() {
    let (private_pem, public_pem) = alipay_test_keypair();
    let payment = fixture_payment("AlipayF2F", json!({ "public_key": public_pem }));
    let signed = BTreeMap::from([
        ("out_trade_no".to_string(), "T202607060020".to_string()),
        ("trade_no".to_string(), "ALIPAY-CALLBACK-1".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
        ("total_amount".to_string(), "12.34".to_string()),
    ]);
    let sign = alipay_sign(&private_pem, &canonical_query(&signed)).unwrap();
    let mut params = signed.into_iter().collect::<HashMap<_, _>>();
    params.insert("sign".to_string(), sign);
    params.insert("sign_type".to_string(), "RSA2".to_string());

    let verified = unwrap_verified(alipay_f2f_notify(&payment, &params).unwrap());

    assert_eq!(verified.trade_no, "T202607060020");
    assert_eq!(verified.callback_no, "ALIPAY-CALLBACK-1");
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn alipay_f2f_notify_rejects_tampered_payload() {
    let (private_pem, public_pem) = alipay_test_keypair();
    let payment = fixture_payment("AlipayF2F", json!({ "public_key": public_pem }));
    let signed = BTreeMap::from([
        ("out_trade_no".to_string(), "T202607060020".to_string()),
        ("trade_no".to_string(), "ALIPAY-CALLBACK-1".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
        ("total_amount".to_string(), "12.34".to_string()),
    ]);
    let sign = alipay_sign(&private_pem, &canonical_query(&signed)).unwrap();
    let mut params = signed.into_iter().collect::<HashMap<_, _>>();
    params.insert("sign".to_string(), sign);
    params.insert("sign_type".to_string(), "RSA2".to_string());
    // Attacker rewrites the amount after Alipay signed the notification.
    params.insert("total_amount".to_string(), "9999.00".to_string());

    assert!(alipay_f2f_notify(&payment, &params).is_err());
}

#[test]
fn alipay_f2f_notify_ignores_non_success_trade_status() {
    // The trade_status gate runs before signature verification (like Laravel's
    // AlipayF2F::notify, which checks trade_status before $gateway->verify), so a
    // WAIT_BUYER_PAY callback needs no key. The RESPONSE, though, diverges by
    // design: Laravel returns false here, which PaymentController turns into
    // abort(500,'fail'); Rust acks with 200 "success" instead. That only changes
    // whether the gateway retries a non-success/terminal callback — it never marks
    // an order paid (the money path is identical and separately tested) — so the
    // safer ack is a deliberate, self-consistent improvement, not a contract break.
    let payment = fixture_payment("AlipayF2F", json!({ "public_key": "unused" }));
    let params = HashMap::from([
        ("out_trade_no".to_string(), "T202607060020".to_string()),
        ("trade_status".to_string(), "WAIT_BUYER_PAY".to_string()),
    ]);

    assert_eq!(
        unwrap_ignored(alipay_f2f_notify(&payment, &params).unwrap()),
        "success"
    );
}

// --- WeChat Pay Native (MD5 over sorted key=value pairs). ---

#[test]
fn wechat_pay_native_notify_verifies_signed_callback() {
    let payment = fixture_payment("WechatPayNative", json!({ "api_key": "wechat-secret" }));
    let signed = BTreeMap::from([
        ("return_code".to_string(), "SUCCESS".to_string()),
        ("result_code".to_string(), "SUCCESS".to_string()),
        ("out_trade_no".to_string(), "T202607060030".to_string()),
        ("total_fee".to_string(), "1234".to_string()),
        ("transaction_id".to_string(), "WX-CALLBACK-1".to_string()),
    ]);
    let sign = wechat_sign(&signed, "wechat-secret");
    let mut params = signed.into_iter().collect::<HashMap<_, _>>();
    params.insert("sign".to_string(), sign);
    let input = PaymentNotifyInput {
        params,
        body: Vec::new(),
        headers: HashMap::new(),
    };

    let verified = unwrap_verified(wechat_pay_native_notify(&payment, &input).unwrap());

    assert_eq!(verified.trade_no, "T202607060030");
    assert_eq!(verified.callback_no, "WX-CALLBACK-1");
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn wechat_pay_native_notify_rejects_tampered_payload() {
    let payment = fixture_payment("WechatPayNative", json!({ "api_key": "wechat-secret" }));
    let signed = BTreeMap::from([
        ("return_code".to_string(), "SUCCESS".to_string()),
        ("result_code".to_string(), "SUCCESS".to_string()),
        ("out_trade_no".to_string(), "T202607060030".to_string()),
        ("transaction_id".to_string(), "WX-CALLBACK-1".to_string()),
    ]);
    let sign = wechat_sign(&signed, "wechat-secret");
    let mut params = signed.into_iter().collect::<HashMap<_, _>>();
    params.insert("sign".to_string(), sign);
    params.insert("out_trade_no".to_string(), "T-INJECTED".to_string());
    let input = PaymentNotifyInput {
        params,
        body: Vec::new(),
        headers: HashMap::new(),
    };

    assert!(wechat_pay_native_notify(&payment, &input).is_err());
}

// --- CoinPayments (HMAC-SHA512 over the form-encoded body, header 'hmac'). ---

#[test]
fn coinpayments_notify_verifies_signed_ipn() {
    let payment = fixture_payment(
        "CoinPayments",
        json!({
            "coinpayments_merchant_id": "MID",
            "coinpayments_ipn_secret": "ipn-secret",
            "coinpayments_currency": "USD",
        }),
    );
    let signed = BTreeMap::from([
        ("amount1".to_string(), "12.34".to_string()),
        ("currency1".to_string(), "USD".to_string()),
        ("merchant".to_string(), "MID".to_string()),
        ("status".to_string(), "100".to_string()),
        ("item_number".to_string(), "T202607060040".to_string()),
        ("txn_id".to_string(), "CP-CALLBACK-1".to_string()),
    ]);
    let body = form_query(&signed).unwrap().into_bytes();
    let hmac = hmac_sha512_hex(b"ipn-secret", &body).unwrap();
    let input = PaymentNotifyInput {
        params: signed.into_iter().collect(),
        body,
        headers: HashMap::from([("hmac".to_string(), hmac)]),
    };

    let verified = unwrap_verified(coinpayments_notify(&payment, &input).unwrap());

    assert_eq!(verified.trade_no, "T202607060040");
    assert_eq!(verified.callback_no, "CP-CALLBACK-1");
    assert_eq!(verified.custom_result.as_deref(), Some("IPN OK"));
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn coinpayments_notify_rejects_tampered_hmac() {
    let payment = fixture_payment(
        "CoinPayments",
        json!({
            "coinpayments_merchant_id": "MID",
            "coinpayments_ipn_secret": "ipn-secret",
            "coinpayments_currency": "USD",
        }),
    );
    let params = BTreeMap::from([
        ("amount1".to_string(), "12.34".to_string()),
        ("currency1".to_string(), "USD".to_string()),
        ("merchant".to_string(), "MID".to_string()),
        ("status".to_string(), "100".to_string()),
        ("item_number".to_string(), "T202607060040".to_string()),
        ("txn_id".to_string(), "CP-CALLBACK-1".to_string()),
    ]);
    let body = form_query(&params).unwrap().into_bytes();
    let input = PaymentNotifyInput {
        params: params.into_iter().collect(),
        body,
        headers: HashMap::from([("hmac".to_string(), "deadbeef".to_string())]),
    };

    assert!(coinpayments_notify(&payment, &input).is_err());
}

#[test]
fn coinpayments_notify_rejects_wrong_merchant() {
    let payment = fixture_payment(
        "CoinPayments",
        json!({
            "coinpayments_merchant_id": "MID",
            "coinpayments_ipn_secret": "ipn-secret",
            "coinpayments_currency": "USD",
        }),
    );
    let params = BTreeMap::from([
        ("amount1".to_string(), "12.34".to_string()),
        ("currency1".to_string(), "USD".to_string()),
        ("merchant".to_string(), "OTHER".to_string()),
        ("status".to_string(), "100".to_string()),
        ("item_number".to_string(), "T202607060040".to_string()),
        ("txn_id".to_string(), "CP-CALLBACK-1".to_string()),
    ]);
    let body = form_query(&params).unwrap().into_bytes();
    let hmac = hmac_sha512_hex(b"ipn-secret", &body).unwrap();
    let input = PaymentNotifyInput {
        params: params.into_iter().collect(),
        body,
        headers: HashMap::from([("hmac".to_string(), hmac)]),
    };

    assert!(coinpayments_notify(&payment, &input).is_err());
}

#[test]
fn coinpayments_notify_requires_signed_amount_currency_and_terminal_status() {
    let payment = fixture_payment(
        "CoinPayments",
        json!({
            "coinpayments_merchant_id": "MID",
            "coinpayments_ipn_secret": "ipn-secret",
            "coinpayments_currency": "USD",
        }),
    );
    let make_input = |amount: Option<&str>, currency: Option<&str>, status: &str| {
        let mut signed = BTreeMap::from([
            ("merchant".to_string(), "MID".to_string()),
            ("status".to_string(), status.to_string()),
            ("item_number".to_string(), "T202607060040".to_string()),
            ("txn_id".to_string(), "CP-CALLBACK-1".to_string()),
        ]);
        if let Some(amount) = amount {
            signed.insert("amount1".to_string(), amount.to_string());
        }
        if let Some(currency) = currency {
            signed.insert("currency1".to_string(), currency.to_string());
        }
        let body = form_query(&signed).unwrap().into_bytes();
        let hmac = hmac_sha512_hex(b"ipn-secret", &body).unwrap();
        PaymentNotifyInput {
            params: signed.into_iter().collect(),
            body,
            headers: HashMap::from([("hmac".to_string(), hmac)]),
        }
    };

    assert!(coinpayments_notify(&payment, &make_input(None, Some("USD"), "100")).is_err());
    assert!(coinpayments_notify(&payment, &make_input(Some("12.34"), Some("EUR"), "100")).is_err());
    assert!(
        coinpayments_notify(&payment, &make_input(Some("12.345"), Some("USD"), "100")).is_err()
    );
    assert!(matches!(
        coinpayments_notify(
            &payment,
            &make_input(Some("12.34"), Some("USD"), "1")
        )
        .unwrap(),
        PaymentNotifyOutcome::Ignored(body) if body == "IPN OK: pending"
    ));
}

// --- Coinbase Commerce (HMAC-SHA256 over the raw body, header
//     'x-cc-webhook-signature'). ---

#[test]
fn coinbase_notify_verifies_signed_webhook() {
    let payment = fixture_payment("Coinbase", json!({ "coinbase_webhook_key": "cb-secret" }));
    let json_body = json!({
        "event": {
            "id": "CB-EVENT-1",
            "type": "charge:confirmed",
            "data": {
                "metadata": { "outTradeNo": "T202607060050" },
                "pricing": { "local": { "amount": "12.34", "currency": "CNY" } }
            }
        }
    })
    .to_string();
    // The signature covers these surrounding bytes too; the verifier must not
    // trim before authenticating, while JSON decoding may accept the whitespace.
    let body = format!("\n{json_body}\n").into_bytes();
    let signature = hmac_sha256_hex(b"cb-secret", &body).unwrap();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("x-cc-webhook-signature".to_string(), signature)]),
    };

    let verified = unwrap_verified(coinbase_notify(&payment, &input).unwrap());

    assert_eq!(verified.trade_no, "T202607060050");
    assert_eq!(verified.callback_no, "CB-EVENT-1");
    assert_eq!(verified.settled_amount_cents, Some(1_234));
}

#[test]
fn coinbase_notify_rejects_tampered_signature() {
    let payment = fixture_payment("Coinbase", json!({ "coinbase_webhook_key": "cb-secret" }));
    let body = json!({
        "event": {
            "id": "CB-EVENT-1",
            "type": "charge:confirmed",
            "data": {
                "metadata": { "outTradeNo": "T1" },
                "pricing": { "local": { "amount": "1.00", "currency": "CNY" } }
            }
        }
    })
    .to_string()
    .into_bytes();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("x-cc-webhook-signature".to_string(), "deadbeef".to_string())]),
    };

    assert!(coinbase_notify(&payment, &input).is_err());
}

#[test]
fn coinbase_notify_ignores_authenticated_non_confirmed_events() {
    let payment = fixture_payment("Coinbase", json!({ "coinbase_webhook_key": "cb-secret" }));
    let body = json!({
        "event": { "id": "CB-EVENT-1", "type": "charge:pending", "data": {} }
    })
    .to_string()
    .into_bytes();
    let signature = hmac_sha256_hex(b"cb-secret", &body).unwrap();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("x-cc-webhook-signature".to_string(), signature)]),
    };

    assert!(matches!(
        coinbase_notify(&payment, &input).unwrap(),
        PaymentNotifyOutcome::Ignored(body) if body == "success"
    ));
}

#[test]
fn coinbase_notify_does_not_accept_a_signature_for_trimmed_bytes() {
    let payment = fixture_payment("Coinbase", json!({ "coinbase_webhook_key": "cb-secret" }));
    let json_body = json!({
        "event": { "id": "CB-EVENT-1", "type": "charge:pending", "data": {} }
    })
    .to_string();
    let body = format!(" {json_body}\n").into_bytes();
    let signature = hmac_sha256_hex(b"cb-secret", json_body.as_bytes()).unwrap();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("x-cc-webhook-signature".to_string(), signature)]),
    };

    assert!(coinbase_notify(&payment, &input).is_err());
}

#[test]
fn coinbase_notify_decodes_the_body_only_after_authenticating_exact_bytes() {
    let payment = fixture_payment("Coinbase", json!({ "coinbase_webhook_key": "cb-secret" }));
    let body = vec![0xff, 0xfe, 0xfd];
    let signature = hmac_sha256_hex(b"cb-secret", &body).unwrap();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("x-cc-webhook-signature".to_string(), signature)]),
    };

    let error = coinbase_notify(&payment, &input).unwrap_err();
    assert_eq!(error.to_string(), "Payment notify body is invalid");
}

// --- Stripe (all-cards / payment_intents) webhook HMAC. ---

#[test]
fn exchange_rate_cache_decision_covers_fresh_stale_and_expired_boundaries() {
    let rate = Decimal::new(7_123_456, 6);
    let fetched_at = 1_000_000;
    let cached = Some(CachedExchangeRate { rate, fetched_at });

    assert_eq!(
        exchange_rate_cache_decision(cached, fetched_at + EXCHANGE_RATE_FRESH_TTL_SECS),
        ExchangeRateCacheDecision::Fresh(rate)
    );
    assert_eq!(
        exchange_rate_cache_decision(cached, fetched_at + EXCHANGE_RATE_FRESH_TTL_SECS + 1),
        ExchangeRateCacheDecision::Refresh { stale: Some(rate) }
    );
    assert_eq!(
        exchange_rate_cache_decision(cached, fetched_at + EXCHANGE_RATE_STALE_TTL_SECS),
        ExchangeRateCacheDecision::Refresh { stale: Some(rate) }
    );
    assert_eq!(
        exchange_rate_cache_decision(cached, fetched_at + EXCHANGE_RATE_STALE_TTL_SECS + 1),
        ExchangeRateCacheDecision::Refresh { stale: None }
    );
    assert_eq!(
        exchange_rate_cache_decision(None, fetched_at),
        ExchangeRateCacheDecision::Refresh { stale: None }
    );
    assert_eq!(
        exchange_rate_cache_decision(
            Some(CachedExchangeRate {
                rate: Decimal::ZERO,
                fetched_at,
            }),
            fetched_at,
        ),
        ExchangeRateCacheDecision::Refresh { stale: None }
    );
}

#[test]
fn stripe_payment_amount_respects_currency_minor_units() {
    assert_eq!(
        stripe_payment_amount(1_000, Decimal::ONE, "cny").unwrap(),
        1_000
    );
    assert_eq!(
        stripe_payment_amount(1_000, Decimal::from(20), "jpy").unwrap(),
        200
    );
    assert_eq!(
        stripe_payment_amount(1_000, Decimal::new(5, 2), "kwd").unwrap(),
        500
    );
    assert_eq!(
        stripe_payment_amount(1_000, Decimal::new(2_005, 2), "isk").unwrap(),
        20_000
    );
    assert!(stripe_payment_amount(i32::MAX, Decimal::MAX, "cny").is_err());
}

#[test]
fn every_stripe_creation_shape_carries_the_complete_settlement_metadata() {
    let order = PaymentOrder {
        notify_url: "https://example.test/notify".to_string(),
        return_url: "https://example.test/return".to_string(),
        trade_no: "T-METADATA".to_string(),
        total_amount: 5_678,
        user_id: 9,
    };
    for prefix in ["metadata", "payment_intent_data[metadata]"] {
        let mut params = BTreeMap::new();
        add_stripe_settlement_metadata(&mut params, prefix, 7, &order, 1_234, "cny");
        for (key, expected) in [
            ("user_id", "9"),
            ("out_trade_no", "T-METADATA"),
            ("payment_id", "7"),
            ("expected_local_amount", "5678"),
            ("expected_gateway_amount", "1234"),
            ("expected_amount", "1234"),
            ("expected_currency", "cny"),
        ] {
            assert_eq!(
                params.get(&format!("{prefix}[{key}]")).map(String::as_str),
                Some(expected),
            );
        }
    }
}

#[test]
fn stripe_credit_reuses_only_the_intent_bound_to_the_exact_order() {
    let intent = json!({
        "status": "requires_payment_method",
        "amount": 1234,
        "currency": "cny",
        "metadata": {
            "out_trade_no": "T202607060059",
            "payment_id": "7",
            "user_id": "9",
            "expected_local_amount": "5678",
            "expected_gateway_amount": "1234",
            "expected_amount": "1234",
            "expected_currency": "cny"
        }
    });
    assert!(reusable_stripe_credit_intent_matches(
        &intent,
        "T202607060059",
        7,
        9,
        5678,
        1234,
        "cny",
    ));

    for path in [
        "/metadata/out_trade_no",
        "/metadata/payment_id",
        "/metadata/user_id",
        "/metadata/expected_local_amount",
        "/metadata/expected_gateway_amount",
        "/metadata/expected_amount",
        "/metadata/expected_currency",
    ] {
        let mut drifted = intent.clone();
        *drifted.pointer_mut(path).expect("fixture metadata path") = json!("drifted");
        assert!(!reusable_stripe_credit_intent_matches(
            &drifted,
            "T202607060059",
            7,
            9,
            5678,
            1234,
            "cny",
        ));
    }

    let mut canceled = intent;
    canceled["status"] = json!("canceled");
    assert!(!reusable_stripe_credit_intent_matches(
        &canceled,
        "T202607060059",
        7,
        9,
        5678,
        1234,
        "cny",
    ));
}

#[test]
fn stripe_credit_notify_verifies_intent_amount_currency_and_signature() {
    let payment = fixture_payment(
        "StripeCredit",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let body = json!({
        "type": "payment_intent.succeeded",
        "data": { "object": {
            "status": "succeeded",
            "id": "pi_credit_1",
            "amount": 1234,
            "amount_received": 1234,
            "currency": "cny",
            "metadata": {
                "out_trade_no": "T202607060059",
                "payment_id": "1",
                "user_id": "9",
                "expected_local_amount": "5678",
                "expected_gateway_amount": "1234",
                "expected_amount": "1234",
                "expected_currency": "cny"
            }
        }}
    })
    .to_string()
    .into_bytes();
    let timestamp = Utc::now().timestamp().to_string();
    let signature = stripe_test_signature(&timestamp, &body);
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1={signature}"),
        )]),
    };

    let verified = unwrap_verified(stripe_payment_intent_notify(&payment, &input).unwrap());
    assert_eq!(verified.trade_no, "T202607060059");
    assert_eq!(verified.callback_no, "pi_credit_1");
    assert_eq!(verified.authenticated_user_id, Some(9));
    assert_eq!(verified.settled_amount_cents, Some(5_678));
}

#[test]
fn stripe_credit_notify_rejects_gateway_currency_or_binding_mismatch() {
    let payment = fixture_payment(
        "StripeCredit",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let event = json!({
        "type": "payment_intent.succeeded",
        "data": { "object": {
            "status": "succeeded",
            "id": "pi_credit_1",
            "amount": 1234,
            "amount_received": 1234,
            "currency": "cny",
            "metadata": stripe_metadata_fixture("T202607060059", 1, 5678, 1234, "cny")
        }}
    });
    for (path, value) in [
        ("/data/object/amount", json!(1)),
        ("/data/object/amount_received", json!(1)),
        ("/data/object/currency", json!("usd")),
        ("/data/object/metadata/expected_gateway_amount", json!("1")),
        ("/data/object/metadata/expected_currency", json!("usd")),
        ("/data/object/metadata/payment_id", json!("2")),
    ] {
        let mut drifted = event.clone();
        *drifted.pointer_mut(path).expect("fixture path") = value;
        assert!(stripe_payment_intent_notify(&payment, &stripe_signed_input(drifted)).is_err());
    }

    let mut local_drift = event;
    local_drift["data"]["object"]["metadata"]["expected_local_amount"] = json!("1");
    let verified = unwrap_verified(
        stripe_payment_intent_notify(&payment, &stripe_signed_input(local_drift)).unwrap(),
    );
    assert_eq!(verified.settled_amount_cents, Some(1));
    assert!(!payment_amount_matches(
        5_678,
        None,
        verified.settled_amount_cents.unwrap(),
    ));

    let mut user_drift = stripe_metadata_fixture("T202607060059", 1, 5678, 1234, "cny");
    user_drift["user_id"] = json!("10");
    let verified = unwrap_verified(
        stripe_payment_intent_notify(
            &payment,
            &stripe_signed_input(json!({
                "type": "payment_intent.succeeded",
                "data": { "object": {
                    "status": "succeeded",
                    "id": "pi_credit_1",
                    "amount": 1234,
                    "amount_received": 1234,
                    "currency": "cny",
                    "metadata": user_drift
                }}
            })),
        )
        .unwrap(),
    );
    let expected = ExpectedPaymentBinding {
        payment_id: 1,
        user_id: verified.authenticated_user_id,
        callback_no: Some(verified.callback_no),
    };
    let callback_no_hash = payment_identifier_hash("pi_credit_1");
    assert!(!payment_binding_matches(
        &expected,
        9,
        Some(1),
        Some("pi_credit_1"),
        Some(callback_no_hash.as_slice()),
    ));
}

#[test]
fn stripe_all_notify_verifies_payment_intent_succeeded() {
    let payment = fixture_payment("StripeALL", json!({ "stripe_webhook_key": "whsec_test" }));
    let body = json!({
        "type": "payment_intent.succeeded",
        "data": {
            "object": {
                "status": "succeeded",
                "id": "pi_callback_1",
                "amount": 1234,
                "amount_received": 1234,
                "currency": "cny",
                "metadata": stripe_metadata_fixture("T202607060060", 1, 5678, 1234, "cny")
            }
        }
    })
    .to_string()
    .into_bytes();
    let timestamp = Utc::now().timestamp().to_string();
    let signature = stripe_test_signature(&timestamp, &body);
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1={signature}"),
        )]),
    };

    let verified = unwrap_verified(stripe_all_notify(&payment, &input).unwrap());

    assert_eq!(verified.trade_no, "T202607060060");
    assert_eq!(verified.callback_no, "pi_callback_1");
    assert_eq!(verified.authenticated_user_id, Some(9));
    assert_eq!(verified.settled_amount_cents, Some(5_678));
}

#[test]
fn stripe_all_payment_intent_rejects_signed_amount_currency_or_payment_drift() {
    let payment = fixture_payment("StripeALL", json!({ "stripe_webhook_key": "whsec_test" }));
    let event = json!({
        "type": "payment_intent.succeeded",
        "data": { "object": {
            "status": "succeeded",
            "id": "pi_callback_1",
            "amount": 1234,
            "amount_received": 1234,
            "currency": "cny",
            "metadata": stripe_metadata_fixture("T-ALL", 1, 5678, 1234, "cny")
        }}
    });
    for (path, value) in [
        ("/data/object/amount", json!(1)),
        ("/data/object/amount_received", json!(1)),
        ("/data/object/currency", json!("usd")),
        ("/data/object/metadata/payment_id", json!("2")),
    ] {
        let mut drifted = event.clone();
        *drifted.pointer_mut(path).expect("fixture path") = value;
        assert!(stripe_all_notify(&payment, &stripe_signed_input(drifted)).is_err());
    }
}

#[test]
fn stripe_all_checkout_session_verifies_signed_gateway_and_local_amounts() {
    let payment = fixture_payment("StripeALL", json!({ "stripe_webhook_key": "whsec_test" }));
    let input = stripe_signed_input(json!({
        "type": "checkout.session.completed",
        "data": { "object": {
            "payment_status": "paid",
            "client_reference_id": "T-ALL-CARDS",
            "payment_intent": "pi_all_cards",
            "amount_total": 1234,
            "currency": "cny",
            "metadata": stripe_metadata_fixture("T-ALL-CARDS", 1, 5678, 1234, "cny")
        }}
    }));
    let verified = unwrap_verified(stripe_all_notify(&payment, &input).unwrap());
    assert_eq!(verified.trade_no, "T-ALL-CARDS");
    assert_eq!(verified.callback_no, "pi_all_cards");
    assert_eq!(verified.settled_amount_cents, Some(5_678));
}

#[test]
fn stripe_all_notify_rejects_tampered_signature() {
    let payment = fixture_payment("StripeALL", json!({ "stripe_webhook_key": "whsec_test" }));
    let body = json!({
        "type": "payment_intent.succeeded",
        "data": { "object": { "status": "succeeded", "id": "pi_1", "metadata": { "out_trade_no": "T1" } } }
    })
    .to_string()
    .into_bytes();
    let timestamp = Utc::now().timestamp().to_string();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1=deadbeef"),
        )]),
    };

    assert!(stripe_all_notify(&payment, &input).is_err());
}

// --- Stripe source webhook (charge.succeeded path is signature-checked but
//     needs no network). ---

#[test]
fn stripe_source_charge_copies_the_authenticated_settlement_metadata() {
    let payment = fixture_payment(
        "StripeAlipay",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let source = json!({
        "id": "src_callback_1",
        "amount": 1234,
        "currency": "cny",
        "metadata": stripe_metadata_fixture("T-SOURCE", 1, 5678, 1234, "cny")
    });
    let params = stripe_source_charge_params(&payment, &source).unwrap();
    assert_eq!(
        params.get("source").map(String::as_str),
        Some("src_callback_1")
    );
    for key in [
        "user_id",
        "out_trade_no",
        "payment_id",
        "expected_local_amount",
        "expected_gateway_amount",
        "expected_amount",
        "expected_currency",
    ] {
        assert_eq!(
            params.get(&format!("metadata[{key}]")).map(String::as_str),
            source["metadata"].get(key).and_then(Value::as_str),
        );
    }
}

#[tokio::test]
async fn stripe_source_notify_verifies_charge_succeeded() {
    let payment = fixture_payment(
        "StripeAlipay",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let body = json!({
        "type": "charge.succeeded",
        "data": {
            "object": {
                "status": "succeeded",
                "paid": true,
                "id": "ch_callback_1",
                "amount": 1234,
                "currency": "cny",
                "metadata": stripe_metadata_fixture("T202607060070", 1, 5678, 1234, "cny")
            }
        }
    })
    .to_string()
    .into_bytes();
    let timestamp = Utc::now().timestamp().to_string();
    let signature = stripe_test_signature(&timestamp, &body);
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1={signature}"),
        )]),
    };

    let verified = unwrap_verified(stripe_source_notify(&payment, &input).await.unwrap());

    assert_eq!(verified.trade_no, "T202607060070");
    assert_eq!(verified.callback_no, "ch_callback_1");
    assert_eq!(verified.authenticated_user_id, Some(9));
    assert_eq!(verified.settled_amount_cents, Some(5_678));
}

#[tokio::test]
async fn stripe_source_charge_rejects_signed_amount_currency_or_payment_drift() {
    let payment = fixture_payment(
        "StripeAlipay",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let event = json!({
        "type": "charge.succeeded",
        "data": { "object": {
            "status": "succeeded",
            "paid": true,
            "id": "ch_callback_1",
            "amount": 1234,
            "currency": "cny",
            "metadata": stripe_metadata_fixture("T-SOURCE", 1, 5678, 1234, "cny")
        }}
    });
    for (path, value) in [
        ("/data/object/amount", json!(1)),
        ("/data/object/currency", json!("usd")),
        ("/data/object/paid", json!(false)),
        ("/data/object/metadata/payment_id", json!("2")),
    ] {
        let mut drifted = event.clone();
        *drifted.pointer_mut(path).expect("fixture path") = value;
        assert!(
            stripe_source_notify(&payment, &stripe_signed_input(drifted))
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn stripe_source_notify_rejects_tampered_signature() {
    let payment = fixture_payment(
        "StripeAlipay",
        json!({ "stripe_webhook_key": "whsec_test" }),
    );
    let body = json!({
        "type": "charge.succeeded",
        "data": { "object": { "status": "succeeded", "id": "ch_1", "metadata": { "out_trade_no": "T1" } } }
    })
    .to_string()
    .into_bytes();
    let timestamp = Utc::now().timestamp().to_string();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([(
            "stripe-signature".to_string(),
            format!("t={timestamp},v1=deadbeef"),
        )]),
    };

    assert!(stripe_source_notify(&payment, &input).await.is_err());
}

// --- BTCPay (HMAC-SHA256 with a 'sha256=' prefix, header 'btcpay-sig'). ---

#[tokio::test]
async fn btcpay_notify_rejects_tampered_signature() {
    let payment = fixture_payment(
        "BTCPay",
        json!({
            "btcpay_webhook_key": "bp-secret",
            "btcpay_url": "https://btcpay.example/",
            "btcpay_storeId": "store1",
            "btcpay_api_key": "apikey",
        }),
    );
    let body = json!({ "type": "InvoiceSettled", "invoiceId": "INV-1" })
        .to_string()
        .into_bytes();
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("btcpay-sig".to_string(), "sha256=deadbeef".to_string())]),
    };

    assert!(btcpay_notify(&payment, &input).await.is_err());
}

#[tokio::test]
async fn btcpay_notify_accepts_correctly_signed_webhook() {
    let payment = fixture_payment(
        "BTCPay",
        json!({
            "btcpay_webhook_key": "bp-secret",
            // Connection-refused loopback port: the downstream invoice fetch fails
            // fast and deterministically, so the test isolates the HMAC gate.
            "btcpay_url": "http://127.0.0.1:1/",
            "btcpay_storeId": "store1",
            "btcpay_api_key": "apikey",
        }),
    );
    let body = format!(
        "\n{}\n",
        json!({ "type": "InvoiceSettled", "invoiceId": "INV-1" })
    )
    .into_bytes();
    let sign = format!(
        "sha256={}",
        hmac_sha256_hex(b"bp-secret", &body).expect("sign")
    );
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("btcpay-sig".to_string(), sign)]),
    };

    // A correctly-signed body clears the HMAC gate; the only failure left is the
    // downstream invoice fetch, never a signature rejection. Proves the signed
    // happy path is accepted, complementing the tampered-signature rejection test.
    let error = match btcpay_notify(&payment, &input).await {
        Ok(_) => panic!("signed webhook cannot resolve without the invoice fetch"),
        Err(error) => error,
    };
    assert_eq!(error.to_string(), "Payment gateway request failed");
}

#[tokio::test]
async fn btcpay_notify_ignores_authenticated_non_settled_events_without_fetching() {
    let payment = fixture_payment(
        "BTCPay",
        json!({
            "btcpay_webhook_key": "bp-secret",
            "btcpay_url": "http://127.0.0.1:1/",
            "btcpay_storeId": "store1",
            "btcpay_api_key": "apikey",
        }),
    );
    let body = json!({ "type": "InvoiceProcessing", "invoiceId": "INV-1" })
        .to_string()
        .into_bytes();
    let sign = format!("sha256={}", hmac_sha256_hex(b"bp-secret", &body).unwrap());
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("btcpay-sig".to_string(), sign)]),
    };

    assert!(matches!(
        btcpay_notify(&payment, &input).await.unwrap(),
        PaymentNotifyOutcome::Ignored(body) if body == "success"
    ));
}

#[tokio::test]
async fn btcpay_notify_does_not_accept_a_signature_for_trimmed_bytes() {
    let payment = fixture_payment(
        "BTCPay",
        json!({
            "btcpay_webhook_key": "bp-secret",
            "btcpay_url": "http://127.0.0.1:1/",
            "btcpay_storeId": "store1",
            "btcpay_api_key": "apikey",
        }),
    );
    let json_body = json!({ "type": "InvoiceProcessing", "invoiceId": "INV-1" }).to_string();
    let body = format!(" {json_body}\n").into_bytes();
    let sign = format!(
        "sha256={}",
        hmac_sha256_hex(b"bp-secret", json_body.as_bytes()).unwrap()
    );
    let input = PaymentNotifyInput {
        params: HashMap::new(),
        body,
        headers: HashMap::from([("btcpay-sig".to_string(), sign)]),
    };

    assert!(btcpay_notify(&payment, &input).await.is_err());
}

#[test]
fn btcpay_invoice_requires_final_safe_settlement_and_bound_values() {
    let invoice = json!({
        "id": "INV-1",
        "status": "Settled",
        "additionalStatus": "None",
        "amount": "12.34",
        "currency": "CNY",
        "metadata": { "orderId": "T202607060060" }
    });
    let settlement = btcpay_invoice_settlement(&invoice, "INV-1").unwrap();
    assert_eq!(settlement.trade_no, "T202607060060");
    assert_eq!(settlement.amount_cents, 1_234);

    let mut overpaid = invoice.clone();
    overpaid["additionalStatus"] = json!("PaidOver");
    assert!(btcpay_invoice_settlement(&overpaid, "INV-1").is_ok());

    for unsafe_status in ["PaidPartial", "PaidLate", "Invalid"] {
        let mut unsafe_invoice = invoice.clone();
        unsafe_invoice["additionalStatus"] = json!(unsafe_status);
        assert!(btcpay_invoice_settlement(&unsafe_invoice, "INV-1").is_err());
    }

    let mut processing = invoice.clone();
    processing["status"] = json!("Processing");
    assert!(btcpay_invoice_settlement(&processing, "INV-1").is_err());

    let mut wrong_id = invoice.clone();
    wrong_id["id"] = json!("INV-OTHER");
    assert!(btcpay_invoice_settlement(&wrong_id, "INV-1").is_err());

    let mut wrong_currency = invoice;
    wrong_currency["currency"] = json!("USD");
    assert!(btcpay_invoice_settlement(&wrong_currency, "INV-1").is_err());
}

#[test]
fn signed_decimal_amounts_convert_exactly_without_binary_floats() {
    assert_eq!(decimal_amount_cents("12.34").unwrap(), 1_234);
    assert_eq!(decimal_amount_cents("12.340").unwrap(), 1_234);
    assert!(decimal_amount_cents("12.345").is_err());
    assert!(decimal_amount_cents("0").is_err());
    assert!(decimal_amount_cents("-1.00").is_err());
    assert!(decimal_amount_cents("99999999999999999999999999").is_err());
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
    assert_eq!(commission_amount_cents(5, Some(10), 0).unwrap(), 1);
    assert_eq!(commission_amount_cents(-5, Some(10), 0).unwrap(), -1);
    assert_eq!(commission_amount_cents(333, Some(10), 0).unwrap(), 33);
    assert_eq!(commission_amount_cents(10_000, Some(0), 10).unwrap(), 1_000);
    assert_eq!(
        commission_amount_cents(10_000, Some(-10), 0).unwrap(),
        -1_000
    );
    assert!(commission_amount_cents(i64::MAX, Some(100), 0).is_err());
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
        show: 1,
        sort: Some(1),
        renew: 1,
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
        r#type: order_type,
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
        super::lifecycle::purchasable_period_price(&plan, "month_price").unwrap(),
        0
    );

    plan.month_price = Some(-1);
    assert!(super::lifecycle::purchasable_period_price(&plan, "month_price").is_err());
    assert!(super::lifecycle::purchasable_period_price(&plan, "half_year_price").is_err());
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
        super::lifecycle::checked_unused_traffic(&user).unwrap(),
        42 * GIB
    );

    user.u = i64::MAX;
    user.d = 1;
    assert!(super::lifecycle::checked_unused_traffic(&user).is_err());

    user.transfer_enable = i64::MAX;
    user.u = -1;
    user.d = 0;
    assert!(super::lifecycle::checked_unused_traffic(&user).is_err());
}

#[test]
fn surplus_aggregate_and_duration_math_rejects_boundary_overflow() {
    assert_eq!(
        super::lifecycle::checked_order_month_sum(12, 24).unwrap(),
        36
    );
    assert!(super::lifecycle::checked_order_month_sum(u32::MAX, 1).is_err());

    assert_eq!(
        super::lifecycle::checked_order_amount_sum(10, 100, Some(20), Some(30), Some(5)).unwrap(),
        155
    );
    assert!(super::lifecycle::checked_order_amount_sum(i64::MAX, 1, None, None, None).is_err());
    assert!(super::lifecycle::checked_order_amount_sum(i64::MIN, 0, None, None, Some(1)).is_err());

    assert_eq!(
        super::lifecycle::checked_surplus_seconds(1_000, 400).unwrap(),
        600
    );
    assert!(super::lifecycle::checked_surplus_seconds(i64::MAX, i64::MIN).is_err());
    assert!(super::lifecycle::checked_surplus_seconds(i64::MIN, i64::MAX).is_err());

    assert_eq!(
        super::lifecycle::checked_surplus_add_months(GRANT_NOW, 1).unwrap(),
        add_months(GRANT_NOW, 1)
    );
    assert!(super::lifecycle::checked_surplus_add_months(GRANT_NOW, u32::MAX).is_err());

    assert!(super::lifecycle::checked_surplus_mul(Decimal::MAX, Decimal::from(2)).is_err());
    assert!(super::lifecycle::checked_surplus_add(Decimal::MAX, Decimal::from(1)).is_err());
    assert!(super::lifecycle::checked_surplus_div(Decimal::from(1), Decimal::ZERO).is_err());
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
fn unfinished_order_invariant_has_both_lock_and_database_guard() {
    assert!(USER_FOR_ORDER_SQL.contains("FOR UPDATE"));
    assert!(UNFINISHED_ORDER_FOR_UPDATE_SQL.ends_with("FOR UPDATE"));
    let migration = include_str!("../../../../migrations-postgres/0001_initial.sql");
    assert!(migration.contains(UNFINISHED_ORDER_UNIQUE_KEY));
    assert!(migration.contains("status"));
    assert!(migration.contains("user_id"));
    assert!(migration.contains("0, 1"));
}

#[test]
fn pending_payment_guards_have_a_selective_database_index() {
    let migration = include_str!("../../../../migrations-postgres/0001_initial.sql");
    assert!(migration.contains("idx_order_payment_status"));
    assert!(migration.contains("payment_id"));
    assert!(migration.contains("status"));
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

#[test]
fn capacity_slot_decision_treats_pending_reservations_as_consumed() {
    assert!(super::lifecycle::capacity_has_slot(2, 1));
    assert!(!super::lifecycle::capacity_has_slot(2, 2));
    assert!(!super::lifecycle::capacity_has_slot(2, 3));
    assert!(!super::lifecycle::capacity_has_slot(-1, 0));
}

#[test]
fn cent_addition_rejects_deposit_and_refund_overflow() {
    assert_eq!(
        super::lifecycle::checked_add_cents(100, 25, "overflow").unwrap(),
        125
    );
    assert!(super::lifecycle::checked_add_cents(i32::MAX, 1, "overflow").is_err());
}
