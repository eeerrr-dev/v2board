mod config;
mod repository;
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
use chrono::Utc;
use redis::AsyncCommands;
use serde::Serialize;
use sqlx::FromRow;
use v2board_compat::ApiError;
use v2board_db::server::AvailableServerRow;

use crate::runtime::AppState;

use self::{
    config::{
        server_deepbwork_config, server_trojan_tidalab_config, server_uniproxy_config,
        server_v2_config_value,
    },
    repository::load_server_node,
    request::{required_i32_param, server_identity, server_request_input, validate_server_token},
    response::{raw_value_response, server_fail_response},
    traffic::{server_alive, server_alive_list, server_push},
    users::{server_tidalab_user, server_uniproxy_user},
};

const REDIS_MGET_BATCH_SIZE: usize = 500;

pub(super) async fn server_v1(
    State(state): State<AppState>,
    Path((class, action)): Path<(String, String)>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let input = server_request_input(request).await?;
    let config = state.config_snapshot();
    let class = class.to_ascii_lowercase();
    let action = action.to_ascii_lowercase();
    let identity = server_identity(&class, &input.params)?;
    validate_server_token(&state.db, &config, &headers, &input.params, &identity).await?;

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
    let config = state.config_snapshot();
    let identity = match server_identity("v2node", &input.params) {
        Ok(identity) => identity,
        Err(error) => return Ok(server_fail_response(error.to_string())),
    };
    if let Err(error) =
        validate_server_token(&state.db, &config, &headers, &input.params, &identity).await
    {
        return Ok(server_fail_response(error.to_string()));
    }
    let node_id = match required_i32_param(&input.params, "node_id") {
        Ok(node_id) => node_id,
        Err(error) => return Ok(server_fail_response(error.to_string())),
    };
    let Some(node) = load_server_node(&state.db, "v2node", node_id).await? else {
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

/// Hydrate `is_online` / `last_check_at` on the available-server rows from the node health
/// cache keys the node API writes (`SERVER_<TYPE>_LAST_CHECK_AT_<id>`, uppercase type).
///
/// `fetch_available_servers` (v2board_db) cannot reach Redis, so it emits every row with
/// `last_check_at = None` / `is_online = 0`. This mirrors ServerService::getAvailableServers
/// (ServerService.php:245-246): a node is online when its last check-in is newer than 300s ago,
/// and a child (relay) node inherits its parent's check-in (the per-protocol `getAvailable*`
/// helpers key `last_check_at` on `parent_id ?? id`). The `cache_key` embeds `is_online`
/// (ServerService.php:246), so its trailing segment is rewritten in place.
///
/// Call this from `main.rs` immediately after `fetch_available_servers`, on a `mut` binding.
pub(super) async fn hydrate_online_status(
    state: &AppState,
    servers: &mut [AvailableServerRow],
) -> Result<(), ApiError> {
    if servers.is_empty() {
        return Ok(());
    }
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let now = Utc::now().timestamp();
    for servers in servers.chunks_mut(REDIS_MGET_BATCH_SIZE) {
        let keys = servers
            .iter()
            .map(|server| {
                let check_id = server.parent_id.unwrap_or(server.id);
                state.redis_key(&format!(
                    "SERVER_{}_LAST_CHECK_AT_{check_id}",
                    server.r#type.to_ascii_uppercase()
                ))
            })
            .collect::<Vec<_>>();
        let last_check_values = conn.mget::<_, Vec<Option<i64>>>(&keys).await?;
        for (server, last_check_at) in servers.iter_mut().zip(last_check_values) {
            server.last_check_at = last_check_at;
            // ServerService.php:245 — is_online = (time() - 300 > last_check_at) ? 0 : 1; a missing
            // key (null) compares as 0 in PHP, i.e. offline.
            let is_online = server_online_status(now, last_check_at);
            server.is_online = is_online;
            // cache_key is `{type}-{id}-{updated_at}-{is_online}` — rewrite only the last segment.
            // Take an owned prefix first so the borrow ends before the reassignment.
            if let Some(prefix) = server
                .cache_key
                .rsplit_once('-')
                .map(|(prefix, _)| prefix.to_owned())
            {
                server.cache_key = format!("{prefix}-{is_online}");
            }
        }
    }
    Ok(())
}

fn server_online_status(now: i64, last_check_at: Option<i64>) -> i16 {
    // Widen before subtracting so the exact legacy boundary (`now - 300 > last_check_at`) is
    // preserved without collapsing an i64 underflow onto `i64::MIN`.
    if i128::from(now) - 300 > i128::from(last_check_at.unwrap_or(0)) {
        0
    } else {
        1
    }
}

#[derive(Debug, Clone, FromRow)]
struct ServerNodeRow {
    id: i32,
    group_id: String,
    route_id: Option<String>,
    rate: String,
    host: String,
    server_port: i32,
    created_at: i64,
    listen_ip: Option<String>,
    protocol: Option<String>,
    version: Option<i32>,
    tls: Option<i16>,
    tls_settings: Option<String>,
    flow: Option<String>,
    network: Option<String>,
    network_settings: Option<String>,
    encryption: Option<String>,
    encryption_settings: Option<String>,
    zero_rtt_handshake: Option<i16>,
    congestion_control: Option<String>,
    cipher: Option<String>,
    obfs: Option<String>,
    obfs_settings: Option<String>,
    obfs_password: Option<String>,
    padding_scheme: Option<String>,
    server_name: Option<String>,
    up_mbps: Option<i32>,
    down_mbps: Option<i32>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
struct ServerUserRow {
    id: i64,
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_limit: Option<i32>,
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

#[derive(Debug, Clone, FromRow)]
struct ServerRouteRow {
    id: i32,
    match_text: String,
    action: String,
    action_value: Option<String>,
}
