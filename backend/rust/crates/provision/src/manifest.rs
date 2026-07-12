use std::{
    collections::BTreeSet,
    fs, io,
    io::Read,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};

use hmac::{Hmac, KeyInit, Mac};
use ipnet::IpNet;
use percent_encoding::percent_decode_str;
use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use sha2::Sha256;
use url::Url;
use uuid::Uuid;
use v2board_config::{
    AppConfig, FILE_ONLY_RUNTIME_KEYS_V1, MAX_CONFIG_DURATION_MINUTES, RuntimePaths,
};

pub const LEGACY_REFERENCE_COMMIT: &str = "7e77de9f4873b317157490529f7be7d6f8a62421";
const MAX_SPEC_BYTES: u64 = 1024 * 1024;

// V2 deliberately requires a complete runtime document. Adding a new runtime
// setting requires a new spec version or an explicit compatibility decision;
// a legacy migration must never acquire new behavior from an implicit default.
const RUNTIME_KEYS_V1: &[&str] = FILE_ONLY_RUNTIME_KEYS_V1;

const BOOL_RUNTIME_KEYS: &[&str] = &[
    "privileged_step_up_enable",
    "legacy_auth_params_enable",
    "force_https",
    "email_verify",
    "stop_register",
    "invite_force",
    "invite_never_expire",
    "email_whitelist_enable",
    "email_gmail_limit_enable",
    "recaptcha_enable",
    "register_limit_by_ip_enable",
    "telegram_bot_enable",
    "withdraw_close_enable",
    "commission_distribution_enable",
    "commission_auto_check_enable",
    "show_info_to_server_enable",
    "plan_change_enable",
    "surplus_enable",
    "commission_first_time_enable",
    "server_legacy_token_enable",
    "server_require_idempotency_key",
    "server_log_enable",
    "safe_mode_enable",
    "password_limit_enable",
];

const INTEGER_RUNTIME_KEYS: &[&str] = &[
    "http_connect_timeout_seconds",
    "http_request_timeout_seconds",
    "api_request_timeout_seconds",
    "password_kdf_max_parallel",
    "auth_session_ttl_seconds",
    "privileged_auth_session_ttl_seconds",
    "auth_session_max_per_user",
    "privileged_step_up_ttl_seconds",
    "privileged_step_up_max_attempts",
    "privileged_step_up_attempt_window_seconds",
    "legacy_jwt_cutoff_unix",
    "email_port",
    "register_limit_count",
    "register_limit_expire",
    "show_subscribe_method",
    "show_subscribe_expire",
    "allow_new_period",
    "reset_traffic_method",
    "try_out_enable",
    "try_out_plan_id",
    "try_out_hour",
    "invite_commission",
    "new_order_event_id",
    "renew_order_event_id",
    "change_order_event_id",
    "invite_gen_limit",
    "ticket_status",
    "commission_withdraw_limit",
    "server_push_interval",
    "server_pull_interval",
    "server_node_report_min_traffic",
    "server_device_online_min_traffic",
    "device_limit_mode",
    "password_limit_count",
    "password_limit_expire",
];

const LIST_RUNTIME_KEYS: &[&str] = &[
    "cors_allowed_origins",
    "trusted_proxy_cidrs",
    "email_whitelist_suffix",
    "commission_withdraw_method",
    "deposit_bounus",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionSpec {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: ProvisionKind,
    pub reference_commit: String,
    lifecycle_audit_key: String,
    pub source: SourceSpec,
    pub target: TargetSpec,
    pub runtime: Map<String, Value>,
    pub decisions: DecisionSpec,
    pub attestations: AttestationSpec,
    #[serde(skip)]
    manifest_binding_hmac_sha256: String,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionKind {
    LegacyReferenceMigration,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    pub database_url: String,
    pub redis_default_url: String,
    pub redis_cache_url: String,
    pub redis_connection_prefix: String,
    pub redis_cache_prefix: String,
    pub legacy_cache_driver: LegacyCacheDriver,
    pub legacy_show_subscribe_method: i32,
    pub legacy_show_subscribe_expire_minutes: i64,
    pub legacy_subscription_issuance_stopped_at_unix: i64,
    pub transport_security: SourceTransportSecurity,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyCacheDriver {
    Redis,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceTransportSecurity {
    VerifiedTls,
    TrustedMaintenanceNetwork,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    pub bootstrap_database_url: String,
    pub application_database_url: String,
    pub application_account_host: String,
    pub redis_url: String,
    pub runtime_config_path: PathBuf,
    pub require_database_absent: bool,
    pub require_account_absent: bool,
    pub require_empty_redis: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionSpec {
    pub legacy_configuration: LegacyConfigurationDecision,
    pub sessions: SessionDecision,
    pub legacy_cache: LegacyCacheDecision,
    pub legacy_stripe: LegacyStripeDecision,
    pub legacy_subscription_tokens: LegacySubscriptionTokenDecision,
    pub nodes: NodeDecision,
    pub legacy_theme: LegacyThemeDecision,
    pub legacy_custom_rules: LegacyCustomRulesDecision,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyConfigurationDecision {
    ManualOnly,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionDecision {
    LogoutAll,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyCacheDecision {
    DiscardEphemeralAfterFence,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStripeDecision {
    AssertNone,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacySubscriptionTokenDecision {
    AssertNone,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeDecision {
    MaintenanceCutover,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyThemeDecision {
    DiscardConfirmed,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyCustomRulesDecision {
    None,
    DiscardConfirmed,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttestationSpec {
    pub source_writers_stopped: bool,
    pub source_workers_stopped: bool,
    pub node_reporters_stopped: bool,
    pub legacy_queues_drained: bool,
    pub backup_reference: Option<String>,
    pub restore_tested: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProvisionSpecError {
    #[error("cannot inspect provision spec metadata: {0}")]
    Metadata(#[source] io::Error),
    #[error("provision spec must be a regular non-symlink file")]
    UnsafeFileType,
    #[error("provision spec contains secrets and must not grant group or world permissions")]
    UnsafePermissions,
    #[error("provision spec must be between 1 byte and 1 MiB")]
    UnsafeSize,
    #[error("cannot read provision spec: {0}")]
    Read(#[source] io::Error),
    #[error("provision spec is not valid strict JSON: {0}")]
    Json(#[source] serde_json::Error),
    #[error("unsupported provision spec schema_version; expected 2")]
    SchemaVersion,
    #[error("operation_id must be a UUID")]
    OperationId,
    #[error("reference_commit does not match the pinned legacy source")]
    ReferenceCommit,
    #[error("{0}")]
    Validation(&'static str),
    #[error("runtime config is incomplete; missing keys: {0}")]
    MissingRuntimeKeys(String),
    #[error("runtime config contains unsupported keys: {0}")]
    UnknownRuntimeKeys(String),
    #[error("runtime config key {0} has the wrong JSON type")]
    RuntimeType(String),
    #[error("runtime config is not loadable by the native application: {0}")]
    RuntimeSemantics(String),
}

pub fn load_provision_spec(path: impl AsRef<Path>) -> Result<ProvisionSpec, ProvisionSpecError> {
    let path = path.as_ref();
    let mut file = fs::File::open(path).map_err(ProvisionSpecError::Metadata)?;
    let metadata = file.metadata().map_err(ProvisionSpecError::Metadata)?;
    let path_metadata = fs::symlink_metadata(path).map_err(ProvisionSpecError::Metadata)?;
    if !metadata.file_type().is_file()
        || !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
    {
        return Err(ProvisionSpecError::UnsafeFileType);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.dev() != path_metadata.dev() || metadata.ino() != path_metadata.ino() {
            return Err(ProvisionSpecError::UnsafeFileType);
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ProvisionSpecError::UnsafePermissions);
        }
    }
    if metadata.len() == 0 || metadata.len() > MAX_SPEC_BYTES {
        return Err(ProvisionSpecError::UnsafeSize);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(ProvisionSpecError::Read)?;
    let unique = serde_json::from_slice::<UniqueJson>(&bytes).map_err(ProvisionSpecError::Json)?;
    let mut spec =
        serde_json::from_value::<ProvisionSpec>(unique.0).map_err(ProvisionSpecError::Json)?;
    validate_spec(&spec)?;
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(spec.lifecycle_audit_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(b"v2board-provision-manifest-v2\0");
    mac.update(&bytes);
    spec.manifest_binding_hmac_sha256 = hex::encode(mac.finalize().into_bytes());
    Ok(spec)
}

fn validate_spec(spec: &ProvisionSpec) -> Result<(), ProvisionSpecError> {
    if spec.schema_version != 2 {
        return Err(ProvisionSpecError::SchemaVersion);
    }
    Uuid::parse_str(&spec.operation_id).map_err(|_| ProvisionSpecError::OperationId)?;
    if spec.reference_commit != LEGACY_REFERENCE_COMMIT {
        return Err(ProvisionSpecError::ReferenceCommit);
    }
    if spec.target.runtime_config_path != Path::new("/var/lib/v2board/config/config.json") {
        return Err(ProvisionSpecError::Validation(
            "target.runtime_config_path must be /var/lib/v2board/config/config.json in v2",
        ));
    }
    if !spec.target.require_database_absent
        || !spec.target.require_account_absent
        || !spec.target.require_empty_redis
    {
        return Err(ProvisionSpecError::Validation(
            "target application database and account must be declared absent and target Redis must be declared empty",
        ));
    }
    validate_mysql_url(&spec.source.database_url, "source.database_url")?;
    validate_mysql_url(
        &spec.target.bootstrap_database_url,
        "target.bootstrap_database_url",
    )?;
    validate_mysql_url(
        &spec.target.application_database_url,
        "target.application_database_url",
    )?;
    validate_target_mysql_urls(&spec.target)?;
    validate_redis_url(&spec.source.redis_default_url, "source.redis_default_url")?;
    validate_redis_url(&spec.source.redis_cache_url, "source.redis_cache_url")?;
    validate_redis_url(&spec.target.redis_url, "target.redis_url")?;
    if [
        &spec.source.redis_connection_prefix,
        &spec.source.redis_cache_prefix,
    ]
    .iter()
    .any(|prefix| {
        prefix.chars().any(|character| {
            character.is_control() || matches!(character, '*' | '?' | '[' | ']' | '\\')
        })
    }) {
        return Err(ProvisionSpecError::Validation(
            "source Redis prefixes must not contain glob or control characters",
        ));
    }
    if spec.source.transport_security == SourceTransportSecurity::VerifiedTls
        && (!mysql_url_verifies_identity(&spec.source.database_url)
            || !redis_url_uses_tls(&spec.source.redis_default_url)
            || !redis_url_uses_tls(&spec.source.redis_cache_url))
    {
        return Err(ProvisionSpecError::Validation(
            "source verified_tls requires MySQL VERIFY_IDENTITY and rediss:// for both Redis databases",
        ));
    }
    let source_database_identity = datastore_identity(&spec.source.database_url)?;
    if source_database_identity == datastore_identity(&spec.target.application_database_url)? {
        return Err(ProvisionSpecError::Validation(
            "source and target application databases must be different",
        ));
    }
    if source_database_identity == datastore_identity(&spec.target.bootstrap_database_url)? {
        return Err(ProvisionSpecError::Validation(
            "target bootstrap database must not be the source database",
        ));
    }
    if !(0..=2).contains(&spec.source.legacy_show_subscribe_method) {
        return Err(ProvisionSpecError::Validation(
            "source.legacy_show_subscribe_method must be 0, 1, or 2",
        ));
    }
    if !(1..=525_600).contains(&spec.source.legacy_show_subscribe_expire_minutes) {
        return Err(ProvisionSpecError::Validation(
            "source.legacy_show_subscribe_expire_minutes must be between 1 and 525600",
        ));
    }
    let target_redis_identity = datastore_identity(&spec.target.redis_url)?;
    if datastore_identity(&spec.source.redis_default_url)? == target_redis_identity
        || datastore_identity(&spec.source.redis_cache_url)? == target_redis_identity
    {
        return Err(ProvisionSpecError::Validation(
            "both source Redis databases must be different from target Redis",
        ));
    }
    if spec
        .attestations
        .backup_reference
        .as_deref()
        .is_some_and(|reference| is_placeholder(reference, 8))
    {
        return Err(ProvisionSpecError::Validation(
            "attestations.backup_reference must be an explicit, non-placeholder snapshot identifier",
        ));
    }
    validate_runtime(&spec.runtime)?;
    if is_placeholder(&spec.lifecycle_audit_key, 32) {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be an independent non-placeholder secret of at least 32 bytes",
        ));
    }
    if ["app_key", "server_token"].iter().any(|key| {
        spec.runtime.get(*key).and_then(Value::as_str) == Some(spec.lifecycle_audit_key.as_str())
    }) {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be different from runtime.app_key and runtime.server_token",
        ));
    }
    let target_mysql_password_reuses_audit_key = [
        &spec.target.bootstrap_database_url,
        &spec.target.application_database_url,
    ]
    .iter()
    .any(|value| {
        Url::parse(value)
            .expect("validated target MySQL URL")
            .password()
            .map(strict_percent_decode)
            .transpose()
            .expect("validated target MySQL password encoding")
            .as_deref()
            == Some(spec.lifecycle_audit_key.as_str())
    });
    let target_redis_password_reuses_audit_key = Url::parse(&spec.target.redis_url)
        .expect("validated target Redis URL")
        .password()
        .map(strict_percent_decode)
        .transpose()
        .expect("validated target Redis password encoding")
        .as_deref()
        == Some(spec.lifecycle_audit_key.as_str());
    if target_mysql_password_reuses_audit_key || target_redis_password_reuses_audit_key {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be different from target datastore passwords",
        ));
    }
    AppConfig::try_from_config_map(
        spec.materialized_runtime_config(),
        RuntimePaths {
            config: spec.target.runtime_config_path.clone(),
            frontend: PathBuf::from("/opt/v2board/frontend"),
            rules: PathBuf::from("/var/lib/v2board/rules"),
        },
    )
    .map_err(|error| ProvisionSpecError::RuntimeSemantics(error.to_string()))?;
    Ok(())
}

fn validate_runtime(runtime: &Map<String, Value>) -> Result<(), ProvisionSpecError> {
    let expected = RUNTIME_KEYS_V1.iter().copied().collect::<BTreeSet<_>>();
    let actual = runtime.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ProvisionSpecError::MissingRuntimeKeys(missing.join(", ")));
    }
    let unknown = actual.difference(&expected).copied().collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(ProvisionSpecError::UnknownRuntimeKeys(unknown.join(", ")));
    }
    if runtime.get("environment").and_then(Value::as_str) != Some("production") {
        return Err(ProvisionSpecError::Validation(
            "runtime.environment must be production",
        ));
    }
    if runtime.get("configuration_source").and_then(Value::as_str) != Some("file_only") {
        return Err(ProvisionSpecError::Validation(
            "runtime.configuration_source must be file_only",
        ));
    }
    for key in BOOL_RUNTIME_KEYS {
        if !runtime.get(*key).is_some_and(Value::is_boolean) {
            return Err(ProvisionSpecError::RuntimeType((*key).to_string()));
        }
    }
    for key in INTEGER_RUNTIME_KEYS {
        let value = runtime.get(*key);
        if *key == "email_port" && value.is_some_and(Value::is_null) {
            continue;
        }
        if !value.is_some_and(|value| value.as_i64().is_some()) {
            return Err(ProvisionSpecError::RuntimeType((*key).to_string()));
        }
    }
    for key in LIST_RUNTIME_KEYS {
        if !runtime.get(*key).is_some_and(|value| {
            value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string))
        }) {
            return Err(ProvisionSpecError::RuntimeType((*key).to_string()));
        }
    }
    for (key, value) in runtime {
        if BOOL_RUNTIME_KEYS.contains(&key.as_str())
            || INTEGER_RUNTIME_KEYS.contains(&key.as_str())
            || LIST_RUNTIME_KEYS.contains(&key.as_str())
        {
            continue;
        }
        if !value.is_null() && !value.is_string() {
            return Err(ProvisionSpecError::RuntimeType(key.clone()));
        }
    }
    if runtime.get("legacy_auth_params_enable") != Some(&Value::Bool(false))
        || runtime
            .get("legacy_jwt_cutoff_unix")
            .and_then(Value::as_i64)
            != Some(0)
    {
        return Err(ProvisionSpecError::Validation(
            "logout_all requires legacy_auth_params_enable=false and legacy_jwt_cutoff_unix=0",
        ));
    }
    if runtime.get("server_legacy_token_enable") != Some(&Value::Bool(false))
        || runtime.get("server_require_idempotency_key") != Some(&Value::Bool(true))
    {
        return Err(ProvisionSpecError::Validation(
            "maintenance_cutover requires legacy node tokens disabled and idempotency keys required",
        ));
    }
    require_non_placeholder_secret(runtime, "app_key", 32)?;
    require_non_placeholder_secret(runtime, "server_token", 32)?;
    if runtime.get("app_key") == runtime.get("server_token") {
        return Err(ProvisionSpecError::Validation(
            "runtime.app_key and runtime.server_token must be different secrets",
        ));
    }
    require_nonempty_string(runtime, "app_name")?;
    require_nonempty_string(runtime, "app_url")?;
    require_nonempty_string(runtime, "secure_path")?;
    require_nonempty_string(runtime, "bind_addr")?;
    validate_runtime_values(runtime)?;
    Ok(())
}

fn validate_runtime_values(runtime: &Map<String, Value>) -> Result<(), ProvisionSpecError> {
    let in_range = |key: &str, minimum: i64, maximum: i64| {
        runtime
            .get(key)
            .and_then(Value::as_i64)
            .is_some_and(|value| (minimum..=maximum).contains(&value))
    };
    for (key, minimum, maximum) in [
        ("show_subscribe_method", 0, 2),
        ("ticket_status", 0, 2),
        ("reset_traffic_method", 0, 4),
        ("try_out_enable", 0, 1),
        ("allow_new_period", 0, 1),
        ("new_order_event_id", 0, 1),
        ("renew_order_event_id", 0, 1),
        ("change_order_event_id", 0, 1),
        ("device_limit_mode", 0, 1),
        ("show_subscribe_expire", 1, MAX_CONFIG_DURATION_MINUTES),
    ] {
        if !in_range(key, minimum, maximum) {
            return Err(ProvisionSpecError::Validation(
                "runtime enum or duration setting is outside its supported range",
            ));
        }
    }
    let secure_path = runtime
        .get("secure_path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if is_placeholder(secure_path, 8)
        || secure_path.chars().count() < 8
        || !secure_path
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(ProvisionSpecError::Validation(
            "runtime.secure_path must be at least 8 ASCII letters, digits, underscores, or hyphens",
        ));
    }
    if runtime
        .get("subscribe_path")
        .and_then(Value::as_str)
        .is_none_or(|path| !path.starts_with('/'))
    {
        return Err(ProvisionSpecError::Validation(
            "runtime.subscribe_path must start with /",
        ));
    }
    if runtime
        .get("bind_addr")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .is_none()
    {
        return Err(ProvisionSpecError::Validation(
            "runtime.bind_addr must be an IP socket address",
        ));
    }
    for key in [
        "app_url",
        "subscribe_url",
        "server_api_url",
        "logo",
        "tos_url",
        "telegram_discuss_link",
        "frontend_background_url",
        "windows_download_url",
        "macos_download_url",
        "android_download_url",
    ] {
        if let Some(value) = runtime.get(key).and_then(Value::as_str) {
            let candidates = if key == "subscribe_url" {
                value.split(',').map(str::trim).collect::<Vec<_>>()
            } else {
                vec![value]
            };
            for candidate in candidates {
                let url = Url::parse(candidate).map_err(|_| {
                    ProvisionSpecError::Validation("runtime URL setting is not an absolute URL")
                })?;
                if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                    return Err(ProvisionSpecError::Validation(
                        "runtime URL setting must use http or https with a host",
                    ));
                }
                if !url.username().is_empty() || url.password().is_some() {
                    return Err(ProvisionSpecError::Validation(
                        "runtime URL settings must not contain userinfo credentials",
                    ));
                }
                if url_has_placeholder_host(&url) {
                    return Err(ProvisionSpecError::Validation(
                        "runtime URL setting still contains a reserved placeholder host",
                    ));
                }
                if matches!(key, "app_url" | "subscribe_url" | "server_api_url")
                    && (url.scheme() != "https"
                        || url.path() != "/"
                        || url.query().is_some()
                        || url.fragment().is_some())
                {
                    return Err(ProvisionSpecError::Validation(
                        "runtime app_url, subscribe_url, and server_api_url entries must be canonical HTTPS origins",
                    ));
                }
            }
        }
    }
    for origin in runtime
        .get("cors_allowed_origins")
        .and_then(Value::as_array)
        .expect("list type was validated")
    {
        let url =
            Url::parse(origin.as_str().expect("list item type was validated")).map_err(|_| {
                ProvisionSpecError::Validation("runtime CORS origin is not an absolute URL")
            })?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ProvisionSpecError::Validation(
                "runtime CORS entries must be canonical HTTPS origins",
            ));
        }
        if url_has_placeholder_host(&url) {
            return Err(ProvisionSpecError::Validation(
                "runtime CORS origin still contains a reserved placeholder host",
            ));
        }
    }
    if runtime.get("recaptcha_enable") == Some(&Value::Bool(true))
        && ["recaptcha_site_key", "recaptcha_key"]
            .iter()
            .any(|key| !runtime_non_placeholder_string(runtime, key, 16))
    {
        return Err(ProvisionSpecError::Validation(
            "enabled reCAPTCHA requires both runtime keys",
        ));
    }
    if runtime.get("telegram_bot_enable") == Some(&Value::Bool(true))
        && (!runtime_non_placeholder_string(runtime, "telegram_bot_token", 16)
            || !runtime
                .get("telegram_bot_token")
                .and_then(Value::as_str)
                .is_some_and(basic_telegram_bot_token))
    {
        return Err(ProvisionSpecError::Validation(
            "enabled Telegram bot requires runtime.telegram_bot_token",
        ));
    }
    for key in [
        "email_username",
        "email_password",
        "recaptcha_site_key",
        "recaptcha_key",
        "telegram_bot_token",
    ] {
        if runtime
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty() && contains_placeholder_marker(value))
        {
            return Err(ProvisionSpecError::Validation(
                "runtime integration credentials must not contain placeholder markers",
            ));
        }
    }
    if runtime
        .get("telegram_bot_token")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty() && !basic_telegram_bot_token(value))
    {
        return Err(ProvisionSpecError::Validation(
            "runtime.telegram_bot_token must use Telegram bot-id:secret syntax",
        ));
    }
    if runtime.get("email_verify") == Some(&Value::Bool(true))
        && ["email_host", "email_from_address"]
            .iter()
            .any(|key| !runtime_non_placeholder_string(runtime, key, 3))
    {
        return Err(ProvisionSpecError::Validation(
            "enabled email verification requires host and from address",
        ));
    }
    if runtime.get("email_verify") == Some(&Value::Bool(true)) {
        let host = runtime
            .get("email_host")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let from = runtime
            .get("email_from_address")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if host.chars().any(char::is_whitespace)
            || host.contains("://")
            || host_is_reserved_placeholder(host)
            || !basic_email(from)
        {
            return Err(ProvisionSpecError::Validation(
                "enabled email verification requires a hostname and valid from address",
            ));
        }
        let username_present = runtime
            .get("email_username")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        let password_present = runtime
            .get("email_password")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        if username_present != password_present {
            return Err(ProvisionSpecError::Validation(
                "runtime email username and password must either both be set or both be null",
            ));
        }
    }
    let deposit_tiers = runtime
        .get("deposit_bounus")
        .and_then(Value::as_array)
        .expect("list type was validated");
    if deposit_tiers.iter().any(|tier| {
        let tier = tier.as_str().expect("list item type was validated");
        !tier.is_empty()
            && tier
                .split_once(':')
                .is_none_or(|(amount, bonus)| !decimal_text(amount) || !decimal_text(bonus))
    }) {
        return Err(ProvisionSpecError::Validation(
            "runtime.deposit_bounus entries must use amount:bonus decimal syntax",
        ));
    }
    Ok(())
}

fn runtime_non_placeholder_string(runtime: &Map<String, Value>, key: &str, minimum: usize) -> bool {
    runtime
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| !is_placeholder(value, minimum))
}

fn basic_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.contains('@')
        && !host_is_reserved_placeholder(domain)
        && !value.chars().any(char::is_whitespace)
}

fn basic_telegram_bot_token(value: &str) -> bool {
    let Some((bot_id, secret)) = value.split_once(':') else {
        return false;
    };
    (1..=20).contains(&bot_id.len())
        && bot_id.bytes().all(|byte| byte.is_ascii_digit())
        && !bot_id.starts_with('0')
        && (20..=128).contains(&secret.len())
        && secret
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn url_has_placeholder_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return true;
    };
    if host_is_reserved_placeholder(host) {
        return true;
    }
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized == "localhost" || normalized.ends_with(".localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .is_ok_and(|address| address.is_loopback() || address.is_unspecified())
}

fn host_is_reserved_placeholder(host: &str) -> bool {
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    normalized.contains("replace")
        || matches!(
            normalized.as_str(),
            "example" | "invalid" | "test" | "example.com" | "example.net" | "example.org"
        )
        || [
            ".example.com",
            ".example.net",
            ".example.org",
            ".example",
            ".invalid",
            ".test",
        ]
        .iter()
        .any(|suffix| normalized.ends_with(suffix))
}

fn decimal_text(value: &str) -> bool {
    let mut dot = false;
    let mut digits = 0_usize;
    for character in value.chars() {
        if character == '.' && !dot {
            dot = true;
        } else if character.is_ascii_digit() {
            digits += 1;
        } else {
            return false;
        }
    }
    digits != 0 && !value.starts_with('.') && !value.ends_with('.')
}

fn require_nonempty_string(
    runtime: &Map<String, Value>,
    key: &'static str,
) -> Result<(), ProvisionSpecError> {
    if runtime
        .get(key)
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(ProvisionSpecError::Validation(match key {
            "app_name" => "runtime.app_name must be explicit",
            "app_url" => "runtime.app_url must be explicit",
            "secure_path" => "runtime.secure_path must be explicit",
            "bind_addr" => "runtime.bind_addr must be explicit",
            _ => "required runtime string is missing",
        }));
    }
    Ok(())
}

fn require_non_placeholder_secret(
    runtime: &Map<String, Value>,
    key: &'static str,
    minimum: usize,
) -> Result<(), ProvisionSpecError> {
    let Some(value) = runtime.get(key).and_then(Value::as_str) else {
        return Err(ProvisionSpecError::Validation(
            "runtime secrets must be explicit strings",
        ));
    };
    if is_placeholder(value, minimum) {
        return Err(ProvisionSpecError::Validation(match key {
            "app_key" => "runtime.app_key must be a non-placeholder secret of at least 32 bytes",
            "server_token" => {
                "runtime.server_token must be a non-placeholder secret of at least 32 bytes"
            }
            _ => "runtime secret is invalid",
        }));
    }
    Ok(())
}

fn is_placeholder(value: &str, minimum: usize) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    value.len() < minimum
        || value.bytes().all(|byte| byte == value.as_bytes()[0])
        || contains_placeholder_marker(value)
        || lower.contains("example")
}

fn contains_placeholder_marker(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    value.starts_with('<')
        || [
            "change-me",
            "changeme",
            "replace-me",
            "replaceme",
            "replace_with",
            "replace-with",
            "your-secret",
            "your_secret",
            "your-password",
            "your_password",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn strict_percent_decode(value: &str) -> Result<String, ProvisionSpecError> {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return Err(ProvisionSpecError::Validation(
                    "URL components must use valid percent encoding",
                ));
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    percent_decode_str(value)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
        .map_err(|_| {
            ProvisionSpecError::Validation("URL components must use valid UTF-8 percent encoding")
        })
}

fn valid_mysql_account_host(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 253
        || value.trim() != value
        || value.chars().any(|character| {
            character.is_control() || matches!(character, '%' | '_' | '\\' | '\'' | '"')
        })
        || host_is_reserved_placeholder(value)
    {
        return false;
    }
    if let Ok(address) = value.parse::<IpAddr>() {
        return !address.is_loopback() && !address.is_unspecified();
    }
    if value.contains('/') {
        let Ok(network) = value.parse::<IpNet>() else {
            return false;
        };
        return matches!(network, IpNet::V4(network) if network.addr() == network.network()
            && !network.addr().is_loopback()
            && !network.addr().is_unspecified());
    }
    if value.ends_with('.')
        || value.eq_ignore_ascii_case("localhost")
        || value.to_ascii_lowercase().ends_with(".localhost")
    {
        return false;
    }
    value.split('.').all(|label| {
        (1..=63).contains(&label.len())
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn validate_mysql_url(value: &str, field: &'static str) -> Result<(), ProvisionSpecError> {
    let url = Url::parse(value)
        .map_err(|_| ProvisionSpecError::Validation("database URL must be a valid mysql:// URL"))?;
    let username = strict_percent_decode(url.username())?;
    let password = url
        .password()
        .map(strict_percent_decode)
        .transpose()?
        .unwrap_or_default();
    let database_path = strict_percent_decode(url.path())?;
    let database = database_path.strip_prefix('/').unwrap_or_default();
    if url.scheme() != "mysql"
        || url.host_str().is_none()
        || database.is_empty()
        || database.contains('/')
        || username.is_empty()
        || password.is_empty()
        || url.fragment().is_some()
    {
        return Err(ProvisionSpecError::Validation(
            "database URL must include host, database name, username, and password",
        ));
    }
    if url.host_str().is_some_and(host_is_reserved_placeholder) {
        return Err(ProvisionSpecError::Validation(
            "database URL still contains a reserved placeholder host",
        ));
    }
    let minimum = if matches!(
        field,
        "target.bootstrap_database_url" | "target.application_database_url"
    ) {
        16
    } else {
        1
    };
    if is_placeholder(&password, minimum) {
        return Err(ProvisionSpecError::Validation(
            "database URL password must not be a placeholder",
        ));
    }
    Ok(())
}

fn validate_target_mysql_urls(target: &TargetSpec) -> Result<(), ProvisionSpecError> {
    let bootstrap = Url::parse(&target.bootstrap_database_url)
        .map_err(|_| ProvisionSpecError::Validation("target bootstrap database URL is invalid"))?;
    let application = Url::parse(&target.application_database_url).map_err(|_| {
        ProvisionSpecError::Validation("target application database URL is invalid")
    })?;
    if mysql_endpoint_identity(&bootstrap) != mysql_endpoint_identity(&application) {
        return Err(ProvisionSpecError::Validation(
            "target bootstrap and application database URLs must use the same MySQL host and port",
        ));
    }
    if !mysql_url_verifies_identity(&target.bootstrap_database_url)
        || !mysql_url_verifies_identity(&target.application_database_url)
    {
        return Err(ProvisionSpecError::Validation(
            "both target MySQL URLs must use ssl-mode=VERIFY_IDENTITY",
        ));
    }
    let bootstrap_database = target_database_name(&bootstrap)?;
    let application_database = target_database_name(&application)?;
    if bootstrap_database.eq_ignore_ascii_case(&application_database) {
        return Err(ProvisionSpecError::Validation(
            "target bootstrap database and application database must be different",
        ));
    }
    let bootstrap_username = strict_percent_decode(bootstrap.username())?;
    let application_username = strict_percent_decode(application.username())?;
    if bootstrap_username == application_username {
        return Err(ProvisionSpecError::Validation(
            "target bootstrap and application database usernames must be different",
        ));
    }
    let bootstrap_password = strict_percent_decode(
        bootstrap
            .password()
            .expect("validated target bootstrap password"),
    )?;
    let application_password = strict_percent_decode(
        application
            .password()
            .expect("validated target application password"),
    )?;
    if bootstrap_password == application_password {
        return Err(ProvisionSpecError::Validation(
            "target bootstrap and application database passwords must be different",
        ));
    }
    if application_username.len() > 32
        || !application_username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'$'))
    {
        return Err(ProvisionSpecError::Validation(
            "target application database username must be 1-32 ASCII letters, digits, underscores, hyphens, or dollar signs",
        ));
    }
    if matches!(
        application_database.to_ascii_lowercase().as_str(),
        "mysql" | "information_schema" | "performance_schema" | "sys"
    ) {
        return Err(ProvisionSpecError::Validation(
            "target application database must not use a reserved MySQL system schema name",
        ));
    }
    if !valid_mysql_account_host(&target.application_account_host) {
        return Err(ProvisionSpecError::Validation(
            "target.application_account_host must be an exact hostname, IP address, or CIDR; MySQL wildcard hosts are forbidden",
        ));
    }
    Ok(())
}

fn mysql_endpoint_identity(url: &Url) -> (String, u16) {
    (
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default().unwrap_or(3306),
    )
}

fn target_database_name(url: &Url) -> Result<String, ProvisionSpecError> {
    let path = strict_percent_decode(url.path().strip_prefix('/').unwrap_or_default())?;
    if path.is_empty()
        || path.len() > 64
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(ProvisionSpecError::Validation(
            "target MySQL database names must be 1-64 ASCII letters, digits, or underscores",
        ));
    }
    Ok(path)
}

fn validate_redis_url(value: &str, _field: &'static str) -> Result<(), ProvisionSpecError> {
    let url = Url::parse(value).map_err(|_| {
        ProvisionSpecError::Validation("Redis URL must be a valid redis:// or rediss:// URL")
    })?;
    if !matches!(url.scheme(), "redis" | "rediss") || url.host_str().is_none() {
        return Err(ProvisionSpecError::Validation(
            "Redis URL must be a valid redis:// or rediss:// URL with a host",
        ));
    }
    if url.host_str().is_some_and(host_is_reserved_placeholder) {
        return Err(ProvisionSpecError::Validation(
            "Redis URL still contains a reserved placeholder host",
        ));
    }
    strict_percent_decode(url.username())?;
    let password = url.password().map(strict_percent_decode).transpose()?;
    if password
        .as_deref()
        .is_some_and(|password| is_placeholder(password, 1))
    {
        return Err(ProvisionSpecError::Validation(
            "Redis URL password must not be a placeholder",
        ));
    }
    Ok(())
}

fn mysql_url_verifies_identity(value: &str) -> bool {
    Url::parse(value).ok().is_some_and(|url| {
        let modes = url
            .query_pairs()
            .filter(|(key, _)| key.eq_ignore_ascii_case("ssl-mode"))
            .map(|(_, value)| value.to_ascii_lowercase().replace('-', "_"))
            .collect::<Vec<_>>();
        modes.as_slice() == ["verify_identity"]
    })
}

fn redis_url_uses_tls(value: &str) -> bool {
    Url::parse(value)
        .ok()
        .is_some_and(|url| url.scheme() == "rediss")
}

fn datastore_identity(value: &str) -> Result<String, ProvisionSpecError> {
    let url = Url::parse(value)
        .map_err(|_| ProvisionSpecError::Validation("datastore URL is invalid"))?;
    let host = url
        .host_str()
        .ok_or(ProvisionSpecError::Validation("datastore URL has no host"))?;
    let port = url.port_or_known_default().unwrap_or(0);
    let datastore_kind = if url.scheme() == "mysql" {
        "mysql"
    } else {
        "redis"
    };
    let path = strict_percent_decode(url.path())?;
    Ok(format!(
        "{}://{}:{}{}",
        datastore_kind,
        host.to_ascii_lowercase(),
        port,
        path
    ))
}

impl TargetSpec {
    pub(crate) fn application_database_name(&self) -> String {
        let url = Url::parse(&self.application_database_url)
            .expect("validated target application database URL");
        target_database_name(&url).expect("validated target application database name")
    }

    pub(crate) fn application_username(&self) -> String {
        let url = Url::parse(&self.application_database_url)
            .expect("validated target application database URL");
        strict_percent_decode(url.username()).expect("validated target application username")
    }
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
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
        D: serde::Deserializer<'de>,
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

impl ProvisionSpec {
    pub fn manifest_binding_hmac_sha256(&self) -> &str {
        &self.manifest_binding_hmac_sha256
    }

    pub fn materialized_runtime_config(&self) -> Map<String, Value> {
        let mut runtime = self.runtime.clone();
        runtime.insert(
            "database_url".to_string(),
            Value::String(self.target.application_database_url.clone()),
        );
        runtime.insert(
            "redis_url".to_string(),
            Value::String(self.target.redis_url.clone()),
        );
        runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn complete_runtime() -> Map<String, Value> {
        let mut map = Map::new();
        for key in RUNTIME_KEYS_V1 {
            let value = if BOOL_RUNTIME_KEYS.contains(key) {
                Value::Bool(false)
            } else if INTEGER_RUNTIME_KEYS.contains(key) {
                Value::from(1)
            } else if LIST_RUNTIME_KEYS.contains(key) {
                Value::Array(Vec::new())
            } else {
                Value::Null
            };
            map.insert((*key).to_string(), value);
        }
        map.insert(
            "configuration_source".to_string(),
            Value::String("file_only".into()),
        );
        map.insert(
            "environment".to_string(),
            Value::String("production".into()),
        );
        map.insert(
            "bind_addr".to_string(),
            Value::String("0.0.0.0:8080".into()),
        );
        map.insert(
            "subscribe_path".to_string(),
            Value::String("/api/v1/client/subscribe".into()),
        );
        map.insert(
            "app_key".to_string(),
            Value::String("0123456789abcdef0123456789abcdef".into()),
        );
        map.insert("app_name".to_string(), Value::String("V2Board".into()));
        map.insert(
            "app_url".to_string(),
            Value::String("https://panel.company.net".into()),
        );
        map.insert(
            "secure_path".to_string(),
            Value::String("admin-safe".into()),
        );
        map.insert(
            "server_token".to_string(),
            Value::String("abcdef0123456789abcdef0123456789".into()),
        );
        map.insert("legacy_auth_params_enable".to_string(), Value::Bool(false));
        map.insert("legacy_jwt_cutoff_unix".to_string(), Value::from(0));
        map.insert("server_legacy_token_enable".to_string(), Value::Bool(false));
        map.insert(
            "server_require_idempotency_key".to_string(),
            Value::Bool(true),
        );
        map.insert("show_subscribe_expire".to_string(), Value::from(5));
        map
    }

    #[test]
    fn complete_runtime_rejects_unknown_and_missing_keys() {
        let runtime = complete_runtime();
        validate_runtime(&runtime).expect("complete runtime");

        let mut missing = runtime.clone();
        missing.remove("app_name");
        assert!(matches!(
            validate_runtime(&missing),
            Err(ProvisionSpecError::MissingRuntimeKeys(_))
        ));

        let mut unknown = runtime;
        unknown.insert("typo_setting".to_string(), Value::Bool(true));
        assert!(matches!(
            validate_runtime(&unknown),
            Err(ProvisionSpecError::UnknownRuntimeKeys(_))
        ));
    }

    #[test]
    fn decisions_force_logout_and_strict_node_credentials() {
        let mut runtime = complete_runtime();
        runtime.insert("legacy_jwt_cutoff_unix".to_string(), Value::from(60));
        assert!(validate_runtime(&runtime).is_err());

        let mut runtime = complete_runtime();
        runtime.insert("server_legacy_token_enable".to_string(), Value::Bool(true));
        assert!(validate_runtime(&runtime).is_err());
    }

    #[test]
    fn placeholders_and_secret_reuse_are_rejected() {
        let mut runtime = complete_runtime();
        let app_key = runtime.get("app_key").cloned().expect("app key");
        runtime.insert("server_token".to_string(), app_key);
        assert!(validate_runtime(&runtime).is_err());

        let mut runtime = complete_runtime();
        runtime.insert("recaptcha_enable".to_string(), Value::Bool(true));
        runtime.insert(
            "recaptcha_site_key".to_string(),
            Value::String("REPLACE_WITH_SITE_KEY".into()),
        );
        runtime.insert(
            "recaptcha_key".to_string(),
            Value::String("REPLACE_WITH_SECRET_KEY".into()),
        );
        assert!(validate_runtime(&runtime).is_err());

        let mut runtime = complete_runtime();
        runtime.insert(
            "app_url".to_string(),
            Value::String("https://panel.invalid".into()),
        );
        assert!(validate_runtime(&runtime).is_err());

        let mut runtime = complete_runtime();
        runtime.insert(
            "app_url".to_string(),
            Value::String("https://invalid".into()),
        );
        assert!(validate_runtime(&runtime).is_err());

        let mut runtime = complete_runtime();
        runtime.insert("telegram_bot_enable".to_string(), Value::Bool(true));
        runtime.insert(
            "telegram_bot_token".to_string(),
            Value::String("totally-not-a-valid-token".into()),
        );
        assert!(validate_runtime(&runtime).is_err());
        assert!(basic_telegram_bot_token(
            "123456789:abcdefghijklmnopqrstuvwxyz_12345678"
        ));

        assert!(
            validate_mysql_url(
                "mysql://user:secret@database.invalid/v2board",
                "source.database_url"
            )
            .is_err()
        );

        for invalid in [
            "https://user:secret@panel.company.net",
            "https://panel.company.net/base",
            "https://panel.company.net/?old=value",
            "https://panel.company.net/#fragment",
            "http://panel.company.net",
        ] {
            let mut runtime = complete_runtime();
            runtime.insert("app_url".to_string(), Value::String(invalid.to_string()));
            assert!(validate_runtime(&runtime).is_err(), "accepted {invalid}");
        }

        let mut runtime = complete_runtime();
        runtime.insert(
            "cors_allowed_origins".to_string(),
            Value::Array(vec![Value::String(
                "https://panel.company.net/path".to_string(),
            )]),
        );
        assert!(validate_runtime(&runtime).is_err());
    }

    #[test]
    fn source_and_target_identity_ignores_credentials() {
        let source = datastore_identity("mysql://old:secret@db.example/v2board").unwrap();
        let target = datastore_identity("mysql://new:other@db.example/v2board").unwrap();
        assert_eq!(source, target);
        assert_eq!(
            datastore_identity("mysql://old:secret@db.example/v2%62oard").unwrap(),
            target
        );
    }

    #[test]
    fn encoded_credentials_and_broad_mysql_account_hosts_are_rejected() {
        assert!(
            validate_mysql_url(
                "mysql://app:%52%45%50%4c%41%43%45%5f%57%49%54%48%5f%50%41%53%53%57%4f%52%44@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY",
                "target.application_database_url",
            )
            .is_err()
        );
        assert!(
            validate_redis_url(
                "rediss://:%52%45%50%4c%41%43%45%5f%57%49%54%48%5f%50%41%53%53%57%4f%52%44@cache.company.net/1",
                "target.redis_url",
            )
            .is_err()
        );
        for valid in ["api.internal", "10.0.0.10", "10.0.0.0/24"] {
            assert!(valid_mysql_account_host(valid), "rejected {valid}");
        }
        for invalid in [
            "%",
            "10.0.%",
            " api.internal",
            "api_internal",
            "10.0.0.1/24",
            "localhost",
        ] {
            assert!(!valid_mysql_account_host(invalid), "accepted {invalid}");
        }
    }

    #[test]
    fn target_bootstrap_and_application_urls_are_strictly_separated() {
        let target = |bootstrap: &str, application: &str| TargetSpec {
            bootstrap_database_url: bootstrap.to_string(),
            application_database_url: application.to_string(),
            application_account_host: "10.0.0.0/24".to_string(),
            redis_url: "rediss://cache.company.net/1".to_string(),
            runtime_config_path: PathBuf::from("/var/lib/v2board/config/config.json"),
            require_database_absent: true,
            require_account_absent: true,
            require_empty_redis: true,
        };
        let bootstrap =
            "mysql://admin:0123456789abcdef@db.company.net/mysql?ssl-mode=VERIFY_IDENTITY";
        let application =
            "mysql://app:abcdef0123456789@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY";
        validate_mysql_url(bootstrap, "target.bootstrap_database_url").unwrap();
        validate_mysql_url(application, "target.application_database_url").unwrap();
        validate_target_mysql_urls(&target(bootstrap, application)).unwrap();

        let other_host =
            "mysql://app:abcdef0123456789@other.company.net/v2board?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, other_host)).is_err());

        let same_database =
            "mysql://app:abcdef0123456789@db.company.net/mysql?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, same_database)).is_err());

        let same_credentials =
            "mysql://admin:0123456789abcdef@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, same_credentials)).is_err());

        let same_username =
            "mysql://admin:abcdef0123456789@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, same_username)).is_err());

        let same_password =
            "mysql://app:0123456789abcdef@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, same_password)).is_err());

        let encoded_same_username =
            "mysql://%61dmin:abcdef0123456789@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, encoded_same_username)).is_err());

        let system_database =
            "mysql://app:abcdef0123456789@db.company.net/sys?ssl-mode=VERIFY_IDENTITY";
        assert!(validate_target_mysql_urls(&target(bootstrap, system_database)).is_err());

        let mut global_account = target(bootstrap, application);
        global_account.application_account_host = "%".to_string();
        assert!(validate_target_mysql_urls(&global_account).is_err());

        let no_verified_tls =
            "mysql://app:abcdef0123456789@db.company.net/v2board?ssl-mode=REQUIRED";
        assert!(validate_target_mysql_urls(&target(bootstrap, no_verified_tls)).is_err());
    }

    #[test]
    fn complete_runtime_is_loadable_with_file_only_semantics() {
        let mut runtime = complete_runtime();
        for (key, value) in [
            ("http_connect_timeout_seconds", 10),
            ("http_request_timeout_seconds", 30),
            ("api_request_timeout_seconds", 45),
            ("password_kdf_max_parallel", 4),
            ("auth_session_ttl_seconds", 3_600),
            ("privileged_auth_session_ttl_seconds", 900),
            ("auth_session_max_per_user", 20),
            ("privileged_step_up_ttl_seconds", 300),
            ("privileged_step_up_max_attempts", 5),
            ("privileged_step_up_attempt_window_seconds", 300),
        ] {
            runtime.insert(key.to_string(), Value::from(value));
        }
        runtime.insert(
            "database_url".to_string(),
            Value::String(
                "mysql://user:secret@db.company.net/v2board?ssl-mode=VERIFY_IDENTITY".into(),
            ),
        );
        runtime.insert(
            "redis_url".to_string(),
            Value::String("rediss://cache.company.net/1".into()),
        );
        AppConfig::try_from_config_map(
            runtime,
            RuntimePaths {
                config: PathBuf::from("/var/lib/v2board/config/config.json"),
                frontend: PathBuf::from("/opt/v2board/frontend"),
                rules: PathBuf::from("/var/lib/v2board/rules"),
            },
        )
        .expect("complete file-only runtime must load");
    }

    #[test]
    fn loader_accepts_a_complete_secret_file_and_rejects_duplicate_keys() {
        let mut runtime = complete_runtime();
        for (key, value) in [
            ("http_connect_timeout_seconds", 10),
            ("http_request_timeout_seconds", 30),
            ("api_request_timeout_seconds", 45),
            ("password_kdf_max_parallel", 4),
            ("auth_session_ttl_seconds", 3_600),
            ("privileged_auth_session_ttl_seconds", 900),
            ("auth_session_max_per_user", 20),
            ("privileged_step_up_ttl_seconds", 300),
            ("privileged_step_up_max_attempts", 5),
            ("privileged_step_up_attempt_window_seconds", 300),
        ] {
            runtime.insert(key.to_string(), Value::from(value));
        }
        let document = serde_json::json!({
            "schema_version": 2,
            "operation_id": "40aa4a80-eb4b-4b25-9c3b-e17ed047873d",
            "kind": "legacy_reference_migration",
            "reference_commit": LEGACY_REFERENCE_COMMIT,
            "lifecycle_audit_key": "lifecycle-audit-0123456789abcdef0123456789abcdef",
            "source": {
                "database_url": "mysql://readonly:secret@old-db.company.net/v2board",
                "redis_default_url": "redis://old-cache.company.net/0",
                "redis_cache_url": "redis://old-cache.company.net/1",
                "redis_connection_prefix": "v2board_database_",
                "redis_cache_prefix": "v2board_cache",
                "legacy_cache_driver": "redis",
                "legacy_show_subscribe_method": 0,
                "legacy_show_subscribe_expire_minutes": 5,
                "legacy_subscription_issuance_stopped_at_unix": 0,
                "transport_security": "trusted_maintenance_network"
            },
            "target": {
                "bootstrap_database_url": "mysql://bootstrap:0123456789abcdef@new-db.company.net/mysql?ssl-mode=VERIFY_IDENTITY",
                "application_database_url": "mysql://v2board:abcdef0123456789@new-db.company.net/v2board?ssl-mode=VERIFY_IDENTITY",
                "application_account_host": "10.0.0.0/24",
                "redis_url": "rediss://new-cache.company.net/1",
                "runtime_config_path": "/var/lib/v2board/config/config.json",
                "require_database_absent": true,
                "require_account_absent": true,
                "require_empty_redis": true
            },
            "runtime": runtime,
            "decisions": {
                "legacy_configuration": "manual_only",
                "sessions": "logout_all",
                "legacy_cache": "discard_ephemeral_after_fence",
                "legacy_stripe": "assert_none",
                "legacy_subscription_tokens": "assert_none",
                "nodes": "maintenance_cutover",
                "legacy_theme": "discard_confirmed",
                "legacy_custom_rules": "none"
            },
            "attestations": {
                "source_writers_stopped": false,
                "source_workers_stopped": false,
                "node_reporters_stopped": false,
                "legacy_queues_drained": false,
                "backup_reference": null,
                "restore_tested": false
            }
        });
        let root = std::env::temp_dir().join(format!(
            "v2board-provision-manifest-test-{}-{}",
            std::process::id(),
            TEST_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("test root");
        let valid_path = root.join("valid.json");
        write_private_test_file(&valid_path, &serde_json::to_vec(&document).expect("JSON"));
        let first_binding = load_provision_spec(&valid_path)
            .expect("complete manifest")
            .manifest_binding_hmac_sha256()
            .to_string();

        let mut changed_document = document.clone();
        changed_document["runtime"]["app_url"] =
            Value::String("https://other.company.net".to_string());
        let changed_path = root.join("changed.json");
        write_private_test_file(
            &changed_path,
            &serde_json::to_vec(&changed_document).expect("JSON"),
        );
        let changed_binding = load_provision_spec(&changed_path)
            .expect("changed complete manifest")
            .manifest_binding_hmac_sha256()
            .to_string();
        assert_ne!(first_binding, changed_binding);

        let mut reused_audit_key_document = document.clone();
        let reused_secret = "audit-datastore-secret-0123456789abcdef";
        reused_audit_key_document["lifecycle_audit_key"] = Value::String(reused_secret.to_string());
        reused_audit_key_document["target"]["bootstrap_database_url"] = Value::String(format!(
            "mysql://bootstrap:{reused_secret}@new-db.company.net/mysql?ssl-mode=VERIFY_IDENTITY"
        ));
        let reused_path = root.join("reused-audit-key.json");
        write_private_test_file(
            &reused_path,
            &serde_json::to_vec(&reused_audit_key_document).expect("JSON"),
        );
        let error = match load_provision_spec(&reused_path) {
            Ok(_) => panic!("reused audit key must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("target datastore passwords"));

        let duplicate_path = root.join("duplicate.json");
        write_private_test_file(
            &duplicate_path,
            br#"{"schema_version":2,"schema_version":2}"#,
        );
        let error = match load_provision_spec(&duplicate_path) {
            Ok(_) => panic!("duplicate key must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("duplicate JSON key"));
        fs::remove_dir_all(root).expect("remove test root");
    }

    fn write_private_test_file(path: &Path, bytes: &[u8]) {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).expect("private test file");
        use std::io::Write;
        file.write_all(bytes).expect("write test file");
    }
}
