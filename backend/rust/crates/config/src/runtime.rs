use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, Utc};

use crate::{keys::MAX_CONFIG_DURATION_MINUTES, validation::env_path};

/// Convert a validated minute setting to seconds. The clamp is a final defense
/// for manually constructed `AppConfig` values in embedders/tests; normal file
/// and environment loading rejects out-of-range values instead.
pub fn duration_minutes_to_seconds(minutes: i64) -> u64 {
    minutes.clamp(1, MAX_CONFIG_DURATION_MINUTES) as u64 * 60
}

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
pub(crate) enum ConfigParseMode {
    FullRuntime,
    BootOnly,
    OperatorAuthority,
}

impl ConfigParseMode {
    pub(crate) const fn is_operator_authority(self) -> bool {
        matches!(self, Self::OperatorAuthority)
    }
}

impl RuntimeRole {
    pub(crate) const fn file_value(self) -> &'static str {
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

impl RuntimePaths {
    pub(crate) fn from_env(runtime_role: RuntimeRole) -> Self {
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
