use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::{DateTime, FixedOffset, Utc};
use ipnet::IpNet;
use percent_encoding::percent_decode_str;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

/// Installation-bound Redis key namespace shared by the API and worker.
///
/// Redis is deliberately disposable, but sharing one Redis service or logical
/// database must never let two native installations read, delete, or lock each
/// other's state. The immutable PostgreSQL installation identity supplies that
/// boundary without adding an operator-controlled alias or compatibility key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedisKeyspace {
    prefix: String,
}

impl RedisKeyspace {
    pub fn new(installation_id: Uuid) -> Self {
        Self {
            prefix: format!("v2board:{installation_id}:"),
        }
    }

    pub fn key(&self, logical_key: &str) -> String {
        format!("{}{logical_key}", self.prefix)
    }

    pub fn pattern(&self, logical_pattern: &str) -> String {
        self.key(logical_pattern)
    }
}

/// Operator-entered minute durations are bounded to one year. This is long
/// enough for every subscription/rate-limit use while keeping Redis expiry and
/// timestamp arithmetic inside a small, predictable range.
pub const MAX_CONFIG_DURATION_MINUTES: i64 = 365 * 24 * 60;
const DEFAULT_PRIVILEGED_SESSION_TTL_SECONDS: i64 = 30 * 60;
const CONFIGURATION_SOURCE_KEY: &str = "configuration_source";
const FILE_ONLY_CONFIGURATION_SOURCE: &str = "file_only";
const CONFIGURATION_SCOPE_KEY: &str = "configuration_scope";
const BOOT_ONLY_CONFIGURATION_SCOPE: &str = "boot_only";
const OPERATOR_AUTHORITY_MARKER: &str = "_v2board_operator_authority_v1";
const MAX_CONFIG_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Exact common key set for a long-lived role-owned bootstrap document.  The
/// worker validator additionally requires its four ClickHouse writer keys.
/// Dynamic behavior and integration secrets deliberately live only in the
/// PostgreSQL operator authority and therefore cannot leak back into these
/// files after initial materialization.
pub const BOOT_ONLY_RUNTIME_KEYS_V1: &[&str] = &[
    "configuration_source",
    "configuration_scope",
    "runtime_role",
    "environment",
    "bind_addr",
    "cors_allowed_origins",
    "trusted_proxy_cidrs",
    "http_connect_timeout_seconds",
    "http_request_timeout_seconds",
    "api_request_timeout_seconds",
    "password_kdf_max_parallel",
    "app_key",
    "database_url",
    "peer_database_principal",
    "redis_url",
];

/// Every application-behavior key required by the version-1 file-only runtime
/// document. Database and Redis URLs are supplied by the target configuration and
/// are required in the materialized file in addition to this list.
pub const FILE_ONLY_RUNTIME_KEYS_V1: &[&str] = &[
    "configuration_source",
    "environment",
    "bind_addr",
    "cors_allowed_origins",
    "trusted_proxy_cidrs",
    "http_connect_timeout_seconds",
    "http_request_timeout_seconds",
    "api_request_timeout_seconds",
    "password_kdf_max_parallel",
    "auth_session_ttl_seconds",
    "privileged_auth_session_ttl_seconds",
    "auth_session_max_per_user",
    "privileged_step_up_enable",
    "privileged_step_up_ttl_seconds",
    "privileged_step_up_max_attempts",
    "privileged_step_up_attempt_window_seconds",
    "app_key",
    "app_name",
    "app_url",
    "app_description",
    "logo",
    "tos_url",
    "force_https",
    "email_verify",
    "email_template",
    "email_host",
    "email_port",
    "email_username",
    "email_password",
    "email_encryption",
    "email_from_address",
    "stop_register",
    "invite_force",
    "invite_never_expire",
    "email_whitelist_enable",
    "email_whitelist_suffix",
    "email_gmail_limit_enable",
    "recaptcha_enable",
    "recaptcha_site_key",
    "recaptcha_key",
    "register_limit_by_ip_enable",
    "register_limit_count",
    "register_limit_expire",
    "telegram_bot_enable",
    "telegram_bot_token",
    "telegram_discuss_id",
    "telegram_channel_id",
    "telegram_discuss_link",
    "commission_withdraw_method",
    "withdraw_close_enable",
    "currency",
    "currency_symbol",
    "commission_distribution_enable",
    "commission_auto_check_enable",
    "commission_distribution_l1",
    "commission_distribution_l2",
    "commission_distribution_l3",
    "subscribe_url",
    "subscribe_path",
    "show_subscribe_method",
    "show_subscribe_expire",
    "show_info_to_server_enable",
    "allow_new_period",
    "reset_traffic_method",
    "try_out_enable",
    "try_out_plan_id",
    "try_out_hour",
    "plan_change_enable",
    "surplus_enable",
    "invite_commission",
    "commission_first_time_enable",
    "new_order_event_id",
    "renew_order_event_id",
    "change_order_event_id",
    "deposit_bounus",
    "invite_gen_limit",
    "ticket_status",
    "commission_withdraw_limit",
    "server_token",
    "server_require_idempotency_key",
    "server_api_url",
    "server_push_interval",
    "server_pull_interval",
    "server_node_report_min_traffic",
    "server_device_online_min_traffic",
    "device_limit_mode",
    "server_log_enable",
    "server_v2ray_domain",
    "server_v2ray_protocol",
    "frontend_theme_color",
    "frontend_background_url",
    "frontend_admin_path",
    "secure_path",
    "safe_mode_enable",
    "password_limit_enable",
    "password_limit_count",
    "password_limit_expire",
    "windows_version",
    "windows_download_url",
    "macos_version",
    "macos_download_url",
    "android_version",
    "android_download_url",
];

/// Reserved public top-level path segments (docs/api-dialect.md §10.2). The
/// resolved admin path serves the admin HTML subtree ahead of the user-SPA
/// fallback and builds the dynamic admin API prefix, so it may not shadow a
/// fixed public route root, a user-SPA route root, or a reserved API
/// namespace. The `crc32b_hex` fallback is eight hex characters and can never
/// equal an entry here.
const RESERVED_ADMIN_PATH_SEGMENTS: &[&str] = &[
    // Fixed public routes.
    "api",
    "assets",
    "healthz",
    "readyz",
    // User-SPA route roots.
    "login",
    "register",
    "forgetpassword",
    "dashboard",
    "plan",
    "order",
    "profile",
    "node",
    "traffic",
    "invite",
    "ticket",
    "knowledge",
    // API namespaces.
    "auth",
    "user",
    "public",
    "staff",
    "client",
    "server",
    "guest",
    "passport",
];

/// Application behavior owned by the versioned operator-configuration
/// authority.  Bootstrap-only credentials and process topology (database,
/// Redis, ClickHouse, listener, paths, environment, and `app_key`) are
/// deliberately absent: changing those values requires a coordinated restart.
///
/// Keep this list in lockstep with [`AppConfig::operator_config_map`].  A
/// snapshot read from PostgreSQL is rejected if it contains a key outside this
/// set, preventing a database row from becoming an undocumented override for a
/// boot-bound setting.
pub const OPERATOR_CONFIG_KEYS_V1: &[&str] = &[
    "auth_session_ttl_seconds",
    "privileged_auth_session_ttl_seconds",
    "auth_session_max_per_user",
    "privileged_step_up_enable",
    "privileged_step_up_ttl_seconds",
    "privileged_step_up_max_attempts",
    "privileged_step_up_attempt_window_seconds",
    "app_name",
    "app_url",
    "app_description",
    "logo",
    "tos_url",
    "force_https",
    "email_verify",
    "email_template",
    "email_host",
    "email_port",
    "email_username",
    "email_password",
    "email_encryption",
    "email_from_address",
    "stop_register",
    "invite_force",
    "invite_never_expire",
    "email_whitelist_enable",
    "email_whitelist_suffix",
    "email_gmail_limit_enable",
    "recaptcha_enable",
    "recaptcha_site_key",
    "recaptcha_key",
    "register_limit_by_ip_enable",
    "register_limit_count",
    "register_limit_expire",
    "telegram_bot_enable",
    "telegram_bot_token",
    "telegram_discuss_id",
    "telegram_channel_id",
    "telegram_discuss_link",
    "commission_withdraw_method",
    "withdraw_close_enable",
    "currency",
    "currency_symbol",
    "commission_distribution_enable",
    "commission_auto_check_enable",
    "commission_distribution_l1",
    "commission_distribution_l2",
    "commission_distribution_l3",
    "subscribe_url",
    "subscribe_path",
    "show_subscribe_method",
    "show_subscribe_expire",
    "show_info_to_server_enable",
    "allow_new_period",
    "reset_traffic_method",
    "try_out_enable",
    "try_out_plan_id",
    "try_out_hour",
    "plan_change_enable",
    "surplus_enable",
    "invite_commission",
    "commission_first_time_enable",
    "new_order_event_id",
    "renew_order_event_id",
    "change_order_event_id",
    "deposit_bounus",
    "invite_gen_limit",
    "ticket_status",
    "commission_withdraw_limit",
    "server_token",
    "server_require_idempotency_key",
    "server_api_url",
    "server_push_interval",
    "server_pull_interval",
    "server_node_report_min_traffic",
    "server_device_online_min_traffic",
    "device_limit_mode",
    "server_log_enable",
    "server_v2ray_domain",
    "server_v2ray_protocol",
    "frontend_theme_color",
    "frontend_background_url",
    "chat_widget_provider",
    "chat_widget_crisp_website_id",
    "chat_widget_tawk_property_id",
    "chat_widget_tawk_widget_id",
    "frontend_admin_path",
    "secure_path",
    "legacy_hash_redirect_enable",
    "safe_mode_enable",
    "password_limit_enable",
    "password_limit_count",
    "password_limit_expire",
    "windows_version",
    "windows_download_url",
    "macos_version",
    "macos_download_url",
    "android_version",
    "android_download_url",
];

/// Convert a validated minute setting to seconds. The clamp is a final defense
/// for manually constructed `AppConfig` values in embedders/tests; normal file
/// and environment loading rejects out-of-range values instead.
pub fn duration_minutes_to_seconds(minutes: i64) -> u64 {
    minutes.clamp(1, MAX_CONFIG_DURATION_MINUTES) as u64 * 60
}

static CONFIG_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Calendar-boundary computations (traffic reset day, statistics day, renewal
/// expiry day) are evaluated in Asia/Shanghai (UTC+8, no DST), independent of
/// the host/system timezone.
pub fn app_timezone() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("Asia/Shanghai is a valid fixed offset")
}

/// Current time in the pinned application timezone (`Asia/Shanghai`).
pub fn app_now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&app_timezone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    pub config: PathBuf,
    pub frontend: PathBuf,
    pub rules: PathBuf,
}

/// Deployment mode is parsed once and shared by every security-sensitive
/// default. Keeping this typed prevents `prod` and `production` from being
/// interpreted differently by configuration and runtime code.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeEnvironment {
    #[default]
    Local,
    Development,
    Testing,
    Staging,
    Production,
}

/// Selects the only datastore credential set a long-lived process may load.
/// The MySQL import manifest is shared operator input, but its materialized
/// runtime documents are deliberately role-specific.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRole {
    Api,
    Worker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigParseMode {
    FullRuntime,
    BootOnly,
    OperatorAuthority,
}

impl ConfigParseMode {
    const fn is_operator_authority(self) -> bool {
        matches!(self, Self::OperatorAuthority)
    }
}

impl RuntimeRole {
    const fn file_value(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Worker => "worker",
        }
    }

    const fn default_config_relative_path(self) -> &'static str {
        match self {
            Self::Api => "api/config.json",
            Self::Worker => "worker/config.json",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ClickHouseWriterConfig {
    pub url: String,
    pub database: String,
    pub username: String,
    pub password: Option<String>,
}

impl RuntimeEnvironment {
    pub fn parse(value: Option<&str>) -> Result<Self, &'static str> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(Self::Local),
            Some(value) if value.eq_ignore_ascii_case("local") => Ok(Self::Local),
            Some(value) if value.eq_ignore_ascii_case("dev") => Ok(Self::Development),
            Some(value) if value.eq_ignore_ascii_case("development") => Ok(Self::Development),
            Some(value) if value.eq_ignore_ascii_case("test") => Ok(Self::Testing),
            Some(value) if value.eq_ignore_ascii_case("testing") => Ok(Self::Testing),
            Some(value) if value.eq_ignore_ascii_case("stage") => Ok(Self::Staging),
            Some(value) if value.eq_ignore_ascii_case("staging") => Ok(Self::Staging),
            Some(value) if value.eq_ignore_ascii_case("prod") => Ok(Self::Production),
            Some(value) if value.eq_ignore_ascii_case("production") => Ok(Self::Production),
            Some(_) => {
                Err("V2BOARD_ENV must be local, development, testing, staging, or production")
            }
        }
    }

    pub const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

// Do not derive Debug: this snapshot contains database, Redis, SMTP, recaptcha,
// Telegram and server credentials. An accidental `?config` log must not expose
// the complete secret-bearing configuration.
#[derive(Clone)]
pub struct AppConfig {
    operator_revision: Option<i64>,
    pub runtime_role: RuntimeRole,
    pub environment: RuntimeEnvironment,
    pub bind_addr: String,
    pub database_url: String,
    pub peer_database_principal: String,
    pub clickhouse_writer: Option<ClickHouseWriterConfig>,
    pub redis_url: String,
    pub cors_allowed_origins: Vec<String>,
    pub trusted_proxy_cidrs: Vec<IpNet>,
    pub http_connect_timeout_seconds: u64,
    pub http_request_timeout_seconds: u64,
    pub api_request_timeout_seconds: u64,
    pub password_kdf_max_parallel: usize,
    pub auth_session_ttl_seconds: u64,
    pub privileged_auth_session_ttl_seconds: u64,
    pub auth_session_max_per_user: usize,
    pub privileged_step_up_enable: bool,
    pub privileged_step_up_ttl_seconds: u64,
    pub privileged_step_up_max_attempts: u64,
    pub privileged_step_up_attempt_window_seconds: u64,
    pub runtime_paths: RuntimePaths,
    pub app_key: String,
    pub app_name: String,
    pub app_url: Option<String>,
    pub app_description: Option<String>,
    pub logo: Option<String>,
    pub tos_url: Option<String>,
    pub force_https: bool,
    pub email_verify: bool,
    pub email_template: Option<String>,
    pub email_host: Option<String>,
    pub email_port: Option<i32>,
    pub email_username: Option<String>,
    pub email_password: Option<String>,
    pub email_encryption: Option<String>,
    pub email_from_address: Option<String>,
    pub stop_register: bool,
    pub invite_force: bool,
    pub invite_never_expire: bool,
    pub email_whitelist_enable: bool,
    pub email_whitelist_suffix: Vec<String>,
    pub email_gmail_limit_enable: bool,
    pub recaptcha_enable: bool,
    pub recaptcha_site_key: Option<String>,
    pub recaptcha_key: Option<String>,
    pub register_limit_by_ip_enable: bool,
    pub register_limit_count: i64,
    pub register_limit_expire: i64,
    pub telegram_bot_enable: bool,
    pub telegram_bot_token: Option<String>,
    pub telegram_discuss_id: Option<String>,
    pub telegram_channel_id: Option<String>,
    pub telegram_discuss_link: Option<String>,
    pub commission_withdraw_method: Vec<String>,
    pub withdraw_close_enable: bool,
    pub currency: String,
    pub currency_symbol: String,
    pub commission_distribution_enable: bool,
    pub commission_auto_check_enable: bool,
    pub commission_distribution_l1: Option<String>,
    pub commission_distribution_l2: Option<String>,
    pub commission_distribution_l3: Option<String>,
    pub subscribe_url: Option<String>,
    pub subscribe_path: String,
    pub show_subscribe_method: i32,
    pub show_subscribe_expire: i64,
    pub show_info_to_server_enable: bool,
    pub allow_new_period: i32,
    pub reset_traffic_method: i32,
    pub try_out_enable: bool,
    pub try_out_plan_id: i32,
    pub try_out_hour: Decimal,
    pub plan_change_enable: bool,
    pub surplus_enable: bool,
    pub invite_commission: i32,
    pub commission_first_time_enable: bool,
    pub new_order_event_id: i32,
    pub renew_order_event_id: i32,
    pub change_order_event_id: i32,
    pub deposit_bounus: Vec<String>,
    pub invite_gen_limit: i64,
    pub ticket_status: i32,
    pub commission_withdraw_limit: Decimal,
    pub server_token: Option<String>,
    pub server_require_idempotency_key: bool,
    pub server_api_url: Option<String>,
    pub server_push_interval: i32,
    pub server_pull_interval: i32,
    pub server_node_report_min_traffic: i32,
    pub server_device_online_min_traffic: i32,
    pub device_limit_mode: i32,
    pub server_log_enable: bool,
    pub server_v2ray_domain: Option<String>,
    pub server_v2ray_protocol: Option<String>,
    pub frontend_theme_color: Option<String>,
    pub frontend_background_url: Option<String>,
    /// First-class chat-widget integration (docs/api-dialect.md §10.6):
    /// `crisp` or `tawk`. Replaces the removed operator `custom_html`
    /// injection path — the user SPA loads the provider SDK from typed
    /// runtime config, and CSP gains the provider hosts only when a
    /// provider is configured.
    pub chat_widget_provider: Option<String>,
    pub chat_widget_crisp_website_id: Option<String>,
    pub chat_widget_tawk_property_id: Option<String>,
    pub chat_widget_tawk_widget_id: Option<String>,
    pub frontend_admin_path: Option<String>,
    pub secure_path: Option<String>,
    /// docs/api-dialect.md §10.3: client-side `#/…` → history-URL translation
    /// toggle, injected into the frontend runtime config. Default ON.
    pub legacy_hash_redirect_enable: bool,
    pub safe_mode_enable: bool,
    pub password_limit_enable: bool,
    pub password_limit_count: i64,
    pub password_limit_expire: i64,
    pub windows_version: Option<String>,
    pub windows_download_url: Option<String>,
    pub macos_version: Option<String>,
    pub macos_download_url: Option<String>,
    pub android_version: Option<String>,
    pub android_download_url: Option<String>,
}

impl AppConfig {
    pub fn from_api_env() -> Self {
        Self::try_from_api_env()
            .unwrap_or_else(|error| panic!("failed to load API config: {error}"))
    }

    pub fn try_from_api_env() -> io::Result<Self> {
        Self::try_from_env_for_role(RuntimeRole::Api, ConfigParseMode::FullRuntime)
    }

    pub fn try_from_worker_env() -> io::Result<Self> {
        Self::try_from_env_for_role(RuntimeRole::Worker, ConfigParseMode::FullRuntime)
    }

    /// Parses the role-owned bootstrap document used by a long-lived API
    /// process. The returned snapshot is intentionally incomplete and may only
    /// be used to connect to PostgreSQL and load a full operator revision.
    pub fn try_from_api_boot_env() -> io::Result<Self> {
        Self::try_from_env_for_role(RuntimeRole::Api, ConfigParseMode::BootOnly)
    }

    /// Worker equivalent of [`Self::try_from_api_boot_env`].
    pub fn try_from_worker_boot_env() -> io::Result<Self> {
        Self::try_from_env_for_role(RuntimeRole::Worker, ConfigParseMode::BootOnly)
    }

    /// Loads a complete configuration snapshot without panicking. Long-lived
    /// processes use this path for hot reloads so a malformed external edit can
    /// be rejected while the last-known-good snapshot remains active.
    fn try_from_env_for_role(
        runtime_role: RuntimeRole,
        parse_mode: ConfigParseMode,
    ) -> io::Result<Self> {
        let env_path = env_path("V2BOARD_ENV_PATH");
        if let Some(path) = env_path.as_ref() {
            if !path.is_file() {
                return Err(invalid_setting(
                    "V2BOARD_ENV_PATH",
                    "must name an existing regular file",
                ));
            }
            dotenvy::from_path(path).map_err(|error| {
                invalid_setting("V2BOARD_ENV_PATH", &format!("could not be loaded: {error}"))
            })?;
        }
        validate_role_environment(runtime_role)?;
        let config = Self::try_from_runtime_paths(
            runtime_role,
            RuntimePaths::from_env(runtime_role),
            parse_mode,
        )?;
        if env_path.is_some() && config.environment.is_production() {
            return Err(invalid_setting(
                "V2BOARD_ENV_PATH",
                "dotenv files are forbidden in production; use the role-owned file-only JSON",
            ));
        }
        Ok(config)
    }

    /// Reloads the exact runtime config file used by this snapshot. Ordinary
    /// installs retain environment overrides; a generated file-only
    /// `configuration_source=file_only` snapshot ignores value overrides.
    /// Runtime paths cannot jump to a different file during reload.
    pub fn reload(&self) -> io::Result<Self> {
        let next = Self::try_from_runtime_paths(
            self.runtime_role,
            self.runtime_paths.clone(),
            ConfigParseMode::FullRuntime,
        )?;
        self.validate_reload_compatible(&next)?;
        Ok(next)
    }

    /// PostgreSQL revision that supplied the dynamic operator portion of this
    /// snapshot. `None` is only valid for the parsed bootstrap snapshot before
    /// the API initializes authority (or before the worker loads it).
    pub const fn operator_revision(&self) -> Option<i64> {
        self.operator_revision
    }

    /// Attaches the database revision to a snapshot that has already passed
    /// typed validation. Revision identifiers are generated by PostgreSQL and
    /// are therefore positive by construction.
    pub fn at_operator_revision(mut self, revision: i64) -> Self {
        debug_assert!(revision > 0);
        self.operator_revision = Some(revision);
        self
    }

    /// Returns a complete, typed operator snapshot. Optional values remain
    /// explicit JSON nulls so a committed clear cannot later fall through to an
    /// environment variable or a role-specific bootstrap default.
    pub fn operator_config_map(&self) -> Map<String, Value> {
        let mut values = json_object(json!({
            "auth_session_ttl_seconds": self.auth_session_ttl_seconds,
            "privileged_auth_session_ttl_seconds": self.privileged_auth_session_ttl_seconds,
            "auth_session_max_per_user": self.auth_session_max_per_user,
            "privileged_step_up_enable": self.privileged_step_up_enable,
            "privileged_step_up_ttl_seconds": self.privileged_step_up_ttl_seconds,
            "privileged_step_up_max_attempts": self.privileged_step_up_max_attempts,
            "privileged_step_up_attempt_window_seconds": self.privileged_step_up_attempt_window_seconds,
            "app_name": self.app_name,
            "app_url": self.app_url,
            "app_description": self.app_description,
            "logo": self.logo,
            "tos_url": self.tos_url,
            "force_https": self.force_https,
            "email_verify": self.email_verify,
            "email_template": self.email_template,
            "email_host": self.email_host,
            "email_port": self.email_port,
            "email_username": self.email_username,
            "email_password": self.email_password,
            "email_encryption": self.email_encryption,
            "email_from_address": self.email_from_address,
            "stop_register": self.stop_register,
            "invite_force": self.invite_force,
            "invite_never_expire": self.invite_never_expire,
            "email_whitelist_enable": self.email_whitelist_enable,
            "email_whitelist_suffix": self.email_whitelist_suffix,
            "email_gmail_limit_enable": self.email_gmail_limit_enable,
            "recaptcha_enable": self.recaptcha_enable,
            "recaptcha_site_key": self.recaptcha_site_key,
            "recaptcha_key": self.recaptcha_key,
            "register_limit_by_ip_enable": self.register_limit_by_ip_enable,
            "register_limit_count": self.register_limit_count,
            "register_limit_expire": self.register_limit_expire,
        }));
        values.extend(json_object(json!({
            "telegram_bot_enable": self.telegram_bot_enable,
            "telegram_bot_token": self.telegram_bot_token,
            "telegram_discuss_id": self.telegram_discuss_id,
            "telegram_channel_id": self.telegram_channel_id,
            "telegram_discuss_link": self.telegram_discuss_link,
            "commission_withdraw_method": self.commission_withdraw_method,
            "withdraw_close_enable": self.withdraw_close_enable,
            "currency": self.currency,
            "currency_symbol": self.currency_symbol,
            "commission_distribution_enable": self.commission_distribution_enable,
            "commission_auto_check_enable": self.commission_auto_check_enable,
            "commission_distribution_l1": self.commission_distribution_l1,
            "commission_distribution_l2": self.commission_distribution_l2,
            "commission_distribution_l3": self.commission_distribution_l3,
            "subscribe_url": self.subscribe_url,
            "subscribe_path": self.subscribe_path,
            "show_subscribe_method": self.show_subscribe_method,
            "show_subscribe_expire": self.show_subscribe_expire,
            "show_info_to_server_enable": self.show_info_to_server_enable,
            "allow_new_period": self.allow_new_period,
            "reset_traffic_method": self.reset_traffic_method,
            "try_out_enable": self.try_out_enable,
            "try_out_plan_id": self.try_out_plan_id,
            "try_out_hour": self.try_out_hour.normalize().to_string(),
            "plan_change_enable": self.plan_change_enable,
            "surplus_enable": self.surplus_enable,
            "invite_commission": self.invite_commission,
            "commission_first_time_enable": self.commission_first_time_enable,
            "new_order_event_id": self.new_order_event_id,
            "renew_order_event_id": self.renew_order_event_id,
            "change_order_event_id": self.change_order_event_id,
        })));
        values.extend(json_object(json!({
            "deposit_bounus": self.deposit_bounus,
            "invite_gen_limit": self.invite_gen_limit,
            "ticket_status": self.ticket_status,
            "commission_withdraw_limit": self.commission_withdraw_limit.normalize().to_string(),
            "server_token": self.server_token,
            "server_require_idempotency_key": self.server_require_idempotency_key,
            "server_api_url": self.server_api_url,
            "server_push_interval": self.server_push_interval,
            "server_pull_interval": self.server_pull_interval,
            "server_node_report_min_traffic": self.server_node_report_min_traffic,
            "server_device_online_min_traffic": self.server_device_online_min_traffic,
            "device_limit_mode": self.device_limit_mode,
            "server_log_enable": self.server_log_enable,
            "server_v2ray_domain": self.server_v2ray_domain,
            "server_v2ray_protocol": self.server_v2ray_protocol,
            "frontend_theme_color": self.frontend_theme_color,
            "frontend_background_url": self.frontend_background_url,
            "chat_widget_provider": self.chat_widget_provider,
            "chat_widget_crisp_website_id": self.chat_widget_crisp_website_id,
            "chat_widget_tawk_property_id": self.chat_widget_tawk_property_id,
            "chat_widget_tawk_widget_id": self.chat_widget_tawk_widget_id,
            "frontend_admin_path": self.frontend_admin_path,
            "secure_path": self.secure_path,
            "legacy_hash_redirect_enable": self.legacy_hash_redirect_enable,
            "safe_mode_enable": self.safe_mode_enable,
            "password_limit_enable": self.password_limit_enable,
            "password_limit_count": self.password_limit_count,
            "password_limit_expire": self.password_limit_expire,
            "windows_version": self.windows_version,
            "windows_download_url": self.windows_download_url,
            "macos_version": self.macos_version,
            "macos_download_url": self.macos_download_url,
            "android_version": self.android_version,
            "android_download_url": self.android_download_url,
        })));
        values
    }

    /// Applies a complete authoritative operator snapshot on top of the role's
    /// bootstrap file, then runs the same typed and cross-field validation as a
    /// cold start. The current active snapshot supplies the restart-bound
    /// comparison, so a malformed or boot-bound candidate never becomes live.
    pub fn with_operator_config(
        &self,
        operator: &Map<String, Value>,
        revision: i64,
    ) -> io::Result<Self> {
        if revision <= 0 {
            return Err(invalid_setting("operator revision", "must be positive"));
        }
        if self
            .operator_revision
            .is_some_and(|active_revision| revision < active_revision)
        {
            return Err(invalid_setting(
                "operator revision",
                "must not move backwards from the active revision",
            ));
        }
        for key in operator.keys() {
            if !OPERATOR_CONFIG_KEYS_V1.contains(&key.as_str()) {
                return Err(invalid_setting(
                    "operator configuration",
                    &format!("contains unsupported key {key}"),
                ));
            }
        }
        if let Some(missing) = OPERATOR_CONFIG_KEYS_V1
            .iter()
            .find(|key| !operator.contains_key(**key))
        {
            return Err(invalid_setting(
                "operator configuration",
                &format!("is missing required key {missing}"),
            ));
        }

        let mut merged = load_config(&self.runtime_paths.config).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to load {}: {error}",
                    self.runtime_paths.config.display()
                ),
            )
        })?;
        for (key, value) in operator {
            merged.insert(key.clone(), value.clone());
        }
        merged.insert(OPERATOR_AUTHORITY_MARKER.to_string(), Value::Bool(true));
        let mut next = Self::try_from_config_map_for_role(
            self.runtime_role,
            merged,
            self.runtime_paths.clone(),
            ConfigParseMode::OperatorAuthority,
        )?;
        next.operator_revision = Some(revision);
        self.validate_reload_compatible(&next)?;
        Ok(next)
    }

    /// A config parse succeeding does not make every field hot-reloadable.
    /// These values have already been captured by listeners, pools, reusable
    /// clients, or route construction. Accepting a changed snapshot would make
    /// the displayed config disagree with the resources actually serving
    /// requests, which is especially dangerous for datastore cutovers.
    fn validate_reload_compatible(&self, next: &Self) -> io::Result<()> {
        let mut restart_required = Vec::new();
        macro_rules! restart_bound {
            ($field:ident) => {
                if self.$field != next.$field {
                    restart_required.push(stringify!($field));
                }
            };
        }
        restart_bound!(runtime_role);
        restart_bound!(environment);
        restart_bound!(bind_addr);
        restart_bound!(database_url);
        restart_bound!(peer_database_principal);
        restart_bound!(clickhouse_writer);
        restart_bound!(redis_url);
        restart_bound!(http_connect_timeout_seconds);
        restart_bound!(http_request_timeout_seconds);
        restart_bound!(api_request_timeout_seconds);
        restart_bound!(password_kdf_max_parallel);
        restart_bound!(app_key);
        restart_bound!(runtime_paths);
        if restart_required.is_empty() {
            Ok(())
        } else {
            Err(invalid_setting(
                "runtime configuration reload",
                &format!(
                    "changed restart-required fields: {}",
                    restart_required.join(", ")
                ),
            ))
        }
    }

    fn try_from_runtime_paths(
        runtime_role: RuntimeRole,
        runtime_paths: RuntimePaths,
        parse_mode: ConfigParseMode,
    ) -> io::Result<Self> {
        let file_config = load_config(&runtime_paths.config).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to load {}: {error}", runtime_paths.config.display()),
            )
        })?;
        Self::try_from_config_map_for_role(runtime_role, file_config, runtime_paths, parse_mode)
    }

    /// Builds and validates a runtime snapshot from an already parsed JSON
    /// document. Lifecycle tooling uses this to prove that a complete,
    /// file-only document has the same semantics the API and worker will load,
    /// without staging or writing the eventual runtime file.
    pub fn try_from_api_config_map(
        file_config: Map<String, Value>,
        runtime_paths: RuntimePaths,
    ) -> io::Result<Self> {
        Self::try_from_config_map_for_role(
            RuntimeRole::Api,
            file_config,
            runtime_paths,
            ConfigParseMode::FullRuntime,
        )
    }

    pub fn try_from_worker_config_map(
        file_config: Map<String, Value>,
        runtime_paths: RuntimePaths,
    ) -> io::Result<Self> {
        Self::try_from_config_map_for_role(
            RuntimeRole::Worker,
            file_config,
            runtime_paths,
            ConfigParseMode::FullRuntime,
        )
    }

    pub fn try_from_api_boot_config_map(
        file_config: Map<String, Value>,
        runtime_paths: RuntimePaths,
    ) -> io::Result<Self> {
        Self::try_from_config_map_for_role(
            RuntimeRole::Api,
            file_config,
            runtime_paths,
            ConfigParseMode::BootOnly,
        )
    }

    pub fn try_from_worker_boot_config_map(
        file_config: Map<String, Value>,
        runtime_paths: RuntimePaths,
    ) -> io::Result<Self> {
        Self::try_from_config_map_for_role(
            RuntimeRole::Worker,
            file_config,
            runtime_paths,
            ConfigParseMode::BootOnly,
        )
    }

    fn try_from_config_map_for_role(
        runtime_role: RuntimeRole,
        file_config: Map<String, Value>,
        runtime_paths: RuntimePaths,
        parse_mode: ConfigParseMode,
    ) -> io::Result<Self> {
        if file_config.contains_key(OPERATOR_AUTHORITY_MARKER)
            && !parse_mode.is_operator_authority()
        {
            return Err(invalid_setting(
                OPERATOR_AUTHORITY_MARKER,
                "is reserved for the internal authority overlay",
            ));
        }
        validate_scalar_config(&file_config, runtime_role, parse_mode)?;
        let environment = RuntimeEnvironment::parse(
            config_or_env(&file_config, "environment", "V2BOARD_ENV").as_deref(),
        )
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        if parse_mode == ConfigParseMode::BootOnly
            && environment.is_production()
            && !configuration_is_boot_only(&file_config)
        {
            return Err(invalid_setting(
                CONFIGURATION_SCOPE_KEY,
                "production boot parsing requires a file_only boot_only document",
            ));
        }
        let app_key = resolve_app_key(
            environment,
            config_or_env(&file_config, "app_key", "APP_KEY"),
        )
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        let database_url = config_or_env(&file_config, "database_url", "DATABASE_URL")
            .unwrap_or_else(|| "postgresql://v2board:v2board@postgres:5432/v2board".to_string());
        let peer_database_principal = config_or_env(
            &file_config,
            "peer_database_principal",
            "V2BOARD_PEER_DATABASE_PRINCIPAL",
        )
        .unwrap_or_else(|| match runtime_role {
            RuntimeRole::Api => "v2board_worker".to_string(),
            RuntimeRole::Worker => "v2board_api".to_string(),
        });
        let clickhouse_writer =
            (runtime_role == RuntimeRole::Worker).then(|| ClickHouseWriterConfig {
                url: config_or_env(&file_config, "clickhouse_url", "V2BOARD_CLICKHOUSE_URL")
                    .unwrap_or_else(|| "http://clickhouse:8123".to_string()),
                database: config_or_env(
                    &file_config,
                    "clickhouse_database",
                    "V2BOARD_CLICKHOUSE_DATABASE",
                )
                .unwrap_or_else(|| "v2board_analytics".to_string()),
                username: config_or_env(
                    &file_config,
                    "clickhouse_writer_username",
                    "V2BOARD_CLICKHOUSE_WRITER_USERNAME",
                )
                .unwrap_or_else(|| "v2board_analytics_writer".to_string()),
                password: config_or_env(
                    &file_config,
                    "clickhouse_writer_password",
                    "V2BOARD_CLICKHOUSE_WRITER_PASSWORD",
                ),
            });
        let redis_url = config_or_env(&file_config, "redis_url", "REDIS_URL")
            .unwrap_or_else(|| "redis://redis:6379/1".to_string());
        validate_datastore_transport(
            environment,
            &database_url,
            &peer_database_principal,
            clickhouse_writer.as_ref(),
            &redis_url,
        )?;
        let server_token = config_or_env(&file_config, "server_token", "V2BOARD_SERVER_TOKEN");
        if parse_mode != ConfigParseMode::BootOnly {
            validate_production_secret(environment, "server_token", server_token.as_deref())?;
        }
        let bind_addr = config_or_env(&file_config, "bind_addr", "RUST_BIND_ADDR")
            .unwrap_or_else(|| "0.0.0.0:8080".to_string());
        if environment.is_production() && bind_addr != "127.0.0.1:8080" {
            return Err(invalid_setting(
                "bind_addr",
                "bare-metal production must bind 127.0.0.1:8080 behind same-host cloudflared",
            ));
        }

        let trusted_proxy_cidrs = parse_trusted_proxy_cidrs(&file_config)?;
        validate_production_proxy_topology(environment, &trusted_proxy_cidrs)?;

        let force_https = config_bool(
            &file_config,
            "force_https",
            "V2BOARD_FORCE_HTTPS",
            environment.is_production(),
        );
        let app_url = config_or_env(&file_config, "app_url", "APP_URL");
        let cors_allowed_origins = load_cors_allowed_origins(&file_config, app_url.as_deref())?;
        if parse_mode != ConfigParseMode::BootOnly {
            validate_https_configuration(
                environment,
                force_https,
                app_url.as_deref(),
                !trusted_proxy_cidrs.is_empty(),
            )?;
        }

        let auth_session_ttl_seconds = config_i64(
            &file_config,
            "auth_session_ttl_seconds",
            "V2BOARD_AUTH_SESSION_TTL_SECONDS",
            30 * 24 * 60 * 60,
        )
        .clamp(60 * 60, 365 * 24 * 60 * 60) as u64;
        // Keep the built-in admin client usable without weakening the mutation
        // gate: a password-authenticated privileged session is deliberately
        // short, and its initial recent-authentication window covers that same
        // lifetime. Clients that opt into a longer privileged session must use
        // the explicit step-up endpoint after the configured window.
        let privileged_default = DEFAULT_PRIVILEGED_SESSION_TTL_SECONDS;
        let privileged_auth_session_ttl_seconds = config_i64(
            &file_config,
            "privileged_auth_session_ttl_seconds",
            "V2BOARD_PRIVILEGED_AUTH_SESSION_TTL_SECONDS",
            privileged_default,
        )
        .clamp(15 * 60, 7 * 24 * 60 * 60) as u64;
        if privileged_auth_session_ttl_seconds >= auth_session_ttl_seconds {
            return Err(invalid_setting(
                "privileged_auth_session_ttl_seconds",
                "must be shorter than auth_session_ttl_seconds",
            ));
        }
        let privileged_step_up_ttl_seconds = config_i64(
            &file_config,
            "privileged_step_up_ttl_seconds",
            "V2BOARD_PRIVILEGED_STEP_UP_TTL_SECONDS",
            privileged_auth_session_ttl_seconds as i64,
        )
        .clamp(60, 60 * 60) as u64;
        if privileged_step_up_ttl_seconds > privileged_auth_session_ttl_seconds {
            return Err(invalid_setting(
                "privileged_step_up_ttl_seconds",
                "must not exceed privileged_auth_session_ttl_seconds",
            ));
        }
        let privileged_step_up_max_attempts = config_i64(
            &file_config,
            "privileged_step_up_max_attempts",
            "V2BOARD_PRIVILEGED_STEP_UP_MAX_ATTEMPTS",
            5,
        )
        .clamp(1, 20) as u64;
        let privileged_step_up_attempt_window_seconds = config_i64(
            &file_config,
            "privileged_step_up_attempt_window_seconds",
            "V2BOARD_PRIVILEGED_STEP_UP_ATTEMPT_WINDOW_SECONDS",
            15 * 60,
        )
        .clamp(60, 24 * 60 * 60) as u64;
        let server_require_idempotency_key = config_bool(
            &file_config,
            "server_require_idempotency_key",
            "V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY",
            environment.is_production(),
        );
        if parse_mode != ConfigParseMode::BootOnly {
            validate_node_report_contract(
                environment,
                configuration_is_file_only(&file_config),
                server_require_idempotency_key,
            )?;
        }

        let snapshot = Self {
            operator_revision: None,
            runtime_role,
            environment,
            bind_addr,
            database_url,
            peer_database_principal,
            clickhouse_writer,
            redis_url,
            cors_allowed_origins,
            trusted_proxy_cidrs,
            http_connect_timeout_seconds: config_i64(
                &file_config,
                "http_connect_timeout_seconds",
                "V2BOARD_HTTP_CONNECT_TIMEOUT_SECONDS",
                10,
            )
            .max(1) as u64,
            http_request_timeout_seconds: config_i64(
                &file_config,
                "http_request_timeout_seconds",
                "V2BOARD_HTTP_REQUEST_TIMEOUT_SECONDS",
                30,
            )
            .max(1) as u64,
            api_request_timeout_seconds: config_i64(
                &file_config,
                "api_request_timeout_seconds",
                "V2BOARD_API_REQUEST_TIMEOUT_SECONDS",
                45,
            )
            .max(1) as u64,
            password_kdf_max_parallel: config_i64(
                &file_config,
                "password_kdf_max_parallel",
                "V2BOARD_PASSWORD_KDF_MAX_PARALLEL",
                4,
            )
            .clamp(1, 64) as usize,
            auth_session_ttl_seconds,
            privileged_auth_session_ttl_seconds,
            auth_session_max_per_user: config_i64(
                &file_config,
                "auth_session_max_per_user",
                "V2BOARD_AUTH_SESSION_MAX_PER_USER",
                20,
            )
            .clamp(1, 100) as usize,
            privileged_step_up_enable: config_bool(
                &file_config,
                "privileged_step_up_enable",
                "V2BOARD_PRIVILEGED_STEP_UP_ENABLE",
                environment.is_production(),
            ),
            privileged_step_up_ttl_seconds,
            privileged_step_up_max_attempts,
            privileged_step_up_attempt_window_seconds,
            runtime_paths,
            app_key,
            app_name: config_or_env(&file_config, "app_name", "V2BOARD_APP_NAME")
                .unwrap_or_else(|| "V2Board".to_string()),
            app_url,
            app_description: config_or_env(
                &file_config,
                "app_description",
                "V2BOARD_APP_DESCRIPTION",
            )
            .or_else(|| Some("V2Board is best".to_string())),
            logo: config_or_env(&file_config, "logo", "V2BOARD_LOGO"),
            tos_url: config_or_env(&file_config, "tos_url", "V2BOARD_TOS_URL"),
            force_https,
            email_verify: config_bool(&file_config, "email_verify", "V2BOARD_EMAIL_VERIFY", false),
            email_template: config_or_env(&file_config, "email_template", "V2BOARD_EMAIL_TEMPLATE")
                .or_else(|| Some("default".to_string())),
            email_host: config_or_env(&file_config, "email_host", "V2BOARD_EMAIL_HOST"),
            email_port: config_or_env(&file_config, "email_port", "V2BOARD_EMAIL_PORT")
                .and_then(|value| value.parse::<i32>().ok()),
            email_username: config_or_env(&file_config, "email_username", "V2BOARD_EMAIL_USERNAME"),
            email_password: config_or_env(&file_config, "email_password", "V2BOARD_EMAIL_PASSWORD"),
            email_encryption: config_or_env(
                &file_config,
                "email_encryption",
                "V2BOARD_EMAIL_ENCRYPTION",
            ),
            email_from_address: config_or_env(
                &file_config,
                "email_from_address",
                "V2BOARD_EMAIL_FROM_ADDRESS",
            ),
            stop_register: config_bool(
                &file_config,
                "stop_register",
                "V2BOARD_STOP_REGISTER",
                false,
            ),
            invite_force: config_bool(&file_config, "invite_force", "V2BOARD_INVITE_FORCE", false),
            invite_never_expire: config_bool(
                &file_config,
                "invite_never_expire",
                "V2BOARD_INVITE_NEVER_EXPIRE",
                false,
            ),
            email_whitelist_enable: config_bool(
                &file_config,
                "email_whitelist_enable",
                "V2BOARD_EMAIL_WHITELIST_ENABLE",
                false,
            ),
            email_whitelist_suffix: config_list(
                &file_config,
                "email_whitelist_suffix",
                "V2BOARD_EMAIL_WHITELIST_SUFFIX",
                // Laravel `Dict::EMAIL_WHITELIST_SUFFIX_DEFAULT` (order-preserving).
                &[
                    "gmail.com",
                    "qq.com",
                    "163.com",
                    "yahoo.com",
                    "sina.com",
                    "126.com",
                    "outlook.com",
                    "yeah.net",
                    "foxmail.com",
                ],
            ),
            email_gmail_limit_enable: config_bool(
                &file_config,
                "email_gmail_limit_enable",
                "V2BOARD_EMAIL_GMAIL_LIMIT_ENABLE",
                false,
            ),
            recaptcha_enable: config_bool(
                &file_config,
                "recaptcha_enable",
                "V2BOARD_RECAPTCHA_ENABLE",
                false,
            ),
            recaptcha_site_key: config_or_env(
                &file_config,
                "recaptcha_site_key",
                "V2BOARD_RECAPTCHA_SITE_KEY",
            ),
            recaptcha_key: config_or_env(&file_config, "recaptcha_key", "V2BOARD_RECAPTCHA_KEY"),
            register_limit_by_ip_enable: config_bool(
                &file_config,
                "register_limit_by_ip_enable",
                "V2BOARD_REGISTER_LIMIT_BY_IP_ENABLE",
                register_ip_limit_default(environment),
            ),
            register_limit_count: config_i64(
                &file_config,
                "register_limit_count",
                "V2BOARD_REGISTER_LIMIT_COUNT",
                3,
            ),
            register_limit_expire: config_duration_minutes(
                &file_config,
                "register_limit_expire",
                "V2BOARD_REGISTER_LIMIT_EXPIRE",
                60,
            )?,
            telegram_bot_enable: config_bool(
                &file_config,
                "telegram_bot_enable",
                "V2BOARD_TELEGRAM_BOT_ENABLE",
                false,
            ),
            telegram_bot_token: config_or_env(
                &file_config,
                "telegram_bot_token",
                "V2BOARD_TELEGRAM_BOT_TOKEN",
            ),
            telegram_discuss_id: config_or_env(
                &file_config,
                "telegram_discuss_id",
                "V2BOARD_TELEGRAM_DISCUSS_ID",
            ),
            telegram_channel_id: config_or_env(
                &file_config,
                "telegram_channel_id",
                "V2BOARD_TELEGRAM_CHANNEL_ID",
            ),
            telegram_discuss_link: config_or_env(
                &file_config,
                "telegram_discuss_link",
                "V2BOARD_TELEGRAM_DISCUSS_LINK",
            ),
            commission_withdraw_method: config_list(
                &file_config,
                "commission_withdraw_method",
                "V2BOARD_COMMISSION_WITHDRAW_METHOD",
                &["支付宝", "USDT", "Paypal"],
            ),
            withdraw_close_enable: config_bool(
                &file_config,
                "withdraw_close_enable",
                "V2BOARD_WITHDRAW_CLOSE_ENABLE",
                false,
            ),
            currency: config_or_env(&file_config, "currency", "V2BOARD_CURRENCY")
                .unwrap_or_else(|| "CNY".to_string()),
            currency_symbol: config_or_env(
                &file_config,
                "currency_symbol",
                "V2BOARD_CURRENCY_SYMBOL",
            )
            .unwrap_or_else(|| "\u{00a5}".to_string()),
            commission_distribution_enable: config_bool(
                &file_config,
                "commission_distribution_enable",
                "V2BOARD_COMMISSION_DISTRIBUTION_ENABLE",
                false,
            ),
            commission_auto_check_enable: config_bool(
                &file_config,
                "commission_auto_check_enable",
                "V2BOARD_COMMISSION_AUTO_CHECK_ENABLE",
                true,
            ),
            commission_distribution_l1: config_or_env(
                &file_config,
                "commission_distribution_l1",
                "V2BOARD_COMMISSION_DISTRIBUTION_L1",
            ),
            commission_distribution_l2: config_or_env(
                &file_config,
                "commission_distribution_l2",
                "V2BOARD_COMMISSION_DISTRIBUTION_L2",
            ),
            commission_distribution_l3: config_or_env(
                &file_config,
                "commission_distribution_l3",
                "V2BOARD_COMMISSION_DISTRIBUTION_L3",
            ),
            subscribe_url: config_or_env(&file_config, "subscribe_url", "V2BOARD_SUBSCRIBE_URL"),
            subscribe_path: config_or_env(&file_config, "subscribe_path", "V2BOARD_SUBSCRIBE_PATH")
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| "/api/v1/client/subscribe".to_string()),
            show_subscribe_method: config_i32(
                &file_config,
                "show_subscribe_method",
                "V2BOARD_SHOW_SUBSCRIBE_METHOD",
                0,
            ),
            show_subscribe_expire: config_duration_minutes(
                &file_config,
                "show_subscribe_expire",
                "V2BOARD_SHOW_SUBSCRIBE_EXPIRE",
                5,
            )?,
            show_info_to_server_enable: config_bool(
                &file_config,
                "show_info_to_server_enable",
                "V2BOARD_SHOW_INFO_TO_SERVER_ENABLE",
                false,
            ),
            allow_new_period: config_i32(
                &file_config,
                "allow_new_period",
                "V2BOARD_ALLOW_NEW_PERIOD",
                0,
            ),
            reset_traffic_method: config_i32(
                &file_config,
                "reset_traffic_method",
                "V2BOARD_RESET_TRAFFIC_METHOD",
                0,
            ),
            try_out_enable: config_bool(
                &file_config,
                "try_out_enable",
                "V2BOARD_TRY_OUT_ENABLE",
                false,
            ),
            try_out_plan_id: config_i32(
                &file_config,
                "try_out_plan_id",
                "V2BOARD_TRY_OUT_PLAN_ID",
                0,
            ),
            try_out_hour: config_decimal(
                &file_config,
                "try_out_hour",
                "V2BOARD_TRY_OUT_HOUR",
                Decimal::ONE,
            )?,
            plan_change_enable: config_bool(
                &file_config,
                "plan_change_enable",
                "V2BOARD_PLAN_CHANGE_ENABLE",
                true,
            ),
            surplus_enable: config_bool(
                &file_config,
                "surplus_enable",
                "V2BOARD_SURPLUS_ENABLE",
                true,
            ),
            invite_commission: config_i32(
                &file_config,
                "invite_commission",
                "V2BOARD_INVITE_COMMISSION",
                10,
            ),
            commission_first_time_enable: config_bool(
                &file_config,
                "commission_first_time_enable",
                "V2BOARD_COMMISSION_FIRST_TIME_ENABLE",
                true,
            ),
            new_order_event_id: config_i32(
                &file_config,
                "new_order_event_id",
                "V2BOARD_NEW_ORDER_EVENT_ID",
                0,
            ),
            renew_order_event_id: config_i32(
                &file_config,
                "renew_order_event_id",
                "V2BOARD_RENEW_ORDER_EVENT_ID",
                0,
            ),
            change_order_event_id: config_i32(
                &file_config,
                "change_order_event_id",
                "V2BOARD_CHANGE_ORDER_EVENT_ID",
                0,
            ),
            deposit_bounus: config_list(
                &file_config,
                "deposit_bounus",
                "V2BOARD_DEPOSIT_BOUNUS",
                &[],
            ),
            invite_gen_limit: config_i64(
                &file_config,
                "invite_gen_limit",
                "V2BOARD_INVITE_GEN_LIMIT",
                5,
            ),
            ticket_status: config_i32(&file_config, "ticket_status", "V2BOARD_TICKET_STATUS", 0),
            commission_withdraw_limit: config_decimal(
                &file_config,
                "commission_withdraw_limit",
                "V2BOARD_COMMISSION_WITHDRAW_LIMIT",
                Decimal::from(100),
            )?,
            server_token,
            server_require_idempotency_key,
            server_api_url: config_or_env(&file_config, "server_api_url", "V2BOARD_SERVER_API_URL"),
            server_push_interval: config_i32(
                &file_config,
                "server_push_interval",
                "V2BOARD_SERVER_PUSH_INTERVAL",
                60,
            ),
            server_pull_interval: config_i32(
                &file_config,
                "server_pull_interval",
                "V2BOARD_SERVER_PULL_INTERVAL",
                60,
            ),
            server_node_report_min_traffic: config_i32(
                &file_config,
                "server_node_report_min_traffic",
                "V2BOARD_SERVER_NODE_REPORT_MIN_TRAFFIC",
                0,
            ),
            server_device_online_min_traffic: config_i32(
                &file_config,
                "server_device_online_min_traffic",
                "V2BOARD_SERVER_DEVICE_ONLINE_MIN_TRAFFIC",
                0,
            ),
            device_limit_mode: config_i32(
                &file_config,
                "device_limit_mode",
                "V2BOARD_DEVICE_LIMIT_MODE",
                0,
            ),
            server_log_enable: config_bool(
                &file_config,
                "server_log_enable",
                "V2BOARD_SERVER_LOG_ENABLE",
                false,
            ),
            server_v2ray_domain: config_or_env(
                &file_config,
                "server_v2ray_domain",
                "V2BOARD_SERVER_V2RAY_DOMAIN",
            ),
            server_v2ray_protocol: config_or_env(
                &file_config,
                "server_v2ray_protocol",
                "V2BOARD_SERVER_V2RAY_PROTOCOL",
            ),
            frontend_theme_color: config_or_env(
                &file_config,
                "frontend_theme_color",
                "V2BOARD_FRONTEND_THEME_COLOR",
            )
            .or_else(|| Some("default".to_string())),
            frontend_background_url: config_or_env(
                &file_config,
                "frontend_background_url",
                "V2BOARD_FRONTEND_BACKGROUND_URL",
            ),
            chat_widget_provider: config_or_env(
                &file_config,
                "chat_widget_provider",
                "V2BOARD_CHAT_WIDGET_PROVIDER",
            ),
            chat_widget_crisp_website_id: config_or_env(
                &file_config,
                "chat_widget_crisp_website_id",
                "V2BOARD_CHAT_WIDGET_CRISP_WEBSITE_ID",
            ),
            chat_widget_tawk_property_id: config_or_env(
                &file_config,
                "chat_widget_tawk_property_id",
                "V2BOARD_CHAT_WIDGET_TAWK_PROPERTY_ID",
            ),
            chat_widget_tawk_widget_id: config_or_env(
                &file_config,
                "chat_widget_tawk_widget_id",
                "V2BOARD_CHAT_WIDGET_TAWK_WIDGET_ID",
            ),
            frontend_admin_path: config_or_env(
                &file_config,
                "frontend_admin_path",
                "V2BOARD_FRONTEND_ADMIN_PATH",
            ),
            secure_path: config_or_env(&file_config, "secure_path", "V2BOARD_SECURE_PATH"),
            legacy_hash_redirect_enable: config_bool(
                &file_config,
                "legacy_hash_redirect_enable",
                "V2BOARD_LEGACY_HASH_REDIRECT_ENABLE",
                true,
            ),
            safe_mode_enable: config_bool(
                &file_config,
                "safe_mode_enable",
                "V2BOARD_SAFE_MODE_ENABLE",
                false,
            ),
            password_limit_enable: config_bool(
                &file_config,
                "password_limit_enable",
                "V2BOARD_PASSWORD_LIMIT_ENABLE",
                true,
            ),
            password_limit_count: config_i64(
                &file_config,
                "password_limit_count",
                "V2BOARD_PASSWORD_LIMIT_COUNT",
                5,
            ),
            password_limit_expire: config_duration_minutes(
                &file_config,
                "password_limit_expire",
                "V2BOARD_PASSWORD_LIMIT_EXPIRE",
                60,
            )?,
            windows_version: config_or_env(
                &file_config,
                "windows_version",
                "V2BOARD_WINDOWS_VERSION",
            ),
            windows_download_url: config_or_env(
                &file_config,
                "windows_download_url",
                "V2BOARD_WINDOWS_DOWNLOAD_URL",
            ),
            macos_version: config_or_env(&file_config, "macos_version", "V2BOARD_MACOS_VERSION"),
            macos_download_url: config_or_env(
                &file_config,
                "macos_download_url",
                "V2BOARD_MACOS_DOWNLOAD_URL",
            ),
            android_version: config_or_env(
                &file_config,
                "android_version",
                "V2BOARD_ANDROID_VERSION",
            ),
            android_download_url: config_or_env(
                &file_config,
                "android_download_url",
                "V2BOARD_ANDROID_DOWNLOAD_URL",
            ),
        };
        if parse_mode != ConfigParseMode::BootOnly {
            validate_operator_dependencies(&snapshot)?;
        }
        Ok(snapshot)
    }

    pub fn subscribe_url_for_token(&self, token: &str) -> String {
        let path = if self.subscribe_path.trim().is_empty() {
            "/api/v1/client/subscribe"
        } else {
            self.subscribe_path.as_str()
        };
        let path_with_token = format!("{path}?token={token}");
        // Helper::getSubscribeUrl distributed links across the comma-separated
        // mirror list with a fresh rand() per render (Helper.php:107-108). Keep
        // the multi-mirror distribution, but pick deterministically by hashing
        // the token so each token stays on one stable mirror across renders,
        // restarts, and replicas instead of flapping per request.
        let mirrors = self
            .subscribe_url
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        if !mirrors.is_empty() {
            let index = usize::try_from(fnv1a_64(token.as_bytes()) % mirrors.len() as u64)
                .unwrap_or_default();
            return format!(
                "{}{}",
                mirrors[index].trim_end_matches('/'),
                path_with_token
            );
        }

        if let Some(app_url) = self.app_url.as_deref().filter(|value| !value.is_empty()) {
            return format!("{}{}", app_url.trim_end_matches('/'), path_with_token);
        }

        path_with_token
    }

    /// Laravel `OrderService::getbounus` / `OrderController::getbounus`: the deposit reward for a
    /// cents amount. Tiers are `"amount:bonus"` yuan strings; the best-matching tier wins.
    pub fn deposit_bonus(&self, total_amount: i32) -> i32 {
        deposit_bonus_from_tiers(&self.deposit_bounus, total_amount)
    }

    pub fn admin_path(&self) -> String {
        self.secure_path
            .as_deref()
            .or(self.frontend_admin_path.as_deref())
            .map(str::trim)
            .map(|path| path.trim_matches('/'))
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| crc32b_hex(self.app_key.as_bytes()))
    }

    /// Validates the security-sensitive cross-field portion of an admin config
    /// update before the file is persisted. Runtime reload is therefore not the
    /// first point at which a bad production secret or HTTPS combination is
    /// discovered.
    pub fn validate_security_update(
        &self,
        server_token: Option<&str>,
        force_https: bool,
        app_url: Option<&str>,
    ) -> io::Result<()> {
        validate_production_secret(self.environment, "server_token", server_token)?;
        validate_https_configuration(
            self.environment,
            force_https,
            app_url,
            !self.trusted_proxy_cidrs.is_empty(),
        )
    }
}

impl RuntimePaths {
    fn from_env(runtime_role: RuntimeRole) -> Self {
        let root =
            env_path("V2BOARD_RUNTIME_ROOT").unwrap_or_else(|| PathBuf::from("/var/lib/v2board"));
        let frontend = env_path("V2BOARD_FRONTEND_DIR")
            .unwrap_or_else(|| PathBuf::from("/opt/v2board/current/frontend"));

        Self {
            config: env_path("V2BOARD_CONFIG_PATH")
                .unwrap_or_else(|| root.join(runtime_role.default_config_relative_path())),
            frontend,
            rules: env_path("V2BOARD_RULE_DIR").unwrap_or_else(|| root.join("rules")),
        }
    }
}

const fn register_ip_limit_default(environment: RuntimeEnvironment) -> bool {
    environment.is_production()
}

fn env_opt(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

/// Reads a one-shot secret from exactly one of a direct environment value, an
/// explicit absolute file, or a systemd credential. Long-lived runtime JSON
/// uses its own role loader; this helper is only for transient administrative
/// commands whose credentials must not be retained in a unit Environment.
pub fn one_shot_secret(
    value_environment: &str,
    file_environment: &str,
    systemd_credential_name: &str,
) -> io::Result<Option<String>> {
    let direct = env_opt(value_environment);
    let explicit_path = env::var_os(file_environment)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    if explicit_path
        .as_ref()
        .is_some_and(|path| !path.is_absolute())
    {
        return Err(invalid_setting(
            file_environment,
            "must be an absolute path",
        ));
    }
    let credential_path = env::var_os("CREDENTIALS_DIRECTORY")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|directory| directory.join(systemd_credential_name))
        .filter(|path| path.exists());
    let source_count = usize::from(direct.is_some())
        + usize::from(explicit_path.is_some())
        + usize::from(credential_path.is_some());
    if source_count > 1 {
        return Err(invalid_setting(
            value_environment,
            "must use exactly one direct, *_FILE, or systemd credential source",
        ));
    }
    if let Some(value) = direct {
        return Ok(Some(value));
    }
    explicit_path
        .or(credential_path)
        .map(|path| read_one_shot_secret_file(&path, file_environment))
        .transpose()
}

fn read_one_shot_secret_file(path: &Path, setting: &str) -> io::Result<String> {
    const MAX_ONE_SHOT_SECRET_BYTES: u64 = 64 * 1024;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        invalid_setting(setting, &format!("could not inspect secret file: {error}"))
    })?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(invalid_setting(
            setting,
            "must name a non-empty regular, non-symlink file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid_setting(
                setting,
                "secret file must not grant group or world permissions",
            ));
        }
    }
    if metadata.len() > MAX_ONE_SHOT_SECRET_BYTES {
        return Err(invalid_setting(setting, "secret file exceeds 64 KiB"));
    }
    let bytes = fs::read(path).map_err(|error| {
        invalid_setting(setting, &format!("could not read secret file: {error}"))
    })?;
    let decoded = std::str::from_utf8(&bytes)
        .map_err(|_| invalid_setting(setting, "secret file must be valid UTF-8"))?;
    let value = decoded
        .strip_suffix("\r\n")
        .or_else(|| decoded.strip_suffix('\n'))
        .unwrap_or(decoded);
    if value.is_empty()
        || value
            .chars()
            .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(invalid_setting(
            setting,
            "secret file must contain exactly one non-empty text line",
        ));
    }
    Ok(value.to_string())
}

fn validate_role_environment(runtime_role: RuntimeRole) -> io::Result<()> {
    let forbidden: &[&str] = match runtime_role {
        RuntimeRole::Api => &[
            "V2BOARD_WORKER_DATABASE_URL",
            "V2BOARD_CLICKHOUSE_URL",
            "V2BOARD_CLICKHOUSE_DATABASE",
            "V2BOARD_CLICKHOUSE_READER_USERNAME",
            "V2BOARD_CLICKHOUSE_READER_PASSWORD",
            "V2BOARD_CLICKHOUSE_WRITER_USERNAME",
            "V2BOARD_CLICKHOUSE_WRITER_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "V2BOARD_CLICKHOUSE_SCHEMA_DATABASE",
            "V2BOARD_CLICKHOUSE_SCHEMA_USERNAME",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD_FILE",
        ],
        RuntimeRole::Worker => &[
            "V2BOARD_WORKER_DATABASE_URL",
            "V2BOARD_NEW_PASSWORD",
            "V2BOARD_NEW_PASSWORD_FILE",
            "V2BOARD_CLICKHOUSE_READER_USERNAME",
            "V2BOARD_CLICKHOUSE_READER_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "V2BOARD_CLICKHOUSE_SCHEMA_DATABASE",
            "V2BOARD_CLICKHOUSE_SCHEMA_USERNAME",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD_FILE",
            "V2BOARD_MIGRATION_DATABASE_URL",
            "V2BOARD_MIGRATION_DATABASE_URL_FILE",
        ],
    };
    let present = forbidden
        .iter()
        .copied()
        .filter(|key| env_opt(key).is_some())
        .collect::<Vec<_>>();
    if present.is_empty() {
        Ok(())
    } else {
        Err(invalid_setting(
            "runtime role environment",
            &format!(
                "contains credentials or datastore settings forbidden for {}: {}",
                runtime_role.file_value(),
                present.join(", ")
            ),
        ))
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_opt(key).map(PathBuf::from)
}

fn resolve_app_key(
    environment: RuntimeEnvironment,
    configured: Option<String>,
) -> Result<String, &'static str> {
    if environment.is_production() {
        let Some(key) = configured.as_deref() else {
            return Err("APP_KEY must be explicitly configured for production");
        };
        if key.len() < 32 {
            return Err("APP_KEY must contain at least 32 bytes of secret material in production");
        }
        if is_obvious_secret_placeholder(key) {
            return Err("APP_KEY must not be a placeholder in production");
        }
    }
    Ok(configured.unwrap_or_else(|| "local-rust-dev-key".to_string()))
}

fn validate_https_configuration(
    environment: RuntimeEnvironment,
    force_https: bool,
    app_url: Option<&str>,
    has_trusted_proxy: bool,
) -> io::Result<()> {
    if environment.is_production() && !force_https {
        return Err(invalid_setting(
            "force_https",
            "must remain enabled for the fixed production Cloudflare Tunnel ingress",
        ));
    }
    if !force_https {
        return Ok(());
    }
    let Some(url) = app_url else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app_url/APP_URL is required when force_https is enabled",
        ));
    };
    let Some(authority) = url.strip_prefix("https://") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app_url/APP_URL must use https when force_https is enabled",
        ));
    };
    if authority
        .split(['/', '?', '#'])
        .next()
        .is_none_or(|host| host.is_empty() || host.contains('@'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app_url/APP_URL must contain a valid HTTPS authority",
        ));
    }
    if !has_trusted_proxy {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trusted_proxy_cidrs must identify same-host cloudflared when force_https is enabled",
        ));
    }
    Ok(())
}

/// Validate the independent schema job before it sends DDL credentials. The
/// schema binary does not load the application snapshot, so it must enforce
/// the same origin, identifier, TLS, and production-secret boundary itself.
pub fn validate_clickhouse_schema_connection(
    environment: RuntimeEnvironment,
    endpoint: &str,
    database: &str,
    username: &str,
    password: Option<&str>,
) -> io::Result<()> {
    let url = url::Url::parse(endpoint).map_err(|_| {
        invalid_setting(
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "must be a valid ClickHouse HTTP(S) origin",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_setting(
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "must be an HTTP(S) origin without credentials, path, query, or fragment",
        ));
    }
    if !valid_datastore_identifier(database) || !valid_datastore_identifier(username) {
        return Err(invalid_setting(
            "ClickHouse schema identity",
            "database and username must be unquoted ASCII identifiers",
        ));
    }
    if environment.is_production() {
        if url.scheme() != "https" {
            return Err(invalid_setting(
                "V2BOARD_CLICKHOUSE_SCHEMA_URL",
                "must use https:// with certificate verification in production",
            ));
        }
        validate_production_secret(environment, "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD", password)?;
    }
    Ok(())
}

fn validate_datastore_transport(
    environment: RuntimeEnvironment,
    database_url: &str,
    peer_database_principal: &str,
    clickhouse_writer: Option<&ClickHouseWriterConfig>,
    redis_url: &str,
) -> io::Result<()> {
    let database = url::Url::parse(database_url)
        .map_err(|_| invalid_setting("DATABASE_URL", "must be a valid PostgreSQL URL"))?;
    if !matches!(database.scheme(), "postgres" | "postgresql") {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must use the postgres or postgresql URL scheme",
        ));
    }
    if database.host_str().is_none() {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must include a host and database name",
        ));
    }
    validate_postgres_connection_query(&database, environment.is_production())?;
    postgres_database_name(&database)?;
    if !valid_postgres_identifier(peer_database_principal) {
        return Err(invalid_setting(
            "peer_database_principal",
            "must be one unquoted PostgreSQL identifier",
        ));
    }
    let database_principal = postgres_principal_name(&database)?;
    if database_principal == peer_database_principal {
        return Err(invalid_setting(
            "peer_database_principal",
            "must differ from the principal in database_url",
        ));
    }
    let redis = url::Url::parse(redis_url)
        .map_err(|_| invalid_setting("REDIS_URL", "must be a valid Redis URL"))?;
    if !matches!(redis.scheme(), "redis" | "rediss") {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use the redis or rediss URL scheme",
        ));
    }
    validate_redis_url(&redis, environment.is_production())?;
    if let Some(writer) = clickhouse_writer {
        let clickhouse = url::Url::parse(&writer.url).map_err(|_| {
            invalid_setting(
                "V2BOARD_CLICKHOUSE_URL",
                "must be a valid ClickHouse HTTP(S) endpoint",
            )
        })?;
        if !matches!(clickhouse.scheme(), "http" | "https")
            || clickhouse.host_str().is_none()
            || !clickhouse.username().is_empty()
            || clickhouse.password().is_some()
            || clickhouse.path() != "/"
            || clickhouse.query().is_some()
            || clickhouse.fragment().is_some()
        {
            return Err(invalid_setting(
                "V2BOARD_CLICKHOUSE_URL",
                "must be an HTTP(S) origin without credentials, path, query, or fragment",
            ));
        }
        if !valid_datastore_identifier(&writer.database)
            || !valid_datastore_identifier(&writer.username)
        {
            return Err(invalid_setting(
                "ClickHouse writer identity",
                "database and username must be unquoted ASCII identifiers",
            ));
        }
        if environment.is_production() {
            if clickhouse.scheme() != "https" {
                return Err(invalid_setting(
                    "V2BOARD_CLICKHOUSE_URL",
                    "must use https:// with certificate verification in production",
                ));
            }
            validate_production_secret(
                environment,
                "V2BOARD_CLICKHOUSE_WRITER_PASSWORD",
                writer.password.as_deref(),
            )?;
        }
    }
    if !environment.is_production() {
        return Ok(());
    }
    if redis.scheme() != "rediss" {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use rediss:// with certificate verification in production",
        ));
    }
    if database_principal.is_empty() {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must name an explicit principal in production",
        ));
    }
    Ok(())
}

/// Redis URI parsing is stricter than `redis::Client::open`: security checks
/// must cover the exact endpoint and database the driver will select, with no
/// alternate query/fragment form that can escape the reviewed boundary.
fn validate_redis_url(redis: &url::Url, production: bool) -> io::Result<()> {
    if redis.host_str().is_none() {
        return Err(invalid_setting("REDIS_URL", "must include a host"));
    }
    if redis.query().is_some() || redis.fragment().is_some() {
        return Err(invalid_setting(
            "REDIS_URL",
            "must not contain a query or fragment",
        ));
    }

    let database_component = redis.path().strip_prefix('/').unwrap_or_default();
    let database = (!database_component.is_empty()
        && database_component.bytes().all(|byte| byte.is_ascii_digit()))
    .then_some(database_component.parse::<u32>().ok())
    .flatten();
    if production && (database != Some(0) || database_component != "0") {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use canonical database /0 on a dedicated Redis instance in production; the installation keyspace supplies in-instance namespacing, not shared-instance isolation",
        ));
    }

    if !production {
        return Ok(());
    }

    let username = percent_decode_str(redis.username())
        .decode_utf8()
        .map_err(|_| invalid_setting("REDIS_URL username", "must be valid UTF-8"))?;
    if !valid_datastore_identifier(&username) || username == "default" {
        return Err(invalid_setting(
            "REDIS_URL username",
            "must name an explicit non-default ACL principal in production",
        ));
    }

    let encoded_password = redis.password().ok_or_else(|| {
        invalid_setting(
            "REDIS_URL",
            "must include an explicit password in production",
        )
    })?;
    let password = percent_decode_str(encoded_password)
        .decode_utf8()
        .map_err(|_| invalid_setting("REDIS_URL password", "must be valid UTF-8"))?;
    if password
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(invalid_setting(
            "REDIS_URL password",
            "must not contain control delimiters",
        ));
    }
    validate_production_secret(
        RuntimeEnvironment::Production,
        "REDIS_URL password",
        Some(password.as_ref()),
    )
}

/// URL usernames are percent-encoded components. Security comparisons must use
/// the decoded PostgreSQL role name so `%61pi` cannot masquerade as a principal
/// distinct from `api` while SQLx authenticates both as the same role.
pub fn postgres_principal_name(url: &url::Url) -> io::Result<String> {
    percent_decode_str(url.username())
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| invalid_setting("PostgreSQL URL username", "must be valid UTF-8"))
}

/// Return the exact decoded database component accepted by native configuration.
/// Requiring one unquoted identifier avoids treating a trailing
/// slash or alternate percent-encoding as an equivalent target during a
/// split-brain check when SQLx could interpret it differently.
pub fn postgres_database_name(url: &url::Url) -> io::Result<String> {
    let raw = url.path().strip_prefix('/').unwrap_or_default();
    let name = percent_decode_str(raw)
        .decode_utf8()
        .map_err(|_| invalid_setting("PostgreSQL URL database", "must be valid UTF-8"))?
        .into_owned();
    if !valid_postgres_identifier(&name) {
        return Err(invalid_setting(
            "PostgreSQL URL database",
            "must be one unquoted ASCII identifier of at most 63 bytes",
        ));
    }
    Ok(name)
}

/// Reject PostgreSQL URI query forms that can override the authority/path
/// inspected by connection and security checks. SQLx accepts libpq-style
/// identity overrides (`host`, `dbname`, `user`, ...) and both `sslmode` and
/// `ssl-mode`; accepting them here would let the validated URL describe one
/// endpoint while the driver connects to another.
pub fn validate_postgres_connection_query(
    url: &url::Url,
    require_verified_identity: bool,
) -> io::Result<()> {
    let mut seen = BTreeSet::new();
    let mut sslmode = None;
    for (key, value) in url.query_pairs() {
        let lowercase = key.to_ascii_lowercase();
        if key.as_ref() != lowercase {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "parameter names must use canonical lowercase spelling",
            ));
        }
        let canonical = lowercase.replace('_', "-");
        if !seen.insert(canonical.clone()) {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "duplicate or aliased parameters are not allowed",
            ));
        }
        if matches!(
            canonical.as_str(),
            "host"
                | "hostaddr"
                | "port"
                | "dbname"
                | "database"
                | "user"
                | "username"
                | "password"
                | "passfile"
                | "service"
                | "servicefile"
        ) {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "endpoint, database, principal, and secret overrides are forbidden",
            ));
        }
        if canonical == "ssl-mode" {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "ssl-mode aliases are forbidden; use exactly one sslmode parameter",
            ));
        }
        if canonical == "sslmode" {
            sslmode = Some(value.into_owned());
        }
    }
    if require_verified_identity && sslmode.as_deref() != Some("verify-full") {
        return Err(invalid_setting(
            "PostgreSQL URL query",
            "production URLs must set exactly one sslmode=verify-full",
        ));
    }
    Ok(())
}

fn validate_node_report_contract(
    environment: RuntimeEnvironment,
    file_only_config: bool,
    server_require_idempotency_key: bool,
) -> io::Result<()> {
    if !environment.is_production() && !file_only_config {
        return Ok(());
    }
    if !server_require_idempotency_key {
        return Err(invalid_setting(
            "server authentication",
            "production and file-only configurations require scoped node credentials and idempotency keys",
        ));
    }
    Ok(())
}

fn valid_postgres_identifier(value: &str) -> bool {
    valid_datastore_identifier(value) && value.len() <= 63
}

fn valid_datastore_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
        && value.len() <= 128
}

fn load_cors_allowed_origins(
    config: &Map<String, Value>,
    app_url: Option<&str>,
) -> io::Result<Vec<String>> {
    let configured = environment_value(config, "V2BOARD_CORS_ALLOWED_ORIGINS")
        .map(|value| parse_list(&value))
        .or_else(|| {
            config.get("cors_allowed_origins").map(|value| match value {
                Value::Array(items) => items.iter().filter_map(config_value_string).collect(),
                value => config_value_string(value)
                    .map(|value| parse_list(&value))
                    .unwrap_or_default(),
            })
        });
    let strict_origin_entries = configured.is_some();
    let candidates = configured.unwrap_or_else(|| {
        app_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .into_iter()
            .collect()
    });
    let mut origins = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let parsed = url::Url::parse(&candidate).map_err(|_| {
            invalid_setting(
                "cors_allowed_origins",
                "each entry must be an absolute HTTP(S) origin",
            )
        })?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || (strict_origin_entries
                && (parsed.query().is_some()
                    || parsed.fragment().is_some()
                    || parsed.path() != "/"))
        {
            return Err(invalid_setting(
                "cors_allowed_origins",
                "each entry must contain only an HTTP(S) scheme, host, and optional port",
            ));
        }
        let origin = parsed.origin().ascii_serialization();
        if origin == "null" {
            return Err(invalid_setting(
                "cors_allowed_origins",
                "opaque and wildcard origins are not allowed",
            ));
        }
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    Ok(origins)
}

fn parse_trusted_proxy_cidrs(config: &Map<String, Value>) -> io::Result<Vec<IpNet>> {
    config_list(
        config,
        "trusted_proxy_cidrs",
        "V2BOARD_TRUSTED_PROXY_CIDRS",
        &[],
    )
    .into_iter()
    .map(|cidr| {
        cidr.parse::<IpNet>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("trusted_proxy_cidrs contains invalid CIDR: {cidr}"),
            )
        })
    })
    .collect()
}

fn validate_production_proxy_topology(
    environment: RuntimeEnvironment,
    trusted_proxy_cidrs: &[IpNet],
) -> io::Result<()> {
    if !environment.is_production() {
        return Ok(());
    }
    let loopback_cloudflared = "127.0.0.1/32"
        .parse::<IpNet>()
        .expect("canonical loopback cloudflared CIDR");
    if trusted_proxy_cidrs.len() != 1 || trusted_proxy_cidrs[0] != loopback_cloudflared {
        return Err(invalid_setting(
            "trusted_proxy_cidrs",
            "bare-metal production must trust exactly same-host cloudflared at 127.0.0.1/32",
        ));
    }
    Ok(())
}

fn validate_production_secret(
    environment: RuntimeEnvironment,
    name: &str,
    value: Option<&str>,
) -> io::Result<()> {
    if !environment.is_production() {
        return Ok(());
    }
    let Some(value) = value else {
        return Err(invalid_setting(
            name,
            "must be explicitly configured in production",
        ));
    };
    if value.len() < 32 {
        return Err(invalid_setting(
            name,
            "must contain at least 32 bytes of secret material in production",
        ));
    }
    if is_obvious_secret_placeholder(value) {
        return Err(invalid_setting(
            name,
            "must not be a placeholder in production",
        ));
    }
    Ok(())
}

/// Cross-field checks shared by every full-runtime source: MySQL import manifest,
/// database authority overlay, and admin-save candidate. Keeping these checks
/// here ensures a candidate cannot pass one ingestion path and fail only after
/// it has become the active revision.
fn validate_operator_dependencies(config: &AppConfig) -> io::Result<()> {
    let configured = |value: Option<&str>| value.is_some_and(|value| !value.trim().is_empty());

    if config.recaptcha_enable
        && (!configured(config.recaptcha_site_key.as_deref())
            || !configured(config.recaptcha_key.as_deref()))
    {
        return Err(invalid_setting(
            "recaptcha_enable",
            "requires both recaptcha_site_key and recaptcha_key",
        ));
    }
    if config.telegram_bot_enable && !configured(config.telegram_bot_token.as_deref()) {
        return Err(invalid_setting(
            "telegram_bot_enable",
            "requires telegram_bot_token",
        ));
    }
    if config.email_verify
        && (!configured(config.email_host.as_deref())
            || !configured(config.email_from_address.as_deref()))
    {
        return Err(invalid_setting(
            "email_verify",
            "requires email_host and email_from_address",
        ));
    }
    if configured(config.email_username.as_deref()) != configured(config.email_password.as_deref())
    {
        return Err(invalid_setting(
            "email credentials",
            "email_username and email_password must be configured together",
        ));
    }
    validate_admin_path_configuration(config)?;
    validate_chat_widget_configuration(config)
}

/// docs/api-dialect.md §10.6: the chat widget is typed configuration, not
/// injected HTML. A configured provider must carry its complete, well-formed
/// identifiers — the SPA builds the official embed from these values and the
/// CSP allowlist widens per provider, so a malformed identifier must fail the
/// config save instead of shipping a broken (or attacker-shaped) embed.
fn validate_chat_widget_configuration(config: &AppConfig) -> io::Result<()> {
    let trimmed = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
    };
    let Some(provider) = trimmed(config.chat_widget_provider.as_deref()) else {
        return Ok(());
    };
    match provider.as_str() {
        "crisp" => {
            let website_id = config
                .chat_widget_crisp_website_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if !is_uuid_shaped(website_id) {
                return Err(invalid_setting(
                    "chat_widget_crisp_website_id",
                    "must be the Crisp website ID (UUID) when chat_widget_provider is `crisp`",
                ));
            }
        }
        "tawk" => {
            let property_id = config
                .chat_widget_tawk_property_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if property_id.len() != 24 || !property_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(invalid_setting(
                    "chat_widget_tawk_property_id",
                    "must be the 24-character hex Tawk property ID when chat_widget_provider is `tawk`",
                ));
            }
            let widget_id = config
                .chat_widget_tawk_widget_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if widget_id.is_empty()
                || widget_id.len() > 64
                || !widget_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                return Err(invalid_setting(
                    "chat_widget_tawk_widget_id",
                    "must be 1-64 characters of ASCII letters, digits, '_', or '-' when chat_widget_provider is `tawk`",
                ));
            }
        }
        _ => {
            return Err(invalid_setting(
                "chat_widget_provider",
                "must be `crisp` or `tawk`",
            ));
        }
    }
    Ok(())
}

/// Canonical 8-4-4-4-12 hex UUID shape (Crisp website IDs).
fn is_uuid_shaped(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// docs/api-dialect.md §10.2/§12: both admin-path knobs must satisfy
/// `secure_path`'s syntactic rule (≥ 8 characters of ASCII alphanumeric,
/// `_`, or `-`), and the resolved admin path may not equal a reserved
/// top-level segment — under the history-routing HTML subtree it would
/// shadow a user-SPA route root (including the backend-minted
/// `/order/{trade_no}` payment return), a fixed public route, or an API
/// namespace of the dynamic `/api/v1/{admin_path}/` prefix.
fn validate_admin_path_configuration(config: &AppConfig) -> io::Result<()> {
    for (setting, value) in [
        ("secure_path", config.secure_path.as_deref()),
        ("frontend_admin_path", config.frontend_admin_path.as_deref()),
    ] {
        let Some(effective) = value
            .map(str::trim)
            .map(|value| value.trim_matches('/'))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if effective.chars().count() < 8 {
            return Err(invalid_setting(
                setting,
                "must be at least 8 characters long",
            ));
        }
        if !effective
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            return Err(invalid_setting(
                setting,
                "must contain only ASCII letters, digits, '_', or '-'",
            ));
        }
    }

    let resolved = config.admin_path();
    let subscribe_first_segment = config
        .subscribe_path
        .trim()
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty());
    if RESERVED_ADMIN_PATH_SEGMENTS.contains(&resolved.as_str())
        || subscribe_first_segment == Some(resolved.as_str())
    {
        return Err(invalid_setting(
            "secure_path",
            &format!("admin path `{resolved}` collides with a reserved top-level path segment"),
        ));
    }
    Ok(())
}

fn is_obvious_secret_placeholder(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || (value.starts_with('<') && value.ends_with('>'))
        || value.bytes().all(|byte| byte == value.as_bytes()[0])
    {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    lower == "local-rust-dev-key"
        || [
            "change-me",
            "changeme",
            "replace-me",
            "replaceme",
            "replace-with",
            "your-secret",
            "insert-secret",
            "inject-at-least",
            "inject-a-different",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

/// Reject malformed scalar settings before the snapshot is built. Previously a
/// typo such as `V2BOARD_RECAPTCHA_ENABLE=tru` silently became `false`, and a
/// malformed integer silently selected its default. Security controls must not
/// fail open this way.
fn validate_scalar_config(
    config: &Map<String, Value>,
    runtime_role: RuntimeRole,
    parse_mode: ConfigParseMode,
) -> io::Result<()> {
    validate_configuration_source(config, runtime_role, parse_mode)?;
    const BOOL_SETTINGS: &[(&str, &str)] = &[
        ("force_https", "V2BOARD_FORCE_HTTPS"),
        ("email_verify", "V2BOARD_EMAIL_VERIFY"),
        ("stop_register", "V2BOARD_STOP_REGISTER"),
        ("invite_force", "V2BOARD_INVITE_FORCE"),
        ("invite_never_expire", "V2BOARD_INVITE_NEVER_EXPIRE"),
        ("email_whitelist_enable", "V2BOARD_EMAIL_WHITELIST_ENABLE"),
        (
            "email_gmail_limit_enable",
            "V2BOARD_EMAIL_GMAIL_LIMIT_ENABLE",
        ),
        ("recaptcha_enable", "V2BOARD_RECAPTCHA_ENABLE"),
        (
            "register_limit_by_ip_enable",
            "V2BOARD_REGISTER_LIMIT_BY_IP_ENABLE",
        ),
        ("telegram_bot_enable", "V2BOARD_TELEGRAM_BOT_ENABLE"),
        ("withdraw_close_enable", "V2BOARD_WITHDRAW_CLOSE_ENABLE"),
        (
            "commission_distribution_enable",
            "V2BOARD_COMMISSION_DISTRIBUTION_ENABLE",
        ),
        (
            "commission_auto_check_enable",
            "V2BOARD_COMMISSION_AUTO_CHECK_ENABLE",
        ),
        (
            "show_info_to_server_enable",
            "V2BOARD_SHOW_INFO_TO_SERVER_ENABLE",
        ),
        ("try_out_enable", "V2BOARD_TRY_OUT_ENABLE"),
        ("plan_change_enable", "V2BOARD_PLAN_CHANGE_ENABLE"),
        ("surplus_enable", "V2BOARD_SURPLUS_ENABLE"),
        (
            "commission_first_time_enable",
            "V2BOARD_COMMISSION_FIRST_TIME_ENABLE",
        ),
        ("server_log_enable", "V2BOARD_SERVER_LOG_ENABLE"),
        (
            "legacy_hash_redirect_enable",
            "V2BOARD_LEGACY_HASH_REDIRECT_ENABLE",
        ),
        ("safe_mode_enable", "V2BOARD_SAFE_MODE_ENABLE"),
        ("password_limit_enable", "V2BOARD_PASSWORD_LIMIT_ENABLE"),
        (
            "privileged_step_up_enable",
            "V2BOARD_PRIVILEGED_STEP_UP_ENABLE",
        ),
        (
            "server_require_idempotency_key",
            "V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY",
        ),
    ];
    const INTEGER_SETTINGS: &[(&str, &str)] = &[
        (
            "http_connect_timeout_seconds",
            "V2BOARD_HTTP_CONNECT_TIMEOUT_SECONDS",
        ),
        (
            "http_request_timeout_seconds",
            "V2BOARD_HTTP_REQUEST_TIMEOUT_SECONDS",
        ),
        (
            "api_request_timeout_seconds",
            "V2BOARD_API_REQUEST_TIMEOUT_SECONDS",
        ),
        (
            "password_kdf_max_parallel",
            "V2BOARD_PASSWORD_KDF_MAX_PARALLEL",
        ),
        (
            "auth_session_ttl_seconds",
            "V2BOARD_AUTH_SESSION_TTL_SECONDS",
        ),
        (
            "privileged_auth_session_ttl_seconds",
            "V2BOARD_PRIVILEGED_AUTH_SESSION_TTL_SECONDS",
        ),
        (
            "auth_session_max_per_user",
            "V2BOARD_AUTH_SESSION_MAX_PER_USER",
        ),
        (
            "privileged_step_up_ttl_seconds",
            "V2BOARD_PRIVILEGED_STEP_UP_TTL_SECONDS",
        ),
        (
            "privileged_step_up_max_attempts",
            "V2BOARD_PRIVILEGED_STEP_UP_MAX_ATTEMPTS",
        ),
        (
            "privileged_step_up_attempt_window_seconds",
            "V2BOARD_PRIVILEGED_STEP_UP_ATTEMPT_WINDOW_SECONDS",
        ),
        ("email_port", "V2BOARD_EMAIL_PORT"),
        ("register_limit_count", "V2BOARD_REGISTER_LIMIT_COUNT"),
        ("register_limit_expire", "V2BOARD_REGISTER_LIMIT_EXPIRE"),
        ("show_subscribe_method", "V2BOARD_SHOW_SUBSCRIBE_METHOD"),
        ("show_subscribe_expire", "V2BOARD_SHOW_SUBSCRIBE_EXPIRE"),
        ("allow_new_period", "V2BOARD_ALLOW_NEW_PERIOD"),
        ("reset_traffic_method", "V2BOARD_RESET_TRAFFIC_METHOD"),
        ("try_out_plan_id", "V2BOARD_TRY_OUT_PLAN_ID"),
        ("invite_commission", "V2BOARD_INVITE_COMMISSION"),
        ("new_order_event_id", "V2BOARD_NEW_ORDER_EVENT_ID"),
        ("renew_order_event_id", "V2BOARD_RENEW_ORDER_EVENT_ID"),
        ("change_order_event_id", "V2BOARD_CHANGE_ORDER_EVENT_ID"),
        ("invite_gen_limit", "V2BOARD_INVITE_GEN_LIMIT"),
        ("ticket_status", "V2BOARD_TICKET_STATUS"),
        ("server_push_interval", "V2BOARD_SERVER_PUSH_INTERVAL"),
        ("server_pull_interval", "V2BOARD_SERVER_PULL_INTERVAL"),
        (
            "server_node_report_min_traffic",
            "V2BOARD_SERVER_NODE_REPORT_MIN_TRAFFIC",
        ),
        (
            "server_device_online_min_traffic",
            "V2BOARD_SERVER_DEVICE_ONLINE_MIN_TRAFFIC",
        ),
        ("device_limit_mode", "V2BOARD_DEVICE_LIMIT_MODE"),
        ("password_limit_count", "V2BOARD_PASSWORD_LIMIT_COUNT"),
        ("password_limit_expire", "V2BOARD_PASSWORD_LIMIT_EXPIRE"),
    ];
    const I32_SETTINGS: &[&str] = &[
        "email_port",
        "show_subscribe_method",
        "allow_new_period",
        "reset_traffic_method",
        "try_out_plan_id",
        "invite_commission",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "ticket_status",
        "server_push_interval",
        "server_pull_interval",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "device_limit_mode",
    ];

    for &(config_key, env_key) in BOOL_SETTINGS {
        if let Some(value) = scalar_setting(config, config_key, env_key)?
            && parse_bool_strict(&value).is_none()
        {
            return Err(invalid_setting(
                config_key,
                "must be true/false, yes/no, on/off, or 1/0",
            ));
        }
    }
    for &(config_key, env_key) in INTEGER_SETTINGS {
        if let Some(value) = scalar_setting(config, config_key, env_key)? {
            if I32_SETTINGS.contains(&config_key) {
                value
                    .parse::<i32>()
                    .map_err(|_| invalid_setting(config_key, "must be a 32-bit integer"))?;
            } else {
                value
                    .parse::<i64>()
                    .map_err(|_| invalid_setting(config_key, "must be an integer"))?;
            }
        }
    }
    for (config_key, env_key, maximum) in [
        (
            "try_out_hour",
            "V2BOARD_TRY_OUT_HOUR",
            Decimal::from(i64::MAX) / Decimal::from(3_600),
        ),
        (
            "commission_withdraw_limit",
            "V2BOARD_COMMISSION_WITHDRAW_LIMIT",
            Decimal::from(i64::MAX) / Decimal::from(100),
        ),
    ] {
        if let Some(value) = scalar_setting(config, config_key, env_key)? {
            let value = value
                .parse::<Decimal>()
                .map_err(|_| invalid_setting(config_key, "must be a finite decimal number"))?;
            if value.is_sign_negative() || value > maximum {
                return Err(invalid_setting(
                    config_key,
                    "must be non-negative and within the supported range",
                ));
            }
        }
    }

    validate_integer_range(
        config,
        "http_connect_timeout_seconds",
        "V2BOARD_HTTP_CONNECT_TIMEOUT_SECONDS",
        1,
        300,
    )?;
    validate_integer_range(
        config,
        "http_request_timeout_seconds",
        "V2BOARD_HTTP_REQUEST_TIMEOUT_SECONDS",
        1,
        600,
    )?;
    validate_integer_range(
        config,
        "api_request_timeout_seconds",
        "V2BOARD_API_REQUEST_TIMEOUT_SECONDS",
        1,
        600,
    )?;
    validate_integer_range(
        config,
        "password_kdf_max_parallel",
        "V2BOARD_PASSWORD_KDF_MAX_PARALLEL",
        1,
        64,
    )?;
    validate_integer_range(
        config,
        "auth_session_ttl_seconds",
        "V2BOARD_AUTH_SESSION_TTL_SECONDS",
        3_600,
        365 * 24 * 60 * 60,
    )?;
    validate_integer_range(
        config,
        "privileged_auth_session_ttl_seconds",
        "V2BOARD_PRIVILEGED_AUTH_SESSION_TTL_SECONDS",
        15 * 60,
        7 * 24 * 60 * 60,
    )?;
    validate_integer_range(
        config,
        "privileged_step_up_ttl_seconds",
        "V2BOARD_PRIVILEGED_STEP_UP_TTL_SECONDS",
        60,
        60 * 60,
    )?;
    validate_integer_range(
        config,
        "privileged_step_up_max_attempts",
        "V2BOARD_PRIVILEGED_STEP_UP_MAX_ATTEMPTS",
        1,
        20,
    )?;
    validate_integer_range(
        config,
        "privileged_step_up_attempt_window_seconds",
        "V2BOARD_PRIVILEGED_STEP_UP_ATTEMPT_WINDOW_SECONDS",
        60,
        24 * 60 * 60,
    )?;
    validate_integer_range(
        config,
        "auth_session_max_per_user",
        "V2BOARD_AUTH_SESSION_MAX_PER_USER",
        1,
        100,
    )?;
    validate_integer_range(config, "email_port", "V2BOARD_EMAIL_PORT", 1, 65_535)?;
    validate_integer_range(
        config,
        "register_limit_count",
        "V2BOARD_REGISTER_LIMIT_COUNT",
        1,
        10_000,
    )?;
    validate_integer_range(
        config,
        "password_limit_count",
        "V2BOARD_PASSWORD_LIMIT_COUNT",
        1,
        1_000,
    )?;
    Ok(())
}

fn scalar_setting(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
) -> io::Result<Option<String>> {
    if !operator_value_is_authoritative(config, config_key)
        && let Some(value) = environment_value(config, env_key)
    {
        return Ok(Some(value));
    }
    let Some(value) = config.get(config_key) else {
        return Ok(None);
    };
    match value {
        Value::String(value) => Ok(Some(value.trim().to_string())),
        Value::Number(value) => Ok(Some(value.to_string())),
        Value::Bool(value) => Ok(Some(if *value { "true" } else { "false" }.to_string())),
        Value::Null => Ok(None),
        Value::Array(_) | Value::Object(_) => Err(invalid_setting(
            config_key,
            "must be a scalar string, number, or boolean",
        )),
    }
}

fn validate_integer_range(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    minimum: i64,
    maximum: i64,
) -> io::Result<()> {
    let Some(value) = scalar_setting(config, config_key, env_key)? else {
        return Ok(());
    };
    let value = value
        .parse::<i64>()
        .map_err(|_| invalid_setting(config_key, "must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(invalid_setting(
            config_key,
            &format!("must be between {minimum} and {maximum}"),
        ));
    }
    Ok(())
}

fn invalid_setting(config_key: &str, message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{config_key} {message}"),
    )
}

fn json_object(value: Value) -> Map<String, Value> {
    value
        .as_object()
        .expect("operator configuration literal is an object")
        .clone()
}

/// Reads the native runtime configuration. A missing file is an empty document;
/// an existing file must be a bounded, owner-only regular file whose identity
/// and length remain stable for the entire read. Malformed JSON, duplicate keys,
/// and non-object roots are surfaced instead of being interpreted as partially
/// valid configuration.
pub fn load_config(path: impl AsRef<Path>) -> io::Result<Map<String, Value>> {
    let path = path.as_ref();
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error),
    };
    validate_config_file_metadata(&before)?;

    let mut file = fs::File::open(path)?;
    let opened = file.metadata()?;
    validate_config_file_metadata(&opened)?;
    if !same_config_file(&before, &opened) {
        return Err(config_file_changed());
    }

    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or_default());
    (&mut file)
        .take(MAX_CONFIG_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_FILE_BYTES || bytes.len() as u64 != opened.len() {
        return Err(config_file_changed());
    }

    let opened_after = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    validate_config_file_metadata(&opened_after)?;
    validate_config_file_metadata(&after)?;
    if !same_config_file(&opened, &opened_after) || !same_config_file(&opened, &after) {
        return Err(config_file_changed());
    }

    let value = serde_json::from_slice::<UniqueJson>(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    value
        .0
        .as_object()
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "config root must be an object"))
}

fn validate_config_file_metadata(metadata: &fs::Metadata) -> io::Result<()> {
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config must be a regular non-symlink file",
        ));
    }
    if metadata.len() > MAX_CONFIG_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("config exceeds the {MAX_CONFIG_FILE_BYTES}-byte limit"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "config must not grant group or world permissions",
            ));
        }
    }
    Ok(())
}

fn same_config_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    if !left.file_type().is_file() || !right.file_type().is_file() || left.len() != right.len() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    true
}

fn config_file_changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "config identity or length changed while it was being read",
    )
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJson)
            .ok_or_else(|| E::custom("JSON number must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(UniqueJson(Value::String(value.to_string())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON key: {key}")));
            }
            let value = object.next_value::<UniqueJson>()?;
            values.insert(key, value.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

/// Atomically replaces the native runtime configuration while holding the same
/// sibling lock used by read/modify/write updates. Both file contents and the
/// parent-directory rename are synced before success is returned.
pub fn save_config_atomic(path: impl AsRef<Path>, config: &Map<String, Value>) -> io::Result<()> {
    let path = path.as_ref();
    with_config_lock(path, || save_config_atomic_unlocked(path, config))
}

/// Serializes a complete read/modify/write cycle across threads and processes.
/// The lock lives beside (rather than on) the config file because the final
/// atomic rename replaces the config inode.
pub fn update_config_atomic<T>(
    path: impl AsRef<Path>,
    update: impl FnOnce(&mut Map<String, Value>) -> io::Result<T>,
) -> io::Result<T> {
    let path = path.as_ref();
    with_config_lock(path, || {
        let mut config = load_config(path)?;
        let output = update(&mut config)?;
        save_config_atomic_unlocked(path, &config)?;
        Ok(output)
    })
}

fn with_config_lock<T>(path: &Path, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid config file name"))?;
    let lock_path = parent.join(format!(".{name}.lock"));
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(lock_path)?;
    lock.lock()?;
    let result = operation();
    let unlock = lock.unlock();
    match (result, unlock) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(output), Ok(())) => Ok(output),
    }
}

fn save_config_atomic_unlocked(path: &Path, config: &Map<String, Value>) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent)?;

    let mut bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    bytes.push(b'\n');

    let (temporary_path, mut temporary) = create_config_temp_file(path)?;
    if let Err(error) = temporary
        .write_all(&bytes)
        .and_then(|_| temporary.sync_all())
    {
        drop(temporary);
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    drop(temporary);

    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    fs::File::open(parent)?.sync_all()
}

fn create_config_temp_file(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid config file name"))?;

    for _ in 0..32 {
        let sequence = CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(".{name}.tmp-{}-{sequence}", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate config temporary file",
    ))
}

fn crc32b_hex(bytes: &[u8]) -> String {
    format!("{:08x}", crc32fast::hash(bytes))
}

/// FNV-1a (64-bit), implemented inline so the subscribe-mirror pick is
/// deterministic across processes, restarts, and platforms. `std`'s
/// `RandomState` hashing is per-process seeded and must never be used here.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn config_or_env(config: &Map<String, Value>, config_key: &str, env_key: &str) -> Option<String> {
    if operator_value_is_authoritative(config, config_key) {
        return config
            .get(config_key)
            .and_then(config_value_string)
            .filter(|value| !value.is_empty());
    }
    environment_value(config, env_key).or_else(|| {
        config
            .get(config_key)
            .and_then(config_value_string)
            .filter(|value| !value.is_empty())
    })
}

fn config_value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn config_bool(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: bool,
) -> bool {
    config_or_env(config, config_key, env_key)
        .as_deref()
        .map(parse_bool)
        .unwrap_or(default)
}

fn config_i32(config: &Map<String, Value>, config_key: &str, env_key: &str, default: i32) -> i32 {
    config_or_env(config, config_key, env_key)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
}

fn config_i64(config: &Map<String, Value>, config_key: &str, env_key: &str, default: i64) -> i64 {
    config_or_env(config, config_key, env_key)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

fn config_decimal(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: Decimal,
) -> io::Result<Decimal> {
    let Some(value) = config_or_env(config, config_key, env_key) else {
        return Ok(default);
    };
    value
        .parse::<Decimal>()
        .map_err(|_| invalid_setting(config_key, "must be a finite decimal number"))
}

fn config_duration_minutes(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: i64,
) -> io::Result<i64> {
    let value = match config_or_env(config, config_key, env_key) {
        Some(value) => value.parse::<i64>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{config_key} must be an integer number of minutes"),
            )
        })?,
        None => default,
    };
    if !(1..=MAX_CONFIG_DURATION_MINUTES).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{config_key} must be between 1 and {MAX_CONFIG_DURATION_MINUTES} minutes"),
        ));
    }
    Ok(value)
}

fn config_list(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: &[&str],
) -> Vec<String> {
    if !operator_value_is_authoritative(config, config_key)
        && let Some(value) = environment_value(config, env_key)
    {
        return parse_list(&value);
    }
    if let Some(value) = config.get(config_key) {
        return match value {
            // An explicit empty JSON array means empty. Treating it as missing
            // would silently resurrect built-in operator defaults after a
            // provision spec deliberately disabled every item.
            Value::Array(items) => items.iter().filter_map(config_value_string).collect(),
            value => config_value_string(value)
                .map(|value| parse_list(&value))
                .unwrap_or_default(),
        };
    }
    default.iter().map(|item| (*item).to_string()).collect()
}

fn operator_value_is_authoritative(config: &Map<String, Value>, key: &str) -> bool {
    config
        .get(OPERATOR_AUTHORITY_MARKER)
        .and_then(Value::as_bool)
        == Some(true)
        && OPERATOR_CONFIG_KEYS_V1.contains(&key)
}

fn environment_value(config: &Map<String, Value>, env_key: &str) -> Option<String> {
    (!configuration_is_file_only(config))
        .then(|| env_opt(env_key))
        .flatten()
}

fn configuration_is_file_only(config: &Map<String, Value>) -> bool {
    config.get(CONFIGURATION_SOURCE_KEY).and_then(Value::as_str)
        == Some(FILE_ONLY_CONFIGURATION_SOURCE)
}

fn configuration_is_boot_only(config: &Map<String, Value>) -> bool {
    configuration_is_file_only(config)
        && config.get(CONFIGURATION_SCOPE_KEY).and_then(Value::as_str)
            == Some(BOOT_ONLY_CONFIGURATION_SCOPE)
}

fn validate_configuration_source(
    config: &Map<String, Value>,
    runtime_role: RuntimeRole,
    parse_mode: ConfigParseMode,
) -> io::Result<()> {
    match config.get(CONFIGURATION_SOURCE_KEY) {
        None => Ok(()),
        Some(Value::String(value)) if value == FILE_ONLY_CONFIGURATION_SOURCE => {
            if config.get("runtime_role").and_then(Value::as_str) != Some(runtime_role.file_value())
            {
                return Err(invalid_setting(
                    "runtime_role",
                    &format!(
                        "must be the exact string {} for this process",
                        runtime_role.file_value()
                    ),
                ));
            }
            let boot_only = config.get(CONFIGURATION_SCOPE_KEY).and_then(Value::as_str)
                == Some(BOOT_ONLY_CONFIGURATION_SCOPE);
            match parse_mode {
                ConfigParseMode::BootOnly if !boot_only => {
                    return Err(invalid_setting(
                        CONFIGURATION_SCOPE_KEY,
                        "must be the exact string boot_only for the boot parser",
                    ));
                }
                ConfigParseMode::FullRuntime if config.contains_key(CONFIGURATION_SCOPE_KEY) => {
                    return Err(invalid_setting(
                        CONFIGURATION_SCOPE_KEY,
                        "is not supported by the full-runtime parser",
                    ));
                }
                ConfigParseMode::OperatorAuthority
                | ConfigParseMode::BootOnly
                | ConfigParseMode::FullRuntime => {}
            }
            let mut expected = if boot_only {
                BOOT_ONLY_RUNTIME_KEYS_V1
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
            } else {
                FILE_ONLY_RUNTIME_KEYS_V1
                    .iter()
                    .copied()
                    .chain([
                        "runtime_role",
                        "database_url",
                        "peer_database_principal",
                        "redis_url",
                    ])
                    .collect::<BTreeSet<_>>()
            };
            if parse_mode.is_operator_authority() && boot_only {
                expected.extend(OPERATOR_CONFIG_KEYS_V1.iter().copied());
            }
            if runtime_role == RuntimeRole::Worker {
                expected.extend([
                    "clickhouse_url",
                    "clickhouse_database",
                    "clickhouse_writer_username",
                    "clickhouse_writer_password",
                ]);
            }
            let mut actual = config.keys().map(String::as_str).collect::<BTreeSet<_>>();
            if parse_mode.is_operator_authority() {
                actual.remove(OPERATOR_AUTHORITY_MARKER);
            }
            let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(invalid_setting(
                    CONFIGURATION_SOURCE_KEY,
                    &format!("file_only document is missing keys: {}", missing.join(", ")),
                ));
            }
            let unknown = actual.difference(&expected).copied().collect::<Vec<_>>();
            if !unknown.is_empty() {
                return Err(invalid_setting(
                    CONFIGURATION_SOURCE_KEY,
                    &format!(
                        "file_only document contains unsupported keys: {}",
                        unknown.join(", ")
                    ),
                ));
            }
            Ok(())
        }
        Some(_) => Err(invalid_setting(
            CONFIGURATION_SOURCE_KEY,
            "must be the exact string file_only when present",
        )),
    }
}

fn parse_bool(value: &str) -> bool {
    parse_bool_strict(value).unwrap_or(false)
}

fn parse_bool_strict(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// `"amount:bonus"` yuan tiers → the best reward (in cents) whose threshold `total_amount`
/// (cents) has reached. Decimal arithmetic preserves the established truncation to whole cents
/// without binary floating-point underflow at values such as 0.29 yuan.
fn deposit_bonus_from_tiers(tiers: &[String], total_amount: i32) -> i32 {
    let mut bonus = 0;
    for tier in tiers {
        let Some((amount, value)) = tier.split_once(':') else {
            continue;
        };
        let (Some(amount), Some(value)) = (yuan_to_cents(amount), yuan_to_cents(value)) else {
            continue;
        };
        if total_amount >= amount {
            bonus = bonus.max(value);
        }
    }
    bonus
}

fn yuan_to_cents(value: &str) -> Option<i32> {
    let value = value.trim().parse::<Decimal>().ok()?;
    if value.is_sign_negative() {
        return None;
    }
    (value * Decimal::from(100)).trunc().to_i32()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn file_only_document(role: RuntimeRole) -> Map<String, Value> {
        let mut config = FILE_ONLY_RUNTIME_KEYS_V1
            .iter()
            .map(|key| ((*key).to_string(), Value::Null))
            .collect::<Map<_, _>>();
        config.insert(
            "configuration_source".to_string(),
            Value::String("file_only".to_string()),
        );
        config.insert(
            "runtime_role".to_string(),
            Value::String(role.file_value().to_string()),
        );
        config.insert("database_url".to_string(), Value::Null);
        config.insert("peer_database_principal".to_string(), Value::Null);
        config.insert("redis_url".to_string(), Value::Null);
        config.insert(
            "server_require_idempotency_key".to_string(),
            Value::Bool(true),
        );
        if role == RuntimeRole::Worker {
            for key in [
                "clickhouse_url",
                "clickhouse_database",
                "clickhouse_writer_username",
                "clickhouse_writer_password",
            ] {
                config.insert(key.to_string(), Value::Null);
            }
        }
        config
    }

    fn boot_only_document(role: RuntimeRole) -> Map<String, Value> {
        let mut config = BOOT_ONLY_RUNTIME_KEYS_V1
            .iter()
            .map(|key| ((*key).to_string(), Value::Null))
            .collect::<Map<_, _>>();
        config.insert(
            "configuration_source".to_string(),
            Value::String("file_only".to_string()),
        );
        config.insert(
            "configuration_scope".to_string(),
            Value::String("boot_only".to_string()),
        );
        config.insert(
            "runtime_role".to_string(),
            Value::String(role.file_value().to_string()),
        );
        if role == RuntimeRole::Worker {
            for key in [
                "clickhouse_url",
                "clickhouse_database",
                "clickhouse_writer_username",
                "clickhouse_writer_password",
            ] {
                config.insert(key.to_string(), Value::Null);
            }
        }
        config
    }

    #[test]
    fn one_shot_secret_files_are_single_line_regular_files() {
        let root = env::temp_dir().join(format!(
            "v2board-one-shot-secret-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create secret test directory");
        let path = root.join("credential");
        fs::write(&path, b"secret-value\n").expect("write secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("restrict secret permissions");
        }
        assert_eq!(
            read_one_shot_secret_file(&path, "TEST_SECRET").expect("read secret"),
            "secret-value"
        );
        fs::write(&path, b"first\nsecond\n").expect("write multiline secret");
        assert!(read_one_shot_secret_file(&path, "TEST_SECRET").is_err());
        fs::remove_dir_all(root).expect("remove secret test directory");
    }

    #[test]
    fn minute_durations_are_bounded_before_seconds_conversion() {
        assert_eq!(duration_minutes_to_seconds(1), 60);
        assert_eq!(duration_minutes_to_seconds(0), 60);
        assert_eq!(
            duration_minutes_to_seconds(i64::MAX),
            MAX_CONFIG_DURATION_MINUTES as u64 * 60
        );

        let mut config = serde_json::json!({ "unsafe_minutes": i64::MAX })
            .as_object()
            .expect("object")
            .clone();
        let error =
            config_duration_minutes(&config, "unsafe_minutes", "V2BOARD_TEST_UNSET_DURATION", 5)
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        config.insert("unsafe_minutes".to_string(), Value::from("not-a-number"));
        assert!(
            config_duration_minutes(&config, "unsafe_minutes", "V2BOARD_TEST_UNSET_DURATION", 5,)
                .is_err()
        );
    }

    #[test]
    fn deposit_bonus_picks_the_best_reached_tier() {
        let tiers = vec!["10:1".to_string(), "50:8".to_string(), "100:20".to_string()];
        assert_eq!(deposit_bonus_from_tiers(&tiers, 999), 0);
        assert_eq!(deposit_bonus_from_tiers(&tiers, 1000), 100);
        assert_eq!(deposit_bonus_from_tiers(&tiers, 5000), 800);
        assert_eq!(deposit_bonus_from_tiers(&tiers, 20000), 2000);
        assert_eq!(deposit_bonus_from_tiers(&[], 20000), 0);
    }

    #[test]
    fn deposit_bonus_uses_exact_decimal_cents_and_ignores_invalid_tiers() {
        let tiers = vec![
            "0.29:0.10".to_string(),
            "invalid:999".to_string(),
            "1:-2".to_string(),
        ];
        assert_eq!(deposit_bonus_from_tiers(&tiers, 28), 0);
        assert_eq!(deposit_bonus_from_tiers(&tiers, 29), 10);
    }

    #[test]
    fn admin_path_fallback_uses_crc32b() {
        assert_eq!(crc32b_hex(b"test"), "d87f7e0c");
    }

    /// docs/api-dialect.md §10.2: both admin-path knobs carry `secure_path`'s
    /// syntactic rule, and the resolved path may not shadow a reserved
    /// top-level segment once the HTML fallback claims the admin subtree.
    #[test]
    fn admin_path_knobs_are_validated_syntactically_and_against_reserved_segments() {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let mut config = AppConfig::try_from_api_config_map(Map::new(), paths)
            .expect("development config parses");

        config.secure_path = Some("valid-admin-path".to_string());
        config.frontend_admin_path = None;
        assert!(validate_admin_path_configuration(&config).is_ok());

        // Same syntactic rule as secure_path: ≥ 8 chars, alphanumeric/_/-.
        config.secure_path = None;
        config.frontend_admin_path = Some("short".to_string());
        let error = validate_admin_path_configuration(&config).unwrap_err();
        assert!(error.to_string().contains("frontend_admin_path"));
        config.frontend_admin_path = Some("bad/path!chars".to_string());
        assert!(validate_admin_path_configuration(&config).is_err());

        // Reserved collisions: user-SPA roots and API namespaces are legal
        // syntactically but would shadow public routes.
        for reserved in ["dashboard", "knowledge", "passport-x"] {
            config.frontend_admin_path = Some(reserved.to_string());
            let result = validate_admin_path_configuration(&config);
            if reserved == "passport-x" {
                assert!(result.is_ok(), "non-reserved {reserved} must pass");
            } else {
                assert!(result.is_err(), "reserved {reserved} must be rejected");
            }
        }

        // The operator subscribe alias's first segment is reserved too.
        config.frontend_admin_path = Some("mysubscribe".to_string());
        config.subscribe_path = "/mysubscribe/feed".to_string();
        assert!(validate_admin_path_configuration(&config).is_err());
        config.subscribe_path = "/subs/feed".to_string();
        assert!(validate_admin_path_configuration(&config).is_ok());

        // Unset knobs fall back to the 8-hex-char crc32b digest, which can
        // never collide with a reserved segment.
        config.secure_path = None;
        config.frontend_admin_path = None;
        assert!(validate_admin_path_configuration(&config).is_ok());
    }

    /// docs/api-dialect.md §10.6: a configured chat provider must carry its
    /// complete, well-formed identifiers; anything else fails the config save.
    #[test]
    fn chat_widget_configuration_requires_complete_well_formed_provider_ids() {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let mut config = AppConfig::try_from_api_config_map(Map::new(), paths)
            .expect("development config parses");

        // No provider: identifiers are irrelevant.
        assert!(validate_chat_widget_configuration(&config).is_ok());
        config.chat_widget_provider = Some("   ".to_string());
        assert!(validate_chat_widget_configuration(&config).is_ok());

        config.chat_widget_provider = Some("livechat".to_string());
        let error = validate_chat_widget_configuration(&config).unwrap_err();
        assert!(error.to_string().contains("chat_widget_provider"));

        // Crisp requires a UUID-shaped website ID.
        config.chat_widget_provider = Some("crisp".to_string());
        assert!(validate_chat_widget_configuration(&config).is_err());
        config.chat_widget_crisp_website_id = Some("not-a-uuid".to_string());
        assert!(validate_chat_widget_configuration(&config).is_err());
        config.chat_widget_crisp_website_id =
            Some("a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d".to_string());
        assert!(validate_chat_widget_configuration(&config).is_ok());

        // Tawk requires the 24-hex property ID plus a bounded widget ID.
        config.chat_widget_provider = Some("Tawk".to_string());
        assert!(validate_chat_widget_configuration(&config).is_err());
        config.chat_widget_tawk_property_id = Some("5f0c1d2e3a4b5c6d7e8f9a0b".to_string());
        assert!(validate_chat_widget_configuration(&config).is_err());
        config.chat_widget_tawk_widget_id = Some("default".to_string());
        assert!(validate_chat_widget_configuration(&config).is_ok());
        config.chat_widget_tawk_property_id = Some("nothexnothexnothexnothex".to_string());
        assert!(validate_chat_widget_configuration(&config).is_err());
        config.chat_widget_tawk_property_id = Some("5f0c1d2e3a4b5c6d7e8f9a0b".to_string());
        config.chat_widget_tawk_widget_id = Some("bad widget!".to_string());
        assert!(validate_chat_widget_configuration(&config).is_err());
    }

    fn subscribe_config(subscribe_url: Option<&str>) -> AppConfig {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let mut document = Map::new();
        document.insert("app_url".to_string(), json!("https://panel.example"));
        if let Some(subscribe_url) = subscribe_url {
            document.insert("subscribe_url".to_string(), json!(subscribe_url));
        }
        AppConfig::try_from_api_config_map(document, paths).expect("subscribe test config")
    }

    #[test]
    fn subscribe_url_single_mirror_and_app_url_fallback_are_unchanged() {
        let single = subscribe_config(Some("https://mirror.example/"));
        assert_eq!(
            single.subscribe_url_for_token("token-a"),
            "https://mirror.example/api/v1/client/subscribe?token=token-a"
        );

        // No configured mirror falls back to app_url (which the container
        // environment may override, so derive the expectation from the snapshot).
        let unconfigured = subscribe_config(None);
        let app_url = unconfigured.app_url.clone().expect("app_url");
        assert_eq!(
            unconfigured.subscribe_url_for_token("token-a"),
            format!(
                "{}/api/v1/client/subscribe?token=token-a",
                app_url.trim_end_matches('/')
            )
        );
    }

    #[test]
    fn subscribe_url_multi_mirror_pick_is_a_stable_token_hash() {
        // Helper.php:107-108 rotated with rand(); the native pick must instead be
        // the deterministic FNV-1a(token) % mirror-count so a token never flaps.
        let config = subscribe_config(Some("https://m0.example,https://m1.example/"));
        // fnv1a_64("token-a") = 0xe572a608d45b6244 -> index 0.
        assert_eq!(
            config.subscribe_url_for_token("token-a"),
            "https://m0.example/api/v1/client/subscribe?token=token-a"
        );
        // fnv1a_64("token-b") = 0xe572a908d45b675d -> index 1.
        assert_eq!(
            config.subscribe_url_for_token("token-b"),
            "https://m1.example/api/v1/client/subscribe?token=token-b"
        );
        // Stable across repeated renders of the same token.
        assert_eq!(
            config.subscribe_url_for_token("token-b"),
            "https://m1.example/api/v1/client/subscribe?token=token-b"
        );
    }

    #[test]
    fn subscribe_url_mirror_parsing_skips_empty_and_whitespace_entries() {
        // Empty/whitespace entries are dropped before the modulus, so the two
        // real mirrors keep the same assignment as a clean two-entry list.
        let config = subscribe_config(Some(" , https://m0.example ,,\thttps://m1.example/ ,"));
        assert_eq!(
            config.subscribe_url_for_token("token-a"),
            "https://m0.example/api/v1/client/subscribe?token=token-a"
        );
        assert_eq!(
            config.subscribe_url_for_token("token-b"),
            "https://m1.example/api/v1/client/subscribe?token=token-b"
        );

        // A mirror list with only blank entries behaves like no mirror at all.
        let blank_only = subscribe_config(Some(" , ,\t"));
        let app_url = blank_only.app_url.clone().expect("app_url");
        assert_eq!(
            blank_only.subscribe_url_for_token("token-a"),
            format!(
                "{}/api/v1/client/subscribe?token=token-a",
                app_url.trim_end_matches('/')
            )
        );
    }

    #[test]
    fn production_aliases_require_a_strong_explicit_app_key() {
        assert_eq!(
            RuntimeEnvironment::parse(Some("prod")).unwrap(),
            RuntimeEnvironment::Production
        );
        assert!(resolve_app_key(RuntimeEnvironment::Production, None).is_err());
        assert!(
            resolve_app_key(
                RuntimeEnvironment::Production,
                Some("local-rust-dev-key".to_string())
            )
            .is_err()
        );
        assert!(
            resolve_app_key(
                RuntimeEnvironment::Production,
                Some("production-secret".to_string())
            )
            .is_err()
        );
        assert_eq!(
            resolve_app_key(
                RuntimeEnvironment::Production,
                Some("0123456789abcdef0123456789abcdef".to_string())
            )
            .unwrap(),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            resolve_app_key(RuntimeEnvironment::Local, None).unwrap(),
            "local-rust-dev-key"
        );
        assert!(RuntimeEnvironment::parse(Some("prdduction")).is_err());
    }

    #[test]
    fn malformed_security_scalars_fail_closed() {
        let invalid_bool = serde_json::json!({ "recaptcha_enable": "tru" })
            .as_object()
            .expect("object")
            .clone();
        let error = validate_scalar_config(
            &invalid_bool,
            RuntimeRole::Api,
            ConfigParseMode::FullRuntime,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("recaptcha_enable"));

        let invalid_integer = serde_json::json!({ "password_limit_count": "many" })
            .as_object()
            .expect("object")
            .clone();
        assert!(
            validate_scalar_config(
                &invalid_integer,
                RuntimeRole::Api,
                ConfigParseMode::FullRuntime
            )
            .is_err()
        );

        let out_of_range = serde_json::json!({ "auth_session_max_per_user": 101 })
            .as_object()
            .expect("object")
            .clone();
        assert!(
            validate_scalar_config(
                &out_of_range,
                RuntimeRole::Api,
                ConfigParseMode::FullRuntime
            )
            .is_err()
        );

        let overflowing_i32 = serde_json::json!({ "server_push_interval": 2147483648_i64 })
            .as_object()
            .expect("object")
            .clone();
        assert!(
            validate_scalar_config(
                &overflowing_i32,
                RuntimeRole::Api,
                ConfigParseMode::FullRuntime
            )
            .is_err()
        );

        let structural = serde_json::json!({ "force_https": [] })
            .as_object()
            .expect("object")
            .clone();
        assert!(
            validate_scalar_config(&structural, RuntimeRole::Api, ConfigParseMode::FullRuntime)
                .is_err()
        );
    }

    #[test]
    fn enabled_integrations_require_their_complete_credentials() {
        let paths = || RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let parse = |value: Value| {
            AppConfig::try_from_api_config_map(value.as_object().expect("object").clone(), paths())
        };

        assert!(parse(json!({ "recaptcha_enable": true })).is_err());
        assert!(
            parse(json!({
                "recaptcha_enable": true,
                "recaptcha_site_key": "site",
                "recaptcha_key": "secret"
            }))
            .is_ok()
        );
        assert!(parse(json!({ "telegram_bot_enable": true })).is_err());
        assert!(
            parse(json!({
                "telegram_bot_enable": true,
                "telegram_bot_token": "token"
            }))
            .is_ok()
        );
        assert!(parse(json!({ "email_verify": true })).is_err());
        assert!(
            parse(json!({
                "email_verify": true,
                "email_host": "smtp.example.com",
                "email_from_address": "noreply@example.com"
            }))
            .is_ok()
        );
        assert!(parse(json!({ "email_username": "user" })).is_err());
        assert!(parse(json!({ "email_password": "password" })).is_err());
        assert!(
            parse(json!({
                "email_username": "user",
                "email_password": "password"
            }))
            .is_ok()
        );
    }

    #[test]
    fn optional_email_port_can_be_cleared_to_null() {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let configured = AppConfig::try_from_api_config_map(
            json!({ "email_port": 587 })
                .as_object()
                .expect("object")
                .clone(),
            paths.clone(),
        )
        .expect("configured port");
        assert_eq!(configured.email_port, Some(587));

        let cleared = AppConfig::try_from_api_config_map(
            json!({ "email_port": null })
                .as_object()
                .expect("object")
                .clone(),
            paths,
        )
        .expect("cleared port");
        assert_eq!(cleared.email_port, None);
    }

    #[test]
    fn production_locks_https_and_requires_a_canonical_app_url() {
        assert!(
            validate_https_configuration(RuntimeEnvironment::Development, false, None, false)
                .is_ok()
        );
        assert!(
            validate_https_configuration(
                RuntimeEnvironment::Production,
                false,
                Some("https://example.com"),
                true,
            )
            .is_err()
        );
        assert!(
            validate_https_configuration(RuntimeEnvironment::Development, true, None, true)
                .is_err()
        );
        assert!(
            validate_https_configuration(
                RuntimeEnvironment::Development,
                true,
                Some("http://example.com"),
                true,
            )
            .is_err()
        );
        assert!(
            validate_https_configuration(
                RuntimeEnvironment::Development,
                true,
                Some("https://user@example.com"),
                true,
            )
            .is_err()
        );
        assert!(
            validate_https_configuration(
                RuntimeEnvironment::Development,
                true,
                Some("https://example.com"),
                false,
            )
            .is_err()
        );
        assert!(
            validate_https_configuration(
                RuntimeEnvironment::Production,
                true,
                Some("https://example.com"),
                true,
            )
            .is_ok()
        );
    }

    #[test]
    fn operator_updates_cannot_disable_production_https() {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let mut config =
            AppConfig::try_from_api_config_map(Map::new(), paths).expect("development config");
        config.environment = RuntimeEnvironment::Production;
        config.trusted_proxy_cidrs = vec!["127.0.0.1/32".parse().unwrap()];

        let error = config
            .validate_security_update(
                Some("0123456789abcdef0123456789abcdef"),
                false,
                Some("https://panel.example.com"),
            )
            .expect_err("production force_https must be immutable");
        assert!(error.to_string().contains("force_https"));
    }

    #[test]
    fn production_server_master_token_is_explicit_and_strong() {
        assert!(
            validate_production_secret(RuntimeEnvironment::Production, "server_token", None)
                .is_err()
        );
        assert!(
            validate_production_secret(
                RuntimeEnvironment::Production,
                "server_token",
                Some("short-secret")
            )
            .is_err()
        );
        for placeholder in [
            "<inject-at-least-32-random-bytes>",
            "<inject-a-different-32-byte-random-secret>",
            "replace-with-a-real-production-secret-now",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(
                validate_production_secret(
                    RuntimeEnvironment::Production,
                    "server_token",
                    Some(placeholder),
                )
                .is_err(),
                "placeholder must fail closed: {placeholder}"
            );
            assert!(
                resolve_app_key(
                    RuntimeEnvironment::Production,
                    Some(placeholder.to_string())
                )
                .is_err(),
                "APP_KEY placeholder must fail closed: {placeholder}"
            );
        }
        assert!(
            validate_production_secret(
                RuntimeEnvironment::Production,
                "server_token",
                Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            )
            .is_err()
        );
        assert!(
            validate_production_secret(
                RuntimeEnvironment::Production,
                "server_token",
                Some("0123456789abcdef0123456789abcdef")
            )
            .is_ok()
        );
        assert!(
            validate_production_secret(RuntimeEnvironment::Local, "server_token", None).is_ok()
        );
    }

    #[test]
    fn json_config_round_trips_through_atomic_storage() {
        let root = env::temp_dir().join(format!(
            "v2board-config-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("config/config.json");
        let expected = serde_json::json!({
            "app_name": "Native V2Board",
            "email_verify": true,
            "email_whitelist_suffix": ["example.com", "example.org"]
        })
        .as_object()
        .expect("object")
        .clone();

        save_config_atomic(&path, &expected).expect("atomic save");
        assert_eq!(load_config(&path).expect("load config"), expected);
        assert!(
            fs::read_to_string(&path)
                .expect("stored config")
                .ends_with('\n')
        );

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn missing_config_remains_an_empty_local_document() {
        let path = env::temp_dir().join(format!(
            "v2board-config-missing-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert_eq!(load_config(path).expect("missing config"), Map::new());
    }

    #[cfg(unix)]
    #[test]
    fn config_loader_rejects_symlinks_and_group_or_world_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let root = env::temp_dir().join(format!(
            "v2board-config-file-boundary-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let target = root.join("target.json");
        let link = root.join("config.json");
        save_config_atomic(&target, &Map::new()).expect("write target");
        symlink(&target, &link).expect("create config symlink");
        assert!(load_config(&link).is_err());

        fs::set_permissions(&target, fs::Permissions::from_mode(0o640))
            .expect("make target group-readable");
        let error = load_config(&target).expect_err("permissive config must fail");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn config_loader_rejects_duplicate_keys_at_every_object_depth() {
        let root = env::temp_dir().join(format!(
            "v2board-config-duplicate-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("config.json");
        save_config_atomic(&path, &Map::new()).expect("create owner-only config");
        fs::write(&path, br#"{"outer":{"key":1,"key":2}}"#).expect("write duplicate JSON");

        let error = load_config(&path).expect_err("duplicate key must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("duplicate JSON key: key"));

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn config_loader_rejects_oversized_files_before_parsing() {
        let root = env::temp_dir().join(format!(
            "v2board-config-size-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("config.json");
        save_config_atomic(&path, &Map::new()).expect("create owner-only config");
        fs::write(&path, vec![b' '; MAX_CONFIG_FILE_BYTES as usize + 1])
            .expect("write oversized config");

        let error = load_config(&path).expect_err("oversized config must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds"));

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn locked_updates_do_not_lose_concurrent_keys() {
        let root = env::temp_dir().join(format!(
            "v2board-config-lock-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("config/config.json");
        save_config_atomic(&path, &Map::new()).expect("initial config");
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers));
        let handles = (0..workers)
            .map(|index| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    update_config_atomic(&path, |config| {
                        config.insert(format!("worker-{index}"), Value::from(index));
                        Ok(())
                    })
                    .expect("locked update");
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().expect("update thread");
        }
        let stored = load_config(&path).expect("stored config");
        for index in 0..workers {
            assert_eq!(
                stored.get(&format!("worker-{index}")),
                Some(&Value::from(index))
            );
        }
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn reload_rejects_malformed_edits_and_can_recover() {
        let root = env::temp_dir().join(format!(
            "v2board-config-reload-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("config/config.json");
        let runtime_paths = RuntimePaths {
            config: path.clone(),
            frontend: root.join("frontend"),
            rules: root.join("rules"),
        };
        let initial = serde_json::json!({ "ticket_status": 17 })
            .as_object()
            .expect("object")
            .clone();
        save_config_atomic(&path, &initial).expect("initial config");
        let snapshot = AppConfig::try_from_runtime_paths(
            RuntimeRole::Api,
            runtime_paths,
            ConfigParseMode::FullRuntime,
        )
        .expect("initial snapshot");
        assert_eq!(snapshot.ticket_status, 17);
        assert_eq!(snapshot.privileged_auth_session_ttl_seconds, 30 * 60);
        assert_eq!(
            snapshot.privileged_step_up_ttl_seconds,
            snapshot.privileged_auth_session_ttl_seconds
        );

        fs::write(&path, b"{not-json").expect("malformed external edit");
        assert!(snapshot.reload().is_err());
        assert_eq!(
            snapshot.ticket_status, 17,
            "the prior snapshot is immutable"
        );

        let repaired = serde_json::json!({ "ticket_status": 23 })
            .as_object()
            .expect("object")
            .clone();
        save_config_atomic(&path, &repaired).expect("repair config");
        assert_eq!(
            snapshot.reload().expect("reloaded snapshot").ticket_status,
            23
        );

        let restart_bound_edit = serde_json::json!({
            "ticket_status": 23,
            "password_kdf_max_parallel": 5
        })
        .as_object()
        .expect("object")
        .clone();
        save_config_atomic(&path, &restart_bound_edit).expect("restart-bound edit");
        let error = match snapshot.reload() {
            Ok(_) => panic!("datastore cutover must require a process restart"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("password_kdf_max_parallel"));
        assert!(error.to_string().contains("restart-required"));
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn native_json_arrays_are_loaded_as_config_lists() {
        let config = serde_json::json!({
            "domains": ["example.com", "example.org"]
        })
        .as_object()
        .expect("object")
        .clone();
        assert_eq!(
            config_list(&config, "domains", "V2BOARD_TEST_UNUSED_LIST", &[]),
            vec!["example.com", "example.org"]
        );

        let empty = serde_json::json!({ "domains": [] })
            .as_object()
            .expect("object")
            .clone();
        assert!(
            config_list(
                &empty,
                "domains",
                "V2BOARD_TEST_UNUSED_LIST",
                &["default.example"]
            )
            .is_empty(),
            "an explicit empty list must not restore built-in defaults"
        );
    }

    #[test]
    fn file_only_configuration_ignores_value_environment() {
        assert!(env::var("PATH").is_ok(), "test process must have PATH");
        let config = serde_json::json!({
            "configuration_source": "file_only",
            "app_name": "From file"
        })
        .as_object()
        .expect("object")
        .clone();
        assert_eq!(environment_value(&config, "PATH"), None);
        assert_eq!(
            config_or_env(&config, "app_name", "PATH").as_deref(),
            Some("From file")
        );
    }

    #[test]
    fn operator_values_override_environment_without_changing_boot_precedence() {
        let path = env::var("PATH").expect("test process must have PATH");
        let mut config = serde_json::json!({ "app_name": "From file" })
            .as_object()
            .expect("object")
            .clone();
        assert_eq!(
            config_or_env(&config, "app_name", "PATH").as_deref(),
            Some(path.as_str()),
            "ordinary boot documents retain the established environment override"
        );
        config.insert(OPERATOR_AUTHORITY_MARKER.to_string(), Value::Bool(true));
        assert_eq!(
            config_or_env(&config, "app_name", "PATH").as_deref(),
            Some("From file"),
            "the versioned operator snapshot is authoritative"
        );

        let runtime_paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        assert!(AppConfig::try_from_api_config_map(config, runtime_paths).is_err());
    }

    #[test]
    fn file_only_api_and_worker_accept_only_internal_operator_overlay() {
        for role in [RuntimeRole::Api, RuntimeRole::Worker] {
            let root = env::temp_dir().join(format!(
                "v2board-operator-overlay-test-{}-{}-{}",
                std::process::id(),
                role.file_value(),
                CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let paths = RuntimePaths {
                config: root.join("config.json"),
                frontend: root.join("frontend"),
                rules: root.join("rules"),
            };
            let document = boot_only_document(role);
            save_config_atomic(&paths.config, &document).expect("write role boot document");
            let baseline = match role {
                RuntimeRole::Api => {
                    AppConfig::try_from_api_boot_config_map(document, paths.clone())
                        .expect("API boot")
                }
                RuntimeRole::Worker => {
                    AppConfig::try_from_worker_boot_config_map(document, paths.clone())
                        .expect("worker boot")
                }
            };
            let mut operator = baseline.operator_config_map();
            operator.insert(
                "app_name".to_string(),
                Value::String(format!("authority-{}", role.file_value())),
            );
            operator.insert("try_out_hour".to_string(), Value::String("1.5".to_string()));
            operator.insert(
                "commission_withdraw_limit".to_string(),
                Value::String("10.05".to_string()),
            );
            operator.insert(
                "server_require_idempotency_key".to_string(),
                Value::Bool(true),
            );
            let applied = baseline
                .with_operator_config(&operator, 7)
                .expect("internal authority overlay");
            assert_eq!(applied.operator_revision(), Some(7));
            assert_eq!(applied.app_name, format!("authority-{}", role.file_value()));
            assert_eq!(applied.try_out_hour, Decimal::new(15, 1));
            assert_eq!(applied.commission_withdraw_limit, Decimal::new(1005, 2));
            assert_eq!(
                applied.operator_config_map()["commission_withdraw_limit"],
                Value::String("10.05".to_string())
            );
            fs::remove_dir_all(root).expect("remove overlay test root");
        }
    }

    #[test]
    fn operator_revision_cannot_move_backwards_in_memory() {
        let root = env::temp_dir().join(format!(
            "v2board-operator-monotonic-test-{}-{}",
            std::process::id(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = RuntimePaths {
            config: root.join("config.json"),
            frontend: root.join("frontend"),
            rules: root.join("rules"),
        };
        save_config_atomic(&paths.config, &Map::new()).expect("write boot document");
        let baseline = AppConfig::try_from_api_config_map(Map::new(), paths)
            .expect("boot snapshot")
            .at_operator_revision(9);
        let operator = baseline.operator_config_map();
        let error = match baseline.with_operator_config(&operator, 8) {
            Ok(_) => panic!("revision rollback must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("must not move backwards"));
        fs::remove_dir_all(root).expect("remove monotonic test root");
    }

    #[test]
    fn file_only_documents_are_strictly_bound_to_one_runtime_role() {
        let api = file_only_document(RuntimeRole::Api);
        validate_configuration_source(&api, RuntimeRole::Api, ConfigParseMode::FullRuntime)
            .expect("API document");
        assert!(
            validate_configuration_source(&api, RuntimeRole::Worker, ConfigParseMode::FullRuntime)
                .is_err()
        );

        let mut api_with_worker_secret = api;
        api_with_worker_secret.insert(
            "clickhouse_writer_password".to_string(),
            Value::String("must-not-load".to_string()),
        );
        assert!(
            validate_configuration_source(
                &api_with_worker_secret,
                RuntimeRole::Api,
                ConfigParseMode::FullRuntime,
            )
            .is_err()
        );

        let mut worker = file_only_document(RuntimeRole::Worker);
        validate_configuration_source(&worker, RuntimeRole::Worker, ConfigParseMode::FullRuntime)
            .expect("worker document");
        worker.insert(
            "clickhouse_reader_password".to_string(),
            Value::String("must-not-load".to_string()),
        );
        assert!(
            validate_configuration_source(
                &worker,
                RuntimeRole::Worker,
                ConfigParseMode::FullRuntime,
            )
            .is_err()
        );
    }

    #[test]
    fn boot_only_documents_reject_dynamic_operator_keys_and_full_parser() {
        let paths = RuntimePaths {
            config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
            frontend: PathBuf::from("/tmp/frontend"),
            rules: PathBuf::from("/tmp/rules"),
        };
        let api = boot_only_document(RuntimeRole::Api);
        AppConfig::try_from_api_boot_config_map(api.clone(), paths.clone())
            .expect("exact API bootstrap document");
        assert!(AppConfig::try_from_api_config_map(api.clone(), paths.clone()).is_err());

        let mut leaked_secret = api;
        leaked_secret.insert(
            "telegram_bot_token".to_string(),
            Value::String("must-live-in-database".to_string()),
        );
        assert!(AppConfig::try_from_api_boot_config_map(leaked_secret, paths).is_err());
    }

    #[test]
    fn trusted_proxy_cidrs_parse_ipv4_and_ipv6() {
        let config = serde_json::json!({
            "trusted_proxy_cidrs": ["10.0.0.0/8", "2001:db8::/32"]
        })
        .as_object()
        .expect("object")
        .clone();
        let parsed = parse_trusted_proxy_cidrs(&config).unwrap();
        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].contains(&"10.4.5.6".parse::<std::net::IpAddr>().unwrap()));
        assert!(parsed[1].contains(&"2001:db8::42".parse::<std::net::IpAddr>().unwrap()));

        let invalid = serde_json::json!({ "trusted_proxy_cidrs": ["not-a-cidr"] })
            .as_object()
            .expect("object")
            .clone();
        assert!(parse_trusted_proxy_cidrs(&invalid).is_err());
    }

    #[test]
    fn production_trusts_only_same_host_cloudflared() {
        let loopback = "127.0.0.1/32".parse::<IpNet>().unwrap();
        let private_network = "10.0.0.0/8".parse::<IpNet>().unwrap();

        assert!(
            validate_production_proxy_topology(RuntimeEnvironment::Production, &[loopback]).is_ok()
        );
        assert!(
            validate_production_proxy_topology(RuntimeEnvironment::Production, &[private_network])
                .is_err()
        );
        assert!(
            validate_production_proxy_topology(
                RuntimeEnvironment::Production,
                &[loopback, private_network]
            )
            .is_err()
        );
        assert!(validate_production_proxy_topology(RuntimeEnvironment::Development, &[]).is_ok());
    }

    #[test]
    fn production_datastores_require_verified_transport() {
        const REDIS: &str =
            "rediss://api_runtime:0123456789abcdef0123456789abcdef@cache.example.test/0";

        fn production_clickhouse(url: &str) -> ClickHouseWriterConfig {
            ClickHouseWriterConfig {
                url: url.to_string(),
                database: "v2board_analytics".to_string(),
                username: "v2board_writer".to_string(),
                password: Some("0123456789abcdef0123456789abcdef".to_string()),
            }
        }
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
                "worker",
                Some(&production_clickhouse("https://analytics.example.test")),
                REDIS,
            )
            .is_ok()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "postgresql://api:secret@db.example.test/v2board",
                "worker",
                Some(&production_clickhouse("https://analytics.example.test")),
                REDIS,
            )
            .is_err()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
                "worker",
                Some(&production_clickhouse("http://analytics.example.test")),
                REDIS,
            )
            .is_err()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
                "worker",
                Some(&production_clickhouse("https://analytics.example.test")),
                "redis://cache.example.test/1",
            )
            .is_err()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Local,
                "postgresql://v2board:v2board@postgres/v2board",
                "v2board_worker",
                Some(&ClickHouseWriterConfig {
                    url: "http://clickhouse:8123".to_string(),
                    database: "v2board_analytics".to_string(),
                    username: "v2board_analytics_writer".to_string(),
                    password: None,
                }),
                "redis://redis/1",
            )
            .is_ok()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
                "worker",
                None,
                REDIS,
            )
            .is_ok()
        );
    }

    #[test]
    fn production_redis_url_has_one_canonical_isolated_authority() {
        let valid = url::Url::parse(
            "rediss://api_runtime:0123456789abcdef0123456789abcdef@cache.example.test:6380/0",
        )
        .unwrap();
        assert!(validate_redis_url(&valid, true).is_ok());

        for invalid in [
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test",
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/",
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/01",
            "rediss://:short@cache.example.test/1",
            "rediss://cache.example.test/1",
            "rediss://default:0123456789abcdef0123456789abcdef@cache.example.test/0",
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/%31",
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/1",
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/1?db=2",
            "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/1#other",
        ] {
            let parsed = url::Url::parse(invalid).unwrap();
            assert!(
                validate_redis_url(&parsed, true).is_err(),
                "accepted {invalid}"
            );
        }

        let local = url::Url::parse("redis://redis/0").unwrap();
        assert!(validate_redis_url(&local, false).is_ok());
    }

    #[test]
    fn redis_keyspace_is_bound_to_the_immutable_installation_id() {
        let first = RedisKeyspace::new(Uuid::from_u128(1));
        let second = RedisKeyspace::new(Uuid::from_u128(2));
        assert_eq!(
            first.key("AUTH_SESSION_deadbeef"),
            "v2board:00000000-0000-0000-0000-000000000001:AUTH_SESSION_deadbeef"
        );
        assert_ne!(first.key("shared"), second.key("shared"));
        assert_eq!(
            first.pattern("RUST_SCHEDULER_LOCK_*"),
            first.key("RUST_SCHEDULER_LOCK_*")
        );
    }

    #[test]
    fn postgres_database_identity_is_exact_and_decoded() {
        let encoded = url::Url::parse("postgresql://api@db/%76%32board").unwrap();
        assert_eq!(postgres_database_name(&encoded).unwrap(), "v2board");

        for invalid in [
            "postgresql://api@db/",
            "postgresql://api@db/v2board/",
            "postgresql://api@db/v2-board",
            "postgresql://api@db/%FF",
        ] {
            let url = url::Url::parse(invalid).unwrap();
            assert!(postgres_database_name(&url).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn postgres_query_cannot_override_the_validated_connection() {
        let allowed = url::Url::parse(
            "postgresql://api@db/v2board?sslmode=verify-full&sslrootcert=%2Fcerts%2Fca.pem",
        )
        .unwrap();
        assert!(validate_postgres_connection_query(&allowed, true).is_ok());

        for attack in [
            "sslmode=verify-full&ssl-mode=disable",
            "sslmode=verify-full&host=other.example.test",
            "sslmode=verify-full&hostaddr=127.0.0.1",
            "sslmode=verify-full&port=15432",
            "sslmode=verify-full&dbname=other",
            "sslmode=verify-full&user=shared",
            "sslmode=verify-full&password=other",
            "sslmode=verify-full&sslmode=disable",
            "SSLMODE=verify-full",
            "sslmode=VERIFY-FULL",
            "sslmode=verify-full&h%6fst=other.example.test",
        ] {
            let url = url::Url::parse(&format!("postgresql://api@db/v2board?{attack}")).unwrap();
            assert!(
                validate_postgres_connection_query(&url, true).is_err(),
                "accepted {attack}"
            );
        }
    }

    #[test]
    fn production_and_file_only_require_node_report_idempotency() {
        assert!(
            validate_node_report_contract(RuntimeEnvironment::Production, false, false).is_err()
        );
        assert!(validate_node_report_contract(RuntimeEnvironment::Local, true, false).is_err());
        assert!(validate_node_report_contract(RuntimeEnvironment::Local, false, false).is_ok());
        assert!(validate_node_report_contract(RuntimeEnvironment::Production, false, true).is_ok());
    }

    #[test]
    fn cors_origins_are_canonical_and_never_wildcarded() {
        let explicit = serde_json::json!({
            "cors_allowed_origins": [
                "https://app.example.test",
                "https://app.example.test:443"
            ]
        })
        .as_object()
        .expect("object")
        .clone();
        assert_eq!(
            load_cors_allowed_origins(&explicit, Some("https://ignored.example.test")).unwrap(),
            vec!["https://app.example.test"]
        );

        let default = Map::new();
        assert_eq!(
            load_cors_allowed_origins(
                &default,
                Some("https://app.example.test/admin/?source=deploy")
            )
            .unwrap(),
            vec!["https://app.example.test"]
        );
        let invalid = serde_json::json!({ "cors_allowed_origins": ["*"] })
            .as_object()
            .expect("object")
            .clone();
        assert!(load_cors_allowed_origins(&invalid, None).is_err());
    }

    #[test]
    fn registration_ip_limit_defaults_on_only_in_production() {
        assert!(register_ip_limit_default(RuntimeEnvironment::Production));
        assert!(!register_ip_limit_default(RuntimeEnvironment::Local));
        assert!(!register_ip_limit_default(RuntimeEnvironment::Testing));
    }
}
