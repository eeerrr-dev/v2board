use std::collections::HashMap;

use axum::{body::to_bytes, extract::Request, http::header};
use quick_xml::{Reader, escape::resolve_predefined_entity, events::Event};
use v2board_compat::ApiError;

const MAX_PARAMETER_COUNT: usize = 4096;
const MAX_PARAMETER_KEY_BYTES: usize = 512;
const MAX_PARAMETER_VALUE_BYTES: usize = 256 * 1024;
const MAX_FORWARDED_HEADER_COUNT: usize = 128;
const MAX_FORWARDED_HEADER_VALUE_BYTES: usize = 16 * 1024;

pub(crate) async fn payment_request_input(
    request: Request,
    method: &str,
) -> Result<v2board_domain::order::PaymentNotifyInput, ApiError> {
    let mut params = HashMap::new();
    if let Some(query) = request.uri().query().filter(|query| !query.is_empty()) {
        params.extend(parse_urlencoded_params(query)?);
    }
    let headers = bounded_payment_headers(request.headers())?;
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
        validate_params(&params, "Invalid payment notify body")?;
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
    validate_params(&params, "Invalid payment notify body")?;
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
        validate_params(&params, "Invalid admin request body")?;
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
    validate_params(&params, "Invalid admin request body")?;
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
    let params = serde_urlencoded::from_str::<HashMap<String, String>>(value)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    validate_params(&params, "Invalid payment notify body")?;
    Ok(params)
}

fn validate_params(
    params: &HashMap<String, String>,
    message: &'static str,
) -> Result<(), ApiError> {
    if params.len() > MAX_PARAMETER_COUNT
        || params.iter().any(|(key, value)| {
            key.len() > MAX_PARAMETER_KEY_BYTES || value.len() > MAX_PARAMETER_VALUE_BYTES
        })
    {
        return Err(ApiError::bad_request(message));
    }
    Ok(())
}

fn bounded_payment_headers(
    headers: &axum::http::HeaderMap,
) -> Result<HashMap<String, String>, ApiError> {
    if headers.len() > MAX_FORWARDED_HEADER_COUNT {
        return Err(ApiError::bad_request("Invalid payment notify headers"));
    }
    headers
        .iter()
        .map(|(key, value)| {
            let value = value
                .to_str()
                .map_err(|_| ApiError::bad_request("Invalid payment notify headers"))?;
            if value.len() > MAX_FORWARDED_HEADER_VALUE_BYTES {
                return Err(ApiError::bad_request("Invalid payment notify headers"));
            }
            Ok((key.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect()
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

/// Decodes a flat payment-notify XML document (WeChat Pay v1 is the only
/// wired producer) into a top-level tag → text map, mirroring the legacy
/// Omnipay `xml2array` anchor (SimpleXML): the root `<xml>` envelope is
/// skipped, CDATA unwraps to its literal content, attributes are ignored with
/// the text content kept, and XML entities decode. Signed providers verify
/// the decoded map, so any mis-parse fails closed at signature verification.
fn parse_xml_params(bytes: &[u8]) -> Result<HashMap<String, String>, ApiError> {
    fn invalid() -> ApiError {
        ApiError::bad_request("Invalid payment notify body")
    }
    let body = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let mut reader = Reader::from_str(body);
    let mut params = HashMap::new();
    // The parameter element currently being captured, with its accumulated
    // direct text. Like the SimpleXML string cast, only a parameter's direct
    // text/CDATA content is kept.
    let mut current: Option<(String, String)> = None;
    // Depth of markup nested inside the current parameter element; nested
    // contents are skipped rather than serialized back into the value.
    let mut nested = 0usize;
    loop {
        match reader.read_event().map_err(|_| invalid())? {
            Event::Start(start) => {
                if current.is_some() {
                    nested += 1;
                } else {
                    let name = std::str::from_utf8(start.name().as_ref())
                        .map_err(|_| invalid())?
                        .to_string();
                    // `<xml>` is the WeChat Pay v1 envelope, never a parameter.
                    if name != "xml" {
                        current = Some((name, String::new()));
                    }
                }
            }
            Event::End(_) => {
                if nested > 0 {
                    nested -= 1;
                } else if let Some((name, value)) = current.take() {
                    params.insert(name, value);
                }
            }
            Event::Text(text) => {
                if nested == 0
                    && let Some((_, value)) = &mut current
                {
                    value.push_str(&text.decode().map_err(|_| invalid())?);
                }
            }
            Event::CData(cdata) => {
                if nested == 0
                    && let Some((_, value)) = &mut current
                {
                    value.push_str(&cdata.decode().map_err(|_| invalid())?);
                }
            }
            Event::GeneralRef(reference) => {
                // Deliberate improvement over the retired hand-rolled scanner,
                // toward the Omnipay xml2array anchor: predefined XML entities
                // and character references decode; unknown entities fail
                // closed.
                if nested == 0
                    && let Some((_, value)) = &mut current
                {
                    if let Some(resolved) = reference.resolve_char_ref().map_err(|_| invalid())? {
                        value.push(resolved);
                    } else {
                        let name = reference.decode().map_err(|_| invalid())?;
                        let resolved = resolve_predefined_entity(&name).ok_or_else(invalid)?;
                        value.push_str(resolved);
                    }
                }
            }
            // The retired scanner never captured self-closing tags and WeChat
            // Pay v1 does not emit them; keep dropping them.
            Event::Empty(_) => {}
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_) => {}
            Event::Eof => break,
        }
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
    use std::collections::HashMap;

    use super::{MAX_PARAMETER_COUNT, parse_xml_params, raw_json_payment_notify, validate_params};

    #[test]
    fn flat_wechat_notify_body_parses_to_tag_text_map() {
        let body = concat!(
            "<xml>\n",
            "  <appid><![CDATA[wx2421b1c4370ec43b]]></appid>\n",
            "  <bank_type><![CDATA[CFT]]></bank_type>\n",
            "  <fee_type><![CDATA[CNY]]></fee_type>\n",
            "  <mch_id><![CDATA[10000100]]></mch_id>\n",
            "  <nonce_str><![CDATA[5d2b6c2a8db53831f7eda20af46e531c]]></nonce_str>\n",
            "  <out_trade_no><![CDATA[1409811653]]></out_trade_no>\n",
            "  <result_code><![CDATA[SUCCESS]]></result_code>\n",
            "  <return_code><![CDATA[SUCCESS]]></return_code>\n",
            "  <sign><![CDATA[B552ED6B279343CB493C5DD0D78AB241]]></sign>\n",
            "  <time_end><![CDATA[20140903131540]]></time_end>\n",
            "  <total_fee>1</total_fee>\n",
            "  <transaction_id><![CDATA[1004400740201409030005092168]]></transaction_id>\n",
            "</xml>",
        );
        let params = parse_xml_params(body.as_bytes()).expect("flat WeChat notify parses");
        assert_eq!(params.len(), 12);
        assert_eq!(params["out_trade_no"], "1409811653");
        assert_eq!(params["transaction_id"], "1004400740201409030005092168");
        assert_eq!(params["total_fee"], "1");
        assert_eq!(params["return_code"], "SUCCESS");
        assert_eq!(params["result_code"], "SUCCESS");
        assert_eq!(params["sign"], "B552ED6B279343CB493C5DD0D78AB241");
        // The root <xml> envelope is a wrapper, never a parameter.
        assert!(!params.contains_key("xml"));
    }

    #[test]
    fn cdata_values_unwrap_to_their_literal_content() {
        let params = parse_xml_params(
            b"<xml><attach><![CDATA[a<b & c &amp; d]]></attach><blank><![CDATA[]]></blank></xml>",
        )
        .expect("CDATA values parse");
        // CDATA content is literal: markup-significant bytes and entity
        // spellings inside the wrapper survive byte-for-byte.
        assert_eq!(params["attach"], "a<b & c &amp; d");
        assert_eq!(params["blank"], "");
    }

    #[test]
    fn entity_bearing_text_decodes() {
        // Deliberate change from the retired hand-rolled scanner (which kept
        // `&amp;` spellings raw), toward the legacy Omnipay xml2array anchor
        // (SimpleXML): predefined entities and character references decode.
        let params =
            parse_xml_params(b"<xml><return_msg>a &amp; b &lt;ok&gt; &#65;</return_msg></xml>")
                .expect("entity-bearing text parses");
        assert_eq!(params["return_msg"], "a & b <ok> A");
    }

    #[test]
    fn attribute_bearing_tags_parse_with_text_content_kept() {
        // Deliberate change from the retired hand-rolled scanner (which
        // dropped any attribute-bearing tag), toward the legacy Omnipay
        // xml2array anchor (SimpleXML): attributes are ignored and the text
        // content is kept.
        let params =
            parse_xml_params(b"<xml><total_fee type=\"int\">100</total_fee><sign>abc</sign></xml>")
                .expect("attribute-bearing body parses");
        assert_eq!(params["total_fee"], "100");
        assert_eq!(params["sign"], "abc");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn malformed_xml_fails_closed() {
        // The retired scanner silently produced a partial map for malformed
        // XML; the structured parser rejects the body outright. Signed
        // providers failed closed either way (a missing `sign` never
        // verifies).
        let error = parse_xml_params(b"<xml><out_trade_no>42</mismatch></xml>")
            .expect_err("mismatched close tag is rejected");
        assert_eq!(error.to_string(), "Invalid payment notify body");
    }

    #[test]
    fn self_closing_tags_are_dropped() {
        let params = parse_xml_params(b"<xml><pad/><out_trade_no>42</out_trade_no></xml>")
            .expect("self-closing body parses");
        assert!(!params.contains_key("pad"));
        assert_eq!(params["out_trade_no"], "42");
    }

    #[test]
    fn non_utf8_xml_body_is_rejected() {
        let error = parse_xml_params(b"<xml><a>\xff\xfe</a></xml>")
            .expect_err("non-UTF-8 body is rejected");
        assert_eq!(error.to_string(), "Invalid payment notify body");
    }

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

    #[test]
    fn decoded_parameter_maps_have_count_and_field_limits() {
        let mut params = HashMap::new();
        params.insert("key".to_string(), "value".to_string());
        assert!(validate_params(&params, "invalid").is_ok());

        let oversized = (0..=MAX_PARAMETER_COUNT)
            .map(|index| (index.to_string(), String::new()))
            .collect::<HashMap<_, _>>();
        assert!(validate_params(&oversized, "invalid").is_err());

        params.insert("x".repeat(513), String::new());
        assert!(validate_params(&params, "invalid").is_err());
    }
}
