use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::apply_journal::DurableTargetMutationPermit;
use crate::inspect::ProvisionPlan;
use crate::manifest::{
    ClickHouseTargetSpec, PostgresTargetSpec, ProvisionFlow, ProvisionKind, ProvisionSpec,
    ProvisionSpecError, SourceSpec, TargetSpec,
};

const API_CONFIG_PATH: &str = "/var/lib/v2board/api/config.json";
const WORKER_CONFIG_PATH: &str = "/var/lib/v2board/worker/config.json";
const RELEASES_ROOT: &str = "/opt/v2board/releases";
#[cfg(test)]
const CURRENT_RELEASE_PATH: &str = "/opt/v2board/current";
#[cfg(test)]
const API_UNIT: &str = "v2board-api.service";
#[cfg(test)]
const WORKER_UNIT: &str = "v2board-worker.service";
const API_OWNER: &str = "v2board-api";
const WORKER_OWNER: &str = "v2board-worker";
const OWNER_ONLY_MODE: u32 = 0o600;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryBoundary {
    BeforeNativeWriterStart,
    AfterNativeWriterStartForwardRecoveryOnly,
}

#[derive(Debug, thiserror::Error)]
pub enum TargetActivationError {
    #[error("target activation only supports fresh_install and legacy_reference_migration")]
    UnsupportedProvisionKind,
    #[error("target activation token is not bound to this operation and manifest")]
    BindingMismatch,
    #[error("invalid release artifact: {0}")]
    InvalidRelease(&'static str),
    #[error("invalid {stage} proof: {reason}")]
    InvalidProof {
        stage: &'static str,
        reason: &'static str,
    },
    #[error("cannot materialize role runtime config: {0}")]
    RuntimeConfig(#[from] ProvisionSpecError),
    #[error("cannot serialize role runtime config: {0}")]
    RuntimeConfigSerialization(#[from] serde_json::Error),
    #[error("external target activation stage {stage} failed with sanitized code {code}")]
    External { stage: &'static str, code: String },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("external executor failure: {code}")]
pub struct ExecutorError {
    code: String,
}

impl ExecutorError {
    pub fn sanitized(code: impl AsRef<str>) -> Self {
        let code = code.as_ref();
        let valid = !code.is_empty()
            && code.len() <= 128
            && code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
        Self {
            code: if valid {
                code.to_string()
            } else {
                "invalid_or_unsanitized_executor_error".to_string()
            },
        }
    }

    fn into_activation(self, stage: &'static str) -> TargetActivationError {
        TargetActivationError::External {
            stage,
            code: self.code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PostgresEmptyProof {
    pub server_major: u16,
    pub fsync_on: bool,
    pub full_page_writes_on: bool,
    pub synchronous_commit_on: bool,
    pub data_checksums_on: bool,
    pub tls_session_verified: bool,
    pub wal_level_replica_or_logical: bool,
    pub archive_mode_on_or_always: bool,
    pub archive_command_or_library_enabled: bool,
    pub database_absent: bool,
    pub migration_role_absent: bool,
    pub api_role_absent: bool,
    pub worker_role_absent: bool,
    pub recoverable_objects_owned_by_operation: bool,
    pub bootstrap_capability_verified: bool,
    pub external_access_evidence_verified: bool,
}

impl PostgresEmptyProof {
    fn validate(&self) -> Result<(), TargetActivationError> {
        if self.server_major != 18 {
            return invalid_proof("postgres_empty", "PostgreSQL major must be exactly 18");
        }
        if !self.fsync_on
            || !self.full_page_writes_on
            || !self.synchronous_commit_on
            || !self.data_checksums_on
            || !self.tls_session_verified
            || !self.wal_level_replica_or_logical
            || !self.archive_mode_on_or_always
            || !self.archive_command_or_library_enabled
        {
            return invalid_proof(
                "postgres_empty",
                "PostgreSQL durability or server-observed TLS settings are unsafe",
            );
        }
        let entirely_absent = self.database_absent
            && self.migration_role_absent
            && self.api_role_absent
            && self.worker_role_absent;
        if !entirely_absent && !self.recoverable_objects_owned_by_operation {
            return invalid_proof(
                "postgres_empty",
                "database and all three target roles must be absent",
            );
        }
        if !self.bootstrap_capability_verified || !self.external_access_evidence_verified {
            return invalid_proof(
                "postgres_empty",
                "bootstrap capability and external access evidence must be verified",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostgresRuntimeAclSchemaState {
    Empty,
    FrozenBaseline,
    Drifted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PostgresReadyProof {
    pub server_major: u16,
    pub fsync_on: bool,
    pub full_page_writes_on: bool,
    pub synchronous_commit_on: bool,
    pub data_checksums_on: bool,
    pub tls_session_verified: bool,
    pub wal_level_replica_or_logical: bool,
    pub archive_mode_on_or_always: bool,
    pub archive_command_or_library_enabled: bool,
    pub database_owned_by_operation: bool,
    pub roles_owned_by_operation: bool,
    pub collation: String,
    pub ctype: String,
    pub principals_distinct: bool,
    pub migration_role_is_ddl_only: bool,
    pub api_role_is_api_dml_only: bool,
    pub worker_role_is_worker_dml_only: bool,
    pub runtime_acl_schema_state: PostgresRuntimeAclSchemaState,
    pub runtime_table_acl_exact: bool,
    pub protected_table_acl_exact: bool,
    pub runtime_sequence_acl_minimal: bool,
    pub runtime_default_acl_fail_closed: bool,
    pub runtime_boundary_acl_exact: bool,
    pub bootstrap_role_absent_from_runtime: bool,
}

impl PostgresReadyProof {
    fn validate(&self) -> Result<(), TargetActivationError> {
        if self.server_major != 18 || self.collation != "C.UTF-8" || self.ctype != "C.UTF-8" {
            return invalid_proof(
                "postgres_ready",
                "PostgreSQL version, collation, or ctype does not match the frozen target",
            );
        }
        if !self.fsync_on
            || !self.full_page_writes_on
            || !self.synchronous_commit_on
            || !self.data_checksums_on
            || !self.tls_session_verified
            || !self.wal_level_replica_or_logical
            || !self.archive_mode_on_or_always
            || !self.archive_command_or_library_enabled
        {
            return invalid_proof(
                "postgres_ready",
                "PostgreSQL durability or server-observed TLS settings drifted",
            );
        }
        if !self.database_owned_by_operation
            || !self.roles_owned_by_operation
            || !self.principals_distinct
            || !self.migration_role_is_ddl_only
            || !self.api_role_is_api_dml_only
            || !self.worker_role_is_worker_dml_only
            || self.runtime_acl_schema_state == PostgresRuntimeAclSchemaState::Drifted
            || !self.runtime_table_acl_exact
            || !self.protected_table_acl_exact
            || !self.runtime_sequence_acl_minimal
            || !self.runtime_default_acl_fail_closed
            || !self.runtime_boundary_acl_exact
            || !self.bootstrap_role_absent_from_runtime
        {
            return invalid_proof(
                "postgres_ready",
                "PostgreSQL ownership or least-privilege verification failed",
            );
        }
        Ok(())
    }

    #[cfg(test)]
    fn validate_for_activation(&self) -> Result<(), TargetActivationError> {
        self.validate()?;
        if self.runtime_acl_schema_state != PostgresRuntimeAclSchemaState::FrozenBaseline {
            return invalid_proof(
                "postgres_ready",
                "PostgreSQL runtime ACLs must cover the frozen migration lineage before activation",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClickHouseEmptyProof {
    pub server_major: u16,
    pub server_minor: u16,
    pub database_absent: bool,
    pub schema_principal_absent: bool,
    pub writer_principal_absent: bool,
    pub reader_principal_absent: bool,
    pub recoverable_objects_owned_by_operation: bool,
    pub bootstrap_capability_verified: bool,
    pub standalone_non_replicated: bool,
    pub network_policy_evidence_verified: bool,
}

impl ClickHouseEmptyProof {
    fn validate(&self) -> Result<(), TargetActivationError> {
        if (self.server_major, self.server_minor) != (26, 3) {
            return invalid_proof(
                "clickhouse_empty",
                "ClickHouse release family must be exactly 26.3",
            );
        }
        let entirely_absent = self.database_absent
            && self.schema_principal_absent
            && self.writer_principal_absent
            && self.reader_principal_absent;
        if !entirely_absent && !self.recoverable_objects_owned_by_operation {
            return invalid_proof(
                "clickhouse_empty",
                "database and all three target principals must be absent",
            );
        }
        if !self.bootstrap_capability_verified
            || !self.standalone_non_replicated
            || !self.network_policy_evidence_verified
        {
            return invalid_proof(
                "clickhouse_empty",
                "ClickHouse topology, bootstrap capability, or network evidence failed",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClickHouseReadyProof {
    pub server_major: u16,
    pub server_minor: u16,
    pub database_owned_by_operation: bool,
    pub principals_owned_by_operation: bool,
    pub standalone_non_replicated: bool,
    pub schema_is_ddl_metadata_and_ledger_only: bool,
    pub writer_is_insert_and_verify_only: bool,
    pub reader_is_select_only: bool,
    pub bootstrap_principal_absent_from_runtime: bool,
}

impl ClickHouseReadyProof {
    fn validate(&self) -> Result<(), TargetActivationError> {
        if (self.server_major, self.server_minor) != (26, 3) {
            return invalid_proof(
                "clickhouse_ready",
                "ClickHouse release family must be exactly 26.3",
            );
        }
        if !self.database_owned_by_operation
            || !self.principals_owned_by_operation
            || !self.standalone_non_replicated
            || !self.schema_is_ddl_metadata_and_ledger_only
            || !self.writer_is_insert_and_verify_only
            || !self.reader_is_select_only
            || !self.bootstrap_principal_absent_from_runtime
        {
            return invalid_proof(
                "clickhouse_ready",
                "ClickHouse ownership or least-privilege verification failed",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RedisEmptyProof {
    pub key_count: u64,
    pub namespace_entries: u64,
    pub target_run_id: String,
    pub inspect_review_sha256: String,
    pub source_identity_distinct: bool,
    pub tls_identity_verified: bool,
    pub required_commands_available: bool,
}

impl RedisEmptyProof {
    fn validate(
        &self,
        binding: &TargetRedisInspectionBinding,
    ) -> Result<(), TargetActivationError> {
        if self.key_count != 0 || self.namespace_entries != 0 {
            return invalid_proof(
                "redis_empty",
                "target Redis must already be empty; FLUSHDB is forbidden",
            );
        }
        if self.target_run_id != binding.target_run_id
            || self.inspect_review_sha256 != binding.inspect_review_sha256
            || binding
                .source_run_ids
                .iter()
                .any(|source| source == &self.target_run_id)
            || !self.source_identity_distinct
            || !self.tls_identity_verified
            || !self.required_commands_available
        {
            return invalid_proof(
                "redis_empty",
                "Redis identity, TLS, or command capability verification failed",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TargetRedisInspectionBinding {
    inspect_review_sha256: String,
    target_run_id: String,
    source_run_ids: Vec<String>,
}

impl TargetRedisInspectionBinding {
    pub fn from_inspection(
        spec: &ProvisionSpec,
        inspection: &ProvisionPlan,
        permit: &DurableTargetMutationPermit,
    ) -> Result<Self, TargetActivationError> {
        if inspection.operation_id != spec.operation_id
            || inspection.operation_id != permit.operation_id()
            || inspection.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
            || inspection.review_binding_sha256 != permit.inspect_review_sha256()
        {
            return Err(TargetActivationError::BindingMismatch);
        }
        let target =
            inspection
                .target_redis
                .as_ref()
                .ok_or(TargetActivationError::InvalidProof {
                    stage: "redis_inspection_binding",
                    reason: "signed inspection has no target Redis identity",
                })?;
        if !valid_redis_run_id(&target.target_run_id) {
            return invalid_proof(
                "redis_inspection_binding",
                "signed target Redis run_id is invalid",
            );
        }
        let source_run_ids = match spec.kind {
            ProvisionKind::LegacyReferenceMigration => {
                let source = inspection.source_redis.as_ref().ok_or(
                    TargetActivationError::InvalidProof {
                        stage: "redis_inspection_binding",
                        reason: "legacy inspection has no source Redis identities",
                    },
                )?;
                let values = vec![
                    source.source_default_run_id.clone(),
                    source.source_cache_run_id.clone(),
                ];
                if values.iter().any(|value| !valid_redis_run_id(value))
                    || values.iter().any(|value| value == &target.target_run_id)
                {
                    return invalid_proof(
                        "redis_inspection_binding",
                        "source and target Redis run_id values are invalid or not isolated",
                    );
                }
                values
            }
            ProvisionKind::FreshInstall => Vec::new(),
            ProvisionKind::NativeUpgrade => {
                return Err(TargetActivationError::UnsupportedProvisionKind);
            }
        };
        Ok(Self {
            inspect_review_sha256: inspection.review_binding_sha256.clone(),
            target_run_id: target.target_run_id.clone(),
            source_run_ids,
        })
    }

    /// Reconstructs the Redis identity portion of an already-authorized
    /// inspection during forward recovery. The reviewed report itself is
    /// bound by the authorization and filesystem journal; this constructor
    /// only accepts freshly observed run ids and never relaxes source/target
    /// isolation.
    pub(crate) fn from_recovery_observation(
        inspect_review_sha256: &str,
        target_run_id: &str,
        source_run_ids: Vec<String>,
    ) -> Result<Self, TargetActivationError> {
        if !is_lower_hex_sha256(inspect_review_sha256)
            || !valid_redis_run_id(target_run_id)
            || source_run_ids.len() != 2
            || source_run_ids
                .iter()
                .any(|value| !valid_redis_run_id(value) || value == target_run_id)
        {
            return invalid_proof(
                "redis_inspection_recovery_binding",
                "fresh Redis identities do not match the strict source/target shape",
            );
        }
        Ok(Self {
            inspect_review_sha256: inspect_review_sha256.to_string(),
            target_run_id: target_run_id.to_string(),
            source_run_ids,
        })
    }

    pub fn inspect_review_sha256(&self) -> &str {
        &self.inspect_review_sha256
    }

    pub fn target_run_id(&self) -> &str {
        &self.target_run_id
    }

    pub fn source_run_ids(&self) -> &[String] {
        &self.source_run_ids
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TargetCreationReceipt {
    pub operation_id: String,
    pub installation_id: String,
    pub permit_generation: u64,
    pub permit_event_sha256: String,
    pub inspect_review_sha256: String,
    pub backup_restore_proof_sha256: String,
    pub backup_reference_sha256: String,
    pub final_recheck_report_sha256: String,
    pub source_fingerprint_sha256: String,
    pub objects_created_for_this_operation: bool,
    pub broad_if_not_exists_not_used: bool,
}

impl TargetCreationReceipt {
    fn validate(
        &self,
        permit: &DurableTargetMutationPermit,
        stage: &'static str,
    ) -> Result<(), TargetActivationError> {
        if self.operation_id != permit.operation_id()
            || self.installation_id != permit.installation_id()
            || self.permit_generation != permit.generation()
            || self.permit_event_sha256 != permit.event_sha256()
            || self.inspect_review_sha256 != permit.inspect_review_sha256()
            || self.backup_restore_proof_sha256 != permit.backup_restore_proof_sha256()
            || self.backup_reference_sha256 != permit.backup_reference_sha256()
            || self.final_recheck_report_sha256 != permit.final_recheck_report_sha256()
            || self.source_fingerprint_sha256 != permit.source_fingerprint_sha256()
            || !self.objects_created_for_this_operation
            || !self.broad_if_not_exists_not_used
        {
            return invalid_proof(
                stage,
                "target creation was not journaled and bound exclusively to this operation",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseArtifactSpec {
    release_id: String,
    staged_path: PathBuf,
    external_archive_sha256: String,
}

impl ReleaseArtifactSpec {
    pub fn new(
        release_id: impl Into<String>,
        external_archive_sha256: impl Into<String>,
    ) -> Result<Self, TargetActivationError> {
        let release_id = release_id.into();
        if !valid_release_id(&release_id) {
            return Err(TargetActivationError::InvalidRelease(
                "release_id must be 1-128 safe ASCII characters without dot segments",
            ));
        }
        let external_archive_sha256 = external_archive_sha256.into();
        if !is_lower_hex_sha256(&external_archive_sha256) {
            return Err(TargetActivationError::InvalidRelease(
                "external archive SHA-256 must be 64 lowercase hexadecimal characters",
            ));
        }
        let staged_path = Path::new(RELEASES_ROOT).join(&release_id);
        Ok(Self {
            release_id,
            staged_path,
            external_archive_sha256,
        })
    }

    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    pub fn staged_path(&self) -> &Path {
        &self.staged_path
    }

    pub fn external_archive_sha256(&self) -> &str {
        &self.external_archive_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleasePreflightProof {
    pub release_id: String,
    pub canonical_staged_path: PathBuf,
    pub external_archive_sha256: String,
    pub internal_sha256sums_valid: bool,
    pub exact_long_lived_binary_set: bool,
    pub validated_frontend_tree_present: bool,
    pub release_metadata_valid: bool,
    pub root_owned_and_runtime_read_only: bool,
    pub systemd_analyze_verify_passed: bool,
    pub unit_exec_paths_use_current_symlink: bool,
    pub api_unit_uses_dedicated_identity_and_config: bool,
    pub worker_unit_uses_dedicated_identity_and_config: bool,
    pub api_bind_is_loopback_only: bool,
}

impl ReleasePreflightProof {
    #[cfg(test)]
    fn validate(&self, release: &ReleaseArtifactSpec) -> Result<(), TargetActivationError> {
        if self.release_id != release.release_id
            || self.canonical_staged_path != release.staged_path
            || self.external_archive_sha256 != release.external_archive_sha256
        {
            return invalid_proof(
                "release_preflight",
                "release proof does not bind the exact staged path and external digest",
            );
        }
        if !self.internal_sha256sums_valid
            || !self.exact_long_lived_binary_set
            || !self.validated_frontend_tree_present
            || !self.release_metadata_valid
            || !self.root_owned_and_runtime_read_only
            || !self.systemd_analyze_verify_passed
            || !self.unit_exec_paths_use_current_symlink
            || !self.api_unit_uses_dedicated_identity_and_config
            || !self.worker_unit_uses_dedicated_identity_and_config
            || !self.api_bind_is_loopback_only
        {
            return invalid_proof(
                "release_preflight",
                "checksum, ownership, frontend, or systemd preflight failed",
            );
        }
        Ok(())
    }
}

pub struct RoleConfigBundle {
    operation_id: String,
    api_path: PathBuf,
    worker_path: PathBuf,
    api_bytes: Vec<u8>,
    worker_bytes: Vec<u8>,
    api_binding_hmac_sha256: String,
    worker_binding_hmac_sha256: String,
}

impl RoleConfigBundle {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn api_path(&self) -> &Path {
        &self.api_path
    }

    pub fn worker_path(&self) -> &Path {
        &self.worker_path
    }

    pub fn api_bytes(&self) -> &[u8] {
        &self.api_bytes
    }

    pub fn worker_bytes(&self) -> &[u8] {
        &self.worker_bytes
    }

    pub fn api_binding_hmac_sha256(&self) -> &str {
        &self.api_binding_hmac_sha256
    }

    pub fn worker_binding_hmac_sha256(&self) -> &str {
        &self.worker_binding_hmac_sha256
    }

    pub fn api_owner(&self) -> &str {
        API_OWNER
    }

    pub fn worker_owner(&self) -> &str {
        WORKER_OWNER
    }

    pub fn mode(&self) -> u32 {
        OWNER_ONLY_MODE
    }
}

impl Drop for RoleConfigBundle {
    fn drop(&mut self) {
        self.api_bytes.fill(0);
        self.worker_bytes.fill(0);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigInstallReceipt {
    pub operation_id: String,
    pub api_path: PathBuf,
    pub worker_path: PathBuf,
    pub api_owner: String,
    pub worker_owner: String,
    pub api_mode: u32,
    pub worker_mode: u32,
    pub regular_files_without_symlinks: bool,
    pub temp_files_fsynced: bool,
    pub atomic_renames_complete: bool,
    pub parent_directories_fsynced: bool,
    pub rollback_handle: String,
}

impl ConfigInstallReceipt {
    #[cfg(test)]
    fn validate(&self, operation_id: &str) -> Result<(), TargetActivationError> {
        if self.operation_id != operation_id
            || self.api_path != Path::new(API_CONFIG_PATH)
            || self.worker_path != Path::new(WORKER_CONFIG_PATH)
            || self.api_owner != API_OWNER
            || self.worker_owner != WORKER_OWNER
            || self.api_mode != OWNER_ONLY_MODE
            || self.worker_mode != OWNER_ONLY_MODE
            || !self.regular_files_without_symlinks
            || !self.temp_files_fsynced
            || !self.atomic_renames_complete
            || !self.parent_directories_fsynced
            || !valid_opaque_handle(&self.rollback_handle)
        {
            return invalid_proof(
                "config_install",
                "role configs were not installed with the frozen ownership, fsync, and atomic rename contract",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigVerificationProof {
    pub api_binding_hmac_sha256: String,
    pub worker_binding_hmac_sha256: String,
    pub api_role_load_verified: bool,
    pub worker_role_load_verified: bool,
    pub cross_role_secret_absence_verified: bool,
    pub owner_mode_and_path_reverified: bool,
}

impl ConfigVerificationProof {
    #[cfg(test)]
    fn validate(&self, bundle: &RoleConfigBundle) -> Result<(), TargetActivationError> {
        if self.api_binding_hmac_sha256 != bundle.api_binding_hmac_sha256
            || self.worker_binding_hmac_sha256 != bundle.worker_binding_hmac_sha256
            || !self.api_role_load_verified
            || !self.worker_role_load_verified
            || !self.cross_role_secret_absence_verified
            || !self.owner_mode_and_path_reverified
        {
            return invalid_proof(
                "config_verify",
                "role config content, semantics, isolation, ownership, or mode verification failed",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfflineMigrationVerification {
    pub operation_id: String,
    pub manifest_binding_hmac_sha256: String,
    pub installation_id: String,
    pub inspect_review_sha256: String,
    pub backup_restore_report_sha256: String,
    pub backup_reference_sha256: String,
    pub final_recheck_report_sha256: String,
    pub source_fingerprint_sha256: String,
    pub bootstrap_permit_generation: u64,
    pub bootstrap_permit_event_sha256: String,
    pub target_permit_generation: u64,
    pub target_permit_event_sha256: String,
    pub data_verification_report_sha256: String,
    pub analytics_projection_report_sha256: String,
    pub node_cutover_report_sha256: Option<String>,
    pub old_writers_fenced: bool,
    pub new_writers_still_stopped: bool,
    pub postgres_is_transaction_authority: bool,
    pub clickhouse_is_rebuildable_projection: bool,
}

impl OfflineMigrationVerification {
    #[cfg(test)]
    fn validate(&self, spec: &ProvisionSpec) -> Result<(), TargetActivationError> {
        let node_proof_valid = match spec.kind {
            ProvisionKind::LegacyReferenceMigration => self
                .node_cutover_report_sha256
                .as_deref()
                .is_some_and(is_lower_hex_sha256),
            ProvisionKind::FreshInstall => self.node_cutover_report_sha256.is_none(),
            ProvisionKind::NativeUpgrade => false,
        };
        if self.operation_id != spec.operation_id
            || self.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
            || !valid_non_nil_uuid(&self.installation_id)
            || !is_lower_hex_sha256(&self.inspect_review_sha256)
            || !is_lower_hex_sha256(&self.backup_restore_report_sha256)
            || !is_lower_hex_sha256(&self.backup_reference_sha256)
            || !is_lower_hex_sha256(&self.final_recheck_report_sha256)
            || !is_lower_hex_sha256(&self.source_fingerprint_sha256)
            || self.bootstrap_permit_generation == 0
            || !is_lower_hex_sha256(&self.bootstrap_permit_event_sha256)
            || self.target_permit_generation == 0
            || !is_lower_hex_sha256(&self.target_permit_event_sha256)
            || !is_lower_hex_sha256(&self.data_verification_report_sha256)
            || !is_lower_hex_sha256(&self.analytics_projection_report_sha256)
            || !node_proof_valid
            || !self.old_writers_fenced
            || !self.new_writers_still_stopped
            || !self.postgres_is_transaction_authority
            || !self.clickhouse_is_rebuildable_projection
        {
            return invalid_proof(
                "offline_migration",
                "offline migration proof is incomplete or not bound to this operation",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActivationCommitGateProof {
    pub operation_id: String,
    pub installation_id: String,
    pub inspect_review_sha256: String,
    pub backup_restore_report_sha256: String,
    pub backup_reference_sha256: String,
    pub final_recheck_report_sha256: String,
    pub source_fingerprint_sha256: String,
    pub target_permit_generation: u64,
    pub target_permit_event_sha256: String,
    pub durable_journal_in_verifying_state: bool,
    pub postgres_ledger_exactly_current: bool,
    pub clickhouse_ledger_exactly_current: bool,
    pub data_reports_validated_for_commit: bool,
    pub backup_report_bound_in_ledger: bool,
    pub new_writers_still_stopped: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeActivationCommitReceipt {
    pub operation_id: String,
    pub installation_id: String,
    pub target_permit_generation: u64,
    pub target_permit_event_sha256: String,
    pub backup_reference_sha256: String,
    pub source_fingerprint_sha256: String,
    pub lifecycle_ledger_cas_committed: bool,
    pub postgres_installation_active: bool,
    pub postgres_is_only_transaction_authority: bool,
    pub old_writer_fence_still_verified: bool,
}

impl NativeActivationCommitReceipt {
    #[cfg(test)]
    fn validate(
        &self,
        verification: &OfflineMigrationVerification,
    ) -> Result<(), TargetActivationError> {
        if self.operation_id != verification.operation_id
            || self.installation_id != verification.installation_id
            || self.target_permit_generation != verification.target_permit_generation
            || self.target_permit_event_sha256 != verification.target_permit_event_sha256
            || self.backup_reference_sha256 != verification.backup_reference_sha256
            || self.source_fingerprint_sha256 != verification.source_fingerprint_sha256
            || !self.lifecycle_ledger_cas_committed
            || !self.postgres_installation_active
            || !self.postgres_is_only_transaction_authority
            || !self.old_writer_fence_still_verified
        {
            return invalid_proof(
                "native_activation_commit",
                "PostgreSQL authority activation was not CAS-committed with the fenced journal binding",
            );
        }
        Ok(())
    }
}

impl ActivationCommitGateProof {
    #[cfg(test)]
    fn validate(
        &self,
        verification: &OfflineMigrationVerification,
    ) -> Result<(), TargetActivationError> {
        if self.operation_id != verification.operation_id
            || self.installation_id != verification.installation_id
            || self.inspect_review_sha256 != verification.inspect_review_sha256
            || self.backup_restore_report_sha256 != verification.backup_restore_report_sha256
            || self.backup_reference_sha256 != verification.backup_reference_sha256
            || self.final_recheck_report_sha256 != verification.final_recheck_report_sha256
            || self.source_fingerprint_sha256 != verification.source_fingerprint_sha256
            || self.target_permit_generation != verification.target_permit_generation
            || self.target_permit_event_sha256 != verification.target_permit_event_sha256
            || !self.durable_journal_in_verifying_state
            || !self.postgres_ledger_exactly_current
            || !self.clickhouse_ledger_exactly_current
            || !self.data_reports_validated_for_commit
            || !self.backup_report_bound_in_ledger
            || !self.new_writers_still_stopped
        {
            return invalid_proof(
                "activation_commit_gate",
                "journal, ledger, report binding, or writer fence is not ready for the single commit",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseSwitchReceipt {
    pub operation_id: String,
    pub current_path: PathBuf,
    pub staged_path: PathBuf,
    pub previous_target: Option<PathBuf>,
    pub atomic_symlink_rename_complete: bool,
    pub parent_directory_fsynced: bool,
    pub rollback_handle: String,
}

impl ReleaseSwitchReceipt {
    #[cfg(test)]
    fn validate(
        &self,
        operation_id: &str,
        release: &ReleaseArtifactSpec,
    ) -> Result<(), TargetActivationError> {
        if self.operation_id != operation_id
            || self.current_path != Path::new(CURRENT_RELEASE_PATH)
            || self.staged_path != release.staged_path
            || !self.atomic_symlink_rename_complete
            || !self.parent_directory_fsynced
            || !valid_opaque_handle(&self.rollback_handle)
            || self
                .previous_target
                .as_ref()
                .is_some_and(|path| !path.starts_with(RELEASES_ROOT))
        {
            return invalid_proof(
                "release_switch",
                "current release was not switched atomically to the verified staged path",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UnitStartReceipt {
    pub operation_id: String,
    pub unit: String,
    pub invocation_count_for_operation: u32,
    pub start_request_accepted: bool,
}

impl UnitStartReceipt {
    #[cfg(test)]
    fn validate(
        &self,
        operation_id: &str,
        expected_unit: &'static str,
    ) -> Result<(), TargetActivationError> {
        if self.operation_id != operation_id
            || self.unit != expected_unit
            || self.invocation_count_for_operation != 1
            || !self.start_request_accepted
        {
            return invalid_proof(
                "unit_start",
                "native unit was not started exactly once for this operation",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ServiceReadinessProof {
    pub operation_id: String,
    pub unit: String,
    pub installation_id: String,
    pub release_id: String,
    pub postgres_ledger_exactly_current: bool,
    pub runtime_role_and_config_verified: bool,
    pub ready: bool,
    pub systemd_notify_ready: Option<bool>,
    pub watchdog_healthy: Option<bool>,
}

impl ServiceReadinessProof {
    #[cfg(test)]
    fn validate(
        &self,
        operation_id: &str,
        installation_id: &str,
        release_id: &str,
        unit: &'static str,
    ) -> Result<(), TargetActivationError> {
        let unit_specific_ready = if unit == API_UNIT {
            self.systemd_notify_ready.is_none() && self.watchdog_healthy.is_none()
        } else {
            self.systemd_notify_ready == Some(true) && self.watchdog_healthy == Some(true)
        };
        if self.operation_id != operation_id
            || self.unit != unit
            || self.installation_id != installation_id
            || self.release_id != release_id
            || !self.postgres_ledger_exactly_current
            || !self.runtime_role_and_config_verified
            || !self.ready
            || !unit_specific_ready
        {
            return invalid_proof(
                "service_readiness",
                "service readiness is not bound to the exact installation, release, ledger, and role config",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreActivationRecovery {
    operation_id: String,
    config_receipt: ConfigInstallReceipt,
    release_receipt: Option<ReleaseSwitchReceipt>,
}

impl PreActivationRecovery {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn config_receipt(&self) -> &ConfigInstallReceipt {
        &self.config_receipt
    }

    pub fn release_receipt(&self) -> Option<&ReleaseSwitchReceipt> {
        self.release_receipt.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreActivationRestoreProof {
    pub operation_id: String,
    pub native_units_never_started: bool,
    pub prior_configs_restored_or_new_configs_removed: bool,
    pub prior_release_link_restored_or_new_link_removed: bool,
    pub filesystem_state_fsynced: bool,
    pub prepared_targets_retained_for_same_operation_only: bool,
}

impl PreActivationRestoreProof {
    #[cfg(test)]
    fn validate(&self, operation_id: &str) -> Result<(), TargetActivationError> {
        if self.operation_id != operation_id
            || !self.native_units_never_started
            || !self.prior_configs_restored_or_new_configs_removed
            || !self.prior_release_link_restored_or_new_link_removed
            || !self.filesystem_state_fsynced
            || !self.prepared_targets_retained_for_same_operation_only
        {
            return invalid_proof(
                "pre_activation_restore",
                "pre-start filesystem restore or operation ownership verification failed",
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PreparationFailure {
    pub error: TargetActivationError,
    pub boundary: RecoveryBoundary,
    pub recovery: Option<Box<PreActivationRecovery>>,
}

#[derive(Debug)]
pub struct ActivationAttemptFailure {
    pub error: TargetActivationError,
    pub boundary: RecoveryBoundary,
    pub recovery: Option<Box<PreActivationRecovery>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BootstrappedInitialTarget {
    operation_id: String,
    manifest_binding_hmac_sha256: String,
    installation_id: String,
    inspect_review_sha256: String,
    backup_restore_proof_sha256: String,
    backup_reference_sha256: String,
    final_recheck_report_sha256: String,
    source_fingerprint_sha256: String,
    permit_generation: u64,
    permit_event_sha256: String,
    redis_binding: TargetRedisInspectionBinding,
    target_proof_sha256: String,
}

#[derive(Serialize)]
struct TargetProofMaterial<'a> {
    operation_id: &'a str,
    installation_id: &'a str,
    inspect_review_sha256: &'a str,
    backup_restore_proof_sha256: &'a str,
    backup_reference_sha256: &'a str,
    final_recheck_report_sha256: &'a str,
    source_fingerprint_sha256: &'a str,
    permit_generation: u64,
    permit_event_sha256: &'a str,
    redis_binding: &'a TargetRedisInspectionBinding,
    postgres_empty: &'a PostgresEmptyProof,
    postgres_created: &'a TargetCreationReceipt,
    postgres_ready: &'a PostgresReadyProof,
    clickhouse_empty: &'a ClickHouseEmptyProof,
    clickhouse_created: &'a TargetCreationReceipt,
    clickhouse_ready: &'a ClickHouseReadyProof,
    redis_still_empty: &'a RedisEmptyProof,
}

impl BootstrappedInitialTarget {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn target_proof_sha256(&self) -> &str {
        &self.target_proof_sha256
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    pub const fn permit_generation(&self) -> u64 {
        self.permit_generation
    }

    pub fn permit_event_sha256(&self) -> &str {
        &self.permit_event_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedActivation {
    operation_id: String,
    installation_id: String,
    provision_kind: ProvisionKind,
    verification: OfflineMigrationVerification,
    release: ReleaseArtifactSpec,
    config_receipt: ConfigInstallReceipt,
}

impl PreparedActivation {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    pub fn release(&self) -> &ReleaseArtifactSpec {
        &self.release
    }

    pub fn into_recovery(self) -> PreActivationRecovery {
        PreActivationRecovery {
            operation_id: self.operation_id,
            config_receipt: self.config_receipt,
            release_receipt: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActivatedTarget {
    operation_id: String,
    installation_id: String,
    provision_kind: ProvisionKind,
    release_id: String,
}

impl ActivatedTarget {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    pub fn recovery_boundary(&self) -> RecoveryBoundary {
        RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly
    }
}

pub struct LegacySourceRetirementRequest<'a> {
    operation_id: &'a str,
    installation_id: &'a str,
    source: &'a SourceSpec,
    cold_archive_reference: &'a str,
    cold_archive_sha256: &'a str,
}

impl<'a> LegacySourceRetirementRequest<'a> {
    pub fn operation_id(&self) -> &str {
        self.operation_id
    }

    pub fn installation_id(&self) -> &str {
        self.installation_id
    }

    pub fn source(&self) -> &SourceSpec {
        self.source
    }

    pub fn cold_archive_reference(&self) -> &str {
        self.cold_archive_reference
    }

    pub fn cold_archive_sha256(&self) -> &str {
        self.cold_archive_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RetirementMutationReceipt {
    pub operation_id: String,
    pub retirement_attempted_after_native_activation: bool,
    pub mysql_stop_disable_and_network_isolation_attempted: bool,
    pub source_redis_stop_disable_and_network_isolation_attempted: bool,
    pub credential_revocation_attempted: bool,
}

impl RetirementMutationReceipt {
    #[cfg(test)]
    fn validate(&self, operation_id: &str) -> Result<(), TargetActivationError> {
        if self.operation_id != operation_id
            || !self.retirement_attempted_after_native_activation
            || !self.mysql_stop_disable_and_network_isolation_attempted
            || !self.source_redis_stop_disable_and_network_isolation_attempted
            || !self.credential_revocation_attempted
        {
            return invalid_proof(
                "source_retirement_mutation",
                "source retirement did not attempt every irreversible action",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceRetirementObservation {
    pub operation_id: String,
    pub source_retired: bool,
    pub mysql_reachable: bool,
    pub source_redis_reachable: bool,
    pub source_credentials_revoked: bool,
    pub legacy_runtime_compat: bool,
    pub postgres_is_only_transaction_authority: bool,
    pub mysql_unreachable_evidence: String,
    pub source_redis_unreachable_evidence: String,
    pub credential_revocation_evidence: String,
    pub runtime_compat_evidence: String,
    pub postgres_authority_evidence: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRetirementProof {
    pub schema_version: u32,
    pub operation_id: String,
    pub source_retired: bool,
    pub mysql_reachable: bool,
    pub source_redis_reachable: bool,
    pub source_credentials_revoked: bool,
    pub legacy_runtime_compat: bool,
    pub postgres_is_only_transaction_authority: bool,
    pub cold_archive_reference: String,
    pub cold_archive_sha256: String,
    pub mysql_unreachable_evidence: String,
    pub source_redis_unreachable_evidence: String,
    pub credential_revocation_evidence: String,
    pub runtime_compat_evidence: String,
    pub postgres_authority_evidence: String,
}

impl SourceRetirementProof {
    pub fn validate(&self, expected_operation_id: &str) -> Result<(), TargetActivationError> {
        if self.schema_version != 1 || self.operation_id != expected_operation_id {
            return invalid_proof(
                "source_retirement",
                "retirement proof schema or operation binding is invalid",
            );
        }
        if !self.source_retired
            || self.mysql_reachable
            || self.source_redis_reachable
            || !self.source_credentials_revoked
            || self.legacy_runtime_compat
            || !self.postgres_is_only_transaction_authority
        {
            return invalid_proof(
                "source_retirement",
                "legacy source is not completely retired",
            );
        }
        if !is_lower_hex_sha256(&self.cold_archive_sha256)
            || !valid_evidence(&self.cold_archive_reference)
            || [
                &self.mysql_unreachable_evidence,
                &self.source_redis_unreachable_evidence,
                &self.credential_revocation_evidence,
                &self.runtime_compat_evidence,
                &self.postgres_authority_evidence,
            ]
            .iter()
            .any(|value| !valid_evidence(value))
        {
            return invalid_proof(
                "source_retirement",
                "cold archive or retirement evidence is missing or malformed",
            );
        }
        Ok(())
    }
}

pub trait TargetActivationExecutor {
    fn inspect_empty_postgres(
        &mut self,
        operation_id: &str,
        target: &PostgresTargetSpec,
    ) -> Result<PostgresEmptyProof, ExecutorError> {
        let _ = (operation_id, target);
        denied()
    }

    fn create_postgres_database_and_roles(
        &mut self,
        permit: &DurableTargetMutationPermit,
        target: &PostgresTargetSpec,
    ) -> Result<TargetCreationReceipt, ExecutorError> {
        let _ = (permit, target);
        denied()
    }

    fn verify_postgres_database_and_roles(
        &mut self,
        operation_id: &str,
        target: &PostgresTargetSpec,
    ) -> Result<PostgresReadyProof, ExecutorError> {
        let _ = (operation_id, target);
        denied()
    }

    fn inspect_empty_clickhouse(
        &mut self,
        operation_id: &str,
        target: &ClickHouseTargetSpec,
    ) -> Result<ClickHouseEmptyProof, ExecutorError> {
        let _ = (operation_id, target);
        denied()
    }

    fn create_clickhouse_database_and_roles(
        &mut self,
        permit: &DurableTargetMutationPermit,
        target: &ClickHouseTargetSpec,
    ) -> Result<TargetCreationReceipt, ExecutorError> {
        let _ = (permit, target);
        denied()
    }

    fn verify_clickhouse_database_and_roles(
        &mut self,
        operation_id: &str,
        target: &ClickHouseTargetSpec,
    ) -> Result<ClickHouseReadyProof, ExecutorError> {
        let _ = (operation_id, target);
        denied()
    }

    fn inspect_empty_target_redis(
        &mut self,
        operation_id: &str,
        redis_url: &str,
        binding: &TargetRedisInspectionBinding,
    ) -> Result<RedisEmptyProof, ExecutorError> {
        let _ = (operation_id, redis_url, binding);
        denied()
    }

    fn verify_release_artifact(
        &mut self,
        operation_id: &str,
        release: &ReleaseArtifactSpec,
    ) -> Result<ReleasePreflightProof, ExecutorError> {
        let _ = (operation_id, release);
        denied()
    }

    #[cfg(test)]
    fn verify_activation_commit_gate(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<ActivationCommitGateProof, ExecutorError> {
        let _ = verification;
        denied()
    }

    #[cfg(test)]
    fn commit_native_activation(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<NativeActivationCommitReceipt, ExecutorError> {
        let _ = verification;
        denied()
    }

    /// On `Err`, the executor must leave both prior config files unchanged.
    fn install_role_configs_atomically(
        &mut self,
        bundle: &RoleConfigBundle,
    ) -> Result<ConfigInstallReceipt, ExecutorError> {
        let _ = bundle;
        denied()
    }

    fn verify_role_configs(
        &mut self,
        bundle: &RoleConfigBundle,
        receipt: &ConfigInstallReceipt,
    ) -> Result<ConfigVerificationProof, ExecutorError> {
        let _ = (bundle, receipt);
        denied()
    }

    /// On `Err`, the executor must leave `/opt/v2board/current` unchanged.
    #[cfg(test)]
    fn switch_current_release_atomically(
        &mut self,
        operation_id: &str,
        release: &ReleaseArtifactSpec,
    ) -> Result<ReleaseSwitchReceipt, ExecutorError> {
        let _ = (operation_id, release);
        denied()
    }

    #[cfg(test)]
    fn start_unit_once(
        &mut self,
        operation_id: &str,
        unit: &'static str,
    ) -> Result<UnitStartReceipt, ExecutorError> {
        let _ = (operation_id, unit);
        denied()
    }

    #[cfg(test)]
    fn wait_for_unit_readiness(
        &mut self,
        operation_id: &str,
        installation_id: &str,
        release: &ReleaseArtifactSpec,
        unit: &'static str,
    ) -> Result<ServiceReadinessProof, ExecutorError> {
        let _ = (operation_id, installation_id, release, unit);
        denied()
    }

    #[cfg(test)]
    fn restore_before_native_start(
        &mut self,
        recovery: &PreActivationRecovery,
    ) -> Result<PreActivationRestoreProof, ExecutorError> {
        let _ = recovery;
        denied()
    }

    #[cfg(test)]
    fn execute_legacy_source_retirement(
        &mut self,
        request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<RetirementMutationReceipt, ExecutorError> {
        let _ = request;
        denied()
    }

    #[cfg(test)]
    fn inspect_legacy_source_retirement(
        &mut self,
        request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<SourceRetirementObservation, ExecutorError> {
        let _ = request;
        denied()
    }
}

#[derive(Default)]
pub struct DenyRealTargetActivation;

impl TargetActivationExecutor for DenyRealTargetActivation {}

pub fn bootstrap_empty_initial_targets<E: TargetActivationExecutor>(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    redis_binding: TargetRedisInspectionBinding,
    executor: &mut E,
) -> Result<BootstrappedInitialTarget, TargetActivationError> {
    if permit.operation_id() != spec.operation_id
        || !valid_non_nil_uuid(permit.installation_id())
        || permit.generation() == 0
        || !is_lower_hex_sha256(permit.inspect_review_sha256())
        || !is_lower_hex_sha256(permit.event_sha256())
        || !is_lower_hex_sha256(permit.backup_restore_proof_sha256())
        || !is_lower_hex_sha256(permit.backup_reference_sha256())
        || !is_lower_hex_sha256(permit.final_recheck_report_sha256())
        || !is_lower_hex_sha256(permit.source_fingerprint_sha256())
        || redis_binding.inspect_review_sha256 != permit.inspect_review_sha256()
    {
        return Err(TargetActivationError::BindingMismatch);
    }
    let target = initial_target(spec)?;
    let postgres_empty = executor
        .inspect_empty_postgres(&spec.operation_id, &target.postgres)
        .map_err(|error| error.into_activation("inspect_empty_postgres"))?;
    postgres_empty.validate()?;
    let clickhouse_empty = executor
        .inspect_empty_clickhouse(&spec.operation_id, &target.clickhouse)
        .map_err(|error| error.into_activation("inspect_empty_clickhouse"))?;
    clickhouse_empty.validate()?;
    let redis_empty = executor
        .inspect_empty_target_redis(&spec.operation_id, &target.redis_url, &redis_binding)
        .map_err(|error| error.into_activation("inspect_empty_target_redis"))?;
    redis_empty.validate(&redis_binding)?;

    let postgres_created = executor
        .create_postgres_database_and_roles(permit, &target.postgres)
        .map_err(|error| error.into_activation("create_postgres_database_and_roles"))?;
    postgres_created.validate(permit, "postgres_create")?;
    let postgres_ready = executor
        .verify_postgres_database_and_roles(&spec.operation_id, &target.postgres)
        .map_err(|error| error.into_activation("verify_postgres_database_and_roles"))?;
    postgres_ready.validate()?;

    let clickhouse_created = executor
        .create_clickhouse_database_and_roles(permit, &target.clickhouse)
        .map_err(|error| error.into_activation("create_clickhouse_database_and_roles"))?;
    clickhouse_created.validate(permit, "clickhouse_create")?;
    let clickhouse_ready = executor
        .verify_clickhouse_database_and_roles(&spec.operation_id, &target.clickhouse)
        .map_err(|error| error.into_activation("verify_clickhouse_database_and_roles"))?;
    clickhouse_ready.validate()?;

    let redis_still_empty = executor
        .inspect_empty_target_redis(&spec.operation_id, &target.redis_url, &redis_binding)
        .map_err(|error| error.into_activation("recheck_empty_target_redis"))?;
    redis_still_empty.validate(&redis_binding)?;

    let proof_bytes = serde_json::to_vec(&TargetProofMaterial {
        operation_id: permit.operation_id(),
        installation_id: permit.installation_id(),
        inspect_review_sha256: permit.inspect_review_sha256(),
        backup_restore_proof_sha256: permit.backup_restore_proof_sha256(),
        backup_reference_sha256: permit.backup_reference_sha256(),
        final_recheck_report_sha256: permit.final_recheck_report_sha256(),
        source_fingerprint_sha256: permit.source_fingerprint_sha256(),
        permit_generation: permit.generation(),
        permit_event_sha256: permit.event_sha256(),
        redis_binding: &redis_binding,
        postgres_empty: &postgres_empty,
        postgres_created: &postgres_created,
        postgres_ready: &postgres_ready,
        clickhouse_empty: &clickhouse_empty,
        clickhouse_created: &clickhouse_created,
        clickhouse_ready: &clickhouse_ready,
        redis_still_empty: &redis_still_empty,
    })?;
    Ok(BootstrappedInitialTarget {
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        installation_id: permit.installation_id().to_string(),
        inspect_review_sha256: permit.inspect_review_sha256().to_string(),
        backup_restore_proof_sha256: permit.backup_restore_proof_sha256().to_string(),
        backup_reference_sha256: permit.backup_reference_sha256().to_string(),
        final_recheck_report_sha256: permit.final_recheck_report_sha256().to_string(),
        source_fingerprint_sha256: permit.source_fingerprint_sha256().to_string(),
        permit_generation: permit.generation(),
        permit_event_sha256: permit.event_sha256().to_string(),
        redis_binding,
        target_proof_sha256: sha256_hex(&proof_bytes),
    })
}

#[cfg(test)]
pub fn prepare_bare_metal_activation<E: TargetActivationExecutor>(
    spec: &ProvisionSpec,
    bootstrapped: &BootstrappedInitialTarget,
    verification: &OfflineMigrationVerification,
    release: ReleaseArtifactSpec,
    executor: &mut E,
) -> Result<PreparedActivation, PreparationFailure> {
    let prepare = || PreparationFailure {
        error: TargetActivationError::BindingMismatch,
        boundary: RecoveryBoundary::BeforeNativeWriterStart,
        recovery: None,
    };
    if bootstrapped.operation_id != spec.operation_id
        || bootstrapped.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
        || verification.installation_id != bootstrapped.installation_id
        || verification.inspect_review_sha256 != bootstrapped.inspect_review_sha256
        || verification.backup_restore_report_sha256 != bootstrapped.backup_restore_proof_sha256
        || verification.backup_reference_sha256 != bootstrapped.backup_reference_sha256
        || verification.final_recheck_report_sha256 != bootstrapped.final_recheck_report_sha256
        || verification.source_fingerprint_sha256 != bootstrapped.source_fingerprint_sha256
        || verification.bootstrap_permit_generation != bootstrapped.permit_generation
        || verification.bootstrap_permit_event_sha256 != bootstrapped.permit_event_sha256
    {
        return Err(prepare());
    }
    if let Err(error) = verification.validate(spec) {
        return Err(PreparationFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: None,
        });
    }
    if let Err(error) = verify_target_before_activation(spec, &bootstrapped.redis_binding, executor)
    {
        return Err(PreparationFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: None,
        });
    }
    let gate = match executor.verify_activation_commit_gate(verification) {
        Ok(gate) => gate,
        Err(error) => {
            return Err(PreparationFailure {
                error: error.into_activation("verify_activation_commit_gate"),
                boundary: RecoveryBoundary::BeforeNativeWriterStart,
                recovery: None,
            });
        }
    };
    if let Err(error) = gate.validate(verification) {
        return Err(PreparationFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: None,
        });
    }
    let release_proof = match executor.verify_release_artifact(&spec.operation_id, &release) {
        Ok(proof) => proof,
        Err(error) => {
            return Err(PreparationFailure {
                error: error.into_activation("verify_release_artifact"),
                boundary: RecoveryBoundary::BeforeNativeWriterStart,
                recovery: None,
            });
        }
    };
    if let Err(error) = release_proof.validate(&release) {
        return Err(PreparationFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: None,
        });
    }

    let bundle = match materialize_role_configs(spec) {
        Ok(bundle) => bundle,
        Err(error) => {
            return Err(PreparationFailure {
                error,
                boundary: RecoveryBoundary::BeforeNativeWriterStart,
                recovery: None,
            });
        }
    };
    let config_receipt = match executor.install_role_configs_atomically(&bundle) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Err(PreparationFailure {
                error: error.into_activation("install_role_configs_atomically"),
                boundary: RecoveryBoundary::BeforeNativeWriterStart,
                recovery: None,
            });
        }
    };
    let recovery = PreActivationRecovery {
        operation_id: spec.operation_id.clone(),
        config_receipt: config_receipt.clone(),
        release_receipt: None,
    };
    if let Err(error) = config_receipt.validate(&spec.operation_id) {
        return Err(PreparationFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: Some(Box::new(recovery)),
        });
    }
    let config_proof = match executor.verify_role_configs(&bundle, &config_receipt) {
        Ok(proof) => proof,
        Err(error) => {
            return Err(PreparationFailure {
                error: error.into_activation("verify_role_configs"),
                boundary: RecoveryBoundary::BeforeNativeWriterStart,
                recovery: Some(Box::new(recovery)),
            });
        }
    };
    if let Err(error) = config_proof.validate(&bundle) {
        return Err(PreparationFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: Some(Box::new(recovery)),
        });
    }

    Ok(PreparedActivation {
        operation_id: spec.operation_id.clone(),
        installation_id: verification.installation_id.clone(),
        provision_kind: spec.kind,
        verification: verification.clone(),
        release,
        config_receipt,
    })
}

#[cfg(test)]
pub fn activate_native_once<E: TargetActivationExecutor>(
    prepared: PreparedActivation,
    executor: &mut E,
) -> Result<ActivatedTarget, ActivationAttemptFailure> {
    let release_proof =
        match executor.verify_release_artifact(&prepared.operation_id, &prepared.release) {
            Ok(proof) => proof,
            Err(error) => {
                return Err(ActivationAttemptFailure {
                    error: error.into_activation("final_verify_release_artifact"),
                    boundary: RecoveryBoundary::BeforeNativeWriterStart,
                    recovery: Some(Box::new(prepared.into_recovery())),
                });
            }
        };
    if let Err(error) = release_proof.validate(&prepared.release) {
        return Err(ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: Some(Box::new(prepared.into_recovery())),
        });
    }
    let release_receipt = match executor
        .switch_current_release_atomically(&prepared.operation_id, &prepared.release)
    {
        Ok(receipt) => receipt,
        Err(error) => {
            return Err(ActivationAttemptFailure {
                error: error.into_activation("switch_current_release_atomically"),
                boundary: RecoveryBoundary::BeforeNativeWriterStart,
                recovery: Some(Box::new(prepared.into_recovery())),
            });
        }
    };
    let recovery = PreActivationRecovery {
        operation_id: prepared.operation_id.clone(),
        config_receipt: prepared.config_receipt.clone(),
        release_receipt: Some(release_receipt.clone()),
    };
    if let Err(error) = release_receipt.validate(&prepared.operation_id, &prepared.release) {
        return Err(ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::BeforeNativeWriterStart,
            recovery: Some(Box::new(recovery)),
        });
    }

    // The PostgreSQL CAS makes native PostgreSQL the sole transactional
    // authority. An error response is ambiguous, so from this invocation
    // onward neither the old source nor the previous config/release may be
    // restored as a runtime rollback.
    let activation_commit = executor
        .commit_native_activation(&prepared.verification)
        .map_err(|error| ActivationAttemptFailure {
            error: error.into_activation("commit_native_activation"),
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    activation_commit
        .validate(&prepared.verification)
        .map_err(|error| ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;

    // The boundary already moved at the authority CAS. Each unit is still
    // invoked exactly once; failures require native forward recovery.
    let api_start = executor
        .start_unit_once(&prepared.operation_id, API_UNIT)
        .map_err(|error| ActivationAttemptFailure {
            error: error.into_activation("start_api_once"),
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    api_start
        .validate(&prepared.operation_id, API_UNIT)
        .map_err(|error| ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    let api_ready = executor
        .wait_for_unit_readiness(
            &prepared.operation_id,
            &prepared.installation_id,
            &prepared.release,
            API_UNIT,
        )
        .map_err(|error| ActivationAttemptFailure {
            error: error.into_activation("wait_api_ready"),
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    api_ready
        .validate(
            &prepared.operation_id,
            &prepared.installation_id,
            prepared.release.release_id(),
            API_UNIT,
        )
        .map_err(|error| ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;

    let worker_start = executor
        .start_unit_once(&prepared.operation_id, WORKER_UNIT)
        .map_err(|error| ActivationAttemptFailure {
            error: error.into_activation("start_worker_once"),
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    worker_start
        .validate(&prepared.operation_id, WORKER_UNIT)
        .map_err(|error| ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    let worker_ready = executor
        .wait_for_unit_readiness(
            &prepared.operation_id,
            &prepared.installation_id,
            &prepared.release,
            WORKER_UNIT,
        )
        .map_err(|error| ActivationAttemptFailure {
            error: error.into_activation("wait_worker_ready"),
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;
    worker_ready
        .validate(
            &prepared.operation_id,
            &prepared.installation_id,
            prepared.release.release_id(),
            WORKER_UNIT,
        )
        .map_err(|error| ActivationAttemptFailure {
            error,
            boundary: RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly,
            recovery: None,
        })?;

    Ok(ActivatedTarget {
        operation_id: prepared.operation_id,
        installation_id: prepared.installation_id,
        provision_kind: prepared.provision_kind,
        release_id: prepared.release.release_id,
    })
}

#[cfg(test)]
pub fn restore_before_native_start<E: TargetActivationExecutor>(
    recovery: PreActivationRecovery,
    executor: &mut E,
) -> Result<PreActivationRestoreProof, TargetActivationError> {
    let proof = executor
        .restore_before_native_start(&recovery)
        .map_err(|error| error.into_activation("restore_before_native_start"))?;
    proof.validate(&recovery.operation_id)?;
    Ok(proof)
}

#[cfg(test)]
pub fn retire_legacy_source<E: TargetActivationExecutor>(
    spec: &ProvisionSpec,
    activated: &ActivatedTarget,
    cold_archive_reference: &str,
    cold_archive_sha256: &str,
    executor: &mut E,
) -> Result<SourceRetirementProof, TargetActivationError> {
    if activated.operation_id != spec.operation_id
        || activated.provision_kind != ProvisionKind::LegacyReferenceMigration
        || !is_lower_hex_sha256(cold_archive_sha256)
        || !valid_evidence(cold_archive_reference)
    {
        return Err(TargetActivationError::BindingMismatch);
    }
    let source = match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => source,
        _ => return Err(TargetActivationError::UnsupportedProvisionKind),
    };
    let request = LegacySourceRetirementRequest {
        operation_id: &spec.operation_id,
        installation_id: &activated.installation_id,
        source,
        cold_archive_reference,
        cold_archive_sha256,
    };
    let mutation = executor
        .execute_legacy_source_retirement(&request)
        .map_err(|error| error.into_activation("execute_legacy_source_retirement"))?;
    mutation.validate(&spec.operation_id)?;
    let observation = executor
        .inspect_legacy_source_retirement(&request)
        .map_err(|error| error.into_activation("inspect_legacy_source_retirement"))?;
    let proof = SourceRetirementProof {
        schema_version: 1,
        operation_id: observation.operation_id,
        source_retired: observation.source_retired,
        mysql_reachable: observation.mysql_reachable,
        source_redis_reachable: observation.source_redis_reachable,
        source_credentials_revoked: observation.source_credentials_revoked,
        legacy_runtime_compat: observation.legacy_runtime_compat,
        postgres_is_only_transaction_authority: observation.postgres_is_only_transaction_authority,
        cold_archive_reference: cold_archive_reference.to_string(),
        cold_archive_sha256: cold_archive_sha256.to_string(),
        mysql_unreachable_evidence: observation.mysql_unreachable_evidence,
        source_redis_unreachable_evidence: observation.source_redis_unreachable_evidence,
        credential_revocation_evidence: observation.credential_revocation_evidence,
        runtime_compat_evidence: observation.runtime_compat_evidence,
        postgres_authority_evidence: observation.postgres_authority_evidence,
    };
    proof.validate(&spec.operation_id)?;
    Ok(proof)
}

fn initial_target(spec: &ProvisionSpec) -> Result<&TargetSpec, TargetActivationError> {
    match &spec.flow {
        ProvisionFlow::FreshInstall { target, .. }
        | ProvisionFlow::LegacyReferenceMigration { target, .. } => Ok(target),
        ProvisionFlow::NativeUpgrade { .. } => Err(TargetActivationError::UnsupportedProvisionKind),
    }
}

#[cfg(test)]
fn verify_target_before_activation<E: TargetActivationExecutor>(
    spec: &ProvisionSpec,
    redis_binding: &TargetRedisInspectionBinding,
    executor: &mut E,
) -> Result<(), TargetActivationError> {
    let target = initial_target(spec)?;
    let postgres = executor
        .verify_postgres_database_and_roles(&spec.operation_id, &target.postgres)
        .map_err(|error| error.into_activation("final_verify_postgres_database_and_roles"))?;
    postgres.validate_for_activation()?;
    let clickhouse = executor
        .verify_clickhouse_database_and_roles(&spec.operation_id, &target.clickhouse)
        .map_err(|error| error.into_activation("final_verify_clickhouse_database_and_roles"))?;
    clickhouse.validate()?;
    let redis = executor
        .inspect_empty_target_redis(&spec.operation_id, &target.redis_url, redis_binding)
        .map_err(|error| error.into_activation("final_recheck_empty_target_redis"))?;
    redis.validate(redis_binding)?;
    Ok(())
}

pub(crate) fn materialize_role_configs(
    spec: &ProvisionSpec,
) -> Result<RoleConfigBundle, TargetActivationError> {
    let api = spec.materialized_api_runtime_config()?;
    let worker = spec.materialized_worker_runtime_config()?;
    let api_bytes = canonical_config_bytes(&api)?;
    let worker_bytes = canonical_config_bytes(&worker)?;
    let api_binding_hmac_sha256 = role_config_binding(spec, b"api", &api_bytes);
    let worker_binding_hmac_sha256 = role_config_binding(spec, b"worker", &worker_bytes);
    Ok(RoleConfigBundle {
        operation_id: spec.operation_id.clone(),
        api_path: PathBuf::from(API_CONFIG_PATH),
        worker_path: PathBuf::from(WORKER_CONFIG_PATH),
        api_bytes,
        worker_bytes,
        api_binding_hmac_sha256,
        worker_binding_hmac_sha256,
    })
}

fn canonical_config_bytes(config: &Map<String, Value>) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(config)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn role_config_binding(spec: &ProvisionSpec, role: &[u8], bytes: &[u8]) -> String {
    let mut binding_input = Vec::with_capacity(24 + role.len() + bytes.len());
    binding_input.extend_from_slice(b"role-config-v1\0");
    binding_input.extend_from_slice(role);
    binding_input.push(0);
    binding_input.extend_from_slice(bytes);
    spec.report_binding_hmac_sha256(&binding_input)
}

fn invalid_proof<T>(stage: &'static str, reason: &'static str) -> Result<T, TargetActivationError> {
    Err(TargetActivationError::InvalidProof { stage, reason })
}

fn denied<T>() -> Result<T, ExecutorError> {
    Err(ExecutorError::sanitized("real_target_activation_disabled"))
}

fn valid_release_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
fn valid_opaque_handle(value: &str) -> bool {
    value.len() >= 16
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.'))
}

fn valid_evidence(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 8
        && trimmed.len() <= 1024
        && !trimmed.eq_ignore_ascii_case("placeholder")
        && !trimmed.to_ascii_lowercase().starts_with("replace_with")
        && !trimmed.chars().any(char::is_control)
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_redis_run_id(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_non_nil_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .ok()
        .is_some_and(|value| !value.is_nil())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use crate::apply_journal::{ApplyCheckpoint, ApplyJournal, ApplyJournalBinding};

    const OPERATION_ID: &str = "85be1924-a948-4d43-8a59-4d2785ce87d0";
    const INSTALLATION_ID: &str = "7e8b8f52-f658-40b0-8718-a11ba44de020";
    const INSPECT_REVIEW_SHA256: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BACKUP_REPORT_SHA256: &str =
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const BACKUP_REFERENCE_SHA256: &str =
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const FINAL_RECHECK_SHA256: &str =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const SOURCE_FINGERPRINT_SHA256: &str =
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    static JOURNAL_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct ActivationMock {
        events: Vec<String>,
        fail_commit: bool,
        fail_api_start: bool,
    }

    impl TargetActivationExecutor for ActivationMock {
        fn verify_release_artifact(
            &mut self,
            operation_id: &str,
            release: &ReleaseArtifactSpec,
        ) -> Result<ReleasePreflightProof, ExecutorError> {
            self.events.push("verify_release".to_string());
            assert_eq!(operation_id, OPERATION_ID);
            Ok(ReleasePreflightProof {
                release_id: release.release_id().to_string(),
                canonical_staged_path: release.staged_path().to_path_buf(),
                external_archive_sha256: release.external_archive_sha256().to_string(),
                internal_sha256sums_valid: true,
                exact_long_lived_binary_set: true,
                validated_frontend_tree_present: true,
                release_metadata_valid: true,
                root_owned_and_runtime_read_only: true,
                systemd_analyze_verify_passed: true,
                unit_exec_paths_use_current_symlink: true,
                api_unit_uses_dedicated_identity_and_config: true,
                worker_unit_uses_dedicated_identity_and_config: true,
                api_bind_is_loopback_only: true,
            })
        }

        fn switch_current_release_atomically(
            &mut self,
            operation_id: &str,
            release: &ReleaseArtifactSpec,
        ) -> Result<ReleaseSwitchReceipt, ExecutorError> {
            self.events.push("switch_release".to_string());
            Ok(ReleaseSwitchReceipt {
                operation_id: operation_id.to_string(),
                current_path: PathBuf::from(CURRENT_RELEASE_PATH),
                staged_path: release.staged_path().to_path_buf(),
                previous_target: None,
                atomic_symlink_rename_complete: true,
                parent_directory_fsynced: true,
                rollback_handle: "rollback:release:123456".to_string(),
            })
        }

        fn commit_native_activation(
            &mut self,
            verification: &OfflineMigrationVerification,
        ) -> Result<NativeActivationCommitReceipt, ExecutorError> {
            self.events.push("commit_authority".to_string());
            if self.fail_commit {
                return Err(ExecutorError::sanitized("mock_commit_failure"));
            }
            Ok(NativeActivationCommitReceipt {
                operation_id: verification.operation_id.clone(),
                installation_id: verification.installation_id.clone(),
                target_permit_generation: verification.target_permit_generation,
                target_permit_event_sha256: verification.target_permit_event_sha256.clone(),
                backup_reference_sha256: verification.backup_reference_sha256.clone(),
                source_fingerprint_sha256: verification.source_fingerprint_sha256.clone(),
                lifecycle_ledger_cas_committed: true,
                postgres_installation_active: true,
                postgres_is_only_transaction_authority: true,
                old_writer_fence_still_verified: true,
            })
        }

        fn start_unit_once(
            &mut self,
            operation_id: &str,
            unit: &'static str,
        ) -> Result<UnitStartReceipt, ExecutorError> {
            self.events.push(format!("start:{unit}"));
            if self.fail_api_start && unit == API_UNIT {
                return Err(ExecutorError::sanitized("mock_api_start_failure"));
            }
            Ok(UnitStartReceipt {
                operation_id: operation_id.to_string(),
                unit: unit.to_string(),
                invocation_count_for_operation: 1,
                start_request_accepted: true,
            })
        }

        fn wait_for_unit_readiness(
            &mut self,
            operation_id: &str,
            installation_id: &str,
            release: &ReleaseArtifactSpec,
            unit: &'static str,
        ) -> Result<ServiceReadinessProof, ExecutorError> {
            self.events.push(format!("ready:{unit}"));
            let worker = unit == WORKER_UNIT;
            Ok(ServiceReadinessProof {
                operation_id: operation_id.to_string(),
                unit: unit.to_string(),
                installation_id: installation_id.to_string(),
                release_id: release.release_id().to_string(),
                postgres_ledger_exactly_current: true,
                runtime_role_and_config_verified: true,
                ready: true,
                systemd_notify_ready: worker.then_some(true),
                watchdog_healthy: worker.then_some(true),
            })
        }
    }

    fn digest() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
    }

    fn target_mutation_permit() -> DurableTargetMutationPermit {
        let root = std::env::temp_dir().join(format!(
            "v2board-target-activation-journal-{}-{}",
            std::process::id(),
            JOURNAL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let binding =
            ApplyJournalBinding::new(OPERATION_ID, INSPECT_REVIEW_SHA256).expect("journal binding");
        let (journal, pending) =
            ApplyJournal::create_pending(&root, binding).expect("pending journal");
        let mut current = journal.begin(&pending).expect("begin");
        for checkpoint in [
            ApplyCheckpoint::MaintenanceFenced,
            ApplyCheckpoint::SourceDrained,
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, digest())
                .expect("advance pre-target checkpoint");
        }
        current = journal
            .record_backup_restore_verified(&current, BACKUP_REPORT_SHA256, BACKUP_REFERENCE_SHA256)
            .expect("bind backup proof");
        current = journal
            .record_final_recheck_passed(&current, FINAL_RECHECK_SHA256, SOURCE_FINGERPRINT_SHA256)
            .expect("bind final recheck");
        current = journal
            .reserve_installation_identity(&current, INSTALLATION_ID)
            .expect("reserve installation");
        let permit = journal
            .target_mutation_permit(&current)
            .expect("target permit");
        fs::remove_dir_all(root).expect("remove journal fixture");
        permit
    }

    #[test]
    fn target_creation_receipt_must_match_durable_journal_permit_exactly() {
        let permit = target_mutation_permit();
        let mut receipt = TargetCreationReceipt {
            operation_id: permit.operation_id().to_string(),
            installation_id: permit.installation_id().to_string(),
            permit_generation: permit.generation(),
            permit_event_sha256: permit.event_sha256().to_string(),
            inspect_review_sha256: permit.inspect_review_sha256().to_string(),
            backup_restore_proof_sha256: permit.backup_restore_proof_sha256().to_string(),
            backup_reference_sha256: permit.backup_reference_sha256().to_string(),
            final_recheck_report_sha256: permit.final_recheck_report_sha256().to_string(),
            source_fingerprint_sha256: permit.source_fingerprint_sha256().to_string(),
            objects_created_for_this_operation: true,
            broad_if_not_exists_not_used: true,
        };
        receipt
            .validate(&permit, "test_target_create")
            .expect("matching durable permit");
        receipt.permit_generation += 1;
        assert!(receipt.validate(&permit, "test_target_create").is_err());
    }

    fn prepared_activation() -> PreparedActivation {
        let verification = OfflineMigrationVerification {
            operation_id: OPERATION_ID.to_string(),
            manifest_binding_hmac_sha256: "d".repeat(64),
            installation_id: INSTALLATION_ID.to_string(),
            inspect_review_sha256: INSPECT_REVIEW_SHA256.to_string(),
            backup_restore_report_sha256: BACKUP_REPORT_SHA256.to_string(),
            backup_reference_sha256: BACKUP_REFERENCE_SHA256.to_string(),
            final_recheck_report_sha256: FINAL_RECHECK_SHA256.to_string(),
            source_fingerprint_sha256: SOURCE_FINGERPRINT_SHA256.to_string(),
            bootstrap_permit_generation: 6,
            bootstrap_permit_event_sha256: "e".repeat(64),
            target_permit_generation: 6,
            target_permit_event_sha256: "e".repeat(64),
            data_verification_report_sha256: "f".repeat(64),
            analytics_projection_report_sha256: "1".repeat(64),
            node_cutover_report_sha256: Some("2".repeat(64)),
            old_writers_fenced: true,
            new_writers_still_stopped: true,
            postgres_is_transaction_authority: true,
            clickhouse_is_rebuildable_projection: true,
        };
        PreparedActivation {
            operation_id: OPERATION_ID.to_string(),
            installation_id: INSTALLATION_ID.to_string(),
            provision_kind: ProvisionKind::LegacyReferenceMigration,
            verification,
            release: ReleaseArtifactSpec::new("release-1", digest()).expect("release"),
            config_receipt: ConfigInstallReceipt {
                operation_id: OPERATION_ID.to_string(),
                api_path: PathBuf::from(API_CONFIG_PATH),
                worker_path: PathBuf::from(WORKER_CONFIG_PATH),
                api_owner: API_OWNER.to_string(),
                worker_owner: WORKER_OWNER.to_string(),
                api_mode: OWNER_ONLY_MODE,
                worker_mode: OWNER_ONLY_MODE,
                regular_files_without_symlinks: true,
                temp_files_fsynced: true,
                atomic_renames_complete: true,
                parent_directories_fsynced: true,
                rollback_handle: "rollback:config:123456".to_string(),
            },
        }
    }

    #[test]
    fn mock_activation_starts_each_native_unit_once_in_fixed_order() {
        let mut executor = ActivationMock::default();
        let activated =
            activate_native_once(prepared_activation(), &mut executor).expect("activation");
        assert_eq!(activated.operation_id(), OPERATION_ID);
        assert_eq!(
            executor.events,
            [
                "verify_release",
                "switch_release",
                "commit_authority",
                "start:v2board-api.service",
                "ready:v2board-api.service",
                "start:v2board-worker.service",
                "ready:v2board-worker.service",
            ]
        );
    }

    #[test]
    fn ambiguous_api_start_failure_crosses_forward_only_boundary() {
        let mut executor = ActivationMock {
            fail_api_start: true,
            ..ActivationMock::default()
        };
        let failure = activate_native_once(prepared_activation(), &mut executor)
            .expect_err("API start must fail");
        assert_eq!(
            failure.boundary,
            RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly
        );
        assert!(failure.recovery.is_none());
    }

    #[test]
    fn ambiguous_authority_commit_failure_forbids_source_rollback() {
        let mut executor = ActivationMock {
            fail_commit: true,
            ..ActivationMock::default()
        };
        let failure = activate_native_once(prepared_activation(), &mut executor)
            .expect_err("authority commit must fail");
        assert_eq!(
            failure.boundary,
            RecoveryBoundary::AfterNativeWriterStartForwardRecoveryOnly
        );
        assert!(failure.recovery.is_none());
        assert!(
            !executor
                .events
                .iter()
                .any(|event| event.starts_with("start:"))
        );
    }

    #[test]
    fn release_path_is_derived_and_cannot_escape_release_root() {
        let release = ReleaseArtifactSpec::new("2026.07.12-abc123", digest()).expect("release");
        assert_eq!(
            release.staged_path(),
            Path::new("/opt/v2board/releases/2026.07.12-abc123")
        );
        assert!(ReleaseArtifactSpec::new("../escape", digest()).is_err());
        assert!(ReleaseArtifactSpec::new("release..escape", digest()).is_err());
        assert!(ReleaseArtifactSpec::new(".hidden", digest()).is_err());
        assert!(ReleaseArtifactSpec::new("valid", digest().to_ascii_uppercase()).is_err());
    }

    #[test]
    fn source_retirement_proof_requires_every_irreversible_fact() {
        let operation_id = OPERATION_ID;
        let mut proof = SourceRetirementProof {
            schema_version: 1,
            operation_id: operation_id.to_string(),
            source_retired: true,
            mysql_reachable: false,
            source_redis_reachable: false,
            source_credentials_revoked: true,
            legacy_runtime_compat: false,
            postgres_is_only_transaction_authority: true,
            cold_archive_reference: "vault:legacy/archive-20260712".to_string(),
            cold_archive_sha256: digest(),
            mysql_unreachable_evidence: "probe:mysql-denied:20260712".to_string(),
            source_redis_unreachable_evidence: "probe:source-redis-denied:20260712".to_string(),
            credential_revocation_evidence: "iam:revocation-123456".to_string(),
            runtime_compat_evidence: "release-audit:no-legacy-runtime".to_string(),
            postgres_authority_evidence: "ledger:postgres-authority-only".to_string(),
        };
        proof.validate(operation_id).expect("complete proof");

        proof.mysql_reachable = true;
        assert!(proof.validate(operation_id).is_err());
        proof.mysql_reachable = false;
        proof.legacy_runtime_compat = true;
        assert!(proof.validate(operation_id).is_err());
    }

    #[test]
    fn readiness_distinguishes_api_from_notify_watchdog_worker() {
        let operation_id = OPERATION_ID;
        let installation_id = INSTALLATION_ID;
        let mut proof = ServiceReadinessProof {
            operation_id: operation_id.to_string(),
            unit: API_UNIT.to_string(),
            installation_id: installation_id.to_string(),
            release_id: "release-1".to_string(),
            postgres_ledger_exactly_current: true,
            runtime_role_and_config_verified: true,
            ready: true,
            systemd_notify_ready: None,
            watchdog_healthy: None,
        };
        proof
            .validate(operation_id, installation_id, "release-1", API_UNIT)
            .expect("API ready");

        proof.unit = WORKER_UNIT.to_string();
        assert!(
            proof
                .validate(operation_id, installation_id, "release-1", WORKER_UNIT)
                .is_err()
        );
        proof.systemd_notify_ready = Some(true);
        proof.watchdog_healthy = Some(true);
        proof
            .validate(operation_id, installation_id, "release-1", WORKER_UNIT)
            .expect("worker ready");
    }

    #[test]
    fn default_executor_refuses_even_read_only_entry_to_real_activation() {
        let mut executor = DenyRealTargetActivation;
        let binding = TargetRedisInspectionBinding {
            inspect_review_sha256: INSPECT_REVIEW_SHA256.to_string(),
            target_run_id: "0123456789abcdef0123456789abcdef01234567".to_string(),
            source_run_ids: Vec::new(),
        };
        let error = executor
            .inspect_empty_target_redis("operation", "rediss://target.invalid/1", &binding)
            .expect_err("default executor must deny");
        assert_eq!(error.code, "real_target_activation_disabled");
    }

    #[test]
    fn executor_errors_never_echo_unsanitized_details() {
        let error = ExecutorError::sanitized("password=top-secret host failure");
        assert_eq!(error.code, "invalid_or_unsanitized_executor_error");
    }
}
