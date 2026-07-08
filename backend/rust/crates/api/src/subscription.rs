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
    // content-disposition header value; per-format so main.rs can emit it verbatim
    // (Clash.php:27, Stash.php:27, Surge.php:25, Singbox.php:33).
    pub(super) content_disposition: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriptionFormat {
    General,
    Base64Uri,
    V2RayTun,
    Clash,
    ClashMeta,
    Stash,
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

// Clash-family template variants. Clash uses the ss/vmess/trojan subset; Meta
// (also Verge/Nyanpasu) adds vless/tuic/anytls/hysteria; Stash uses its own
// template plus the forced-DIRECT rule (Stash.php:100-103).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClashKind {
    Clash,
    Meta,
    Stash,
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
        } else if normalized.contains("quantumult x") {
            // QuantumultX::$flag is the literal `quantumult%20x` (normalized here to
            // `quantumult x`); the original non-X `Quantumult/…` app must fall through.
            Self::QuantumultX
        } else if normalized.contains("v2raytun") {
            // v2RayTun reuses V2rayN's base64-URI body but has its own
            // quoted-filename content-disposition (v2RayTun.php:58), so it gets a
            // dedicated format instead of the shared Base64Uri disposition.
            Self::V2RayTun
        } else if normalized.contains("v2rayn")
            || normalized.contains("v2rayng")
            || normalized.contains("passwall")
            || normalized.contains("ssrplus")
        {
            Self::Base64Uri
        } else if normalized.contains("stash") {
            // Stash has its own protocol handler in Laravel (Stash.php) — own
            // template, own content-disposition, and an active forced-DIRECT rule.
            Self::Stash
        } else if normalized.contains("meta")
            || normalized.contains("nyanpasu")
            || normalized.contains("verge")
        {
            // `mihomo` intentionally dropped: Laravel has no `mihomo` flag, so a
            // mihomo UA falls through to General (base64). ClashVerge/ClashNyanpasu
            // are ClashMeta clones (identical build path), so route them to Meta.
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
    // Request Host header, used for Surge/Surfboard `$subs_domain` and Stash's
    // forced-DIRECT rule (`$_SERVER['HTTP_HOST']` in Laravel). Pass "" if absent.
    host: &str,
) -> Result<SubscriptionDocument, ApiError> {
    let format = SubscriptionFormat::detect(flag);
    let body = match format {
        SubscriptionFormat::General => build_general_subscription(&user.uuid, servers),
        SubscriptionFormat::Base64Uri | SubscriptionFormat::V2RayTun => {
            build_base64_uri_subscription(&user.uuid, servers)
        }
        SubscriptionFormat::Clash => {
            build_clash_subscription(config, &user.uuid, servers, ClashKind::Clash, host)
        }
        SubscriptionFormat::ClashMeta => {
            build_clash_subscription(config, &user.uuid, servers, ClashKind::Meta, host)
        }
        SubscriptionFormat::Stash => {
            build_clash_subscription(config, &user.uuid, servers, ClashKind::Stash, host)
        }
        SubscriptionFormat::SingBox => {
            build_singbox_subscription(config, &user.uuid, servers, true)?
        }
        SubscriptionFormat::SingBoxLegacy => {
            build_singbox_subscription(config, &user.uuid, servers, false)?
        }
        SubscriptionFormat::Surge => build_surge_subscription(config, user, servers, host),
        SubscriptionFormat::Surfboard => build_surfboard_subscription(config, user, servers, host),
        SubscriptionFormat::Loon => build_loon_subscription(&user.uuid, servers),
        SubscriptionFormat::Shadowsocks => build_shadowsocks_sip008_subscription(user, servers)?,
        SubscriptionFormat::Shadowrocket => build_shadowrocket_subscription(user, servers),
        SubscriptionFormat::SagerNet => build_sagernet_subscription(&user.uuid, servers),
        SubscriptionFormat::QuantumultX => build_quantumultx_subscription(&user.uuid, servers),
    };
    let content_type = match format {
        SubscriptionFormat::Clash | SubscriptionFormat::ClashMeta | SubscriptionFormat::Stash => {
            "application/yaml; charset=utf-8"
        }
        SubscriptionFormat::SingBox
        | SubscriptionFormat::SingBoxLegacy
        | SubscriptionFormat::Shadowsocks => "application/json; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    };
    let encoded_name = percent_encode(&config.app_name);
    let content_disposition = match format {
        // Stash omits the `attachment` disposition (Stash.php:27).
        SubscriptionFormat::Stash => format!("filename*=UTF-8''{encoded_name}"),
        // Surge/Surfboard append a `.conf` suffix (Surge.php:25).
        SubscriptionFormat::Surge | SubscriptionFormat::Surfboard => {
            format!("attachment;filename*=UTF-8''{encoded_name}.conf")
        }
        // Sing-box and v2RayTun use a plain quoted, non-encoded filename
        // (Singbox.php:33, v2RayTun.php:58).
        SubscriptionFormat::SingBox
        | SubscriptionFormat::SingBoxLegacy
        | SubscriptionFormat::V2RayTun => {
            format!("attachment; filename=\"{}\"", config.app_name)
        }
        // Clash/Meta and the base64 formats (Clash.php:27).
        _ => format!("attachment;filename*=UTF-8''{encoded_name}"),
    };
    Ok(SubscriptionDocument {
        body,
        content_type,
        content_disposition,
    })
}

/// Build the `show_info_to_server_enable` pseudo-nodes
/// (ClientController::setSubscribeInfoToServers). Each row clones the first
/// server (so it renders as a working node) and overrides only its display name
/// with the remaining-traffic / next-reset / plan-expiry banners. The returned
/// rows are already in prepend order (front-to-back); main.rs must splice them
/// in front of the real server list before rendering. `reset_day` is computed by
/// main.rs from the user's plan (`UserService::getResetDay`); pass `None` to omit
/// the reset banner.
pub(super) fn build_info_servers(
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
    reset_day: Option<i64>,
    config: &AppConfig,
) -> Vec<v2board_db::server::AvailableServerRow> {
    // Laravel returns early when there are no servers or the feature is off.
    if servers.is_empty() || !config.show_info_to_server_enable {
        return Vec::new();
    }
    let base = &servers[0];
    let use_traffic = user.u + user.d;
    let remaining = traffic_convert(user.transfer_enable - use_traffic);
    // `$user['expired_at'] ? date('Y-m-d', ...) : '长期有效'` — 0/null are falsy.
    let expired = user
        .expired_at
        .filter(|&timestamp| timestamp != 0)
        .map(format_date_timestamp)
        .unwrap_or_else(|| "长期有效".to_string());
    let named = |name: String| {
        let mut row = base.clone();
        row.name = name;
        row
    };
    // array_unshift stacking yields front-to-back: remaining, [reset], expiry.
    let mut rows = vec![named(format!("剩余流量：{remaining}"))];
    if let Some(days) = reset_day {
        rows.push(named(format!("距离下次重置剩余：{days} 天")));
    }
    rows.push(named(format!("套餐到期：{expired}")));
    rows
}

// Helper::trafficConvert — byte formatter with a `< 0 => "0"` branch that is
// deliberately checked after the KB branch (Helper.php:82-98).
fn traffic_convert(byte: i64) -> String {
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

// Embedded Clash/Stash templates. Laravel parses `resources/rules/default.clash.yaml`
// (~516 rules + regionalised proxy-groups) and Stash uses `default.stash.yaml`.
// No YAML parser is available to this crate, so the DEFAULT templates were
// converted to JSON offline and are embedded here; the rendered output is YAML
// via `render_yaml`. Operator `custom.clash.yaml`/`custom.stash.yaml` overrides
// are honoured by `load_clash_template` when JSON-encoded; genuine YAML custom
// files still need a YAML dependency — see report.
const CLASH_TEMPLATE: &str = include_str!("../resources/rules/default.clash.json");
const STASH_TEMPLATE: &str = include_str!("../resources/rules/default.stash.json");

fn build_clash_subscription(
    config: &AppConfig,
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
    kind: ClashKind,
    host: &str,
) -> String {
    // Clash only emits ss/vmess/trojan; Meta/Stash add the extended protocols.
    let meta = !matches!(kind, ClashKind::Clash);
    let proxies = servers
        .iter()
        .filter_map(|server| build_clash_proxy(uuid, server, meta))
        .collect::<Vec<_>>();
    // Operator custom templates override the embedded default: Clash/Meta share
    // `custom.clash.yaml`, Stash uses `custom.stash.yaml` (Clash.php:29-35,
    // ClashMeta.php:28-34, Stash.php:29-35). The dir mirrors Laravel's
    // `resources/rules` via `runtime_paths.rules`, the same source the sing-box
    // loader reads.
    let (custom_name, embedded) = match kind {
        ClashKind::Stash => ("custom.stash.yaml", STASH_TEMPLATE),
        _ => ("custom.clash.yaml", CLASH_TEMPLATE),
    };
    let template = load_clash_template(config, custom_name, embedded);
    // Only Stash keeps the forced-DIRECT rule active (Stash.php:100-103); Clash
    // and ClashMeta leave it commented out.
    let forced_direct_host = matches!(kind, ClashKind::Stash).then_some(host);
    render_clash_document(template, proxies, &config.app_name, forced_direct_host)
}

// Load the Clash/Stash template, preferring an operator custom file over the
// embedded default (Clash.php:31-35 `\File::exists($customConfig)`). NOTE: no YAML
// parser is linked into this crate, so the custom file is read as JSON — this
// covers JSON-encoded custom templates, but a genuine YAML `custom.clash.yaml`
// cannot be parsed here and falls back to the embedded default. Full YAML custom
// support needs a YAML dependency (see report).
fn load_clash_template(config: &AppConfig, custom_name: &str, embedded: &str) -> Value {
    let custom_path = config.runtime_paths.rules.join(custom_name);
    if let Ok(body) = fs::read_to_string(&custom_path)
        && let Ok(value) = serde_json::from_str::<Value>(&body)
    {
        return value;
    }
    serde_json::from_str(embedded).unwrap_or_else(|_| json!({}))
}

fn render_clash_document(
    mut config: Value,
    proxies: Vec<Value>,
    app_name: &str,
    forced_direct_host: Option<&str>,
) -> String {
    let proxy_names = proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();

    // $config['proxies'] = array_merge($config['proxies'] ?: [], $proxy)
    let mut merged = match config.get("proxies") {
        Some(Value::Array(existing)) => existing.clone(),
        _ => Vec::new(),
    };
    merged.extend(proxies);
    config["proxies"] = Value::Array(merged);

    // The default templates use no regex filters, so every proxy-group receives
    // all generated proxy names (Clash.php:65-86 array_merge branch); groups that
    // stay empty are dropped.
    if let Some(groups) = config.get_mut("proxy-groups").and_then(Value::as_array_mut) {
        for group in groups.iter_mut() {
            if !group.get("proxies").is_some_and(Value::is_array) {
                group["proxies"] = json!([]);
            }
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

    // Stash prepends `DOMAIN,<HTTP_HOST>,DIRECT` (Stash.php:100-103).
    if let Some(host) = forced_direct_host.filter(|host| !host.is_empty())
        && let Some(rules) = config.get_mut("rules").and_then(Value::as_array_mut)
    {
        rules.insert(0, Value::String(format!("DOMAIN,{host},DIRECT")));
    }

    // Laravel str_replace('$app_name', ...) after dumping the YAML.
    render_yaml(&config).replace("$app_name", app_name)
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
        .filter_map(|server| build_singbox_proxy(uuid, server, modern))
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

// Embedded managed-config templates (~500 rules, DoH, [Replica], [URL Rewrite]).
// These are plain-text with `$subs_link` / `$subs_domain` / `$proxies` /
// `$proxy_group` / `$subscribe_info` placeholders (Surge.php:62-88).
const SURGE_TEMPLATE: &str = include_str!("../resources/rules/default.surge.conf");
const SURFBOARD_TEMPLATE: &str = include_str!("../resources/rules/default.surfboard.conf");

fn build_surge_subscription(
    config: &AppConfig,
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
    host: &str,
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_surge_proxy(&user.uuid, server))
        .collect::<String>();
    let proxy_group = servers
        .iter()
        .filter(|server| supports_surge(server))
        .map(|server| server.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    render_managed_config(SURGE_TEMPLATE, config, user, host, &proxies, &proxy_group)
}

fn build_surfboard_subscription(
    config: &AppConfig,
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
    host: &str,
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_surfboard_proxy(&user.uuid, server))
        .collect::<String>();
    let proxy_group = servers
        .iter()
        .filter(|server| supports_surfboard(server))
        .map(|server| server.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    render_managed_config(
        SURFBOARD_TEMPLATE,
        config,
        user,
        host,
        &proxies,
        &proxy_group,
    )
}

// Shared Surge/Surfboard placeholder substitution (Surge.php:70-87).
fn render_managed_config(
    template: &str,
    config: &AppConfig,
    user: &v2board_db::user::UserAccessRow,
    host: &str,
    proxies: &str,
    proxy_group: &str,
) -> String {
    let upload = round2(user.u as f64 / GIB);
    let download = round2(user.d as f64 / GIB);
    let use_traffic = upload + download;
    let total = round2(user.transfer_enable as f64 / GIB);
    let expire = user
        .expired_at
        .filter(|&timestamp| timestamp != 0)
        .map(format_datetime_timestamp)
        .unwrap_or_else(|| "长期有效".to_string());
    // Note the literal `\n` sequences (PHP `\\n`) inside the info banner.
    let subscribe_info = format!(
        "title={app_name}订阅信息, content=上传流量：{upload}GB\\n下载流量：{download}GB\\n剩余流量：{use}GB\\n套餐流量：{total}GB\\n到期时间：{expire}",
        app_name = config.app_name,
        upload = php_round2(upload),
        download = php_round2(download),
        use = php_round2(use_traffic),
        total = php_round2(total),
        expire = expire,
    );
    template
        .replace("$subs_link", &config.subscribe_url_for_token(&user.token))
        .replace("$subs_domain", host)
        .replace("$proxies", proxies)
        .replace("$proxy_group", proxy_group)
        .replace("$subscribe_info", &subscribe_info)
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
    let upload = php_round2(round2(user.u as f64 / GIB));
    let download = php_round2(round2(user.d as f64 / GIB));
    let total = php_round2(round2(user.transfer_enable as f64 / GIB));
    // Shadowrocket.php:28 has no null guard: `date('Y-m-d', $user['expired_at'])`.
    // A null timestamp coerces to time() (today), not 长期有效.
    let expire = user
        .expired_at
        .map(format_date_timestamp)
        .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
    // Restore the 🚀 / 💡 emojis and trailing-zero-trimmed numbers (Shadowrocket.php:29).
    let mut lines =
        format!("STATUS=🚀↑:{upload}GB,↓:{download}GB,TOT:{total}GB💡Expires:{expire}\r\n");
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
        // SagerNet.php:24-26 skips only the raw `type === 'hysteria'`, so a
        // v2node-wrapped hysteria (type `v2node`, protocol `hysteria`) is kept.
        if server.r#type == "hysteria" {
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
        // Clash (non-meta) only accepts the four basic ciphers (Clash.php:43-49);
        // Meta/Stash accept every cipher, including ss2022 (ClashMeta.php:44-47,
        // Stash.php:43-46).
        "shadowsocks"
            if meta
                || extra_string(server, "cipher")
                    .as_deref()
                    .map(is_basic_shadowsocks_cipher)
                    .unwrap_or(false) =>
        {
            build_clash_shadowsocks(uuid, server)
        }
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
    modern: bool,
) -> Option<Value> {
    match server_protocol(server).as_str() {
        "shadowsocks" => build_singbox_shadowsocks(uuid, server, modern),
        "vmess" => build_singbox_vmess(uuid, server, modern),
        "vless" => build_singbox_vless(uuid, server, modern),
        "trojan" => build_singbox_trojan(uuid, server, modern),
        "tuic" => build_singbox_tuic(uuid, server, modern),
        // anytls has no legacy (SingboxOld) builder, so legacy clients skip it.
        "anytls" if modern => build_singbox_anytls(uuid, server),
        "hysteria" => build_singbox_hysteria(uuid, server, modern),
        "hysteria2" => build_singbox_hysteria2(uuid, server, modern),
        _ => None,
    }
}

// Modern sing-box (>= 1.12) tags every outbound with `domain_resolver: local`;
// the legacy builder (SingboxOld.php) omits it entirely.
fn insert_singbox_domain_resolver(object: &mut Map<String, Value>, modern: bool) {
    if modern {
        object.insert(
            "domain_resolver".to_string(),
            Value::String("local".to_string()),
        );
    }
}

fn build_singbox_shadowsocks(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
    modern: bool,
) -> Option<Value> {
    let cipher = extra_string(server, "cipher")?;
    let mut object = singbox_base(server, "shadowsocks");
    object.insert("method".to_string(), Value::String(cipher));
    object.insert(
        "password".to_string(),
        Value::String(shadowsocks_password(uuid, server)?),
    );
    insert_singbox_domain_resolver(&mut object, modern);
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
    } else if extra_string(server, "network").as_deref() == Some("http") {
        // network==http obfs fallback (Singbox.php:131-140 / SingboxOld:124-133):
        // only when network_settings.Host is present.
        let settings = extra_json(server, "network_settings");
        if let Some(host) = json_path_string(&settings, &["Host"]) {
            let path = json_path_string(&settings, &["path"]).unwrap_or_else(|| "/".to_string());
            object.insert(
                "plugin".to_string(),
                Value::String("obfs-local".to_string()),
            );
            object.insert(
                "plugin_opts".to_string(),
                Value::String(format!("obfs=http;obfs-host={host};path={path}")),
            );
        }
    }
    Some(Value::Object(object))
}

fn build_singbox_vmess(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
    modern: bool,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "vmess");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("security".to_string(), Value::String("auto".to_string()));
    object.insert("alter_id".to_string(), Value::from(0));
    insert_singbox_domain_resolver(&mut object, modern);
    if tls != 0 {
        object.insert(
            "tls".to_string(),
            singbox_tls(server, &tls_settings, tls, false, modern),
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
    modern: bool,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "vless");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "packet_encoding".to_string(),
        Value::String("xudp".to_string()),
    );
    insert_opt_string(&mut object, "flow", extra_string(server, "flow"));
    if tls != 0 {
        object.insert(
            "tls".to_string(),
            singbox_tls(server, &tls_settings, tls, true, modern),
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
    modern: bool,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "trojan");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "tls".to_string(),
        singbox_tls(server, &tls_settings, 1, false, modern),
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
    modern: bool,
) -> Option<Value> {
    // Singbox.php:333-357 (modern) / SingboxOld.php:281-304 (legacy). The two
    // builders are identical apart from the modern `domain_resolver` tag.
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "tuic");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    // `?? 'cubic'` / `?? 'native'` only substitute an unset value, so keep an
    // explicit (even empty) column value rather than dropping it.
    object.insert(
        "congestion_control".to_string(),
        Value::String(
            extra_string(server, "congestion_control").unwrap_or_else(|| "cubic".to_string()),
        ),
    );
    object.insert(
        "udp_relay_mode".to_string(),
        Value::String(
            extra_string(server, "udp_relay_mode").unwrap_or_else(|| "native".to_string()),
        ),
    );
    object.insert(
        "zero_rtt_handshake".to_string(),
        Value::Bool(extra_i64(server, "zero_rtt_handshake").unwrap_or_default() == 1),
    );
    insert_singbox_domain_resolver(&mut object, modern);
    // Fixed inline TLS block: alpn ['h3'] + disable_sni, never ECH/reality/utls.
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        // insecure => ($server['insecure'] ?? $tlsSettings['allow_insecure'] ?? 0) == 1
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    tls.insert("alpn".to_string(), json!(["h3"]));
    tls.insert(
        "disable_sni".to_string(),
        Value::Bool(extra_i64(server, "disable_sni").unwrap_or_default() == 1),
    );
    tls.insert(
        "server_name".to_string(),
        // server_name => $server['server_name'] ?? $tlsSettings['server_name'] ?? ''
        Value::String(
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
    );
    object.insert("tls".to_string(), Value::Object(tls));
    Some(Value::Object(object))
}

fn build_singbox_anytls(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<Value> {
    // Singbox.php:359-418. anytls is modern-only (SingboxOld has no builder), so
    // it always emits `domain_resolver`. TLS: alpn ['h2','http/1.1'], optional
    // reality (tls==2) + utls when tls_settings present, never ECH.
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let tls_mode = extra_i64(server, "tls").unwrap_or(1);
    let mut object = singbox_base(server, "anytls");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    tls.insert("alpn".to_string(), json!(["h2", "http/1.1"]));
    tls.insert(
        "server_name".to_string(),
        Value::String(
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
    );
    if value_is_non_empty(&tls_settings) {
        if tls_mode == 2 {
            tls.insert(
                "reality".to_string(),
                json!({
                    "enabled": true,
                    "public_key": json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
                    "short_id": json_path_string(&tls_settings, &["short_id"]).unwrap_or_default()
                }),
            );
        }
        tls.insert(
            "utls".to_string(),
            json!({
                "enabled": true,
                "fingerprint": json_path_string(&tls_settings, &["fingerprint"]).unwrap_or_else(|| "chrome".to_string())
            }),
        );
    }
    object.insert("tls".to_string(), Value::Object(tls));
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
    modern: bool,
) -> Option<Value> {
    // Singbox.php:420-478 (modern) / SingboxOld.php:306-351 (legacy). A single
    // builder covers hysteria v1 AND v2. Modern additionally emits
    // `domain_resolver` and multi-port `server_ports`; legacy keeps the single
    // first `server_port` from `singbox_base`.
    let mut object = singbox_base(server, "hysteria");
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "tls".to_string(),
        json!({
            "enabled": true,
            "insecure": extra_i64(server, "insecure").unwrap_or_default() == 1,
            "server_name": extra_string(server, "server_name").unwrap_or_default()
        }),
    );
    if modern {
        set_singbox_hysteria_ports(&mut object, server);
    }
    let version = extra_i64(server, "version");
    if version.is_none() || version == Some(1) {
        object.insert("auth_str".to_string(), Value::String(uuid.to_string()));
        object.insert("type".to_string(), Value::String("hysteria".to_string()));
        // NOTE: the up/down swap is intentional (matches the oracle). The
        // `min($mbps, $user->speed_limit)` clamp cannot be reproduced — the Rust
        // access row carries no per-user speed_limit — so raw column values are
        // emitted unclamped. Reported as a data gap.
        object.insert(
            "up_mbps".to_string(),
            Value::from(extra_i64(server, "down_mbps").unwrap_or_default()),
        );
        object.insert(
            "down_mbps".to_string(),
            Value::from(extra_i64(server, "up_mbps").unwrap_or_default()),
        );
        // obfs = obfs_password, gated on BOTH columns being set (isset && isset).
        if extra_string(server, "obfs").is_some()
            && let Some(obfs_password) = extra_string(server, "obfs_password")
        {
            object.insert("obfs".to_string(), Value::String(obfs_password));
        }
        object.insert("disable_mtu_discovery".to_string(), Value::Bool(true));
    } else if version == Some(2) {
        object.insert("password".to_string(), Value::String(uuid.to_string()));
        object.insert("type".to_string(), Value::String("hysteria2".to_string()));
        if let Some(obfs) = extra_string(server, "obfs") {
            object.insert(
                "obfs".to_string(),
                json!({
                    "type": obfs,
                    "password": extra_string(server, "obfs_password").unwrap_or_default()
                }),
            );
        }
    }
    Some(Value::Object(object))
}

fn build_singbox_hysteria2(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
    modern: bool,
) -> Option<Value> {
    // Singbox.php:480-509 (modern) / SingboxOld.php:353-381 (legacy). Always a
    // single first `server_port` (from `singbox_base`), never `server_ports`.
    // Modern adds `domain_resolver`. TLS reads tls_settings and never emits ECH.
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "hysteria2");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "tls".to_string(),
        json!({
            "enabled": true,
            "insecure": json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() == 1,
            "server_name": json_path_string(&tls_settings, &["server_name"]).unwrap_or_default()
        }),
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
    // Loon.php:27-44 dispatch. vless requires tcp/ws, trojan excludes grpc,
    // hysteria only matches the raw `hysteria` type at version 2 (there is no
    // `hysteria2` case), and anytls/vmess/shadowsocks are unconditional.
    match server_protocol(server).as_str() {
        "shadowsocks" => build_loon_shadowsocks(uuid, server),
        "vmess" => build_loon_vmess(uuid, server),
        "vless"
            if matches!(
                extra_string(server, "network").as_deref(),
                Some("tcp") | Some("ws")
            ) =>
        {
            build_loon_vless(uuid, server)
        }
        "trojan" if extra_string(server, "network").as_deref() != Some("grpc") => {
            build_loon_trojan(uuid, server)
        }
        "hysteria" if extra_i64(server, "version") == Some(2) => build_loon_hysteria(uuid, server),
        "anytls" => build_loon_anytls(uuid, server),
        _ => None,
    }
}

// Loon.php:49-83. All shadowsocks are eligible (no cipher filter). The http-obfs
// block mirrors ServerService::getAvailableShadowsocks flattening obfs_settings
// into obfs-host/obfs-path (ServerService.php:161-164).
fn build_loon_shadowsocks(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    let mut config = vec![
        format!("{}=Shadowsocks", server.name),
        server.host.clone(),
        first_port(server),
        cipher,
        shadowsocks_password(uuid, server)?,
    ];
    if extra_string(server, "obfs").as_deref() == Some("http") {
        config.push("obfs-name=http".to_string());
        let obfs_settings = extra_json(server, "obfs_settings");
        insert_opt_part(
            &mut config,
            "obfs-host",
            json_path_string(&obfs_settings, &["host"]),
        );
        // obfs-uri is `isset()`-gated only (no !empty), so an explicit empty path
        // still emits `obfs-uri=`.
        if let Some(path) = json_path_string(&obfs_settings, &["path"]) {
            config.push(format!("obfs-uri={path}"));
        }
    }
    config.push("fast-open=false".to_string());
    config.push("udp=true".to_string());
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:85-135. vmess reads the legacy camelCase tlsSettings inner keys
// (allowInsecure/serverName) and the networkSettings `security`; block order is
// base, TCP transport, TLS, WS transport.
fn build_loon_vmess(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_default();
    let network_settings = extra_json(server, "network_settings");
    let tls_settings = extra_json(server, "tls_settings");
    let security =
        json_path_string(&network_settings, &["security"]).unwrap_or_else(|| "auto".to_string());
    let mut config = vec![
        format!("{}=vmess", server.name),
        server.host.clone(),
        first_port(server),
        security,
        uuid.to_string(),
        "fast-open=false".to_string(),
        "udp=true".to_string(),
        "alterId=0".to_string(),
    ];
    if network == "tcp" {
        // header.type == 'http' rewrites transport=tcp to transport=http.
        let transport = json_path_string(&network_settings, &["header", "type"])
            .filter(|value| value == "http")
            .unwrap_or_else(|| "tcp".to_string());
        config.push(format!("transport={transport}"));
        insert_opt_part(
            &mut config,
            "path",
            json_path_first_string(&network_settings, &["header", "request", "path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_first_string(&network_settings, &["header", "request", "headers", "Host"]),
        );
    }
    if extra_i64(server, "tls").unwrap_or_default() != 0 {
        config.push("over-tls=true".to_string());
        // The !empty() guard means the emitted value is always true.
        if json_path_i64(&tls_settings, &["allowInsecure"]).unwrap_or_default() != 0 {
            config.push("skip-cert-verify=true".to_string());
        }
        insert_opt_part(
            &mut config,
            "tls-name",
            json_path_string(&tls_settings, &["serverName"]),
        );
    }
    if network == "ws" {
        config.push("transport=ws".to_string());
        insert_opt_part(
            &mut config,
            "path",
            json_path_string(&network_settings, &["path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_string(&network_settings, &["headers", "Host"]),
        );
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:137-199. vless uses snake_case tls_settings and strict tls === 1/2;
// `flow` is emitted (raw, even empty) inside each TLS branch.
fn build_loon_vless(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_default();
    let network_settings = extra_json(server, "network_settings");
    let tls_settings = extra_json(server, "tls_settings");
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let mut config = vec![
        format!("{}=vless", server.name),
        server.host.clone(),
        first_port(server),
        uuid.to_string(),
        "fast-open=false".to_string(),
        "udp=true".to_string(),
        "alterId=0".to_string(),
    ];
    if network == "tcp" {
        let transport = json_path_string(&network_settings, &["header", "type"])
            .filter(|value| value == "http")
            .unwrap_or_else(|| "tcp".to_string());
        config.push(format!("transport={transport}"));
        insert_opt_part(
            &mut config,
            "path",
            json_path_first_string(&network_settings, &["header", "request", "path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_first_string(&network_settings, &["header", "request", "headers", "Host"]),
        );
    }
    if tls == 1 {
        config.push("over-tls=true".to_string());
        config.push(format!(
            "flow={}",
            extra_string(server, "flow").unwrap_or_default()
        ));
        if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() != 0 {
            config.push("skip-cert-verify=true".to_string());
        }
        insert_opt_part(
            &mut config,
            "tls-name",
            json_path_string(&tls_settings, &["server_name"]),
        );
    } else if tls == 2 {
        config.push(format!(
            "flow={}",
            extra_string(server, "flow").unwrap_or_default()
        ));
        insert_opt_part(
            &mut config,
            "public-key",
            json_path_string(&tls_settings, &["public_key"]),
        );
        insert_opt_part(
            &mut config,
            "short-id",
            json_path_string(&tls_settings, &["short_id"]),
        );
        insert_opt_part(
            &mut config,
            "sni",
            json_path_string(&tls_settings, &["server_name"]),
        );
        if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() != 0 {
            config.push("skip-cert-verify=true".to_string());
        }
    }
    if network == "ws" {
        config.push("transport=ws".to_string());
        insert_opt_part(
            &mut config,
            "path",
            json_path_string(&network_settings, &["path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_string(&network_settings, &["headers", "Host"]),
        );
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:201-229. tls-name is positional (before fast-open) and array_filter
// drops it when server_name is empty; adds the ws block after skip-cert-verify.
fn build_loon_trojan(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let mut config = vec![
        format!("{}=trojan", server.name),
        server.host.clone(),
        first_port(server),
        uuid.to_string(),
    ];
    if let Some(server_name) = extra_string(server, "server_name").filter(|value| !value.is_empty())
    {
        config.push(format!("tls-name={server_name}"));
    }
    config.push("fast-open=false".to_string());
    config.push("udp=true".to_string());
    if matches!(extra_i64(server, "allow_insecure"), Some(value) if value != 0) {
        config.push("skip-cert-verify=true".to_string());
    }
    if extra_string(server, "network").as_deref() == Some("ws") {
        config.push("ws=true".to_string());
        let network_settings = extra_json(server, "network_settings");
        insert_opt_part(
            &mut config,
            "ws-path",
            json_path_string(&network_settings, &["path"]),
        );
        if let Some(host) =
            json_path_string(&network_settings, &["headers", "Host"]).filter(|v| !v.is_empty())
        {
            config.push(format!("ws-headers=Host:{host}"));
        }
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:231-262. hysteria2 only (dispatched on raw type `hysteria` v2). sni is
// positional (before udp); salamander-password is gated on isset(obfs).
fn build_loon_hysteria(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let mut config = vec![
        format!("{}=hysteria2", server.name),
        server.host.clone(),
        first_port(server),
        format!("password={uuid}"),
        format!(
            "download-bandwidth={}",
            extra_i64(server, "up_mbps").unwrap_or_default()
        ),
    ];
    if let Some(server_name) = extra_string(server, "server_name").filter(|value| !value.is_empty())
    {
        config.push(format!("sni={server_name}"));
    }
    config.push("udp=true".to_string());
    if matches!(extra_i64(server, "insecure"), Some(value) if value != 0) {
        config.push("skip-cert-verify=true".to_string());
    }
    if extra_string(server, "obfs").is_some() {
        config.push(format!(
            "salamander-password={}",
            extra_string(server, "obfs_password").unwrap_or_default()
        ));
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:264-284. skip-cert-verify is always emitted (true/false); sni follows
// udp and coalesces server_name then tls_settings.server_name.
fn build_loon_anytls(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut config = vec![
        format!("{}=anytls", server.name),
        server.host.clone(),
        first_port(server),
        uuid.to_string(),
        "udp=true".to_string(),
    ];
    if let Some(sni) = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
        .filter(|value| !value.is_empty())
    {
        config.push(format!("sni={sni}"));
    }
    let insecure = extra_i64(server, "insecure")
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .unwrap_or_default()
        != 0;
    config.push(format!(
        "skip-cert-verify={}",
        if insecure { "true" } else { "false" }
    ));
    Some(format!("{}\r\n", config.join(",")))
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
    // QuantumultX.php only handles ss/vmess/vless/trojan — there is no anytls
    // (nor hysteria) case, so those protocols emit nothing.
    match server_protocol(server).as_str() {
        "shadowsocks" => build_quantumultx_shadowsocks(uuid, server),
        "vmess" => Some(build_quantumultx_vmess(uuid, server)),
        "vless" => build_quantumultx_vless(uuid, server),
        "trojan" => Some(build_quantumultx_trojan(uuid, server)),
        _ => None,
    }
}

// QuantumultX.php:62-117. Laravel first standardizes a legacy `v2_server_shadowsocks`
// row (which carries an `obfs` column) into the v2node shape: `network = obfs` and
// `obfs_settings.host/path` are folded into `network_settings.headers.Host` / `.path`.
// It then reads `network = server['network'] ?? 'tcp'` and, when it is `http`, emits the
// http obfs transport (`obfs=http` plus obfs-host/obfs-uri). A v2node ss node reaches the
// http case via its own `network` column with an empty `obfs`, so gating on the `obfs`
// column alone (as before) dropped obfs for those nodes.
fn build_quantumultx_shadowsocks(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    let password = shadowsocks_password(uuid, server)?;
    let mut config = vec![
        format!("shadowsocks={}:{}", server.host, first_port(server)),
        format!("method={cipher}"),
        format!("password={password}"),
    ];

    let obfs = extra_string(server, "obfs").filter(|value| !value.is_empty());
    let network = match &obfs {
        // Legacy node: `network = obfs` (QuantumultX.php:66).
        Some(obfs) => obfs.clone(),
        // v2node: the `network` column, defaulting to tcp (QuantumultX.php:95).
        None => extra_string(server, "network").unwrap_or_else(|| "tcp".to_string()),
    };
    if network == "http" {
        config.push("obfs=http".to_string());
        let net_settings = extra_json(server, "network_settings");
        // network_settings host/path (`headers.Host ?? Host`, `path`).
        let net_host = json_path_string(&net_settings, &["headers", "Host"])
            .or_else(|| json_path_string(&net_settings, &["Host"]));
        let net_path = json_path_string(&net_settings, &["path"]);
        // Legacy obfs_settings.host/path take precedence when present (the fold at :70-74).
        let (host, path) = if obfs.is_some() {
            let obfs_settings = extra_json(server, "obfs_settings");
            (
                json_path_string(&obfs_settings, &["host"])
                    .filter(|value| !value.is_empty())
                    .or(net_host),
                json_path_string(&obfs_settings, &["path"])
                    .filter(|value| !value.is_empty())
                    .or(net_path),
            )
        } else {
            (net_host, net_path)
        };
        if let Some(host) = host.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-host={host}"));
        }
        if let Some(path) = path.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-uri={path}"));
        }
    }
    config.push("fast-open=false".to_string());
    config.push("udp-relay=true".to_string());
    config.push(format!("tag={}", server.name));
    Some(format!("{}\r\n", config.join(",")))
}

// QuantumultX.php:119-219
fn build_quantumultx_vmess(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> String {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let net_settings = extra_json(server, "network_settings");
    let is_tls = extra_i64(server, "tls").unwrap_or_default() != 0;
    let mut config = vec![
        format!("vmess={}:{}", server.host, first_port(server)),
        "method=chacha20-poly1305".to_string(),
        format!("password={uuid}"),
        "fast-open=true".to_string(),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    // WS with an explicit non-auto security overrides the method (QX has no auto).
    if network == "ws"
        && let Some(security) =
            json_path_string(&net_settings, &["security"]).filter(|value| value != "auto")
        && let Some(slot) = config.iter_mut().find(|value| value.starts_with("method="))
    {
        *slot = format!("method={security}");
    }
    let allow_insecure = json_path_i64(&tls_settings, &["allow_insecure"])
        .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        != 0;
    if is_tls {
        config.push("tls13=true".to_string());
        if allow_insecure {
            config.push("tls-verification=false".to_string());
        }
    }
    push_quantumultx_transport(&mut config, &network, &net_settings, is_tls);
    let sni = json_path_string(&tls_settings, &["server_name"])
        .or_else(|| json_path_string(&tls_settings, &["serverName"]));
    push_quantumultx_host_path(&mut config, &network, &net_settings, is_tls, sni);
    format!("{}\r\n", config.join(","))
}

// QuantumultX.php:221-311
fn build_quantumultx_vless(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let is_tls = tls != 0;
    let tls_settings = extra_json(server, "tls_settings");
    let net_settings = extra_json(server, "network_settings");
    let mut config = vec![
        format!("vless={}:{}", server.host, first_port(server)),
        "method=none".to_string(),
        format!("password={uuid}"),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    // REALITY disables fast-open; everything else enables it.
    config.push(
        if tls == 2 {
            "fast-open=false"
        } else {
            "fast-open=true"
        }
        .to_string(),
    );
    // QX cannot express VLESS encryption: skip the node entirely when both the
    // encryption and its settings are present (:235-238).
    let encryption = extra_string(server, "encryption").unwrap_or_default();
    if !encryption.is_empty() && value_is_non_empty(&extra_json(server, "encryption_settings")) {
        return None;
    }
    if is_tls {
        config.push("tls13=true".to_string());
        if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() != 0 {
            config.push("tls-verification=false".to_string());
        }
        if let Some(flow) = extra_string(server, "flow").filter(|value| !value.is_empty()) {
            config.push(format!("vless-flow={flow}"));
        }
        if tls == 2 {
            if let Some(public_key) = json_path_string(&tls_settings, &["public_key"]) {
                config.push(format!("reality-base64-pubkey={public_key}"));
            }
            if let Some(short_id) = json_path_string(&tls_settings, &["short_id"]) {
                config.push(format!("reality-hex-shortid={short_id}"));
            }
        }
    }
    push_quantumultx_transport(&mut config, &network, &net_settings, is_tls);
    let sni = json_path_string(&tls_settings, &["server_name"]);
    push_quantumultx_host_path(&mut config, &network, &net_settings, is_tls, sni);
    Some(format!("{}\r\n", config.join(",")))
}

// QuantumultX.php:313-378
fn build_quantumultx_trojan(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> String {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let net_settings = extra_json(server, "network_settings");
    let sni = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]));
    let allow_insecure = extra_i64(server, "allow_insecure")
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .unwrap_or_default()
        != 0;
    let mut config = vec![
        format!("trojan={}:{}", server.host, first_port(server)),
        format!("password={uuid}"),
        "fast-open=true".to_string(),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    if network == "tcp" {
        config.push("over-tls=true".to_string());
        if let Some(sni) = sni.as_deref().filter(|value| !value.is_empty()) {
            config.push(format!("tls-host={sni}"));
        }
        config.push(format!(
            "tls-verification={}",
            if allow_insecure { "false" } else { "true" }
        ));
    }
    if network == "ws" {
        config.push("obfs=wss".to_string());
        let mut host = json_path_string(&net_settings, &["headers", "Host"]);
        let path = json_path_string(&net_settings, &["path"]);
        if host.as_deref().map(str::is_empty).unwrap_or(true) {
            host = sni.filter(|value| !value.is_empty());
        }
        if let Some(host) = host.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-host={host}"));
        }
        if let Some(path) = path.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-uri={path}"));
        }
        if allow_insecure {
            config.push("tls-verification=false".to_string());
        }
    }
    format!("{}\r\n", config.join(","))
}

// Shared vmess/vless `obfs=` transport tag selection (QuantumultX.php).
fn push_quantumultx_transport(
    config: &mut Vec<String>,
    network: &str,
    net_settings: &Value,
    is_tls: bool,
) {
    if network == "ws" {
        config.push(if is_tls { "obfs=wss" } else { "obfs=ws" }.to_string());
    } else if network == "tcp" {
        if is_tls {
            config.push("obfs=over-tls".to_string());
        } else if json_path_string(net_settings, &["header", "type"]).as_deref() == Some("http") {
            config.push("obfs=http".to_string());
        }
    }
}

// Shared vmess/vless obfs-host / obfs-uri emission, falling back to the SNI host.
fn push_quantumultx_host_path(
    config: &mut Vec<String>,
    network: &str,
    net_settings: &Value,
    _is_tls: bool,
    sni: Option<String>,
) {
    let (mut host, path) = match network {
        "tcp" if json_path_string(net_settings, &["header", "type"]).as_deref() == Some("http") => {
            (
                json_path_first_string(net_settings, &["header", "request", "headers", "Host"]),
                json_path_first_string(net_settings, &["header", "request", "path"]),
            )
        }
        "ws" => (
            json_path_string(net_settings, &["headers", "Host"]),
            json_path_string(net_settings, &["path"]),
        ),
        _ => (None, None),
    };
    if host.as_deref().map(str::is_empty).unwrap_or(true) {
        host = sni.filter(|value| !value.is_empty());
    }
    if let Some(host) = host.filter(|value| !value.is_empty()) {
        config.push(format!("obfs-host={host}"));
    }
    if let Some(path) = path.filter(|value| !value.is_empty()) {
        config.push(format!("obfs-uri={path}"));
    }
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

// insecure/server_name resolution shared by every sing-box TLS block, checking
// both v2node (`allow_insecure`/`server_name`) and legacy (`allowInsecure`/
// `serverName`) key spellings plus the outer server columns.
fn singbox_insecure(server: &v2board_db::server::AvailableServerRow, tls_settings: &Value) -> bool {
    extra_i64(server, "insecure")
        .or_else(|| extra_i64(server, "allow_insecure"))
        .or_else(|| json_path_i64(tls_settings, &["allow_insecure"]))
        .or_else(|| json_path_i64(tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        == 1
}

fn singbox_server_name(
    server: &v2board_db::server::AvailableServerRow,
    tls_settings: &Value,
) -> String {
    extra_string(server, "server_name")
        .or_else(|| json_path_string(tls_settings, &["server_name"]))
        .or_else(|| json_path_string(tls_settings, &["serverName"]))
        .unwrap_or_default()
}

fn singbox_tls(
    server: &v2board_db::server::AvailableServerRow,
    tls_settings: &Value,
    tls_mode: i64,
    utls: bool,
    // Only vmess/vless/trojan on modern sing-box emit ECH; legacy sing-box and
    // tuic/anytls/hysteria2 never do.
    include_ech: bool,
) -> Value {
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        Value::Bool(singbox_insecure(server, tls_settings)),
    );
    tls.insert(
        "server_name".to_string(),
        Value::String(singbox_server_name(server, tls_settings)),
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
    if include_ech {
        add_singbox_ech(&mut tls, tls_settings);
    }
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

// Modern sing-box hysteria port logic (Singbox.php:422-453): a lone single port
// stays `server_port`; anything else becomes `server_ports` with the range
// entries only (bare single ports inside a comma list are discarded).
fn set_singbox_hysteria_ports(
    object: &mut Map<String, Value>,
    server: &v2board_db::server::AvailableServerRow,
) {
    let raw = port_text(server);
    let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() == 1 && !parts[0].contains('-') {
        object.insert(
            "server_port".to_string(),
            Value::from(parts[0].parse::<i64>().unwrap_or_default()),
        );
    } else {
        object.remove("server_port");
        let ranges = parts
            .iter()
            .filter(|part| part.contains('-'))
            .map(|part| part.replace('-', ":"))
            .collect::<Vec<_>>();
        object.insert("server_ports".to_string(), json!(ranges));
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

const KIB: f64 = 1024.0;
const MIB: f64 = 1_048_576.0;
const GIB: f64 = 1_073_741_824.0;

// Round half-away-from-zero to 2 decimals (PHP `round($x, 2)` default mode).
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

// Format a rounded value the way PHP prints a float: trailing zeros and a bare
// decimal point are dropped (e.g. 1.0 -> "1", 1.50 -> "1.5", 1.05 -> "1.05").
fn php_round2(value: f64) -> String {
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

fn format_datetime_timestamp(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
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
        // Laravel Helper.php:215 emits `path={$server['obfs-path']}` where obfs-path is the
        // raw obfs_settings.path (ServerService.php:164, no default), so an unset path renders
        // an empty `path=` — not "/".
        let path = json_path_string(&obfs_settings, &["path"]).unwrap_or_default();
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
    // PHP json_encode preserves INSERTION order (v,ps,add,port,id,...) and escapes
    // `/` and non-ASCII; serde's BTreeMap would sort keys and skip those escapes.
    // Build an ordered list and serialise it PHP-compatibly (Helper.php:223-289).
    let mut config: Vec<(String, Value)> = Vec::new();
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
    let payload = php_json_encode_object(&config);
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
    config: &mut Vec<(String, serde_json::Value)>,
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
        // Laravel Helper.php:391 concatenates "&obfsParam{$obfs_password}" with no '='
        // separator (a v1 quirk); match it byte-for-byte for the externally-consumed URI.
        uri.push_str("&obfsParam");
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

// Ordered-map upsert: replace an existing key in place (PHP array assignment
// keeps insertion position) or append a new one.
fn vmess_set(config: &mut Vec<(String, serde_json::Value)>, key: &str, value: serde_json::Value) {
    if let Some(entry) = config.iter_mut().find(|(existing, _)| existing == key) {
        entry.1 = value;
    } else {
        config.push((key.to_string(), value));
    }
}

fn json_insert_str(config: &mut Vec<(String, serde_json::Value)>, key: &str, value: &str) {
    vmess_set(config, key, serde_json::Value::String(value.to_string()));
}

fn json_insert_i64(config: &mut Vec<(String, serde_json::Value)>, key: &str, value: i64) {
    vmess_set(config, key, serde_json::Value::from(value));
}

// PHP-compatible json_encode for the vmess payload: ordered keys, `/` escaped as
// `\/`, and non-ASCII emitted as lowercase `\uXXXX` (UTF-16, surrogate pairs for
// astral code points) — matching PHP's default flags (Helper.php:289).
fn php_json_encode_object(entries: &[(String, serde_json::Value)]) -> String {
    let mut out = String::from("{");
    for (index, (key, value)) in entries.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        php_json_encode_string(&mut out, key);
        out.push(':');
        php_json_encode_value(&mut out, value);
    }
    out.push('}');
    out
}

fn php_json_encode_value(out: &mut String, value: &serde_json::Value) {
    match value {
        serde_json::Value::String(text) => php_json_encode_string(out, text),
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        serde_json::Value::Number(number) => out.push_str(&number.to_string()),
        other => out.push_str(&serde_json::to_string(other).unwrap_or_default()),
    }
}

fn php_json_encode_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            ascii if (ascii as u32) < 0x80 => out.push(ascii),
            other => {
                let mut units = [0u16; 2];
                for unit in other.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
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

// Legacy v2 transport settings store Host/path as arrays; read the first element
// if the target is an array, otherwise stringify the scalar (QuantumultX.php uses
// `$header['request']['path'][0]` etc.).
fn json_path_first_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    match json_path_value(value, path) {
        Some(serde_json::Value::Array(items)) => items.first().and_then(value_to_string),
        other => other.and_then(value_to_string),
    }
}

// PHP `!empty()` for a JSON value: null / empty string / empty array / empty
// object / 0 count as empty.
fn value_is_non_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::String(text) => !text.is_empty() && text != "0",
        serde_json::Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(true),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
    }
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
    fn quantumultx_skips_anytls() {
        // QuantumultX.php has no anytls case (it only handles ss/vmess/vless/
        // trojan), so anytls servers must emit nothing.
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

        assert!(build_quantumultx_proxy("pwd", &server).is_none());
    }

    fn server_row(
        kind: &str,
        port: serde_json::Value,
        extra: serde_json::Value,
    ) -> v2board_db::server::AvailableServerRow {
        v2board_db::server::AvailableServerRow {
            id: 1,
            parent_id: None,
            group_id: vec![1],
            route_id: None,
            name: "node".to_string(),
            rate: "1".to_string(),
            r#type: kind.to_string(),
            host: "example.com".to_string(),
            port,
            cache_key: "k".to_string(),
            last_check_at: None,
            is_online: 0,
            tags: None,
            sort: None,
            extra,
        }
    }

    #[test]
    fn singbox_tuic_modern_adds_domain_resolver_and_alpn() {
        let server = server_row(
            "tuic",
            json!(443),
            json!({ "server_name": "sni.example", "insecure": 1, "disable_sni": 1 }),
        );
        let modern = build_singbox_proxy("uuid", &server, true).unwrap();
        assert_eq!(modern["domain_resolver"], json!("local"));
        assert_eq!(modern["tls"]["alpn"], json!(["h3"]));
        assert_eq!(modern["tls"]["disable_sni"], json!(true));
        assert_eq!(modern["tls"]["insecure"], json!(true));
        assert!(modern["tls"].get("ech").is_none());

        let legacy = build_singbox_proxy("uuid", &server, false).unwrap();
        assert!(legacy.get("domain_resolver").is_none());
        assert_eq!(legacy["tls"]["alpn"], json!(["h3"]));
    }

    #[test]
    fn singbox_anytls_is_modern_only_with_h2_alpn() {
        let server = server_row(
            "anytls",
            json!(443),
            json!({
                "network": "tcp",
                "tls": 1,
                "tls_settings": { "server_name": "sni.example", "allow_insecure": 1 }
            }),
        );
        let modern = build_singbox_proxy("uuid", &server, true).unwrap();
        assert_eq!(modern["type"], json!("anytls"));
        assert_eq!(modern["tls"]["alpn"], json!(["h2", "http/1.1"]));
        assert_eq!(modern["domain_resolver"], json!("local"));
        assert!(modern["tls"].get("ech").is_none());
        assert!(modern["tls"].get("utls").is_some());
        // Legacy sing-box has no anytls builder.
        assert!(build_singbox_proxy("uuid", &server, false).is_none());
    }

    #[test]
    fn singbox_hysteria_v1_swaps_mbps_and_gates_ports() {
        let server = server_row(
            "hysteria",
            json!("20000-50000"),
            json!({
                "version": 1, "up_mbps": 100, "down_mbps": 200,
                "server_name": "sni", "insecure": 1,
                "obfs": "salamander", "obfs_password": "pw"
            }),
        );
        let modern = build_singbox_proxy("uuid", &server, true).unwrap();
        assert_eq!(modern["type"], json!("hysteria"));
        assert_eq!(modern["disable_mtu_discovery"], json!(true));
        assert_eq!(modern["up_mbps"], json!(200));
        assert_eq!(modern["down_mbps"], json!(100));
        assert_eq!(modern["obfs"], json!("pw"));
        assert_eq!(modern["server_ports"], json!(["20000:50000"]));
        assert!(modern.get("server_port").is_none());
        assert_eq!(modern["domain_resolver"], json!("local"));

        let legacy = build_singbox_proxy("uuid", &server, false).unwrap();
        assert!(legacy.get("domain_resolver").is_none());
        assert!(legacy.get("server_ports").is_none());
        assert_eq!(legacy["server_port"], json!(20000));
        assert_eq!(legacy["disable_mtu_discovery"], json!(true));
    }

    #[test]
    fn singbox_hysteria2_uses_single_first_port_without_ech() {
        let server = server_row(
            "hysteria2",
            json!("443-500"),
            json!({
                "tls_settings": { "server_name": "sni", "allow_insecure": 1 },
                "obfs": "salamander", "obfs_password": "pw"
            }),
        );
        let out = build_singbox_proxy("uuid", &server, true).unwrap();
        assert_eq!(out["type"], json!("hysteria2"));
        assert_eq!(out["server_port"], json!(443));
        assert!(out.get("server_ports").is_none());
        assert_eq!(out["obfs"]["type"], json!("salamander"));
        assert!(out["tls"].get("ech").is_none());
    }

    #[test]
    fn singbox_vmess_ech_only_on_modern() {
        let server = server_row(
            "vmess",
            json!(443),
            json!({
                "network": "tcp", "tls": 1,
                "tls_settings": { "server_name": "sni", "allow_insecure": 1, "ech": "cloudflare" }
            }),
        );
        let modern = build_singbox_proxy("uuid", &server, true).unwrap();
        assert!(modern["tls"].get("ech").is_some());
        assert_eq!(modern["domain_resolver"], json!("local"));

        let legacy = build_singbox_proxy("uuid", &server, false).unwrap();
        assert!(legacy["tls"].get("ech").is_none());
        assert!(legacy.get("domain_resolver").is_none());
    }

    #[test]
    fn loon_vmess_uses_network_security_and_tls_before_ws() {
        let server = server_row(
            "vmess",
            json!(443),
            json!({
                "network": "ws", "tls": 1,
                "network_settings": {
                    "security": "chacha20", "path": "/ws", "headers": { "Host": "h.example" }
                },
                "tls_settings": { "allowInsecure": 1, "serverName": "sni.example" }
            }),
        );
        let line = build_loon_proxy("uuid", &server).unwrap();
        assert!(line.contains("=vmess,example.com,443,chacha20,uuid,"));
        let tls_pos = line.find("over-tls=true").unwrap();
        let ws_pos = line.find("transport=ws").unwrap();
        assert!(tls_pos < ws_pos);
        assert!(line.contains("skip-cert-verify=true"));
        assert!(line.contains("tls-name=sni.example"));
        assert!(line.contains("path=/ws"));
        assert!(line.contains("host=h.example"));
    }

    #[test]
    fn loon_vless_emits_flow_inside_reality_branch() {
        let server = server_row(
            "vless",
            json!(443),
            json!({
                "network": "tcp", "tls": 2, "flow": "xtls-rprx-vision",
                "tls_settings": {
                    "public_key": "PK", "short_id": "SID",
                    "server_name": "sni", "allow_insecure": 1
                }
            }),
        );
        let line = build_loon_proxy("uuid", &server).unwrap();
        assert!(line.contains("flow=xtls-rprx-vision"));
        assert!(line.contains("public-key=PK"));
        assert!(line.contains("short-id=SID"));
        assert!(line.contains("sni=sni"));
        assert!(line.contains("skip-cert-verify=true"));
        assert!(!line.contains("over-tls=true"));
    }

    #[test]
    fn loon_trojan_tls_name_positional_with_ws_block() {
        let server = server_row(
            "trojan",
            json!(443),
            json!({
                "server_name": "sni.example", "allow_insecure": 1, "network": "ws",
                "network_settings": { "path": "/p", "headers": { "Host": "h" } }
            }),
        );
        let line = build_loon_proxy("uuid", &server).unwrap();
        let name_pos = line.find("tls-name=sni.example").unwrap();
        let fo_pos = line.find("fast-open=false").unwrap();
        assert!(name_pos < fo_pos);
        assert!(line.contains("skip-cert-verify=true"));
        assert!(line.contains("ws=true"));
        assert!(line.contains("ws-path=/p"));
        assert!(line.contains("ws-headers=Host:h"));
    }

    #[test]
    fn loon_hysteria_sni_precedes_udp() {
        let server = server_row(
            "hysteria",
            json!("20000-50000"),
            json!({
                "version": 2, "up_mbps": 100, "server_name": "sni.example",
                "insecure": 1, "obfs": "salamander", "obfs_password": "pw"
            }),
        );
        let line = build_loon_proxy("uuid", &server).unwrap();
        assert!(
            line.contains("=hysteria2,example.com,20000,password=uuid,download-bandwidth=100,")
        );
        let sni_pos = line.find("sni=sni.example").unwrap();
        let udp_pos = line.find("udp=true").unwrap();
        assert!(sni_pos < udp_pos);
        assert!(line.contains("skip-cert-verify=true"));
        assert!(line.contains("salamander-password=pw"));
    }

    #[test]
    fn loon_anytls_always_emits_skip_cert_verify() {
        let server = server_row(
            "anytls",
            json!(443),
            json!({
                "server_name": "sni.example", "insecure": 0,
                "tls_settings": { "server_name": "ts.sni", "allow_insecure": 0 }
            }),
        );
        let line = build_loon_proxy("uuid", &server).unwrap();
        assert!(line.contains("sni=sni.example"));
        assert!(line.contains("skip-cert-verify=false"));
    }

    #[test]
    fn loon_skips_raw_hysteria2_type() {
        let server = server_row(
            "hysteria2",
            json!(443),
            json!({ "version": 2, "up_mbps": 100 }),
        );
        assert!(build_loon_proxy("uuid", &server).is_none());
    }

    #[test]
    fn detect_v2raytun_is_its_own_format() {
        assert_eq!(
            SubscriptionFormat::detect("v2raytun"),
            SubscriptionFormat::V2RayTun
        );
        assert_eq!(
            SubscriptionFormat::detect("V2rayTun/1.0"),
            SubscriptionFormat::V2RayTun
        );
        // V2rayN/NG and the other base64 clients keep the shared Base64Uri format.
        assert_eq!(
            SubscriptionFormat::detect("v2rayng"),
            SubscriptionFormat::Base64Uri
        );
        assert_eq!(
            SubscriptionFormat::detect("v2rayn"),
            SubscriptionFormat::Base64Uri
        );
    }

    #[test]
    fn quantumultx_shadowsocks_emits_http_obfs_fields() {
        // QuantumultX.php:97-106 emits obfs=http + obfs-host/obfs-uri for an http
        // obfs shadowsocks node, ahead of the trailing fast-open/udp-relay/tag.
        let server = server_row(
            "shadowsocks",
            json!(8388),
            json!({
                "cipher": "aes-128-gcm",
                "obfs": "http",
                "obfs_settings": { "host": "bing.com", "path": "/ray" }
            }),
        );
        let line = build_quantumultx_proxy("pwd", &server).unwrap();
        assert!(line.starts_with("shadowsocks=example.com:8388,method=aes-128-gcm,password=pwd,"));
        assert!(line.contains(",obfs=http,"));
        assert!(line.contains("obfs-host=bing.com"));
        assert!(line.contains("obfs-uri=/ray"));
        let obfs_pos = line.find("obfs=http").unwrap();
        let fast_open_pos = line.find("fast-open=false").unwrap();
        assert!(obfs_pos < fast_open_pos);
        assert!(line.trim_end().ends_with("tag=node"));
    }

    #[test]
    fn quantumultx_shadowsocks_without_obfs_has_no_transport() {
        let server = server_row(
            "shadowsocks",
            json!(8388),
            json!({ "cipher": "aes-256-gcm" }),
        );
        let line = build_quantumultx_proxy("pwd", &server).unwrap();
        assert!(!line.contains("obfs="));
        assert_eq!(
            line,
            "shadowsocks=example.com:8388,method=aes-256-gcm,password=pwd,fast-open=false,udp-relay=true,tag=node\r\n"
        );
    }

    #[test]
    fn clash_shadowsocks_cipher_filter_is_meta_only() {
        // Clash.php:43-49 only builds ss for the four basic ciphers; Meta/Stash
        // (meta=true) accept every cipher, including ss2022.
        let basic = server_row(
            "shadowsocks",
            json!(8388),
            json!({ "cipher": "aes-128-gcm" }),
        );
        let ss2022 = server_row(
            "shadowsocks",
            json!(8388),
            json!({ "cipher": "2022-blake3-aes-128-gcm", "created_at": "1700000000" }),
        );
        assert!(build_clash_proxy("uuid", &basic, false).is_some());
        assert!(build_clash_proxy("uuid", &ss2022, false).is_none());
        assert!(build_clash_proxy("uuid", &basic, true).is_some());
        assert!(build_clash_proxy("uuid", &ss2022, true).is_some());
    }
}
