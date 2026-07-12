use chrono::TimeZone;
use serde_json::{Map, Value, json};
use v2board_compat::ApiError;
use v2board_config::{AppConfig, app_now, app_timezone};

use super::codec::{percent_encode, prefix_bytes, safe_base64_encode, standard_base64_encode};
use super::json_value::{value_to_i64, value_to_string};

mod clash;
mod shared;
mod singbox;
mod surge_family;
mod uri;

use self::clash::build_clash_subscription;
use self::shared::*;
use self::singbox::build_singbox_subscription;
use self::surge_family::{
    build_loon_subscription, build_quantumultx_subscription, build_surfboard_subscription,
    build_surge_subscription,
};
use self::uri::{
    build_base64_uri_subscription, build_general_subscription, build_sagernet_subscription,
    build_shadowrocket_subscription, build_shadowsocks_sip008_subscription,
};

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

pub(super) async fn build_subscription_document(
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
            build_clash_subscription(config, &user.uuid, servers, ClashKind::Clash, host).await?
        }
        SubscriptionFormat::ClashMeta => {
            build_clash_subscription(config, &user.uuid, servers, ClashKind::Meta, host).await?
        }
        SubscriptionFormat::Stash => {
            build_clash_subscription(config, &user.uuid, servers, ClashKind::Stash, host).await?
        }
        SubscriptionFormat::SingBox => {
            build_singbox_subscription(config, &user.uuid, servers, true).await?
        }
        SubscriptionFormat::SingBoxLegacy => {
            build_singbox_subscription(config, &user.uuid, servers, false).await?
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

pub(super) async fn build_client_app_config(
    config: &AppConfig,
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> Result<String, ApiError> {
    clash::build_client_app_config(config, uuid, servers).await
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
    let use_traffic = i128::from(user.u) + i128::from(user.d);
    let remaining = traffic_convert(i128::from(user.transfer_enable) - use_traffic);
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

#[cfg(test)]
mod tests;
