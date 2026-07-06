use std::{collections::HashMap, fs, net::SocketAddr};

use axum::{
    Json, Router,
    body::to_bytes,
    extract::{ConnectInfo, Form, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::{Datelike, Duration, Local, TimeZone, Utc};
use hmac::{Hmac, KeyInit, Mac};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha1::{Digest, Sha1};
use sqlx::{AssertSqlSafe, FromRow, MySql, QueryBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data, legacy_page};
use v2board_config::AppConfig;
use v2board_db::{DbPool, connect_mysql};
use v2board_domain::auth::{AuthService, AuthUser, EmailVerifyInput, ForgetInput, RegisterInput};

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    db: DbPool,
    redis: redis::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env();
    let db = connect_mysql(&config.database_url).await?;
    let redis = redis::Client::open(config.redis_url.clone())?;
    let state = AppState {
        config: config.clone(),
        db,
        redis,
    };

    let mut app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1/guest/comm/config", get(guest_config))
        .route(
            "/api/v1/guest/payment/notify/{method}/{uuid}",
            get(payment_notify).post(payment_notify),
        )
        .route("/api/v1/guest/telegram/webhook", post(telegram_webhook))
        .route("/api/v1/client/subscribe", get(client_subscribe))
        .route("/api/v1/client/app/getConfig", get(client_app_config))
        .route("/api/v1/client/app/getVersion", get(client_app_version))
        .route("/api/v1/passport/auth/register", post(register))
        .route("/api/v1/passport/auth/login", post(login))
        .route("/api/v1/passport/auth/token2Login", get(token2_login))
        .route("/api/v1/passport/auth/forget", post(forget_password))
        .route(
            "/api/v1/passport/auth/getQuickLoginUrl",
            post(passport_quick_login_url),
        )
        .route(
            "/api/v1/passport/comm/sendEmailVerify",
            post(send_email_verify),
        )
        .route("/api/v1/passport/comm/pv", post(passport_pv))
        .route("/api/v1/user/info", get(user_info))
        .route("/api/v1/user/checkLogin", get(check_login))
        .route("/api/v1/user/getStat", get(user_stat))
        .route("/api/v1/user/getSubscribe", get(user_subscribe))
        .route("/api/v1/user/newPeriod", post(user_new_period))
        .route("/api/v1/user/redeemgiftcard", post(redeem_giftcard))
        .route("/api/v1/user/update", post(user_update))
        .route("/api/v1/user/changePassword", post(change_password))
        .route("/api/v1/user/resetSecurity", get(reset_security))
        .route("/api/v1/user/unbindTelegram", get(unbind_telegram))
        .route("/api/v1/user/transfer", post(user_transfer))
        .route("/api/v1/user/getQuickLoginUrl", post(user_quick_login_url))
        .route("/api/v1/user/getActiveSession", get(active_sessions))
        .route(
            "/api/v1/user/removeActiveSession",
            post(remove_active_session),
        )
        .route("/api/v1/user/plan/fetch", get(user_plan_fetch))
        .route("/api/v1/user/order/save", post(order_save))
        .route("/api/v1/user/order/checkout", post(order_checkout))
        .route("/api/v1/user/order/fetch", get(order_fetch))
        .route("/api/v1/user/order/detail", get(order_detail))
        .route("/api/v1/user/order/check", get(order_check))
        .route("/api/v1/user/order/cancel", post(order_cancel))
        .route(
            "/api/v1/user/order/getPaymentMethod",
            get(order_payment_methods),
        )
        .route("/api/v1/user/invite/save", get(invite_save))
        .route("/api/v1/user/invite/fetch", get(invite_fetch))
        .route("/api/v1/user/invite/details", get(invite_details))
        .route("/api/v1/user/ticket/fetch", get(ticket_fetch))
        .route("/api/v1/user/ticket/save", post(ticket_save))
        .route("/api/v1/user/ticket/reply", post(ticket_reply))
        .route("/api/v1/user/ticket/close", post(ticket_close))
        .route("/api/v1/user/ticket/withdraw", post(ticket_withdraw))
        .route("/api/v1/user/server/fetch", get(server_fetch))
        .route("/api/v1/user/coupon/check", post(coupon_check))
        .route("/api/v1/user/knowledge/fetch", get(knowledge_fetch))
        .route(
            "/api/v1/user/knowledge/getCategory",
            get(knowledge_categories),
        )
        .route("/api/v1/user/notice/fetch", get(user_notice_fetch))
        .route("/api/v1/user/telegram/getBotInfo", get(telegram_bot_info))
        .route("/api/v1/user/comm/config", get(user_comm_config))
        .route(
            "/api/v1/user/comm/getStripePublicKey",
            post(stripe_public_key),
        )
        .route("/api/v1/user/stat/getTrafficLog", get(user_traffic_logs))
        .route(
            "/api/v1/admin/{*admin_path}",
            get(admin_get).post(admin_post),
        )
        .route(
            "/api/v1/staff/{*staff_path}",
            get(staff_get).post(staff_post),
        )
        .route(
            "/api/v1/server/{class}/{action}",
            get(server_v1).post(server_v1),
        )
        .route(
            "/api/v2/server/config",
            get(server_v2_config).post(server_v2_config),
        );

    if let Some(path) = custom_subscribe_route_path(&config) {
        tracing::info!(path = %path, "registering custom subscribe route");
        app = app.route(&path, get(client_subscribe));
    }

    let app = app.with_state(state).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(bind_addr = %config.bind_addr, "v2board rust api listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("v2board_api=info,tower_http=info"));
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn custom_subscribe_route_path(config: &AppConfig) -> Option<String> {
    custom_subscribe_route_path_from_str(&config.subscribe_path)
}

fn custom_subscribe_route_path_from_str(path: &str) -> Option<String> {
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

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

#[derive(Debug, Serialize)]
struct GuestConfig {
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

async fn guest_config(State(state): State<AppState>) -> Json<LegacyEnvelope<GuestConfig>> {
    let email_whitelist_suffix = if state.config.email_whitelist_enable {
        json!(state.config.email_whitelist_suffix)
    } else {
        json!(0)
    };

    legacy_data(GuestConfig {
        tos_url: state.config.tos_url,
        is_email_verify: state.config.email_verify as i32,
        is_invite_force: state.config.invite_force as i32,
        email_whitelist_suffix,
        is_recaptcha: state.config.recaptcha_enable as i32,
        recaptcha_site_key: state.config.recaptcha_site_key,
        app_description: state.config.app_description,
        app_url: state.config.app_url,
        logo: state.config.logo,
    })
}

#[derive(Debug, Deserialize)]
struct ClientSubscribeQuery {
    token: Option<String>,
    flag: Option<String>,
}

async fn client_subscribe(
    State(state): State<AppState>,
    Query(query): Query<ClientSubscribeQuery>,
    headers: HeaderMap,
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

    let subscription = build_subscription_document(&state.config, &user, &servers, &flag)?;
    let mut response = subscription.body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(subscription.content_type),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename*=UTF-8''{}",
            percent_encode(&state.config.app_name)
        ))
        .map_err(|_| ApiError::internal("invalid subscription filename"))?,
    );
    if let Some(app_url) = state.config.app_url.as_deref() {
        headers.insert(
            header::HeaderName::from_static("profile-web-page-url"),
            HeaderValue::from_str(app_url)
                .map_err(|_| ApiError::internal("invalid profile web page url"))?,
        );
    }
    headers.insert(
        header::HeaderName::from_static("profile-title"),
        HeaderValue::from_str(&format!(
            "base64:{}",
            standard_base64_encode(state.config.app_name.as_bytes())
        ))
        .map_err(|_| ApiError::internal("invalid profile title"))?,
    );
    headers.insert(
        header::HeaderName::from_static("subscription-userinfo"),
        HeaderValue::from_str(&format!(
            "upload={}; download={}; total={}; expire={}",
            user.u,
            user.d,
            user.transfer_enable,
            user.expired_at.unwrap_or_default()
        ))
        .map_err(|_| ApiError::internal("invalid subscription userinfo header"))?,
    );
    headers.insert(
        header::HeaderName::from_static("profile-update-interval"),
        HeaderValue::from_static("24"),
    );
    Ok(response)
}

async fn client_app_config(
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
    let proxy_names = servers
        .iter()
        .map(|server| server.name.as_str())
        .collect::<Vec<_>>();
    let mut body = String::from("mixed-port: 7890\nallow-lan: false\nmode: rule\nproxies:\n");
    for server in &servers {
        body.push_str(&format!(
            "  - name: \"{}\"\n    type: {}\n    server: \"{}\"\n    port: {}\n",
            yaml_escape(&server.name),
            server.r#type,
            yaml_escape(&server.host),
            server.port
        ));
    }
    body.push_str("proxy-groups:\n  - name: PROXY\n    type: select\n    proxies:\n");
    if proxy_names.is_empty() {
        body.push_str("      - DIRECT\n");
    } else {
        for name in proxy_names {
            body.push_str(&format!("      - \"{}\"\n", yaml_escape(name)));
        }
    }
    body.push_str("rules:\n  - MATCH,PROXY\n");
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/yaml; charset=utf-8"),
    );
    Ok(response)
}

async fn client_app_version(
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
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if ua.contains("tidalab/4.0.0") || ua.contains("tunnelab/4.0.0") {
        if ua.contains("Win64") {
            return Ok(legacy_data(json!({
                "version": state.config.windows_version,
                "download_url": state.config.windows_download_url,
            })));
        }
        return Ok(legacy_data(json!({
            "version": state.config.macos_version,
            "download_url": state.config.macos_download_url,
        })));
    }
    Ok(legacy_data(json!({
        "windows_version": state.config.windows_version,
        "windows_download_url": state.config.windows_download_url,
        "macos_version": state.config.macos_version,
        "macos_download_url": state.config.macos_download_url,
        "android_version": state.config.android_version,
        "android_download_url": state.config.android_download_url,
    })))
}

async fn payment_notify(
    State(state): State<AppState>,
    Path((method, uuid)): Path<(String, String)>,
    request: Request,
) -> Result<Response, ApiError> {
    let input = payment_request_input(request).await?;
    let service = v2board_domain::order::OrderService::new(state.db, state.config);
    let result = service.handle_payment_notify(&method, &uuid, input).await?;
    Ok(result.body.into_response())
}

async fn admin_get(
    State(state): State<AppState>,
    Path(admin_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _admin = require_admin(&state, &headers, params.get("auth_data").cloned()).await?;
    let service = v2board_domain::admin::AdminService::new(state.db, state.redis, state.config);
    admin_response(service.get(&admin_path, params).await?)
}

async fn admin_post(
    State(state): State<AppState>,
    Path(admin_path): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let admin = require_admin(&state, &headers, params.get("auth_data").cloned()).await?;
    params.insert("_admin_email".to_string(), admin.email);
    let service = v2board_domain::admin::AdminService::new(state.db, state.redis, state.config);
    admin_response(service.post(&admin_path, params).await?)
}

async fn staff_get(
    State(state): State<AppState>,
    Path(staff_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !staff_path_allowed(&staff_path, Method::GET) {
        return Err(ApiError::not_found("Staff endpoint does not exist"));
    }
    let _staff = require_staff(&state, &headers, params.get("auth_data").cloned()).await?;
    let service = v2board_domain::admin::AdminService::new(state.db, state.redis, state.config);
    admin_response(service.staff_get(&staff_path, params).await?)
}

async fn staff_post(
    State(state): State<AppState>,
    Path(staff_path): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    if !staff_path_allowed(&staff_path, Method::POST) {
        return Err(ApiError::not_found("Staff endpoint does not exist"));
    }
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let staff = require_staff(&state, &headers, params.get("auth_data").cloned()).await?;
    params.insert("_admin_email".to_string(), staff.email);
    let service = v2board_domain::admin::AdminService::new(state.db, state.redis, state.config);
    admin_response(service.staff_post(&staff_path, params).await?)
}

fn staff_path_allowed(path: &str, method: Method) -> bool {
    let path = path.trim_matches('/');
    match method {
        Method::GET => matches!(
            path,
            "ticket/fetch" | "user/getUserInfoById" | "plan/fetch" | "notice/fetch"
        ),
        Method::POST => matches!(
            path,
            "ticket/reply"
                | "ticket/close"
                | "user/update"
                | "user/sendMail"
                | "user/ban"
                | "notice/save"
                | "notice/update"
                | "notice/drop"
        ),
        _ => false,
    }
}

async fn telegram_webhook(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    request: Request,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = state
        .config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("telegram bot token is null"))?;
    let expected = format!("{:x}", md5::compute(token.as_bytes()));
    if query.get("access_token").map(String::as_str) != Some(expected.as_str()) {
        return Err(ApiError::Http {
            status: StatusCode::UNAUTHORIZED,
            message: "Unauthorized".to_string(),
        });
    }

    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid telegram webhook body"))?;
    let payload = if body.is_empty() {
        serde_json::Value::Object(Default::default())
    } else if content_type.contains("application/json") || body.first() == Some(&b'{') {
        serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|_| ApiError::bad_request("Invalid telegram webhook body"))?
    } else {
        let body = std::str::from_utf8(&body)
            .map_err(|_| ApiError::bad_request("Invalid telegram webhook body"))?;
        let params = parse_urlencoded_params(body)?;
        serde_json::to_value(params)
            .map_err(|_| ApiError::internal("telegram body encode failed"))?
    };

    if let Some(join) = payload.get("chat_join_request") {
        let telegram_id = join
            .get("from")
            .and_then(|from| from.get("id"))
            .and_then(value_to_i64);
        let chat_id = join
            .get("chat")
            .and_then(|chat| chat.get("id"))
            .and_then(value_to_i64);
        if let (Some(telegram_id), Some(chat_id)) = (telegram_id, chat_id) {
            let user = sqlx::query_as::<_, v2board_db::user::UserAccessRow>(
                r#"
                SELECT id, token, uuid, group_id, banned, u, d, transfer_enable, expired_at, commission_balance
                FROM v2_user
                WHERE telegram_id = ?
                LIMIT 1
                "#,
            )
            .bind(telegram_id)
            .fetch_optional(&state.db)
            .await?;
            let method = if user.as_ref().is_some_and(user_is_available) {
                "approveChatJoinRequest"
            } else {
                "declineChatJoinRequest"
            };
            telegram_chat_join_request(token, method, chat_id, telegram_id).await?;
        }
    }

    Ok(Json(json!({ "data": true })))
}

async fn server_v1(
    State(state): State<AppState>,
    Path((class, action)): Path<(String, String)>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let input = server_request_input(request).await?;
    validate_server_token(&state.config, &input.params)?;
    let class = class.to_ascii_lowercase();
    let action = action.to_ascii_lowercase();

    match (class.as_str(), action.as_str()) {
        ("uniproxy", "user") => server_uniproxy_user(&state, &headers, &input.params).await,
        ("uniproxy", "push") => {
            server_push(&state, &input.params, input.body.as_ref(), true, None).await
        }
        ("uniproxy", "alivelist") => server_alive_list(&state).await,
        ("uniproxy", "alive") => server_alive(&state, &input.params, input.body.as_ref()).await,
        ("uniproxy", "config") => server_uniproxy_config(&state, &headers, &input.params).await,
        ("shadowsockstidalab", "user") => {
            server_tidalab_user(&state, &headers, "shadowsocks", &input.params).await
        }
        ("shadowsockstidalab", "submit") => {
            server_push(
                &state,
                &input.params,
                input.body.as_ref(),
                false,
                Some("shadowsocks"),
            )
            .await
        }
        ("trojantidalab", "user") => {
            server_tidalab_user(&state, &headers, "trojan", &input.params).await
        }
        ("trojantidalab", "submit") => {
            server_push(
                &state,
                &input.params,
                input.body.as_ref(),
                false,
                Some("trojan"),
            )
            .await
        }
        ("trojantidalab", "config") => server_trojan_tidalab_config(&state, &input.params).await,
        ("deepbwork", "user") => {
            server_tidalab_user(&state, &headers, "vmess", &input.params).await
        }
        ("deepbwork", "submit") => {
            server_push(
                &state,
                &input.params,
                input.body.as_ref(),
                false,
                Some("vmess"),
            )
            .await
        }
        ("deepbwork", "config") => server_deepbwork_config(&state, &input.params).await,
        _ => Err(ApiError::not_found("Server route not found")),
    }
}

async fn server_v2_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let input = server_request_input(request).await?;
    if let Err(error) = validate_server_token(&state.config, &input.params) {
        return Ok(server_fail_response(error.to_string()));
    }
    let node_id = match required_i32_param(&input.params, "node_id") {
        Ok(node_id) => node_id,
        Err(error) => return Ok(server_fail_response(error.to_string())),
    };
    let Some(node) = load_server_node(&state.db, "v2node", node_id).await? else {
        return Ok(server_fail_response("server is not exist"));
    };
    let value = server_v2_config_value(&state, &node).await?;
    raw_value_response(value, &headers, false)
}

#[derive(Debug)]
struct ServerRequestInput {
    params: HashMap<String, String>,
    body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, FromRow)]
struct ServerNodeRow {
    id: i32,
    group_id: String,
    route_id: Option<String>,
    rate: String,
    host: String,
    server_port: i32,
    created_at: i64,
    listen_ip: Option<String>,
    protocol: Option<String>,
    version: Option<i32>,
    tls: Option<i8>,
    tls_settings: Option<String>,
    flow: Option<String>,
    network: Option<String>,
    network_settings: Option<String>,
    encryption: Option<String>,
    encryption_settings: Option<String>,
    zero_rtt_handshake: Option<i8>,
    congestion_control: Option<String>,
    cipher: Option<String>,
    obfs: Option<String>,
    obfs_settings: Option<String>,
    obfs_password: Option<String>,
    padding_scheme: Option<String>,
    server_name: Option<String>,
    up_mbps: Option<i32>,
    down_mbps: Option<i32>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
struct ServerUserRow {
    id: i64,
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_limit: Option<i32>,
}

#[derive(Debug, Clone)]
struct TrafficEntry {
    user_id: i64,
    u: i64,
    d: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ServerRouteRow {
    id: i32,
    match_text: String,
    action: String,
    action_value: Option<String>,
}

async fn server_request_input(request: Request) -> Result<ServerRequestInput, ApiError> {
    let mut params = HashMap::new();
    if let Some(query) = request.uri().query().filter(|query| !query.is_empty()) {
        params.extend(parse_urlencoded_params(query)?);
    }
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let body = to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid server request body"))?;
    if body.is_empty() {
        return Ok(ServerRequestInput { params, body: None });
    }
    if content_type.contains("application/json")
        || body.first() == Some(&b'{')
        || body.first() == Some(&b'[')
    {
        let value = serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|_| ApiError::bad_request("Invalid server request body"))?;
        flatten_admin_json(None, &value, &mut params);
        return Ok(ServerRequestInput {
            params,
            body: Some(value),
        });
    }
    let body = std::str::from_utf8(&body)
        .map_err(|_| ApiError::bad_request("Invalid server request body"))?;
    params.extend(parse_urlencoded_params(body)?);
    Ok(ServerRequestInput { params, body: None })
}

fn validate_server_token(
    config: &AppConfig,
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    let token = params
        .get("token")
        .map(String::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError::legacy("token is null"))?;
    if config.server_token.as_deref() != Some(token) {
        return Err(ApiError::legacy("token is error"));
    }
    Ok(())
}

async fn server_uniproxy_user(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    server_cache_timestamp(&state.redis, "LAST_CHECK_AT", &node_type, node.id).await?;
    let users =
        server_available_users(&state.db, parse_i32_json_list(Some(&node.group_id))).await?;
    raw_value_response(
        json!({ "users": users }),
        headers,
        response_wants_msgpack(headers),
    )
}

async fn server_tidalab_user(
    state: &AppState,
    headers: &HeaderMap,
    node_type: &str,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let Some(node) = load_server_node(&state.db, node_type, node_id).await? else {
        return Err(ApiError::legacy("fail"));
    };
    server_cache_timestamp(&state.redis, "LAST_CHECK_AT", node_type, node.id).await?;
    let users =
        server_available_users(&state.db, parse_i32_json_list(Some(&node.group_id))).await?;
    let value = match node_type {
        "shadowsocks" => {
            let data = users
                .iter()
                .map(|user| {
                    json!({
                        "id": user.id,
                        "port": node.server_port,
                        "cipher": node.cipher,
                        "secret": user.uuid,
                    })
                })
                .collect::<Vec<_>>();
            json!({ "data": data })
        }
        "trojan" => {
            let data = users
                .iter()
                .map(|user| {
                    let mut item = server_user_without_uuid(user);
                    item.insert("trojan_user".to_string(), json!({ "password": user.uuid }));
                    serde_json::Value::Object(item)
                })
                .collect::<Vec<_>>();
            json!({ "msg": "ok", "data": data })
        }
        "vmess" => {
            let data = users
                .iter()
                .map(|user| {
                    let mut item = server_user_without_uuid(user);
                    item.insert(
                        "v2ray_user".to_string(),
                        json!({
                            "uuid": user.uuid,
                            "email": format!("{}@v2board.user", user.uuid),
                            "alter_id": 0,
                            "level": 0,
                        }),
                    );
                    serde_json::Value::Object(item)
                })
                .collect::<Vec<_>>();
            json!({ "msg": "ok", "data": data })
        }
        _ => json!({ "msg": "ok", "data": users }),
    };
    raw_value_response(value, headers, false)
}

async fn server_push(
    state: &AppState,
    params: &HashMap<String, String>,
    body: Option<&serde_json::Value>,
    uniproxy: bool,
    fallback_node_type: Option<&str>,
) -> Result<Response, ApiError> {
    let (node_type, node) = if uniproxy {
        load_uniproxy_node(state, params).await?
    } else {
        let node_type = match params
            .get("node_type")
            .map(String::as_str)
            .map(normalize_server_node_type)
        {
            Some(node_type) => node_type,
            None => fallback_node_type.unwrap_or("shadowsocks").to_string(),
        };
        let node_id = required_i32_param(params, "node_id")?;
        let Some(node) = load_server_node(&state.db, &node_type, node_id).await? else {
            return Ok(Json(json!({ "ret": 0, "msg": "server is not found" })).into_response());
        };
        (node_type, node)
    };

    let entries = parse_traffic_entries(body, params);
    server_cache_count(
        &state.redis,
        "ONLINE_USER",
        &node_type,
        node.id,
        entries.len() as i64,
    )
    .await?;
    server_cache_timestamp(&state.redis, "LAST_PUSH_AT", &node_type, node.id).await?;
    if !entries.is_empty() {
        persist_traffic_fetch(state, &node, &node_type, &entries).await?;
    }

    if uniproxy {
        Ok(Json(json!({ "data": true })).into_response())
    } else {
        Ok(Json(json!({ "ret": 1, "msg": "ok" })).into_response())
    }
}

async fn server_alive_list(state: &AppState) -> Result<Response, ApiError> {
    let user_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM v2_user
        WHERE u + d < transfer_enable
          AND (expired_at >= ? OR expired_at IS NULL)
          AND banned = 0
          AND device_limit > 0
        "#,
    )
    .bind(Utc::now().timestamp())
    .fetch_all(&state.db)
    .await?;

    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let mut alive = serde_json::Map::new();
    for user_id in user_ids {
        let key = format!("ALIVE_IP_USER_{user_id}");
        if let Some(value) = conn.get::<_, Option<String>>(&key).await?
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&value)
            && let Some(alive_ip) = value.get("alive_ip").and_then(value_to_i64)
        {
            alive.insert(user_id.to_string(), json!(alive_ip));
        }
    }
    Ok(Json(json!({ "alive": alive })).into_response())
}

async fn server_alive(
    state: &AppState,
    params: &HashMap<String, String>,
    body: Option<&serde_json::Value>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    let Some(object) = body.and_then(serde_json::Value::as_object) else {
        return Ok(Json(json!({ "data": true })).into_response());
    };

    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let now = Utc::now().timestamp();
    for (uid, ips) in object {
        let Some(user_id) = uid.parse::<i64>().ok() else {
            continue;
        };
        let Some(ips) = ips.as_array() else {
            continue;
        };
        let key = format!("ALIVE_IP_USER_{user_id}");
        let mut value = conn
            .get::<_, Option<String>>(&key)
            .await?
            .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        value.insert(
            format!("{node_type}{}", node.id),
            json!({
                "aliveips": ips,
                "lastupdateAt": now,
            }),
        );
        let stale_keys = value
            .iter()
            .filter_map(|(key, value)| {
                if key == "alive_ip" {
                    return None;
                }
                let last = value
                    .get("lastupdateAt")
                    .and_then(value_to_i64)
                    .unwrap_or(0);
                (now - last > 100).then_some(key.clone())
            })
            .collect::<Vec<_>>();
        for key in stale_keys {
            value.remove(&key);
        }
        let alive_ip = value
            .iter()
            .filter(|(key, _)| key.as_str() != "alive_ip")
            .filter_map(|(_, value)| value.get("aliveips").and_then(serde_json::Value::as_array))
            .map(Vec::len)
            .sum::<usize>();
        value.insert("alive_ip".to_string(), json!(alive_ip));
        let _: () = conn
            .set_ex(key, serde_json::Value::Object(value).to_string(), 120)
            .await?;
    }
    Ok(Json(json!({ "data": true })).into_response())
}

async fn server_uniproxy_config(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    let value = server_v1_config_value(state, &node_type, &node).await?;
    raw_value_response(value, headers, false)
}

async fn server_trojan_tidalab_config(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let local_port = params
        .get("local_port")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))?;
    let node = load_server_node(&state.db, "trojan", node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("节点不存在"))?;
    Ok(Json(json!({
        "run_type": "server",
        "local_addr": "0.0.0.0",
        "local_port": node.server_port,
        "remote_addr": "www.taobao.com",
        "remote_port": 80,
        "password": [],
        "ssl": {
            "cert": "/root/.cert/server.crt",
            "key": "/root/.cert/server.key",
            "sni": node.server_name.as_deref().unwrap_or(&node.host),
        },
        "api": {
            "enabled": true,
            "api_addr": "127.0.0.1",
            "api_port": local_port,
        }
    }))
    .into_response())
}

async fn server_deepbwork_config(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    let node_id = required_i32_param(params, "node_id")?;
    let local_port = params
        .get("local_port")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))?;
    let node = load_server_node(&state.db, "vmess", node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("节点不存在"))?;
    let network = node.network.as_deref().unwrap_or("tcp");
    let mut stream_settings = serde_json::Map::new();
    stream_settings.insert("network".to_string(), json!(network));
    if let Some(settings) = json_text(node.network_settings.as_deref())
        .as_object()
        .cloned()
    {
        let key = match network {
            "kcp" => "kcpSettings",
            "ws" => "wsSettings",
            "http" => "httpSettings",
            "domainsocket" => "dsSettings",
            "quic" => "quicSettings",
            "grpc" => "grpcSettings",
            _ => "tcpSettings",
        };
        stream_settings.insert(key.to_string(), serde_json::Value::Object(settings));
    }
    if node.tls.unwrap_or_default() != 0 {
        stream_settings.insert("security".to_string(), json!("tls"));
        stream_settings.insert(
            "tlsSettings".to_string(),
            json!({
                "certificates": [{
                    "certificateFile": "/root/.cert/server.crt",
                    "keyFile": "/root/.cert/server.key",
                }]
            }),
        );
    }
    Ok(Json(json!({
        "log": { "loglevel": "none", "access": "access.log", "error": "error.log" },
        "api": { "services": ["HandlerService", "StatsService"], "tag": "api" },
        "dns": {},
        "stats": {},
        "inbounds": [
            {
                "port": node.server_port,
                "protocol": "vmess",
                "settings": { "clients": [] },
                "sniffing": { "enabled": true, "destOverride": ["http", "tls"] },
                "streamSettings": serde_json::Value::Object(stream_settings),
                "tag": "proxy",
            },
            {
                "listen": "127.0.0.1",
                "port": local_port,
                "protocol": "dokodemo-door",
                "settings": { "address": "0.0.0.0" },
                "tag": "api",
            }
        ],
        "outbounds": [
            { "protocol": "freedom", "settings": {} },
            { "protocol": "blackhole", "settings": {}, "tag": "block" }
        ],
        "routing": { "rules": [{ "type": "field", "inboundTag": "api", "outboundTag": "api" }] },
        "policy": {
            "levels": {
                "0": {
                    "handshake": 4,
                    "connIdle": 300,
                    "uplinkOnly": 5,
                    "downlinkOnly": 30,
                    "statsUserUplink": true,
                    "statsUserDownlink": true,
                }
            }
        }
    }))
    .into_response())
}

fn admin_response(output: v2board_domain::admin::AdminOutput) -> Result<Response, ApiError> {
    match output {
        v2board_domain::admin::AdminOutput::Data(data) => Ok(legacy_data(data).into_response()),
        v2board_domain::admin::AdminOutput::Page { data, total } => {
            Ok(legacy_page(data, total).into_response())
        }
        v2board_domain::admin::AdminOutput::Csv { filename, body } => {
            let mut response = body.into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                    .map_err(|_| ApiError::internal("invalid csv filename"))?,
            );
            Ok(response)
        }
    }
}

fn response_wants_msgpack(headers: &HeaderMap) -> bool {
    headers
        .get("x-response-format")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("msgpack"))
}

fn raw_value_response(
    value: serde_json::Value,
    headers: &HeaderMap,
    msgpack: bool,
) -> Result<Response, ApiError> {
    if msgpack {
        let body = rmp_serde::to_vec_named(&value)
            .map_err(|_| ApiError::internal("msgpack encode failed"))?;
        let etag = sha1_hex(&body);
        if etag_matches(headers, &etag) {
            return not_modified_response(&etag);
        }
        let mut response = body.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-msgpack"),
        );
        insert_etag(response.headers_mut(), &etag)?;
        return Ok(response);
    }

    let body = serde_json::to_vec(&value).map_err(|_| ApiError::internal("json encode failed"))?;
    let etag = sha1_hex(&body);
    if etag_matches(headers, &etag) {
        return not_modified_response(&etag);
    }
    let mut response = Json(value).into_response();
    insert_etag(response.headers_mut(), &etag)?;
    Ok(response)
}

fn server_fail_response(message: impl Into<String>) -> Response {
    Json(json!({
        "status": "fail",
        "message": message.into(),
    }))
    .into_response()
}

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains(etag))
}

fn not_modified_response(etag: &str) -> Result<Response, ApiError> {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    insert_etag(response.headers_mut(), etag)?;
    Ok(response)
}

fn insert_etag(headers: &mut HeaderMap, etag: &str) -> Result<(), ApiError> {
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{etag}\""))
            .map_err(|_| ApiError::internal("invalid etag"))?,
    );
    Ok(())
}

async fn telegram_chat_join_request(
    bot_token: &str,
    method: &str,
    chat_id: i64,
    user_id: i64,
) -> Result<(), ApiError> {
    let body = serde_urlencoded::to_string([
        ("chat_id", chat_id.to_string()),
        ("user_id", user_id.to_string()),
    ])
    .map_err(|_| ApiError::internal("telegram request encode failed"))?;
    let response = reqwest::Client::new()
        .post(format!("https://api.telegram.org/bot{bot_token}/{method}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|error| ApiError::legacy(format!("Telegram request failed: {error}")))?;
    if !response.status().is_success() {
        return Err(ApiError::legacy("Telegram request failed"));
    }
    Ok(())
}

fn required_i32_param(params: &HashMap<String, String>, key: &str) -> Result<i32, ApiError> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| ApiError::legacy("参数错误"))
}

fn normalize_server_node_type(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "v2ray" => "vmess".to_string(),
        "hysteria2" => "hysteria".to_string(),
        value => value.to_string(),
    }
}

async fn load_uniproxy_node(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<(String, ServerNodeRow), ApiError> {
    let node_type = params
        .get("node_type")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_server_node_type)
        .ok_or_else(|| ApiError::legacy("server is not exist"))?;
    let node_id = required_i32_param(params, "node_id")?;
    let node = load_server_node(&state.db, &node_type, node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("server is not exist"))?;
    Ok((node_type, node))
}

async fn load_server_node(
    db: &DbPool,
    node_type: &str,
    node_id: i32,
) -> Result<Option<ServerNodeRow>, ApiError> {
    let Some(sql) = server_node_sql(node_type) else {
        return Ok(None);
    };
    Ok(
        sqlx::query_as::<_, ServerNodeRow>(AssertSqlSafe(sql.to_string()))
            .bind(node_id)
            .fetch_optional(db)
            .await?,
    )
}

fn server_node_sql(node_type: &str) -> Option<&'static str> {
    match node_type {
        "shadowsocks" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, NULL AS disable_sni,
                   NULL AS udp_relay_mode, NULL AS zero_rtt_handshake, NULL AS congestion_control,
                   cipher, obfs, obfs_settings, NULL AS obfs_password, NULL AS padding_scheme,
                   NULL AS allow_insecure, NULL AS server_name, NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_shadowsocks
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "vmess" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, tls,
                   tlsSettings AS tls_settings, NULL AS flow, network,
                   networkSettings AS network_settings, NULL AS encryption,
                   NULL AS encryption_settings, NULL AS disable_sni, NULL AS udp_relay_mode,
                   NULL AS zero_rtt_handshake, NULL AS congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, NULL AS allow_insecure, NULL AS server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_vmess
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "trojan" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, network,
                   network_settings, NULL AS encryption, NULL AS encryption_settings,
                   NULL AS disable_sni, NULL AS udp_relay_mode, NULL AS zero_rtt_handshake,
                   NULL AS congestion_control, NULL AS cipher, NULL AS obfs,
                   NULL AS obfs_settings, NULL AS obfs_password, NULL AS padding_scheme,
                   allow_insecure, server_name, NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_trojan
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "vless" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, tls,
                   tls_settings, flow, network, network_settings, encryption,
                   encryption_settings, NULL AS disable_sni, NULL AS udp_relay_mode,
                   NULL AS zero_rtt_handshake, NULL AS congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, NULL AS allow_insecure, NULL AS server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_vless
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "tuic" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, disable_sni,
                   udp_relay_mode, zero_rtt_handshake, congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, insecure AS allow_insecure, server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_tuic
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "hysteria" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, NULL AS disable_sni,
                   NULL AS udp_relay_mode, NULL AS zero_rtt_handshake, NULL AS congestion_control,
                   NULL AS cipher, obfs, NULL AS obfs_settings, obfs_password,
                   NULL AS padding_scheme, insecure AS allow_insecure, server_name,
                   up_mbps, down_mbps
            FROM v2_server_hysteria
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "anytls" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL AS version, NULL AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, NULL AS disable_sni,
                   NULL AS udp_relay_mode, NULL AS zero_rtt_handshake, NULL AS congestion_control,
                   NULL AS cipher, NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   padding_scheme, insecure AS allow_insecure, server_name,
                   NULL AS up_mbps, NULL AS down_mbps
            FROM v2_server_anytls
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        "v2node" => Some(
            r#"
            SELECT id, group_id, route_id, name, rate, host, CAST(port AS CHAR) AS port,
                   server_port, created_at, listen_ip, protocol, NULL AS version, tls,
                   tls_settings, flow, network, network_settings, encryption,
                   encryption_settings, disable_sni, udp_relay_mode, zero_rtt_handshake,
                   congestion_control, cipher, obfs, NULL AS obfs_settings, obfs_password,
                   padding_scheme, NULL AS allow_insecure, NULL AS server_name,
                   up_mbps, down_mbps
            FROM v2_server_v2node
            WHERE id = ?
            LIMIT 1
            "#,
        ),
        _ => None,
    }
}

async fn server_available_users(
    db: &DbPool,
    group_ids: Vec<i32>,
) -> Result<Vec<ServerUserRow>, ApiError> {
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<MySql>::new(
        "SELECT id, uuid, speed_limit, device_limit FROM v2_user WHERE group_id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for group_id in group_ids {
            separated.push_bind(group_id);
        }
    }
    builder.push(") AND u + d < transfer_enable AND (expired_at >= ");
    builder.push_bind(Utc::now().timestamp());
    builder.push(" OR expired_at IS NULL) AND banned = 0");
    Ok(builder
        .build_query_as::<ServerUserRow>()
        .fetch_all(db)
        .await?)
}

fn server_user_without_uuid(user: &ServerUserRow) -> serde_json::Map<String, serde_json::Value> {
    let mut item = serde_json::Map::new();
    item.insert("id".to_string(), json!(user.id));
    if let Some(speed_limit) = user.speed_limit {
        item.insert("speed_limit".to_string(), json!(speed_limit));
    }
    if let Some(device_limit) = user.device_limit {
        item.insert("device_limit".to_string(), json!(device_limit));
    }
    item
}

async fn server_cache_timestamp(
    redis: &redis::Client,
    suffix: &str,
    node_type: &str,
    node_id: i32,
) -> Result<(), ApiError> {
    server_cache_count(redis, suffix, node_type, node_id, Utc::now().timestamp()).await
}

async fn server_cache_count(
    redis: &redis::Client,
    suffix: &str,
    node_type: &str,
    node_id: i32,
    value: i64,
) -> Result<(), ApiError> {
    let key = format!(
        "SERVER_{}_{}_{node_id}",
        node_type.to_ascii_uppercase(),
        suffix
    );
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let _: () = conn.set_ex(key, value, 3600).await?;
    Ok(())
}

fn parse_traffic_entries(
    body: Option<&serde_json::Value>,
    params: &HashMap<String, String>,
) -> Vec<TrafficEntry> {
    if let Some(value) = body {
        let entries = traffic_entries_from_value(value);
        if !entries.is_empty() {
            return entries;
        }
    }
    match (
        params
            .get("user_id")
            .and_then(|value| value.parse::<i64>().ok()),
        params.get("u").and_then(|value| value.parse::<i64>().ok()),
        params.get("d").and_then(|value| value.parse::<i64>().ok()),
    ) {
        (Some(user_id), Some(u), Some(d)) => vec![TrafficEntry { user_id, u, d }],
        _ => Vec::new(),
    }
}

fn traffic_entries_from_value(value: &serde_json::Value) -> Vec<TrafficEntry> {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                let object = item.as_object()?;
                Some(TrafficEntry {
                    user_id: object.get("user_id").and_then(value_to_i64)?,
                    u: object.get("u").and_then(value_to_i64).unwrap_or_default(),
                    d: object.get("d").and_then(value_to_i64).unwrap_or_default(),
                })
            })
            .collect(),
        serde_json::Value::Object(object) => {
            if let Some(user_id) = object.get("user_id").and_then(value_to_i64) {
                return vec![TrafficEntry {
                    user_id,
                    u: object.get("u").and_then(value_to_i64).unwrap_or_default(),
                    d: object.get("d").and_then(value_to_i64).unwrap_or_default(),
                }];
            }
            object
                .iter()
                .filter_map(|(user_id, value)| {
                    let user_id = user_id.parse::<i64>().ok()?;
                    let (u, d) = traffic_pair_from_value(value)?;
                    Some(TrafficEntry { user_id, u, d })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn traffic_pair_from_value(value: &serde_json::Value) -> Option<(i64, i64)> {
    match value {
        serde_json::Value::Array(items) => Some((
            items.first().and_then(value_to_i64).unwrap_or_default(),
            items.get(1).and_then(value_to_i64).unwrap_or_default(),
        )),
        serde_json::Value::Object(object) => Some((
            object.get("u").and_then(value_to_i64).unwrap_or_default(),
            object.get("d").and_then(value_to_i64).unwrap_or_default(),
        )),
        _ => None,
    }
}

async fn persist_traffic_fetch(
    state: &AppState,
    node: &ServerNodeRow,
    node_type: &str,
    entries: &[TrafficEntry],
) -> Result<(), ApiError> {
    let rate = node.rate.parse::<f64>().unwrap_or(1.0);
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    for entry in entries {
        let upload = (entry.u as f64 * rate).round() as i64;
        let download = (entry.d as f64 * rate).round() as i64;
        let _: () = redis::cmd("HINCRBY")
            .arg("v2board_upload_traffic")
            .arg(entry.user_id)
            .arg(upload)
            .query_async(&mut conn)
            .await?;
        let _: () = redis::cmd("HINCRBY")
            .arg("v2board_download_traffic")
            .arg(entry.user_id)
            .arg(download)
            .query_async(&mut conn)
            .await?;
    }

    let record_at = today_start_timestamp();
    let now = Utc::now().timestamp();
    let mut total_u = 0_i64;
    let mut total_d = 0_i64;
    for entry in entries {
        total_u += entry.u;
        total_d += entry.d;
        sqlx::query(
            r#"
            INSERT INTO v2_stat_user
                (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'd', ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                u = u + VALUES(u),
                d = d + VALUES(d),
                updated_at = VALUES(updated_at)
            "#,
        )
        .bind(entry.user_id)
        .bind(rate)
        .bind(entry.u)
        .bind(entry.d)
        .bind(record_at)
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO v2_stat_server
            (server_id, server_type, u, d, record_type, record_at, created_at, updated_at)
        VALUES (?, ?, ?, ?, 'd', ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            u = u + VALUES(u),
            d = d + VALUES(d),
            updated_at = VALUES(updated_at)
        "#,
    )
    .bind(node.id)
    .bind(node_type)
    .bind(total_u)
    .bind(total_d)
    .bind(record_at)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn server_v1_config_value(
    state: &AppState,
    node_type: &str,
    node: &ServerNodeRow,
) -> Result<serde_json::Value, ApiError> {
    let mut response = serde_json::Map::new();
    match node_type {
        "shadowsocks" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("cipher".to_string(), json!(node.cipher));
            response.insert("obfs".to_string(), json!(node.obfs));
            response.insert(
                "obfs_settings".to_string(),
                json_text(node.obfs_settings.as_deref()),
            );
            if let Some(cipher) = node.cipher.as_deref() {
                if cipher == "2022-blake3-aes-128-gcm" {
                    response.insert(
                        "server_key".to_string(),
                        json!(server_key(node.created_at, 16)),
                    );
                } else if cipher == "2022-blake3-aes-256-gcm" {
                    response.insert(
                        "server_key".to_string(),
                        json!(server_key(node.created_at, 32)),
                    );
                }
            }
        }
        "vmess" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings.as_deref()),
            );
            response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
        }
        "vless" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings.as_deref()),
            );
            response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
            response.insert("flow".to_string(), json!(node.flow));
            response.insert(
                "tls_settings".to_string(),
                json_text(node.tls_settings.as_deref()),
            );
            response.insert("encryption".to_string(), json!(node.encryption));
            response.insert(
                "encryption_settings".to_string(),
                json_text(node.encryption_settings.as_deref()),
            );
        }
        "trojan" => {
            response.insert("host".to_string(), json!(node.host));
            response.insert("network".to_string(), json!(node.network));
            response.insert(
                "networkSettings".to_string(),
                json_text(node.network_settings.as_deref()),
            );
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
        }
        "tuic" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
            response.insert(
                "congestion_control".to_string(),
                json!(node.congestion_control),
            );
            response.insert(
                "zero_rtt_handshake".to_string(),
                json!(node.zero_rtt_handshake.unwrap_or_default() != 0),
            );
        }
        "hysteria" => {
            let version = node.version.unwrap_or(2);
            response.insert("version".to_string(), json!(version));
            response.insert("host".to_string(), json!(node.host));
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
            response.insert(
                "up_mbps".to_string(),
                json!(node.up_mbps.unwrap_or_default()),
            );
            response.insert(
                "down_mbps".to_string(),
                json!(node.down_mbps.unwrap_or_default()),
            );
            if version == 1 {
                response.insert("obfs".to_string(), json!(node.obfs_password));
            } else {
                let ignore = node.up_mbps.unwrap_or_default() == 0
                    && node.down_mbps.unwrap_or_default() == 0;
                response.insert("ignore_client_bandwidth".to_string(), json!(ignore));
                response.insert("obfs".to_string(), json!(node.obfs));
                response.insert("obfs-password".to_string(), json!(node.obfs_password));
            }
        }
        "anytls" => {
            response.insert("server_port".to_string(), json!(node.server_port));
            response.insert("server_name".to_string(), json!(node.server_name));
            response.insert(
                "padding_scheme".to_string(),
                json_text(node.padding_scheme.as_deref()),
            );
        }
        _ => {}
    }
    response.insert(
        "base_config".to_string(),
        json!({
            "push_interval": state.config.server_push_interval,
            "pull_interval": state.config.server_pull_interval,
        }),
    );
    let routes = server_routes(&state.db, parse_i32_json_list(node.route_id.as_ref())).await?;
    if !routes.is_empty() {
        response.insert("routes".to_string(), json!(routes));
    }
    Ok(serde_json::Value::Object(response))
}

async fn server_v2_config_value(
    state: &AppState,
    node: &ServerNodeRow,
) -> Result<serde_json::Value, ApiError> {
    let mut response = serde_json::Map::new();
    response.insert("listen_ip".to_string(), json!(node.listen_ip));
    response.insert("server_port".to_string(), json!(node.server_port));
    response.insert("network".to_string(), json!(node.network));
    response.insert(
        "network_settings".to_string(),
        json_text(node.network_settings.as_deref()),
    );
    response.insert("protocol".to_string(), json!(node.protocol));
    response.insert("tls".to_string(), json!(node.tls.unwrap_or_default()));
    response.insert(
        "tls_settings".to_string(),
        json_text(node.tls_settings.as_deref()),
    );
    response.insert("encryption".to_string(), json!(node.encryption));
    response.insert(
        "encryption_settings".to_string(),
        json_text(node.encryption_settings.as_deref()),
    );
    response.insert("flow".to_string(), json!(node.flow));
    response.insert("cipher".to_string(), json!(node.cipher));
    response.insert(
        "congestion_control".to_string(),
        json!(node.congestion_control),
    );
    response.insert(
        "zero_rtt_handshake".to_string(),
        json!(node.zero_rtt_handshake.unwrap_or_default() != 0),
    );
    response.insert(
        "up_mbps".to_string(),
        json!(node.up_mbps.unwrap_or_default()),
    );
    response.insert(
        "down_mbps".to_string(),
        json!(node.down_mbps.unwrap_or_default()),
    );
    response.insert("obfs".to_string(), json!(node.obfs));
    response.insert("obfs_password".to_string(), json!(node.obfs_password));
    response.insert(
        "padding_scheme".to_string(),
        json_text(node.padding_scheme.as_deref()),
    );
    if let Some(cipher) = node.cipher.as_deref() {
        if cipher == "2022-blake3-aes-128-gcm" {
            response.insert(
                "server_key".to_string(),
                json!(server_key(node.created_at, 16)),
            );
        } else if cipher == "2022-blake3-aes-256-gcm" {
            response.insert(
                "server_key".to_string(),
                json!(server_key(node.created_at, 32)),
            );
        }
    }
    response.insert(
        "ignore_client_bandwidth".to_string(),
        json!(node.up_mbps.unwrap_or_default() == 0 && node.down_mbps.unwrap_or_default() == 0),
    );
    response.insert(
        "base_config".to_string(),
        json!({
            "push_interval": state.config.server_push_interval,
            "pull_interval": state.config.server_pull_interval,
            "node_report_min_traffic": state.config.server_node_report_min_traffic,
            "device_online_min_traffic": state.config.server_device_online_min_traffic,
        }),
    );
    let routes = server_routes(&state.db, parse_i32_json_list(node.route_id.as_ref())).await?;
    if !routes.is_empty() {
        response.insert("routes".to_string(), json!(routes));
    }
    Ok(serde_json::Value::Object(response))
}

async fn server_routes(
    db: &DbPool,
    route_ids: Vec<i32>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    if route_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<MySql>::new(
        "SELECT id, `match` AS match_text, action, action_value FROM v2_server_route WHERE id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for route_id in &route_ids {
            separated.push_bind(*route_id);
        }
    }
    builder.push(") ORDER BY FIELD(id, ");
    {
        let mut separated = builder.separated(", ");
        for route_id in &route_ids {
            separated.push_bind(*route_id);
        }
    }
    builder.push(")");
    let rows = builder
        .build_query_as::<ServerRouteRow>()
        .fetch_all(db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "match": json_text(Some(&row.match_text)),
                "action": row.action,
                "action_value": row.action_value,
            })
        })
        .collect())
}

fn json_text(value: Option<&str>) -> serde_json::Value {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn server_key(created_at: i64, length: usize) -> String {
    let seed = format!("{:x}", md5::compute(created_at.to_string().as_bytes()));
    standard_base64_encode(prefix_bytes(&seed, length))
}

fn parse_i32_json_list(value: Option<&String>) -> Vec<i32> {
    let Some(value) = value
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<i32>>(value)
        .ok()
        .or_else(|| value.parse::<i32>().ok().map(|value| vec![value]))
        .unwrap_or_default()
}

fn today_start_timestamp() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Form(payload): Form<LoginRequest>,
) -> Result<Json<LegacyEnvelope<v2board_domain::auth::AuthData>>, ApiError> {
    let auth = AuthService::new(state.db, state.redis, state.config);
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let ip = Some(addr.ip().to_string());
    let data = auth
        .login(&payload.email, &payload.password, ip, user_agent)
        .await?;
    Ok(legacy_data(data))
}

async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Form(payload): Form<RegisterInput>,
) -> Result<Json<LegacyEnvelope<v2board_domain::auth::AuthData>>, ApiError> {
    let auth = AuthService::new(state.db, state.redis, state.config);
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let data = auth
        .register(payload, Some(addr.ip().to_string()), user_agent)
        .await?;
    Ok(legacy_data(data))
}

#[derive(Debug, Deserialize)]
struct Token2LoginQuery {
    token: Option<String>,
    verify: Option<String>,
    redirect: Option<String>,
}

async fn token2_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<Token2LoginQuery>,
) -> Result<Response, ApiError> {
    let auth = AuthService::new(state.db, state.redis, state.config);
    if let Some(token) = query.token.as_deref().filter(|value| !value.is_empty()) {
        let url = auth.login_redirect_url(token, query.redirect.as_deref());
        return Ok(Redirect::temporary(&url).into_response());
    }
    if let Some(verify) = query.verify.as_deref().filter(|value| !value.is_empty()) {
        let user_agent = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let data = auth
            .token_login(verify, Some(addr.ip().to_string()), user_agent)
            .await?;
        return Ok(legacy_data(data).into_response());
    }
    Err(ApiError::bad_request("Token error"))
}

async fn forget_password(
    State(state): State<AppState>,
    Form(payload): Form<ForgetInput>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = AuthService::new(state.db, state.redis, state.config);
    Ok(legacy_data(auth.forget(payload).await?))
}

#[derive(Debug, Deserialize)]
struct QuickLoginRequest {
    auth_data: Option<String>,
    redirect: Option<String>,
}

async fn passport_quick_login_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<QuickLoginRequest>,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, payload.auth_data).await?;
    let auth = AuthService::new(state.db, state.redis, state.config);
    let url = auth
        .quick_login_url(user.id, payload.redirect.as_deref())
        .await?;
    Ok(legacy_data(url))
}

async fn send_email_verify(
    State(state): State<AppState>,
    Form(payload): Form<EmailVerifyInput>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = AuthService::new(state.db, state.redis, state.config);
    Ok(legacy_data(auth.send_email_verify(payload).await?))
}

#[derive(Debug, Deserialize)]
struct PassportPvRequest {
    invite_code: Option<String>,
}

async fn passport_pv(
    State(state): State<AppState>,
    Form(payload): Form<PassportPvRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = AuthService::new(state.db, state.redis, state.config);
    Ok(legacy_data(
        auth.passport_pv(payload.invite_code.as_deref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
struct AuthQuery {
    auth_data: Option<String>,
}

async fn user_info(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::user::UserInfoRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let info = v2board_db::user::find_user_info(&state.db, user.id)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(legacy_data(info))
}

#[derive(Debug, Serialize)]
struct CheckLoginResult {
    is_login: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_admin: Option<bool>,
}

async fn check_login(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<CheckLoginResult>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    Ok(legacy_data(CheckLoginResult {
        is_login: true,
        is_admin: (user.is_admin != 0).then_some(true),
    }))
}

async fn user_stat(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<[i64; 3]>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let pending_orders = v2board_db::user::count_pending_orders(&state.db, user.id).await?;
    let pending_tickets = v2board_db::user::count_pending_tickets(&state.db, user.id).await?;
    let invited_users = v2board_db::user::count_invited_users(&state.db, user.id).await?;
    Ok(legacy_data([
        pending_orders,
        pending_tickets,
        invited_users,
    ]))
}

#[derive(Debug, Deserialize)]
struct UserUpdateRequest {
    auto_renewal: Option<i8>,
    remind_expire: Option<i8>,
    remind_traffic: Option<i8>,
}

async fn user_update(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<UserUpdateRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    validate_binary("auto_renewal", payload.auto_renewal)?;
    validate_binary("remind_expire", payload.remind_expire)?;
    validate_binary("remind_traffic", payload.remind_traffic)?;

    v2board_db::user::update_preferences(
        &state.db,
        user.id,
        payload.auto_renewal,
        payload.remind_expire,
        payload.remind_traffic,
        Utc::now().timestamp(),
    )
    .await?;
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<ChangePasswordRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = AuthService::new(state.db, state.redis, state.config);
    auth.change_password(user.id, &payload.old_password, &payload.new_password)
        .await?;
    Ok(legacy_data(true))
}

async fn reset_security(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = AuthService::new(state.db, state.redis, state.config);
    let subscribe_url = auth.reset_security(user.id).await?;
    Ok(legacy_data(subscribe_url))
}

async fn unbind_telegram(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let updated =
        v2board_db::user::clear_telegram_id(&state.db, user.id, Utc::now().timestamp()).await?;
    if !updated {
        return Err(ApiError::legacy("Unbind telegram failed"));
    }
    Ok(legacy_data(true))
}

async fn active_sessions(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<serde_json::Map<String, serde_json::Value>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = AuthService::new(state.db, state.redis, state.config);
    let sessions = auth.sessions(user.id).await?;
    Ok(legacy_data(sessions))
}

#[derive(Debug, Deserialize)]
struct RemoveActiveSessionRequest {
    session_id: String,
}

async fn remove_active_session(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<RemoveActiveSessionRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = AuthService::new(state.db, state.redis, state.config);
    let removed = auth.remove_session(user.id, &payload.session_id).await?;
    Ok(legacy_data(removed))
}

#[derive(Debug, Serialize)]
struct UserCommConfig {
    is_telegram: i32,
    telegram_discuss_link: Option<String>,
    stripe_pk: Option<String>,
    withdraw_methods: Vec<String>,
    withdraw_close: i32,
    currency: String,
    currency_symbol: String,
    commission_distribution_enable: i32,
    commission_distribution_l1: Option<String>,
    commission_distribution_l2: Option<String>,
    commission_distribution_l3: Option<String>,
}

async fn user_comm_config(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<UserCommConfig>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    Ok(legacy_data(UserCommConfig {
        is_telegram: state.config.telegram_bot_enable as i32,
        telegram_discuss_link: state.config.telegram_discuss_link,
        stripe_pk: state.config.stripe_pk_live,
        withdraw_methods: state.config.commission_withdraw_method,
        withdraw_close: state.config.withdraw_close_enable as i32,
        currency: state.config.currency,
        currency_symbol: state.config.currency_symbol,
        commission_distribution_enable: state.config.commission_distribution_enable as i32,
        commission_distribution_l1: state.config.commission_distribution_l1,
        commission_distribution_l2: state.config.commission_distribution_l2,
        commission_distribution_l3: state.config.commission_distribution_l3,
    }))
}

#[derive(Debug, Deserialize)]
struct StripePublicKeyRequest {
    id: i32,
}

async fn stripe_public_key(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<StripePublicKeyRequest>,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    let public_key = v2board_db::payment::find_stripe_public_key(&state.db, payload.id)
        .await?
        .ok_or_else(|| ApiError::legacy("payment is not found"))?;
    Ok(legacy_data(public_key))
}

#[derive(Debug, Serialize)]
struct SubscribeInfo {
    plan_id: Option<i32>,
    token: String,
    expired_at: Option<i64>,
    u: i64,
    d: i64,
    transfer_enable: i64,
    device_limit: Option<i32>,
    email: String,
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<v2board_db::plan::PlanRow>,
    alive_ip: i64,
    subscribe_url: String,
    reset_day: Option<i64>,
    allow_new_period: i32,
}

async fn user_subscribe(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<SubscribeInfo>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let subscribe = v2board_db::user::find_user_subscribe(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    let plan = match subscribe.plan_id {
        Some(plan_id) => Some(
            v2board_db::plan::find_plan(&state.db, plan_id)
                .await?
                .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?,
        ),
        None => None,
    };
    let alive_ip = alive_ip(&state.redis, user.id).await?;
    let reset_day = reset_day(subscribe.expired_at, plan.as_ref(), &state.config);
    let subscribe_url = state.config.subscribe_url_for_token(&subscribe.token);

    Ok(legacy_data(SubscribeInfo {
        plan_id: subscribe.plan_id,
        token: subscribe.token,
        expired_at: subscribe.expired_at,
        u: subscribe.u,
        d: subscribe.d,
        transfer_enable: subscribe.transfer_enable,
        device_limit: subscribe.device_limit,
        email: subscribe.email,
        uuid: subscribe.uuid,
        plan,
        alive_ip,
        subscribe_url,
        reset_day,
        allow_new_period: state.config.allow_new_period,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct UserPeriodRow {
    transfer_enable: i64,
    u: i64,
    d: i64,
    expired_at: Option<i64>,
    reset_traffic_method: Option<i8>,
}

async fn user_new_period(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if state.config.allow_new_period == 0 {
        return Err(ApiError::legacy("Renewal is not allowed"));
    }
    let row = sqlx::query_as::<_, UserPeriodRow>(
        r#"
        SELECT u.transfer_enable, u.u, u.d, u.expired_at, p.reset_traffic_method
        FROM v2_user u
        LEFT JOIN v2_plan p ON p.id = u.plan_id
        WHERE u.id = ?
        LIMIT 1
        "#,
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if row.transfer_enable > row.u + row.d {
        return Err(ApiError::legacy(
            "You have not used up your traffic, you cannot renew your subscription",
        ));
    }
    let expired_at = row
        .expired_at
        .filter(|expired_at| *expired_at > Utc::now().timestamp())
        .ok_or_else(|| ApiError::legacy("You do not allow to renew the subscription"))?;
    let mut reset_day = reset_day_by_method(expired_at, row.reset_traffic_method, &state.config)
        .ok_or_else(|| ApiError::legacy("You do not allow to renew the subscription"))?;
    let mut period = reset_period_by_method(row.reset_traffic_method, &state.config)
        .ok_or_else(|| ApiError::legacy("You do not allow to renew the subscription"))?;
    match period {
        1 => {
            reset_day = 30;
            period = 30;
        }
        30 => {}
        12 => {
            reset_day = 365;
            period = 365;
        }
        365 => {}
        _ => return Err(ApiError::legacy("Invalid reset period")),
    }
    if reset_day <= 0 {
        reset_day = period;
    }
    if (period + 1) * 86_400 < expired_at - Utc::now().timestamp() {
        sqlx::query("UPDATE v2_user SET expired_at = ?, u = 0, d = 0, updated_at = ? WHERE id = ?")
            .bind(expired_at - reset_day * 86_400)
            .bind(Utc::now().timestamp())
            .bind(user.id)
            .execute(&state.db)
            .await?;
        Ok(legacy_data(true))
    } else {
        Err(ApiError::legacy(
            "You do not have enough time to renew your subscription",
        ))
    }
}

#[derive(Debug, Deserialize)]
struct RedeemGiftcardRequest {
    giftcard: String,
}

#[derive(Debug, sqlx::FromRow)]
struct GiftcardRow {
    id: i64,
    r#type: i8,
    value: Option<i32>,
    plan_id: Option<i32>,
    limit_use: Option<i32>,
    used_user_ids: Option<String>,
    started_at: i64,
    ended_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct GiftUserRow {
    id: i64,
    balance: i32,
    expired_at: Option<i64>,
    transfer_enable: i64,
    u: i64,
    d: i64,
    plan_id: Option<i32>,
}

async fn redeem_giftcard(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<RedeemGiftcardRequest>,
) -> Result<Response, ApiError> {
    let auth_user = require_user(&state, &headers, query.auth_data).await?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    let mut user = sqlx::query_as::<_, GiftUserRow>(
        "SELECT id, balance, expired_at, transfer_enable, u, d, plan_id FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(auth_user.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    let giftcard = sqlx::query_as::<_, GiftcardRow>(
        r#"
        SELECT id, `type`, value, plan_id, limit_use, used_user_ids, started_at, ended_at
        FROM v2_giftcard
        WHERE code = ?
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(payload.giftcard.trim())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::legacy("The gift card does not exist"))?;
    if giftcard.started_at != 0 && now < giftcard.started_at {
        return Err(ApiError::legacy("The gift card is not yet valid"));
    }
    if giftcard.ended_at != 0 && now > giftcard.ended_at {
        return Err(ApiError::legacy("The gift card has expired"));
    }
    if giftcard.limit_use.is_some_and(|limit| limit <= 0) {
        return Err(ApiError::legacy(
            "The gift card usage limit has been reached",
        ));
    }
    let mut used_user_ids = parse_i64_json_list(giftcard.used_user_ids.as_deref());
    if used_user_ids.contains(&auth_user.id) {
        return Err(ApiError::legacy(
            "The gift card has already been used by this user",
        ));
    }
    used_user_ids.push(auth_user.id);
    let value = giftcard.value.unwrap_or_default();
    let mut group_id = None::<i32>;
    let mut device_limit = None::<i32>;
    let mut speed_limit = None::<i32>;
    match giftcard.r#type {
        1 => user.balance += value,
        2 => {
            let expired_at = user
                .expired_at
                .ok_or_else(|| ApiError::legacy("Not suitable gift card type"))?;
            user.expired_at = Some(if expired_at <= now {
                now + i64::from(value) * 86_400
            } else {
                expired_at + i64::from(value) * 86_400
            });
        }
        3 => user.transfer_enable += i64::from(value) * 1_073_741_824,
        4 => {
            user.u = 0;
            user.d = 0;
        }
        5 => {
            let can_apply = user.plan_id.is_none()
                || user.expired_at.is_some_and(|expired_at| expired_at < now);
            if !can_apply {
                return Err(ApiError::legacy("Not suitable gift card type"));
            }
            let plan_id = giftcard
                .plan_id
                .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
            let plan = v2board_db::plan::find_plan(&state.db, plan_id)
                .await?
                .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
            user.plan_id = Some(plan.id);
            group_id = Some(plan.group_id);
            device_limit = plan.device_limit;
            speed_limit = plan.speed_limit;
            user.transfer_enable = plan.transfer_enable * 1_073_741_824;
            user.u = 0;
            user.d = 0;
            user.expired_at = if value == 0 {
                None
            } else {
                Some(now + i64::from(value) * 86_400)
            };
        }
        _ => return Err(ApiError::legacy("Unknown gift card type")),
    }

    sqlx::query(
        r#"
        UPDATE v2_user
        SET balance = ?, expired_at = ?, transfer_enable = ?, u = ?, d = ?,
            plan_id = ?, group_id = COALESCE(?, group_id), device_limit = COALESCE(?, device_limit),
            speed_limit = COALESCE(?, speed_limit), updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(user.balance)
    .bind(user.expired_at)
    .bind(user.transfer_enable)
    .bind(user.u)
    .bind(user.d)
    .bind(user.plan_id)
    .bind(group_id)
    .bind(device_limit)
    .bind(speed_limit)
    .bind(now)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    let used_json = serde_json::to_string(&used_user_ids)
        .map_err(|error| ApiError::internal(format!("giftcard json error: {error}")))?;
    sqlx::query(
        "UPDATE v2_giftcard SET used_user_ids = ?, limit_use = ?, updated_at = ? WHERE id = ?",
    )
    .bind(used_json)
    .bind(giftcard.limit_use.map(|limit| limit - 1))
    .bind(now)
    .bind(giftcard.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "data": true,
        "type": giftcard.r#type,
        "value": giftcard.value,
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
struct TransferRequest {
    transfer_amount: i32,
}

async fn user_transfer(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TransferRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if payload.transfer_amount <= 0 {
        return Err(ApiError::legacy("Invalid transfer amount"));
    }
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    let commission_balance: i32 =
        sqlx::query_scalar("SELECT commission_balance FROM v2_user WHERE id = ? FOR UPDATE")
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if payload.transfer_amount > commission_balance {
        return Err(ApiError::legacy("Insufficient commission balance"));
    }
    sqlx::query(
        "UPDATE v2_user SET commission_balance = commission_balance - ?, balance = balance + ?, updated_at = ? WHERE id = ?",
    )
    .bind(payload.transfer_amount)
    .bind(payload.transfer_amount)
    .bind(now)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO v2_order (
            user_id, plan_id, period, trade_no, total_amount, surplus_amount, type, status,
            callback_no, commission_status, commission_balance, created_at, updated_at
        )
        VALUES (?, 0, 'deposit', ?, 0, ?, 9, 3, '佣金划转 Commission transfer', 0, 0, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(generate_trade_no())
    .bind(payload.transfer_amount)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
struct UserQuickLoginRequest {
    redirect: Option<String>,
}

async fn user_quick_login_url(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<UserQuickLoginRequest>,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let auth = AuthService::new(state.db, state.redis, state.config);
    Ok(legacy_data(
        auth.quick_login_url(user.id, payload.redirect.as_deref())
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
struct PlanFetchQuery {
    id: Option<i32>,
    auth_data: Option<String>,
}

async fn user_plan_fetch(
    State(state): State<AppState>,
    Query(query): Query<PlanFetchQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if let Some(id) = query.id {
        let subscribe = v2board_db::user::find_user_subscribe(&state.db, user.id)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
        let plan = v2board_db::plan::find_plan(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
        let hidden_plan = plan.show == 0;
        let unavailable_hidden_plan =
            hidden_plan && (plan.renew == 0 || subscribe.plan_id != Some(plan.id));
        if unavailable_hidden_plan {
            return Err(ApiError::legacy("Subscription plan does not exist"));
        }
        return Ok(legacy_data(plan).into_response());
    }

    let counts = v2board_db::plan::count_active_users_by_plan(&state.db).await?;
    let mut plans = v2board_db::plan::fetch_visible_plans(&state.db).await?;
    for plan in &mut plans {
        if let Some(capacity_limit) = plan.capacity_limit
            && let Some(count) = counts.get(&plan.id)
        {
            plan.capacity_limit = Some(capacity_limit - (*count as i32));
        }
    }
    Ok(legacy_data(plans).into_response())
}

#[derive(Debug, Deserialize)]
struct OrderFetchQuery {
    status: Option<i8>,
    auth_data: Option<String>,
}

async fn order_fetch(
    State(state): State<AppState>,
    Query(query): Query<OrderFetchQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::order::OrderRow>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let orders = v2board_db::order::fetch_user_orders(&state.db, user.id, query.status).await?;
    Ok(legacy_data(orders))
}

async fn order_save(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<v2board_domain::order::SaveOrderInput>,
) -> Result<Json<LegacyEnvelope<String>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let service = v2board_domain::order::OrderService::new(state.db, state.config);
    let trade_no = service.save(user.id, payload).await?;
    Ok(legacy_data(trade_no))
}

#[derive(Debug, Serialize)]
struct CheckoutEnvelope {
    r#type: i8,
    data: serde_json::Value,
}

async fn order_checkout(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<v2board_domain::order::CheckoutOrderInput>,
) -> Result<Json<CheckoutEnvelope>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let service = v2board_domain::order::OrderService::new(state.db, state.config);
    let result = service.checkout(user.id, payload).await?;
    Ok(Json(CheckoutEnvelope {
        r#type: result.r#type,
        data: result.data,
    }))
}

#[derive(Debug, Deserialize)]
struct TradeNoQuery {
    trade_no: String,
    auth_data: Option<String>,
}

async fn order_detail(
    State(state): State<AppState>,
    Query(query): Query<TradeNoQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::order::OrderRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let order = v2board_db::order::find_user_order(
        &state.db,
        user.id,
        &query.trade_no,
        state.config.try_out_plan_id,
    )
    .await?
    .ok_or_else(|| ApiError::legacy("Order does not exist or has been paid"))?;
    if order.plan_id != 0 && order.plan.is_none() {
        return Err(ApiError::legacy("Subscription plan does not exist"));
    }
    Ok(legacy_data(order))
}

async fn order_check(
    State(state): State<AppState>,
    Query(query): Query<TradeNoQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<i8>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let status = v2board_db::order::find_order_status(&state.db, user.id, &query.trade_no)
        .await?
        .ok_or_else(|| ApiError::legacy("Order does not exist"))?;
    Ok(legacy_data(status))
}

async fn order_payment_methods(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::payment::PaymentMethodRow>>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    let methods = v2board_db::payment::fetch_enabled_payment_methods(&state.db).await?;
    Ok(legacy_data(methods))
}

#[derive(Debug, Deserialize)]
struct OrderCancelRequest {
    trade_no: String,
}

async fn order_cancel(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<OrderCancelRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if payload.trade_no.trim().is_empty() {
        return Err(ApiError::legacy("Invalid parameter"));
    }
    let candidate = v2board_db::order::find_cancel_candidate(&state.db, user.id, &payload.trade_no)
        .await?
        .ok_or_else(|| ApiError::legacy("Order does not exist"))?;
    if candidate.status != 0 {
        return Err(ApiError::legacy("You can only cancel pending orders"));
    }
    let cancelled = v2board_db::order::cancel_pending_order(
        &state.db,
        user.id,
        &payload.trade_no,
        candidate.balance_amount,
        Utc::now().timestamp(),
    )
    .await?;
    if !cancelled {
        return Err(ApiError::legacy("Cancel failed"));
    }
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
struct CouponCheckRequest {
    code: String,
    plan_id: Option<i32>,
}

async fn coupon_check(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<CouponCheckRequest>,
) -> Result<Json<LegacyEnvelope<v2board_db::coupon::CouponRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if payload.code.trim().is_empty() {
        return Err(ApiError::legacy("Coupon cannot be empty"));
    }
    let coupon = v2board_db::coupon::find_coupon(&state.db, &payload.code)
        .await?
        .ok_or_else(|| ApiError::legacy("Invalid coupon"))?;
    validate_coupon_for_check(&state.db, user.id, &coupon, payload.plan_id).await?;
    Ok(legacy_data(coupon))
}

async fn invite_save(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let created = v2board_db::invite::create_invite_code(
        &state.db,
        user.id,
        state.config.invite_gen_limit,
        Utc::now().timestamp(),
    )
    .await?;
    if !created {
        return Err(ApiError::legacy(
            "The maximum number of creations has been reached",
        ));
    }
    Ok(legacy_data(true))
}

async fn invite_fetch(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::invite::InviteFetchRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let data = v2board_db::invite::fetch_invite(&state.db, user.id).await?;
    Ok(legacy_data(data))
}

#[derive(Debug, Deserialize)]
struct PageQuery {
    current: Option<i64>,
    #[serde(rename = "page_size", alias = "pageSize")]
    page_size: Option<i64>,
    auth_data: Option<String>,
}

async fn invite_details(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<
    Json<v2board_compat::LegacyPageEnvelope<Vec<v2board_db::invite::CommissionDetailRow>>>,
    ApiError,
> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let current = query.current.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(10).max(10);
    let (rows, total) =
        v2board_db::invite::fetch_commission_details(&state.db, user.id, current, page_size)
            .await?;
    Ok(legacy_page(rows, total))
}

#[derive(Debug, Deserialize)]
struct TicketFetchQuery {
    id: Option<i32>,
    auth_data: Option<String>,
}

async fn ticket_fetch(
    State(state): State<AppState>,
    Query(query): Query<TicketFetchQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if let Some(id) = query.id {
        let ticket = v2board_db::ticket::fetch_ticket_detail(&state.db, user.id, id)
            .await?
            .ok_or_else(|| ApiError::not_found("Ticket does not exist"))?;
        return Ok(legacy_data(ticket).into_response());
    }
    let tickets = v2board_db::ticket::fetch_tickets(&state.db, user.id).await?;
    Ok(legacy_data(tickets).into_response())
}

#[derive(Debug, Deserialize)]
struct TicketSaveRequest {
    subject: Option<String>,
    level: Option<i8>,
    message: Option<String>,
}

async fn ticket_save(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TicketSaveRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let subject = required_trimmed(payload.subject.as_deref(), "Ticket subject cannot be empty")?;
    let level = payload
        .level
        .ok_or_else(|| ApiError::legacy("Ticket level cannot be empty"))?;
    if !matches!(level, 0..=2) {
        return Err(ApiError::legacy("Incorrect ticket level format"));
    }
    let message = required_trimmed(payload.message.as_deref(), "Message cannot be empty")?;
    if v2board_db::ticket::count_open_tickets(&state.db, user.id).await? > 0 {
        return Err(ApiError::legacy("There are other unresolved tickets"));
    }
    match state.config.ticket_status {
        0 => {}
        1 => {
            if v2board_db::ticket::count_paid_orders(&state.db, user.id).await? == 0 {
                return Err(ApiError::legacy("请先购买套餐"));
            }
        }
        2 => return Err(ApiError::legacy("当前套餐不允许发起工单")),
        _ => return Err(ApiError::legacy("未知的工单状态")),
    }
    v2board_db::ticket::create_ticket(
        &state.db,
        user.id,
        subject,
        level,
        message,
        Utc::now().timestamp(),
    )
    .await?;
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
struct TicketReplyRequest {
    id: Option<i32>,
    message: Option<String>,
}

async fn ticket_reply(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TicketReplyRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let id = payload
        .id
        .ok_or_else(|| ApiError::legacy("Invalid parameter"))?;
    let message = required_trimmed(payload.message.as_deref(), "Message cannot be empty")?;
    let ticket = v2board_db::ticket::find_ticket_for_reply(&state.db, user.id, id)
        .await?
        .ok_or_else(|| ApiError::legacy("Ticket does not exist"))?;
    if ticket.status != 0 {
        return Err(ApiError::legacy(
            "The ticket is closed and cannot be replied",
        ));
    }
    if let Some(last) = v2board_db::ticket::find_last_message(&state.db, id).await?
        && last.user_id == user.id
    {
        return Err(ApiError::legacy(
            "Please wait for the technical enginneer to reply",
        ));
    }
    v2board_db::ticket::reply_ticket(&state.db, id, user.id, message, Utc::now().timestamp())
        .await?;
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
struct IdRequest {
    id: Option<i32>,
}

async fn ticket_close(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<IdRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let id = payload
        .id
        .ok_or_else(|| ApiError::legacy("Invalid parameter"))?;
    let closed =
        v2board_db::ticket::close_ticket(&state.db, user.id, id, Utc::now().timestamp()).await?;
    if !closed {
        return Err(ApiError::legacy("Ticket does not exist"));
    }
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
struct TicketWithdrawRequest {
    withdraw_method: Option<String>,
    withdraw_account: Option<String>,
}

async fn ticket_withdraw(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TicketWithdrawRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if state.config.withdraw_close_enable {
        return Err(ApiError::legacy(
            "user.ticket.withdraw.not_support_withdraw",
        ));
    }
    let method = required_trimmed(
        payload.withdraw_method.as_deref(),
        "The withdrawal method cannot be empty",
    )?;
    let account = required_trimmed(
        payload.withdraw_account.as_deref(),
        "The withdrawal account cannot be empty",
    )?;
    if !state
        .config
        .commission_withdraw_method
        .iter()
        .any(|allowed| allowed == method)
    {
        return Err(ApiError::legacy("Unsupported withdrawal method"));
    }
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if state.config.commission_withdraw_limit > access.commission_balance / 100 {
        return Err(ApiError::legacy(format!(
            "The current required minimum withdrawal commission is {}",
            state.config.commission_withdraw_limit
        )));
    }
    v2board_db::ticket::create_withdraw_ticket(
        &state.db,
        user.id,
        method,
        account,
        Utc::now().timestamp(),
    )
    .await?;
    Ok(legacy_data(true))
}

async fn server_fetch(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::server::AvailableServerRow>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if !user_is_available(&access) {
        return Ok(legacy_data(Vec::new()));
    }
    let servers = v2board_db::server::fetch_available_servers(&state.db, access.group_id).await?;
    Ok(legacy_data(servers))
}

#[derive(Debug, Deserialize)]
struct KnowledgeQuery {
    id: Option<i32>,
    language: Option<String>,
    keyword: Option<String>,
    auth_data: Option<String>,
}

async fn knowledge_fetch(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    if let Some(id) = query.id {
        let access = v2board_db::user::find_user_access(&state.db, user.id)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
        let mut knowledge = v2board_db::knowledge::find_knowledge(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::legacy("Article does not exist"))?;
        if !user_is_available(&access) {
            knowledge.body = format_access_blocks(&knowledge.body);
        }
        let subscribe_url = state.config.subscribe_url_for_token(&access.token);
        knowledge.body = render_knowledge_body(
            &knowledge.body,
            &state.config,
            &subscribe_url,
            &access.token,
        );
        return Ok(legacy_data(knowledge).into_response());
    }
    let language = query.language.as_deref().unwrap_or("zh-CN");
    let rows =
        v2board_db::knowledge::fetch_knowledge(&state.db, language, query.keyword.as_deref())
            .await?;
    Ok(legacy_data(rows).into_response())
}

async fn knowledge_categories(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<serde_json::Value>>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    let language = query.language.as_deref().unwrap_or("zh-CN");
    let categories = sqlx::query_scalar::<_, String>(
        "SELECT category FROM v2_knowledge WHERE language = ? AND `show` = 1 GROUP BY category ORDER BY category ASC",
    )
    .bind(language)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|category| json!({ "category": category }))
    .collect::<Vec<_>>();
    Ok(legacy_data(categories))
}

async fn telegram_bot_info(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<serde_json::Value>>, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    let token = state
        .config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Telegram bot is not configured"))?;
    let body: serde_json::Value = reqwest::Client::new()
        .get(format!("https://api.telegram.org/bot{token}/getMe"))
        .send()
        .await
        .map_err(|error| ApiError::legacy(format!("Telegram request failed: {error}")))?
        .json()
        .await
        .map_err(|error| ApiError::legacy(format!("Telegram response failed: {error}")))?;
    let username = body
        .get("result")
        .and_then(|result| result.get("username"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ApiError::legacy("Telegram bot response is invalid"))?;
    Ok(legacy_data(json!({ "username": username })))
}

#[derive(Debug, Deserialize)]
struct NoticeFetchQuery {
    id: Option<i32>,
    current: Option<i64>,
    #[serde(rename = "pageSize", alias = "page_size")]
    page_size: Option<i64>,
    auth_data: Option<String>,
}

async fn user_notice_fetch(
    State(state): State<AppState>,
    Query(query): Query<NoticeFetchQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _user = require_user(&state, &headers, query.auth_data).await?;
    if let Some(id) = query.id {
        let notice = v2board_db::notice::find_visible_notice(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::not_found("Notice not found"))?;
        return Ok(legacy_data(notice).into_response());
    }

    let current = query.current.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(5).clamp(1, 100);
    let (notices, total) =
        v2board_db::notice::fetch_visible_notices(&state.db, current, page_size).await?;
    Ok(legacy_page(notices, total).into_response())
}

async fn user_traffic_logs(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::stat::TrafficLogRow>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let logs =
        v2board_db::stat::fetch_traffic_logs(&state.db, user.id, first_day_of_month_timestamp())
            .await?;
    Ok(legacy_data(logs))
}

async fn require_user(
    state: &AppState,
    headers: &HeaderMap,
    auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let auth_data = auth_data
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        })
        .ok_or_else(ApiError::unauthorized)?;
    let auth = AuthService::new(state.db.clone(), state.redis.clone(), state.config.clone());
    auth.user_from_auth_data(&auth_data).await
}

async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let user = require_user(state, headers, auth_data).await?;
    if user.is_admin == 0 {
        return Err(forbidden("Permission denied"));
    }
    Ok(user)
}

async fn require_staff(
    state: &AppState,
    headers: &HeaderMap,
    auth_data: Option<String>,
) -> Result<AuthUser, ApiError> {
    let user = require_user(state, headers, auth_data).await?;
    if user.is_staff == 0 {
        return Err(forbidden("Permission denied"));
    }
    Ok(user)
}

async fn resolve_subscribe_token(state: &AppState, token: &str) -> Result<String, ApiError> {
    match state.config.show_subscribe_method {
        0 => Ok(token.to_string()),
        1 => resolve_one_time_subscribe_token(state, token).await,
        2 => resolve_totp_subscribe_token(state, token).await,
        _ => Ok(token.to_string()),
    }
}

async fn resolve_one_time_subscribe_token(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let user_token: Option<String> = conn.get(format!("otpn_{token}")).await?;
    let Some(user_token) = user_token else {
        return Err(forbidden("token is error"));
    };
    let _: i64 = conn.del(format!("otpn_{token}")).await?;
    let _: i64 = conn.del(format!("otp_{user_token}")).await?;
    Ok(user_token)
}

async fn resolve_totp_subscribe_token(state: &AppState, token: &str) -> Result<String, ApiError> {
    let cache_key = format!("totp_{token}");
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    if let Some(user_token) = conn.get::<_, Option<String>>(&cache_key).await? {
        return Ok(user_token);
    }

    let decoded = base64_decode_url_safe(token).ok_or_else(|| forbidden("token is error"))?;
    let decoded = String::from_utf8(decoded).map_err(|_| forbidden("token is error"))?;
    let (user_id, client_hash) = decoded
        .split_once(':')
        .ok_or_else(|| forbidden("token is error"))?;
    if user_id.is_empty() || client_hash.is_empty() {
        return Err(forbidden("token is error"));
    }
    let user_id = user_id
        .parse::<i64>()
        .map_err(|_| forbidden("token is error"))?;
    let user = v2board_db::user::find_user_access(&state.db, user_id)
        .await?
        .ok_or_else(|| forbidden("token is error"))?;

    let timestep = (state.config.show_subscribe_expire.max(1) * 60) as u64;
    let counter = Utc::now().timestamp().max(0) as u64 / timestep;
    let mut counter_bytes = [0_u8; 8];
    counter_bytes[4..].copy_from_slice(&(counter as u32).to_be_bytes());
    let expected = hmac_sha1_hex(user.token.as_bytes(), &counter_bytes)?;
    if client_hash != expected {
        return Err(forbidden("token is error"));
    }

    let _: () = conn.set_ex(cache_key, &user.token, timestep).await?;
    Ok(user.token)
}

async fn payment_request_input(
    request: Request,
) -> Result<v2board_domain::order::PaymentNotifyInput, ApiError> {
    let mut params = HashMap::new();
    if let Some(query) = request.uri().query().filter(|query| !query.is_empty()) {
        params.extend(parse_urlencoded_params(query)?);
    }
    let headers = request
        .headers()
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (key.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    if body.is_empty() {
        return Ok(v2board_domain::order::PaymentNotifyInput {
            params,
            body: Vec::new(),
            headers,
        });
    }
    if content_type.contains("application/json") || body.first() == Some(&b'{') {
        params.extend(parse_json_object_params(&body)?);
    } else if content_type.contains("xml") || body.first() == Some(&b'<') {
        params.extend(parse_xml_params(&body)?);
    } else {
        let body = std::str::from_utf8(&body)
            .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
        params.extend(parse_urlencoded_params(body)?);
    }
    Ok(v2board_domain::order::PaymentNotifyInput {
        params,
        body: body.to_vec(),
        headers,
    })
}

async fn admin_request_params(request: Request) -> Result<HashMap<String, String>, ApiError> {
    let mut params = HashMap::new();
    if let Some(query) = request.uri().query().filter(|query| !query.is_empty()) {
        params.extend(parse_urlencoded_params(query)?);
    }
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad_request("Invalid admin request body"))?;
    if body.is_empty() {
        return Ok(params);
    }
    if content_type.contains("application/json") || body.first() == Some(&b'{') {
        let value = serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|_| ApiError::bad_request("Invalid admin request body"))?;
        flatten_admin_json(None, &value, &mut params);
    } else {
        let body = std::str::from_utf8(&body)
            .map_err(|_| ApiError::bad_request("Invalid admin request body"))?;
        params.extend(parse_urlencoded_params(body)?);
    }
    Ok(params)
}

fn flatten_admin_json(
    prefix: Option<String>,
    value: &serde_json::Value,
    params: &mut HashMap<String, String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let key = prefix
                    .as_ref()
                    .map(|prefix| format!("{prefix}[{key}]"))
                    .unwrap_or_else(|| key.clone());
                flatten_admin_json(Some(key), value, params);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                let key = prefix
                    .as_ref()
                    .map(|prefix| format!("{prefix}[{index}]"))
                    .unwrap_or_else(|| index.to_string());
                flatten_admin_json(Some(key), value, params);
            }
        }
        serde_json::Value::Null => {
            if let Some(prefix) = prefix {
                params.insert(prefix, "null".to_string());
            }
        }
        serde_json::Value::String(value) => {
            if let Some(prefix) = prefix {
                params.insert(prefix, value.clone());
            }
        }
        serde_json::Value::Number(value) => {
            if let Some(prefix) = prefix {
                params.insert(prefix, value.to_string());
            }
        }
        serde_json::Value::Bool(value) => {
            if let Some(prefix) = prefix {
                params.insert(prefix, if *value { "1" } else { "0" }.to_string());
            }
        }
    }
}

fn parse_urlencoded_params(value: &str) -> Result<HashMap<String, String>, ApiError> {
    serde_urlencoded::from_str::<HashMap<String, String>>(value)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))
}

fn parse_json_object_params(bytes: &[u8]) -> Result<HashMap<String, String>, ApiError> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    let Some(object) = value.as_object() else {
        return Err(ApiError::bad_request("Invalid payment notify body"));
    };
    Ok(object
        .iter()
        .filter_map(|(key, value)| json_scalar_to_string(value).map(|value| (key.clone(), value)))
        .collect())
}

fn parse_xml_params(bytes: &[u8]) -> Result<HashMap<String, String>, ApiError> {
    let body = std::str::from_utf8(bytes)
        .map_err(|_| ApiError::bad_request("Invalid payment notify body"))?;
    let mut params = HashMap::new();
    let mut cursor = body;
    while let Some(start) = cursor.find('<') {
        let cursor_after_open = &cursor[start + 1..];
        let Some(close) = cursor_after_open.find('>') else {
            break;
        };
        let tag = &cursor_after_open[..close];
        if tag.starts_with('/') || tag == "xml" || tag.contains(' ') {
            cursor = &cursor_after_open[close + 1..];
            continue;
        }
        let value_start = start + 1 + close + 1;
        let close_tag = format!("</{tag}>");
        let Some(value_end_rel) = cursor[value_start..].find(&close_tag) else {
            cursor = &cursor_after_open[close + 1..];
            continue;
        };
        let raw_value = &cursor[value_start..value_start + value_end_rel];
        let value = raw_value
            .strip_prefix("<![CDATA[")
            .and_then(|value| value.strip_suffix("]]>"))
            .unwrap_or(raw_value)
            .to_string();
        params.insert(tag.to_string(), value);
        cursor = &cursor[value_start + value_end_rel + close_tag.len()..];
    }
    Ok(params)
}

fn json_scalar_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

async fn alive_ip(redis: &redis::Client, user_id: i64) -> Result<i64, ApiError> {
    let key = format!("ALIVE_IP_USER_{user_id}");
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let current: Option<String> = conn.get(key).await?;
    let Some(current) = current else {
        return Ok(0);
    };
    Ok(serde_json::from_str::<serde_json::Value>(&current)
        .ok()
        .and_then(|value| value.get("alive_ip").and_then(|alive| alive.as_i64()))
        .unwrap_or(0))
}

fn reset_day(
    expired_at: Option<i64>,
    plan: Option<&v2board_db::plan::PlanRow>,
    config: &AppConfig,
) -> Option<i64> {
    let expired_at = expired_at?;
    if expired_at <= Utc::now().timestamp() {
        return None;
    }
    let method = plan
        .and_then(|plan| plan.reset_traffic_method)
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method);

    match method {
        0 => Some(reset_day_by_month_first_day()),
        1 => Some(reset_day_by_expire_day(expired_at)),
        2 => None,
        3 => days_until_year_first_day(),
        4 => days_until_year_expire_day(expired_at),
        _ => None,
    }
}

fn reset_day_by_method(
    expired_at: i64,
    plan_reset_method: Option<i8>,
    config: &AppConfig,
) -> Option<i64> {
    if expired_at <= Utc::now().timestamp() {
        return None;
    }
    match plan_reset_method
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method)
    {
        0 => Some(reset_day_by_month_first_day()),
        1 => Some(reset_day_by_expire_day(expired_at)),
        2 => None,
        3 => days_until_year_first_day(),
        4 => days_until_year_expire_day(expired_at),
        _ => None,
    }
}

fn reset_period_by_method(plan_reset_method: Option<i8>, config: &AppConfig) -> Option<i64> {
    match plan_reset_method
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method)
    {
        0 => Some(1),
        1 => Some(30),
        2 => None,
        3 => Some(12),
        4 => Some(365),
        _ => None,
    }
}

fn reset_day_by_month_first_day() -> i64 {
    let today = Local::now().date_naive();
    i64::from(last_day_of_current_month() - today.day())
}

fn reset_day_by_expire_day(expired_at: i64) -> i64 {
    let today = Local::now().date_naive();
    let expire_day = Local
        .timestamp_opt(expired_at, 0)
        .single()
        .map(|date| date.day())
        .unwrap_or(today.day());
    let today_day = today.day();
    let last_day = last_day_of_current_month();

    if expire_day >= today_day && expire_day >= last_day {
        return i64::from(last_day - today_day);
    }
    if expire_day >= today_day {
        return i64::from(expire_day - today_day);
    }
    i64::from(last_day - today_day + expire_day)
}

fn days_until_year_first_day() -> Option<i64> {
    let now = Local::now();
    let next_year = Local
        .with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0)
        .single()?;
    Some((next_year.timestamp() - now.timestamp()) / 86_400)
}

fn days_until_year_expire_day(expired_at: i64) -> Option<i64> {
    let now = Local::now();
    let expired = Local.timestamp_opt(expired_at, 0).single()?;
    let this_year = Local
        .with_ymd_and_hms(now.year(), expired.month(), expired.day(), 0, 0, 0)
        .single();
    let target = match this_year {
        Some(target) if target > now => target,
        _ => Local
            .with_ymd_and_hms(now.year() + 1, expired.month(), expired.day(), 0, 0, 0)
            .single()?,
    };
    Some((target.timestamp() - now.timestamp()) / 86_400)
}

fn parse_i64_json_list(value: Option<&str>) -> Vec<i64> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<i64>>(value).unwrap_or_default()
}

fn generate_trade_no() -> String {
    format!(
        "{}{}",
        Utc::now().format("%Y%m%d%H%M%S"),
        Uuid::new_v4().simple()
    )
}

fn yaml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn last_day_of_current_month() -> u32 {
    let today = Local::now().date_naive();
    let (year, month) = if today.month() == 12 {
        (today.year() + 1, 1)
    } else {
        (today.year(), today.month() + 1)
    };
    let first_next_month = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
    (first_next_month - Duration::days(1)).day()
}

fn first_day_of_month_timestamp() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

async fn validate_coupon_for_check(
    db: &DbPool,
    user_id: i64,
    coupon: &v2board_db::coupon::CouponRow,
    plan_id: Option<i32>,
) -> Result<(), ApiError> {
    if coupon.show == 0 {
        return Err(ApiError::legacy("Invalid coupon"));
    }
    if matches!(coupon.limit_use, Some(limit_use) if limit_use <= 0) {
        return Err(ApiError::legacy("This coupon is no longer available"));
    }
    let now = Utc::now().timestamp();
    if now < coupon.started_at {
        return Err(ApiError::legacy("This coupon has not yet started"));
    }
    if now > coupon.ended_at {
        return Err(ApiError::legacy("This coupon has expired"));
    }
    if let (Some(plan_id), Some(limit_plan_ids)) = (plan_id, coupon.limit_plan_ids.as_ref())
        && !limit_plan_ids.contains(&plan_id)
    {
        return Err(ApiError::legacy(
            "The coupon code cannot be used for this subscription",
        ));
    }
    if let Some(limit) = coupon.limit_use_with_user {
        let used = v2board_db::coupon::count_user_coupon_uses(db, coupon.id, user_id).await?;
        if used >= i64::from(limit) {
            return Err(ApiError::legacy(format!(
                "The coupon can only be used {limit} per person"
            )));
        }
    }
    Ok(())
}

fn required_trimmed<'a>(value: Option<&'a str>, message: &str) -> Result<&'a str, ApiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy(message))
}

fn forbidden(message: impl Into<String>) -> ApiError {
    ApiError::Http {
        status: StatusCode::FORBIDDEN,
        message: message.into(),
    }
}

fn user_is_available(user: &v2board_db::user::UserAccessRow) -> bool {
    let unexpired = user
        .expired_at
        .map(|expired_at| expired_at > Utc::now().timestamp())
        .unwrap_or(true);
    user.banned == 0 && user.transfer_enable > 0 && unexpired
}

struct SubscriptionDocument {
    body: String,
    content_type: &'static str,
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

fn build_subscription_document(
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
    _config: &AppConfig,
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
    let mut config = load_singbox_template(modern);
    inject_singbox_proxies(&mut config, &proxy_tags, proxies);
    serde_json::to_string(&config)
        .map_err(|_| ApiError::internal("failed to render sing-box subscription"))
}

fn load_singbox_template(modern: bool) -> Value {
    let candidates = if modern {
        ["custom.sing-box.json", "default.sing-box.json"]
    } else {
        ["custom.sing-box.old.json", "default.sing-box.old.json"]
    };
    for filename in candidates {
        let path = format!("/laravel/resources/rules/{filename}");
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
        _ => None,
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

fn value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Array(values) => values.first().and_then(value_to_string),
        serde_json::Value::Null | serde_json::Value::Object(_) => None,
    }
}

fn value_to_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(value) => value.as_i64(),
        serde_json::Value::String(value) => value.parse::<i64>().ok(),
        serde_json::Value::Bool(value) => Some(i64::from(*value)),
        serde_json::Value::Array(values) => values.first().and_then(value_to_i64),
        serde_json::Value::Null | serde_json::Value::Object(_) => None,
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

fn prefix_bytes(value: &str, length: usize) -> &[u8] {
    let end = value
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(value.len()))
        .nth(length)
        .unwrap_or(value.len());
    &value.as_bytes()[..end]
}

fn hmac_sha1_hex(key: &[u8], message: &[u8]) -> Result<String, ApiError> {
    type HmacSha1 = Hmac<Sha1>;
    let mut mac =
        HmacSha1::new_from_slice(key).map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(message);
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn render_knowledge_body(
    body: &str,
    config: &AppConfig,
    subscribe_url: &str,
    subscribe_token: &str,
) -> String {
    body.replace("{{siteName}}", &config.app_name)
        .replace("{{subscribeUrl}}", subscribe_url)
        .replace("{{urlEncodeSubscribeUrl}}", &percent_encode(subscribe_url))
        .replace(
            "{{safeBase64SubscribeUrl}}",
            &safe_base64_encode(subscribe_url.as_bytes()),
        )
        .replace("{{subscribeToken}}", subscribe_token)
}

fn format_access_blocks(body: &str) -> String {
    let mut output = body.to_string();
    while let Some(start) = output.find("<!--access start-->") {
        let Some(relative_end) = output[start..].find("<!--access end-->") else {
            break;
        };
        let end = start + relative_end + "<!--access end-->".len();
        output.replace_range(
            start..end,
            "<div class=\"v2board-no-access\">You must have a valid subscription to view content in this area</div>",
        );
    }
    output
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn base64_decode_url_safe(value: &str) -> Option<Vec<u8>> {
    let mut normalized = value.replace('-', "+").replace('_', "/");
    match normalized.len() % 4 {
        0 => {}
        2 => normalized.push_str("=="),
        3 => normalized.push('='),
        _ => return None,
    }
    base64_decode(&normalized)
}

fn base64_decode(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
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

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn standard_base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();

        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(
            TABLE[(((b0 & 0b0000_0011) << 4) | (b1.unwrap_or_default() >> 4)) as usize] as char,
        );
        if let Some(b1) = b1 {
            output.push(
                TABLE[(((b1 & 0b0000_1111) << 2) | (b2.unwrap_or_default() >> 6)) as usize] as char,
            );
        } else {
            output.push('=');
        }
        if let Some(b2) = b2 {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
        index += 3;
    }
    output
}

fn safe_base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();

        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(
            TABLE[(((b0 & 0b0000_0011) << 4) | (b1.unwrap_or_default() >> 4)) as usize] as char,
        );
        if let Some(b1) = b1 {
            output.push(
                TABLE[(((b1 & 0b0000_1111) << 2) | (b2.unwrap_or_default() >> 6)) as usize] as char,
            );
        } else {
            output.push('=');
        }
        if let Some(b2) = b2 {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
        index += 3;
    }
    output.replace('+', "-").replace('/', "_").replace('=', "")
}

fn validate_binary(field: &str, value: Option<i8>) -> Result<(), ApiError> {
    match value {
        Some(0 | 1) | None => Ok(()),
        Some(_) => Err(ApiError::bad_request(format!("{field} must be 0 or 1"))),
    }
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
    fn custom_subscribe_route_skips_default_and_registers_custom_path() {
        assert_eq!(
            custom_subscribe_route_path_from_str("/api/v1/client/subscribe"),
            None
        );
        assert_eq!(
            custom_subscribe_route_path_from_str("/api/v1/client/subscribe/"),
            None
        );
        assert_eq!(
            custom_subscribe_route_path_from_str("/custom/subscribe"),
            Some("/custom/subscribe".to_string())
        );
        assert_eq!(
            custom_subscribe_route_path_from_str("/custom/subscribe/"),
            Some("/custom/subscribe".to_string())
        );
    }
}
