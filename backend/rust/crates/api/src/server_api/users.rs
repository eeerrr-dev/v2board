use std::collections::HashMap;

use axum::{
    Json,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use serde_json::json;
use v2board_compat::ApiError;
use v2board_domain_model::ServerKind;

use crate::runtime::AppState;

use super::{
    ServerUserRow,
    request::{load_server_node, load_uniproxy_node, required_i32_param},
    response::{
        etag_matches, insert_etag, not_modified_response, raw_value_response,
        response_wants_msgpack, sha1_hex,
    },
    traffic::server_cache_timestamp,
};

pub(super) async fn server_uniproxy_user(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    server_cache_timestamp(state, node_type, node.id).await?;
    let users = state
        .server_runtime_service()
        .users(&node.group_ids, chrono::Utc::now().timestamp())
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .into_iter()
        .map(ServerUserRow::from)
        .collect::<Vec<_>>();
    raw_value_response(
        json!({ "users": users }),
        headers,
        response_wants_msgpack(headers),
    )
}

pub(super) async fn server_tidalab_user(
    state: &AppState,
    headers: &HeaderMap,
    node_type: &str,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let kind = ServerKind::try_from(node_type).map_err(|_| ApiError::legacy("fail"))?;
    let Some(node) = load_server_node(state, kind, node_id).await? else {
        return Err(ApiError::legacy("fail"));
    };
    server_cache_timestamp(state, kind, node.id).await?;
    let users = state
        .server_runtime_service()
        .users(&node.group_ids, chrono::Utc::now().timestamp())
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .into_iter()
        .map(ServerUserRow::from)
        .collect::<Vec<_>>();
    let data = match node_type {
        "shadowsocks" => users
            .iter()
            .map(|user| {
                json!({
                    "id": user.id,
                    "port": node.server_port,
                    "cipher": node.cipher,
                    "secret": user.uuid,
                })
            })
            .collect::<Vec<_>>(),
        "trojan" => users
            .iter()
            .map(|user| {
                let mut item = server_user_without_uuid(user);
                item.insert("trojan_user".to_string(), json!({ "password": user.uuid }));
                serde_json::Value::Object(item)
            })
            .collect::<Vec<_>>(),
        "vmess" => users
            .iter()
            .map(|user| {
                let mut item = server_user_without_uuid(user);
                item.insert(
                    "v2ray_user".to_string(),
                    json!({
                        "uuid": user.uuid,
                        "email": format!("{}@v2board.user", user.uuid),
                        "alter_id": 0,
                        "level": 0,
                    }),
                );
                serde_json::Value::Object(item)
            })
            .collect::<Vec<_>>(),
        _ => users
            .iter()
            .map(|user| {
                serde_json::to_value(user).expect("server user rows are always JSON serializable")
            })
            .collect::<Vec<_>>(),
    };
    legacy_tidalab_user_response(node_type, data, headers)
}

/// Response for the legacy tidalab/deepbwork `user` endpoints.
///
/// These controllers compute the ETag over the BARE `$result` data array
/// (`sha1(json_encode($result))` — ShadowsocksTidalab:51 / Trojan:53 / Deepbwork:56), NOT
/// over the `{msg,data}` / `{data}` wrapper they actually return. UniProxy is different (it
/// hashes the whole `{users:...}` response) and stays on `raw_value_response`.
pub(super) fn legacy_tidalab_user_response(
    node_type: &str,
    data: Vec<serde_json::Value>,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let etag =
        sha1_hex(&serde_json::to_vec(&data).map_err(|_| ApiError::internal("json encode failed"))?);
    if etag_matches(headers, &etag) {
        return not_modified_response(&etag);
    }
    // shadowsocks returns {data}; trojan/vmess return {msg:"ok", data}.
    let body = if node_type == "shadowsocks" {
        json!({ "data": data })
    } else {
        json!({ "msg": "ok", "data": data })
    };
    let mut response = Json(body).into_response();
    insert_etag(response.headers_mut(), &etag)?;
    Ok(response)
}

/// Build the trojan/vmess (TrojanTidalab / Deepbwork) per-user object.
///
/// UniProxyController::user `array_filter`s null keys away, but TrojanTidalabController :46-52 and
/// DeepbworkController :46-55 serialize the RAW user model, so `speed_limit` / `device_limit` are
/// ALWAYS present there — emitted as JSON `null` when the column is null (both are uncast on the
/// User model, so null stays null). Match that: keep the keys instead of dropping them when None.
/// (ShadowsocksTidalab is different: it emits only id/port/cipher/secret and never carries these
/// two keys, so it does not use this helper.)
pub(super) fn server_user_without_uuid(
    user: &ServerUserRow,
) -> serde_json::Map<String, serde_json::Value> {
    let mut item = serde_json::Map::new();
    item.insert("id".to_string(), json!(user.id));
    item.insert("speed_limit".to_string(), json!(user.speed_limit));
    item.insert("device_limit".to_string(), json!(user.device_limit));
    item
}
