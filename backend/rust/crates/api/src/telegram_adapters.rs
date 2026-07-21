use axum::http::{StatusCode, header};
use redis::AsyncCommands as _;
use v2board_application::{
    telegram::{TelegramExternal, TelegramExternalError, TelegramJoinDecision, TelegramParseMode},
    ticket::{OperatorIdentity, TicketError},
};

use crate::{
    request_params::parse_urlencoded_params, runtime::AppState, user::resolve_totp_subscribe_token,
};

const TELEGRAM_RETRY_DELAYS: [std::time::Duration; 2] = [
    std::time::Duration::from_millis(250),
    std::time::Duration::from_millis(750),
];

#[derive(Clone)]
pub(crate) struct RuntimeTelegramExternal {
    state: AppState,
    bot_token: String,
}

impl RuntimeTelegramExternal {
    pub(crate) fn new(state: AppState, bot_token: String) -> Self {
        Self { state, bot_token }
    }
}

impl TelegramExternal for RuntimeTelegramExternal {
    async fn claim_update(
        &self,
        replay_scope: &str,
        update_id: i64,
    ) -> Result<bool, TelegramExternalError> {
        let key = self
            .state
            .redis_key(&format!("TELEGRAM_UPDATE_{replay_scope}_{update_id}"));
        let mut connection = self.state.auth_redis.clone();
        let claimed: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("processing")
            .arg("NX")
            .arg("EX")
            .arg(30 * 24 * 60 * 60)
            .query_async(&mut connection)
            .await
            .map_err(TelegramExternalError::new)?;
        Ok(claimed.is_some())
    }

    async fn release_update(&self, replay_scope: &str, update_id: i64) {
        let key = self
            .state
            .redis_key(&format!("TELEGRAM_UPDATE_{replay_scope}_{update_id}"));
        let mut connection = self.state.auth_redis.clone();
        if let Err(error) = connection.del::<_, i64>(key).await {
            tracing::warn!(?error, "failed to release Telegram update claim");
        }
    }

    async fn resolve_subscribe_url(
        &self,
        subscribe_url: &str,
    ) -> Result<Option<String>, TelegramExternalError> {
        let Some(query) = subscribe_url.split_once('?').map(|(_, query)| query) else {
            return Ok(None);
        };
        let Some(token) = parse_urlencoded_params(query)
            .map_err(TelegramExternalError::new)?
            .remove("token")
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
        else {
            return Ok(None);
        };
        match self.state.config_snapshot().show_subscribe_method {
            0 => Ok(Some(token)),
            1 => {
                let mut connection = self.state.auth_redis.clone();
                let resolved = connection
                    .get::<_, Option<String>>(self.state.redis_key(&format!("otpn_{token}")))
                    .await
                    .map_err(TelegramExternalError::new)?;
                resolved
                    .map(Some)
                    .ok_or_else(|| TelegramExternalError::new("token is error"))
            }
            2 => resolve_totp_subscribe_token(&self.state, &token)
                .await
                .map(Some)
                .map_err(TelegramExternalError::new),
            _ => Ok(Some(token)),
        }
    }

    async fn bot_username(&self) -> Result<String, TelegramExternalError> {
        let response = telegram_api_send(|| {
            self.state.http.get(format!(
                "https://api.telegram.org/bot{}/getMe",
                self.bot_token
            ))
        })
        .await
        .map_err(TelegramExternalError::new)?;
        let value: serde_json::Value = v2board_http_adapters::bounded_json(
            response,
            v2board_http_adapters::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram response failed",
        )
        .await
        .map_err(|_| TelegramExternalError::new("Telegram response failed"))?;
        value
            .get("result")
            .and_then(|result| result.get("username"))
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| TelegramExternalError::new("Telegram bot response is invalid"))
    }

    async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        parse_mode: TelegramParseMode,
    ) -> Result<(), TelegramExternalError> {
        let text = match parse_mode {
            TelegramParseMode::Plain => text.to_string(),
            TelegramParseMode::Markdown => escape_telegram_markdown(text),
        };
        let mut params = vec![("chat_id", chat_id.to_string()), ("text", text)];
        if parse_mode == TelegramParseMode::Markdown {
            params.push(("parse_mode", "markdown".to_string()));
        }
        let body = serde_urlencoded::to_string(params).map_err(TelegramExternalError::new)?;
        let response = telegram_api_send(|| {
            self.state
                .http
                .post(format!(
                    "https://api.telegram.org/bot{}/sendMessage",
                    self.bot_token
                ))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body.clone())
        })
        .await
        .map_err(TelegramExternalError::new)?;
        let status = response.status();
        v2board_http_adapters::bounded_bytes(
            response,
            v2board_http_adapters::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await
        .map_err(|_| TelegramExternalError::new("Telegram response failed"))?;
        if status.is_success() {
            Ok(())
        } else {
            Err(TelegramExternalError::new("Telegram request failed"))
        }
    }

    async fn decide_chat_join(
        &self,
        chat_id: i64,
        user_id: i64,
        decision: TelegramJoinDecision,
    ) -> Result<(), TelegramExternalError> {
        let method = match decision {
            TelegramJoinDecision::Approve => "approveChatJoinRequest",
            TelegramJoinDecision::Decline => "declineChatJoinRequest",
        };
        let body = serde_urlencoded::to_string([
            ("chat_id", chat_id.to_string()),
            ("user_id", user_id.to_string()),
        ])
        .map_err(TelegramExternalError::new)?;
        let response = telegram_api_send(|| {
            self.state
                .http
                .post(format!(
                    "https://api.telegram.org/bot{}/{method}",
                    self.bot_token
                ))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body.clone())
        })
        .await
        .map_err(TelegramExternalError::new)?;
        let status = response.status();
        v2board_http_adapters::bounded_bytes(
            response,
            v2board_http_adapters::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await
        .map_err(TelegramExternalError::new)?;
        if status.is_success() {
            Ok(())
        } else {
            Err(TelegramExternalError::new("Telegram request failed"))
        }
    }

    async fn reply_ticket(
        &self,
        ticket_id: i64,
        operator_user_id: i64,
        message: &str,
        now: i64,
    ) -> Result<(), TelegramExternalError> {
        match self
            .state
            .ticket_service()
            .reply_as_operator(
                ticket_id,
                OperatorIdentity::UserId(operator_user_id),
                message.to_string(),
                now,
                false,
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(TicketError::NotFound) => Err(TelegramExternalError::new("工单不存在")),
            Err(TicketError::UnresolvedTicketExists) => Err(TelegramExternalError::new(
                "用户存在其他未解决工单，无法重新打开该工单",
            )),
            Err(TicketError::Validation { detail, .. }) => Err(TelegramExternalError::new(detail)),
            Err(error) => Err(TelegramExternalError::new(error)),
        }
    }
}

pub(crate) async fn telegram_api_send<F>(build_request: F) -> Result<reqwest::Response, String>
where
    F: Fn() -> reqwest::RequestBuilder,
{
    let mut attempt = 0_usize;
    loop {
        let retry_delay = TELEGRAM_RETRY_DELAYS.get(attempt).copied();
        match build_request().send().await {
            Ok(response) => {
                let status = response.status();
                let transient = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                let Some(delay) = retry_delay.filter(|_| transient) else {
                    return Ok(response);
                };
                tracing::warn!(%status, attempt, "retrying transient Telegram API status");
                tokio::time::sleep(delay).await;
            }
            Err(error) => {
                let transient = error.is_connect() || error.is_timeout();
                let sanitized = format!("Telegram request failed: {}", error.without_url());
                let Some(delay) = retry_delay.filter(|_| transient) else {
                    return Err(sanitized);
                };
                tracing::warn!(error = %sanitized, attempt, "retrying transient Telegram transport failure");
                tokio::time::sleep(delay).await;
            }
        }
        attempt += 1;
    }
}

pub(crate) fn escape_telegram_markdown(text: &str) -> String {
    text.replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};

    use super::{escape_telegram_markdown, telegram_api_send};

    fn scripted_server(responses: Vec<&'static [u8]>) -> (String, std::thread::JoinHandle<usize>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let mut served = 0;
            for raw_response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let mut used = 0;
                while used < request.len() {
                    let read = stream.read(&mut request[used..]).unwrap();
                    if read == 0 {
                        break;
                    }
                    used += read;
                    if request[..used]
                        .windows(4)
                        .any(|window| window == b"\r\n\r\n")
                    {
                        break;
                    }
                }
                stream.write_all(raw_response).unwrap();
                stream.flush().unwrap();
                stream.shutdown(std::net::Shutdown::Write).unwrap();
                served += 1;
            }
            served
        });
        (format!("http://{address}/"), handle)
    }

    #[tokio::test]
    async fn telegram_send_retries_only_transient_failures() {
        let (url, server) = scripted_server(vec![
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        ]);
        let client = reqwest::Client::new();
        assert!(
            telegram_api_send(|| client.get(url.clone()))
                .await
                .unwrap()
                .status()
                .is_success()
        );
        assert_eq!(server.join().unwrap(), 3);

        let (url, server) = scripted_server(vec![
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ]);
        assert_eq!(
            telegram_api_send(|| client.get(url.clone()))
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::NOT_FOUND
        );
        assert_eq!(server.join().unwrap(), 1);
    }

    #[test]
    fn markdown_escape_preserves_the_legacy_rule() {
        assert_eq!(escape_telegram_markdown("hello_world"), "hello\\_world");
    }
}
