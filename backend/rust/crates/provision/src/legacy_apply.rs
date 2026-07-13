//! One-shot legacy migration orchestration.
//!
//! This module owns ordering and crash semantics only. Datastore and native
//! host mutations are supplied by an executor and must themselves be
//! operation-bound and retry-idempotent. There is intentionally no CDC,
//! dual-write, shadow-read, gradual node rollout, or MySQL runtime fallback.

use std::{
    fs::{self, File, OpenOptions},
    future::Future,
    io,
    path::{Path, PathBuf},
    pin::Pin,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ApplyAuthorization, ApplyAuthorizationError, ProvisionSpec,
    apply_journal::{
        ApplyCheckpoint, ApplyJournal, ApplyJournalBinding, ApplyJournalError,
        ApplyJournalSnapshot, ApplyJournalState, ApplyOutcomeCode, DurableMutationPermit,
        DurableNativeStartPermit, DurableTargetMutationPermit, NativeAuthorityBinding,
        backup_reference_sha256,
    },
};

pub type ApplyFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyApplyStage {
    FenceSource,
    DrainSource,
    BackupRestore,
    FinalRecheck,
    BootstrapTargets,
    MirrorJournal,
    CopyPostgres,
    VerifyPostgres,
    ProjectClickhouse,
    MaterializeRuntime,
    VerifyNodes,
    CollectActivationEvidence,
    CommitNativeAuthority,
    StartNativeServices,
    RetireSource,
    CollectCompletionEvidence,
    CompleteLedger,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageFailure {
    outcome: ApplyOutcomeCode,
    code: String,
}

impl StageFailure {
    pub fn sanitized(outcome: ApplyOutcomeCode, code: impl AsRef<str>) -> Self {
        let code = code.as_ref();
        let safe = !code.is_empty()
            && code.len() <= 128
            && code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
        Self {
            outcome,
            code: if safe {
                code.to_string()
            } else {
                "invalid_or_unsanitized_stage_failure".to_string()
            },
        }
    }

    pub const fn outcome(&self) -> ApplyOutcomeCode {
        self.outcome
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStageProof {
    report_sha256: String,
}

impl VerifiedStageProof {
    pub fn new(report_sha256: impl Into<String>) -> Result<Self, LegacyApplyError> {
        let proof = Self {
            report_sha256: report_sha256.into(),
        };
        if !is_lower_sha256(&proof.report_sha256) {
            return Err(LegacyApplyError::InvalidProof);
        }
        Ok(proof)
    }

    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupRestoreProof {
    report_sha256: String,
    backup_reference: String,
    backup_reference_sha256: String,
}

impl BackupRestoreProof {
    pub fn new(
        report_sha256: impl Into<String>,
        backup_reference: impl Into<String>,
    ) -> Result<Self, LegacyApplyError> {
        let report_sha256 = report_sha256.into();
        let backup_reference = backup_reference.into();
        if !is_lower_sha256(&report_sha256) || !valid_reference(&backup_reference) {
            return Err(LegacyApplyError::InvalidProof);
        }
        let backup_reference_sha256 = backup_reference_sha256(&backup_reference)
            .map_err(|_| LegacyApplyError::InvalidProof)?;
        Ok(Self {
            report_sha256,
            backup_reference,
            backup_reference_sha256,
        })
    }

    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }

    pub fn backup_reference(&self) -> &str {
        &self.backup_reference
    }

    pub fn backup_reference_sha256(&self) -> &str {
        &self.backup_reference_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalRecheckProof {
    report_sha256: String,
    source_fingerprint_sha256: String,
    source_fence_still_held: bool,
    target_still_empty: bool,
}

impl FinalRecheckProof {
    pub fn new(
        report_sha256: impl Into<String>,
        source_fingerprint_sha256: impl Into<String>,
        source_fence_still_held: bool,
        target_still_empty: bool,
    ) -> Result<Self, LegacyApplyError> {
        let proof = Self {
            report_sha256: report_sha256.into(),
            source_fingerprint_sha256: source_fingerprint_sha256.into(),
            source_fence_still_held,
            target_still_empty,
        };
        if !is_lower_sha256(&proof.report_sha256)
            || !is_lower_sha256(&proof.source_fingerprint_sha256)
            || !proof.source_fence_still_held
            || !proof.target_still_empty
        {
            return Err(LegacyApplyError::InvalidProof);
        }
        Ok(proof)
    }

    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }

    pub fn source_fingerprint_sha256(&self) -> &str {
        &self.source_fingerprint_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActivationEvidence {
    pub data_verification_report_sha256: String,
    pub analytics_projection_report_sha256: String,
    pub node_cutover_report_sha256: String,
    pub old_writers_fenced: bool,
    pub new_writers_still_stopped: bool,
    pub postgres_is_transaction_authority: bool,
    pub clickhouse_is_rebuildable_projection: bool,
}

impl ActivationEvidence {
    pub fn validate(&self) -> Result<(), LegacyApplyError> {
        if [
            &self.data_verification_report_sha256,
            &self.analytics_projection_report_sha256,
            &self.node_cutover_report_sha256,
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
            || !self.old_writers_fenced
            || !self.new_writers_still_stopped
            || !self.postgres_is_transaction_authority
            || !self.clickhouse_is_rebuildable_projection
        {
            return Err(LegacyApplyError::InvalidProof);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionEvidence {
    pub data_verification_report_sha256: String,
    pub analytics_projection_report_sha256: String,
    pub node_cutover_report_sha256: String,
    pub old_writers_fenced: bool,
    pub postgres_is_transaction_authority: bool,
    pub clickhouse_is_rebuildable_projection: bool,
    pub native_runtime_report_sha256: String,
    pub native_runtime_running_and_verified: bool,
    pub verified_backup_reference: String,
    pub verified_backup_receipt_sha256: String,
    pub verified_backup_artifact_sha256: String,
    pub source_retired: bool,
    pub mysql_reachable: bool,
    pub source_redis_reachable: bool,
    pub source_access_permanently_disabled: bool,
    pub legacy_runtime_compat: bool,
}

/// A terminal-recovery capability. Unlike `DurableTargetMutationPermit`, this
/// can only be used to re-observe source retirement and finish the permanent
/// ledger after the filesystem journal is completed; it cannot authorize DDL,
/// data copy, activation, or any other target mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionRecoveryPermit {
    operation_id: String,
    installation_id: String,
    inspect_review_sha256: String,
    backup_restore_proof_sha256: String,
    backup_reference_sha256: String,
    final_recheck_report_sha256: String,
    source_fingerprint_sha256: String,
    source_retirement_report_sha256: String,
    source_retired_generation: u64,
    source_retired_event_sha256: String,
    source_retired_predecessor_event_sha256: String,
    native_authority: NativeAuthorityBinding,
}

struct CompletionRecoveryBinding<'a> {
    operation_id: &'a str,
    installation_id: &'a str,
    inspect_review_sha256: &'a str,
    backup_restore_proof_sha256: &'a str,
    backup_reference_sha256: &'a str,
    final_recheck_report_sha256: &'a str,
    source_fingerprint_sha256: &'a str,
    source_retirement_report_sha256: &'a str,
    source_retired_generation: u64,
    source_retired_event_sha256: &'a str,
    source_retired_predecessor_event_sha256: &'a str,
    native_authority: NativeAuthorityBinding,
}

impl CompletionRecoveryPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    pub fn inspect_review_sha256(&self) -> &str {
        &self.inspect_review_sha256
    }

    pub fn backup_restore_proof_sha256(&self) -> &str {
        &self.backup_restore_proof_sha256
    }

    pub fn backup_reference_sha256(&self) -> &str {
        &self.backup_reference_sha256
    }

    pub fn final_recheck_report_sha256(&self) -> &str {
        &self.final_recheck_report_sha256
    }

    pub fn source_fingerprint_sha256(&self) -> &str {
        &self.source_fingerprint_sha256
    }

    pub fn source_retired_event_sha256(&self) -> &str {
        &self.source_retired_event_sha256
    }

    pub const fn source_retired_generation(&self) -> u64 {
        self.source_retired_generation
    }

    pub fn source_retired_predecessor_event_sha256(&self) -> &str {
        &self.source_retired_predecessor_event_sha256
    }

    pub fn source_retirement_report_sha256(&self) -> &str {
        &self.source_retirement_report_sha256
    }

    pub fn native_authority_binding(&self) -> &NativeAuthorityBinding {
        &self.native_authority
    }

    fn from_source_retired_snapshot(
        snapshot: &ApplyJournalSnapshot,
    ) -> Result<Self, LegacyApplyError> {
        if snapshot.state() != ApplyJournalState::Verifying
            || snapshot.checkpoint() != ApplyCheckpoint::SourceRetired
        {
            return Err(LegacyApplyError::InvalidJournalState);
        }
        Self::new(CompletionRecoveryBinding {
            operation_id: snapshot.binding().operation_id(),
            installation_id: snapshot
                .installation_id()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            inspect_review_sha256: snapshot.binding().inspect_review_sha256(),
            backup_restore_proof_sha256: snapshot
                .backup_restore_proof_sha256()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            backup_reference_sha256: snapshot
                .backup_reference_sha256()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            final_recheck_report_sha256: snapshot
                .final_recheck_report_sha256()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            source_fingerprint_sha256: snapshot
                .source_fingerprint_sha256()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            source_retirement_report_sha256: snapshot
                .checkpoint_proof_sha256()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            source_retired_generation: snapshot.generation(),
            source_retired_event_sha256: snapshot.event_sha256(),
            source_retired_predecessor_event_sha256: snapshot
                .previous_event_sha256()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
            native_authority: snapshot
                .native_authority_binding()
                .ok_or(LegacyApplyError::InvalidJournalState)?,
        })
    }

    fn new(binding: CompletionRecoveryBinding<'_>) -> Result<Self, LegacyApplyError> {
        let operation_id = Uuid::parse_str(binding.operation_id)
            .map_err(|_| LegacyApplyError::InvalidJournalState)?;
        let installation_id = Uuid::parse_str(binding.installation_id)
            .map_err(|_| LegacyApplyError::InvalidJournalState)?;
        if operation_id.is_nil()
            || installation_id.is_nil()
            || [
                binding.inspect_review_sha256,
                binding.backup_restore_proof_sha256,
                binding.backup_reference_sha256,
                binding.final_recheck_report_sha256,
                binding.source_fingerprint_sha256,
                binding.source_retirement_report_sha256,
                binding.source_retired_event_sha256,
                binding.source_retired_predecessor_event_sha256,
            ]
            .into_iter()
            .any(|value| !is_lower_sha256(value))
            || binding.source_retired_generation < 2
        {
            return Err(LegacyApplyError::InvalidJournalState);
        }
        Ok(Self {
            operation_id: operation_id.hyphenated().to_string(),
            installation_id: installation_id.hyphenated().to_string(),
            inspect_review_sha256: binding.inspect_review_sha256.to_string(),
            backup_restore_proof_sha256: binding.backup_restore_proof_sha256.to_string(),
            backup_reference_sha256: binding.backup_reference_sha256.to_string(),
            final_recheck_report_sha256: binding.final_recheck_report_sha256.to_string(),
            source_fingerprint_sha256: binding.source_fingerprint_sha256.to_string(),
            source_retirement_report_sha256: binding.source_retirement_report_sha256.to_string(),
            source_retired_generation: binding.source_retired_generation,
            source_retired_event_sha256: binding.source_retired_event_sha256.to_string(),
            source_retired_predecessor_event_sha256: binding
                .source_retired_predecessor_event_sha256
                .to_string(),
            native_authority: binding.native_authority,
        })
    }
}

impl CompletionEvidence {
    pub fn validate(&self) -> Result<(), LegacyApplyError> {
        if [
            &self.data_verification_report_sha256,
            &self.analytics_projection_report_sha256,
            &self.node_cutover_report_sha256,
            &self.native_runtime_report_sha256,
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
            || !self.old_writers_fenced
            || !self.postgres_is_transaction_authority
            || !self.clickhouse_is_rebuildable_projection
            || !self.native_runtime_running_and_verified
            || !valid_reference(&self.verified_backup_reference)
            || !is_lower_sha256(&self.verified_backup_receipt_sha256)
            || !is_lower_sha256(&self.verified_backup_artifact_sha256)
            || !self.source_retired
            || self.mysql_reachable
            || self.source_redis_reachable
            || !self.source_access_permanently_disabled
            || self.legacy_runtime_compat
        {
            return Err(LegacyApplyError::InvalidProof);
        }
        Ok(())
    }
}

/// Every mutating method is retried after an interrupted process. Concrete
/// implementations therefore must compare operation ownership and the exact
/// desired value instead of treating a broad `IF NOT EXISTS` as success.
pub(crate) trait LegacyApplyExecutor {
    fn fence_source<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn drain_source<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn backup_and_restore_test<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<BackupRestoreProof, StageFailure>>;

    fn final_recheck<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        reviewed_inspect_review_sha256: &'a str,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<FinalRecheckProof, StageFailure>>;

    fn bootstrap_targets<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
        history: &'a [ApplyJournalSnapshot],
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    /// Reconcile the complete hash-chained history, not only its current head.
    /// A failed stage may add both `needs_recovery` and `resume` events while
    /// PostgreSQL is unavailable.
    fn mirror_journal_history<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        history: &'a [ApplyJournalSnapshot],
    ) -> ApplyFuture<'a, Result<(), StageFailure>>;

    fn copy_postgres<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn verify_postgres<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn project_clickhouse<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn materialize_runtime<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn verify_nodes_offline<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn collect_activation_evidence<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<ActivationEvidence, StageFailure>>;

    /// Atomically commits PostgreSQL as the sole transactional authority but
    /// must not start any native service. Lost acknowledgements are recovered
    /// by observing the append-only PostgreSQL activation commit and returning
    /// its original NodesVerified anchor.
    fn commit_native_authority_once<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
        evidence: &'a ActivationEvidence,
    ) -> ApplyFuture<'a, Result<NativeAuthorityBinding, StageFailure>>;

    /// Starts/reconciles the native API, worker, and all offline-staged node
    /// reporters only after the authority proof itself is fsync-durable.
    fn start_native_services_once<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableNativeStartPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn retire_source<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>>;

    fn collect_completion_evidence<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a CompletionRecoveryPermit,
    ) -> ApplyFuture<'a, Result<CompletionEvidence, StageFailure>>;

    /// Idempotently commit the exact completed filesystem head to PostgreSQL.
    /// The disposable lifecycle binary remains available for forward recovery;
    /// its later manual removal is an operator cleanup action, never a
    /// prerequisite for recording successful migration.
    fn complete_permanent_ledger<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        completed: &'a ApplyJournalSnapshot,
        evidence: &'a CompletionEvidence,
    ) -> ApplyFuture<'a, Result<(), StageFailure>>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyApplyResult {
    pub operation_id: String,
    pub installation_id: String,
    pub journal_generation: u64,
    pub journal_event_sha256: String,
    pub completed: bool,
    pub mysql_runtime_retired: bool,
    pub manual_cleanup: ManualCleanupAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManualCleanupAction {
    pub status: &'static str,
    pub run_as: &'static str,
    pub argv: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LegacyApplyError {
    #[error("one-shot authorization is invalid: {0}")]
    Authorization(#[from] ApplyAuthorizationError),
    #[error("one-shot journal failed: {0}")]
    Journal(#[from] ApplyJournalError),
    #[error("another process already owns this one-shot operation")]
    ConcurrentExecution,
    #[error("one-shot execution lock failed: {0}")]
    Lock(#[source] io::Error),
    #[error("a stage returned malformed or incomplete proof")]
    InvalidProof,
    #[error("journal state cannot be executed or recovered")]
    InvalidJournalState,
    #[error(
        "stage {stage:?} failed with sanitized code {code}; recovery generation {recovery_generation}"
    )]
    Stage {
        stage: LegacyApplyStage,
        code: String,
        recovery_generation: u64,
    },
}

pub(crate) async fn start_legacy_apply<E: LegacyApplyExecutor>(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    now_unix: i64,
    journal_root: &Path,
    executor: &mut E,
) -> Result<LegacyApplyResult, LegacyApplyError> {
    authorization.verify_new_apply(spec, now_unix)?;
    let binding =
        ApplyJournalBinding::new(&spec.operation_id, &authorization.inspect_review_sha256)?;
    let (journal, head) = ApplyJournal::create_pending(journal_root, binding)?;
    let _lock = ApplyExecutionLock::acquire(journal_root, &spec.operation_id)?;
    run_legacy_apply(spec, authorization, &journal, head, executor).await
}

/// Resume the same durable operation without a second cutover decision.
/// Authorization expiry is ignored only after the exact journal already
/// exists; its HMAC, manifest, operation, and reviewed-report binding are
/// still verified.
pub(crate) async fn resume_legacy_apply<E: LegacyApplyExecutor>(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    journal_root: &Path,
    executor: &mut E,
) -> Result<LegacyApplyResult, LegacyApplyError> {
    authorization.verify_resume_binding(spec)?;
    let binding =
        ApplyJournalBinding::new(&spec.operation_id, &authorization.inspect_review_sha256)?;
    let (journal, mut head) = ApplyJournal::open(journal_root, binding)?;
    let _lock = ApplyExecutionLock::acquire(journal_root, &spec.operation_id)?;
    if head.state() == ApplyJournalState::NeedsRecovery {
        head = journal.resume(&head)?;
    }
    run_legacy_apply(spec, authorization, &journal, head, executor).await
}

async fn run_legacy_apply<E: LegacyApplyExecutor>(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    journal: &ApplyJournal,
    mut head: ApplyJournalSnapshot,
    executor: &mut E,
) -> Result<LegacyApplyResult, LegacyApplyError> {
    loop {
        if head.state() == ApplyJournalState::Completed {
            return finalize_completed(spec, journal, head, executor).await;
        }
        if matches!(
            head.state(),
            ApplyJournalState::Failed | ApplyJournalState::NeedsRecovery
        ) {
            return Err(LegacyApplyError::InvalidJournalState);
        }
        if head.checkpoint() >= ApplyCheckpoint::TargetsBootstrapped {
            let history = journal.verified_history()?;
            if let Err(failure) = executor.mirror_journal_history(spec, &history).await {
                return Err(record_stage_failure(
                    journal,
                    &head,
                    LegacyApplyStage::MirrorJournal,
                    failure,
                ));
            }
        }

        match head.checkpoint() {
            ApplyCheckpoint::PendingDurable => {
                if head.state() == ApplyJournalState::Pending {
                    head = journal.begin(&head)?;
                    continue;
                }
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::FenceSource,
                    executor.fence_source(spec, &head).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::MaintenanceFenced,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::MaintenanceFenced => {
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::DrainSource,
                    executor.drain_source(spec, &head).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::SourceDrained,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::SourceDrained => {
                let proof = match executor.backup_and_restore_test(spec, &head).await {
                    Ok(proof) => proof,
                    Err(failure) => {
                        return Err(record_stage_failure(
                            journal,
                            &head,
                            LegacyApplyStage::BackupRestore,
                            failure,
                        ));
                    }
                };
                head = journal.record_backup_restore_verified(
                    &head,
                    proof.report_sha256(),
                    proof.backup_reference_sha256(),
                )?;
            }
            ApplyCheckpoint::BackupRestoreVerified => {
                let proof = match executor
                    .final_recheck(spec, &authorization.inspect_review_sha256, &head)
                    .await
                {
                    Ok(proof) => proof,
                    Err(failure) => {
                        return Err(record_stage_failure(
                            journal,
                            &head,
                            LegacyApplyStage::FinalRecheck,
                            failure,
                        ));
                    }
                };
                if proof.source_fingerprint_sha256().is_empty() {
                    return Err(LegacyApplyError::InvalidProof);
                }
                head = journal.record_final_recheck_passed(
                    &head,
                    proof.report_sha256(),
                    proof.source_fingerprint_sha256(),
                )?;
            }
            ApplyCheckpoint::FinalRecheckPassed => {
                let installation_id = Uuid::new_v4();
                head = journal.reserve_installation_identity(
                    &head,
                    installation_id.hyphenated().to_string(),
                )?;
            }
            ApplyCheckpoint::InstallationIdentityReserved => {
                let permit = journal.target_mutation_permit(&head)?;
                let history = journal.verified_history()?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::BootstrapTargets,
                    executor.bootstrap_targets(spec, &permit, &history).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::TargetsBootstrapped,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::TargetsBootstrapped => {
                let permit = journal.target_mutation_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::CopyPostgres,
                    executor.copy_postgres(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::PostgresBulkCopied,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::PostgresBulkCopied => {
                if head.state() == ApplyJournalState::Running {
                    head = journal.enter_verification(&head)?;
                    continue;
                }
                let permit = journal.target_mutation_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::VerifyPostgres,
                    executor.verify_postgres(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::PostgresValueVerified,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::PostgresValueVerified => {
                let permit = journal.target_mutation_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::ProjectClickhouse,
                    executor.project_clickhouse(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::ClickhouseProjected,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::ClickhouseProjected => {
                let permit = journal.target_mutation_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::MaterializeRuntime,
                    executor.materialize_runtime(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::RuntimeMaterialized,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::RuntimeMaterialized => {
                let permit = journal.target_mutation_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::VerifyNodes,
                    executor.verify_nodes_offline(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::NodesVerified,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::NodesVerified => {
                let permit = journal.target_mutation_permit(&head)?;
                let evidence = match executor.collect_activation_evidence(spec, &permit).await {
                    Ok(evidence) => evidence,
                    Err(failure) => {
                        return Err(record_stage_failure(
                            journal,
                            &head,
                            LegacyApplyStage::CollectActivationEvidence,
                            failure,
                        ));
                    }
                };
                evidence.validate()?;
                let authority = match executor
                    .commit_native_authority_once(spec, &permit, &evidence)
                    .await
                {
                    Ok(authority) => authority,
                    Err(failure) => {
                        return Err(record_stage_failure(
                            journal,
                            &head,
                            LegacyApplyStage::CommitNativeAuthority,
                            failure,
                        ));
                    }
                };
                if authority.data_verification_report_sha256()
                    != evidence.data_verification_report_sha256
                    || authority.analytics_projection_report_sha256()
                        != evidence.analytics_projection_report_sha256
                    || authority.node_cutover_report_sha256() != evidence.node_cutover_report_sha256
                {
                    return Err(LegacyApplyError::InvalidProof);
                }
                head = journal.record_native_authority_committed(&head, &authority)?;
            }
            ApplyCheckpoint::NativeAuthorityCommitted => {
                let permit = journal.native_start_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::StartNativeServices,
                    executor.start_native_services_once(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::CutoverCommitted,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::CutoverCommitted => {
                let permit = journal.mutation_permit(&head)?;
                let proof = run_proof_stage(
                    journal,
                    &head,
                    LegacyApplyStage::RetireSource,
                    executor.retire_source(spec, &permit).await,
                )?;
                head = journal.checkpoint_with_proof(
                    &head,
                    ApplyCheckpoint::SourceRetired,
                    proof.report_sha256(),
                )?;
            }
            ApplyCheckpoint::SourceRetired => {
                let completion_permit =
                    CompletionRecoveryPermit::from_source_retired_snapshot(&head)?;
                let evidence = match executor
                    .collect_completion_evidence(spec, &completion_permit)
                    .await
                {
                    Ok(evidence) => evidence,
                    Err(failure) => {
                        return Err(record_stage_failure(
                            journal,
                            &head,
                            LegacyApplyStage::CollectCompletionEvidence,
                            failure,
                        ));
                    }
                };
                evidence.validate()?;
                validate_completion_binding(&completion_permit, &evidence)?;
                let completed = journal.complete(&head, completion_evidence_sha256(&evidence)?)?;
                if let Err(failure) = executor
                    .complete_permanent_ledger(spec, &completed, &evidence)
                    .await
                {
                    // The filesystem journal is already terminal. The exact
                    // completed head is recovered idempotently on the next
                    // invocation rather than appending an impossible state.
                    return Err(LegacyApplyError::Stage {
                        stage: LegacyApplyStage::CompleteLedger,
                        code: failure.code,
                        recovery_generation: completed.generation(),
                    });
                }
                return completed_result(spec, &completed);
            }
            ApplyCheckpoint::CompletionVerified => {
                return Err(LegacyApplyError::InvalidJournalState);
            }
        }
    }
}

async fn finalize_completed<E: LegacyApplyExecutor>(
    spec: &ProvisionSpec,
    journal: &ApplyJournal,
    completed: ApplyJournalSnapshot,
    executor: &mut E,
) -> Result<LegacyApplyResult, LegacyApplyError> {
    if completed.checkpoint() != ApplyCheckpoint::CompletionVerified {
        return Err(LegacyApplyError::InvalidJournalState);
    }
    let history = journal.verified_history()?;
    // The permit cannot be issued from a terminal head. Recover the last
    // non-terminal SourceRetired event and require it to bind the completed
    // event as its direct successor.
    let source_retired = history
        .iter()
        .rev()
        .find(|snapshot| snapshot.checkpoint() == ApplyCheckpoint::SourceRetired)
        .ok_or(LegacyApplyError::InvalidJournalState)?;
    if completed.previous_event_sha256() != Some(source_retired.event_sha256()) {
        return Err(LegacyApplyError::InvalidJournalState);
    }
    let permit = CompletionRecoveryPermit::from_source_retired_snapshot(source_retired)?;
    let evidence = executor
        .collect_completion_evidence(spec, &permit)
        .await
        .map_err(|failure| LegacyApplyError::Stage {
            stage: LegacyApplyStage::CollectCompletionEvidence,
            code: failure.code,
            recovery_generation: completed.generation(),
        })?;
    evidence.validate()?;
    validate_completion_binding(&permit, &evidence)?;
    if completed.checkpoint_proof_sha256() != Some(completion_evidence_sha256(&evidence)?.as_str())
    {
        return Err(LegacyApplyError::InvalidProof);
    }
    executor
        .complete_permanent_ledger(spec, &completed, &evidence)
        .await
        .map_err(|failure| LegacyApplyError::Stage {
            stage: LegacyApplyStage::CompleteLedger,
            code: failure.code,
            recovery_generation: completed.generation(),
        })?;
    completed_result(spec, &completed)
}

fn validate_completion_binding(
    permit: &CompletionRecoveryPermit,
    evidence: &CompletionEvidence,
) -> Result<(), LegacyApplyError> {
    let authority = permit.native_authority_binding();
    if evidence.data_verification_report_sha256 != authority.data_verification_report_sha256()
        || evidence.analytics_projection_report_sha256
            != authority.analytics_projection_report_sha256()
        || evidence.node_cutover_report_sha256 != authority.node_cutover_report_sha256()
        || evidence.verified_backup_receipt_sha256 != permit.backup_restore_proof_sha256()
        || backup_reference_sha256(&evidence.verified_backup_reference)
            .map_err(|_| LegacyApplyError::InvalidProof)?
            != permit.backup_reference_sha256()
    {
        return Err(LegacyApplyError::InvalidProof);
    }
    Ok(())
}

fn completion_evidence_sha256(evidence: &CompletionEvidence) -> Result<String, LegacyApplyError> {
    let bytes = serde_json::to_vec(evidence).map_err(|_| LegacyApplyError::InvalidProof)?;
    let length = u64::try_from(bytes.len()).map_err(|_| LegacyApplyError::InvalidProof)?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-one-shot-completion-evidence-v1\0");
    digest.update(length.to_be_bytes());
    digest.update(bytes);
    Ok(hex::encode(digest.finalize()))
}

fn run_proof_stage(
    journal: &ApplyJournal,
    head: &ApplyJournalSnapshot,
    stage: LegacyApplyStage,
    result: Result<VerifiedStageProof, StageFailure>,
) -> Result<VerifiedStageProof, LegacyApplyError> {
    match result {
        Ok(proof) if is_lower_sha256(proof.report_sha256()) => Ok(proof),
        Ok(_) => Err(LegacyApplyError::InvalidProof),
        Err(failure) => Err(record_stage_failure(journal, head, stage, failure)),
    }
}

fn record_stage_failure(
    journal: &ApplyJournal,
    head: &ApplyJournalSnapshot,
    stage: LegacyApplyStage,
    failure: StageFailure,
) -> LegacyApplyError {
    match journal.mark_needs_recovery(head, failure.outcome) {
        Ok(recovery) => LegacyApplyError::Stage {
            stage,
            code: failure.code,
            recovery_generation: recovery.generation(),
        },
        Err(error) => LegacyApplyError::Journal(error),
    }
}

fn completed_result(
    spec: &ProvisionSpec,
    completed: &ApplyJournalSnapshot,
) -> Result<LegacyApplyResult, LegacyApplyError> {
    let installation_id = completed
        .installation_id()
        .ok_or(LegacyApplyError::InvalidJournalState)?;
    let lifecycle_tool_path = spec
        .legacy_apply_execution()
        .ok_or(LegacyApplyError::InvalidJournalState)?
        .source_retirement
        .lifecycle_tool_path
        .to_string_lossy()
        .into_owned();
    Ok(LegacyApplyResult {
        operation_id: completed.binding().operation_id().to_string(),
        installation_id: installation_id.to_string(),
        journal_generation: completed.generation(),
        journal_event_sha256: completed.event_sha256().to_string(),
        completed: true,
        mysql_runtime_retired: true,
        manual_cleanup: ManualCleanupAction {
            status: "operator_action_required",
            run_as: "root",
            argv: vec!["rm".to_string(), "--".to_string(), lifecycle_tool_path],
        },
    })
}

struct ApplyExecutionLock {
    _file: File,
    _path: PathBuf,
}

impl ApplyExecutionLock {
    fn acquire(root: &Path, operation_id: &str) -> Result<Self, LegacyApplyError> {
        let path = root.join(format!(".{operation_id}.apply.lock"));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let before = fs::symlink_metadata(&path).ok();
        if before
            .as_ref()
            .is_some_and(|metadata| !metadata.file_type().is_file())
        {
            return Err(LegacyApplyError::ConcurrentExecution);
        }
        let file = options.open(&path).map_err(LegacyApplyError::Lock)?;
        let opened = file.metadata().map_err(LegacyApplyError::Lock)?;
        let after = fs::symlink_metadata(&path).map_err(LegacyApplyError::Lock)?;
        if !opened.file_type().is_file()
            || !after.file_type().is_file()
            || after.file_type().is_symlink()
        {
            return Err(LegacyApplyError::ConcurrentExecution);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if opened.dev() != after.dev()
                || opened.ino() != after.ino()
                || opened.permissions().mode() & 0o077 != 0
            {
                return Err(LegacyApplyError::ConcurrentExecution);
            }
        }
        match file.try_lock() {
            Ok(()) => Ok(Self {
                _file: file,
                _path: path,
            }),
            // `TryLockError` intentionally distinguishes contention from an
            // OS locking failure, but neither condition may permit a second
            // executor to enter this irreversible operation.
            Err(_) => Err(LegacyApplyError::ConcurrentExecution),
        }
    }
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
}

#[cfg(test)]
#[path = "legacy_apply_fault_matrix.rs"]
mod fault_matrix;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn private_test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "v2board-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create private test root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .expect("make test root private");
        }
        root
    }

    #[derive(Default)]
    struct SuccessfulExecutor {
        calls: Vec<&'static str>,
    }

    impl SuccessfulExecutor {
        fn proof<'a>(
            &'a mut self,
            call: &'static str,
            byte: char,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.calls.push(call);
            Box::pin(async move {
                Ok(VerifiedStageProof::new(byte.to_string().repeat(64))
                    .expect("valid test stage proof"))
            })
        }

        fn activation_evidence() -> ActivationEvidence {
            ActivationEvidence {
                data_verification_report_sha256: "8".repeat(64),
                analytics_projection_report_sha256: "9".repeat(64),
                node_cutover_report_sha256: "b".repeat(64),
                old_writers_fenced: true,
                new_writers_still_stopped: true,
                postgres_is_transaction_authority: true,
                clickhouse_is_rebuildable_projection: true,
            }
        }
    }

    impl LegacyApplyExecutor for SuccessfulExecutor {
        fn fence_source<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _head: &'a ApplyJournalSnapshot,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("fence", '1')
        }

        fn drain_source<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _head: &'a ApplyJournalSnapshot,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("drain", '2')
        }

        fn backup_and_restore_test<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _head: &'a ApplyJournalSnapshot,
        ) -> ApplyFuture<'a, Result<BackupRestoreProof, StageFailure>> {
            self.calls.push("backup");
            Box::pin(async {
                Ok(
                    BackupRestoreProof::new("3".repeat(64), "backup:test/snapshot-1")
                        .expect("valid backup proof"),
                )
            })
        }

        fn final_recheck<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _reviewed_inspect_review_sha256: &'a str,
            _head: &'a ApplyJournalSnapshot,
        ) -> ApplyFuture<'a, Result<FinalRecheckProof, StageFailure>> {
            self.calls.push("final");
            Box::pin(async {
                Ok(
                    FinalRecheckProof::new("4".repeat(64), "5".repeat(64), true, true)
                        .expect("valid final proof"),
                )
            })
        }

        fn bootstrap_targets<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            permit: &'a DurableTargetMutationPermit,
            _history: &'a [ApplyJournalSnapshot],
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            assert_eq!(permit.source_fingerprint_sha256(), "5".repeat(64));
            assert_eq!(
                permit.backup_reference_sha256(),
                backup_reference_sha256("backup:test/snapshot-1")
                    .expect("canonical backup reference")
            );
            self.proof("bootstrap", '6')
        }

        fn mirror_journal_history<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _history: &'a [ApplyJournalSnapshot],
        ) -> ApplyFuture<'a, Result<(), StageFailure>> {
            self.calls.push("mirror");
            Box::pin(async { Ok(()) })
        }

        fn copy_postgres<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableTargetMutationPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("copy", '7')
        }

        fn verify_postgres<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableTargetMutationPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("verify", '8')
        }

        fn project_clickhouse<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableTargetMutationPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("clickhouse", '9')
        }

        fn materialize_runtime<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableTargetMutationPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("runtime", 'a')
        }

        fn verify_nodes_offline<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableTargetMutationPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("nodes", 'b')
        }

        fn collect_activation_evidence<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableTargetMutationPermit,
        ) -> ApplyFuture<'a, Result<ActivationEvidence, StageFailure>> {
            self.calls.push("activation_evidence");
            Box::pin(async { Ok(Self::activation_evidence()) })
        }

        fn commit_native_authority_once<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            permit: &'a DurableTargetMutationPermit,
            evidence: &'a ActivationEvidence,
        ) -> ApplyFuture<'a, Result<NativeAuthorityBinding, StageFailure>> {
            self.calls.push("commit_authority");
            let generation = permit.generation();
            let event = permit.event_sha256().to_string();
            let data = evidence.data_verification_report_sha256.clone();
            let analytics = evidence.analytics_projection_report_sha256.clone();
            let nodes = evidence.node_cutover_report_sha256.clone();
            Box::pin(async move {
                NativeAuthorityBinding::new(generation, event, data, analytics, nodes).map_err(
                    |_| {
                        StageFailure::sanitized(
                            ApplyOutcomeCode::ActivationFailed,
                            "authority_binding_invalid",
                        )
                    },
                )
            })
        }

        fn start_native_services_once<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            permit: &'a DurableNativeStartPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            assert_eq!(
                permit
                    .native_authority_binding()
                    .data_verification_report_sha256(),
                "8".repeat(64)
            );
            self.proof("start_native", 'c')
        }

        fn retire_source<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a DurableMutationPermit,
        ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
            self.proof("retire", 'd')
        }

        fn collect_completion_evidence<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _permit: &'a CompletionRecoveryPermit,
        ) -> ApplyFuture<'a, Result<CompletionEvidence, StageFailure>> {
            self.calls.push("completion_evidence");
            Box::pin(async {
                let activation = Self::activation_evidence();
                Ok(CompletionEvidence {
                    data_verification_report_sha256: activation.data_verification_report_sha256,
                    analytics_projection_report_sha256: activation
                        .analytics_projection_report_sha256,
                    node_cutover_report_sha256: activation.node_cutover_report_sha256,
                    old_writers_fenced: true,
                    postgres_is_transaction_authority: true,
                    clickhouse_is_rebuildable_projection: true,
                    native_runtime_report_sha256: "f".repeat(64),
                    native_runtime_running_and_verified: true,
                    verified_backup_reference: "backup:test/snapshot-1".to_string(),
                    verified_backup_receipt_sha256: "3".repeat(64),
                    verified_backup_artifact_sha256: "1".repeat(64),
                    source_retired: true,
                    mysql_reachable: false,
                    source_redis_reachable: false,
                    source_access_permanently_disabled: true,
                    legacy_runtime_compat: false,
                })
            })
        }

        fn complete_permanent_ledger<'a>(
            &'a mut self,
            _spec: &'a ProvisionSpec,
            _completed: &'a ApplyJournalSnapshot,
            _evidence: &'a CompletionEvidence,
        ) -> ApplyFuture<'a, Result<(), StageFailure>> {
            self.calls.push("complete");
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn one_shot_order_completes_and_terminal_ledger_retry_never_reconfirms() {
        let root = private_test_root("legacy-apply-orchestration");
        let spec = crate::manifest::tests::legacy_spec_for_orchestration();
        let report_sha256 = "0".repeat(64);
        let binding = ApplyJournalBinding::new(&spec.operation_id, &report_sha256)
            .expect("valid journal binding");
        let journal_root = root.join("journal");
        let (journal, head) =
            ApplyJournal::create_pending(&journal_root, binding).expect("pending journal");
        let authorization = ApplyAuthorization {
            authorization_version: 3,
            operation_id: spec.operation_id.clone(),
            manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
            inspect_review_sha256: report_sha256,
            inspect_review_binding_hmac_sha256: "1".repeat(64),
            authorized_snapshot_report_sha256: "3".repeat(64),
            authorized_snapshot_report_binding_hmac_sha256: "4".repeat(64),
            reviewed_target_redis_run_id: "5".repeat(40),
            reviewed_target_redis_database_index: 1,
            reviewed_source_default_redis_run_id: "6".repeat(40),
            reviewed_source_cache_redis_run_id: "7".repeat(40),
            issued_at_unix: 1_700_000_000,
            expires_at_unix: 1_700_086_400,
            irreversible_one_shot_approved: true,
            authorization_binding_hmac_sha256: "2".repeat(64),
        };
        let mut executor = SuccessfulExecutor::default();
        let result = run_legacy_apply(&spec, &authorization, &journal, head, &mut executor)
            .await
            .expect("complete one-shot orchestration");
        assert!(result.completed);
        assert!(result.mysql_runtime_retired);
        assert_eq!(result.manual_cleanup.status, "operator_action_required");
        assert_eq!(result.manual_cleanup.run_as, "root");
        assert_eq!(
            result.manual_cleanup.argv,
            ["rm", "--", "/opt/v2board/lifecycle/v2board-lifecycle"]
        );
        let non_mirror = executor
            .calls
            .iter()
            .copied()
            .filter(|call| *call != "mirror")
            .collect::<Vec<_>>();
        assert_eq!(
            non_mirror,
            [
                "fence",
                "drain",
                "backup",
                "final",
                "bootstrap",
                "copy",
                "verify",
                "clickhouse",
                "runtime",
                "nodes",
                "activation_evidence",
                "commit_authority",
                "start_native",
                "retire",
                "completion_evidence",
                "complete",
            ]
        );

        let completed = journal.reload().expect("reload completed journal");
        assert_eq!(completed.state(), ApplyJournalState::Completed);
        let mut recovery = SuccessfulExecutor::default();
        let recovered = run_legacy_apply(&spec, &authorization, &journal, completed, &mut recovery)
            .await
            .expect("idempotently finish permanent ledger");
        assert_eq!(recovered.journal_event_sha256, result.journal_event_sha256);
        assert_eq!(
            recovery.calls,
            ["completion_evidence", "complete"],
            "terminal recovery must not rerun cutover or ask for authorization",
        );
        fs::remove_dir_all(root).expect("remove orchestration test root");
    }

    #[test]
    fn execution_lock_allows_exactly_one_local_process_owner() {
        let root = private_test_root("legacy-apply-lock");
        let operation = "018f47b8-5ab1-7a00-8000-000000000001";
        let first = ApplyExecutionLock::acquire(&root, operation).expect("first lock");
        assert!(matches!(
            ApplyExecutionLock::acquire(&root, operation),
            Err(LegacyApplyError::ConcurrentExecution)
        ));
        drop(first);
        ApplyExecutionLock::acquire(&root, operation).expect("released lock is reusable");
        fs::remove_dir_all(root).expect("remove lock test root");
    }

    #[test]
    fn proofs_and_failure_codes_are_bounded_and_fail_closed() {
        let invalid = StageFailure::sanitized(
            ApplyOutcomeCode::ConversionFailed,
            "password=must-never-appear",
        );
        assert_eq!(invalid.code(), "invalid_or_unsanitized_stage_failure");
        assert!(VerifiedStageProof::new("a".repeat(64)).is_ok());
        assert!(VerifiedStageProof::new("A".repeat(64)).is_err());
        let backup = BackupRestoreProof::new("b".repeat(64), "backup:operation/snapshot-1")
            .expect("valid backup proof");
        assert_eq!(
            backup.backup_reference_sha256(),
            backup_reference_sha256(backup.backup_reference())
                .expect("canonical backup reference hash")
        );

        let evidence = ActivationEvidence {
            data_verification_report_sha256: "a".repeat(64),
            analytics_projection_report_sha256: "b".repeat(64),
            node_cutover_report_sha256: "c".repeat(64),
            old_writers_fenced: true,
            new_writers_still_stopped: true,
            postgres_is_transaction_authority: true,
            clickhouse_is_rebuildable_projection: true,
        };
        assert!(evidence.validate().is_ok());
        let mut incomplete = evidence;
        incomplete.new_writers_still_stopped = false;
        assert!(incomplete.validate().is_err());
    }
}
