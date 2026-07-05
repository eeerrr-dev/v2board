use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, Serialize)]
pub struct AvailableServerRow {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub group_id: Vec<i32>,
    pub route_id: Option<Vec<i32>>,
    pub name: String,
    pub rate: String,
    pub r#type: String,
    pub host: String,
    pub port: serde_json::Value,
    pub cache_key: String,
    pub last_check_at: Option<i64>,
    pub is_online: i8,
    pub tags: Option<Vec<String>>,
    pub sort: Option<i32>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, FromRow)]
struct RawServerRow {
    id: i32,
    parent_id: Option<i32>,
    group_id: String,
    route_id: Option<String>,
    name: String,
    rate: String,
    r#type: String,
    host: String,
    port: String,
    tags: Option<String>,
    sort: Option<i32>,
    updated_at: i64,
    extra: String,
}

pub async fn fetch_available_servers(
    pool: &MySqlPool,
    user_group_id: Option<i32>,
) -> Result<Vec<AvailableServerRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, RawServerRow>(AVAILABLE_SERVERS_SQL)
        .fetch_all(pool)
        .await?;
    let mut servers = Vec::new();
    for row in rows {
        let group_id = parse_i32_list(&row.group_id);
        let Some(user_group_id) = user_group_id else {
            continue;
        };
        if !group_id.contains(&user_group_id) {
            continue;
        }
        let last_check_at = None;
        let is_online = 0;
        let cache_key = format!("{}-{}-{}-{is_online}", row.r#type, row.id, row.updated_at);
        let port = row
            .port
            .parse::<i64>()
            .map(serde_json::Value::from)
            .unwrap_or_else(|_| serde_json::Value::String(row.port));
        servers.push(AvailableServerRow {
            id: row.id,
            parent_id: row.parent_id,
            group_id,
            route_id: row.route_id.as_deref().and_then(parse_i32_list_optional),
            name: row.name,
            rate: row.rate,
            r#type: row.r#type,
            host: row.host,
            port,
            cache_key,
            last_check_at,
            is_online,
            tags: row.tags.as_deref().and_then(parse_string_list_optional),
            sort: row.sort,
            extra: serde_json::from_str(&row.extra).unwrap_or(serde_json::Value::Null),
        });
    }
    servers.sort_by_key(|server| server.sort.unwrap_or_default());
    Ok(servers)
}

fn parse_i32_list(value: &str) -> Vec<i32> {
    parse_i32_list_optional(value).unwrap_or_default()
}

fn parse_i32_list_optional(value: &str) -> Option<Vec<i32>> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<i32>>(value)
        .ok()
        .or_else(|| value.parse::<i32>().ok().map(|item| vec![item]))
        .filter(|items| !items.is_empty())
}

fn parse_string_list_optional(value: &str) -> Option<Vec<String>> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<String>>(value)
        .ok()
        .or_else(|| {
            Some(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|items| !items.is_empty())
}

const AVAILABLE_SERVERS_SQL: &str = r#"
SELECT
    id, parent_id, group_id, route_id, name, rate, 'shadowsocks' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('cipher', cipher, 'obfs', obfs, 'obfs_settings', obfs_settings, 'created_at', created_at) AS extra
FROM v2_server_shadowsocks
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'vmess' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('network', network, 'tls', tls, 'network_settings', networkSettings, 'tls_settings', tlsSettings) AS extra
FROM v2_server_vmess
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'trojan' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('network', network, 'network_settings', network_settings, 'allow_insecure', allow_insecure, 'server_name', server_name) AS extra
FROM v2_server_trojan
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'tuic' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('server_name', server_name, 'insecure', insecure, 'disable_sni', disable_sni, 'udp_relay_mode', udp_relay_mode, 'zero_rtt_handshake', zero_rtt_handshake, 'congestion_control', congestion_control) AS extra
FROM v2_server_tuic
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'hysteria' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('version', version, 'up_mbps', up_mbps, 'down_mbps', down_mbps, 'obfs', obfs, 'obfs_password', obfs_password, 'server_name', server_name, 'insecure', insecure) AS extra
FROM v2_server_hysteria
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'vless' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('tls', tls, 'tls_settings', tls_settings, 'flow', flow, 'network', network, 'network_settings', network_settings, 'encryption', encryption, 'encryption_settings', encryption_settings) AS extra
FROM v2_server_vless
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'anytls' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT('server_name', server_name, 'insecure', insecure, 'padding_scheme', padding_scheme) AS extra
FROM v2_server_anytls
WHERE `show` = 1
UNION ALL
SELECT
    id, parent_id, group_id, route_id, name, rate, 'v2node' AS `type`,
    host, CAST(port AS CHAR) AS port, tags, sort, updated_at,
    JSON_OBJECT(
        'protocol', protocol, 'tls', tls, 'tls_settings', tls_settings, 'flow', flow,
        'network', network, 'network_settings', network_settings, 'encryption', encryption,
        'encryption_settings', encryption_settings, 'disable_sni', disable_sni,
        'udp_relay_mode', udp_relay_mode, 'zero_rtt_handshake', zero_rtt_handshake,
        'congestion_control', congestion_control, 'cipher', cipher, 'up_mbps', up_mbps,
        'down_mbps', down_mbps, 'obfs', obfs, 'obfs_password', obfs_password,
        'padding_scheme', padding_scheme
    ) AS extra
FROM v2_server_v2node
WHERE `show` = 1
"#;
