//! Administrative configuration and operator-messaging use cases.
//!
//! This module deliberately knows nothing about JSON, PostgreSQL, SMTP, or
//! Telegram's HTTP API. Transport adapters translate values at the boundary;
//! production adapters implement the atomic authority/outbox contracts below.

use std::collections::BTreeMap;

use crate::admin_user::{UserFilterClause, validate_user_filters};

const REDACTED_SECRET: &str = "********";
const BULK_MAIL_MAX_RECIPIENTS: usize = 50_000;
const SECRET_KEYS: &[&str] = &[
    "server_token",
    "email_password",
    "telegram_bot_token",
    "recaptcha_key",
];

pub type ConfigurationMap = BTreeMap<String, ConfigurationValue>;
pub type ConfigurationGroups = BTreeMap<String, ConfigurationMap>;

#[derive(Clone, Debug, PartialEq)]
pub enum ConfigurationValue {
    Null,
    Bool(bool),
    Integer(i64),
    Number(String),
    String(String),
    StringList(Vec<String>),
}

impl ConfigurationValue {
    fn string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationCode {
    ConfigRevisionConflict,
    ConfigValidationFailed,
    InvalidParameter,
    MailIdempotencyConflict,
    MailInvalid,
    MailSendFailed,
    MailSenderNotConfigured,
    TelegramRequestFailed,
    TelegramTokenInvalid,
    TelegramWebhookFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("validation failed for {field}: {message}")]
    Validation { field: String, message: String },
    #[error("configuration business error: {code:?}")]
    Business {
        code: ConfigurationCode,
        detail: Option<String>,
    },
    #[error("configuration internal error: {0}")]
    Internal(String),
}

impl ConfigurationError {
    pub fn business(code: ConfigurationCode) -> Self {
        Self::Business { code, detail: None }
    }

    pub fn business_detail(code: ConfigurationCode, detail: impl Into<String>) -> Self {
        Self::Business {
            code,
            detail: Some(detail.into()),
        }
    }

    fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct ConfigurationSnapshot {
    pub revision: i64,
    pub groups: ConfigurationGroups,
}

#[derive(Clone)]
pub struct ActiveConfiguration {
    pub revision: i64,
    pub values: ConfigurationMap,
    pub effective_admin_path: String,
}

pub struct MaterializedConfiguration<A> {
    pub values: ConfigurationMap,
    pub activation: A,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationPortError {
    #[error("operator configuration changed concurrently")]
    Conflict,
    #[error("configuration validation failed: {detail}")]
    Validation { detail: String, security: bool },
    #[error("validation failed for {field}: {message}")]
    FieldValidation { field: String, message: String },
    #[error("configuration adapter business error: {code:?}")]
    Business {
        code: ConfigurationCode,
        detail: Option<String>,
    },
    #[error("configuration adapter failed: {0}")]
    Internal(String),
}

impl From<ConfigurationPortError> for ConfigurationError {
    fn from(error: ConfigurationPortError) -> Self {
        match error {
            ConfigurationPortError::Conflict => {
                Self::business(ConfigurationCode::ConfigRevisionConflict)
            }
            ConfigurationPortError::Validation { detail, security } => Self::business_detail(
                ConfigurationCode::ConfigValidationFailed,
                format!(
                    "{}: {detail}",
                    if security {
                        "配置安全校验失败"
                    } else {
                        "配置校验失败"
                    }
                ),
            ),
            ConfigurationPortError::FieldValidation { field, message } => {
                Self::Validation { field, message }
            }
            ConfigurationPortError::Business { code, detail } => Self::Business { code, detail },
            ConfigurationPortError::Internal(detail) => Self::Internal(detail),
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait ConfigurationRepository: Send + Sync {
    type Activation: Send;

    fn current_snapshot(&self) -> Result<ConfigurationSnapshot, ConfigurationPortError>;
    async fn load_active(&self) -> Result<ActiveConfiguration, ConfigurationPortError>;
    async fn materialize(
        &self,
        active: &ActiveConfiguration,
        changes: &ConfigurationMap,
    ) -> Result<MaterializedConfiguration<Self::Activation>, ConfigurationPortError>;
    async fn commit(
        &self,
        expected_revision: i64,
        values: &ConfigurationMap,
        actor: &str,
    ) -> Result<i64, ConfigurationPortError>;
    fn at_revision(&self, activation: Self::Activation, revision: i64) -> Self::Activation;
}

#[allow(async_fn_in_trait)]
pub trait ConfigurationExternal: Send + Sync {
    async fn send_test_mail(&self, recipient: &str) -> Result<(), ConfigurationPortError>;
    async fn set_telegram_webhook(&self, token: Option<&str>)
    -> Result<(), ConfigurationPortError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailAudience {
    Admin,
    Staff,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BulkMailInput {
    pub subject: String,
    pub content: String,
    pub filter: Option<Vec<UserFilterClause>>,
}

pub struct BulkMailCommand<'a> {
    pub audience: MailAudience,
    pub actor: &'a str,
    pub idempotency_key: &'a str,
    pub subject: &'a str,
    pub content: &'a str,
    pub filter: Option<&'a [UserFilterClause]>,
    pub maximum_recipients: usize,
}

#[allow(async_fn_in_trait)]
pub trait BulkMailRepository: Send + Sync {
    /// Atomically reserves the replay identity, snapshots a bounded audience,
    /// and enqueues the complete immutable envelope. Identical replays are a
    /// successful no-op; mismatched payloads return `MailIdempotencyConflict`.
    async fn enqueue(&self, command: BulkMailCommand<'_>) -> Result<(), ConfigurationPortError>;
}

#[derive(Debug)]
pub enum ConfigurationPatchOutcome<A> {
    Unchanged,
    Committed { activation: A, revision: i64 },
}

#[derive(Clone)]
pub struct ConfigurationService<R, E, M> {
    repository: R,
    external: E,
    bulk_mail: M,
}

impl<R, E, M> ConfigurationService<R, E, M>
where
    R: ConfigurationRepository,
    E: ConfigurationExternal,
    M: BulkMailRepository,
{
    pub const fn new(repository: R, external: E, bulk_mail: M) -> Self {
        Self {
            repository,
            external,
            bulk_mail,
        }
    }

    pub fn view(&self, group: Option<&str>) -> Result<ConfigurationSnapshot, ConfigurationError> {
        let mut snapshot = self.repository.current_snapshot()?;
        if snapshot.revision <= 0 {
            return Err(ConfigurationError::Internal(
                "operator configuration authority is not active".to_string(),
            ));
        }
        redact_groups(&mut snapshot.groups);
        if let Some(group) = group
            && snapshot.groups.contains_key(group)
        {
            snapshot.groups.retain(|name, _| name == group);
        }
        Ok(snapshot)
    }

    pub fn email_templates(&self) -> Vec<String> {
        vec!["default".to_string()]
    }

    pub async fn patch(
        &self,
        mut changes: ConfigurationMap,
        expected_revision: i64,
        admin_email: &str,
    ) -> Result<ConfigurationPatchOutcome<R::Activation>, ConfigurationError> {
        if expected_revision <= 0 {
            return Err(ConfigurationError::business_detail(
                ConfigurationCode::ConfigValidationFailed,
                "expected_revision must be a positive integer",
            ));
        }
        strip_redacted_secrets(&mut changes);
        let active = self.repository.load_active().await?;
        if active.revision != expected_revision {
            return Err(ConfigurationError::business(
                ConfigurationCode::ConfigRevisionConflict,
            ));
        }
        drop_unchanged_effective_secure_path(&mut changes, &active.effective_admin_path);
        validate_patch(&changes)?;
        let materialized = self.repository.materialize(&active, &changes).await?;
        if materialized.values == active.values {
            return Ok(ConfigurationPatchOutcome::Unchanged);
        }
        let actor = format!("admin:{}", admin_email.trim());
        let revision = self
            .repository
            .commit(expected_revision, &materialized.values, &actor)
            .await?;
        if revision <= 0 {
            return Err(ConfigurationError::Internal(
                "committed operator configuration revision is missing".to_string(),
            ));
        }
        Ok(ConfigurationPatchOutcome::Committed {
            activation: self
                .repository
                .at_revision(materialized.activation, revision),
            revision,
        })
    }

    pub async fn test_mail(&self, recipient: &str) -> Result<(), ConfigurationError> {
        self.external.send_test_mail(recipient).await?;
        Ok(())
    }

    pub async fn set_telegram_webhook(
        &self,
        token: Option<&str>,
    ) -> Result<(), ConfigurationError> {
        self.external.set_telegram_webhook(token).await?;
        Ok(())
    }

    pub async fn send_bulk_mail(
        &self,
        audience: MailAudience,
        input: &BulkMailInput,
        actor_email: &str,
        idempotency_key: &str,
    ) -> Result<(), ConfigurationError> {
        if input.subject.trim().is_empty() {
            return Err(ConfigurationError::validation(
                "subject",
                "邮件主题不能为空",
            ));
        }
        if input.content.trim().is_empty() {
            return Err(ConfigurationError::validation(
                "content",
                "邮件内容不能为空",
            ));
        }
        if let Some(filters) = &input.filter {
            validate_user_filters(filters).map_err(|violation| {
                ConfigurationError::validation(violation.field, violation.message)
            })?;
        }
        let actor = format!(
            "{}:{}",
            match audience {
                MailAudience::Admin => "admin",
                MailAudience::Staff => "staff",
            },
            actor_email
        );
        self.bulk_mail
            .enqueue(BulkMailCommand {
                audience,
                actor: &actor,
                idempotency_key,
                subject: &input.subject,
                content: &input.content,
                filter: input.filter.as_deref(),
                maximum_recipients: BULK_MAIL_MAX_RECIPIENTS,
            })
            .await?;
        Ok(())
    }
}

fn redact_groups(groups: &mut ConfigurationGroups) {
    for group in groups.values_mut() {
        for key in SECRET_KEYS {
            let Some(value) = group.get_mut(*key) else {
                continue;
            };
            if value.string().is_some_and(|value| !value.is_empty()) {
                *value = ConfigurationValue::String(REDACTED_SECRET.to_string());
            }
        }
    }
}

fn strip_redacted_secrets(changes: &mut ConfigurationMap) {
    changes.retain(|key, value| {
        !(SECRET_KEYS.contains(&key.as_str()) && value.string() == Some(REDACTED_SECRET))
    });
}

fn drop_unchanged_effective_secure_path(changes: &mut ConfigurationMap, effective: &str) {
    if changes
        .get("secure_path")
        .and_then(ConfigurationValue::string)
        .is_some_and(|path| path.trim_matches('/') == effective)
    {
        changes.remove("secure_path");
    }
}

const CONFIG_KEYS: &[&str] = &[
    "deposit_bounus",
    "ticket_status",
    "invite_force",
    "invite_commission",
    "invite_gen_limit",
    "invite_never_expire",
    "commission_first_time_enable",
    "commission_auto_check_enable",
    "commission_withdraw_limit",
    "commission_withdraw_method",
    "withdraw_close_enable",
    "commission_distribution_enable",
    "commission_distribution_l1",
    "commission_distribution_l2",
    "commission_distribution_l3",
    "logo",
    "force_https",
    "stop_register",
    "app_name",
    "app_description",
    "app_url",
    "legacy_hash_redirect_enable",
    "subscribe_url",
    "subscribe_path",
    "try_out_enable",
    "try_out_plan_id",
    "try_out_hour",
    "tos_url",
    "currency",
    "currency_symbol",
    "plan_change_enable",
    "reset_traffic_method",
    "surplus_enable",
    "allow_new_period",
    "new_order_event_id",
    "renew_order_event_id",
    "change_order_event_id",
    "show_info_to_server_enable",
    "show_subscribe_method",
    "show_subscribe_expire",
    "server_api_url",
    "server_token",
    "server_pull_interval",
    "server_push_interval",
    "device_limit_mode",
    "server_node_report_min_traffic",
    "server_device_online_min_traffic",
    "frontend_theme_color",
    "frontend_background_url",
    "email_template",
    "email_host",
    "email_port",
    "email_username",
    "email_password",
    "email_encryption",
    "email_from_address",
    "telegram_bot_enable",
    "telegram_bot_token",
    "telegram_discuss_id",
    "telegram_channel_id",
    "telegram_discuss_link",
    "windows_version",
    "windows_download_url",
    "macos_version",
    "macos_download_url",
    "android_version",
    "android_download_url",
    "email_whitelist_enable",
    "email_whitelist_suffix",
    "email_gmail_limit_enable",
    "recaptcha_enable",
    "recaptcha_key",
    "recaptcha_site_key",
    "email_verify",
    "safe_mode_enable",
    "admin_mfa_force",
    "register_limit_by_ip_enable",
    "register_limit_count",
    "register_limit_expire",
    "secure_path",
    "password_limit_enable",
    "password_limit_count",
    "password_limit_expire",
];

fn validate_patch(changes: &ConfigurationMap) -> Result<(), ConfigurationError> {
    const FLAGS: &[&str] = &[
        "invite_force",
        "invite_never_expire",
        "commission_first_time_enable",
        "commission_auto_check_enable",
        "withdraw_close_enable",
        "commission_distribution_enable",
        "force_https",
        "stop_register",
        "try_out_enable",
        "plan_change_enable",
        "surplus_enable",
        "allow_new_period",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "show_info_to_server_enable",
        "device_limit_mode",
        "telegram_bot_enable",
        "email_whitelist_enable",
        "email_gmail_limit_enable",
        "recaptcha_enable",
        "email_verify",
        "legacy_hash_redirect_enable",
        "safe_mode_enable",
        "admin_mfa_force",
        "register_limit_by_ip_enable",
        "password_limit_enable",
    ];
    const STRING_ARRAYS: &[&str] = &[
        "deposit_bounus",
        "commission_withdraw_method",
        "email_whitelist_suffix",
    ];
    const INTEGERS: &[&str] = &[
        "invite_commission",
        "invite_gen_limit",
        "try_out_plan_id",
        "show_subscribe_expire",
        "server_pull_interval",
        "server_push_interval",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "register_limit_count",
        "register_limit_expire",
        "password_limit_count",
        "password_limit_expire",
    ];
    const NUMBERS: &[&str] = &[
        "try_out_hour",
        "commission_distribution_l1",
        "commission_distribution_l2",
        "commission_distribution_l3",
    ];

    for (key, value) in changes {
        if !CONFIG_KEYS.contains(&key.as_str()) {
            return Err(ConfigurationError::validation(key, "不支持的配置项"));
        }
        if key == "secure_path" {
            let Some(path) = value.string().map(str::trim) else {
                return Err(ConfigurationError::validation(key, "后台路径不能为空"));
            };
            if path.is_empty() {
                return Err(ConfigurationError::validation(key, "后台路径不能为空"));
            }
            if path.chars().count() < 8 {
                return Err(ConfigurationError::validation(key, "后台路径长度最小为8位"));
            }
            if !path.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            }) {
                return Err(ConfigurationError::validation(
                    key,
                    "后台路径只能为字母或数字",
                ));
            }
            continue;
        }
        if matches!(value, ConfigurationValue::Null) {
            continue;
        }
        if FLAGS.contains(&key.as_str()) {
            if !matches!(value, ConfigurationValue::Bool(_)) {
                return Err(ConfigurationError::validation(key, "参数格式有误"));
            }
            continue;
        }
        if STRING_ARRAYS.contains(&key.as_str()) {
            let ConfigurationValue::StringList(items) = value else {
                return Err(ConfigurationError::validation(key, "数组参数格式有误"));
            };
            if key == "deposit_bounus"
                && items
                    .iter()
                    .map(|item| item.trim())
                    .any(|item| !item.is_empty() && !is_deposit_bonus_tier(item))
            {
                return Err(ConfigurationError::validation(
                    key,
                    "充值奖励格式不正确，必须为充值金额:奖励金额",
                ));
            }
            continue;
        }
        if INTEGERS.contains(&key.as_str()) {
            let ConfigurationValue::Integer(parsed) = value else {
                return Err(ConfigurationError::validation(key, "参数格式有误"));
            };
            let (minimum, maximum) = match key.as_str() {
                "show_subscribe_expire" | "register_limit_expire" | "password_limit_expire" => {
                    (1, 525_600)
                }
                "server_pull_interval" | "server_push_interval" => (1, i64::from(i32::MAX)),
                "register_limit_count" | "password_limit_count" => (1, i64::MAX),
                "invite_commission"
                | "try_out_plan_id"
                | "server_node_report_min_traffic"
                | "server_device_online_min_traffic" => (0, i64::from(i32::MAX)),
                "invite_gen_limit" => (0, i64::MAX),
                _ => (i64::MIN, i64::MAX),
            };
            if !(*parsed >= minimum && *parsed <= maximum) {
                return Err(ConfigurationError::validation(key, "参数超出支持范围"));
            }
            continue;
        }
        if NUMBERS.contains(&key.as_str()) {
            let ConfigurationValue::Number(raw) = value else {
                return Err(ConfigurationError::validation(key, "参数格式有误"));
            };
            if raw.starts_with('-') {
                return Err(ConfigurationError::validation(key, "参数不能为负数"));
            }
            let maximum = if key == "try_out_hour" {
                "2562047788015215.5019444444"
            } else {
                "79228162514264337593543950335"
            };
            if !is_unsigned_decimal(raw) || !decimal_leq(raw, maximum) {
                return Err(ConfigurationError::validation(key, "参数超出支持范围"));
            }
            continue;
        }
        match key.as_str() {
            "ticket_status" | "show_subscribe_method" => {
                if !matches!(value, ConfigurationValue::Integer(parsed) if (0..=2).contains(parsed))
                {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                }
            }
            "reset_traffic_method" => {
                if !matches!(value, ConfigurationValue::Integer(parsed) if (0..=4).contains(parsed))
                {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                }
            }
            "email_port" => {
                if !matches!(value, ConfigurationValue::Integer(port) if (1..=i64::from(u16::MAX)).contains(port))
                {
                    return Err(ConfigurationError::validation(
                        key,
                        "端口必须在1到65535之间",
                    ));
                }
            }
            "commission_withdraw_limit" => {
                let Some(raw) = value.string().map(str::trim) else {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                };
                if !raw.is_empty() && !is_unsigned_decimal(raw) {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                }
                if !raw.is_empty() && !decimal_leq(raw, "92233720368547758.07") {
                    return Err(ConfigurationError::validation(key, "参数超出支持范围"));
                }
            }
            "frontend_theme_color" => {
                let Some(color) = value.string().map(str::trim) else {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                };
                if !color.is_empty() && !matches!(color, "default" | "darkblue" | "black" | "green")
                {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                }
            }
            "logo"
            | "app_url"
            | "tos_url"
            | "telegram_discuss_link"
            | "frontend_background_url" => {
                let Some(url) = value.string().map(str::trim) else {
                    return Err(ConfigurationError::validation(key, "参数格式有误"));
                };
                if !url.is_empty() && !is_valid_url(url) {
                    let message = match key.as_str() {
                        "logo" => "LOGO URL格式不正确，必须携带https(s)://",
                        "app_url" => "站点URL格式不正确，必须携带http(s)://",
                        "tos_url" => "服务条款URL格式不正确，必须携带http(s)://",
                        "telegram_discuss_link" => {
                            "Telegram群组地址必须为URL格式，必须携带http(s)://"
                        }
                        _ => "参数格式有误",
                    };
                    return Err(ConfigurationError::validation(key, message));
                }
            }
            "subscribe_path" => {
                let Some(path) = value.string().map(str::trim) else {
                    return Err(ConfigurationError::validation(key, "订阅路径必须以/开头"));
                };
                if !path.is_empty() && !path.starts_with('/') {
                    return Err(ConfigurationError::validation(key, "订阅路径必须以/开头"));
                }
            }
            "server_token" => {
                let Some(token) = value.string().map(str::trim) else {
                    return Err(ConfigurationError::validation(
                        key,
                        "通讯密钥长度必须大于16位",
                    ));
                };
                if !token.is_empty() && token.chars().count() < 16 {
                    return Err(ConfigurationError::validation(
                        key,
                        "通讯密钥长度必须大于16位",
                    ));
                }
            }
            _ if value.string().is_none() => {
                return Err(ConfigurationError::validation(key, "参数格式有误"));
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_deposit_bonus_tier(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(amount, bonus)| is_unsigned_decimal(amount) && is_unsigned_decimal(bonus))
}

fn is_unsigned_decimal(value: &str) -> bool {
    let (whole, fractional) = value
        .split_once('.')
        .map_or((value, None), |(whole, fractional)| {
            (whole, Some(fractional))
        });
    !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fractional
            .is_none_or(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn decimal_leq(value: &str, maximum: &str) -> bool {
    let (whole, fractional) = decimal_parts(value);
    let (maximum_whole, maximum_fractional) = decimal_parts(maximum);
    match whole.len().cmp(&maximum_whole.len()) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => match whole.cmp(maximum_whole) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => {
                let width = fractional.len().max(maximum_fractional.len());
                let mut left = fractional.to_string();
                let mut right = maximum_fractional.to_string();
                left.extend(std::iter::repeat_n('0', width - left.len()));
                right.extend(std::iter::repeat_n('0', width - right.len()));
                left <= right
            }
        },
    }
}

fn decimal_parts(value: &str) -> (&str, &str) {
    let (whole, fractional) = value
        .split_once('.')
        .map_or((value, ""), |(whole, fractional)| (whole, fractional));
    (
        whole.trim_start_matches('0'),
        fractional.trim_end_matches('0'),
    )
}

fn is_valid_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    let scheme_bytes = scheme.as_bytes();
    if scheme_bytes.is_empty()
        || !scheme_bytes[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty() && !host.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests;
