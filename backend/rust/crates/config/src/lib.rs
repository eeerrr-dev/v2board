use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::{DateTime, FixedOffset, Utc};
use ipnet::IpNet;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::{Map, Value};

/// Operator-entered minute durations are bounded to one year. This is long
/// enough for every subscription/rate-limit use while keeping Redis expiry and
/// timestamp arithmetic inside a small, predictable range.
pub const MAX_CONFIG_DURATION_MINUTES: i64 = 365 * 24 * 60;
const DEFAULT_PRIVILEGED_SESSION_TTL_SECONDS: i64 = 30 * 60;

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

#[derive(Clone, Debug)]
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
    pub environment: RuntimeEnvironment,
    pub bind_addr: String,
    pub database_url: String,
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
    pub legacy_auth_params_enable: bool,
    pub legacy_jwt_cutoff_unix: i64,
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
    pub try_out_plan_id: i32,
    pub try_out_hour: i64,
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
    pub commission_withdraw_limit: i32,
    pub server_token: Option<String>,
    pub server_legacy_token_enable: bool,
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
    pub frontend_custom_html: Option<String>,
    pub frontend_admin_path: Option<String>,
    pub secure_path: Option<String>,
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
    pub fn from_env() -> Self {
        Self::try_from_env().unwrap_or_else(|error| panic!("failed to load native config: {error}"))
    }

    /// Loads a complete configuration snapshot without panicking. Long-lived
    /// processes use this path for hot reloads so a malformed external edit can
    /// be rejected while the last-known-good snapshot remains active.
    pub fn try_from_env() -> io::Result<Self> {
        if let Some(path) = env_path(&["V2BOARD_ENV_PATH", "RUST_ENV_PATH"])
            && path.exists()
        {
            let _ = dotenvy::from_path(path);
        }
        let _ = dotenvy::dotenv();
        Self::try_from_runtime_paths(RuntimePaths::from_env())
    }

    /// Reloads the exact runtime config file used by this snapshot. Environment
    /// overrides remain authoritative, while runtime paths cannot jump to a
    /// different file because another thread mutates process environment.
    pub fn reload(&self) -> io::Result<Self> {
        Self::try_from_runtime_paths(self.runtime_paths.clone())
    }

    fn try_from_runtime_paths(runtime_paths: RuntimePaths) -> io::Result<Self> {
        let file_config = load_config(&runtime_paths.config).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to load {}: {error}", runtime_paths.config.display()),
            )
        })?;
        validate_scalar_config(&file_config)?;
        let environment = RuntimeEnvironment::parse(env_opt("V2BOARD_ENV").as_deref())
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        let app_key = resolve_app_key(environment, env_opt("APP_KEY"))
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        let database_url = env_or("DATABASE_URL", "mysql://v2board:v2board@mysql:3306/v2board");
        let redis_url = env_or("REDIS_URL", "redis://redis:6379/1");
        validate_datastore_transport(environment, &database_url, &redis_url)?;
        let server_token = config_or_env(&file_config, "server_token", "V2BOARD_SERVER_TOKEN");
        validate_production_secret(environment, "server_token", server_token.as_deref())?;

        let trusted_proxy_cidrs = parse_trusted_proxy_cidrs(&file_config)?;

        let force_https = config_bool(
            &file_config,
            "force_https",
            "V2BOARD_FORCE_HTTPS",
            environment.is_production(),
        );
        let app_url = config_or_env(&file_config, "app_url", "APP_URL");
        let cors_allowed_origins = load_cors_allowed_origins(&file_config, app_url.as_deref())?;
        validate_https_configuration(
            force_https,
            app_url.as_deref(),
            !trusted_proxy_cidrs.is_empty(),
        )?;

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

        Ok(Self {
            environment,
            bind_addr: env_or("RUST_BIND_ADDR", "0.0.0.0:8080"),
            database_url,
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
            legacy_auth_params_enable: config_bool(
                &file_config,
                "legacy_auth_params_enable",
                "V2BOARD_LEGACY_AUTH_PARAMS_ENABLE",
                !environment.is_production(),
            ),
            legacy_jwt_cutoff_unix: config_i64(
                &file_config,
                "legacy_jwt_cutoff_unix",
                "V2BOARD_LEGACY_JWT_CUTOFF_UNIX",
                0,
            )
            .max(0),
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
            email_template: config_or_env(&file_config, "email_template", "V2BOARD_EMAIL_TEMPLATE"),
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
            try_out_plan_id: config_i32(
                &file_config,
                "try_out_plan_id",
                "V2BOARD_TRY_OUT_PLAN_ID",
                0,
            ),
            try_out_hour: config_i64(&file_config, "try_out_hour", "V2BOARD_TRY_OUT_HOUR", 1),
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
            commission_withdraw_limit: config_i32(
                &file_config,
                "commission_withdraw_limit",
                "V2BOARD_COMMISSION_WITHDRAW_LIMIT",
                100,
            ),
            server_token,
            server_legacy_token_enable: config_bool(
                &file_config,
                "server_legacy_token_enable",
                "V2BOARD_SERVER_LEGACY_TOKEN_ENABLE",
                !environment.is_production(),
            ),
            server_require_idempotency_key: config_bool(
                &file_config,
                "server_require_idempotency_key",
                "V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY",
                environment.is_production(),
            ),
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
            frontend_custom_html: config_or_env(
                &file_config,
                "frontend_custom_html",
                "V2BOARD_FRONTEND_CUSTOM_HTML",
            ),
            frontend_admin_path: config_or_env(
                &file_config,
                "frontend_admin_path",
                "V2BOARD_FRONTEND_ADMIN_PATH",
            ),
            secure_path: config_or_env(&file_config, "secure_path", "V2BOARD_SECURE_PATH"),
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
        })
    }

    pub fn subscribe_url_for_token(&self, token: &str) -> String {
        let path = if self.subscribe_path.trim().is_empty() {
            "/api/v1/client/subscribe"
        } else {
            self.subscribe_path.as_str()
        };
        let path_with_token = format!("{path}?token={token}");
        let configured_url = self
            .subscribe_url
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .find(|value| !value.is_empty());

        if let Some(base) = configured_url {
            return format!("{}{}", base.trim_end_matches('/'), path_with_token);
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

    pub fn admin_api_route(&self) -> String {
        format!("/api/v1/{}/{{*admin_path}}", self.admin_path())
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
        validate_https_configuration(force_https, app_url, !self.trusted_proxy_cidrs.is_empty())
    }
}

impl RuntimePaths {
    fn from_env() -> Self {
        let root = env_path(&["V2BOARD_RUNTIME_ROOT", "RUST_RUNTIME_ROOT"])
            .unwrap_or_else(|| PathBuf::from("/var/lib/v2board"));
        let frontend = env_path(&["V2BOARD_FRONTEND_DIR", "RUST_FRONTEND_DIR"])
            .unwrap_or_else(|| PathBuf::from("/opt/v2board/frontend"));

        Self {
            config: env_path(&["V2BOARD_CONFIG_PATH", "RUST_CONFIG_PATH"])
                .unwrap_or_else(|| root.join("config/config.json")),
            frontend,
            rules: env_path(&["V2BOARD_RULE_DIR", "RUST_RULE_DIR"])
                .unwrap_or_else(|| root.join("rules")),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty() && value != "null")
        .unwrap_or_else(|| default.to_string())
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

fn env_path(keys: &[&str]) -> Option<PathBuf> {
    keys.iter().find_map(|key| env_opt(key).map(PathBuf::from))
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
    force_https: bool,
    app_url: Option<&str>,
    has_trusted_proxy: bool,
) -> io::Result<()> {
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
            "trusted_proxy_cidrs is required when force_https is enabled",
        ));
    }
    Ok(())
}

/// Production database credentials and application data must not cross the
/// network in clear text. Local/development Compose intentionally remains on
/// its isolated plaintext network; production uses the TLS URL forms already
/// supported by the selected Redis and SQLx rustls features.
fn validate_datastore_transport(
    environment: RuntimeEnvironment,
    database_url: &str,
    redis_url: &str,
) -> io::Result<()> {
    let database = url::Url::parse(database_url)
        .map_err(|_| invalid_setting("DATABASE_URL", "must be a valid MySQL URL"))?;
    if database.scheme() != "mysql" {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must use the mysql URL scheme",
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
    if !environment.is_production() {
        return Ok(());
    }

    if redis.scheme() != "rediss" {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use rediss:// with certificate verification in production",
        ));
    }
    let verify_identity = database.query_pairs().any(|(key, value)| {
        key.eq_ignore_ascii_case("ssl-mode")
            && matches!(
                value.to_ascii_lowercase().replace('-', "_").as_str(),
                "verify_identity"
            )
    });
    if !verify_identity {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must set ssl-mode=VERIFY_IDENTITY in production",
        ));
    }
    Ok(())
}

fn load_cors_allowed_origins(
    config: &Map<String, Value>,
    app_url: Option<&str>,
) -> io::Result<Vec<String>> {
    let configured = env_opt("V2BOARD_CORS_ALLOWED_ORIGINS")
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
fn validate_scalar_config(config: &Map<String, Value>) -> io::Result<()> {
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
        ("plan_change_enable", "V2BOARD_PLAN_CHANGE_ENABLE"),
        ("surplus_enable", "V2BOARD_SURPLUS_ENABLE"),
        (
            "commission_first_time_enable",
            "V2BOARD_COMMISSION_FIRST_TIME_ENABLE",
        ),
        ("server_log_enable", "V2BOARD_SERVER_LOG_ENABLE"),
        ("safe_mode_enable", "V2BOARD_SAFE_MODE_ENABLE"),
        ("password_limit_enable", "V2BOARD_PASSWORD_LIMIT_ENABLE"),
        (
            "legacy_auth_params_enable",
            "V2BOARD_LEGACY_AUTH_PARAMS_ENABLE",
        ),
        (
            "privileged_step_up_enable",
            "V2BOARD_PRIVILEGED_STEP_UP_ENABLE",
        ),
        (
            "server_legacy_token_enable",
            "V2BOARD_SERVER_LEGACY_TOKEN_ENABLE",
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
        ("legacy_jwt_cutoff_unix", "V2BOARD_LEGACY_JWT_CUTOFF_UNIX"),
        ("email_port", "V2BOARD_EMAIL_PORT"),
        ("register_limit_count", "V2BOARD_REGISTER_LIMIT_COUNT"),
        ("register_limit_expire", "V2BOARD_REGISTER_LIMIT_EXPIRE"),
        ("show_subscribe_method", "V2BOARD_SHOW_SUBSCRIBE_METHOD"),
        ("show_subscribe_expire", "V2BOARD_SHOW_SUBSCRIBE_EXPIRE"),
        ("allow_new_period", "V2BOARD_ALLOW_NEW_PERIOD"),
        ("reset_traffic_method", "V2BOARD_RESET_TRAFFIC_METHOD"),
        ("try_out_plan_id", "V2BOARD_TRY_OUT_PLAN_ID"),
        ("try_out_hour", "V2BOARD_TRY_OUT_HOUR"),
        ("invite_commission", "V2BOARD_INVITE_COMMISSION"),
        ("new_order_event_id", "V2BOARD_NEW_ORDER_EVENT_ID"),
        ("renew_order_event_id", "V2BOARD_RENEW_ORDER_EVENT_ID"),
        ("change_order_event_id", "V2BOARD_CHANGE_ORDER_EVENT_ID"),
        ("invite_gen_limit", "V2BOARD_INVITE_GEN_LIMIT"),
        ("ticket_status", "V2BOARD_TICKET_STATUS"),
        (
            "commission_withdraw_limit",
            "V2BOARD_COMMISSION_WITHDRAW_LIMIT",
        ),
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
        "commission_withdraw_limit",
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
    validate_integer_range(
        config,
        "legacy_jwt_cutoff_unix",
        "V2BOARD_LEGACY_JWT_CUTOFF_UNIX",
        0,
        i64::MAX,
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
    if let Some(value) = env_opt(env_key) {
        return Ok(Some(value));
    }
    let Some(value) = config.get(config_key) else {
        return Ok(None);
    };
    match value {
        Value::String(value) => Ok(Some(value.trim().to_string())),
        Value::Number(value) => Ok(Some(value.to_string())),
        Value::Bool(value) => Ok(Some(if *value { "true" } else { "false" }.to_string())),
        Value::Null | Value::Array(_) | Value::Object(_) => Err(invalid_setting(
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

/// Reads the native runtime configuration. A missing file is an empty document;
/// malformed JSON and non-object roots are surfaced to callers instead of being
/// interpreted as partially valid configuration.
pub fn load_config(path: impl AsRef<Path>) -> io::Result<Map<String, Value>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error),
    };
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "config root must be an object"))
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

fn config_or_env(config: &Map<String, Value>, config_key: &str, env_key: &str) -> Option<String> {
    env_opt(env_key).or_else(|| {
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
    env_opt(env_key)
        .map(|value| parse_list(&value))
        .or_else(|| {
            config.get(config_key).map(|value| match value {
                Value::Array(items) => items.iter().filter_map(config_value_string).collect(),
                value => config_value_string(value)
                    .map(|value| parse_list(&value))
                    .unwrap_or_default(),
            })
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| default.iter().map(|item| (*item).to_string()).collect())
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
        let error = validate_scalar_config(&invalid_bool).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("recaptcha_enable"));

        let invalid_integer = serde_json::json!({ "password_limit_count": "many" })
            .as_object()
            .expect("object")
            .clone();
        assert!(validate_scalar_config(&invalid_integer).is_err());

        let out_of_range = serde_json::json!({ "auth_session_max_per_user": 101 })
            .as_object()
            .expect("object")
            .clone();
        assert!(validate_scalar_config(&out_of_range).is_err());

        let overflowing_i32 = serde_json::json!({ "server_push_interval": 2147483648_i64 })
            .as_object()
            .expect("object")
            .clone();
        assert!(validate_scalar_config(&overflowing_i32).is_err());

        let structural = serde_json::json!({ "force_https": [] })
            .as_object()
            .expect("object")
            .clone();
        assert!(validate_scalar_config(&structural).is_err());
    }

    #[test]
    fn force_https_requires_a_canonical_https_app_url() {
        assert!(validate_https_configuration(false, None, false).is_ok());
        assert!(validate_https_configuration(true, None, true).is_err());
        assert!(validate_https_configuration(true, Some("http://example.com"), true).is_err());
        assert!(
            validate_https_configuration(true, Some("https://user@example.com"), true).is_err()
        );
        assert!(validate_https_configuration(true, Some("https://example.com"), false).is_err());
        assert!(validate_https_configuration(true, Some("https://example.com"), true).is_ok());
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
        let snapshot = AppConfig::try_from_runtime_paths(runtime_paths).expect("initial snapshot");
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
    fn production_datastores_require_verified_transport() {
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "mysql://user:secret@db.example.test/v2board?ssl-mode=VERIFY_IDENTITY",
                "rediss://cache.example.test/1",
            )
            .is_ok()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "mysql://user:secret@db.example.test/v2board",
                "rediss://cache.example.test/1",
            )
            .is_err()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Production,
                "mysql://user:secret@db.example.test/v2board?ssl-mode=VERIFY_IDENTITY",
                "redis://cache.example.test/1",
            )
            .is_err()
        );
        assert!(
            validate_datastore_transport(
                RuntimeEnvironment::Local,
                "mysql://v2board:v2board@mysql/v2board",
                "redis://redis/1",
            )
            .is_ok()
        );
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
