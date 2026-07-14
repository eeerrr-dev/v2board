use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

use percent_encoding::percent_decode_str;
use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::{Host, Url};
use v2board_config::{
    AppConfig, BOOT_ONLY_RUNTIME_KEYS_V1, FILE_ONLY_RUNTIME_KEYS_V1, RuntimePaths,
};

use crate::mysql_import_converter::MYSQL_IMPORT_SCHEMA_VERSION;

pub const MYSQL_SOURCE_REFERENCE_COMMIT: &str = "7e77de9f4873b317157490529f7be7d6f8a62421";

const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const BOOL_RUNTIME_KEYS: &[&str] = &[
    "privileged_step_up_enable",
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
    "try_out_enable",
    "plan_change_enable",
    "surplus_enable",
    "commission_first_time_enable",
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
    "email_port",
    "register_limit_count",
    "register_limit_expire",
    "show_subscribe_method",
    "show_subscribe_expire",
    "allow_new_period",
    "reset_traffic_method",
    "try_out_plan_id",
    "invite_commission",
    "new_order_event_id",
    "renew_order_event_id",
    "change_order_event_id",
    "invite_gen_limit",
    "ticket_status",
    "server_push_interval",
    "server_pull_interval",
    "server_node_report_min_traffic",
    "server_device_online_min_traffic",
    "device_limit_mode",
    "password_limit_count",
    "password_limit_expire",
];

const DECIMAL_RUNTIME_KEYS: &[&str] = &["try_out_hour", "commission_withdraw_limit"];
const LIST_RUNTIME_KEYS: &[&str] = &[
    "cors_allowed_origins",
    "trusted_proxy_cidrs",
    "email_whitelist_suffix",
    "commission_withdraw_method",
    "deposit_bounus",
];

#[derive(Clone)]
pub struct MysqlImportSpec {
    pub schema_version: u32,
    pub source: MysqlImportSourceSpec,
    pub target: Map<String, Value>,
    pub runtime: Map<String, Value>,
    manifest_sha256: String,
}

/// Fully validated, secret-bearing execution inputs for the disposable
/// lifecycle binary. This value must never be formatted with `Debug` or
/// serialized as a report: it contains every transient datastore credential.
pub struct MysqlImportExecutionPlan {
    pub postgres: MysqlImportPostgresPlan,
    pub clickhouse: MysqlImportClickHousePlan,
    pub analytics_admission: MysqlImportAnalyticsAdmissionPlan,
    pub redis_bootstrap_url: String,
    pub config_output_directory: PathBuf,
    pub api_boot_config: Map<String, Value>,
    pub worker_boot_config: Map<String, Value>,
    pub operator_config: Map<String, Value>,
    pub app_key: String,
}

pub struct MysqlImportPostgresPlan {
    pub bootstrap_database_url: String,
    pub migration_database_url: String,
    pub api_database_url: String,
    pub worker_database_url: String,
}

pub struct MysqlImportClickHousePlan {
    pub endpoint: String,
    pub database: String,
    pub bootstrap_username: String,
    pub bootstrap_password: String,
    pub schema_username: String,
    pub schema_password: String,
    pub writer_username: String,
    pub writer_password: String,
    pub raw_retention_days: u32,
    pub aggregate_retention_days: u32,
}

#[derive(Clone)]
pub struct MysqlImportAnalyticsAdmissionPlan {
    pub recovery_pending_rows: u64,
    pub soft_pending_rows: u64,
    pub hard_pending_rows: u64,
    pub recovery_relation_bytes: u64,
    pub soft_relation_bytes: u64,
    pub hard_relation_bytes: u64,
    pub recovery_oldest_age_seconds: u64,
    pub soft_oldest_age_seconds: u64,
    pub hard_oldest_age_seconds: u64,
    pub database_capacity_bytes: u64,
    pub hard_min_headroom_bytes: u64,
    pub soft_min_headroom_bytes: u64,
    pub recovery_min_headroom_bytes: u64,
    pub event_reservation_bytes: u64,
    pub soft_max_new_rows_per_second: u64,
    pub sample_interval_seconds: u64,
    pub stale_after_seconds: u64,
    pub capacity_evidence: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MysqlImportDocument {
    schema_version: u32,
    source: MysqlImportSourceSpec,
    target: Map<String, Value>,
    runtime: Map<String, Value>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MysqlImportSourceSpec {
    pub dump_path: PathBuf,
    pub dump_sha256: String,
    pub database_url: String,
}

#[derive(Debug, Error)]
pub enum MysqlImportSpecError {
    #[error("manifest could not be read safely")]
    Read,
    #[error("manifest must be a root-owned regular non-symlink file no larger than 2 MiB")]
    UnsafeFile,
    #[error("manifest contains invalid JSON or an unsupported field shape: {0}")]
    Json(String),
    #[error("{0}")]
    Invalid(String),
}

pub fn load_mysql_import_spec(
    path: impl AsRef<Path>,
) -> Result<MysqlImportSpec, MysqlImportSpecError> {
    let path = path.as_ref();
    let mut file = File::open(path).map_err(|_| MysqlImportSpecError::Read)?;
    let opened = file.metadata().map_err(|_| MysqlImportSpecError::Read)?;
    let path_metadata = fs::symlink_metadata(path).map_err(|_| MysqlImportSpecError::Read)?;
    if !opened.file_type().is_file()
        || !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || opened.dev() != path_metadata.dev()
        || opened.ino() != path_metadata.ino()
        || opened.len() == 0
        || opened.len() > MAX_MANIFEST_BYTES
        || opened.permissions().mode() & 0o077 != 0
        || opened.uid() != 0
        || path_metadata.uid() != 0
    {
        return Err(MysqlImportSpecError::UnsafeFile);
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| MysqlImportSpecError::Read)?;
    let opened_after = file.metadata().map_err(|_| MysqlImportSpecError::Read)?;
    let after = fs::symlink_metadata(path).map_err(|_| MysqlImportSpecError::Read)?;
    if bytes.len() as u64 != opened.len()
        || opened.dev() != opened_after.dev()
        || opened.ino() != opened_after.ino()
        || opened.len() != opened_after.len()
        || opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || opened.len() != after.len()
        || !after.file_type().is_file()
        || after.file_type().is_symlink()
        || opened_after.permissions().mode() & 0o077 != 0
        || after.permissions().mode() & 0o077 != 0
        || opened_after.uid() != 0
        || after.uid() != 0
    {
        return Err(MysqlImportSpecError::UnsafeFile);
    }
    parse_mysql_import_spec(&bytes)
}

fn parse_mysql_import_spec(bytes: &[u8]) -> Result<MysqlImportSpec, MysqlImportSpecError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(MysqlImportSpecError::UnsafeFile);
    }
    let raw = serde_json::from_slice::<UniqueJson>(bytes)
        .map_err(|error| MysqlImportSpecError::Json(error.to_string()))?
        .0;
    let document: MysqlImportDocument = serde_json::from_value(raw)
        .map_err(|error| MysqlImportSpecError::Json(error.to_string()))?;
    if document.schema_version != MYSQL_IMPORT_SCHEMA_VERSION {
        return Err(MysqlImportSpecError::Invalid(
            "schema_version must equal the single pre-release MySQL import schema version 1"
                .to_string(),
        ));
    }
    validate_document(&document)?;
    Ok(MysqlImportSpec {
        schema_version: document.schema_version,
        source: document.source,
        target: document.target,
        runtime: document.runtime,
        manifest_sha256: hex::encode(Sha256::digest(bytes)),
    })
}

fn validate_document(document: &MysqlImportDocument) -> Result<(), MysqlImportSpecError> {
    validate_source(&document.source)?;
    let target = validate_target(&document.target)?;
    validate_runtime(&document.runtime, &target)
}

fn validate_source(source: &MysqlImportSourceSpec) -> Result<(), MysqlImportSpecError> {
    validate_absolute_normalized_path(&source.dump_path, "source.dump_path")?;
    validate_sha256(&source.dump_sha256, "source.dump_sha256")?;
    let url = parse_url(&source.database_url, "source.database_url")?;
    let username = strict_percent_decode(url.username(), "source.database_url username")?;
    let password = strict_percent_decode(
        url.password().unwrap_or_default(),
        "source.database_url password",
    )?;
    let path = strict_percent_decode(url.path(), "source.database_url database")?;
    let database = path.strip_prefix('/').unwrap_or_default();
    if url.scheme() != "mysql"
        || url.host_str().is_none()
        || username.is_empty()
        || contains_placeholder(&username)
        || password.len() < 16
        || contains_placeholder(&password)
        || !valid_datastore_identifier(database)
        || url.fragment().is_some()
    {
        return Err(MysqlImportSpecError::Invalid(
            "source.database_url must name the legacy MySQL database through a dedicated read-only account with host, username, and non-placeholder password".to_string(),
        ));
    }
    validate_mysql_connection_query(&url)?;
    if !is_loopback_host(&url) {
        return Err(MysqlImportSpecError::Invalid(
            "legacy MySQL must use localhost, 127.0.0.0/8, or ::1 because lifecycle runs on the old production host"
                .to_string(),
        ));
    }
    Ok(())
}

fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        Some(Host::Ipv4(host)) => host.is_loopback(),
        Some(Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatedImportTarget {
    postgres: ValidatedPostgresTarget,
    clickhouse: ValidatedClickHouseTarget,
    analytics_admission: ValidatedAnalyticsAdmission,
    redis_bootstrap_url: String,
    config_output_directory: PathBuf,
    require_empty_redis: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatedPostgresTarget {
    bootstrap_database_url: String,
    migration_database_url: String,
    api_database_url: String,
    worker_database_url: String,
    require_database_absent: bool,
    require_roles_absent: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatedClickHouseTarget {
    endpoint: String,
    database: String,
    bootstrap_principal: ValidatedPrincipal,
    schema_principal: ValidatedPrincipal,
    writer_principal: ValidatedPrincipal,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
    require_database_absent: bool,
    require_principals_absent: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatedPrincipal {
    username: String,
    password: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatedAnalyticsAdmission {
    recovery_pending_rows: u64,
    soft_pending_rows: u64,
    hard_pending_rows: u64,
    recovery_relation_bytes: u64,
    soft_relation_bytes: u64,
    hard_relation_bytes: u64,
    recovery_oldest_age_seconds: u64,
    soft_oldest_age_seconds: u64,
    hard_oldest_age_seconds: u64,
    database_capacity_bytes: u64,
    hard_min_headroom_bytes: u64,
    soft_min_headroom_bytes: u64,
    recovery_min_headroom_bytes: u64,
    event_reservation_bytes: u64,
    soft_max_new_rows_per_second: u64,
    sample_interval_seconds: u64,
    stale_after_seconds: u64,
    capacity_evidence: String,
}

fn validate_target(
    target: &Map<String, Value>,
) -> Result<ValidatedImportTarget, MysqlImportSpecError> {
    let target = serde_json::from_value::<ValidatedImportTarget>(Value::Object(target.clone()))
        .map_err(|error| MysqlImportSpecError::Json(error.to_string()))?;
    if !target.postgres.require_database_absent || !target.postgres.require_roles_absent {
        return Err(MysqlImportSpecError::Invalid(
            "target PostgreSQL database and roles must be declared absent".to_string(),
        ));
    }
    let postgres_urls = [
        validate_postgres_url(&target.postgres.bootstrap_database_url)?,
        validate_postgres_url(&target.postgres.migration_database_url)?,
        validate_postgres_url(&target.postgres.api_database_url)?,
        validate_postgres_url(&target.postgres.worker_database_url)?,
    ];
    let endpoint = postgres_endpoint(&postgres_urls[0]);
    if postgres_urls
        .iter()
        .any(|candidate| postgres_endpoint(candidate) != endpoint)
    {
        return Err(MysqlImportSpecError::Invalid(
            "all target PostgreSQL URLs must use one host and port".to_string(),
        ));
    }
    let bootstrap_database = postgres_database(&postgres_urls[0])?;
    let target_database = postgres_database(&postgres_urls[1])?;
    if bootstrap_database != "postgres" {
        return Err(MysqlImportSpecError::Invalid(
            "PostgreSQL bootstrap URL must use the postgres maintenance database of a dedicated empty cluster"
                .to_string(),
        ));
    }
    if bootstrap_database == target_database
        || postgres_urls[2..]
            .iter()
            .any(|url| postgres_database(url).ok().as_deref() != Some(&target_database))
    {
        return Err(MysqlImportSpecError::Invalid(
            "PostgreSQL bootstrap database must differ while migration/API/worker use one target database"
                .to_string(),
        ));
    }
    let usernames = postgres_urls
        .iter()
        .map(|url| strict_percent_decode(url.username(), "target.postgres URL username"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let passwords = postgres_urls
        .iter()
        .map(|url| {
            strict_percent_decode(
                url.password().unwrap_or_default(),
                "target.postgres URL password",
            )
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if usernames.len() != 4 || passwords.len() != 4 {
        return Err(MysqlImportSpecError::Invalid(
            "PostgreSQL bootstrap/migration/API/worker principals and secrets must be distinct"
                .to_string(),
        ));
    }
    validate_clickhouse_target(&target.clickhouse)?;
    validate_analytics_admission(&target.analytics_admission)?;
    validate_target_redis(&target.redis_bootstrap_url)?;
    if !target.require_empty_redis {
        return Err(MysqlImportSpecError::Invalid(
            "target.require_empty_redis must be true".to_string(),
        ));
    }
    validate_absolute_normalized_path(
        &target.config_output_directory,
        "target.config_output_directory",
    )?;
    if target.config_output_directory == Path::new("/") {
        return Err(MysqlImportSpecError::Invalid(
            "target.config_output_directory must not be the filesystem root".to_string(),
        ));
    }
    Ok(target)
}

fn validate_postgres_url(value: &str) -> Result<Url, MysqlImportSpecError> {
    let url = parse_url(value, "target.postgres URL")?;
    let username = strict_percent_decode(url.username(), "target.postgres URL username")?;
    let password = strict_percent_decode(
        url.password().unwrap_or_default(),
        "target.postgres URL password",
    )?;
    if !matches!(url.scheme(), "postgres" | "postgresql")
        || url.host_str().is_none()
        || !valid_postgres_identifier(&username)
        || password.len() < 16
        || contains_placeholder(&password)
        || url.fragment().is_some()
    {
        return Err(MysqlImportSpecError::Invalid(
            "target PostgreSQL URLs require host, database, valid principal, and non-placeholder password"
                .to_string(),
        ));
    }
    postgres_database(&url)?;
    v2board_config::validate_postgres_connection_query(&url, true).map_err(|_| {
        MysqlImportSpecError::Invalid(
            "target PostgreSQL URLs require exactly one canonical sslmode=verify-full and no connection override or duplicate query parameters"
                .to_string(),
        )
    })?;
    Ok(url)
}

fn postgres_endpoint(url: &Url) -> (String, u16) {
    (
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default().unwrap_or(5432),
    )
}

fn postgres_database(url: &Url) -> Result<String, MysqlImportSpecError> {
    let path = strict_percent_decode(url.path(), "target.postgres URL database")?;
    let value = path.strip_prefix('/').unwrap_or_default();
    if !valid_postgres_identifier(value) {
        return Err(MysqlImportSpecError::Invalid(
            "target PostgreSQL database names must be unquoted identifiers of at most 63 bytes"
                .to_string(),
        ));
    }
    Ok(value.to_string())
}

fn postgres_principal(value: &str) -> Result<String, MysqlImportSpecError> {
    let url = Url::parse(value).map_err(|_| {
        MysqlImportSpecError::Invalid("validated PostgreSQL URL became invalid".to_string())
    })?;
    let username = strict_percent_decode(url.username(), "target.postgres URL username")?;
    if !valid_postgres_identifier(&username) {
        return Err(MysqlImportSpecError::Invalid(
            "target PostgreSQL principal is invalid".to_string(),
        ));
    }
    Ok(username)
}

fn validate_clickhouse_target(
    clickhouse: &ValidatedClickHouseTarget,
) -> Result<(), MysqlImportSpecError> {
    let endpoint = parse_url(&clickhouse.endpoint, "target.clickhouse.endpoint")?;
    if endpoint.scheme() != "https"
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.path() != "/"
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || !valid_datastore_identifier(&clickhouse.database)
    {
        return Err(MysqlImportSpecError::Invalid(
            "target ClickHouse requires a credential-free HTTPS origin and an unquoted database identifier"
                .to_string(),
        ));
    }
    let principals = [
        &clickhouse.bootstrap_principal,
        &clickhouse.schema_principal,
        &clickhouse.writer_principal,
    ];
    let mut usernames = BTreeSet::new();
    let mut passwords = BTreeSet::new();
    for principal in principals {
        if !valid_datastore_identifier(&principal.username)
            || principal.password.len() < 32
            || contains_placeholder(&principal.password)
        {
            return Err(MysqlImportSpecError::Invalid(
                "ClickHouse principals require valid identifiers and non-placeholder secrets"
                    .to_string(),
            ));
        }
        usernames.insert(principal.username.as_str());
        passwords.insert(principal.password.as_str());
    }
    if usernames.len() != 3 || passwords.len() != 3 {
        return Err(MysqlImportSpecError::Invalid(
            "ClickHouse bootstrap/schema/writer principals and secrets must be distinct"
                .to_string(),
        ));
    }
    if clickhouse.raw_retention_days == 0
        || clickhouse.aggregate_retention_days < clickhouse.raw_retention_days
        || clickhouse.aggregate_retention_days > 36_500
        || !clickhouse.require_database_absent
        || !clickhouse.require_principals_absent
    {
        return Err(MysqlImportSpecError::Invalid(
            "ClickHouse target must be empty and use 1..=36500 day ordered retention".to_string(),
        ));
    }
    Ok(())
}

fn validate_analytics_admission(
    policy: &ValidatedAnalyticsAdmission,
) -> Result<(), MysqlImportSpecError> {
    let values = [
        policy.recovery_pending_rows,
        policy.soft_pending_rows,
        policy.hard_pending_rows,
        policy.recovery_relation_bytes,
        policy.soft_relation_bytes,
        policy.hard_relation_bytes,
        policy.recovery_oldest_age_seconds,
        policy.soft_oldest_age_seconds,
        policy.hard_oldest_age_seconds,
        policy.database_capacity_bytes,
        policy.hard_min_headroom_bytes,
        policy.soft_min_headroom_bytes,
        policy.recovery_min_headroom_bytes,
        policy.event_reservation_bytes,
        policy.soft_max_new_rows_per_second,
        policy.sample_interval_seconds,
        policy.stale_after_seconds,
    ];
    let ordered = policy.recovery_pending_rows < policy.soft_pending_rows
        && policy.soft_pending_rows < policy.hard_pending_rows
        && policy.recovery_relation_bytes < policy.soft_relation_bytes
        && policy.soft_relation_bytes < policy.hard_relation_bytes
        && policy.recovery_oldest_age_seconds < policy.soft_oldest_age_seconds
        && policy.soft_oldest_age_seconds < policy.hard_oldest_age_seconds
        && policy.hard_min_headroom_bytes < policy.soft_min_headroom_bytes
        && policy.soft_min_headroom_bytes < policy.recovery_min_headroom_bytes;
    if values.iter().any(|value| i64::try_from(*value).is_err())
        || !ordered
        || policy.database_capacity_bytes <= policy.recovery_min_headroom_bytes
        || policy.event_reservation_bytes == 0
        || policy.event_reservation_bytes > policy.hard_relation_bytes
        || !(100_000..=10_000_000).contains(&policy.soft_max_new_rows_per_second)
        || !(1..=60).contains(&policy.sample_interval_seconds)
        || !(policy.sample_interval_seconds.saturating_mul(2)..=600)
            .contains(&policy.stale_after_seconds)
    {
        return Err(MysqlImportSpecError::Invalid(
            "analytics admission thresholds are unordered or outside the supported signed range"
                .to_string(),
        ));
    }
    validate_evidence(
        &policy.capacity_evidence,
        "target.analytics_admission.capacity_evidence",
    )
}

fn validate_target_redis(value: &str) -> Result<(), MysqlImportSpecError> {
    let url = parse_url(value, "target.redis_bootstrap_url")?;
    let username = strict_percent_decode(url.username(), "target.redis_bootstrap_url username")?;
    let path = strict_percent_decode(url.path(), "target.redis_bootstrap_url database")?;
    let database = path.strip_prefix('/').unwrap_or_default();
    let database_number = database.parse::<u32>().ok();
    let password = strict_percent_decode(
        url.password().unwrap_or_default(),
        "target.redis_bootstrap_url password",
    )?;
    if url.scheme() != "rediss"
        || url.host_str().is_none()
        || !valid_datastore_identifier(&username)
        || username == "default"
        || database.is_empty()
        || database_number != Some(0)
        || url.path() != "/0"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.password().is_none()
        || password.len() < 32
        || contains_placeholder(&password)
    {
        return Err(MysqlImportSpecError::Invalid(
            "target Redis must use an explicit non-default bootstrap ACL username, a dedicated non-placeholder rediss endpoint, a password of at least 32 bytes, and canonical database /0"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_evidence(value: &str, field: &str) -> Result<(), MysqlImportSpecError> {
    if !(8..=1024).contains(&value.trim().len()) || contains_placeholder(value) {
        return Err(MysqlImportSpecError::Invalid(format!(
            "{field} must contain explicit 8..=1024 byte evidence"
        )));
    }
    Ok(())
}

fn valid_datastore_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
        && value.len() <= 128
}

fn valid_postgres_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 63
        && matches!(bytes.next(), Some(b'_' | b'a'..=b'z'))
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn validate_runtime(
    runtime: &Map<String, Value>,
    target: &ValidatedImportTarget,
) -> Result<(), MysqlImportSpecError> {
    let expected = FILE_ONLY_RUNTIME_KEYS_V1
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    require_exact_keys(runtime, &expected, "runtime")?;
    if runtime.get("configuration_source").and_then(Value::as_str) != Some("file_only")
        || runtime.get("environment").and_then(Value::as_str) != Some("production")
    {
        return Err(MysqlImportSpecError::Invalid(
            "runtime must use configuration_source=file_only and environment=production"
                .to_string(),
        ));
    }
    for key in BOOL_RUNTIME_KEYS {
        if !runtime.get(*key).is_some_and(Value::is_boolean) {
            return Err(MysqlImportSpecError::Invalid(format!(
                "runtime.{key} must be a JSON boolean"
            )));
        }
    }
    for key in INTEGER_RUNTIME_KEYS {
        let value = runtime.get(*key);
        if *key == "email_port" && value.is_some_and(Value::is_null) {
            continue;
        }
        if !value.is_some_and(|value| value.as_i64().is_some()) {
            return Err(MysqlImportSpecError::Invalid(format!(
                "runtime.{key} must be a JSON integer"
            )));
        }
    }
    for key in DECIMAL_RUNTIME_KEYS {
        if !runtime
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(valid_decimal_text)
        {
            return Err(MysqlImportSpecError::Invalid(format!(
                "runtime.{key} must be an exact non-negative decimal string"
            )));
        }
    }
    for key in LIST_RUNTIME_KEYS {
        if !runtime.get(*key).is_some_and(|value| {
            value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string))
        }) {
            return Err(MysqlImportSpecError::Invalid(format!(
                "runtime.{key} must be an array of strings"
            )));
        }
    }
    for (key, value) in runtime {
        if BOOL_RUNTIME_KEYS.contains(&key.as_str())
            || INTEGER_RUNTIME_KEYS.contains(&key.as_str())
            || DECIMAL_RUNTIME_KEYS.contains(&key.as_str())
            || LIST_RUNTIME_KEYS.contains(&key.as_str())
        {
            continue;
        }
        if !value.is_null() && !value.is_string() {
            return Err(MysqlImportSpecError::Invalid(format!(
                "runtime.{key} must be a string or null"
            )));
        }
    }
    if runtime.get("bind_addr").and_then(Value::as_str) != Some("127.0.0.1:8080")
        || runtime.get("force_https") != Some(&Value::Bool(true))
        || runtime.get("server_require_idempotency_key") != Some(&Value::Bool(true))
    {
        return Err(MysqlImportSpecError::Invalid(
            "production runtime requires loopback bind, HTTPS, and node idempotency keys"
                .to_string(),
        ));
    }
    let app_key = string_field(runtime, "app_key", "runtime")?;
    let server_token = string_field(runtime, "server_token", "runtime")?;
    if app_key.len() < 32
        || server_token.len() < 32
        || app_key == server_token
        || contains_placeholder(app_key)
        || contains_placeholder(server_token)
    {
        return Err(MysqlImportSpecError::Invalid(
            "runtime app_key and server_token must be distinct non-placeholder secrets of at least 32 bytes"
                .to_string(),
        ));
    }

    let api_principal = postgres_principal(&target.postgres.api_database_url)?;
    let worker_principal = postgres_principal(&target.postgres.worker_database_url)?;
    let mut api_runtime = runtime.clone();
    inject_runtime_datastore(
        &mut api_runtime,
        "api",
        &target.postgres.api_database_url,
        &worker_principal,
        &target.redis_bootstrap_url,
    );
    let mut worker_runtime = runtime.clone();
    inject_runtime_datastore(
        &mut worker_runtime,
        "worker",
        &target.postgres.worker_database_url,
        &api_principal,
        &target.redis_bootstrap_url,
    );
    worker_runtime.insert(
        "clickhouse_url".to_string(),
        Value::String(target.clickhouse.endpoint.clone()),
    );
    worker_runtime.insert(
        "clickhouse_database".to_string(),
        Value::String(target.clickhouse.database.clone()),
    );
    worker_runtime.insert(
        "clickhouse_writer_username".to_string(),
        Value::String(target.clickhouse.writer_principal.username.clone()),
    );
    worker_runtime.insert(
        "clickhouse_writer_password".to_string(),
        Value::String(target.clickhouse.writer_principal.password.clone()),
    );
    let api = AppConfig::try_from_api_config_map(
        api_runtime.clone(),
        runtime_paths(PathBuf::from("/var/lib/v2board/api/config.json")),
    )
    .map_err(|error| {
        MysqlImportSpecError::Invalid(format!(
            "runtime is not loadable by the typed API parser: {error}"
        ))
    })?;
    let worker = AppConfig::try_from_worker_config_map(
        worker_runtime.clone(),
        runtime_paths(PathBuf::from("/var/lib/v2board/worker/config.json")),
    )
    .map_err(|error| {
        MysqlImportSpecError::Invalid(format!(
            "runtime is not loadable by the typed worker parser: {error}"
        ))
    })?;
    if api.operator_config_map() != worker.operator_config_map() {
        return Err(MysqlImportSpecError::Invalid(
            "API and worker normalized different operator configuration".to_string(),
        ));
    }
    AppConfig::try_from_api_boot_config_map(
        boot_runtime_config(&api_runtime, false)?,
        runtime_paths(PathBuf::from("/var/lib/v2board/api/config.json")),
    )
    .map_err(|error| {
        MysqlImportSpecError::Invalid(format!("derived API boot config is invalid: {error}"))
    })?;
    AppConfig::try_from_worker_boot_config_map(
        boot_runtime_config(&worker_runtime, true)?,
        runtime_paths(PathBuf::from("/var/lib/v2board/worker/config.json")),
    )
    .map_err(|error| {
        MysqlImportSpecError::Invalid(format!("derived worker boot config is invalid: {error}"))
    })?;
    Ok(())
}

fn inject_runtime_datastore(
    runtime: &mut Map<String, Value>,
    role: &str,
    database_url: &str,
    peer_database_principal: &str,
    redis_url: &str,
) {
    for (key, value) in [
        ("runtime_role", role),
        ("database_url", database_url),
        ("peer_database_principal", peer_database_principal),
        ("redis_url", redis_url),
    ] {
        runtime.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn boot_runtime_config(
    full: &Map<String, Value>,
    worker: bool,
) -> Result<Map<String, Value>, MysqlImportSpecError> {
    let mut boot = BOOT_ONLY_RUNTIME_KEYS_V1
        .iter()
        .filter(|key| **key != "configuration_scope")
        .map(|key| {
            full.get(*key)
                .cloned()
                .map(|value| ((*key).to_string(), value))
                .ok_or_else(|| {
                    MysqlImportSpecError::Invalid(format!("derived boot config is missing {key}"))
                })
        })
        .collect::<Result<Map<_, _>, _>>()?;
    boot.insert(
        "configuration_scope".to_string(),
        Value::String("boot_only".to_string()),
    );
    if worker {
        for key in [
            "clickhouse_url",
            "clickhouse_database",
            "clickhouse_writer_username",
            "clickhouse_writer_password",
        ] {
            boot.insert(
                key.to_string(),
                full.get(key).cloned().ok_or_else(|| {
                    MysqlImportSpecError::Invalid(format!(
                        "derived worker boot config is missing {key}"
                    ))
                })?,
            );
        }
    }
    Ok(boot)
}

fn runtime_paths(config: PathBuf) -> RuntimePaths {
    RuntimePaths {
        config,
        frontend: PathBuf::from("/opt/v2board/frontend"),
        rules: PathBuf::from("/var/lib/v2board/rules"),
    }
}

fn validate_sha256(value: &str, field: &str) -> Result<(), MysqlImportSpecError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(MysqlImportSpecError::Invalid(format!(
            "{field} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_absolute_normalized_path(path: &Path, field: &str) -> Result<(), MysqlImportSpecError> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(MysqlImportSpecError::Invalid(format!(
            "{field} must be an absolute normalized path"
        )));
    }
    Ok(())
}

fn parse_url(value: &str, field: &str) -> Result<Url, MysqlImportSpecError> {
    if contains_placeholder(value) {
        return Err(MysqlImportSpecError::Invalid(format!(
            "{field} contains a placeholder"
        )));
    }
    Url::parse(value)
        .map_err(|_| MysqlImportSpecError::Invalid(format!("{field} is not a valid URL")))
}

fn strict_percent_decode(value: &str, field: &str) -> Result<String, MysqlImportSpecError> {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return Err(MysqlImportSpecError::Invalid(format!(
                    "{field} contains invalid percent encoding"
                )));
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
            MysqlImportSpecError::Invalid(format!("{field} contains non-UTF-8 percent encoding"))
        })
}

fn validate_mysql_connection_query(url: &Url) -> Result<Option<String>, MysqlImportSpecError> {
    let mut seen = BTreeSet::new();
    let mut ssl_mode = None;
    for (key, value) in url.query_pairs() {
        let lowercase = key.to_ascii_lowercase();
        if key.as_ref() != lowercase {
            return Err(MysqlImportSpecError::Invalid(
                "MySQL URL query parameter names must use canonical lowercase spelling".to_string(),
            ));
        }
        let canonical = lowercase.replace('_', "-");
        if !seen.insert(canonical.clone()) {
            return Err(MysqlImportSpecError::Invalid(
                "MySQL URL duplicate or aliased query parameters are forbidden".to_string(),
            ));
        }
        if matches!(
            canonical.as_str(),
            "host"
                | "hostaddr"
                | "port"
                | "db"
                | "dbname"
                | "database"
                | "user"
                | "username"
                | "password"
                | "socket"
        ) {
            return Err(MysqlImportSpecError::Invalid(
                "MySQL URL connection identity overrides are forbidden".to_string(),
            ));
        }
        if key.as_ref() == "sslmode" || key.as_ref() == "ssl_mode" {
            return Err(MysqlImportSpecError::Invalid(
                "MySQL URL sslmode aliases are forbidden; use ssl-mode".to_string(),
            ));
        }
        if canonical == "ssl-mode" {
            ssl_mode = Some(value.to_ascii_lowercase().replace('-', "_"));
        }
    }
    Ok(ssl_mode)
}

fn contains_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "replace",
        "change-me",
        "changeme",
        "your-secret",
        "your_secret",
        "your-password",
        "your_password",
        "invalid",
        "example",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn valid_decimal_text(value: &str) -> bool {
    let mut decimal_point = false;
    let mut digits = 0_usize;
    for character in value.chars() {
        if character == '.' && !decimal_point {
            decimal_point = true;
        } else if character.is_ascii_digit() {
            digits += 1;
        } else {
            return false;
        }
    }
    digits != 0 && !value.starts_with('.') && !value.ends_with('.')
}

fn require_exact_keys(
    object: &Map<String, Value>,
    expected: &BTreeSet<&str>,
    field: &str,
) -> Result<(), MysqlImportSpecError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != *expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unknown = actual.difference(expected).copied().collect::<Vec<_>>();
        return Err(MysqlImportSpecError::Invalid(format!(
            "{field} keys are incomplete or unknown; missing={missing:?}, unknown={unknown:?}"
        )));
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    parent: &str,
) -> Result<&'a str, MysqlImportSpecError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| MysqlImportSpecError::Invalid(format!("{parent}.{key} must be a string")))
}

impl MysqlImportSpec {
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// Rebuild the already-validated typed plan used by the one-shot executor.
    /// Keeping this derivation here ensures `validate`, `inspect`, and
    /// `execute` share exactly one manifest grammar and one runtime parser.
    pub fn execution_plan(&self) -> Result<MysqlImportExecutionPlan, MysqlImportSpecError> {
        let target = validate_target(&self.target)?;
        validate_runtime(&self.runtime, &target)?;

        let api_principal = postgres_principal(&target.postgres.api_database_url)?;
        let worker_principal = postgres_principal(&target.postgres.worker_database_url)?;
        let mut api_runtime = self.runtime.clone();
        inject_runtime_datastore(
            &mut api_runtime,
            "api",
            &target.postgres.api_database_url,
            &worker_principal,
            &target.redis_bootstrap_url,
        );
        let mut worker_runtime = self.runtime.clone();
        inject_runtime_datastore(
            &mut worker_runtime,
            "worker",
            &target.postgres.worker_database_url,
            &api_principal,
            &target.redis_bootstrap_url,
        );
        worker_runtime.insert(
            "clickhouse_url".to_string(),
            Value::String(target.clickhouse.endpoint.clone()),
        );
        worker_runtime.insert(
            "clickhouse_database".to_string(),
            Value::String(target.clickhouse.database.clone()),
        );
        worker_runtime.insert(
            "clickhouse_writer_username".to_string(),
            Value::String(target.clickhouse.writer_principal.username.clone()),
        );
        worker_runtime.insert(
            "clickhouse_writer_password".to_string(),
            Value::String(target.clickhouse.writer_principal.password.clone()),
        );

        let api = AppConfig::try_from_api_config_map(
            api_runtime.clone(),
            runtime_paths(PathBuf::from("/var/lib/v2board/api/config.json")),
        )
        .map_err(|error| {
            MysqlImportSpecError::Invalid(format!(
                "runtime is not loadable by the typed API parser: {error}"
            ))
        })?;
        let operator_config = api.operator_config_map();
        let app_key = string_field(&self.runtime, "app_key", "runtime")?.to_string();

        Ok(MysqlImportExecutionPlan {
            postgres: MysqlImportPostgresPlan {
                bootstrap_database_url: target.postgres.bootstrap_database_url,
                migration_database_url: target.postgres.migration_database_url,
                api_database_url: target.postgres.api_database_url,
                worker_database_url: target.postgres.worker_database_url,
            },
            clickhouse: MysqlImportClickHousePlan {
                endpoint: target.clickhouse.endpoint,
                database: target.clickhouse.database,
                bootstrap_username: target.clickhouse.bootstrap_principal.username,
                bootstrap_password: target.clickhouse.bootstrap_principal.password,
                schema_username: target.clickhouse.schema_principal.username,
                schema_password: target.clickhouse.schema_principal.password,
                writer_username: target.clickhouse.writer_principal.username,
                writer_password: target.clickhouse.writer_principal.password,
                raw_retention_days: target.clickhouse.raw_retention_days,
                aggregate_retention_days: target.clickhouse.aggregate_retention_days,
            },
            analytics_admission: MysqlImportAnalyticsAdmissionPlan {
                recovery_pending_rows: target.analytics_admission.recovery_pending_rows,
                soft_pending_rows: target.analytics_admission.soft_pending_rows,
                hard_pending_rows: target.analytics_admission.hard_pending_rows,
                recovery_relation_bytes: target.analytics_admission.recovery_relation_bytes,
                soft_relation_bytes: target.analytics_admission.soft_relation_bytes,
                hard_relation_bytes: target.analytics_admission.hard_relation_bytes,
                recovery_oldest_age_seconds: target.analytics_admission.recovery_oldest_age_seconds,
                soft_oldest_age_seconds: target.analytics_admission.soft_oldest_age_seconds,
                hard_oldest_age_seconds: target.analytics_admission.hard_oldest_age_seconds,
                database_capacity_bytes: target.analytics_admission.database_capacity_bytes,
                hard_min_headroom_bytes: target.analytics_admission.hard_min_headroom_bytes,
                soft_min_headroom_bytes: target.analytics_admission.soft_min_headroom_bytes,
                recovery_min_headroom_bytes: target.analytics_admission.recovery_min_headroom_bytes,
                event_reservation_bytes: target.analytics_admission.event_reservation_bytes,
                soft_max_new_rows_per_second: target
                    .analytics_admission
                    .soft_max_new_rows_per_second,
                sample_interval_seconds: target.analytics_admission.sample_interval_seconds,
                stale_after_seconds: target.analytics_admission.stale_after_seconds,
                capacity_evidence: target.analytics_admission.capacity_evidence,
            },
            redis_bootstrap_url: target.redis_bootstrap_url,
            config_output_directory: target.config_output_directory,
            api_boot_config: boot_runtime_config(&api_runtime, false)?,
            worker_boot_config: boot_runtime_config(&worker_runtime, true)?,
            operator_config,
            app_key,
        })
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

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::*;

    fn runtime() -> Map<String, Value> {
        let mut runtime = FILE_ONLY_RUNTIME_KEYS_V1
            .iter()
            .map(|key| ((*key).to_string(), Value::Null))
            .collect::<Map<_, _>>();
        for key in BOOL_RUNTIME_KEYS {
            runtime.insert((*key).to_string(), json!(false));
        }
        for key in INTEGER_RUNTIME_KEYS {
            runtime.insert((*key).to_string(), json!(0));
        }
        runtime.insert("email_port".to_string(), Value::Null);
        for key in DECIMAL_RUNTIME_KEYS {
            runtime.insert((*key).to_string(), json!("1"));
        }
        for key in LIST_RUNTIME_KEYS {
            runtime.insert((*key).to_string(), json!([]));
        }
        for (key, value) in [
            ("configuration_source", json!("file_only")),
            ("environment", json!("production")),
            ("bind_addr", json!("127.0.0.1:8080")),
            ("app_key", json!("A7m2Q9x4K8p3V6n1R5t0Y2u7I4o9P6s3")),
            ("app_name", json!("V2Board")),
            ("app_url", json!("https://panel.acme.internal")),
            ("app_description", json!("V2Board")),
            ("email_template", json!("default")),
            ("currency", json!("CNY")),
            ("currency_symbol", json!("¥")),
            ("subscribe_url", json!("https://panel.acme.internal")),
            ("subscribe_path", json!("/api/v1/client/subscribe")),
            ("server_token", json!("B8n3R0y5L9q4W7m2S6u1Z3v8J5p0Q7t4")),
            ("server_api_url", json!("https://panel.acme.internal")),
            ("frontend_theme_color", json!("default")),
            ("secure_path", json!("private_admin")),
        ] {
            runtime.insert(key.to_string(), value);
        }
        for (key, value) in [
            ("http_connect_timeout_seconds", 10),
            ("http_request_timeout_seconds", 30),
            ("api_request_timeout_seconds", 45),
            ("password_kdf_max_parallel", 4),
            ("auth_session_ttl_seconds", 2_592_000),
            ("privileged_auth_session_ttl_seconds", 1_800),
            ("auth_session_max_per_user", 20),
            ("privileged_step_up_ttl_seconds", 900),
            ("privileged_step_up_max_attempts", 5),
            ("privileged_step_up_attempt_window_seconds", 900),
            ("register_limit_count", 3),
            ("register_limit_expire", 60),
            ("show_subscribe_expire", 5),
            ("invite_commission", 10),
            ("invite_gen_limit", 5),
            ("server_push_interval", 60),
            ("server_pull_interval", 60),
            ("password_limit_count", 5),
            ("password_limit_expire", 60),
        ] {
            runtime.insert(key.to_string(), json!(value));
        }
        for key in [
            "privileged_step_up_enable",
            "force_https",
            "register_limit_by_ip_enable",
            "withdraw_close_enable",
            "commission_auto_check_enable",
            "plan_change_enable",
            "surplus_enable",
            "commission_first_time_enable",
            "server_require_idempotency_key",
            "password_limit_enable",
        ] {
            runtime.insert(key.to_string(), json!(true));
        }
        runtime.insert(
            "cors_allowed_origins".to_string(),
            json!(["https://panel.acme.internal"]),
        );
        runtime.insert("trusted_proxy_cidrs".to_string(), json!(["10.0.0.0/8"]));
        runtime
    }

    fn import_document() -> Value {
        json!({
            "schema_version": 1,
            "source": {
                "dump_path": "/secure/legacy.sql",
                "dump_sha256": "a".repeat(64),
                "database_url": "mysql://legacy_reader:J0legacyReadOnlySecret@127.0.0.1:3306/v2board"
            },
            "target": {
                "postgres": {
                    "bootstrap_database_url": "postgresql://bootstrap:A1bootstrapSecret@postgres.acme.internal:5432/postgres?sslmode=verify-full",
                    "migration_database_url": "postgresql://migration:B2migrationSecret@postgres.acme.internal:5432/v2board?sslmode=verify-full",
                    "api_database_url": "postgresql://api:C3apiRuntimeSecret@postgres.acme.internal:5432/v2board?sslmode=verify-full",
                    "worker_database_url": "postgresql://worker:D4workerRuntimeSecret@postgres.acme.internal:5432/v2board?sslmode=verify-full",
                    "require_database_absent": true,
                    "require_roles_absent": true
                },
                "clickhouse": {
                    "endpoint": "https://clickhouse.acme.internal",
                    "database": "v2board_analytics",
                    "bootstrap_principal": {"username": "ch_bootstrap", "password": "E5clickhouseBootstrapSecretMaterial32"},
                    "schema_principal": {"username": "ch_schema", "password": "F6clickhouseSchemaSecretMaterialValue32"},
                    "writer_principal": {"username": "ch_writer", "password": "G7clickhouseWriterSecretMaterialValue32"},
                    "raw_retention_days": 90,
                    "aggregate_retention_days": 730,
                    "require_database_absent": true,
                    "require_principals_absent": true
                },
                "analytics_admission": {
                    "recovery_pending_rows": 750000,
                    "soft_pending_rows": 1000000,
                    "hard_pending_rows": 2000000,
                    "recovery_relation_bytes": 3221225472_u64,
                    "soft_relation_bytes": 4294967296_u64,
                    "hard_relation_bytes": 8589934592_u64,
                    "recovery_oldest_age_seconds": 120,
                    "soft_oldest_age_seconds": 300,
                    "hard_oldest_age_seconds": 1800,
                    "database_capacity_bytes": 68719476736_u64,
                    "hard_min_headroom_bytes": 8589934592_u64,
                    "soft_min_headroom_bytes": 17179869184_u64,
                    "recovery_min_headroom_bytes": 21474836480_u64,
                    "event_reservation_bytes": 4096,
                    "soft_max_new_rows_per_second": 100000,
                    "sample_interval_seconds": 5,
                    "stale_after_seconds": 30,
                    "capacity_evidence": "dedicated-postgresql-volume-quota-1042"
                },
                "redis_bootstrap_url": "rediss://import_bootstrap:I9redisRuntimeSecret-32-bytes-long@redis.acme.internal:6380/0",
                "config_output_directory": "/secure/private/v2board-import-output",
                "require_empty_redis": true
            },
            "runtime": runtime()
        })
    }

    #[test]
    fn accepts_the_single_pre_release_import_shape() {
        let spec =
            parse_mysql_import_spec(&serde_json::to_vec(&import_document()).unwrap()).unwrap();
        assert_eq!(spec.schema_version, 1);
        assert_eq!(spec.manifest_sha256().len(), 64);

        for host in ["localhost", "127.0.0.1", "127.20.30.40", "[::1]"] {
            let mut loopback = import_document();
            loopback["source"]["database_url"] = json!(format!(
                "mysql://legacy_reader:J0legacyReadOnlySecret@{host}:3306/v2board"
            ));
            assert!(parse_mysql_import_spec(&serde_json::to_vec(&loopback).unwrap()).is_ok());
        }
    }

    #[test]
    fn accepts_only_schema_version_one_and_known_fields() {
        for version in [0, 2] {
            let mut document = import_document();
            document["schema_version"] = json!(version);
            assert!(parse_mysql_import_spec(&serde_json::to_vec(&document).unwrap()).is_err());
        }
        let mut document = import_document();
        document
            .as_object_mut()
            .unwrap()
            .insert("unknown_field".to_string(), json!(true));
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&document).unwrap()).is_err());
    }

    #[test]
    fn duplicate_keys_fail_and_sha256_binds_exact_file_bytes() {
        let document = import_document();
        let compact = serde_json::to_vec(&document).unwrap();
        let pretty = serde_json::to_vec_pretty(&document).unwrap();
        let compact_binding = parse_mysql_import_spec(&compact)
            .unwrap()
            .manifest_sha256()
            .to_string();
        let pretty_binding = parse_mysql_import_spec(&pretty)
            .unwrap()
            .manifest_sha256()
            .to_string();
        assert_ne!(compact_binding, pretty_binding);

        let duplicate = String::from_utf8(compact).unwrap().replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
            1,
        );
        let error = parse_mysql_import_spec(duplicate.as_bytes())
            .err()
            .expect("duplicate manifest keys must fail");
        assert!(error.to_string().contains("duplicate JSON key"));
    }

    #[test]
    fn target_runtime_and_tls_shapes_fail_closed() {
        let mut missing_analytics = import_document();
        missing_analytics["target"]["analytics_admission"]
            .as_object_mut()
            .unwrap()
            .remove("capacity_evidence");
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&missing_analytics).unwrap()).is_err());

        let mut unknown_nested = import_document();
        unknown_nested["target"]["clickhouse"]
            .as_object_mut()
            .unwrap()
            .insert("unknown_field".to_string(), json!(true));
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&unknown_nested).unwrap()).is_err());

        let mut wrong_runtime_type = import_document();
        wrong_runtime_type["runtime"]["force_https"] = json!("true");
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&wrong_runtime_type).unwrap()).is_err()
        );

        let mut duplicate_postgres_tls = import_document();
        duplicate_postgres_tls["target"]["postgres"]["api_database_url"] = json!(
            "postgresql://api:C3apiRuntimeSecret@postgres.acme.internal:5432/v2board?sslmode=disable&sslmode=verify-full"
        );
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&duplicate_postgres_tls).unwrap()).is_err()
        );

        let mut shared_postgres_cluster = import_document();
        shared_postgres_cluster["target"]["postgres"]["bootstrap_database_url"] = json!(
            "postgresql://bootstrap:A1bootstrapSecret@postgres.acme.internal:5432/maintenance?sslmode=verify-full"
        );
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&shared_postgres_cluster).unwrap())
                .is_err()
        );

        let mut duplicate_mysql_tls = import_document();
        duplicate_mysql_tls["source"]["database_url"] = json!(
            "mysql://legacy_reader:J0legacyReadOnlySecret@127.0.0.1:3306/v2board?ssl_mode=DISABLED&ssl-mode=VERIFY_IDENTITY"
        );
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&duplicate_mysql_tls).unwrap()).is_err()
        );

        let mut remote_source = import_document();
        remote_source["source"]["database_url"] = json!(
            "mysql://legacy_reader:J0legacyReadOnlySecret@legacy.internal:3306/v2board?ssl-mode=VERIFY_IDENTITY"
        );
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&remote_source).unwrap()).is_err());

        let mut retired_redis_name = import_document();
        let target = retired_redis_name["target"].as_object_mut().unwrap();
        let value = target.remove("redis_bootstrap_url").unwrap();
        target.insert("redis_url".to_string(), value);
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&retired_redis_name).unwrap()).is_err()
        );

        let mut default_redis_user = import_document();
        default_redis_user["target"]["redis_bootstrap_url"] =
            json!("rediss://default:I9redisRuntimeSecret-32-bytes-long@redis.acme.internal:6380/0");
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&default_redis_user).unwrap()).is_err()
        );

        let mut missing_redis_user = import_document();
        missing_redis_user["target"]["redis_bootstrap_url"] =
            json!("rediss://:I9redisRuntimeSecret-32-bytes-long@redis.acme.internal:6380/0");
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&missing_redis_user).unwrap()).is_err()
        );
    }

    #[test]
    fn encoded_credentials_are_decoded_before_validation_and_identity_comparison() {
        let mut encoded_special_characters = import_document();
        encoded_special_characters["source"]["database_url"] =
            json!("mysql://legacy_reader:J0legacy%40ReadOnlySecret@127.0.0.1:3306/v2board");
        encoded_special_characters["target"]["postgres"]["api_database_url"] = json!(
            "postgresql://api:C3api%40RuntimeSecret@postgres.acme.internal:5432/v2board?sslmode=verify-full"
        );
        encoded_special_characters["target"]["redis_bootstrap_url"] = json!(
            "rediss://import_bootstrap:I9redis%40RuntimeSecret-32-bytes-long@redis.acme.internal:6380/0"
        );
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&encoded_special_characters).unwrap())
                .is_ok()
        );

        let mut encoded_placeholder = import_document();
        encoded_placeholder["target"]["postgres"]["api_database_url"] = json!(
            "postgresql://api:C3api%52eplaceSecret@postgres.acme.internal:5432/v2board?sslmode=verify-full"
        );
        assert!(
            parse_mysql_import_spec(&serde_json::to_vec(&encoded_placeholder).unwrap()).is_err()
        );

        let mut encoded_duplicate = import_document();
        encoded_duplicate["target"]["postgres"]["migration_database_url"] = json!(
            "postgresql://migration:A1bootstrap%53ecret@postgres.acme.internal:5432/v2board?sslmode=verify-full"
        );
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&encoded_duplicate).unwrap()).is_err());

        assert!(strict_percent_decode("%GG", "test").is_err());
    }

    #[test]
    fn postgres_identifiers_are_unquoted_and_bounded() {
        let mut long_principal = import_document();
        long_principal["target"]["postgres"]["bootstrap_database_url"] = json!(format!(
            "postgresql://{}:A1bootstrapSecret@postgres.acme.internal:5432/postgres?sslmode=verify-full",
            "a".repeat(64)
        ));
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&long_principal).unwrap()).is_err());

        let mut folded_principal = import_document();
        folded_principal["target"]["postgres"]["api_database_url"] = json!(
            "postgresql://%41pi:C3apiRuntimeSecret@postgres.acme.internal:5432/v2board?sslmode=verify-full"
        );
        assert!(parse_mysql_import_spec(&serde_json::to_vec(&folded_principal).unwrap()).is_err());
    }
}
