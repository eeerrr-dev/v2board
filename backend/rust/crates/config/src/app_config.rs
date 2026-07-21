use std::io;

use ipnet::IpNet;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};

use crate::{
    file_io::{crc32b_hex, fnv1a_64, load_config},
    keys::{
        CONFIGURATION_SCOPE_KEY, DEFAULT_PRIVILEGED_SESSION_TTL_SECONDS, OPERATOR_AUTHORITY_MARKER,
        OPERATOR_CONFIG_KEYS_V1,
    },
    runtime::{
        ClickHouseWriterConfig, ConfigParseMode, RuntimeEnvironment, RuntimePaths, RuntimeRole,
    },
    validation::{
        env_path, invalid_setting, json_object, load_cors_allowed_origins,
        parse_trusted_proxy_cidrs, register_ip_limit_default, resolve_app_key,
        validate_datastore_transport, validate_https_configuration, validate_node_report_contract,
        validate_operator_dependencies, validate_production_proxy_topology,
        validate_production_secret, validate_role_environment, validate_scalar_config,
    },
    values::{
        config_bool, config_decimal, config_duration_minutes, config_i32, config_i64, config_list,
        config_or_env, configuration_is_boot_only, configuration_is_file_only,
        deposit_bonus_from_tiers,
    },
};

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
    pub frontend_admin_path: Option<String>,
    pub secure_path: Option<String>,
    /// docs/api-dialect.md §10.3: client-side `#/…` → history-URL translation
    /// toggle, injected into the frontend runtime config. Default ON.
    pub legacy_hash_redirect_enable: bool,
    pub safe_mode_enable: bool,
    /// docs/api-dialect.md §6.10: with this on, an admin/staff session without
    /// an enabled TOTP factor may only reach its own `account/mfa` family —
    /// every other privileged route answers 403 `mfa_enrollment_required`.
    /// Default OFF.
    pub admin_mfa_force: bool,
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
            "frontend_admin_path": self.frontend_admin_path,
            "secure_path": self.secure_path,
            "legacy_hash_redirect_enable": self.legacy_hash_redirect_enable,
            "safe_mode_enable": self.safe_mode_enable,
            "admin_mfa_force": self.admin_mfa_force,
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

    pub(crate) fn try_from_runtime_paths(
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
            admin_mfa_force: config_bool(
                &file_config,
                "admin_mfa_force",
                "V2BOARD_ADMIN_MFA_FORCE",
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
