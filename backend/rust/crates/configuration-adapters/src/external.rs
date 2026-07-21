use std::{sync::Arc, time::Duration};

use lettre::{AsyncTransport, Message, message::header::ContentType};
use serde_json::{Value, json};
use v2board_application::configuration::{
    ConfigurationCode, ConfigurationExternal, ConfigurationPortError,
};
use v2board_config::AppConfig;
use v2board_mail_adapters::smtp::{SmtpSettings, SmtpTransportCache};

use crate::telegram_webhook_secret;

const REDACTED_SECRET: &str = "********";

#[derive(Clone)]
pub struct RuntimeConfigurationExternal {
    config: Arc<AppConfig>,
    http: reqwest::Client,
    smtp: SmtpTransportCache,
}

impl RuntimeConfigurationExternal {
    pub const fn new(
        config: Arc<AppConfig>,
        http: reqwest::Client,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self { config, http, smtp }
    }
}

impl ConfigurationExternal for RuntimeConfigurationExternal {
    async fn send_test_mail(&self, recipient: &str) -> Result<(), ConfigurationPortError> {
        let settings = smtp_settings(&self.config)?;
        let from = settings
            .from_address
            .as_deref()
            .or(settings.username.as_deref())
            .ok_or_else(|| business(ConfigurationCode::MailSenderNotConfigured, None))?;
        let body = v2board_mail_adapters::mail::render_notify(
            &self.config.app_name,
            self.config.app_url.as_deref().unwrap_or_default(),
            "This is v2board test email",
        );
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.app_name, from)
                    .parse()
                    .map_err(|_| {
                        business(
                            ConfigurationCode::MailSenderNotConfigured,
                            Some("Email sender is invalid"),
                        )
                    })?,
            )
            .to(recipient.parse().map_err(|_| {
                business(
                    ConfigurationCode::MailInvalid,
                    Some("Email recipient is invalid"),
                )
            })?)
            .subject("This is v2board test email")
            .header(ContentType::TEXT_HTML)
            .body(body)
            .map_err(|_| {
                business(
                    ConfigurationCode::MailInvalid,
                    Some("Email content is invalid"),
                )
            })?;
        let transport = self
            .smtp
            .transport(&settings)
            .map_err(|error| ConfigurationPortError::Internal(error.to_string()))?;
        tokio::time::timeout(
            Duration::from_secs(self.config.http_request_timeout_seconds),
            transport.send(email),
        )
        .await
        .map_err(|_| {
            business(
                ConfigurationCode::MailSendFailed,
                Some("Email send timed out"),
            )
        })?
        .map_err(|error| {
            tracing::warn!(?error, "test mail send failed");
            business(ConfigurationCode::MailSendFailed, None)
        })?;
        Ok(())
    }

    async fn set_telegram_webhook(
        &self,
        token: Option<&str>,
    ) -> Result<(), ConfigurationPortError> {
        let token = token
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != REDACTED_SECRET)
            .or(self.config.telegram_bot_token.as_deref())
            .ok_or_else(|| {
                business(
                    ConfigurationCode::TelegramTokenInvalid,
                    Some("Telegram bot token cannot be empty"),
                )
            })?;
        let hook_url = format!(
            "{}/api/v1/guest/telegram/webhook",
            self.config
                .app_url
                .as_deref()
                .unwrap_or_default()
                .trim_end_matches('/')
        );
        let secret_token = telegram_webhook_secret(&self.config.app_key, token);
        let response = self
            .http
            .get(format!("https://api.telegram.org/bot{token}/getMe"))
            .send()
            .await
            .map_err(|_| business(ConfigurationCode::TelegramRequestFailed, None))?;
        let me: Value = v2board_http_adapters::bounded_json(
            response,
            v2board_http_adapters::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await
        .map_err(|_| business(ConfigurationCode::TelegramRequestFailed, None))?;
        if me.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(business(ConfigurationCode::TelegramTokenInvalid, None));
        }
        let response = self
            .http
            .post(format!("https://api.telegram.org/bot{token}/setWebhook"))
            .json(&json!({ "url": hook_url, "secret_token": secret_token }))
            .send()
            .await
            .map_err(|_| business(ConfigurationCode::TelegramRequestFailed, None))?;
        let result: Value = v2board_http_adapters::bounded_json(
            response,
            v2board_http_adapters::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await
        .map_err(|_| business(ConfigurationCode::TelegramRequestFailed, None))?;
        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(business(ConfigurationCode::TelegramWebhookFailed, None));
        }
        Ok(())
    }
}

fn smtp_settings(config: &AppConfig) -> Result<SmtpSettings, ConfigurationPortError> {
    let host = config
        .email_host
        .clone()
        .filter(|host| !host.trim().is_empty())
        .ok_or_else(|| {
            business(
                ConfigurationCode::MailSenderNotConfigured,
                Some("Email host is not configured"),
            )
        })?;
    Ok(SmtpSettings {
        host,
        port: config
            .email_port
            .and_then(|value| u16::try_from(value).ok()),
        username: config.email_username.clone(),
        password: config.email_password.clone(),
        encryption: config
            .email_encryption
            .as_deref()
            .map(str::to_ascii_lowercase),
        from_address: config.email_from_address.clone(),
    })
}

fn business(code: ConfigurationCode, detail: Option<&str>) -> ConfigurationPortError {
    ConfigurationPortError::Business {
        code,
        detail: detail.map(str::to_string),
    }
}
