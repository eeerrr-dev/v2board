use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
};

use anyhow::{Context, Result, bail};

pub fn run() -> Result<()> {
    let admin_path = normalized_admin_path();
    let reference_root = env_or(
        "ROUTE_AUDIT_REFERENCE_ROOT",
        "/src/references/wyx2685-v2board",
    );
    let rust_root = env_or("ROUTE_AUDIT_RUST_ROOT", "/src/backend/rust");
    let reference = collect_reference_routes(Path::new(&reference_root), &admin_path)?;
    let rust = collect_rust_routes(Path::new(&rust_root), &admin_path)?;
    let retired = retired_reference_routes(&admin_path);
    let stale_retirements = retired.difference(&reference).cloned().collect::<Vec<_>>();
    if !stale_retirements.is_empty() {
        for route in &stale_retirements {
            println!("STALE RETIREMENT {} {}", route.method, route.path);
        }
        bail!(
            "route audit failed: {} retired routes no longer exist in the reference",
            stale_retirements.len()
        );
    }
    // Post-migration split (docs/api-dialect.md, the internal dialect
    // completed with W14): frozen §2 external reference routes must exist in
    // Rust verbatim; every internal reference route must translate through
    // the old→new route map to modern route(s) Rust serves. An internal
    // reference route with neither a retirement nor a translation fails the
    // audit loudly.
    let translation = internal_route_translation(&admin_path);
    let mut required = BTreeSet::new();
    let mut frozen_external = 0_usize;
    let mut unmapped = Vec::new();
    for route in reference.difference(&retired) {
        if is_frozen_external_reference_path(&route.path) {
            frozen_external += 1;
            required.insert(route.clone());
            continue;
        }
        match translation.get(route) {
            Some(moderns) => required.extend(moderns.iter().cloned()),
            None => unmapped.push(route.clone()),
        }
    }
    if !unmapped.is_empty() {
        println!("Internal reference routes with no modern translation:");
        for route in &unmapped {
            println!("UNMAPPED {} {}", route.method, route.path);
        }
        bail!(
            "route audit failed: {} internal reference routes are neither retired nor mapped to a modern route",
            unmapped.len()
        );
    }
    let missing = required.difference(&rust).cloned().collect::<Vec<_>>();

    if !missing.is_empty() {
        println!("Required routes missing in Rust:");
        for route in &missing {
            println!("MISSING {} {}", route.method, route.path);
        }
        bail!(
            "route audit failed: {} required routes are missing",
            missing.len()
        );
    }
    println!(
        "Route audit OK: {} required routes are served by Rust ({} frozen external reference routes verbatim, {} modern translations of the internal reference namespaces); {} obsolete routes are explicitly retired.",
        required.len(),
        frozen_external,
        required.len() - frozen_external,
        retired.len()
    );
    Ok(())
}

/// The frozen §2 external namespaces (docs/api-dialect.md §2). These
/// reference routes are byte-frozen and must exist in Rust verbatim. Guest
/// comm/config is internal (modernized to `/public/config`), so only the
/// payment-notify and Telegram-webhook guest subtrees are frozen.
fn is_frozen_external_reference_path(path: &str) -> bool {
    path.starts_with("/api/v1/client/")
        || path.starts_with("/api/v1/server/")
        || path.starts_with("/api/v2/server/")
        || path.starts_with("/api/v1/guest/payment/")
        || path.starts_with("/api/v1/guest/telegram/")
}

/// Translates each internal legacy reference route to the modern route(s)
/// that replaced it, with the live admin prefix substituted for
/// `{secure_path}`.
fn internal_route_translation(admin_path: &str) -> BTreeMap<RouteKey, BTreeSet<RouteKey>> {
    let mut map: BTreeMap<RouteKey, BTreeSet<RouteKey>> = BTreeMap::new();
    for (legacy_method, legacy_path, modern_method, modern_path) in INTERNAL_ROUTE_MAP {
        map.entry(route_key(
            *legacy_method,
            api_v1_path(legacy_path, admin_path),
        ))
        .or_default()
        .insert(route_key(
            *modern_method,
            api_v1_path(modern_path, admin_path),
        ));
    }
    map
}

fn api_v1_path(path: &str, admin_path: &str) -> String {
    format!("/api/v1{}", path.replace("{secure_path}", admin_path))
}

/// The internal old->new route map (docs/api-dialect.md SS5-6, Appendix A),
/// generated from the canonical machine-readable map
/// (`frontend/tests/lib/dialect/route-map.mjs`, SS13.1):
/// `(legacy method, legacy path, modern method, modern path)` relative to
/// `/api/v1`, with `{secure_path}` standing for the dynamic admin prefix.
/// Query/body-discriminated legacy splits map one legacy route to every
/// modern replacement; legacy aliases repeat the shared modern row. Rows the
/// reference never registered (native additions such as step-up or the
/// Stripe intent) are inert here.
const INTERNAL_ROUTE_MAP: &[(&str, &str, &str, &str)] = &[
    (
        "GET",
        "/{secure_path}/config/fetch",
        "GET",
        "/{secure_path}/config",
    ),
    (
        "GET",
        "/{secure_path}/config/getEmailTemplate",
        "GET",
        "/{secure_path}/email-templates",
    ),
    (
        "POST",
        "/{secure_path}/config/save",
        "PATCH",
        "/{secure_path}/config",
    ),
    (
        "POST",
        "/{secure_path}/config/setTelegramWebhook",
        "POST",
        "/{secure_path}/telegram-webhook",
    ),
    (
        "POST",
        "/{secure_path}/config/testSendMail",
        "POST",
        "/{secure_path}/test-mail",
    ),
    (
        "POST",
        "/{secure_path}/coupon/drop",
        "DELETE",
        "/{secure_path}/coupons/{id}",
    ),
    (
        "GET",
        "/{secure_path}/coupon/fetch",
        "GET",
        "/{secure_path}/coupons",
    ),
    (
        "POST",
        "/{secure_path}/coupon/generate",
        "POST",
        "/{secure_path}/coupons",
    ),
    (
        "POST",
        "/{secure_path}/coupon/generate",
        "PATCH",
        "/{secure_path}/coupons/{id}",
    ),
    (
        "POST",
        "/{secure_path}/coupon/show",
        "PATCH",
        "/{secure_path}/coupons/{id}",
    ),
    (
        "POST",
        "/{secure_path}/giftcard/drop",
        "DELETE",
        "/{secure_path}/gift-cards/{id}",
    ),
    (
        "GET",
        "/{secure_path}/giftcard/fetch",
        "GET",
        "/{secure_path}/gift-cards",
    ),
    (
        "POST",
        "/{secure_path}/giftcard/generate",
        "POST",
        "/{secure_path}/gift-cards",
    ),
    (
        "POST",
        "/{secure_path}/giftcard/generate",
        "PATCH",
        "/{secure_path}/gift-cards/{id}",
    ),
    (
        "POST",
        "/{secure_path}/knowledge/drop",
        "DELETE",
        "/{secure_path}/knowledge/{id}",
    ),
    (
        "GET",
        "/{secure_path}/knowledge/fetch",
        "GET",
        "/{secure_path}/knowledge",
    ),
    (
        "GET",
        "/{secure_path}/knowledge/fetch",
        "GET",
        "/{secure_path}/knowledge/{id}",
    ),
    (
        "GET",
        "/{secure_path}/knowledge/getCategory",
        "GET",
        "/{secure_path}/knowledge-categories",
    ),
    (
        "POST",
        "/{secure_path}/knowledge/save",
        "POST",
        "/{secure_path}/knowledge",
    ),
    (
        "POST",
        "/{secure_path}/knowledge/save",
        "PATCH",
        "/{secure_path}/knowledge/{id}",
    ),
    (
        "POST",
        "/{secure_path}/knowledge/show",
        "PATCH",
        "/{secure_path}/knowledge/{id}",
    ),
    (
        "POST",
        "/{secure_path}/knowledge/sort",
        "POST",
        "/{secure_path}/knowledge/sort",
    ),
    (
        "POST",
        "/{secure_path}/notice/drop",
        "DELETE",
        "/{secure_path}/notices/{id}",
    ),
    (
        "GET",
        "/{secure_path}/notice/fetch",
        "GET",
        "/{secure_path}/notices",
    ),
    (
        "POST",
        "/{secure_path}/notice/save",
        "POST",
        "/{secure_path}/notices",
    ),
    (
        "POST",
        "/{secure_path}/notice/save",
        "PATCH",
        "/{secure_path}/notices/{id}",
    ),
    (
        "POST",
        "/{secure_path}/notice/show",
        "PATCH",
        "/{secure_path}/notices/{id}",
    ),
    (
        "POST",
        "/{secure_path}/notice/update",
        "PATCH",
        "/{secure_path}/notices/{id}",
    ),
    (
        "POST",
        "/{secure_path}/order/assign",
        "POST",
        "/{secure_path}/orders",
    ),
    (
        "POST",
        "/{secure_path}/order/cancel",
        "POST",
        "/{secure_path}/orders/{trade_no}/cancel",
    ),
    (
        "POST",
        "/{secure_path}/order/detail",
        "GET",
        "/{secure_path}/orders/{trade_no}",
    ),
    (
        "GET",
        "/{secure_path}/order/fetch",
        "GET",
        "/{secure_path}/orders",
    ),
    (
        "POST",
        "/{secure_path}/order/paid",
        "POST",
        "/{secure_path}/orders/{trade_no}/mark-paid",
    ),
    (
        "GET",
        "/{secure_path}/order/reconciliation/fetch",
        "GET",
        "/{secure_path}/payment-reconciliations",
    ),
    (
        "POST",
        "/{secure_path}/order/update",
        "PATCH",
        "/{secure_path}/orders/{trade_no}",
    ),
    (
        "POST",
        "/{secure_path}/order/update",
        "POST",
        "/{secure_path}/payment-reconciliations/{id}/resolve",
    ),
    (
        "POST",
        "/{secure_path}/payment/drop",
        "DELETE",
        "/{secure_path}/payments/{id}",
    ),
    (
        "GET",
        "/{secure_path}/payment/fetch",
        "GET",
        "/{secure_path}/payments",
    ),
    (
        "POST",
        "/{secure_path}/payment/getPaymentForm",
        "GET",
        "/{secure_path}/payment-providers/{code}/form",
    ),
    (
        "GET",
        "/{secure_path}/payment/getPaymentMethods",
        "GET",
        "/{secure_path}/payment-providers",
    ),
    (
        "POST",
        "/{secure_path}/payment/save",
        "POST",
        "/{secure_path}/payments",
    ),
    (
        "POST",
        "/{secure_path}/payment/save",
        "PATCH",
        "/{secure_path}/payments/{id}",
    ),
    (
        "POST",
        "/{secure_path}/payment/show",
        "PATCH",
        "/{secure_path}/payments/{id}",
    ),
    (
        "POST",
        "/{secure_path}/payment/sort",
        "POST",
        "/{secure_path}/payments/sort",
    ),
    (
        "POST",
        "/{secure_path}/plan/drop",
        "DELETE",
        "/{secure_path}/plans/{id}",
    ),
    (
        "GET",
        "/{secure_path}/plan/fetch",
        "GET",
        "/{secure_path}/plans",
    ),
    (
        "POST",
        "/{secure_path}/plan/save",
        "POST",
        "/{secure_path}/plans",
    ),
    (
        "POST",
        "/{secure_path}/plan/save",
        "PATCH",
        "/{secure_path}/plans/{id}",
    ),
    (
        "POST",
        "/{secure_path}/plan/sort",
        "POST",
        "/{secure_path}/plans/sort",
    ),
    (
        "POST",
        "/{secure_path}/plan/update",
        "PATCH",
        "/{secure_path}/plans/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/anytls/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/anytls/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/anytls/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/anytls/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/anytls/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/group/drop",
        "DELETE",
        "/{secure_path}/server-groups/{id}",
    ),
    (
        "GET",
        "/{secure_path}/server/group/fetch",
        "GET",
        "/{secure_path}/server-groups",
    ),
    (
        "POST",
        "/{secure_path}/server/group/save",
        "POST",
        "/{secure_path}/server-groups",
    ),
    (
        "POST",
        "/{secure_path}/server/group/save",
        "PATCH",
        "/{secure_path}/server-groups/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/hysteria/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/hysteria/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/hysteria/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/hysteria/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/hysteria/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "GET",
        "/{secure_path}/server/manage/getNodes",
        "GET",
        "/{secure_path}/nodes",
    ),
    (
        "POST",
        "/{secure_path}/server/manage/sort",
        "POST",
        "/{secure_path}/nodes/sort",
    ),
    (
        "POST",
        "/{secure_path}/server/route/drop",
        "DELETE",
        "/{secure_path}/server-routes/{id}",
    ),
    (
        "GET",
        "/{secure_path}/server/route/fetch",
        "GET",
        "/{secure_path}/server-routes",
    ),
    (
        "POST",
        "/{secure_path}/server/route/save",
        "POST",
        "/{secure_path}/server-routes",
    ),
    (
        "POST",
        "/{secure_path}/server/route/save",
        "PATCH",
        "/{secure_path}/server-routes/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/shadowsocks/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/shadowsocks/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/shadowsocks/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/shadowsocks/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/shadowsocks/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/trojan/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/trojan/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/trojan/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/trojan/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/trojan/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/tuic/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/tuic/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/tuic/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/tuic/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/tuic/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/v2node/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/v2node/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/v2node/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/v2node/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/v2node/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/vless/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/vless/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/vless/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/vless/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/vless/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/vmess/copy",
        "POST",
        "/{secure_path}/servers/{type}/{id}/copy",
    ),
    (
        "POST",
        "/{secure_path}/server/vmess/drop",
        "DELETE",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/vmess/save",
        "POST",
        "/{secure_path}/servers/{type}",
    ),
    (
        "POST",
        "/{secure_path}/server/vmess/save",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "POST",
        "/{secure_path}/server/vmess/update",
        "PATCH",
        "/{secure_path}/servers/{type}/{id}",
    ),
    (
        "GET",
        "/{secure_path}/stat/getOrder",
        "GET",
        "/{secure_path}/stats/orders",
    ),
    (
        "GET",
        "/{secure_path}/stat/getOverride",
        "GET",
        "/{secure_path}/stats/summary",
    ),
    (
        "GET",
        "/{secure_path}/stat/getRanking",
        "GET",
        "/{secure_path}/stats/summary",
    ),
    (
        "GET",
        "/{secure_path}/stat/getServerLastRank",
        "GET",
        "/{secure_path}/stats/server-rank",
    ),
    (
        "GET",
        "/{secure_path}/stat/getServerTodayRank",
        "GET",
        "/{secure_path}/stats/server-rank",
    ),
    (
        "GET",
        "/{secure_path}/stat/getStat",
        "GET",
        "/{secure_path}/stats/summary",
    ),
    (
        "GET",
        "/{secure_path}/stat/getStatRecord",
        "GET",
        "/{secure_path}/stats/records",
    ),
    (
        "GET",
        "/{secure_path}/stat/getStatUser",
        "GET",
        "/{secure_path}/stats/user-traffic",
    ),
    (
        "GET",
        "/{secure_path}/stat/getUserLastRank",
        "GET",
        "/{secure_path}/stats/user-rank",
    ),
    (
        "GET",
        "/{secure_path}/stat/getUserTodayRank",
        "GET",
        "/{secure_path}/stats/user-rank",
    ),
    (
        "GET",
        "/{secure_path}/system/getQueueMasters",
        "GET",
        "/{secure_path}/system/queue-masters",
    ),
    (
        "GET",
        "/{secure_path}/system/getQueueStats",
        "GET",
        "/{secure_path}/system/queue-stats",
    ),
    (
        "GET",
        "/{secure_path}/system/getQueueWorkload",
        "GET",
        "/{secure_path}/system/queue-workload",
    ),
    (
        "GET",
        "/{secure_path}/system/getSystemLog",
        "GET",
        "/{secure_path}/system/logs",
    ),
    (
        "GET",
        "/{secure_path}/system/getSystemStatus",
        "GET",
        "/{secure_path}/system/status",
    ),
    (
        "POST",
        "/{secure_path}/ticket/close",
        "POST",
        "/{secure_path}/tickets/{id}/close",
    ),
    (
        "GET",
        "/{secure_path}/ticket/fetch",
        "GET",
        "/{secure_path}/tickets",
    ),
    (
        "GET",
        "/{secure_path}/ticket/fetch",
        "GET",
        "/{secure_path}/tickets/{id}",
    ),
    (
        "POST",
        "/{secure_path}/ticket/reply",
        "POST",
        "/{secure_path}/tickets/{id}/replies",
    ),
    (
        "POST",
        "/{secure_path}/user/allDel",
        "POST",
        "/{secure_path}/users/bulk-delete",
    ),
    (
        "POST",
        "/{secure_path}/user/ban",
        "POST",
        "/{secure_path}/users/ban",
    ),
    (
        "POST",
        "/{secure_path}/user/delUser",
        "DELETE",
        "/{secure_path}/users/{id}",
    ),
    (
        "POST",
        "/{secure_path}/user/dumpCSV",
        "POST",
        "/{secure_path}/users/export",
    ),
    (
        "GET",
        "/{secure_path}/user/fetch",
        "GET",
        "/{secure_path}/users",
    ),
    (
        "POST",
        "/{secure_path}/user/generate",
        "POST",
        "/{secure_path}/users",
    ),
    (
        "GET",
        "/{secure_path}/user/getUserInfoById",
        "GET",
        "/{secure_path}/users/{id}",
    ),
    (
        "POST",
        "/{secure_path}/user/resetSecret",
        "POST",
        "/{secure_path}/users/{id}/reset-secret",
    ),
    (
        "POST",
        "/{secure_path}/user/sendMail",
        "POST",
        "/{secure_path}/users/mail",
    ),
    (
        "POST",
        "/{secure_path}/user/setInviteUser",
        "POST",
        "/{secure_path}/users/{id}/set-inviter",
    ),
    (
        "POST",
        "/{secure_path}/user/update",
        "PATCH",
        "/{secure_path}/users/{id}",
    ),
    ("GET", "/guest/comm/config", "GET", "/public/config"),
    (
        "POST",
        "/passport/auth/forget",
        "POST",
        "/auth/password-reset",
    ),
    (
        "POST",
        "/passport/auth/getQuickLoginUrl",
        "POST",
        "/auth/quick-login-url",
    ),
    ("POST", "/passport/auth/login", "POST", "/auth/login"),
    ("POST", "/passport/auth/register", "POST", "/auth/register"),
    ("POST", "/passport/auth/stepUp", "POST", "/auth/step-up"),
    (
        "GET",
        "/passport/auth/token2Login",
        "GET",
        "/auth/quick-login",
    ),
    (
        "GET",
        "/passport/auth/token2Login",
        "POST",
        "/auth/token-login",
    ),
    ("POST", "/passport/comm/pv", "POST", "/public/invite-views"),
    (
        "POST",
        "/passport/comm/sendEmailVerify",
        "POST",
        "/auth/email-codes",
    ),
    (
        "POST",
        "/staff/notice/drop",
        "DELETE",
        "/staff/notices/{id}",
    ),
    ("GET", "/staff/notice/fetch", "GET", "/staff/notices"),
    ("POST", "/staff/notice/save", "POST", "/staff/notices"),
    ("POST", "/staff/notice/save", "PATCH", "/staff/notices/{id}"),
    (
        "POST",
        "/staff/notice/update",
        "PATCH",
        "/staff/notices/{id}",
    ),
    ("GET", "/staff/plan/fetch", "GET", "/staff/plans"),
    (
        "POST",
        "/staff/ticket/close",
        "POST",
        "/staff/tickets/{id}/close",
    ),
    ("GET", "/staff/ticket/fetch", "GET", "/staff/tickets"),
    ("GET", "/staff/ticket/fetch", "GET", "/staff/tickets/{id}"),
    (
        "POST",
        "/staff/ticket/reply",
        "POST",
        "/staff/tickets/{id}/replies",
    ),
    ("POST", "/staff/user/ban", "POST", "/staff/users/ban"),
    (
        "GET",
        "/staff/user/getUserInfoById",
        "GET",
        "/staff/users/{id}",
    ),
    ("POST", "/staff/user/sendMail", "POST", "/staff/users/mail"),
    ("POST", "/staff/user/update", "PATCH", "/staff/users/{id}"),
    ("POST", "/user/changePassword", "PUT", "/user/password"),
    ("GET", "/user/checkLogin", "GET", "/auth/session"),
    ("GET", "/user/comm/config", "GET", "/user/config"),
    ("POST", "/user/coupon/check", "POST", "/user/coupons/check"),
    ("GET", "/user/getActiveSession", "GET", "/user/sessions"),
    (
        "POST",
        "/user/getQuickLoginUrl",
        "POST",
        "/auth/quick-login-url",
    ),
    ("GET", "/user/getStat", "GET", "/user/stats"),
    ("GET", "/user/getSubscribe", "GET", "/user/subscription"),
    ("GET", "/user/info", "GET", "/user/profile"),
    ("GET", "/user/invite/details", "GET", "/user/commissions"),
    ("GET", "/user/invite/fetch", "GET", "/user/invite"),
    ("GET", "/user/invite/save", "POST", "/user/invite-codes"),
    ("GET", "/user/knowledge/fetch", "GET", "/user/knowledge"),
    (
        "GET",
        "/user/knowledge/fetch",
        "GET",
        "/user/knowledge/{id}",
    ),
    (
        "GET",
        "/user/knowledge/getCategory",
        "GET",
        "/user/knowledge-categories",
    ),
    ("POST", "/user/logout", "DELETE", "/auth/session"),
    (
        "POST",
        "/user/newPeriod",
        "POST",
        "/user/subscription/new-period",
    ),
    ("GET", "/user/notice/fetch", "GET", "/user/notices"),
    (
        "POST",
        "/user/order/cancel",
        "POST",
        "/user/orders/{trade_no}/cancel",
    ),
    (
        "GET",
        "/user/order/check",
        "GET",
        "/user/orders/{trade_no}/status",
    ),
    (
        "POST",
        "/user/order/checkout",
        "POST",
        "/user/orders/{trade_no}/checkout",
    ),
    (
        "GET",
        "/user/order/detail",
        "GET",
        "/user/orders/{trade_no}",
    ),
    ("GET", "/user/order/fetch", "GET", "/user/orders"),
    (
        "GET",
        "/user/order/getPaymentMethod",
        "GET",
        "/user/payment-methods",
    ),
    ("POST", "/user/order/save", "POST", "/user/orders"),
    (
        "POST",
        "/user/order/stripe/intent",
        "POST",
        "/user/orders/{trade_no}/stripe-intent",
    ),
    ("GET", "/user/plan/fetch", "GET", "/user/plans"),
    ("GET", "/user/plan/fetch", "GET", "/user/plans/{id}"),
    (
        "POST",
        "/user/redeemgiftcard",
        "POST",
        "/user/gift-card-redemptions",
    ),
    (
        "POST",
        "/user/removeActiveSession",
        "DELETE",
        "/user/sessions/{session_id}",
    ),
    (
        "GET",
        "/user/resetSecurity",
        "POST",
        "/user/subscription/reset-token",
    ),
    ("GET", "/user/server/fetch", "GET", "/user/servers"),
    (
        "GET",
        "/user/stat/getTrafficLog",
        "GET",
        "/user/traffic-logs",
    ),
    (
        "GET",
        "/user/telegram/getBotInfo",
        "GET",
        "/user/telegram-bot",
    ),
    (
        "POST",
        "/user/ticket/close",
        "POST",
        "/user/tickets/{id}/close",
    ),
    ("GET", "/user/ticket/fetch", "GET", "/user/tickets"),
    ("GET", "/user/ticket/fetch", "GET", "/user/tickets/{id}"),
    (
        "POST",
        "/user/ticket/reply",
        "POST",
        "/user/tickets/{id}/replies",
    ),
    ("POST", "/user/ticket/save", "POST", "/user/tickets"),
    (
        "POST",
        "/user/ticket/withdraw",
        "POST",
        "/user/withdrawal-tickets",
    ),
    (
        "POST",
        "/user/transfer",
        "POST",
        "/user/commission-transfers",
    ),
    (
        "GET",
        "/user/unbindTelegram",
        "DELETE",
        "/user/telegram-binding",
    ),
    ("POST", "/user/update", "PATCH", "/user/profile"),
];

fn retired_reference_routes(admin_path: &str) -> BTreeSet<RouteKey> {
    // The package-theme API was removed with the server-installed frontend theme
    // subsystem. Branding remains native config (color/background/custom HTML).
    // Stripe's public-key endpoint was replaced by server-created PaymentIntents.
    [
        route_key(
            "GET",
            format!("/api/v1/{admin_path}/config/getThemeTemplate"),
        ),
        route_key("GET", format!("/api/v1/{admin_path}/theme/getThemes")),
        route_key("POST", format!("/api/v1/{admin_path}/theme/getThemeConfig")),
        route_key(
            "POST",
            format!("/api/v1/{admin_path}/theme/saveThemeConfig"),
        ),
        route_key("POST", "/api/v1/user/comm/getStripePublicKey"),
    ]
    .into_iter()
    .collect()
}

fn normalized_admin_path() -> String {
    let configured = env_or("ROUTE_AUDIT_ADMIN_PATH", "admin");
    let configured = configured.trim_matches('/');
    if configured.is_empty() {
        "admin".to_string()
    } else {
        configured.to_string()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RouteKey {
    method: String,
    path: String,
}

fn route_key(method: impl AsRef<str>, path: impl AsRef<str>) -> RouteKey {
    RouteKey {
        method: method.as_ref().to_ascii_uppercase(),
        path: normalize_route_path(path.as_ref()),
    }
}

fn collect_reference_routes(root: &Path, admin_path: &str) -> Result<BTreeSet<RouteKey>> {
    let v1 = root.join("app/Http/Routes/V1");
    let v2 = root.join("app/Http/Routes/V2");
    let mut routes = BTreeSet::new();
    for (file, prefix, root_segment) in [
        ("GuestRoute.php", "/api/v1/guest", "guest"),
        ("ClientRoute.php", "/api/v1/client", "client"),
        ("PassportRoute.php", "/api/v1/passport", "passport"),
        ("UserRoute.php", "/api/v1/user", "user"),
        ("StaffRoute.php", "/api/v1/staff", "staff"),
        ("AdminRoute.php", "", ""),
        ("ServerRoute.php", "/api/v1/server", "server"),
    ] {
        let prefix = if file == "AdminRoute.php" {
            format!("/api/v1/{admin_path}")
        } else {
            prefix.to_string()
        };
        routes.extend(parse_reference_route_file(
            &v1.join(file),
            &prefix,
            root_segment,
        )?);
    }
    routes.extend(parse_reference_route_file(
        &v2.join("ServerRoute.php"),
        "/api/v2/server",
        "server",
    )?);
    Ok(routes)
}

fn parse_reference_route_file(
    path: &Path,
    root_prefix: &str,
    root_segment: &str,
) -> Result<BTreeSet<RouteKey>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("read reference route file {path:?}"))?;
    let mut routes = BTreeSet::new();
    let mut prefixes = vec![normalize_route_path(root_prefix)];
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("});") && prefixes.len() > 1 {
            prefixes.pop();
            continue;
        }
        if let Some(prefix) = extract_reference_group_prefix(trimmed) {
            if prefixes.len() == 1 && prefix.trim_matches('/') == root_segment {
                continue;
            }
            prefixes.push(prefix);
            continue;
        }
        let Some((methods, route_path)) = extract_reference_route(trimmed) else {
            // A line that registers a routing verb but yields no literal path
            // (multi-line, variable/const path, or an unmapped verb) would be
            // silently dropped from the required set, letting a genuinely-missing
            // Rust route pass the audit. Fail loudly instead. `->group(` lines are
            // handled above and never reach here as verbs.
            if let Some(verb) = reference_route_verb(trimmed) {
                bail!(
                    "route audit: unparseable {verb} route in {path:?} line {}: `{trimmed}` \
                     — could not extract a literal path; this route would be silently \
                     dropped from the audit",
                    line_index + 1
                );
            }
            continue;
        };
        let full_path = join_route_parts(
            prefixes
                .iter()
                .map(String::as_str)
                .chain([route_path.as_str()]),
        );
        for method in methods {
            routes.insert(route_key(method, &full_path));
        }
    }
    Ok(routes)
}

fn collect_rust_routes(root: &Path, admin_path: &str) -> Result<BTreeSet<RouteKey>> {
    let api_main = fs::read_to_string(root.join("crates/api/src/main.rs"))?;
    let api_routes = fs::read_to_string(root.join("crates/api/src/routes.rs"))?;
    let admin = fs::read_to_string(root.join("crates/api/src/admin.rs"))?;
    let mut routes = collect_rust_axum_routes(&format!("{api_main}\n{api_routes}"));
    routes.retain(|route| {
        !route.path.contains("{*admin_path}") && !route.path.contains("{*staff_path}")
    });

    // The modern admin resources are a nested method-aware router relative
    // to the live `secure_path` prefix; the staff mirror nests at its fixed
    // prefix (docs/api-dialect.md §6/§6.9).
    for route in collect_rust_axum_routes(&function_block(&admin, "fn admin_router(")?) {
        routes.insert(route_key(
            route.method,
            format!("/api/v1/{admin_path}{}", route.path),
        ));
    }
    for route in collect_rust_axum_routes(&function_block(&admin, "fn staff_router(")?) {
        routes.insert(route_key(
            route.method,
            format!("/api/v1/staff{}", route.path),
        ));
    }
    Ok(routes)
}

/// The source text of one top-level `fn`, from its marker to the next
/// column-zero closing brace. Fails loudly if the marker vanishes so a
/// renamed router cannot silently drop every nested route from the audit.
fn function_block(content: &str, marker: &str) -> Result<String> {
    let start = content
        .find(marker)
        .with_context(|| format!("route audit: `{marker}` not found in crates/api/src/admin.rs"))?;
    let rest = &content[start..];
    let end = rest
        .find("\n}")
        .map(|index| index + 2)
        .unwrap_or(rest.len());
    Ok(rest[..end].to_string())
}

fn collect_rust_axum_routes(content: &str) -> BTreeSet<RouteKey> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut routes = BTreeSet::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].contains(".route(") {
            index += 1;
            continue;
        }
        let (block, next_index) = rust_route_block(&lines, index);
        index = next_index;
        let Some(path) = quoted_strings(&block)
            .into_iter()
            .find(|value| value.starts_with('/'))
        else {
            continue;
        };
        if path.contains("{*") {
            continue;
        }
        for (needle, method) in [
            ("get(", "GET"),
            ("post(", "POST"),
            ("put(", "PUT"),
            ("patch(", "PATCH"),
            ("delete(", "DELETE"),
        ] {
            if block.contains(needle) {
                routes.insert(route_key(method, &path));
            }
        }
    }
    routes
}

fn rust_route_block(lines: &[&str], start: usize) -> (String, usize) {
    let mut block = String::new();
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let segment = if index == start {
            line.split_once(".route(")
                .map(|(_, right)| format!(".route({right}"))
                .unwrap_or_else(|| (*line).to_string())
        } else {
            (*line).to_string()
        };
        for ch in segment.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        block.push_str(&segment);
        block.push(' ');
        if depth <= 0 {
            return (block, index + 1);
        }
    }
    (block, lines.len())
}

fn extract_reference_group_prefix(line: &str) -> Option<String> {
    let (left, right) = line.split_once("=>")?;
    if !left.contains("'prefix'") && !left.contains("\"prefix\"") {
        return None;
    }
    let right = right.trim();
    if !(right.starts_with('\'') || right.starts_with('"')) {
        return None;
    }
    quoted_strings(right).into_iter().next()
}

/// Returns the routing verb of a `$router->verb(` registration line, if it is one
/// of the HTTP verbs (not `group`/`resource`/other builder calls). Used to decide
/// whether a line that failed path extraction is a genuinely-dropped route.
fn reference_route_verb(line: &str) -> Option<String> {
    let rest = line.split_once("$router->")?.1;
    let verb = rest.split_once('(')?.0.trim().to_ascii_lowercase();
    matches!(
        verb.as_str(),
        "get" | "post" | "any" | "match" | "put" | "patch" | "delete"
    )
    .then_some(verb)
}

fn extract_reference_route(line: &str) -> Option<(Vec<&'static str>, String)> {
    let router = line.find("$router->")?;
    let rest = &line[router + "$router->".len()..];
    let method = rest.split_once('(')?.0.trim().to_ascii_lowercase();
    let methods = match method.as_str() {
        "get" => vec!["GET"],
        "post" => vec!["POST"],
        "any" => vec!["GET", "POST"],
        "match" => vec!["GET", "POST"],
        _ => return None,
    };
    let route_path = quoted_strings(rest).into_iter().find(|value| {
        !matches!(value.as_str(), "get" | "post" | "put" | "patch" | "delete")
            && !value.contains('\\')
    })?;
    Some((methods, route_path))
}

fn quoted_strings(input: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '\'' && ch != '"' {
            continue;
        }
        let quote = ch;
        let mut value = String::new();
        let mut escaped = false;
        for (_, ch) in chars.by_ref() {
            if escaped {
                value.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                break;
            }
            value.push(ch);
        }
        output.push(value);
    }
    output
}

fn join_route_parts<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let body = parts
        .into_iter()
        .flat_map(|part| part.split('/'))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{body}")
}

fn normalize_route_path(path: &str) -> String {
    join_route_parts([path])
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_route_verb_recognizes_http_verbs_not_group() {
        assert_eq!(
            reference_route_verb("$router->post('/a', 'C@m');").as_deref(),
            Some("post")
        );
        assert_eq!(
            reference_route_verb("$router->match(['get','post'], '/a', 'C@m');").as_deref(),
            Some("match")
        );
        // Capitalized verb (a real occurrence in the reference) normalizes.
        assert_eq!(
            reference_route_verb("$router->Post('/a', 'C@m');").as_deref(),
            Some("post")
        );
        // group / non-route builder calls are not routing verbs.
        assert_eq!(
            reference_route_verb("$router->group(['prefix' => 'x'],"),
            None
        );
        assert_eq!(reference_route_verb("something else"), None);
    }

    #[test]
    fn extract_reference_route_handles_match_array_and_backslash_controller() {
        // The `match` form lists methods first; the path is the first quoted
        // string that is neither a verb nor a backslashed controller reference.
        let (methods, path) = extract_reference_route(
            "$router->match(['get', 'post'], '/payment/notify/{method}/{uuid}', 'V1\\Guest\\PaymentController@notify');",
        )
        .expect("match route parses");
        assert_eq!(methods, vec!["GET", "POST"]);
        assert_eq!(path, "/payment/notify/{method}/{uuid}");
    }

    #[test]
    fn parse_reference_route_file_collects_literal_routes() {
        let path =
            std::env::temp_dir().join(format!("v2board_route_audit_ok_{}.php", std::process::id()));
        std::fs::write(
            &path,
            "<?php\n$router->post('/order/save', 'V1\\\\User\\\\OrderController@save');\n\
             $router->match(['get', 'post'], '/payment/notify/{method}/{uuid}', 'V1\\\\Guest\\\\PaymentController@notify');\n",
        )
        .unwrap();

        let routes = parse_reference_route_file(&path, "/api/v1/user", "user").unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(routes.contains(&route_key("POST", "/api/v1/user/order/save")));
        assert!(routes.contains(&route_key(
            "GET",
            "/api/v1/user/payment/notify/{method}/{uuid}"
        )));
        assert!(routes.contains(&route_key(
            "POST",
            "/api/v1/user/payment/notify/{method}/{uuid}"
        )));
    }

    #[test]
    fn parse_reference_route_file_hard_fails_on_unparseable_route() {
        // A verb registration whose path is a PHP variable (not a literal) used to
        // be silently dropped, hiding a genuinely-missing Rust route.
        let path = std::env::temp_dir().join(format!(
            "v2board_route_audit_bad_{}.php",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "<?php\n$router->post($dynamicPath, 'V1\\\\User\\\\OrderController@save');\n",
        )
        .unwrap();

        let result = parse_reference_route_file(&path, "/api/v1/user", "user");
        let _ = std::fs::remove_file(&path);

        let error = result.expect_err("unparseable route must fail the audit");
        assert!(error.to_string().contains("unparseable"));
    }

    #[test]
    fn internal_route_translation_is_admin_path_aware_and_splits_discriminators() {
        let map = internal_route_translation("private-admin");
        // One legacy upsert requires both modern replacements.
        let generate = map
            .get(&route_key("POST", "/api/v1/private-admin/coupon/generate"))
            .expect("coupon generate is mapped");
        assert!(generate.contains(&route_key("POST", "/api/v1/private-admin/coupons")));
        assert!(generate.contains(&route_key("PATCH", "/api/v1/private-admin/coupons/{id}")));
        // Query-discriminated legacy splits fan out to every modern route.
        let token2login = map
            .get(&route_key("GET", "/api/v1/passport/auth/token2Login"))
            .expect("token2Login is mapped");
        assert!(token2login.contains(&route_key("GET", "/api/v1/auth/quick-login")));
        assert!(token2login.contains(&route_key("POST", "/api/v1/auth/token-login")));
    }

    #[test]
    fn retired_reference_routes_are_exact_and_admin_path_aware() {
        let retired = retired_reference_routes("private-admin");
        assert_eq!(retired.len(), 5);
        assert!(retired.contains(&route_key("GET", "/api/v1/private-admin/theme/getThemes")));
        assert!(retired.contains(&route_key("POST", "/api/v1/user/comm/getStripePublicKey")));
    }
}
