use super::*;

// Helper::trafficConvert — byte formatter with a `< 0 => "0"` branch that is
// deliberately checked after the KB branch (Helper.php:82-98).
pub(super) fn traffic_convert(byte: i128) -> String {
    let value = byte as f64;
    if value > GIB {
        format!("{} GB", php_round2(value / GIB))
    } else if value > MIB {
        format!("{} MB", php_round2(value / MIB))
    } else if value > KIB {
        format!("{} KB", php_round2(value / KIB))
    } else if byte < 0 {
        "0".to_string()
    } else {
        format!("{} B", php_round2(value))
    }
}

pub(super) fn insert_opt_part(parts: &mut Vec<String>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        parts.push(format!("{key}={value}"));
    }
}

pub(super) fn insert_query_param(
    params: &mut Vec<(String, String)>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        params.push((key.to_string(), value));
    }
}

pub(super) fn is_basic_shadowsocks_cipher(cipher: &str) -> bool {
    matches!(
        cipher,
        "aes-128-gcm" | "aes-192-gcm" | "aes-256-gcm" | "chacha20-ietf-poly1305"
    )
}

pub(super) fn format_date_timestamp(timestamp: i64) -> String {
    app_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

pub(super) fn add_multi_port_fields(
    object: &mut Map<String, Value>,
    server: &v2board_db::server::AvailableServerRow,
) {
    if let Some(mport) = mport(server) {
        object.insert("ports".to_string(), Value::String(mport.clone()));
        object.insert("mport".to_string(), Value::String(mport));
    }
}

pub(super) fn insert_opt_string(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        object.insert(key.to_string(), Value::String(value));
    }
}

pub(super) fn insert_opt_value(object: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value.filter(|value| !value.is_null()) {
        object.insert(key.to_string(), value);
    }
}

pub(super) fn port_value(server: &v2board_db::server::AvailableServerRow) -> Value {
    first_port(server)
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(first_port(server)))
}

pub(super) fn shadowsocks_password(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    if cipher.contains("2022-blake3") {
        let length = if cipher == "2022-blake3-aes-128-gcm" {
            16
        } else {
            32
        };
        let created_at = extra_string(server, "created_at").unwrap_or_default();
        let server_key_seed = format!("{:x}", md5::compute(created_at.as_bytes()));
        let server_key = standard_base64_encode(prefix_bytes(&server_key_seed, length));
        let user_key = standard_base64_encode(prefix_bytes(uuid, length));
        Some(format!("{server_key}:{user_key}"))
    } else {
        Some(uuid.to_string())
    }
}

pub(super) fn obfs_plugin_opts(mode: &str, host: Option<String>, path: Option<String>) -> String {
    let mut parts = vec![format!("obfs={mode}")];
    if let Some(host) = host.filter(|value| !value.is_empty()) {
        parts.push(format!("obfs-host={host}"));
    }
    if let Some(path) = path.filter(|value| !value.is_empty()) {
        parts.push(format!("path={path}"));
    }
    parts.join(";")
}

pub(super) fn split_jsonish_list(value: &str) -> Value {
    if let Ok(values) = serde_json::from_str::<Vec<String>>(value) {
        json!(values)
    } else {
        Value::String(value.to_string())
    }
}

const KIB: f64 = 1024.0;
const MIB: f64 = 1_048_576.0;
pub(super) const GIB: f64 = 1_073_741_824.0;

// Round half-away-from-zero to 2 decimals (PHP `round($x, 2)` default mode).
pub(super) fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

// Format a rounded value the way PHP prints a float: trailing zeros and a bare
// decimal point are dropped (e.g. 1.0 -> "1", 1.50 -> "1.5", 1.05 -> "1.05").
pub(super) fn php_round2(value: f64) -> String {
    let cents = (value * 100.0).round() as i64;
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    let whole = cents / 100;
    let frac = cents % 100;
    if frac == 0 {
        format!("{sign}{whole}")
    } else if frac % 10 == 0 {
        format!("{sign}{whole}.{}", frac / 10)
    } else {
        format!("{sign}{whole}.{frac:02}")
    }
}

pub(super) fn format_datetime_timestamp(timestamp: i64) -> String {
    app_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

pub(super) fn server_protocol(server: &v2board_db::server::AvailableServerRow) -> String {
    if server.r#type == "v2node" {
        return extra_string(server, "protocol").unwrap_or_else(|| "v2node".to_string());
    }
    server.r#type.clone()
}

pub(super) fn extra_json(
    server: &v2board_db::server::AvailableServerRow,
    key: &str,
) -> serde_json::Value {
    match server.extra.get(key) {
        Some(serde_json::Value::String(value)) => {
            serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.clone()))
        }
        Some(value) => value.clone(),
        None => serde_json::Value::Null,
    }
}

pub(super) fn extra_string(
    server: &v2board_db::server::AvailableServerRow,
    key: &str,
) -> Option<String> {
    server.extra.get(key).and_then(value_to_string)
}

pub(super) fn extra_i64(server: &v2board_db::server::AvailableServerRow, key: &str) -> Option<i64> {
    server.extra.get(key).and_then(value_to_i64)
}

pub(super) fn json_path_value<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub(super) fn json_path_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    json_path_value(value, path).and_then(value_to_string)
}

pub(super) fn json_path_i64(value: &serde_json::Value, path: &[&str]) -> Option<i64> {
    json_path_value(value, path).and_then(value_to_i64)
}

// Legacy v2 transport settings store Host/path as arrays; read the first element
// if the target is an array, otherwise stringify the scalar (QuantumultX.php uses
// `$header['request']['path'][0]` etc.).
pub(super) fn json_path_first_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    match json_path_value(value, path) {
        Some(serde_json::Value::Array(items)) => items.first().and_then(value_to_string),
        other => other.and_then(value_to_string),
    }
}

// PHP `!empty()` for a JSON value: null / empty string / empty array / empty
// object / 0 count as empty.
pub(super) fn value_is_non_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::String(text) => !text.is_empty() && text != "0",
        serde_json::Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(true),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
    }
}

pub(super) fn first_port(server: &v2board_db::server::AvailableServerRow) -> String {
    port_text(server)
        .split(',')
        .next()
        .unwrap_or_default()
        .split('-')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(super) fn mport(server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let port = port_text(server);
    (port.contains('-') || port.contains(',')).then_some(port)
}

pub(super) fn port_text(server: &v2board_db::server::AvailableServerRow) -> String {
    value_to_string(&server.port).unwrap_or_default()
}

pub(super) fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

pub(super) fn encode_uri_component(value: &str) -> String {
    percent_encode(value)
        .replace("%21", "!")
        .replace("%2A", "*")
        .replace("%27", "'")
        .replace("%28", "(")
        .replace("%29", ")")
}
