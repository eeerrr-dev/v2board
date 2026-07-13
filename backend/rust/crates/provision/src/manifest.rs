use std::{
    collections::BTreeSet,
    fs, io,
    io::Read,
    net::{IpAddr, SocketAddr},
    path::{Component, Path, PathBuf},
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
    AppConfig, BOOT_ONLY_RUNTIME_KEYS_V1, FILE_ONLY_RUNTIME_KEYS_V1, MAX_CONFIG_DURATION_MINUTES,
    OPERATOR_CONFIG_KEYS_V1, RuntimePaths,
};

pub const LEGACY_REFERENCE_COMMIT: &str = "7e77de9f4873b317157490529f7be7d6f8a62421";
const MAX_SPEC_BYTES: u64 = 1024 * 1024;

// V3 deliberately requires a complete runtime document. Adding a new runtime
// setting requires a new spec version or an explicit compatibility decision;
// no lifecycle path may acquire new behavior from an implicit default.
const RUNTIME_KEYS_V1: &[&str] = FILE_ONLY_RUNTIME_KEYS_V1;
const MANIFEST_HMAC_DOMAIN_V3: &[u8] = b"v2board-provision-manifest-v3\0";
const MANIFEST_HMAC_DOMAIN_V4: &[u8] = b"v2board-provision-manifest-v4\0";
const MANIFEST_HMAC_DOMAIN_V5: &[u8] = b"v2board-provision-manifest-v5\0";
pub(crate) const REPORT_HMAC_DOMAIN_V3: &[u8] = b"v2board-provision-report-v3\0";
pub(crate) const REPORT_HMAC_DOMAIN_V4: &[u8] = b"v2board-provision-report-v4\0";
pub(crate) const REPORT_HMAC_DOMAIN_V5: &[u8] = b"v2board-provision-report-v5\0";
const APPLY_AUTHORIZATION_HMAC_DOMAIN_V3: &[u8] = b"v2board-provision-apply-authorization-v3\0";
const LEGACY_EXECUTION_HMAC_DOMAIN_V1: &[u8] = b"v2board-provision-legacy-execution-v1\0";
const LEGACY_EXECUTION_HMAC_DOMAIN_V2: &[u8] = b"v2board-provision-legacy-execution-v2\0";
const LEGACY_RUNTIME_RECEIPT_HMAC_DOMAIN_V1: &[u8] =
    b"v2board-provision-legacy-runtime-receipt-v1\0";
const LEGACY_RUNTIME_RECEIPT_HMAC_DOMAIN_V2: &[u8] =
    b"v2board-provision-legacy-runtime-receipt-v2\0";

const LIFECYCLE_STATE_ROOT: &str = "/var/lib/v2board/lifecycle";
const LIFECYCLE_SECRET_ROOT: &str = "/run/v2board-lifecycle-secrets";
const JOURNAL_ROOT: &str = "/var/lib/v2board/lifecycle/journal";
const ACTIVATION_STATE_ROOT: &str = "/var/lib/v2board/lifecycle/activation";
const RELEASES_ROOT: &str = "/opt/v2board/releases";
const CURRENT_RELEASE_PATH: &str = "/opt/v2board/current";
const API_UNIT: &str = "v2board-api.service";
const WORKER_UNIT: &str = "v2board-worker.service";
const API_READY_URL: &str = "http://127.0.0.1:8080/readyz";
const WORKER_HEALTH_PATH: &str = "/run/v2board-worker/health";

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

pub struct ProvisionSpec {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: ProvisionKind,
    lifecycle_audit_key: String,
    pub(crate) flow: ProvisionFlow,
    manifest_binding_hmac_sha256: String,
}

/// Exact dynamic configuration normalized by the native typed parser from the
/// single operator-maintained manifest. The inner map deliberately stays
/// private so callers cannot construct an unvalidated authority candidate.
/// This value contains integration secrets and therefore must never implement
/// `Debug` or `Serialize`.
pub struct NormalizedOperatorConfigCandidate {
    values: Map<String, Value>,
}

impl NormalizedOperatorConfigCandidate {
    pub fn as_map(&self) -> &Map<String, Value> {
        &self.values
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionKind {
    FreshInstall,
    LegacyReferenceMigration,
    NativeUpgrade,
}

// A provision manifest is loaded once per lifecycle process. Keeping each flow's
// validated fields together avoids extra indirection across security-sensitive
// validation and binding code for no meaningful steady-state memory saving.
#[allow(clippy::large_enum_variant)]
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
        attestations: Option<LegacyAttestationSpec>,
        execution: Option<Box<LegacyExecutionSpec>>,
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
    #[serde(default)]
    pub database_fence_url: Option<String>,
    pub redis_default_url: String,
    pub redis_cache_url: String,
    pub redis_connection_prefix: String,
    pub redis_cache_prefix: String,
    #[serde(default)]
    pub redis_horizon_prefix: String,
    pub legacy_cache_driver: LegacyCacheDriver,
    pub transport_security: SourceTransportSecurity,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyCacheDriver {
    Redis,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    pub analytics_admission: AnalyticsAdmissionSpec,
    pub redis_url: String,
    pub api_runtime_config_path: PathBuf,
    pub worker_runtime_config_path: PathBuf,
    pub require_empty_redis: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsAdmissionSpec {
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

#[derive(Serialize)]
pub struct LegacyDecisionSpec {
    pub legacy_configuration: LegacyConfigurationDecision,
    pub sessions: SessionDecision,
    pub legacy_cache: LegacyCacheDecision,
    pub legacy_stripe: LegacyStripeDecision,
    pub temporary_subscription_links: TemporarySubscriptionLinkDecision,
    pub nodes: NodeDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_traffic_details: Option<LegacyTrafficDetailsDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_operational_logs: Option<LegacyOperationalLogsDecision>,
    pub legacy_theme: LegacyThemeDecision,
    pub legacy_custom_rules: LegacyCustomRulesDecision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDecisionSpecV3 {
    legacy_configuration: LegacyConfigurationDecision,
    sessions: SessionDecision,
    legacy_cache: LegacyCacheDecision,
    legacy_stripe: LegacyStripeDecision,
    temporary_subscription_links: TemporarySubscriptionLinkDecision,
    nodes: NodeDecision,
    legacy_theme: LegacyThemeDecision,
    legacy_custom_rules: LegacyCustomRulesDecision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDecisionSpecV4 {
    legacy_custom_rules: LegacyCustomRulesDecision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDecisionSpecV5 {
    nodes: NodeDecision,
    legacy_traffic_details: LegacyTrafficDetailsDecision,
    legacy_operational_logs: LegacyOperationalLogsDecision,
    legacy_custom_rules: LegacyCustomRulesDecision,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyConfigurationDecision {
    ManualOnly,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDecision {
    LogoutAll,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyCacheDecision {
    DiscardEphemeralAfterFence,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStripeDecision {
    AssertNone,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporarySubscriptionLinkDecision {
    InvalidateAtCutover,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeDecision {
    OneShotOfflineCutover,
    DiscardAndManualRebuild,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyTrafficDetailsDecision {
    Discard,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyOperationalLogsDecision {
    Discard,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyThemeDecision {
    DiscardConfirmed,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
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

/// Bare-metal inputs for one irreversible legacy cutover. This section is
/// present in schema v4 and v5. Runtime observations and completion claims do
/// not belong here: their content hashes are appended to the durable journal
/// after the corresponding action actually succeeds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyExecutionSpec {
    #[serde(skip)]
    pub journal: LegacyJournalExecutionSpec,
    pub release: LegacyReleaseExecutionSpec,
    pub systemd: LegacySystemdExecutionSpec,
    pub source_control: LegacySourceControlExecutionSpec,
    pub receipts: LegacyReceiptExecutionSpec,
    pub backup: LegacyBackupExecutionSpec,
    pub nodes: LegacyNodeExecutionSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_traffic_details: Option<LegacyTrafficDetailsDecision>,
    #[serde(skip)]
    pub source_retirement: LegacySourceRetirementExecutionSpec,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyJournalExecutionSpec {
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(skip)]
    pub authorization_path: PathBuf,
    #[serde(skip)]
    pub activation_state_root: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyReleaseExecutionSpec {
    pub release_id: String,
    #[serde(skip)]
    pub archive_path: PathBuf,
    pub archive_sha256: String,
    #[serde(skip)]
    pub releases_root: PathBuf,
    #[serde(skip)]
    pub current_symlink: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySystemdExecutionSpec {
    #[serde(skip)]
    pub api_unit: String,
    #[serde(skip)]
    pub worker_unit: String,
    #[serde(skip)]
    pub api_ready_url: String,
    #[serde(skip)]
    pub worker_health_path: PathBuf,
    pub legacy_writer_units: Vec<String>,
    pub legacy_worker_units: Vec<String>,
    pub legacy_scheduler_units: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySourceControlExecutionSpec {
    pub datastores: LegacySourceDatastoreControlSet,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySourceDatastoreControlSet {
    pub mysql: LegacySourceDatastoreControlSpec,
    pub default_redis: LegacySourceDatastoreControlSpec,
    pub cache_redis: LegacySourceDatastoreControlSpec,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySourceDatastoreControlSpec {
    pub unit: String,
}

/// The release receipt is an immutable input and therefore has an exact
/// pre-authorized digest. Every other path is an output slot: binding a future
/// `completed=true` receipt digest before the action happened would be a false
/// proof. The executor must hash-chain the file after it is created.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyReceiptExecutionSpec {
    pub release_archive: ImmutableReceiptSpec,
    #[serde(skip)]
    pub source_fence_path: PathBuf,
    #[serde(skip)]
    pub source_drain_path: PathBuf,
    #[serde(skip)]
    pub backup_restore_path: PathBuf,
    #[serde(skip)]
    pub redis_fence_armed_path: PathBuf,
    #[serde(skip)]
    pub redis_fence_path: PathBuf,
    #[serde(skip)]
    pub datastore_fence_armed_path: PathBuf,
    #[serde(skip)]
    pub datastore_fence_path: PathBuf,
    #[serde(skip)]
    pub source_retirement_path: PathBuf,
    #[serde(skip)]
    pub runtime_compatibility_disabled_path: PathBuf,
    #[serde(skip)]
    pub postgres_authority_path: PathBuf,
    /// Schema-v5-only durable preimage for the PostgreSQL value-verification
    /// report. Derived from the operation ID; never supplied by the operator
    /// and never serialized into the frozen schema-v4 execution binding.
    #[serde(skip)]
    pub postgres_verification_path: Option<PathBuf>,
    /// Schema-v5-only durable preimage for the ClickHouse projection report.
    /// See `postgres_verification_path` for the compatibility rationale.
    #[serde(skip)]
    pub clickhouse_projection_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImmutableReceiptSpec {
    #[serde(skip)]
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyBackupExecutionSpec {
    #[serde(skip)]
    pub mode: LegacyBackupMode,
    pub backup_reference: String,
    #[serde(skip)]
    pub encrypted_backup_output_path: PathBuf,
    #[serde(skip)]
    pub encryption_recipient_path: PathBuf,
    pub encryption_recipient_sha256: String,
    #[serde(skip)]
    pub decryption_identity_path: PathBuf,
    pub decryption_identity_sha256: String,
    #[serde(skip)]
    pub isolated_restore_state_path: PathBuf,
    pub isolated_restore_database_url: String,
    pub isolated_restore_transport_security: SourceTransportSecurity,
    pub command_timeout_seconds: u64,
    pub maximum_encrypted_backup_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyBackupMode {
    #[default]
    MysqlLogicalDumpAndIsolatedRestore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Each stage consumes its variant only when that executor is wired.
pub(crate) enum LegacyRuntimeReceiptKind {
    SourceFence,
    SourceDrain,
    BackupRestore,
    RedisFenceArmed,
    RedisFence,
    DatastoreFenceArmed,
    DatastoreFence,
    SourceRetirement,
    RuntimeCompatibilityDisabled,
    PostgresAuthority,
    PostgresVerificationReport,
    ClickHouseProjectionReport,
}

impl LegacyRuntimeReceiptKind {
    const fn domain_label(self) -> &'static [u8] {
        match self {
            Self::SourceFence => b"source_fence",
            Self::SourceDrain => b"source_drain",
            Self::BackupRestore => b"backup_restore",
            Self::RedisFenceArmed => b"redis_fence_armed",
            Self::RedisFence => b"redis_fence",
            Self::DatastoreFenceArmed => b"datastore_fence_armed",
            Self::DatastoreFence => b"datastore_fence",
            Self::SourceRetirement => b"source_retirement",
            Self::RuntimeCompatibilityDisabled => b"runtime_compatibility_disabled",
            Self::PostgresAuthority => b"postgres_authority",
            Self::PostgresVerificationReport => b"postgres_verification_report",
            Self::ClickHouseProjectionReport => b"clickhouse_projection_report",
        }
    }

    const fn schema_v5_only(self) -> bool {
        matches!(
            self,
            Self::PostgresVerificationReport | Self::ClickHouseProjectionReport
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyNodeExecutionSpec {
    pub activation_transport: LegacyNodeActivationTransportSpec,
    pub inventory: Vec<LegacyNodeIdentitySpec>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyNodeIdentitySpec {
    pub node_type: String,
    pub node_id: i32,
    pub credential_epoch: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LegacyNodeActivationTransportSpec {
    NotRequiredNoNodes,
    DiscardAndManualRebuild,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySourceRetirementExecutionSpec {
    #[serde(skip)]
    pub lifecycle_tool_path: PathBuf,
    #[serde(skip)]
    pub retirement_probe_state_path: PathBuf,
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
struct LegacyMigrationDocumentV3 {
    schema_version: u32,
    operation_id: String,
    kind: ProvisionKind,
    reference_commit: String,
    lifecycle_audit_key: String,
    source: SourceSpec,
    target: TargetSpec,
    runtime: Map<String, Value>,
    decisions: LegacyDecisionSpecV3,
    attestations: LegacyAttestationSpec,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMigrationDocumentV4 {
    schema_version: u32,
    operation_id: String,
    kind: ProvisionKind,
    lifecycle_audit_key: String,
    source: SourceSpec,
    target: TargetSpec,
    runtime: Map<String, Value>,
    decisions: LegacyDecisionSpecV4,
    execution: LegacyExecutionDocumentV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMigrationDocumentV5 {
    schema_version: u32,
    operation_id: String,
    kind: ProvisionKind,
    lifecycle_audit_key: String,
    source: SourceSpec,
    target: TargetSpec,
    runtime: Map<String, Value>,
    decisions: LegacyDecisionSpecV5,
    execution: LegacyExecutionDocumentV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyExecutionDocumentV4 {
    release: LegacyReleaseExecutionSpec,
    systemd: LegacySystemdExecutionSpec,
    source_control: LegacySourceControlExecutionSpec,
    receipts: LegacyReceiptExecutionSpec,
    backup: LegacyBackupExecutionSpec,
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
    #[error("unsupported provision spec schema_version; expected 3, 4, or 5")]
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
    let schema_version = unique
        .0
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ProvisionSpecError::SchemaVersion)?;
    if !matches!(schema_version, 3..=5) {
        return Err(ProvisionSpecError::SchemaVersion);
    }
    let kind = serde_json::from_value::<ProvisionKind>(
        unique.0.get("kind").cloned().unwrap_or(Value::Null),
    )
    .map_err(ProvisionSpecError::Json)?;
    let mut spec = match (schema_version, kind) {
        (3, ProvisionKind::FreshInstall) => {
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
        (3, ProvisionKind::LegacyReferenceMigration) => {
            let document = serde_json::from_value::<LegacyMigrationDocumentV3>(unique.0)
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
                    decisions: hydrate_legacy_v3_decisions(document.decisions),
                    attestations: Some(document.attestations),
                    execution: None,
                },
                manifest_binding_hmac_sha256: String::new(),
            }
        }
        (4, ProvisionKind::LegacyReferenceMigration) => {
            let mut value = unique.0;
            hydrate_legacy_v4_target(&mut value)?;
            let document = serde_json::from_value::<LegacyMigrationDocumentV4>(value)
                .map_err(ProvisionSpecError::Json)?;
            let execution = hydrate_legacy_execution(
                &document.operation_id,
                document.execution,
                LegacyNodeActivationTransportSpec::NotRequiredNoNodes,
                None,
                false,
            );
            ProvisionSpec {
                schema_version: document.schema_version,
                operation_id: document.operation_id,
                kind: document.kind,
                lifecycle_audit_key: document.lifecycle_audit_key,
                flow: ProvisionFlow::LegacyReferenceMigration {
                    reference_commit: LEGACY_REFERENCE_COMMIT.to_string(),
                    source: document.source,
                    target: document.target,
                    runtime: document.runtime,
                    decisions: hydrate_legacy_v4_decisions(document.decisions),
                    attestations: None,
                    execution: Some(Box::new(execution)),
                },
                manifest_binding_hmac_sha256: String::new(),
            }
        }
        (5, ProvisionKind::LegacyReferenceMigration) => {
            let mut value = unique.0;
            hydrate_legacy_v4_target(&mut value)?;
            let document = serde_json::from_value::<LegacyMigrationDocumentV5>(value)
                .map_err(ProvisionSpecError::Json)?;
            let decisions = hydrate_legacy_v5_decisions(document.decisions);
            let execution = hydrate_legacy_execution(
                &document.operation_id,
                document.execution,
                LegacyNodeActivationTransportSpec::DiscardAndManualRebuild,
                Some(LegacyTrafficDetailsDecision::Discard),
                true,
            );
            ProvisionSpec {
                schema_version: document.schema_version,
                operation_id: document.operation_id,
                kind: document.kind,
                lifecycle_audit_key: document.lifecycle_audit_key,
                flow: ProvisionFlow::LegacyReferenceMigration {
                    reference_commit: LEGACY_REFERENCE_COMMIT.to_string(),
                    source: document.source,
                    target: document.target,
                    runtime: document.runtime,
                    decisions,
                    attestations: None,
                    execution: Some(Box::new(execution)),
                },
                manifest_binding_hmac_sha256: String::new(),
            }
        }
        (3, ProvisionKind::NativeUpgrade) => {
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
        (4, ProvisionKind::FreshInstall | ProvisionKind::NativeUpgrade) => {
            return Err(ProvisionSpecError::Validation(
                "schema_version 4 currently defines execution inputs only for legacy_reference_migration",
            ));
        }
        (5, ProvisionKind::FreshInstall | ProvisionKind::NativeUpgrade) => {
            return Err(ProvisionSpecError::Validation(
                "schema_version 5 currently defines discard-policy execution inputs only for legacy_reference_migration",
            ));
        }
        _ => return Err(ProvisionSpecError::SchemaVersion),
    };
    validate_spec(&spec)?;
    validate_manifest_execution_separation(path, &spec)?;
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(spec.lifecycle_audit_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(match spec.schema_version {
        3 => MANIFEST_HMAC_DOMAIN_V3,
        4 => MANIFEST_HMAC_DOMAIN_V4,
        5 => MANIFEST_HMAC_DOMAIN_V5,
        _ => return Err(ProvisionSpecError::SchemaVersion),
    });
    mac.update(&bytes);
    if spec.schema_version == 4 {
        mac.update(&[0]);
        mac.update(&legacy_v4_hydrated_facts(&spec)?);
    } else if spec.schema_version == 5 {
        mac.update(&[0]);
        mac.update(&legacy_v5_hydrated_facts(&spec)?);
    }
    spec.manifest_binding_hmac_sha256 = hex::encode(mac.finalize().into_bytes());
    Ok(spec)
}

#[derive(Serialize)]
struct LegacyV4HydratedFacts<'a> {
    binding_version: u32,
    reference_commit: &'a str,
    decisions: &'a LegacyDecisionSpec,
    execution: &'a LegacyExecutionSpec,
}

#[derive(Serialize)]
struct LegacyV5HydratedFacts<'a> {
    binding_version: u32,
    reference_commit: &'a str,
    decisions: &'a LegacyDecisionSpec,
    execution: &'a LegacyExecutionSpec,
}

fn legacy_v4_hydrated_facts(spec: &ProvisionSpec) -> Result<Vec<u8>, ProvisionSpecError> {
    let ProvisionFlow::LegacyReferenceMigration {
        reference_commit,
        decisions,
        execution: Some(execution),
        ..
    } = &spec.flow
    else {
        return Err(ProvisionSpecError::Validation(
            "schema v4 hydrated legacy facts are unavailable",
        ));
    };
    serde_json::to_vec(&LegacyV4HydratedFacts {
        binding_version: 1,
        reference_commit,
        decisions,
        execution,
    })
    .map_err(ProvisionSpecError::Json)
}

fn legacy_v5_hydrated_facts(spec: &ProvisionSpec) -> Result<Vec<u8>, ProvisionSpecError> {
    let ProvisionFlow::LegacyReferenceMigration {
        reference_commit,
        decisions,
        execution: Some(execution),
        ..
    } = &spec.flow
    else {
        return Err(ProvisionSpecError::Validation(
            "schema v5 hydrated legacy facts are unavailable",
        ));
    };
    serde_json::to_vec(&LegacyV5HydratedFacts {
        binding_version: 3,
        reference_commit,
        decisions,
        execution,
    })
    .map_err(ProvisionSpecError::Json)
}

/// Schema v4 accepts only facts and choices. Values fixed by the one-shot
/// product contract are inserted after strict raw JSON parsing, and both the
/// raw bytes and the complete hydrated facts are HMAC-bound. Explicitly
/// writing one is rejected instead of silently accepting a second
/// source of truth. Schema v3 remains byte-for-byte compatible.
fn hydrate_legacy_v4_target(value: &mut Value) -> Result<(), ProvisionSpecError> {
    let root = value.as_object_mut().ok_or(ProvisionSpecError::Validation(
        "schema v4 manifest must be a JSON object",
    ))?;
    let source = object_at_mut(root, "source")?;
    insert_derived(source, "legacy_cache_driver", Value::String("redis".into()))?;

    let target = object_at_mut(root, "target")?;
    for (key, value) in [
        (
            "api_runtime_config_path",
            Value::String("/var/lib/v2board/api/config.json".into()),
        ),
        (
            "worker_runtime_config_path",
            Value::String("/var/lib/v2board/worker/config.json".into()),
        ),
        ("require_empty_redis", Value::Bool(true)),
    ] {
        insert_derived(target, key, value)?;
    }

    let postgres = object_at_mut(target, "postgres")?;
    for (key, value) in [
        ("database_collation", Value::String("C.UTF-8".into())),
        ("database_ctype", Value::String("C.UTF-8".into())),
        ("require_database_absent", Value::Bool(true)),
        ("require_roles_absent", Value::Bool(true)),
    ] {
        insert_derived(postgres, key, value)?;
    }
    let external_access = object_at_mut(postgres, "external_access")?;
    insert_derived(
        external_access,
        "pg_hba_managed_externally",
        Value::Bool(true),
    )?;
    insert_derived(
        external_access,
        "network_policy_managed_externally",
        Value::Bool(true),
    )?;

    let clickhouse = object_at_mut(target, "clickhouse")?;
    for key in [
        "require_database_absent",
        "require_principals_absent",
        "require_standalone_non_replicated",
    ] {
        insert_derived(clickhouse, key, Value::Bool(true))?;
    }
    let privileges = object_at_mut(clickhouse, "privileges")?;
    for key in [
        "bootstrap_manages_database_and_principals",
        "schema_has_ddl_metadata_read_and_ledger_write_only",
        "writer_is_insert_and_verify_only",
        "reader_is_select_only",
    ] {
        insert_derived(privileges, key, Value::Bool(true))?;
    }
    Ok(())
}

fn object_at_mut<'a>(
    parent: &'a mut serde_json::Map<String, Value>,
    key: &'static str,
) -> Result<&'a mut serde_json::Map<String, Value>, ProvisionSpecError> {
    parent
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .ok_or(ProvisionSpecError::Validation(
            "schema v4 source, target, and nested target declarations must be JSON objects",
        ))
}

fn insert_derived(
    object: &mut serde_json::Map<String, Value>,
    key: &'static str,
    value: Value,
) -> Result<(), ProvisionSpecError> {
    if object.contains_key(key) {
        return Err(ProvisionSpecError::Validation(
            "schema v4 must omit fixed or derived target fields",
        ));
    }
    object.insert(key.to_string(), value);
    Ok(())
}

/// Fill paths and fixed one-shot policies that are part of the schema-v4
/// implementation contract, rather than operator choices. Keeping these
/// values out of the manifest prevents a hand-edited file from restating (and
/// potentially mistyping) values already bound by `operation_id` and the
/// installed lifecycle release.
fn hydrate_legacy_v3_decisions(decisions: LegacyDecisionSpecV3) -> LegacyDecisionSpec {
    LegacyDecisionSpec {
        legacy_configuration: decisions.legacy_configuration,
        sessions: decisions.sessions,
        legacy_cache: decisions.legacy_cache,
        legacy_stripe: decisions.legacy_stripe,
        temporary_subscription_links: decisions.temporary_subscription_links,
        nodes: decisions.nodes,
        legacy_traffic_details: None,
        legacy_operational_logs: None,
        legacy_theme: decisions.legacy_theme,
        legacy_custom_rules: decisions.legacy_custom_rules,
    }
}

fn hydrate_legacy_v4_decisions(decisions: LegacyDecisionSpecV4) -> LegacyDecisionSpec {
    LegacyDecisionSpec {
        legacy_configuration: LegacyConfigurationDecision::ManualOnly,
        sessions: SessionDecision::LogoutAll,
        legacy_cache: LegacyCacheDecision::DiscardEphemeralAfterFence,
        legacy_stripe: LegacyStripeDecision::AssertNone,
        temporary_subscription_links: TemporarySubscriptionLinkDecision::InvalidateAtCutover,
        nodes: NodeDecision::OneShotOfflineCutover,
        legacy_traffic_details: None,
        legacy_operational_logs: None,
        legacy_theme: LegacyThemeDecision::DiscardConfirmed,
        legacy_custom_rules: decisions.legacy_custom_rules,
    }
}

fn hydrate_legacy_v5_decisions(decisions: LegacyDecisionSpecV5) -> LegacyDecisionSpec {
    LegacyDecisionSpec {
        legacy_configuration: LegacyConfigurationDecision::ManualOnly,
        sessions: SessionDecision::LogoutAll,
        legacy_cache: LegacyCacheDecision::DiscardEphemeralAfterFence,
        legacy_stripe: LegacyStripeDecision::AssertNone,
        temporary_subscription_links: TemporarySubscriptionLinkDecision::InvalidateAtCutover,
        nodes: decisions.nodes,
        legacy_traffic_details: Some(decisions.legacy_traffic_details),
        legacy_operational_logs: Some(decisions.legacy_operational_logs),
        legacy_theme: LegacyThemeDecision::DiscardConfirmed,
        legacy_custom_rules: decisions.legacy_custom_rules,
    }
}

fn hydrate_legacy_execution(
    operation_id: &str,
    input: LegacyExecutionDocumentV4,
    activation_transport: LegacyNodeActivationTransportSpec,
    legacy_traffic_details: Option<LegacyTrafficDetailsDecision>,
    durable_report_receipts: bool,
) -> LegacyExecutionSpec {
    let mut execution = LegacyExecutionSpec {
        journal: LegacyJournalExecutionSpec::default(),
        release: input.release,
        systemd: input.systemd,
        source_control: input.source_control,
        receipts: input.receipts,
        backup: input.backup,
        nodes: LegacyNodeExecutionSpec {
            activation_transport,
            inventory: Vec::new(),
        },
        legacy_traffic_details,
        source_retirement: LegacySourceRetirementExecutionSpec::default(),
    };
    let operation_root = Path::new(LIFECYCLE_STATE_ROOT)
        .join("operations")
        .join(operation_id);

    execution.journal.root = PathBuf::from(JOURNAL_ROOT);
    execution.journal.authorization_path = operation_root.join("authorization.json");
    execution.journal.activation_state_root = PathBuf::from(ACTIVATION_STATE_ROOT);

    execution.release.archive_path = operation_root.join("inputs/native-release.tar.gz");
    execution.release.releases_root = PathBuf::from(RELEASES_ROOT);
    execution.release.current_symlink = PathBuf::from(CURRENT_RELEASE_PATH);

    execution.systemd.api_unit = API_UNIT.to_string();
    execution.systemd.worker_unit = WORKER_UNIT.to_string();
    execution.systemd.api_ready_url = API_READY_URL.to_string();
    execution.systemd.worker_health_path = PathBuf::from(WORKER_HEALTH_PATH);

    let receipt_root = operation_root.join("receipts");
    execution.receipts.release_archive.path = receipt_root.join("release-archive.json");
    execution.receipts.source_fence_path = receipt_root.join("source-fence.json");
    execution.receipts.source_drain_path = receipt_root.join("source-drain.json");
    execution.receipts.backup_restore_path = receipt_root.join("backup-restore.json");
    execution.receipts.redis_fence_armed_path = receipt_root.join("redis-fence-armed.json");
    execution.receipts.redis_fence_path = receipt_root.join("redis-fence.json");
    execution.receipts.datastore_fence_armed_path = receipt_root.join("datastore-fence-armed.json");
    execution.receipts.datastore_fence_path = receipt_root.join("datastore-fence.json");
    execution.receipts.source_retirement_path = receipt_root.join("source-retirement.json");
    execution.receipts.runtime_compatibility_disabled_path =
        receipt_root.join("runtime-compatibility-disabled.json");
    execution.receipts.postgres_authority_path = receipt_root.join("postgres-authority.json");
    if durable_report_receipts {
        execution.receipts.postgres_verification_path =
            Some(receipt_root.join("postgres-verification-report.json"));
        execution.receipts.clickhouse_projection_path =
            Some(receipt_root.join("clickhouse-projection-report.json"));
    }

    execution.backup.mode = LegacyBackupMode::MysqlLogicalDumpAndIsolatedRestore;
    execution.backup.encrypted_backup_output_path =
        operation_root.join("outputs/legacy-backup.age");
    execution.backup.encryption_recipient_path = operation_root.join("inputs/backup-recipient.txt");
    execution.backup.decryption_identity_path = Path::new(LIFECYCLE_SECRET_ROOT)
        .join(operation_id)
        .join("age-identity");
    execution.backup.isolated_restore_state_path =
        operation_root.join("outputs/isolated-restore-state.json");

    execution.source_retirement.lifecycle_tool_path =
        PathBuf::from("/opt/v2board/lifecycle/v2board-lifecycle");
    execution.source_retirement.retirement_probe_state_path =
        operation_root.join("outputs/retirement-probe.json");
    execution
}

fn validate_manifest_execution_separation(
    manifest_path: &Path,
    spec: &ProvisionSpec,
) -> Result<(), ProvisionSpecError> {
    let Some(execution) = spec.legacy_apply_execution() else {
        return Ok(());
    };
    let manifest = fs::canonicalize(manifest_path).map_err(ProvisionSpecError::Metadata)?;
    let receipts = &execution.receipts;
    let restore_state_name = execution
        .backup
        .isolated_restore_state_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProvisionSpecError::Validation(
            "isolated restore state path requires a UTF-8 file name",
        ))?;
    let archive_materialization_state_path = execution
        .backup
        .isolated_restore_state_path
        .with_file_name(format!(".{restore_state_name}.archive-materialization"));
    let mut declared = vec![
        execution.journal.root.as_path(),
        execution.journal.authorization_path.as_path(),
        execution.journal.activation_state_root.as_path(),
        execution.release.archive_path.as_path(),
        execution.release.releases_root.as_path(),
        execution.release.current_symlink.as_path(),
        receipts.release_archive.path.as_path(),
        receipts.source_fence_path.as_path(),
        receipts.source_drain_path.as_path(),
        receipts.backup_restore_path.as_path(),
        receipts.redis_fence_armed_path.as_path(),
        receipts.redis_fence_path.as_path(),
        receipts.datastore_fence_armed_path.as_path(),
        receipts.datastore_fence_path.as_path(),
        receipts.source_retirement_path.as_path(),
        receipts.runtime_compatibility_disabled_path.as_path(),
        receipts.postgres_authority_path.as_path(),
        execution.backup.encrypted_backup_output_path.as_path(),
        execution.backup.encryption_recipient_path.as_path(),
        execution.backup.decryption_identity_path.as_path(),
        execution.backup.isolated_restore_state_path.as_path(),
        archive_materialization_state_path.as_path(),
        execution
            .source_retirement
            .retirement_probe_state_path
            .as_path(),
    ];
    declared.extend(
        receipts
            .postgres_verification_path
            .iter()
            .map(PathBuf::as_path),
    );
    declared.extend(
        receipts
            .clickhouse_projection_path
            .iter()
            .map(PathBuf::as_path),
    );
    if declared.contains(&manifest.as_path()) {
        return Err(ProvisionSpecError::Validation(
            "the lifecycle manifest must not alias any journal, release, receipt, backup, node, or retirement path",
        ));
    }
    Ok(())
}

fn validate_spec(spec: &ProvisionSpec) -> Result<(), ProvisionSpecError> {
    if !matches!(spec.schema_version, 3..=5) {
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
    let datastore_secrets = spec.target_secret_values()?;
    if datastore_secrets
        .iter()
        .any(|secret| secret == &spec.lifecycle_audit_key)
    {
        return Err(ProvisionSpecError::Validation(
            "lifecycle_audit_key must be different from target datastore passwords",
        ));
    }
    if datastore_secrets.iter().collect::<BTreeSet<_>>().len() != datastore_secrets.len() {
        return Err(ProvisionSpecError::Validation(
            "target and isolated-restore secrets must be pairwise distinct",
        ));
    }
    if ["app_key", "server_token"].iter().any(|key| {
        runtime
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(|value| datastore_secrets.iter().any(|secret| secret == value))
    }) {
        return Err(ProvisionSpecError::Validation(
            "runtime app_key and server_token must differ from datastore passwords",
        ));
    }
    let operator = spec.normalized_operator_config_candidate()?;
    let expected_operator = OPERATOR_CONFIG_KEYS_V1
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual_operator = operator
        .as_map()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual_operator != expected_operator {
        return Err(ProvisionSpecError::RuntimeSemantics(
            "typed operator candidate does not have the exact version-1 key set".to_string(),
        ));
    }

    let api_boot = spec.materialized_api_runtime_config()?;
    let worker_boot = spec.materialized_worker_runtime_config()?;
    if OPERATOR_CONFIG_KEYS_V1
        .iter()
        .any(|key| api_boot.contains_key(*key) || worker_boot.contains_key(*key))
    {
        return Err(ProvisionSpecError::RuntimeSemantics(
            "long-lived role document contains dynamic operator configuration".to_string(),
        ));
    }
    AppConfig::try_from_api_boot_config_map(
        api_boot,
        RuntimePaths {
            config: spec.api_runtime_config_path().to_path_buf(),
            frontend: PathBuf::from("/opt/v2board/frontend"),
            rules: PathBuf::from("/var/lib/v2board/rules"),
        },
    )
    .map_err(|error| ProvisionSpecError::RuntimeSemantics(error.to_string()))?;
    AppConfig::try_from_worker_boot_config_map(
        worker_boot,
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
            runtime,
            decisions,
            attestations,
            execution,
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
                || decisions.temporary_subscription_links
                    != TemporarySubscriptionLinkDecision::InvalidateAtCutover
                || decisions.legacy_theme != LegacyThemeDecision::DiscardConfirmed
                || !matches!(
                    decisions.legacy_custom_rules,
                    LegacyCustomRulesDecision::None | LegacyCustomRulesDecision::DiscardConfirmed
                )
            {
                return Err(if matches!(spec.schema_version, 3 | 4) {
                    ProvisionSpecError::Validation(
                        "legacy migration decisions must use manual config, logout-all, discard-fenced cache, zero Stripe, invalidated temporary subscription links, and one-shot offline cutover",
                    )
                } else {
                    ProvisionSpecError::Validation(
                        "legacy migration decisions must use manual config, logout-all, discard-fenced cache, zero Stripe, and invalidated temporary subscription links",
                    )
                });
            }
            let schema_decisions_are_valid = match spec.schema_version {
                3 | 4 => {
                    decisions.nodes == NodeDecision::OneShotOfflineCutover
                        && decisions.legacy_traffic_details.is_none()
                        && decisions.legacy_operational_logs.is_none()
                }
                5 => {
                    decisions.nodes == NodeDecision::DiscardAndManualRebuild
                        && decisions.legacy_traffic_details
                            == Some(LegacyTrafficDetailsDecision::Discard)
                        && decisions.legacy_operational_logs
                            == Some(LegacyOperationalLogsDecision::Discard)
                }
                _ => false,
            };
            if !schema_decisions_are_valid {
                return Err(if matches!(spec.schema_version, 3 | 4) {
                    ProvisionSpecError::Validation(
                        "legacy migration decisions must use manual config, logout-all, discard-fenced cache, zero Stripe, invalidated temporary subscription links, and one-shot offline cutover",
                    )
                } else {
                    ProvisionSpecError::Validation(
                        "legacy migration node, traffic-detail, and operational-log decisions do not match the selected schema version",
                    )
                });
            }
            validate_legacy_source(source, target)?;
            validate_target(target)?;
            if !matches!(
                runtime.get("show_subscribe_method").and_then(Value::as_i64),
                Some(0 | 1)
            ) {
                return Err(ProvisionSpecError::Validation(
                    "legacy migration target runtime.show_subscribe_method must be 0 or 1 so old temporary URLs are invalid at cutover",
                ));
            }
            match (spec.schema_version, execution, attestations) {
                (3, None, Some(_)) => {}
                (4, Some(execution), None) => {
                    validate_legacy_execution(
                        &spec.operation_id,
                        source,
                        execution,
                        LegacyNodeActivationTransportSpec::NotRequiredNoNodes,
                        None,
                        false,
                    )?;
                }
                (5, Some(execution), None) => {
                    validate_legacy_execution(
                        &spec.operation_id,
                        source,
                        execution,
                        LegacyNodeActivationTransportSpec::DiscardAndManualRebuild,
                        Some(LegacyTrafficDetailsDecision::Discard),
                        true,
                    )?;
                }
                _ => {
                    return Err(ProvisionSpecError::Validation(
                        "legacy execution inputs are required by schema_version 4 and 5 and forbidden in schema_version 3",
                    ));
                }
            }
            if attestations
                .as_ref()
                .and_then(|attestations| attestations.backup_reference.as_deref())
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

fn validate_legacy_execution(
    operation_id: &str,
    source: &SourceSpec,
    execution: &LegacyExecutionSpec,
    expected_node_transport: LegacyNodeActivationTransportSpec,
    expected_traffic_details: Option<LegacyTrafficDetailsDecision>,
    durable_report_receipts: bool,
) -> Result<(), ProvisionSpecError> {
    let journal = &execution.journal;
    if journal.root != Path::new(JOURNAL_ROOT)
        || journal.activation_state_root != Path::new(ACTIVATION_STATE_ROOT)
    {
        return Err(ProvisionSpecError::Validation(
            "legacy execution journal and activation roots must use the frozen /var/lib/v2board/lifecycle locations",
        ));
    }
    validate_absolute_normalized_path(&journal.root)?;
    validate_absolute_normalized_path(&journal.activation_state_root)?;
    validate_operation_private_path(operation_id, &journal.authorization_path)?;

    let release = &execution.release;
    if !valid_release_id(&release.release_id)
        || !is_lower_hex(&release.archive_sha256, 64)
        || release.releases_root != Path::new(RELEASES_ROOT)
        || release.current_symlink != Path::new(CURRENT_RELEASE_PATH)
    {
        return Err(ProvisionSpecError::Validation(
            "legacy release must bind a safe release_id, lowercase archive SHA-256, and the frozen release/current paths",
        ));
    }
    validate_absolute_normalized_path(&release.releases_root)?;
    validate_absolute_normalized_path(&release.current_symlink)?;
    validate_operation_private_path(operation_id, &release.archive_path)?;

    let systemd = &execution.systemd;
    if systemd.api_unit != API_UNIT
        || systemd.worker_unit != WORKER_UNIT
        || systemd.api_ready_url != API_READY_URL
        || systemd.worker_health_path != Path::new(WORKER_HEALTH_PATH)
    {
        return Err(ProvisionSpecError::Validation(
            "legacy systemd target units, API readiness URL, and worker health path must match the frozen bare-metal release contract",
        ));
    }
    validate_absolute_normalized_path(&systemd.worker_health_path)?;
    validate_legacy_source_units(systemd)?;
    validate_legacy_source_control(source, systemd, &execution.source_control)?;

    let receipts = &execution.receipts;
    if !is_lower_hex(&receipts.release_archive.sha256, 64) {
        return Err(ProvisionSpecError::Validation(
            "the immutable release archive receipt requires a lowercase SHA-256",
        ));
    }
    if receipts.postgres_verification_path.is_some() != durable_report_receipts
        || receipts.clickhouse_projection_path.is_some() != durable_report_receipts
    {
        return Err(ProvisionSpecError::Validation(
            "schema-v5 requires both derived durable report receipt paths and schema-v4 forbids them",
        ));
    }
    let mut receipt_paths = vec![
        receipts.release_archive.path.as_path(),
        receipts.source_fence_path.as_path(),
        receipts.source_drain_path.as_path(),
        receipts.backup_restore_path.as_path(),
        receipts.redis_fence_armed_path.as_path(),
        receipts.redis_fence_path.as_path(),
        receipts.datastore_fence_armed_path.as_path(),
        receipts.datastore_fence_path.as_path(),
        receipts.source_retirement_path.as_path(),
        receipts.runtime_compatibility_disabled_path.as_path(),
        receipts.postgres_authority_path.as_path(),
    ];
    receipt_paths.extend(
        receipts
            .postgres_verification_path
            .iter()
            .map(PathBuf::as_path),
    );
    receipt_paths.extend(
        receipts
            .clickhouse_projection_path
            .iter()
            .map(PathBuf::as_path),
    );
    for path in &receipt_paths {
        validate_operation_private_path(operation_id, path)?;
    }
    require_unique_paths(
        &receipt_paths,
        "legacy receipt paths must be pairwise distinct",
    )?;

    let backup = &execution.backup;
    if backup.mode != LegacyBackupMode::MysqlLogicalDumpAndIsolatedRestore
        || !valid_opaque_reference(&backup.backup_reference)
        || !is_lower_hex(&backup.encryption_recipient_sha256, 64)
        || !is_lower_hex(&backup.decryption_identity_sha256, 64)
        || backup.encryption_recipient_sha256 == backup.decryption_identity_sha256
        || !(300..=604_800).contains(&backup.command_timeout_seconds)
        || !(16 * 1024 * 1024..=16 * 1024_u64.pow(4))
            .contains(&backup.maximum_encrypted_backup_bytes)
    {
        return Err(ProvisionSpecError::Validation(
            "legacy backup must use an encrypted MySQL logical dump, an empty isolated restore, full verification, and cleanup",
        ));
    }
    validate_operation_private_path(operation_id, &backup.encrypted_backup_output_path)?;
    validate_operation_private_path(operation_id, &backup.encryption_recipient_path)?;
    validate_backup_identity_path(operation_id, &backup.decryption_identity_path)?;
    validate_operation_private_path(operation_id, &backup.isolated_restore_state_path)?;
    validate_mysql_url(
        &backup.isolated_restore_database_url,
        "execution.backup.isolated_restore_database_url",
    )?;
    let restore_database =
        Url::parse(&backup.isolated_restore_database_url).expect("validated isolated restore URL");
    let restore_database = strict_percent_decode(
        restore_database
            .path()
            .strip_prefix('/')
            .unwrap_or_default(),
    )?;
    if matches!(
        restore_database.to_ascii_lowercase().as_str(),
        "mysql" | "information_schema" | "performance_schema" | "sys"
    ) {
        return Err(ProvisionSpecError::Validation(
            "isolated restore database must not name a MySQL system schema",
        ));
    }
    if backup.isolated_restore_transport_security == SourceTransportSecurity::VerifiedTls
        && !mysql_url_verifies_identity(&backup.isolated_restore_database_url)
    {
        return Err(ProvisionSpecError::Validation(
            "verified isolated restore transport requires MySQL VERIFY_IDENTITY",
        ));
    }
    if datastore_identity(&backup.isolated_restore_database_url)?
        == datastore_identity(&source.database_url)?
    {
        return Err(ProvisionSpecError::Validation(
            "the isolated restore database must not alias the legacy source database",
        ));
    }
    let source_password = Url::parse(&source.database_url)
        .expect("validated source URL")
        .password()
        .map(strict_percent_decode)
        .transpose()?
        .unwrap_or_default();
    let restore_password = Url::parse(&backup.isolated_restore_database_url)
        .expect("validated restore URL")
        .password()
        .map(strict_percent_decode)
        .transpose()?
        .unwrap_or_default();
    if source_password == restore_password {
        return Err(ProvisionSpecError::Validation(
            "the isolated restore credential must be independent from the legacy source credential",
        ));
    }

    let nodes = &execution.nodes;
    if !nodes.inventory.is_empty()
        || nodes.activation_transport != expected_node_transport
        || execution.legacy_traffic_details != expected_traffic_details
    {
        return Err(match expected_node_transport {
            LegacyNodeActivationTransportSpec::NotRequiredNoNodes => {
                ProvisionSpecError::Validation(
                    "schema-v4 legacy migration requires an empty node inventory and not_required_no_nodes because external node activation is outside this repository",
                )
            }
            LegacyNodeActivationTransportSpec::DiscardAndManualRebuild => {
                ProvisionSpecError::Validation(
                    "schema-v5 legacy migration requires an empty target node inventory and discard_and_manual_rebuild; source inventory must come from inspection",
                )
            }
        });
    }

    let retirement = &execution.source_retirement;
    if !matches!(
        retirement.lifecycle_tool_path.to_str(),
        Some("/usr/local/sbin/v2board-lifecycle")
            | Some("/opt/v2board/lifecycle/v2board-lifecycle")
    ) {
        return Err(ProvisionSpecError::Validation(
            "legacy source retirement requires an allowed disposable lifecycle tool path",
        ));
    }
    validate_absolute_normalized_path(&retirement.lifecycle_tool_path)?;
    validate_operation_private_path(operation_id, &retirement.retirement_probe_state_path)?;

    let restore_state_name = backup
        .isolated_restore_state_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProvisionSpecError::Validation(
            "isolated restore state path requires a UTF-8 file name",
        ))?;
    let archive_materialization_state_path = backup
        .isolated_restore_state_path
        .with_file_name(format!(".{restore_state_name}.archive-materialization"));
    validate_operation_private_path(operation_id, &archive_materialization_state_path)?;
    let mut all_file_paths = vec![
        journal.authorization_path.as_path(),
        release.archive_path.as_path(),
        receipts.release_archive.path.as_path(),
        receipts.source_fence_path.as_path(),
        receipts.source_drain_path.as_path(),
        receipts.backup_restore_path.as_path(),
        receipts.redis_fence_armed_path.as_path(),
        receipts.redis_fence_path.as_path(),
        receipts.datastore_fence_armed_path.as_path(),
        receipts.datastore_fence_path.as_path(),
        receipts.source_retirement_path.as_path(),
        receipts.runtime_compatibility_disabled_path.as_path(),
        receipts.postgres_authority_path.as_path(),
        backup.encrypted_backup_output_path.as_path(),
        backup.encryption_recipient_path.as_path(),
        backup.decryption_identity_path.as_path(),
        backup.isolated_restore_state_path.as_path(),
        archive_materialization_state_path.as_path(),
        retirement.retirement_probe_state_path.as_path(),
    ];
    all_file_paths.extend(
        receipts
            .postgres_verification_path
            .iter()
            .map(PathBuf::as_path),
    );
    all_file_paths.extend(
        receipts
            .clickhouse_projection_path
            .iter()
            .map(PathBuf::as_path),
    );
    require_unique_paths(
        &all_file_paths,
        "legacy execution file paths must not alias one another",
    )?;
    let mut all_declared_paths = vec![
        journal.root.as_path(),
        journal.activation_state_root.as_path(),
        release.releases_root.as_path(),
        release.current_symlink.as_path(),
        Path::new("/var/lib/v2board/api/config.json"),
        Path::new("/var/lib/v2board/worker/config.json"),
        retirement.lifecycle_tool_path.as_path(),
        systemd.worker_health_path.as_path(),
    ];
    all_declared_paths.extend(all_file_paths);
    require_unique_paths(
        &all_declared_paths,
        "legacy execution paths must not alias journal, release, config, lifecycle-tool, health, input, or output paths",
    )?;
    Ok(())
}

fn validate_legacy_source_units(
    systemd: &LegacySystemdExecutionSpec,
) -> Result<(), ProvisionSpecError> {
    if systemd.legacy_writer_units.is_empty()
        || systemd.legacy_worker_units.is_empty()
        || systemd.legacy_scheduler_units.is_empty()
        || systemd
            .legacy_writer_units
            .iter()
            .chain(&systemd.legacy_worker_units)
            .any(|unit| !unit.ends_with(".service"))
        || !systemd
            .legacy_scheduler_units
            .iter()
            .any(|unit| unit.ends_with(".timer"))
        || !systemd
            .legacy_scheduler_units
            .iter()
            .any(|unit| unit.ends_with(".service"))
        || !scheduler_units_are_exact_pairs(&systemd.legacy_scheduler_units)
    {
        return Err(ProvisionSpecError::Validation(
            "legacy source control requires nonempty writer/worker services and both the dedicated scheduler timer and its triggered service",
        ));
    }
    let units = systemd
        .legacy_writer_units
        .iter()
        .chain(&systemd.legacy_worker_units)
        .chain(&systemd.legacy_scheduler_units)
        .collect::<Vec<_>>();
    if units.iter().any(|unit| {
        !valid_systemd_unit_name(unit) || matches!(unit.as_str(), API_UNIT | WORKER_UNIT)
    }) {
        return Err(ProvisionSpecError::Validation(
            "legacy source systemd units must use safe unique service/timer names and must not name native units",
        ));
    }
    if units
        .iter()
        .map(|unit| unit.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != units.len()
    {
        return Err(ProvisionSpecError::Validation(
            "legacy source systemd units must be pairwise distinct across writer, worker, and scheduler roles",
        ));
    }
    Ok(())
}

fn validate_legacy_source_control(
    source: &SourceSpec,
    systemd: &LegacySystemdExecutionSpec,
    control: &LegacySourceControlExecutionSpec,
) -> Result<(), ProvisionSpecError> {
    if [
        source.redis_connection_prefix.as_str(),
        source.redis_cache_prefix.as_str(),
    ]
    .into_iter()
    .any(|prefix| !valid_redis_physical_prefix(prefix, true))
    {
        return Err(ProvisionSpecError::Validation(
            "executable legacy schemas require the exact nonempty Laravel Redis connection and cache prefixes",
        ));
    }
    if !valid_redis_physical_prefix(&source.redis_horizon_prefix, true) {
        return Err(ProvisionSpecError::Validation(
            "executable legacy schemas require the exact nonempty Laravel Horizon Redis prefix",
        ));
    }
    if [
        source.database_url.as_str(),
        source
            .database_fence_url
            .as_deref()
            .ok_or(ProvisionSpecError::Validation(
                "executable legacy schemas require source.database_fence_url for the durable MySQL write fence",
            ))?,
        source.redis_default_url.as_str(),
        source.redis_cache_url.as_str(),
    ]
    .into_iter()
    .any(|url| !url_uses_literal_loopback(url))
    {
        return Err(ProvisionSpecError::Validation(
            "executable legacy schemas require literal-loopback local_dedicated_systemd source datastores; remote or managed sources need a future provider-specific automation adapter",
        ));
    }
    let datastore_units = [
        &control.datastores.mysql,
        &control.datastores.default_redis,
        &control.datastores.cache_redis,
    ];
    for redis_url in [&source.redis_default_url, &source.redis_cache_url] {
        let parsed = Url::parse(redis_url)
            .map_err(|_| ProvisionSpecError::Validation("source Redis fence URL must be valid"))?;
        if parsed.username().is_empty() || parsed.username() == "default" {
            return Err(ProvisionSpecError::Validation(
                "executable legacy schemas require source Redis URLs to use a dedicated named lifecycle ACL user with the reviewed full-access drain/fence grant",
            ));
        }
    }
    let legacy_runtime_units = systemd
        .legacy_writer_units
        .iter()
        .chain(&systemd.legacy_worker_units)
        .chain(&systemd.legacy_scheduler_units)
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for datastore in datastore_units {
        let unit = datastore.unit.as_str();
        if !valid_systemd_unit_name(unit)
            || !unit.ends_with(".service")
            || matches!(unit, API_UNIT | WORKER_UNIT)
            || legacy_runtime_units.contains(unit)
        {
            return Err(ProvisionSpecError::Validation(
                "local source datastore units must be dedicated .service names disjoint from legacy and native runtime units",
            ));
        }
    }
    let mysql_unit = control.datastores.mysql.unit.as_str();
    let default_redis_unit = control.datastores.default_redis.unit.as_str();
    let cache_redis_unit = control.datastores.cache_redis.unit.as_str();
    if mysql_unit == default_redis_unit || mysql_unit == cache_redis_unit {
        return Err(ProvisionSpecError::Validation(
            "the dedicated MySQL unit must be distinct from every Redis unit",
        ));
    }
    let redis_same_process = redis_service_identity(&source.redis_default_url)?
        == redis_service_identity(&source.redis_cache_url)?;
    if (default_redis_unit == cache_redis_unit) != redis_same_process {
        return Err(ProvisionSpecError::Validation(
            "default and cache Redis must name the same systemd unit exactly when their URLs name the same local Redis process",
        ));
    }
    Ok(())
}

fn url_uses_literal_loopback(value: &str) -> bool {
    Url::parse(value)
        .ok()
        .and_then(|url| normalized_url_host(url.host_str()?).parse::<IpAddr>().ok())
        .is_some_and(|address| address.is_loopback())
}

fn redis_service_identity(value: &str) -> Result<(String, u16), ProvisionSpecError> {
    let url = Url::parse(value)
        .map_err(|_| ProvisionSpecError::Validation("source Redis URL is invalid"))?;
    let host = url.host_str().ok_or(ProvisionSpecError::Validation(
        "source Redis URL has no host",
    ))?;
    let port = url
        .port_or_known_default()
        .ok_or(ProvisionSpecError::Validation(
            "source Redis URL has no port",
        ))?;
    Ok((normalized_url_host(host).to_ascii_lowercase(), port))
}

fn normalized_url_host(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host)
}

fn valid_systemd_unit_name(value: &str) -> bool {
    (1..=255).contains(&value.len())
        && !value.starts_with('.')
        && !value.contains("..")
        && matches!(
            value.rsplit_once('.').map(|(_, suffix)| suffix),
            Some("service" | "timer")
        )
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.' | b'@')
        })
}

pub(crate) fn scheduler_units_are_exact_pairs(units: &[String]) -> bool {
    let inventory = units.iter().map(String::as_str).collect::<BTreeSet<_>>();
    !units.is_empty()
        && units.len().is_multiple_of(2)
        && units.iter().all(|unit| {
            scheduler_unit_counterpart(unit)
                .is_some_and(|counterpart| inventory.contains(counterpart.as_str()))
        })
}

pub(crate) fn scheduler_unit_counterpart(unit: &str) -> Option<String> {
    unit.strip_suffix(".timer")
        .map(|stem| format!("{stem}.service"))
        .or_else(|| {
            unit.strip_suffix(".service")
                .map(|stem| format!("{stem}.timer"))
        })
}

fn valid_release_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && !value.starts_with('.')
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_opaque_reference(value: &str) -> bool {
    let value = value.trim();
    (8..=1024).contains(&value.len())
        && value == value.trim()
        && !is_placeholder(value, 8)
        && !value.chars().any(char::is_control)
        && Url::parse(value)
            .ok()
            .is_none_or(|url| url.username().is_empty() && url.password().is_none())
}

fn validate_absolute_normalized_path(path: &Path) -> Result<(), ProvisionSpecError> {
    let Some(text) = path.to_str() else {
        return Err(ProvisionSpecError::Validation(
            "legacy execution paths must be UTF-8 absolute paths",
        ));
    };
    if !path.is_absolute()
        || text.len() > 4096
        || text.ends_with('/')
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(ProvisionSpecError::Validation(
            "legacy execution paths must be normalized absolute paths without dot components or trailing slash",
        ));
    }
    Ok(())
}

fn validate_operation_private_path(
    operation_id: &str,
    path: &Path,
) -> Result<(), ProvisionSpecError> {
    validate_absolute_normalized_path(path)?;
    let private_root = Path::new(LIFECYCLE_STATE_ROOT);
    if path == private_root
        || !path.starts_with(private_root)
        || !path
            .components()
            .any(|component| component.as_os_str() == operation_id)
    {
        return Err(ProvisionSpecError::Validation(
            "operation files and directories must be under /var/lib/v2board/lifecycle and contain the exact operation_id path component",
        ));
    }
    Ok(())
}

fn validate_backup_identity_path(
    operation_id: &str,
    path: &Path,
) -> Result<(), ProvisionSpecError> {
    validate_absolute_normalized_path(path)?;
    let expected = Path::new(LIFECYCLE_SECRET_ROOT)
        .join(operation_id)
        .join("age-identity");
    if path != expected {
        return Err(ProvisionSpecError::Validation(
            "backup decryption identity must be supplied separately at /run/v2board-lifecycle-secrets/<operation_id>/age-identity",
        ));
    }
    Ok(())
}

fn require_unique_paths(paths: &[&Path], message: &'static str) -> Result<(), ProvisionSpecError> {
    if paths.iter().copied().collect::<BTreeSet<_>>().len() != paths.len() {
        return Err(ProvisionSpecError::Validation(message));
    }
    Ok(())
}

fn validate_legacy_source(
    source: &SourceSpec,
    target: &TargetSpec,
) -> Result<(), ProvisionSpecError> {
    validate_mysql_url(&source.database_url, "source.database_url")?;
    if let Some(database_fence_url) = &source.database_fence_url {
        validate_mysql_fence_url(database_fence_url)?;
        if mysql_server_endpoint_identity(database_fence_url)?
            != mysql_server_endpoint_identity(&source.database_url)?
        {
            return Err(ProvisionSpecError::Validation(
                "source.database_fence_url must name the exact same MySQL server endpoint as source.database_url",
            ));
        }
        let reader = Url::parse(&source.database_url).expect("validated source URL");
        let fence = Url::parse(database_fence_url).expect("validated source fence URL");
        if reader.username() == fence.username() || reader.password() == fence.password() {
            return Err(ProvisionSpecError::Validation(
                "source.database_fence_url must use a username and password independent from the read-only source credential",
            ));
        }
    }
    validate_redis_url(&source.redis_default_url, "source.redis_default_url")?;
    validate_redis_url(&source.redis_cache_url, "source.redis_cache_url")?;
    if [&source.redis_connection_prefix, &source.redis_cache_prefix]
        .iter()
        .any(|prefix| !valid_redis_physical_prefix(prefix, false))
        || (!source.redis_horizon_prefix.is_empty()
            && !valid_redis_physical_prefix(&source.redis_horizon_prefix, true))
    {
        return Err(ProvisionSpecError::Validation(
            "source Redis prefixes must not contain glob/control characters; a declared Horizon prefix must be nonempty",
        ));
    }
    if source.transport_security == SourceTransportSecurity::VerifiedTls
        && (!mysql_url_verifies_identity(&source.database_url)
            || source
                .database_fence_url
                .as_deref()
                .is_some_and(|url| !mysql_url_verifies_identity(url))
            || !redis_url_uses_tls(&source.redis_default_url)
            || !redis_url_uses_tls(&source.redis_cache_url))
    {
        return Err(ProvisionSpecError::Validation(
            "source verified_tls requires MySQL VERIFY_IDENTITY and rediss:// for both Redis databases",
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

fn valid_redis_physical_prefix(value: &str, require_nonempty: bool) -> bool {
    (!require_nonempty || !value.is_empty())
        && value.len() <= 1024
        && !value.chars().any(|character| {
            character.is_control() || matches!(character, '*' | '?' | '[' | ']' | '\\')
        })
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
    validate_analytics_admission(&target.analytics_admission)?;
    validate_redis_url(&target.redis_url, "target.redis_url")?;
    if !redis_url_uses_tls(&target.redis_url) {
        return Err(ProvisionSpecError::Validation(
            "target.redis_url must use rediss:// with certificate verification",
        ));
    }
    Ok(())
}

fn validate_analytics_admission(policy: &AnalyticsAdmissionSpec) -> Result<(), ProvisionSpecError> {
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
        || is_placeholder(&policy.capacity_evidence, 8)
        || policy.capacity_evidence.len() > 1024
    {
        return Err(ProvisionSpecError::Validation(
            "analytics admission thresholds must be ordered, fit signed PostgreSQL integers, reserve one event, bind a 100000..=10000000-row soft window, keep a sample fresh within 600 seconds, and include capacity evidence",
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
    for key in DECIMAL_RUNTIME_KEYS {
        let value = runtime.get(*key);
        let lossless_decimal = match value {
            Some(Value::String(value)) => value.trim().parse::<rust_decimal::Decimal>().is_ok(),
            // Historical schema-v3 manifests used integer JSON values. Keep
            // accepting those exact integers, but never pass a JSON float
            // through binary floating-point conversion.
            Some(Value::Number(value)) => value.as_i64().is_some(),
            _ => false,
        };
        if !lossless_decimal {
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
            || DECIMAL_RUNTIME_KEYS.contains(&key.as_str())
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

fn validate_mysql_fence_url(value: &str) -> Result<(), ProvisionSpecError> {
    let url = Url::parse(value).map_err(|_| {
        ProvisionSpecError::Validation("source.database_fence_url must be a valid mysql:// URL")
    })?;
    let username = strict_percent_decode(url.username())?;
    let password = url
        .password()
        .map(strict_percent_decode)
        .transpose()?
        .unwrap_or_default();
    if url.scheme() != "mysql"
        || url.host_str().is_none()
        || !matches!(url.path(), "" | "/")
        || username.is_empty()
        || password.is_empty()
        || url.fragment().is_some()
        || is_placeholder(&password, 1)
    {
        return Err(ProvisionSpecError::Validation(
            "source.database_fence_url must include host, independent username/password, and no default database",
        ));
    }
    validate_mysql_connection_query(&url)?;
    Ok(())
}

fn mysql_server_endpoint_identity(value: &str) -> Result<(String, u16), ProvisionSpecError> {
    let url = Url::parse(value)
        .map_err(|_| ProvisionSpecError::Validation("MySQL server endpoint URL must be valid"))?;
    let host = url
        .host_str()
        .map(normalized_url_host)
        .ok_or(ProvisionSpecError::Validation(
            "MySQL server endpoint URL must include a host",
        ))?;
    Ok((
        host.to_string(),
        url.port_or_known_default().unwrap_or(3306),
    ))
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

    /// Returns the complete, HMAC-bound bare-metal intent for schema-v4 and
    /// schema-v5 legacy migrations. Schema v3 remains loadable for
    /// validate/inspect, but deliberately has no apply execution accessor.
    pub fn legacy_apply_execution(&self) -> Option<&LegacyExecutionSpec> {
        match &self.flow {
            ProvisionFlow::LegacyReferenceMigration {
                execution: Some(execution),
                ..
            } if matches!(self.schema_version, 4 | 5) => Some(execution.as_ref()),
            _ => None,
        }
    }

    /// A scoped binding for constructors that only need execution policy.
    /// The whole raw manifest remains bound by `manifest_binding_hmac_sha256`.
    pub fn legacy_execution_binding_hmac_sha256(&self) -> Option<String> {
        let execution = self.legacy_apply_execution()?;
        let bytes =
            serde_json::to_vec(execution).expect("LegacyExecutionSpec serialization cannot fail");
        let mut mac =
            <Hmac<Sha256> as KeyInit>::new_from_slice(self.lifecycle_audit_key.as_bytes())
                .expect("HMAC accepts keys of any length");
        mac.update(match self.schema_version {
            4 => LEGACY_EXECUTION_HMAC_DOMAIN_V1,
            5 => LEGACY_EXECUTION_HMAC_DOMAIN_V2,
            _ => return None,
        });
        mac.update(self.operation_id.as_bytes());
        mac.update(&[0]);
        mac.update(&bytes);
        Some(hex::encode(mac.finalize().into_bytes()))
    }

    /// Binds evidence produced after authorization without exposing the audit
    /// key or pretending the evidence digest existed in the manifest. Only a
    /// schema-v4 or schema-v5 legacy operation can mint or verify this scoped
    /// receipt MAC.
    pub(crate) fn source_receipt_binding_hmac_sha256(
        &self,
        kind: LegacyRuntimeReceiptKind,
        canonical_bytes: &[u8],
    ) -> Option<String> {
        let mac = self.source_receipt_mac(kind, canonical_bytes)?;
        Some(hex::encode(mac.finalize().into_bytes()))
    }

    pub(crate) fn verify_source_receipt_binding_hmac_sha256(
        &self,
        kind: LegacyRuntimeReceiptKind,
        canonical_bytes: &[u8],
        expected_hex: &str,
    ) -> bool {
        if !is_lower_hex(expected_hex, 64) {
            return false;
        }
        let Ok(expected) = hex::decode(expected_hex) else {
            return false;
        };
        self.source_receipt_mac(kind, canonical_bytes)
            .is_some_and(|mac| mac.verify_slice(&expected).is_ok())
    }

    fn source_receipt_mac(
        &self,
        kind: LegacyRuntimeReceiptKind,
        canonical_bytes: &[u8],
    ) -> Option<Hmac<Sha256>> {
        self.legacy_apply_execution()?;
        if kind.schema_v5_only() && self.schema_version != 5 {
            return None;
        }
        let mut mac =
            <Hmac<Sha256> as KeyInit>::new_from_slice(self.lifecycle_audit_key.as_bytes())
                .expect("HMAC accepts keys of any length");
        mac.update(match self.schema_version {
            4 => LEGACY_RUNTIME_RECEIPT_HMAC_DOMAIN_V1,
            5 => LEGACY_RUNTIME_RECEIPT_HMAC_DOMAIN_V2,
            _ => return None,
        });
        mac.update(kind.domain_label());
        mac.update(&[0]);
        mac.update(self.operation_id.as_bytes());
        mac.update(&[0]);
        mac.update(self.manifest_binding_hmac_sha256.as_bytes());
        mac.update(&[0]);
        mac.update(canonical_bytes);
        Some(mac)
    }

    /// Private full materialization used only while validating and normalizing
    /// the one operator-maintained manifest. It must never be installed as a
    /// long-lived role file because it contains the dynamic secret baseline.
    fn full_api_runtime_config(&self) -> Result<Map<String, Value>, ProvisionSpecError> {
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

    /// Worker counterpart of [`Self::full_api_runtime_config`]. The full map is
    /// short-lived typed-validation input only and is never serialized.
    fn full_worker_runtime_config(&self) -> Result<Map<String, Value>, ProvisionSpecError> {
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

    /// Returns the exact dynamic authority derived from both independently
    /// typed role views. A manifest is rejected unless the normalized API and
    /// Worker candidates are byte-for-byte equal as JSON values.
    pub fn normalized_operator_config_candidate(
        &self,
    ) -> Result<NormalizedOperatorConfigCandidate, ProvisionSpecError> {
        let api = AppConfig::try_from_api_config_map(
            self.full_api_runtime_config()?,
            RuntimePaths {
                config: self.api_runtime_config_path().to_path_buf(),
                frontend: PathBuf::from("/opt/v2board/frontend"),
                rules: PathBuf::from("/var/lib/v2board/rules"),
            },
        )
        .map_err(|error| ProvisionSpecError::RuntimeSemantics(error.to_string()))?;
        let worker = AppConfig::try_from_worker_config_map(
            self.full_worker_runtime_config()?,
            RuntimePaths {
                config: self.worker_runtime_config_path().to_path_buf(),
                frontend: PathBuf::from("/opt/v2board/frontend"),
                rules: PathBuf::from("/var/lib/v2board/rules"),
            },
        )
        .map_err(|error| ProvisionSpecError::RuntimeSemantics(error.to_string()))?;
        let values = api.operator_config_map();
        if values != worker.operator_config_map() {
            return Err(ProvisionSpecError::RuntimeSemantics(
                "API and Worker normalized different operator configuration candidates".to_string(),
            ));
        }
        Ok(NormalizedOperatorConfigCandidate { values })
    }

    fn boot_runtime_base(&self) -> Map<String, Value> {
        let mut boot = BOOT_ONLY_RUNTIME_KEYS_V1
            .iter()
            .filter_map(|key| {
                self.runtime()
                    .get(*key)
                    .cloned()
                    .map(|value| ((*key).to_string(), value))
            })
            .collect::<Map<_, _>>();
        boot.insert(
            "configuration_source".to_string(),
            Value::String("file_only".to_string()),
        );
        boot.insert(
            "configuration_scope".to_string(),
            Value::String("boot_only".to_string()),
        );
        boot
    }

    /// Exact long-lived API boot document. Dynamic configuration and all four
    /// operator secrets are absent; the lifecycle process seeds them directly
    /// into PostgreSQL before either service can start.
    pub fn materialized_api_runtime_config(
        &self,
    ) -> Result<Map<String, Value>, ProvisionSpecError> {
        let full = self.full_api_runtime_config()?;
        let mut boot = self.boot_runtime_base();
        for key in [
            "runtime_role",
            "database_url",
            "peer_database_principal",
            "redis_url",
        ] {
            let value = full.get(key).cloned().ok_or_else(|| {
                ProvisionSpecError::RuntimeSemantics(format!(
                    "API boot materialization is missing {key}"
                ))
            })?;
            boot.insert(key.to_string(), value);
        }
        Ok(boot)
    }

    /// Exact long-lived Worker boot document. It includes only the Worker
    /// datastore credentials needed to reach the encrypted shared authority;
    /// no dynamic integration secret is ever disclosed to this file.
    pub fn materialized_worker_runtime_config(
        &self,
    ) -> Result<Map<String, Value>, ProvisionSpecError> {
        let full = self.full_worker_runtime_config()?;
        let mut boot = self.boot_runtime_base();
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
            let value = full.get(key).cloned().ok_or_else(|| {
                ProvisionSpecError::RuntimeSemantics(format!(
                    "Worker boot materialization is missing {key}"
                ))
            })?;
            boot.insert(key.to_string(), value);
        }
        Ok(boot)
    }

    pub(crate) fn operator_app_key(&self) -> &str {
        self.runtime()
            .get("app_key")
            .and_then(Value::as_str)
            .expect("validated runtime.app_key is a string")
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
        mac.update(match self.schema_version {
            3 => REPORT_HMAC_DOMAIN_V3,
            4 => REPORT_HMAC_DOMAIN_V4,
            5 => REPORT_HMAC_DOMAIN_V5,
            _ => unreachable!("validated provision schema"),
        });
        mac.update(bytes);
        hex::encode(mac.finalize().into_bytes())
    }

    pub(crate) fn apply_authorization_binding_hmac_sha256(&self, bytes: &[u8]) -> String {
        let mut mac =
            <Hmac<Sha256> as KeyInit>::new_from_slice(self.lifecycle_audit_key.as_bytes())
                .expect("HMAC accepts keys of any length");
        mac.update(APPLY_AUTHORIZATION_HMAC_DOMAIN_V3);
        mac.update(bytes);
        hex::encode(mac.finalize().into_bytes())
    }

    pub(crate) fn verify_apply_authorization_binding_hmac_sha256(
        &self,
        bytes: &[u8],
        expected_hex: &str,
    ) -> bool {
        let Ok(expected) = hex::decode(expected_hex) else {
            return false;
        };
        let mut mac =
            <Hmac<Sha256> as KeyInit>::new_from_slice(self.lifecycle_audit_key.as_bytes())
                .expect("HMAC accepts keys of any length");
        mac.update(APPLY_AUTHORIZATION_HMAC_DOMAIN_V3);
        mac.update(bytes);
        mac.verify_slice(&expected).is_ok()
    }

    fn target_secret_values(&self) -> Result<Vec<String>, ProvisionSpecError> {
        let mut secrets = Vec::new();
        fn add_url_password(
            secrets: &mut Vec<String>,
            value: &str,
        ) -> Result<(), ProvisionSpecError> {
            if let Some(password) = Url::parse(value)
                .expect("validated datastore URL")
                .password()
            {
                secrets.push(strict_percent_decode(password)?);
            }
            Ok(())
        }
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
                    add_url_password(&mut secrets, value)?;
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
                    add_url_password(&mut secrets, value)?;
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
        if let Some(execution) = self.legacy_apply_execution() {
            add_url_password(
                &mut secrets,
                &execution.backup.isolated_restore_database_url,
            )?;
        }
        Ok(secrets)
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
            } else if DECIMAL_RUNTIME_KEYS.contains(key) {
                Value::String("1".to_string())
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
                "sample_interval_seconds": 1,
                "stale_after_seconds": 10,
                "capacity_evidence": "dedicated-postgresql-volume-quota-ticket-1042"
            },
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
                "database_url": "mysql://legacy_readonly:legacy-secret@127.0.0.1:3306/v2board",
                "database_fence_url": "mysql://legacy_fence:independent-fence-secret@127.0.0.1:3306",
                "redis_default_url": "redis://legacy_lifecycle:redis-lifecycle-secret@127.0.0.1:6379/0",
                "redis_cache_url": "redis://legacy_lifecycle:redis-lifecycle-secret@127.0.0.1:6379/1",
                "redis_connection_prefix": "v2board_database_",
                "redis_cache_prefix": "v2board_cache",
                "redis_horizon_prefix": "v2board_horizon:",
                "legacy_cache_driver": "redis",
                "transport_security": "trusted_maintenance_network"
            }),
        );
        document["decisions"] = serde_json::json!({
            "legacy_configuration": "manual_only",
            "sessions": "logout_all",
            "legacy_cache": "discard_ephemeral_after_fence",
            "legacy_stripe": "assert_none",
            "temporary_subscription_links": "invalidate_at_cutover",
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

    fn legacy_execution() -> Value {
        serde_json::json!({
            "release": {
                "release_id": "2026.07.12-content-abc123",
                "archive_sha256": "a".repeat(64)
            },
            "systemd": {
                "legacy_writer_units": ["php-fpm-v2board.service"],
                "legacy_worker_units": ["v2board-legacy-queue.service"],
                "legacy_scheduler_units": [
                    "v2board-legacy-scheduler.timer",
                    "v2board-legacy-scheduler.service"
                ]
            },
            "source_control": {
                "datastores": {
                    "mysql": {
                        "unit": "mysql-v2board.service"
                    },
                    "default_redis": {
                        "unit": "redis-v2board.service"
                    },
                    "cache_redis": {
                        "unit": "redis-v2board.service"
                    }
                }
            },
            "receipts": {
                "release_archive": {
                    "sha256": "b".repeat(64)
                }
            },
            "backup": {
                "backup_reference": "vault:legacy/backup-20260712",
                "encryption_recipient_sha256": "c".repeat(64),
                "decryption_identity_sha256": "e".repeat(64),
                "isolated_restore_database_url": "mysql://restore_admin:isolated-restore-secret@restore-db.company.net/v2board_restore?ssl-mode=VERIFY_IDENTITY",
                "isolated_restore_transport_security": "verified_tls",
                "command_timeout_seconds": 86400,
                "maximum_encrypted_backup_bytes": 274877906944_u64
            },
            "nodes": {
                "activation_transport": {
                    "kind": "not_required_no_nodes"
                },
                "inventory": []
            }
        })
    }

    fn legacy_v4_document() -> Value {
        let mut document = legacy_document();
        document["schema_version"] = Value::from(4);
        document
            .as_object_mut()
            .expect("document")
            .remove("attestations");
        document
            .as_object_mut()
            .expect("document")
            .insert("execution".to_string(), legacy_execution());
        remove_legacy_v4_derived_inputs(&mut document);
        document
    }

    fn legacy_v5_document() -> Value {
        let mut document = legacy_v4_document();
        document["schema_version"] = Value::from(5);
        document["decisions"] = serde_json::json!({
            "nodes": "discard_and_manual_rebuild",
            "legacy_traffic_details": "discard",
            "legacy_operational_logs": "discard",
            "legacy_custom_rules": "none"
        });
        document
    }

    fn remove_legacy_v4_derived_inputs(document: &mut Value) {
        document
            .as_object_mut()
            .expect("document")
            .remove("reference_commit");
        let custom_rules = document["decisions"]["legacy_custom_rules"].clone();
        document["decisions"] = serde_json::json!({
            "legacy_custom_rules": custom_rules
        });
        document["execution"]
            .as_object_mut()
            .expect("execution")
            .remove("nodes");
        document["source"]
            .as_object_mut()
            .expect("source")
            .remove("legacy_cache_driver");
        let target = document["target"].as_object_mut().expect("target");
        for key in [
            "api_runtime_config_path",
            "worker_runtime_config_path",
            "require_empty_redis",
        ] {
            target.remove(key);
        }
        let postgres = target["postgres"].as_object_mut().expect("PostgreSQL");
        for key in [
            "database_collation",
            "database_ctype",
            "require_database_absent",
            "require_roles_absent",
        ] {
            postgres.remove(key);
        }
        let external_access = postgres["external_access"]
            .as_object_mut()
            .expect("PostgreSQL external access");
        external_access.remove("pg_hba_managed_externally");
        external_access.remove("network_policy_managed_externally");
        let clickhouse = target["clickhouse"].as_object_mut().expect("ClickHouse");
        for key in [
            "require_database_absent",
            "require_principals_absent",
            "require_standalone_non_replicated",
        ] {
            clickhouse.remove(key);
        }
        let privileges = clickhouse["privileges"]
            .as_object_mut()
            .expect("ClickHouse privileges");
        for key in [
            "bootstrap_manages_database_and_principals",
            "schema_has_ddl_metadata_read_and_ledger_write_only",
            "writer_is_insert_and_verify_only",
            "reader_is_select_only",
        ] {
            privileges.remove(key);
        }
    }

    pub(crate) fn legacy_spec_for_orchestration() -> ProvisionSpec {
        load_document(&legacy_v4_document()).expect("valid shared legacy test manifest")
    }

    pub(crate) fn legacy_spec_for_orchestration_operation(operation_id: &str) -> ProvisionSpec {
        let mut document = legacy_v4_document();
        document["operation_id"] = Value::String(operation_id.to_string());
        load_document(&document).expect("valid operation-bound legacy test manifest")
    }

    pub(crate) fn legacy_v5_spec_for_orchestration_operation(operation_id: &str) -> ProvisionSpec {
        let mut document = legacy_v5_document();
        document["operation_id"] = Value::String(operation_id.to_string());
        load_document(&document).expect("valid operation-bound schema-v5 legacy test manifest")
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
    fn v4_legacy_execution_is_complete_strict_and_hmac_bound() {
        let document = legacy_v4_document();
        let spec = load_document(&document).expect("valid v4 legacy execution manifest");
        assert_eq!(spec.schema_version, 4);
        let execution = spec
            .legacy_apply_execution()
            .expect("v4 exposes apply intent");
        assert_eq!(execution.release.release_id, "2026.07.12-content-abc123");
        assert_eq!(
            execution.journal.root,
            Path::new("/var/lib/v2board/lifecycle/journal")
        );
        assert!(execution.nodes.inventory.is_empty());
        assert!(matches!(
            execution.nodes.activation_transport,
            LegacyNodeActivationTransportSpec::NotRequiredNoNodes
        ));
        assert!(execution.legacy_traffic_details.is_none());
        let ProvisionFlow::LegacyReferenceMigration {
            reference_commit,
            decisions,
            ..
        } = &spec.flow
        else {
            panic!("legacy flow");
        };
        assert_eq!(reference_commit, LEGACY_REFERENCE_COMMIT);
        assert!(decisions.legacy_configuration == LegacyConfigurationDecision::ManualOnly);
        assert!(decisions.sessions == SessionDecision::LogoutAll);
        assert!(decisions.legacy_cache == LegacyCacheDecision::DiscardEphemeralAfterFence);
        assert!(decisions.legacy_stripe == LegacyStripeDecision::AssertNone);
        assert!(
            decisions.temporary_subscription_links
                == TemporarySubscriptionLinkDecision::InvalidateAtCutover
        );
        assert!(decisions.nodes == NodeDecision::OneShotOfflineCutover);
        assert!(decisions.legacy_traffic_details.is_none());
        assert!(decisions.legacy_operational_logs.is_none());
        assert!(decisions.legacy_theme == LegacyThemeDecision::DiscardConfirmed);

        #[derive(Serialize)]
        struct HistoricalV4Decisions {
            legacy_configuration: LegacyConfigurationDecision,
            sessions: SessionDecision,
            legacy_cache: LegacyCacheDecision,
            legacy_stripe: LegacyStripeDecision,
            temporary_subscription_links: TemporarySubscriptionLinkDecision,
            nodes: NodeDecision,
            legacy_theme: LegacyThemeDecision,
            legacy_custom_rules: LegacyCustomRulesDecision,
        }
        #[derive(Serialize)]
        struct HistoricalV4HydratedFacts<'a> {
            binding_version: u32,
            reference_commit: &'a str,
            decisions: HistoricalV4Decisions,
            execution: &'a LegacyExecutionSpec,
        }
        let hydrated_fact_bytes = legacy_v4_hydrated_facts(&spec).expect("hydrated facts binding");
        let historical_fact_bytes = serde_json::to_vec(&HistoricalV4HydratedFacts {
            binding_version: 1,
            reference_commit,
            decisions: HistoricalV4Decisions {
                legacy_configuration: decisions.legacy_configuration,
                sessions: decisions.sessions,
                legacy_cache: decisions.legacy_cache,
                legacy_stripe: decisions.legacy_stripe,
                temporary_subscription_links: decisions.temporary_subscription_links,
                nodes: decisions.nodes,
                legacy_theme: decisions.legacy_theme,
                legacy_custom_rules: decisions.legacy_custom_rules,
            },
            execution,
        })
        .expect("historical v4 facts serialize");
        assert_eq!(
            hydrated_fact_bytes, historical_fact_bytes,
            "schema v4 hydrated fact bytes must remain historical-byte compatible"
        );
        let hydrated_facts: Value =
            serde_json::from_slice(&hydrated_fact_bytes).expect("hydrated facts JSON");
        assert_eq!(
            hydrated_facts["reference_commit"],
            Value::String(LEGACY_REFERENCE_COMMIT.to_string())
        );
        assert_eq!(
            hydrated_facts["execution"]["nodes"]["activation_transport"]["kind"],
            Value::String("not_required_no_nodes".to_string())
        );
        assert!(
            hydrated_facts["decisions"]
                .get("legacy_traffic_details")
                .is_none(),
            "schema v4 hydrated bytes must not gain a schema-v5 field"
        );
        assert!(
            hydrated_facts["decisions"]
                .get("legacy_operational_logs")
                .is_none(),
            "schema v4 hydrated bytes must not gain a schema-v5 field"
        );
        assert!(
            hydrated_facts["execution"]
                .get("legacy_traffic_details")
                .is_none(),
            "schema v4 execution bytes must not gain a schema-v5 field"
        );
        assert!(execution.receipts.postgres_verification_path.is_none());
        assert!(execution.receipts.clickhouse_projection_path.is_none());
        assert_eq!(spec.manifest_binding_hmac_sha256().len(), 64);
        let first = spec
            .legacy_execution_binding_hmac_sha256()
            .expect("scoped execution binding");
        assert_eq!(first.len(), 64);
        let receipt = spec
            .source_receipt_binding_hmac_sha256(
                LegacyRuntimeReceiptKind::SourceFence,
                br#"{"status":"fenced"}"#,
            )
            .expect("v4 legacy receipt binding");
        assert!(spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceFence,
            br#"{"status":"fenced"}"#,
            &receipt,
        ));
        assert!(!spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceDrain,
            br#"{"status":"fenced"}"#,
            &receipt,
        ));
        assert!(!spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceFence,
            br#"{"status":"changed"}"#,
            &receipt,
        ));
        let journal_bound_receipt = spec
            .source_receipt_binding_hmac_sha256(
                LegacyRuntimeReceiptKind::SourceFence,
                br#"{"journal_anchor_event_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","result_checkpoint":"maintenance_fenced"}"#,
            )
            .expect("journal-bound source receipt");
        assert!(!spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceFence,
            br#"{"journal_anchor_event_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","result_checkpoint":"maintenance_fenced"}"#,
            &journal_bound_receipt,
        ));
        assert!(
            spec.source_receipt_binding_hmac_sha256(
                LegacyRuntimeReceiptKind::PostgresVerificationReport,
                b"v5-only",
            )
            .is_none(),
            "schema v4 must not mint a schema-v5 report receipt"
        );

        let mut changed = document.clone();
        changed["execution"]["release"]["release_id"] =
            Value::String("2026.07.12-content-def456".to_string());
        let second = load_document(&changed)
            .expect("changed valid execution")
            .legacy_execution_binding_hmac_sha256()
            .expect("scoped execution binding");
        assert_ne!(first, second);

        let v3 = load_document(&legacy_document()).expect("v3 remains readable");
        assert!(v3.legacy_apply_execution().is_none());
        assert!(v3.legacy_execution_binding_hmac_sha256().is_none());
        assert!(
            v3.source_receipt_binding_hmac_sha256(
                LegacyRuntimeReceiptKind::SourceFence,
                b"receipt"
            )
            .is_none()
        );

        let mut method_two_reuse = legacy_v4_document();
        method_two_reuse["runtime"]["show_subscribe_method"] = Value::from(2);
        assert!(matches!(
            load_document(&method_two_reuse),
            Err(ProvisionSpecError::Validation(
                "legacy migration target runtime.show_subscribe_method must be 0 or 1 so old temporary URLs are invalid at cutover"
            ))
        ));

        let mut obsolete_source_hint = legacy_v4_document();
        obsolete_source_hint["source"]["legacy_show_subscribe_method"] = Value::from(0);
        assert!(load_document(&obsolete_source_hint).is_err());
    }

    #[test]
    fn v5_binds_explicit_discard_policies_without_operator_inventory() {
        let document = legacy_v5_document();
        let spec = load_document(&document).expect("valid v5 discard migration manifest");
        assert_eq!(spec.schema_version, 5);
        let execution = spec
            .legacy_apply_execution()
            .expect("v5 exposes apply intent");
        assert!(execution.nodes.inventory.is_empty());
        assert!(matches!(
            execution.nodes.activation_transport,
            LegacyNodeActivationTransportSpec::DiscardAndManualRebuild
        ));
        assert!(execution.legacy_traffic_details == Some(LegacyTrafficDetailsDecision::Discard));
        assert_eq!(
            execution
                .receipts
                .postgres_verification_path
                .as_deref()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str()),
            Some("postgres-verification-report.json")
        );
        assert_eq!(
            execution
                .receipts
                .clickhouse_projection_path
                .as_deref()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str()),
            Some("clickhouse-projection-report.json")
        );
        assert!(
            spec.source_receipt_binding_hmac_sha256(
                LegacyRuntimeReceiptKind::PostgresVerificationReport,
                b"report",
            )
            .is_some()
        );

        let ProvisionFlow::LegacyReferenceMigration { decisions, .. } = &spec.flow else {
            panic!("legacy flow");
        };
        assert!(decisions.nodes == NodeDecision::DiscardAndManualRebuild);
        assert!(decisions.legacy_traffic_details == Some(LegacyTrafficDetailsDecision::Discard));
        assert!(decisions.legacy_operational_logs == Some(LegacyOperationalLogsDecision::Discard));

        let hydrated_facts: Value = serde_json::from_slice(
            &legacy_v5_hydrated_facts(&spec).expect("v5 hydrated facts binding"),
        )
        .expect("hydrated facts JSON");
        assert_eq!(hydrated_facts["binding_version"], Value::from(3));
        assert_eq!(
            hydrated_facts["decisions"]["nodes"],
            Value::String("discard_and_manual_rebuild".to_string())
        );
        assert_eq!(
            hydrated_facts["decisions"]["legacy_traffic_details"],
            Value::String("discard".to_string())
        );
        assert_eq!(
            hydrated_facts["decisions"]["legacy_operational_logs"],
            Value::String("discard".to_string())
        );
        assert_eq!(
            hydrated_facts["execution"]["nodes"]["activation_transport"]["kind"],
            Value::String("discard_and_manual_rebuild".to_string())
        );
        assert_eq!(
            hydrated_facts["execution"]["legacy_traffic_details"],
            Value::String("discard".to_string())
        );

        let v4 = load_document(&legacy_v4_document()).expect("unchanged v4 manifest");
        assert_ne!(
            spec.manifest_binding_hmac_sha256(),
            v4.manifest_binding_hmac_sha256(),
            "schema-v5 manifest binding has an independent domain"
        );
        assert_ne!(
            spec.legacy_execution_binding_hmac_sha256(),
            v4.legacy_execution_binding_hmac_sha256(),
            "schema-v5 execution binding has an independent domain"
        );

        let mut operator_inventory = document.clone();
        operator_inventory["execution"]["nodes"] = serde_json::json!({
            "activation_transport": {"kind": "discard_and_manual_rebuild"},
            "inventory": [{"node_type": "v2ray", "node_id": 7, "credential_epoch": 1}]
        });
        assert_unknown_field(&operator_inventory, "nodes");

        for field in ["nodes", "legacy_traffic_details", "legacy_operational_logs"] {
            let mut missing_decision = document.clone();
            missing_decision["decisions"]
                .as_object_mut()
                .expect("decisions")
                .remove(field);
            assert!(
                load_document(&missing_decision).is_err(),
                "v5 accepted missing explicit decision {field}"
            );
        }

        let mut legacy_node_policy = document;
        legacy_node_policy["decisions"]["nodes"] =
            Value::String("one_shot_offline_cutover".to_string());
        assert!(matches!(
            load_document(&legacy_node_policy),
            Err(ProvisionSpecError::Validation(
                "legacy migration node, traffic-detail, and operational-log decisions do not match the selected schema version"
            ))
        ));
    }

    #[test]
    fn v4_rejects_unknown_fields_aliases_and_unbound_paths() {
        let mut restated_fixed_target = legacy_v4_document();
        restated_fixed_target["target"]["require_empty_redis"] = Value::Bool(true);
        assert!(matches!(
            load_document(&restated_fixed_target),
            Err(ProvisionSpecError::Validation(
                "schema v4 must omit fixed or derived target fields"
            ))
        ));

        let mut restated_source_policy = legacy_v4_document();
        restated_source_policy["source"]["legacy_cache_driver"] =
            Value::String("redis".to_string());
        assert!(matches!(
            load_document(&restated_source_policy),
            Err(ProvisionSpecError::Validation(
                "schema v4 must omit fixed or derived target fields"
            ))
        ));

        let mut unknown = legacy_v4_document();
        unknown["execution"]["release"]["future_default"] = Value::Bool(true);
        assert!(load_document(&unknown).is_err());

        let mut wrong_operation = legacy_v4_document();
        wrong_operation["execution"]["receipts"]["source_fence_path"] = Value::String(
            "/var/lib/v2board/lifecycle/operations/another-operation/source-fence.json".to_string(),
        );
        assert!(load_document(&wrong_operation).is_err());

        let mut restated_reference = legacy_v4_document();
        restated_reference["reference_commit"] = Value::String(LEGACY_REFERENCE_COMMIT.to_string());
        assert_unknown_field(&restated_reference, "reference_commit");

        for (field, value) in [
            ("legacy_configuration", "manual_only"),
            ("sessions", "logout_all"),
            ("legacy_cache", "discard_ephemeral_after_fence"),
            ("legacy_stripe", "assert_none"),
            ("temporary_subscription_links", "invalidate_at_cutover"),
            ("nodes", "one_shot_offline_cutover"),
            ("legacy_traffic_details", "discard"),
            ("legacy_operational_logs", "discard"),
            ("legacy_theme", "discard_confirmed"),
        ] {
            let mut restated_decision = legacy_v4_document();
            restated_decision["decisions"][field] = Value::String(value.to_string());
            assert_unknown_field(&restated_decision, field);
        }

        let mut restated_nodes = legacy_v4_document();
        restated_nodes["execution"]["nodes"] = serde_json::json!({
            "activation_transport": {"kind": "not_required_no_nodes"},
            "inventory": []
        });
        assert_unknown_field(&restated_nodes, "nodes");

        let mut duplicate_unit = legacy_v4_document();
        duplicate_unit["execution"]["systemd"]["legacy_worker_units"] =
            serde_json::json!(["php-fpm-v2board.service"]);
        assert!(load_document(&duplicate_unit).is_err());

        let mut guessed_horizon = legacy_v4_document();
        guessed_horizon["source"]["redis_horizon_prefix"] = Value::String(String::new());
        assert!(load_document(&guessed_horizon).is_err());

        let mut external_managed = legacy_v4_document();
        external_managed["execution"]["source_control"]["datastores"]["mysql"] =
            serde_json::json!({"management": "external_managed", "unit": null});
        assert!(load_document(&external_managed).is_err());

        let mut remote_local_unit = legacy_v4_document();
        remote_local_unit["source"]["database_url"] = Value::String(
            "mysql://legacy_readonly:legacy-secret@managed-db.invalid/v2board".to_string(),
        );
        assert!(load_document(&remote_local_unit).is_err());

        let mut split_one_redis_process = legacy_v4_document();
        split_one_redis_process["execution"]["source_control"]["datastores"]["cache_redis"]["unit"] =
            Value::String("another-redis.service".to_string());
        assert!(load_document(&split_one_redis_process).is_err());

        let mut future_outcome_input = legacy_v4_document();
        future_outcome_input["execution"]["source_control"]["provider_fence_receipts"] =
            serde_json::json!([]);
        assert!(load_document(&future_outcome_input).is_err());

        let mut co_located_decryption_key = legacy_v4_document();
        co_located_decryption_key["execution"]["backup"]["decryption_identity_path"] =
            Value::String(format!(
                "/var/lib/v2board/lifecycle/operations/{}/inputs/age-identity",
                co_located_decryption_key["operation_id"]
                    .as_str()
                    .expect("operation")
            ));
        assert!(load_document(&co_located_decryption_key).is_err());

        let mut unsafe_backup_bound = legacy_v4_document();
        unsafe_backup_bound["execution"]["backup"]["command_timeout_seconds"] = Value::from(299);
        assert!(load_document(&unsafe_backup_bound).is_err());

        let mut system_restore_database = legacy_v4_document();
        system_restore_database["execution"]["backup"]["isolated_restore_database_url"] =
            Value::String(
                "mysql://restore_admin:isolated-restore-secret@restore-db.company.net/mysql?ssl-mode=VERIFY_IDENTITY"
                    .into(),
            );
        assert!(load_document(&system_restore_database).is_err());
    }

    #[test]
    fn literal_ipv6_loopback_and_redis_identity_are_bracket_normalized() {
        assert!(url_uses_literal_loopback(
            "mysql://readonly:secret@[::1]:3306/v2board"
        ));
        assert_eq!(
            redis_service_identity("redis://[::1]:6379/0").expect("IPv6 Redis identity"),
            ("::1".to_string(), 6379)
        );
        assert_eq!(
            redis_service_identity("redis://LOCALHOST:6379/1").expect("hostname Redis identity"),
            ("localhost".to_string(), 6379)
        );
    }

    #[test]
    fn v4_hydrates_empty_node_policy_and_rejects_node_input() {
        let mut document = legacy_v4_document();
        let spec = load_document(&document).expect("derived no-node transport");
        let nodes = &spec.legacy_apply_execution().expect("execution").nodes;
        assert!(nodes.inventory.is_empty());
        assert!(matches!(
            nodes.activation_transport,
            LegacyNodeActivationTransportSpec::NotRequiredNoNodes
        ));

        document["execution"]["nodes"] = serde_json::json!({
            "activation_transport": {"kind": "not_required_no_nodes"},
            "inventory": []
        });
        assert!(load_document(&document).is_err());
    }

    #[test]
    fn v4_requires_exact_nonempty_redis_prefixes_while_v3_remains_compatible() {
        for field in ["redis_connection_prefix", "redis_cache_prefix"] {
            let mut v4 = legacy_v4_document();
            v4["source"][field] = Value::String(String::new());
            assert!(load_document(&v4).is_err(), "v4 accepted empty {field}");
        }

        let mut v3 = legacy_document();
        v3["source"]["redis_connection_prefix"] = Value::String(String::new());
        v3["source"]["redis_cache_prefix"] = Value::String(String::new());
        load_document(&v3).expect("v3 preserves historical empty-prefix compatibility");
    }

    #[test]
    fn v3_and_v4_shapes_cannot_be_reinterpreted() {
        let mut v3_with_execution = legacy_document();
        v3_with_execution["execution"] = legacy_execution();
        assert!(load_document(&v3_with_execution).is_err());

        let mut v4_with_attestation = legacy_v4_document();
        v4_with_attestation["attestations"] = serde_json::json!({
            "source_writers_stopped": true,
            "source_workers_stopped": true,
            "node_reporters_stopped": true,
            "legacy_queues_drained": true,
            "backup_reference": "pretend-backup",
            "restore_tested": true
        });
        assert!(load_document(&v4_with_attestation).is_err());

        let mut fresh_v4 = fresh_document();
        fresh_v4["schema_version"] = Value::from(4);
        assert!(matches!(
            load_document(&fresh_v4),
            Err(ProvisionSpecError::Validation(_))
        ));
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
            let operator = spec
                .normalized_operator_config_candidate()
                .expect("one exact typed operator candidate");
            assert_eq!(
                operator
                    .as_map()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                OPERATOR_CONFIG_KEYS_V1
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
            );
            assert_eq!(
                operator.as_map()["try_out_hour"],
                Value::String("1".to_string())
            );
            assert_eq!(
                operator.as_map()["commission_withdraw_limit"],
                Value::String("1".to_string())
            );
            let api = spec.materialized_api_runtime_config().expect("API config");
            assert_eq!(api["configuration_source"], "file_only");
            assert_eq!(api["configuration_scope"], "boot_only");
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
            for forbidden in OPERATOR_CONFIG_KEYS_V1 {
                assert!(
                    !api.contains_key(*forbidden),
                    "API leaked operator {forbidden}"
                );
            }
            AppConfig::try_from_api_boot_config_map(
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
            assert_eq!(worker["configuration_source"], "file_only");
            assert_eq!(worker["configuration_scope"], "boot_only");
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
            for forbidden in OPERATOR_CONFIG_KEYS_V1 {
                assert!(
                    !worker.contains_key(*forbidden),
                    "worker leaked operator {forbidden}"
                );
            }
            AppConfig::try_from_worker_boot_config_map(
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
    fn decimal_runtime_values_are_lossless_and_boolean_trial_flag_is_typed() {
        let exact = "1234567890123.123456789012345";
        let mut document = fresh_document();
        document["runtime"]["try_out_enable"] = Value::Bool(true);
        document["runtime"]["try_out_hour"] = Value::String(exact.to_string());
        document["runtime"]["commission_withdraw_limit"] = Value::from(100);
        let spec = load_document(&document).expect("lossless decimal strings are valid");
        let operator = spec
            .normalized_operator_config_candidate()
            .expect("typed exact operator candidate");
        assert_eq!(operator.as_map()["try_out_enable"], Value::Bool(true));
        assert_eq!(
            operator.as_map()["try_out_hour"],
            Value::String(exact.to_string())
        );
        assert_eq!(
            operator.as_map()["commission_withdraw_limit"],
            Value::String("100".to_string())
        );

        let mut floating = fresh_document();
        floating["runtime"]["try_out_hour"] = serde_json::json!(1.5);
        assert!(matches!(
            load_document(&floating),
            Err(ProvisionSpecError::RuntimeType(key)) if key == "try_out_hour"
        ));

        let mut integer_boolean = fresh_document();
        integer_boolean["runtime"]["try_out_enable"] = Value::from(1);
        assert!(matches!(
            load_document(&integer_boolean),
            Err(ProvisionSpecError::RuntimeType(key)) if key == "try_out_enable"
        ));
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
        assert_eq!(plan.report_version, 5);
        assert_eq!(plan.report_sha256.len(), 64);
        assert_eq!(plan.report_binding_hmac_sha256.len(), 64);
        assert!(plan.converter_available);
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

    #[test]
    fn legacy_scheduler_inventory_requires_exact_timer_service_pairs() {
        assert!(scheduler_units_are_exact_pairs(&[
            "legacy-scheduler.timer".to_string(),
            "legacy-scheduler.service".to_string(),
        ]));
        assert!(!scheduler_units_are_exact_pairs(&[
            "legacy-scheduler.timer".to_string(),
            "different-scheduler.service".to_string(),
        ]));
        assert!(!scheduler_units_are_exact_pairs(&[
            "legacy-scheduler.timer".to_string(),
        ]));
    }

    fn load_document(document: &Value) -> Result<ProvisionSpec, ProvisionSpecError> {
        load_bytes(&serde_json::to_vec(document).expect("JSON"))
    }

    fn assert_unknown_field(document: &Value, field: &str) {
        let error = match load_document(document) {
            Ok(_) => panic!("v4 accepted restated fixed field {field}"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(
            message.contains("unknown field"),
            "unexpected error: {message}"
        );
        assert!(message.contains(field), "unexpected error: {message}");
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
