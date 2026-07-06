use std::fs;

use chrono::{Local, TimeZone};
use serde_json::{Map, Value, json};
use v2board_compat::ApiError;
use v2board_config::AppConfig;

use super::codec::{percent_encode, prefix_bytes, safe_base64_encode, standard_base64_encode};
use super::json_value::{value_to_i64, value_to_string};

pub(super) struct SubscriptionDocument {
    pub(super) body: String,
    pub(super) content_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriptionFormat {
    General,
    Base64Uri,
    Clash,
    ClashMeta,
    SingBox,
    SingBoxLegacy,
    Surge,
    Surfboard,
    Loon,
    Shadowsocks,
    Shadowrocket,
    SagerNet,
    QuantumultX,
}

impl SubscriptionFormat {
    fn detect(flag: &str) -> Self {
        let normalized = flag
            .replace("%20", " ")
            .replace(['_', '-', '/'], " ")
            .to_lowercase();
        if normalized.contains("sing") {
            if singbox_modern_flag(&normalized) {
                Self::SingBox
            } else {
                Self::SingBoxLegacy
            }
        } else if normalized.contains("surfboard") {
            Self::Surfboard
        } else if normalized.contains("surge") {
            Self::Surge
        } else if normalized.contains("loon") {
            Self::Loon
        } else if normalized.contains("shadowrocket") {
            Self::Shadowrocket
        } else if normalized.contains("shadowsocks") {
            Self::Shadowsocks
        } else if normalized.contains("sagernet") {
            Self::SagerNet
        } else if normalized.contains("quantumult") {
            Self::QuantumultX
        } else if normalized.contains("v2rayn")
            || normalized.contains("v2rayng")
            || normalized.contains("v2raytun")
            || normalized.contains("passwall")
            || normalized.contains("ssrplus")
        {
            Self::Base64Uri
        } else if normalized.contains("meta")
            || normalized.contains("mihomo")
            || normalized.contains("stash")
            || normalized.contains("nyanpasu")
            || normalized.contains("verge")
        {
            Self::ClashMeta
        } else if normalized.contains("clash") {
            Self::Clash
        } else {
            Self::General
        }
    }
}

fn singbox_modern_flag(normalized_flag: &str) -> bool {
    let marker = ["sing-box", "sing box", "singbox", "sing"]
        .into_iter()
        .find_map(|marker| {
            normalized_flag
                .find(marker)
                .map(|start| (start, marker.len()))
        });
    let Some((start, marker_len)) = marker else {
        return false;
    };
    let version_start = normalized_flag[start + marker_len..]
        .char_indices()
        .find_map(|(index, ch)| ch.is_ascii_digit().then_some(index));
    let Some(version_start) = version_start else {
        return false;
    };
    let rest = &normalized_flag[start + marker_len + version_start..];
    let version = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();
    version_at_least(&version, &[1, 12, 0])
}

fn version_at_least(version: &str, minimum: &[u64]) -> bool {
    let parts = version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or_default())
        .collect::<Vec<_>>();
    for (index, min) in minimum.iter().enumerate() {
        let value = parts.get(index).copied().unwrap_or_default();
        if value > *min {
            return true;
        }
        if value < *min {
            return false;
        }
    }
    true
}

pub(super) fn build_subscription_document(
    config: &AppConfig,
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
    flag: &str,
) -> Result<SubscriptionDocument, ApiError> {
    let format = SubscriptionFormat::detect(flag);
    let body = match format {
        SubscriptionFormat::General => build_general_subscription(&user.uuid, servers),
        SubscriptionFormat::Base64Uri => build_base64_uri_subscription(&user.uuid, servers),
        SubscriptionFormat::Clash => build_clash_subscription(config, &user.uuid, servers, false),
        SubscriptionFormat::ClashMeta => {
            build_clash_subscription(config, &user.uuid, servers, true)
        }
        SubscriptionFormat::SingBox => {
            build_singbox_subscription(config, &user.uuid, servers, true)?
        }
        SubscriptionFormat::SingBoxLegacy => {
            build_singbox_subscription(config, &user.uuid, servers, false)?
        }
        SubscriptionFormat::Surge => build_surge_subscription(config, user, servers),
        SubscriptionFormat::Surfboard => build_surfboard_subscription(config, user, servers),
        SubscriptionFormat::Loon => build_loon_subscription(&user.uuid, servers),
        SubscriptionFormat::Shadowsocks => build_shadowsocks_sip008_subscription(user, servers)?,
        SubscriptionFormat::Shadowrocket => build_shadowrocket_subscription(user, servers),
        SubscriptionFormat::SagerNet => build_sagernet_subscription(&user.uuid, servers),
        SubscriptionFormat::QuantumultX => build_quantumultx_subscription(&user.uuid, servers),
    };
    let content_type = match format {
        SubscriptionFormat::Clash | SubscriptionFormat::ClashMeta => {
            "application/yaml; charset=utf-8"
        }
        SubscriptionFormat::SingBox
        | SubscriptionFormat::SingBoxLegacy
        | SubscriptionFormat::Shadowsocks => "application/json; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    };
    Ok(SubscriptionDocument { body, content_type })
}

fn build_clash_subscription(
    config: &AppConfig,
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
    meta: bool,
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_clash_proxy(uuid, server, meta))
        .collect::<Vec<_>>();
    let proxy_names = proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    let selector_name = config.app_name.clone();
    let mut config = json!({
        "mixed-port": 7890,
        "allow-lan": true,
        "bind-address": "*",
        "mode": "rule",
        "log-level": "info",
        "external-controller": "127.0.0.1:9090",
        "dns": {
            "enable": true,
            "ipv6": false,
            "enhanced-mode": "fake-ip",
            "fake-ip-range": "198.18.0.1/16",
            "default-nameserver": ["223.5.5.5", "119.29.29.29", "114.114.114.114"],
            "nameserver": ["223.5.5.5", "119.29.29.29", "114.114.114.114"],
            "fallback": ["1.1.1.1", "8.8.8.8"]
        },
        "proxies": proxies,
        "proxy-groups": [
            {
                "name": selector_name,
                "type": "select",
                "proxies": ["自动选择", "故障转移"]
            },
            {
                "name": "自动选择",
                "type": "url-test",
                "proxies": [],
                "url": "http://www.gstatic.com/generate_204",
                "interval": 86400
            },
            {
                "name": "故障转移",
                "type": "fallback",
                "proxies": [],
                "url": "http://www.gstatic.com/generate_204",
                "interval": 7200
            }
        ],
        "rules": [
            format!("MATCH,{}", config.app_name)
        ]
    });

    if let Some(groups) = config.get_mut("proxy-groups").and_then(Value::as_array_mut) {
        for group in groups.iter_mut() {
            if let Some(values) = group.get_mut("proxies").and_then(Value::as_array_mut) {
                values.extend(proxy_names.iter().cloned().map(Value::String));
            }
        }
        groups.retain(|group| {
            group
                .get("proxies")
                .and_then(Value::as_array)
                .map(|proxies| !proxies.is_empty())
                .unwrap_or(false)
        });
    }

    render_yaml(&config)
}

fn build_singbox_subscription(
    config: &AppConfig,
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
    modern: bool,
) -> Result<String, ApiError> {
    let proxies = servers
        .iter()
        .filter(|server| modern || server_protocol(server) != "anytls")
        .filter_map(|server| build_singbox_proxy(uuid, server))
        .collect::<Vec<_>>();
    let proxy_tags = proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .get("tag")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    let mut template = load_singbox_template(config, modern);
    inject_singbox_proxies(&mut template, &proxy_tags, proxies);
    serde_json::to_string(&template)
        .map_err(|_| ApiError::internal("failed to render sing-box subscription"))
}

fn load_singbox_template(config: &AppConfig, modern: bool) -> Value {
    let candidates = if modern {
        ["custom.sing-box.json", "default.sing-box.json"]
    } else {
        ["custom.sing-box.old.json", "default.sing-box.old.json"]
    };
    for filename in candidates {
        let path = config.runtime_paths.rules.join(filename);
        if let Ok(body) = fs::read_to_string(path)
            && let Ok(value) = serde_json::from_str::<Value>(&body)
        {
            return value;
        }
    }
    fallback_singbox_template(modern)
}

fn inject_singbox_proxies(config: &mut Value, proxy_tags: &[String], proxies: Vec<Value>) {
    if !config.get("outbounds").is_some_and(Value::is_array) {
        config["outbounds"] = json!([]);
    }
    let Some(outbounds) = config.get_mut("outbounds").and_then(Value::as_array_mut) else {
        return;
    };
    for outbound in outbounds.iter_mut() {
        let outbound_type = outbound.get("type").and_then(Value::as_str);
        let tag = outbound
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let should_attach = (outbound_type == Some("selector") && tag == "节点选择")
            || (outbound_type == Some("urltest") && tag == "自动选择")
            || (outbound_type == Some("selector") && tag.starts_with('#'));
        if !should_attach {
            continue;
        }
        if !outbound.get("outbounds").is_some_and(Value::is_array) {
            outbound["outbounds"] = json!([]);
        }
        if let Some(items) = outbound.get_mut("outbounds").and_then(Value::as_array_mut) {
            for tag in proxy_tags {
                if !items.iter().any(|item| item.as_str() == Some(tag.as_str())) {
                    items.push(json!(tag));
                }
            }
        }
    }
    outbounds.extend(proxies);
}

fn fallback_singbox_template(modern: bool) -> Value {
    if modern {
        json!({
            "dns": {
                "servers": [
                    { "type": "local", "tag": "local" },
                    { "type": "udp", "tag": "remote", "server": "1.1.1.1" },
                    { "type": "udp", "tag": "cn", "server": "223.5.5.5" }
                ],
                "final": "remote"
            },
            "inbounds": [
                {
                    "tag": "tun-in",
                    "type": "tun",
                    "address": ["172.19.0.1/30", "2001:0470:f9da:fdfa::1/64"],
                    "auto_route": true,
                    "mtu": 9000,
                    "stack": "system",
                    "strict_route": true,
                    "route_exclude_address_set": ["geoip-cn"]
                },
                {
                    "tag": "mixed-in",
                    "type": "mixed",
                    "listen": "127.0.0.1",
                    "listen_port": 2334,
                    "users": []
                }
            ],
            "outbounds": [
                { "tag": "DIRECT", "type": "direct", "domain_resolver": { "server": "local" } },
                { "tag": "节点选择", "type": "selector", "interrupt_exist_connections": true, "outbounds": ["自动选择"] },
                { "tag": "自动选择", "type": "urltest", "url": "https://www.gstatic.com/generate_204", "interval": "10m", "tolerance": 50, "idle_timeout": "30m", "interrupt_exist_connections": false, "outbounds": [] }
            ],
            "route": {
                "rules": [
                    { "action": "sniff" },
                    { "protocol": "dns", "action": "hijack-dns" },
                    { "ip_is_private": true, "action": "route", "outbound": "DIRECT" },
                    { "rule_set": ["geosite-cn", "geoip-cn"], "action": "route", "outbound": "DIRECT" }
                ],
                "auto_detect_interface": true,
                "final": "节点选择",
                "default_domain_resolver": { "server": "remote" },
                "rule_set": [
                    { "tag": "geoip-cn", "type": "remote", "format": "binary", "url": "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/srs/cn.srs", "download_detour": "节点选择" },
                    { "tag": "geosite-cn", "type": "remote", "format": "binary", "url": "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-cn.srs", "download_detour": "节点选择" }
                ]
            },
            "experimental": {
                "cache_file": { "enabled": true },
                "clash_api": { "default_mode": "海外代理", "external_controller": "127.0.0.1:9090", "secret": "" }
            }
        })
    } else {
        json!({
            "dns": {
                "rules": [
                    { "outbound": ["any"], "server": "local" },
                    { "clash_mode": "全局代理", "server": "remote" },
                    { "clash_mode": "关闭代理", "server": "local" },
                    { "rule_set": ["geosite-cn"], "server": "local" },
                    { "rule_set": ["category-ads-all"], "server": "block" }
                ],
                "servers": [
                    { "address": "1.1.1.1", "detour": "节点选择", "tag": "remote" },
                    { "address": "https://223.5.5.5/dns-query", "detour": "direct", "tag": "local" },
                    { "address": "rcode://refused", "tag": "block" }
                ],
                "final": "remote",
                "strategy": "ipv4_only",
                "disable_cache": false
            },
            "experimental": {
                "cache_file": { "enabled": true },
                "clash_api": { "default_mode": "海外代理", "external_controller": "127.0.0.1:9090", "secret": "" }
            },
            "inbounds": [
                {
                    "auto_route": true,
                    "domain_strategy": "prefer_ipv4",
                    "endpoint_independent_nat": true,
                    "address": ["172.19.0.1/30", "2001:0470:f9da:fdfa::1/64"],
                    "mtu": 9000,
                    "sniff_override_destination": true,
                    "stack": "system",
                    "strict_route": true,
                    "type": "tun"
                },
                {
                    "domain_strategy": "prefer_ipv4",
                    "listen": "127.0.0.1",
                    "listen_port": 2334,
                    "sniff": true,
                    "sniff_override_destination": true,
                    "tag": "mixed-in",
                    "type": "mixed",
                    "users": []
                }
            ],
            "outbounds": [
                { "type": "selector", "tag": "节点选择", "outbounds": ["自动选择"] },
                { "type": "urltest", "tag": "自动选择", "outbounds": [] },
                { "type": "direct", "tag": "direct" }
            ],
            "route": {
                "auto_detect_interface": true,
                "rules": [
                    { "action": "sniff" },
                    { "protocol": "dns", "action": "hijack-dns" },
                    { "clash_mode": "关闭代理", "outbound": "direct" },
                    { "clash_mode": "全局代理", "outbound": "节点选择" },
                    { "rule_set": ["geosite-cn", "geoip-cn"], "outbound": "direct" },
                    { "ip_is_private": true, "outbound": "direct" },
                    { "rule_set": ["category-ads-all"], "action": "reject" }
                ],
                "rule_set": [
                    { "tag": "geosite-cn", "type": "remote", "format": "binary", "url": "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-cn.srs", "download_detour": "节点选择" },
                    { "tag": "category-ads-all", "type": "remote", "format": "binary", "url": "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-category-ads-all.srs", "download_detour": "节点选择" },
                    { "tag": "geoip-cn", "type": "remote", "format": "binary", "url": "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/srs/cn.srs", "download_detour": "节点选择" }
                ]
            }
        })
    }
}

fn build_surge_subscription(
    config: &AppConfig,
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_surge_proxy(&user.uuid, server))
        .collect::<Vec<_>>();
    let proxy_names = servers
        .iter()
        .filter(|server| supports_surge(server))
        .map(|server| server.name.clone())
        .collect::<Vec<_>>();
    let proxy_group = proxy_names.join(", ");
    let upload = bytes_to_gib(user.u);
    let download = bytes_to_gib(user.d);
    let total = bytes_to_gib(user.transfer_enable);
    let expire = user
        .expired_at
        .map(|expired_at| expired_at.to_string())
        .unwrap_or_else(|| "长期有效".to_string());
    format!(
        r#"#!MANAGED-CONFIG {subscribe_url} interval=43200 strict=true
[General]
loglevel = notify
dns-server = 223.5.5.5, 114.114.114.114
allow-wifi-access = true
http-listen = 0.0.0.0:6152
socks5-listen = 0.0.0.0:6153
proxy-test-url = http://www.gstatic.com/generate_204

[Panel]
SubscribeInfo = title={app_name}订阅信息, content=上传流量：{upload:.2}GB\n下载流量：{download:.2}GB\n套餐流量：{total:.2}GB\n到期时间：{expire}, style=info

[Proxy]
{proxies}

[Proxy Group]
Proxy = select, auto, fallback, {proxy_group}
auto = url-test, {proxy_group}, url=http://www.gstatic.com/generate_204, interval=43200
fallback = fallback, {proxy_group}, url=http://www.gstatic.com/generate_204, interval=43200

[Rule]
FINAL,Proxy
"#,
        subscribe_url = config.subscribe_url_for_token(&user.token),
        app_name = config.app_name,
        upload = upload,
        download = download,
        total = total,
        expire = expire,
        proxies = proxies.join(""),
        proxy_group = proxy_group
    )
}

fn build_quantumultx_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let lines = servers
        .iter()
        .filter(|server| {
            !matches!(
                extra_string(server, "network").as_deref(),
                Some("grpc" | "httpupgrade" | "xhttp")
            )
        })
        .filter_map(|server| build_quantumultx_proxy(uuid, server))
        .collect::<String>();
    standard_base64_encode(lines.as_bytes())
}

fn build_surfboard_subscription(
    config: &AppConfig,
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_surfboard_proxy(&user.uuid, server))
        .collect::<Vec<_>>();
    let proxy_names = servers
        .iter()
        .filter(|server| supports_surfboard(server))
        .map(|server| server.name.clone())
        .collect::<Vec<_>>();
    let proxy_group = proxy_names.join(", ");
    let upload = bytes_to_gib(user.u);
    let download = bytes_to_gib(user.d);
    let total = bytes_to_gib(user.transfer_enable);
    let expire = user
        .expired_at
        .map(|expired_at| expired_at.to_string())
        .unwrap_or_else(|| "长期有效".to_string());
    format!(
        r#"#!MANAGED-CONFIG {subscribe_url} interval=43200 strict=true
[General]
loglevel = notify
dns-server = 223.5.5.5, 114.114.114.114
proxy-test-url = http://www.gstatic.com/generate_204

[Panel]
SubscribeInfo = title={app_name}订阅信息, content=上传流量：{upload:.2}GB\n下载流量：{download:.2}GB\n套餐流量：{total:.2}GB\n到期时间：{expire}, style=info

[Proxy]
{proxies}

[Proxy Group]
Proxy = select, auto, fallback, {proxy_group}
auto = url-test, {proxy_group}, url=http://www.gstatic.com/generate_204, interval=43200
fallback = fallback, {proxy_group}, url=http://www.gstatic.com/generate_204, interval=43200

[Rule]
FINAL,Proxy
"#,
        subscribe_url = config.subscribe_url_for_token(&user.token),
        app_name = config.app_name,
        upload = upload,
        download = download,
        total = total,
        expire = expire,
        proxies = proxies.join(""),
        proxy_group = proxy_group
    )
}

fn build_loon_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    servers
        .iter()
        .filter_map(|server| build_loon_proxy(uuid, server))
        .collect::<String>()
}

fn build_shadowsocks_sip008_subscription(
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
) -> Result<String, ApiError> {
    let configs = servers
        .iter()
        .filter(|server| server_protocol(server) == "shadowsocks")
        .filter(|server| {
            extra_string(server, "cipher")
                .as_deref()
                .map(is_basic_shadowsocks_cipher)
                .unwrap_or(false)
        })
        .filter_map(|server| {
            Some(json!({
                "id": server.id,
                "remarks": server.name,
                "server": server.host,
                "server_port": port_value(server),
                "password": shadowsocks_password(&user.uuid, server)?,
                "method": extra_string(server, "cipher")?,
            }))
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "version": 1,
        "bytes_used": user.u + user.d,
        "bytes_remaining": user.transfer_enable - user.u - user.d,
        "servers": configs,
    }))
    .map_err(|_| ApiError::internal("failed to render shadowsocks subscription"))
}

fn build_shadowrocket_subscription(
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let upload = bytes_to_gib(user.u);
    let download = bytes_to_gib(user.d);
    let total = bytes_to_gib(user.transfer_enable);
    let expire = user
        .expired_at
        .map(format_date_timestamp)
        .unwrap_or_else(|| "长期有效".to_string());
    let mut lines =
        format!("STATUS=↑:{upload:.2}GB,↓:{download:.2}GB,TOT:{total:.2}GB Expires:{expire}\r\n");
    for server in servers {
        if server_protocol(server) == "vmess" {
            if let Some(uri) = build_shadowrocket_vmess_uri(&user.uuid, server) {
                lines.push_str(&uri);
            }
        } else if let Some(uri) = build_server_uri(&user.uuid, server) {
            lines.push_str(&uri);
        }
    }
    standard_base64_encode(lines.as_bytes())
}

fn build_sagernet_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let mut uris = String::new();
    for server in servers {
        if server_protocol(server) == "hysteria" {
            continue;
        }
        if let Some(uri) = build_server_uri(uuid, server) {
            uris.push_str(&uri);
        }
    }
    standard_base64_encode(uris.as_bytes())
}

fn build_base64_uri_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    build_general_subscription(uuid, servers)
}

fn build_general_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let mut uris = String::new();
    for server in servers {
        if let Some(uri) = build_server_uri(uuid, server) {
            uris.push_str(&uri);
        }
    }
    standard_base64_encode(uris.as_bytes())
}

fn build_server_uri(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => build_shadowsocks_uri(uuid, server),
        "vmess" => build_vmess_uri(uuid, server),
        "vless" => build_vless_uri(uuid, server),
        "trojan" => build_trojan_uri(uuid, server),
        "hysteria" => build_hysteria_uri(uuid, server),
        "hysteria2" => build_hysteria2_uri(uuid, server),
        "tuic" => build_tuic_uri(uuid, server),
        "anytls" => build_anytls_uri(uuid, server),
        _ => None,
    }
}

fn build_clash_proxy(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
    meta: bool,
) -> Option<Value> {
    match server_protocol(server).as_str() {
        "shadowsocks" => build_clash_shadowsocks(uuid, server),
        "vmess" => build_clash_vmess(uuid, server),
        "vless" if meta => build_clash_vless(uuid, server),
        "trojan" => build_clash_trojan(uuid, server),
        "tuic" if meta => build_clash_tuic(uuid, server),
        "anytls" if meta => build_clash_anytls(uuid, server),
        "hysteria" if meta => build_clash_hysteria(uuid, server),
        "hysteria2" if meta => build_clash_hysteria2(uuid, server),
        _ => None,
    }
}

fn build_clash_shadowsocks(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let cipher = extra_string(server, "cipher")?;
    let mut object = proxy_base(server, "ss");
    object.insert("cipher".to_string(), Value::String(cipher));
    object.insert(
        "password".to_string(),
        Value::String(shadowsocks_password(uuid, server)?),
    );
    object.insert("udp".to_string(), Value::Bool(true));
    if extra_string(server, "obfs").as_deref() == Some("http") {
        object.insert("plugin".to_string(), Value::String("obfs".to_string()));
        let settings = extra_json(server, "obfs_settings");
        let mut opts = Map::new();
        opts.insert("mode".to_string(), Value::String("http".to_string()));
        insert_opt_string(&mut opts, "host", json_path_string(&settings, &["host"]));
        insert_opt_string(&mut opts, "path", json_path_string(&settings, &["path"]));
        object.insert("plugin-opts".to_string(), Value::Object(opts));
    } else if extra_string(server, "network").as_deref() == Some("http") {
        let settings = extra_json(server, "network_settings");
        let mut opts = Map::new();
        opts.insert("mode".to_string(), Value::String("http".to_string()));
        insert_opt_string(
            &mut opts,
            "host",
            json_path_string(&settings, &["Host"])
                .or_else(|| json_path_string(&settings, &["headers", "Host"])),
        );
        insert_opt_string(&mut opts, "path", json_path_string(&settings, &["path"]));
        object.insert("plugin".to_string(), Value::String("obfs".to_string()));
        object.insert("plugin-opts".to_string(), Value::Object(opts));
    }
    Some(Value::Object(object))
}

fn build_clash_vmess(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "vmess");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("alterId".to_string(), Value::from(0));
    object.insert("cipher".to_string(), Value::String("auto".to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    if tls != 0 {
        object.insert("tls".to_string(), Value::Bool(true));
        object.insert(
            "skip-cert-verify".to_string(),
            Value::Bool(
                json_path_i64(&tls_settings, &["allow_insecure"])
                    .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                    .unwrap_or_default()
                    == 1,
            ),
        );
        insert_opt_string(
            &mut object,
            "servername",
            json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"])),
        );
    }
    add_clash_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    add_clash_ech(&mut object, &tls_settings);
    Some(Value::Object(object))
}

fn build_clash_vless(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "vless");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    insert_opt_string(&mut object, "flow", extra_string(server, "flow"));
    if tls != 0 {
        object.insert("tls".to_string(), Value::Bool(true));
        object.insert(
            "skip-cert-verify".to_string(),
            Value::Bool(json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() == 1),
        );
        object.insert(
            "client-fingerprint".to_string(),
            Value::String(
                json_path_string(&tls_settings, &["fingerprint"])
                    .unwrap_or_else(|| "chrome".to_string()),
            ),
        );
        insert_opt_string(
            &mut object,
            "servername",
            json_path_string(&tls_settings, &["server_name"]),
        );
        if tls == 2 {
            object.insert(
                "reality-opts".to_string(),
                json!({
                    "public-key": json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
                    "short-id": json_path_string(&tls_settings, &["short_id"]).unwrap_or_default(),
                }),
            );
        }
    }
    add_clash_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    add_clash_ech(&mut object, &tls_settings);
    Some(Value::Object(object))
}

fn build_clash_trojan(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "trojan");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "allow_insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    add_clash_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    add_clash_ech(&mut object, &tls_settings);
    Some(Value::Object(object))
}

fn build_clash_tuic(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "tuic");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert("alpn".to_string(), json!(["h3"]));
    object.insert(
        "disable-sni".to_string(),
        Value::Bool(extra_i64(server, "disable_sni").unwrap_or_default() == 1),
    );
    object.insert(
        "reduce-rtt".to_string(),
        Value::Bool(extra_i64(server, "zero_rtt_handshake").unwrap_or_default() == 1),
    );
    insert_opt_string(
        &mut object,
        "udp-relay-mode",
        extra_string(server, "udp_relay_mode"),
    );
    insert_opt_string(
        &mut object,
        "congestion-controller",
        extra_string(server, "congestion_control"),
    );
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    Some(Value::Object(object))
}

fn build_clash_anytls(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "anytls");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "client-fingerprint".to_string(),
        Value::String("chrome".to_string()),
    );
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert("alpn".to_string(), json!(["h2", "http/1.1"]));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    Some(Value::Object(object))
}

fn build_clash_hysteria(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    if extra_i64(server, "version") == Some(2) {
        return build_clash_hysteria2(uuid, server);
    }
    let mut object = proxy_base(server, "hysteria");
    object.insert("auth_str".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert("protocol".to_string(), Value::String("udp".to_string()));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(extra_i64(server, "insecure").unwrap_or_default() == 1),
    );
    insert_opt_string(&mut object, "sni", extra_string(server, "server_name"));
    object.insert(
        "up".to_string(),
        Value::from(extra_i64(server, "down_mbps").unwrap_or_default()),
    );
    object.insert(
        "down".to_string(),
        Value::from(extra_i64(server, "up_mbps").unwrap_or_default()),
    );
    if let Some(obfs_password) = extra_string(server, "obfs_password") {
        object.insert("obfs".to_string(), Value::String(obfs_password));
    }
    add_multi_port_fields(&mut object, server);
    Some(Value::Object(object))
}

fn build_clash_hysteria2(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "hysteria2");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    if let Some(obfs) = extra_string(server, "obfs") {
        object.insert("obfs".to_string(), Value::String(obfs));
        insert_opt_string(
            &mut object,
            "obfs-password",
            extra_string(server, "obfs_password"),
        );
    }
    add_multi_port_fields(&mut object, server);
    Some(Value::Object(object))
}

fn build_singbox_proxy(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    match server_protocol(server).as_str() {
        "shadowsocks" => build_singbox_shadowsocks(uuid, server),
        "vmess" => build_singbox_vmess(uuid, server),
        "vless" => build_singbox_vless(uuid, server),
        "trojan" => build_singbox_trojan(uuid, server),
        "tuic" => build_singbox_tuic(uuid, server),
        "anytls" => build_singbox_anytls(uuid, server),
        "hysteria" => build_singbox_hysteria(uuid, server),
        "hysteria2" => build_singbox_hysteria2(uuid, server),
        _ => None,
    }
}

fn build_singbox_shadowsocks(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let cipher = extra_string(server, "cipher")?;
    let mut object = singbox_base(server, "shadowsocks");
    object.insert("method".to_string(), Value::String(cipher));
    object.insert(
        "password".to_string(),
        Value::String(shadowsocks_password(uuid, server)?),
    );
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    if extra_string(server, "obfs").as_deref() == Some("http") {
        let settings = extra_json(server, "obfs_settings");
        object.insert(
            "plugin".to_string(),
            Value::String("obfs-local".to_string()),
        );
        object.insert(
            "plugin_opts".to_string(),
            Value::String(obfs_plugin_opts(
                "http",
                json_path_string(&settings, &["host"]),
                json_path_string(&settings, &["path"]),
            )),
        );
    }
    Some(Value::Object(object))
}

fn build_singbox_vmess(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "vmess");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("security".to_string(), Value::String("auto".to_string()));
    object.insert("alter_id".to_string(), Value::from(0));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    if tls != 0 {
        object.insert(
            "tls".to_string(),
            singbox_tls(server, &tls_settings, tls, false),
        );
    }
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_vless(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "vless");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    object.insert(
        "packet_encoding".to_string(),
        Value::String("xudp".to_string()),
    );
    insert_opt_string(&mut object, "flow", extra_string(server, "flow"));
    if tls != 0 {
        object.insert(
            "tls".to_string(),
            singbox_tls(server, &tls_settings, tls, true),
        );
    }
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_trojan(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "trojan");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    object.insert(
        "tls".to_string(),
        singbox_tls(server, &tls_settings, 1, false),
    );
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_tuic(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "tuic");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    insert_opt_string(
        &mut object,
        "congestion_control",
        extra_string(server, "congestion_control").or_else(|| Some("cubic".to_string())),
    );
    insert_opt_string(
        &mut object,
        "udp_relay_mode",
        extra_string(server, "udp_relay_mode").or_else(|| Some("native".to_string())),
    );
    object.insert(
        "zero_rtt_handshake".to_string(),
        Value::Bool(extra_i64(server, "zero_rtt_handshake").unwrap_or_default() == 1),
    );
    object.insert(
        "tls".to_string(),
        singbox_tls(server, &tls_settings, 1, false),
    );
    Some(Value::Object(object))
}

fn build_singbox_anytls(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "anytls");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    object.insert(
        "tls".to_string(),
        singbox_tls(
            server,
            &tls_settings,
            extra_i64(server, "tls").unwrap_or(1),
            true,
        ),
    );
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_hysteria(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    if extra_i64(server, "version") == Some(2) {
        return build_singbox_hysteria2(uuid, server);
    }
    let mut object = singbox_base(server, "hysteria");
    object.insert("auth_str".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    object.insert(
        "up_mbps".to_string(),
        Value::from(extra_i64(server, "down_mbps").unwrap_or_default()),
    );
    object.insert(
        "down_mbps".to_string(),
        Value::from(extra_i64(server, "up_mbps").unwrap_or_default()),
    );
    object.insert(
        "tls".to_string(),
        json!({
            "enabled": true,
            "insecure": extra_i64(server, "insecure").unwrap_or_default() == 1,
            "server_name": extra_string(server, "server_name").unwrap_or_default()
        }),
    );
    if let Some(obfs_password) = extra_string(server, "obfs_password") {
        object.insert("obfs".to_string(), Value::String(obfs_password));
    }
    add_singbox_multi_port_fields(&mut object, server);
    Some(Value::Object(object))
}

fn build_singbox_hysteria2(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "hysteria2");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    object.insert(
        "tls".to_string(),
        singbox_tls(server, &tls_settings, 1, false),
    );
    if let Some(obfs) = extra_string(server, "obfs") {
        object.insert(
            "obfs".to_string(),
            json!({
                "type": obfs,
                "password": extra_string(server, "obfs_password").unwrap_or_default()
            }),
        );
    }
    add_singbox_multi_port_fields(&mut object, server);
    Some(Value::Object(object))
}

fn build_surge_proxy(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => Some(format!(
            "{}=ss,{},{},encrypt-method={},password={},fast-open=false,udp=true\r\n",
            server.name,
            server.host,
            first_port(server),
            extra_string(server, "cipher")?,
            shadowsocks_password(uuid, server)?
        )),
        "vmess" => {
            let mut parts = vec![
                format!("{}=vmess", server.name),
                server.host.clone(),
                first_port(server),
                format!("username={uuid}"),
                "vmess-aead=true".to_string(),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            if extra_i64(server, "tls").unwrap_or_default() != 0 {
                parts.push("tls=true".to_string());
                let tls_settings = extra_json(server, "tls_settings");
                if json_path_i64(&tls_settings, &["allow_insecure"])
                    .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                    .unwrap_or_default()
                    == 1
                {
                    parts.push("skip-cert-verify=true".to_string());
                }
                if let Some(sni) = json_path_string(&tls_settings, &["server_name"])
                    .or_else(|| json_path_string(&tls_settings, &["serverName"]))
                {
                    parts.push(format!("sni={sni}"));
                }
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "trojan" => {
            let tls_settings = extra_json(server, "tls_settings");
            let mut parts = vec![
                format!("{}=trojan", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            if let Some(sni) = extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
            {
                parts.push(format!("sni={sni}"));
            }
            if extra_i64(server, "allow_insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1
            {
                parts.push("skip-cert-verify=true".to_string());
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "hysteria" | "hysteria2" if extra_i64(server, "version") == Some(2) => {
            let mut parts = vec![
                format!("{}=hysteria2", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                format!(
                    "download-bandwidth={}",
                    extra_i64(server, "up_mbps").unwrap_or_default()
                ),
                "udp-relay=true".to_string(),
            ];
            if let Some(sni) = extra_string(server, "server_name") {
                parts.push(format!("sni={sni}"));
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "anytls" => Some(format!(
            "{}=anytls,{},{},password={},udp-relay=true\r\n",
            server.name,
            server.host,
            first_port(server),
            uuid
        )),
        _ => None,
    }
}

fn build_surfboard_proxy(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => Some(format!(
            "{}=ss,{},{},encrypt-method={},password={},tfo=true,udp-relay=true\r\n",
            server.name,
            server.host,
            first_port(server),
            extra_string(server, "cipher")?,
            shadowsocks_password(uuid, server)?
        )),
        "vmess" => {
            let mut parts = vec![
                format!("{}=vmess", server.name),
                server.host.clone(),
                first_port(server),
                format!("username={uuid}"),
                "vmess-aead=true".to_string(),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            append_surge_like_tls(server, &mut parts);
            append_surge_like_ws(server, &mut parts);
            Some(format!("{}\r\n", parts.join(",")))
        }
        "trojan" => {
            let mut parts = vec![
                format!("{}=trojan", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            append_sni_and_insecure(server, &mut parts, "sni");
            append_surge_like_ws(server, &mut parts);
            Some(format!("{}\r\n", parts.join(",")))
        }
        "anytls" => {
            let tls_settings = extra_json(server, "tls_settings");
            let insecure = extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1;
            let mut parts = vec![
                format!("{}=anytls", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                format!("skip-cert-verify={insecure}"),
                "reuse=false".to_string(),
            ];
            if let Some(sni) = extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
            {
                parts.push(format!("sni={sni}"));
            }
            Some(format!("{}\r\n", parts.join(", ")))
        }
        _ => None,
    }
}

fn build_loon_proxy(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => Some(format!(
            "{}=Shadowsocks,{},{},{},{},fast-open=false,udp=true\r\n",
            server.name,
            server.host,
            first_port(server),
            extra_string(server, "cipher")?,
            shadowsocks_password(uuid, server)?
        )),
        "vmess" => {
            let mut parts = vec![
                format!("{}=vmess", server.name),
                server.host.clone(),
                first_port(server),
                "auto".to_string(),
                uuid.to_string(),
                "fast-open=false".to_string(),
                "udp=true".to_string(),
                "alterId=0".to_string(),
            ];
            append_loon_transport_and_tls(server, &mut parts);
            Some(format!("{}\r\n", parts.join(",")))
        }
        "vless" => {
            let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
            if !matches!(network.as_str(), "tcp" | "ws") {
                return None;
            }
            let mut parts = vec![
                format!("{}=vless", server.name),
                server.host.clone(),
                first_port(server),
                uuid.to_string(),
                "fast-open=false".to_string(),
                "udp=true".to_string(),
                "alterId=0".to_string(),
            ];
            insert_opt_part(&mut parts, "flow", extra_string(server, "flow"));
            append_loon_transport_and_tls(server, &mut parts);
            Some(format!("{}\r\n", parts.join(",")))
        }
        "trojan" => {
            if extra_string(server, "network").as_deref() == Some("grpc") {
                return None;
            }
            let mut parts = vec![
                format!("{}=trojan", server.name),
                server.host.clone(),
                first_port(server),
                uuid.to_string(),
                "fast-open=false".to_string(),
                "udp=true".to_string(),
            ];
            append_sni_and_insecure(server, &mut parts, "tls-name");
            Some(format!("{}\r\n", parts.join(",")))
        }
        "hysteria" | "hysteria2" if extra_i64(server, "version") == Some(2) => {
            let mut parts = vec![
                format!("{}=hysteria2", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                format!(
                    "download-bandwidth={}",
                    extra_i64(server, "up_mbps").unwrap_or_default()
                ),
                "udp=true".to_string(),
            ];
            append_sni_and_insecure(server, &mut parts, "sni");
            if let Some(obfs_password) = extra_string(server, "obfs_password") {
                parts.push(format!("salamander-password={obfs_password}"));
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "anytls" => {
            let mut parts = vec![
                format!("{}=anytls", server.name),
                server.host.clone(),
                first_port(server),
                uuid.to_string(),
                "udp=true".to_string(),
            ];
            append_sni_and_insecure(server, &mut parts, "sni");
            Some(format!("{}\r\n", parts.join(",")))
        }
        _ => None,
    }
}

fn build_shadowrocket_vmess_uri(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let userinfo = standard_base64_encode(
        format!("auto:{uuid}@{}:{}", server.host, first_port(server)).as_bytes(),
    );
    let mut params = vec![
        ("tfo".to_string(), "1".to_string()),
        ("remark".to_string(), server.name.clone()),
        ("alterId".to_string(), "0".to_string()),
    ];
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    if tls != 0 {
        params.push(("tls".to_string(), "1".to_string()));
        params.push((
            "allowInsecure".to_string(),
            json_path_i64(&tls_settings, &["allow_insecure"])
                .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                .unwrap_or_default()
                .to_string(),
        ));
        insert_query_param(
            &mut params,
            "peer",
            json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"])),
        );
    }
    match extra_string(server, "network").as_deref() {
        Some("tcp") => {
            let settings = extra_json(server, "network_settings");
            insert_query_param(
                &mut params,
                "obfs",
                json_path_string(&settings, &["header", "type"]),
            );
            insert_query_param(
                &mut params,
                "path",
                json_path_string(&settings, &["header", "request", "path"]),
            );
            insert_query_param(
                &mut params,
                "obfsParam",
                json_path_string(&settings, &["header", "request", "headers", "Host"]),
            );
        }
        Some("ws") => {
            let settings = extra_json(server, "network_settings");
            params.push(("obfs".to_string(), "websocket".to_string()));
            insert_query_param(&mut params, "path", json_path_string(&settings, &["path"]));
            insert_query_param(
                &mut params,
                "obfsParam",
                json_path_string(&settings, &["headers", "Host"]),
            );
            insert_query_param(
                &mut params,
                "method",
                json_path_string(&settings, &["security"]),
            );
        }
        Some("grpc") => {
            let settings = extra_json(server, "network_settings");
            params.push(("obfs".to_string(), "grpc".to_string()));
            insert_query_param(
                &mut params,
                "path",
                json_path_string(&settings, &["serviceName"]),
            );
            params.push((
                "host".to_string(),
                json_path_string(&tls_settings, &["server_name"])
                    .unwrap_or_else(|| server.host.clone()),
            ));
        }
        _ => {}
    }
    Some(format!("vmess://{userinfo}?{}\r\n", query_string(&params)))
}

fn build_quantumultx_proxy(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => Some(format!(
            "shadowsocks={}:{},method={},password={},fast-open=false,udp-relay=true,tag={}\r\n",
            server.host,
            first_port(server),
            extra_string(server, "cipher")?,
            shadowsocks_password(uuid, server)?,
            server.name
        )),
        "vmess" => Some(format!(
            "vmess={}:{},method=chacha20-poly1305,password={},fast-open=true,udp-relay=true,tag={}\r\n",
            server.host,
            first_port(server),
            uuid,
            server.name
        )),
        "vless" => {
            if !extra_string(server, "encryption")
                .unwrap_or_default()
                .is_empty()
            {
                return None;
            }
            Some(format!(
                "vless={}:{},method=none,password={},udp-relay=true,fast-open=true,tag={}\r\n",
                server.host,
                first_port(server),
                uuid,
                server.name
            ))
        }
        "trojan" => Some(format!(
            "trojan={}:{},password={},fast-open=true,udp-relay=true,tag={}\r\n",
            server.host,
            first_port(server),
            uuid,
            server.name
        )),
        "anytls" => Some(build_quantumultx_anytls(uuid, server)),
        _ => None,
    }
}

fn build_quantumultx_anytls(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> String {
    let mut config = vec![
        format!("anytls={}:{}", server.host, first_port(server)),
        format!("password={uuid}"),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    if network == "tcp" {
        config.push("over-tls=true".to_string());
        let tls_settings = extra_json(server, "tls_settings");
        if let Some(sni) = json_path_string(&tls_settings, &["server_name"])
            .or_else(|| extra_string(server, "server_name"))
        {
            config.push(format!("tls-host={sni}"));
        }
        let allow_insecure = json_path_i64(&tls_settings, &["allow_insecure"])
            .or_else(|| extra_i64(server, "allow_insecure"))
            .unwrap_or_default()
            != 0;
        config.push(format!(
            "tls-verification={}",
            if allow_insecure { "false" } else { "true" }
        ));
    }
    format!("{}\r\n", config.join(","))
}

fn supports_surge(server: &v2board_db::server::AvailableServerRow) -> bool {
    matches!(
        server_protocol(server).as_str(),
        "shadowsocks" | "vmess" | "trojan" | "anytls"
    ) || (matches!(server_protocol(server).as_str(), "hysteria" | "hysteria2")
        && extra_i64(server, "version") == Some(2))
}

fn supports_surfboard(server: &v2board_db::server::AvailableServerRow) -> bool {
    matches!(
        server_protocol(server).as_str(),
        "shadowsocks" | "vmess" | "trojan" | "anytls"
    )
}

fn append_surge_like_tls(server: &v2board_db::server::AvailableServerRow, parts: &mut Vec<String>) {
    if extra_i64(server, "tls").unwrap_or_default() == 0 {
        return;
    }
    parts.push("tls=true".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    if json_path_i64(&tls_settings, &["allow_insecure"])
        .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        == 1
    {
        parts.push("skip-cert-verify=true".to_string());
    }
    if let Some(sni) = json_path_string(&tls_settings, &["server_name"])
        .or_else(|| json_path_string(&tls_settings, &["serverName"]))
    {
        parts.push(format!("sni={sni}"));
    }
}

fn append_surge_like_ws(server: &v2board_db::server::AvailableServerRow, parts: &mut Vec<String>) {
    if extra_string(server, "network").as_deref() != Some("ws") {
        return;
    }
    let settings = extra_json(server, "network_settings");
    parts.push("ws=true".to_string());
    insert_opt_part(parts, "ws-path", json_path_string(&settings, &["path"]));
    insert_opt_part(
        parts,
        "ws-headers",
        json_path_string(&settings, &["headers", "Host"]).map(|host| format!("Host:{host}")),
    );
    insert_opt_part(
        parts,
        "encrypt-method",
        json_path_string(&settings, &["security"]),
    );
}

fn append_sni_and_insecure(
    server: &v2board_db::server::AvailableServerRow,
    parts: &mut Vec<String>,
    sni_key: &str,
) {
    let tls_settings = extra_json(server, "tls_settings");
    if let Some(sni) = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
        .or_else(|| json_path_string(&tls_settings, &["serverName"]))
    {
        parts.push(format!("{sni_key}={sni}"));
    }
    if extra_i64(server, "allow_insecure")
        .or_else(|| extra_i64(server, "insecure"))
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        == 1
    {
        parts.push("skip-cert-verify=true".to_string());
    }
}

fn append_loon_transport_and_tls(
    server: &v2board_db::server::AvailableServerRow,
    parts: &mut Vec<String>,
) {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let settings = extra_json(server, "network_settings");
    match network.as_str() {
        "tcp" => {
            let transport = json_path_string(&settings, &["header", "type"])
                .filter(|value| value == "http")
                .unwrap_or_else(|| "tcp".to_string());
            parts.push(format!("transport={transport}"));
            insert_opt_part(
                parts,
                "path",
                json_path_string(&settings, &["header", "request", "path"]),
            );
            insert_opt_part(
                parts,
                "host",
                json_path_string(&settings, &["header", "request", "headers", "Host"]),
            );
        }
        "ws" => {
            parts.push("transport=ws".to_string());
            insert_opt_part(parts, "path", json_path_string(&settings, &["path"]));
            insert_opt_part(
                parts,
                "host",
                json_path_string(&settings, &["headers", "Host"]),
            );
        }
        _ => {}
    }
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    if tls == 1 {
        parts.push("over-tls=true".to_string());
        insert_opt_part(
            parts,
            "tls-name",
            json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"])),
        );
    } else if tls == 2 {
        insert_opt_part(
            parts,
            "public-key",
            json_path_string(&tls_settings, &["public_key"]),
        );
        insert_opt_part(
            parts,
            "short-id",
            json_path_string(&tls_settings, &["short_id"]),
        );
        insert_opt_part(
            parts,
            "sni",
            json_path_string(&tls_settings, &["server_name"]),
        );
    }
    if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() == 1 {
        parts.push("skip-cert-verify=true".to_string());
    }
}

fn insert_opt_part(parts: &mut Vec<String>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        parts.push(format!("{key}={value}"));
    }
}

fn insert_query_param(params: &mut Vec<(String, String)>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        params.push((key.to_string(), value));
    }
}

fn is_basic_shadowsocks_cipher(cipher: &str) -> bool {
    matches!(
        cipher,
        "aes-128-gcm" | "aes-192-gcm" | "aes-256-gcm" | "chacha20-ietf-poly1305"
    )
}

fn format_date_timestamp(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn proxy_base(
    server: &v2board_db::server::AvailableServerRow,
    proxy_type: &str,
) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("name".to_string(), Value::String(server.name.clone()));
    object.insert("type".to_string(), Value::String(proxy_type.to_string()));
    object.insert("server".to_string(), Value::String(server.host.clone()));
    object.insert("port".to_string(), port_value(server));
    object
}

fn singbox_base(
    server: &v2board_db::server::AvailableServerRow,
    proxy_type: &str,
) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("tag".to_string(), Value::String(server.name.clone()));
    object.insert("type".to_string(), Value::String(proxy_type.to_string()));
    object.insert("server".to_string(), Value::String(server.host.clone()));
    object.insert("server_port".to_string(), port_value(server));
    object
}

fn add_clash_transport(object: &mut Map<String, Value>, network: &str, settings: &Value) {
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                object.insert("network".to_string(), Value::String("http".to_string()));
                let mut opts = Map::new();
                if let Some(host) =
                    json_path_string(settings, &["header", "request", "headers", "Host"])
                {
                    let hosts = split_jsonish_list(&host);
                    opts.insert("headers".to_string(), json!({ "Host": hosts }));
                }
                insert_opt_value(
                    &mut opts,
                    "path",
                    json_path_value(settings, &["header", "request", "path"]).cloned(),
                );
                object.insert("http-opts".to_string(), Value::Object(opts));
            }
        }
        "ws" => {
            object.insert("network".to_string(), Value::String("ws".to_string()));
            let mut opts = Map::new();
            insert_opt_string(&mut opts, "path", json_path_string(settings, &["path"]));
            if let Some(host) = json_path_string(settings, &["headers", "Host"]) {
                opts.insert("headers".to_string(), json!({ "Host": host }));
            }
            object.insert("ws-opts".to_string(), Value::Object(opts));
        }
        "grpc" => {
            object.insert("network".to_string(), Value::String("grpc".to_string()));
            object.insert(
                "grpc-opts".to_string(),
                json!({ "grpc-service-name": json_path_string(settings, &["serviceName"]).unwrap_or_default() }),
            );
        }
        "xhttp" => {
            object.insert("network".to_string(), Value::String("xhttp".to_string()));
            let mut opts = Map::new();
            insert_opt_string(&mut opts, "path", json_path_string(settings, &["path"]));
            insert_opt_string(&mut opts, "host", json_path_string(settings, &["host"]));
            insert_opt_string(&mut opts, "mode", json_path_string(settings, &["mode"]));
            object.insert("xhttp-opts".to_string(), Value::Object(opts));
        }
        _ => {}
    }
}

fn add_singbox_transport(object: &mut Map<String, Value>, network: &str, settings: &Value) {
    let mut transport = Map::new();
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                transport.insert("type".to_string(), Value::String("http".to_string()));
                insert_opt_string(
                    &mut transport,
                    "host",
                    json_path_string(settings, &["header", "request", "headers", "Host"]),
                );
                insert_opt_string(
                    &mut transport,
                    "path",
                    json_path_string(settings, &["header", "request", "path"]),
                );
            }
        }
        "ws" => {
            transport.insert("type".to_string(), Value::String("ws".to_string()));
            transport.insert(
                "path".to_string(),
                Value::String(
                    json_path_string(settings, &["path"]).unwrap_or_else(|| "/".to_string()),
                ),
            );
            if let Some(host) = json_path_string(settings, &["headers", "Host"]) {
                transport.insert("headers".to_string(), json!({ "Host": [host] }));
            }
            transport.insert("max_early_data".to_string(), Value::from(2048));
            transport.insert(
                "early_data_header_name".to_string(),
                Value::String("Sec-WebSocket-Protocol".to_string()),
            );
        }
        "grpc" => {
            transport.insert("type".to_string(), Value::String("grpc".to_string()));
            insert_opt_string(
                &mut transport,
                "service_name",
                json_path_string(settings, &["serviceName"]),
            );
        }
        _ => {}
    }
    if !transport.is_empty() {
        object.insert("transport".to_string(), Value::Object(transport));
    }
}

fn singbox_tls(
    server: &v2board_db::server::AvailableServerRow,
    tls_settings: &Value,
    tls_mode: i64,
    utls: bool,
) -> Value {
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| extra_i64(server, "allow_insecure"))
                .or_else(|| json_path_i64(tls_settings, &["allow_insecure"]))
                .or_else(|| json_path_i64(tls_settings, &["allowInsecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    tls.insert(
        "server_name".to_string(),
        Value::String(
            extra_string(server, "server_name")
                .or_else(|| json_path_string(tls_settings, &["server_name"]))
                .or_else(|| json_path_string(tls_settings, &["serverName"]))
                .unwrap_or_default(),
        ),
    );
    if tls_mode == 2 {
        tls.insert(
            "reality".to_string(),
            json!({
                "enabled": true,
                "public_key": json_path_string(tls_settings, &["public_key"]).unwrap_or_default(),
                "short_id": json_path_string(tls_settings, &["short_id"]).unwrap_or_default()
            }),
        );
    }
    if utls {
        tls.insert(
            "utls".to_string(),
            json!({
                "enabled": true,
                "fingerprint": json_path_string(tls_settings, &["fingerprint"]).unwrap_or_else(|| "chrome".to_string())
            }),
        );
    }
    add_singbox_ech(&mut tls, tls_settings);
    Value::Object(tls)
}

fn add_clash_ech(object: &mut Map<String, Value>, tls_settings: &Value) {
    match json_path_string(tls_settings, &["ech"]).as_deref() {
        Some("cloudflare") => {
            object.insert(
                "ech-opts".to_string(),
                json!({ "enable": true, "query-server-name": "cloudflare-ech.com" }),
            );
        }
        Some("custom") => {
            if let Some(config) = json_path_string(tls_settings, &["ech_config"]) {
                object.insert(
                    "ech-opts".to_string(),
                    json!({ "enable": true, "config": [config] }),
                );
            }
        }
        _ => {}
    }
}

fn add_singbox_ech(object: &mut Map<String, Value>, tls_settings: &Value) {
    match json_path_string(tls_settings, &["ech"]).as_deref() {
        Some("cloudflare") => {
            object.insert(
                "ech".to_string(),
                json!({ "enabled": true, "query_server_name": "cloudflare-ech.com" }),
            );
        }
        Some("custom") => {
            if let Some(config) = json_path_string(tls_settings, &["ech_config"]) {
                object.insert(
                    "ech".to_string(),
                    json!({ "enabled": true, "config": [config] }),
                );
            }
        }
        _ => {}
    }
}

fn add_multi_port_fields(
    object: &mut Map<String, Value>,
    server: &v2board_db::server::AvailableServerRow,
) {
    if let Some(mport) = mport(server) {
        object.insert("ports".to_string(), Value::String(mport.clone()));
        object.insert("mport".to_string(), Value::String(mport));
    }
}

fn add_singbox_multi_port_fields(
    object: &mut Map<String, Value>,
    server: &v2board_db::server::AvailableServerRow,
) {
    let raw = port_text(server);
    if raw.contains('-') || raw.contains(',') {
        let ranges = raw
            .split(',')
            .map(str::trim)
            .filter(|part| part.contains('-'))
            .map(|part| part.replace('-', ":"))
            .collect::<Vec<_>>();
        if !ranges.is_empty() {
            object.remove("server_port");
            object.insert("server_ports".to_string(), json!(ranges));
        }
    }
}

fn insert_opt_string(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn insert_opt_value(object: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value.filter(|value| !value.is_null()) {
        object.insert(key.to_string(), value);
    }
}

fn port_value(server: &v2board_db::server::AvailableServerRow) -> Value {
    first_port(server)
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(first_port(server)))
}

fn shadowsocks_password(
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

fn obfs_plugin_opts(mode: &str, host: Option<String>, path: Option<String>) -> String {
    let mut parts = vec![format!("obfs={mode}")];
    if let Some(host) = host.filter(|value| !value.is_empty()) {
        parts.push(format!("obfs-host={host}"));
    }
    if let Some(path) = path.filter(|value| !value.is_empty()) {
        parts.push(format!("path={path}"));
    }
    parts.join(";")
}

fn split_jsonish_list(value: &str) -> Value {
    if let Ok(values) = serde_json::from_str::<Vec<String>>(value) {
        json!(values)
    } else {
        Value::String(value.to_string())
    }
}

fn bytes_to_gib(value: i64) -> f64 {
    value as f64 / 1_073_741_824_f64
}

fn render_yaml(value: &Value) -> String {
    let mut output = String::new();
    write_yaml_value(&mut output, value, 0);
    output
}

fn write_yaml_value(output: &mut String, value: &Value, indent: usize) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                output.push_str("{}\n");
                return;
            }
            for (key, value) in map {
                output.push_str(&" ".repeat(indent));
                output.push_str(&yaml_key(key));
                output.push(':');
                if yaml_scalar(value) {
                    output.push(' ');
                    output.push_str(&yaml_scalar_value(value));
                    output.push('\n');
                } else {
                    output.push('\n');
                    write_yaml_value(output, value, indent + 2);
                }
            }
        }
        Value::Array(values) => {
            if values.is_empty() {
                output.push_str(&" ".repeat(indent));
                output.push_str("[]\n");
                return;
            }
            for value in values {
                output.push_str(&" ".repeat(indent));
                output.push('-');
                if yaml_scalar(value) {
                    output.push(' ');
                    output.push_str(&yaml_scalar_value(value));
                    output.push('\n');
                } else {
                    output.push('\n');
                    write_yaml_value(output, value, indent + 2);
                }
            }
        }
        _ => {
            output.push_str(&" ".repeat(indent));
            output.push_str(&yaml_scalar_value(value));
            output.push('\n');
        }
    }
}

fn yaml_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn yaml_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        key.to_string()
    } else {
        yaml_quote(key)
    }
}

fn yaml_scalar_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => yaml_quote(value),
        Value::Array(_) | Value::Object(_) => unreachable!("nested value is not a YAML scalar"),
    }
}

fn yaml_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn build_shadowsocks_uri(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    let password = shadowsocks_password(uuid, server)?;
    let auth = safe_base64_encode(format!("{cipher}:{password}").as_bytes());
    let mut uri = format!(
        "ss://{auth}@{}:{}",
        format_host(&server.host),
        first_port(server)
    );

    if extra_string(server, "obfs").as_deref() == Some("http") {
        let obfs_settings = extra_json(server, "obfs_settings");
        let host = json_path_string(&obfs_settings, &["host"]).unwrap_or_default();
        let path = json_path_string(&obfs_settings, &["path"]).unwrap_or_else(|| "/".to_string());
        uri.push_str(&format!(
            "?plugin=obfs-local;obfs=http;obfs-host={};path={}",
            host, path
        ));
    } else if extra_string(server, "network").as_deref() == Some("http") {
        let network_settings = extra_json(server, "network_settings");
        if let Some(host) = json_path_string(&network_settings, &["Host"]) {
            let path =
                json_path_string(&network_settings, &["path"]).unwrap_or_else(|| "/".to_string());
            uri.push_str(&format!(
                "?plugin=obfs-local;obfs=tls;obfs-host={host};path={path}"
            ));
        }
    }

    Some(format!("{uri}#{}\r\n", percent_encode(&server.name)))
}

fn build_vmess_uri(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut config = serde_json::Map::new();
    json_insert_str(&mut config, "v", "2");
    json_insert_str(&mut config, "ps", &server.name);
    json_insert_str(&mut config, "add", &format_host(&server.host));
    json_insert_str(&mut config, "port", &first_port(server));
    json_insert_str(&mut config, "id", uuid);
    json_insert_str(&mut config, "aid", "0");
    json_insert_str(&mut config, "scy", "auto");
    json_insert_str(&mut config, "net", &network);
    json_insert_str(&mut config, "type", "none");
    json_insert_str(&mut config, "host", "");
    json_insert_str(&mut config, "path", "");
    json_insert_str(&mut config, "tls", if tls != 0 { "tls" } else { "" });
    json_insert_str(&mut config, "fp", "chrome");
    if tls != 0 {
        json_insert_i64(
            &mut config,
            "allowInsecure",
            json_path_i64(&tls_settings, &["allow_insecure"])
                .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                .unwrap_or_default(),
        );
        json_insert_str(
            &mut config,
            "sni",
            &json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"]))
                .unwrap_or_default(),
        );
    }
    configure_vmess_network(
        &network,
        &extra_json(server, "network_settings"),
        &mut config,
    );
    let payload = serde_json::to_string(&serde_json::Value::Object(config)).ok()?;
    Some(format!(
        "vmess://{}\r\n",
        standard_base64_encode(payload.as_bytes())
    ))
}

fn build_vless_uri(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut params = vec![
        ("type".to_string(), network.clone()),
        ("encryption".to_string(), "none".to_string()),
        ("host".to_string(), String::new()),
        ("path".to_string(), String::new()),
        ("headerType".to_string(), "none".to_string()),
        ("quicSecurity".to_string(), "none".to_string()),
        ("serviceName".to_string(), String::new()),
        (
            "security".to_string(),
            if tls == 2 {
                "reality".to_string()
            } else if tls != 0 {
                "tls".to_string()
            } else {
                String::new()
            },
        ),
        (
            "flow".to_string(),
            extra_string(server, "flow").unwrap_or_default(),
        ),
        (
            "fp".to_string(),
            json_path_string(&tls_settings, &["fingerprint"])
                .unwrap_or_else(|| "chrome".to_string()),
        ),
        (
            "insecure".to_string(),
            json_path_i64(&tls_settings, &["allow_insecure"])
                .unwrap_or_default()
                .to_string(),
        ),
    ];

    if tls != 0 {
        set_param(
            &mut params,
            "sni",
            json_path_string(&tls_settings, &["server_name"]).unwrap_or_default(),
        );
        if tls == 2 {
            set_param(
                &mut params,
                "pbk",
                json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
            );
            set_param(
                &mut params,
                "sid",
                json_path_string(&tls_settings, &["short_id"]).unwrap_or_default(),
            );
        }
    }
    add_ech_param(&mut params, &tls_settings);
    if extra_string(server, "encryption").as_deref() == Some("mlkem768x25519plus") {
        let settings = extra_json(server, "encryption_settings");
        let mut encryption = format!(
            "mlkem768x25519plus.{}.{}",
            json_path_string(&settings, &["mode"]).unwrap_or_else(|| "native".to_string()),
            json_path_string(&settings, &["rtt"]).unwrap_or_else(|| "1rtt".to_string())
        );
        if let Some(client_padding) =
            json_path_string(&settings, &["client_padding"]).filter(|value| !value.is_empty())
        {
            encryption.push('.');
            encryption.push_str(&client_padding);
        }
        encryption.push('.');
        encryption.push_str(&json_path_string(&settings, &["password"]).unwrap_or_default());
        set_param(&mut params, "encryption", encryption);
    }

    configure_query_network(
        &network,
        &extra_json(server, "network_settings"),
        &mut params,
    );
    Some(build_uri_string(
        "vless",
        uuid,
        server,
        &encode_uri_component(&server.name),
        &params,
    ))
}

fn build_trojan_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let mut params = vec![
        (
            "allowInsecure".to_string(),
            extra_i64(server, "allow_insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "peer".to_string(),
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
        (
            "sni".to_string(),
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
        ("type".to_string(), network.clone()),
    ];
    let network_settings = extra_json(server, "network_settings");
    match network.as_str() {
        "grpc" => {
            if let Some(service_name) = json_path_string(&network_settings, &["serviceName"]) {
                set_param(&mut params, "serviceName", service_name);
            }
        }
        "ws" => {
            if let Some(path) = json_path_string(&network_settings, &["path"]) {
                set_param(&mut params, "path", path);
            }
            if let Some(host) = json_path_string(&network_settings, &["headers", "Host"]) {
                set_param(&mut params, "host", host);
            }
        }
        _ => {}
    }
    add_ech_param(&mut params, &tls_settings);

    Some(format!(
        "trojan://{password}@{}:{}?{}#{}\r\n",
        format_host(&server.host),
        first_port(server),
        query_string(&params),
        percent_encode(&server.name)
    ))
}

fn build_hysteria_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    if extra_i64(server, "version") == Some(2) {
        return build_hysteria2_uri(password, server);
    }
    let mut uri = format!(
        "hysteria://{}:{}/?protocol=udp&auth={}&insecure={}&peer={}&upmbps={}&downmbps={}",
        format_host(&server.host),
        first_port(server),
        percent_encode(password),
        extra_i64(server, "insecure").unwrap_or_default(),
        percent_encode(&extra_string(server, "server_name").unwrap_or_default()),
        extra_i64(server, "down_mbps").unwrap_or_default(),
        extra_i64(server, "up_mbps").unwrap_or_default()
    );
    append_hysteria_obfs(&mut uri, server, false);
    if let Some(mport) = mport(server) {
        uri.push_str("&mport=");
        uri.push_str(&percent_encode(&mport));
    }
    Some(format!("{uri}#{}\r\n", encode_uri_component(&server.name)))
}

fn build_hysteria2_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let insecure = extra_i64(server, "insecure")
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .unwrap_or_default();
    let sni = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
        .unwrap_or_default();
    let mut uri = format!(
        "hysteria2://{}@{}:{}/?insecure={insecure}&sni={}",
        percent_encode(password),
        format_host(&server.host),
        first_port(server),
        percent_encode(&sni)
    );
    append_hysteria_obfs(&mut uri, server, true);
    if let Some(mport) = mport(server) {
        uri.push_str("&mport=");
        uri.push_str(&percent_encode(&mport));
    }
    Some(format!("{uri}#{}\r\n", encode_uri_component(&server.name)))
}

fn build_tuic_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let params = vec![
        (
            "sni".to_string(),
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
        ("alpn".to_string(), "h3".to_string()),
        (
            "congestion_control".to_string(),
            extra_string(server, "congestion_control").unwrap_or_default(),
        ),
        (
            "allow_insecure".to_string(),
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "disable_sni".to_string(),
            extra_i64(server, "disable_sni")
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "udp_relay_mode".to_string(),
            extra_string(server, "udp_relay_mode").unwrap_or_default(),
        ),
    ];
    Some(format!(
        "tuic://{password}:{password}@{}:{}?{}#{}\r\n",
        format_host(&server.host),
        first_port(server),
        query_string(&params),
        encode_uri_component(&server.name)
    ))
}

fn build_anytls_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let mut params = vec![
        ("type".to_string(), network.clone()),
        (
            "insecure".to_string(),
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "fp".to_string(),
            json_path_string(&tls_settings, &["fingerprint"])
                .unwrap_or_else(|| "chrome".to_string()),
        ),
    ];
    if let Some(sni) = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
    {
        set_param(&mut params, "sni", sni);
    }
    if extra_i64(server, "tls") == Some(2) {
        set_param(&mut params, "security", "reality");
        set_param(
            &mut params,
            "pbk",
            json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
        );
        set_param(
            &mut params,
            "sid",
            json_path_string(&tls_settings, &["short_id"]).unwrap_or_default(),
        );
    }
    configure_query_network(
        &network,
        &extra_json(server, "network_settings"),
        &mut params,
    );
    Some(format!(
        "anytls://{}@{}:{}/?{}#{}\r\n",
        percent_encode(password),
        format_host(&server.host),
        first_port(server),
        query_string(&params),
        encode_uri_component(&server.name)
    ))
}

fn server_protocol(server: &v2board_db::server::AvailableServerRow) -> String {
    if server.r#type == "v2node" {
        return extra_string(server, "protocol").unwrap_or_else(|| "v2node".to_string());
    }
    server.r#type.clone()
}

fn configure_vmess_network(
    network: &str,
    settings: &serde_json::Value,
    config: &mut serde_json::Map<String, serde_json::Value>,
) {
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                json_insert_str(config, "type", "http");
                if let Some(host) =
                    json_path_string(settings, &["header", "request", "headers", "Host"])
                {
                    json_insert_str(config, "host", &host);
                }
                if let Some(path) = json_path_string(settings, &["header", "request", "path"]) {
                    json_insert_str(config, "path", &path);
                }
            }
        }
        "ws" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "host",
                &json_path_string(settings, &["headers", "Host"]).unwrap_or_default(),
            );
            if let Some(security) = json_path_string(settings, &["security"]) {
                json_insert_str(config, "scy", &security);
            }
        }
        "grpc" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["serviceName"]).unwrap_or_default(),
            );
        }
        "kcp" => {
            if let Some(seed) = json_path_string(settings, &["seed"]) {
                json_insert_str(config, "path", &seed);
            }
            json_insert_str(
                config,
                "type",
                &json_path_string(settings, &["header", "type"])
                    .unwrap_or_else(|| "none".to_string()),
            );
        }
        "httpupgrade" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "host",
                &json_path_string(settings, &["host"]).unwrap_or_default(),
            );
        }
        "xhttp" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "host",
                &json_path_string(settings, &["host"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "mode",
                &json_path_string(settings, &["mode"]).unwrap_or_else(|| "auto".to_string()),
            );
            if let Some(extra) = json_path_value(settings, &["extra"]) {
                json_insert_str(
                    config,
                    "extra",
                    &serde_json::to_string(extra).unwrap_or_default(),
                );
            }
        }
        _ => {}
    }
}

fn configure_query_network(
    network: &str,
    settings: &serde_json::Value,
    params: &mut Vec<(String, String)>,
) {
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                set_param(params, "headerType", "http");
                set_param(
                    params,
                    "host",
                    json_path_string(settings, &["header", "request", "headers", "Host"])
                        .unwrap_or_default(),
                );
                set_param(
                    params,
                    "path",
                    json_path_string(settings, &["header", "request", "path"]).unwrap_or_default(),
                );
            }
        }
        "ws" => {
            set_param(
                params,
                "path",
                json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            set_param(
                params,
                "host",
                json_path_string(settings, &["headers", "Host"]).unwrap_or_default(),
            );
        }
        "grpc" => {
            set_param(
                params,
                "serviceName",
                json_path_string(settings, &["serviceName"]).unwrap_or_default(),
            );
        }
        "kcp" => {
            set_param(
                params,
                "headerType",
                json_path_string(settings, &["header", "type"])
                    .unwrap_or_else(|| "none".to_string()),
            );
            if let Some(seed) = json_path_string(settings, &["seed"]) {
                set_param(params, "seed", seed);
            }
        }
        "httpupgrade" => {
            set_param(
                params,
                "path",
                json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            set_param(
                params,
                "host",
                json_path_string(settings, &["host"]).unwrap_or_default(),
            );
        }
        "xhttp" => {
            set_param(
                params,
                "path",
                json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            set_param(
                params,
                "host",
                json_path_string(settings, &["host"]).unwrap_or_default(),
            );
            set_param(
                params,
                "mode",
                json_path_string(settings, &["mode"]).unwrap_or_else(|| "auto".to_string()),
            );
            if let Some(extra) = json_path_value(settings, &["extra"]) {
                set_param(
                    params,
                    "extra",
                    serde_json::to_string(extra).unwrap_or_default(),
                );
            }
        }
        _ => {}
    }
}

fn add_ech_param(params: &mut Vec<(String, String)>, tls_settings: &serde_json::Value) {
    match json_path_string(tls_settings, &["ech"]).as_deref() {
        Some("cloudflare") => {
            set_param(
                params,
                "ech",
                "cloudflare-ech.com+https://doh.pub/dns-query",
            );
        }
        Some("custom") => {
            if let Some(ech_config) = json_path_string(tls_settings, &["ech_config"]) {
                set_param(params, "ech", ech_config);
            }
        }
        _ => {}
    }
}

fn append_hysteria_obfs(
    uri: &mut String,
    server: &v2board_db::server::AvailableServerRow,
    hysteria2: bool,
) {
    let Some(obfs) = extra_string(server, "obfs") else {
        return;
    };
    let Some(obfs_password) = extra_string(server, "obfs_password") else {
        return;
    };
    uri.push_str("&obfs=");
    uri.push_str(&percent_encode(&obfs));
    if hysteria2 {
        uri.push_str("&obfs-password=");
    } else {
        uri.push_str("&obfsParam=");
    }
    uri.push_str(&percent_encode(&obfs_password));
}

fn build_uri_string(
    scheme: &str,
    auth: &str,
    server: &v2board_db::server::AvailableServerRow,
    name: &str,
    params: &[(String, String)],
) -> String {
    format!(
        "{scheme}://{auth}@{}:{}?{}#{name}\r\n",
        format_host(&server.host),
        first_port(server),
        query_string(params)
    )
}

fn query_string(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn set_param(params: &mut Vec<(String, String)>, key: &str, value: impl Into<String>) {
    let value = value.into();
    if let Some((_, existing)) = params.iter_mut().find(|(existing, _)| existing == key) {
        *existing = value;
    } else {
        params.push((key.to_string(), value));
    }
}

fn json_insert_str(
    config: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &str,
) {
    config.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
}

fn json_insert_i64(config: &mut serde_json::Map<String, serde_json::Value>, key: &str, value: i64) {
    config.insert(key.to_string(), serde_json::Value::from(value));
}

fn extra_json(server: &v2board_db::server::AvailableServerRow, key: &str) -> serde_json::Value {
    match server.extra.get(key) {
        Some(serde_json::Value::String(value)) => {
            serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.clone()))
        }
        Some(value) => value.clone(),
        None => serde_json::Value::Null,
    }
}

fn extra_string(server: &v2board_db::server::AvailableServerRow, key: &str) -> Option<String> {
    server.extra.get(key).and_then(value_to_string)
}

fn extra_i64(server: &v2board_db::server::AvailableServerRow, key: &str) -> Option<i64> {
    server.extra.get(key).and_then(value_to_i64)
}

fn json_path_value<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn json_path_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    json_path_value(value, path).and_then(value_to_string)
}

fn json_path_i64(value: &serde_json::Value, path: &[&str]) -> Option<i64> {
    json_path_value(value, path).and_then(value_to_i64)
}

fn first_port(server: &v2board_db::server::AvailableServerRow) -> String {
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

fn mport(server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let port = port_text(server);
    (port.contains('-') || port.contains(',')).then_some(port)
}

fn port_text(server: &v2board_db::server::AvailableServerRow) -> String {
    value_to_string(&server.port).unwrap_or_default()
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn encode_uri_component(value: &str) -> String {
    percent_encode(value)
        .replace("%21", "!")
        .replace("%2A", "*")
        .replace("%27", "'")
        .replace("%28", "(")
        .replace("%29", ")")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singbox_flag_uses_legacy_without_modern_version() {
        assert!(!singbox_modern_flag("sing-box"));
        assert!(!singbox_modern_flag("sing-box 1.11.9"));
    }

    #[test]
    fn singbox_flag_uses_modern_for_1_12_and_newer() {
        assert!(singbox_modern_flag("sing-box 1.12.0"));
        assert!(singbox_modern_flag("sing box 1.12.0"));
        assert!(singbox_modern_flag("sing-box/1.13.2"));
    }

    #[test]
    fn quantumultx_anytls_matches_legacy_reference_shape() {
        let server = v2board_db::server::AvailableServerRow {
            id: 1,
            parent_id: None,
            group_id: vec![1],
            route_id: None,
            name: "anytls-reality-tls-01".to_string(),
            rate: "1".to_string(),
            r#type: "anytls".to_string(),
            host: "example.com".to_string(),
            port: serde_json::json!(443),
            cache_key: "anytls-1".to_string(),
            last_check_at: None,
            is_online: 0,
            tags: None,
            sort: None,
            extra: serde_json::json!({
                "network": "tcp",
                "tls_settings": {
                    "server_name": "apple.com",
                    "allow_insecure": false
                }
            }),
        };

        let line = build_quantumultx_proxy("pwd", &server).expect("anytls output");
        assert!(line.contains("anytls=example.com:443"));
        assert!(line.contains("password=pwd"));
        assert!(line.contains("udp-relay=true"));
        assert!(line.contains("tag=anytls-reality-tls-01"));
        assert!(line.contains("over-tls=true"));
        assert!(line.contains("tls-host=apple.com"));
        assert!(line.contains("tls-verification=true"));
        assert!(line.ends_with("\r\n"));
    }
}
