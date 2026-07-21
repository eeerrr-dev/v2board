mod config;
mod request;
mod response;
mod traffic;
mod users;

#[cfg(test)]
mod tests;

use axum::{
    extract::{Path, Request, State},
    http::HeaderMap,
    response::Response,
};
use serde::Serialize;
use v2board_application::server_runtime::{RuntimeServerNode as ServerNodeRow, RuntimeServerUser};
use v2board_compat::ApiError;

use crate::runtime::AppState;

use self::{
    config::{
        server_deepbwork_config, server_trojan_tidalab_config, server_uniproxy_config,
        server_v2_config_value,
    },
    request::{required_i32_param, server_identity, server_request_input, validate_server_token},
    response::{raw_value_response, server_fail_response},
    traffic::{server_alive, server_alive_list, server_push},
    users::{server_tidalab_user, server_uniproxy_user},
};

pub(super) async fn server_v1(
    State(state): State<AppState>,
    Path((class, action)): Path<(String, String)>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let input = server_request_input(request).await?;
    let class = class.to_ascii_lowercase();
    let action = action.to_ascii_lowercase();
    let identity = server_identity(&class, &input.params)?;
    validate_server_token(&state, &headers, &input.params, &identity).await?;

    match (class.as_str(), action.as_str()) {
        ("uniproxy", "user") => server_uniproxy_user(&state, &headers, &input.params).await,
        ("uniproxy", "push") => {
            server_push(
                &state,
                &headers,
                &input.params,
                input.body.as_ref(),
                true,
                None,
            )
            .await
        }
        ("uniproxy", "alivelist") => server_alive_list(&state, &input.params).await,
        ("uniproxy", "alive") => server_alive(&state, &input.params, input.body.as_ref()).await,
        ("uniproxy", "config") => server_uniproxy_config(&state, &headers, &input.params).await,
        ("shadowsockstidalab", "user") => {
            server_tidalab_user(&state, &headers, "shadowsocks", &input.params).await
        }
        ("shadowsockstidalab", "submit") => {
            server_push(
                &state,
                &headers,
                &input.params,
                input.body.as_ref(),
                false,
                Some("shadowsocks"),
            )
            .await
        }
        ("trojantidalab", "user") => {
            server_tidalab_user(&state, &headers, "trojan", &input.params).await
        }
        ("trojantidalab", "submit") => {
            server_push(
                &state,
                &headers,
                &input.params,
                input.body.as_ref(),
                false,
                Some("trojan"),
            )
            .await
        }
        ("trojantidalab", "config") => server_trojan_tidalab_config(&state, &input.params).await,
        ("deepbwork", "user") => {
            server_tidalab_user(&state, &headers, "vmess", &input.params).await
        }
        ("deepbwork", "submit") => {
            server_push(
                &state,
                &headers,
                &input.params,
                input.body.as_ref(),
                false,
                Some("vmess"),
            )
            .await
        }
        ("deepbwork", "config") => server_deepbwork_config(&state, &input.params).await,
        _ => Err(ApiError::not_found("Server route not found")),
    }
}

pub(super) async fn server_v2_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let input = server_request_input(request).await?;
    let identity = match server_identity("v2node", &input.params) {
        Ok(identity) => identity,
        Err(error) => return Ok(server_fail_response(error.to_string())),
    };
    if let Err(error) = validate_server_token(&state, &headers, &input.params, &identity).await {
        return Ok(server_fail_response(error.to_string()));
    }
    let node_id = match required_i32_param(&input.params, "node_id") {
        Ok(node_id) => node_id,
        Err(error) => return Ok(server_fail_response(error.to_string())),
    };
    let Some(node) = state
        .server_runtime_service()
        .node(v2board_domain_model::ServerKind::V2node, node_id)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
    else {
        return Ok(server_fail_response("server is not exist"));
    };
    // COST DECISION (accept, do not cache): every node poll re-materializes and
    // sha1s the config even when the ETag ends up matching. That work is a couple
    // of primary-key point lookups plus a hash of a small payload, driven at the
    // operator-controlled `server_pull_interval` (default 60s), so it stays cheap
    // at node scale. A cache would have to be invalidated on config reload, node
    // edits, and route edits — an invariant surface not worth its risk here. If a
    // deployment ever runs enough nodes at a short interval to make this hot,
    // revisit with a config-generation-keyed cache rather than removing the hash.
    let value = server_v2_config_value(&state, &node).await?;
    // INTENTIONAL DIVERGENCE (kept more correct than the oracle): ServerController.php:105
    // compares `If-None-Match === $eTag` against the UNQUOTED sha1 while emitting a QUOTED
    // `ETag: "<hash>"`, so a conformant client that echoes the quoted ETag never gets a 304
    // (an oracle bug). `raw_value_response` matches on `contains`, so the quoted ETag round-trips
    // and yields the correct 304. Do NOT reintroduce the strict-equality bug.
    raw_value_response(value, &headers, false)
}

#[derive(Debug, Clone, Serialize)]
struct ServerUserRow {
    id: i64,
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_limit: Option<i32>,
}

impl From<RuntimeServerUser> for ServerUserRow {
    fn from(user: RuntimeServerUser) -> Self {
        Self {
            id: user.id,
            uuid: user.uuid,
            speed_limit: user.speed_limit,
            device_limit: user.device_limit,
        }
    }
}

#[derive(Debug, Clone)]
struct TrafficEntry {
    user_id: i64,
    u: i64,
    d: i64,
}

#[derive(Debug)]
struct ParsedTrafficEntries {
    entries: Vec<TrafficEntry>,
    ignored_rows: usize,
    defaulted_counters: usize,
}
