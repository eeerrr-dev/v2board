use std::collections::{BTreeMap, HashMap};

use chrono::Utc;
use openssl::pkey::PKey;
use rust_decimal::Decimal;
use serde_json::{Value, json};

use super::integrations::{
    CachedExchangeRate, EXCHANGE_RATE_FRESH_TTL_SECS, EXCHANGE_RATE_STALE_TTL_SECS,
    ExchangeRateCacheDecision, STRIPE_WEBHOOK_TOLERANCE_SECS, WechatUnifiedOrderResponse,
    add_stripe_settlement_metadata, alipay_f2f_notify, alipay_sign, bepusdt_notify,
    btcpay_invoice_settlement, btcpay_notify, canonical_query, coinbase_notify,
    coinpayments_notify, decimal_amount_cents, epay_notify, exchange_rate_cache_decision,
    form_query, hmac_sha256_hex, hmac_sha512_hex, mgate_notify, payment_http_client,
    require_payment_provider, reusable_stripe_credit_intent_matches, stripe_all_notify,
    stripe_checkout_notify, stripe_payment_amount, stripe_payment_intent_notify,
    stripe_source_charge_params, stripe_source_notify, url_origin, verify_legacy_md5_hex,
    wechat_pay_native_notify, wechat_sign, xml_from_params,
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
        uuid: "payment-uuid".to_string(),
        config: config.to_string(),
        notify_domain: None,
    }
}

fn unwrap_verified(outcome: PaymentNotifyOutcome) -> VerifiedPaymentNotify {
    match outcome {
        PaymentNotifyOutcome::Verified(verified) => verified,
        PaymentNotifyOutcome::Ignored(body) => panic!("expected verified notify, got {body}"),
    }
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
    assert_ne!(verified.settled_amount_cents, Some(5_678));
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

#[test]
fn payment_url_and_wechat_xml_use_structured_codecs() {
    assert_eq!(
        url_origin("https://pay.example.com:8443/order/T1"),
        "https://pay.example.com:8443"
    );
    assert_eq!(url_origin("/order/T1"), "/order/T1");

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
        user_email: None,
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
    assert_ne!(verified.settled_amount_cents, Some(5_678));

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
    assert_eq!(verified.authenticated_user_id, Some(10));
    assert_eq!(verified.callback_no, "pi_credit_1");
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
