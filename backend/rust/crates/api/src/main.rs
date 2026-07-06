use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{ConnectInfo, Form, OriginalUri, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use chrono::{Datelike, Duration, Local, TimeZone, Utc};
use hmac::{Hmac, KeyInit, Mac};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::Sha1;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data, legacy_page};
use v2board_config::AppConfig;
use v2board_db::{DbPool, connect_mysql};
use v2board_domain::auth::{AuthService, AuthUser, EmailVerifyInput, ForgetInput, RegisterInput};

mod codec;
mod i18n;
mod json_value;
mod routes;
mod server_api;
mod subscription;

use codec::{base64_decode_url_safe, percent_encode, safe_base64_encode, standard_base64_encode};

#[derive(Clone)]
struct AppState {
    config: Arc<RwLock<AppConfig>>,
    db: DbPool,
    redis: redis::Client,
}

impl AppState {
    fn new(config: AppConfig, db: DbPool, redis: redis::Client) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            db,
            redis,
        }
    }

    fn config_snapshot(&self) -> AppConfig {
        self.config
            .read()
            .expect("app config lock poisoned")
            .clone()
    }

    fn reload_config(&self) -> AppConfig {
        let config = AppConfig::from_env();
        *self.config.write().expect("app config lock poisoned") = config.clone();
        config
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env();
    let db = connect_mysql(&config.database_url).await?;
    let redis = redis::Client::open(config.redis_url.clone())?;
    let state = AppState::new(config.clone(), db, redis);
    let app = routes::build_app(state, &config);

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

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

async fn dynamic_fallback(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let method = request.method().clone();
    let path = normalize_request_path(request.uri().path());
    let config = state.config_snapshot();

    if method == Method::GET
        && routes::custom_subscribe_route_path_from_str(&config.subscribe_path)
            .as_deref()
            .is_some_and(|subscribe_path| normalize_request_path(subscribe_path) == path)
    {
        let query = serde_urlencoded::from_str::<ClientSubscribeQuery>(
            request.uri().query().unwrap_or_default(),
        )
        .map_err(|_| ApiError::bad_request("Invalid subscribe query"))?;
        return client_subscribe_response(&state, query, headers).await;
    }

    let admin_prefix = format!("/api/v1/{}/", config.admin_path());
    if let Some(admin_path) = path.strip_prefix(&admin_prefix) {
        let admin_path = admin_path.to_string();
        match method {
            Method::GET => {
                let params = request
                    .uri()
                    .query()
                    .map(parse_urlencoded_params)
                    .transpose()?
                    .unwrap_or_default();
                let _admin =
                    require_admin(&state, &headers, params.get("auth_data").cloned()).await?;
                let service = v2board_domain::admin::AdminService::new(
                    state.db.clone(),
                    state.redis.clone(),
                    config.clone(),
                );
                return admin_response(service.get(&admin_path, params).await?);
            }
            Method::POST => {
                let mut params = admin_request_params(request).await?;
                let admin =
                    require_admin(&state, &headers, params.get("auth_data").cloned()).await?;
                params.insert("_admin_email".to_string(), admin.email);
                let service = v2board_domain::admin::AdminService::new(
                    state.db.clone(),
                    state.redis.clone(),
                    config.clone(),
                );
                let output = service.post(&admin_path, params).await?;
                if admin_path.trim_matches('/') == "config/save" {
                    state.reload_config();
                }
                return admin_response(output);
            }
            _ => {}
        }
    }

    Err(ApiError::not_found("Not Found"))
}

fn normalize_request_path(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path).trim_end_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn path_matches_current_admin_route(config: &AppConfig, request_path: &str) -> bool {
    let path = normalize_request_path(request_path);
    let admin_prefix = format!("/api/v1/{}/", config.admin_path());
    path.starts_with(&admin_prefix)
}

async fn language_middleware(request: Request, next: Next) -> Response {
    let locale = request
        .headers()
        .get("content-language")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let response = next.run(request).await;
    // Laravel pins the default AND fallback locale to zh-CN (config/app.php), so a request
    // with no Content-Language header still gets Chinese messages. Mirror that default.
    let locale = locale.unwrap_or_else(|| i18n::DEFAULT_LOCALE.to_string());
    if !response.status().is_client_error() && !response.status().is_server_error() {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, 64 * 1024).await else {
        parts.headers.remove(header::CONTENT_LENGTH);
        return Response::from_parts(parts, Body::empty());
    };
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let Some(message) = json.get("message").and_then(serde_json::Value::as_str) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let localized = localize_legacy_message(message, &locale);
    if localized == message {
        return Response::from_parts(parts, Body::from(bytes));
    }
    json["message"] = json!(localized);
    match serde_json::to_vec(&json) {
        Ok(body) => {
            parts.headers.remove(header::CONTENT_LENGTH);
            Response::from_parts(parts, Body::from(body))
        }
        Err(_) => Response::from_parts(parts, Body::from(bytes)),
    }
}

fn localize_legacy_message(message: &str, locale: &str) -> String {
    let locale = locale.to_ascii_lowercase();
    if locale.starts_with("zh") {
        return localize_zh_cn_message(message).unwrap_or_else(|| message.to_string());
    }
    if locale.starts_with("en") && message == "未登录或登陆已过期" {
        return "You are not logged in or login has expired".to_string();
    }
    message.to_string()
}

fn localize_zh_cn_message(message: &str) -> Option<String> {
    // Dynamically-composed rate-limit string (Laravel interpolates `:minute`); the
    // remaining ~98 static strings resolve through the embedded Laravel catalog below.
    if let Some(minute) = password_limit_minutes(message) {
        return Some(format!("密码错误次数过多，请 {minute} 分钟后再试"));
    }
    i18n::translate_zh_cn(message)
}

fn password_limit_minutes(message: &str) -> Option<&str> {
    let prefix = "There are too many password errors, please try again after ";
    let suffix = " minutes.";
    message
        .strip_prefix(prefix)
        .and_then(|message| message.strip_suffix(suffix))
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
    let config = state.config_snapshot();
    let email_whitelist_suffix = if config.email_whitelist_enable {
        json!(config.email_whitelist_suffix)
    } else {
        json!(0)
    };

    legacy_data(GuestConfig {
        tos_url: config.tos_url,
        is_email_verify: config.email_verify as i32,
        is_invite_force: config.invite_force as i32,
        email_whitelist_suffix,
        is_recaptcha: config.recaptcha_enable as i32,
        recaptcha_site_key: config.recaptcha_site_key,
        app_description: config.app_description,
        app_url: config.app_url,
        logo: config.logo,
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
    client_subscribe_response(&state, query, headers).await
}

async fn client_subscribe_response(
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
        subscription::build_subscription_document(&config, &user, &servers, &flag, &host)?;
    let mut response = subscription.body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(subscription.content_type),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&subscription.content_disposition)
            .map_err(|_| ApiError::internal("invalid subscription filename"))?,
    );
    if let Some(app_url) = config.app_url.as_deref() {
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
            standard_base64_encode(config.app_name.as_bytes())
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

async fn payment_notify(
    State(state): State<AppState>,
    Path((method, uuid)): Path<(String, String)>,
    request: Request,
) -> Result<Response, ApiError> {
    let input = payment_request_input(request).await?;
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
    let result = service.handle_payment_notify(&method, &uuid, input).await?;
    Ok(result.body.into_response())
}

async fn admin_get(
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    Path(admin_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    if !path_matches_current_admin_route(&config, original_uri.path()) {
        return Err(ApiError::not_found("Not Found"));
    }
    let _admin = require_admin(&state, &headers, params.get("auth_data").cloned()).await?;
    let service =
        v2board_domain::admin::AdminService::new(state.db.clone(), state.redis.clone(), config);
    admin_response(service.get(&admin_path, params).await?)
}

async fn admin_post(
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    Path(admin_path): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    if !path_matches_current_admin_route(&config, original_uri.path()) {
        return Err(ApiError::not_found("Not Found"));
    }
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let admin = require_admin(&state, &headers, params.get("auth_data").cloned()).await?;
    params.insert("_admin_email".to_string(), admin.email);
    let service =
        v2board_domain::admin::AdminService::new(state.db.clone(), state.redis.clone(), config);
    let output = service.post(&admin_path, params).await?;
    if admin_path.trim_matches('/') == "config/save" {
        state.reload_config();
    }
    admin_response(output)
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
    let service = v2board_domain::admin::AdminService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let service = v2board_domain::admin::AdminService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let config = state.config_snapshot();
    let token = config
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

    if let Ok(update) = serde_json::from_value::<TelegramUpdate>(payload) {
        if let Some(join) = update.chat_join_request {
            handle_telegram_chat_join_request(&state, token, join).await?;
        }
        if let Some(message) = update.message
            && let Err(error) = handle_telegram_message(&state, token, message).await
            && let Some(chat_id) = error.chat_id
            && let Err(send_error) =
                telegram_send_message(token, chat_id, &error.message, None).await
        {
            tracing::warn!(
                ?send_error,
                "failed to send telegram command error response"
            );
        }
    }

    Ok(Json(json!({ "data": true })))
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

async fn handle_telegram_chat_join_request(
    state: &AppState,
    bot_token: &str,
    join: TelegramChatJoinRequest,
) -> Result<(), ApiError> {
    let telegram_id = join.from.id;
    let chat_id = join.chat.id;
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
    telegram_chat_join_request(bot_token, method, chat_id, telegram_id).await
}

async fn handle_telegram_message(
    state: &AppState,
    bot_token: &str,
    message: TelegramMessage,
) -> Result<(), TelegramCommandError> {
    let Some(text) = message
        .text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return Ok(());
    };

    if let Some((command, args)) = telegram_command_parts(text) {
        let command = normalize_telegram_command(bot_token, command).await?;
        match command.as_str() {
            "/bind" => return telegram_bind(state, bot_token, &message, &args).await,
            "/unbind" => return telegram_unbind(state, bot_token, &message).await,
            "/traffic" => return telegram_traffic(state, bot_token, &message).await,
            "/getlatesturl" => return telegram_latest_url(state, bot_token, &message).await,
            _ => {}
        }
    }

    if let Some(reply_text) = message
        .reply_to_message
        .as_ref()
        .and_then(|reply| reply.text.as_deref())
        && let Some(ticket_id) = telegram_reply_ticket_id(reply_text)
    {
        telegram_reply_ticket(state, bot_token, &message, ticket_id).await?;
    }
    Ok(())
}

async fn normalize_telegram_command(
    bot_token: &str,
    command: &str,
) -> Result<String, TelegramCommandError> {
    let (command, bot_name) = command.split_once('@').unwrap_or((command, ""));
    if bot_name.is_empty() {
        return Ok(command.to_ascii_lowercase());
    }
    let current_bot = telegram_bot_username(bot_token).await?;
    if bot_name.eq_ignore_ascii_case(&current_bot) {
        Ok(command.to_ascii_lowercase())
    } else {
        Ok(String::new())
    }
}

async fn telegram_bind(
    state: &AppState,
    bot_token: &str,
    message: &TelegramMessage,
    args: &[&str],
) -> Result<(), TelegramCommandError> {
    if !message.is_private() {
        return Ok(());
    }
    let chat_id = message.chat.id;
    let subscribe_url = args
        .first()
        .copied()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| TelegramCommandError::new(chat_id, "参数有误，请携带订阅地址发送"))?;
    let token = subscribe_token_from_url(subscribe_url)
        .ok_or_else(|| TelegramCommandError::new(chat_id, "订阅地址无效"))?;
    let token = resolve_subscribe_token_for_telegram_bind(state, &token)
        .await
        .map_err(|error| TelegramCommandError::new(chat_id, error.to_string()))?;
    let user = sqlx::query_as::<_, TelegramUserRow>(
        r#"
        SELECT id, email, telegram_id, is_admin, is_staff, u, d, transfer_enable
        FROM v2_user
        WHERE token = ?
        LIMIT 1
        "#,
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| TelegramCommandError::new(chat_id, error.to_string()))?
    .ok_or_else(|| TelegramCommandError::new(chat_id, "用户不存在"))?;
    if user.telegram_id.is_some() {
        return Err(TelegramCommandError::new(
            chat_id,
            "该账号已经绑定了Telegram账号",
        ));
    }
    let updated = sqlx::query("UPDATE v2_user SET telegram_id = ?, updated_at = ? WHERE id = ?")
        .bind(chat_id)
        .bind(Utc::now().timestamp())
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|error| TelegramCommandError::new(chat_id, error.to_string()))?;
    if updated.rows_affected() == 0 {
        return Err(TelegramCommandError::new(chat_id, "设置失败"));
    }
    telegram_send_message(bot_token, chat_id, "绑定成功", None).await?;
    Ok(())
}

async fn telegram_unbind(
    state: &AppState,
    bot_token: &str,
    message: &TelegramMessage,
) -> Result<(), TelegramCommandError> {
    if !message.is_private() {
        return Ok(());
    }
    let chat_id = message.chat.id;
    let Some(user) = telegram_user_by_chat_id(state, chat_id).await? else {
        telegram_send_message(
            bot_token,
            chat_id,
            "没有查询到您的用户信息，请先绑定账号",
            Some("markdown"),
        )
        .await?;
        return Ok(());
    };
    let updated = v2board_db::user::clear_telegram_id(&state.db, user.id, Utc::now().timestamp())
        .await
        .map_err(|error| TelegramCommandError::new(chat_id, error.to_string()))?;
    if !updated {
        return Err(TelegramCommandError::new(chat_id, "解绑失败"));
    }
    telegram_send_message(bot_token, chat_id, "解绑成功", Some("markdown")).await?;
    Ok(())
}

async fn telegram_traffic(
    state: &AppState,
    bot_token: &str,
    message: &TelegramMessage,
) -> Result<(), TelegramCommandError> {
    if !message.is_private() {
        return Ok(());
    }
    let chat_id = message.chat.id;
    let Some(user) = telegram_user_by_chat_id(state, chat_id).await? else {
        telegram_send_message(
            bot_token,
            chat_id,
            "没有查询到您的用户信息，请先绑定账号",
            Some("markdown"),
        )
        .await?;
        return Ok(());
    };
    let remaining = user.transfer_enable - (user.u + user.d);
    let text = format!(
        "🚥流量查询\n———————————————\n计划流量：`{}`\n已用上行：`{}`\n已用下行：`{}`\n剩余流量：`{}`",
        legacy_traffic_convert(user.transfer_enable),
        legacy_traffic_convert(user.u),
        legacy_traffic_convert(user.d),
        legacy_traffic_convert(remaining),
    );
    telegram_send_message(bot_token, chat_id, &text, Some("markdown")).await?;
    Ok(())
}

async fn telegram_latest_url(
    state: &AppState,
    bot_token: &str,
    message: &TelegramMessage,
) -> Result<(), TelegramCommandError> {
    let config = state.config_snapshot();
    let text = format!(
        "{}的最新网址是：{}",
        config.app_name,
        config.app_url.as_deref().unwrap_or_default()
    );
    telegram_send_message(bot_token, message.chat.id, &text, Some("markdown")).await?;
    Ok(())
}

async fn telegram_reply_ticket(
    state: &AppState,
    bot_token: &str,
    message: &TelegramMessage,
    ticket_id: i32,
) -> Result<(), TelegramCommandError> {
    if !message.is_private() {
        return Ok(());
    }
    let chat_id = message.chat.id;
    let Some(user) = telegram_user_by_chat_id(state, chat_id).await? else {
        return Err(TelegramCommandError::new(chat_id, "用户不存在"));
    };
    if user.is_admin == 0 && user.is_staff == 0 {
        return Ok(());
    }
    let text = message.text.as_deref().unwrap_or_default().trim();
    if text.is_empty() {
        return Ok(());
    }
    reply_ticket_by_admin(state, ticket_id, user.id, text).await?;
    telegram_send_message(
        bot_token,
        chat_id,
        &format!("#`{ticket_id}` 的工单已回复成功"),
        Some("markdown"),
    )
    .await?;
    send_telegram_message_with_admin(
        state,
        bot_token,
        &format!("#`{ticket_id}` 的工单已由 {} 进行回复", user.email),
        true,
    )
    .await?;
    Ok(())
}

async fn reply_ticket_by_admin(
    state: &AppState,
    ticket_id: i32,
    user_id: i64,
    message: &str,
) -> Result<(), ApiError> {
    let Some(ticket_user_id) =
        sqlx::query_scalar::<_, i64>("SELECT user_id FROM v2_ticket WHERE id = ? LIMIT 1")
            .bind(ticket_id)
            .fetch_optional(&state.db)
            .await?
    else {
        return Err(ApiError::legacy("工单不存在"));
    };
    let now = Utc::now().timestamp();
    let reply_status = if user_id != ticket_user_id { 1 } else { 0 };
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO v2_ticket_message (user_id, ticket_id, message, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(ticket_id)
    .bind(message)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE v2_ticket SET status = 0, reply_status = ?, updated_at = ? WHERE id = ?")
        .bind(reply_status)
        .bind(now)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn telegram_user_by_chat_id(
    state: &AppState,
    chat_id: i64,
) -> Result<Option<TelegramUserRow>, ApiError> {
    Ok(sqlx::query_as::<_, TelegramUserRow>(
        r#"
        SELECT id, email, telegram_id, is_admin, is_staff, u, d, transfer_enable
        FROM v2_user
        WHERE telegram_id = ?
        LIMIT 1
        "#,
    )
    .bind(chat_id)
    .fetch_optional(&state.db)
    .await?)
}

async fn send_telegram_message_with_admin(
    state: &AppState,
    bot_token: &str,
    message: &str,
    include_staff: bool,
) -> Result<(), TelegramCommandError> {
    if !state.config_snapshot().telegram_bot_enable {
        return Ok(());
    }
    let users = sqlx::query_as::<_, TelegramAdminRecipient>(
        r#"
        SELECT telegram_id
        FROM v2_user
        WHERE telegram_id IS NOT NULL
          AND (is_admin = 1 OR (? = 1 AND is_staff = 1))
        "#,
    )
    .bind(include_staff as i32)
    .fetch_all(&state.db)
    .await
    .map_err(|error| TelegramCommandError::without_chat(error.to_string()))?;
    for user in users {
        telegram_send_message(bot_token, user.telegram_id, message, Some("markdown")).await?;
    }
    Ok(())
}

async fn telegram_bot_username(bot_token: &str) -> Result<String, TelegramCommandError> {
    let value = reqwest::Client::new()
        .get(format!("https://api.telegram.org/bot{bot_token}/getMe"))
        .send()
        .await
        .map_err(|error| {
            TelegramCommandError::without_chat(format!("Telegram request failed: {error}"))
        })?
        .json::<serde_json::Value>()
        .await
        .map_err(|error| {
            TelegramCommandError::without_chat(format!("Telegram response failed: {error}"))
        })?;
    value
        .get("result")
        .and_then(|result| result.get("username"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| TelegramCommandError::without_chat("Telegram bot response is invalid"))
}

async fn telegram_send_message(
    bot_token: &str,
    chat_id: i64,
    text: &str,
    parse_mode: Option<&str>,
) -> Result<(), TelegramCommandError> {
    let text = if parse_mode == Some("markdown") {
        escape_telegram_markdown(text)
    } else {
        text.to_string()
    };
    let mut params = vec![("chat_id", chat_id.to_string()), ("text", text)];
    if let Some(parse_mode) = parse_mode {
        params.push(("parse_mode", parse_mode.to_string()));
    }
    let body = serde_urlencoded::to_string(params)
        .map_err(|_| TelegramCommandError::new(chat_id, "telegram request encode failed"))?;
    let response = reqwest::Client::new()
        .post(format!(
            "https://api.telegram.org/bot{bot_token}/sendMessage"
        ))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|error| {
            TelegramCommandError::new(chat_id, format!("Telegram request failed: {error}"))
        })?;
    if !response.status().is_success() {
        return Err(TelegramCommandError::new(
            chat_id,
            "Telegram request failed",
        ));
    }
    Ok(())
}

async fn resolve_subscribe_token_for_telegram_bind(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    match state.config_snapshot().show_subscribe_method {
        0 => Ok(token.to_string()),
        1 => {
            let mut conn = state.redis.get_multiplexed_async_connection().await?;
            conn.get::<_, Option<String>>(format!("otpn_{token}"))
                .await?
                .ok_or_else(|| forbidden("token is error"))
        }
        2 => resolve_totp_subscribe_token(state, token).await,
        _ => Ok(token.to_string()),
    }
}

fn telegram_command_parts(text: &str) -> Option<(&str, Vec<&str>)> {
    let mut parts = text.split_whitespace();
    let command = parts.next()?;
    command
        .starts_with('/')
        .then(|| (command, parts.collect::<Vec<_>>()))
}

fn subscribe_token_from_url(value: &str) -> Option<String> {
    let query = value.split_once('?')?.1;
    parse_urlencoded_params(query)
        .ok()?
        .remove("token")
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn telegram_reply_ticket_id(text: &str) -> Option<i32> {
    let after_hash = text.split_once('#')?.1.trim_start();
    let digits = after_hash
        .trim_start_matches('`')
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse::<i32>().ok()
}

fn escape_telegram_markdown(text: &str) -> String {
    text.replace('_', "\\_")
}

fn legacy_traffic_convert(bytes: i64) -> String {
    if bytes < 0 {
        return "0".to_string();
    }
    let value = bytes as f64;
    let kb = 1024.0;
    let mb = 1_048_576.0;
    let gb = 1_073_741_824.0;
    if value > gb {
        format_legacy_decimal(value / gb, "GB")
    } else if value > mb {
        format_legacy_decimal(value / mb, "MB")
    } else if value > kb {
        format_legacy_decimal(value / kb, "KB")
    } else {
        format_legacy_decimal(value, "B")
    }
}

fn format_legacy_decimal(value: f64, unit: &str) -> String {
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    format!("{text} {unit}")
}

#[derive(Debug)]
struct TelegramCommandError {
    chat_id: Option<i64>,
    message: String,
}

impl TelegramCommandError {
    fn new(chat_id: i64, message: impl Into<String>) -> Self {
        Self {
            chat_id: Some(chat_id),
            message: message.into(),
        }
    }

    fn without_chat(message: impl Into<String>) -> Self {
        Self {
            chat_id: None,
            message: message.into(),
        }
    }
}

impl From<ApiError> for TelegramCommandError {
    fn from(error: ApiError) -> Self {
        Self::without_chat(error.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    message: Option<TelegramMessage>,
    chat_join_request: Option<TelegramChatJoinRequest>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    text: Option<String>,
    chat: TelegramChat,
    reply_to_message: Option<TelegramReplyMessage>,
}

impl TelegramMessage {
    fn is_private(&self) -> bool {
        self.chat.kind.as_deref() == Some("private")
    }
}

#[derive(Debug, Deserialize)]
struct TelegramReplyMessage {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramChatJoinRequest {
    from: TelegramUserIdentity,
    chat: TelegramChat,
}

#[derive(Debug, Deserialize)]
struct TelegramUserIdentity {
    id: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TelegramUserRow {
    id: i64,
    email: String,
    telegram_id: Option<i64>,
    is_admin: i8,
    is_staff: i8,
    u: i64,
    d: i64,
    transfer_enable: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TelegramAdminRecipient {
    telegram_id: i64,
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
    let url = auth
        .quick_login_url(user.id, payload.redirect.as_deref())
        .await?;
    Ok(legacy_data(url))
}

async fn send_email_verify(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(payload): Form<EmailVerifyInput>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
    Ok(legacy_data(
        auth.send_email_verify(payload, Some(addr.ip().to_string()))
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
struct PassportPvRequest {
    invite_code: Option<String>,
}

async fn passport_pv(
    State(state): State<AppState>,
    Form(payload): Form<PassportPvRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let config = state.config_snapshot();
    Ok(legacy_data(UserCommConfig {
        is_telegram: config.telegram_bot_enable as i32,
        telegram_discuss_link: config.telegram_discuss_link,
        stripe_pk: config.stripe_pk_live,
        withdraw_methods: config.commission_withdraw_method,
        withdraw_close: config.withdraw_close_enable as i32,
        currency: config.currency,
        currency_symbol: config.currency_symbol,
        commission_distribution_enable: config.commission_distribution_enable as i32,
        commission_distribution_l1: config.commission_distribution_l1,
        commission_distribution_l2: config.commission_distribution_l2,
        commission_distribution_l3: config.commission_distribution_l3,
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
    let config = state.config_snapshot();
    let reset_day = reset_day(subscribe.expired_at, plan.as_ref(), &config);
    let subscribe_url = subscribe_url_for_user(&state, user.id, &subscribe.token).await?;

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
        allow_new_period: config.allow_new_period,
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
    let config = state.config_snapshot();
    if config.allow_new_period == 0 {
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
    let mut reset_day = reset_day_by_method(expired_at, row.reset_traffic_method, &config)
        .ok_or_else(|| ApiError::legacy("You do not allow to renew the subscription"))?;
    let mut period = reset_period_by_method(row.reset_traffic_method, &config)
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
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
    let service =
        v2board_domain::order::OrderService::new(state.db.clone(), state.config_snapshot());
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
    let config = state.config_snapshot();
    let mut order = v2board_db::order::find_user_order(
        &state.db,
        user.id,
        &query.trade_no,
        config.try_out_plan_id,
    )
    .await?
    .ok_or_else(|| ApiError::legacy("Order does not exist or has been paid"))?;
    if order.plan_id != 0 && order.plan.is_none() {
        return Err(ApiError::legacy("Subscription plan does not exist"));
    }
    // Deposit orders (plan_id == 0) advertise the reward tier: `bounus` and
    // `get_amount = total_amount + bounus` (OrderController::detail). The db layer is config-free,
    // so the real tier lookup happens here.
    if order.plan_id == 0 {
        let bonus = config.deposit_bonus(order.total_amount);
        order.bounus = Some(bonus);
        order.get_amount = Some(order.total_amount + bonus);
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
        state.config_snapshot().invite_gen_limit,
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
    match state.config_snapshot().ticket_status {
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
    let config = state.config_snapshot();
    if config.withdraw_close_enable {
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
    if !config
        .commission_withdraw_method
        .iter()
        .any(|allowed| allowed == method)
    {
        return Err(ApiError::legacy("Unsupported withdrawal method"));
    }
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if config.commission_withdraw_limit > access.commission_balance / 100 {
        return Err(ApiError::legacy(format!(
            "The current required minimum withdrawal commission is {}",
            config.commission_withdraw_limit
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
    let mut servers =
        v2board_db::server::fetch_available_servers(&state.db, access.group_id).await?;
    crate::server_api::hydrate_online_status(&state.redis, &mut servers).await?;
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
        let config = state.config_snapshot();
        let subscribe_url = subscribe_url_for_user(&state, user.id, &access.token).await?;
        knowledge.body =
            render_knowledge_body(&knowledge.body, &config, &subscribe_url, &access.token);
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
    let config = state.config_snapshot();
    let token = config
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
    let auth = AuthService::new(
        state.db.clone(),
        state.redis.clone(),
        state.config_snapshot(),
    );
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
    match state.config_snapshot().show_subscribe_method {
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

    let timestep = (state.config_snapshot().show_subscribe_expire.max(1) * 60) as u64;
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

/// Mirror `Helper::getSubscribeUrl`: derive the method-specific token so the generated URL
/// resolves back through [`resolve_subscribe_token`]. Method 0 keeps the raw token; method 1
/// mints/reuses a one-time token; method 2 derives the time-stepped `{id}:{hmac}` token.
async fn subscribe_url_for_user(
    state: &AppState,
    user_id: i64,
    token: &str,
) -> Result<String, ApiError> {
    let config = state.config_snapshot();
    let method_token = match config.show_subscribe_method {
        1 => one_time_subscribe_token(state, token).await?,
        2 => totp_subscribe_token(&config, user_id, token)?,
        _ => token.to_string(),
    };
    Ok(config.subscribe_url_for_token(&method_token))
}

/// Method 1 token: `Cache::add("otp_{token}")` mints a fresh 24-byte url-safe token and stores
/// the reverse `otpn_{newtoken}` mapping the subscribe middleware pulls. The SET NX mirrors
/// `Cache::add`, so a concurrent generator that loses the race reuses the winner's token.
async fn one_time_subscribe_token(state: &AppState, token: &str) -> Result<String, ApiError> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    if let Some(existing) = conn
        .get::<_, Option<String>>(format!("otp_{token}"))
        .await?
        .filter(|value| !value.is_empty())
    {
        return Ok(existing);
    }
    let mut raw = [0_u8; 24];
    raw[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    raw[16..].copy_from_slice(&Uuid::new_v4().as_bytes()[..8]);
    let new_token = safe_base64_encode(&raw);
    let added: Option<String> = redis::cmd("SET")
        .arg(format!("otp_{token}"))
        .arg(&new_token)
        .arg("NX")
        .arg("EX")
        .arg(86400)
        .query_async(&mut conn)
        .await?;
    if added.is_some() {
        let _: () = conn
            .set_ex(format!("otpn_{new_token}"), token, 86400)
            .await?;
        return Ok(new_token);
    }
    conn.get::<_, Option<String>>(format!("otp_{token}"))
        .await?
        .ok_or_else(|| ApiError::internal("subscribe token race lost"))
}

/// Method 2 token: `base64url("{user_id}:{hmac_sha1(counterBytes, token)}")`, derived purely so
/// it stays in lock-step with [`resolve_totp_subscribe_token`] for the same time window.
fn totp_subscribe_token(config: &AppConfig, user_id: i64, token: &str) -> Result<String, ApiError> {
    let timestep = (config.show_subscribe_expire.max(1) * 60) as u64;
    let counter = Utc::now().timestamp().max(0) as u64 / timestep;
    let mut counter_bytes = [0_u8; 8];
    counter_bytes[4..].copy_from_slice(&(counter as u32).to_be_bytes());
    let hash = hmac_sha1_hex(token.as_bytes(), &counter_bytes)?;
    Ok(safe_base64_encode(format!("{user_id}:{hash}").as_bytes()))
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
    fn custom_subscribe_route_skips_default_and_registers_custom_path() {
        assert_eq!(
            routes::custom_subscribe_route_path_from_str("/api/v1/client/subscribe"),
            None
        );
        assert_eq!(
            routes::custom_subscribe_route_path_from_str("/api/v1/client/subscribe/"),
            None
        );
        assert_eq!(
            routes::custom_subscribe_route_path_from_str("/custom/subscribe"),
            Some("/custom/subscribe".to_string())
        );
        assert_eq!(
            routes::custom_subscribe_route_path_from_str("/custom/subscribe/"),
            Some("/custom/subscribe".to_string())
        );
    }

    #[test]
    fn current_admin_route_match_uses_latest_config_path() {
        let mut config = AppConfig::from_env();
        config.secure_path = Some("new-admin".to_string());
        config.frontend_admin_path = None;

        assert!(path_matches_current_admin_route(
            &config,
            "/api/v1/new-admin/config/fetch"
        ));
        assert!(!path_matches_current_admin_route(
            &config,
            "/api/v1/admin/config/fetch"
        ));
    }

    #[test]
    fn legacy_error_localization_covers_password_limit_message() {
        assert_eq!(
            localize_legacy_message(
                "There are too many password errors, please try again after 15 minutes.",
                "zh-CN"
            ),
            "密码错误次数过多，请 15 分钟后再试"
        );
    }

    #[test]
    fn telegram_subscribe_token_is_read_from_url_query() {
        assert_eq!(
            subscribe_token_from_url(
                "https://example.test/api/v1/client/subscribe?token=user-token&flag=clash"
            ),
            Some("user-token".to_string())
        );
        assert_eq!(
            subscribe_token_from_url("https://example.test/api/v1/client/subscribe?flag=clash"),
            None
        );
    }

    #[test]
    fn telegram_reply_ticket_id_accepts_legacy_notification_text() {
        assert_eq!(telegram_reply_ticket_id("📮工单提醒 #123\n主题"), Some(123));
        assert_eq!(
            telegram_reply_ticket_id("#`456` 的工单已回复成功"),
            Some(456)
        );
        assert_eq!(telegram_reply_ticket_id("no ticket id"), None);
    }

    #[test]
    fn telegram_traffic_format_matches_legacy_units() {
        assert_eq!(legacy_traffic_convert(-1), "0");
        assert_eq!(legacy_traffic_convert(1024), "1024 B");
        assert_eq!(legacy_traffic_convert(1025), "1 KB");
        assert_eq!(legacy_traffic_convert(1_048_577), "1 MB");
        assert_eq!(legacy_traffic_convert(2_147_483_648), "2 GB");
    }

    #[test]
    fn telegram_markdown_escape_matches_legacy_underscore_escape() {
        assert_eq!(escape_telegram_markdown("hello_world"), "hello\\_world");
    }
}
