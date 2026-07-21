use axum::{
    Json,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header},
};
use serde::Deserialize;
use serde_json::json;
use v2board_application::telegram::{TelegramChatJoinRequest, TelegramMessage, TelegramUpdate};
use v2board_compat::ApiError;

use crate::{request_params::parse_urlencoded_params, runtime::AppState};

/// Frozen external Telegram webhook: secret header, JSON/form tolerance,
/// replay identity, and `{data:true}` acknowledgement remain byte-compatible.
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
    let expected = v2board_configuration_adapters::telegram_webhook_secret(&config.app_key, token);
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

    let Ok(update) = serde_json::from_value::<WireTelegramUpdate>(payload) else {
        return Ok(Json(json!({ "data": true })));
    };
    let replay_scope = &expected[..16];
    state
        .telegram_service(token.to_string())
        .process_webhook(replay_scope, update.into(), chrono::Utc::now().timestamp())
        .await
        .map_err(|error| ApiError::legacy(error.to_string()))?;
    Ok(Json(json!({ "data": true })))
}

/// Application-backed operator notification entry point used by payment and
/// reconciliation callbacks. SQL audience selection and HTTP delivery no
/// longer live in those inbound handlers.
pub(crate) async fn send_telegram_message_with_admin(
    state: &AppState,
    bot_token: &str,
    message: &str,
    include_staff: bool,
) -> Result<(), ApiError> {
    state
        .telegram_service(bot_token.to_string())
        .notify_admins(message, include_staff)
        .await
        .map_err(|error| ApiError::legacy(error.to_string()))
}

#[derive(Debug, Deserialize)]
struct WireTelegramUpdate {
    update_id: i64,
    message: Option<WireTelegramMessage>,
    chat_join_request: Option<WireTelegramChatJoinRequest>,
}

impl From<WireTelegramUpdate> for TelegramUpdate {
    fn from(update: WireTelegramUpdate) -> Self {
        Self {
            update_id: update.update_id,
            message: update.message.map(Into::into),
            chat_join_request: update.chat_join_request.map(Into::into),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WireTelegramMessage {
    text: Option<String>,
    chat: WireTelegramChat,
    reply_to_message: Option<WireTelegramReplyMessage>,
}

impl From<WireTelegramMessage> for TelegramMessage {
    fn from(message: WireTelegramMessage) -> Self {
        Self {
            chat_id: message.chat.id,
            is_private: message.chat.kind.as_deref() == Some("private"),
            text: message.text,
            reply_to_text: message.reply_to_message.and_then(|reply| reply.text),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WireTelegramReplyMessage {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireTelegramChat {
    id: i64,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireTelegramChatJoinRequest {
    from: WireTelegramUserIdentity,
    chat: WireTelegramChat,
}

impl From<WireTelegramChatJoinRequest> for TelegramChatJoinRequest {
    fn from(request: WireTelegramChatJoinRequest) -> Self {
        Self {
            chat_id: request.chat.id,
            user_id: request.from.id,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WireTelegramUserIdentity {
    id: i64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::WireTelegramUpdate;

    #[test]
    fn frozen_wire_update_requires_replay_id_and_preserves_private_reply_shape() {
        let update: WireTelegramUpdate = serde_json::from_value(json!({
            "update_id": 42,
            "message": {
                "text": "reply",
                "chat": { "id": 7, "type": "private" },
                "reply_to_message": { "text": "ticket #123" }
            }
        }))
        .unwrap();
        let application: v2board_application::telegram::TelegramUpdate = update.into();
        assert_eq!(application.update_id, 42);
        assert!(application.message.as_ref().unwrap().is_private);
        assert_eq!(
            application.message.unwrap().reply_to_text.as_deref(),
            Some("ticket #123")
        );
        assert!(
            serde_json::from_value::<WireTelegramUpdate>(json!({
                "message": { "chat": { "id": 7, "type": "private" } }
            }))
            .is_err()
        );
    }
}
