use std::{
    collections::BTreeSet,
    fs, io,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::percent_decode_str;
use serde::{
    Deserialize, Serialize,
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

// V3 deliberately requires a complete runtime document. Adding a new runtime
// setting requires a new spec version or an explicit compatibility decision;
// no lifecycle path may acquire new behavior from an implicit default.
const RUNTIME_KEYS_V1: &[&str] = FILE_ONLY_RUNTIME_KEYS_V1;
const MANIFEST_HMAC_DOMAIN_V3: &[u8] = b"v2board-provision-manifest-v3\0";
pub(crate) const REPORT_HMAC_DOMAIN_V3: &[u8] = b"v2board-provision-report-v3\0";

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

pub struct ProvisionSpec {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: ProvisionKind,
    lifecycle_audit_key: String,
    pub(crate) flow: ProvisionFlow,
    manifest_binding_hmac_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionKind {
    FreshInstall,
    LegacyReferenceMigration,
    NativeUpgrade,
}

pub(crate) enum ProvisionFlow {
    FreshInstall {
        target: TargetSpec,
        runtime: Map<String, Value>,
        decisions: FreshInstallDecisionSpec,
        attestations: FreshInstallAttestationSpec,
    },
    LegacyReferenceMigration {
        reference_commit: String,
        source: SourceSpec,
        target: TargetSpec,
        runtime: Map<String, Value>,
        decisions: LegacyDecisionSpec,
        attestations: LegacyAttestationSpec,
    },
    NativeUpgrade {
        current: NativeInstallationSpec,
        runtime: Map<String, Value>,
        changes: NativeUpgradeChangeSpec,
        decisions: NativeUpgradeDecisionSpec,
        attestations: NativeUpgradeAttestationSpec,
    },
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
    pub postgres: PostgresTargetSpec,
    pub clickhouse: ClickHouseTargetSpec,
    pub redis_url: String,
    pub api_runtime_config_path: PathBuf,
    pub worker_runtime_config_path: PathBuf,
    pub require_empty_redis: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresTargetSpec {
    pub bootstrap_database_url: String,
    pub migration_database_url: String,
    pub api_database_url: String,
    pub worker_database_url: String,
    pub database_collation: String,
    pub database_ctype: String,
    pub require_database_absent: bool,
    pub require_roles_absent: bool,
    pub external_access: PostgresExternalAccessSpec,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresExternalAccessSpec {
    pub pg_hba_managed_externally: bool,
    pub pg_hba_evidence: String,
    pub network_policy_managed_externally: bool,
    pub network_policy_evidence: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickHouseTargetSpec {
    pub endpoint: String,
    pub database: String,
    pub bootstrap_principal: ClickHousePrincipalSpec,
    pub schema_principal: ClickHousePrincipalSpec,
    pub writer_principal: ClickHousePrincipalSpec,
    pub reader_principal: ClickHousePrincipalSpec,
    pub raw_retention_days: u32,
    pub aggregate_retention_days: u32,
    pub require_database_absent: bool,
    pub require_principals_absent: bool,
    pub require_standalone_non_replicated: bool,
    pub network_policy_evidence: String,
    pub privileges: ClickHousePrivilegeDeclarationSpec,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickHousePrincipalSpec {
    pub username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickHousePrivilegeDeclarationSpec {
    pub bootstrap_manages_database_and_principals: bool,
    pub schema_has_ddl_metadata_read_and_ledger_write_only: bool,
    pub writer_is_insert_and_verify_only: bool,
    pub reader_is_select_only: bool,
    pub evidence: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshInstallDecisionSpec {
    pub initialize_empty_targets: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshInstallAttestationSpec {
    pub target_capacity_reviewed: bool,
    pub external_controls_reviewed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyDecisionSpec {
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
    OneShotOfflineCutover,
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
pub struct LegacyAttestationSpec {
    pub source_writers_stopped: bool,
    pub source_workers_stopped: bool,
    pub node_reporters_stopped: bool,
    pub legacy_queues_drained: bool,
    pub backup_reference: Option<String>,
    pub restore_tested: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeInstallationSpec {
    pub installation_id: String,
    pub current_build_id: String,
    pub postgres_schema_epoch: u64,
    pub clickhouse_schema_epoch: u64,
    pub migration_database_url: String,
    pub api_database_url: String,
    pub worker_database_url: String,
    pub clickhouse_endpoint: String,
    pub clickhouse_database: String,
    pub clickhouse_schema_principal: ClickHousePrincipalSpec,
    pub clickhouse_writer_principal: ClickHousePrincipalSpec,
    pub clickhouse_reader_principal: ClickHousePrincipalSpec,
    pub clickhouse_privileges: NativeClickHousePrivilegeDeclarationSpec,
    pub redis_url: String,
    pub api_runtime_config_path: PathBuf,
    pub worker_runtime_config_path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeClickHousePrivilegeDeclarationSpec {
    pub schema_has_ddl_metadata_read_and_ledger_write_only: bool,
    pub writer_is_insert_and_verify_only: bool,
    pub reader_is_select_only: bool,
    pub evidence: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUpgradeImpactSpec {
    pub resource: String,
    pub impact: String,
    pub rollback: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUpgradeChangeSpec {
    pub target_build_id: String,
    pub target_postgres_schema_epoch: u64,
    pub target_clickhouse_schema_epoch: u64,
    pub destructive_changes: Vec<NativeUpgradeImpactSpec>,
    pub ttl_shortening: Vec<NativeUpgradeImpactSpec>,
    pub drop_operations: Vec<NativeUpgradeImpactSpec>,
    pub repartition_operations: Vec<NativeUpgradeImpactSpec>,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NativeUpgradeStrategy {
    MaintenanceCutover,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUpgradeDecisionSpec {
    pub strategy: NativeUpgradeStrategy,
    pub allow_destructive_changes: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSecondConfirmationSpec {
    pub operation_id: String,
    pub prior_report_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUpgradeAttestationSpec {
    pub maintenance_window_approved: bool,
    pub backup_reference: Option<String>,
    pub restore_tested: bool,
    pub impact_reviewed: bool,
    pub second_confirmation: Option<NativeSecondConfirmationSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FreshInstallDocument {
    schema_version: u32,
    operation_id: String,
    kind: ProvisionKind,
    lifecycle_audit_key: String,
    target: TargetSpec,
    runtime: Map<String, Value>,
    decisions: FreshInstallDecisionSpec,
    attestations: FreshInstallAttestationSpec,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMigrationDocument {
    schema_version: u32,
    operation_id: String,
    kind: ProvisionKind,
    reference_commit: String,
    lifecycle_audit_key: String,
    source: SourceSpec,
    target: TargetSpec,
    runtime: Map<String, Value>,
    decisions: LegacyDecisionSpec,
    attestations: LegacyAttestationSpec,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeUpgradeDocument {
    schema_version: u32,
    operation_id: String,
    kind: ProvisionKind,
    lifecycle_audit_key: String,
    current: NativeInstallationSpec,
    runtime: Map<String, Value>,
    changes: NativeUpgradeChangeSpec,
    decisions: NativeUpgradeDecisionSpec,
    attestations: NativeUpgradeAttestationSpec,
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
    #[error("unsupported provision spec schema_version; expected 3")]
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
    if unique.0.get("schema_version").and_then(Value::as_u64) != Some(3) {
        return Err(ProvisionSpecError::SchemaVersion);
    }
    let kind = serde_json::from_value::<ProvisionKind>(
        unique.0.get("kind").cloned().unwrap_or(Value::Null),
    )
    .map_err(ProvisionSpecError::Json)?;
    let mut spec = match kind {
        ProvisionKind::FreshInstall => {
            let document = serde_json::from_value::<FreshInstallDocument>(unique.0)
                .map_err(ProvisionSpecError::Json)?;
            ProvisionSpec {
                schema_version: document.schema_version,
                operation_id: document.operation_id,
                kind: document.kind,
                lifecycle_audit_key: document.lifecycle_audit_key,
                flow: ProvisionFlow::FreshInstall {
                    target: document.target,
                    runtime: document.runtime,
                    decisions: document.decisions,
                    attestations: document.attestations,
                },
                manifest_binding_hmac_sha256: String::new(),
            }
        }
        ProvisionKind::LegacyReferenceMigration => {
            let document = serde_json::from_value::<LegacyMigrationDocument>(unique.0)
                .map_err(ProvisionSpecError::Json)?;
            ProvisionSpec {
                schema_version: document.schema_version,
                operation_id: document.operation_id,
                kind: document.kind,
                lifecycle_audit_key: document.lifecycle_audit_key,
                flow: ProvisionFlow::LegacyReferenceMigration {
                    reference_commit: document.reference_commit,
                    source: document.source,
                    target: document.target,
                    runtime: document.runtime,
                    decisions: document.decisions,
                    attestations: document.attestations,
                },
                manifest_binding_hmac_sha256: String::new(),
            }
        }
        ProvisionKind::NativeUpgrade => {
            let document = serde_json::from_value::<NativeUpgradeDocument>(unique.0)
                .map_err(ProvisionSpecError::Json)?;
            ProvisionSpec {
                schema_version: document.schema_version,
                operation_id: document.operation_id,
                kind: document.kind,
                lifecycle_audit_key: document.lifecycle_audit_key,
                flow: ProvisionFlow::NativeUpgrade {
                    current: document.current,
                    runtime: document.runtime,
                    changes: document.changes,
                    decisions: document.decisions,
                    attestations: document.attestations,
                },
                manifest_binding_hmac_sha256: String::new(),
            }
        }
    };
    validate_spec(&spec)?;
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(spec.lifecycle_audit_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(MANIFEST_HMAC_DOMAIN_V3);
    mac.update(&bytes);
    spec.manifest_binding_hmac_sha256 = hex::encode(mac.finalize().into_bytes());
    Ok(spec)
}

fn validate_spec(spec: &ProvisionSpec) -> Result<(), ProvisionSpecError> {
    if spec.schema_version != 3 {
        return Err(ProvisionSpecError::SchemaVersion);
    }
    Uuid::parse_str(&spec.operation_id).map_err(|_| ProvisionSpecError::OperationId)?;
    let runtime = spec.runtime();
    validate_runtime(runtime)?;
    if is_placeholder(&spec.lifecycle_audit_key, 32) {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be an independent non-placeholder secret of at least 32 bytes",
        ));
    }
    if ["app_key", "server_token"].iter().any(|key| {
        runtime.get(*key).and_then(Value::as_str) == Some(spec.lifecycle_audit_key.as_str())
    }) {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be different from runtime.app_key and runtime.server_token",
        ));
    }
    validate_flow(spec)?;
    if spec
        .target_secret_values()?
        .iter()
        .any(|secret| secret == &spec.lifecycle_audit_key)
    {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be different from target datastore passwords",
        ));
    }
    AppConfig::try_from_api_config_map(
        spec.materialized_api_runtime_config()?,
        RuntimePaths {
            config: spec.api_runtime_config_path().to_path_buf(),
            frontend: PathBuf::from("/opt/v2board/frontend"),
            rules: PathBuf::from("/var/lib/v2board/rules"),
        },
    )
    .map_err(|error| ProvisionSpecError::RuntimeSemantics(error.to_string()))?;
    AppConfig::try_from_worker_config_map(
        spec.materialized_worker_runtime_config()?,
        RuntimePaths {
            config: spec.worker_runtime_config_path().to_path_buf(),
            frontend: PathBuf::from("/opt/v2board/frontend"),
            rules: PathBuf::from("/var/lib/v2board/rules"),
        },
    )
    .map_err(|error| ProvisionSpecError::RuntimeSemantics(error.to_string()))?;
    Ok(())
}

fn validate_flow(spec: &ProvisionSpec) -> Result<(), ProvisionSpecError> {
    match &spec.flow {
        ProvisionFlow::FreshInstall {
            target, decisions, ..
        } => {
            validate_target(target)?;
            if !decisions.initialize_empty_targets {
                return Err(ProvisionSpecError::Validation(
                    "fresh_install must explicitly choose initialize_empty_targets=true",
                ));
            }
        }
        ProvisionFlow::LegacyReferenceMigration {
            reference_commit,
            source,
            target,
            decisions,
            attestations,
            ..
        } => {
            if reference_commit != LEGACY_REFERENCE_COMMIT {
                return Err(ProvisionSpecError::ReferenceCommit);
            }
            if source.legacy_cache_driver != LegacyCacheDriver::Redis
                || decisions.legacy_configuration != LegacyConfigurationDecision::ManualOnly
                || decisions.sessions != SessionDecision::LogoutAll
                || decisions.legacy_cache != LegacyCacheDecision::DiscardEphemeralAfterFence
                || decisions.legacy_stripe != LegacyStripeDecision::AssertNone
                || decisions.legacy_subscription_tokens
                    != LegacySubscriptionTokenDecision::AssertNone
                || decisions.nodes != NodeDecision::OneShotOfflineCutover
                || decisions.legacy_theme != LegacyThemeDecision::DiscardConfirmed
                || !matches!(
                    decisions.legacy_custom_rules,
                    LegacyCustomRulesDecision::None | LegacyCustomRulesDecision::DiscardConfirmed
                )
            {
                return Err(ProvisionSpecError::Validation(
                    "legacy migration decisions must use manual config, logout-all, discard-fenced cache, zero Stripe/tokens, and one-shot offline cutover",
                ));
            }
            validate_legacy_source(source, target)?;
            validate_target(target)?;
            if attestations
                .backup_reference
                .as_deref()
                .is_some_and(|reference| is_placeholder(reference, 8))
            {
                return Err(ProvisionSpecError::Validation(
                    "legacy attestations.backup_reference must be an explicit snapshot identifier",
                ));
            }
        }
        ProvisionFlow::NativeUpgrade {
            current,
            changes,
            decisions,
            attestations,
            ..
        } => {
            if decisions.strategy != NativeUpgradeStrategy::MaintenanceCutover {
                return Err(ProvisionSpecError::Validation(
                    "native upgrade strategy must be maintenance_cutover",
                ));
            }
            validate_native_upgrade(&spec.operation_id, current, changes, attestations)?;
        }
    }
    Ok(())
}

fn validate_legacy_source(
    source: &SourceSpec,
    target: &TargetSpec,
) -> Result<(), ProvisionSpecError> {
    validate_mysql_url(&source.database_url, "source.database_url")?;
    validate_redis_url(&source.redis_default_url, "source.redis_default_url")?;
    validate_redis_url(&source.redis_cache_url, "source.redis_cache_url")?;
    if [&source.redis_connection_prefix, &source.redis_cache_prefix]
        .iter()
        .any(|prefix| {
            prefix.chars().any(|character| {
                character.is_control() || matches!(character, '*' | '?' | '[' | ']' | '\\')
            })
        })
    {
        return Err(ProvisionSpecError::Validation(
            "source Redis prefixes must not contain glob or control characters",
        ));
    }
    if source.transport_security == SourceTransportSecurity::VerifiedTls
        && (!mysql_url_verifies_identity(&source.database_url)
            || !redis_url_uses_tls(&source.redis_default_url)
            || !redis_url_uses_tls(&source.redis_cache_url))
    {
        return Err(ProvisionSpecError::Validation(
            "source verified_tls requires MySQL VERIFY_IDENTITY and rediss:// for both Redis databases",
        ));
    }
    if !(0..=2).contains(&source.legacy_show_subscribe_method) {
        return Err(ProvisionSpecError::Validation(
            "source.legacy_show_subscribe_method must be 0, 1, or 2",
        ));
    }
    if !(1..=525_600).contains(&source.legacy_show_subscribe_expire_minutes) {
        return Err(ProvisionSpecError::Validation(
            "source.legacy_show_subscribe_expire_minutes must be between 1 and 525600",
        ));
    }
    let target_redis_identity = datastore_identity(&target.redis_url)?;
    if datastore_identity(&source.redis_default_url)? == target_redis_identity
        || datastore_identity(&source.redis_cache_url)? == target_redis_identity
    {
        return Err(ProvisionSpecError::Validation(
            "both source Redis databases must be different from target Redis",
        ));
    }
    Ok(())
}

fn validate_target(target: &TargetSpec) -> Result<(), ProvisionSpecError> {
    if target.api_runtime_config_path != Path::new("/var/lib/v2board/api/config.json")
        || target.worker_runtime_config_path != Path::new("/var/lib/v2board/worker/config.json")
    {
        return Err(ProvisionSpecError::Validation(
            "target API/worker runtime config paths must be /var/lib/v2board/api/config.json and /var/lib/v2board/worker/config.json in v3",
        ));
    }
    if !target.postgres.require_database_absent
        || !target.postgres.require_roles_absent
        || !target.clickhouse.require_database_absent
        || !target.clickhouse.require_principals_absent
        || !target.clickhouse.require_standalone_non_replicated
        || !target.require_empty_redis
    {
        return Err(ProvisionSpecError::Validation(
            "fresh and legacy targets must declare empty PostgreSQL, ClickHouse, and Redis state with absent target principals",
        ));
    }
    validate_target_postgres(&target.postgres)?;
    validate_target_clickhouse(&target.clickhouse)?;
    validate_redis_url(&target.redis_url, "target.redis_url")?;
    if !redis_url_uses_tls(&target.redis_url) {
        return Err(ProvisionSpecError::Validation(
            "target.redis_url must use rediss:// with certificate verification",
        ));
    }
    Ok(())
}

fn validate_target_postgres(postgres: &PostgresTargetSpec) -> Result<(), ProvisionSpecError> {
    let urls = [
        (
            "target.postgres.bootstrap_database_url",
            &postgres.bootstrap_database_url,
        ),
        (
            "target.postgres.migration_database_url",
            &postgres.migration_database_url,
        ),
        (
            "target.postgres.api_database_url",
            &postgres.api_database_url,
        ),
        (
            "target.postgres.worker_database_url",
            &postgres.worker_database_url,
        ),
    ];
    for (field, value) in urls {
        validate_postgres_url(value, field)?;
    }
    validate_postgres_url_set(
        &postgres.bootstrap_database_url,
        &postgres.migration_database_url,
        &postgres.api_database_url,
        &postgres.worker_database_url,
    )?;
    if postgres.database_collation != "C.UTF-8" || postgres.database_ctype != "C.UTF-8" {
        return Err(ProvisionSpecError::Validation(
            "target PostgreSQL database_collation and database_ctype must both be C.UTF-8",
        ));
    }
    let access = &postgres.external_access;
    if !access.pg_hba_managed_externally
        || !access.network_policy_managed_externally
        || is_placeholder(&access.pg_hba_evidence, 8)
        || is_placeholder(&access.network_policy_evidence, 8)
    {
        return Err(ProvisionSpecError::Validation(
            "target PostgreSQL requires explicit external pg_hba and network-policy evidence",
        ));
    }
    Ok(())
}

fn validate_target_clickhouse(clickhouse: &ClickHouseTargetSpec) -> Result<(), ProvisionSpecError> {
    validate_clickhouse_endpoint(&clickhouse.endpoint)?;
    if !valid_datastore_identifier(&clickhouse.database) {
        return Err(ProvisionSpecError::Validation(
            "target ClickHouse database must be an unquoted ASCII identifier",
        ));
    }
    validate_clickhouse_principals([
        &clickhouse.bootstrap_principal,
        &clickhouse.schema_principal,
        &clickhouse.writer_principal,
        &clickhouse.reader_principal,
    ])?;
    if clickhouse.raw_retention_days == 0
        || clickhouse.aggregate_retention_days < clickhouse.raw_retention_days
        || clickhouse.aggregate_retention_days > 36_500
    {
        return Err(ProvisionSpecError::Validation(
            "ClickHouse retention must be nonzero, aggregate >= raw, and at most 36500 days",
        ));
    }
    let privileges = &clickhouse.privileges;
    if !privileges.bootstrap_manages_database_and_principals
        || !privileges.schema_has_ddl_metadata_read_and_ledger_write_only
        || !privileges.writer_is_insert_and_verify_only
        || !privileges.reader_is_select_only
        || is_placeholder(&privileges.evidence, 8)
        || is_placeholder(&clickhouse.network_policy_evidence, 8)
    {
        return Err(ProvisionSpecError::Validation(
            "target ClickHouse topology and least-privilege declarations require explicit evidence",
        ));
    }
    Ok(())
}

fn validate_native_upgrade(
    operation_id: &str,
    current: &NativeInstallationSpec,
    changes: &NativeUpgradeChangeSpec,
    attestations: &NativeUpgradeAttestationSpec,
) -> Result<(), ProvisionSpecError> {
    let installation_id = Uuid::parse_str(&current.installation_id).map_err(|_| {
        ProvisionSpecError::Validation("native current.installation_id must be a UUID")
    })?;
    if installation_id.is_nil()
        || is_placeholder(&current.current_build_id, 8)
        || is_placeholder(&changes.target_build_id, 8)
        || current.current_build_id == changes.target_build_id
        || current.postgres_schema_epoch == 0
        || current.clickhouse_schema_epoch == 0
        || changes.target_postgres_schema_epoch < current.postgres_schema_epoch
        || changes.target_clickhouse_schema_epoch < current.clickhouse_schema_epoch
    {
        return Err(ProvisionSpecError::Validation(
            "native upgrade identity, build IDs, and monotonic schema epochs must be explicit",
        ));
    }
    if current.api_runtime_config_path != Path::new("/var/lib/v2board/api/config.json")
        || current.worker_runtime_config_path != Path::new("/var/lib/v2board/worker/config.json")
    {
        return Err(ProvisionSpecError::Validation(
            "native current API/worker runtime config paths must be /var/lib/v2board/api/config.json and /var/lib/v2board/worker/config.json",
        ));
    }
    for (field, value) in [
        (
            "current.migration_database_url",
            &current.migration_database_url,
        ),
        ("current.api_database_url", &current.api_database_url),
        ("current.worker_database_url", &current.worker_database_url),
    ] {
        validate_postgres_url(value, field)?;
    }
    validate_postgres_runtime_url_set(
        &current.migration_database_url,
        &current.api_database_url,
        &current.worker_database_url,
    )?;
    validate_clickhouse_endpoint(&current.clickhouse_endpoint)?;
    if !valid_datastore_identifier(&current.clickhouse_database) {
        return Err(ProvisionSpecError::Validation(
            "native current.clickhouse_database must be an unquoted ASCII identifier",
        ));
    }
    validate_clickhouse_principals([
        &current.clickhouse_schema_principal,
        &current.clickhouse_writer_principal,
        &current.clickhouse_reader_principal,
    ])?;
    if !current
        .clickhouse_privileges
        .schema_has_ddl_metadata_read_and_ledger_write_only
        || !current
            .clickhouse_privileges
            .writer_is_insert_and_verify_only
        || !current.clickhouse_privileges.reader_is_select_only
        || is_placeholder(&current.clickhouse_privileges.evidence, 8)
    {
        return Err(ProvisionSpecError::Validation(
            "native ClickHouse privileges must limit schema to DDL/metadata-read/ledger-write, writer to insert-and-verify, and reader to SELECT with evidence",
        ));
    }
    validate_redis_url(&current.redis_url, "current.redis_url")?;
    if !redis_url_uses_tls(&current.redis_url) {
        return Err(ProvisionSpecError::Validation(
            "native current.redis_url must use rediss://",
        ));
    }
    for impact in changes
        .destructive_changes
        .iter()
        .chain(&changes.ttl_shortening)
        .chain(&changes.drop_operations)
        .chain(&changes.repartition_operations)
    {
        if [
            impact.resource.as_str(),
            impact.impact.as_str(),
            impact.rollback.as_str(),
        ]
        .iter()
        .any(|value| is_placeholder(value, 3))
        {
            return Err(ProvisionSpecError::Validation(
                "native upgrade impact entries require resource, impact, and rollback text",
            ));
        }
    }
    if attestations
        .backup_reference
        .as_deref()
        .is_some_and(|reference| is_placeholder(reference, 8))
    {
        return Err(ProvisionSpecError::Validation(
            "native backup_reference must be an explicit snapshot identifier",
        ));
    }
    if let Some(confirmation) = &attestations.second_confirmation
        && (confirmation.operation_id != operation_id
            || !is_lower_hex(&confirmation.prior_report_sha256, 64))
    {
        return Err(ProvisionSpecError::Validation(
            "native second_confirmation must bind this operation_id and a prior 64-character report SHA-256",
        ));
    }
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
    if runtime.get("server_require_idempotency_key") != Some(&Value::Bool(true)) {
        return Err(ProvisionSpecError::Validation(
            "one_shot_offline_cutover requires node report idempotency keys",
        ));
    }
    if runtime.get("bind_addr").and_then(Value::as_str) != Some("127.0.0.1:8080") {
        return Err(ProvisionSpecError::Validation(
            "bare-metal production runtime.bind_addr must be 127.0.0.1:8080",
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
    let _ = field;
    if is_placeholder(&password, 1) {
        return Err(ProvisionSpecError::Validation(
            "database URL password must not be a placeholder",
        ));
    }
    validate_mysql_connection_query(&url)?;
    Ok(())
}

fn validate_mysql_connection_query(url: &Url) -> Result<Option<String>, ProvisionSpecError> {
    let mut seen = BTreeSet::new();
    let mut ssl_mode = None;
    for (key, value) in url.query_pairs() {
        let lowercase = key.to_ascii_lowercase();
        if key.as_ref() != lowercase {
            return Err(ProvisionSpecError::Validation(
                "MySQL URL query parameter names must use canonical lowercase spelling",
            ));
        }
        let canonical = lowercase.replace('_', "-");
        if !seen.insert(canonical.clone()) {
            return Err(ProvisionSpecError::Validation(
                "MySQL URL duplicate or aliased query parameters are not allowed",
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
            return Err(ProvisionSpecError::Validation(
                "MySQL URL connection identity overrides are forbidden",
            ));
        }
        if key.as_ref() == "sslmode" || key.as_ref() == "ssl_mode" {
            return Err(ProvisionSpecError::Validation(
                "MySQL URL sslmode aliases are forbidden; use exactly one ssl-mode parameter",
            ));
        }
        if canonical == "ssl-mode" {
            ssl_mode = Some(value.to_ascii_lowercase().replace('-', "_"));
        }
    }
    Ok(ssl_mode)
}

fn postgres_url_principal(value: &str) -> Result<String, ProvisionSpecError> {
    let url = Url::parse(value).map_err(|_| {
        ProvisionSpecError::Validation("PostgreSQL URL must be a valid postgres:// URL")
    })?;
    strict_percent_decode(url.username())
}

fn validate_postgres_url(value: &str, _field: &'static str) -> Result<(), ProvisionSpecError> {
    let url = Url::parse(value).map_err(|_| {
        ProvisionSpecError::Validation("PostgreSQL URL must be a valid postgres:// URL")
    })?;
    let username = strict_percent_decode(url.username())?;
    let password = url
        .password()
        .map(strict_percent_decode)
        .transpose()?
        .unwrap_or_default();
    if !matches!(url.scheme(), "postgres" | "postgresql")
        || url.host_str().is_none()
        || username.is_empty()
        || password.is_empty()
        || url.username().contains('%')
        || url.path().contains('%')
        || url.fragment().is_some()
    {
        return Err(ProvisionSpecError::Validation(
            "PostgreSQL URL must include host, database, username, and password",
        ));
    }
    if url.host_str().is_some_and(host_is_reserved_placeholder)
        || is_placeholder(&password, 16)
        || !valid_postgres_identifier(&username)
    {
        return Err(ProvisionSpecError::Validation(
            "PostgreSQL URL contains a placeholder or invalid principal",
        ));
    }
    postgres_database_name(&url)?;
    v2board_config::validate_postgres_connection_query(&url, true).map_err(|_| {
        ProvisionSpecError::Validation(
            "PostgreSQL URLs must use canonical query parameters, forbid connection overrides, and set exactly one sslmode=verify-full",
        )
    })?;
    Ok(())
}

fn validate_postgres_url_set(
    bootstrap: &str,
    migration: &str,
    api: &str,
    worker: &str,
) -> Result<(), ProvisionSpecError> {
    let bootstrap_url = Url::parse(bootstrap).expect("validated PostgreSQL bootstrap URL");
    let migration_url = Url::parse(migration).expect("validated PostgreSQL migration URL");
    let api_url = Url::parse(api).expect("validated PostgreSQL API URL");
    let worker_url = Url::parse(worker).expect("validated PostgreSQL worker URL");
    let endpoints = [
        postgres_endpoint_identity(&bootstrap_url),
        postgres_endpoint_identity(&migration_url),
        postgres_endpoint_identity(&api_url),
        postgres_endpoint_identity(&worker_url),
    ];
    if endpoints.iter().any(|endpoint| endpoint != &endpoints[0]) {
        return Err(ProvisionSpecError::Validation(
            "all target PostgreSQL URLs must use the same host and port",
        ));
    }
    let bootstrap_database = postgres_database_name(&bootstrap_url)?;
    let target_database = postgres_database_name(&migration_url)?;
    if bootstrap_database == target_database
        || postgres_database_name(&api_url)? != target_database
        || postgres_database_name(&worker_url)? != target_database
    {
        return Err(ProvisionSpecError::Validation(
            "PostgreSQL bootstrap database must differ while migration, API, and worker use one target database",
        ));
    }
    validate_distinct_postgres_principals([&bootstrap_url, &migration_url, &api_url, &worker_url])
}

fn validate_postgres_runtime_url_set(
    migration: &str,
    api: &str,
    worker: &str,
) -> Result<(), ProvisionSpecError> {
    let migration_url = Url::parse(migration).expect("validated PostgreSQL migration URL");
    let api_url = Url::parse(api).expect("validated PostgreSQL API URL");
    let worker_url = Url::parse(worker).expect("validated PostgreSQL worker URL");
    if postgres_endpoint_identity(&migration_url) != postgres_endpoint_identity(&api_url)
        || postgres_endpoint_identity(&migration_url) != postgres_endpoint_identity(&worker_url)
        || postgres_database_name(&migration_url)? != postgres_database_name(&api_url)?
        || postgres_database_name(&migration_url)? != postgres_database_name(&worker_url)?
    {
        return Err(ProvisionSpecError::Validation(
            "native PostgreSQL migration, API, and worker URLs must bind one database endpoint",
        ));
    }
    validate_distinct_postgres_principals([&migration_url, &api_url, &worker_url])
}

fn validate_distinct_postgres_principals<const N: usize>(
    urls: [&Url; N],
) -> Result<(), ProvisionSpecError> {
    let mut usernames = BTreeSet::new();
    let mut passwords = BTreeSet::new();
    for url in urls {
        usernames.insert(strict_percent_decode(url.username())?);
        passwords.insert(strict_percent_decode(
            url.password().expect("validated PostgreSQL password"),
        )?);
    }
    if usernames.len() != N || passwords.len() != N {
        return Err(ProvisionSpecError::Validation(
            "PostgreSQL bootstrap/migration/API/worker principals and secrets must be distinct",
        ));
    }
    Ok(())
}

fn postgres_endpoint_identity(url: &Url) -> (String, u16) {
    (
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default().unwrap_or(5432),
    )
}

fn postgres_database_name(url: &Url) -> Result<String, ProvisionSpecError> {
    let path = strict_percent_decode(url.path().strip_prefix('/').unwrap_or_default())?;
    if path.is_empty() || path.len() > 63 || !valid_postgres_identifier(&path) {
        return Err(ProvisionSpecError::Validation(
            "PostgreSQL database names must be unquoted identifiers of at most 63 bytes",
        ));
    }
    Ok(path)
}

fn valid_postgres_identifier(value: &str) -> bool {
    value.len() <= 63 && valid_datastore_identifier(value)
}

fn valid_datastore_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
        && value.len() <= 128
}

fn validate_clickhouse_endpoint(value: &str) -> Result<(), ProvisionSpecError> {
    let url = Url::parse(value).map_err(|_| {
        ProvisionSpecError::Validation("ClickHouse endpoint must be a valid HTTPS origin")
    })?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_some_and(host_is_reserved_placeholder)
    {
        return Err(ProvisionSpecError::Validation(
            "ClickHouse endpoint must be a non-placeholder HTTPS origin without credentials, path, query, or fragment",
        ));
    }
    Ok(())
}

fn validate_clickhouse_principals<const N: usize>(
    principals: [&ClickHousePrincipalSpec; N],
) -> Result<(), ProvisionSpecError> {
    let mut usernames = BTreeSet::new();
    let mut passwords = BTreeSet::new();
    for principal in principals {
        if !valid_datastore_identifier(&principal.username)
            || is_placeholder(&principal.password, 16)
        {
            return Err(ProvisionSpecError::Validation(
                "ClickHouse principals require valid identifiers and non-placeholder secrets",
            ));
        }
        usernames.insert(principal.username.as_str());
        passwords.insert(principal.password.as_str());
    }
    if usernames.len() != N || passwords.len() != N {
        return Err(ProvisionSpecError::Validation(
            "ClickHouse bootstrap/schema/writer/reader principals and secrets must be distinct",
        ));
    }
    Ok(())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
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
        validate_mysql_connection_query(&url)
            .ok()
            .flatten()
            .as_deref()
            == Some("verify_identity")
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
    let datastore_kind = match url.scheme() {
        "mysql" => "mysql",
        "postgres" | "postgresql" => "postgresql",
        "redis" | "rediss" => "redis",
        _ => {
            return Err(ProvisionSpecError::Validation(
                "unsupported datastore URL scheme",
            ));
        }
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

impl ClickHousePrincipalSpec {
    pub(crate) fn password(&self) -> &str {
        &self.password
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

    pub fn materialized_api_runtime_config(
        &self,
    ) -> Result<Map<String, Value>, ProvisionSpecError> {
        let mut runtime = self.runtime().clone();
        let (database_url, peer_database_url, redis_url) = match &self.flow {
            ProvisionFlow::FreshInstall { target, .. }
            | ProvisionFlow::LegacyReferenceMigration { target, .. } => (
                &target.postgres.api_database_url,
                &target.postgres.worker_database_url,
                &target.redis_url,
            ),
            ProvisionFlow::NativeUpgrade { current, .. } => (
                &current.api_database_url,
                &current.worker_database_url,
                &current.redis_url,
            ),
        };
        runtime.insert("runtime_role".to_string(), Value::String("api".to_string()));
        runtime.insert(
            "database_url".to_string(),
            Value::String(database_url.clone()),
        );
        runtime.insert(
            "peer_database_principal".to_string(),
            Value::String(postgres_url_principal(peer_database_url)?),
        );
        runtime.insert("redis_url".to_string(), Value::String(redis_url.clone()));
        Ok(runtime)
    }

    pub fn materialized_worker_runtime_config(
        &self,
    ) -> Result<Map<String, Value>, ProvisionSpecError> {
        let mut runtime = self.runtime().clone();
        let (
            database_url,
            peer_database_url,
            redis_url,
            clickhouse_url,
            clickhouse_database,
            writer,
        ) = match &self.flow {
            ProvisionFlow::FreshInstall { target, .. }
            | ProvisionFlow::LegacyReferenceMigration { target, .. } => (
                &target.postgres.worker_database_url,
                &target.postgres.api_database_url,
                &target.redis_url,
                &target.clickhouse.endpoint,
                &target.clickhouse.database,
                &target.clickhouse.writer_principal,
            ),
            ProvisionFlow::NativeUpgrade { current, .. } => (
                &current.worker_database_url,
                &current.api_database_url,
                &current.redis_url,
                &current.clickhouse_endpoint,
                &current.clickhouse_database,
                &current.clickhouse_writer_principal,
            ),
        };
        runtime.insert(
            "runtime_role".to_string(),
            Value::String("worker".to_string()),
        );
        runtime.insert(
            "database_url".to_string(),
            Value::String(database_url.clone()),
        );
        runtime.insert(
            "peer_database_principal".to_string(),
            Value::String(postgres_url_principal(peer_database_url)?),
        );
        runtime.insert("redis_url".to_string(), Value::String(redis_url.clone()));
        runtime.insert(
            "clickhouse_url".to_string(),
            Value::String(clickhouse_url.clone()),
        );
        runtime.insert(
            "clickhouse_database".to_string(),
            Value::String(clickhouse_database.clone()),
        );
        runtime.insert(
            "clickhouse_writer_username".to_string(),
            Value::String(writer.username.clone()),
        );
        runtime.insert(
            "clickhouse_writer_password".to_string(),
            Value::String(writer.password.clone()),
        );
        Ok(runtime)
    }

    pub(crate) fn runtime(&self) -> &Map<String, Value> {
        match &self.flow {
            ProvisionFlow::FreshInstall { runtime, .. }
            | ProvisionFlow::LegacyReferenceMigration { runtime, .. }
            | ProvisionFlow::NativeUpgrade { runtime, .. } => runtime,
        }
    }

    pub(crate) fn api_runtime_config_path(&self) -> &Path {
        match &self.flow {
            ProvisionFlow::FreshInstall { target, .. }
            | ProvisionFlow::LegacyReferenceMigration { target, .. } => {
                &target.api_runtime_config_path
            }
            ProvisionFlow::NativeUpgrade { current, .. } => &current.api_runtime_config_path,
        }
    }

    pub(crate) fn worker_runtime_config_path(&self) -> &Path {
        match &self.flow {
            ProvisionFlow::FreshInstall { target, .. }
            | ProvisionFlow::LegacyReferenceMigration { target, .. } => {
                &target.worker_runtime_config_path
            }
            ProvisionFlow::NativeUpgrade { current, .. } => &current.worker_runtime_config_path,
        }
    }

    pub(crate) fn report_binding_hmac_sha256(&self, bytes: &[u8]) -> String {
        let mut mac =
            <Hmac<Sha256> as KeyInit>::new_from_slice(self.lifecycle_audit_key.as_bytes())
                .expect("HMAC accepts keys of any length");
        mac.update(REPORT_HMAC_DOMAIN_V3);
        mac.update(bytes);
        hex::encode(mac.finalize().into_bytes())
    }

    fn target_secret_values(&self) -> Result<Vec<String>, ProvisionSpecError> {
        let mut secrets = Vec::new();
        let mut add_url_password = |value: &str| -> Result<(), ProvisionSpecError> {
            if let Some(password) = Url::parse(value)
                .expect("validated datastore URL")
                .password()
            {
                secrets.push(strict_percent_decode(password)?);
            }
            Ok(())
        };
        match &self.flow {
            ProvisionFlow::FreshInstall { target, .. }
            | ProvisionFlow::LegacyReferenceMigration { target, .. } => {
                for value in [
                    &target.postgres.bootstrap_database_url,
                    &target.postgres.migration_database_url,
                    &target.postgres.api_database_url,
                    &target.postgres.worker_database_url,
                    &target.redis_url,
                ] {
                    add_url_password(value)?;
                }
                for principal in [
                    &target.clickhouse.bootstrap_principal,
                    &target.clickhouse.schema_principal,
                    &target.clickhouse.writer_principal,
                    &target.clickhouse.reader_principal,
                ] {
                    secrets.push(principal.password.clone());
                }
            }
            ProvisionFlow::NativeUpgrade { current, .. } => {
                for value in [
                    &current.migration_database_url,
                    &current.api_database_url,
                    &current.worker_database_url,
                    &current.redis_url,
                ] {
                    add_url_password(value)?;
                }
                for principal in [
                    &current.clickhouse_schema_principal,
                    &current.clickhouse_writer_principal,
                    &current.clickhouse_reader_principal,
                ] {
                    secrets.push(principal.password.clone());
                }
            }
        }
        Ok(secrets)
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
            ("show_subscribe_expire", 5),
        ] {
            map.insert(key.to_string(), Value::from(value));
        }
        for (key, value) in [
            ("configuration_source", "file_only"),
            ("environment", "production"),
            ("bind_addr", "127.0.0.1:8080"),
            ("subscribe_path", "/api/v1/client/subscribe"),
            ("app_key", "0123456789abcdef0123456789abcdef"),
            ("app_name", "V2Board"),
            ("app_url", "https://panel.company.net"),
            ("secure_path", "admin-safe"),
            ("server_token", "abcdef0123456789abcdef0123456789"),
        ] {
            map.insert(key.to_string(), Value::String(value.to_string()));
        }
        map.insert(
            "server_require_idempotency_key".to_string(),
            Value::Bool(true),
        );
        map
    }

    fn postgres_target() -> Value {
        serde_json::json!({
            "bootstrap_database_url": "postgresql://pg_bootstrap:bootstrap-secret-0123456789@pg.company.net/postgres?sslmode=verify-full",
            "migration_database_url": "postgresql://pg_migration:migration-secret-0123456789@pg.company.net/v2board?sslmode=verify-full",
            "api_database_url": "postgresql://pg_api:api-secret-0123456789abcdef@pg.company.net/v2board?sslmode=verify-full",
            "worker_database_url": "postgresql://pg_worker:worker-secret-0123456789@pg.company.net/v2board?sslmode=verify-full",
            "database_collation": "C.UTF-8",
            "database_ctype": "C.UTF-8",
            "require_database_absent": true,
            "require_roles_absent": true,
            "external_access": {
                "pg_hba_managed_externally": true,
                "pg_hba_evidence": "change-ticket-pg-hba-1042",
                "network_policy_managed_externally": true,
                "network_policy_evidence": "network-policy-v2board-1042"
            }
        })
    }

    fn clickhouse_target() -> Value {
        serde_json::json!({
            "endpoint": "https://clickhouse.company.net",
            "database": "v2board_analytics",
            "bootstrap_principal": {
                "username": "ch_bootstrap",
                "password": "bootstrap-clickhouse-secret-012345"
            },
            "schema_principal": {
                "username": "ch_schema",
                "password": "schema-clickhouse-secret-012345678"
            },
            "writer_principal": {
                "username": "ch_writer",
                "password": "writer-clickhouse-secret-01234567"
            },
            "reader_principal": {
                "username": "ch_reader",
                "password": "reader-clickhouse-secret-01234567"
            },
            "raw_retention_days": 90,
            "aggregate_retention_days": 730,
            "require_database_absent": true,
            "require_principals_absent": true,
            "require_standalone_non_replicated": true,
            "network_policy_evidence": "network-policy-clickhouse-1042",
            "privileges": {
                "bootstrap_manages_database_and_principals": true,
                "schema_has_ddl_metadata_read_and_ledger_write_only": true,
                "writer_is_insert_and_verify_only": true,
                "reader_is_select_only": true,
                "evidence": "clickhouse-grant-review-1042"
            }
        })
    }

    fn target() -> Value {
        serde_json::json!({
            "postgres": postgres_target(),
            "clickhouse": clickhouse_target(),
            "redis_url": "rediss://:redis-target-secret-0123456789@redis.company.net/1",
            "api_runtime_config_path": "/var/lib/v2board/api/config.json",
            "worker_runtime_config_path": "/var/lib/v2board/worker/config.json",
            "require_empty_redis": true
        })
    }

    fn fresh_document() -> Value {
        serde_json::json!({
            "schema_version": 3,
            "operation_id": "40aa4a80-eb4b-4b25-9c3b-e17ed047873d",
            "kind": "fresh_install",
            "lifecycle_audit_key": "lifecycle-audit-0123456789abcdef0123456789abcdef",
            "target": target(),
            "runtime": complete_runtime(),
            "decisions": {
                "initialize_empty_targets": true
            },
            "attestations": {
                "target_capacity_reviewed": false,
                "external_controls_reviewed": false
            }
        })
    }

    fn legacy_document() -> Value {
        let mut document = fresh_document();
        document["kind"] = Value::String("legacy_reference_migration".to_string());
        document.as_object_mut().expect("document").insert(
            "reference_commit".to_string(),
            Value::String(LEGACY_REFERENCE_COMMIT.to_string()),
        );
        document.as_object_mut().expect("document").insert(
            "source".to_string(),
            serde_json::json!({
                "database_url": "mysql://legacy_readonly:legacy-secret@legacy-db.company.net/v2board",
                "redis_default_url": "redis://legacy-redis.company.net/0",
                "redis_cache_url": "redis://legacy-redis.company.net/1",
                "redis_connection_prefix": "v2board_database_",
                "redis_cache_prefix": "v2board_cache",
                "legacy_cache_driver": "redis",
                "legacy_show_subscribe_method": 0,
                "legacy_show_subscribe_expire_minutes": 5,
                "legacy_subscription_issuance_stopped_at_unix": 0,
                "transport_security": "trusted_maintenance_network"
            }),
        );
        document["decisions"] = serde_json::json!({
            "legacy_configuration": "manual_only",
            "sessions": "logout_all",
            "legacy_cache": "discard_ephemeral_after_fence",
            "legacy_stripe": "assert_none",
            "legacy_subscription_tokens": "assert_none",
            "nodes": "one_shot_offline_cutover",
            "legacy_theme": "discard_confirmed",
            "legacy_custom_rules": "none"
        });
        document["attestations"] = serde_json::json!({
            "source_writers_stopped": false,
            "source_workers_stopped": false,
            "node_reporters_stopped": false,
            "legacy_queues_drained": false,
            "backup_reference": null,
            "restore_tested": false
        });
        document
    }

    fn native_document() -> Value {
        serde_json::json!({
            "schema_version": 3,
            "operation_id": "40aa4a80-eb4b-4b25-9c3b-e17ed047873d",
            "kind": "native_upgrade",
            "lifecycle_audit_key": "lifecycle-audit-0123456789abcdef0123456789abcdef",
            "current": {
                "installation_id": "e0bb60eb-bb45-4393-8a04-18a3aa510497",
                "current_build_id": "build-content-aaaaaaaaaaaaaaaa",
                "postgres_schema_epoch": 1,
                "clickhouse_schema_epoch": 1,
                "migration_database_url": "postgresql://pg_migration:migration-secret-0123456789@pg.company.net/v2board?sslmode=verify-full",
                "api_database_url": "postgresql://pg_api:api-secret-0123456789abcdef@pg.company.net/v2board?sslmode=verify-full",
                "worker_database_url": "postgresql://pg_worker:worker-secret-0123456789@pg.company.net/v2board?sslmode=verify-full",
                "clickhouse_endpoint": "https://clickhouse.company.net",
                "clickhouse_database": "v2board_analytics",
                "clickhouse_schema_principal": {
                    "username": "ch_schema",
                    "password": "schema-clickhouse-secret-012345678"
                },
                "clickhouse_writer_principal": {
                    "username": "ch_writer",
                    "password": "writer-clickhouse-secret-01234567"
                },
                "clickhouse_reader_principal": {
                    "username": "ch_reader",
                    "password": "reader-clickhouse-secret-01234567"
                },
                "clickhouse_privileges": {
                    "schema_has_ddl_metadata_read_and_ledger_write_only": true,
                    "writer_is_insert_and_verify_only": true,
                    "reader_is_select_only": true,
                    "evidence": "clickhouse-native-grant-review-1042"
                },
                "redis_url": "rediss://:redis-target-secret-0123456789@redis.company.net/1",
                "api_runtime_config_path": "/var/lib/v2board/api/config.json",
                "worker_runtime_config_path": "/var/lib/v2board/worker/config.json"
            },
            "runtime": complete_runtime(),
            "changes": {
                "target_build_id": "build-content-bbbbbbbbbbbbbbbb",
                "target_postgres_schema_epoch": 2,
                "target_clickhouse_schema_epoch": 2,
                "destructive_changes": [],
                "ttl_shortening": [],
                "drop_operations": [],
                "repartition_operations": []
            },
            "decisions": {
                "strategy": "maintenance_cutover",
                "allow_destructive_changes": false
            },
            "attestations": {
                "maintenance_window_approved": false,
                "backup_reference": null,
                "restore_tested": false,
                "impact_reviewed": false,
                "second_confirmation": null
            }
        })
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

        let mut unknown = runtime.clone();
        unknown.insert("typo_setting".to_string(), Value::Bool(true));
        assert!(matches!(
            validate_runtime(&unknown),
            Err(ProvisionSpecError::UnknownRuntimeKeys(_))
        ));

        for key in [
            "legacy_auth_params_enable",
            "legacy_jwt_cutoff_unix",
            "server_legacy_token_enable",
        ] {
            let mut retired = runtime.clone();
            retired.insert(key.to_string(), Value::Bool(false));
            assert!(matches!(
                validate_runtime(&retired),
                Err(ProvisionSpecError::UnknownRuntimeKeys(_))
            ));
        }
    }

    #[test]
    fn target_urls_and_principals_are_strictly_separated() {
        let postgres: PostgresTargetSpec =
            serde_json::from_value(postgres_target()).expect("PostgreSQL target");
        validate_target_postgres(&postgres).expect("valid PostgreSQL target");

        let mut reused = postgres_target();
        reused["worker_database_url"] = reused["api_database_url"].clone();
        let reused: PostgresTargetSpec =
            serde_json::from_value(reused).expect("shape remains valid");
        assert!(validate_target_postgres(&reused).is_err());

        let clickhouse: ClickHouseTargetSpec =
            serde_json::from_value(clickhouse_target()).expect("ClickHouse target");
        validate_target_clickhouse(&clickhouse).expect("valid ClickHouse target");

        let mut plaintext = clickhouse_target();
        plaintext["endpoint"] = Value::String("http://clickhouse.company.net".to_string());
        let plaintext: ClickHouseTargetSpec =
            serde_json::from_value(plaintext).expect("shape remains valid");
        assert!(validate_target_clickhouse(&plaintext).is_err());
    }

    #[test]
    fn datastore_query_aliases_cannot_override_inspected_identity_or_tls() {
        for attack in [
            "sslmode=verify-full&ssl-mode=disable",
            "sslmode=verify-full&host=other.company.net",
            "sslmode=verify-full&dbname=other",
            "sslmode=verify-full&user=shared",
            "SSLMODE=verify-full",
        ] {
            let url = format!(
                "postgresql://api:strong-secret-0123456789@pg.company.net/v2board?{attack}"
            );
            assert!(
                validate_postgres_url(&url, "test").is_err(),
                "accepted {attack}"
            );
        }

        let verified = Url::parse(
            "mysql://reader:strong-secret@legacy.company.net/v2board?ssl-mode=VERIFY_IDENTITY",
        )
        .unwrap();
        assert_eq!(
            validate_mysql_connection_query(&verified)
                .unwrap()
                .as_deref(),
            Some("verify_identity")
        );
        for attack in [
            "ssl-mode=VERIFY_IDENTITY&sslmode=disabled",
            "ssl-mode=VERIFY_IDENTITY&socket=%2Ftmp%2Fmysql.sock",
            "ssl-mode=VERIFY_IDENTITY&host=other.company.net",
            "SSL-MODE=VERIFY_IDENTITY",
        ] {
            let url = Url::parse(&format!(
                "mysql://reader:strong-secret@legacy.company.net/v2board?{attack}"
            ))
            .unwrap();
            assert!(
                validate_mysql_connection_query(&url).is_err(),
                "accepted {attack}"
            );
        }
    }

    #[test]
    fn clickhouse_writer_is_strictly_insert_and_verify_only() {
        let mut target = clickhouse_target();
        target["privileges"]["writer_is_insert_and_verify_only"] = Value::Bool(false);
        let target: ClickHouseTargetSpec =
            serde_json::from_value(target).expect("shape remains valid");
        assert!(validate_target_clickhouse(&target).is_err());

        let mut legacy_name = clickhouse_target();
        let privileges = legacy_name["privileges"]
            .as_object_mut()
            .expect("privilege declaration");
        privileges.remove("writer_is_insert_and_verify_only");
        privileges.insert("writer_is_insert_only".to_string(), Value::Bool(true));
        assert!(serde_json::from_value::<ClickHouseTargetSpec>(legacy_name).is_err());

        let mut native = native_document();
        native["current"]["clickhouse_privileges"]["writer_is_insert_and_verify_only"] =
            Value::Bool(false);
        assert!(load_document(&native).is_err());
    }

    #[test]
    fn v2_is_rejected_before_v3_shape_deserialization() {
        let error = match load_bytes(br#"{"schema_version":2,"kind":"legacy_reference_migration"}"#)
        {
            Ok(_) => panic!("v2 must never be reinterpreted as v3"),
            Err(error) => error,
        };
        assert!(matches!(error, ProvisionSpecError::SchemaVersion));
    }

    #[test]
    fn all_three_v3_kinds_materialize_role_isolated_runtime_documents() {
        for (expected_kind, document) in [
            (ProvisionKind::FreshInstall, fresh_document()),
            (ProvisionKind::LegacyReferenceMigration, legacy_document()),
            (ProvisionKind::NativeUpgrade, native_document()),
        ] {
            let spec = load_document(&document).expect("valid v3 document");
            assert_eq!(spec.kind, expected_kind);
            let api = spec.materialized_api_runtime_config().expect("API config");
            for key in [
                "runtime_role",
                "database_url",
                "peer_database_principal",
                "redis_url",
            ] {
                assert!(api.contains_key(key), "missing API {key}");
            }
            for forbidden in [
                "worker_database_url",
                "clickhouse_url",
                "clickhouse_database",
                "clickhouse_reader_username",
                "clickhouse_reader_password",
                "clickhouse_writer_username",
                "clickhouse_writer_password",
            ] {
                assert!(!api.contains_key(forbidden), "API leaked {forbidden}");
            }
            AppConfig::try_from_api_config_map(
                api,
                RuntimePaths {
                    config: PathBuf::from("/var/lib/v2board/api/config.json"),
                    frontend: PathBuf::from("/opt/v2board/frontend"),
                    rules: PathBuf::from("/var/lib/v2board/rules"),
                },
            )
            .expect("materialized v3 API runtime must load");

            let worker = spec
                .materialized_worker_runtime_config()
                .expect("worker config");
            for key in [
                "runtime_role",
                "database_url",
                "peer_database_principal",
                "redis_url",
                "clickhouse_url",
                "clickhouse_database",
                "clickhouse_writer_username",
                "clickhouse_writer_password",
            ] {
                assert!(worker.contains_key(key), "missing worker {key}");
            }
            for forbidden in [
                "worker_database_url",
                "clickhouse_reader_username",
                "clickhouse_reader_password",
            ] {
                assert!(!worker.contains_key(forbidden), "worker leaked {forbidden}");
            }
            AppConfig::try_from_worker_config_map(
                worker,
                RuntimePaths {
                    config: PathBuf::from("/var/lib/v2board/worker/config.json"),
                    frontend: PathBuf::from("/opt/v2board/frontend"),
                    rules: PathBuf::from("/var/lib/v2board/rules"),
                },
            )
            .expect("materialized v3 worker runtime must load");
        }

        let mut fresh_with_source = fresh_document();
        fresh_with_source["source"] = serde_json::json!({});
        assert!(load_document(&fresh_with_source).is_err());

        let mut native_with_target = native_document();
        native_with_target["target"] = target();
        assert!(load_document(&native_with_target).is_err());
    }

    #[test]
    fn destructive_native_changes_require_structured_impacts_and_bind_confirmation() {
        let mut document = native_document();
        document["changes"]["drop_operations"] = serde_json::json!([{
            "resource": "v2_old_projection",
            "impact": "drops historical projection rows",
            "rollback": "restore snapshot and previous build"
        }]);
        load_document(&document).expect("incomplete attestations belong in the blocked plan");

        document["attestations"]["second_confirmation"] = serde_json::json!({
            "operation_id": "different-operation",
            "prior_report_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        });
        assert!(load_document(&document).is_err());
    }

    #[tokio::test]
    async fn native_plan_is_v3_hmac_bound_and_fail_closed() {
        let spec = load_document(&native_document()).expect("native document");
        let plan = crate::inspect::build_inspection(&spec, crate::inspect::InspectionMode::Online)
            .await
            .expect("metadata-only native plan");
        assert_eq!(plan.report_version, 3);
        assert_eq!(plan.report_sha256.len(), 64);
        assert_eq!(plan.report_binding_hmac_sha256.len(), 64);
        assert!(!plan.converter_available);
        assert!(!plan.apply_available);
        assert!(!plan.passed());
        assert!(matches!(
            plan.verdict,
            crate::inspect::PreflightVerdict::Blocked
        ));
        assert!(
            plan.implementation_blockers
                .iter()
                .any(|blocker| { blocker.contains("installation binding") })
        );
    }

    #[test]
    fn manifest_hmac_is_v3_bound_and_duplicate_keys_fail_closed() {
        let first = load_document(&fresh_document())
            .expect("fresh document")
            .manifest_binding_hmac_sha256()
            .to_string();
        let mut changed = fresh_document();
        changed["runtime"]["app_name"] = Value::String("Changed".to_string());
        let second = load_document(&changed)
            .expect("changed document")
            .manifest_binding_hmac_sha256()
            .to_string();
        assert_ne!(first, second);
        assert_eq!(first.len(), 64);

        let error = match load_bytes(
            br#"{"schema_version":3,"schema_version":3,"kind":"fresh_install"}"#,
        ) {
            Ok(_) => panic!("duplicate key must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("duplicate JSON key"));
    }

    fn load_document(document: &Value) -> Result<ProvisionSpec, ProvisionSpecError> {
        load_bytes(&serde_json::to_vec(document).expect("JSON"))
    }

    fn load_bytes(bytes: &[u8]) -> Result<ProvisionSpec, ProvisionSpecError> {
        let path = std::env::temp_dir().join(format!(
            "v2board-provision-v3-test-{}-{}",
            std::process::id(),
            TEST_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        write_private_test_file(&path, bytes);
        let result = load_provision_spec(&path);
        fs::remove_file(path).expect("remove test manifest");
        result
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
