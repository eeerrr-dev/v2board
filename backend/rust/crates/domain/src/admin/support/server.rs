use super::*;
use base64::{Engine as _, engine::general_purpose};
use v2board_compat::{Code, Problem};

// Order matches ServerService::getAllServers() array_merge concatenation, so the
// getNodes list (before its stable sort by `sort`) preserves Laravel tie order.
pub(in super::super) const SERVER_TABLES: &[(&str, &str)] = &[
    ("shadowsocks", "server_shadowsocks"),
    ("vmess", "server_vmess"),
    ("trojan", "server_trojan"),
    ("tuic", "server_tuic"),
    ("hysteria", "server_hysteria"),
    ("vless", "server_vless"),
    ("anytls", "server_anytls"),
    ("v2node", "server_v2node"),
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

/// How a node column is serialized in the GET `nodes` full-row output. `Json`
/// mirrors an Eloquent `array` cast (decoded JSON). The legacy re-encoded
/// padding_scheme JSON *string* is retired: §4.1 kills stringified-JSON
/// members, so `padding_scheme` crosses as its decoded JSON value.
#[derive(Clone, Copy)]
enum NodeCast {
    Plain,
    Json,
}

use NodeCast::{Json as J, Plain as X};

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
    ("padding_scheme", J),
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
    ("padding_scheme", J),
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
        _ => Err(ApiError::business("Invalid server type")),
    }
}

/// Resolves the `{type}` path segment of the modern §6.7 server routes to its
/// node table; an unknown segment is the registry's 400 `invalid_server_type`.
pub(in super::super) fn server_table_for_kind(kind: &str) -> Result<&'static str, ApiError> {
    SERVER_TABLES
        .iter()
        .find(|(item, _)| *item == kind)
        .map(|(_, table)| *table)
        .ok_or_else(|| Problem::new(Code::InvalidServerType).into())
}

/// The submitted (or empty) TLS settings object, with reality keys generated
/// when `tls == 2` — ports the VlessController tls==2 forcing.
pub(in super::super) fn prepare_tls_settings(
    settings: Option<&Value>,
    tls: i64,
) -> Result<Value, ApiError> {
    let mut settings = settings.cloned().unwrap_or_else(|| json!({}));
    if tls == 2 {
        ensure_reality_keys(&mut settings)?;
    }
    Ok(settings)
}

pub(in super::super) fn prepare_v2node_tls_settings(
    settings: Option<&Value>,
    tls: i64,
) -> Result<Value, ApiError> {
    let mut settings = prepare_tls_settings(settings, tls)?;
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
        .ok_or_else(|| ApiError::business("TLS settings format is invalid"))?;
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

/// The submitted network settings object with the legacy value hygiene:
/// v2node coerces `acceptProxyProtocol` to a bool, and `network == "xhttp"`
/// normalizes the xhttp `extra` scalars. `network` is the value submitted in
/// the same request; a PATCH that omits `network` skips the xhttp
/// normalization (the stored network is not re-read).
pub(in super::super) fn prepare_network_settings(
    settings: &Value,
    network: Option<&str>,
    v2node: bool,
) -> Value {
    let mut settings = settings.clone();
    if v2node && let Some(object) = settings.as_object_mut() {
        coerce_object_bool(object, "acceptProxyProtocol");
    }
    if network == Some("xhttp") {
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

/// The submitted (or empty) encryption settings, with the mlkem768x25519plus
/// enrichment: v2node defaults `mode`/`rtt`/`ticket`, `1rtt` pins the ticket,
/// and one X25519 keypair supplies `private_key` + matching `password` when
/// either is missing.
pub(in super::super) fn prepare_encryption_settings(
    settings: Option<&Value>,
    encryption: Option<&str>,
    v2node: bool,
) -> Result<Value, ApiError> {
    let mut settings = settings.cloned().unwrap_or_else(|| json!({}));
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
        // (VlessController.php:87-99 / V2nodeController.php). The password IS the
        // public key that corresponds to private_key; two independent randoms would
        // leave clients unable to derive the shared secret.
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
