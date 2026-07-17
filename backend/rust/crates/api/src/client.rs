use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};

use crate::{
    codec::standard_base64_encode,
    request_params::payment_request_input,
    runtime::AppState,
    subscription,
    telegram::send_telegram_message_with_admin,
    user::{reset_day, resolve_subscribe_token, user_is_available},
    validation::forbidden,
};

#[derive(Debug, Serialize)]
pub(crate) struct GuestConfig {
    tos_url: Option<String>,
    is_email_verify: i32,
    is_invite_force: i32,
    email_whitelist_suffix: serde_json::Value,
    is_recaptcha: i32,
    recaptcha_site_key: Option<String>,
    app_description: Option<String>,
    app_url: Option<String>,
    logo: Option<String>,
}

pub(crate) async fn guest_config(
    State(state): State<AppState>,
) -> Json<LegacyEnvelope<GuestConfig>> {
    let config = state.config_snapshot();
    let email_whitelist_suffix = if config.email_whitelist_enable {
        json!(config.email_whitelist_suffix)
    } else {
        json!(0)
    };

    legacy_data(GuestConfig {
        tos_url: config.tos_url.clone(),
        is_email_verify: config.email_verify as i32,
        is_invite_force: config.invite_force as i32,
        email_whitelist_suffix,
        is_recaptcha: config.recaptcha_enable as i32,
        recaptcha_site_key: config.recaptcha_site_key.clone(),
        app_description: config.app_description.clone(),
        app_url: config.app_url.clone(),
        logo: config.logo.clone(),
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClientSubscribeQuery {
    token: Option<String>,
    flag: Option<String>,
}

pub(crate) async fn client_subscribe(
    State(state): State<AppState>,
    Query(query): Query<ClientSubscribeQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    client_subscribe_response(&state, query, headers).await
}

pub(crate) async fn client_subscribe_response(
    state: &AppState,
    query: ClientSubscribeQuery,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    let token = query
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| forbidden("token is null"))?;
    let token = resolve_subscribe_token(state, token).await?;
    let user = v2board_db::user::find_user_access_by_token(&state.db, &token)
        .await?
        .ok_or_else(|| forbidden("token is error"))?;

    let mut servers = if user_is_available(&user) {
        v2board_db::server::fetch_available_servers(&state.db, user.group_id).await?
    } else {
        Vec::new()
    };
    // Prepend the show_info_to_server_enable pseudo-nodes (remaining traffic /
    // next reset / expiry). build_info_servers self-checks the config flag and
    // an empty server list, so calling it unconditionally is safe.
    let plan = match user.plan_id {
        Some(plan_id) => v2board_db::plan::find_plan(&state.db, plan_id).await?,
        None => None,
    };
    let reset = reset_day(user.expired_at, plan.as_ref(), &config).filter(|day| *day != 0);
    let info = subscription::build_info_servers(&user, &servers, reset, &config);
    if !info.is_empty() {
        let mut merged = info;
        merged.extend(servers);
        servers = merged;
    }
    let flag = query
        .flag
        .or_else(|| {
            headers
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
        .to_lowercase();

    // Request Host header for Surge/Surfboard `$subs_domain` and Stash's
    // forced-DIRECT rule (Laravel `$_SERVER['HTTP_HOST']`).
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let subscription =
        subscription::build_subscription_document(state, &config, &user, &servers, &flag, &host)
            .await?;
    let profile = subscription_header_profile(&flag);
    let mut response = subscription.body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(subscription.content_type),
    );

    use SubscriptionHeaderProfile as Profile;

    // Content-Disposition: only the protocols whose handle() sets it. v2RayTun uses a
    // plain quoted filename rather than the RFC 5987 form the shared document builds.
    let content_disposition = match profile {
        Profile::Surge | Profile::Clash | Profile::ClashMeta | Profile::SingBox => {
            Some(subscription.content_disposition)
        }
        Profile::V2RayTun => Some(format!("attachment; filename=\"{}\"", config.app_name)),
        Profile::None | Profile::QuantumultX | Profile::Loon => None,
    };
    if let Some(content_disposition) = content_disposition {
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&content_disposition)
                .map_err(|_| ApiError::internal("invalid subscription filename"))?,
        );
    }

    // subscription-userinfo. The `expire=` token differs per family: Clash/Meta/Stash/Loon
    // /Sing-box render an empty string for a null (lifetime) expired_at, QuantumultX
    // renders 0, and v2RayTun omits the token entirely (`isset($user['expired_at'])`).
    let expire_or_empty = user
        .expired_at
        .map(|value| value.to_string())
        .unwrap_or_default();
    let userinfo = match profile {
        Profile::Clash | Profile::ClashMeta | Profile::Loon | Profile::SingBox => Some(format!(
            "upload={}; download={}; total={}; expire={}",
            user.u, user.d, user.transfer_enable, expire_or_empty
        )),
        Profile::QuantumultX => Some(format!(
            "upload={}; download={}; total={}; expire={}",
            user.u,
            user.d,
            user.transfer_enable,
            user.expired_at.unwrap_or(0)
        )),
        Profile::V2RayTun => Some(match user.expired_at {
            Some(expired_at) => format!(
                "upload={}; download={}; total={}; expire={}",
                user.u, user.d, user.transfer_enable, expired_at
            ),
            None => format!(
                "upload={}; download={}; total={}",
                user.u, user.d, user.transfer_enable
            ),
        }),
        Profile::None | Profile::Surge => None,
    };
    if let Some(userinfo) = userinfo {
        headers.insert(
            header::HeaderName::from_static("subscription-userinfo"),
            HeaderValue::from_str(&userinfo)
                .map_err(|_| ApiError::internal("invalid subscription userinfo header"))?,
        );
    }

    // profile-title: Sing-box emits a base64 title; v2RayTun emits the plain app name.
    let profile_title = match profile {
        Profile::SingBox => Some(format!(
            "base64:{}",
            standard_base64_encode(config.app_name.as_bytes())
        )),
        Profile::V2RayTun => Some(config.app_name.clone()),
        _ => None,
    };
    if let Some(profile_title) = profile_title {
        headers.insert(
            header::HeaderName::from_static("profile-title"),
            HeaderValue::from_str(&profile_title)
                .map_err(|_| ApiError::internal("invalid profile title"))?,
        );
    }

    // profile-web-page-url: only base Clash.php sets it, always (even when app_url is empty).
    if profile == Profile::Clash {
        headers.insert(
            header::HeaderName::from_static("profile-web-page-url"),
            HeaderValue::from_str(config.app_url.as_deref().unwrap_or_default())
                .map_err(|_| ApiError::internal("invalid profile web page url"))?,
        );
    }

    // profile-update-interval: 24 — Clash family, Sing-box, and v2RayTun only.
    if matches!(
        profile,
        Profile::Clash | Profile::ClashMeta | Profile::SingBox | Profile::V2RayTun
    ) {
        headers.insert(
            header::HeaderName::from_static("profile-update-interval"),
            HeaderValue::from_static("24"),
        );
    }
    Ok(response)
}

/// Per-protocol subscription response header set. Laravel emits Subscription-Userinfo,
/// Content-Disposition, profile-title, profile-web-page-url, and profile-update-interval
/// differently in each `App\Protocols\*::handle()`; this mirrors exactly which each sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriptionHeaderProfile {
    /// General/V2rayN/V2rayNG/Passwall/SSRPlus/SagerNet/Shadowsocks/Shadowrocket: none.
    None,
    /// v2RayTun.php: profile-title (plain), userinfo (expire omitted when null), interval,
    /// plain-filename content-disposition.
    V2RayTun,
    /// QuantumultX.php: subscription-userinfo only, expire=0 when null.
    QuantumultX,
    /// Loon.php: subscription-userinfo only, expire empty when null.
    Loon,
    /// Surge.php / Surfboard.php: content-disposition only.
    Surge,
    /// Clash.php: userinfo + interval + content-disposition + profile-web-page-url.
    Clash,
    /// ClashMeta/ClashVerge/ClashNyanpasu/Stash: userinfo + interval + content-disposition.
    ClashMeta,
    /// Singbox + SingboxOld: userinfo + interval + base64 profile-title + content-disposition.
    SingBox,
}

/// Classify a subscribe flag into its header profile, mirroring the renderer selection in
/// `subscription::SubscriptionFormat::detect` so the headers always match the emitted body.
fn subscription_header_profile(flag: &str) -> SubscriptionHeaderProfile {
    use SubscriptionHeaderProfile as Profile;
    let normalized = flag
        .replace("%20", " ")
        .replace(['_', '-', '/'], " ")
        .to_lowercase();
    if normalized.contains("sing") {
        Profile::SingBox
    } else if normalized.contains("surfboard") || normalized.contains("surge") {
        Profile::Surge
    } else if normalized.contains("loon") {
        Profile::Loon
    } else if normalized.contains("shadowrocket")
        || normalized.contains("shadowsocks")
        || normalized.contains("sagernet")
    {
        Profile::None
    } else if normalized.contains("quantumult x") {
        // QuantumultX::$flag is the literal `quantumult%20x` (from the app's
        // `Quantumult%20X/…` UA), normalized here to `quantumult x`. Plain
        // `Quantumult/…` (the original, non-X app) does not match and falls through.
        Profile::QuantumultX
    } else if normalized.contains("v2raytun") {
        Profile::V2RayTun
    } else if normalized.contains("v2rayn")
        || normalized.contains("v2rayng")
        || normalized.contains("passwall")
        || normalized.contains("ssrplus")
    {
        Profile::None
    } else if normalized.contains("stash")
        || normalized.contains("meta")
        || normalized.contains("nyanpasu")
        || normalized.contains("verge")
    {
        Profile::ClashMeta
    } else if normalized.contains("clash") {
        Profile::Clash
    } else {
        Profile::None
    }
}

pub(crate) async fn client_app_config(
    State(state): State<AppState>,
    Query(query): Query<ClientSubscribeQuery>,
) -> Result<Response, ApiError> {
    let token = query
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| forbidden("token is null"))?;
    let token = resolve_subscribe_token(&state, token).await?;
    let user = v2board_db::user::find_user_access_by_token(&state.db, &token)
        .await?
        .ok_or_else(|| forbidden("token is error"))?;
    let servers = if user_is_available(&user) {
        v2board_db::server::fetch_available_servers(&state.db, user.group_id).await?
    } else {
        Vec::new()
    };
    let config = state.config_snapshot();
    let body = subscription::build_client_app_config(&config, &user.uuid, &servers).await?;
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/yaml; charset=utf-8"),
    );
    Ok(response)
}

pub(crate) async fn client_app_version(
    State(state): State<AppState>,
    Query(query): Query<ClientSubscribeQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<serde_json::Value>>, ApiError> {
    let token = query
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| forbidden("token is null"))?;
    let token = resolve_subscribe_token(&state, token).await?;
    let _user = v2board_db::user::find_user_access_by_token(&state.db, &token)
        .await?
        .ok_or_else(|| forbidden("token is error"))?;
    let config = state.config_snapshot();
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if ua.contains("tidalab/4.0.0") || ua.contains("tunnelab/4.0.0") {
        if ua.contains("Win64") {
            return Ok(legacy_data(json!({
                "version": config.windows_version,
                "download_url": config.windows_download_url,
            })));
        }
        return Ok(legacy_data(json!({
            "version": config.macos_version,
            "download_url": config.macos_download_url,
        })));
    }
    Ok(legacy_data(json!({
        "windows_version": config.windows_version,
        "windows_download_url": config.windows_download_url,
        "macos_version": config.macos_version,
        "macos_download_url": config.macos_download_url,
        "android_version": config.android_version,
        "android_download_url": config.android_download_url,
    })))
}

pub(crate) async fn payment_notify(
    State(state): State<AppState>,
    Path((method, uuid)): Path<(String, String)>,
    request: Request,
) -> Result<Response, ApiError> {
    let input = payment_request_input(request, &method).await?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let result = service.handle_payment_notify(&method, &uuid, input).await?;
    // Laravel `PaymentController::handle` sends the `成功收款` admin Telegram message only
    // inside the `$order->status !== 0` guard, i.e. exactly on the fresh paid transition
    // (`paid_notice` is `Some`). A gateway replay leaves it `None` and stays silent. The
    // send is best-effort: a Telegram failure must not fail an already-recorded payment.
    if let Some(notice) = &result.paid_notice
        && let Some(bot_token) = state.config_snapshot().telegram_bot_token.clone()
    {
        let message = format!(
            "💰成功收款{}元\n———————————————\n订单号：{}",
            format_paid_amount_yuan(notice.total_amount),
            notice.trade_no
        );
        if let Err(error) =
            send_telegram_message_with_admin(&state, &bot_token, &message, false).await
        {
            tracing::warn!(?error, "payment success telegram notify failed");
        }
    }
    if let Some(notice) = &result.late_payment_notice
        && let Some(bot_token) = state.config_snapshot().telegram_bot_token.clone()
    {
        let settled = notice
            .settled_amount
            .map(format_paid_amount_yuan)
            .unwrap_or_else(|| "网关未提供".to_string());
        let message = format!(
            "🚨收到需人工核对的已认证付款\n———————————————\n订单号：{}\n订单摘要：{}\n交易号：{}\n交易摘要：{}\n原因：{}\n订单状态：{}\n应付：{}元\n实付：{}元",
            notice.trade_no,
            notice.trade_no_hash,
            notice.callback_no,
            notice.callback_no_hash,
            notice.reason,
            notice.order_status,
            format_paid_amount_yuan(notice.expected_amount),
            settled,
        );
        if let Err(error) =
            send_telegram_message_with_admin(&state, &bot_token, &message, false).await
        {
            tracing::warn!(?error, "late payment reconciliation telegram notify failed");
        }
    }
    Ok(result.body.into_response())
}

/// Render an amount in cents as Laravel's `total_amount / 100` string does: the integer
/// yuan value with no decimals when it divides evenly (`1000 -> "10"`), a single decimal
/// when the cents end in a zero (`1050 -> "10.5"`), otherwise two decimals
/// (`1035 -> "10.35"`, `1 -> "0.01"`). Uses integer math to avoid float-repr drift.
fn format_paid_amount_yuan(cents: i64) -> String {
    let negative = cents < 0;
    let cents = cents.unsigned_abs();
    let yuan = cents / 100;
    let frac = cents % 100;
    let body = if frac == 0 {
        format!("{yuan}")
    } else if frac.is_multiple_of(10) {
        format!("{yuan}.{}", frac / 10)
    } else {
        format!("{yuan}.{frac:02}")
    };
    if negative { format!("-{body}") } else { body }
}

#[cfg(test)]
mod tests {
    use super::{SubscriptionHeaderProfile, format_paid_amount_yuan, subscription_header_profile};

    #[test]
    fn subscription_header_profile_matches_each_protocol() {
        use SubscriptionHeaderProfile as P;
        // base64 family (and an unknown flag → General): no extra headers.
        for flag in [
            "general",
            "v2rayn",
            "v2rayng",
            "passwall",
            "ssrplus",
            "shadowrocket",
            "shadowsocks",
            "sagernet",
            "",
        ] {
            assert_eq!(subscription_header_profile(flag), P::None, "flag={flag}");
        }
        // v2RayTun is split out of the base64 bucket even though its body is base64.
        assert_eq!(subscription_header_profile("v2raytun"), P::V2RayTun);
        assert_eq!(
            subscription_header_profile("quantumult%20x"),
            P::QuantumultX
        );
        assert_eq!(
            subscription_header_profile("Quantumult%20X/1.0.5"),
            P::QuantumultX
        );
        // The original non-X Quantumult app must fall through to General (no headers),
        // matching Laravel's literal `quantumult%20x` flag.
        assert_eq!(subscription_header_profile("Quantumult/1.0.0"), P::None);
        assert_eq!(subscription_header_profile("loon"), P::Loon);
        assert_eq!(subscription_header_profile("surge"), P::Surge);
        assert_eq!(subscription_header_profile("surfboard"), P::Surge);
        assert_eq!(subscription_header_profile("clash"), P::Clash);
        // Meta/Verge/Nyanpasu/Stash share the Clash-without-web-page-url header set.
        for flag in [
            "clash.meta",
            "clashmeta",
            "clash-verge",
            "clash.nyanpasu",
            "stash",
        ] {
            assert_eq!(
                subscription_header_profile(flag),
                P::ClashMeta,
                "flag={flag}"
            );
        }
        // Sing-box (modern + legacy) both emit the same headers.
        assert_eq!(subscription_header_profile("sing-box 1.12.0"), P::SingBox);
        assert_eq!(subscription_header_profile("sing-box"), P::SingBox);
    }

    #[test]
    fn paid_amount_yuan_matches_php_total_amount_div_100() {
        // Mirrors PHP `$order->total_amount / 100` string coercion used in the
        // `成功收款` admin Telegram message.
        assert_eq!(format_paid_amount_yuan(1035), "10.35");
        assert_eq!(format_paid_amount_yuan(1000), "10");
        assert_eq!(format_paid_amount_yuan(1050), "10.5");
        assert_eq!(format_paid_amount_yuan(10), "0.1");
        assert_eq!(format_paid_amount_yuan(1), "0.01");
        assert_eq!(format_paid_amount_yuan(0), "0");
        assert_eq!(format_paid_amount_yuan(9_999_999), "99999.99");
    }
}
