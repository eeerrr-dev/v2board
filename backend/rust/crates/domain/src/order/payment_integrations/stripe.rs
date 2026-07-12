use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, LazyLock, RwLock},
};

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::{Value, json};
use sha2::Sha256;
use v2board_compat::ApiError;

use super::{
    CheckoutResult, OrderService, PaymentForCheckout, PaymentHttpClient, PaymentNotifyInput,
    PaymentNotifyOutcome, PaymentOrder, StripePaymentIntentResult, VerifiedPaymentNotify,
    config_required, config_string, form_query, header_value, payment_config, payment_http_client,
    right_chars,
};

pub(in crate::order) const EXCHANGE_RATE_FRESH_TTL_SECS: i64 = 60 * 60;
pub(in crate::order) const EXCHANGE_RATE_STALE_TTL_SECS: i64 = 24 * 60 * 60;
static EXCHANGE_RATE_CACHE: LazyLock<RwLock<HashMap<String, CachedExchangeRate>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static EXCHANGE_RATE_REFRESH_LOCKS: LazyLock<
    tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
> = LazyLock::new(|| tokio::sync::Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::order) struct CachedExchangeRate {
    pub(in crate::order) rate: Decimal,
    pub(in crate::order) fetched_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::order) enum ExchangeRateCacheDecision {
    Fresh(Decimal),
    Refresh { stale: Option<Decimal> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StripeSettlementMetadata {
    trade_no: String,
    user_id: i64,
    local_amount: i64,
    gateway_amount: i64,
    currency: String,
}

fn stripe_currency(config: &serde_json::Map<String, Value>) -> Result<String, ApiError> {
    let currency = config_required(config, "currency")?
        .trim()
        .to_ascii_lowercase();
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Err(ApiError::legacy("Stripe is not configured"));
    }
    Ok(currency)
}

pub(in crate::order) fn add_stripe_settlement_metadata(
    params: &mut BTreeMap<String, String>,
    prefix: &str,
    payment_id: i32,
    order: &PaymentOrder,
    gateway_amount: i64,
    currency: &str,
) {
    for (key, value) in [
        ("user_id", order.user_id.to_string()),
        ("out_trade_no", order.trade_no.clone()),
        ("payment_id", payment_id.to_string()),
        ("expected_local_amount", order.total_amount.to_string()),
        ("expected_gateway_amount", gateway_amount.to_string()),
        // Retain the Payment Element metadata field used by existing clients and
        // in-flight operational tooling while making its meaning explicit above.
        ("expected_amount", gateway_amount.to_string()),
        ("expected_currency", currency.to_string()),
    ] {
        params.insert(format!("{prefix}[{key}]"), value);
    }
}

fn stripe_settlement_metadata(
    payment: &PaymentForCheckout,
    object: &Value,
) -> Result<StripeSettlementMetadata, ApiError> {
    let trade_no = value_path_str(object, &["metadata", "out_trade_no"])
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let user_id = value_path_str(object, &["metadata", "user_id"])
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let payment_id = value_path_str(object, &["metadata", "payment_id"])
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let local_amount = value_path_str(object, &["metadata", "expected_local_amount"])
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let gateway_amount = value_path_str(object, &["metadata", "expected_gateway_amount"])
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let legacy_expected_amount = value_path_str(object, &["metadata", "expected_amount"])
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let currency = value_path_str(object, &["metadata", "expected_currency"])
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_lowercase()))
        .ok_or_else(|| ApiError::legacy("Stripe settlement metadata is invalid"))?;
    let configured_currency = stripe_currency(&payment_config(payment)?)?;
    if payment_id != payment.id
        || legacy_expected_amount != gateway_amount
        || currency != configured_currency
    {
        return Err(ApiError::legacy(
            "Stripe settlement does not match the order",
        ));
    }
    Ok(StripeSettlementMetadata {
        trade_no,
        user_id,
        local_amount,
        gateway_amount,
        currency,
    })
}

fn stripe_actual_amount_matches(
    metadata: &StripeSettlementMetadata,
    amount: Option<i64>,
    currency: Option<&str>,
) -> bool {
    amount == Some(metadata.gateway_amount)
        && currency
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
            == Some(metadata.currency.as_str())
}

fn stripe_payment_intent_verified(
    payment: &PaymentForCheckout,
    object: &Value,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let metadata = stripe_settlement_metadata(payment, object)?;
    if object.get("status").and_then(Value::as_str) != Some("succeeded")
        || !stripe_actual_amount_matches(
            &metadata,
            object.get("amount").and_then(Value::as_i64),
            object.get("currency").and_then(Value::as_str),
        )
        || object.get("amount_received").and_then(Value::as_i64) != Some(metadata.gateway_amount)
    {
        return Err(ApiError::legacy(
            "Stripe PaymentIntent does not match the order",
        ));
    }
    let callback_no = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("pi_"))
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: metadata.trade_no,
        callback_no: callback_no.to_string(),
        custom_result: None,
        authenticated_user_id: Some(metadata.user_id),
        settled_amount_cents: Some(metadata.local_amount),
    }))
}

fn stripe_charge_verified(
    payment: &PaymentForCheckout,
    object: &Value,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let metadata = stripe_settlement_metadata(payment, object)?;
    if object.get("status").and_then(Value::as_str) != Some("succeeded")
        || object.get("paid").and_then(Value::as_bool) != Some(true)
        || !stripe_actual_amount_matches(
            &metadata,
            object.get("amount").and_then(Value::as_i64),
            object.get("currency").and_then(Value::as_str),
        )
    {
        return Err(ApiError::legacy("Stripe charge does not match the order"));
    }
    let callback_no = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("ch_"))
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: metadata.trade_no,
        callback_no: callback_no.to_string(),
        custom_result: None,
        authenticated_user_id: Some(metadata.user_id),
        settled_amount_cents: Some(metadata.local_amount),
    }))
}

fn stripe_source_matches(payment: &PaymentForCheckout, object: &Value) -> Result<(), ApiError> {
    let metadata = stripe_settlement_metadata(payment, object)?;
    if !stripe_actual_amount_matches(
        &metadata,
        object.get("amount").and_then(Value::as_i64),
        object.get("currency").and_then(Value::as_str),
    ) {
        return Err(ApiError::legacy("Stripe source does not match the order"));
    }
    Ok(())
}

pub(in crate::order) fn stripe_source_charge_params(
    payment: &PaymentForCheckout,
    object: &Value,
) -> Result<BTreeMap<String, String>, ApiError> {
    stripe_source_matches(payment, object)?;
    let source_id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("src_"))
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    let amount = object
        .get("amount")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    let currency = object
        .get("currency")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    let mut params = BTreeMap::from([
        ("amount".to_string(), amount.to_string()),
        ("currency".to_string(), currency.to_string()),
        ("source".to_string(), source_id.to_string()),
    ]);
    add_metadata_params(&mut params, "metadata", object.get("metadata"));
    Ok(params)
}

pub(in crate::order) async fn stripe_credit_prepare(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
    existing_intent_id: Option<&str>,
) -> Result<(String, StripePaymentIntentResult), ApiError> {
    let mut config = payment_config(payment)?;
    let currency = stripe_currency(&config)?;
    let public_key = config_required(&config, "stripe_pk_live")?
        .trim()
        .to_string();
    let secret_key = config_required(&config, "stripe_sk_live")?
        .trim()
        .to_string();
    let webhook_secret = config_required(&config, "stripe_webhook_key")?
        .trim()
        .to_string();
    if public_key.is_empty() || secret_key.is_empty() || webhook_secret.is_empty() {
        return Err(ApiError::legacy("Stripe is not configured"));
    }
    config.insert("stripe_sk_live".to_string(), Value::String(secret_key));
    let exchange = exchange_cny_to(&currency).await?;
    let amount = stripe_payment_amount(order.total_amount, exchange, &currency)?;
    if amount <= 0 {
        return Err(ApiError::legacy("Stripe amount must be positive"));
    }

    // The PaymentIntent is the immutable settlement record: its actual amount
    // and currency, plus the duplicate metadata binding below, are what the
    // signed webhook verifies. The transient market rate is therefore neither
    // needed nor authoritative after this amount has been created.

    let mut intent = None;
    if let Some(intent_id) = existing_intent_id.filter(|id| id.starts_with("pi_")) {
        let candidate =
            stripe_get_with_config(&config, &format!("payment_intents/{intent_id}")).await?;
        let matches = reusable_stripe_credit_intent_matches(
            &candidate,
            &order.trade_no,
            payment.id,
            order.user_id,
            order.total_amount,
            amount,
            &currency,
        );
        if matches {
            intent = Some(candidate);
        } else if candidate.get("status").and_then(Value::as_str) == Some("succeeded") {
            return Err(ApiError::legacy(
                "The previous Stripe payment has already succeeded",
            ));
        } else if candidate.get("status").and_then(Value::as_str) != Some("canceled") {
            stripe_post_with_idempotency(
                &config,
                &format!("payment_intents/{intent_id}/cancel"),
                &BTreeMap::new(),
                &format!("v2board:replace:{intent_id}"),
            )
            .await?;
        }
    }

    let intent = if let Some(intent) = intent {
        intent
    } else {
        let mut params = BTreeMap::from([
            ("amount".to_string(), amount.to_string()),
            ("currency".to_string(), currency.clone()),
            (
                "automatic_payment_methods[enabled]".to_string(),
                "true".to_string(),
            ),
        ]);
        add_stripe_settlement_metadata(
            &mut params,
            "metadata",
            payment.id,
            order,
            amount,
            &currency,
        );
        let idempotency_key = format!(
            "v2board:stripe-credit:{}:{}:{amount}:{currency}:{}",
            order.trade_no,
            payment.id,
            existing_intent_id.unwrap_or("initial")
        );
        stripe_post_with_idempotency(&config, "payment_intents", &params, &idempotency_key).await?
    };
    let intent_id = intent
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| id.starts_with("pi_"))
        .ok_or_else(|| ApiError::legacy("Stripe did not return a PaymentIntent id"))?
        .to_string();
    let client_secret = intent
        .get("client_secret")
        .and_then(Value::as_str)
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| ApiError::legacy("Stripe did not return a client secret"))?
        .to_string();
    Ok((
        intent_id,
        StripePaymentIntentResult {
            public_key,
            client_secret,
            amount,
            currency,
        },
    ))
}

pub(in crate::order) fn reusable_stripe_credit_intent_matches(
    intent: &Value,
    trade_no: &str,
    payment_id: i32,
    user_id: i64,
    local_amount: i32,
    amount: i64,
    currency: &str,
) -> bool {
    let payment_id = payment_id.to_string();
    let user_id = user_id.to_string();
    let local_amount = local_amount.to_string();
    let expected_amount = amount.to_string();
    value_path_str(intent, &["metadata", "out_trade_no"]).as_deref() == Some(trade_no)
        && value_path_str(intent, &["metadata", "payment_id"]).as_deref()
            == Some(payment_id.as_str())
        && value_path_str(intent, &["metadata", "user_id"]).as_deref() == Some(user_id.as_str())
        && value_path_str(intent, &["metadata", "expected_local_amount"]).as_deref()
            == Some(local_amount.as_str())
        && value_path_str(intent, &["metadata", "expected_gateway_amount"]).as_deref()
            == Some(expected_amount.as_str())
        && value_path_str(intent, &["metadata", "expected_amount"]).as_deref()
            == Some(expected_amount.as_str())
        && value_path_str(intent, &["metadata", "expected_currency"]).as_deref() == Some(currency)
        && intent.get("amount").and_then(Value::as_i64) == Some(amount)
        && intent.get("currency").and_then(Value::as_str) == Some(currency)
        && intent.get("status").and_then(Value::as_str) != Some("canceled")
}

pub(in crate::order) async fn stripe_source_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
    source_type: &str,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let currency = stripe_currency(&config)?;
    let exchange = exchange_cny_to(&currency).await?;
    let gateway_amount = stripe_payment_amount(order.total_amount, exchange, &currency)?;
    let mut params = BTreeMap::from([
        ("amount".to_string(), gateway_amount.to_string()),
        ("currency".to_string(), currency.clone()),
        ("type".to_string(), source_type.to_string()),
        ("statement_descriptor".to_string(), order.trade_no.clone()),
        ("metadata[identifier]".to_string(), String::new()),
        ("redirect[return_url]".to_string(), order.return_url.clone()),
    ]);
    add_stripe_settlement_metadata(
        &mut params,
        "metadata",
        payment.id,
        order,
        gateway_amount,
        &currency,
    );
    let source = stripe_post(&config, "sources", &params).await?;
    let (result_type, data) = if source_type == "wechat" {
        (
            0,
            value_path_str(&source, &["wechat", "qr_code_url"])
                .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?,
        )
    } else {
        (
            1,
            value_path_str(&source, &["redirect", "url"])
                .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?,
        )
    };
    Ok(CheckoutResult {
        r#type: result_type,
        data: json!(data),
    })
}

pub(in crate::order) async fn stripe_checkout_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let currency = stripe_currency(&config)?;
    let exchange = exchange_cny_to(&currency).await?;
    let gateway_amount = stripe_payment_amount(order.total_amount, exchange, &currency)?;
    let custom_field_name = config_string(&config, "stripe_custom_field_name")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Contact Infomation".to_string());
    let mut params = BTreeMap::from([
        ("success_url".to_string(), order.return_url.clone()),
        ("cancel_url".to_string(), order.return_url.clone()),
        ("client_reference_id".to_string(), order.trade_no.clone()),
        (
            "line_items[0][price_data][currency]".to_string(),
            currency.clone(),
        ),
        (
            "line_items[0][price_data][product_data][name]".to_string(),
            order.trade_no.clone(),
        ),
        (
            "line_items[0][price_data][unit_amount]".to_string(),
            gateway_amount.to_string(),
        ),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
        ("mode".to_string(), "payment".to_string()),
        ("invoice_creation[enabled]".to_string(), "true".to_string()),
        (
            "phone_number_collection[enabled]".to_string(),
            "true".to_string(),
        ),
        (
            "custom_fields[0][key]".to_string(),
            "contactinfo".to_string(),
        ),
        (
            "custom_fields[0][label][type]".to_string(),
            "custom".to_string(),
        ),
        (
            "custom_fields[0][label][custom]".to_string(),
            custom_field_name,
        ),
        ("custom_fields[0][type]".to_string(), "text".to_string()),
    ]);
    add_stripe_settlement_metadata(
        &mut params,
        "metadata",
        payment.id,
        order,
        gateway_amount,
        &currency,
    );
    add_stripe_settlement_metadata(
        &mut params,
        "payment_intent_data[metadata]",
        payment.id,
        order,
        gateway_amount,
        &currency,
    );
    let session = stripe_post(&config, "checkout/sessions", &params).await?;
    let url = session
        .get("url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(url),
    })
}

pub(in crate::order) async fn stripe_all_pay(
    service: &OrderService,
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let method = config_required(&config, "payment_method")?;
    if method == "cards" {
        return stripe_all_cards_pay(service, payment.id, &config, order).await;
    }
    let currency = stripe_currency(&config)?;
    let exchange = exchange_cny_to(&currency).await?;
    let gateway_amount = stripe_payment_amount(order.total_amount, exchange, &currency)?;
    let payment_method = stripe_post(
        &config,
        "payment_methods",
        &BTreeMap::from([("type".to_string(), method.clone())]),
    )
    .await?;
    let payment_method_id = payment_method
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    let user_email = service.user_email(order.user_id).await?.unwrap_or_default();
    let mut params = BTreeMap::from([
        ("amount".to_string(), gateway_amount.to_string()),
        ("currency".to_string(), currency.clone()),
        ("confirm".to_string(), "true".to_string()),
        ("payment_method".to_string(), payment_method_id.to_string()),
        (
            "automatic_payment_methods[enabled]".to_string(),
            "true".to_string(),
        ),
        (
            "statement_descriptor".to_string(),
            format!(
                "user-#{}-{}",
                order.user_id,
                right_chars(&order.trade_no, 8)
            ),
        ),
        ("metadata[customer_email]".to_string(), user_email),
        ("return_url".to_string(), order.return_url.clone()),
    ]);
    add_stripe_settlement_metadata(
        &mut params,
        "metadata",
        payment.id,
        order,
        gateway_amount,
        &currency,
    );
    if method == "wechat_pay" {
        params.insert(
            "payment_method_options[wechat_pay][client]".to_string(),
            "web".to_string(),
        );
    }
    let intent = stripe_post(&config, "payment_intents", &params).await?;
    let (result_type, data) = match method.as_str() {
        "alipay" => (
            1,
            value_path_str(&intent, &["next_action", "alipay_handle_redirect", "url"])
                .ok_or_else(|| ApiError::legacy("unable get Alipay redirect url"))?,
        ),
        "wechat_pay" => (
            0,
            value_path_str(
                &intent,
                &["next_action", "wechat_pay_display_qr_code", "data"],
            )
            .ok_or_else(|| ApiError::legacy("unable get WeChat Pay redirect url"))?,
        ),
        _ => return Err(ApiError::legacy("Payment gateway request failed")),
    };
    Ok(CheckoutResult {
        r#type: result_type,
        data: json!(data),
    })
}

async fn stripe_all_cards_pay(
    service: &OrderService,
    payment_id: i32,
    config: &serde_json::Map<String, Value>,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let currency = stripe_currency(config)?;
    let exchange = exchange_cny_to(&currency).await?;
    let gateway_amount = stripe_payment_amount(order.total_amount, exchange, &currency)?;
    let mut params = BTreeMap::from([
        ("success_url".to_string(), order.return_url.clone()),
        ("client_reference_id".to_string(), order.trade_no.clone()),
        ("payment_method_types[0]".to_string(), "card".to_string()),
        (
            "line_items[0][price_data][currency]".to_string(),
            currency.clone(),
        ),
        (
            "line_items[0][price_data][unit_amount]".to_string(),
            gateway_amount.to_string(),
        ),
        (
            "line_items[0][price_data][product_data][name]".to_string(),
            format!(
                "user-#{}-{}",
                order.user_id,
                right_chars(&order.trade_no, 8)
            ),
        ),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
        ("mode".to_string(), "payment".to_string()),
        ("invoice_creation[enabled]".to_string(), "true".to_string()),
        (
            "phone_number_collection[enabled]".to_string(),
            "false".to_string(),
        ),
    ]);
    add_stripe_settlement_metadata(
        &mut params,
        "metadata",
        payment_id,
        order,
        gateway_amount,
        &currency,
    );
    add_stripe_settlement_metadata(
        &mut params,
        "payment_intent_data[metadata]",
        payment_id,
        order,
        gateway_amount,
        &currency,
    );
    if let Some(email) = service.user_email(order.user_id).await? {
        params.insert("customer_email".to_string(), email);
    }
    let session = stripe_post_with_config(config, "checkout/sessions", &params).await?;
    let url = session
        .get("url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(url),
    })
}

pub(in crate::order) async fn stripe_source_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "source.chargeable" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            let params = stripe_source_charge_params(payment, object)?;
            stripe_post_with_config(&config, "charges", &params).await?;
            Ok(PaymentNotifyOutcome::Ignored("success".to_string()))
        }
        "charge.succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            if object.get("status").and_then(Value::as_str) != Some("succeeded") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            stripe_charge_verified(payment, object)
        }
        _ => Err(ApiError::legacy("event is not support")),
    }
}

pub(in crate::order) fn stripe_payment_intent_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    if event.get("type").and_then(Value::as_str) != Some("payment_intent.succeeded") {
        return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
    }
    let object = event
        .pointer("/data/object")
        .ok_or_else(|| ApiError::legacy("Payment notify body is invalid"))?;
    stripe_payment_intent_verified(payment, object)
}

pub(in crate::order) fn stripe_checkout_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "checkout.session.completed" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            if object.get("payment_status").and_then(Value::as_str) != Some("paid") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            stripe_session_verified(payment, object)
        }
        "checkout.session.async_payment_succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            stripe_session_verified(payment, object)
        }
        _ => Err(ApiError::legacy("event is not support")),
    }
}

pub(in crate::order) fn stripe_all_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "payment_intent.succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            if object.get("status").and_then(Value::as_str) != Some("succeeded") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            stripe_payment_intent_verified(payment, object)
        }
        "checkout.session.completed" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            if object.get("payment_status").and_then(Value::as_str) != Some("paid") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            stripe_session_verified(payment, object)
        }
        "checkout.session.async_payment_succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            stripe_session_verified(payment, object)
        }
        _ => Err(ApiError::legacy("webhook events are not supported")),
    }
}

fn stripe_session_verified(
    payment: &PaymentForCheckout,
    object: &Value,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let metadata = stripe_settlement_metadata(payment, object)?;
    let client_reference_id = object
        .get("client_reference_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    if object.get("payment_status").and_then(Value::as_str) != Some("paid")
        || client_reference_id != metadata.trade_no.as_str()
        || !stripe_actual_amount_matches(
            &metadata,
            object.get("amount_total").and_then(Value::as_i64),
            object.get("currency").and_then(Value::as_str),
        )
    {
        return Err(ApiError::legacy(
            "Stripe Checkout session does not match the order",
        ));
    }
    let callback_no = object
        .get("payment_intent")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("pi_"))
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: metadata.trade_no,
        callback_no: callback_no.to_string(),
        custom_result: None,
        authenticated_user_id: Some(metadata.user_id),
        settled_amount_cents: Some(metadata.local_amount),
    }))
}

fn decimal_rate(value: &Value) -> Option<Decimal> {
    let Value::Number(number) = value else {
        return None;
    };
    let rate = number.to_string().parse::<Decimal>().ok()?;
    (rate > Decimal::ZERO).then_some(rate)
}

pub(in crate::order) fn exchange_rate_cache_decision(
    cached: Option<CachedExchangeRate>,
    now: i64,
) -> ExchangeRateCacheDecision {
    let Some(cached) = cached.filter(|cached| cached.rate > Decimal::ZERO) else {
        return ExchangeRateCacheDecision::Refresh { stale: None };
    };
    let age = now.saturating_sub(cached.fetched_at).max(0);
    if age <= EXCHANGE_RATE_FRESH_TTL_SECS {
        ExchangeRateCacheDecision::Fresh(cached.rate)
    } else if age <= EXCHANGE_RATE_STALE_TTL_SECS {
        ExchangeRateCacheDecision::Refresh {
            stale: Some(cached.rate),
        }
    } else {
        ExchangeRateCacheDecision::Refresh { stale: None }
    }
}

fn cached_exchange_rate(currency: &str) -> Option<CachedExchangeRate> {
    EXCHANGE_RATE_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(currency)
        .copied()
}

fn cache_exchange_rate(currency: &str, rate: Decimal, fetched_at: i64) {
    if rate <= Decimal::ZERO {
        return;
    }
    EXCHANGE_RATE_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            currency.to_string(),
            CachedExchangeRate { rate, fetched_at },
        );
}

async fn exchange_rate_refresh_lock(currency: &str) -> Arc<tokio::sync::Mutex<()>> {
    EXCHANGE_RATE_REFRESH_LOCKS
        .lock()
        .await
        .entry(currency.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

async fn fetch_exchange_rate(
    client: PaymentHttpClient,
    url: impl reqwest::IntoUrl,
    currency: &str,
) -> Option<Decimal> {
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let value: Value = crate::http_response::bounded_json(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Exchange-rate response is invalid",
    )
    .await
    .ok()?;
    value
        .get("rates")
        .and_then(|rates| rates.get(currency))
        .and_then(decimal_rate)
}

async fn exchange_cny_to(currency: &str) -> Result<Decimal, ApiError> {
    let currency = currency.trim().to_ascii_uppercase();
    if currency == "CNY" {
        return Ok(Decimal::ONE);
    }

    let now = Utc::now().timestamp();
    if let ExchangeRateCacheDecision::Fresh(rate) =
        exchange_rate_cache_decision(cached_exchange_rate(&currency), now)
    {
        return Ok(rate);
    }

    // Coalesce refreshes only for the same currency. A slow provider for one
    // currency must not hold up otherwise independent payment preparations.
    let refresh_lock = exchange_rate_refresh_lock(&currency).await;
    let _refresh_guard = refresh_lock.lock().await;
    let now = Utc::now().timestamp();
    match exchange_rate_cache_decision(cached_exchange_rate(&currency), now) {
        ExchangeRateCacheDecision::Fresh(rate) => return Ok(rate),
        ExchangeRateCacheDecision::Refresh { .. } => {}
    }

    let client = payment_http_client("V2Board-Rust-Currency");
    if let Some(rate) = fetch_exchange_rate(
        client,
        "https://api.exchangerate-api.com/v4/latest/CNY",
        &currency,
    )
    .await
    {
        cache_exchange_rate(&currency, rate, Utc::now().timestamp());
        return Ok(rate);
    }

    if let Some(rate) = fetch_exchange_rate(
        client,
        format!("https://api.frankfurter.app/latest?from=CNY&to={currency}"),
        &currency,
    )
    .await
    {
        cache_exchange_rate(&currency, rate, Utc::now().timestamp());
        return Ok(rate);
    }

    if let ExchangeRateCacheDecision::Refresh { stale: Some(rate) } =
        exchange_rate_cache_decision(cached_exchange_rate(&currency), Utc::now().timestamp())
    {
        tracing::warn!(
            currency,
            "both currency-rate providers failed; using a stale cached rate"
        );
        return Ok(rate);
    }

    Err(ApiError::legacy(
        "Currency conversion has timed out, please try again later",
    ))
}

pub(in crate::order) fn stripe_payment_amount(
    cny_cents: i32,
    exchange: Decimal,
    currency: &str,
) -> Result<i64, ApiError> {
    const ZERO_DECIMAL: &[&str] = &[
        "BIF", "CLP", "DJF", "GNF", "JPY", "KMF", "KRW", "MGA", "PYG", "RWF", "UGX", "VND", "VUV",
        "XAF", "XOF", "XPF",
    ];
    const THREE_DECIMAL: &[&str] = &["BHD", "JOD", "KWD", "OMR", "TND"];
    let currency = currency.trim().to_ascii_uppercase();
    let target_units = Decimal::from(cny_cents.max(0))
        .checked_div(Decimal::from(100))
        .and_then(|amount| amount.checked_mul(exchange))
        .ok_or_else(|| {
            ApiError::legacy("Converted payment amount is outside the supported range")
        })?;
    // Stripe represents ISK with two API decimals for backwards compatibility,
    // while requiring the final two digits to stay 00.
    if currency == "ISK" {
        return target_units
            .floor()
            .to_i64()
            .and_then(|amount| amount.checked_mul(100))
            .ok_or_else(|| {
                ApiError::legacy("Converted payment amount is outside the supported range")
            });
    }
    let exponent = if ZERO_DECIMAL.contains(&currency.as_str()) {
        0
    } else if THREE_DECIMAL.contains(&currency.as_str()) {
        3
    } else {
        2
    };
    let minor_units = Decimal::from(10_i64.pow(exponent as u32));
    target_units
        .checked_mul(minor_units)
        .ok_or_else(|| ApiError::legacy("Converted payment amount is outside the supported range"))?
        .floor()
        .to_i64()
        .ok_or_else(|| ApiError::legacy("Converted payment amount is outside the supported range"))
}

async fn stripe_post(
    config: &serde_json::Map<String, Value>,
    path: &str,
    params: &BTreeMap<String, String>,
) -> Result<Value, ApiError> {
    stripe_post_with_config(config, path, params).await
}

async fn stripe_post_with_config(
    config: &serde_json::Map<String, Value>,
    path: &str,
    params: &BTreeMap<String, String>,
) -> Result<Value, ApiError> {
    stripe_post_request(config, path, params, None).await
}

async fn stripe_post_with_idempotency(
    config: &serde_json::Map<String, Value>,
    path: &str,
    params: &BTreeMap<String, String>,
    idempotency_key: &str,
) -> Result<Value, ApiError> {
    stripe_post_request(config, path, params, Some(idempotency_key)).await
}

async fn stripe_post_request(
    config: &serde_json::Map<String, Value>,
    path: &str,
    params: &BTreeMap<String, String>,
    idempotency_key: Option<&str>,
) -> Result<Value, ApiError> {
    let mut request = payment_http_client("Stripe")
        .post(format!("https://api.stripe.com/v1/{path}"))
        .bearer_auth(config_required(config, "stripe_sk_live")?)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_query(params)?);
    if let Some(key) = idempotency_key {
        request = request.header("Idempotency-Key", key);
    }
    let response = request
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    stripe_response(response).await
}

async fn stripe_get_with_config(
    config: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<Value, ApiError> {
    let response = payment_http_client("Stripe")
        .get(format!("https://api.stripe.com/v1/{path}"))
        .bearer_auth(config_required(config, "stripe_sk_live")?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    stripe_response(response).await
}

pub(in crate::order) async fn stripe_cancel_intent(
    config: &serde_json::Map<String, Value>,
    intent_id: &str,
) -> Result<bool, ApiError> {
    let intent = stripe_get_with_config(config, &format!("payment_intents/{intent_id}")).await?;
    match intent.get("status").and_then(Value::as_str) {
        Some("succeeded") => Ok(false),
        Some("canceled") => Ok(true),
        _ => {
            stripe_post_with_idempotency(
                config,
                &format!("payment_intents/{intent_id}/cancel"),
                &BTreeMap::new(),
                &format!("v2board:cancel:{intent_id}"),
            )
            .await?;
            Ok(true)
        }
    }
}

async fn stripe_response(response: reqwest::Response) -> Result<Value, ApiError> {
    let status = response.status();
    let text = crate::http_response::bounded_text(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Payment gateway request failed",
    )
    .await?;
    let value = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "raw": text }));
    if !status.is_success() {
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Payment gateway request failed");
        return Err(ApiError::legacy(message));
    }
    Ok(value)
}

fn stripe_event(
    config: &serde_json::Map<String, Value>,
    input: &PaymentNotifyInput,
) -> Result<Value, ApiError> {
    let signature = header_value(&input.headers, "stripe-signature")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let timestamp = signature
        .split(',')
        .find_map(|part| part.trim().strip_prefix("t="))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let signed_payload = format!("{}.", timestamp)
        .as_bytes()
        .iter()
        .copied()
        .chain(input.body.iter().copied())
        .collect::<Vec<_>>();
    let secret = config_required(config, "stripe_webhook_key")?;
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    let mut verified = false;
    for encoded_signature in signature
        .split(',')
        .filter_map(|part| part.trim().strip_prefix("v1="))
    {
        let Ok(candidate) = hex::decode(encoded_signature) else {
            continue;
        };
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
            .map_err(|_| ApiError::internal("invalid hmac key"))?;
        mac.update(&signed_payload);
        if mac.verify_slice(&candidate).is_ok() {
            verified = true;
            break;
        }
    }
    if !verified {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    // Enforce Stripe's default 300s replay tolerance (matching
    // \Stripe\Webhook::constructEvent). Without this a validly-signed event can
    // be replayed indefinitely.
    let signed_at = timestamp
        .parse::<i64>()
        .map_err(|_| ApiError::legacy("HMAC signature does not match"))?;
    if Utc::now().timestamp().abs_diff(signed_at) > STRIPE_WEBHOOK_TOLERANCE_SECS as u64 {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    serde_json::from_slice::<Value>(&input.body)
        .map_err(|_| ApiError::legacy("Payment notify body is invalid"))
}

/// Stripe's default webhook timestamp tolerance, in seconds.
pub(in crate::order) const STRIPE_WEBHOOK_TOLERANCE_SECS: i64 = 300;

fn add_metadata_params(
    params: &mut BTreeMap<String, String>,
    prefix: &str,
    metadata: Option<&Value>,
) {
    let Some(metadata) = metadata.and_then(Value::as_object) else {
        return;
    };
    for (key, value) in metadata {
        if let Some(value) = json_scalar_string(value) {
            params.insert(format!("{prefix}[{key}]"), value);
        }
    }
}

pub(super) fn value_path_str(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    json_scalar_string(current)
}

fn json_scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}
