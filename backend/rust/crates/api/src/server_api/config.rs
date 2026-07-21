use std::collections::HashMap;

use axum::{
    Json,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use serde_json::json;
use v2board_compat::ApiError;
use v2board_domain_model::ServerKind;

use crate::{
    codec::{prefix_bytes, standard_base64_encode},
    runtime::AppState,
};

use super::{
    ServerNodeRow,
    request::{load_server_node, load_uniproxy_node, required_i32_param},
    response::raw_value_response,
};

pub(super) async fn server_uniproxy_config(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    let value = server_v1_config_value(state, node_type, &node).await?;
    raw_value_response(value, headers, false)
}

pub(super) async fn server_trojan_tidalab_config(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let local_port = params
        .get("local_port")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))?;
    let node = load_server_node(state, ServerKind::Trojan, node_id)
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

pub(super) async fn server_deepbwork_config(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let local_port = params
        .get("local_port")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))?;
    let node = load_server_node(state, ServerKind::Vmess, node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("节点不存在"))?;
    let dns_settings_raw = node.dns_settings_json.as_deref();
    let rule_settings_raw = node.rule_settings_json.as_deref();

    let network = node.network.as_deref().unwrap_or("tcp");
    let mut stream_settings = serde_json::Map::new();
    stream_settings.insert("network".to_string(), json!(network));
    // setNetwork (DeepbworkController :144-171).
    if let Some(settings) = json_text(node.network_settings_json.as_deref())
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
        let tls_in = json_text(node.tls_settings_json.as_deref());
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
    let rule_settings = json_text(rule_settings_raw);
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
    let dns_in = json_text(dns_settings_raw);
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
pub(super) fn php_int_truthy(value: &serde_json::Value) -> bool {
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
pub(super) fn explode_php_lines(value: Option<&str>) -> Vec<serde_json::Value> {
    value
        .into_iter()
        .flat_map(|text| text.split('\n'))
        .filter(|line| !line.is_empty() && *line != "0")
        .map(|line| serde_json::Value::String(line.to_string()))
        .collect()
}
pub(super) fn php_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(text) => !text.is_empty() && text != "0",
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(object) => !object.is_empty(),
    }
}
async fn server_v1_config_value(
    state: &AppState,
    node_type: ServerKind,
    node: &ServerNodeRow,
) -> Result<serde_json::Value, ApiError> {
    let config = state.config_snapshot();
    let mut response = serde_json::Map::new();
    match node_type.as_str() {
        "shadowsocks" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("cipher".to_string(), json!(node.cipher));
            response.insert("obfs".to_string(), json!(node.obfs));
            response.insert(
                "obfs_settings".to_string(),
                json_text(node.obfs_settings_json.as_deref()),
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
                json_text(node.network_settings_json.as_deref()),
            );
            response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
        }
        "vless" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings_json.as_deref()),
            );
            response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
            response.insert("flow".to_string(), json!(node.flow));
            response.insert(
                "tls_settings".to_string(),
                json_text(node.tls_settings_json.as_deref()),
            );
            response.insert("encryption".to_string(), json!(node.encryption));
            response.insert(
                "encryption_settings".to_string(),
                json_text(node.encryption_settings_json.as_deref()),
            );
        }
        "trojan" => {
            response.insert("host".to_string(), json!(node.host));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings_json.as_deref()),
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
                json_text(node.padding_scheme_json.as_deref()),
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
    let routes = server_routes(state, &node.route_ids).await?;
    if !routes.is_empty() {
        response.insert("routes".to_string(), json!(routes));
    }
    Ok(serde_json::Value::Object(response))
}

pub(super) async fn server_v2_config_value(
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
        json_text(node.network_settings_json.as_deref()),
    );
    response.insert("protocol".to_string(), json!(node.protocol));
    response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
    response.insert(
        "tls_settings".to_string(),
        json_text(node.tls_settings_json.as_deref()),
    );
    response.insert("encryption".to_string(), json!(node.encryption));
    response.insert(
        "encryption_settings".to_string(),
        json_text(node.encryption_settings_json.as_deref()),
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
        json_text(node.padding_scheme_json.as_deref()),
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
    let routes = server_routes(state, &node.route_ids).await?;
    if !routes.is_empty() {
        response.insert("routes".to_string(), json!(routes));
    }
    Ok(serde_json::Value::Object(response))
}

async fn server_routes(
    state: &AppState,
    route_ids: &[i32],
) -> Result<Vec<serde_json::Value>, ApiError> {
    if route_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = state
        .server_runtime_service()
        .routes(route_ids)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "match": json_text(Some(&row.match_json)),
                "action": row.action,
                "action_value": row.action_value_json,
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

#[cfg(test)]
pub(super) fn parse_i32_json_list(value: Option<&String>) -> Vec<i32> {
    let Some(value) = value
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<serde_json::Value>>(value)
        .ok()
        .map(|items| {
            items
                .into_iter()
                .filter_map(|item| {
                    item.as_i64()
                        .and_then(|value| i32::try_from(value).ok())
                        .or_else(|| item.as_str().and_then(|value| value.parse::<i32>().ok()))
                })
                .collect::<Vec<_>>()
        })
        .or_else(|| value.parse::<i32>().ok().map(|value| vec![value]))
        .unwrap_or_default()
}
