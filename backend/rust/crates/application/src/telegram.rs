//! Telegram command, webhook replay, join-admission, and operator-notification use cases.
//!
//! Telegram JSON, PostgreSQL, Redis, HTTP retries, token decoding, runtime configuration,
//! and ticket persistence are outer adapters. This module owns command dispatch, account
//! binding policy, subscription admission, traffic presentation, and notification audience.

use v2board_domain_model::SubscriptionAvailability;

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelegramPolicy {
    pub app_name: String,
    pub app_url: String,
    pub notifications_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
    pub chat_join_request: Option<TelegramChatJoinRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelegramMessage {
    pub chat_id: i64,
    pub is_private: bool,
    pub text: Option<String>,
    pub reply_to_text: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelegramChatJoinRequest {
    pub chat_id: i64,
    pub user_id: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelegramUser {
    pub id: i64,
    pub email: String,
    pub is_admin: bool,
    pub is_staff: bool,
    pub uploaded: i64,
    pub downloaded: i64,
    pub transfer_enable: i64,
    pub banned: bool,
    pub expired_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindTelegramOutcome {
    Bound,
    UserNotFound,
    AlreadyBound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnbindTelegramOutcome {
    Unbound,
    UserNotFound,
}

#[allow(async_fn_in_trait)]
pub trait TelegramRepository: Send + Sync {
    async fn user_by_telegram_id(&self, telegram_id: i64)
    -> RepositoryResult<Option<TelegramUser>>;
    async fn bind_telegram(
        &self,
        token: &str,
        telegram_id: i64,
        updated_at: i64,
    ) -> RepositoryResult<BindTelegramOutcome>;
    async fn unbind_telegram(
        &self,
        telegram_id: i64,
        updated_at: i64,
    ) -> RepositoryResult<UnbindTelegramOutcome>;
    async fn admin_recipients(&self, include_staff: bool) -> RepositoryResult<Vec<i64>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelegramParseMode {
    Plain,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelegramJoinDecision {
    Approve,
    Decline,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct TelegramExternalError(String);

impl TelegramExternalError {
    pub fn new(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[allow(async_fn_in_trait)]
pub trait TelegramExternal: Send + Sync {
    async fn claim_update(
        &self,
        replay_scope: &str,
        update_id: i64,
    ) -> Result<bool, TelegramExternalError>;
    async fn release_update(&self, replay_scope: &str, update_id: i64);
    async fn resolve_subscribe_url(
        &self,
        subscribe_url: &str,
    ) -> Result<Option<String>, TelegramExternalError>;
    async fn bot_username(&self) -> Result<String, TelegramExternalError>;
    async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        parse_mode: TelegramParseMode,
    ) -> Result<(), TelegramExternalError>;
    async fn decide_chat_join(
        &self,
        chat_id: i64,
        user_id: i64,
        decision: TelegramJoinDecision,
    ) -> Result<(), TelegramExternalError>;
    async fn reply_ticket(
        &self,
        ticket_id: i64,
        operator_user_id: i64,
        message: &str,
        now: i64,
    ) -> Result<(), TelegramExternalError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TelegramError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("telegram external adapter failed: {0}")]
    External(String),
}

#[derive(Debug)]
struct TelegramCommandError {
    chat_id: i64,
    message: String,
}

impl TelegramCommandError {
    fn new(chat_id: i64, message: impl Into<String>) -> Self {
        Self {
            chat_id,
            message: message.into(),
        }
    }

    fn external(chat_id: i64, error: TelegramExternalError) -> Self {
        Self::new(chat_id, error.to_string())
    }

    fn repository(chat_id: i64, error: RepositoryError) -> Self {
        Self::new(chat_id, error.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct TelegramService<R, E> {
    repository: R,
    external: E,
    policy: TelegramPolicy,
}

impl<R, E> TelegramService<R, E>
where
    R: TelegramRepository,
    E: TelegramExternal,
{
    pub const fn new(repository: R, external: E, policy: TelegramPolicy) -> Self {
        Self {
            repository,
            external,
            policy,
        }
    }

    /// Resolve the public bot identity through the outbound Telegram port.
    ///
    /// The user-facing HTTP endpoint is an inbound adapter like the webhook;
    /// it must not construct provider requests or decode provider JSON itself.
    pub async fn bot_username(&self) -> Result<String, TelegramError> {
        self.external.bot_username().await.map_err(external_error)
    }

    pub async fn process_webhook(
        &self,
        replay_scope: &str,
        update: TelegramUpdate,
        now: i64,
    ) -> Result<(), TelegramError> {
        let claimed = self
            .external
            .claim_update(replay_scope, update.update_id)
            .await
            .map_err(external_error)?;
        if !claimed {
            return Ok(());
        }
        let update_id = update.update_id;
        let result = self.process_claimed_update(update, now).await;
        if result.is_err() {
            self.external.release_update(replay_scope, update_id).await;
        }
        result
    }

    async fn process_claimed_update(
        &self,
        update: TelegramUpdate,
        now: i64,
    ) -> Result<(), TelegramError> {
        if let Some(join) = update.chat_join_request {
            self.handle_chat_join(join, now).await?;
        }
        if let Some(message) = update.message
            && let Err(error) = self.handle_message(&message, now).await
        {
            // Command failures are user-visible but do not make Telegram retry the update.
            let _ = self
                .external
                .send_message(error.chat_id, &error.message, TelegramParseMode::Plain)
                .await;
        }
        Ok(())
    }

    async fn handle_chat_join(
        &self,
        join: TelegramChatJoinRequest,
        now: i64,
    ) -> Result<(), TelegramError> {
        let user = self.repository.user_by_telegram_id(join.user_id).await?;
        let available = user.as_ref().is_some_and(|user| {
            SubscriptionAvailability {
                banned: user.banned,
                transfer_enable: user.transfer_enable,
                expiry: user.expired_at,
            }
            .is_available(now)
        });
        self.external
            .decide_chat_join(
                join.chat_id,
                join.user_id,
                if available {
                    TelegramJoinDecision::Approve
                } else {
                    TelegramJoinDecision::Decline
                },
            )
            .await
            .map_err(external_error)
    }

    async fn handle_message(
        &self,
        message: &TelegramMessage,
        now: i64,
    ) -> Result<(), TelegramCommandError> {
        let Some(text) = message
            .text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        else {
            return Ok(());
        };

        // Reply messages and commands are intentionally mutually exclusive.
        if let Some(reply_text) = message.reply_to_text.as_deref() {
            if let Some(ticket_id) = telegram_reply_ticket_id(reply_text) {
                self.reply_ticket(message, ticket_id, text, now).await?;
            }
            return Ok(());
        }

        let Some((command, args)) = telegram_command_parts(text) else {
            return Ok(());
        };
        let command = self.normalize_command(command, message.chat_id).await?;
        match command.as_str() {
            "/bind" => self.bind(message, &args, now).await,
            "/unbind" => self.unbind(message, now).await,
            "/traffic" => self.traffic(message).await,
            "/getlatesturl" => self.latest_url(message).await,
            _ => Ok(()),
        }
    }

    async fn normalize_command(
        &self,
        command: &str,
        chat_id: i64,
    ) -> Result<String, TelegramCommandError> {
        let (command, bot_name) = command.split_once('@').unwrap_or((command, ""));
        if bot_name.is_empty() {
            return Ok(command.to_string());
        }
        let current_bot = self
            .external
            .bot_username()
            .await
            .map_err(|error| TelegramCommandError::external(chat_id, error))?;
        if bot_name == current_bot {
            Ok(command.to_string())
        } else {
            Ok(String::new())
        }
    }

    async fn bind(
        &self,
        message: &TelegramMessage,
        args: &[&str],
        now: i64,
    ) -> Result<(), TelegramCommandError> {
        if !message.is_private {
            return Ok(());
        }
        let subscribe_url = args
            .first()
            .copied()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                TelegramCommandError::new(message.chat_id, "参数有误，请携带订阅地址发送")
            })?;
        let token = self
            .external
            .resolve_subscribe_url(subscribe_url)
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))?
            .ok_or_else(|| TelegramCommandError::new(message.chat_id, "订阅地址无效"))?;
        match self
            .repository
            .bind_telegram(&token, message.chat_id, now)
            .await
            .map_err(|error| TelegramCommandError::repository(message.chat_id, error))?
        {
            BindTelegramOutcome::Bound => {}
            BindTelegramOutcome::UserNotFound => {
                return Err(TelegramCommandError::new(message.chat_id, "用户不存在"));
            }
            BindTelegramOutcome::AlreadyBound => {
                return Err(TelegramCommandError::new(
                    message.chat_id,
                    "该账号已经绑定了Telegram账号",
                ));
            }
        }
        self.external
            .send_message(message.chat_id, "绑定成功", TelegramParseMode::Plain)
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))
    }

    async fn unbind(
        &self,
        message: &TelegramMessage,
        now: i64,
    ) -> Result<(), TelegramCommandError> {
        if !message.is_private {
            return Ok(());
        }
        let text = match self
            .repository
            .unbind_telegram(message.chat_id, now)
            .await
            .map_err(|error| TelegramCommandError::repository(message.chat_id, error))?
        {
            UnbindTelegramOutcome::Unbound => "解绑成功",
            UnbindTelegramOutcome::UserNotFound => "没有查询到您的用户信息，请先绑定账号",
        };
        self.external
            .send_message(message.chat_id, text, TelegramParseMode::Markdown)
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))
    }

    async fn traffic(&self, message: &TelegramMessage) -> Result<(), TelegramCommandError> {
        if !message.is_private {
            return Ok(());
        }
        let Some(user) = self
            .repository
            .user_by_telegram_id(message.chat_id)
            .await
            .map_err(|error| TelegramCommandError::repository(message.chat_id, error))?
        else {
            return self
                .external
                .send_message(
                    message.chat_id,
                    "没有查询到您的用户信息，请先绑定账号",
                    TelegramParseMode::Markdown,
                )
                .await
                .map_err(|error| TelegramCommandError::external(message.chat_id, error));
        };
        let used = user
            .uploaded
            .checked_add(user.downloaded)
            .ok_or_else(|| TelegramCommandError::new(message.chat_id, "用户流量超出支持范围"))?;
        let remaining = user
            .transfer_enable
            .checked_sub(used)
            .ok_or_else(|| TelegramCommandError::new(message.chat_id, "用户流量超出支持范围"))?;
        let text = format!(
            "🚥流量查询\n———————————————\n计划流量：`{}`\n已用上行：`{}`\n已用下行：`{}`\n剩余流量：`{}`",
            legacy_traffic_convert(user.transfer_enable),
            legacy_traffic_convert(user.uploaded),
            legacy_traffic_convert(user.downloaded),
            legacy_traffic_convert(remaining),
        );
        self.external
            .send_message(message.chat_id, &text, TelegramParseMode::Markdown)
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))
    }

    async fn latest_url(&self, message: &TelegramMessage) -> Result<(), TelegramCommandError> {
        let text = format!(
            "{}的最新网址是：{}",
            self.policy.app_name, self.policy.app_url
        );
        self.external
            .send_message(message.chat_id, &text, TelegramParseMode::Markdown)
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))
    }

    async fn reply_ticket(
        &self,
        message: &TelegramMessage,
        ticket_id: i64,
        text: &str,
        now: i64,
    ) -> Result<(), TelegramCommandError> {
        if !message.is_private {
            return Ok(());
        }
        let Some(user) = self
            .repository
            .user_by_telegram_id(message.chat_id)
            .await
            .map_err(|error| TelegramCommandError::repository(message.chat_id, error))?
        else {
            return Err(TelegramCommandError::new(message.chat_id, "用户不存在"));
        };
        if !user.is_admin && !user.is_staff {
            return Ok(());
        }
        self.external
            .reply_ticket(ticket_id, user.id, text, now)
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))?;
        self.external
            .send_message(
                message.chat_id,
                &format!("#`{ticket_id}` 的工单已回复成功"),
                TelegramParseMode::Markdown,
            )
            .await
            .map_err(|error| TelegramCommandError::external(message.chat_id, error))?;
        self.notify_admins(
            &format!("#`{ticket_id}` 的工单已由 {} 进行回复", user.email),
            true,
        )
        .await
        .map_err(|error| TelegramCommandError::new(message.chat_id, error.to_string()))
    }

    pub async fn notify_admins(
        &self,
        message: &str,
        include_staff: bool,
    ) -> Result<(), TelegramError> {
        if !self.policy.notifications_enabled {
            return Ok(());
        }
        let recipients = self.repository.admin_recipients(include_staff).await?;
        for telegram_id in recipients {
            self.external
                .send_message(telegram_id, message, TelegramParseMode::Markdown)
                .await
                .map_err(external_error)?;
        }
        Ok(())
    }
}

fn external_error(error: TelegramExternalError) -> TelegramError {
    TelegramError::External(error.to_string())
}

fn telegram_command_parts(text: &str) -> Option<(&str, Vec<&str>)> {
    let mut parts = text.split_whitespace();
    let command = parts.next()?;
    command
        .starts_with('/')
        .then(|| (command, parts.collect::<Vec<_>>()))
}

fn telegram_reply_ticket_id(text: &str) -> Option<i64> {
    let after_hash = text.split_once('#')?.1.trim_start();
    let digits = after_hash
        .trim_start_matches('`')
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits.parse::<i64>().ok()
}

fn legacy_traffic_convert(bytes: i64) -> String {
    if bytes < 0 {
        return "0".to_string();
    }
    let value = bytes as f64;
    if value > 1_073_741_824.0 {
        format_legacy_decimal(value / 1_073_741_824.0, "GB")
    } else if value > 1_048_576.0 {
        format_legacy_decimal(value / 1_048_576.0, "MB")
    } else if value > 1024.0 {
        format_legacy_decimal(value / 1024.0, "KB")
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

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Default)]
    struct State {
        user: Option<TelegramUser>,
        bind: Option<BindTelegramOutcome>,
        sent: Vec<(i64, String, TelegramParseMode)>,
        claimed: usize,
        released: usize,
    }

    #[derive(Clone, Default)]
    struct Ports(Arc<Mutex<State>>);

    impl TelegramRepository for Ports {
        async fn user_by_telegram_id(&self, _: i64) -> RepositoryResult<Option<TelegramUser>> {
            Ok(self.0.lock().unwrap().user.clone())
        }

        async fn bind_telegram(
            &self,
            _: &str,
            _: i64,
            _: i64,
        ) -> RepositoryResult<BindTelegramOutcome> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .bind
                .unwrap_or(BindTelegramOutcome::Bound))
        }

        async fn unbind_telegram(&self, _: i64, _: i64) -> RepositoryResult<UnbindTelegramOutcome> {
            Ok(UnbindTelegramOutcome::Unbound)
        }

        async fn admin_recipients(&self, _: bool) -> RepositoryResult<Vec<i64>> {
            Ok(vec![10, 20])
        }
    }

    impl TelegramExternal for Ports {
        async fn claim_update(&self, _: &str, _: i64) -> Result<bool, TelegramExternalError> {
            self.0.lock().unwrap().claimed += 1;
            Ok(true)
        }

        async fn release_update(&self, _: &str, _: i64) {
            self.0.lock().unwrap().released += 1;
        }

        async fn resolve_subscribe_url(
            &self,
            _: &str,
        ) -> Result<Option<String>, TelegramExternalError> {
            Ok(Some("token".to_string()))
        }

        async fn bot_username(&self) -> Result<String, TelegramExternalError> {
            Ok("test_bot".to_string())
        }

        async fn send_message(
            &self,
            chat_id: i64,
            text: &str,
            parse_mode: TelegramParseMode,
        ) -> Result<(), TelegramExternalError> {
            self.0
                .lock()
                .unwrap()
                .sent
                .push((chat_id, text.to_string(), parse_mode));
            Ok(())
        }

        async fn decide_chat_join(
            &self,
            _: i64,
            _: i64,
            _: TelegramJoinDecision,
        ) -> Result<(), TelegramExternalError> {
            Ok(())
        }

        async fn reply_ticket(
            &self,
            _: i64,
            _: i64,
            _: &str,
            _: i64,
        ) -> Result<(), TelegramExternalError> {
            Ok(())
        }
    }

    fn service(ports: Ports) -> TelegramService<Ports, Ports> {
        TelegramService::new(
            ports.clone(),
            ports,
            TelegramPolicy {
                app_name: "Site".to_string(),
                app_url: "https://example.test".to_string(),
                notifications_enabled: true,
            },
        )
    }

    #[test]
    fn bind_and_admin_notification_are_application_owned() {
        let ports = Ports::default();
        let service = service(ports.clone());
        assert_eq!(block_on(service.bot_username()).unwrap(), "test_bot");
        block_on(service.process_webhook(
            "scope",
            TelegramUpdate {
                update_id: 1,
                message: Some(TelegramMessage {
                    chat_id: 7,
                    is_private: true,
                    text: Some("/bind https://example.test/sub?token=x".to_string()),
                    reply_to_text: None,
                }),
                chat_join_request: None,
            },
            10,
        ))
        .unwrap();
        block_on(service.notify_admins("notice", true)).unwrap();
        let state = ports.0.lock().unwrap();
        assert_eq!(state.claimed, 1);
        assert_eq!(state.released, 0);
        assert_eq!(state.sent[0].1, "绑定成功");
        assert_eq!(state.sent[1].0, 10);
        assert_eq!(state.sent[2].0, 20);
    }

    #[test]
    fn traffic_and_reply_parsers_preserve_the_frozen_text_contract() {
        assert_eq!(legacy_traffic_convert(1024), "1024 B");
        assert_eq!(legacy_traffic_convert(1025), "1 KB");
        assert_eq!(legacy_traffic_convert(2_147_483_648), "2 GB");
        assert_eq!(telegram_reply_ticket_id("📮工单提醒 #123\n主题"), Some(123));
        assert_eq!(
            telegram_reply_ticket_id("#`456` 的工单已回复成功"),
            Some(456)
        );
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}
