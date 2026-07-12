use std::collections::HashMap;

use axum::{body::to_bytes, extract::Request, http::header};
use v2board_compat::ApiError;

pub(crate) async fn payment_request_input(
    request: Request,
    method: &str,
) -> Result<v2board_domain::order::PaymentNotifyInput, ApiError> {
    let mut params = HashMap::new();
    if let Some(query) = request.uri().query().filter(|query| !query.is_empty()) {
        params.extend(parse_urlencoded_params(query)?);
    }
    let headers = request
        .headers()
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (key.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    if body.is_empty() {
        return Ok(v2board_domain::order::PaymentNotifyInput {
            params,
            body: Vec::new(),
            headers,
        });
    }
    if content_type.contains("application/json") || body.first() == Some(&b'{') {
        // These providers authenticate the exact raw bytes. Do not even decode
        // JSON at the HTTP adapter boundary: their domain verifier performs
        // HMAC verification first and parses only authenticated input.
        if !raw_json_payment_notify(method) {
            params.extend(parse_json_object_params(&body)?);
        }
    } else if content_type.contains("xml") || body.first() == Some(&b'<') {
        params.extend(parse_xml_params(&body)?);
    } else {
        let body = std::str::from_utf8(&body)
            .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
        params.extend(parse_urlencoded_params(body)?);
    }
    Ok(v2board_domain::order::PaymentNotifyInput {
        params,
        body: body.to_vec(),
        headers,
    })
}

fn raw_json_payment_notify(method: &str) -> bool {
    matches!(
        method,
        "Coinbase"
            | "BTCPay"
            | "StripeCredit"
            | "StripeAlipay"
            | "StripeWepay"
            | "StripeCheckout"
            | "StripeALL"
    )
}

pub(crate) async fn admin_request_params(
    request: Request,
) -> Result<HashMap<String, String>, ApiError> {
    let mut params = HashMap::new();
    if let Some(query) = request.uri().query().filter(|query| !query.is_empty()) {
        params.extend(parse_urlencoded_params(query)?);
    }
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid admin request body"))?;
    if body.is_empty() {
        return Ok(params);
    }
    if content_type.contains("application/json") || body.first() == Some(&b'{') {
        let value = serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|_| ApiError::bad_request("Invalid admin request body"))?;
        flatten_admin_json(None, &value, &mut params);
    } else {
        let body = std::str::from_utf8(&body)
            .map_err(|_| ApiError::bad_request("Invalid admin request body"))?;
        params.extend(parse_urlencoded_params(body)?);
    }
    Ok(params)
}

pub(crate) fn flatten_admin_json(
    prefix: Option<String>,
    value: &serde_json::Value,
    params: &mut HashMap<String, String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let key = prefix
                    .as_ref()
                    .map(|prefix| format!("{prefix}[{key}]"))
                    .unwrap_or_else(|| key.clone());
                flatten_admin_json(Some(key), value, params);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                let key = prefix
                    .as_ref()
                    .map(|prefix| format!("{prefix}[{index}]"))
                    .unwrap_or_else(|| index.to_string());
                flatten_admin_json(Some(key), value, params);
            }
        }
        serde_json::Value::Null => {
            if let Some(prefix) = prefix {
                params.insert(prefix, "null".to_string());
            }
        }
        serde_json::Value::String(value) => {
            if let Some(prefix) = prefix {
                params.insert(prefix, value.clone());
            }
        }
        serde_json::Value::Number(value) => {
            if let Some(prefix) = prefix {
                params.insert(prefix, value.to_string());
            }
        }
        serde_json::Value::Bool(value) => {
            if let Some(prefix) = prefix {
                params.insert(prefix, if *value { "1" } else { "0" }.to_string());
            }
        }
    }
}

pub(crate) fn parse_urlencoded_params(value: &str) -> Result<HashMap<String, String>, ApiError> {
    serde_urlencoded::from_str::<HashMap<String, String>>(value)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))
}

fn parse_json_object_params(bytes: &[u8]) -> Result<HashMap<String, String>, ApiError> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    let Some(object) = value.as_object() else {
        return Err(ApiError::bad_request("Invalid payment notify body"));
    };
    Ok(object
        .iter()
        .filter_map(|(key, value)| json_scalar_to_string(value).map(|value| (key.clone(), value)))
        .collect())
}

fn parse_xml_params(bytes: &[u8]) -> Result<HashMap<String, String>, ApiError> {
    let body = std::str::from_utf8(bytes)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    let mut params = HashMap::new();
    let mut cursor = body;
    while let Some(start) = cursor.find('<') {
        let cursor_after_open = &cursor[start + 1..];
        let Some(close) = cursor_after_open.find('>') else {
            break;
        };
        let tag = &cursor_after_open[..close];
        if tag.starts_with('/') || tag == "xml" || tag.contains(' ') {
            cursor = &cursor_after_open[close + 1..];
            continue;
        }
        let value_start = start + 1 + close + 1;
        let close_tag = format!("</{tag}>");
        let Some(value_end_rel) = cursor[value_start..].find(&close_tag) else {
            cursor = &cursor_after_open[close + 1..];
            continue;
        };
        let raw_value = &cursor[value_start..value_start + value_end_rel];
        let value = raw_value
            .strip_prefix("<![CDATA[")
            .and_then(|value| value.strip_suffix("]]>"))
            .unwrap_or(raw_value)
            .to_string();
        params.insert(tag.to_string(), value);
        cursor = &cursor[value_start + value_end_rel + close_tag.len()..];
    }
    Ok(params)
}

fn json_scalar_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::raw_json_payment_notify;

    #[test]
    fn signed_json_payment_bodies_are_not_preparsed() {
        for method in [
            "Coinbase",
            "BTCPay",
            "StripeCredit",
            "StripeAlipay",
            "StripeWepay",
            "StripeCheckout",
            "StripeALL",
        ] {
            assert!(raw_json_payment_notify(method), "{method}");
        }
        assert!(!raw_json_payment_notify("CoinPayments"));
        assert!(!raw_json_payment_notify("EPay"));
    }
}
