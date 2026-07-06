use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct RuntimePaths {
    pub v2board_config: PathBuf,
    pub mail_templates: PathBuf,
    pub themes: PathBuf,
    pub theme_configs: PathBuf,
    pub rules: PathBuf,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub redis_url: String,
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
    pub stripe_pk_live: Option<String>,
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
    pub server_api_url: Option<String>,
    pub server_push_interval: i32,
    pub server_pull_interval: i32,
    pub server_node_report_min_traffic: i32,
    pub server_device_online_min_traffic: i32,
    pub device_limit_mode: i32,
    pub frontend_theme: String,
    pub frontend_theme_sidebar: Option<String>,
    pub frontend_theme_header: Option<String>,
    pub frontend_theme_color: Option<String>,
    pub frontend_background_url: Option<String>,
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
        if let Some(path) = env_path(&["V2BOARD_ENV_PATH", "RUST_ENV_PATH", "LARAVEL_ENV_PATH"])
            && path.exists()
        {
            let _ = dotenvy::from_path(path);
        }
        let _ = dotenvy::dotenv();
        let runtime_paths = RuntimePaths::from_env();
        let file_config = load_php_config(&runtime_paths.v2board_config);

        Self {
            bind_addr: env_or("RUST_BIND_ADDR", "0.0.0.0:8080"),
            database_url: env_or("DATABASE_URL", "mysql://v2board:v2board@mysql:3306/v2board"),
            redis_url: env_or("REDIS_URL", "redis://redis:6379/1"),
            runtime_paths,
            app_key: env_or("APP_KEY", "local-rust-dev-key"),
            app_name: config_or_env(&file_config, "app_name", "V2BOARD_APP_NAME")
                .unwrap_or_else(|| "V2Board".to_string()),
            app_url: config_or_env(&file_config, "app_url", "APP_URL"),
            app_description: config_or_env(
                &file_config,
                "app_description",
                "V2BOARD_APP_DESCRIPTION",
            )
            .or_else(|| Some("V2Board is best".to_string())),
            logo: config_or_env(&file_config, "logo", "V2BOARD_LOGO"),
            tos_url: config_or_env(&file_config, "tos_url", "V2BOARD_TOS_URL"),
            force_https: config_bool(&file_config, "force_https", "V2BOARD_FORCE_HTTPS", false),
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
                &["gmail.com"],
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
                false,
            ),
            register_limit_count: config_i64(
                &file_config,
                "register_limit_count",
                "V2BOARD_REGISTER_LIMIT_COUNT",
                3,
            ),
            register_limit_expire: config_i64(
                &file_config,
                "register_limit_expire",
                "V2BOARD_REGISTER_LIMIT_EXPIRE",
                60,
            ),
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
            stripe_pk_live: config_or_env(&file_config, "stripe_pk_live", "V2BOARD_STRIPE_PK_LIVE"),
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
            show_subscribe_expire: config_i64(
                &file_config,
                "show_subscribe_expire",
                "V2BOARD_SHOW_SUBSCRIBE_EXPIRE",
                5,
            ),
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
            server_token: config_or_env(&file_config, "server_token", "V2BOARD_SERVER_TOKEN"),
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
            frontend_theme: config_or_env(&file_config, "frontend_theme", "V2BOARD_FRONTEND_THEME")
                .unwrap_or_else(|| "v2board".to_string()),
            frontend_theme_sidebar: config_or_env(
                &file_config,
                "frontend_theme_sidebar",
                "V2BOARD_FRONTEND_THEME_SIDEBAR",
            )
            .or_else(|| Some("light".to_string())),
            frontend_theme_header: config_or_env(
                &file_config,
                "frontend_theme_header",
                "V2BOARD_FRONTEND_THEME_HEADER",
            )
            .or_else(|| Some("dark".to_string())),
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
            password_limit_expire: config_i64(
                &file_config,
                "password_limit_expire",
                "V2BOARD_PASSWORD_LIMIT_EXPIRE",
                60,
            ),
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
        }
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
}

impl RuntimePaths {
    fn from_env() -> Self {
        let root = env_path(&["V2BOARD_RUNTIME_ROOT", "RUST_RUNTIME_ROOT"]);
        let legacy_root = env_path(&["LARAVEL_ROOT"]).unwrap_or_else(|| PathBuf::from("/laravel"));
        let env_file = env_path(&["V2BOARD_ENV_PATH", "RUST_ENV_PATH", "LARAVEL_ENV_PATH"]);

        Self {
            v2board_config: env_path(&[
                "V2BOARD_CONFIG_PATH",
                "RUST_CONFIG_PATH",
                "LARAVEL_CONFIG_PATH",
            ])
            .or_else(|| {
                env_file
                    .as_deref()
                    .and_then(Path::parent)
                    .map(|parent| parent.join("config/v2board.php"))
            })
            .or_else(|| root.as_ref().map(|root| root.join("config/v2board.php")))
            .unwrap_or_else(|| legacy_root.join("config/v2board.php")),
            mail_templates: env_path(&["V2BOARD_MAIL_TEMPLATE_DIR", "RUST_MAIL_TEMPLATE_DIR"])
                .or_else(|| root.as_ref().map(|root| root.join("resources/views/mail")))
                .unwrap_or_else(|| legacy_root.join("resources/views/mail")),
            themes: env_path(&["V2BOARD_THEME_DIR", "RUST_THEME_DIR"])
                .or_else(|| root.as_ref().map(|root| root.join("public/theme")))
                .unwrap_or_else(|| legacy_root.join("public/theme")),
            theme_configs: env_path(&["V2BOARD_THEME_CONFIG_DIR", "RUST_THEME_CONFIG_DIR"])
                .or_else(|| root.as_ref().map(|root| root.join("config/theme")))
                .unwrap_or_else(|| legacy_root.join("config/theme")),
            rules: env_path(&["V2BOARD_RULE_DIR", "RUST_RULE_DIR"])
                .or_else(|| root.as_ref().map(|root| root.join("resources/rules")))
                .unwrap_or_else(|| legacy_root.join("resources/rules")),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty() && value != "null")
        .unwrap_or_else(|| default.to_string())
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

fn load_php_config(path: &Path) -> HashMap<String, String> {
    let Ok(content) = fs::read_to_string(path) else {
        return HashMap::new();
    };

    let lines = content.lines().collect::<Vec<_>>();
    let mut values = HashMap::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        index += 1;
        if !line.starts_with('\'') || !line.contains("=>") {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once("=>") else {
            continue;
        };
        let key = raw_key.trim().trim_matches('\'');
        if key.is_empty() {
            continue;
        }
        let raw_value = raw_value.trim().trim_end_matches(',');
        if raw_value.is_empty() || raw_value.starts_with("array") {
            let (items, next_index) = parse_php_array_lines(&lines, index, raw_value);
            index = next_index;
            values.insert(key.to_string(), items.join(","));
            continue;
        }
        let value = parse_php_scalar(raw_value);
        if let Some(value) = value {
            values.insert(key.to_string(), value);
        }
    }
    values
}

fn parse_php_array_lines(
    lines: &[&str],
    mut index: usize,
    first_value: &str,
) -> (Vec<String>, usize) {
    let mut depth = usize::from(first_value.starts_with("array"));
    if depth == 0 {
        while index < lines.len() {
            let line = lines[index].trim();
            index += 1;
            if line.starts_with("array") {
                depth = 1;
                break;
            }
            if !line.is_empty() {
                return (Vec::new(), index);
            }
        }
    }

    let mut items = Vec::new();
    while index < lines.len() && depth > 0 {
        let line = lines[index].trim().trim_end_matches(',');
        index += 1;
        if line.starts_with("array") {
            depth += 1;
            continue;
        }
        if line.starts_with(')') {
            depth = depth.saturating_sub(1);
            continue;
        }
        if depth == 1 {
            let raw_value = line
                .split_once("=>")
                .map(|(_, value)| value.trim())
                .unwrap_or(line);
            if let Some(value) = parse_php_scalar(raw_value) {
                items.push(value);
            }
        }
    }
    (items, index)
}

fn crc32b_hex(bytes: &[u8]) -> String {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    format!("{:08x}", !crc)
}

fn parse_php_scalar(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches(';').trim();
    if value.eq_ignore_ascii_case("null") {
        return None;
    }
    if value.eq_ignore_ascii_case("true") {
        return Some("1".to_string());
    }
    if value.eq_ignore_ascii_case("false") {
        return Some("0".to_string());
    }
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Some(
            value[1..value.len() - 1]
                .replace("\\'", "'")
                .replace("\\\\", "\\"),
        );
    }
    Some(value.to_string())
}

fn config_or_env(
    config: &HashMap<String, String>,
    config_key: &str,
    env_key: &str,
) -> Option<String> {
    env_opt(env_key).or_else(|| {
        config
            .get(config_key)
            .cloned()
            .filter(|value| !value.is_empty())
    })
}

fn config_bool(
    config: &HashMap<String, String>,
    config_key: &str,
    env_key: &str,
    default: bool,
) -> bool {
    config_or_env(config, config_key, env_key)
        .as_deref()
        .map(parse_bool)
        .unwrap_or(default)
}

fn config_i32(
    config: &HashMap<String, String>,
    config_key: &str,
    env_key: &str,
    default: i32,
) -> i32 {
    config_or_env(config, config_key, env_key)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
}

fn config_i64(
    config: &HashMap<String, String>,
    config_key: &str,
    env_key: &str,
    default: i64,
) -> i64 {
    config_or_env(config, config_key, env_key)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

fn config_list(
    config: &HashMap<String, String>,
    config_key: &str,
    env_key: &str,
    default: &[&str],
) -> Vec<String> {
    config_or_env(config, config_key, env_key)
        .map(|value| parse_list(&value))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| default.iter().map(|item| (*item).to_string()).collect())
}

fn parse_bool(value: &str) -> bool {
    matches!(value, "1" | "true" | "TRUE" | "yes" | "YES")
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_path_fallback_matches_php_crc32b() {
        assert_eq!(crc32b_hex(b"test"), "d87f7e0c");
    }

    #[test]
    fn php_var_export_arrays_parse_as_lists() {
        let lines = [
            "array (",
            "  0 => 'gmail.com',",
            "  1 => 'example.com',",
            "),",
        ];
        let (items, index) = parse_php_array_lines(&lines, 1, "array (");
        assert_eq!(items, vec!["gmail.com", "example.com"]);
        assert_eq!(index, lines.len());
    }
}
