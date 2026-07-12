use std::{
    collections::{BTreeMap, HashMap},
    sync::LazyLock,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    sign::{Signer, Verifier},
};
use quick_xml::{
    Writer,
    events::{BytesEnd, BytesStart, BytesText, Event},
};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Sha256, Sha512};
use url::Url;
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::{AppConfig, app_now};

use super::{
    CheckoutResult, OrderService, PaymentForCheckout, PaymentNotifyInput, PaymentNotifyOutcome,
    PaymentOrder, StripePaymentIntentResult, VerifiedPaymentNotify,
};
use crate::payment_provider::{PaymentProviderManifest, payment_provider_manifest};

mod stripe;

pub(super) use stripe::{
    stripe_all_notify, stripe_all_pay, stripe_cancel_intent, stripe_checkout_notify,
    stripe_checkout_pay, stripe_credit_prepare, stripe_payment_intent_notify, stripe_source_notify,
    stripe_source_pay,
};

use stripe::value_path_str;

#[cfg(test)]
pub(super) use stripe::{
    CachedExchangeRate, EXCHANGE_RATE_FRESH_TTL_SECS, EXCHANGE_RATE_STALE_TTL_SECS,
    ExchangeRateCacheDecision, STRIPE_WEBHOOK_TOLERANCE_SECS, add_stripe_settlement_metadata,
    exchange_rate_cache_decision, reusable_stripe_credit_intent_matches, stripe_payment_amount,
    stripe_source_charge_params,
};

static PAYMENT_HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .expect("static payment HTTP client configuration must be valid")
});
#[derive(Debug, Clone, Copy)]
pub(super) struct PaymentHttpClient {
    user_agent: &'static str,
}

impl PaymentHttpClient {
    pub(super) fn get<U: reqwest::IntoUrl>(self, url: U) -> reqwest::RequestBuilder {
        PAYMENT_HTTP_CLIENT
            .get(url)
            .header(reqwest::header::USER_AGENT, self.user_agent)
    }

    pub(super) fn post<U: reqwest::IntoUrl>(self, url: U) -> reqwest::RequestBuilder {
        PAYMENT_HTTP_CLIENT
            .post(url)
            .header(reqwest::header::USER_AGENT, self.user_agent)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename = "xml")]
pub(super) struct WechatUnifiedOrderResponse {
    pub(super) return_code: String,
    pub(super) return_msg: Option<String>,
    pub(super) code_url: Option<String>,
}

pub(super) fn require_payment_provider(
    method: &str,
) -> Result<&'static PaymentProviderManifest, ApiError> {
    payment_provider_manifest(method)
        .ok_or_else(|| ApiError::legacy(format!("Payment gateway {method} is not supported")))
}

pub(super) fn epay_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let key = config_required(&config, "key")?;
    let mut params = BTreeMap::from([
        ("money".to_string(), amount_yuan_string(order.total_amount)),
        ("name".to_string(), order.trade_no.clone()),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("return_url".to_string(), order.return_url.clone()),
        ("out_trade_no".to_string(), order.trade_no.clone()),
        ("pid".to_string(), config_required(&config, "pid")?),
    ]);
    if let Some(payment_type) = config_string(&config, "type").filter(|value| !value.is_empty()) {
        params.insert("type".to_string(), payment_type);
    }
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&params), key))
    );
    params.insert("sign".to_string(), signature);
    params.insert("sign_type".to_string(), "MD5".to_string());
    let base_url = config_required(&config, "url")?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(format!(
            "{}/submit.php?{}",
            base_url.trim_end_matches('/'),
            form_query(&params)?
        )),
    })
}

pub(super) fn epay_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let sign = params
        .get("sign")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify signature is missing"))?;
    let config = payment_config(payment)?;
    let key = config_required(&config, "key")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "sign" && key.as_str() != "sign_type")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), key))
    );
    if !verify_legacy_md5_hex(&expected, sign) {
        return Err(ApiError::legacy("Payment notify signature is invalid"));
    }
    if params
        .get("trade_status")
        .map(String::as_str)
        .unwrap_or_default()
        != "TRADE_SUCCESS"
    {
        return Ok(PaymentNotifyOutcome::Ignored("fail".to_string()));
    }
    let trade_no = signed
        .remove("out_trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let callback_no = signed
        .remove("trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no,
        callback_no,
        custom_result: None,
        authenticated_user_id: None,
        settled_amount_cents: Some(decimal_amount_cents(&required_param(params, "money")?)?),
    }))
}

pub(super) async fn mgate_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let secret = config_required(&config, "mgate_app_secret")?;
    let mut params = BTreeMap::from([
        ("out_trade_no".to_string(), order.trade_no.clone()),
        ("total_amount".to_string(), order.total_amount.to_string()),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("return_url".to_string(), order.return_url.clone()),
        (
            "app_id".to_string(),
            config_required(&config, "mgate_app_id")?,
        ),
    ]);
    if let Some(currency) =
        config_string(&config, "mgate_source_currency").filter(|value| !value.is_empty())
    {
        params.insert("source_currency".to_string(), currency);
    }
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", form_query(&params)?, secret))
    );
    params.insert("sign".to_string(), signature);
    let base_url = config_required(&config, "mgate_url")?;
    let response = payment_http_client("MGate")
        .post(format!(
            "{}/v1/gateway/fetch",
            base_url.trim_end_matches('/')
        ))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_query(&params)?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("网络异常"))?;
    let result: Value = crate::http_response::bounded_json(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "接口请求失败",
    )
    .await?;

    if result
        .pointer("/data/trade_no")
        .and_then(|value| value.as_str())
        .is_none()
    {
        return Err(ApiError::legacy(payment_gateway_message(
            &result,
            "接口请求失败",
        )));
    }
    let pay_url = result
        .pointer("/data/pay_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("接口请求失败"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(pay_url),
    })
}

pub(super) async fn bepusdt_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let api_token = config_required(&config, "bepusdt_apitoken")?;
    let mut signed = BTreeMap::from([
        ("amount".to_string(), amount_yuan_string(order.total_amount)),
        (
            "trade_type".to_string(),
            config_required(&config, "bepusdt_trade_type")?,
        ),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("order_id".to_string(), order.trade_no.clone()),
        ("redirect_url".to_string(), order.return_url.clone()),
    ]);
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), api_token))
    );
    signed.insert("signature".to_string(), signature);

    let mut json_params = serde_json::Map::new();
    for (key, value) in &signed {
        if key == "amount" {
            json_params.insert(key.clone(), json_number(value, "invalid bepusdt amount")?);
        } else {
            json_params.insert(key.clone(), json!(value));
        }
    }

    let base_url = config_required(&config, "bepusdt_url")?;
    let response = payment_http_client("BEPUSDT")
        .post(format!(
            "{}/api/v1/order/create-transaction",
            base_url.trim_end_matches('/')
        ))
        .json(&json_params)
        .send()
        .await
        .map_err(|_| ApiError::legacy("网络异常"))?;
    let result: Value = crate::http_response::bounded_json(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "接口请求失败",
    )
    .await?;
    if result
        .get("status_code")
        .and_then(|value| value.as_i64())
        .unwrap_or_default()
        != 200
    {
        return Err(ApiError::legacy(format!(
            "Failed to create order. Error: {}",
            payment_gateway_message(&result, "接口请求失败")
        )));
    }
    let payment_url = result
        .pointer("/data/payment_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("接口请求失败"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(payment_url),
    })
}

pub(super) fn mgate_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let sign = params
        .get("sign")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify signature is missing"))?;
    let config = payment_config(payment)?;
    let secret = config_required(&config, "mgate_app_secret")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "sign")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = format!(
        "{:x}",
        md5::compute(format!("{}{}", form_query(&signed)?, secret))
    );
    if !verify_legacy_md5_hex(&expected, sign) {
        return Err(ApiError::legacy("Payment notify signature is invalid"));
    }
    if let Some(status) = params
        .get("status")
        .or_else(|| params.get("trade_status"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        && !matches!(
            status.to_ascii_lowercase().as_str(),
            "1" | "2" | "paid" | "success" | "succeeded" | "trade_success"
        )
    {
        return Ok(PaymentNotifyOutcome::Ignored("failed".to_string()));
    }
    let trade_no = signed
        .remove("out_trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let callback_no = signed
        .remove("trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no,
        callback_no,
        custom_result: None,
        authenticated_user_id: None,
        settled_amount_cents: Some(integer_amount_cents(&required_param(
            params,
            "total_amount",
        )?)?),
    }))
}

pub(super) fn bepusdt_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let sign = params
        .get("signature")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify signature is missing"))?;
    let config = payment_config(payment)?;
    let api_token = config_required(&config, "bepusdt_apitoken")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "signature")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), api_token))
    );
    if !verify_legacy_md5_hex(&expected, sign) {
        return Ok(PaymentNotifyOutcome::Ignored(
            "cannot pass verification".to_string(),
        ));
    }
    if params.get("status").map(String::as_str).unwrap_or_default() != "2" {
        return Ok(PaymentNotifyOutcome::Ignored("failed".to_string()));
    }
    let trade_no = signed
        .remove("order_id")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let callback_no = signed
        .remove("trade_id")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no,
        callback_no,
        custom_result: Some("ok".to_string()),
        authenticated_user_id: None,
        settled_amount_cents: Some(decimal_amount_cents(&required_param(params, "amount")?)?),
    }))
}

pub(super) fn coinpayments_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let params = BTreeMap::from([
        ("cmd".to_string(), "_pay_simple".to_string()),
        ("reset".to_string(), "1".to_string()),
        (
            "merchant".to_string(),
            config_required(&config, "coinpayments_merchant_id")?,
        ),
        ("item_name".to_string(), order.trade_no.clone()),
        ("item_number".to_string(), order.trade_no.clone()),
        ("want_shipping".to_string(), "0".to_string()),
        (
            "currency".to_string(),
            config_required(&config, "coinpayments_currency")?,
        ),
        (
            "amountf".to_string(),
            amount_yuan_fixed_string(order.total_amount),
        ),
        ("success_url".to_string(), url_origin(&order.return_url)),
        ("cancel_url".to_string(), order.return_url.clone()),
        ("ipn_url".to_string(), order.notify_url.clone()),
    ]);
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(format!(
            "https://www.coinpayments.net/index.php?{}",
            form_query(&params)?
        )),
    })
}

pub(super) fn coinpayments_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let merchant = config_required(&config, "coinpayments_merchant_id")?;
    if input.params.get("merchant").map(|value| value.trim()) != Some(merchant.trim()) {
        return Err(ApiError::legacy("No or incorrect Merchant ID passed"));
    }
    let secret = config_required(&config, "coinpayments_ipn_secret")?;
    let signed = input
        .params
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let request = form_query(&signed)?;
    let actual = header_value(&input.headers, "hmac")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    if !verify_hmac_sha512_hex(secret.as_bytes(), request.as_bytes(), &actual)? {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    let expected_currency = config_required(&config, "coinpayments_currency")?;
    require_matching_currency(
        required_param(&input.params, "currency1")?.as_str(),
        &expected_currency,
    )?;
    let settled_amount_cents = decimal_amount_cents(&required_param(&input.params, "amount1")?)?;
    let status = input
        .params
        .get("status")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("Payment notify status is invalid"))?;
    if status >= 100 || status == 2 {
        return Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
            trade_no: required_param(&input.params, "item_number")?,
            callback_no: required_param(&input.params, "txn_id")?,
            custom_result: Some("IPN OK".to_string()),
            authenticated_user_id: None,
            settled_amount_cents: Some(settled_amount_cents),
        }));
    }
    if status < 0 {
        return Err(ApiError::legacy("Payment Timed Out or Error"));
    }
    Ok(PaymentNotifyOutcome::Ignored("IPN OK: pending".to_string()))
}

pub(super) async fn coinbase_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let params = BTreeMap::from([
        ("name".to_string(), "订阅套餐".to_string()),
        (
            "description".to_string(),
            format!("订单号 {}", order.trade_no),
        ),
        ("pricing_type".to_string(), "fixed_price".to_string()),
        (
            "local_price[amount]".to_string(),
            amount_yuan_fixed_string(order.total_amount),
        ),
        ("local_price[currency]".to_string(), "CNY".to_string()),
        ("metadata[outTradeNo]".to_string(), order.trade_no.clone()),
    ]);
    let response = payment_http_client("Coinbase")
        .post(config_required(&config, "coinbase_url")?)
        .header(
            "X-CC-Api-Key",
            config_required(&config, "coinbase_api_key")?,
        )
        .header("X-CC-Version", "2018-03-22")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_query(&params)?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let result: Value = crate::http_response::bounded_json(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Payment gateway request failed",
    )
    .await?;
    let hosted_url = result
        .pointer("/data/hosted_url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("error!"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(hosted_url),
    })
}

pub(super) fn coinbase_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let actual = header_value(&input.headers, "x-cc-webhook-signature")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    if !verify_hmac_sha256_hex(
        config_required(&config, "coinbase_webhook_key")?.as_bytes(),
        &input.body,
        &actual,
    )? {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    // HMAC covers the byte-for-byte request body. Decode JSON only after the
    // signature succeeds; trimming or lossy UTF-8 conversion changes the signed
    // message and can make two distinct requests appear equivalent.
    let value = serde_json::from_slice::<Value>(&input.body)
        .map_err(|_| ApiError::legacy("Payment notify body is invalid"))?;
    if value_path_str(&value, &["event", "type"]).as_deref() != Some("charge:confirmed") {
        return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
    }
    let currency = value_path_str(&value, &["event", "data", "pricing", "local", "currency"])
        .ok_or_else(|| ApiError::legacy("Payment notify currency is missing"))?;
    require_matching_currency(&currency, "CNY")?;
    let settled_amount_cents = decimal_amount_cents(
        &value_path_str(&value, &["event", "data", "pricing", "local", "amount"])
            .ok_or_else(|| ApiError::legacy("Payment notify amount is missing"))?,
    )?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: value_path_str(&value, &["event", "data", "metadata", "outTradeNo"])
            .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?,
        callback_no: value_path_str(&value, &["event", "id"])
            .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?,
        custom_result: None,
        authenticated_user_id: None,
        settled_amount_cents: Some(settled_amount_cents),
    }))
}

pub(super) async fn btcpay_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let payload = json!({
        "jsonResponse": true,
        "amount": amount_yuan_fixed_string(order.total_amount),
        "currency": "CNY",
        "metadata": { "orderId": order.trade_no },
    });
    let base = config_required(&config, "btcpay_url")?;
    let store_id = config_required(&config, "btcpay_storeId")?;
    let response = payment_http_client("BTCPay")
        .post(format!(
            "{}api/v1/stores/{store_id}/invoices",
            ensure_trailing_slash(&base)
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("token {}", config_required(&config, "btcpay_api_key")?),
        )
        .json(&payload)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let result: Value = crate::http_response::bounded_json(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Payment gateway request failed",
    )
    .await?;
    let checkout_link = result
        .get("checkoutLink")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("error!"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(checkout_link),
    })
}

pub(super) async fn btcpay_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let actual = header_value(&input.headers, "btcpay-sig")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let Some(actual) = actual.strip_prefix("sha256=") else {
        return Err(ApiError::legacy("HMAC signature does not match"));
    };
    if !verify_hmac_sha256_hex(
        config_required(&config, "btcpay_webhook_key")?.as_bytes(),
        &input.body,
        actual,
    )? {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    let body = serde_json::from_slice::<Value>(&input.body)
        .map_err(|_| ApiError::legacy("Payment notify body is invalid"))?;
    if body.get("type").and_then(Value::as_str) != Some("InvoiceSettled") {
        return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
    }
    let invoice_id = body
        .get("invoiceId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    let base = config_required(&config, "btcpay_url")?;
    let store_id = config_required(&config, "btcpay_storeId")?;
    let response = payment_http_client("BTCPay")
        .get(format!(
            "{}api/v1/stores/{store_id}/invoices/{invoice_id}",
            ensure_trailing_slash(&base)
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("token {}", config_required(&config, "btcpay_api_key")?),
        )
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?
        .error_for_status()
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let invoice: Value = crate::http_response::bounded_json(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Payment gateway request failed",
    )
    .await?;
    let settlement = btcpay_invoice_settlement(&invoice, invoice_id)?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: settlement.trade_no,
        callback_no: invoice_id.to_string(),
        custom_result: None,
        authenticated_user_id: None,
        settled_amount_cents: Some(settlement.amount_cents),
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BtcpayInvoiceSettlement {
    pub(super) trade_no: String,
    pub(super) amount_cents: i64,
}

pub(super) fn btcpay_invoice_settlement(
    invoice: &Value,
    expected_invoice_id: &str,
) -> Result<BtcpayInvoiceSettlement, ApiError> {
    if let Some(invoice_id) = invoice.get("id")
        && invoice_id.as_str() != Some(expected_invoice_id)
    {
        return Err(ApiError::legacy(
            "BTCPay invoice does not match the webhook",
        ));
    }
    if invoice.get("status").and_then(Value::as_str) != Some("Settled") {
        return Err(ApiError::legacy("BTCPay invoice is not settled"));
    }
    match invoice.get("additionalStatus") {
        None | Some(Value::Null) => {}
        Some(Value::String(status)) if status == "None" || status == "PaidOver" => {}
        _ => return Err(ApiError::legacy("BTCPay invoice settlement is not final")),
    }
    let trade_no = value_path_str(invoice, &["metadata", "orderId"])
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let currency = value_path_str(invoice, &["currency"])
        .ok_or_else(|| ApiError::legacy("Payment notify currency is missing"))?;
    require_matching_currency(&currency, "CNY")?;
    let amount = value_path_str(invoice, &["amount"])
        .ok_or_else(|| ApiError::legacy("Payment notify amount is missing"))?;
    Ok(BtcpayInvoiceSettlement {
        trade_no,
        amount_cents: decimal_amount_cents(&amount)?,
    })
}

pub(super) async fn wechat_pay_native_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let api_key = config_required(&config, "api_key")?;
    let mut params = BTreeMap::from([
        ("appid".to_string(), config_required(&config, "app_id")?),
        ("mch_id".to_string(), config_required(&config, "mch_id")?),
        ("nonce_str".to_string(), Uuid::new_v4().simple().to_string()),
        ("body".to_string(), order.trade_no.clone()),
        ("out_trade_no".to_string(), order.trade_no.clone()),
        ("total_fee".to_string(), order.total_amount.to_string()),
        ("spbill_create_ip".to_string(), "0.0.0.0".to_string()),
        ("fee_type".to_string(), "CNY".to_string()),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("trade_type".to_string(), "NATIVE".to_string()),
    ]);
    params.insert("sign".to_string(), wechat_sign(&params, &api_key));
    let response = payment_http_client("WechatPayNative")
        .post("https://api.mch.weixin.qq.com/pay/unifiedorder")
        .header(reqwest::header::CONTENT_TYPE, "text/xml")
        .body(xml_from_params(&params)?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let response = crate::http_response::bounded_text(
        response,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Payment gateway request failed",
    )
    .await?;
    let response_params = quick_xml::de::from_str::<WechatUnifiedOrderResponse>(&response)
        .map_err(|_| ApiError::legacy("Payment gateway response is invalid"))?;
    if response_params.return_code != "SUCCESS" {
        return Err(ApiError::legacy(
            response_params
                .return_msg
                .unwrap_or_else(|| "Payment gateway request failed".to_string()),
        ));
    }
    let code_url = response_params
        .code_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    Ok(CheckoutResult {
        r#type: 0,
        data: json!(code_url),
    })
}

pub(super) fn wechat_pay_native_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let api_key = config_required(&config, "api_key")?;
    let sign = input
        .params
        .get("sign")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let signed = input
        .params
        .iter()
        .filter(|(key, value)| key.as_str() != "sign" && !value.is_empty())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if !verify_legacy_md5_hex(&wechat_sign(&signed, &api_key), sign) {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    if input.params.get("return_code").map(String::as_str) != Some("SUCCESS")
        || input.params.get("result_code").map(String::as_str) != Some("SUCCESS")
    {
        return Ok(PaymentNotifyOutcome::Ignored("FAIL".to_string()));
    }
    if let Some(currency) = input.params.get("fee_type") {
        require_matching_currency(currency, "CNY")?;
    }
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: required_param(&input.params, "out_trade_no")?,
        callback_no: required_param(&input.params, "transaction_id")?,
        custom_result: Some(
            "<xml><return_code><![CDATA[SUCCESS]]></return_code><return_msg><![CDATA[OK]]></return_msg></xml>"
                .to_string(),
        ),
        authenticated_user_id: None,
        settled_amount_cents: Some(integer_amount_cents(&required_param(
            &input.params,
            "total_fee",
        )?)?),
    }))
}

pub(super) async fn alipay_f2f_pay(
    app_config: &AppConfig,
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let subject = config_string(&config, "product_name")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{} - 订阅", app_config.app_name));
    // PHP's `$order['total_amount'] / 100` returns an int when the cents divide
    // evenly (`200/100 → 2`) and a float otherwise (`250/100 → 2.5`); json_encode
    // then bakes that int-vs-float choice into the RSA2-signed biz_content bytes.
    // Mirror it so whole-yuan orders sign as `2`, not `2.0`.
    let total_amount = json_number(
        &amount_yuan_string(order.total_amount),
        "failed to encode alipay amount",
    )?;
    let biz_content = serde_json::to_string(&json!({
        "subject": subject,
        "out_trade_no": order.trade_no,
        "total_amount": total_amount,
    }))
    .map_err(|_| ApiError::internal("failed to build alipay payload"))?;
    let mut params = BTreeMap::from([
        ("app_id".to_string(), config_required(&config, "app_id")?),
        ("method".to_string(), "alipay.trade.precreate".to_string()),
        ("charset".to_string(), "UTF-8".to_string()),
        ("sign_type".to_string(), "RSA2".to_string()),
        (
            "timestamp".to_string(),
            app_now().format("%Y-%m-%d %H:%M:%S").to_string(),
        ),
        ("biz_content".to_string(), biz_content),
        ("version".to_string(), "1.0".to_string()),
        ("_input_charset".to_string(), "UTF-8".to_string()),
        ("notify_url".to_string(), order.notify_url.clone()),
    ]);
    let signature = alipay_sign(
        &config_required(&config, "private_key")?,
        &canonical_query(&params),
    )?;
    params.insert("sign".to_string(), signature);
    let upstream = payment_http_client("AlipayF2F")
        .get(format!(
            "https://openapi.alipay.com/gateway.do?{}",
            form_query(&params)?
        ))
        .send()
        .await
        .map_err(|_| ApiError::legacy("从支付宝请求失败"))?;
    let response: Value = crate::http_response::bounded_json(
        upstream,
        crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "从支付宝请求失败",
    )
    .await?;
    let response = response
        .get("alipay_trade_precreate_response")
        .ok_or_else(|| ApiError::legacy("从支付宝请求失败"))?;
    if response.get("msg").and_then(Value::as_str) != Some("Success") {
        return Err(ApiError::legacy(
            response
                .get("sub_msg")
                .and_then(Value::as_str)
                .unwrap_or("从支付宝请求失败"),
        ));
    }
    let qr_code = response
        .get("qr_code")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("获取付款二维码失败"))?;
    Ok(CheckoutResult {
        r#type: 0,
        data: json!(qr_code),
    })
}

pub(super) fn alipay_f2f_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    if params.get("trade_status").map(String::as_str) != Some("TRADE_SUCCESS") {
        return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
    }
    let config = payment_config(payment)?;
    let sign = required_param(params, "sign")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "sign" && key.as_str() != "sign_type")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let public_key = config_required(&config, "public_key")?;
    if !alipay_verify(&public_key, &canonical_query(&signed), &sign)? {
        return Err(ApiError::legacy("verify error"));
    }
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: signed
            .remove("out_trade_no")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?,
        callback_no: signed
            .remove("trade_no")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?,
        custom_result: None,
        authenticated_user_id: None,
        settled_amount_cents: Some(decimal_amount_cents(&required_param(
            params,
            "total_amount",
        )?)?),
    }))
}

pub(super) fn payment_notify_url(config: &AppConfig, payment: &PaymentForCheckout) -> String {
    let path = format!(
        "/api/v1/guest/payment/notify/{}/{}",
        payment.payment, payment.uuid
    );
    if let Some(domain) = payment
        .notify_domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}{}", domain.trim_end_matches('/'), path);
    }
    if let Some(app_url) = config
        .app_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}{}", app_url.trim_end_matches('/'), path);
    }
    path
}

pub(super) fn payment_return_url(config: &AppConfig, trade_no: &str) -> String {
    if let Some(app_url) = config
        .app_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}/#/order/{trade_no}", app_url.trim_end_matches('/'));
    }
    format!("/#/order/{trade_no}")
}

pub(super) fn payment_config(
    payment: &PaymentForCheckout,
) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
    match serde_json::from_str::<serde_json::Value>(&payment.config)
        .map_err(|_| ApiError::legacy("Payment config is invalid"))?
    {
        serde_json::Value::Object(config) => Ok(config),
        _ => Err(ApiError::legacy("Payment config is invalid")),
    }
}

fn config_required(
    config: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, ApiError> {
    config_string(config, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy(format!("Payment config {key} is missing")))
}

pub(super) fn config_string(
    config: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    match config.get(key)? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

fn amount_yuan_string(cents: i32) -> String {
    let mut amount = amount_yuan_fixed_string(cents);
    while amount.contains('.') && amount.ends_with('0') {
        amount.pop();
    }
    if amount.ends_with('.') {
        amount.pop();
    }
    amount
}

fn json_number(value: &str, error: &'static str) -> Result<Value, ApiError> {
    value
        .parse::<serde_json::Number>()
        .map(Value::Number)
        .map_err(|_| ApiError::internal(error))
}

fn amount_yuan_fixed_string(cents: i32) -> String {
    let cents = i64::from(cents);
    let sign = if cents < 0 { "-" } else { "" };
    let absolute = cents.abs();
    format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
}

pub(super) fn decimal_amount_cents(value: &str) -> Result<i64, ApiError> {
    let amount = value
        .trim()
        .parse::<Decimal>()
        .map_err(|_| ApiError::legacy("Payment notify amount is invalid"))?;
    if amount <= Decimal::ZERO {
        return Err(ApiError::legacy("Payment notify amount is invalid"));
    }
    let cents = amount
        .checked_mul(Decimal::from(100))
        .ok_or_else(|| ApiError::legacy("Payment notify amount is invalid"))?;
    if cents != cents.trunc() {
        return Err(ApiError::legacy("Payment notify amount is invalid"));
    }
    cents
        .to_i64()
        .ok_or_else(|| ApiError::legacy("Payment notify amount is invalid"))
}

fn integer_amount_cents(value: &str) -> Result<i64, ApiError> {
    value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|amount| *amount > 0)
        .ok_or_else(|| ApiError::legacy("Payment notify amount is invalid"))
}

fn require_matching_currency(actual: &str, expected: &str) -> Result<(), ApiError> {
    let actual = actual.trim();
    let expected = expected.trim();
    if actual.is_empty() || expected.is_empty() || !actual.eq_ignore_ascii_case(expected) {
        return Err(ApiError::legacy(
            "Payment notify currency does not match the payment method",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> Result<String, ApiError> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
        .map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
pub(super) fn hmac_sha512_hex(key: &[u8], payload: &[u8]) -> Result<String, ApiError> {
    let mut mac = <Hmac<Sha512> as KeyInit>::new_from_slice(key)
        .map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn verify_hmac_sha256_hex(
    key: &[u8],
    payload: &[u8],
    encoded_signature: &str,
) -> Result<bool, ApiError> {
    let Ok(signature) = hex::decode(encoded_signature) else {
        return Ok(false);
    };
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
        .map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(payload);
    Ok(mac.verify_slice(&signature).is_ok())
}

fn verify_hmac_sha512_hex(
    key: &[u8],
    payload: &[u8],
    encoded_signature: &str,
) -> Result<bool, ApiError> {
    let Ok(signature) = hex::decode(encoded_signature) else {
        return Ok(false);
    };
    let mut mac = <Hmac<Sha512> as KeyInit>::new_from_slice(key)
        .map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(payload);
    Ok(mac.verify_slice(&signature).is_ok())
}

pub(super) fn verify_legacy_md5_hex(expected: &str, supplied: &str) -> bool {
    let Ok(expected) = hex::decode(expected) else {
        return false;
    };
    let Ok(supplied) = hex::decode(supplied) else {
        return false;
    };
    if expected.len() != 16 || supplied.len() != 16 {
        return false;
    }
    v2board_compat::constant_time_bytes_eq(&expected, &supplied)
}

fn header_value(headers: &HashMap<String, String>, name: &str) -> Option<String> {
    headers
        .get(&name.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .cloned()
}

fn required_param(params: &HashMap<String, String>, key: &str) -> Result<String, ApiError> {
    params
        .get(key)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::legacy(format!("Payment notify {key} is missing")))
}

fn ensure_trailing_slash(value: &str) -> String {
    let value = value.trim();
    if value.ends_with('/') {
        value.to_string()
    } else {
        format!("{value}/")
    }
}

pub(super) fn url_origin(value: &str) -> String {
    Url::parse(value)
        .ok()
        .map(|url| url.origin().ascii_serialization())
        .filter(|origin| origin != "null")
        .unwrap_or_else(|| value.to_string())
}

fn right_chars(value: &str, count: usize) -> String {
    let length = value.chars().count();
    value.chars().skip(length.saturating_sub(count)).collect()
}

pub(super) fn wechat_sign(params: &BTreeMap<String, String>, api_key: &str) -> String {
    let data = params
        .iter()
        .filter(|(key, value)| key.as_str() != "sign" && !value.is_empty())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{:X}", md5::compute(format!("{data}&key={api_key}")))
}

pub(super) fn xml_from_params(params: &BTreeMap<String, String>) -> Result<String, ApiError> {
    let mut writer = Writer::new(Vec::new());
    writer
        .write_event(Event::Start(BytesStart::new("xml")))
        .map_err(|_| ApiError::internal("failed to encode WeChat XML"))?;
    for (key, value) in params {
        writer
            .write_event(Event::Start(BytesStart::new(key)))
            .and_then(|_| writer.write_event(Event::Text(BytesText::new(value))))
            .and_then(|_| writer.write_event(Event::End(BytesEnd::new(key))))
            .map_err(|_| ApiError::internal("failed to encode WeChat XML"))?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("xml")))
        .map_err(|_| ApiError::internal("failed to encode WeChat XML"))?;
    String::from_utf8(writer.into_inner())
        .map_err(|_| ApiError::internal("failed to encode WeChat XML"))
}

pub(super) fn alipay_sign(private_key: &str, data: &str) -> Result<String, ApiError> {
    let private_key = normalize_private_key(private_key);
    let key = PKey::private_key_from_pem(private_key.as_bytes())
        .map_err(|_| ApiError::legacy("支付宝私钥错误"))?;
    let mut signer = Signer::new(MessageDigest::sha256(), &key)
        .map_err(|_| ApiError::legacy("支付宝签名失败"))?;
    signer
        .update(data.as_bytes())
        .map_err(|_| ApiError::legacy("支付宝签名失败"))?;
    let signature = signer
        .sign_to_vec()
        .map_err(|_| ApiError::legacy("支付宝签名失败"))?;
    Ok(STANDARD.encode(signature))
}

fn alipay_verify(public_key: &str, data: &str, signature: &str) -> Result<bool, ApiError> {
    let public_key = normalize_public_key(public_key);
    let key = PKey::public_key_from_pem(public_key.as_bytes())
        .map_err(|_| ApiError::legacy("支付宝公钥错误"))?;
    let signature = STANDARD
        .decode(signature.replace(' ', "+"))
        .ok()
        .ok_or_else(|| ApiError::legacy("verify error"))?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &key)
        .map_err(|_| ApiError::legacy("verify error"))?;
    verifier
        .update(data.as_bytes())
        .map_err(|_| ApiError::legacy("verify error"))?;
    verifier
        .verify(&signature)
        .map_err(|_| ApiError::legacy("verify error"))
}

fn normalize_private_key(value: &str) -> String {
    normalize_pem(value, "RSA PRIVATE KEY")
}

fn normalize_public_key(value: &str) -> String {
    normalize_pem(value, "PUBLIC KEY")
}

fn normalize_pem(value: &str, label: &str) -> String {
    let value = value.trim();
    if value.contains("BEGIN ") {
        return value.to_string();
    }
    let compact = value.split_whitespace().collect::<String>();
    let lines = compact
        .as_bytes()
        .chunks(64)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    format!("-----BEGIN {label}-----\n{lines}\n-----END {label}-----")
}

pub(super) fn canonical_query(params: &BTreeMap<String, String>) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

pub(super) fn form_query(params: &BTreeMap<String, String>) -> Result<String, ApiError> {
    serde_urlencoded::to_string(params)
        .map_err(|_| ApiError::internal("failed to encode payment query"))
}

pub(super) fn payment_http_client(user_agent: &'static str) -> PaymentHttpClient {
    PaymentHttpClient { user_agent }
}

fn payment_gateway_message(value: &serde_json::Value, default: &str) -> String {
    if let Some(message) = value.get("message").and_then(|value| value.as_str()) {
        return message.to_string();
    }
    if let Some(errors) = value.get("errors").and_then(|value| value.as_object()) {
        for value in errors.values() {
            if let Some(message) = value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|value| value.as_str())
            {
                return message.to_string();
            }
        }
    }
    default.to_string()
}
