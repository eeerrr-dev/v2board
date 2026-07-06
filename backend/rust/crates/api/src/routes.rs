use axum::{
    Router, middleware,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;
use v2board_config::AppConfig;

use super::{AppState, language_middleware};

pub(super) fn build_app(state: AppState, config: &AppConfig) -> Router {
    let admin_api_route = config.admin_api_route();

    let mut app = Router::new()
        .route("/healthz", get(super::healthz))
        .route("/api/v1/guest/comm/config", get(super::guest_config))
        .route(
            "/api/v1/guest/payment/notify/{method}/{uuid}",
            get(super::payment_notify).post(super::payment_notify),
        )
        .route(
            "/api/v1/guest/telegram/webhook",
            post(super::telegram_webhook),
        )
        .route("/api/v1/client/subscribe", get(super::client_subscribe))
        .route(
            "/api/v1/client/app/getConfig",
            get(super::client_app_config),
        )
        .route(
            "/api/v1/client/app/getVersion",
            get(super::client_app_version),
        )
        .route("/api/v1/passport/auth/register", post(super::register))
        .route("/api/v1/passport/auth/login", post(super::login))
        .route(
            "/api/v1/passport/auth/token2Login",
            get(super::token2_login),
        )
        .route("/api/v1/passport/auth/forget", post(super::forget_password))
        .route(
            "/api/v1/passport/auth/getQuickLoginUrl",
            post(super::passport_quick_login_url),
        )
        .route(
            "/api/v1/passport/comm/sendEmailVerify",
            post(super::send_email_verify),
        )
        .route("/api/v1/passport/comm/pv", post(super::passport_pv))
        .route("/api/v1/user/info", get(super::user_info))
        .route("/api/v1/user/checkLogin", get(super::check_login))
        .route("/api/v1/user/getStat", get(super::user_stat))
        .route("/api/v1/user/getSubscribe", get(super::user_subscribe))
        .route("/api/v1/user/newPeriod", post(super::user_new_period))
        .route("/api/v1/user/redeemgiftcard", post(super::redeem_giftcard))
        .route("/api/v1/user/update", post(super::user_update))
        .route("/api/v1/user/changePassword", post(super::change_password))
        .route("/api/v1/user/resetSecurity", get(super::reset_security))
        .route("/api/v1/user/unbindTelegram", get(super::unbind_telegram))
        .route("/api/v1/user/transfer", post(super::user_transfer))
        .route(
            "/api/v1/user/getQuickLoginUrl",
            post(super::user_quick_login_url),
        )
        .route("/api/v1/user/getActiveSession", get(super::active_sessions))
        .route(
            "/api/v1/user/removeActiveSession",
            post(super::remove_active_session),
        )
        .route("/api/v1/user/plan/fetch", get(super::user_plan_fetch))
        .route("/api/v1/user/order/save", post(super::order_save))
        .route("/api/v1/user/order/checkout", post(super::order_checkout))
        .route("/api/v1/user/order/fetch", get(super::order_fetch))
        .route("/api/v1/user/order/detail", get(super::order_detail))
        .route("/api/v1/user/order/check", get(super::order_check))
        .route("/api/v1/user/order/cancel", post(super::order_cancel))
        .route(
            "/api/v1/user/order/getPaymentMethod",
            get(super::order_payment_methods),
        )
        .route("/api/v1/user/invite/save", get(super::invite_save))
        .route("/api/v1/user/invite/fetch", get(super::invite_fetch))
        .route("/api/v1/user/invite/details", get(super::invite_details))
        .route("/api/v1/user/ticket/fetch", get(super::ticket_fetch))
        .route("/api/v1/user/ticket/save", post(super::ticket_save))
        .route("/api/v1/user/ticket/reply", post(super::ticket_reply))
        .route("/api/v1/user/ticket/close", post(super::ticket_close))
        .route("/api/v1/user/ticket/withdraw", post(super::ticket_withdraw))
        .route("/api/v1/user/server/fetch", get(super::server_fetch))
        .route("/api/v1/user/coupon/check", post(super::coupon_check))
        .route("/api/v1/user/knowledge/fetch", get(super::knowledge_fetch))
        .route(
            "/api/v1/user/knowledge/getCategory",
            get(super::knowledge_categories),
        )
        .route("/api/v1/user/notice/fetch", get(super::user_notice_fetch))
        .route(
            "/api/v1/user/telegram/getBotInfo",
            get(super::telegram_bot_info),
        )
        .route("/api/v1/user/comm/config", get(super::user_comm_config))
        .route(
            "/api/v1/user/comm/getStripePublicKey",
            post(super::stripe_public_key),
        )
        .route(
            "/api/v1/user/stat/getTrafficLog",
            get(super::user_traffic_logs),
        )
        .route(
            &admin_api_route,
            get(super::admin_get).post(super::admin_post),
        )
        .route(
            "/api/v1/staff/{*staff_path}",
            get(super::staff_get).post(super::staff_post),
        )
        .route(
            "/api/v1/server/{class}/{action}",
            get(super::server_api::server_v1).post(super::server_api::server_v1),
        )
        .route(
            "/api/v2/server/config",
            get(super::server_api::server_v2_config).post(super::server_api::server_v2_config),
        )
        .fallback(super::dynamic_fallback);

    if let Some(path) = custom_subscribe_route_path(config) {
        tracing::info!(path = %path, "registering custom subscribe route");
        app = app.route(&path, get(super::client_subscribe));
    }

    app.with_state(state)
        .layer(middleware::from_fn(language_middleware))
        .layer(TraceLayer::new_for_http())
}

pub(super) fn custom_subscribe_route_path(config: &AppConfig) -> Option<String> {
    custom_subscribe_route_path_from_str(&config.subscribe_path)
}

pub(super) fn custom_subscribe_route_path_from_str(path: &str) -> Option<String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return None;
    }
    let path = raw_path
        .split('?')
        .next()
        .unwrap_or(raw_path)
        .trim_end_matches('/');
    if path == "/api/v1/client/subscribe" {
        return None;
    }
    if !path.starts_with('/') {
        tracing::warn!(
            path,
            "custom subscribe_path must start with /; route skipped"
        );
        return None;
    }
    Some(if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    })
}
