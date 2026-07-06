use std::collections::HashMap;

use axum::{
    Json,
    body::to_bytes,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Datelike, Local, TimeZone, Utc};
use redis::AsyncCommands;
use serde::Serialize;
use serde_json::json;
use sha1::{Digest, Sha1};
use sqlx::{AssertSqlSafe, FromRow, MySql, QueryBuilder};
use v2board_compat::ApiError;
use v2board_config::AppConfig;
use v2board_db::DbPool;
use v2board_db::server::AvailableServerRow;

use super::codec::{prefix_bytes, standard_base64_encode};
use super::json_value::value_to_i64;
use super::{AppState, flatten_admin_json, parse_urlencoded_params};

pub(super) async fn server_v1(
    State(state): State<AppState>,
    Path((class, action)): Path<(String, String)>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let input = server_request_input(request).await?;
    let config = state.config_snapshot();
    validate_server_token(&config, &input.params)?;
    let class = class.to_ascii_lowercase();
    let action = action.to_ascii_lowercase();

    match (class.as_str(), action.as_str()) {
        ("uniproxy", "user") => server_uniproxy_user(&state, &headers, &input.params).await,
        ("uniproxy", "push") => {
            server_push(&state, &input.params, input.body.as_ref(), true, None).await
        }
        ("uniproxy", "alivelist") => server_alive_list(&state).await,
        ("uniproxy", "alive") => server_alive(&state, &input.params, input.body.as_ref()).await,
        ("uniproxy", "config") => server_uniproxy_config(&state, &headers, &input.params).await,
        ("shadowsockstidalab", "user") => {
            server_tidalab_user(&state, &headers, "shadowsocks", &input.params).await
        }
        ("shadowsockstidalab", "submit") => {
            server_push(
                &state,
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
    if let Err(error) = validate_server_token(&config, &input.params) {
        return Ok(server_fail_response(error.to_string()));
    }
    let node_id = match required_i32_param(&input.params, "node_id") {
        Ok(node_id) => node_id,
        Err(error) => return Ok(server_fail_response(error.to_string())),
    };
    let Some(node) = load_server_node(&state.db, "v2node", node_id).await? else {
        return Ok(server_fail_response("server is not exist"));
    };
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
    redis: &redis::Client,
    servers: &mut [AvailableServerRow],
) -> Result<(), ApiError> {
    if servers.is_empty() {
        return Ok(());
    }
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let now = Utc::now().timestamp();
    for server in servers.iter_mut() {
        let check_id = server.parent_id.unwrap_or(server.id);
        let key = format!(
            "SERVER_{}_LAST_CHECK_AT_{check_id}",
            server.r#type.to_ascii_uppercase()
        );
        let last_check_at = conn.get::<_, Option<i64>>(&key).await?;
        server.last_check_at = last_check_at;
        // ServerService.php:245 — is_online = (time() - 300 > last_check_at) ? 0 : 1; a missing
        // key (null) compares as 0 in PHP, i.e. offline.
        let is_online: i8 = if now - 300 > last_check_at.unwrap_or(0) {
            0
        } else {
            1
        };
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
    Ok(())
}

#[derive(Debug)]
struct ServerRequestInput {
    params: HashMap<String, String>,
    body: Option<serde_json::Value>,
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
    tls: Option<i8>,
    tls_settings: Option<String>,
    flow: Option<String>,
    network: Option<String>,
    network_settings: Option<String>,
    encryption: Option<String>,
    encryption_settings: Option<String>,
    zero_rtt_handshake: Option<i8>,
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

#[derive(Debug, Clone, FromRow)]
struct ServerRouteRow {
    id: i32,
    match_text: String,
    action: String,
    action_value: Option<String>,
}

async fn server_request_input(request: Request) -> Result<ServerRequestInput, ApiError> {
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

fn validate_server_token(
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

async fn server_uniproxy_user(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    server_cache_timestamp(&state.redis, "LAST_CHECK_AT", &node_type, node.id).await?;
    let users =
        server_available_users(&state.db, parse_i32_json_list(Some(&node.group_id))).await?;
    raw_value_response(
        json!({ "users": users }),
        headers,
        response_wants_msgpack(headers),
    )
}

async fn server_tidalab_user(
    state: &AppState,
    headers: &HeaderMap,
    node_type: &str,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let Some(node) = load_server_node(&state.db, node_type, node_id).await? else {
        return Err(ApiError::legacy("fail"));
    };
    server_cache_timestamp(&state.redis, "LAST_CHECK_AT", node_type, node.id).await?;
    let users =
        server_available_users(&state.db, parse_i32_json_list(Some(&node.group_id))).await?;
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
            .map(|user| serde_json::to_value(user).unwrap_or(serde_json::Value::Null))
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
fn legacy_tidalab_user_response(
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

async fn server_push(
    state: &AppState,
    params: &HashMap<String, String>,
    body: Option<&serde_json::Value>,
    uniproxy: bool,
    fallback_node_type: Option<&str>,
) -> Result<Response, ApiError> {
    let (node_type, node) = if uniproxy {
        load_uniproxy_node(state, params).await?
    } else {
        let node_type = match params
            .get("node_type")
            .map(String::as_str)
            .map(normalize_server_node_type)
        {
            Some(node_type) => node_type,
            None => fallback_node_type.unwrap_or("shadowsocks").to_string(),
        };
        let node_id = required_i32_param(params, "node_id")?;
        let Some(node) = load_server_node(&state.db, &node_type, node_id).await? else {
            return Ok(Json(json!({ "ret": 0, "msg": "server is not found" })).into_response());
        };
        (node_type, node)
    };

    let entries = parse_traffic_entries(body, params);
    server_cache_count(
        &state.redis,
        "ONLINE_USER",
        &node_type,
        node.id,
        entries.len() as i64,
    )
    .await?;
    server_cache_timestamp(&state.redis, "LAST_PUSH_AT", &node_type, node.id).await?;
    if !entries.is_empty() {
        // UserService::trafficFetch (UserService.php:224-229) only *dispatches*
        // TrafficFetchJob/StatUserJob/StatServerJob onto async queues; the HTTP push returns its
        // success payload immediately and never surfaces a persistence failure to the node. Any
        // abort(500) inside those jobs (StatUserJob:97 / StatServerJob:84) happens later in a queue
        // worker and cannot reach the response. Mirror that decoupling: a transient Redis/DB error
        // must NOT become a 5xx here, or the node would retry the push and re-run the Redis HINCRBY
        // (persist_traffic_fetch), double-counting traffic. Log and swallow to keep the wire
        // contract (200 + success body) identical to Laravel.
        if let Err(error) = persist_traffic_fetch(state, &node, &node_type, &entries).await {
            tracing::warn!(node_id = node.id, %error, "server push traffic persistence failed");
        }
    }

    if uniproxy {
        Ok(Json(json!({ "data": true })).into_response())
    } else {
        Ok(Json(json!({ "ret": 1, "msg": "ok" })).into_response())
    }
}

async fn server_alive_list(state: &AppState) -> Result<Response, ApiError> {
    let user_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM v2_user
        WHERE u + d < transfer_enable
          AND (expired_at >= ? OR expired_at IS NULL)
          AND banned = 0
          AND device_limit > 0
        "#,
    )
    .bind(Utc::now().timestamp())
    .fetch_all(&state.db)
    .await?;

    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let mut alive = serde_json::Map::new();
    for user_id in user_ids {
        let key = format!("ALIVE_IP_USER_{user_id}");
        if let Some(value) = conn.get::<_, Option<String>>(&key).await?
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&value)
            && let Some(alive_ip) = value.get("alive_ip").and_then(value_to_i64)
        {
            alive.insert(user_id.to_string(), json!(alive_ip));
        }
    }
    Ok(Json(json!({ "alive": alive })).into_response())
}

async fn server_alive(
    state: &AppState,
    params: &HashMap<String, String>,
    body: Option<&serde_json::Value>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    let Some(object) = body.and_then(serde_json::Value::as_object) else {
        return Ok(Json(json!({ "data": true })).into_response());
    };

    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let now = Utc::now().timestamp();
    // UniProxyController::alive :174 reads device_limit_mode to decide how alive IPs are counted.
    let device_limit_mode = state.config_snapshot().device_limit_mode;
    for (uid, ips) in object {
        let Some(user_id) = uid.parse::<i64>().ok() else {
            continue;
        };
        let Some(ips) = ips.as_array() else {
            continue;
        };
        let key = format!("ALIVE_IP_USER_{user_id}");
        let mut value = conn
            .get::<_, Option<String>>(&key)
            .await?
            .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        value.insert(
            format!("{node_type}{}", node.id),
            json!({
                "aliveips": ips,
                "lastupdateAt": now,
            }),
        );
        let stale_keys = value
            .iter()
            .filter_map(|(key, value)| {
                if key == "alive_ip" {
                    return None;
                }
                let last = value
                    .get("lastupdateAt")
                    .and_then(value_to_i64)
                    .unwrap_or(0);
                (now - last > 100).then_some(key.clone())
            })
            .collect::<Vec<_>>();
        for key in stale_keys {
            value.remove(&key);
        }
        let alive_ip = count_alive_ips(&value, device_limit_mode);
        value.insert("alive_ip".to_string(), json!(alive_ip));
        let _: () = conn
            .set_ex(key, serde_json::Value::Object(value).to_string(), 120)
            .await?;
    }
    Ok(Json(json!({ "data": true })).into_response())
}

/// Count alive IPs for a user across their per-node `aliveips` buckets.
///
/// Mirrors UniProxyController::alive (:172-192): with `device_limit_mode == 1` the count is
/// the number of UNIQUE client IPs (deduped by `explode("_", ip_NodeId)[0]`, the substring
/// before the first `_`); otherwise it is the raw sum of connection entries across nodes.
/// The `alive_ip` bookkeeping key is never itself a node bucket, so it is skipped.
fn count_alive_ips(
    nodes: &serde_json::Map<String, serde_json::Value>,
    device_limit_mode: i32,
) -> usize {
    if device_limit_mode == 1 {
        let mut unique = std::collections::HashSet::new();
        for (key, node) in nodes {
            if key == "alive_ip" {
                continue;
            }
            let Some(ips) = node.get("aliveips").and_then(serde_json::Value::as_array) else {
                continue;
            };
            for ip_node in ips {
                if let Some(text) = ip_node.as_str() {
                    // explode("_", ip_NodeId)[0]: substring before the first '_'.
                    unique.insert(text.split('_').next().unwrap_or(text).to_string());
                }
            }
        }
        unique.len()
    } else {
        nodes
            .iter()
            .filter(|(key, _)| key.as_str() != "alive_ip")
            .filter_map(|(_, node)| node.get("aliveips").and_then(serde_json::Value::as_array))
            .map(Vec::len)
            .sum()
    }
}

async fn server_uniproxy_config(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    let value = server_v1_config_value(state, &node_type, &node).await?;
    raw_value_response(value, headers, false)
}

async fn server_trojan_tidalab_config(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let local_port = params
        .get("local_port")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))?;
    let node = load_server_node(&state.db, "trojan", node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("节点不存在"))?;
    Ok(Json(json!({
        "run_type": "server",
        "local_addr": "0.0.0.0",
        "local_port": node.server_port,
        "remote_addr": "www.taobao.com",
        "remote_port": 80,
        "password": [],
        "ssl": {
            "cert": "/root/.cert/server.crt",
            "key": "/root/.cert/server.key",
            "sni": node.server_name.as_deref().unwrap_or(&node.host),
        },
        "api": {
            "enabled": true,
            "api_addr": "127.0.0.1",
            "api_port": local_port,
        }
    }))
    .into_response())
}

async fn server_deepbwork_config(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let local_port = params
        .get("local_port")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))?;
    let node = load_server_node(&state.db, "vmess", node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("节点不存在"))?;
    // The shared node row already carries tls_settings/network_settings; dnsSettings and
    // ruleSettings live only on v2_server_vmess, so fetch just those two extra columns.
    let (dns_settings_raw, rule_settings_raw) =
        sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT dnsSettings, ruleSettings FROM v2_server_vmess WHERE id = ? LIMIT 1",
        )
        .bind(node_id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or((None, None));

    let network = node.network.as_deref().unwrap_or("tcp");
    let mut stream_settings = serde_json::Map::new();
    stream_settings.insert("network".to_string(), json!(network));
    // setNetwork (DeepbworkController :144-171).
    if let Some(settings) = json_text(node.network_settings.as_deref())
        .as_object()
        .cloned()
    {
        let key = match network {
            "kcp" => "kcpSettings",
            "ws" => "wsSettings",
            "http" => "httpSettings",
            "domainsocket" => "dsSettings",
            "quic" => "quicSettings",
            "grpc" => "grpcSettings",
            _ => "tcpSettings",
        };
        stream_settings.insert(key.to_string(), serde_json::Value::Object(settings));
    }
    // setTls (DeepbworkController :213-231): serverName / allowInsecure are pulled from
    // tlsSettings (only when present) alongside the fixed certificate paths.
    if node.tls.unwrap_or_default() != 0 {
        stream_settings.insert("security".to_string(), json!("tls"));
        let tls_in = json_text(node.tls_settings.as_deref());
        let mut tls_out = serde_json::Map::new();
        if let Some(server_name) = tls_in.get("serverName") {
            tls_out.insert("serverName".to_string(), json!(php_string(server_name)));
        }
        if let Some(allow_insecure) = tls_in.get("allowInsecure") {
            tls_out.insert(
                "allowInsecure".to_string(),
                json!(php_int_truthy(allow_insecure)),
            );
        }
        tls_out.insert(
            "certificates".to_string(),
            json!([{
                "certificateFile": "/root/.cert/server.crt",
                "keyFile": "/root/.cert/server.key",
            }]),
        );
        stream_settings.insert(
            "tlsSettings".to_string(),
            serde_json::Value::Object(tls_out),
        );
    }

    // setRule (DeepbworkController :173-211). Start from the panel-wide block lists
    // (`array_filter(explode(PHP_EOL, config('v2board.server_v2ray_domain')))`), then append
    // the node's own filtered ruleSettings domain/protocol entries via array_merge.
    let config = state.config_snapshot();
    let rule_settings = json_text(rule_settings_raw.as_deref());
    let mut domain_rules: Vec<serde_json::Value> =
        explode_php_lines(config.server_v2ray_domain.as_deref());
    let mut protocol_rules: Vec<serde_json::Value> =
        explode_php_lines(config.server_v2ray_protocol.as_deref());
    if let Some(object) = rule_settings.as_object() {
        if let Some(domains) = object.get("domain").and_then(serde_json::Value::as_array) {
            domain_rules.extend(domains.iter().filter(|value| php_truthy(value)).cloned());
        }
        if let Some(protocols) = object.get("protocol").and_then(serde_json::Value::as_array) {
            protocol_rules.extend(protocols.iter().filter(|value| php_truthy(value)).cloned());
        }
    }
    let has_domain_rules = !domain_rules.is_empty();
    let has_protocol_rules = !protocol_rules.is_empty();
    // Sniffing stays enabled only while at least one block rule exists (:208-210).
    let sniffing_enabled = has_domain_rules || has_protocol_rules;
    let mut routing_rules =
        vec![json!({ "type": "field", "inboundTag": "api", "outboundTag": "api" })];
    if has_domain_rules {
        routing_rules
            .push(json!({ "type": "field", "domain": domain_rules, "outboundTag": "block" }));
    }
    if has_protocol_rules {
        routing_rules
            .push(json!({ "type": "field", "protocol": protocol_rules, "outboundTag": "block" }));
    }

    // setDns (DeepbworkController :131-142): merge the node dnsSettings, append the fixed
    // resolvers, and switch the freedom outbound to UseIP.
    let dns_in = json_text(dns_settings_raw.as_deref());
    let (dns_value, use_ip) = if dns_in.is_object() || dns_in.is_array() {
        let mut dns = dns_in;
        if let Some(servers) = dns
            .get_mut("servers")
            .and_then(serde_json::Value::as_array_mut)
        {
            servers.push(json!("1.1.1.1"));
            servers.push(json!("localhost"));
        }
        (dns, true)
    } else {
        (json!({}), false)
    };
    let freedom_settings = if use_ip {
        json!({ "domainStrategy": "UseIP" })
    } else {
        json!({})
    };

    // loglevel = `(int)config('v2board.server_log_enable') ? 'debug' : 'none'` (:119).
    let loglevel = if config.server_log_enable {
        "debug"
    } else {
        "none"
    };
    Ok(Json(json!({
        "log": { "loglevel": loglevel, "access": "access.log", "error": "error.log" },
        "api": { "services": ["HandlerService", "StatsService"], "tag": "api" },
        "dns": dns_value,
        "stats": {},
        "inbounds": [
            {
                "port": node.server_port,
                "protocol": "vmess",
                "settings": { "clients": [] },
                "sniffing": { "enabled": sniffing_enabled, "destOverride": ["http", "tls"] },
                "streamSettings": serde_json::Value::Object(stream_settings),
                "tag": "proxy",
            },
            {
                "listen": "127.0.0.1",
                "port": local_port,
                "protocol": "dokodemo-door",
                "settings": { "address": "0.0.0.0" },
                "tag": "api",
            }
        ],
        "outbounds": [
            { "protocol": "freedom", "settings": freedom_settings },
            { "protocol": "blackhole", "settings": {}, "tag": "block" }
        ],
        "routing": { "rules": routing_rules },
        "policy": {
            "levels": {
                "0": {
                    "handshake": 4,
                    "connIdle": 300,
                    "uplinkOnly": 5,
                    "downlinkOnly": 30,
                    "statsUserUplink": true,
                    "statsUserDownlink": true,
                }
            }
        }
    }))
    .into_response())
}

/// PHP `(string)` cast used by Deepbwork setTls for `serverName`.
fn php_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(true) => "1".to_string(),
        serde_json::Value::Bool(false) => String::new(),
        other => other.to_string(),
    }
}

/// PHP `(int)$x ? true : false` used by Deepbwork setTls for `allowInsecure`.
fn php_int_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(text) => {
            let text = text.trim();
            text.parse::<i64>()
                .map(|value| value != 0)
                .unwrap_or_else(|_| text.parse::<f64>().is_ok_and(|value| value != 0.0))
        }
        _ => false,
    }
}

/// PHP truthiness for `array_filter` (used on Deepbwork ruleSettings domain/protocol lists):
/// drops null, `false`, 0, and the empty string / "0".
/// Mirror `array_filter(explode(PHP_EOL, config(...)))`: split on newlines and drop the
/// PHP-falsy lines (`""` and `"0"`). A missing config value behaves like `explode` on `null`,
/// which yields a single empty element that `array_filter` then removes — i.e. an empty list.
fn explode_php_lines(value: Option<&str>) -> Vec<serde_json::Value> {
    value
        .into_iter()
        .flat_map(|text| text.split('\n'))
        .filter(|line| !line.is_empty() && *line != "0")
        .map(|line| serde_json::Value::String(line.to_string()))
        .collect()
}
fn php_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(text) => !text.is_empty() && text != "0",
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(object) => !object.is_empty(),
    }
}
fn response_wants_msgpack(headers: &HeaderMap) -> bool {
    headers
        .get("x-response-format")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("msgpack"))
}

fn raw_value_response(
    value: serde_json::Value,
    headers: &HeaderMap,
    msgpack: bool,
) -> Result<Response, ApiError> {
    if msgpack {
        let body = rmp_serde::to_vec_named(&value)
            .map_err(|_| ApiError::internal("msgpack encode failed"))?;
        let etag = sha1_hex(&body);
        if etag_matches(headers, &etag) {
            return not_modified_response(&etag);
        }
        let mut response = body.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-msgpack"),
        );
        insert_etag(response.headers_mut(), &etag)?;
        return Ok(response);
    }

    let body = serde_json::to_vec(&value).map_err(|_| ApiError::internal("json encode failed"))?;
    let etag = sha1_hex(&body);
    if etag_matches(headers, &etag) {
        return not_modified_response(&etag);
    }
    let mut response = Json(value).into_response();
    insert_etag(response.headers_mut(), &etag)?;
    Ok(response)
}

fn server_fail_response(message: impl Into<String>) -> Response {
    Json(json!({
        "status": "fail",
        "message": message.into(),
    }))
    .into_response()
}

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains(etag))
}

fn not_modified_response(etag: &str) -> Result<Response, ApiError> {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    insert_etag(response.headers_mut(), etag)?;
    Ok(response)
}

fn insert_etag(headers: &mut HeaderMap, etag: &str) -> Result<(), ApiError> {
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{etag}\""))
            .map_err(|_| ApiError::internal("invalid etag"))?,
    );
    Ok(())
}
fn required_i32_param(params: &HashMap<String, String>, key: &str) -> Result<i32, ApiError> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))
}

fn normalize_server_node_type(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "v2ray" => "vmess".to_string(),
        "hysteria2" => "hysteria".to_string(),
        value => value.to_string(),
    }
}

async fn load_uniproxy_node(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<(String, ServerNodeRow), ApiError> {
    let node_type = params
        .get("node_type")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_server_node_type)
        .ok_or_else(|| ApiError::legacy("server is not exist"))?;
    let node_id = required_i32_param(params, "node_id")?;
    let node = load_server_node(&state.db, &node_type, node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("server is not exist"))?;
    Ok((node_type, node))
}

async fn load_server_node(
    db: &DbPool,
    node_type: &str,
    node_id: i32,
) -> Result<Option<ServerNodeRow>, ApiError> {
    let Some(sql) = server_node_sql(node_type) else {
        return Ok(None);
    };
    Ok(
        sqlx::query_as::<_, ServerNodeRow>(AssertSqlSafe(sql.to_string()))
            .bind(node_id)
            .fetch_optional(db)
            .await?,
    )
}

fn server_node_sql(node_type: &str) -> Option<&'static str> {
    match node_type {
        "shadowsocks" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, NULL AS disable_sni,
                   NULL AS udp_relay_mode, NULL AS zero_rtt_handshake, NULL AS congestion_control,
                   cipher, obfs, obfs_settings, NULL AS obfs_password, NULL AS padding_scheme,
                   NULL AS allow_insecure, NULL AS server_name, NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_shadowsocks
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "vmess" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, tls,
                   tlsSettings AS tls_settings, NULL AS flow, network,
                   networkSettings AS network_settings, NULL AS encryption,
                   NULL AS encryption_settings, NULL AS disable_sni, NULL AS udp_relay_mode,
                   NULL AS zero_rtt_handshake, NULL AS congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, NULL AS allow_insecure, NULL AS server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_vmess
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "trojan" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, network,
                   network_settings, NULL AS encryption, NULL AS encryption_settings,
                   NULL AS disable_sni, NULL AS udp_relay_mode, NULL AS zero_rtt_handshake,
                   NULL AS congestion_control, NULL AS cipher, NULL AS obfs,
                   NULL AS obfs_settings, NULL AS obfs_password, NULL AS padding_scheme,
                   allow_insecure, server_name, NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_trojan
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "vless" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, tls,
                   tls_settings, flow, network, network_settings, encryption,
                   encryption_settings, NULL AS disable_sni, NULL AS udp_relay_mode,
                   NULL AS zero_rtt_handshake, NULL AS congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, NULL AS allow_insecure, NULL AS server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_vless
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "tuic" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, disable_sni,
                   udp_relay_mode, zero_rtt_handshake, congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, insecure AS allow_insecure, server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_tuic
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "hysteria" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, NULL AS disable_sni,
                   NULL AS udp_relay_mode, NULL AS zero_rtt_handshake, NULL AS congestion_control,
                   NULL AS cipher, obfs, NULL AS obfs_settings, obfs_password,
                   NULL AS padding_scheme, insecure AS allow_insecure, server_name,
                   up_mbps, down_mbps
            FROM v2_server_hysteria
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "anytls" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, NULL AS disable_sni,
                   NULL AS udp_relay_mode, NULL AS zero_rtt_handshake, NULL AS congestion_control,
                   NULL AS cipher, NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   padding_scheme, insecure AS allow_insecure, server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_anytls
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "v2node" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at, listen_ip, protocol, NULL AS version, tls,
                   tls_settings, flow, network, network_settings, encryption,
                   encryption_settings, disable_sni, udp_relay_mode, zero_rtt_handshake,
                   congestion_control, cipher, obfs, NULL AS obfs_settings, obfs_password,
                   padding_scheme, NULL AS allow_insecure, NULL AS server_name,
                   up_mbps, down_mbps
            FROM v2_server_v2node
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        _ => None,
    }
}

async fn server_available_users(
    db: &DbPool,
    group_ids: Vec<i32>,
) -> Result<Vec<ServerUserRow>, ApiError> {
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<MySql>::new(
        "SELECT id, uuid, speed_limit, device_limit FROM v2_user WHERE group_id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for group_id in group_ids {
            separated.push_bind(group_id);
        }
    }
    builder.push(") AND u + d < transfer_enable AND (expired_at >= ");
    builder.push_bind(Utc::now().timestamp());
    builder.push(" OR expired_at IS NULL) AND banned = 0");
    Ok(builder
        .build_query_as::<ServerUserRow>()
        .fetch_all(db)
        .await?)
}

/// Build the trojan/vmess (TrojanTidalab / Deepbwork) per-user object.
///
/// UniProxyController::user `array_filter`s null keys away, but TrojanTidalabController :46-52 and
/// DeepbworkController :46-55 serialize the RAW user model, so `speed_limit` / `device_limit` are
/// ALWAYS present there — emitted as JSON `null` when the column is null (both are uncast on the
/// User model, so null stays null). Match that: keep the keys instead of dropping them when None.
/// (ShadowsocksTidalab is different: it emits only id/port/cipher/secret and never carries these
/// two keys, so it does not use this helper.)
fn server_user_without_uuid(user: &ServerUserRow) -> serde_json::Map<String, serde_json::Value> {
    let mut item = serde_json::Map::new();
    item.insert("id".to_string(), json!(user.id));
    item.insert("speed_limit".to_string(), json!(user.speed_limit));
    item.insert("device_limit".to_string(), json!(user.device_limit));
    item
}

async fn server_cache_timestamp(
    redis: &redis::Client,
    suffix: &str,
    node_type: &str,
    node_id: i32,
) -> Result<(), ApiError> {
    server_cache_count(redis, suffix, node_type, node_id, Utc::now().timestamp()).await
}

async fn server_cache_count(
    redis: &redis::Client,
    suffix: &str,
    node_type: &str,
    node_id: i32,
    value: i64,
) -> Result<(), ApiError> {
    let key = format!(
        "SERVER_{}_{}_{node_id}",
        node_type.to_ascii_uppercase(),
        suffix
    );
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let _: () = conn.set_ex(key, value, 3600).await?;
    Ok(())
}

fn parse_traffic_entries(
    body: Option<&serde_json::Value>,
    params: &HashMap<String, String>,
) -> Vec<TrafficEntry> {
    if let Some(value) = body {
        let entries = traffic_entries_from_value(value);
        if !entries.is_empty() {
            return entries;
        }
    }
    match (
        params
            .get("user_id")
            .and_then(|value| value.parse::<i64>().ok()),
        params.get("u").and_then(|value| value.parse::<i64>().ok()),
        params.get("d").and_then(|value| value.parse::<i64>().ok()),
    ) {
        (Some(user_id), Some(u), Some(d)) => vec![TrafficEntry { user_id, u, d }],
        _ => Vec::new(),
    }
}

fn traffic_entries_from_value(value: &serde_json::Value) -> Vec<TrafficEntry> {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                let object = item.as_object()?;
                Some(TrafficEntry {
                    user_id: object.get("user_id").and_then(value_to_i64)?,
                    u: object.get("u").and_then(value_to_i64).unwrap_or_default(),
                    d: object.get("d").and_then(value_to_i64).unwrap_or_default(),
                })
            })
            .collect(),
        serde_json::Value::Object(object) => {
            if let Some(user_id) = object.get("user_id").and_then(value_to_i64) {
                return vec![TrafficEntry {
                    user_id,
                    u: object.get("u").and_then(value_to_i64).unwrap_or_default(),
                    d: object.get("d").and_then(value_to_i64).unwrap_or_default(),
                }];
            }
            object
                .iter()
                .filter_map(|(user_id, value)| {
                    let user_id = user_id.parse::<i64>().ok()?;
                    let (u, d) = traffic_pair_from_value(value)?;
                    Some(TrafficEntry { user_id, u, d })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn traffic_pair_from_value(value: &serde_json::Value) -> Option<(i64, i64)> {
    match value {
        serde_json::Value::Array(items) => Some((
            items.first().and_then(value_to_i64).unwrap_or_default(),
            items.get(1).and_then(value_to_i64).unwrap_or_default(),
        )),
        serde_json::Value::Object(object) => Some((
            object.get("u").and_then(value_to_i64).unwrap_or_default(),
            object.get("d").and_then(value_to_i64).unwrap_or_default(),
        )),
        _ => None,
    }
}

async fn persist_traffic_fetch(
    state: &AppState,
    node: &ServerNodeRow,
    node_type: &str,
    entries: &[TrafficEntry],
) -> Result<(), ApiError> {
    // Laravel multiplies traffic by `$server['rate']` as a raw string (TrafficFetchJob:43-44)
    // and stores it verbatim into the decimal `server_rate` column (StatUserJob). PHP coerces a
    // non-numeric / empty rate to 0 (so `(u+d)*rate == 0`), NOT to 1 — match that here. This is
    // the pinned "traffic-charge coercion" contract.
    let rate = node.rate.parse::<f64>().unwrap_or(0.0);
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    for entry in entries {
        let upload = (entry.u as f64 * rate).round() as i64;
        let download = (entry.d as f64 * rate).round() as i64;
        let _: () = redis::cmd("HINCRBY")
            .arg("v2board_upload_traffic")
            .arg(entry.user_id)
            .arg(upload)
            .query_async(&mut conn)
            .await?;
        let _: () = redis::cmd("HINCRBY")
            .arg("v2board_download_traffic")
            .arg(entry.user_id)
            .arg(download)
            .query_async(&mut conn)
            .await?;
    }

    let record_at = today_start_timestamp();
    let now = Utc::now().timestamp();
    let mut total_u = 0_i64;
    let mut total_d = 0_i64;
    for entry in entries {
        total_u += entry.u;
        total_d += entry.d;
        sqlx::query(
            r#"
            INSERT INTO v2_stat_user
                (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'd', ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                u = u + VALUES(u),
                d = d + VALUES(d),
                updated_at = VALUES(updated_at)
            "#,
        )
        .bind(entry.user_id)
        .bind(rate)
        .bind(entry.u)
        .bind(entry.d)
        .bind(record_at)
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO v2_stat_server
            (server_id, server_type, u, d, record_type, record_at, created_at, updated_at)
        VALUES (?, ?, ?, ?, 'd', ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            u = u + VALUES(u),
            d = d + VALUES(d),
            updated_at = VALUES(updated_at)
        "#,
    )
    .bind(node.id)
    .bind(node_type)
    .bind(total_u)
    .bind(total_d)
    .bind(record_at)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn server_v1_config_value(
    state: &AppState,
    node_type: &str,
    node: &ServerNodeRow,
) -> Result<serde_json::Value, ApiError> {
    let config = state.config_snapshot();
    let mut response = serde_json::Map::new();
    match node_type {
        "shadowsocks" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("cipher".to_string(), json!(node.cipher));
            response.insert("obfs".to_string(), json!(node.obfs));
            response.insert(
                "obfs_settings".to_string(),
                json_text(node.obfs_settings.as_deref()),
            );
            if let Some(cipher) = node.cipher.as_deref() {
                if cipher == "2022-blake3-aes-128-gcm" {
                    response.insert(
                        "server_key".to_string(),
                        json!(server_key(node.created_at, 16)),
                    );
                } else if cipher == "2022-blake3-aes-256-gcm" {
                    response.insert(
                        "server_key".to_string(),
                        json!(server_key(node.created_at, 32)),
                    );
                }
            }
        }
        "vmess" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings.as_deref()),
            );
            response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
        }
        "vless" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings.as_deref()),
            );
            response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
            response.insert("flow".to_string(), json!(node.flow));
            response.insert(
                "tls_settings".to_string(),
                json_text(node.tls_settings.as_deref()),
            );
            response.insert("encryption".to_string(), json!(node.encryption));
            response.insert(
                "encryption_settings".to_string(),
                json_text(node.encryption_settings.as_deref()),
            );
        }
        "trojan" => {
            response.insert("host".to_string(), json!(node.host));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings.as_deref()),
            );
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
        }
        "tuic" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
            response.insert(
                "congestion_control".to_string(),
                json!(node.congestion_control),
            );
            response.insert(
                "zero_rtt_handshake".to_string(),
                json!(node.zero_rtt_handshake.unwrap_or_default() != 0),
            );
        }
        "hysteria" => {
            let version = node.version.unwrap_or(2);
            response.insert("version".to_string(), json!(version));
            response.insert("host".to_string(), json!(node.host));
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
            response.insert(
                "up_mbps".to_string(),
                json!(node.up_mbps.unwrap_or_default()),
            );
            response.insert(
                "down_mbps".to_string(),
                json!(node.down_mbps.unwrap_or_default()),
            );
            if version == 1 {
                response.insert("obfs".to_string(), json!(node.obfs_password));
            } else {
                let ignore = node.up_mbps.unwrap_or_default() == 0
                    && node.down_mbps.unwrap_or_default() == 0;
                response.insert("ignore_client_bandwidth".to_string(), json!(ignore));
                response.insert("obfs".to_string(), json!(node.obfs));
                response.insert("obfs-password".to_string(), json!(node.obfs_password));
            }
        }
        "anytls" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
            response.insert(
                "padding_scheme".to_string(),
                json_text(node.padding_scheme.as_deref()),
            );
        }
        _ => {}
    }
    response.insert(
        "base_config".to_string(),
        json!({
            "push_interval": config.server_push_interval,
            "pull_interval": config.server_pull_interval,
        }),
    );
    let routes = server_routes(&state.db, parse_i32_json_list(node.route_id.as_ref())).await?;
    if !routes.is_empty() {
        response.insert("routes".to_string(), json!(routes));
    }
    Ok(serde_json::Value::Object(response))
}

async fn server_v2_config_value(
    state: &AppState,
    node: &ServerNodeRow,
) -> Result<serde_json::Value, ApiError> {
    let config = state.config_snapshot();
    let mut response = serde_json::Map::new();
    response.insert("listen_ip".to_string(), json!(node.listen_ip));
    response.insert("server_port".to_string(), json!(node.server_port));
    response.insert("network".to_string(), json!(node.network));
    response.insert(
        "network_settings".to_string(),
        json_text(node.network_settings.as_deref()),
    );
    response.insert("protocol".to_string(), json!(node.protocol));
    response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
    response.insert(
        "tls_settings".to_string(),
        json_text(node.tls_settings.as_deref()),
    );
    response.insert("encryption".to_string(), json!(node.encryption));
    response.insert(
        "encryption_settings".to_string(),
        json_text(node.encryption_settings.as_deref()),
    );
    response.insert("flow".to_string(), json!(node.flow));
    response.insert("cipher".to_string(), json!(node.cipher));
    response.insert(
        "congestion_control".to_string(),
        json!(node.congestion_control),
    );
    response.insert(
        "zero_rtt_handshake".to_string(),
        json!(node.zero_rtt_handshake.unwrap_or_default() != 0),
    );
    response.insert(
        "up_mbps".to_string(),
        json!(node.up_mbps.unwrap_or_default()),
    );
    response.insert(
        "down_mbps".to_string(),
        json!(node.down_mbps.unwrap_or_default()),
    );
    response.insert("obfs".to_string(), json!(node.obfs));
    response.insert("obfs_password".to_string(), json!(node.obfs_password));
    response.insert(
        "padding_scheme".to_string(),
        json_text(node.padding_scheme.as_deref()),
    );
    if let Some(cipher) = node.cipher.as_deref() {
        if cipher == "2022-blake3-aes-128-gcm" {
            response.insert(
                "server_key".to_string(),
                json!(server_key(node.created_at, 16)),
            );
        } else if cipher == "2022-blake3-aes-256-gcm" {
            response.insert(
                "server_key".to_string(),
                json!(server_key(node.created_at, 32)),
            );
        }
    }
    response.insert(
        "ignore_client_bandwidth".to_string(),
        json!(node.up_mbps.unwrap_or_default() == 0 && node.down_mbps.unwrap_or_default() == 0),
    );
    response.insert(
        "base_config".to_string(),
        json!({
            "push_interval": config.server_push_interval,
            "pull_interval": config.server_pull_interval,
            "node_report_min_traffic": config.server_node_report_min_traffic,
            "device_online_min_traffic": config.server_device_online_min_traffic,
        }),
    );
    let routes = server_routes(&state.db, parse_i32_json_list(node.route_id.as_ref())).await?;
    if !routes.is_empty() {
        response.insert("routes".to_string(), json!(routes));
    }
    Ok(serde_json::Value::Object(response))
}

async fn server_routes(
    db: &DbPool,
    route_ids: Vec<i32>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    if route_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<MySql>::new(
        "SELECT id, `match` AS match_text, action, action_value FROM v2_server_route WHERE id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for route_id in &route_ids {
            separated.push_bind(*route_id);
        }
    }
    builder.push(") ORDER BY FIELD(id, ");
    {
        let mut separated = builder.separated(", ");
        for route_id in &route_ids {
            separated.push_bind(*route_id);
        }
    }
    builder.push(")");
    let rows = builder
        .build_query_as::<ServerRouteRow>()
        .fetch_all(db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "match": json_text(Some(&row.match_text)),
                "action": row.action,
                "action_value": row.action_value,
            })
        })
        .collect())
}

fn json_text(value: Option<&str>) -> serde_json::Value {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn server_key(created_at: i64, length: usize) -> String {
    let seed = format!("{:x}", md5::compute(created_at.to_string().as_bytes()));
    standard_base64_encode(prefix_bytes(&seed, length))
}

fn parse_i32_json_list(value: Option<&String>) -> Vec<i32> {
    let Some(value) = value
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<i32>>(value)
        .ok()
        .or_else(|| value.parse::<i32>().ok().map(|value| vec![value]))
        .unwrap_or_default()
}

fn today_start_timestamp() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn count_alive_ips_mode0_sums_raw_connections() {
        // device_limit_mode 0 (UniProxyController::alive :185-191): raw connection count.
        let nodes = object(json!({
            "shadowsocks5": { "aliveips": ["1.1.1.1_5", "1.1.1.1_5", "2.2.2.2_5"], "lastupdateAt": 0 },
            "trojan9": { "aliveips": ["1.1.1.1_9"], "lastupdateAt": 0 },
            "alive_ip": 99,
        }));
        assert_eq!(count_alive_ips(&nodes, 0), 4);
    }

    #[test]
    fn count_alive_ips_mode1_dedups_unique_ips() {
        // device_limit_mode 1 (UniProxyController::alive :174-184): unique IPs, deduped by the
        // substring before the first '_', across every node bucket.
        let nodes = object(json!({
            "shadowsocks5": { "aliveips": ["1.1.1.1_5", "1.1.1.1_5", "2.2.2.2_5"], "lastupdateAt": 0 },
            "trojan9": { "aliveips": ["1.1.1.1_9"], "lastupdateAt": 0 },
            "alive_ip": 99,
        }));
        // {1.1.1.1, 2.2.2.2} regardless of node id suffix -> 2.
        assert_eq!(count_alive_ips(&nodes, 1), 2);
    }

    #[test]
    fn legacy_tidalab_user_etag_scopes_to_data_array() {
        // ShadowsocksTidalab:51 hashes the BARE $result array, not the {data} wrapper.
        let data =
            vec![json!({ "id": 1, "port": 443, "cipher": "aes-128-gcm", "secret": "uuid-1" })];
        let array_etag = sha1_hex(&serde_json::to_vec(&data).unwrap());
        let wrapper_etag = sha1_hex(&serde_json::to_vec(&json!({ "data": data.clone() })).unwrap());
        assert_ne!(
            array_etag, wrapper_etag,
            "wrapper and array hashes must differ"
        );

        let response =
            legacy_tidalab_user_response("shadowsocks", data, &HeaderMap::new()).unwrap();
        let etag = response
            .headers()
            .get(header::ETAG)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(etag, format!("\"{array_etag}\""));
    }

    #[test]
    fn v2_config_etag_matches_quoted_if_none_match() {
        // Item 7 pin (intentionally more correct than ServerController.php:105): a client that
        // echoes the emitted quoted ETag is matched via `contains`, yielding a 304.
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_static("\"abc123\""),
        );
        assert!(etag_matches(&headers, "abc123"));
    }

    #[test]
    fn php_int_truthy_matches_php_cast() {
        assert!(php_int_truthy(&json!(1)));
        assert!(php_int_truthy(&json!(true)));
        assert!(php_int_truthy(&json!("1")));
        assert!(!php_int_truthy(&json!(0)));
        assert!(!php_int_truthy(&json!(false)));
        assert!(!php_int_truthy(&json!("")));
        assert!(!php_int_truthy(&json!("0")));
    }

    #[test]
    fn php_truthy_filters_like_array_filter() {
        assert!(php_truthy(&json!("geosite:cn")));
        assert!(!php_truthy(&json!("")));
        assert!(!php_truthy(&json!("0")));
        assert!(!php_truthy(&serde_json::Value::Null));
    }

    #[test]
    fn tidalab_user_keeps_null_speed_and_device_limit() {
        // TrojanTidalab/Deepbwork serialize the raw user model, so both keys stay present and are
        // emitted as JSON null when the column is null (they are NOT array_filtered like UniProxy).
        let user = ServerUserRow {
            id: 7,
            uuid: "uuid-7".to_string(),
            speed_limit: None,
            device_limit: None,
        };
        let item = server_user_without_uuid(&user);
        assert_eq!(item.get("id"), Some(&json!(7)));
        assert_eq!(item.get("speed_limit"), Some(&serde_json::Value::Null));
        assert_eq!(item.get("device_limit"), Some(&serde_json::Value::Null));
        assert!(!item.contains_key("uuid"));

        let user = ServerUserRow {
            id: 8,
            uuid: "uuid-8".to_string(),
            speed_limit: Some(100),
            device_limit: Some(3),
        };
        let item = server_user_without_uuid(&user);
        assert_eq!(item.get("speed_limit"), Some(&json!(100)));
        assert_eq!(item.get("device_limit"), Some(&json!(3)));
    }

    #[test]
    fn uniproxy_user_drops_null_speed_and_device_limit() {
        // UniProxyController::user array_filters null attributes away, so the struct serialization
        // (used only by the uniproxy user endpoint) must keep skipping None. Guard against a
        // regression that would leak null keys onto the uniproxy path.
        let user = ServerUserRow {
            id: 1,
            uuid: "uuid-1".to_string(),
            speed_limit: None,
            device_limit: None,
        };
        let value = serde_json::to_value(&user).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.get("uuid"), Some(&json!("uuid-1")));
        assert!(!object.contains_key("speed_limit"));
        assert!(!object.contains_key("device_limit"));
    }

    #[test]
    fn explode_php_lines_splits_and_filters_falsy() {
        assert!(explode_php_lines(None).is_empty());
        assert!(explode_php_lines(Some("")).is_empty());
        assert_eq!(
            explode_php_lines(Some("baidu.com\n\ngoogle.com\n0")),
            vec![json!("baidu.com"), json!("google.com")]
        );
    }
}
