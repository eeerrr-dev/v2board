use super::*;
use base64::{Engine as _, engine::general_purpose};

// Order matches ServerService::getAllServers() array_merge concatenation, so the
// getNodes list (before its stable sort by `sort`) preserves Laravel tie order.
pub(in super::super) const SERVER_TABLES: &[(&str, &str)] = &[
    ("shadowsocks", "v2_server_shadowsocks"),
    ("vmess", "v2_server_vmess"),
    ("trojan", "v2_server_trojan"),
    ("tuic", "v2_server_tuic"),
    ("hysteria", "v2_server_hysteria"),
    ("vless", "v2_server_vless"),
    ("anytls", "v2_server_anytls"),
    ("v2node", "v2_server_v2node"),
];

const SERVER_SHADOWSOCKS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "cipher",
    "obfs",
    "obfs_settings",
    "show",
    "sort",
];

const SERVER_TROJAN_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "network",
    "network_settings",
    "allow_insecure",
    "server_name",
    "show",
    "sort",
];

const SERVER_VMESS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tls",
    "tags",
    "rate",
    "network",
    "rules",
    "networkSettings",
    "tlsSettings",
    "ruleSettings",
    "dnsSettings",
    "show",
    "sort",
];

const SERVER_TUIC_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "server_name",
    "insecure",
    "disable_sni",
    "udp_relay_mode",
    "zero_rtt_handshake",
    "congestion_control",
];

const SERVER_HYSTERIA_COLUMNS: &[&str] = &[
    "version",
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "up_mbps",
    "down_mbps",
    "obfs",
    "obfs_password",
    "server_name",
    "insecure",
];

const SERVER_VLESS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tls",
    "tls_settings",
    "flow",
    "network",
    "network_settings",
    "encryption",
    "encryption_settings",
    "tags",
    "rate",
    "show",
    "sort",
];

const SERVER_ANYTLS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "server_name",
    "insecure",
    "padding_scheme",
];

const SERVER_V2NODE_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "listen_ip",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "protocol",
    "tls",
    "tls_settings",
    "flow",
    "network",
    "network_settings",
    "encryption",
    "encryption_settings",
    "disable_sni",
    "udp_relay_mode",
    "zero_rtt_handshake",
    "congestion_control",
    "cipher",
    "up_mbps",
    "down_mbps",
    "obfs",
    "obfs_password",
    "padding_scheme",
];

/// How a node column is serialized in the getNodes full-row output. `Json`
/// mirrors an Eloquent `array` cast (decoded JSON); `Padding` mirrors the
/// re-encoded padding_scheme (a JSON string) in getAllAnyTLS/getAllV2node.
#[derive(Clone, Copy)]
enum NodeCast {
    Plain,
    Json,
    Padding,
}

use NodeCast::{Json as J, Padding as P, Plain as X};

const SHADOWSOCKS_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("parent_id", X),
    ("tags", J),
    ("name", X),
    ("rate", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("cipher", X),
    ("obfs", X),
    ("obfs_settings", J),
    ("show", X),
    ("sort", X),
    ("created_at", X),
    ("updated_at", X),
];

const VMESS_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("name", X),
    ("parent_id", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("tls", X),
    ("tags", J),
    ("rate", X),
    ("network", X),
    ("rules", X),
    ("networkSettings", J),
    ("tlsSettings", J),
    ("ruleSettings", J),
    ("dnsSettings", J),
    ("show", X),
    ("sort", X),
    ("created_at", X),
    ("updated_at", X),
];

const TROJAN_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("parent_id", X),
    ("tags", J),
    ("name", X),
    ("rate", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("network", X),
    ("network_settings", J),
    ("allow_insecure", X),
    ("server_name", X),
    ("show", X),
    ("sort", X),
    ("created_at", X),
    ("updated_at", X),
];

const TUIC_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("name", X),
    ("parent_id", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("tags", J),
    ("rate", X),
    ("show", X),
    ("sort", X),
    ("server_name", X),
    ("insecure", X),
    ("disable_sni", X),
    ("udp_relay_mode", X),
    ("zero_rtt_handshake", X),
    ("congestion_control", X),
    ("created_at", X),
    ("updated_at", X),
];

const HYSTERIA_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("version", X),
    ("group_id", J),
    ("route_id", J),
    ("name", X),
    ("parent_id", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("tags", J),
    ("rate", X),
    ("show", X),
    ("sort", X),
    ("up_mbps", X),
    ("down_mbps", X),
    ("obfs", X),
    ("obfs_password", X),
    ("server_name", X),
    ("insecure", X),
    ("created_at", X),
    ("updated_at", X),
];

const VLESS_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("name", X),
    ("parent_id", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("tls", X),
    ("tls_settings", J),
    ("flow", X),
    ("network", X),
    ("network_settings", J),
    ("encryption", X),
    ("encryption_settings", J),
    ("tags", J),
    ("rate", X),
    ("show", X),
    ("sort", X),
    ("created_at", X),
    ("updated_at", X),
];

const ANYTLS_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("name", X),
    ("parent_id", X),
    ("host", X),
    ("port", X),
    ("server_port", X),
    ("tags", J),
    ("rate", X),
    ("show", X),
    ("sort", X),
    ("server_name", X),
    ("insecure", X),
    ("padding_scheme", P),
    ("created_at", X),
    ("updated_at", X),
];

const V2NODE_NODE_COLS: &[(&str, NodeCast)] = &[
    ("id", X),
    ("group_id", J),
    ("route_id", J),
    ("name", X),
    ("parent_id", X),
    ("host", X),
    ("listen_ip", X),
    ("port", X),
    ("server_port", X),
    ("tags", J),
    ("rate", X),
    ("show", X),
    ("sort", X),
    ("protocol", X),
    ("tls", X),
    ("tls_settings", J),
    ("flow", X),
    ("network", X),
    ("network_settings", J),
    ("encryption", X),
    ("encryption_settings", J),
    ("disable_sni", X),
    ("udp_relay_mode", X),
    ("zero_rtt_handshake", X),
    ("congestion_control", X),
    ("cipher", X),
    ("up_mbps", X),
    ("down_mbps", X),
    ("obfs", X),
    ("obfs_password", X),
    ("padding_scheme", P),
    ("created_at", X),
    ("updated_at", X),
];

fn node_columns(kind: &str) -> &'static [(&'static str, NodeCast)] {
    match kind {
        "shadowsocks" => SHADOWSOCKS_NODE_COLS,
        "vmess" => VMESS_NODE_COLS,
        "trojan" => TROJAN_NODE_COLS,
        "tuic" => TUIC_NODE_COLS,
        "hysteria" => HYSTERIA_NODE_COLS,
        "vless" => VLESS_NODE_COLS,
        "anytls" => ANYTLS_NODE_COLS,
        "v2node" => V2NODE_NODE_COLS,
        _ => &[],
    }
}

/// Builds the getNodes SELECT for one protocol table: every DB column (with the
/// same JSON casts Eloquent's toArray() applies) plus the `type` label. Ports the
/// getAll<Protocol> getters in ServerService. `kind`/`table`/column names are all
/// compile-time literals, so the assembled SQL is injection-safe.
pub(in super::super) fn server_node_select(kind: &str, table: &str) -> String {
    let mut pairs = String::new();
    for (name, cast) in node_columns(kind) {
        let expr = match cast {
            NodeCast::Plain => format!("\"{name}\""),
            NodeCast::Json => format!("CAST(\"{name}\" AS JSONB)"),
            NodeCast::Padding => format!("CAST(\"{name}\" AS TEXT)"),
        };
        pairs.push_str(&format!("'{name}', {expr}, "));
    }
    format!(
        "SELECT jsonb_build_object({pairs}'type', '{kind}') FROM {table} \
         ORDER BY sort ASC NULLS FIRST"
    )
}

/// PHP escapeshellarg(): wraps in single quotes, escaping embedded quotes.
pub(in super::super) fn escapeshellarg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(in super::super) fn server_copy_columns(
    kind: &str,
) -> Result<&'static [&'static str], ApiError> {
    match kind {
        "shadowsocks" => Ok(SERVER_SHADOWSOCKS_COLUMNS),
        "trojan" => Ok(SERVER_TROJAN_COLUMNS),
        "vmess" => Ok(SERVER_VMESS_COLUMNS),
        "tuic" => Ok(SERVER_TUIC_COLUMNS),
        "hysteria" => Ok(SERVER_HYSTERIA_COLUMNS),
        "vless" => Ok(SERVER_VLESS_COLUMNS),
        "anytls" => Ok(SERVER_ANYTLS_COLUMNS),
        "v2node" => Ok(SERVER_V2NODE_COLUMNS),
        _ => Err(ApiError::legacy("Invalid server type")),
    }
}

pub(in super::super) fn server_save_values(
    kind: &str,
    params: &HashMap<String, String>,
) -> Result<Vec<(&'static str, AdminSqlValue)>, ApiError> {
    let mut values = Vec::new();
    push_common_server_values(&mut values, params)?;
    // Each arm mirrors its Server*Save rule set: `required` columns are pushed
    // unconditionally (always in `$params`), a handful the controller assigns
    // after validation are pushed unconditionally with the computed value, and
    // every remaining `nullable` column is present-gated so an omitted key is left
    // untouched on update — matching `$server->update($request->validated())`.
    match kind {
        "shadowsocks" => {
            values.push(("cipher", text_value(required_string(params, "cipher")?)));
            if param_present(params, "obfs") {
                values.push(("obfs", optional_text_value(params, "obfs")));
            }
            if param_present(params, "obfs_settings") {
                values.push((
                    "obfs_settings",
                    optional_json_text_value(params, "obfs_settings"),
                ));
            }
        }
        "trojan" => {
            values.push(("network", text_value(required_string(params, "network")?)));
            if param_present(params, "network_settings") {
                values.push((
                    "network_settings",
                    optional_json_text_value(params, "network_settings"),
                ));
            }
            if param_present(params, "allow_insecure") {
                values.push((
                    "allow_insecure",
                    optional_int_value(params, "allow_insecure", 0),
                ));
            }
            if param_present(params, "server_name") {
                values.push(("server_name", optional_text_value(params, "server_name")));
            }
        }
        "vmess" => {
            // ServerVmessSave has no `rules` rule, so the legacy `rules` column is
            // never written by save (dropped here, not present-gated).
            values.push(("tls", optional_int_value(params, "tls", 0)));
            values.push(("network", text_value(required_string(params, "network")?)));
            for key in [
                "networkSettings",
                "tlsSettings",
                "ruleSettings",
                "dnsSettings",
            ] {
                if param_present(params, key) {
                    values.push((key, optional_json_text_value(params, key)));
                }
            }
        }
        "tuic" => {
            if param_present(params, "server_name") {
                values.push(("server_name", optional_text_value(params, "server_name")));
            }
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
            values.push(("disable_sni", optional_int_value(params, "disable_sni", 0)));
            if param_present(params, "udp_relay_mode") {
                values.push((
                    "udp_relay_mode",
                    optional_text_value(params, "udp_relay_mode"),
                ));
            }
            values.push((
                "zero_rtt_handshake",
                optional_int_value(params, "zero_rtt_handshake", 0),
            ));
            if param_present(params, "congestion_control") {
                values.push((
                    "congestion_control",
                    optional_text_value(params, "congestion_control"),
                ));
            }
        }
        "hysteria" => {
            values.push(("version", optional_int_value(params, "version", 2)));
            // up_mbps/down_mbps default to 0 in the controller, so they are always
            // written; obfs_password is likewise always assigned (null when no obfs).
            values.push(("up_mbps", optional_int_value(params, "up_mbps", 0)));
            values.push(("down_mbps", optional_int_value(params, "down_mbps", 0)));
            if param_present(params, "obfs") {
                values.push(("obfs", optional_text_value(params, "obfs")));
            }
            values.push((
                "obfs_password",
                hysteria_obfs_password(params, params.get("obfs")),
            ));
            if param_present(params, "server_name") {
                values.push(("server_name", optional_text_value(params, "server_name")));
            }
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
        }
        "vless" => {
            // Unlike every other protocol table, PostgreSQL stores VLESS `port`
            // as INTEGER. Replace the common text value with an integer bind so
            // the dynamic INSERT/UPDATE has the target column's real type.
            let port = required_i64(params, "port")?;
            if let Some((_, value)) = values.iter_mut().find(|(column, _)| *column == "port") {
                *value = AdminSqlValue::Integer(port);
            }
            let tls = optional_i64(params, "tls").unwrap_or_default();
            let network = required_string(params, "network")?;
            let encryption = optional_string(params, "encryption");
            let mut flow = optional_string(params, "flow");
            if network != "tcp" {
                flow = None;
            }
            values.push(("tls", AdminSqlValue::Integer(tls)));
            // tls==2 forces reality settings into $params even when unsubmitted.
            if tls == 2 || param_present(params, "tls_settings") {
                values.push((
                    "tls_settings",
                    json_value(prepare_tls_settings(params, tls)?),
                ));
            }
            // flow is forced to null when network != tcp; otherwise present-gated.
            if network != "tcp" || param_present(params, "flow") {
                values.push(("flow", optional_text(flow)));
            }
            values.push(("network", text_value(network.clone())));
            if param_present(params, "network_settings") {
                values.push((
                    "network_settings",
                    json_value(prepare_network_settings(
                        params,
                        "network_settings",
                        &network,
                        false,
                    )),
                ));
            }
            if param_present(params, "encryption") {
                values.push(("encryption", optional_text(encryption.clone())));
            }
            // mlkem encryption forces encryption_settings into $params.
            if encryption.as_deref() == Some("mlkem768x25519plus")
                || param_present(params, "encryption_settings")
            {
                values.push((
                    "encryption_settings",
                    json_value(prepare_encryption_settings(
                        params,
                        encryption.as_deref(),
                        false,
                    )?),
                ));
            }
            if param_present(params, "sort") {
                values.push(("sort", optional_int_or_null_value(params, "sort")));
            }
        }
        "anytls" => {
            if param_present(params, "server_name") {
                values.push(("server_name", optional_text_value(params, "server_name")));
            }
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
            if param_present(params, "padding_scheme") {
                values.push((
                    "padding_scheme",
                    optional_decoded_json_text_value(params, "padding_scheme"),
                ));
            }
        }
        "v2node" => {
            let protocol = required_string(params, "protocol")?;
            let mut tls = optional_i64(params, "tls").unwrap_or_default();
            if (protocol == "anytls" && tls == 0)
                || matches!(protocol.as_str(), "hysteria2" | "trojan" | "tuic")
            {
                tls = 1;
            }
            let network = required_string(params, "network")?;
            let encryption = optional_string(params, "encryption");
            // Laravel only nulls flow when encryption is *present* and not mlkem
            // (V2nodeController.php: `... && isset($params['encryption']) && ...`).
            let force_flow_null = network != "tcp"
                && encryption.is_some()
                && encryption.as_deref() != Some("mlkem768x25519plus");
            let flow = if force_flow_null {
                None
            } else {
                optional_string(params, "flow")
            };
            if param_present(params, "listen_ip") {
                values.push(("listen_ip", optional_text_value(params, "listen_ip")));
            }
            values.push(("protocol", text_value(protocol.clone())));
            values.push(("tls", AdminSqlValue::Integer(tls)));
            if tls == 2 || param_present(params, "tls_settings") {
                values.push((
                    "tls_settings",
                    json_value(prepare_v2node_tls_settings(params, tls)?),
                ));
            }
            if force_flow_null || param_present(params, "flow") {
                values.push(("flow", optional_text(flow)));
            }
            values.push(("network", text_value(network.clone())));
            if param_present(params, "network_settings") {
                values.push((
                    "network_settings",
                    json_value(prepare_network_settings(
                        params,
                        "network_settings",
                        &network,
                        true,
                    )),
                ));
            }
            if param_present(params, "encryption") {
                values.push(("encryption", optional_text(encryption.clone())));
            }
            if encryption.as_deref() == Some("mlkem768x25519plus")
                || param_present(params, "encryption_settings")
            {
                values.push((
                    "encryption_settings",
                    json_value(prepare_encryption_settings(
                        params,
                        encryption.as_deref(),
                        true,
                    )?),
                ));
            }
            values.push(("disable_sni", optional_int_value(params, "disable_sni", 0)));
            if param_present(params, "udp_relay_mode") {
                values.push((
                    "udp_relay_mode",
                    optional_text_value(params, "udp_relay_mode"),
                ));
            }
            values.push((
                "zero_rtt_handshake",
                optional_int_value(params, "zero_rtt_handshake", 0),
            ));
            if param_present(params, "congestion_control") {
                values.push((
                    "congestion_control",
                    optional_text_value(params, "congestion_control"),
                ));
            }
            // cipher defaults to aes-128-gcm for shadowsocks; otherwise present-gated.
            if protocol == "shadowsocks" || param_present(params, "cipher") {
                values.push((
                    "cipher",
                    optional_text(optional_string(params, "cipher").or_else(|| {
                        (protocol == "shadowsocks").then(|| "aes-128-gcm".to_string())
                    })),
                ));
            }
            values.push(("up_mbps", optional_int_value(params, "up_mbps", 0)));
            values.push(("down_mbps", optional_int_value(params, "down_mbps", 0)));
            if param_present(params, "obfs") {
                values.push(("obfs", optional_text_value(params, "obfs")));
            }
            values.push((
                "obfs_password",
                hysteria_obfs_password(params, params.get("obfs")),
            ));
            if param_present(params, "padding_scheme") {
                values.push((
                    "padding_scheme",
                    optional_decoded_json_text_value(params, "padding_scheme"),
                ));
            }
            if param_present(params, "sort") {
                values.push(("sort", optional_int_or_null_value(params, "sort")));
            }
        }
        _ => return Err(ApiError::legacy("Invalid server type")),
    }
    Ok(values)
}

pub(in super::super) fn push_common_server_values(
    values: &mut Vec<(&'static str, AdminSqlValue)>,
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    // Every Server*Controller::save writes `$request->validated()`/`validate()`
    // then `update($params)`/`create($params)`, so only keys the request actually
    // supplied reach the row. `required` columns are always present; `nullable`/``
    // columns must be present-gated so a partial update leaves an omitted column
    // untouched instead of resetting it to a default. `sort` is intentionally
    // absent here — only vless/v2node declare a rule for it (handled per protocol);
    // the other protocols never write it, so drag-ordering survives every edit.
    values.push((
        "group_id",
        AdminSqlValue::Json(Some(
            serde_json::from_str(&required_json_array_string(params, "group_id")?)
                .map_err(|_| ApiError::validation_field("group_id", "节点组格式不正确"))?,
        )),
    ));
    values.push(("name", text_value(required_string(params, "name")?)));
    values.push(("rate", text_value(required_string(params, "rate")?)));
    values.push(("host", text_value(required_string(params, "host")?)));
    values.push(("port", text_value(required_string(params, "port")?)));
    values.push((
        "server_port",
        AdminSqlValue::Integer(required_i64(params, "server_port")?),
    ));
    if param_present(params, "route_id") {
        values.push((
            "route_id",
            optional_json_array_text_value(params, "route_id"),
        ));
    }
    if param_present(params, "parent_id") {
        values.push(("parent_id", optional_int_or_null_value(params, "parent_id")));
    }
    if param_present(params, "tags") {
        values.push(("tags", optional_json_array_text_value(params, "tags")));
    }
    if param_present(params, "show") {
        values.push(("show", optional_int_value(params, "show", 0)));
    }
    Ok(())
}

pub(in super::super) fn prepare_tls_settings(
    params: &HashMap<String, String>,
    tls: i64,
) -> Result<Value, ApiError> {
    let mut settings = optional_json_value(params, "tls_settings").unwrap_or_else(|| json!({}));
    if tls == 2 {
        ensure_reality_keys(&mut settings)?;
    }
    Ok(settings)
}

pub(in super::super) fn prepare_v2node_tls_settings(
    params: &HashMap<String, String>,
    tls: i64,
) -> Result<Value, ApiError> {
    let mut settings = prepare_tls_settings(params, tls)?;
    if let Some(object) = settings.as_object_mut()
        && object.get("ech").and_then(Value::as_str) == Some("custom")
    {
        let outer_sni = object
            .get("ech_server_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if outer_sni.is_empty() {
            object.insert("ech".to_string(), Value::String(String::new()));
        } else if object
            .get("ech_key")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .is_none()
            || object
                .get("ech_config")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            let (ech_key, ech_config) = generate_ech_key_pair(&outer_sni)?;
            object
                .entry("ech_key".to_string())
                .or_insert(json!(ech_key));
            object
                .entry("ech_config".to_string())
                .or_insert(json!(ech_config));
        }
    }
    Ok(settings)
}

pub(in super::super) fn ensure_reality_keys(settings: &mut Value) -> Result<(), ApiError> {
    let object = settings
        .as_object_mut()
        .ok_or_else(|| ApiError::legacy("TLS settings format is invalid"))?;
    let missing_public = object
        .get("public_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none();
    let missing_private = object
        .get("private_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none();
    if missing_public || missing_private {
        let (public_key, private_key) = x25519_key_pair_urlsafe()?;
        object
            .entry("public_key".to_string())
            .or_insert(json!(public_key));
        object
            .entry("private_key".to_string())
            .or_insert(json!(private_key));
    }
    if object
        .get("short_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none()
        && let Some(private_key) = object.get("private_key").and_then(Value::as_str)
    {
        // Laravel: substr(sha1($private_key), 0, 8) (VlessController.php:46) — SHA1 of the
        // base64url private_key string, first 8 lowercase-hex chars, NOT MD5.
        object.insert(
            "short_id".to_string(),
            json!(hex::encode(openssl::sha::sha1(private_key.as_bytes()))[..8].to_string()),
        );
    }
    object
        .entry("server_port".to_string())
        .or_insert(json!("443"));
    Ok(())
}

pub(in super::super) fn prepare_network_settings(
    params: &HashMap<String, String>,
    key: &str,
    network: &str,
    v2node: bool,
) -> Value {
    let mut settings = optional_json_value(params, key).unwrap_or_else(|| json!({}));
    if v2node && let Some(object) = settings.as_object_mut() {
        coerce_object_bool(object, "acceptProxyProtocol");
    }
    if network == "xhttp" {
        normalize_xhttp_settings(&mut settings, v2node);
    }
    settings
}

pub(in super::super) fn normalize_xhttp_settings(settings: &mut Value, v2node: bool) {
    let Some(object) = settings.as_object_mut() else {
        return;
    };
    let Some(extra) = object.get_mut("extra").and_then(Value::as_object_mut) else {
        return;
    };
    if v2node {
        coerce_object_bool(extra, "xPaddingObfsMode");
    }
    coerce_object_bool(extra, "noGRPCHeader");
    coerce_object_bool(extra, "noSSEHeader");
    coerce_object_i64(extra, "scMaxBufferedPosts");
    if let Some(xmux) = extra.get_mut("xmux").and_then(Value::as_object_mut) {
        coerce_object_i64(xmux, "hKeepAlivePeriod");
    }
    if let Some(download) = extra
        .get_mut("downloadSettings")
        .and_then(Value::as_object_mut)
    {
        coerce_object_i64(download, "port");
    }
}

pub(in super::super) fn prepare_encryption_settings(
    params: &HashMap<String, String>,
    encryption: Option<&str>,
    v2node: bool,
) -> Result<Value, ApiError> {
    let mut settings =
        optional_json_value(params, "encryption_settings").unwrap_or_else(|| json!({}));
    if encryption != Some("mlkem768x25519plus") {
        return Ok(settings);
    }
    let Some(object) = settings.as_object_mut() else {
        return Ok(json!({}));
    };
    if v2node {
        object.entry("mode".to_string()).or_insert(json!("native"));
    }
    match object.get("rtt").and_then(Value::as_str) {
        Some("1rtt") => {
            object.insert("ticket".to_string(), json!("0s"));
        }
        Some(_) => {}
        None if v2node => {
            object.insert("rtt".to_string(), json!("0rtt"));
            object.insert("ticket".to_string(), json!("600s"));
        }
        None => {}
    }
    if object
        .get("private_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none()
        || object
            .get("password")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        // Laravel generates ONE crypto_box (X25519) keypair and stores
        // private_key = base64url(secretkey), password = base64url(publickey)
        // (VlessController.php:87-99 / V2nodeController.php). The password IS the public
        // key that corresponds to private_key; two independent randoms would leave clients
        // unable to derive the shared secret.
        let (public_key, private_key) = x25519_key_pair_urlsafe()?;
        object
            .entry("private_key".to_string())
            .or_insert(json!(private_key));
        object
            .entry("password".to_string())
            .or_insert(json!(public_key));
    }
    Ok(settings)
}

pub(in super::super) fn coerce_object_bool(object: &mut Map<String, Value>, key: &str) {
    if let Some(value) = object.get_mut(key) {
        *value = Value::Bool(match value {
            Value::Bool(value) => *value,
            Value::Number(value) => value.as_i64().unwrap_or_default() != 0,
            Value::String(value) => matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        });
    }
}

pub(in super::super) fn coerce_object_i64(object: &mut Map<String, Value>, key: &str) {
    if let Some(value) = object.get_mut(key)
        && let Some(parsed) = match value {
            Value::Number(value) => value.as_i64(),
            Value::String(value) => value.parse::<i64>().ok(),
            Value::Bool(value) => Some(i64::from(*value)),
            _ => None,
        }
    {
        *value = json!(parsed);
    }
}

pub(in super::super) fn hysteria_obfs_password(
    params: &HashMap<String, String>,
    obfs: Option<&String>,
) -> AdminSqlValue {
    if obfs
        .map(|value| value.trim().is_empty() || value.eq_ignore_ascii_case("null"))
        .unwrap_or(true)
    {
        return AdminSqlValue::TextNull;
    }
    optional_string(params, "obfs_password")
        .map(AdminSqlValue::Text)
        .unwrap_or_else(|| {
            AdminSqlValue::Text(server_key(
                optional_i64(params, "created_at").unwrap_or_else(|| Utc::now().timestamp()),
                16,
            ))
        })
}

pub(in super::super) fn server_key(timestamp: i64, length: usize) -> String {
    let digest = format!("{:x}", md5::compute(timestamp.to_string()));
    standard_base64_encode(&digest.as_bytes()[..length.min(digest.len())])
}

pub(in super::super) fn x25519_key_pair_urlsafe() -> Result<(String, String), ApiError> {
    let key = PKey::generate_x25519()
        .map_err(|error| ApiError::legacy(format!("X25519 key generation failed: {error}")))?;
    let public_key = key
        .raw_public_key()
        .map_err(|error| ApiError::legacy(format!("X25519 public key export failed: {error}")))?;
    let private_key = key
        .raw_private_key()
        .map_err(|error| ApiError::legacy(format!("X25519 private key export failed: {error}")))?;
    Ok((
        base64_url_no_pad(&public_key),
        base64_url_no_pad(&private_key),
    ))
}

pub(in super::super) fn generate_ech_key_pair(
    outer_sni: &str,
) -> Result<(String, String), ApiError> {
    let key = PKey::generate_x25519()
        .map_err(|error| ApiError::legacy(format!("ECH key generation failed: {error}")))?;
    let public_key = key
        .raw_public_key()
        .map_err(|error| ApiError::legacy(format!("ECH public key export failed: {error}")))?;
    let private_key = key
        .raw_private_key()
        .map_err(|error| ApiError::legacy(format!("ECH private key export failed: {error}")))?;
    let config_id = Uuid::new_v4().as_bytes()[0];

    let mut config_data = Vec::new();
    config_data.push(config_id);
    config_data.extend_from_slice(&0x0020_u16.to_be_bytes());
    config_data.extend_from_slice(&(public_key.len() as u16).to_be_bytes());
    config_data.extend_from_slice(&public_key);
    let suites = [0x0001_u16, 0x0001, 0x0001, 0x0002, 0x0001, 0x0003];
    config_data.extend_from_slice(&((suites.len() * 2) as u16).to_be_bytes());
    for suite in suites {
        config_data.extend_from_slice(&suite.to_be_bytes());
    }
    config_data.push(0);
    config_data.push(outer_sni.len().min(u8::MAX as usize) as u8);
    config_data.extend_from_slice(&outer_sni.as_bytes()[..outer_sni.len().min(u8::MAX as usize)]);
    config_data.extend_from_slice(&0_u16.to_be_bytes());

    let mut ech_config = Vec::new();
    ech_config.extend_from_slice(&0xfe0d_u16.to_be_bytes());
    ech_config.extend_from_slice(&(config_data.len() as u16).to_be_bytes());
    ech_config.extend_from_slice(&config_data);

    let mut ech_keys = Vec::new();
    ech_keys.extend_from_slice(&(ech_config.len() as u16).to_be_bytes());
    ech_keys.extend_from_slice(&ech_config);
    ech_keys.extend_from_slice(&1_u16.to_be_bytes());
    ech_keys.push(config_id);
    ech_keys.extend_from_slice(&(private_key.len() as u16).to_be_bytes());
    ech_keys.extend_from_slice(&private_key);

    Ok((
        standard_base64_encode(&ech_keys),
        standard_base64_encode(&ech_config),
    ))
}

pub(in super::super) fn base64_url_no_pad(bytes: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(in super::super) fn standard_base64_encode(bytes: &[u8]) -> String {
    general_purpose::STANDARD.encode(bytes)
}
