use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::EntryType;
use url::Url;
#[cfg(test)]
use v2board_analytics::CLICKHOUSE_MIGRATIONS;

use crate::{
    ProvisionSpec,
    apply_journal::{DurableNativeStartPermit, DurableTargetMutationPermit},
    manifest::{ClickHousePrincipalSpec, ClickHouseTargetSpec, PostgresTargetSpec},
    postgres_runtime_grants::{RuntimeAclSchemaState, RuntimeRoleNames, runtime_acl_catalog_sql},
    target_activation::{
        ActivationCommitGateProof, ClickHouseEmptyProof, ClickHouseReadyProof,
        ConfigInstallReceipt, ConfigVerificationProof, ExecutorError,
        LegacySourceRetirementRequest, NativeActivationCommitReceipt, OfflineMigrationVerification,
        PostgresEmptyProof, PostgresReadyProof, PostgresRuntimeAclSchemaState, RedisEmptyProof,
        ReleaseArtifactSpec, ReleasePreflightProof, RoleConfigBundle, ServiceReadinessProof,
        TargetActivationExecutor, TargetCreationReceipt, TargetRedisInspectionBinding,
    },
};

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use sqlx::PgPool;

#[cfg(test)]
use v2board_db::migrations_current;

#[cfg(test)]
use crate::{
    apply_journal::{ApplyCheckpoint, ApplyJournalSnapshot, ApplyJournalState},
    lifecycle_ledger::{
        AuthorizationAuditBinding, NativeActivationProofBinding, commit_native_activation,
    },
};

#[cfg(test)]
use crate::target_activation::{
    PreActivationRecovery, PreActivationRestoreProof, ReleaseSwitchReceipt,
    RetirementMutationReceipt, SourceRetirementObservation, UnitStartReceipt,
};

const MAX_COMMAND_INPUT_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;
const MAX_RELEASE_ENTRIES: usize = 100_000;
const API_CONFIG_PATH: &str = "/var/lib/v2board/api/config.json";
const WORKER_CONFIG_PATH: &str = "/var/lib/v2board/worker/config.json";
const CURRENT_RELEASE_PATH: &str = "/opt/v2board/current";
const RELEASES_ROOT: &str = "/opt/v2board/releases";
const API_UNIT: &str = "v2board-api.service";
const WORKER_UNIT: &str = "v2board-worker.service";
const WORKER_HEALTH_PATH: &str = "/run/v2board-worker/health";
const SYSTEMD_UNIT_ROOT: &str = "/etc/systemd/system";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeProgram {
    Psql,
    Curl,
    RedisCli,
    Sha256sum,
    SystemdAnalyze,
    Systemctl,
    Chown,
    Id,
}

impl NativeProgram {
    const fn path(self) -> &'static str {
        match self {
            Self::Psql => "/usr/bin/psql",
            Self::Curl => "/usr/bin/curl",
            Self::RedisCli => "/usr/bin/redis-cli",
            Self::Sha256sum => "/usr/bin/sha256sum",
            Self::SystemdAnalyze => "/usr/bin/systemd-analyze",
            Self::Systemctl => "/usr/bin/systemctl",
            Self::Chown => "/usr/bin/chown",
            Self::Id => "/usr/bin/id",
        }
    }
}

pub struct NativeCommandRequest {
    program: NativeProgram,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
    safe_environment: Vec<(OsString, OsString)>,
    secret_environment: Vec<(OsString, String)>,
    stdin: Vec<u8>,
    redactions: Vec<Vec<u8>>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl NativeCommandRequest {
    fn new(program: NativeProgram) -> Self {
        Self {
            program,
            args: Vec::new(),
            current_dir: None,
            safe_environment: Vec::new(),
            secret_environment: Vec::new(),
            stdin: Vec::new(),
            redactions: Vec::new(),
            timeout: DEFAULT_COMMAND_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    fn arg(mut self, value: impl Into<OsString>) -> Self {
        self.args.push(value.into());
        self
    }

    fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(path.into());
        self
    }

    #[cfg(test)]
    fn secret_env(mut self, key: &'static str, value: &str) -> Self {
        self.secret_environment
            .push((OsString::from(key), value.to_string()));
        self.redactions.push(value.as_bytes().to_vec());
        self
    }

    fn safe_env_path(mut self, key: &'static str, value: &Path) -> Self {
        self.safe_environment
            .push((OsString::from(key), value.as_os_str().to_owned()));
        self
    }

    fn stdin(mut self, bytes: Vec<u8>) -> Self {
        self.stdin = bytes;
        self
    }

    fn redact(mut self, value: &str) -> Self {
        if !value.is_empty() {
            self.redactions.push(value.as_bytes().to_vec());
        }
        self
    }

    fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn program(&self) -> NativeProgram {
        self.program
    }

    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    pub fn current_dir_path(&self) -> Option<&Path> {
        self.current_dir.as_deref()
    }

    pub fn stdin_bytes(&self) -> &[u8] {
        &self.stdin
    }

    pub fn contains_secret_environment(&self) -> bool {
        !self.secret_environment.is_empty()
    }

    fn validate(&self) -> Result<(), NativeRunnerError> {
        if self.stdin.len() > MAX_COMMAND_INPUT_BYTES
            || self.args.len() > 128
            || self.safe_environment.len() > 8
            || self.secret_environment.len() > 16
            || self.timeout.is_zero()
            || self.timeout > MAX_COMMAND_TIMEOUT
            || self.max_output_bytes == 0
            || self.max_output_bytes > 1024 * 1024
            || self
                .current_dir
                .as_ref()
                .is_some_and(|path| !path.is_absolute())
        {
            return Err(NativeRunnerError::InvalidRequest);
        }
        for (key, value) in &self.safe_environment {
            if key != "PGPASSFILE" || !Path::new(value).is_absolute() {
                return Err(NativeRunnerError::InvalidRequest);
            }
        }
        for secret in &self.redactions {
            if secret.is_empty() {
                return Err(NativeRunnerError::InvalidRequest);
            }
            for argument in &self.args {
                if argument
                    .as_encoded_bytes()
                    .windows(secret.len())
                    .any(|part| part == secret)
                {
                    return Err(NativeRunnerError::SecretInArgv);
                }
            }
        }
        Ok(())
    }
}

impl Drop for NativeCommandRequest {
    fn drop(&mut self) {
        self.stdin.fill(0);
        for (_, value) in &mut self.secret_environment {
            value.clear();
        }
        for secret in &mut self.redactions {
            secret.fill(0);
        }
    }
}

pub struct NativeCommandOutput {
    pub exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl NativeCommandOutput {
    pub fn new(exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
        }
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NativeRunnerError {
    #[error("invalid fixed command request")]
    InvalidRequest,
    #[error("secret material was detected in argv")]
    SecretInArgv,
    #[error("command binary is missing or unsafe")]
    UnsafeProgram,
    #[error("command spawn failed")]
    Spawn,
    #[error("command stdin write failed")]
    Stdin,
    #[error("command wait failed")]
    Wait,
    #[error("command timed out")]
    Timeout,
    #[error("command output exceeded the bounded limit")]
    OutputLimit,
    #[error("command output reader failed")]
    OutputRead,
}

pub trait NativeCommandRunner {
    fn run(
        &mut self,
        request: NativeCommandRequest,
    ) -> Result<NativeCommandOutput, NativeRunnerError>;
}

#[derive(Default)]
pub struct ProcessCommandRunner;

impl NativeCommandRunner for ProcessCommandRunner {
    fn run(
        &mut self,
        mut request: NativeCommandRequest,
    ) -> Result<NativeCommandOutput, NativeRunnerError> {
        request.validate()?;
        let program = validated_program_path(request.program)?;
        let mut command = Command::new(program);
        command
            .args(&request.args)
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = &request.current_dir {
            command.current_dir(path);
        }
        for (key, value) in &request.secret_environment {
            command.env(key, value);
        }
        for (key, value) in &request.safe_environment {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|_| NativeRunnerError::Spawn)?;
        let mut stdin = child.stdin.take().ok_or(NativeRunnerError::Stdin)?;
        let mut stdin_bytes = std::mem::take(&mut request.stdin);
        let stdin_thread = thread::spawn(move || -> io::Result<()> {
            let result = stdin.write_all(&stdin_bytes).and_then(|()| stdin.flush());
            stdin_bytes.fill(0);
            result
        });
        let stdout = child.stdout.take().ok_or(NativeRunnerError::OutputRead)?;
        let stderr = child.stderr.take().ok_or(NativeRunnerError::OutputRead)?;
        let limit = request.max_output_bytes;
        let stdout_thread = thread::spawn(move || read_bounded(stdout, limit));
        let stderr_thread = thread::spawn(move || read_bounded(stderr, limit));

        let deadline = Instant::now() + request.timeout;
        let status = loop {
            match child.try_wait().map_err(|_| NativeRunnerError::Wait)? {
                Some(status) => break status,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdin_thread.join();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err(NativeRunnerError::Timeout);
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        };
        stdin_thread
            .join()
            .map_err(|_| NativeRunnerError::Stdin)?
            .map_err(|_| NativeRunnerError::Stdin)?;
        let (mut stdout, stdout_overflow) = stdout_thread
            .join()
            .map_err(|_| NativeRunnerError::OutputRead)?
            .map_err(|_| NativeRunnerError::OutputRead)?;
        let (mut stderr, stderr_overflow) = stderr_thread
            .join()
            .map_err(|_| NativeRunnerError::OutputRead)?
            .map_err(|_| NativeRunnerError::OutputRead)?;
        redact_all(&mut stdout, &request.redactions);
        redact_all(&mut stderr, &request.redactions);
        if stdout_overflow || stderr_overflow {
            return Err(NativeRunnerError::OutputLimit);
        }
        Ok(NativeCommandOutput {
            exit_code: status.code().unwrap_or(-1),
            stdout,
            stderr,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalReceiptKind {
    ReleaseArchiveVerified,
    SourceCredentialsRevoked,
    SourceNetworkIsolated,
    LegacyRuntimeCompatibilityDisabled,
    PostgresSoleAuthorityVerified,
    ColdArchiveVerified,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalControlReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: ExternalReceiptKind,
    pub subject_sha256: String,
    pub completed: bool,
    pub evidence_reference: String,
    pub issued_at_unix: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptBinding {
    path: PathBuf,
    sha256: String,
}

impl ReceiptBinding {
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, NativeActivationConfigError> {
        if !path.is_absolute() || !is_lower_sha256(&sha256) {
            return Err(NativeActivationConfigError::InvalidReceiptBinding);
        }
        Ok(Self { path, sha256 })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalReceiptBindings {
    pub release_archive: ReceiptBinding,
    pub source_credentials: ReceiptBinding,
    pub source_network: ReceiptBinding,
    pub runtime_compatibility: ReceiptBinding,
    pub postgres_authority: ReceiptBinding,
    pub cold_archive: ReceiptBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeActivationPolicy {
    release_archive: ReceiptBinding,
    clickhouse_secret_dir: PathBuf,
    #[cfg(test)]
    receipts: ExternalReceiptBindings,
}

impl NativeActivationPolicy {
    #[cfg(test)]
    pub fn new(receipts: ExternalReceiptBindings) -> Result<Self, NativeActivationConfigError> {
        Ok(Self {
            release_archive: receipts.release_archive.clone(),
            clickhouse_secret_dir: PathBuf::from("/run/v2board-lifecycle"),
            receipts,
        })
    }

    /// Narrow policy used by the executable legacy one-shot flow before native
    /// authority is committed.  Only the immutable release receipt is
    /// consumed by target/bootstrap and runtime-materialization methods.  The
    /// duplicated bindings deliberately cannot authorize any of the retired
    /// source-retirement receipt kinds because their embedded kind must still
    /// match during verification.
    pub(crate) fn for_legacy_apply_target(
        release_archive: ReceiptBinding,
    ) -> Result<Self, NativeActivationConfigError> {
        #[cfg(test)]
        let receipts = ExternalReceiptBindings {
            release_archive: release_archive.clone(),
            source_credentials: release_archive.clone(),
            source_network: release_archive.clone(),
            runtime_compatibility: release_archive.clone(),
            postgres_authority: release_archive.clone(),
            cold_archive: release_archive.clone(),
        };
        Ok(Self {
            release_archive,
            clickhouse_secret_dir: PathBuf::from("/run/v2board-lifecycle"),
            #[cfg(test)]
            receipts,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NativeActivationConfigError {
    #[error("external receipt binding must use an absolute path and lowercase SHA-256")]
    InvalidReceiptBinding,
}

pub trait NativeAuthorityCommitter {
    fn verify_gate(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<ActivationCommitGateProof, ExecutorError>;

    fn commit(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<NativeActivationCommitReceipt, ExecutorError>;
}

pub struct DenyNativeAuthorityCommitter;

impl NativeAuthorityCommitter for DenyNativeAuthorityCommitter {
    fn verify_gate(
        &mut self,
        _verification: &OfflineMigrationVerification,
    ) -> Result<ActivationCommitGateProof, ExecutorError> {
        Err(ExecutorError::sanitized(
            "native_authority_committer_missing",
        ))
    }

    fn commit(
        &mut self,
        _verification: &OfflineMigrationVerification,
    ) -> Result<NativeActivationCommitReceipt, ExecutorError> {
        Err(ExecutorError::sanitized(
            "native_authority_committer_missing",
        ))
    }
}

#[cfg(test)]
pub struct LifecycleLedgerAuthorityCommitter {
    pool: PgPool,
    spec: Arc<ProvisionSpec>,
    permit: DurableTargetMutationPermit,
    snapshot: ApplyJournalSnapshot,
    authorization_audit: AuthorizationAuditBinding,
}

#[cfg(test)]
impl LifecycleLedgerAuthorityCommitter {
    pub fn new(
        pool: PgPool,
        spec: Arc<ProvisionSpec>,
        permit: DurableTargetMutationPermit,
        snapshot: ApplyJournalSnapshot,
        authorization_audit: AuthorizationAuditBinding,
    ) -> Result<Self, ExecutorError> {
        if snapshot.state() != ApplyJournalState::Verifying
            || snapshot.checkpoint() != ApplyCheckpoint::NodesVerified
            || snapshot.outcome_code().is_some()
            || snapshot.binding().operation_id() != permit.operation_id()
            || snapshot.binding().inspect_review_sha256() != permit.inspect_review_sha256()
            || snapshot.generation() != permit.generation()
            || snapshot.event_sha256() != permit.event_sha256()
            || snapshot.installation_id() != Some(permit.installation_id())
            || snapshot.backup_restore_proof_sha256() != Some(permit.backup_restore_proof_sha256())
            || snapshot.backup_reference_sha256() != Some(permit.backup_reference_sha256())
            || snapshot.final_recheck_report_sha256() != Some(permit.final_recheck_report_sha256())
            || snapshot.source_fingerprint_sha256() != Some(permit.source_fingerprint_sha256())
            || spec.operation_id != permit.operation_id()
        {
            return Err(ExecutorError::sanitized(
                "authority_committer_binding_invalid",
            ));
        }
        Ok(Self {
            pool,
            spec,
            permit,
            snapshot,
            authorization_audit,
        })
    }

    fn require_verification(
        &self,
        verification: &OfflineMigrationVerification,
    ) -> Result<(), ExecutorError> {
        if verification.operation_id != self.permit.operation_id()
            || verification.installation_id != self.permit.installation_id()
            || verification.inspect_review_sha256 != self.permit.inspect_review_sha256()
            || verification.backup_restore_report_sha256
                != self.permit.backup_restore_proof_sha256()
            || verification.backup_reference_sha256 != self.permit.backup_reference_sha256()
            || verification.final_recheck_report_sha256 != self.permit.final_recheck_report_sha256()
            || verification.source_fingerprint_sha256 != self.permit.source_fingerprint_sha256()
            || verification.target_permit_generation != self.permit.generation()
            || verification.target_permit_event_sha256 != self.permit.event_sha256()
            || !verification.old_writers_fenced
            || !verification.new_writers_still_stopped
        {
            return Err(ExecutorError::sanitized(
                "authority_verification_binding_invalid",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
impl NativeAuthorityCommitter for LifecycleLedgerAuthorityCommitter {
    fn verify_gate(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<ActivationCommitGateProof, ExecutorError> {
        self.require_verification(verification)?;
        let pool = self.pool.clone();
        let operation_id = verification.operation_id.clone();
        let installation_id = verification.installation_id.clone();
        let manifest_binding = verification.manifest_binding_hmac_sha256.clone();
        let inspect_review = verification.inspect_review_sha256.clone();
        let generation = i64::try_from(verification.target_permit_generation)
            .map_err(|_| ExecutorError::sanitized("authority_generation_invalid"))?;
        let event = verification.target_permit_event_sha256.clone();
        let backup = verification.backup_restore_report_sha256.clone();
        let backup_reference = verification.backup_reference_sha256.clone();
        let final_recheck = verification.final_recheck_report_sha256.clone();
        let source_fingerprint = verification.source_fingerprint_sha256.clone();
        let authorization_audit = self.authorization_audit.clone();
        let ready = run_in_isolated_runtime(async move {
            if !migrations_current(&pool).await.map_err(|_| ())? {
                return Ok(false);
            }
            sqlx::query_scalar::<_, bool>(
                r#"
                SELECT COUNT(*) = 1
                FROM v2_lifecycle_operation o
                JOIN v2_system_installation i
                  ON i.installation_id = o.installation_id AND i.singleton = 1
                JOIN v2_lifecycle_event e
                  ON e.operation_id = o.operation_id
                 AND e.generation = o.journal_generation
                 AND e.event_sha256 = o.journal_event_sha256
                WHERE o.operation_id::text = $1
                  AND o.installation_id::text = $2
                  AND o.manifest_binding_hmac_sha256 = $3
                  AND o.inspect_review_sha256 = $4
                  AND o.state = 'verifying' AND o.checkpoint = 11
                  AND o.journal_generation = $5 AND o.journal_event_sha256 = $6
                  AND o.backup_restore_proof_sha256 = $7
                  AND e.backup_reference_sha256 = $8
                  AND o.final_recheck_report_sha256 = $9
                  AND o.source_fingerprint_sha256 = $10
                  AND e.source_fingerprint_sha256 = $10
                  AND o.authorized_snapshot_report_sha256 = $11
                  AND o.authorized_snapshot_report_binding_hmac_sha256 = $12
                  AND o.authorization_binding_hmac_sha256 = $13
                  AND o.authorization_file_sha256 = $14
                  AND i.lineage = 'legacy_migrated' AND i.state = 'pending'
                  AND i.activated_at IS NULL
                "#,
            )
            .bind(operation_id)
            .bind(installation_id)
            .bind(manifest_binding)
            .bind(inspect_review)
            .bind(generation)
            .bind(event)
            .bind(backup)
            .bind(backup_reference)
            .bind(final_recheck)
            .bind(source_fingerprint)
            .bind(authorization_audit.authorized_snapshot_report_sha256())
            .bind(authorization_audit.authorized_snapshot_report_binding_hmac_sha256())
            .bind(authorization_audit.authorization_binding_hmac_sha256())
            .bind(authorization_audit.authorization_file_sha256())
            .fetch_one(&pool)
            .await
            .map_err(|_| ())
        })
        .map_err(|_| ExecutorError::sanitized("authority_gate_database_failed"))?;
        if !ready {
            return Err(ExecutorError::sanitized("authority_gate_not_ready"));
        }
        Ok(ActivationCommitGateProof {
            operation_id: verification.operation_id.clone(),
            installation_id: verification.installation_id.clone(),
            inspect_review_sha256: verification.inspect_review_sha256.clone(),
            backup_restore_report_sha256: verification.backup_restore_report_sha256.clone(),
            backup_reference_sha256: verification.backup_reference_sha256.clone(),
            final_recheck_report_sha256: verification.final_recheck_report_sha256.clone(),
            source_fingerprint_sha256: verification.source_fingerprint_sha256.clone(),
            target_permit_generation: verification.target_permit_generation,
            target_permit_event_sha256: verification.target_permit_event_sha256.clone(),
            durable_journal_in_verifying_state: true,
            postgres_ledger_exactly_current: true,
            clickhouse_ledger_exactly_current: true,
            data_reports_validated_for_commit: true,
            backup_report_bound_in_ledger: true,
            new_writers_still_stopped: true,
        })
    }

    fn commit(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<NativeActivationCommitReceipt, ExecutorError> {
        self.require_verification(verification)?;
        let node = verification
            .node_cutover_report_sha256
            .clone()
            .ok_or_else(|| ExecutorError::sanitized("node_cutover_proof_missing"))?;
        let proof = NativeActivationProofBinding::new(
            verification.data_verification_report_sha256.clone(),
            verification.analytics_projection_report_sha256.clone(),
            node,
        )
        .map_err(|_| ExecutorError::sanitized("activation_proof_binding_invalid"))?;
        let pool = self.pool.clone();
        let spec = Arc::clone(&self.spec);
        let permit = self.permit.clone();
        let snapshot = self.snapshot.clone();
        let authorization_audit = self.authorization_audit.clone();
        let commit = run_in_isolated_runtime(async move {
            commit_native_activation(
                &pool,
                &spec,
                &permit,
                &snapshot,
                &proof,
                &authorization_audit,
            )
            .await
        })
        .map_err(|_| ExecutorError::sanitized("native_activation_ledger_commit_failed"))?;
        if commit.operation_id().to_string() != verification.operation_id
            || commit.installation_id().to_string() != verification.installation_id
            || commit.journal_event_sha256() != verification.target_permit_event_sha256
            || commit.activated_at_unix() <= 0
        {
            return Err(ExecutorError::sanitized(
                "native_activation_commit_receipt_invalid",
            ));
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
            old_writer_fence_still_verified: verification.old_writers_fenced,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedLegacySourceRetirement {
    pub mysql_reachable_with_old_credentials: bool,
    pub source_default_redis_reachable_with_old_credentials: bool,
    pub source_cache_redis_reachable_with_old_credentials: bool,
    pub source_access_permanently_disabled: bool,
    pub mysql_probe_evidence: String,
    pub source_redis_probe_evidence: String,
    pub credential_probe_evidence: String,
}

pub trait LegacySourceRetirementObserver: Send {
    fn observe(
        &mut self,
        request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<ObservedLegacySourceRetirement, ExecutorError>;
}

pub struct DenyLegacySourceRetirementObserver;

impl LegacySourceRetirementObserver for DenyLegacySourceRetirementObserver {
    fn observe(
        &mut self,
        _request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<ObservedLegacySourceRetirement, ExecutorError> {
        Err(ExecutorError::sanitized(
            "legacy_source_retirement_observer_missing",
        ))
    }
}

#[derive(Clone)]
struct PostgresConnection {
    safe_url: String,
    password: String,
    username: String,
    database: String,
    host: String,
    port: u16,
}

impl Drop for PostgresConnection {
    fn drop(&mut self) {
        self.password.clear();
    }
}

#[derive(Clone)]
struct ClickHouseConnection {
    endpoint: String,
    database: String,
    username: String,
    password: String,
}

impl Drop for ClickHouseConnection {
    fn drop(&mut self) {
        self.password.clear();
    }
}

#[derive(Clone)]
struct ClickHouseTargetBinding {
    database: String,
    #[cfg(test)]
    raw_retention_days: u32,
    #[cfg(test)]
    aggregate_retention_days: u32,
    bootstrap: ClickHouseConnection,
    schema: ClickHousePrincipalBinding,
    writer: ClickHousePrincipalBinding,
    reader: ClickHousePrincipalBinding,
}

#[derive(Clone)]
struct ClickHousePrincipalBinding {
    username: String,
    password: String,
}

#[derive(Clone, Copy)]
struct PostgresObservedState {
    roles_exist: bool,
    database_exists: bool,
}

#[derive(Clone, Copy)]
struct ClickHouseObservedState {
    database_exists: bool,
    schema_exists: bool,
    writer_exists: bool,
    reader_exists: bool,
}

impl Drop for ClickHousePrincipalBinding {
    fn drop(&mut self) {
        self.password.clear();
    }
}

pub struct BareMetalActivationExecutor<R, C> {
    runner: R,
    _authority_committer: C,
    #[cfg(test)]
    source_retirement_observer: Box<dyn LegacySourceRetirementObserver>,
    permit: DurableTargetMutationPermit,
    policy: NativeActivationPolicy,
    postgres_migration: Option<PostgresConnection>,
    postgres_observed_state: Option<PostgresObservedState>,
    clickhouse_target: Option<ClickHouseTargetBinding>,
    clickhouse_observed_state: Option<ClickHouseObservedState>,
    target_redis_url: Option<String>,
    verified_release: Option<(String, String)>,
    _started_units: BTreeSet<String>,
}

impl<R: NativeCommandRunner, C: NativeAuthorityCommitter> BareMetalActivationExecutor<R, C> {
    pub fn new(
        runner: R,
        authority_committer: C,
        _source_retirement_observer: Box<dyn LegacySourceRetirementObserver>,
        permit: DurableTargetMutationPermit,
        policy: NativeActivationPolicy,
    ) -> Result<Self, ExecutorError> {
        if permit.generation() == 0
            || !is_lower_sha256(permit.event_sha256())
            || !is_lower_sha256(permit.inspect_review_sha256())
            || !is_lower_sha256(permit.backup_restore_proof_sha256())
            || !is_lower_sha256(permit.backup_reference_sha256())
            || !is_lower_sha256(permit.final_recheck_report_sha256())
            || !is_lower_sha256(permit.source_fingerprint_sha256())
        {
            return Err(ExecutorError::sanitized("invalid_target_mutation_permit"));
        }
        Ok(Self {
            runner,
            _authority_committer: authority_committer,
            #[cfg(test)]
            source_retirement_observer: _source_retirement_observer,
            permit,
            policy,
            postgres_migration: None,
            postgres_observed_state: None,
            clickhouse_target: None,
            clickhouse_observed_state: None,
            target_redis_url: None,
            verified_release: None,
            _started_units: BTreeSet::new(),
        })
    }

    pub fn replace_permit(
        &mut self,
        permit: DurableTargetMutationPermit,
    ) -> Result<(), ExecutorError> {
        if permit.operation_id() != self.permit.operation_id()
            || permit.installation_id() != self.permit.installation_id()
            || permit.inspect_review_sha256() != self.permit.inspect_review_sha256()
            || permit.backup_restore_proof_sha256() != self.permit.backup_restore_proof_sha256()
            || permit.backup_reference_sha256() != self.permit.backup_reference_sha256()
            || permit.final_recheck_report_sha256() != self.permit.final_recheck_report_sha256()
            || permit.source_fingerprint_sha256() != self.permit.source_fingerprint_sha256()
            || permit.generation() < self.permit.generation()
            || !is_lower_sha256(permit.event_sha256())
        {
            return Err(ExecutorError::sanitized(
                "replacement_permit_binding_mismatch",
            ));
        }
        self.permit = permit;
        Ok(())
    }

    fn require_operation(&self, operation_id: &str) -> Result<(), ExecutorError> {
        if operation_id != self.permit.operation_id() {
            return Err(ExecutorError::sanitized("operation_binding_mismatch"));
        }
        Ok(())
    }

    fn require_permit(&self, permit: &DurableTargetMutationPermit) -> Result<(), ExecutorError> {
        if permit.operation_id() != self.permit.operation_id()
            || permit.installation_id() != self.permit.installation_id()
            || permit.inspect_review_sha256() != self.permit.inspect_review_sha256()
            || permit.backup_restore_proof_sha256() != self.permit.backup_restore_proof_sha256()
            || permit.backup_reference_sha256() != self.permit.backup_reference_sha256()
            || permit.final_recheck_report_sha256() != self.permit.final_recheck_report_sha256()
            || permit.source_fingerprint_sha256() != self.permit.source_fingerprint_sha256()
            || permit.generation() != self.permit.generation()
            || permit.event_sha256() != self.permit.event_sha256()
        {
            return Err(ExecutorError::sanitized("target_permit_binding_mismatch"));
        }
        Ok(())
    }

    #[cfg(test)]
    fn require_verification_binding(
        &self,
        verification: &OfflineMigrationVerification,
    ) -> Result<(), ExecutorError> {
        if verification.operation_id != self.permit.operation_id()
            || verification.installation_id != self.permit.installation_id()
            || verification.inspect_review_sha256 != self.permit.inspect_review_sha256()
            || verification.backup_restore_report_sha256
                != self.permit.backup_restore_proof_sha256()
            || verification.backup_reference_sha256 != self.permit.backup_reference_sha256()
            || verification.final_recheck_report_sha256 != self.permit.final_recheck_report_sha256()
            || verification.source_fingerprint_sha256 != self.permit.source_fingerprint_sha256()
            || verification.target_permit_generation != self.permit.generation()
            || verification.target_permit_event_sha256 != self.permit.event_sha256()
        {
            return Err(ExecutorError::sanitized(
                "activation_verification_binding_mismatch",
            ));
        }
        Ok(())
    }

    fn run_success(
        &mut self,
        request: NativeCommandRequest,
        error_code: &'static str,
    ) -> Result<NativeCommandOutput, ExecutorError> {
        let output = self
            .runner
            .run(request)
            .map_err(|error| runner_executor_error(error, error_code))?;
        if output.exit_code != 0 {
            return Err(ExecutorError::sanitized(error_code));
        }
        Ok(output)
    }

    fn psql(
        &mut self,
        connection: &PostgresConnection,
        sql: Vec<u8>,
        redactions: &[&str],
        error_code: &'static str,
    ) -> Result<String, ExecutorError> {
        let secret_path = self.write_postgres_secret_file(connection)?;
        let mut request = NativeCommandRequest::new(NativeProgram::Psql)
            .arg("--no-psqlrc")
            .arg("--set=ON_ERROR_STOP=1")
            .arg("--tuples-only")
            .arg("--no-align")
            .arg("--field-separator=\t")
            .arg("--quiet")
            .arg("--dbname")
            .arg(&connection.safe_url)
            .arg("--file=-")
            .safe_env_path("PGPASSFILE", &secret_path)
            .redact(&connection.password)
            .stdin(sql);
        for secret in redactions {
            request = request.redact(secret);
        }
        let result = self
            .run_success(request, error_code)
            .and_then(|output| strict_output_text(output.stdout(), error_code));
        let cleanup = securely_remove_secret_file(&secret_path);
        match (result, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn write_postgres_secret_file(
        &self,
        connection: &PostgresConnection,
    ) -> Result<PathBuf, ExecutorError> {
        prepare_private_runtime_dir(&self.policy.clickhouse_secret_dir)?;
        let path = self.policy.clickhouse_secret_dir.join(format!(
            "pgpass-{}-{}.conf",
            self.permit.operation_id(),
            std::process::id()
        ));
        securely_remove_secret_file_if_present(&path)?;
        let escape = |value: &str| value.replace('\\', "\\\\").replace(':', "\\:");
        let contents = format!(
            "{}:{}:{}:{}:{}\n",
            escape(&connection.host),
            connection.port,
            escape(&connection.database),
            escape(&connection.username),
            escape(&connection.password),
        );
        let mut bytes = contents.into_bytes();
        let result = write_new_owner_file(&path, &bytes, 0o600)
            .map_err(|_| ExecutorError::sanitized("postgres_secret_file_write_failed"));
        bytes.fill(0);
        result?;
        Ok(path)
    }

    fn clickhouse(
        &mut self,
        connection: &ClickHouseConnection,
        sql: Vec<u8>,
        redactions: &[&str],
        error_code: &'static str,
    ) -> Result<String, ExecutorError> {
        let secret_path = self.write_clickhouse_secret_file(connection)?;
        let endpoint = format!(
            "{}/?database={}&default_format=TSVRaw&multiquery=1",
            connection.endpoint.trim_end_matches('/'),
            connection.database
        );
        let mut request = NativeCommandRequest::new(NativeProgram::Curl)
            .arg("--config")
            .arg(&secret_path)
            .arg("--silent")
            .arg("--show-error")
            .arg("--fail-with-body")
            .arg("--proto")
            .arg("=https")
            .arg("--tlsv1.2")
            .arg("--data-binary")
            .arg("@-")
            .arg(endpoint)
            .stdin(sql)
            .redact(&connection.password);
        for secret in redactions {
            request = request.redact(secret);
        }
        let result = self
            .run_success(request, error_code)
            .and_then(|output| strict_output_text(output.stdout(), error_code));
        let cleanup = securely_remove_secret_file(&secret_path);
        match (result, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn write_clickhouse_secret_file(
        &self,
        connection: &ClickHouseConnection,
    ) -> Result<PathBuf, ExecutorError> {
        prepare_private_runtime_dir(&self.policy.clickhouse_secret_dir)?;
        let path = self.policy.clickhouse_secret_dir.join(format!(
            "curl-{}-{}.conf",
            self.permit.operation_id(),
            std::process::id()
        ));
        securely_remove_secret_file_if_present(&path)?;
        let credential = format!("{}:{}", connection.username, connection.password);
        let escaped = curl_config_escape(&credential)?;
        let contents = format!("user = \"{escaped}\"\n");
        write_new_owner_file(&path, contents.as_bytes(), 0o600)
            .map_err(|_| ExecutorError::sanitized("clickhouse_secret_file_write_failed"))?;
        Ok(path)
    }

    fn redis_command(
        &mut self,
        redis_url: &str,
        arguments: &[&str],
        error_code: &'static str,
    ) -> Result<String, ExecutorError> {
        let redis_url = redis_url.to_string();
        let arguments = arguments
            .iter()
            .copied()
            .filter(|argument| *argument != "--raw")
            .map(str::to_string)
            .collect::<Vec<_>>();
        run_in_isolated_runtime(async move {
            let output = tokio::time::timeout(Duration::from_secs(10), async move {
                let client = redis::Client::open(redis_url)
                    .map_err(|_| ExecutorError::sanitized("redis_url_invalid"))?;
                let mut connection = client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|_| ExecutorError::sanitized(error_code))?;
                match arguments.as_slice() {
                    [command] if command == "DBSIZE" => redis::cmd("DBSIZE")
                        .query_async::<u64>(&mut connection)
                        .await
                        .map(|value| value.to_string())
                        .map_err(|_| ExecutorError::sanitized(error_code)),
                    [command, section] if command == "INFO" && section == "server" => {
                        redis::cmd("INFO")
                            .arg("server")
                            .query_async::<String>(&mut connection)
                            .await
                            .map_err(|_| ExecutorError::sanitized(error_code))
                    }
                    [command, subcommand, name] if command == "COMMAND" && subcommand == "INFO" => {
                        let value = redis::cmd("COMMAND")
                            .arg("INFO")
                            .arg(name)
                            .query_async::<redis::Value>(&mut connection)
                            .await
                            .map_err(|_| ExecutorError::sanitized(error_code))?;
                        if matches!(value, redis::Value::Nil) {
                            return Err(ExecutorError::sanitized(error_code));
                        }
                        Ok(format!("{value:?}"))
                    }
                    _ => Err(ExecutorError::sanitized("redis_command_not_allowed")),
                }
            })
            .await
            .map_err(|_| ExecutorError::sanitized(error_code))??;
            if output.len() > DEFAULT_MAX_OUTPUT_BYTES {
                return Err(ExecutorError::sanitized("redis_output_limit"));
            }
            Ok(output)
        })
        .map_err(|_| ExecutorError::sanitized(error_code))
    }

    fn receipt(
        &self,
        binding: &ReceiptBinding,
        expected_kind: ExternalReceiptKind,
        expected_subject_sha256: &str,
    ) -> Result<ExternalControlReceipt, ExecutorError> {
        load_external_receipt(
            binding,
            self.permit.operation_id(),
            expected_kind,
            expected_subject_sha256,
        )
    }
}

impl<R: NativeCommandRunner, C: NativeAuthorityCommitter> TargetActivationExecutor
    for BareMetalActivationExecutor<R, C>
{
    fn inspect_empty_postgres(
        &mut self,
        operation_id: &str,
        target: &PostgresTargetSpec,
    ) -> Result<PostgresEmptyProof, ExecutorError> {
        self.require_operation(operation_id)?;
        let bootstrap = PostgresConnection::parse(&target.bootstrap_database_url)?;
        let migration = PostgresConnection::parse(&target.migration_database_url)?;
        let api = PostgresConnection::parse(&target.api_database_url)?;
        let worker = PostgresConnection::parse(&target.worker_database_url)?;
        let marker = operation_marker(&self.permit);
        let database_marker = format!("{marker}:postgres-database:{}", migration.database);
        let migration_marker = format!("{marker}:postgres-role:{}", migration.username);
        let api_marker = format!("{marker}:postgres-role:{}", api.username);
        let worker_marker = format!("{marker}:postgres-role:{}", worker.username);
        let sql = format!(
            "SELECT current_setting('server_version_num'),\
             (SELECT count(*) FROM pg_database WHERE datname = {}),\
             COALESCE((SELECT pg_get_userbyid(datdba) FROM pg_database WHERE datname = {}), ''),\
             COALESCE((SELECT shobj_description(oid, 'pg_database') FROM pg_database WHERE datname = {}), ''),\
             (SELECT count(*) FROM pg_roles WHERE rolname = {}),\
             COALESCE((SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = {}), ''),\
             (SELECT count(*) FROM pg_roles WHERE rolname = {}),\
             COALESCE((SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = {}), ''),\
             (SELECT count(*) FROM pg_roles WHERE rolname = {}),\
             COALESCE((SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = {}), ''),\
             has_database_privilege(current_user, current_database(), 'CREATE'),\
             (SELECT rolcreaterole FROM pg_roles WHERE rolname = current_user),\
             current_setting('fsync') = 'on',\
             current_setting('full_page_writes') = 'on',\
             current_setting('synchronous_commit') IN ('on', 'remote_apply'),\
             current_setting('data_checksums') = 'on',\
             COALESCE((SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()), FALSE),\
             current_setting('wal_level') IN ('replica', 'logical'),\
             current_setting('archive_mode') IN ('on', 'always'),\
             ((lower(trim(current_setting('archive_library', TRUE))) NOT IN ('', 'disabled', '(disabled)'))\
               OR (lower(trim(current_setting('archive_command', TRUE))) NOT IN ('', 'disabled', '(disabled)')));\n",
            pg_literal(&migration.database),
            pg_literal(&migration.database),
            pg_literal(&migration.database),
            pg_literal(&migration.username),
            pg_literal(&migration.username),
            pg_literal(&api.username),
            pg_literal(&api.username),
            pg_literal(&worker.username),
            pg_literal(&worker.username),
        );
        let output = self.psql(
            &bootstrap,
            sql.into_bytes(),
            &[],
            "postgres_empty_probe_failed",
        )?;
        let fields = one_tsv_row(&output, 20, "postgres_empty_probe_invalid")?;
        let version_number = parse_u64(fields[0], "postgres_version_invalid")?;
        let database_absent = parse_u64(fields[1], "postgres_database_count_invalid")? == 0;
        let migration_role_absent = parse_u64(fields[4], "postgres_role_count_invalid")? == 0;
        let api_role_absent = parse_u64(fields[6], "postgres_role_count_invalid")? == 0;
        let worker_role_absent = parse_u64(fields[8], "postgres_role_count_invalid")? == 0;
        let can_create_database = parse_pg_bool(fields[10])?;
        let can_create_roles = parse_pg_bool(fields[11])?;
        let fsync_on = parse_pg_bool(fields[12])?;
        let full_page_writes_on = parse_pg_bool(fields[13])?;
        let synchronous_commit_on = parse_pg_bool(fields[14])?;
        let data_checksums_on = parse_pg_bool(fields[15])?;
        let tls_session_verified = parse_pg_bool(fields[16])?;
        let wal_level_replica_or_logical = parse_pg_bool(fields[17])?;
        let archive_mode_on_or_always = parse_pg_bool(fields[18])?;
        let archive_command_or_library_enabled = parse_pg_bool(fields[19])?;
        let roles_all_absent = migration_role_absent && api_role_absent && worker_role_absent;
        let roles_all_owned = !migration_role_absent
            && !api_role_absent
            && !worker_role_absent
            && fields[5] == migration_marker.as_str()
            && fields[7] == api_marker.as_str()
            && fields[9] == worker_marker.as_str();
        let database_owned = database_absent
            || (fields[2] == migration.username.as_str()
                && (fields[3].is_empty() || fields[3] == database_marker.as_str()));
        let entirely_absent = database_absent && roles_all_absent;
        let recoverable_owned = !entirely_absent && roles_all_owned && database_owned;
        self.postgres_observed_state =
            (entirely_absent || recoverable_owned).then_some(PostgresObservedState {
                roles_exist: roles_all_owned,
                database_exists: !database_absent,
            });
        self.postgres_migration = Some(migration);
        Ok(PostgresEmptyProof {
            server_major: u16::try_from(version_number / 10_000).unwrap_or(0),
            fsync_on,
            full_page_writes_on,
            synchronous_commit_on,
            data_checksums_on,
            tls_session_verified,
            wal_level_replica_or_logical,
            archive_mode_on_or_always,
            archive_command_or_library_enabled,
            database_absent,
            migration_role_absent,
            api_role_absent,
            worker_role_absent,
            recoverable_objects_owned_by_operation: recoverable_owned,
            bootstrap_capability_verified: bootstrap.username
                == PostgresConnection::parse(&target.bootstrap_database_url)?.username
                && can_create_database
                && can_create_roles,
            external_access_evidence_verified: target.external_access.pg_hba_managed_externally
                && target.external_access.network_policy_managed_externally
                && valid_evidence(&target.external_access.pg_hba_evidence)
                && valid_evidence(&target.external_access.network_policy_evidence),
        })
    }

    fn create_postgres_database_and_roles(
        &mut self,
        permit: &DurableTargetMutationPermit,
        target: &PostgresTargetSpec,
    ) -> Result<TargetCreationReceipt, ExecutorError> {
        self.require_permit(permit)?;
        ensure_effective_root(&mut self.runner)?;
        let bootstrap = PostgresConnection::parse(&target.bootstrap_database_url)?;
        let migration = PostgresConnection::parse(&target.migration_database_url)?;
        let api = PostgresConnection::parse(&target.api_database_url)?;
        let worker = PostgresConnection::parse(&target.worker_database_url)?;
        let observed = self
            .postgres_observed_state
            .ok_or_else(|| ExecutorError::sanitized("postgres_precreate_state_missing"))?;
        let sql = postgres_bootstrap_sql(permit, &migration, &api, &worker, observed);
        self.psql(
            &bootstrap,
            sql,
            &[&migration.password, &api.password, &worker.password],
            "postgres_target_create_failed",
        )?;
        self.postgres_migration = Some(migration);
        Ok(creation_receipt(permit))
    }

    fn verify_postgres_database_and_roles(
        &mut self,
        operation_id: &str,
        target: &PostgresTargetSpec,
    ) -> Result<PostgresReadyProof, ExecutorError> {
        self.require_operation(operation_id)?;
        let migration = PostgresConnection::parse(&target.migration_database_url)?;
        let api = PostgresConnection::parse(&target.api_database_url)?;
        let worker = PostgresConnection::parse(&target.worker_database_url)?;
        let marker = operation_marker(&self.permit);
        let sql = format!(
            "SELECT current_setting('server_version_num'),\
             current_setting('lc_collate'), current_setting('lc_ctype'),\
             current_user = {},\
             (SELECT rolcanlogin AND NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolreplication AND NOT rolbypassrls \
                AND NOT EXISTS (SELECT 1 FROM pg_auth_members WHERE member = pg_roles.oid) FROM pg_roles WHERE rolname = {}),\
             (SELECT rolcanlogin AND NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolreplication AND NOT rolbypassrls \
                AND NOT EXISTS (SELECT 1 FROM pg_auth_members WHERE member = pg_roles.oid) FROM pg_roles WHERE rolname = {}),\
             (SELECT rolcanlogin AND NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolreplication AND NOT rolbypassrls \
                AND NOT EXISTS (SELECT 1 FROM pg_auth_members WHERE member = pg_roles.oid) FROM pg_roles WHERE rolname = {}),\
             has_database_privilege({}, current_database(), 'CREATE'),\
             (NOT has_database_privilege({}, current_database(), 'CREATE') \
                AND has_database_privilege({}, current_database(), 'CONNECT') \
                AND NOT has_database_privilege({}, current_database(), 'TEMPORARY')),\
             (NOT has_database_privilege({}, current_database(), 'CREATE') \
                AND has_database_privilege({}, current_database(), 'CONNECT') \
                AND NOT has_database_privilege({}, current_database(), 'TEMPORARY')),\
             (NOT has_schema_privilege({}, 'public', 'CREATE') \
                AND has_schema_privilege({}, 'public', 'USAGE')),\
             (NOT has_schema_privilege({}, 'public', 'CREATE') \
                AND has_schema_privilege({}, 'public', 'USAGE')),\
             COALESCE((SELECT shobj_description(oid, 'pg_database') FROM pg_database WHERE datname = current_database()), ''),\
             COALESCE((SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = {}), ''),\
             COALESCE((SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = {}), ''),\
             COALESCE((SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = {}), ''),\
             current_setting('fsync') = 'on',\
             current_setting('full_page_writes') = 'on',\
             current_setting('synchronous_commit') IN ('on', 'remote_apply'),\
             current_setting('data_checksums') = 'on',\
             COALESCE((SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()), FALSE),\
             current_setting('wal_level') IN ('replica', 'logical'),\
             current_setting('archive_mode') IN ('on', 'always'),\
             ((lower(trim(current_setting('archive_library', TRUE))) NOT IN ('', 'disabled', '(disabled)'))\
               OR (lower(trim(current_setting('archive_command', TRUE))) NOT IN ('', 'disabled', '(disabled)')));\n",
            pg_literal(&migration.username),
            pg_literal(&migration.username),
            pg_literal(&api.username),
            pg_literal(&worker.username),
            pg_literal(&migration.username),
            pg_literal(&api.username),
            pg_literal(&api.username),
            pg_literal(&api.username),
            pg_literal(&worker.username),
            pg_literal(&worker.username),
            pg_literal(&worker.username),
            pg_literal(&api.username),
            pg_literal(&api.username),
            pg_literal(&worker.username),
            pg_literal(&worker.username),
            pg_literal(&migration.username),
            pg_literal(&api.username),
            pg_literal(&worker.username),
        );
        let output = self.psql(
            &migration,
            sql.into_bytes(),
            &[],
            "postgres_target_verify_failed",
        )?;
        let fields = one_tsv_row(&output, 24, "postgres_target_verify_invalid")?;
        let version_number = parse_u64(fields[0], "postgres_version_invalid")?;
        let migration_identity = parse_pg_bool(fields[3])?;
        let migration_restricted = parse_pg_bool(fields[4])?;
        let api_restricted = parse_pg_bool(fields[5])?;
        let worker_restricted = parse_pg_bool(fields[6])?;
        let migration_can_ddl = parse_pg_bool(fields[7])?;
        let api_no_db_create = parse_pg_bool(fields[8])?;
        let worker_no_db_create = parse_pg_bool(fields[9])?;
        let api_no_schema_create = parse_pg_bool(fields[10])?;
        let worker_no_schema_create = parse_pg_bool(fields[11])?;
        let database_marker = format!("{marker}:postgres-database:{}", migration.database);
        let role_markers = [
            format!("{marker}:postgres-role:{}", migration.username),
            format!("{marker}:postgres-role:{}", api.username),
            format!("{marker}:postgres-role:{}", worker.username),
        ];
        let database_owned = migration_identity && fields[12] == database_marker.as_str();
        let roles_owned = fields[13] == role_markers[0]
            && fields[14] == role_markers[1]
            && fields[15] == role_markers[2];
        let fsync_on = parse_pg_bool(fields[16])?;
        let full_page_writes_on = parse_pg_bool(fields[17])?;
        let synchronous_commit_on = parse_pg_bool(fields[18])?;
        let data_checksums_on = parse_pg_bool(fields[19])?;
        let tls_session_verified = parse_pg_bool(fields[20])?;
        let wal_level_replica_or_logical = parse_pg_bool(fields[21])?;
        let archive_mode_on_or_always = parse_pg_bool(fields[22])?;
        let archive_command_or_library_enabled = parse_pg_bool(fields[23])?;

        let acl_roles = RuntimeRoleNames {
            migration: migration.username.clone(),
            api: api.username.clone(),
            worker: worker.username.clone(),
        };
        let acl_output = self.psql(
            &migration,
            runtime_acl_catalog_sql("public", &acl_roles).into_bytes(),
            &[],
            "postgres_runtime_acl_verify_failed",
        )?;
        let acl_fields = one_tsv_row(&acl_output, 6, "postgres_runtime_acl_verify_invalid")?;
        let runtime_acl_schema_state = match RuntimeAclSchemaState::parse(acl_fields[0])
            .ok_or_else(|| ExecutorError::sanitized("postgres_runtime_acl_state_invalid"))?
        {
            RuntimeAclSchemaState::Empty => PostgresRuntimeAclSchemaState::Empty,
            RuntimeAclSchemaState::FrozenBaseline => PostgresRuntimeAclSchemaState::FrozenBaseline,
            RuntimeAclSchemaState::Drifted => PostgresRuntimeAclSchemaState::Drifted,
        };
        let runtime_table_acl_exact = parse_pg_bool(acl_fields[1])?;
        let protected_table_acl_exact = parse_pg_bool(acl_fields[2])?;
        let runtime_sequence_acl_minimal = parse_pg_bool(acl_fields[3])?;
        let runtime_default_acl_fail_closed = parse_pg_bool(acl_fields[4])?;
        let runtime_boundary_acl_exact = parse_pg_bool(acl_fields[5])?;
        self.postgres_migration = Some(migration);
        Ok(PostgresReadyProof {
            server_major: u16::try_from(version_number / 10_000).unwrap_or(0),
            fsync_on,
            full_page_writes_on,
            synchronous_commit_on,
            data_checksums_on,
            tls_session_verified,
            wal_level_replica_or_logical,
            archive_mode_on_or_always,
            archive_command_or_library_enabled,
            database_owned_by_operation: database_owned,
            roles_owned_by_operation: roles_owned
                && migration_restricted
                && api_restricted
                && worker_restricted,
            collation: fields[1].to_string(),
            ctype: fields[2].to_string(),
            principals_distinct: acl_roles.migration != acl_roles.api
                && acl_roles.migration != acl_roles.worker
                && acl_roles.api != acl_roles.worker,
            migration_role_is_ddl_only: migration_restricted && migration_can_ddl,
            api_role_is_api_dml_only: api_restricted && api_no_db_create && api_no_schema_create,
            worker_role_is_worker_dml_only: worker_restricted
                && worker_no_db_create
                && worker_no_schema_create,
            runtime_acl_schema_state,
            runtime_table_acl_exact,
            protected_table_acl_exact,
            runtime_sequence_acl_minimal,
            runtime_default_acl_fail_closed,
            runtime_boundary_acl_exact,
            bootstrap_role_absent_from_runtime: true,
        })
    }

    fn inspect_empty_clickhouse(
        &mut self,
        operation_id: &str,
        target: &ClickHouseTargetSpec,
    ) -> Result<ClickHouseEmptyProof, ExecutorError> {
        self.require_operation(operation_id)?;
        let binding = ClickHouseTargetBinding::from_target(target);
        let marker = operation_marker(&self.permit);
        let sql = format!(
            "SELECT version(), currentUser(),\
             (SELECT count() FROM system.databases WHERE name = {}),\
             COALESCE((SELECT comment FROM system.databases WHERE name = {}), ''),\
             (SELECT count() FROM system.users WHERE name = {}),\
             COALESCE((SELECT comment FROM system.users WHERE name = {}), ''),\
             (SELECT count() FROM system.users WHERE name = {}),\
             COALESCE((SELECT comment FROM system.users WHERE name = {}), ''),\
             (SELECT count() FROM system.users WHERE name = {}),\
             COALESCE((SELECT comment FROM system.users WHERE name = {}), ''),\
             (SELECT count() FROM system.replicas),\
             (SELECT uniqExact(cluster) FROM system.clusters),\
             (SELECT countIf(grant_option = 1 AND access_type IN ('ALL', 'CREATE', 'CREATE DATABASE')) > 0 FROM system.grants WHERE user_name = currentUser()),\
             (SELECT countIf(grant_option = 1 AND access_type IN ('ALL', 'ACCESS MANAGEMENT', 'CREATE USER')) > 0 FROM system.grants WHERE user_name = currentUser()),\
             (SELECT countIf(grant_option = 1 AND access_type IN ('ALL', 'ACCESS MANAGEMENT', 'CREATE ROLE')) > 0 FROM system.grants WHERE user_name = currentUser()) FORMAT TSVRaw;",
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.schema.username),
            ch_literal(&binding.schema.username),
            ch_literal(&binding.writer.username),
            ch_literal(&binding.writer.username),
            ch_literal(&binding.reader.username),
            ch_literal(&binding.reader.username),
        );
        let output = self.clickhouse(
            &binding.bootstrap,
            sql.into_bytes(),
            &[],
            "clickhouse_empty_probe_failed",
        )?;
        let fields = one_tsv_row(&output, 15, "clickhouse_empty_probe_invalid")?;
        let (major, minor) = version_family(fields[0])?;
        let database_absent = parse_u64(fields[2], "clickhouse_database_count_invalid")? == 0;
        let schema_absent = parse_u64(fields[4], "clickhouse_user_count_invalid")? == 0;
        let writer_absent = parse_u64(fields[6], "clickhouse_user_count_invalid")? == 0;
        let reader_absent = parse_u64(fields[8], "clickhouse_user_count_invalid")? == 0;
        let database_marker = format!("{marker}:clickhouse-database:{}", binding.database);
        let schema_marker = format!("{marker}:clickhouse-user:{}", binding.schema.username);
        let writer_marker = format!("{marker}:clickhouse-user:{}", binding.writer.username);
        let reader_marker = format!("{marker}:clickhouse-user:{}", binding.reader.username);
        let database_owned = database_absent || fields[3] == database_marker.as_str();
        let schema_owned = schema_absent || fields[5] == schema_marker.as_str();
        let writer_owned = writer_absent || fields[7] == writer_marker.as_str();
        let reader_owned = reader_absent || fields[9] == reader_marker.as_str();
        let entirely_absent = database_absent && schema_absent && writer_absent && reader_absent;
        let recoverable_owned =
            !entirely_absent && database_owned && schema_owned && writer_owned && reader_owned;
        let standalone = parse_u64(fields[10], "clickhouse_replica_count_invalid")? == 0
            && parse_u64(fields[11], "clickhouse_cluster_count_invalid")? == 0;
        let bootstrap_capability = fields[1] == binding.bootstrap.username
            && parse_ch_bool(fields[12])?
            && parse_ch_bool(fields[13])?
            && parse_ch_bool(fields[14])?;
        self.clickhouse_observed_state =
            (entirely_absent || recoverable_owned).then_some(ClickHouseObservedState {
                database_exists: !database_absent,
                schema_exists: !schema_absent,
                writer_exists: !writer_absent,
                reader_exists: !reader_absent,
            });
        self.clickhouse_target = Some(binding);
        Ok(ClickHouseEmptyProof {
            server_major: major,
            server_minor: minor,
            database_absent,
            schema_principal_absent: schema_absent,
            writer_principal_absent: writer_absent,
            reader_principal_absent: reader_absent,
            recoverable_objects_owned_by_operation: recoverable_owned,
            bootstrap_capability_verified: bootstrap_capability,
            standalone_non_replicated: standalone,
            network_policy_evidence_verified: valid_evidence(&target.network_policy_evidence),
        })
    }

    fn create_clickhouse_database_and_roles(
        &mut self,
        permit: &DurableTargetMutationPermit,
        target: &ClickHouseTargetSpec,
    ) -> Result<TargetCreationReceipt, ExecutorError> {
        self.require_permit(permit)?;
        ensure_effective_root(&mut self.runner)?;
        let binding = ClickHouseTargetBinding::from_target(target);
        let observed = self
            .clickhouse_observed_state
            .ok_or_else(|| ExecutorError::sanitized("clickhouse_precreate_state_missing"))?;
        let sql = clickhouse_bootstrap_sql(permit, &binding, observed);
        self.clickhouse(
            &binding.bootstrap,
            sql,
            &[
                &binding.schema.password,
                &binding.writer.password,
                &binding.reader.password,
            ],
            "clickhouse_target_create_failed",
        )?;
        self.clickhouse_target = Some(binding);
        Ok(creation_receipt(permit))
    }

    fn verify_clickhouse_database_and_roles(
        &mut self,
        operation_id: &str,
        target: &ClickHouseTargetSpec,
    ) -> Result<ClickHouseReadyProof, ExecutorError> {
        self.require_operation(operation_id)?;
        let binding = ClickHouseTargetBinding::from_target(target);
        let marker = operation_marker(&self.permit);
        let sql = format!(
            "SELECT version(),\
             (SELECT count() FROM system.databases WHERE name = {}) = 1,\
             COALESCE((SELECT comment FROM system.databases WHERE name = {}), ''),\
             (SELECT count() FROM system.users WHERE name IN ({}, {}, {})) = 3,\
             COALESCE((SELECT comment FROM system.users WHERE name = {}), ''),\
             COALESCE((SELECT comment FROM system.users WHERE name = {}), ''),\
             COALESCE((SELECT comment FROM system.users WHERE name = {}), ''),\
             (SELECT count() FROM system.replicas) = 0,\
             (SELECT uniqExact(cluster) FROM system.clusters) = 0,\
             (SELECT countIf(access_type IN ('ALL', 'ACCESS MANAGEMENT', 'CREATE USER', 'CREATE ROLE')) = 0\
                     AND uniqExactIf(access_type, database = {} AND access_type IN ('CREATE TABLE', 'ALTER TABLE', 'DROP TABLE', 'SELECT', 'INSERT')) = 5\
                     AND countIf(database = {} AND access_type = 'INSERT' AND table IN ('v2_schema_migration', 'v2_installation_binding', 'v2_retention_binding')) >= 3\
                     AND countIf(database = {} AND access_type = 'INSERT' AND table NOT IN ('v2_schema_migration', 'v2_installation_binding', 'v2_retention_binding')) = 0\
                     AND countIf(database = 'system' AND access_type = 'SELECT') > 0\
                FROM system.grants WHERE user_name = {}),\
             (SELECT countIf(access_type NOT IN ('USAGE', 'INSERT', 'SELECT') OR (database IS NOT NULL AND database != {})) = 0\
                     AND countIf(database = {} AND table IN ('v2_traffic_reported_v1', 'v2_traffic_accounted_v1', 'v2_traffic_reported_daily_v1', 'v2_traffic_accounted_daily_v1') AND access_type IN ('INSERT', 'SELECT')) >= 8\
                     AND countIf(database = {} AND table IN ('v2_schema_migration', 'v2_installation_binding', 'v2_retention_binding') AND access_type = 'SELECT') >= 3\
                     AND countIf(database = {} AND table IN ('v2_schema_migration', 'v2_installation_binding', 'v2_retention_binding') AND access_type = 'INSERT') = 0\
                FROM system.grants WHERE user_name = {}),\
             (SELECT countIf(access_type NOT IN ('USAGE', 'SELECT') OR (database IS NOT NULL AND database != {})) = 0\
                     AND countIf(database = {} AND access_type = 'SELECT') > 0\
                FROM system.grants WHERE user_name = {}) FORMAT TSVRaw;",
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.schema.username),
            ch_literal(&binding.writer.username),
            ch_literal(&binding.reader.username),
            ch_literal(&binding.schema.username),
            ch_literal(&binding.writer.username),
            ch_literal(&binding.reader.username),
            ch_literal(&binding.database),
            ch_literal(&binding.schema.username),
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.writer.username),
            ch_literal(&binding.database),
            ch_literal(&binding.database),
            ch_literal(&binding.reader.username),
        );
        let output = self.clickhouse(
            &binding.bootstrap,
            sql.into_bytes(),
            &[],
            "clickhouse_target_verify_failed",
        )?;
        let fields = one_tsv_row(&output, 12, "clickhouse_target_verify_invalid")?;
        let (major, minor) = version_family(fields[0])?;
        let database_ready = parse_ch_bool(fields[1])?;
        let principals_ready = parse_ch_bool(fields[3])?;
        let database_marker = format!("{marker}:clickhouse-database:{}", binding.database);
        let schema_marker = format!("{marker}:clickhouse-user:{}", binding.schema.username);
        let writer_marker = format!("{marker}:clickhouse-user:{}", binding.writer.username);
        let reader_marker = format!("{marker}:clickhouse-user:{}", binding.reader.username);
        let ownership_ready = fields[2] == database_marker.as_str()
            && fields[4] == schema_marker.as_str()
            && fields[5] == writer_marker.as_str()
            && fields[6] == reader_marker.as_str();
        let standalone = parse_ch_bool(fields[7])? && parse_ch_bool(fields[8])?;
        let schema_restricted = parse_ch_bool(fields[9])?;
        let writer_restricted = parse_ch_bool(fields[10])?;
        let reader_restricted = parse_ch_bool(fields[11])?;
        self.clickhouse_target = Some(binding);
        Ok(ClickHouseReadyProof {
            server_major: major,
            server_minor: minor,
            database_owned_by_operation: database_ready && ownership_ready,
            principals_owned_by_operation: principals_ready && ownership_ready,
            standalone_non_replicated: standalone,
            schema_is_ddl_metadata_and_ledger_only: schema_restricted,
            writer_is_insert_and_verify_only: writer_restricted,
            reader_is_select_only: reader_restricted,
            bootstrap_principal_absent_from_runtime: true,
        })
    }

    fn inspect_empty_target_redis(
        &mut self,
        operation_id: &str,
        redis_url: &str,
        binding: &TargetRedisInspectionBinding,
    ) -> Result<RedisEmptyProof, ExecutorError> {
        self.require_operation(operation_id)?;
        let size = self.redis_command(redis_url, &["--raw", "DBSIZE"], "redis_dbsize_failed")?;
        let key_count = parse_u64(size.trim(), "redis_dbsize_invalid")?;
        let info = self.redis_command(
            redis_url,
            &["--raw", "INFO", "server"],
            "redis_server_info_failed",
        )?;
        let target_run_id = redis_info_field(&info, "run_id")?;
        let getdel = self.redis_command(
            redis_url,
            &["--raw", "COMMAND", "INFO", "GETDEL"],
            "redis_getdel_probe_failed",
        )?;
        let evalsha = self.redis_command(
            redis_url,
            &["--raw", "COMMAND", "INFO", "EVALSHA"],
            "redis_evalsha_probe_failed",
        )?;
        let script = self.redis_command(
            redis_url,
            &["--raw", "COMMAND", "INFO", "SCRIPT"],
            "redis_script_probe_failed",
        )?;
        self.target_redis_url = Some(redis_url.to_string());
        Ok(RedisEmptyProof {
            key_count,
            namespace_entries: key_count,
            target_run_id: target_run_id.clone(),
            inspect_review_sha256: binding.inspect_review_sha256().to_string(),
            source_identity_distinct: target_run_id == binding.target_run_id()
                && binding
                    .source_run_ids()
                    .iter()
                    .all(|source| source != &target_run_id),
            tls_identity_verified: true,
            required_commands_available: !getdel.trim().is_empty()
                && !evalsha.trim().is_empty()
                && !script.trim().is_empty(),
        })
    }

    fn verify_release_artifact(
        &mut self,
        operation_id: &str,
        release: &ReleaseArtifactSpec,
    ) -> Result<ReleasePreflightProof, ExecutorError> {
        self.require_operation(operation_id)?;
        let subject = release.external_archive_sha256();
        self.receipt(
            &self.policy.release_archive,
            ExternalReceiptKind::ReleaseArchiveVerified,
            subject,
        )?;
        let canonical = fs::canonicalize(release.staged_path())
            .map_err(|_| ExecutorError::sanitized("release_path_unavailable"))?;
        if canonical != release.staged_path()
            || !canonical.starts_with(RELEASES_ROOT)
            || canonical.parent() != Some(Path::new(RELEASES_ROOT))
        {
            return Err(ExecutorError::sanitized("release_path_binding_invalid"));
        }
        let ownership = verify_release_tree(&canonical)?;
        let checksum = self.run_success(
            NativeCommandRequest::new(NativeProgram::Sha256sum)
                .arg("--check")
                .arg("--strict")
                .arg("--quiet")
                .arg("SHA256SUMS")
                .current_dir(&canonical),
            "release_internal_checksum_failed",
        )?;
        if !checksum.stdout().is_empty() || !checksum.stderr().is_empty() {
            return Err(ExecutorError::sanitized(
                "release_checksum_output_unexpected",
            ));
        }
        let exact_binaries = exact_directory_entries(
            &canonical.join("bin"),
            &["v2board-api", "v2board-workers", "v2board-analytics-schema"],
        )?;
        let frontend_valid = release_frontend_valid(&canonical)?;
        let release_metadata_valid =
            bounded_regular_file(&canonical.join("RELEASE"), 1, 64 * 1024)?;
        let api_unit = canonical.join("systemd/v2board-api.service");
        let worker_unit = canonical.join("systemd/v2board-worker.service");
        self.run_success(
            NativeCommandRequest::new(NativeProgram::SystemdAnalyze)
                .arg("verify")
                .arg(&api_unit)
                .arg(&worker_unit),
            "systemd_unit_verify_failed",
        )?;
        let api_text = read_bounded_regular_utf8(&api_unit, 128 * 1024)?;
        let worker_text = read_bounded_regular_utf8(&worker_unit, 128 * 1024)?;
        let units_exact = api_text.contains("ExecStart=/opt/v2board/current/bin/v2board-api")
            && worker_text.contains("ExecStart=/opt/v2board/current/bin/v2board-workers");
        let api_identity = api_text.contains("User=v2board-api")
            && api_text.contains("Group=v2board-api")
            && api_text.contains("V2BOARD_CONFIG_PATH=/var/lib/v2board/api/config.json");
        let worker_identity = worker_text.contains("User=v2board-worker")
            && worker_text.contains("Group=v2board-worker")
            && worker_text.contains("V2BOARD_CONFIG_PATH=/var/lib/v2board/worker/config.json")
            && worker_text.contains("Type=notify")
            && worker_text.contains("WatchdogSec=30s");
        let loopback = api_text.contains("V2BOARD_CONFIG_PATH=/var/lib/v2board/api/config.json")
            && !api_text.contains("RUST_BIND_ADDR=")
            && !api_text.contains("0.0.0.0:8080");
        self.verified_release = Some((release.release_id().to_string(), subject.to_string()));
        Ok(ReleasePreflightProof {
            release_id: release.release_id().to_string(),
            canonical_staged_path: canonical,
            external_archive_sha256: subject.to_string(),
            internal_sha256sums_valid: true,
            exact_long_lived_binary_set: exact_binaries,
            validated_frontend_tree_present: frontend_valid,
            release_metadata_valid,
            root_owned_and_runtime_read_only: ownership,
            systemd_analyze_verify_passed: true,
            unit_exec_paths_use_current_symlink: units_exact,
            api_unit_uses_dedicated_identity_and_config: api_identity,
            worker_unit_uses_dedicated_identity_and_config: worker_identity,
            api_bind_is_loopback_only: loopback,
        })
    }

    #[cfg(test)]
    fn verify_activation_commit_gate(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<ActivationCommitGateProof, ExecutorError> {
        self.require_verification_binding(verification)?;
        let clickhouse = self
            .clickhouse_target
            .clone()
            .ok_or_else(|| ExecutorError::sanitized("clickhouse_target_binding_missing"))?;
        let migration_count = CLICKHOUSE_MIGRATIONS.len();
        let raw_ttl = format!(
            "TTL accounting_date + toIntervalDay({})",
            clickhouse.raw_retention_days
        );
        let aggregate_ttl = format!(
            "TTL accounting_date + toIntervalDay({})",
            clickhouse.aggregate_retention_days
        );
        let sql = format!(
            "SELECT\
             (SELECT count() = {migration_count} AND uniqExact(version) = {migration_count} AND min(version) = 1 AND max(version) = {migration_count} FROM {}.v2_schema_migration),\
             (SELECT count() = 1 AND any(toString(installation_id)) = {} FROM {}.v2_installation_binding WHERE singleton = 1),\
             (SELECT count() = 1 AND any(toString(installation_id)) = {} AND any(raw_retention_days) = {} AND any(aggregate_retention_days) = {} FROM {}.v2_retention_binding WHERE singleton = 1),\
             ((SELECT count() FROM {}.v2_traffic_reported_v1) + (SELECT count() FROM {}.v2_traffic_accounted_v1) + (SELECT count() FROM {}.v2_traffic_reported_daily_v1) + (SELECT count() FROM {}.v2_traffic_accounted_daily_v1)) = 0,\
             (SELECT count() = 4\
                     AND countIf(name IN ('v2_traffic_reported_v1', 'v2_traffic_accounted_v1') AND position(create_table_query, {}) > 0) = 2\
                     AND countIf(name IN ('v2_traffic_reported_daily_v1', 'v2_traffic_accounted_daily_v1') AND position(create_table_query, {}) > 0) = 2\
                FROM system.tables WHERE database = {} AND name IN ('v2_traffic_reported_v1', 'v2_traffic_accounted_v1', 'v2_traffic_reported_daily_v1', 'v2_traffic_accounted_daily_v1'))\
             FORMAT TSVRaw;",
            clickhouse.database,
            ch_literal(&verification.installation_id),
            clickhouse.database,
            ch_literal(&verification.installation_id),
            clickhouse.raw_retention_days,
            clickhouse.aggregate_retention_days,
            clickhouse.database,
            clickhouse.database,
            clickhouse.database,
            clickhouse.database,
            clickhouse.database,
            ch_literal(&raw_ttl),
            ch_literal(&aggregate_ttl),
            ch_literal(&clickhouse.database),
        );
        let output = self.clickhouse(
            &clickhouse.bootstrap,
            sql.into_bytes(),
            &[],
            "clickhouse_commit_gate_failed",
        )?;
        let fields = one_tsv_row(&output, 5, "clickhouse_commit_gate_invalid")?;
        for field in fields {
            if !parse_ch_bool(field)? {
                return Err(ExecutorError::sanitized(
                    "clickhouse_ledger_not_exactly_current",
                ));
            }
        }
        let proof = self._authority_committer.verify_gate(verification)?;
        if !proof.clickhouse_ledger_exactly_current {
            return Err(ExecutorError::sanitized(
                "authority_committer_clickhouse_proof_invalid",
            ));
        }
        Ok(proof)
    }

    #[cfg(test)]
    fn commit_native_activation(
        &mut self,
        verification: &OfflineMigrationVerification,
    ) -> Result<NativeActivationCommitReceipt, ExecutorError> {
        self.require_verification_binding(verification)?;
        self._authority_committer.commit(verification)
    }

    fn install_role_configs_atomically(
        &mut self,
        bundle: &RoleConfigBundle,
    ) -> Result<ConfigInstallReceipt, ExecutorError> {
        self.require_operation(bundle.operation_id())?;
        if bundle.api_path() != Path::new(API_CONFIG_PATH)
            || bundle.worker_path() != Path::new(WORKER_CONFIG_PATH)
            || bundle.api_owner() != "v2board-api"
            || bundle.worker_owner() != "v2board-worker"
            || bundle.mode() != 0o600
        {
            return Err(ExecutorError::sanitized("role_config_contract_invalid"));
        }
        ensure_effective_root(&mut self.runner)?;
        let api_uid = lookup_uid(&mut self.runner, bundle.api_owner())?;
        let worker_uid = lookup_uid(&mut self.runner, bundle.worker_owner())?;
        verify_private_parent(bundle.api_path(), api_uid)?;
        verify_private_parent(bundle.worker_path(), worker_uid)?;
        install_config_pair(
            &mut self.runner,
            bundle,
            api_uid,
            worker_uid,
            self.permit.operation_id(),
        )?;
        Ok(ConfigInstallReceipt {
            operation_id: bundle.operation_id().to_string(),
            api_path: bundle.api_path().to_path_buf(),
            worker_path: bundle.worker_path().to_path_buf(),
            api_owner: bundle.api_owner().to_string(),
            worker_owner: bundle.worker_owner().to_string(),
            api_mode: bundle.mode(),
            worker_mode: bundle.mode(),
            regular_files_without_symlinks: true,
            temp_files_fsynced: true,
            atomic_renames_complete: true,
            parent_directories_fsynced: true,
            rollback_handle: config_rollback_handle(bundle.operation_id()),
        })
    }

    fn verify_role_configs(
        &mut self,
        bundle: &RoleConfigBundle,
        receipt: &ConfigInstallReceipt,
    ) -> Result<ConfigVerificationProof, ExecutorError> {
        self.require_operation(bundle.operation_id())?;
        if receipt.operation_id != bundle.operation_id()
            || receipt.rollback_handle != config_rollback_handle(bundle.operation_id())
        {
            return Err(ExecutorError::sanitized("config_receipt_binding_invalid"));
        }
        let api_uid = lookup_uid(&mut self.runner, bundle.api_owner())?;
        let worker_uid = lookup_uid(&mut self.runner, bundle.worker_owner())?;
        verify_exact_role_config(bundle.api_path(), bundle.api_bytes(), api_uid)?;
        verify_exact_role_config(bundle.worker_path(), bundle.worker_bytes(), worker_uid)?;
        Ok(ConfigVerificationProof {
            api_binding_hmac_sha256: bundle.api_binding_hmac_sha256().to_string(),
            worker_binding_hmac_sha256: bundle.worker_binding_hmac_sha256().to_string(),
            api_role_load_verified: true,
            worker_role_load_verified: true,
            cross_role_secret_absence_verified: true,
            owner_mode_and_path_reverified: true,
        })
    }

    #[cfg(test)]
    fn switch_current_release_atomically(
        &mut self,
        operation_id: &str,
        release: &ReleaseArtifactSpec,
    ) -> Result<ReleaseSwitchReceipt, ExecutorError> {
        self.require_operation(operation_id)?;
        ensure_effective_root(&mut self.runner)?;
        if self.verified_release.as_ref()
            != Some(&(
                release.release_id().to_string(),
                release.external_archive_sha256().to_string(),
            ))
        {
            return Err(ExecutorError::sanitized("release_not_preflighted"));
        }
        let current = Path::new(CURRENT_RELEASE_PATH);
        let parent = current
            .parent()
            .ok_or_else(|| ExecutorError::sanitized("current_release_parent_missing"))?;
        verify_root_directory(parent)?;
        let previous_target = match fs::symlink_metadata(current) {
            Ok(metadata) => {
                if !metadata.file_type().is_symlink() {
                    return Err(ExecutorError::sanitized("current_release_not_symlink"));
                }
                let target = fs::read_link(current)
                    .map_err(|_| ExecutorError::sanitized("current_release_read_failed"))?;
                let absolute = if target.is_absolute() {
                    target
                } else {
                    parent.join(target)
                };
                let canonical = fs::canonicalize(&absolute)
                    .map_err(|_| ExecutorError::sanitized("previous_release_invalid"))?;
                if !canonical.starts_with(RELEASES_ROOT) {
                    return Err(ExecutorError::sanitized("previous_release_outside_root"));
                }
                Some(canonical)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return Err(ExecutorError::sanitized("current_release_inspect_failed")),
        };
        if previous_target.as_deref() == Some(release.staged_path()) {
            sync_directory(parent)
                .map_err(|_| ExecutorError::sanitized("current_release_parent_sync_failed"))?;
            return Ok(ReleaseSwitchReceipt {
                operation_id: operation_id.to_string(),
                current_path: current.to_path_buf(),
                staged_path: release.staged_path().to_path_buf(),
                // Restoring an already-promoted release must remain a no-op.
                previous_target,
                atomic_symlink_rename_complete: true,
                parent_directory_fsynced: true,
                rollback_handle: release_rollback_handle(operation_id),
            });
        }
        let temporary = parent.join(format!(".current.{operation_id}.tmp"));
        if fs::symlink_metadata(&temporary).is_ok() {
            return Err(ExecutorError::sanitized("current_release_temp_exists"));
        }
        symlink(release.staged_path(), &temporary)
            .map_err(|_| ExecutorError::sanitized("current_release_temp_create_failed"))?;
        let result = fs::rename(&temporary, current)
            .and_then(|()| sync_directory(parent))
            .map_err(|_| ExecutorError::sanitized("current_release_atomic_switch_failed"));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        Ok(ReleaseSwitchReceipt {
            operation_id: operation_id.to_string(),
            current_path: current.to_path_buf(),
            staged_path: release.staged_path().to_path_buf(),
            previous_target,
            atomic_symlink_rename_complete: true,
            parent_directory_fsynced: true,
            rollback_handle: release_rollback_handle(operation_id),
        })
    }

    #[cfg(test)]
    fn start_unit_once(
        &mut self,
        operation_id: &str,
        unit: &'static str,
    ) -> Result<UnitStartReceipt, ExecutorError> {
        self.require_operation(operation_id)?;
        ensure_effective_root(&mut self.runner)?;
        if !matches!(unit, API_UNIT | WORKER_UNIT) {
            return Err(ExecutorError::sanitized("systemd_unit_not_allowed"));
        }
        let marker_present = verify_unit_start_attempt(&self.permit, unit)?;
        let active = self
            .runner
            .run(
                NativeCommandRequest::new(NativeProgram::Systemctl)
                    .arg("is-active")
                    .arg("--quiet")
                    .arg(unit),
            )
            .map_err(|error| runner_executor_error(error, "systemd_prestart_probe_failed"))?;
        if active.exit_code == 0 {
            if !marker_present {
                return Err(ExecutorError::sanitized(
                    "systemd_unit_active_before_operation_start",
                ));
            }
            self._started_units.insert(unit.to_string());
            return Ok(UnitStartReceipt {
                operation_id: operation_id.to_string(),
                unit: unit.to_string(),
                invocation_count_for_operation: 1,
                start_request_accepted: true,
            });
        }
        // The intent is durable before invoking systemd. If the process dies
        // between these operations, a retry may issue another idempotent
        // `systemctl start`; it never invents a second logical activation.
        persist_or_verify_unit_start_attempt(&self.permit, unit)?;
        self.run_success(
            NativeCommandRequest::new(NativeProgram::Systemctl)
                .arg("start")
                .arg(unit)
                .timeout(Duration::from_secs(60)),
            if unit == API_UNIT {
                "systemd_api_start_failed"
            } else {
                "systemd_worker_start_failed"
            },
        )?;
        self._started_units.insert(unit.to_string());
        Ok(UnitStartReceipt {
            operation_id: operation_id.to_string(),
            unit: unit.to_string(),
            invocation_count_for_operation: 1,
            start_request_accepted: true,
        })
    }

    #[cfg(test)]
    fn wait_for_unit_readiness(
        &mut self,
        operation_id: &str,
        installation_id: &str,
        release: &ReleaseArtifactSpec,
        unit: &'static str,
    ) -> Result<ServiceReadinessProof, ExecutorError> {
        self.require_operation(operation_id)?;
        if installation_id != self.permit.installation_id()
            || !self._started_units.contains(unit)
            || self.verified_release.as_ref()
                != Some(&(
                    release.release_id().to_string(),
                    release.external_archive_sha256().to_string(),
                ))
        {
            return Err(ExecutorError::sanitized("readiness_binding_invalid"));
        }
        self.run_success(
            NativeCommandRequest::new(NativeProgram::Systemctl)
                .arg("is-active")
                .arg("--quiet")
                .arg(unit),
            "systemd_unit_not_active",
        )?;
        if unit == API_UNIT {
            let output = self.run_success(
                NativeCommandRequest::new(NativeProgram::Curl)
                    .arg("--silent")
                    .arg("--show-error")
                    .arg("--fail")
                    .arg("--proto")
                    .arg("=http")
                    .arg("--max-time")
                    .arg("5")
                    .arg("http://127.0.0.1:8080/readyz"),
                "api_ready_probe_failed",
            )?;
            let value: serde_json::Value = serde_json::from_slice(output.stdout())
                .map_err(|_| ExecutorError::sanitized("api_ready_output_invalid"))?;
            if value.get("ok") != Some(&serde_json::Value::Bool(true)) {
                return Err(ExecutorError::sanitized("api_not_ready"));
            }
        } else if unit == WORKER_UNIT {
            let output = self.run_success(
                NativeCommandRequest::new(NativeProgram::Systemctl)
                    .arg("show")
                    .arg("--property=ActiveState")
                    .arg("--property=SubState")
                    .arg("--property=WatchdogTimestampMonotonic")
                    .arg("--value")
                    .arg(unit),
                "worker_systemd_state_failed",
            )?;
            let fields = strict_output_text(output.stdout(), "worker_systemd_state_invalid")?;
            let lines = fields.lines().collect::<Vec<_>>();
            if lines.len() != 3
                || lines[0] != "active"
                || lines[1] != "running"
                || parse_u64(lines[2], "worker_watchdog_timestamp_invalid")? == 0
                || !recent_regular_health_file(Path::new(WORKER_HEALTH_PATH))?
            {
                return Err(ExecutorError::sanitized("worker_not_ready"));
            }
        } else {
            return Err(ExecutorError::sanitized("systemd_unit_not_allowed"));
        }
        verify_current_release(release.staged_path())?;
        Ok(ServiceReadinessProof {
            operation_id: operation_id.to_string(),
            unit: unit.to_string(),
            installation_id: installation_id.to_string(),
            release_id: release.release_id().to_string(),
            postgres_ledger_exactly_current: true,
            runtime_role_and_config_verified: true,
            ready: true,
            systemd_notify_ready: (unit == WORKER_UNIT).then_some(true),
            watchdog_healthy: (unit == WORKER_UNIT).then_some(true),
        })
    }

    #[cfg(test)]
    fn restore_before_native_start(
        &mut self,
        recovery: &PreActivationRecovery,
    ) -> Result<PreActivationRestoreProof, ExecutorError> {
        self.require_operation(recovery.operation_id())?;
        if self._started_units.contains(API_UNIT) || self._started_units.contains(WORKER_UNIT) {
            return Err(ExecutorError::sanitized("native_unit_already_started"));
        }
        ensure_effective_root(&mut self.runner)?;
        if let Some(release) = recovery.release_receipt() {
            if release.rollback_handle != release_rollback_handle(recovery.operation_id()) {
                return Err(ExecutorError::sanitized("release_rollback_binding_invalid"));
            }
            restore_release_link(release)?;
        }
        if recovery.config_receipt().rollback_handle
            != config_rollback_handle(recovery.operation_id())
        {
            return Err(ExecutorError::sanitized("config_rollback_binding_invalid"));
        }
        restore_config_pair(recovery.operation_id())?;
        Ok(PreActivationRestoreProof {
            operation_id: recovery.operation_id().to_string(),
            native_units_never_started: true,
            prior_configs_restored_or_new_configs_removed: true,
            prior_release_link_restored_or_new_link_removed: true,
            filesystem_state_fsynced: true,
            prepared_targets_retained_for_same_operation_only: true,
        })
    }

    #[cfg(test)]
    fn execute_legacy_source_retirement(
        &mut self,
        request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<RetirementMutationReceipt, ExecutorError> {
        self.require_operation(request.operation_id())?;
        if request.installation_id() != self.permit.installation_id() {
            return Err(ExecutorError::sanitized("retirement_installation_mismatch"));
        }
        let source_subject = legacy_source_subject(request)?;
        self.receipt(
            &self.policy.receipts.source_credentials,
            ExternalReceiptKind::SourceCredentialsRevoked,
            &source_subject,
        )?;
        self.receipt(
            &self.policy.receipts.source_network,
            ExternalReceiptKind::SourceNetworkIsolated,
            &source_subject,
        )?;
        let release_subject = self
            .verified_release
            .as_ref()
            .map(|(_, digest)| digest.as_str())
            .ok_or_else(|| ExecutorError::sanitized("retirement_release_not_verified"))?;
        self.receipt(
            &self.policy.receipts.runtime_compatibility,
            ExternalReceiptKind::LegacyRuntimeCompatibilityDisabled,
            release_subject,
        )?;
        let authority_subject = sha256_domain(
            b"postgres-authority-v1\0",
            request.installation_id().as_bytes(),
        );
        self.receipt(
            &self.policy.receipts.postgres_authority,
            ExternalReceiptKind::PostgresSoleAuthorityVerified,
            &authority_subject,
        )?;
        let archive_subject = cold_archive_subject(request);
        self.receipt(
            &self.policy.receipts.cold_archive,
            ExternalReceiptKind::ColdArchiveVerified,
            &archive_subject,
        )?;
        Ok(RetirementMutationReceipt {
            operation_id: request.operation_id().to_string(),
            retirement_attempted_after_native_activation: true,
            mysql_stop_disable_and_network_isolation_attempted: true,
            source_redis_stop_disable_and_network_isolation_attempted: true,
            credential_revocation_attempted: true,
        })
    }

    #[cfg(test)]
    fn inspect_legacy_source_retirement(
        &mut self,
        request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<SourceRetirementObservation, ExecutorError> {
        self.require_operation(request.operation_id())?;
        let source_subject = legacy_source_subject(request)?;
        let credentials = self.receipt(
            &self.policy.receipts.source_credentials,
            ExternalReceiptKind::SourceCredentialsRevoked,
            &source_subject,
        )?;
        let network = self.receipt(
            &self.policy.receipts.source_network,
            ExternalReceiptKind::SourceNetworkIsolated,
            &source_subject,
        )?;
        let release_subject = self
            .verified_release
            .as_ref()
            .map(|(_, digest)| digest.as_str())
            .ok_or_else(|| ExecutorError::sanitized("retirement_release_not_verified"))?;
        let runtime = self.receipt(
            &self.policy.receipts.runtime_compatibility,
            ExternalReceiptKind::LegacyRuntimeCompatibilityDisabled,
            release_subject,
        )?;
        let authority_subject = sha256_domain(
            b"postgres-authority-v1\0",
            request.installation_id().as_bytes(),
        );
        let authority = self.receipt(
            &self.policy.receipts.postgres_authority,
            ExternalReceiptKind::PostgresSoleAuthorityVerified,
            &authority_subject,
        )?;
        let archive = self.receipt(
            &self.policy.receipts.cold_archive,
            ExternalReceiptKind::ColdArchiveVerified,
            &cold_archive_subject(request),
        )?;
        let observed = self.source_retirement_observer.observe(request)?;
        if observed.mysql_reachable_with_old_credentials
            || observed.source_default_redis_reachable_with_old_credentials
            || observed.source_cache_redis_reachable_with_old_credentials
            || !observed.source_access_permanently_disabled
            || !valid_evidence(&observed.mysql_probe_evidence)
            || !valid_evidence(&observed.source_redis_probe_evidence)
            || !valid_evidence(&observed.credential_probe_evidence)
        {
            return Err(ExecutorError::sanitized(
                "legacy_source_retirement_observation_failed",
            ));
        }
        Ok(SourceRetirementObservation {
            operation_id: request.operation_id().to_string(),
            source_retired: true,
            mysql_reachable: false,
            source_redis_reachable: false,
            source_credentials_revoked: true,
            legacy_runtime_compat: false,
            postgres_is_only_transaction_authority: true,
            mysql_unreachable_evidence: observed.mysql_probe_evidence,
            source_redis_unreachable_evidence: observed.source_redis_probe_evidence,
            credential_revocation_evidence: format!(
                "{};{};{}",
                observed.credential_probe_evidence,
                credentials.evidence_reference,
                network.evidence_reference
            ),
            runtime_compat_evidence: runtime.evidence_reference,
            postgres_authority_evidence: format!(
                "{};{}",
                authority.evidence_reference, archive.evidence_reference
            ),
        })
    }
}

impl PostgresConnection {
    fn parse(value: &str) -> Result<Self, ExecutorError> {
        let mut url =
            Url::parse(value).map_err(|_| ExecutorError::sanitized("postgres_url_invalid"))?;
        if !matches!(url.scheme(), "postgres" | "postgresql")
            || url.host_str().is_none()
            || url.username().is_empty()
            || url.password().is_none()
        {
            return Err(ExecutorError::sanitized("postgres_url_invalid"));
        }
        let password = percent_decode(url.password().unwrap_or_default())?;
        let username = percent_decode(url.username())?;
        let database = percent_decode(url.path().trim_start_matches('/'))?;
        let host = url
            .host_str()
            .ok_or_else(|| ExecutorError::sanitized("postgres_url_invalid"))?
            .to_string();
        let port = url.port().unwrap_or(5432);
        if database.is_empty()
            || !valid_identifier(&database, 63)
            || !valid_identifier(&username, 63)
            || password.is_empty()
        {
            return Err(ExecutorError::sanitized("postgres_url_identity_invalid"));
        }
        url.set_password(None)
            .map_err(|_| ExecutorError::sanitized("postgres_url_redaction_failed"))?;
        Ok(Self {
            safe_url: url.to_string(),
            password,
            username,
            database,
            host,
            port,
        })
    }
}

impl ClickHouseTargetBinding {
    fn from_target(target: &ClickHouseTargetSpec) -> Self {
        Self {
            database: target.database.clone(),
            #[cfg(test)]
            raw_retention_days: target.raw_retention_days,
            #[cfg(test)]
            aggregate_retention_days: target.aggregate_retention_days,
            bootstrap: ClickHouseConnection {
                endpoint: target.endpoint.clone(),
                database: "system".to_string(),
                username: target.bootstrap_principal.username.clone(),
                password: target.bootstrap_principal.password().to_string(),
            },
            schema: principal_binding(&target.schema_principal),
            writer: principal_binding(&target.writer_principal),
            reader: principal_binding(&target.reader_principal),
        }
    }
}

fn principal_binding(principal: &ClickHousePrincipalSpec) -> ClickHousePrincipalBinding {
    ClickHousePrincipalBinding {
        username: principal.username.clone(),
        password: principal.password().to_string(),
    }
}

fn postgres_bootstrap_sql(
    permit: &DurableTargetMutationPermit,
    migration: &PostgresConnection,
    api: &PostgresConnection,
    worker: &PostgresConnection,
    observed: PostgresObservedState,
) -> Vec<u8> {
    let marker = operation_marker(permit);
    let mut sql = String::from("\\set ON_ERROR_STOP on\nSET synchronous_commit = on;\n");
    if !observed.roles_exist {
        sql.push_str("BEGIN;\nSET LOCAL synchronous_commit = on;\n");
        for connection in [migration, api, worker] {
            sql.push_str(&format!(
                "CREATE ROLE {} LOGIN PASSWORD {} NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;\n\
                 COMMENT ON ROLE {} IS {};\n",
                connection.username,
                pg_literal(&connection.password),
                connection.username,
                pg_literal(&format!("{marker}:postgres-role:{}", connection.username)),
            ));
        }
        sql.push_str("COMMIT;\n");
    }
    if !observed.database_exists {
        sql.push_str(&format!(
            "CREATE DATABASE {} OWNER {} ENCODING 'UTF8' LC_COLLATE 'C.UTF-8' LC_CTYPE 'C.UTF-8' TEMPLATE template0;\n",
            migration.database, migration.username
        ));
    }
    sql.push_str(&format!(
        "COMMENT ON DATABASE {} IS {};\n\
         REVOKE CONNECT, TEMPORARY ON DATABASE {} FROM PUBLIC;\n\
         GRANT CONNECT ON DATABASE {} TO {}, {}, {};\n\
         \\connect {}\n\
         REVOKE ALL PRIVILEGES ON SCHEMA public FROM PUBLIC;\n\
         GRANT USAGE ON SCHEMA public TO {}, {};\n\
         REVOKE CREATE ON SCHEMA public FROM {}, {};\n\
         ALTER DEFAULT PRIVILEGES FOR ROLE {} IN SCHEMA public REVOKE ALL PRIVILEGES ON TABLES FROM {}, {};\n\
         ALTER DEFAULT PRIVILEGES FOR ROLE {} IN SCHEMA public REVOKE ALL PRIVILEGES ON SEQUENCES FROM {}, {};\n",
        migration.database,
        pg_literal(&format!(
            "{marker}:postgres-database:{}",
            migration.database
        )),
        migration.database,
        migration.database,
        migration.username,
        api.username,
        worker.username,
        migration.database,
        api.username,
        worker.username,
        api.username,
        worker.username,
        migration.username,
        api.username,
        worker.username,
        migration.username,
        api.username,
        worker.username,
    ));
    sql.into_bytes()
}

fn clickhouse_bootstrap_sql(
    permit: &DurableTargetMutationPermit,
    target: &ClickHouseTargetBinding,
    observed: ClickHouseObservedState,
) -> Vec<u8> {
    let marker = operation_marker(permit);
    let mut sql = String::new();
    if !observed.database_exists {
        sql.push_str(&format!(
            "CREATE DATABASE {} COMMENT {};\n",
            target.database,
            ch_literal(&format!("{marker}:clickhouse-database:{}", target.database))
        ));
    }
    for (principal, exists) in [
        (&target.schema, observed.schema_exists),
        (&target.writer, observed.writer_exists),
        (&target.reader, observed.reader_exists),
    ] {
        if !exists {
            sql.push_str(&format!(
                "CREATE USER {} IDENTIFIED WITH sha256_password BY {} COMMENT {};\n",
                principal.username,
                ch_literal(&principal.password),
                ch_literal(&format!("{marker}:clickhouse-user:{}", principal.username)),
            ));
        }
    }
    sql.push_str(&format!(
        "GRANT CREATE TABLE, ALTER TABLE, DROP TABLE, SELECT ON {}.* TO {};\n\
         GRANT SELECT ON system.* TO {};\n\
         GRANT INSERT ON {}.v2_schema_migration TO {};\n\
         GRANT INSERT ON {}.v2_installation_binding TO {};\n\
         GRANT INSERT ON {}.v2_retention_binding TO {};\n\
         GRANT INSERT, SELECT ON {}.v2_traffic_reported_v1 TO {};\n\
         GRANT INSERT, SELECT ON {}.v2_traffic_accounted_v1 TO {};\n\
         GRANT INSERT, SELECT ON {}.v2_traffic_reported_daily_v1 TO {};\n\
         GRANT INSERT, SELECT ON {}.v2_traffic_accounted_daily_v1 TO {};\n\
         GRANT SELECT ON {}.v2_schema_migration TO {};\n\
         GRANT SELECT ON {}.v2_installation_binding TO {};\n\
         GRANT SELECT ON {}.v2_retention_binding TO {};\n\
         GRANT SELECT ON {}.* TO {};\n",
        target.database,
        target.schema.username,
        target.schema.username,
        target.database,
        target.schema.username,
        target.database,
        target.schema.username,
        target.database,
        target.schema.username,
        target.database,
        target.writer.username,
        target.database,
        target.writer.username,
        target.database,
        target.writer.username,
        target.database,
        target.writer.username,
        target.database,
        target.writer.username,
        target.database,
        target.writer.username,
        target.database,
        target.writer.username,
        target.database,
        target.reader.username,
    ));
    sql.into_bytes()
}

fn operation_marker(permit: &DurableTargetMutationPermit) -> String {
    format!(
        "v2board-operation:{}:installation:{}",
        permit.operation_id(),
        permit.installation_id()
    )
}

fn creation_receipt(permit: &DurableTargetMutationPermit) -> TargetCreationReceipt {
    TargetCreationReceipt {
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
    }
}

fn pg_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn ch_literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn percent_decode(value: &str) -> Result<String, ExecutorError> {
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(String::from)
        .map_err(|_| ExecutorError::sanitized("url_percent_encoding_invalid"))
}

fn valid_identifier(value: &str, maximum: usize) -> bool {
    let mut bytes = value.bytes();
    value.len() <= maximum
        && matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validated_program_path(program: NativeProgram) -> Result<PathBuf, NativeRunnerError> {
    let declared = Path::new(program.path());
    let canonical = fs::canonicalize(declared).map_err(|_| NativeRunnerError::UnsafeProgram)?;
    if !canonical.is_absolute()
        || !(canonical.starts_with("/usr/bin") || canonical.starts_with("/bin"))
    {
        return Err(NativeRunnerError::UnsafeProgram);
    }
    let metadata = fs::metadata(&canonical).map_err(|_| NativeRunnerError::UnsafeProgram)?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.permissions().mode() & 0o111 == 0
    {
        return Err(NativeRunnerError::UnsafeProgram);
    }
    Ok(canonical)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut overflow = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let available = limit.saturating_sub(bytes.len());
        let retained = available.min(read);
        bytes.extend_from_slice(&buffer[..retained]);
        overflow |= retained != read;
    }
    Ok((bytes, overflow))
}

fn redact_all(bytes: &mut Vec<u8>, secrets: &[Vec<u8>]) {
    for secret in secrets {
        while let Some(offset) = find_subslice(bytes, secret) {
            bytes.splice(offset..offset + secret.len(), b"[REDACTED]".iter().copied());
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|candidate| candidate == needle)
}

fn runner_executor_error(error: NativeRunnerError, operation_code: &'static str) -> ExecutorError {
    let suffix = match error {
        NativeRunnerError::InvalidRequest => "invalid_request",
        NativeRunnerError::SecretInArgv => "secret_in_argv",
        NativeRunnerError::UnsafeProgram => "unsafe_program",
        NativeRunnerError::Spawn => "spawn",
        NativeRunnerError::Stdin => "stdin",
        NativeRunnerError::Wait => "wait",
        NativeRunnerError::Timeout => "timeout",
        NativeRunnerError::OutputLimit => "output_limit",
        NativeRunnerError::OutputRead => "output_read",
    };
    ExecutorError::sanitized(format!("{operation_code}_{suffix}"))
}

fn strict_output_text(bytes: &[u8], code: &'static str) -> Result<String, ExecutorError> {
    if bytes.contains(&0) {
        return Err(ExecutorError::sanitized(code));
    }
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|_| ExecutorError::sanitized(code))
}

fn one_tsv_row<'a>(
    output: &'a str,
    expected_fields: usize,
    code: &'static str,
) -> Result<Vec<&'a str>, ExecutorError> {
    let output = output.strip_suffix('\n').unwrap_or(output);
    if output.contains('\n') || output.contains('\r') {
        return Err(ExecutorError::sanitized(code));
    }
    let fields = output.split('\t').collect::<Vec<_>>();
    if fields.len() != expected_fields {
        return Err(ExecutorError::sanitized(code));
    }
    Ok(fields)
}

fn parse_u64(value: &str, code: &'static str) -> Result<u64, ExecutorError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ExecutorError::sanitized(code));
    }
    value.parse().map_err(|_| ExecutorError::sanitized(code))
}

fn parse_pg_bool(value: &str) -> Result<bool, ExecutorError> {
    match value {
        "t" => Ok(true),
        "f" => Ok(false),
        _ => Err(ExecutorError::sanitized("postgres_boolean_invalid")),
    }
}

fn parse_ch_bool(value: &str) -> Result<bool, ExecutorError> {
    match value {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(ExecutorError::sanitized("clickhouse_boolean_invalid")),
    }
}

fn version_family(value: &str) -> Result<(u16, u16), ExecutorError> {
    let mut parts = value.split('.');
    let major = parts
        .next()
        .ok_or_else(|| ExecutorError::sanitized("clickhouse_version_invalid"))?
        .parse()
        .map_err(|_| ExecutorError::sanitized("clickhouse_version_invalid"))?;
    let minor = parts
        .next()
        .ok_or_else(|| ExecutorError::sanitized("clickhouse_version_invalid"))?
        .parse()
        .map_err(|_| ExecutorError::sanitized("clickhouse_version_invalid"))?;
    Ok((major, minor))
}

fn redis_info_field(info: &str, key: &str) -> Result<String, ExecutorError> {
    let prefix = format!("{key}:");
    let value = info
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(str::trim)
        .ok_or_else(|| ExecutorError::sanitized("redis_info_field_missing"))?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(ExecutorError::sanitized("redis_info_field_invalid"));
    }
    Ok(value.to_string())
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_evidence(value: &str) -> bool {
    let value = value.trim();
    (8..=1024).contains(&value.len()) && !value.chars().any(char::is_control)
}

#[cfg(test)]
fn sha256_domain(domain: &[u8], value: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(value);
    hex::encode(hash.finalize())
}

fn run_in_isolated_runtime<F, T, E>(future: F) -> Result<T, ()>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ())?;
        runtime.block_on(future).map_err(|_| ())
    })
    .join()
    .map_err(|_| ())?
}

fn load_external_receipt(
    binding: &ReceiptBinding,
    operation_id: &str,
    expected_kind: ExternalReceiptKind,
    expected_subject_sha256: &str,
) -> Result<ExternalControlReceipt, ExecutorError> {
    let mut file = File::open(&binding.path)
        .map_err(|_| ExecutorError::sanitized("external_receipt_open_failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| ExecutorError::sanitized("external_receipt_metadata_failed"))?;
    let path_metadata = fs::symlink_metadata(&binding.path)
        .map_err(|_| ExecutorError::sanitized("external_receipt_metadata_failed"))?;
    if !metadata.is_file()
        || !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || metadata.dev() != path_metadata.dev()
        || metadata.ino() != path_metadata.ino()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() == 0
        || metadata.len() > MAX_RECEIPT_BYTES
    {
        return Err(ExecutorError::sanitized("external_receipt_file_unsafe"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| ExecutorError::sanitized("external_receipt_read_failed"))?;
    if hex::encode(Sha256::digest(&bytes)) != binding.sha256 {
        return Err(ExecutorError::sanitized("external_receipt_hash_mismatch"));
    }
    let receipt: ExternalControlReceipt = serde_json::from_slice(&bytes)
        .map_err(|_| ExecutorError::sanitized("external_receipt_json_invalid"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0);
    if receipt.schema_version != 1
        || receipt.operation_id != operation_id
        || receipt.kind != expected_kind
        || receipt.subject_sha256 != expected_subject_sha256
        || !receipt.completed
        || !valid_evidence(&receipt.evidence_reference)
        || receipt.issued_at_unix <= 0
        || receipt.issued_at_unix > now
    {
        return Err(ExecutorError::sanitized("external_receipt_binding_invalid"));
    }
    Ok(receipt)
}

#[cfg(test)]
fn legacy_source_subject(
    request: &LegacySourceRetirementRequest<'_>,
) -> Result<String, ExecutorError> {
    let source = request.source();
    let mut identities = [
        redacted_url_identity(&source.database_url)?,
        redacted_url_identity(&source.redis_default_url)?,
        redacted_url_identity(&source.redis_cache_url)?,
    ];
    identities.sort();
    Ok(sha256_domain(
        b"legacy-source-retirement-v1\0",
        identities.join("\0").as_bytes(),
    ))
}

#[cfg(test)]
fn redacted_url_identity(value: &str) -> Result<String, ExecutorError> {
    let url =
        Url::parse(value).map_err(|_| ExecutorError::sanitized("source_identity_url_invalid"))?;
    let host = url
        .host_str()
        .ok_or_else(|| ExecutorError::sanitized("source_identity_url_invalid"))?;
    let port = url.port_or_known_default().unwrap_or(0);
    Ok(format!(
        "{}://{}:{}{}",
        url.scheme(),
        host.to_ascii_lowercase(),
        port,
        url.path()
    ))
}

#[cfg(test)]
fn cold_archive_subject(request: &LegacySourceRetirementRequest<'_>) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(request.cold_archive_reference().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(request.cold_archive_sha256().as_bytes());
    sha256_domain(b"cold-archive-v1\0", &bytes)
}

fn ensure_effective_root(runner: &mut impl NativeCommandRunner) -> Result<(), ExecutorError> {
    let output = runner
        .run(NativeCommandRequest::new(NativeProgram::Id).arg("-u"))
        .map_err(|error| runner_executor_error(error, "effective_uid_probe_failed"))?;
    if output.exit_code != 0
        || strict_output_text(output.stdout(), "effective_uid_probe_invalid")?.trim() != "0"
    {
        return Err(ExecutorError::sanitized("lifecycle_executor_requires_root"));
    }
    Ok(())
}

fn lookup_uid(runner: &mut impl NativeCommandRunner, user: &str) -> Result<u32, ExecutorError> {
    if !matches!(user, "v2board-api" | "v2board-worker") {
        return Err(ExecutorError::sanitized("runtime_user_not_allowed"));
    }
    let output = runner
        .run(
            NativeCommandRequest::new(NativeProgram::Id)
                .arg("-u")
                .arg(user),
        )
        .map_err(|error| runner_executor_error(error, "runtime_uid_probe_failed"))?;
    if output.exit_code != 0 {
        return Err(ExecutorError::sanitized("runtime_uid_probe_failed"));
    }
    let value = strict_output_text(output.stdout(), "runtime_uid_probe_invalid")?;
    u32::try_from(parse_u64(value.trim(), "runtime_uid_probe_invalid")?)
        .map_err(|_| ExecutorError::sanitized("runtime_uid_probe_invalid"))
}

fn verify_private_parent(path: &Path, expected_uid: u32) -> Result<(), ExecutorError> {
    let parent = path
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| ExecutorError::sanitized("config_parent_metadata_failed"))?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(ExecutorError::sanitized("config_parent_unsafe"));
    }
    Ok(())
}

struct ConfigSlot<'a> {
    path: &'a Path,
    temporary: PathBuf,
    backup: PathBuf,
    absent_marker: PathBuf,
    already_promoted: bool,
}

fn install_config_pair(
    runner: &mut impl NativeCommandRunner,
    bundle: &RoleConfigBundle,
    api_uid: u32,
    worker_uid: u32,
    operation_id: &str,
) -> Result<(), ExecutorError> {
    let mut api = prepare_config_slot(
        runner,
        bundle.api_path(),
        bundle.api_bytes(),
        bundle.api_owner(),
        api_uid,
        operation_id,
    )?;
    let mut worker = match prepare_config_slot(
        runner,
        bundle.worker_path(),
        bundle.worker_bytes(),
        bundle.worker_owner(),
        worker_uid,
        operation_id,
    ) {
        Ok(slot) => slot,
        Err(error) => {
            let _ = restore_config_slot(&api);
            return Err(error);
        }
    };
    if let Err(error) = promote_config_slot(&mut api) {
        let _ = restore_config_slot(&api);
        let _ = restore_config_slot(&worker);
        return Err(error);
    }
    if let Err(error) = promote_config_slot(&mut worker) {
        let api_restore = restore_config_slot(&api);
        let worker_restore = restore_config_slot(&worker);
        if api_restore.is_err() || worker_restore.is_err() {
            return Err(ExecutorError::sanitized("config_pair_rollback_failed"));
        }
        return Err(error);
    }
    Ok(())
}

fn prepare_config_slot<'a>(
    runner: &mut impl NativeCommandRunner,
    path: &'a Path,
    bytes: &'a [u8],
    owner: &'a str,
    uid: u32,
    operation_id: &str,
) -> Result<ConfigSlot<'a>, ExecutorError> {
    let parent = path
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ExecutorError::sanitized("config_name_invalid"))?;
    let temporary = parent.join(format!(".{name}.{operation_id}.tmp"));
    let backup = parent.join(format!(".{name}.{operation_id}.previous"));
    let absent_marker = parent.join(format!(".{name}.{operation_id}.absent"));
    let backup_exists = fs::symlink_metadata(&backup).is_ok();
    let absent_exists = fs::symlink_metadata(&absent_marker).is_ok();
    if backup_exists && absent_exists {
        return Err(ExecutorError::sanitized("config_rollback_state_conflict"));
    }
    if !backup_exists && !absent_exists {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                verify_config_metadata(&metadata, uid)?;
                fs::hard_link(path, &backup)
                    .map_err(|_| ExecutorError::sanitized("config_backup_create_failed"))?;
                sync_directory(parent)
                    .map_err(|_| ExecutorError::sanitized("config_parent_sync_failed"))?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                write_new_owner_file(&absent_marker, b"absent\n", 0o600)
                    .map_err(|_| ExecutorError::sanitized("config_absent_marker_failed"))?;
            }
            Err(_) => return Err(ExecutorError::sanitized("config_inspect_failed")),
        }
    } else {
        if backup_exists {
            let metadata = fs::symlink_metadata(&backup)
                .map_err(|_| ExecutorError::sanitized("config_backup_invalid"))?;
            verify_config_metadata(&metadata, uid)?;
        }
        if absent_exists {
            verify_root_owner_file(&absent_marker, 0o600)?;
        }
    }
    securely_remove_config_temp_if_present(&temporary, uid)?;
    write_new_owner_file(&temporary, bytes, 0o600)
        .map_err(|_| ExecutorError::sanitized("config_temp_write_failed"))?;
    chown_runtime_file(runner, owner, &temporary)?;
    let metadata = fs::symlink_metadata(&temporary)
        .map_err(|_| ExecutorError::sanitized("config_temp_metadata_failed"))?;
    verify_config_metadata(&metadata, uid)?;
    let already_promoted = read_regular_limited(path, bytes.len() + 1)
        .ok()
        .is_some_and(|current| current == bytes);
    if already_promoted {
        securely_remove_secret_file(&temporary)?;
    } else if backup_exists {
        let current = read_regular_limited(path, MAX_COMMAND_INPUT_BYTES)?;
        let prior = read_regular_limited(&backup, MAX_COMMAND_INPUT_BYTES)?;
        if current != prior {
            return Err(ExecutorError::sanitized("config_current_drifted"));
        }
    } else if fs::symlink_metadata(path).is_ok() {
        return Err(ExecutorError::sanitized("config_expected_absent"));
    }
    Ok(ConfigSlot {
        path,
        temporary,
        backup,
        absent_marker,
        already_promoted,
    })
}

fn promote_config_slot(slot: &mut ConfigSlot<'_>) -> Result<(), ExecutorError> {
    if slot.already_promoted {
        return Ok(());
    }
    fs::rename(&slot.temporary, slot.path)
        .map_err(|_| ExecutorError::sanitized("config_atomic_rename_failed"))?;
    sync_directory(
        slot.path
            .parent()
            .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?,
    )
    .map_err(|_| ExecutorError::sanitized("config_parent_sync_failed"))?;
    slot.already_promoted = true;
    Ok(())
}

fn restore_config_slot(slot: &ConfigSlot<'_>) -> Result<(), ExecutorError> {
    let parent = slot
        .path
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?;
    if fs::symlink_metadata(&slot.backup).is_ok() {
        fs::rename(&slot.backup, slot.path)
            .map_err(|_| ExecutorError::sanitized("config_backup_restore_failed"))?;
    } else if fs::symlink_metadata(&slot.absent_marker).is_ok() {
        match fs::remove_file(slot.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(ExecutorError::sanitized("config_new_remove_failed")),
        }
        fs::remove_file(&slot.absent_marker)
            .map_err(|_| ExecutorError::sanitized("config_absent_marker_remove_failed"))?;
    } else {
        return Err(ExecutorError::sanitized("config_rollback_state_missing"));
    }
    let _ = securely_remove_secret_file_if_present(&slot.temporary);
    sync_directory(parent).map_err(|_| ExecutorError::sanitized("config_parent_sync_failed"))
}

#[cfg(test)]
fn restore_config_pair(operation_id: &str) -> Result<(), ExecutorError> {
    for path in [Path::new(API_CONFIG_PATH), Path::new(WORKER_CONFIG_PATH)] {
        let parent = path
            .parent()
            .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?;
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ExecutorError::sanitized("config_name_invalid"))?;
        let backup = parent.join(format!(".{name}.{operation_id}.previous"));
        let absent = parent.join(format!(".{name}.{operation_id}.absent"));
        if fs::symlink_metadata(&backup).is_ok() {
            fs::rename(&backup, path)
                .map_err(|_| ExecutorError::sanitized("config_backup_restore_failed"))?;
        } else if fs::symlink_metadata(&absent).is_ok() {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return Err(ExecutorError::sanitized("config_new_remove_failed")),
            }
            fs::remove_file(&absent)
                .map_err(|_| ExecutorError::sanitized("config_absent_marker_remove_failed"))?;
        } else {
            return Err(ExecutorError::sanitized("config_rollback_state_missing"));
        }
        sync_directory(parent)
            .map_err(|_| ExecutorError::sanitized("config_parent_sync_failed"))?;
    }
    Ok(())
}

fn chown_runtime_file(
    runner: &mut impl NativeCommandRunner,
    owner: &str,
    path: &Path,
) -> Result<(), ExecutorError> {
    let output = runner
        .run(
            NativeCommandRequest::new(NativeProgram::Chown)
                .arg("--no-dereference")
                .arg(format!("{owner}:{owner}"))
                .arg(path),
        )
        .map_err(|error| runner_executor_error(error, "config_chown_failed"))?;
    if output.exit_code != 0 {
        return Err(ExecutorError::sanitized("config_chown_failed"));
    }
    Ok(())
}

fn verify_exact_role_config(
    path: &Path,
    expected: &[u8],
    expected_uid: u32,
) -> Result<(), ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("config_verify_metadata_failed"))?;
    verify_config_metadata(&metadata, expected_uid)?;
    let actual = read_regular_limited(path, expected.len() + 1)?;
    if actual != expected {
        return Err(ExecutorError::sanitized("config_verify_content_mismatch"));
    }
    Ok(())
}

fn verify_config_metadata(metadata: &fs::Metadata, expected_uid: u32) -> Result<(), ExecutorError> {
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(ExecutorError::sanitized("config_file_unsafe"));
    }
    Ok(())
}

fn config_rollback_handle(operation_id: &str) -> String {
    format!("config-rollback:{operation_id}")
}

#[cfg(test)]
fn release_rollback_handle(operation_id: &str) -> String {
    format!("release-rollback:{operation_id}")
}

fn write_new_owner_file(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(mode);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_directory(
        path.parent()
            .ok_or_else(|| io::Error::other("file has no parent"))?,
    )
}

fn read_regular_limited(path: &Path, maximum: usize) -> Result<Vec<u8>, ExecutorError> {
    let mut file =
        File::open(path).map_err(|_| ExecutorError::sanitized("bounded_file_open_failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| ExecutorError::sanitized("bounded_file_metadata_failed"))?;
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("bounded_file_metadata_failed"))?;
    if !metadata.is_file()
        || !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || metadata.dev() != path_metadata.dev()
        || metadata.ino() != path_metadata.ino()
        || metadata.len() > maximum as u64
    {
        return Err(ExecutorError::sanitized("bounded_file_unsafe"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| ExecutorError::sanitized("bounded_file_read_failed"))?;
    Ok(bytes)
}

fn securely_remove_config_temp_if_present(
    path: &Path,
    expected_uid: u32,
) -> Result<(), ExecutorError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
                || (metadata.uid() != 0 && metadata.uid() != expected_uid)
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err(ExecutorError::sanitized("stale_config_temp_unsafe"));
            }
            securely_remove_secret_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ExecutorError::sanitized("stale_config_temp_inspect_failed")),
    }
}

fn securely_remove_secret_file_if_present(path: &Path) -> Result<(), ExecutorError> {
    match fs::symlink_metadata(path) {
        Ok(_) => securely_remove_secret_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ExecutorError::sanitized("secret_file_inspect_failed")),
    }
}

fn securely_remove_secret_file(path: &Path) -> Result<(), ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("secret_file_metadata_failed"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > MAX_COMMAND_INPUT_BYTES as u64
    {
        return Err(ExecutorError::sanitized("secret_file_unsafe"));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|_| ExecutorError::sanitized("secret_file_open_failed"))?;
    let zeros = vec![0_u8; metadata.len() as usize];
    file.write_all(&zeros)
        .and_then(|()| file.set_len(0))
        .and_then(|()| file.sync_all())
        .map_err(|_| ExecutorError::sanitized("secret_file_wipe_failed"))?;
    drop(file);
    fs::remove_file(path).map_err(|_| ExecutorError::sanitized("secret_file_remove_failed"))?;
    sync_directory(
        path.parent()
            .ok_or_else(|| ExecutorError::sanitized("secret_file_parent_missing"))?,
    )
    .map_err(|_| ExecutorError::sanitized("secret_file_parent_sync_failed"))
}

fn cleanup_config_rollback_artifacts(
    operation_id: &str,
) -> Result<ConfigRollbackCleanupProof, ExecutorError> {
    cleanup_config_rollback_artifacts_for_paths(
        operation_id,
        &[Path::new(API_CONFIG_PATH), Path::new(WORKER_CONFIG_PATH)],
    )
}

fn cleanup_config_rollback_artifacts_for_paths(
    operation_id: &str,
    active_paths: &[&Path],
) -> Result<ConfigRollbackCleanupProof, ExecutorError> {
    let mut artifacts = Vec::with_capacity(6);
    for active in active_paths {
        let parent = active
            .parent()
            .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?;
        let name = active
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ExecutorError::sanitized("config_name_invalid"))?;
        let backup = parent.join(format!(".{name}.{operation_id}.previous"));
        let absent = parent.join(format!(".{name}.{operation_id}.absent"));
        let temporary = parent.join(format!(".{name}.{operation_id}.tmp"));

        remove_config_backup_after_forward_success(active, &backup)?;
        securely_remove_secret_file_if_present(&absent)?;
        securely_remove_secret_file_if_present(&temporary)?;
        sync_directory(parent)
            .map_err(|_| ExecutorError::sanitized("config_cleanup_parent_sync_failed"))?;
        artifacts.extend([backup, absent, temporary]);
    }
    for artifact in &artifacts {
        match fs::symlink_metadata(artifact) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => return Err(ExecutorError::sanitized("config_cleanup_artifact_remains")),
            Err(_) => {
                return Err(ExecutorError::sanitized(
                    "config_cleanup_artifact_probe_failed",
                ));
            }
        }
    }
    Ok(ConfigRollbackCleanupProof {
        operation_id: operation_id.to_string(),
        artifact_count: artifacts.len() as u8,
        all_artifacts_absent: true,
        parent_directories_fsynced: true,
    })
}

fn remove_config_backup_after_forward_success(
    active: &Path,
    backup: &Path,
) -> Result<(), ExecutorError> {
    let backup_metadata = match fs::symlink_metadata(backup) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ExecutorError::sanitized("config_backup_inspect_failed")),
    };
    if !backup_metadata.file_type().is_file()
        || backup_metadata.file_type().is_symlink()
        || backup_metadata.permissions().mode() & 0o077 != 0
    {
        return Err(ExecutorError::sanitized("config_backup_unsafe"));
    }
    let active_metadata = fs::symlink_metadata(active)
        .map_err(|_| ExecutorError::sanitized("config_active_inspect_failed"))?;
    if !active_metadata.file_type().is_file() || active_metadata.file_type().is_symlink() {
        return Err(ExecutorError::sanitized("config_active_unsafe"));
    }

    if active_metadata.dev() == backup_metadata.dev()
        && active_metadata.ino() == backup_metadata.ino()
    {
        // An already-exact boot config can be hard-linked as its own rollback
        // snapshot. Wiping that inode would corrupt the active file; unlinking
        // only the redundant name is sufficient because its content is the
        // current boot-only document, not a retired plaintext baseline.
        fs::remove_file(backup)
            .map_err(|_| ExecutorError::sanitized("config_backup_unlink_failed"))?;
        sync_directory(
            backup
                .parent()
                .ok_or_else(|| ExecutorError::sanitized("config_parent_missing"))?,
        )
        .map_err(|_| ExecutorError::sanitized("config_cleanup_parent_sync_failed"))
    } else {
        securely_remove_secret_file(backup)
    }
}

fn prepare_private_runtime_dir(path: &Path) -> Result<(), ExecutorError> {
    if !path.is_absolute() {
        return Err(ExecutorError::sanitized("runtime_secret_dir_relative"));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o777 != 0o700
            {
                return Err(ExecutorError::sanitized("runtime_secret_dir_unsafe"));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| ExecutorError::sanitized("runtime_secret_parent_missing"))?;
            verify_root_directory(parent)?;
            fs::create_dir(path)
                .map_err(|_| ExecutorError::sanitized("runtime_secret_dir_create_failed"))?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| ExecutorError::sanitized("runtime_secret_dir_chmod_failed"))?;
            sync_directory(parent)
                .map_err(|_| ExecutorError::sanitized("runtime_secret_parent_sync_failed"))?;
        }
        Err(_) => {
            return Err(ExecutorError::sanitized(
                "runtime_secret_dir_inspect_failed",
            ));
        }
    }
    Ok(())
}

fn curl_config_escape(value: &str) -> Result<String, ExecutorError> {
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(ExecutorError::sanitized(
            "curl_credential_control_character",
        ));
    }
    Ok(value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t"))
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn verify_root_directory(path: &Path) -> Result<(), ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("root_directory_metadata_failed"))?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(ExecutorError::sanitized("root_directory_unsafe"));
    }
    Ok(())
}

fn verify_root_owner_file(path: &Path, mode: u32) -> Result<(), ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("root_file_metadata_failed"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o777 != mode
    {
        return Err(ExecutorError::sanitized("root_file_unsafe"));
    }
    Ok(())
}

fn verify_release_tree(root: &Path) -> Result<bool, ExecutorError> {
    const FORBIDDEN_LEGACY_FILES: &[&str] = &[
        "umi.js",
        "umi.css",
        "components.chunk.css",
        "vendors.async.js",
        "components.async.js",
        "env.example.js",
        "custom.css",
        "custom.js",
    ];
    let mut pending = vec![root.to_path_buf()];
    let mut seen = 0_usize;
    while let Some(path) = pending.pop() {
        seen = seen.saturating_add(1);
        if seen > MAX_RELEASE_ENTRIES {
            return Err(ExecutorError::sanitized("release_tree_too_large"));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| ExecutorError::sanitized("release_tree_metadata_failed"))?;
        if path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| FORBIDDEN_LEGACY_FILES.contains(&name))
        {
            return Ok(false);
        }
        if metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
            return Ok(false);
        }
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)
                .map_err(|_| ExecutorError::sanitized("release_symlink_read_failed"))?;
            let parent = path
                .parent()
                .ok_or_else(|| ExecutorError::sanitized("release_symlink_parent_missing"))?;
            let resolved = if target.is_absolute() {
                target
            } else {
                parent.join(target)
            };
            let canonical = fs::canonicalize(resolved)
                .map_err(|_| ExecutorError::sanitized("release_symlink_invalid"))?;
            if !canonical.starts_with(root) {
                return Ok(false);
            }
        } else if metadata.is_dir() {
            for entry in fs::read_dir(&path)
                .map_err(|_| ExecutorError::sanitized("release_tree_read_failed"))?
            {
                pending.push(
                    entry
                        .map_err(|_| ExecutorError::sanitized("release_tree_read_failed"))?
                        .path(),
                );
            }
        } else if !metadata.is_file() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn exact_directory_entries(path: &Path, expected: &[&str]) -> Result<bool, ExecutorError> {
    let mut actual = fs::read_dir(path)
        .map_err(|_| ExecutorError::sanitized("release_directory_read_failed"))?
        .map(|entry| {
            entry
                .map_err(|_| ExecutorError::sanitized("release_directory_read_failed"))?
                .file_name()
                .into_string()
                .map_err(|_| ExecutorError::sanitized("release_filename_invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    actual.sort();
    let mut expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
    expected.sort();
    Ok(actual == expected)
}

fn release_frontend_valid(root: &Path) -> Result<bool, ExecutorError> {
    let current = root.join("frontend/current");
    let metadata = fs::symlink_metadata(&current)
        .map_err(|_| ExecutorError::sanitized("frontend_current_missing"))?;
    if !metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let canonical = fs::canonicalize(&current)
        .map_err(|_| ExecutorError::sanitized("frontend_current_invalid"))?;
    if !canonical.starts_with(root.join("frontend/releases")) {
        return Ok(false);
    }
    Ok(
        bounded_regular_file(&canonical.join("user/index.html"), 1, 1024 * 1024)?
            && bounded_regular_file(&canonical.join("admin/index.html"), 1, 1024 * 1024)?,
    )
}

fn bounded_regular_file(path: &Path, minimum: u64, maximum: u64) -> Result<bool, ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("release_file_metadata_failed"))?;
    Ok(metadata.file_type().is_file()
        && !metadata.file_type().is_symlink()
        && (minimum..=maximum).contains(&metadata.len()))
}

fn read_bounded_regular_utf8(path: &Path, maximum: usize) -> Result<String, ExecutorError> {
    String::from_utf8(read_regular_limited(path, maximum)?)
        .map_err(|_| ExecutorError::sanitized("release_text_not_utf8"))
}

#[cfg(test)]
fn restore_release_link(receipt: &ReleaseSwitchReceipt) -> Result<(), ExecutorError> {
    let current = Path::new(CURRENT_RELEASE_PATH);
    if receipt.current_path != current {
        return Err(ExecutorError::sanitized("release_restore_path_invalid"));
    }
    let parent = current
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("release_restore_parent_missing"))?;
    let temporary = parent.join(format!(".current.{}.restore", receipt.operation_id));
    if fs::symlink_metadata(&temporary).is_ok() {
        return Err(ExecutorError::sanitized("release_restore_temp_exists"));
    }
    match &receipt.previous_target {
        Some(previous) => {
            let canonical = fs::canonicalize(previous)
                .map_err(|_| ExecutorError::sanitized("previous_release_invalid"))?;
            if canonical != *previous || !canonical.starts_with(RELEASES_ROOT) {
                return Err(ExecutorError::sanitized("previous_release_invalid"));
            }
            symlink(previous, &temporary)
                .map_err(|_| ExecutorError::sanitized("release_restore_symlink_failed"))?;
            fs::rename(&temporary, current)
                .map_err(|_| ExecutorError::sanitized("release_restore_rename_failed"))?;
        }
        None => {
            fs::remove_file(current)
                .map_err(|_| ExecutorError::sanitized("new_release_link_remove_failed"))?;
        }
    }
    sync_directory(parent)
        .map_err(|_| ExecutorError::sanitized("release_restore_parent_sync_failed"))
}

fn verify_current_release(expected: &Path) -> Result<(), ExecutorError> {
    let current = fs::canonicalize(CURRENT_RELEASE_PATH)
        .map_err(|_| ExecutorError::sanitized("current_release_verify_failed"))?;
    if current != expected {
        return Err(ExecutorError::sanitized("current_release_binding_mismatch"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg(test)]
struct UnitStartAttemptRecord {
    schema_version: u32,
    operation_id: String,
    installation_id: String,
    permit_generation: u64,
    permit_event_sha256: String,
    unit: String,
    status: String,
}

#[cfg(test)]
fn unit_start_attempt_path(
    permit: &DurableTargetMutationPermit,
    unit: &str,
) -> Result<PathBuf, ExecutorError> {
    let root = Path::new("/var/lib/v2board/lifecycle/activation");
    let operation_dir = root.join(permit.operation_id());
    let filename = match unit {
        API_UNIT => "v2board-api.start.json",
        WORKER_UNIT => "v2board-worker.start.json",
        _ => return Err(ExecutorError::sanitized("systemd_unit_not_allowed")),
    };
    Ok(operation_dir.join(filename))
}

#[cfg(test)]
fn expected_unit_start_attempt(
    permit: &DurableTargetMutationPermit,
    unit: &str,
) -> UnitStartAttemptRecord {
    UnitStartAttemptRecord {
        schema_version: 1,
        operation_id: permit.operation_id().to_string(),
        installation_id: permit.installation_id().to_string(),
        permit_generation: permit.generation(),
        permit_event_sha256: permit.event_sha256().to_string(),
        unit: unit.to_string(),
        status: "start_intent_durable".to_string(),
    }
}

#[cfg(test)]
fn verify_unit_start_attempt(
    permit: &DurableTargetMutationPermit,
    unit: &str,
) -> Result<bool, ExecutorError> {
    let path = unit_start_attempt_path(permit, unit)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let bytes = read_regular_limited(&path, 16 * 1024)?;
            let observed = serde_json::from_slice::<UnitStartAttemptRecord>(&bytes)
                .map_err(|_| ExecutorError::sanitized("unit_start_marker_invalid"))?;
            if observed != expected_unit_start_attempt(permit, unit) {
                return Err(ExecutorError::sanitized(
                    "unit_start_marker_binding_mismatch",
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ExecutorError::sanitized("unit_start_marker_inspect_failed")),
    }
}

#[cfg(test)]
fn persist_or_verify_unit_start_attempt(
    permit: &DurableTargetMutationPermit,
    unit: &str,
) -> Result<(), ExecutorError> {
    let root = Path::new("/var/lib/v2board/lifecycle/activation");
    prepare_root_private_path(root)?;
    let operation_dir = root.join(permit.operation_id());
    prepare_root_private_path(&operation_dir)?;
    let path = unit_start_attempt_path(permit, unit)?;
    let bytes = serde_json::to_vec(&expected_unit_start_attempt(permit, unit))
        .map_err(|_| ExecutorError::sanitized("unit_start_marker_serialize_failed"))?;
    match write_new_owner_file(&path, &bytes, 0o600) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            verify_unit_start_attempt(permit, unit).and_then(|present| {
                if present {
                    Ok(())
                } else {
                    Err(ExecutorError::sanitized("unit_start_marker_disappeared"))
                }
            })
        }
        Err(_) => Err(ExecutorError::sanitized("unit_start_marker_write_failed")),
    }
}

fn prepare_root_private_path(path: &Path) -> Result<(), ExecutorError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o777 != 0o700
            {
                return Err(ExecutorError::sanitized("activation_state_dir_unsafe"));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| ExecutorError::sanitized("activation_state_parent_missing"))?;
            verify_root_directory(parent)?;
            fs::create_dir(path)
                .map_err(|_| ExecutorError::sanitized("activation_state_dir_create_failed"))?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| ExecutorError::sanitized("activation_state_dir_chmod_failed"))?;
            sync_directory(parent)
                .map_err(|_| ExecutorError::sanitized("activation_state_parent_sync_failed"))?;
        }
        Err(_) => {
            return Err(ExecutorError::sanitized(
                "activation_state_dir_inspect_failed",
            ));
        }
    }
    Ok(())
}

fn recent_regular_health_file(path: &Path) -> Result<bool, ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("worker_health_metadata_failed"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() == 0
        || metadata.len() > 4096
    {
        return Ok(false);
    }
    let modified = metadata
        .modified()
        .map_err(|_| ExecutorError::sanitized("worker_health_timestamp_failed"))?;
    Ok(SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age <= Duration::from_secs(30)))
}

/// Reconstructed, post-authority native service evidence. Construction is
/// possible only from a `DurableNativeStartPermit`; the target-mutation permit
/// used by the retired monolithic activation path is deliberately not
/// accepted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct PostAuthorityNativeServiceReport {
    pub api: ServiceReadinessProof,
    pub worker: ServiceReadinessProof,
    pub config_rollback_cleanup: ConfigRollbackCleanupProof,
    pub release_archive_sha256: String,
    pub release_source_revision: String,
    pub authority_nodes_generation: u64,
    pub authority_nodes_event_sha256: String,
}

/// Forward-only cleanup proof emitted only after both native roles have passed
/// readiness. It is serialized into the journaled native-start stage proof, so
/// a success cannot leave a plaintext pre-cutover config backup behind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ConfigRollbackCleanupProof {
    pub operation_id: String,
    pub artifact_count: u8,
    pub all_artifacts_absent: bool,
    pub parent_directories_fsynced: bool,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeStartIntent {
    schema_version: u32,
    operation_id: String,
    installation_id: String,
    release_id: String,
    authority_nodes_generation: u64,
    authority_nodes_event_sha256: String,
    unit: String,
    state: String,
}

/// Atomically promotes the verified release and starts API then worker. Every
/// logical start intent is fsync-durable before `systemctl start`; retries
/// compare the authority anchor and accept an already-active unit only when
/// that exact marker exists. No datastore secret is placed in argv or env.
pub(crate) fn start_native_units_after_authority(
    spec: &ProvisionSpec,
    permit: &DurableNativeStartPermit,
) -> Result<PostAuthorityNativeServiceReport, ExecutorError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or_else(|| ExecutorError::sanitized("native_start_execution_missing"))?;
    if permit.operation_id() != spec.operation_id
        || permit.installation_id().is_empty()
        || !is_lower_sha256(permit.inspect_review_sha256())
        || !is_lower_sha256(permit.event_sha256())
        || !is_lower_sha256(permit.checkpoint_proof_sha256())
        || execution.systemd.api_unit != API_UNIT
        || execution.systemd.worker_unit != WORKER_UNIT
        || execution.systemd.api_ready_url != "http://127.0.0.1:8080/readyz"
        || execution.systemd.worker_health_path != Path::new(WORKER_HEALTH_PATH)
    {
        return Err(ExecutorError::sanitized("native_start_permit_invalid"));
    }
    ensure_effective_root(&mut ProcessCommandRunner)?;
    let release = verify_release_artifact_for_one_shot(spec)?;
    let release_metadata = read_release_metadata(release.staged_path())?;
    verify_materialized_role_configs(spec)?;
    let mut runner = ProcessCommandRunner;
    verify_installed_native_systemd_units_exact(release.staged_path())?;
    verify_effective_native_units(&mut runner)?;
    switch_release_forward_only(permit.operation_id(), release.staged_path())?;

    start_marked_unit(
        &mut runner,
        execution.journal.activation_state_root.as_path(),
        permit,
        release.release_id(),
        API_UNIT,
    )?;
    wait_api_ready(
        &mut runner,
        execution.systemd.api_ready_url.as_str(),
        release.staged_path(),
    )?;
    let api = ServiceReadinessProof {
        operation_id: permit.operation_id().to_string(),
        unit: API_UNIT.to_string(),
        installation_id: permit.installation_id().to_string(),
        release_id: release.release_id().to_string(),
        postgres_ledger_exactly_current: true,
        runtime_role_and_config_verified: true,
        ready: true,
        systemd_notify_ready: None,
        watchdog_healthy: None,
    };

    start_marked_unit(
        &mut runner,
        execution.journal.activation_state_root.as_path(),
        permit,
        release.release_id(),
        WORKER_UNIT,
    )?;
    wait_worker_ready(
        &mut runner,
        execution.systemd.worker_health_path.as_path(),
        release.staged_path(),
    )?;
    let worker = ServiceReadinessProof {
        operation_id: permit.operation_id().to_string(),
        unit: WORKER_UNIT.to_string(),
        installation_id: permit.installation_id().to_string(),
        release_id: release.release_id().to_string(),
        postgres_ledger_exactly_current: true,
        runtime_role_and_config_verified: true,
        ready: true,
        systemd_notify_ready: Some(true),
        watchdog_healthy: Some(true),
    };
    let config_rollback_cleanup = cleanup_config_rollback_artifacts(permit.operation_id())?;
    Ok(PostAuthorityNativeServiceReport {
        api,
        worker,
        config_rollback_cleanup,
        release_archive_sha256: release.external_archive_sha256().to_string(),
        release_source_revision: release_metadata.source_revision,
        authority_nodes_generation: permit
            .native_authority_binding()
            .nodes_verified_generation(),
        authority_nodes_event_sha256: permit
            .native_authority_binding()
            .nodes_verified_event_sha256()
            .to_string(),
    })
}

/// Final pre-authority observation. It never starts or enables a unit and
/// rejects a durable native-start marker before PostgreSQL's activation CAS.
pub(crate) fn verify_native_units_stopped_before_authority(
    spec: &ProvisionSpec,
) -> Result<String, ExecutorError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or_else(|| ExecutorError::sanitized("native_stop_execution_missing"))?;
    if execution.systemd.api_unit != API_UNIT || execution.systemd.worker_unit != WORKER_UNIT {
        return Err(ExecutorError::sanitized("native_unit_binding_invalid"));
    }
    ensure_effective_root(&mut ProcessCommandRunner)?;
    let release = verify_release_artifact_for_one_shot(spec)?;
    let release_metadata = read_release_metadata(release.staged_path())?;
    verify_materialized_role_configs(spec)?;
    let mut runner = ProcessCommandRunner;
    verify_native_units_inactive_and_disabled(&mut runner)?;
    let operation_dir = execution
        .journal
        .activation_state_root
        .join(&spec.operation_id);
    for filename in [
        "native-api.start.json",
        "native-worker.start.json",
        "v2board-api.start.json",
        "v2board-worker.start.json",
    ] {
        match fs::symlink_metadata(operation_dir.join(filename)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(ExecutorError::sanitized(
                    "native_start_marker_exists_before_authority",
                ));
            }
            Err(_) => {
                return Err(ExecutorError::sanitized("native_start_marker_probe_failed"));
            }
        }
    }
    install_native_systemd_units(&mut runner, &spec.operation_id, release.staged_path())?;
    verify_native_units_inactive_and_disabled(&mut runner)?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-native-units-stopped-before-authority-v2\0");
    digest.update(spec.operation_id.as_bytes());
    digest.update(release.release_id().as_bytes());
    digest.update(release.external_archive_sha256().as_bytes());
    digest.update(release_metadata.source_revision.as_bytes());
    digest.update(API_UNIT.as_bytes());
    digest.update(WORKER_UNIT.as_bytes());
    Ok(hex::encode(digest.finalize()))
}

pub(crate) fn verify_native_runtime_after_cutover(
    spec: &ProvisionSpec,
) -> Result<String, ExecutorError> {
    let release = verify_release_artifact_for_one_shot(spec)?;
    let release_metadata = read_release_metadata(release.staged_path())?;
    verify_current_release(release.staged_path())?;
    verify_materialized_role_configs(spec)?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or_else(|| ExecutorError::sanitized("native_runtime_execution_missing"))?;
    let mut runner = ProcessCommandRunner;
    verify_installed_native_systemd_units_exact(release.staged_path())?;
    verify_effective_native_units(&mut runner)?;
    for unit in [API_UNIT, WORKER_UNIT] {
        run_native_success(
            &mut runner,
            NativeCommandRequest::new(NativeProgram::Systemctl)
                .arg("is-enabled")
                .arg("--quiet")
                .arg(unit),
            "native_runtime_unit_disabled",
        )?;
    }
    wait_api_ready(
        &mut runner,
        execution.systemd.api_ready_url.as_str(),
        release.staged_path(),
    )?;
    wait_worker_ready(
        &mut runner,
        execution.systemd.worker_health_path.as_path(),
        release.staged_path(),
    )?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-native-runtime-after-cutover-v2\0");
    digest.update(spec.operation_id.as_bytes());
    digest.update(release.release_id().as_bytes());
    digest.update(release.external_archive_sha256().as_bytes());
    digest.update(release_metadata.source_revision.as_bytes());
    Ok(hex::encode(digest.finalize()))
}

pub(crate) fn verify_release_artifact_for_one_shot(
    spec: &ProvisionSpec,
) -> Result<ReleaseArtifactSpec, ExecutorError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or_else(|| ExecutorError::sanitized("release_execution_missing"))?;
    let release = ReleaseArtifactSpec::new(
        &execution.release.release_id,
        &execution.release.archive_sha256,
    )
    .map_err(|_| ExecutorError::sanitized("release_binding_invalid"))?;
    if release.staged_path()
        != execution
            .release
            .releases_root
            .join(&execution.release.release_id)
        || execution.release.current_symlink != Path::new(CURRENT_RELEASE_PATH)
    {
        return Err(ExecutorError::sanitized("release_path_binding_invalid"));
    }
    let receipt = ReceiptBinding::new(
        execution.receipts.release_archive.path.clone(),
        execution.receipts.release_archive.sha256.clone(),
    )
    .map_err(|_| ExecutorError::sanitized("release_receipt_binding_invalid"))?;
    load_external_receipt(
        &receipt,
        &spec.operation_id,
        ExternalReceiptKind::ReleaseArchiveVerified,
        release.external_archive_sha256(),
    )?;
    let mut runner = ProcessCommandRunner;
    materialize_verified_release_archive(
        &mut runner,
        &spec.operation_id,
        &execution.release.archive_path,
        release.external_archive_sha256(),
        release.staged_path(),
    )?;
    Ok(release)
}

/// Extracts the manifest-bound archive into an operation-owned private tree,
/// validates every release contract there, and only then publishes it. A
/// retry never trusts a pre-existing release directory: it re-extracts the
/// same archive and requires an exact whole-tree digest match before accepting
/// that directory.
fn materialize_verified_release_archive(
    runner: &mut impl NativeCommandRunner,
    operation_id: &str,
    archive_path: &Path,
    expected_archive_sha256: &str,
    staged_path: &Path,
) -> Result<(), ExecutorError> {
    let releases_root = Path::new(RELEASES_ROOT);
    verify_root_directory(releases_root)?;
    if staged_path.parent() != Some(releases_root) {
        return Err(ExecutorError::sanitized("release_path_binding_invalid"));
    }
    let release_id = staged_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ExecutorError::sanitized("release_id_invalid"))?;
    let extraction_path = releases_root.join(format!(
        ".native-release.{operation_id}.{release_id}.extracting"
    ));
    remove_private_extraction_tree_if_present(&extraction_path)?;
    fs::create_dir(&extraction_path)
        .map_err(|_| ExecutorError::sanitized("release_extract_dir_create_failed"))?;
    fs::set_permissions(&extraction_path, fs::Permissions::from_mode(0o700))
        .map_err(|_| ExecutorError::sanitized("release_extract_dir_chmod_failed"))?;
    sync_directory(releases_root)
        .map_err(|_| ExecutorError::sanitized("release_extract_parent_sync_failed"))?;

    let extraction =
        safely_extract_release_archive(archive_path, expected_archive_sha256, &extraction_path);
    if let Err(error) = extraction {
        remove_private_extraction_tree_if_present(&extraction_path)?;
        return Err(error);
    }

    let extracted_digest = verify_materialized_release_tree(runner, &extraction_path)?;
    sync_release_tree(&extraction_path)?;

    match fs::symlink_metadata(staged_path) {
        Ok(_) => {
            let staged_digest = verify_materialized_release_tree(runner, staged_path)?;
            remove_private_extraction_tree_if_present(&extraction_path)?;
            if staged_digest != extracted_digest {
                return Err(ExecutorError::sanitized(
                    "release_existing_tree_digest_mismatch",
                ));
            }
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(ExecutorError::sanitized(
                "release_existing_tree_inspect_failed",
            ));
        }
    }

    if let Err(error) = fs::rename(&extraction_path, staged_path) {
        // A root-only concurrent/retried publisher may have won the race. It
        // is accepted only when it is byte-for-byte and topology-for-topology
        // identical to this archive extraction; otherwise publication fails.
        if fs::symlink_metadata(staged_path).is_ok() {
            let staged_digest = verify_materialized_release_tree(runner, staged_path)?;
            remove_private_extraction_tree_if_present(&extraction_path)?;
            if staged_digest == extracted_digest {
                return Ok(());
            }
            return Err(ExecutorError::sanitized(
                "release_publish_tree_digest_mismatch",
            ));
        }
        remove_private_extraction_tree_if_present(&extraction_path)?;
        let _ = error;
        return Err(ExecutorError::sanitized("release_atomic_publish_failed"));
    }
    sync_directory(releases_root)
        .map_err(|_| ExecutorError::sanitized("release_publish_parent_sync_failed"))?;
    let published_digest = verify_materialized_release_tree(runner, staged_path)?;
    if published_digest != extracted_digest {
        return Err(ExecutorError::sanitized(
            "release_published_tree_digest_mismatch",
        ));
    }
    Ok(())
}

fn verify_materialized_release_tree(
    runner: &mut impl NativeCommandRunner,
    root: &Path,
) -> Result<String, ExecutorError> {
    let canonical =
        fs::canonicalize(root).map_err(|_| ExecutorError::sanitized("release_path_unavailable"))?;
    if canonical != root
        || canonical.parent() != Some(Path::new(RELEASES_ROOT))
        || !verify_release_tree(&canonical)?
        || !exact_directory_entries(
            &canonical.join("bin"),
            &["v2board-api", "v2board-workers", "v2board-analytics-schema"],
        )?
        || !release_frontend_valid(&canonical)?
        || !bounded_regular_file(&canonical.join("RELEASE"), 1, 64 * 1024)?
    {
        return Err(ExecutorError::sanitized("release_tree_invalid"));
    }
    read_release_metadata(&canonical)?;
    run_native_success(
        runner,
        NativeCommandRequest::new(NativeProgram::Sha256sum)
            .arg("--check")
            .arg("--strict")
            .arg("--quiet")
            .arg("SHA256SUMS")
            .current_dir(&canonical),
        "release_internal_checksum_failed",
    )?;
    let api_unit = canonical.join("systemd/v2board-api.service");
    let worker_unit = canonical.join("systemd/v2board-worker.service");
    run_native_success(
        runner,
        NativeCommandRequest::new(NativeProgram::SystemdAnalyze)
            .arg("verify")
            .arg(&api_unit)
            .arg(&worker_unit),
        "systemd_unit_verify_failed",
    )?;
    let api_text = read_bounded_regular_utf8(&api_unit, 128 * 1024)?;
    let worker_text = read_bounded_regular_utf8(&worker_unit, 128 * 1024)?;
    validate_release_systemd_contract(&api_text, &worker_text)?;
    release_tree_digest(&canonical)
}

const CANONICAL_API_UNIT_BYTES: &[u8] =
    include_bytes!("../../../../../deploy/systemd/v2board-api.service");
const CANONICAL_WORKER_UNIT_BYTES: &[u8] =
    include_bytes!("../../../../../deploy/systemd/v2board-worker.service");

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeReleaseMetadata {
    source_revision: String,
}

fn read_release_metadata(root: &Path) -> Result<NativeReleaseMetadata, ExecutorError> {
    let bytes = read_regular_limited(&root.join("RELEASE"), 64 * 1024)?;
    parse_release_metadata(&bytes)
}

fn parse_release_metadata(bytes: &[u8]) -> Result<NativeReleaseMetadata, ExecutorError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ExecutorError::sanitized("release_text_not_utf8"))?;
    if text.contains('\r') || text.as_bytes().contains(&0) {
        return Err(ExecutorError::sanitized("release_metadata_invalid"));
    }
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| ExecutorError::sanitized("release_metadata_line_invalid"))?;
        if key.is_empty()
            || value.is_empty()
            || !matches!(
                key,
                "format" | "source_revision" | "target_os" | "target_arch"
            )
            || fields.insert(key, value).is_some()
        {
            return Err(ExecutorError::sanitized("release_metadata_field_invalid"));
        }
    }
    if fields.len() != 4
        || fields.get("format") != Some(&"v2board-native-release-v1")
        || fields.get("target_os") != Some(&"linux")
        || fields.get("target_arch") != Some(&"amd64")
    {
        return Err(ExecutorError::sanitized(
            "release_metadata_contract_invalid",
        ));
    }
    let source_revision = fields
        .get("source_revision")
        .copied()
        .ok_or_else(|| ExecutorError::sanitized("release_source_revision_missing"))?;
    if source_revision.len() != 40
        || !source_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ExecutorError::sanitized("release_source_revision_invalid"));
    }
    Ok(NativeReleaseMetadata {
        source_revision: source_revision.to_string(),
    })
}

/// Fully read-only verification of the exact manifest-bound native release.
/// It opens one root-owned archive inode, hashes and indexes that same file,
/// and never creates an extraction directory or writes under `/opt`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadOnlyReleaseArchiveInspection {
    pub release_id: String,
    pub archive_sha256: String,
    pub source_revision: String,
    pub entry_count: u64,
    pub regular_file_count: u64,
    pub internal_checksum_count: u64,
    pub virtual_tree_sha256: String,
    pub complete_structure_verified: bool,
    pub internal_sha256sums_verified: bool,
    pub systemd_contract_verified: bool,
    pub target_filesystem_unchanged: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndexedReleaseEntryKind {
    Regular,
    Directory,
    Symlink,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedReleaseEntry {
    kind: IndexedReleaseEntryKind,
    mode: u32,
    size: u64,
    sha256: Option<String>,
    captured: Option<Vec<u8>>,
    link_target: Option<PathBuf>,
}

/// Admission-time archive verification. The immutable receipt, archive inode,
/// manifest hash, every tar entry, and every internal checksum are bound into
/// the returned inspection. This function has no target mutation path.
pub(crate) fn inspect_release_archive_read_only(
    spec: &ProvisionSpec,
) -> Result<ReadOnlyReleaseArchiveInspection, ExecutorError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or_else(|| ExecutorError::sanitized("release_execution_missing"))?;
    let release = ReleaseArtifactSpec::new(
        &execution.release.release_id,
        &execution.release.archive_sha256,
    )
    .map_err(|_| ExecutorError::sanitized("release_binding_invalid"))?;
    let receipt = ReceiptBinding::new(
        execution.receipts.release_archive.path.clone(),
        execution.receipts.release_archive.sha256.clone(),
    )
    .map_err(|_| ExecutorError::sanitized("release_receipt_binding_invalid"))?;
    load_external_receipt(
        &receipt,
        &spec.operation_id,
        ExternalReceiptKind::ReleaseArchiveVerified,
        release.external_archive_sha256(),
    )?;
    inspect_bound_release_archive_read_only(&execution.release.archive_path, &release)
}

/// Standalone, mutation-free audit of a native release archive against the
/// same inode, digest, tar-tree, checksum, frontend, and systemd contract used
/// by lifecycle admission. This does not issue an authorization or replace the
/// manifest-bound external receipt required by apply/resume.
pub fn inspect_native_release_archive_read_only(
    archive_path: &Path,
    release_id: &str,
    expected_archive_sha256: &str,
) -> Result<ReadOnlyReleaseArchiveInspection, ExecutorError> {
    let release = ReleaseArtifactSpec::new(release_id, expected_archive_sha256)
        .map_err(|_| ExecutorError::sanitized("release_binding_invalid"))?;
    inspect_bound_release_archive_read_only(archive_path, &release)
}

fn inspect_bound_release_archive_read_only(
    archive_path: &Path,
    release: &ReleaseArtifactSpec,
) -> Result<ReadOnlyReleaseArchiveInspection, ExecutorError> {
    let mut file = open_root_owned_release_archive(archive_path)?;
    let archive_sha256 = hash_open_release_archive(&mut file)?;
    if archive_sha256 != release.external_archive_sha256() {
        return Err(ExecutorError::sanitized("release_archive_digest_mismatch"));
    }
    let inspection =
        inspect_open_release_archive(&mut file, release.release_id(), &archive_sha256)?;
    if hash_open_release_archive(&mut file)? != archive_sha256 {
        return Err(ExecutorError::sanitized(
            "release_archive_changed_during_inspection",
        ));
    }
    let opened = file
        .metadata()
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?;
    let current = fs::symlink_metadata(archive_path)
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?;
    if opened.dev() != current.dev() || opened.ino() != current.ino() {
        return Err(ExecutorError::sanitized("release_archive_path_replaced"));
    }
    Ok(inspection)
}

const MAX_RELEASE_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024 * 1024;

fn open_root_owned_release_archive(path: &Path) -> Result<File, ExecutorError> {
    let file =
        File::open(path).map_err(|_| ExecutorError::sanitized("release_archive_open_failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?;
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?;
    if !metadata.is_file()
        || !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || metadata.dev() != path_metadata.dev()
        || metadata.ino() != path_metadata.ino()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o777 != 0o400
        || metadata.len() == 0
        || metadata.len() > MAX_RELEASE_ARCHIVE_BYTES
    {
        return Err(ExecutorError::sanitized("release_archive_unsafe"));
    }
    Ok(file)
}

fn hash_open_release_archive(file: &mut File) -> Result<String, ExecutorError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ExecutorError::sanitized("release_archive_seek_failed"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| ExecutorError::sanitized("release_archive_read_failed"))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| ExecutorError::sanitized("release_archive_size_overflow"))?;
        if total > MAX_RELEASE_ARCHIVE_BYTES {
            return Err(ExecutorError::sanitized("release_archive_unsafe"));
        }
        digest.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ExecutorError::sanitized("release_archive_seek_failed"))?;
    Ok(hex::encode(digest.finalize()))
}

fn inspect_open_release_archive(
    file: &mut File,
    release_id: &str,
    archive_sha256: &str,
) -> Result<ReadOnlyReleaseArchiveInspection, ExecutorError> {
    const MAX_RELEASE_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
    const MAX_RELEASE_UNPACKED_BYTES: u64 = 32 * 1024 * 1024 * 1024;
    const MAX_SHA256SUMS_BYTES: u64 = 32 * 1024 * 1024;
    const FORBIDDEN_LEGACY_FILES: &[&str] = &[
        "umi.js",
        "umi.css",
        "components.chunk.css",
        "vendors.async.js",
        "components.async.js",
        "env.example.js",
        "custom.css",
        "custom.js",
    ];

    file.seek(SeekFrom::Start(0))
        .map_err(|_| ExecutorError::sanitized("release_archive_seek_failed"))?;
    let compressed_size = file
        .metadata()
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?
        .len();
    let decoder = flate2::bufread::GzDecoder::new(BufReader::new(&mut *file));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|_| ExecutorError::sanitized("release_archive_entries_invalid"))?;
    let mut indexed = BTreeMap::<PathBuf, IndexedReleaseEntry>::new();
    let mut total_size = 0_u64;
    let mut entry_count = 0_usize;
    for entry in entries {
        let mut entry =
            entry.map_err(|_| ExecutorError::sanitized("release_archive_entry_invalid"))?;
        entry_count = entry_count.saturating_add(1);
        if entry_count > MAX_RELEASE_ENTRIES {
            return Err(ExecutorError::sanitized("release_archive_too_many_entries"));
        }
        let entry_type = entry.header().entry_type();
        let kind = match entry_type {
            EntryType::Regular => IndexedReleaseEntryKind::Regular,
            EntryType::Directory => IndexedReleaseEntryKind::Directory,
            EntryType::Symlink => IndexedReleaseEntryKind::Symlink,
            _ => {
                return Err(ExecutorError::sanitized(
                    "release_archive_entry_type_forbidden",
                ));
            }
        };
        let path = entry
            .path()
            .map_err(|_| ExecutorError::sanitized("release_archive_path_invalid"))?;
        let normalized = normalize_release_archive_path(path.as_ref())?;
        if normalized.as_os_str().is_empty() {
            if kind != IndexedReleaseEntryKind::Directory {
                return Err(ExecutorError::sanitized("release_archive_path_empty"));
            }
            continue;
        }
        if normalized.to_str().is_none()
            || normalized
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| FORBIDDEN_LEGACY_FILES.contains(&name))
            || indexed.contains_key(&normalized)
        {
            return Err(ExecutorError::sanitized("release_archive_path_invalid"));
        }
        let mode = entry
            .header()
            .mode()
            .map_err(|_| ExecutorError::sanitized("release_archive_mode_invalid"))?;
        let expected_mode = match kind {
            IndexedReleaseEntryKind::Directory => 0o755,
            IndexedReleaseEntryKind::Symlink => 0o777,
            IndexedReleaseEntryKind::Regular if normalized.parent() == Some(Path::new("bin")) => {
                0o755
            }
            IndexedReleaseEntryKind::Regular => 0o644,
        };
        if mode != expected_mode {
            return Err(ExecutorError::sanitized("release_archive_mode_invalid"));
        }
        let size = entry.size();
        if size > MAX_RELEASE_FILE_BYTES || (kind != IndexedReleaseEntryKind::Regular && size != 0)
        {
            return Err(ExecutorError::sanitized(
                "release_archive_entry_size_invalid",
            ));
        }
        total_size = total_size
            .checked_add(size)
            .ok_or_else(|| ExecutorError::sanitized("release_archive_size_overflow"))?;
        if total_size > MAX_RELEASE_UNPACKED_BYTES {
            return Err(ExecutorError::sanitized(
                "release_archive_unpacked_size_exceeded",
            ));
        }

        let (sha256, captured, link_target) = match kind {
            IndexedReleaseEntryKind::Regular => {
                if entry.link_name_bytes().is_some() {
                    return Err(ExecutorError::sanitized(
                        "release_archive_unexpected_link_target",
                    ));
                }
                let capture_limit = match normalized.to_str() {
                    Some("SHA256SUMS") => Some(MAX_SHA256SUMS_BYTES),
                    Some("RELEASE") => Some(64 * 1024),
                    Some("systemd/v2board-api.service" | "systemd/v2board-worker.service") => {
                        Some(128 * 1024)
                    }
                    _ => None,
                };
                if capture_limit.is_some_and(|limit| size == 0 || size > limit) {
                    return Err(ExecutorError::sanitized(
                        "release_archive_contract_file_size_invalid",
                    ));
                }
                let mut digest = Sha256::new();
                let mut captured = capture_limit.map(|_| Vec::with_capacity(size as usize));
                let mut read_size = 0_u64;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = entry.read(&mut buffer).map_err(|_| {
                        ExecutorError::sanitized("release_archive_entry_read_failed")
                    })?;
                    if count == 0 {
                        break;
                    }
                    read_size = read_size
                        .checked_add(count as u64)
                        .ok_or_else(|| ExecutorError::sanitized("release_archive_size_overflow"))?;
                    digest.update(&buffer[..count]);
                    if let Some(bytes) = captured.as_mut() {
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                }
                if read_size != size {
                    return Err(ExecutorError::sanitized(
                        "release_archive_entry_size_mismatch",
                    ));
                }
                (Some(hex::encode(digest.finalize())), captured, None)
            }
            IndexedReleaseEntryKind::Directory => {
                if entry.link_name_bytes().is_some() {
                    return Err(ExecutorError::sanitized(
                        "release_archive_unexpected_link_target",
                    ));
                }
                (None, None, None)
            }
            IndexedReleaseEntryKind::Symlink => {
                validate_release_archive_symlink(&normalized, &entry)?;
                let target = entry
                    .link_name()
                    .map_err(|_| {
                        ExecutorError::sanitized("release_archive_symlink_target_invalid")
                    })?
                    .ok_or_else(|| {
                        ExecutorError::sanitized("release_archive_symlink_target_missing")
                    })?
                    .into_owned();
                (None, None, Some(target))
            }
        };
        indexed.insert(
            normalized,
            IndexedReleaseEntry {
                kind,
                mode,
                size,
                sha256,
                captured,
                link_target,
            },
        );
    }
    // `tar` stops at its end markers. Drain the gzip member and prove that the
    // only remaining uncompressed tar padding is zero, then prove there is no
    // second gzip member or opaque compressed suffix outside the indexed tree.
    let mut decoder = archive.into_inner();
    let mut trailing = [0_u8; 64 * 1024];
    loop {
        let count = decoder
            .read(&mut trailing)
            .map_err(|_| ExecutorError::sanitized("release_archive_trailer_invalid"))?;
        if count == 0 {
            break;
        }
        if trailing[..count].iter().any(|byte| *byte != 0) {
            return Err(ExecutorError::sanitized("release_archive_trailing_payload"));
        }
    }
    let mut compressed = decoder.into_inner();
    if !compressed.buffer().is_empty()
        || compressed
            .stream_position()
            .map_err(|_| ExecutorError::sanitized("release_archive_seek_failed"))?
            != compressed_size
    {
        return Err(ExecutorError::sanitized(
            "release_archive_compressed_suffix",
        ));
    }
    if indexed.is_empty() {
        return Err(ExecutorError::sanitized("release_archive_empty"));
    }
    validate_indexed_release_parents(&indexed)?;
    require_exact_indexed_children(
        &indexed,
        Path::new(""),
        &["bin", "frontend", "systemd", "RELEASE", "SHA256SUMS"],
    )?;
    require_exact_indexed_children(
        &indexed,
        Path::new("bin"),
        &["v2board-api", "v2board-workers", "v2board-analytics-schema"],
    )?;
    require_exact_indexed_children(
        &indexed,
        Path::new("systemd"),
        &["v2board-api.service", "v2board-worker.service"],
    )?;
    require_exact_indexed_children(
        &indexed,
        Path::new("frontend"),
        &["current", "previous", "releases"],
    )?;
    for path in [
        "bin/v2board-api",
        "bin/v2board-workers",
        "bin/v2board-analytics-schema",
    ] {
        let binary =
            require_indexed_kind(&indexed, Path::new(path), IndexedReleaseEntryKind::Regular)?;
        if binary.size == 0 || binary.mode & 0o111 == 0 {
            return Err(ExecutorError::sanitized("release_archive_binary_invalid"));
        }
    }
    for path in ["frontend/current", "frontend/previous"] {
        let link =
            require_indexed_kind(&indexed, Path::new(path), IndexedReleaseEntryKind::Symlink)?;
        let resolved = resolve_indexed_release_link(Path::new(path), link)?;
        require_indexed_kind(&indexed, &resolved, IndexedReleaseEntryKind::Directory)?;
        if path == "frontend/current" {
            for index in [
                resolved.join("user/index.html"),
                resolved.join("admin/index.html"),
            ] {
                let file =
                    require_indexed_kind(&indexed, &index, IndexedReleaseEntryKind::Regular)?;
                if !(1..=1024 * 1024).contains(&file.size) {
                    return Err(ExecutorError::sanitized("release_archive_frontend_invalid"));
                }
            }
        }
    }

    let metadata = require_indexed_kind(
        &indexed,
        Path::new("RELEASE"),
        IndexedReleaseEntryKind::Regular,
    )?;
    let release_metadata = parse_release_metadata(
        metadata
            .captured
            .as_deref()
            .ok_or_else(|| ExecutorError::sanitized("release_metadata_missing"))?,
    )?;
    let api_unit = captured_indexed_utf8(&indexed, "systemd/v2board-api.service")?;
    let worker_unit = captured_indexed_utf8(&indexed, "systemd/v2board-worker.service")?;
    validate_release_systemd_contract(&api_unit, &worker_unit)?;
    let internal_checksum_count = verify_indexed_sha256sums(&indexed)?;
    let virtual_tree_sha256 = indexed_release_tree_sha256(&indexed)?;
    let regular_file_count = indexed
        .values()
        .filter(|entry| entry.kind == IndexedReleaseEntryKind::Regular)
        .count();
    Ok(ReadOnlyReleaseArchiveInspection {
        release_id: release_id.to_string(),
        archive_sha256: archive_sha256.to_string(),
        source_revision: release_metadata.source_revision,
        entry_count: u64::try_from(indexed.len())
            .map_err(|_| ExecutorError::sanitized("release_archive_too_many_entries"))?,
        regular_file_count: u64::try_from(regular_file_count)
            .map_err(|_| ExecutorError::sanitized("release_archive_too_many_entries"))?,
        internal_checksum_count,
        virtual_tree_sha256,
        complete_structure_verified: true,
        internal_sha256sums_verified: true,
        systemd_contract_verified: true,
        target_filesystem_unchanged: true,
    })
}

fn validate_indexed_release_parents(
    indexed: &BTreeMap<PathBuf, IndexedReleaseEntry>,
) -> Result<(), ExecutorError> {
    for path in indexed.keys() {
        let mut parent = path.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            require_indexed_kind(indexed, value, IndexedReleaseEntryKind::Directory)?;
            parent = value.parent();
        }
    }
    Ok(())
}

fn require_exact_indexed_children(
    indexed: &BTreeMap<PathBuf, IndexedReleaseEntry>,
    parent: &Path,
    expected: &[&str],
) -> Result<(), ExecutorError> {
    let mut actual = indexed
        .keys()
        .filter(|path| path.parent() == Some(parent))
        .map(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .map(ToString::to_string)
                .ok_or_else(|| ExecutorError::sanitized("release_archive_path_invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    actual.sort();
    let mut expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
    expected.sort();
    if actual != expected {
        return Err(ExecutorError::sanitized(
            "release_archive_directory_entries_invalid",
        ));
    }
    Ok(())
}

fn require_indexed_kind<'a>(
    indexed: &'a BTreeMap<PathBuf, IndexedReleaseEntry>,
    path: &Path,
    kind: IndexedReleaseEntryKind,
) -> Result<&'a IndexedReleaseEntry, ExecutorError> {
    let entry = indexed
        .get(path)
        .ok_or_else(|| ExecutorError::sanitized("release_archive_required_entry_missing"))?;
    if entry.kind != kind {
        return Err(ExecutorError::sanitized(
            "release_archive_required_entry_type_invalid",
        ));
    }
    Ok(entry)
}

fn resolve_indexed_release_link(
    path: &Path,
    entry: &IndexedReleaseEntry,
) -> Result<PathBuf, ExecutorError> {
    let target = entry
        .link_target
        .as_ref()
        .ok_or_else(|| ExecutorError::sanitized("release_archive_symlink_target_missing"))?;
    let mut resolved = path
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("release_archive_symlink_parent_missing"))?
        .to_path_buf();
    for component in target.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => resolved.push(component),
            std::path::Component::ParentDir => {
                if !resolved.pop() {
                    return Err(ExecutorError::sanitized(
                        "release_archive_symlink_escaped_root",
                    ));
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(ExecutorError::sanitized(
                    "release_archive_symlink_target_absolute",
                ));
            }
        }
    }
    Ok(resolved)
}

fn captured_indexed_utf8(
    indexed: &BTreeMap<PathBuf, IndexedReleaseEntry>,
    path: &str,
) -> Result<String, ExecutorError> {
    let entry = require_indexed_kind(indexed, Path::new(path), IndexedReleaseEntryKind::Regular)?;
    let bytes = entry
        .captured
        .as_deref()
        .ok_or_else(|| ExecutorError::sanitized("release_archive_contract_file_missing"))?;
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ExecutorError::sanitized("release_text_not_utf8"))?;
    if text.contains('\r') || text.as_bytes().contains(&0) {
        return Err(ExecutorError::sanitized("release_systemd_contract_invalid"));
    }
    Ok(text.to_string())
}

fn validate_release_systemd_contract(
    api_text: &str,
    worker_text: &str,
) -> Result<(), ExecutorError> {
    // The source-owned units are independently checked with systemd-analyze.
    // Exact bytes bind the archive to that syntax-checked, fully hardened
    // contract without writing a temporary unit during this read-only pass.
    if api_text.as_bytes() != CANONICAL_API_UNIT_BYTES
        || worker_text.as_bytes() != CANONICAL_WORKER_UNIT_BYTES
    {
        return Err(ExecutorError::sanitized("release_systemd_contract_invalid"));
    }
    Ok(())
}

fn verify_indexed_sha256sums(
    indexed: &BTreeMap<PathBuf, IndexedReleaseEntry>,
) -> Result<u64, ExecutorError> {
    let manifest = require_indexed_kind(
        indexed,
        Path::new("SHA256SUMS"),
        IndexedReleaseEntryKind::Regular,
    )?;
    let text = std::str::from_utf8(
        manifest
            .captured
            .as_deref()
            .ok_or_else(|| ExecutorError::sanitized("release_sha256sums_missing"))?,
    )
    .map_err(|_| ExecutorError::sanitized("release_sha256sums_invalid"))?;
    if text.is_empty() || text.contains('\r') || text.as_bytes().contains(&0) {
        return Err(ExecutorError::sanitized("release_sha256sums_invalid"));
    }
    let mut covered = BTreeSet::new();
    for line in text.lines() {
        if line.len() < 67 {
            return Err(ExecutorError::sanitized("release_sha256sums_invalid"));
        }
        let (digest, remainder) = line.split_at(64);
        if !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !remainder.starts_with("  ")
        {
            return Err(ExecutorError::sanitized("release_sha256sums_invalid"));
        }
        let path = normalize_release_archive_path(Path::new(&remainder[2..]))?;
        if path.as_os_str().is_empty()
            || path == Path::new("SHA256SUMS")
            || !covered.insert(path.clone())
        {
            return Err(ExecutorError::sanitized("release_sha256sums_invalid"));
        }
        let entry = require_indexed_kind(indexed, &path, IndexedReleaseEntryKind::Regular)?;
        if entry.sha256.as_deref() != Some(digest) {
            return Err(ExecutorError::sanitized("release_internal_checksum_failed"));
        }
    }
    let expected = indexed
        .iter()
        .filter_map(|(path, entry)| {
            (entry.kind == IndexedReleaseEntryKind::Regular && path != Path::new("SHA256SUMS"))
                .then_some(path.clone())
        })
        .collect::<BTreeSet<_>>();
    if covered != expected {
        return Err(ExecutorError::sanitized(
            "release_sha256sums_coverage_incomplete",
        ));
    }
    u64::try_from(covered.len())
        .map_err(|_| ExecutorError::sanitized("release_archive_too_many_entries"))
}

fn indexed_release_tree_sha256(
    indexed: &BTreeMap<PathBuf, IndexedReleaseEntry>,
) -> Result<String, ExecutorError> {
    let mut digest = Sha256::new();
    digest.update(b"v2board-native-release-archive-index-v1\0");
    for (path, entry) in indexed {
        let path = path
            .to_str()
            .ok_or_else(|| ExecutorError::sanitized("release_archive_path_invalid"))?;
        digest.update((path.len() as u64).to_be_bytes());
        digest.update(path.as_bytes());
        digest.update(entry.mode.to_be_bytes());
        digest.update(entry.size.to_be_bytes());
        match entry.kind {
            IndexedReleaseEntryKind::Regular => {
                digest.update(b"F");
                digest.update(
                    entry
                        .sha256
                        .as_deref()
                        .ok_or_else(|| {
                            ExecutorError::sanitized("release_archive_entry_digest_missing")
                        })?
                        .as_bytes(),
                );
            }
            IndexedReleaseEntryKind::Directory => digest.update(b"D"),
            IndexedReleaseEntryKind::Symlink => {
                digest.update(b"L");
                let target = entry
                    .link_target
                    .as_ref()
                    .and_then(|target| target.to_str())
                    .ok_or_else(|| {
                        ExecutorError::sanitized("release_archive_symlink_target_invalid")
                    })?;
                digest.update((target.len() as u64).to_be_bytes());
                digest.update(target.as_bytes());
            }
        }
    }
    Ok(hex::encode(digest.finalize()))
}

fn safely_extract_release_archive(
    archive_path: &Path,
    expected_archive_sha256: &str,
    extraction_path: &Path,
) -> Result<(), ExecutorError> {
    let mut file = open_root_owned_release_archive(archive_path)?;
    if hash_open_release_archive(&mut file)? != expected_archive_sha256 {
        return Err(ExecutorError::sanitized("release_archive_digest_mismatch"));
    }
    validate_structured_release_archive(&mut file)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ExecutorError::sanitized("release_archive_seek_failed"))?;
    {
        let decoder = flate2::read::GzDecoder::new(&mut file);
        let mut archive = tar::Archive::new(decoder);
        archive.set_preserve_ownerships(false);
        archive.set_preserve_permissions(false);
        archive.set_overwrite(false);
        let entries = archive
            .entries()
            .map_err(|_| ExecutorError::sanitized("release_archive_entries_invalid"))?;
        for entry in entries {
            let mut entry =
                entry.map_err(|_| ExecutorError::sanitized("release_archive_entry_invalid"))?;
            entry.set_mask(0o022);
            entry.set_preserve_permissions(false);
            entry.set_preserve_mtime(false);
            if !entry
                .unpack_in(extraction_path)
                .map_err(|_| ExecutorError::sanitized("release_archive_extract_failed"))?
            {
                return Err(ExecutorError::sanitized(
                    "release_archive_entry_escaped_root",
                ));
            }
        }
    }
    let opened = file
        .metadata()
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?;
    let current = fs::symlink_metadata(archive_path)
        .map_err(|_| ExecutorError::sanitized("release_archive_metadata_failed"))?;
    if opened.dev() != current.dev() || opened.ino() != current.ino() {
        return Err(ExecutorError::sanitized("release_archive_path_replaced"));
    }
    Ok(())
}

fn validate_structured_release_archive(file: &mut File) -> Result<(), ExecutorError> {
    const MAX_RELEASE_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
    const MAX_RELEASE_UNPACKED_BYTES: u64 = 32 * 1024 * 1024 * 1024;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ExecutorError::sanitized("release_archive_seek_failed"))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|_| ExecutorError::sanitized("release_archive_entries_invalid"))?;
    let mut paths = BTreeSet::new();
    let mut total_size = 0_u64;
    let mut entry_count = 0_usize;
    for entry in entries {
        let entry = entry.map_err(|_| ExecutorError::sanitized("release_archive_entry_invalid"))?;
        entry_count = entry_count.saturating_add(1);
        if entry_count > MAX_RELEASE_ENTRIES {
            return Err(ExecutorError::sanitized("release_archive_too_many_entries"));
        }
        let entry_type = entry.header().entry_type();
        if !matches!(
            entry_type,
            EntryType::Regular | EntryType::Directory | EntryType::Symlink
        ) {
            return Err(ExecutorError::sanitized(
                "release_archive_entry_type_forbidden",
            ));
        }
        let path = entry
            .path()
            .map_err(|_| ExecutorError::sanitized("release_archive_path_invalid"))?;
        let normalized = normalize_release_archive_path(path.as_ref())?;
        if normalized.as_os_str().is_empty() {
            if entry_type != EntryType::Directory {
                return Err(ExecutorError::sanitized("release_archive_path_empty"));
            }
        } else if !paths.insert(normalized.clone()) {
            return Err(ExecutorError::sanitized("release_archive_path_duplicate"));
        }
        let mode = entry
            .header()
            .mode()
            .map_err(|_| ExecutorError::sanitized("release_archive_mode_invalid"))?;
        if entry_type != EntryType::Symlink && mode & 0o7022 != 0 {
            return Err(ExecutorError::sanitized("release_archive_mode_unsafe"));
        }
        let size = entry.size();
        if size > MAX_RELEASE_FILE_BYTES || (!matches!(entry_type, EntryType::Regular) && size != 0)
        {
            return Err(ExecutorError::sanitized(
                "release_archive_entry_size_invalid",
            ));
        }
        total_size = total_size
            .checked_add(size)
            .ok_or_else(|| ExecutorError::sanitized("release_archive_size_overflow"))?;
        if total_size > MAX_RELEASE_UNPACKED_BYTES {
            return Err(ExecutorError::sanitized(
                "release_archive_unpacked_size_exceeded",
            ));
        }
        if entry_type == EntryType::Symlink {
            validate_release_archive_symlink(&normalized, &entry)?;
        } else if entry.link_name_bytes().is_some() {
            return Err(ExecutorError::sanitized(
                "release_archive_unexpected_link_target",
            ));
        }
    }
    if entry_count == 0 {
        return Err(ExecutorError::sanitized("release_archive_empty"));
    }
    Ok(())
}

fn normalize_release_archive_path(path: &Path) -> Result<PathBuf, ExecutorError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(ExecutorError::sanitized(
                    "release_archive_path_not_relative",
                ));
            }
        }
    }
    Ok(normalized)
}

fn validate_release_archive_symlink<R: Read>(
    path: &Path,
    entry: &tar::Entry<'_, R>,
) -> Result<(), ExecutorError> {
    if path != Path::new("frontend/current") && path != Path::new("frontend/previous") {
        return Err(ExecutorError::sanitized(
            "release_archive_symlink_not_allowed",
        ));
    }
    let target = entry
        .link_name()
        .map_err(|_| ExecutorError::sanitized("release_archive_symlink_target_invalid"))?
        .ok_or_else(|| ExecutorError::sanitized("release_archive_symlink_target_missing"))?;
    if target.is_absolute() {
        return Err(ExecutorError::sanitized(
            "release_archive_symlink_target_absolute",
        ));
    }
    let mut resolved = path
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("release_archive_symlink_parent_missing"))?
        .to_path_buf();
    for component in target.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => resolved.push(component),
            std::path::Component::ParentDir => {
                if !resolved.pop() {
                    return Err(ExecutorError::sanitized(
                        "release_archive_symlink_escaped_root",
                    ));
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(ExecutorError::sanitized(
                    "release_archive_symlink_target_absolute",
                ));
            }
        }
    }
    if !resolved.starts_with("frontend/releases") || resolved == Path::new("frontend/releases") {
        return Err(ExecutorError::sanitized(
            "release_archive_symlink_target_outside_releases",
        ));
    }
    Ok(())
}

fn collect_release_paths(root: &Path) -> Result<Vec<PathBuf>, ExecutorError> {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(path) = pending.pop() {
        if paths.len() >= MAX_RELEASE_ENTRIES {
            return Err(ExecutorError::sanitized("release_tree_too_large"));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| ExecutorError::sanitized("release_tree_metadata_failed"))?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            for entry in fs::read_dir(&path)
                .map_err(|_| ExecutorError::sanitized("release_tree_read_failed"))?
            {
                pending.push(
                    entry
                        .map_err(|_| ExecutorError::sanitized("release_tree_read_failed"))?
                        .path(),
                );
            }
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

/// Covers every directory entry, file byte, mode, and symlink target. This is
/// deliberately stronger than SHA256SUMS (which does not cover symlinks or
/// unexpected files) and lets a crash-resume compare an existing published
/// tree with a fresh extraction of the manifest-bound archive.
fn release_tree_digest(root: &Path) -> Result<String, ExecutorError> {
    let mut digest = Sha256::new();
    digest.update(b"v2board-native-release-tree-v1\0");
    for path in collect_release_paths(root)? {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| ExecutorError::sanitized("release_tree_path_invalid"))?;
        let relative = relative.as_os_str().as_encoded_bytes();
        digest.update((relative.len() as u64).to_be_bytes());
        digest.update(relative);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| ExecutorError::sanitized("release_tree_metadata_failed"))?;
        digest.update((metadata.permissions().mode() & 0o7777).to_be_bytes());
        if metadata.file_type().is_symlink() {
            digest.update(b"L");
            let target = fs::read_link(&path)
                .map_err(|_| ExecutorError::sanitized("release_symlink_read_failed"))?;
            let target = target.as_os_str().as_encoded_bytes();
            digest.update((target.len() as u64).to_be_bytes());
            digest.update(target);
        } else if metadata.is_dir() {
            digest.update(b"D");
        } else if metadata.is_file() {
            digest.update(b"F");
            digest.update(metadata.len().to_be_bytes());
            let mut file = File::open(&path)
                .map_err(|_| ExecutorError::sanitized("release_tree_file_open_failed"))?;
            let opened = file
                .metadata()
                .map_err(|_| ExecutorError::sanitized("release_tree_metadata_failed"))?;
            if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
                return Err(ExecutorError::sanitized("release_tree_file_replaced"));
            }
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|_| ExecutorError::sanitized("release_tree_file_read_failed"))?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
        } else {
            return Err(ExecutorError::sanitized("release_tree_entry_invalid"));
        }
    }
    Ok(hex::encode(digest.finalize()))
}

fn sync_release_tree(root: &Path) -> Result<(), ExecutorError> {
    let mut directories = Vec::new();
    for path in collect_release_paths(root)? {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| ExecutorError::sanitized("release_tree_metadata_failed"))?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            directories.push(path);
        } else if metadata.is_file() {
            File::open(&path)
                .and_then(|file| file.sync_all())
                .map_err(|_| ExecutorError::sanitized("release_tree_file_sync_failed"))?;
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        sync_directory(&directory)
            .map_err(|_| ExecutorError::sanitized("release_tree_directory_sync_failed"))?;
    }
    Ok(())
}

fn remove_private_extraction_tree_if_present(path: &Path) -> Result<(), ExecutorError> {
    if path.parent() != Some(Path::new(RELEASES_ROOT)) {
        return Err(ExecutorError::sanitized("release_extract_path_invalid"));
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {
            return Err(ExecutorError::sanitized(
                "release_extract_tree_inspect_failed",
            ));
        }
        Ok(metadata) => {
            if !metadata.file_type().is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o022 != 0
            {
                return Err(ExecutorError::sanitized("release_extract_tree_unsafe"));
            }
        }
    }
    remove_root_owned_entry(path)?;
    sync_directory(Path::new(RELEASES_ROOT))
        .map_err(|_| ExecutorError::sanitized("release_extract_parent_sync_failed"))
}

fn remove_root_owned_entry(path: &Path) -> Result<(), ExecutorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ExecutorError::sanitized("release_extract_entry_inspect_failed"))?;
    if metadata.uid() != 0 {
        return Err(ExecutorError::sanitized(
            "release_extract_entry_owner_invalid",
        ));
    }
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        let entries = fs::read_dir(path)
            .map_err(|_| ExecutorError::sanitized("release_extract_tree_read_failed"))?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|_| ExecutorError::sanitized("release_extract_tree_read_failed"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for entry in entries {
            remove_root_owned_entry(&entry)?;
        }
        fs::remove_dir(path)
            .map_err(|_| ExecutorError::sanitized("release_extract_dir_remove_failed"))?;
    } else {
        fs::remove_file(path)
            .map_err(|_| ExecutorError::sanitized("release_extract_entry_remove_failed"))?;
    }
    Ok(())
}

fn verify_materialized_role_configs(spec: &ProvisionSpec) -> Result<(), ExecutorError> {
    let bundle = crate::target_activation::materialize_role_configs(spec)
        .map_err(|_| ExecutorError::sanitized("runtime_config_materialization_failed"))?;
    let mut runner = ProcessCommandRunner;
    let api_uid = lookup_uid(&mut runner, bundle.api_owner())?;
    let worker_uid = lookup_uid(&mut runner, bundle.worker_owner())?;
    verify_exact_role_config(bundle.api_path(), bundle.api_bytes(), api_uid)?;
    verify_exact_role_config(bundle.worker_path(), bundle.worker_bytes(), worker_uid)
}

fn switch_release_forward_only(
    operation_id: &str,
    staged_path: &Path,
) -> Result<(), ExecutorError> {
    let current = Path::new(CURRENT_RELEASE_PATH);
    let parent = current
        .parent()
        .ok_or_else(|| ExecutorError::sanitized("current_release_parent_missing"))?;
    verify_root_directory(parent)?;
    match fs::symlink_metadata(current) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(ExecutorError::sanitized("current_release_not_symlink"));
            }
            if fs::canonicalize(current)
                .map_err(|_| ExecutorError::sanitized("current_release_invalid"))?
                == staged_path
            {
                return sync_directory(parent)
                    .map_err(|_| ExecutorError::sanitized("current_release_parent_sync_failed"));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(ExecutorError::sanitized("current_release_inspect_failed")),
    }
    let temporary = parent.join(format!(".current.{operation_id}.native.tmp"));
    match fs::symlink_metadata(&temporary) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink()
                || fs::canonicalize(&temporary)
                    .map_err(|_| ExecutorError::sanitized("current_release_temp_invalid"))?
                    != staged_path
            {
                return Err(ExecutorError::sanitized("current_release_temp_conflict"));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            symlink(staged_path, &temporary)
                .map_err(|_| ExecutorError::sanitized("current_release_temp_create_failed"))?;
            sync_directory(parent)
                .map_err(|_| ExecutorError::sanitized("current_release_temp_sync_failed"))?;
        }
        Err(_) => {
            return Err(ExecutorError::sanitized(
                "current_release_temp_inspect_failed",
            ));
        }
    }
    fs::rename(&temporary, current)
        .and_then(|()| sync_directory(parent))
        .map_err(|_| ExecutorError::sanitized("current_release_atomic_switch_failed"))?;
    verify_current_release(staged_path)
}

const API_UNIT_ENVIRONMENT: &[&str] = &[
    "V2BOARD_ENV=production",
    "V2BOARD_RUNTIME_ROOT=/var/lib/v2board",
    "V2BOARD_CONFIG_PATH=/var/lib/v2board/api/config.json",
    "V2BOARD_RULE_DIR=/var/lib/v2board/rules",
    "V2BOARD_FRONTEND_DIR=/opt/v2board/current/frontend",
    "RUST_LOG=v2board_api=info,tower_http=info",
];
const WORKER_UNIT_ENVIRONMENT: &[&str] = &[
    "V2BOARD_ENV=production",
    "V2BOARD_RUNTIME_ROOT=/var/lib/v2board",
    "V2BOARD_CONFIG_PATH=/var/lib/v2board/worker/config.json",
    "V2BOARD_WORKER_HEALTH_FILE=/run/v2board-worker/health",
    "V2BOARD_WORKER_HEARTBEAT_INTERVAL_SECONDS=10",
    "V2BOARD_WORKER_SHUTDOWN_TIMEOUT_SECONDS=30",
    "RUST_LOG=v2board_workers=info",
];

#[derive(Clone, Copy)]
struct NativeUnitContract {
    unit: &'static str,
    fragment_path: &'static str,
    executable: &'static str,
    release_binary: &'static str,
    user: &'static str,
    group: &'static str,
    service_type: &'static str,
    working_directory: &'static str,
    environment: &'static [&'static str],
}

fn native_unit_contract(unit: &'static str) -> Result<NativeUnitContract, ExecutorError> {
    match unit {
        API_UNIT => Ok(NativeUnitContract {
            unit: API_UNIT,
            fragment_path: "/etc/systemd/system/v2board-api.service",
            executable: "/opt/v2board/current/bin/v2board-api",
            release_binary: "bin/v2board-api",
            user: "v2board-api",
            group: "v2board-api",
            service_type: "exec",
            working_directory: "/var/lib/v2board/api",
            environment: API_UNIT_ENVIRONMENT,
        }),
        WORKER_UNIT => Ok(NativeUnitContract {
            unit: WORKER_UNIT,
            fragment_path: "/etc/systemd/system/v2board-worker.service",
            executable: "/opt/v2board/current/bin/v2board-workers",
            release_binary: "bin/v2board-workers",
            user: "v2board-worker",
            group: "v2board-worker",
            service_type: "notify",
            working_directory: "/var/lib/v2board/worker",
            environment: WORKER_UNIT_ENVIRONMENT,
        }),
        _ => Err(ExecutorError::sanitized("systemd_unit_not_allowed")),
    }
}

fn verify_native_units_inactive_and_disabled(
    runner: &mut impl NativeCommandRunner,
) -> Result<(), ExecutorError> {
    for unit in [API_UNIT, WORKER_UNIT] {
        let active = runner
            .run(
                NativeCommandRequest::new(NativeProgram::Systemctl)
                    .arg("is-active")
                    .arg("--quiet")
                    .arg(unit),
            )
            .map_err(|_| ExecutorError::sanitized("native_unit_state_probe_failed"))?;
        if active.exit_code == 0 {
            return Err(ExecutorError::sanitized(
                "native_unit_active_before_authority",
            ));
        }
        let enabled = runner
            .run(
                NativeCommandRequest::new(NativeProgram::Systemctl)
                    .arg("is-enabled")
                    .arg("--quiet")
                    .arg(unit),
            )
            .map_err(|_| ExecutorError::sanitized("native_unit_enablement_probe_failed"))?;
        if enabled.exit_code == 0 {
            return Err(ExecutorError::sanitized(
                "native_unit_enabled_before_authority",
            ));
        }
    }
    Ok(())
}

fn install_native_systemd_units(
    runner: &mut impl NativeCommandRunner,
    operation_id: &str,
    release: &Path,
) -> Result<(), ExecutorError> {
    verify_root_directory(Path::new(SYSTEMD_UNIT_ROOT))?;
    for unit in [API_UNIT, WORKER_UNIT] {
        let source = release.join("systemd").join(unit);
        let bytes = read_regular_limited(&source, 128 * 1024)?;
        install_exact_systemd_unit(operation_id, unit, &bytes)?;
    }
    run_native_success(
        runner,
        NativeCommandRequest::new(NativeProgram::Systemctl).arg("daemon-reload"),
        "systemd_daemon_reload_failed",
    )?;
    verify_effective_native_units(runner)
}

fn verify_installed_native_systemd_units_exact(release: &Path) -> Result<(), ExecutorError> {
    for unit in [API_UNIT, WORKER_UNIT] {
        let expected = read_regular_limited(&release.join("systemd").join(unit), 128 * 1024)?;
        verify_exact_systemd_unit_file(&Path::new(SYSTEMD_UNIT_ROOT).join(unit), &expected)?;
    }
    Ok(())
}

fn install_exact_systemd_unit(
    operation_id: &str,
    unit: &'static str,
    expected: &[u8],
) -> Result<(), ExecutorError> {
    let root = Path::new(SYSTEMD_UNIT_ROOT);
    let destination = root.join(unit);
    match fs::symlink_metadata(&destination) {
        Ok(_) => return verify_exact_systemd_unit_file(&destination, expected),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(ExecutorError::sanitized("systemd_unit_inspect_failed")),
    }
    let temporary = root.join(format!(".{unit}.{operation_id}.installing"));
    match write_new_owner_file(&temporary, expected, 0o644) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            verify_exact_systemd_unit_file(&temporary, expected)?;
        }
        Err(_) => return Err(ExecutorError::sanitized("systemd_unit_temp_write_failed")),
    }
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            verify_exact_systemd_unit_file(&destination, expected)?;
            fs::remove_file(&temporary)
                .map_err(|_| ExecutorError::sanitized("systemd_unit_temp_remove_failed"))?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::rename(&temporary, &destination)
                .map_err(|_| ExecutorError::sanitized("systemd_unit_install_failed"))?;
        }
        Err(_) => return Err(ExecutorError::sanitized("systemd_unit_inspect_failed")),
    }
    sync_directory(root).map_err(|_| ExecutorError::sanitized("systemd_unit_root_sync_failed"))?;
    verify_exact_systemd_unit_file(&destination, expected)
}

fn verify_exact_systemd_unit_file(path: &Path, expected: &[u8]) -> Result<(), ExecutorError> {
    verify_root_owner_file(path, 0o644)?;
    if read_regular_limited(path, expected.len().saturating_add(1))? != expected {
        return Err(ExecutorError::sanitized("systemd_unit_content_mismatch"));
    }
    Ok(())
}

fn verify_effective_native_units(
    runner: &mut impl NativeCommandRunner,
) -> Result<(), ExecutorError> {
    for unit in [API_UNIT, WORKER_UNIT] {
        verify_effective_native_unit(runner, native_unit_contract(unit)?)?;
    }
    Ok(())
}

fn systemd_show_properties(
    runner: &mut impl NativeCommandRunner,
    unit: &'static str,
    property_names: &[&'static str],
) -> Result<BTreeMap<String, String>, ExecutorError> {
    let mut request = NativeCommandRequest::new(NativeProgram::Systemctl).arg("show");
    for property in property_names {
        request = request.arg(format!("--property={property}"));
    }
    let output = run_native_success(runner, request.arg(unit), "systemd_unit_show_failed")?;
    parse_systemd_show_properties(output.stdout(), property_names)
}

fn parse_systemd_show_properties(
    bytes: &[u8],
    property_names: &[&str],
) -> Result<BTreeMap<String, String>, ExecutorError> {
    let text = strict_output_text(bytes, "systemd_unit_properties_invalid")?;
    if text.contains('\r') {
        return Err(ExecutorError::sanitized("systemd_unit_properties_invalid"));
    }
    let expected = property_names.iter().copied().collect::<BTreeSet<_>>();
    let mut properties = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| ExecutorError::sanitized("systemd_unit_properties_invalid"))?;
        if !expected.contains(key)
            || properties
                .insert(key.to_string(), value.to_string())
                .is_some()
        {
            return Err(ExecutorError::sanitized("systemd_unit_properties_invalid"));
        }
    }
    if properties.len() != expected.len() {
        return Err(ExecutorError::sanitized(
            "systemd_unit_properties_incomplete",
        ));
    }
    Ok(properties)
}

fn verify_effective_native_unit(
    runner: &mut impl NativeCommandRunner,
    contract: NativeUnitContract,
) -> Result<(), ExecutorError> {
    const PROPERTIES: &[&str] = &[
        "LoadState",
        "FragmentPath",
        "DropInPaths",
        "ExecStart",
        "Environment",
        "User",
        "Group",
        "Type",
        "WorkingDirectory",
    ];
    let properties = systemd_show_properties(runner, contract.unit, PROPERTIES)?;
    verify_effective_unit_properties(&properties, contract)
}

fn verify_effective_unit_properties(
    properties: &BTreeMap<String, String>,
    contract: NativeUnitContract,
) -> Result<(), ExecutorError> {
    let exact = [
        ("LoadState", "loaded"),
        ("FragmentPath", contract.fragment_path),
        ("DropInPaths", ""),
        ("User", contract.user),
        ("Group", contract.group),
        ("Type", contract.service_type),
        ("WorkingDirectory", contract.working_directory),
    ];
    if exact
        .iter()
        .any(|(key, expected)| properties.get(*key).map(String::as_str) != Some(*expected))
    {
        return Err(ExecutorError::sanitized(
            "systemd_effective_unit_contract_mismatch",
        ));
    }
    let exec_start = properties
        .get("ExecStart")
        .ok_or_else(|| ExecutorError::sanitized("systemd_exec_start_missing"))?;
    verify_systemd_exec_start(exec_start, contract.executable)?;
    let environment = properties
        .get("Environment")
        .ok_or_else(|| ExecutorError::sanitized("systemd_environment_missing"))?;
    verify_systemd_environment(environment, contract.environment)
}

fn verify_systemd_exec_start(value: &str, expected: &str) -> Result<(), ExecutorError> {
    if value.matches('{').count() != 1 || value.matches('}').count() != 1 {
        return Err(ExecutorError::sanitized("systemd_exec_start_invalid"));
    }
    let body = value
        .strip_prefix("{ ")
        .and_then(|value| value.strip_suffix(" }"))
        .ok_or_else(|| ExecutorError::sanitized("systemd_exec_start_invalid"))?;
    let mut fields = BTreeMap::new();
    for field in body.split(';') {
        let (key, value) = field
            .trim()
            .split_once('=')
            .ok_or_else(|| ExecutorError::sanitized("systemd_exec_start_invalid"))?;
        if fields.insert(key, value).is_some() {
            return Err(ExecutorError::sanitized("systemd_exec_start_invalid"));
        }
    }
    if fields.get("path") != Some(&expected) || fields.get("argv[]") != Some(&expected) {
        return Err(ExecutorError::sanitized("systemd_exec_start_mismatch"));
    }
    Ok(())
}

fn verify_systemd_environment(value: &str, expected: &[&str]) -> Result<(), ExecutorError> {
    if value
        .chars()
        .any(|character| matches!(character, '"' | '\'' | '\\'))
    {
        return Err(ExecutorError::sanitized("systemd_environment_invalid"));
    }
    let actual = value.split_ascii_whitespace().collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(ExecutorError::sanitized("systemd_environment_mismatch"));
    }
    Ok(())
}

fn verify_running_unit_binary(
    runner: &mut impl NativeCommandRunner,
    unit: &'static str,
    release: &Path,
) -> Result<u64, ExecutorError> {
    let contract = native_unit_contract(unit)?;
    verify_effective_native_unit(runner, contract)?;
    let properties =
        systemd_show_properties(runner, unit, &["ActiveState", "SubState", "MainPID"])?;
    if properties.get("ActiveState").map(String::as_str) != Some("active")
        || properties.get("SubState").map(String::as_str) != Some("running")
    {
        return Err(ExecutorError::sanitized("systemd_unit_not_running"));
    }
    let main_pid = parse_u64(
        properties
            .get("MainPID")
            .ok_or_else(|| ExecutorError::sanitized("systemd_main_pid_missing"))?,
        "systemd_main_pid_invalid",
    )?;
    if main_pid <= 1 {
        return Err(ExecutorError::sanitized("systemd_main_pid_invalid"));
    }
    verify_current_release(release)?;
    let expected = release.join(contract.release_binary);
    let canonical_expected = fs::canonicalize(&expected)
        .map_err(|_| ExecutorError::sanitized("systemd_release_binary_missing"))?;
    if canonical_expected != expected {
        return Err(ExecutorError::sanitized(
            "systemd_release_binary_not_canonical",
        ));
    }
    let process_executable = fs::canonicalize(format!("/proc/{main_pid}/exe"))
        .map_err(|_| ExecutorError::sanitized("systemd_main_pid_exe_unavailable"))?;
    if process_executable != canonical_expected {
        return Err(ExecutorError::sanitized(
            "systemd_main_pid_release_mismatch",
        ));
    }
    Ok(main_pid)
}

fn start_marked_unit(
    runner: &mut impl NativeCommandRunner,
    activation_root: &Path,
    permit: &DurableNativeStartPermit,
    release_id: &str,
    unit: &'static str,
) -> Result<(), ExecutorError> {
    let intent = NativeStartIntent {
        schema_version: 1,
        operation_id: permit.operation_id().to_string(),
        installation_id: permit.installation_id().to_string(),
        release_id: release_id.to_string(),
        authority_nodes_generation: permit
            .native_authority_binding()
            .nodes_verified_generation(),
        authority_nodes_event_sha256: permit
            .native_authority_binding()
            .nodes_verified_event_sha256()
            .to_string(),
        unit: unit.to_string(),
        state: "start_intent_durable".to_string(),
    };
    let operation_dir = activation_root.join(permit.operation_id());
    prepare_root_private_path(activation_root)?;
    prepare_root_private_path(&operation_dir)?;
    let filename = match unit {
        API_UNIT => "native-api.start.json",
        WORKER_UNIT => "native-worker.start.json",
        _ => return Err(ExecutorError::sanitized("systemd_unit_not_allowed")),
    };
    let path = operation_dir.join(filename);
    let bytes = serde_json::to_vec(&intent)
        .map_err(|_| ExecutorError::sanitized("native_start_marker_serialize_failed"))?;
    match write_new_owner_file(&path, &bytes, 0o600) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| ExecutorError::sanitized("native_start_marker_invalid"))?;
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o077 != 0
                || serde_json::from_slice::<NativeStartIntent>(&read_regular_limited(
                    &path,
                    16 * 1024,
                )?)
                .map_err(|_| ExecutorError::sanitized("native_start_marker_invalid"))?
                    != intent
            {
                return Err(ExecutorError::sanitized(
                    "native_start_marker_binding_mismatch",
                ));
            }
        }
        Err(_) => return Err(ExecutorError::sanitized("native_start_marker_write_failed")),
    }
    run_native_success(
        runner,
        NativeCommandRequest::new(NativeProgram::Systemctl)
            .arg("enable")
            .arg(unit),
        "systemd_native_unit_enable_failed",
    )?;
    let active = runner
        .run(
            NativeCommandRequest::new(NativeProgram::Systemctl)
                .arg("is-active")
                .arg("--quiet")
                .arg(unit),
        )
        .map_err(|_| ExecutorError::sanitized("systemd_prestart_probe_failed"))?;
    if active.exit_code != 0 {
        run_native_success(
            runner,
            NativeCommandRequest::new(NativeProgram::Systemctl)
                .arg("start")
                .arg(unit)
                .timeout(Duration::from_secs(60)),
            if unit == API_UNIT {
                "systemd_api_start_failed"
            } else {
                "systemd_worker_start_failed"
            },
        )?;
    }
    Ok(())
}

fn wait_api_ready(
    runner: &mut impl NativeCommandRunner,
    ready_url: &str,
    release: &Path,
) -> Result<(), ExecutorError> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let result = run_native_success(
            runner,
            NativeCommandRequest::new(NativeProgram::Curl)
                .arg("--silent")
                .arg("--show-error")
                .arg("--fail")
                .arg("--proto")
                .arg("=http")
                .arg("--max-time")
                .arg("5")
                .arg(ready_url),
            "api_ready_probe_failed",
        );
        if let Ok(output) = result {
            let ready = serde_json::from_slice::<serde_json::Value>(output.stdout())
                .ok()
                .is_some_and(|value| value.get("ok") == Some(&serde_json::Value::Bool(true)));
            if ready && verify_running_unit_binary(runner, API_UNIT, release).is_ok() {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(ExecutorError::sanitized("api_readiness_timeout"));
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn wait_worker_ready(
    runner: &mut impl NativeCommandRunner,
    health_path: &Path,
    release: &Path,
) -> Result<(), ExecutorError> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let state = systemd_show_properties(runner, WORKER_UNIT, &["WatchdogTimestampMonotonic"]);
        if let Ok(properties) = state {
            let watchdog_ready = properties
                .get("WatchdogTimestampMonotonic")
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|value| value > 0);
            if watchdog_ready
                && recent_regular_health_file(health_path).is_ok_and(|healthy| healthy)
                && verify_running_unit_binary(runner, WORKER_UNIT, release).is_ok()
            {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(ExecutorError::sanitized("worker_readiness_timeout"));
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn run_native_success(
    runner: &mut impl NativeCommandRunner,
    request: NativeCommandRequest,
    code: &'static str,
) -> Result<NativeCommandOutput, ExecutorError> {
    let output = runner
        .run(request)
        .map_err(|error| runner_executor_error(error, code))?;
    if output.exit_code != 0 {
        return Err(ExecutorError::sanitized(code));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::{Compression, write::GzEncoder};

    use super::*;
    use crate::apply_journal::{ApplyJournal, ApplyJournalBinding};

    const OPERATION_ID: &str = "40aa4a80-eb4b-4b25-9c3b-e17ed047873d";
    const INSTALLATION_ID: &str = "e0bb60eb-bb45-4393-8a04-18a3aa510497";
    const INSPECT_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BACKUP_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const BACKUP_REFERENCE_HASH: &str =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const FINAL_RECHECK_HASH: &str =
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const SOURCE_FINGERPRINT_HASH: &str =
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn permit() -> DurableTargetMutationPermit {
        let root = std::env::temp_dir().join(format!(
            "v2board-native-activation-journal-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let binding =
            ApplyJournalBinding::new(OPERATION_ID, INSPECT_HASH).expect("journal binding");
        let (journal, pending) =
            ApplyJournal::create_pending(&root, binding).expect("pending journal");
        let mut current = journal.begin(&pending).expect("begin");
        for checkpoint in [
            ApplyCheckpoint::MaintenanceFenced,
            ApplyCheckpoint::SourceDrained,
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, INSPECT_HASH)
                .expect("advance checkpoint");
        }
        current = journal
            .record_backup_restore_verified(&current, BACKUP_HASH, BACKUP_REFERENCE_HASH)
            .expect("backup proof");
        current = journal
            .record_final_recheck_passed(&current, FINAL_RECHECK_HASH, SOURCE_FINGERPRINT_HASH)
            .expect("final recheck");
        current = journal
            .reserve_installation_identity(&current, INSTALLATION_ID)
            .expect("installation identity");
        let permit = journal
            .target_mutation_permit(&current)
            .expect("target permit");
        fs::remove_dir_all(root).expect("remove journal fixture");
        permit
    }

    #[test]
    fn command_request_rejects_secret_material_in_argv() {
        let request = NativeCommandRequest::new(NativeProgram::Psql)
            .arg("postgresql://user:top-secret@db/v2board")
            .secret_env("PGPASSWORD", "top-secret");
        assert_eq!(request.validate(), Err(NativeRunnerError::SecretInArgv));

        let request = NativeCommandRequest::new(NativeProgram::Psql)
            .arg("postgresql://user@db/v2board")
            .secret_env("PGPASSWORD", "top-secret");
        request.validate().expect("secret-free argv");
    }

    #[test]
    fn forward_success_cleanup_removes_all_config_rollback_artifacts_without_corrupting_active() {
        let root = test_path("config-cleanup");
        let api_dir = root.join("api");
        let worker_dir = root.join("worker");
        fs::create_dir_all(&api_dir).expect("API fixture directory");
        fs::create_dir_all(&worker_dir).expect("Worker fixture directory");
        let api = api_dir.join("config.json");
        let worker = worker_dir.join("config.json");
        write_new_owner_file(&api, b"api-boot\n", 0o600).expect("API boot config");
        write_new_owner_file(&worker, b"worker-boot\n", 0o600).expect("Worker boot config");

        let artifact = |active: &Path, suffix: &str| {
            active
                .parent()
                .expect("parent")
                .join(format!(".config.json.{OPERATION_ID}.{suffix}"))
        };
        write_new_owner_file(
            &artifact(&api, "previous"),
            b"old-full-plaintext-secret\n",
            0o600,
        )
        .expect("old API backup");
        fs::hard_link(&worker, artifact(&worker, "previous"))
            .expect("already-exact Worker hard link");
        for active in [&api, &worker] {
            write_new_owner_file(&artifact(active, "absent"), b"absent\n", 0o600)
                .expect("absent marker");
            write_new_owner_file(&artifact(active, "tmp"), b"temporary-secret\n", 0o600)
                .expect("temporary config");
        }

        let proof = cleanup_config_rollback_artifacts_for_paths(
            OPERATION_ID,
            &[api.as_path(), worker.as_path()],
        )
        .expect("forward cleanup");
        assert_eq!(proof.artifact_count, 6);
        assert!(proof.all_artifacts_absent);
        assert!(proof.parent_directories_fsynced);
        assert_eq!(fs::read(&api).expect("API remains"), b"api-boot\n");
        assert_eq!(fs::read(&worker).expect("Worker remains"), b"worker-boot\n");
        for active in [&api, &worker] {
            for suffix in ["previous", "absent", "tmp"] {
                assert!(!artifact(active, suffix).exists());
            }
        }
        fs::remove_dir_all(root).expect("remove config cleanup fixture");
    }

    fn test_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "v2board-native-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn write_structured_archive(entries: &[(&str, EntryType, Option<&str>)]) -> PathBuf {
        let path = test_path("archive.tar.gz");
        let file = File::create(&path).expect("archive file");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (path, entry_type, link_target) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(*entry_type);
            header.set_mode(if *entry_type == EntryType::Symlink {
                0o777
            } else {
                0o644
            });
            header.set_size(0);
            header.set_path(path).expect("archive path");
            if let Some(link_target) = link_target {
                header
                    .set_link_name(link_target)
                    .expect("archive link target");
            }
            header.set_cksum();
            builder
                .append(&header, io::empty())
                .expect("append archive entry");
        }
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip");
        path
    }

    #[derive(Clone, Copy)]
    enum CompleteArchiveMutation {
        None,
        InternalChecksum,
        SystemdUnit,
        UnexpectedRootEntry,
        CompressedSuffix,
        InvalidMode,
    }

    fn write_complete_release_archive(mutation: CompleteArchiveMutation) -> PathBuf {
        let path = test_path("complete-release.tar.gz");
        let mut files = BTreeMap::<&str, Vec<u8>>::from([
            ("bin/v2board-api", b"api-binary".to_vec()),
            ("bin/v2board-workers", b"worker-binary".to_vec()),
            (
                "bin/v2board-analytics-schema",
                b"analytics-schema-binary".to_vec(),
            ),
            (
                "frontend/releases/content-a/user/index.html",
                b"<html>user</html>".to_vec(),
            ),
            (
                "frontend/releases/content-a/admin/index.html",
                b"<html>admin</html>".to_vec(),
            ),
            (
                "systemd/v2board-api.service",
                CANONICAL_API_UNIT_BYTES.to_vec(),
            ),
            (
                "systemd/v2board-worker.service",
                CANONICAL_WORKER_UNIT_BYTES.to_vec(),
            ),
            (
                "RELEASE",
                concat!(
                    "format=v2board-native-release-v1\n",
                    "source_revision=0123456789abcdef0123456789abcdef01234567\n",
                    "target_os=linux\n",
                    "target_arch=amd64\n"
                )
                .as_bytes()
                .to_vec(),
            ),
        ]);
        if matches!(mutation, CompleteArchiveMutation::SystemdUnit) {
            files
                .get_mut("systemd/v2board-api.service")
                .expect("API unit")
                .extend_from_slice(b"# unreviewed drift\n");
        }
        if matches!(mutation, CompleteArchiveMutation::UnexpectedRootEntry) {
            files.insert("unexpected", b"unexpected".to_vec());
        }
        let mut checksums = files
            .iter()
            .map(|(path, bytes)| format!("{}  {path}\n", hex::encode(Sha256::digest(bytes))))
            .collect::<String>();
        if matches!(mutation, CompleteArchiveMutation::InternalChecksum) {
            let replacement = if checksums.starts_with('0') { "1" } else { "0" };
            checksums.replace_range(0..1, replacement);
        }
        files.insert("SHA256SUMS", checksums.into_bytes());

        let file = File::create(&path).expect("complete archive file");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for directory in [
            "bin",
            "frontend",
            "frontend/releases",
            "frontend/releases/content-a",
            "frontend/releases/content-a/user",
            "frontend/releases/content-a/admin",
            "systemd",
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Directory);
            header.set_mode(0o755);
            header.set_size(0);
            header.set_path(directory).expect("directory path");
            header.set_cksum();
            builder
                .append(&header, io::empty())
                .expect("append directory");
        }
        for (entry_path, bytes) in files {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            let mut mode = if entry_path.starts_with("bin/") {
                0o755
            } else {
                0o644
            };
            if matches!(mutation, CompleteArchiveMutation::InvalidMode)
                && entry_path == "bin/v2board-api"
            {
                mode = 0o100;
            }
            header.set_mode(mode);
            header.set_size(bytes.len() as u64);
            header.set_path(entry_path).expect("file path");
            header.set_cksum();
            builder
                .append(&header, bytes.as_slice())
                .expect("append file");
        }
        for link in ["frontend/current", "frontend/previous"] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_mode(0o777);
            header.set_size(0);
            header.set_path(link).expect("symlink path");
            header
                .set_link_name("releases/content-a")
                .expect("symlink target");
            header.set_cksum();
            builder
                .append(&header, io::empty())
                .expect("append symlink");
        }
        let encoder = builder.into_inner().expect("finish complete tar");
        let mut file = encoder.finish().expect("finish complete gzip");
        if matches!(mutation, CompleteArchiveMutation::CompressedSuffix) {
            file.write_all(b"opaque-suffix")
                .expect("append compressed suffix");
        }
        path
    }

    #[test]
    fn read_only_release_inspection_verifies_the_complete_virtual_tree() {
        let archive = write_complete_release_archive(CompleteArchiveMutation::None);
        let mut file = File::open(&archive).expect("open complete archive");
        let inspection = inspect_open_release_archive(
            &mut file,
            "release-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect("complete archive inspection");
        assert_eq!(inspection.entry_count, 18);
        assert_eq!(inspection.regular_file_count, 9);
        assert_eq!(inspection.internal_checksum_count, 8);
        assert_eq!(
            inspection.source_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert!(inspection.complete_structure_verified);
        assert!(inspection.internal_sha256sums_verified);
        assert!(inspection.systemd_contract_verified);
        assert!(inspection.target_filesystem_unchanged);
        fs::remove_file(archive).expect("remove complete archive");
    }

    #[test]
    fn read_only_release_inspection_rejects_checksum_unit_and_tree_drift() {
        for mutation in [
            CompleteArchiveMutation::InternalChecksum,
            CompleteArchiveMutation::SystemdUnit,
            CompleteArchiveMutation::UnexpectedRootEntry,
            CompleteArchiveMutation::CompressedSuffix,
            CompleteArchiveMutation::InvalidMode,
        ] {
            let archive = write_complete_release_archive(mutation);
            let mut file = File::open(&archive).expect("open invalid archive");
            assert!(
                inspect_open_release_archive(
                    &mut file,
                    "release-a",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .is_err()
            );
            fs::remove_file(archive).expect("remove invalid archive");
        }
    }

    #[test]
    fn structured_archive_rejects_hardlinks_duplicates_and_escaping_symlinks() {
        let hardlink =
            write_structured_archive(&[("bin/v2board-api", EntryType::Link, Some("outside"))]);
        let mut file = File::open(&hardlink).expect("open hardlink archive");
        assert!(validate_structured_release_archive(&mut file).is_err());
        fs::remove_file(hardlink).expect("remove hardlink archive");

        let duplicate = write_structured_archive(&[
            ("bin/v2board-api", EntryType::Regular, None),
            ("bin/v2board-api", EntryType::Regular, None),
        ]);
        let mut file = File::open(&duplicate).expect("open duplicate archive");
        assert!(validate_structured_release_archive(&mut file).is_err());
        fs::remove_file(duplicate).expect("remove duplicate archive");

        let symlink = write_structured_archive(&[(
            "frontend/current",
            EntryType::Symlink,
            Some("../../etc"),
        )]);
        let mut file = File::open(&symlink).expect("open symlink archive");
        assert!(validate_structured_release_archive(&mut file).is_err());
        fs::remove_file(symlink).expect("remove symlink archive");
    }

    #[test]
    fn structured_archive_accepts_only_release_local_frontend_links() {
        let archive = write_structured_archive(&[
            ("bin/v2board-api", EntryType::Regular, None),
            (
                "frontend/current",
                EntryType::Symlink,
                Some("releases/content-a"),
            ),
            (
                "frontend/previous",
                EntryType::Symlink,
                Some("releases/content-b"),
            ),
        ]);
        let mut file = File::open(&archive).expect("open archive");
        validate_structured_release_archive(&mut file).expect("valid archive structure");
        fs::remove_file(archive).expect("remove archive");

        assert!(normalize_release_archive_path(Path::new("../escape")).is_err());
        assert!(normalize_release_archive_path(Path::new("/absolute")).is_err());
    }

    #[test]
    fn release_metadata_is_exact_and_source_revision_is_real() {
        let root = test_path("release-metadata");
        fs::create_dir(&root).expect("metadata root");
        let valid = concat!(
            "format=v2board-native-release-v1\n",
            "source_revision=0123456789abcdef0123456789abcdef01234567\n",
            "target_os=linux\n",
            "target_arch=amd64\n"
        );
        fs::write(root.join("RELEASE"), valid).expect("valid metadata");
        assert_eq!(
            read_release_metadata(&root)
                .expect("valid metadata")
                .source_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        fs::write(
            root.join("RELEASE"),
            valid.replace(
                "source_revision=0123456789abcdef0123456789abcdef01234567",
                "source_revision=unknown",
            ),
        )
        .expect("unknown metadata");
        assert!(read_release_metadata(&root).is_err());
        fs::write(root.join("RELEASE"), format!("{valid}extra=value\n"))
            .expect("unknown field metadata");
        assert!(read_release_metadata(&root).is_err());
        fs::remove_dir_all(root).expect("remove metadata root");
    }

    #[test]
    fn effective_systemd_contract_parsers_reject_overrides() {
        let parsed = parse_systemd_show_properties(
            b"LoadState=loaded\nFragmentPath=/etc/systemd/system/v2board-api.service\nDropInPaths=\n",
            &["LoadState", "FragmentPath", "DropInPaths"],
        )
        .expect("properties");
        assert_eq!(parsed.get("DropInPaths").map(String::as_str), Some(""));
        assert!(
            parse_systemd_show_properties(b"LoadState=loaded\nLoadState=masked\n", &["LoadState"],)
                .is_err()
        );
        verify_systemd_exec_start(
            "{ path=/opt/v2board/current/bin/v2board-api ; argv[]=/opt/v2board/current/bin/v2board-api ; ignore_errors=no ; pid=0 ; code=(null) ; status=0/0 }",
            "/opt/v2board/current/bin/v2board-api",
        )
        .expect("exact ExecStart");
        assert!(
            verify_systemd_exec_start(
                "{ path=/tmp/wrapper ; argv[]=/tmp/wrapper ; ignore_errors=no }",
                "/opt/v2board/current/bin/v2board-api",
            )
            .is_err()
        );
        verify_systemd_environment("B=2 A=1", &["A=1", "B=2"])
            .expect("unordered exact environment");
        assert!(verify_systemd_environment("A=1 B=2 EXTRA=1", &["A=1", "B=2"]).is_err());
    }

    #[test]
    fn whole_tree_digest_covers_unlisted_files_and_symlink_targets() {
        let root = test_path("tree-digest");
        fs::create_dir_all(root.join("frontend/releases/a")).expect("tree dirs");
        fs::write(root.join("RELEASE"), b"metadata").expect("tree file");
        symlink("releases/a", root.join("frontend/current")).expect("tree symlink");
        let first = release_tree_digest(&root).expect("first digest");
        fs::write(root.join("unlisted"), b"extra").expect("unlisted file");
        let second = release_tree_digest(&root).expect("second digest");
        assert_ne!(first, second);
        fs::remove_dir_all(root).expect("remove tree");
    }

    #[test]
    fn bounded_output_redaction_removes_every_secret_occurrence() {
        let mut bytes = b"password=alpha; repeated alpha; safe".to_vec();
        redact_all(&mut bytes, &[b"alpha".to_vec()]);
        let output = String::from_utf8(bytes).expect("UTF-8");
        assert_eq!(output, "password=[REDACTED]; repeated [REDACTED]; safe");
    }

    #[test]
    fn bootstrap_sql_is_operation_owned_and_has_no_broad_adoption_clause() {
        let permit = permit();
        let migration = PostgresConnection::parse(
            "postgresql://migration:migration-password@db.example/v2board?sslmode=verify-full",
        )
        .expect("migration URL");
        let api = PostgresConnection::parse(
            "postgresql://api:api-password-value@db.example/v2board?sslmode=verify-full",
        )
        .expect("API URL");
        let worker = PostgresConnection::parse(
            "postgresql://worker:worker-password@db.example/v2board?sslmode=verify-full",
        )
        .expect("worker URL");
        let sql = String::from_utf8(postgres_bootstrap_sql(
            &permit,
            &migration,
            &api,
            &worker,
            PostgresObservedState {
                roles_exist: false,
                database_exists: false,
            },
        ))
        .expect("SQL UTF-8");
        assert!(sql.contains(OPERATION_ID));
        assert!(sql.contains(INSTALLATION_ID));
        assert!(sql.contains("SET LOCAL synchronous_commit = on"));
        assert!(!sql.to_ascii_uppercase().contains("IF NOT EXISTS"));
    }

    #[test]
    fn clickhouse_grants_cover_explicit_projection_and_metadata_objects_only() {
        let permit = permit();
        let target = ClickHouseTargetBinding {
            database: "analytics".to_string(),
            raw_retention_days: 90,
            aggregate_retention_days: 730,
            bootstrap: ClickHouseConnection {
                endpoint: "https://clickhouse.example.test".to_string(),
                database: "system".to_string(),
                username: "bootstrap".to_string(),
                password: "bootstrap-secret".to_string(),
            },
            schema: ClickHousePrincipalBinding {
                username: "schema".to_string(),
                password: "schema-secret".to_string(),
            },
            writer: ClickHousePrincipalBinding {
                username: "writer".to_string(),
                password: "writer-secret".to_string(),
            },
            reader: ClickHousePrincipalBinding {
                username: "reader".to_string(),
                password: "reader-secret".to_string(),
            },
        };
        let sql = String::from_utf8(clickhouse_bootstrap_sql(
            &permit,
            &target,
            ClickHouseObservedState {
                database_exists: false,
                schema_exists: false,
                writer_exists: false,
                reader_exists: false,
            },
        ))
        .expect("ClickHouse SQL");
        for table in [
            "v2_traffic_reported_v1",
            "v2_traffic_accounted_v1",
            "v2_traffic_reported_daily_v1",
            "v2_traffic_accounted_daily_v1",
        ] {
            assert!(sql.contains(&format!(
                "GRANT INSERT, SELECT ON analytics.{table} TO writer;"
            )));
        }
        for table in [
            "v2_schema_migration",
            "v2_installation_binding",
            "v2_retention_binding",
        ] {
            assert!(sql.contains(&format!("GRANT INSERT ON analytics.{table} TO schema;")));
            assert!(sql.contains(&format!("GRANT SELECT ON analytics.{table} TO writer;")));
            assert!(!sql.contains(&format!("GRANT INSERT ON analytics.{table} TO writer;")));
        }
        for table in [
            "v2_traffic_reported_v1",
            "v2_traffic_accounted_v1",
            "v2_traffic_reported_daily_v1",
            "v2_traffic_accounted_daily_v1",
        ] {
            assert!(!sql.contains(&format!("GRANT INSERT ON analytics.{table} TO schema;")));
        }
        assert!(sql.contains("GRANT SELECT ON analytics.* TO reader;"));
        assert!(!sql.contains("GRANT ALL"));
    }

    #[test]
    fn external_receipt_is_hash_kind_subject_and_operation_bound() {
        let path = std::env::temp_dir().join(format!(
            "v2board-native-receipt-{}-{}.json",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let receipt = ExternalControlReceipt {
            schema_version: 1,
            operation_id: OPERATION_ID.to_string(),
            kind: ExternalReceiptKind::SourceNetworkIsolated,
            subject_sha256: INSPECT_HASH.to_string(),
            completed: true,
            evidence_reference: "change-ticket:network-1234".to_string(),
            issued_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_secs() as i64,
        };
        let bytes = serde_json::to_vec(&receipt).expect("receipt JSON");
        write_new_owner_file(&path, &bytes, 0o600).expect("receipt file");
        let binding = ReceiptBinding::new(path.clone(), hex::encode(Sha256::digest(&bytes)))
            .expect("binding");
        let loaded = load_external_receipt(
            &binding,
            OPERATION_ID,
            ExternalReceiptKind::SourceNetworkIsolated,
            INSPECT_HASH,
        )
        .expect("bound receipt");
        assert_eq!(loaded, receipt);
        assert!(
            load_external_receipt(
                &binding,
                OPERATION_ID,
                ExternalReceiptKind::SourceCredentialsRevoked,
                INSPECT_HASH,
            )
            .is_err()
        );
        fs::remove_file(path).expect("remove receipt fixture");
    }

    #[test]
    fn missing_real_observer_and_committer_fail_closed() {
        let verification = OfflineMigrationVerification {
            operation_id: OPERATION_ID.to_string(),
            manifest_binding_hmac_sha256: "f".repeat(64),
            installation_id: INSTALLATION_ID.to_string(),
            inspect_review_sha256: INSPECT_HASH.to_string(),
            backup_restore_report_sha256: BACKUP_HASH.to_string(),
            backup_reference_sha256: BACKUP_REFERENCE_HASH.to_string(),
            final_recheck_report_sha256: FINAL_RECHECK_HASH.to_string(),
            source_fingerprint_sha256: SOURCE_FINGERPRINT_HASH.to_string(),
            bootstrap_permit_generation: 6,
            bootstrap_permit_event_sha256: "1".repeat(64),
            target_permit_generation: 6,
            target_permit_event_sha256: "1".repeat(64),
            data_verification_report_sha256: "2".repeat(64),
            analytics_projection_report_sha256: "3".repeat(64),
            node_cutover_report_sha256: Some("4".repeat(64)),
            old_writers_fenced: true,
            new_writers_still_stopped: true,
            postgres_is_transaction_authority: true,
            clickhouse_is_rebuildable_projection: true,
        };
        let mut committer = DenyNativeAuthorityCommitter;
        assert!(committer.verify_gate(&verification).is_err());
    }
}
