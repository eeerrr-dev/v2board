use std::collections::HashMap;

use axum::{body::to_bytes, extract::Request, http::header};
use v2board_compat::ApiError;
use v2board_config::AppConfig;

use crate::request_params::{flatten_admin_json, parse_urlencoded_params};

#[derive(Debug)]
pub(super) struct ServerRequestInput {
    pub(super) params: HashMap<String, String>,
    pub(super) body: Option<serde_json::Value>,
}

pub(super) async fn server_request_input(request: Request) -> Result<ServerRequestInput, ApiError> {
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
    let body = to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid server request body"))?;
    if body.is_empty() {
        return Ok(ServerRequestInput { params, body: None });
    }
    if content_type.contains("application/json")
        || body.first() == Some(&b'{')
        || body.first() == Some(&b'[')
    {
        let value = serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|_| ApiError::bad_request("Invalid server request body"))?;
        flatten_admin_json(None, &value, &mut params);
        return Ok(ServerRequestInput {
            params,
            body: Some(value),
        });
    }
    let body = std::str::from_utf8(&body)
        .map_err(|_| ApiError::bad_request("Invalid server request body"))?;
    params.extend(parse_urlencoded_params(body)?);
    Ok(ServerRequestInput { params, body: None })
}

pub(super) fn validate_server_token(
    config: &AppConfig,
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    let token = params
        .get("token")
        .map(String::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError::legacy("token is null"))?;
    if config.server_token.as_deref() != Some(token) {
        return Err(ApiError::legacy("token is error"));
    }
    Ok(())
}

pub(super) fn required_i32_param(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<i32, ApiError> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))
}

pub(super) fn normalize_server_node_type(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "v2ray" => "vmess".to_string(),
        "hysteria2" => "hysteria".to_string(),
        value => value.to_string(),
    }
}
