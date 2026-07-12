use std::collections::HashMap;

use axum::{
    body::to_bytes,
    extract::Request,
    http::{HeaderMap, header},
};
use v2board_compat::ApiError;
use v2board_config::AppConfig;
use v2board_db::DbPool;

use crate::request_params::{flatten_admin_json, parse_urlencoded_params};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServerIdentity {
    pub(super) node_type: String,
    pub(super) node_id: i32,
}

pub(super) fn server_identity(
    class: &str,
    params: &HashMap<String, String>,
) -> Result<ServerIdentity, ApiError> {
    let node_type = match class {
        "uniproxy" => params
            .get("node_type")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_server_node_type)
            .ok_or_else(|| ApiError::legacy("server is not exist"))?,
        "shadowsockstidalab" => "shadowsocks".to_string(),
        "trojantidalab" => "trojan".to_string(),
        "deepbwork" => "vmess".to_string(),
        "v2node" => "v2node".to_string(),
        _ => return Err(ApiError::not_found("Server route not found")),
    };
    Ok(ServerIdentity {
        node_type,
        node_id: required_i32_param(params, "node_id")?,
    })
}

pub(super) async fn validate_server_token(
    db: &DbPool,
    config: &AppConfig,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
    identity: &ServerIdentity,
) -> Result<(), ApiError> {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let explicit_header = headers
        .get("x-v2board-server-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let parameter = params
        .get("token")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut candidates = [authorization, explicit_header, parameter]
        .into_iter()
        .flatten();
    let token = candidates
        .next()
        .ok_or_else(|| ApiError::legacy("token is null"))?;
    if candidates.any(|candidate| candidate != token) {
        return Err(ApiError::legacy("token is error"));
    }

    let master_key = config
        .server_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("token is error"))?;
    let credential_epoch = sqlx::query_scalar::<_, i64>(
        "SELECT credential_epoch FROM v2_server_credential \
         WHERE node_type = ? AND node_id = ? LIMIT 1",
    )
    .bind(&identity.node_type)
    .bind(identity.node_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| ApiError::legacy("token is error"))?;
    if v2board_domain::server_credentials::verify_node_token(
        master_key,
        &identity.node_type,
        identity.node_id,
        credential_epoch,
        token,
    ) {
        return Ok(());
    }
    if config.server_legacy_token_enable
        && v2board_compat::constant_time_secret_eq(master_key, token)
    {
        tracing::warn!(
            node_type = identity.node_type,
            node_id = identity.node_id,
            "accepted deprecated global server token; rotate this node to its scoped credential"
        );
        return Ok(());
    }
    Err(ApiError::legacy("token is error"))
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
