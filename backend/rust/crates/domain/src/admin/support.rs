use super::*;

#[derive(Debug, FromRow)]
pub(super) struct PaymentRow {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) payment: String,
    pub(super) icon: Option<String>,
    pub(super) handling_fee_fixed: Option<i64>,
    pub(super) handling_fee_percent: Option<f64>,
    pub(super) uuid: String,
    pub(super) config: String,
    pub(super) notify_domain: Option<String>,
    pub(super) enable: i8,
    pub(super) sort: Option<i64>,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct NoticeRaw {
    id: i64,
    title: String,
    content: String,
    img_url: Option<String>,
    tags: Option<String>,
    show: i8,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct NoticeDto {
    id: i64,
    title: String,
    content: String,
    img_url: Option<String>,
    tags: Option<Vec<String>>,
    show: i8,
    created_at: i64,
    updated_at: i64,
}

impl From<NoticeRaw> for NoticeDto {
    fn from(row: NoticeRaw) -> Self {
        let tags = row.tags.and_then(|value| {
            serde_json::from_str::<Vec<String>>(&value)
                .ok()
                .or_else(|| (!value.trim().is_empty()).then_some(vec![value]))
        });
        Self {
            id: row.id,
            title: row.title,
            content: row.content,
            img_url: row.img_url,
            tags,
            show: row.show,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
pub(super) struct UserCsvRow {
    pub(super) id: i64,
    pub(super) email: String,
    pub(super) token: String,
    pub(super) uuid: String,
    pub(super) created_at: i64,
}

pub(super) struct MailSettings {
    pub(super) host: String,
    pub(super) port: Option<u16>,
    pub(super) username: Option<String>,
    pub(super) password: Option<String>,
    pub(super) encryption: Option<String>,
    pub(super) from_address: Option<String>,
}

impl MailSettings {
    pub(super) fn load(_config: &AppConfig) -> Result<Self, ApiError> {
        let values = read_php_config("/laravel/config/v2board.php");
        let host = config_string(&values, "email_host")
            .ok_or_else(|| ApiError::legacy("Email host is not configured"))?;
        Ok(Self {
            host,
            port: config_string(&values, "email_port").and_then(|value| value.parse().ok()),
            username: config_string(&values, "email_username"),
            password: config_string(&values, "email_password"),
            encryption: config_string(&values, "email_encryption")
                .map(|value| value.to_ascii_lowercase()),
            from_address: config_string(&values, "email_from_address"),
        })
    }
}

pub(super) const SERVER_TABLES: &[(&str, &str)] = &[
    ("shadowsocks", "v2_server_shadowsocks"),
    ("vmess", "v2_server_vmess"),
    ("trojan", "v2_server_trojan"),
    ("tuic", "v2_server_tuic"),
    ("vless", "v2_server_vless"),
    ("hysteria", "v2_server_hysteria"),
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

#[derive(Debug, Clone)]
pub(super) enum AdminSqlValue {
    Null,
    Integer(i64),
    Text(String),
}

pub(super) fn push_admin_sql_value(
    separated: &mut sqlx::query_builder::Separated<'_, MySql, &str>,
    value: &AdminSqlValue,
) {
    match value {
        AdminSqlValue::Null => {
            separated.push_bind(Option::<String>::None);
        }
        AdminSqlValue::Integer(value) => {
            separated.push_bind(*value);
        }
        AdminSqlValue::Text(value) => {
            separated.push_bind(value.clone());
        }
    }
}

pub(super) fn push_admin_sql_bind(builder: &mut QueryBuilder<MySql>, value: &AdminSqlValue) {
    match value {
        AdminSqlValue::Null => {
            builder.push_bind(Option::<String>::None);
        }
        AdminSqlValue::Integer(value) => {
            builder.push_bind(*value);
        }
        AdminSqlValue::Text(value) => {
            builder.push_bind(value.clone());
        }
    }
}

pub(super) fn server_copy_columns(kind: &str) -> Result<&'static [&'static str], ApiError> {
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

pub(super) fn server_save_values(
    kind: &str,
    params: &HashMap<String, String>,
) -> Result<Vec<(&'static str, AdminSqlValue)>, ApiError> {
    let mut values = Vec::new();
    push_common_server_values(&mut values, params)?;
    match kind {
        "shadowsocks" => {
            values.push(("cipher", text_value(required_string(params, "cipher")?)));
            values.push(("obfs", optional_text_value(params, "obfs")));
            values.push((
                "obfs_settings",
                optional_json_text_value(params, "obfs_settings"),
            ));
        }
        "trojan" => {
            values.push(("network", text_value(required_string(params, "network")?)));
            values.push((
                "network_settings",
                optional_json_text_value(params, "network_settings"),
            ));
            values.push((
                "allow_insecure",
                optional_int_value(params, "allow_insecure", 0),
            ));
            values.push(("server_name", optional_text_value(params, "server_name")));
        }
        "vmess" => {
            values.push(("tls", optional_int_value(params, "tls", 0)));
            values.push(("network", text_value(required_string(params, "network")?)));
            values.push(("rules", optional_json_text_value(params, "rules")));
            values.push((
                "networkSettings",
                optional_json_text_value(params, "networkSettings"),
            ));
            values.push((
                "tlsSettings",
                optional_json_text_value(params, "tlsSettings"),
            ));
            values.push((
                "ruleSettings",
                optional_json_text_value(params, "ruleSettings"),
            ));
            values.push((
                "dnsSettings",
                optional_json_text_value(params, "dnsSettings"),
            ));
        }
        "tuic" => {
            values.push(("server_name", optional_text_value(params, "server_name")));
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
            values.push(("disable_sni", optional_int_value(params, "disable_sni", 0)));
            values.push((
                "udp_relay_mode",
                optional_text_value(params, "udp_relay_mode"),
            ));
            values.push((
                "zero_rtt_handshake",
                optional_int_value(params, "zero_rtt_handshake", 0),
            ));
            values.push((
                "congestion_control",
                optional_text_value(params, "congestion_control"),
            ));
        }
        "hysteria" => {
            values.push(("version", optional_int_value(params, "version", 2)));
            values.push(("up_mbps", optional_int_value(params, "up_mbps", 0)));
            values.push(("down_mbps", optional_int_value(params, "down_mbps", 0)));
            values.push(("obfs", optional_text_value(params, "obfs")));
            values.push((
                "obfs_password",
                hysteria_obfs_password(params, params.get("obfs")),
            ));
            values.push(("server_name", optional_text_value(params, "server_name")));
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
        }
        "vless" => {
            let tls = optional_i64(params, "tls").unwrap_or_default();
            let network = required_string(params, "network")?;
            let encryption = optional_string(params, "encryption");
            let mut flow = optional_string(params, "flow");
            if network != "tcp" {
                flow = None;
            }
            values.push(("tls", AdminSqlValue::Integer(tls)));
            values.push((
                "tls_settings",
                json_value(prepare_tls_settings(params, tls)?),
            ));
            values.push(("flow", optional_text(flow)));
            values.push(("network", text_value(network.clone())));
            values.push((
                "network_settings",
                json_value(prepare_network_settings(
                    params,
                    "network_settings",
                    &network,
                    false,
                )),
            ));
            values.push(("encryption", optional_text(encryption.clone())));
            values.push((
                "encryption_settings",
                json_value(prepare_encryption_settings(
                    params,
                    encryption.as_deref(),
                    false,
                )),
            ));
        }
        "anytls" => {
            values.push(("server_name", optional_text_value(params, "server_name")));
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
            values.push((
                "padding_scheme",
                optional_decoded_json_text_value(params, "padding_scheme"),
            ));
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
            let mut flow = optional_string(params, "flow");
            if network != "tcp" && encryption.as_deref() != Some("mlkem768x25519plus") {
                flow = None;
            }
            values.push((
                "listen_ip",
                text_value(
                    optional_string(params, "listen_ip").unwrap_or_else(|| "0.0.0.0".to_string()),
                ),
            ));
            values.push(("protocol", text_value(protocol.clone())));
            values.push(("tls", AdminSqlValue::Integer(tls)));
            values.push((
                "tls_settings",
                json_value(prepare_v2node_tls_settings(params, tls)?),
            ));
            values.push(("flow", optional_text(flow)));
            values.push(("network", text_value(network.clone())));
            values.push((
                "network_settings",
                json_value(prepare_network_settings(
                    params,
                    "network_settings",
                    &network,
                    true,
                )),
            ));
            values.push(("encryption", optional_text(encryption.clone())));
            values.push((
                "encryption_settings",
                json_value(prepare_encryption_settings(
                    params,
                    encryption.as_deref(),
                    true,
                )),
            ));
            values.push(("disable_sni", optional_int_value(params, "disable_sni", 0)));
            values.push((
                "udp_relay_mode",
                optional_text_value(params, "udp_relay_mode"),
            ));
            values.push((
                "zero_rtt_handshake",
                optional_int_value(params, "zero_rtt_handshake", 0),
            ));
            values.push((
                "congestion_control",
                optional_text_value(params, "congestion_control"),
            ));
            values.push((
                "cipher",
                optional_text(
                    optional_string(params, "cipher")
                        .or_else(|| (protocol == "shadowsocks").then(|| "aes-128-gcm".to_string())),
                ),
            ));
            values.push(("up_mbps", optional_int_value(params, "up_mbps", 0)));
            values.push(("down_mbps", optional_int_value(params, "down_mbps", 0)));
            values.push(("obfs", optional_text_value(params, "obfs")));
            values.push((
                "obfs_password",
                hysteria_obfs_password(params, params.get("obfs")),
            ));
            values.push((
                "padding_scheme",
                optional_decoded_json_text_value(params, "padding_scheme"),
            ));
        }
        _ => return Err(ApiError::legacy("Invalid server type")),
    }
    Ok(values)
}

pub(super) fn push_common_server_values(
    values: &mut Vec<(&'static str, AdminSqlValue)>,
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    values.push((
        "group_id",
        text_value(required_json_array_string(params, "group_id")?),
    ));
    values.push((
        "route_id",
        optional_json_array_text_value(params, "route_id"),
    ));
    values.push(("parent_id", optional_int_or_null_value(params, "parent_id")));
    values.push(("tags", optional_json_array_text_value(params, "tags")));
    values.push(("name", text_value(required_string(params, "name")?)));
    values.push(("rate", text_value(required_string(params, "rate")?)));
    values.push(("host", text_value(required_string(params, "host")?)));
    values.push(("port", text_value(required_string(params, "port")?)));
    values.push((
        "server_port",
        AdminSqlValue::Integer(required_i64(params, "server_port")?),
    ));
    values.push(("show", optional_int_value(params, "show", 0)));
    values.push(("sort", optional_int_or_null_value(params, "sort")));
    Ok(())
}

pub(super) fn text_value(value: String) -> AdminSqlValue {
    AdminSqlValue::Text(value)
}

pub(super) fn optional_text(value: Option<String>) -> AdminSqlValue {
    value
        .filter(|value| !value.trim().is_empty() && !value.eq_ignore_ascii_case("null"))
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::Null)
}

pub(super) fn optional_text_value(params: &HashMap<String, String>, key: &str) -> AdminSqlValue {
    optional_text(optional_string(params, key))
}

pub(super) fn optional_int_value(
    params: &HashMap<String, String>,
    key: &str,
    default: i64,
) -> AdminSqlValue {
    AdminSqlValue::Integer(optional_i64(params, key).unwrap_or(default))
}

pub(super) fn optional_int_or_null_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    optional_i64(params, key)
        .map(AdminSqlValue::Integer)
        .unwrap_or(AdminSqlValue::Null)
}

pub(super) fn optional_string(params: &HashMap<String, String>, key: &str) -> Option<String> {
    params
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
}

pub(super) fn required_json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<String, ApiError> {
    json_array_string(params, key)?.ok_or_else(|| ApiError::legacy("参数有误"))
}

pub(super) fn optional_json_array_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    json_array_string(params, key)
        .ok()
        .flatten()
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::Null)
}

pub(super) fn json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<String>, ApiError> {
    if let Some(value) = optional_string(params, key) {
        if serde_json::from_str::<Value>(&value).is_ok() {
            return Ok(Some(value));
        }
        return Ok(Some(json_string(&Value::Array(vec![json_scalar(&value)]))));
    }
    let values = json_array_param(params, key);
    Ok((!values.is_empty()).then(|| json_string(&Value::Array(values))))
}

pub(super) fn optional_json_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    optional_json_value(params, key)
        .map(json_value)
        .unwrap_or(AdminSqlValue::Null)
}

pub(super) fn optional_decoded_json_text_value(
    params: &HashMap<String, String>,
    key: &str,
) -> AdminSqlValue {
    let Some(value) = optional_string(params, key) else {
        return optional_json_text_value(params, key);
    };
    serde_json::from_str::<Value>(&value)
        .map(json_value)
        .unwrap_or(AdminSqlValue::Null)
}

pub(super) fn optional_json_value(params: &HashMap<String, String>, key: &str) -> Option<Value> {
    if let Some(value) = optional_string(params, key)
        && let Ok(parsed) = serde_json::from_str::<Value>(&value)
    {
        return Some(parsed);
    }
    let value = nested_json(params, key);
    match &value {
        Value::Object(object) if object.is_empty() => None,
        _ => Some(value),
    }
}

pub(super) fn json_value(value: Value) -> AdminSqlValue {
    AdminSqlValue::Text(json_string(&value))
}

pub(super) fn prepare_tls_settings(
    params: &HashMap<String, String>,
    tls: i64,
) -> Result<Value, ApiError> {
    let mut settings = optional_json_value(params, "tls_settings").unwrap_or_else(|| json!({}));
    if tls == 2 {
        ensure_reality_keys(&mut settings)?;
    }
    Ok(settings)
}

pub(super) fn prepare_v2node_tls_settings(
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

pub(super) fn ensure_reality_keys(settings: &mut Value) -> Result<(), ApiError> {
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
        object.insert(
            "short_id".to_string(),
            json!(format!("{:x}", md5::compute(private_key))[..8].to_string()),
        );
    }
    object
        .entry("server_port".to_string())
        .or_insert(json!("443"));
    Ok(())
}

pub(super) fn prepare_network_settings(
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

pub(super) fn normalize_xhttp_settings(settings: &mut Value, v2node: bool) {
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

pub(super) fn prepare_encryption_settings(
    params: &HashMap<String, String>,
    encryption: Option<&str>,
    v2node: bool,
) -> Value {
    let mut settings =
        optional_json_value(params, "encryption_settings").unwrap_or_else(|| json!({}));
    if encryption != Some("mlkem768x25519plus") {
        return settings;
    }
    let Some(object) = settings.as_object_mut() else {
        return json!({});
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
        let private_key = random_urlsafe_key(32);
        let password = random_urlsafe_key(32);
        object
            .entry("private_key".to_string())
            .or_insert(json!(private_key));
        object
            .entry("password".to_string())
            .or_insert(json!(password));
    }
    settings
}

pub(super) fn coerce_object_bool(object: &mut Map<String, Value>, key: &str) {
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

pub(super) fn coerce_object_i64(object: &mut Map<String, Value>, key: &str) {
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

pub(super) fn hysteria_obfs_password(
    params: &HashMap<String, String>,
    obfs: Option<&String>,
) -> AdminSqlValue {
    if obfs
        .map(|value| value.trim().is_empty() || value.eq_ignore_ascii_case("null"))
        .unwrap_or(true)
    {
        return AdminSqlValue::Null;
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

pub(super) fn server_key(timestamp: i64, length: usize) -> String {
    let digest = format!("{:x}", md5::compute(timestamp.to_string()));
    standard_base64_encode(&digest.as_bytes()[..length.min(digest.len())])
}

pub(super) fn x25519_key_pair_urlsafe() -> Result<(String, String), ApiError> {
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

pub(super) fn generate_ech_key_pair(outer_sni: &str) -> Result<(String, String), ApiError> {
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

pub(super) fn random_urlsafe_key(length: usize) -> String {
    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    bytes.truncate(length);
    base64_url_no_pad(&bytes)
}

pub(super) fn base64_url_no_pad(bytes: &[u8]) -> String {
    standard_base64_encode(bytes)
        .replace('+', "-")
        .replace('/', "_")
        .replace('=', "")
}

pub(super) fn standard_base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub(super) fn normalize_admin_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

pub(super) fn bool_i(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

pub(super) async fn fetch_json_list(db: &MySqlPool, sql: &str) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(super) async fn fetch_json_list_bind(
    db: &MySqlPool,
    sql: &str,
    bind: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(super) async fn fetch_json_list_page(
    db: &MySqlPool,
    sql: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(super) async fn fetch_json_list_page_bind(
    db: &MySqlPool,
    sql: &str,
    bind: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(super) async fn fetch_json_list_page_bind_text(
    db: &MySqlPool,
    sql: &str,
    bind: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(super) async fn fetch_json_one(
    db: &MySqlPool,
    sql: &str,
    bind: i64,
) -> Result<Option<Value>, ApiError> {
    let Some(row) = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .fetch_optional(db)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(row.0))
}

pub(super) fn json_rows(rows: Vec<Json<Value>>) -> Vec<Value> {
    rows.into_iter().map(|row| row.0).collect()
}

pub(super) fn required_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<String, ApiError> {
    params
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy(format!("{key} cannot be empty")))
}

pub(super) fn optional_i64(params: &HashMap<String, String>, key: &str) -> Option<i64> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| value.parse::<i64>().ok())
}

pub(super) fn optional_f64(params: &HashMap<String, String>, key: &str) -> Option<f64> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| value.parse::<f64>().ok())
}

pub(super) fn required_i64(params: &HashMap<String, String>, key: &str) -> Result<i64, ApiError> {
    optional_i64(params, key).ok_or_else(|| ApiError::legacy(format!("{key} cannot be empty")))
}

pub(super) fn page(params: &HashMap<String, String>) -> (i64, i64) {
    let current = optional_i64(params, "current").unwrap_or(1).max(1);
    let page_size = optional_i64(params, "pageSize")
        .or_else(|| optional_i64(params, "page_size"))
        .unwrap_or(10)
        .max(10);
    (current, page_size)
}

pub(super) fn offset(current: i64, page_size: i64) -> i64 {
    (current - 1) * page_size
}

pub(super) fn array_param(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Vec<i64>, ApiError> {
    let mut values = BTreeMap::<usize, i64>::new();
    for (raw_key, raw_value) in params {
        if let Some(index) = bracket_index(raw_key, key)
            && let Ok(value) = raw_value.parse::<i64>()
        {
            values.insert(index, value);
        }
    }
    if let Some(value) = params.get(key)
        && let Ok(parsed) = serde_json::from_str::<Vec<i64>>(value)
    {
        return Ok(parsed);
    }
    let values = values.into_values().collect::<Vec<_>>();
    if values.is_empty() {
        return Err(ApiError::legacy("参数有误"));
    }
    Ok(values)
}

pub(super) fn json_array_param(params: &HashMap<String, String>, key: &str) -> Vec<Value> {
    let mut values = BTreeMap::<usize, Value>::new();
    for (raw_key, raw_value) in params {
        if let Some(index) = bracket_index(raw_key, key) {
            values.insert(index, json_scalar(raw_value));
        }
    }
    values.into_values().collect()
}

pub(super) fn optional_json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Option<String> {
    if let Some(value) = params.get(key)
        && serde_json::from_str::<Value>(value).is_ok()
    {
        return Some(value.clone());
    }
    let values = json_array_param(params, key);
    (!values.is_empty()).then(|| json_string(&Value::Array(values)))
}

pub(super) fn bracket_index(raw_key: &str, key: &str) -> Option<usize> {
    raw_key
        .strip_prefix(&format!("{key}["))
        .and_then(|value| value.strip_suffix(']'))
        .and_then(|value| value.parse::<usize>().ok())
}

pub(super) fn nested_json(params: &HashMap<String, String>, key: &str) -> Value {
    let mut root = Value::Object(Map::new());
    for (raw_key, raw_value) in params {
        if let Some(path) = bracket_path(raw_key, key) {
            insert_nested_json(&mut root, &path, json_scalar(raw_value));
        }
    }
    if matches!(&root, Value::Object(object) if object.is_empty())
        && let Some(value) = params.get(key)
        && let Ok(parsed) = serde_json::from_str::<Value>(value)
    {
        return parsed;
    }
    root
}

pub(super) fn bracket_path(raw_key: &str, key: &str) -> Option<Vec<String>> {
    let mut rest = raw_key.strip_prefix(key)?;
    if rest.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    while let Some(value) = rest.strip_prefix('[') {
        let (part, tail) = value.split_once(']')?;
        parts.push(part.to_string());
        rest = tail;
    }
    (rest.is_empty() && !parts.is_empty()).then_some(parts)
}

pub(super) fn insert_nested_json(root: &mut Value, path: &[String], value: Value) {
    let Some((head, tail)) = path.split_first() else {
        *root = value;
        return;
    };
    if tail.is_empty() {
        if let Value::Object(object) = root {
            object.insert(head.clone(), value);
        }
        return;
    }
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    let Value::Object(object) = root else {
        return;
    };
    let child = object
        .entry(head.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    insert_nested_json(child, tail, value);
}

pub(super) fn json_scalar(value: &str) -> Value {
    if value.eq_ignore_ascii_case("null") {
        Value::Null
    } else if value == "true" {
        Value::Bool(true)
    } else if value == "false" {
        Value::Bool(false)
    } else if let Ok(value) = value.parse::<i64>() {
        json!(value)
    } else if let Ok(value) = value.parse::<f64>() {
        json!(value)
    } else {
        json!(value)
    }
}

pub(super) fn json_string(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

pub(super) fn truthy(value: Option<&String>) -> bool {
    matches!(
        value.map(String::as_str),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

pub(super) fn random_short() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

pub(super) fn random_token() -> String {
    Uuid::new_v4().simple().to_string()
}

pub(super) fn list_names(path: &str) -> Vec<String> {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

pub(super) fn read_php_config(path: &str) -> Map<String, Value> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Map::new();
    };
    let lines = content.lines().collect::<Vec<_>>();
    let mut values = Map::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim().trim_end_matches(',');
        index += 1;
        if !line.starts_with('\'') || !line.contains("=>") {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once("=>") else {
            continue;
        };
        let key = raw_key.trim().trim_matches('\'');
        if key.is_empty() {
            continue;
        }
        let raw_value = raw_value.trim();
        if raw_value.is_empty() || raw_value.starts_with("array") {
            let (array, next_index) = parse_php_array_lines(&lines, index, raw_value);
            index = next_index;
            values.insert(key.to_string(), Value::Array(array));
        } else {
            values.insert(key.to_string(), parse_php_value(raw_value));
        }
    }
    values
}

pub(super) fn config_string(config: &Map<String, Value>, key: &str) -> Option<String> {
    match config.get(key)? {
        Value::String(value) => (!value.trim().is_empty()).then(|| value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(super) fn parse_php_value(value: &str) -> Value {
    let value = value.trim().trim_end_matches(',').trim();
    if value.eq_ignore_ascii_case("null") {
        Value::Null
    } else if value.eq_ignore_ascii_case("true") {
        Value::Bool(true)
    } else if value.eq_ignore_ascii_case("false") {
        Value::Bool(false)
    } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        json!(
            value[1..value.len() - 1]
                .replace("\\'", "'")
                .replace("\\\\", "\\")
        )
    } else if let Ok(value) = value.parse::<i64>() {
        json!(value)
    } else if let Ok(value) = value.parse::<f64>() {
        json!(value)
    } else {
        json!(value)
    }
}

pub(super) fn parse_php_array_lines(
    lines: &[&str],
    mut index: usize,
    first_value: &str,
) -> (Vec<Value>, usize) {
    let mut depth = usize::from(first_value.starts_with("array"));
    if depth == 0 {
        while index < lines.len() {
            let line = lines[index].trim();
            index += 1;
            if line.starts_with("array") {
                depth = 1;
                break;
            }
            if !line.is_empty() {
                return (Vec::new(), index);
            }
        }
    }

    let mut items = Vec::new();
    while index < lines.len() && depth > 0 {
        let line = lines[index].trim().trim_end_matches(',');
        index += 1;
        if line.starts_with("array") {
            depth += 1;
            continue;
        }
        if line.starts_with(')') {
            depth = depth.saturating_sub(1);
            continue;
        }
        if depth == 1 {
            let raw_value = line
                .split_once("=>")
                .map(|(_, value)| value.trim())
                .unwrap_or(line);
            items.push(parse_php_value(raw_value));
        }
    }
    (items, index)
}

pub(super) fn merge_config_params(
    config: &mut Map<String, Value>,
    params: &HashMap<String, String>,
) {
    let mut arrays = BTreeMap::<String, BTreeMap<usize, Value>>::new();
    for (key, value) in params {
        if key == "auth_data" {
            continue;
        }
        if let Some((base, index)) = key
            .split_once('[')
            .and_then(|(base, rest)| rest.strip_suffix(']').map(|rest| (base, rest)))
            .and_then(|(base, index)| index.parse::<usize>().ok().map(|index| (base, index)))
        {
            arrays
                .entry(base.to_string())
                .or_default()
                .insert(index, json_scalar(value));
            continue;
        }
        config.insert(key.clone(), json_scalar(value));
    }
    for (key, values) in arrays {
        config.insert(key, Value::Array(values.into_values().collect()));
    }
}

pub(super) fn write_php_config(path: &str, value: &Value) -> Result<(), ApiError> {
    let path = std::path::Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| ApiError::legacy("配置目录不可写"))?;
    }
    let content = format!("<?php\n return {} ;", php_export(value, 0));
    std::fs::write(path, content).map_err(|_| ApiError::legacy("修改失败"))
}

pub(super) fn php_export(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'")),
        Value::Array(items) => {
            let inner_indent = " ".repeat(indent + 2);
            let closing_indent = " ".repeat(indent);
            let items = items
                .iter()
                .map(|value| format!("{inner_indent}{},", php_export(value, indent + 2)))
                .collect::<Vec<_>>()
                .join("\n");
            format!("array (\n{items}\n{closing_indent})")
        }
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let inner_indent = " ".repeat(indent + 2);
            let closing_indent = " ".repeat(indent);
            let items = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{inner_indent}'{}' => {},",
                        key.replace('\'', "\\'"),
                        php_export(&object[key], indent + 2)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("array (\n{items}\n{closing_indent})")
        }
    }
}

pub(super) fn ensure_theme_name(name: &str) -> Result<(), ApiError> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(ApiError::legacy("主题不存在"))
    }
}

pub(super) fn standard_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut normalized = value.trim().replace('-', "+").replace('_', "/");
    match normalized.len() % 4 {
        0 => {}
        2 => normalized.push_str("=="),
        3 => normalized.push('='),
        _ => return None,
    }
    let bytes = normalized.as_bytes();
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let c0 = base64_value(chunk[0])?;
        let c1 = base64_value(chunk[1])?;
        let c2 = if chunk[2] == b'=' {
            0
        } else {
            base64_value(chunk[2])?
        };
        let c3 = if chunk[3] == b'=' {
            0
        } else {
            base64_value(chunk[3])?
        };
        let combined = ((c0 as u32) << 18) | ((c1 as u32) << 12) | ((c2 as u32) << 6) | c3 as u32;
        output.push(((combined >> 16) & 0xff) as u8);
        if chunk[2] != b'=' {
            output.push(((combined >> 8) & 0xff) as u8);
        }
        if chunk[3] != b'=' {
            output.push((combined & 0xff) as u8);
        }
    }
    Some(output)
}

pub(super) fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub(super) fn is_server_path(path: &str, action: &str) -> bool {
    path.starts_with("server/") && path.ends_with(&format!("/{action}"))
}

pub(super) fn server_table_from_path(path: &str) -> Result<&'static str, ApiError> {
    let kind = server_kind_from_path(path)?;
    SERVER_TABLES
        .iter()
        .find(|(item, _)| *item == kind)
        .map(|(_, table)| *table)
        .ok_or_else(|| ApiError::legacy("Invalid server type"))
}

pub(super) fn server_kind_from_path(path: &str) -> Result<&str, ApiError> {
    let mut parts = path.split('/');
    let _server = parts.next();
    parts
        .next()
        .ok_or_else(|| ApiError::legacy("Invalid server type"))
}

pub(super) fn ensure_safe_table(table: &str) -> Result<(), ApiError> {
    let allowed = [
        "v2_plan",
        "v2_payment",
        "v2_notice",
        "v2_knowledge",
        "v2_coupon",
        "v2_giftcard",
        "v2_server_group",
        "v2_server_route",
        "v2_user",
        "v2_server_shadowsocks",
        "v2_server_vmess",
        "v2_server_trojan",
        "v2_server_tuic",
        "v2_server_vless",
        "v2_server_hysteria",
        "v2_server_anytls",
        "v2_server_v2node",
    ];
    if allowed.contains(&table) {
        Ok(())
    } else {
        Err(ApiError::legacy("Invalid table"))
    }
}

pub(super) fn ensure_toggle_column(column: &str) -> Result<(), ApiError> {
    if matches!(column, "show" | "enable") {
        Ok(())
    } else {
        Err(ApiError::legacy("Invalid column"))
    }
}

pub(super) fn first_day_of_month() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

pub(super) fn first_day_of_previous_month() -> i64 {
    let now = Local::now();
    let (year, month) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    Local
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

pub(super) fn start_of_today() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

pub(super) fn start_of_yesterday() -> i64 {
    start_of_today() - 86_400
}
