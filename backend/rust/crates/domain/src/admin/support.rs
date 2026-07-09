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

pub(super) struct MailSettings {
    pub(super) host: String,
    pub(super) port: Option<u16>,
    pub(super) username: Option<String>,
    pub(super) password: Option<String>,
    pub(super) encryption: Option<String>,
    pub(super) from_address: Option<String>,
}

impl MailSettings {
    pub(super) fn load(config: &AppConfig) -> Result<Self, ApiError> {
        let values = read_php_config(&config.runtime_paths.v2board_config);
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

// Order matches ServerService::getAllServers() array_merge concatenation, so the
// getNodes list (before its stable sort by `sort`) preserves Laravel tie order.
pub(super) const SERVER_TABLES: &[(&str, &str)] = &[
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
pub(super) fn server_node_select(kind: &str, table: &str) -> String {
    let mut pairs = String::new();
    for (name, cast) in node_columns(kind) {
        let expr = match cast {
            NodeCast::Plain => format!("`{name}`"),
            NodeCast::Json => format!("CAST(`{name}` AS JSON)"),
            NodeCast::Padding => format!("CAST(`{name}` AS CHAR)"),
        };
        pairs.push_str(&format!("'{name}', {expr}, "));
    }
    format!("SELECT JSON_OBJECT({pairs}'type', '{kind}') FROM {table} ORDER BY sort ASC")
}

/// PHP escapeshellarg(): wraps in single quotes, escaping embedded quotes.
pub(super) fn escapeshellarg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

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

pub(super) fn push_common_server_values(
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
        text_value(required_json_array_string(params, "group_id")?),
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

pub(super) fn text_value(value: String) -> AdminSqlValue {
    AdminSqlValue::Text(value)
}

/// Validated coupon columns (excluding `code`) present in a generate request.
/// Mirrors CouponGenerate rules; used for both single create/update and the
/// per-row inserts of multiGenerate.
pub(super) fn coupon_field_values(
    params: &HashMap<String, String>,
) -> Vec<(&'static str, AdminSqlValue)> {
    let mut values = Vec::new();
    if params.contains_key("name") {
        values.push(("name", optional_text_value(params, "name")));
    }
    for key in [
        "type",
        "value",
        "started_at",
        "ended_at",
        "limit_use",
        "limit_use_with_user",
    ] {
        if params.contains_key(key) {
            values.push((key, optional_int_or_null_value(params, key)));
        }
    }
    for key in ["limit_plan_ids", "limit_period"] {
        if params
            .keys()
            .any(|param| param == key || param.starts_with(&format!("{key}[")))
        {
            values.push((key, optional_json_array_text_value(params, key)));
        }
    }
    values
}

/// Validated giftcard columns (excluding `code`) present in a generate request.
pub(super) fn giftcard_field_values(
    params: &HashMap<String, String>,
) -> Vec<(&'static str, AdminSqlValue)> {
    let mut values = Vec::new();
    if params.contains_key("name") {
        values.push(("name", optional_text_value(params, "name")));
    }
    for key in [
        "type",
        "value",
        "plan_id",
        "started_at",
        "ended_at",
        "limit_use",
    ] {
        if params.contains_key(key) {
            values.push((key, optional_int_or_null_value(params, key)));
        }
    }
    values
}

/// Joins a reconstructed array param with `/` for CSV display, or returns the
/// localized "unlimited" placeholder when the param was not supplied.
pub(super) fn joined_array_display(params: &HashMap<String, String>, key: &str) -> String {
    let present = params
        .keys()
        .any(|param| param == key || param.starts_with(&format!("{key}[")));
    if !present {
        return "不限制".to_string();
    }
    json_array_param(params, key)
        .iter()
        .map(|value| match value {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
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

/// Builds a Laravel-style 422 validation error for a single field: the message
/// doubles as the top-level message and the field's first error.
pub(super) fn validation_error(field: &str, message: &str) -> ApiError {
    ApiError::validation(
        message,
        HashMap::from([(field.to_string(), vec![message.to_string()])]),
    )
}

/// A scalar request value trimmed of surrounding whitespace (Laravel's global
/// `TrimStrings` middleware), yielding `None` when the key is absent or the
/// value is empty after trimming. This is the presence test Laravel's
/// `required`/`nullable`/`integer` rules operate on — note it does NOT treat the
/// literal string `"null"` as empty (unlike `optional_string`), because Laravel
/// does not either.
pub(super) fn present_value<'a>(params: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// True when `key` (scalar or bracketed array) appears in the request params.
/// Mirrors Laravel's `required` presence check for nested inputs.
pub(super) fn param_present(params: &HashMap<String, String>, key: &str) -> bool {
    params
        .keys()
        .any(|param| param == key || param.starts_with(&format!("{key}[")))
}

/// Approximates Laravel's `url` rule (filter_var FILTER_VALIDATE_URL): requires a
/// `scheme://host` shape with an alphabetic-led scheme and a non-empty host.
pub(super) fn is_valid_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    let scheme_bytes = scheme.as_bytes();
    if scheme_bytes.is_empty()
        || !scheme_bytes[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty()
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

/// `ORDER BY` clause for admin list endpoints that accept `sort`/`sort_type`
/// (coupon/giftcard fetch), mirroring `Coupon::orderBy($sort, $sortType)`: the
/// direction is whitelisted to ASC/DESC (anything else, including a missing param,
/// falls back to DESC), and the column defaults to `id` when the param is absent or
/// empty. The column is backtick-wrapped with any backticks doubled, the same way
/// Laravel's query grammar quotes an identifier, so an unknown column produces a SQL
/// error rather than an injection point.
pub(super) fn admin_sort_clause(params: &HashMap<String, String>) -> String {
    let direction = match params.get("sort_type").map(String::as_str) {
        Some("ASC") => "ASC",
        _ => "DESC",
    };
    let column = params
        .get("sort")
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("id");
    format!("ORDER BY `{}` {direction}", column.replace('`', "``"))
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

pub(super) fn list_names(path: impl AsRef<std::path::Path>) -> Vec<String> {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

pub(super) fn read_php_config(path: impl AsRef<std::path::Path>) -> Map<String, Value> {
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

/// Whitelist of persistable config keys, ported verbatim from ConfigSave::RULES
/// (laravel .../Admin/ConfigSave.php:9-107). ConfigController::save only writes
/// keys present in this list, so `_admin_email`, `auth_data`, and any stray
/// field never reach the config file.
pub(super) fn config_save_whitelisted(base: &str) -> bool {
    const KEYS: &[&str] = &[
        "deposit_bounus",
        "ticket_status",
        "invite_force",
        "invite_commission",
        "invite_gen_limit",
        "invite_never_expire",
        "commission_first_time_enable",
        "commission_auto_check_enable",
        "commission_withdraw_limit",
        "commission_withdraw_method",
        "withdraw_close_enable",
        "commission_distribution_enable",
        "commission_distribution_l1",
        "commission_distribution_l2",
        "commission_distribution_l3",
        "logo",
        "force_https",
        "stop_register",
        "app_name",
        "app_description",
        "app_url",
        "subscribe_url",
        "subscribe_path",
        "try_out_enable",
        "try_out_plan_id",
        "try_out_hour",
        "tos_url",
        "currency",
        "currency_symbol",
        "plan_change_enable",
        "reset_traffic_method",
        "surplus_enable",
        "allow_new_period",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "show_info_to_server_enable",
        "show_subscribe_method",
        "show_subscribe_expire",
        "server_api_url",
        "server_token",
        "server_pull_interval",
        "server_push_interval",
        "device_limit_mode",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "frontend_theme",
        "frontend_theme_sidebar",
        "frontend_theme_header",
        "frontend_theme_color",
        "frontend_background_url",
        "email_template",
        "email_host",
        "email_port",
        "email_username",
        "email_password",
        "email_encryption",
        "email_from_address",
        "telegram_bot_enable",
        "telegram_bot_token",
        "telegram_discuss_id",
        "telegram_channel_id",
        "telegram_discuss_link",
        "windows_version",
        "windows_download_url",
        "macos_version",
        "macos_download_url",
        "android_version",
        "android_download_url",
        "email_whitelist_enable",
        "email_whitelist_suffix",
        "email_gmail_limit_enable",
        "recaptcha_enable",
        "recaptcha_key",
        "recaptcha_site_key",
        "email_verify",
        "safe_mode_enable",
        "register_limit_by_ip_enable",
        "register_limit_count",
        "register_limit_expire",
        "secure_path",
        "password_limit_enable",
        "password_limit_count",
        "password_limit_expire",
    ];
    KEYS.contains(&base)
}

/// Validates a config/save payload against ConfigSave::RULES before it is
/// written. Ports the enum (`in:...`), integer/numeric, url, regex and length
/// rules that guard against corrupt config; only present, non-empty values are
/// checked, matching Laravel's implicit skipping of empty optional inputs.
/// Returns the first failure as a Laravel-style 422.
pub(super) fn validate_config_params(params: &HashMap<String, String>) -> Result<(), ApiError> {
    let value = |key: &str| -> Option<&str> {
        params
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    };

    const IN_0_1: &[&str] = &[
        "invite_force",
        "invite_never_expire",
        "commission_first_time_enable",
        "commission_auto_check_enable",
        "withdraw_close_enable",
        "commission_distribution_enable",
        "force_https",
        "stop_register",
        "try_out_enable",
        "plan_change_enable",
        "surplus_enable",
        "allow_new_period",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "show_info_to_server_enable",
        "device_limit_mode",
        "telegram_bot_enable",
        "email_whitelist_enable",
        "email_gmail_limit_enable",
        "recaptcha_enable",
        "email_verify",
        "safe_mode_enable",
        "register_limit_by_ip_enable",
        "password_limit_enable",
    ];
    for key in IN_0_1 {
        if let Some(value) = value(key)
            && value != "0"
            && value != "1"
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }
    for key in ["ticket_status", "show_subscribe_method"] {
        if let Some(value) = value(key)
            && !matches!(value, "0" | "1" | "2")
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }
    if let Some(value) = value("reset_traffic_method")
        && !matches!(value, "0" | "1" | "2" | "3" | "4")
    {
        return Err(validation_error("reset_traffic_method", "参数格式有误"));
    }
    for key in ["frontend_theme_sidebar", "frontend_theme_header"] {
        if let Some(value) = value(key)
            && !matches!(value, "dark" | "light")
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }
    if let Some(value) = value("frontend_theme_color")
        && !matches!(value, "default" | "darkblue" | "black" | "green")
    {
        return Err(validation_error("frontend_theme_color", "参数格式有误"));
    }

    for (key, message) in [
        ("logo", "LOGO URL格式不正确，必须携带https(s)://"),
        ("app_url", "站点URL格式不正确，必须携带http(s)://"),
        ("tos_url", "服务条款URL格式不正确，必须携带http(s)://"),
        (
            "telegram_discuss_link",
            "Telegram群组地址必须为URL格式，必须携带http(s)://",
        ),
        ("frontend_background_url", "参数格式有误"),
    ] {
        if let Some(value) = value(key)
            && !is_valid_url(value)
        {
            return Err(validation_error(key, message));
        }
    }

    if let Some(value) = value("subscribe_path")
        && !value.starts_with('/')
    {
        return Err(validation_error("subscribe_path", "订阅路径必须以/开头"));
    }
    if let Some(value) = value("server_token")
        && value.chars().count() < 16
    {
        return Err(validation_error("server_token", "通讯密钥长度必须大于16位"));
    }
    if let Some(value) = value("secure_path") {
        if value.chars().count() < 8 {
            return Err(validation_error("secure_path", "后台路径长度最小为8位"));
        }
        if !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            return Err(validation_error("secure_path", "后台路径只能为字母或数字"));
        }
    }

    const INTEGERS: &[&str] = &[
        "invite_commission",
        "invite_gen_limit",
        "try_out_plan_id",
        "show_subscribe_expire",
        "server_pull_interval",
        "server_push_interval",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "register_limit_count",
        "register_limit_expire",
        "password_limit_count",
        "password_limit_expire",
    ];
    for key in INTEGERS {
        if let Some(value) = value(key)
            && value.parse::<i64>().is_err()
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }
    const NUMERICS: &[&str] = &[
        "try_out_hour",
        "commission_withdraw_limit",
        "commission_distribution_l1",
        "commission_distribution_l2",
        "commission_distribution_l3",
    ];
    for key in NUMERICS {
        if let Some(value) = value(key)
            && value.parse::<f64>().is_err()
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }

    // deposit_bounus[] tiers must match `<amount>:<bounus>` (empty tiers allowed).
    for tier in json_array_param(params, "deposit_bounus") {
        let Value::String(tier) = tier else {
            continue;
        };
        if tier.is_empty() {
            continue;
        }
        if !is_deposit_bounus_tier(&tier) {
            return Err(validation_error(
                "deposit_bounus",
                "充值奖励格式不正确，必须为充值金额:奖励金额",
            ));
        }
    }
    Ok(())
}

/// Ports `CouponGenerate::rules()`. Every failable rule declares a custom Chinese
/// message, so this returns the first failure in Laravel's field-declaration
/// order with the exact message the FormRequest emits (HTTP 422).
pub(super) fn coupon_generate_validation(params: &HashMap<String, String>) -> Result<(), ApiError> {
    // generate_count: nullable|integer|max:500
    if let Some(value) = present_value(params, "generate_count") {
        let Ok(count) = value.parse::<i64>() else {
            return Err(validation_error("generate_count", "生成数量必须为数字"));
        };
        if count > 500 {
            return Err(validation_error("generate_count", "生成数量最大为500个"));
        }
    }
    // name: required
    if present_value(params, "name").is_none() {
        return Err(validation_error("name", "名称不能为空"));
    }
    // type: required|in:1,2
    match present_value(params, "type") {
        None => return Err(validation_error("type", "类型不能为空")),
        Some(value) if !matches!(value, "1" | "2") => {
            return Err(validation_error("type", "类型格式有误"));
        }
        _ => {}
    }
    // value: required|integer
    match present_value(params, "value") {
        None => return Err(validation_error("value", "金额或比例不能为空")),
        Some(value) if value.parse::<i64>().is_err() => {
            return Err(validation_error("value", "金额或比例格式有误"));
        }
        _ => {}
    }
    // started_at / ended_at: required|integer
    for (key, required_msg, integer_msg) in [
        ("started_at", "开始时间不能为空", "开始时间格式有误"),
        ("ended_at", "结束时间不能为空", "结束时间格式有误"),
    ] {
        match present_value(params, key) {
            None => return Err(validation_error(key, required_msg)),
            Some(value) if value.parse::<i64>().is_err() => {
                return Err(validation_error(key, integer_msg));
            }
            _ => {}
        }
    }
    // limit_use / limit_use_with_user: nullable|integer
    for (key, integer_msg) in [
        ("limit_use", "最大使用次数格式有误"),
        ("limit_use_with_user", "限制用户使用次数格式有误"),
    ] {
        if let Some(value) = present_value(params, key)
            && value.parse::<i64>().is_err()
        {
            return Err(validation_error(key, integer_msg));
        }
    }
    // limit_plan_ids / limit_period: nullable|array. A scalar (non-bracketed)
    // value fails the `array` rule; a bracketed `key[..]` submission is an array
    // and passes, as does absence.
    for (key, array_msg) in [
        ("limit_plan_ids", "指定订阅格式有误"),
        ("limit_period", "指定周期格式有误"),
    ] {
        if present_value(params, key).is_some() {
            return Err(validation_error(key, array_msg));
        }
    }
    Ok(())
}

/// Ports `GiftcardGenerate::rules()`. `value` and `plan_id` use `required_if`, not
/// `required` — so V2Board's `value.required`/`plan_id.required` custom messages
/// never fire, and with no `zh-CN/validation.php` lang file the `required_if`,
/// `integer` fallbacks surface the untranslated key (e.g. `validation.required_if`)
/// exactly as the real backend does at HTTP 422.
pub(super) fn giftcard_generate_validation(
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    // generate_count: nullable|integer|max:500
    if let Some(value) = present_value(params, "generate_count") {
        let Ok(count) = value.parse::<i64>() else {
            return Err(validation_error("generate_count", "生成数量必须为数字"));
        };
        if count > 500 {
            return Err(validation_error("generate_count", "生成数量最大为500个"));
        }
    }
    // name: required
    if present_value(params, "name").is_none() {
        return Err(validation_error("name", "名称不能为空"));
    }
    // type: required|in:1,2,3,4,5
    let card_type = match present_value(params, "type") {
        None => return Err(validation_error("type", "类型不能为空")),
        Some(value) if !matches!(value, "1" | "2" | "3" | "4" | "5") => {
            return Err(validation_error("type", "类型格式有误"));
        }
        Some(value) => value,
    };
    // value: required_if:type,1,2,3,5 | nullable | integer
    match present_value(params, "value") {
        None if matches!(card_type, "1" | "2" | "3" | "5") => {
            return Err(validation_error("value", "validation.required_if"));
        }
        Some(value) if value.parse::<i64>().is_err() => {
            return Err(validation_error("value", "数值格式有误"));
        }
        _ => {}
    }
    // plan_id: required_if:type,5 | nullable | integer (no custom messages)
    match present_value(params, "plan_id") {
        None if card_type == "5" => {
            return Err(validation_error("plan_id", "validation.required_if"));
        }
        Some(value) if value.parse::<i64>().is_err() => {
            return Err(validation_error("plan_id", "validation.integer"));
        }
        _ => {}
    }
    // started_at / ended_at: required|integer
    for (key, required_msg, integer_msg) in [
        ("started_at", "开始时间不能为空", "开始时间格式有误"),
        ("ended_at", "结束时间不能为空", "结束时间格式有误"),
    ] {
        match present_value(params, key) {
            None => return Err(validation_error(key, required_msg)),
            Some(value) if value.parse::<i64>().is_err() => {
                return Err(validation_error(key, integer_msg));
            }
            _ => {}
        }
    }
    // limit_use: nullable|integer
    if let Some(value) = present_value(params, "limit_use")
        && value.parse::<i64>().is_err()
    {
        return Err(validation_error("limit_use", "最大使用次数格式有误"));
    }
    Ok(())
}

/// Ports `UserGenerate::rules()`. Only `generate_count` declares custom messages;
/// `expired_at`/`plan_id` (`integer`) and `email_suffix` (`required`) fall back to
/// the untranslated validation keys because there is no `zh-CN/validation.php`.
pub(super) fn user_generate_validation(params: &HashMap<String, String>) -> Result<(), ApiError> {
    // generate_count: nullable|integer|max:500
    if let Some(value) = present_value(params, "generate_count") {
        let Ok(count) = value.parse::<i64>() else {
            return Err(validation_error("generate_count", "生成数量必须为数字"));
        };
        if count > 500 {
            return Err(validation_error("generate_count", "生成数量最大为500个"));
        }
    }
    // expired_at / plan_id: nullable|integer
    for key in ["expired_at", "plan_id"] {
        if let Some(value) = present_value(params, key)
            && value.parse::<i64>().is_err()
        {
            return Err(validation_error(key, "validation.integer"));
        }
    }
    // email_suffix: required
    if present_value(params, "email_suffix").is_none() {
        return Err(validation_error("email_suffix", "validation.required"));
    }
    Ok(())
}

/// Matches ConfigSave's deposit_bounus regex `^\d+(\.\d+)?:\d+(\.\d+)?$`.
fn is_deposit_bounus_tier(tier: &str) -> bool {
    let Some((amount, bounus)) = tier.split_once(':') else {
        return false;
    };
    is_unsigned_decimal(amount) && is_unsigned_decimal(bounus)
}

fn is_unsigned_decimal(value: &str) -> bool {
    let (int_part, frac_part) = match value.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (value, None),
    };
    if int_part.is_empty() || !int_part.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match frac_part {
        Some(frac) => !frac.is_empty() && frac.bytes().all(|byte| byte.is_ascii_digit()),
        None => true,
    }
}

pub(super) fn merge_config_params(
    config: &mut Map<String, Value>,
    params: &HashMap<String, String>,
) {
    let mut arrays = BTreeMap::<String, BTreeMap<usize, Value>>::new();
    for (key, value) in params {
        if let Some((base, index)) = key
            .split_once('[')
            .and_then(|(base, rest)| rest.strip_suffix(']').map(|rest| (base, rest)))
            .and_then(|(base, index)| index.parse::<usize>().ok().map(|index| (base, index)))
        {
            if !config_save_whitelisted(base) {
                continue;
            }
            arrays
                .entry(base.to_string())
                .or_default()
                .insert(index, json_scalar(value));
            continue;
        }
        if !config_save_whitelisted(key) {
            continue;
        }
        config.insert(key.clone(), json_scalar(value));
    }
    for (key, values) in arrays {
        config.insert(key, Value::Array(values.into_values().collect()));
    }
}

pub(super) fn write_php_config(
    path: impl AsRef<std::path::Path>,
    value: &Value,
) -> Result<(), ApiError> {
    let path = path.as_ref();
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

/// PHP `array_filter()` (no callback) drops falsy scalars: '', '0', 0, 0.0,
/// false, null, and empty arrays/objects.
pub(super) fn php_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(value) => !value,
        Value::Number(value) => value.as_f64().map(|value| value == 0.0).unwrap_or(false),
        Value::String(value) => value.is_empty() || value == "0",
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
    }
}

/// Reconstructs the route `match` values from either a raw JSON-array string or
/// bracketed `match[i]` params. Mirrors the `(array)($params['match'] ?? [])`
/// cast in RouteController::save.
pub(super) fn route_match_values(params: &HashMap<String, String>) -> Vec<Value> {
    if let Some(raw) = params.get("match")
        && let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw)
    {
        return items;
    }
    json_array_param(params, "match")
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

// ---------------------------------------------------------------------------
// Admin user filtering / sorting.
// Ports UserController::filter (laravel .../Admin/UserController.php:36-62) and
// the sort/sort_type parsing in fetch (:66-69). All dynamic SQL is guarded by
// column and operator whitelists to stay injection-safe.
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(super) enum UserFilterClause {
    Compare {
        column: &'static str,
        op: &'static str,
        value: FilterBind,
    },
    IsNull {
        column: &'static str,
    },
}

#[derive(Debug)]
pub(super) enum FilterBind {
    Int(i64),
    Text(String),
}

/// Whitelisted v2_user columns usable in a filter[] key or a sort. Guards the
/// dynamically-built WHERE/ORDER BY clauses against SQL injection.
pub(super) fn user_column(key: &str) -> Option<&'static str> {
    const COLUMNS: &[&str] = &[
        "id",
        "email",
        "telegram_id",
        "balance",
        "discount",
        "commission_type",
        "commission_rate",
        "commission_balance",
        "t",
        "u",
        "d",
        "transfer_enable",
        "device_limit",
        "banned",
        "is_admin",
        "is_staff",
        "last_login_at",
        "uuid",
        "group_id",
        "plan_id",
        "speed_limit",
        "token",
        "expired_at",
        "remarks",
        "invite_user_id",
        "created_at",
        "updated_at",
    ];
    COLUMNS.iter().copied().find(|column| *column == key)
}

pub(super) fn user_filter_operator(condition: &str) -> Option<&'static str> {
    match condition {
        "=" => Some("="),
        ">" => Some(">"),
        "<" => Some("<"),
        ">=" => Some(">="),
        "<=" => Some("<="),
        "<>" | "!=" => Some("<>"),
        "like" | "LIKE" => Some("like"),
        _ => None,
    }
}

/// Returns the validated `(ORDER BY expression, direction)`. Mirrors fetch():
/// sort defaults to created_at, sort_type is DESC unless exactly "ASC".
pub(super) fn user_sort(params: &HashMap<String, String>) -> (String, &'static str) {
    let direction = match params.get("sort_type").map(String::as_str) {
        Some("ASC") => "ASC",
        _ => "DESC",
    };
    let sort_expr = match params
        .get("sort")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some("total_used") => "(u.u + u.d)".to_string(),
        Some(sort) => match user_column(sort) {
            Some(column) => format!("u.{column}"),
            None => "u.created_at".to_string(),
        },
        None => "u.created_at".to_string(),
    };
    (sort_expr, direction)
}

pub(super) fn push_user_where(builder: &mut QueryBuilder<MySql>, clauses: &[UserFilterClause]) {
    for clause in clauses {
        builder.push(" AND ");
        match clause {
            UserFilterClause::Compare { column, op, value } => {
                builder.push(format!("u.{column} {op} "));
                match value {
                    FilterBind::Int(value) => {
                        builder.push_bind(*value);
                    }
                    FilterBind::Text(value) => {
                        builder.push_bind(value.clone());
                    }
                }
            }
            UserFilterClause::IsNull { column } => {
                builder.push(format!("u.{column} IS NULL"));
            }
        }
    }
}

/// A single WHERE comparison for an order filter[]. Values are bound as strings,
/// matching Laravel's default PDO parameter binding.
#[derive(Debug)]
pub(super) enum OrderFilterClause {
    Compare {
        column: &'static str,
        op: &'static str,
        value: String,
    },
}

/// Whitelisted v2_order columns usable in a filter[] key. Guards the dynamically
/// built WHERE clause (OrderController::filter trusts the raw request key).
pub(super) fn order_column(key: &str) -> Option<&'static str> {
    const COLUMNS: &[&str] = &[
        "id",
        "invite_user_id",
        "user_id",
        "plan_id",
        "coupon_id",
        "payment_id",
        "type",
        "period",
        "trade_no",
        "callback_no",
        "total_amount",
        "handling_amount",
        "discount_amount",
        "surplus_amount",
        "refund_amount",
        "balance_amount",
        "status",
        "commission_status",
        "commission_balance",
        "actual_commission_balance",
        "paid_at",
        "created_at",
        "updated_at",
    ];
    COLUMNS.iter().copied().find(|column| *column == key)
}

/// Applies the is_commission scope and filter[] clauses to an order builder whose
/// order table is aliased `o`. Ports OrderController::fetch (:58-63) + filter().
pub(super) fn push_order_where(
    builder: &mut QueryBuilder<MySql>,
    is_commission: bool,
    clauses: &[OrderFilterClause],
) {
    if is_commission {
        builder.push(
            " AND o.invite_user_id IS NOT NULL AND o.status NOT IN (0, 2) AND o.commission_balance > 0",
        );
    }
    for clause in clauses {
        let OrderFilterClause::Compare { column, op, value } = clause;
        builder.push(format!(" AND o.`{column}` {op} "));
        builder.push_bind(value.clone());
    }
}

/// Reconstructs `filter[<i>][<field>]` request keys into per-index maps of raw
/// string values, ordered by index. Kept as raw strings (not `json_scalar`d) so
/// the literal `plan_id == 'null'` sentinel survives.
pub(super) fn collect_filter_entries(
    params: &HashMap<String, String>,
) -> Vec<BTreeMap<String, String>> {
    let mut entries: BTreeMap<usize, BTreeMap<String, String>> = BTreeMap::new();
    for (key, value) in params {
        let Some(rest) = key.strip_prefix("filter[") else {
            continue;
        };
        let Some((index, rest)) = rest.split_once(']') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Some(field) = rest
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            continue;
        };
        entries
            .entry(index)
            .or_default()
            .insert(field.to_string(), value.clone());
    }
    entries.into_values().collect()
}

/// Conditions accepted by FilterScope::scopeSetFilterAllowKeys.
pub(super) const LOG_FILTER_CONDITIONS: &[&str] = &["in", "is", "not", "like", "lt", "gt"];

/// Applies system-log filter[] entries to a builder. `key` is validated to
/// `level`, so the column is fixed. Ports FilterScope's condition mapping
/// (App\Scope\FilterScope): in/is → equality, not → <>, gt/lt → >/<, like → %v%.
pub(super) fn push_log_filters(
    builder: &mut QueryBuilder<MySql>,
    entries: &[BTreeMap<String, String>],
) {
    for entry in entries {
        let condition = entry
            .get("condition")
            .map(String::as_str)
            .unwrap_or_default();
        let value = entry.get("value").cloned().unwrap_or_default();
        match condition {
            "in" | "is" => {
                builder.push(" AND level = ");
                builder.push_bind(value);
            }
            "not" => {
                builder.push(" AND level <> ");
                builder.push_bind(value);
            }
            "gt" => {
                builder.push(" AND level > ");
                builder.push_bind(value);
            }
            "lt" => {
                builder.push(" AND level < ");
                builder.push_bind(value);
            }
            "like" => {
                builder.push(" AND level LIKE ");
                builder.push_bind(format!("%{value}%"));
            }
            _ => {}
        }
    }
}

#[derive(Debug, FromRow)]
pub(super) struct UserDumpRow {
    pub(super) email: String,
    pub(super) balance: i64,
    pub(super) commission_balance: i64,
    pub(super) transfer_enable: i64,
    pub(super) u: i64,
    pub(super) d: i64,
    pub(super) device_limit: Option<i64>,
    pub(super) expired_at: Option<i64>,
    pub(super) plan_name: Option<String>,
    pub(super) token: String,
}

/// Parses an `ALIVE_IP_USER_<id>` cache payload into `(alive_ip, ips)`.
/// Mirrors UserController::fetch :89-102.
pub(super) fn parse_alive_ip(raw: &str) -> (i64, String) {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return (0, String::new());
    };
    let Some(object) = value.as_object() else {
        return (0, String::new());
    };
    let alive_ip = object
        .get("alive_ip")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut ips = Vec::new();
    for (node_type_id, data) in object {
        if node_type_id == "alive_ip" {
            continue;
        }
        let Some(alive_ips) = data.get("aliveips").and_then(Value::as_array) else {
            continue;
        };
        for entry in alive_ips {
            let Some(entry) = entry.as_str() else {
                continue;
            };
            let ip = entry.split('_').next().unwrap_or_default();
            ips.push(format!("{ip}_{node_type_id}"));
        }
    }
    (alive_ip, ips.join(", "))
}

/// Random `[a-zA-Z0-9]` string of `len` chars. Ports Helper::randomChar.
pub(super) fn random_char(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    (0..len)
        .map(|index| CHARS[(bytes[index] as usize) % CHARS.len()] as char)
        .collect()
}

/// PHP `date('Y-m-d H:i:s', ts)` in the server's local timezone.
pub(super) fn local_datetime(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// PHP `date('m-d', ts)` in the server's local timezone.
pub(super) fn local_month_day(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|value| value.format("%m-%d").to_string())
        .unwrap_or_default()
}

/// Node availability status, ported from ServerService::mergeData :414-420.
pub(super) fn node_available_status(
    now: i64,
    last_check_at: Option<i64>,
    last_push_at: Option<i64>,
) -> i64 {
    if now - 300 >= last_check_at.unwrap_or_default() {
        0
    } else if now - 300 >= last_push_at.unwrap_or_default() {
        1
    } else {
        2
    }
}

/// Maps a `v2_stat_server.server_type` onto the canonical node-table key used
/// for name resolution. Legacy stats recorded vmess nodes as `v2ray`.
pub(super) fn normalize_stat_server_type(server_type: &str) -> String {
    match server_type {
        "v2ray" => "vmess".to_string(),
        other => other.to_string(),
    }
}

/// True when a server's `group_id` JSON array contains `target` (loose match,
/// mirroring PHP `in_array` against string/int group ids).
pub(super) fn group_id_contains(group_id_json: &str, target: i64) -> bool {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(group_id_json) else {
        return false;
    };
    let target_string = target.to_string();
    items.iter().any(|item| match item {
        Value::Number(number) => number.as_i64() == Some(target),
        Value::String(value) => value == &target_string,
        _ => false,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_char_has_requested_length_and_charset() {
        let value = random_char(16);
        assert_eq!(value.chars().count(), 16);
        assert!(value.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn group_id_contains_matches_numeric_and_string_members() {
        assert!(group_id_contains("[1, 2, 3]", 2));
        assert!(group_id_contains("[\"1\", \"2\"]", 1));
        assert!(!group_id_contains("[1, 2]", 3));
        assert!(!group_id_contains("not json", 1));
        assert!(!group_id_contains("{}", 1));
    }

    #[test]
    fn node_available_status_reports_three_states() {
        let now = 10_000;
        // last_check older than 5 min -> offline (0)
        assert_eq!(node_available_status(now, Some(now - 400), Some(now)), 0);
        // check fresh, push stale -> degraded (1)
        assert_eq!(node_available_status(now, Some(now), Some(now - 400)), 1);
        // both fresh -> online (2)
        assert_eq!(node_available_status(now, Some(now), Some(now)), 2);
        // missing cache values default to 0 -> offline
        assert_eq!(node_available_status(now, None, None), 0);
    }

    #[test]
    fn normalize_stat_server_type_maps_legacy_v2ray() {
        assert_eq!(normalize_stat_server_type("v2ray"), "vmess");
        assert_eq!(normalize_stat_server_type("shadowsocks"), "shadowsocks");
    }

    #[test]
    fn user_column_and_operator_reject_unknown_input() {
        assert_eq!(user_column("email"), Some("email"));
        assert_eq!(user_column("id"), Some("id"));
        assert_eq!(user_column("password"), None);
        assert_eq!(user_column("email); DROP TABLE"), None);
        assert_eq!(user_filter_operator("="), Some("="));
        assert_eq!(user_filter_operator("like"), Some("like"));
        assert_eq!(user_filter_operator("!="), Some("<>"));
        // 模糊 is rewritten to `like` before reaching the operator whitelist.
        assert_eq!(user_filter_operator("模糊"), None);
        assert_eq!(user_filter_operator("; DELETE"), None);
    }

    #[test]
    fn user_sort_whitelists_expression_and_direction() {
        let mut params = HashMap::new();
        assert_eq!(user_sort(&params), ("u.created_at".to_string(), "DESC"));

        params.insert("sort".to_string(), "total_used".to_string());
        params.insert("sort_type".to_string(), "ASC".to_string());
        assert_eq!(user_sort(&params), ("(u.u + u.d)".to_string(), "ASC"));

        params.insert("sort".to_string(), "email".to_string());
        assert_eq!(user_sort(&params), ("u.email".to_string(), "ASC"));

        params.insert("sort".to_string(), "bogus".to_string());
        params.insert("sort_type".to_string(), "sideways".to_string());
        assert_eq!(user_sort(&params), ("u.created_at".to_string(), "DESC"));
    }

    #[test]
    fn collect_filter_entries_groups_by_index_and_keeps_raw_null() {
        let mut params = HashMap::new();
        params.insert("filter[0][key]".to_string(), "plan_id".to_string());
        params.insert("filter[0][condition]".to_string(), "=".to_string());
        params.insert("filter[0][value]".to_string(), "null".to_string());
        params.insert("filter[1][key]".to_string(), "email".to_string());
        let entries = collect_filter_entries(&params);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].get("key").map(String::as_str), Some("plan_id"));
        // Raw "null" must survive so plan_id == 'null' -> IS NULL still fires.
        assert_eq!(entries[0].get("value").map(String::as_str), Some("null"));
        assert_eq!(entries[1].get("key").map(String::as_str), Some("email"));
    }

    #[test]
    fn joined_array_display_joins_or_defaults() {
        let mut params = HashMap::new();
        assert_eq!(joined_array_display(&params, "limit_plan_ids"), "不限制");
        params.insert("limit_plan_ids[0]".to_string(), "1".to_string());
        params.insert("limit_plan_ids[1]".to_string(), "3".to_string());
        assert_eq!(joined_array_display(&params, "limit_plan_ids"), "1/3");
    }

    #[test]
    fn parse_alive_ip_extracts_count_and_ip_labels() {
        let raw = json!({
            "alive_ip": 2,
            "7": { "aliveips": ["1.2.3.4_ded", "5.6.7.8_abc"] }
        })
        .to_string();
        let (alive_ip, ips) = parse_alive_ip(&raw);
        assert_eq!(alive_ip, 2);
        assert_eq!(ips, "1.2.3.4_7, 5.6.7.8_7");
    }

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    /// Asserts the error is a 422 validation failure on `field` with `message`
    /// (which is also the top-level message), mirroring a single-rule FormRequest.
    fn assert_validation(result: Result<(), ApiError>, field: &str, message: &str) {
        match result {
            Err(ApiError::Validation {
                message: top,
                errors,
            }) => {
                assert_eq!(top, message, "top-level message");
                assert_eq!(
                    errors.get(field).map(Vec::as_slice),
                    Some([message.to_string()].as_slice()),
                    "errors[{field}]"
                );
            }
            other => panic!("expected 422 validation on {field}, got {other:?}"),
        }
    }

    // A complete, valid coupon/giftcard payload used as the baseline that each
    // test perturbs one field at a time.
    fn valid_coupon() -> Vec<(&'static str, &'static str)> {
        vec![
            ("name", "Promo"),
            ("type", "1"),
            ("value", "100"),
            ("started_at", "1700000000"),
            ("ended_at", "1800000000"),
        ]
    }

    #[test]
    fn coupon_generate_validation_reports_first_failure_in_declaration_order() {
        // A fully valid single-create payload passes.
        assert!(coupon_generate_validation(&params(&valid_coupon())).is_ok());

        // generate_count integer then max, ahead of every other field.
        let mut p = valid_coupon();
        p.push(("generate_count", "abc"));
        assert_validation(
            coupon_generate_validation(&params(&p)),
            "generate_count",
            "生成数量必须为数字",
        );
        let mut p = valid_coupon();
        p.push(("generate_count", "501"));
        assert_validation(
            coupon_generate_validation(&params(&p)),
            "generate_count",
            "生成数量最大为500个",
        );
        let mut p = valid_coupon();
        p.push(("generate_count", "500"));
        assert!(coupon_generate_validation(&params(&p)).is_ok());

        // Required + enum checks.
        assert_validation(
            coupon_generate_validation(&params(&[
                ("type", "1"),
                ("value", "1"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "name",
            "名称不能为空",
        );
        assert_validation(
            coupon_generate_validation(&params(&[
                ("name", "n"),
                ("type", "9"),
                ("value", "1"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "type",
            "类型格式有误",
        );
        assert_validation(
            coupon_generate_validation(&params(&[
                ("name", "n"),
                ("type", "1"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "value",
            "金额或比例不能为空",
        );
        assert_validation(
            coupon_generate_validation(&params(&[
                ("name", "n"),
                ("type", "1"),
                ("value", "x"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "value",
            "金额或比例格式有误",
        );

        // A scalar limit_plan_ids fails `array`; a bracketed one passes.
        let mut p = valid_coupon();
        p.push(("limit_plan_ids", "5"));
        assert_validation(
            coupon_generate_validation(&params(&p)),
            "limit_plan_ids",
            "指定订阅格式有误",
        );
        let mut p = valid_coupon();
        p.push(("limit_plan_ids[0]", "5"));
        assert!(coupon_generate_validation(&params(&p)).is_ok());
    }

    #[test]
    fn giftcard_generate_validation_uses_required_if_and_untranslated_keys() {
        // type=5 requires value and plan_id; the required_if failure surfaces the
        // untranslated key, not V2Board's dead `value.required` message.
        assert_validation(
            giftcard_generate_validation(&params(&[
                ("name", "g"),
                ("type", "5"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "value",
            "validation.required_if",
        );
        assert_validation(
            giftcard_generate_validation(&params(&[
                ("name", "g"),
                ("type", "5"),
                ("value", "10"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "plan_id",
            "validation.required_if",
        );
        // type=4 needs neither value nor plan_id.
        assert!(
            giftcard_generate_validation(&params(&[
                ("name", "g"),
                ("type", "4"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ]))
            .is_ok()
        );
        // A non-integer plan_id falls back to `validation.integer`.
        assert_validation(
            giftcard_generate_validation(&params(&[
                ("name", "g"),
                ("type", "5"),
                ("value", "10"),
                ("plan_id", "abc"),
                ("started_at", "1"),
                ("ended_at", "1"),
            ])),
            "plan_id",
            "validation.integer",
        );
        // type enum covers 1..=5.
        assert_validation(
            giftcard_generate_validation(&params(&[("name", "g"), ("type", "6")])),
            "type",
            "类型格式有误",
        );
    }

    #[test]
    fn user_generate_validation_requires_suffix_and_integer_checks() {
        assert!(user_generate_validation(&params(&[("email_suffix", "example.com")])).is_ok());
        assert_validation(
            user_generate_validation(&params(&[])),
            "email_suffix",
            "validation.required",
        );
        assert_validation(
            user_generate_validation(&params(&[("expired_at", "soon"), ("email_suffix", "x")])),
            "expired_at",
            "validation.integer",
        );
        assert_validation(
            user_generate_validation(&params(&[("generate_count", "999"), ("email_suffix", "x")])),
            "generate_count",
            "生成数量最大为500个",
        );
    }

    /// The required common columns every server save request must supply.
    fn server_common() -> Vec<(&'static str, &'static str)> {
        vec![
            ("group_id", "[1]"),
            ("name", "n"),
            ("rate", "1"),
            ("host", "h"),
            ("port", "1"),
            ("server_port", "1"),
        ]
    }

    fn saved_columns(kind: &str, extra: &[(&str, &str)]) -> Vec<&'static str> {
        let mut pairs = server_common();
        pairs.extend_from_slice(extra);
        server_save_values(kind, &params(&pairs))
            .unwrap()
            .into_iter()
            .map(|(column, _)| column)
            .collect()
    }

    #[test]
    fn server_save_omits_unsubmitted_optional_columns() {
        // A minimal shadowsocks save writes only required columns — never `sort`,
        // `show`, or the optional obfs pair — so a partial update preserves them.
        let cols = saved_columns("shadowsocks", &[("cipher", "aes-128-gcm")]);
        assert_eq!(
            cols,
            vec![
                "group_id",
                "name",
                "rate",
                "host",
                "port",
                "server_port",
                "cipher"
            ]
        );
        for absent in ["sort", "show", "obfs", "obfs_settings", "route_id", "tags"] {
            assert!(!cols.contains(&absent), "unexpected column {absent}");
        }

        // Supplying the optional keys opts them back in.
        let cols = saved_columns(
            "shadowsocks",
            &[
                ("cipher", "aes-128-gcm"),
                ("obfs", "http"),
                ("show", "1"),
                ("route_id[0]", "2"),
            ],
        );
        for present in ["obfs", "show", "route_id"] {
            assert!(cols.contains(&present), "missing column {present}");
        }
        assert!(!cols.contains(&"sort"));
    }

    #[test]
    fn server_save_vmess_never_writes_legacy_rules_column() {
        let cols = saved_columns("vmess", &[("tls", "1"), ("network", "tcp")]);
        assert!(cols.contains(&"tls") && cols.contains(&"network"));
        for absent in [
            "rules",
            "networkSettings",
            "tlsSettings",
            "ruleSettings",
            "dnsSettings",
        ] {
            assert!(!cols.contains(&absent), "unexpected column {absent}");
        }
        // A submitted settings blob is written.
        let cols = saved_columns(
            "vmess",
            &[("tls", "1"), ("network", "tcp"), ("tlsSettings[x]", "1")],
        );
        assert!(cols.contains(&"tlsSettings"));
        assert!(!cols.contains(&"rules"));
    }

    #[test]
    fn server_save_hysteria_always_writes_bandwidth_and_obfs_password() {
        // up_mbps/down_mbps/obfs_password are controller-assigned, so always present;
        // obfs and server_name are present-gated.
        let cols = saved_columns("hysteria", &[("version", "2"), ("insecure", "0")]);
        for always in [
            "version",
            "up_mbps",
            "down_mbps",
            "obfs_password",
            "insecure",
        ] {
            assert!(cols.contains(&always), "missing column {always}");
        }
        for absent in ["obfs", "server_name", "sort", "show"] {
            assert!(!cols.contains(&absent), "unexpected column {absent}");
        }
    }

    #[test]
    fn server_save_vless_gates_settings_flow_and_sort() {
        // tcp + tls=0: no forced settings/flow, sort omitted.
        let base = [("tls", "0"), ("network", "tcp")];
        let cols = saved_columns("vless", &base);
        for absent in [
            "tls_settings",
            "flow",
            "sort",
            "network_settings",
            "encryption",
        ] {
            assert!(!cols.contains(&absent), "unexpected column {absent}");
        }
        // tls=2 forces reality tls_settings even when unsubmitted.
        assert!(
            saved_columns("vless", &[("tls", "2"), ("network", "tcp")]).contains(&"tls_settings")
        );
        // A non-tcp network forces flow (to null).
        assert!(saved_columns("vless", &[("tls", "0"), ("network", "ws")]).contains(&"flow"));
        // sort is only written when submitted.
        let mut with_sort = base.to_vec();
        with_sort.push(("sort", "5"));
        assert!(saved_columns("vless", &with_sort).contains(&"sort"));
    }

    #[test]
    fn server_save_v2node_defaults_cipher_only_for_shadowsocks() {
        let ss = saved_columns(
            "v2node",
            &[
                ("protocol", "shadowsocks"),
                ("tls", "0"),
                ("network", "tcp"),
            ],
        );
        assert!(ss.contains(&"cipher"));
        for always in [
            "protocol",
            "up_mbps",
            "down_mbps",
            "obfs_password",
            "disable_sni",
        ] {
            assert!(ss.contains(&always), "missing column {always}");
        }
        for absent in ["listen_ip", "sort", "obfs", "tls_settings"] {
            assert!(!ss.contains(&absent), "unexpected column {absent}");
        }

        // vmess protocol never defaults cipher.
        let vmess = saved_columns(
            "v2node",
            &[("protocol", "vmess"), ("tls", "0"), ("network", "tcp")],
        );
        assert!(!vmess.contains(&"cipher"));
    }
}
