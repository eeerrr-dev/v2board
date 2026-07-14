use axum::{
    Json,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header},
};
use chrono::Utc;
use redis::AsyncCommands;
use serde::Deserialize;
use serde_json::json;
use v2board_compat::ApiError;

use crate::{
    request_params::parse_urlencoded_params,
    runtime::AppState,
    user::{resolve_totp_subscribe_token, user_is_available},
    validation::forbidden,
};

pub(crate) async fn telegram_webhook(
    State(state): State<AppState>,
    request: Request,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = state.config_snapshot();
    let token = config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("telegram bot token is null"))?;
    let expected = v2board_domain::admin::telegram_webhook_secret(&config.app_key, token);
    let supplied = request
        .headers()
        .get("x-telegram-bot-api-secret-token")
        .and_then(|value| value.to_str().ok());
    if !supplied
        .is_some_and(|supplied| v2board_compat::constant_time_secret_eq(&expected, supplied))
    {
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

    let Ok(update) = serde_json::from_value::<TelegramUpdate>(payload) else {
        return Ok(Json(json!({ "data": true })));
    };
    let update_key = state.redis_key(&format!(
        "TELEGRAM_UPDATE_{}_{id}",
        &expected[..16],
        id = update.update_id
    ));
    if !claim_telegram_update(&state, &update_key).await? {
        return Ok(Json(json!({ "data": true })));
    }

    let result = async {
        if let Some(join) = update.chat_join_request {
            handle_telegram_chat_join_request(&state, token, join).await?;
        }
        if let Some(message) = update.message
            && let Err(error) = handle_telegram_message(&state, token, message).await
            && let Some(chat_id) = error.chat_id
            && let Err(send_error) =
                telegram_send_message(&state.http, token, chat_id, &error.message, None).await
        {
            tracing::warn!(
                ?send_error,
                "failed to send telegram command error response"
            );
        }
        Ok::<(), ApiError>(())
    }
    .await;
    if result.is_err() {
        release_telegram_update(&state, &update_key).await;
    }
    result?;

    Ok(Json(json!({ "data": true })))
}

async fn claim_telegram_update(state: &AppState, key: &str) -> Result<bool, ApiError> {
    let mut conn = state.auth_redis.clone();
    let claimed: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg("processing")
        .arg("NX")
        .arg("EX")
        .arg(30 * 24 * 60 * 60)
        .query_async(&mut conn)
        .await?;
    Ok(claimed.is_some())
}

async fn release_telegram_update(state: &AppState, key: &str) {
    let mut conn = state.auth_redis.clone();
    if let Err(error) = conn.del::<_, i64>(key).await {
        tracing::warn!(?error, "failed to release Telegram update claim");
    }
}

async fn telegram_chat_join_request(
    client: &reqwest::Client,
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
    let response = client
        .post(format!("https://api.telegram.org/bot{bot_token}/{method}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|error| {
            ApiError::legacy(format!("Telegram request failed: {}", error.without_url()))
        })?;
    let status = response.status();
    v2board_domain::http_response::bounded_bytes(
        response,
        v2board_domain::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Telegram request failed",
    )
    .await?;
    if !status.is_success() {
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
        SELECT id, token, uuid, group_id, plan_id, banned, u, d, transfer_enable, expired_at, commission_balance
        FROM users
        WHERE telegram_id = $1
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
    telegram_chat_join_request(&state.http, bot_token, method, chat_id, telegram_id).await
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

    // Laravel formatMessage marks a message carrying `reply_to_message.text` as
    // `message_type = 'reply_message'` and dispatches it ONLY through the reply-ticket
    // regex path; a plain message dispatches ONLY through the command table. The two
    // paths are mutually exclusive.
    if let Some(reply_text) = message
        .reply_to_message
        .as_ref()
        .and_then(|reply| reply.text.as_deref())
    {
        if let Some(ticket_id) = telegram_reply_ticket_id(reply_text) {
            telegram_reply_ticket(state, bot_token, &message, ticket_id).await?;
        }
        return Ok(());
    }

    if let Some((command, args)) = telegram_command_parts(text) {
        let command = normalize_telegram_command(&state.http, bot_token, command).await?;
        match command.as_str() {
            "/bind" => return telegram_bind(state, bot_token, &message, &args).await,
            "/unbind" => return telegram_unbind(state, bot_token, &message).await,
            "/traffic" => return telegram_traffic(state, bot_token, &message).await,
            "/getlatesturl" => return telegram_latest_url(state, bot_token, &message).await,
            _ => {}
        }
    }
    Ok(())
}

async fn normalize_telegram_command(
    client: &reqwest::Client,
    bot_token: &str,
    command: &str,
) -> Result<String, TelegramCommandError> {
    // Laravel compares the command and the `@bot` suffix case-sensitively
    // (`$msg->command !== $instance->command`, `$commandName[1] === $botName`), so the
    // raw casing must be preserved rather than lowercased.
    let (command, bot_name) = command.split_once('@').unwrap_or((command, ""));
    if bot_name.is_empty() {
        return Ok(command.to_string());
    }
    let current_bot = telegram_bot_username(client, bot_token).await?;
    if bot_name == current_bot {
        Ok(command.to_string())
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
        FROM users
        WHERE token = $1
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
    let updated = sqlx::query("UPDATE users SET telegram_id = $1, updated_at = $2 WHERE id = $3")
        .bind(chat_id)
        .bind(Utc::now().timestamp())
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|error| TelegramCommandError::new(chat_id, error.to_string()))?;
    if updated.rows_affected() == 0 {
        return Err(TelegramCommandError::new(chat_id, "设置失败"));
    }
    telegram_send_message(&state.http, bot_token, chat_id, "绑定成功", None).await?;
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
            &state.http,
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
    telegram_send_message(
        &state.http,
        bot_token,
        chat_id,
        "解绑成功",
        Some("markdown"),
    )
    .await?;
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
            &state.http,
            bot_token,
            chat_id,
            "没有查询到您的用户信息，请先绑定账号",
            Some("markdown"),
        )
        .await?;
        return Ok(());
    };
    let used = user
        .u
        .checked_add(user.d)
        .ok_or_else(|| ApiError::internal("user traffic exceeds the supported range"))?;
    let remaining = user
        .transfer_enable
        .checked_sub(used)
        .ok_or_else(|| ApiError::internal("user traffic exceeds the supported range"))?;
    let text = format!(
        "🚥流量查询\n———————————————\n计划流量：`{}`\n已用上行：`{}`\n已用下行：`{}`\n剩余流量：`{}`",
        legacy_traffic_convert(user.transfer_enable),
        legacy_traffic_convert(user.u),
        legacy_traffic_convert(user.d),
        legacy_traffic_convert(remaining),
    );
    telegram_send_message(&state.http, bot_token, chat_id, &text, Some("markdown")).await?;
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
    telegram_send_message(
        &state.http,
        bot_token,
        message.chat.id,
        &text,
        Some("markdown"),
    )
    .await?;
    Ok(())
}

async fn telegram_reply_ticket(
    state: &AppState,
    bot_token: &str,
    message: &TelegramMessage,
    ticket_id: i64,
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
        &state.http,
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
    ticket_id: i64,
    user_id: i64,
    message: &str,
) -> Result<(), ApiError> {
    let now = Utc::now().timestamp();
    match v2board_db::ticket::reply_ticket_as_operator(&state.db, ticket_id, user_id, message, now)
        .await?
    {
        v2board_db::ticket::OperatorReplyTargetOutcome::Locked(_) => Ok(()),
        v2board_db::ticket::OperatorReplyTargetOutcome::NotFound => {
            Err(ApiError::legacy("工单不存在"))
        }
        v2board_db::ticket::OperatorReplyTargetOutcome::OtherOpenTicketExists => Err(
            ApiError::legacy("用户存在其他未解决工单，无法重新打开该工单"),
        ),
    }
}

async fn telegram_user_by_chat_id(
    state: &AppState,
    chat_id: i64,
) -> Result<Option<TelegramUserRow>, ApiError> {
    Ok(sqlx::query_as::<_, TelegramUserRow>(
        r#"
        SELECT id, email, telegram_id, is_admin, is_staff, u, d, transfer_enable
        FROM users
        WHERE telegram_id = $1
        LIMIT 1
        "#,
    )
    .bind(chat_id)
    .fetch_optional(&state.db)
    .await?)
}

pub(crate) async fn send_telegram_message_with_admin(
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
        FROM users
        WHERE telegram_id IS NOT NULL
          AND (is_admin = 1 OR ($1 = 1 AND is_staff = 1))
        "#,
    )
    .bind(include_staff as i32)
    .fetch_all(&state.db)
    .await
    .map_err(|error| TelegramCommandError::without_chat(error.to_string()))?;
    for user in users {
        telegram_send_message(
            &state.http,
            bot_token,
            user.telegram_id,
            message,
            Some("markdown"),
        )
        .await?;
    }
    Ok(())
}

async fn telegram_bot_username(
    client: &reqwest::Client,
    bot_token: &str,
) -> Result<String, TelegramCommandError> {
    let response = client
        .get(format!("https://api.telegram.org/bot{bot_token}/getMe"))
        .send()
        .await
        .map_err(|error| {
            TelegramCommandError::without_chat(format!(
                "Telegram request failed: {}",
                error.without_url()
            ))
        })?;
    let value: serde_json::Value = v2board_domain::http_response::bounded_json(
        response,
        v2board_domain::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Telegram response failed",
    )
    .await
    .map_err(|_| TelegramCommandError::without_chat("Telegram response failed"))?;
    value
        .get("result")
        .and_then(|result| result.get("username"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| TelegramCommandError::without_chat("Telegram bot response is invalid"))
}

async fn telegram_send_message(
    client: &reqwest::Client,
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
    let response = client
        .post(format!(
            "https://api.telegram.org/bot{bot_token}/sendMessage"
        ))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|error| {
            TelegramCommandError::new(
                chat_id,
                format!("Telegram request failed: {}", error.without_url()),
            )
        })?;
    let status = response.status();
    v2board_domain::http_response::bounded_bytes(
        response,
        v2board_domain::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Telegram request failed",
    )
    .await
    .map_err(|_| TelegramCommandError::new(chat_id, "Telegram response failed"))?;
    if !status.is_success() {
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
            let mut conn = state.auth_redis.clone();
            conn.get::<_, Option<String>>(state.redis_key(&format!("otpn_{token}")))
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

fn telegram_reply_ticket_id(text: &str) -> Option<i64> {
    let after_hash = text.split_once('#')?.1.trim_start();
    let digits = after_hash
        .trim_start_matches('`')
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse::<i64>().ok()
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
pub(crate) struct TelegramCommandError {
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
    update_id: i64,
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
    is_admin: i16,
    is_staff: i16,
    u: i64,
    d: i64,
    transfer_enable: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TelegramAdminRecipient {
    telegram_id: i64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        TelegramUpdate, escape_telegram_markdown, legacy_traffic_convert, subscribe_token_from_url,
        telegram_reply_ticket_id,
    };

    #[test]
    fn telegram_update_requires_the_replay_identifier() {
        let update: TelegramUpdate = serde_json::from_value(json!({
            "update_id": 42,
            "message": { "chat": { "id": 7, "type": "private" } }
        }))
        .unwrap();
        assert_eq!(update.update_id, 42);
        assert!(
            serde_json::from_value::<TelegramUpdate>(json!({
                "message": { "chat": { "id": 7, "type": "private" } }
            }))
            .is_err()
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
