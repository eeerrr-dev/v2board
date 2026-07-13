//! Concrete schema-v4, bare-metal one-shot executor.
//!
//! This is the only production composition of the generic lifecycle state
//! machine. Every method reconstructs its inputs from the fsync journal,
//! owner-only receipts, or PostgreSQL; values cached in this Rust process are
//! never accepted as evidence after a retry.

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{MySqlPool, PgPool};
use v2board_domain::operator_config;

#[cfg(feature = "bare-metal-fault-matrix")]
use crate::bare_metal_fault_matrix::{
    BareMetalFaultPoint, FaultMatrixError, require_installed_fault_case,
};

#[cfg(feature = "bare-metal-fault-matrix")]
macro_rules! matrix_before {
    ($point:ident) => {
        matrix_before_impl(BareMetalFaultPoint::$point).await?;
    };
}

#[cfg(not(feature = "bare-metal-fault-matrix"))]
macro_rules! matrix_before {
    ($point:ident) => {};
}

#[cfg(feature = "bare-metal-fault-matrix")]
macro_rules! matrix_after_success {
    ($point:ident) => {
        matrix_after_success_impl(BareMetalFaultPoint::$point).await?;
    };
}

#[cfg(not(feature = "bare-metal-fault-matrix"))]
macro_rules! matrix_after_success {
    ($point:ident) => {};
}

use crate::{
    ApplyAuthorization, ProvisionPlan, ProvisionSpec,
    apply_journal::{
        ApplyCheckpoint, ApplyJournal, ApplyJournalBinding, ApplyJournalSnapshot, ApplyOutcomeCode,
        DurableMutationPermit, DurableNativeStartPermit, DurableTargetMutationPermit,
        NativeAuthorityBinding,
    },
    inspect::{
        InspectionMode, TargetBundle, TargetRedisInspection, build_inspection,
        inspect_target_bundle,
    },
    legacy_apply::{
        ActivationEvidence, ApplyFuture, BackupRestoreProof, CompletionEvidence,
        CompletionRecoveryPermit, FinalRecheckProof, LegacyApplyError, LegacyApplyExecutor,
        LegacyApplyResult, StageFailure, VerifiedStageProof, resume_legacy_apply,
        start_legacy_apply,
    },
    legacy_backup::{
        ArchiveMaterializationAnchor, cleanup_runtime_decryption_identity_after_completion,
        destroy_verified_archive_materialization_after_authority,
        ensure_verified_archive_materialization, verify_persisted_backup_archive,
        verify_persisted_backup_archive_after_ledger_completion,
    },
    legacy_clickhouse::{
        legacy_clickhouse_production_blockers, project_legacy_clickhouse,
        verify_legacy_clickhouse_projection_read_only,
    },
    legacy_converter::{
        ConversionRunBinding, DEFAULT_BATCH_SIZE, LEGACY_SEMANTIC_SCHEMA_SHA256, registry_sha256,
    },
    legacy_copy::{
        LegacyCopyAdapter, LegacyCopyVerification, PostgresDurableCopyCheckpointSink,
        legacy_copy_verification_sha256, load_verified_frozen_traffic,
    },
    lifecycle_ledger::{
        AuthorizationAuditBinding, CompletionProofBinding, LifecycleLedgerBinding,
        NativeActivationProofBinding, bootstrap_lifecycle_ledger, bootstrap_postgres_schema,
        commit_native_activation, complete_lifecycle_ledger, mirror_lifecycle_snapshot,
        observe_native_activation_commit,
    },
    manifest::{ProvisionFlow, SourceSpec, TargetSpec},
    native_activation::{
        BareMetalActivationExecutor, DenyNativeAuthorityCommitter, NativeActivationPolicy,
        ProcessCommandRunner, ReceiptBinding, inspect_release_archive_read_only,
        start_native_units_after_authority, verify_native_runtime_after_cutover,
        verify_native_units_stopped_before_authority, verify_release_artifact_for_one_shot,
    },
    native_legacy_source::{
        BareMetalLegacySource, BareMetalRetirementObserver, VerifiedFrozenTrafficReceipt,
        verify_source_retirement_for_completion,
    },
    native_node_cutover::NativeNodeCutover,
    target_activation::{
        ExecutorError, ReleaseArtifactSpec, TargetActivationExecutor, TargetRedisInspectionBinding,
        bootstrap_empty_initial_targets, materialize_role_configs,
    },
};

#[cfg(feature = "bare-metal-fault-matrix")]
pub(crate) const WIRED_BARE_METAL_FAULT_POINTS: &[BareMetalFaultPoint] = &[
    BareMetalFaultPoint::SourceFenceCommit,
    BareMetalFaultPoint::SourceDrainCommit,
    BareMetalFaultPoint::BackupArchivePublish,
    BareMetalFaultPoint::ArchiveMaterializationPublish,
    BareMetalFaultPoint::TargetHostBootstrap,
    BareMetalFaultPoint::PostgresSchemaBootstrap,
    BareMetalFaultPoint::LifecycleLedgerBootstrap,
    BareMetalFaultPoint::LifecycleJournalMirror,
    BareMetalFaultPoint::PostgresBulkCopy,
    BareMetalFaultPoint::ClickhouseProjectionCommit,
    BareMetalFaultPoint::ReleaseArtifactReconcile,
    BareMetalFaultPoint::RuntimeConfigInstall,
    BareMetalFaultPoint::NativeAuthorityCommit,
    BareMetalFaultPoint::ArchiveMaterializationDestroy,
    BareMetalFaultPoint::NativeServicesStart,
    BareMetalFaultPoint::NodeActivationCommit,
    BareMetalFaultPoint::SourceRetirementCommit,
    BareMetalFaultPoint::LifecycleCompletionCommit,
    BareMetalFaultPoint::RuntimeDecryptionIdentityDestroy,
];

pub use crate::legacy_apply_capability::{
    PRODUCTION_LEGACY_APPLY_CAPABILITY, ProductionLegacyApplyBlocker,
    ProductionLegacyApplyCapability, production_legacy_apply_capability_for_spec,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthorizedRedisIdentity {
    target_run_id: String,
    target_database_index: u32,
    source_default_run_id: String,
    source_cache_run_id: String,
}

/// Secret-free, retry-stable evidence that the one manifest-derived dynamic
/// candidate was installed directly into the encrypted PostgreSQL authority.
/// These fields are included in the runtime-materialization stage digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct InitialOperatorAuthorityProof {
    operation_id: String,
    installation_id: String,
    revision: i64,
    revision_id: String,
    format_version: i16,
    config_hmac_sha256: String,
}

impl AuthorizedRedisIdentity {
    fn from_authorization(authorization: &ApplyAuthorization) -> Self {
        Self {
            target_run_id: authorization.reviewed_target_redis_run_id.clone(),
            target_database_index: authorization.reviewed_target_redis_database_index,
            source_default_run_id: authorization.reviewed_source_default_redis_run_id.clone(),
            source_cache_run_id: authorization.reviewed_source_cache_redis_run_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthorizationAuditIdentity {
    authorized_snapshot_report_sha256: String,
    authorized_snapshot_report_binding_hmac_sha256: String,
    authorization_binding_hmac_sha256: String,
    authorization_file_sha256: String,
}

impl AuthorizationAuditIdentity {
    fn from_authorization(
        authorization: &ApplyAuthorization,
        authorization_file_sha256: &str,
    ) -> Self {
        Self {
            authorized_snapshot_report_sha256: authorization
                .authorized_snapshot_report_sha256
                .clone(),
            authorized_snapshot_report_binding_hmac_sha256: authorization
                .authorized_snapshot_report_binding_hmac_sha256
                .clone(),
            authorization_binding_hmac_sha256: authorization
                .authorization_binding_hmac_sha256
                .clone(),
            authorization_file_sha256: authorization_file_sha256.to_string(),
        }
    }

    fn ledger_binding(
        &self,
    ) -> Result<AuthorizationAuditBinding, crate::lifecycle_ledger::LifecycleLedgerError> {
        AuthorizationAuditBinding::new(
            &self.authorized_snapshot_report_sha256,
            &self.authorized_snapshot_report_binding_hmac_sha256,
            &self.authorization_binding_hmac_sha256,
            &self.authorization_file_sha256,
        )
    }
}

struct ProductionLegacyApplyExecutor {
    authorized_redis: Option<AuthorizedRedisIdentity>,
    authorization_audit: AuthorizationAuditIdentity,
}

impl ProductionLegacyApplyExecutor {
    fn for_resume(authorization: &ApplyAuthorization, authorization_file_sha256: &str) -> Self {
        Self {
            authorized_redis: Some(AuthorizedRedisIdentity::from_authorization(authorization)),
            authorization_audit: AuthorizationAuditIdentity::from_authorization(
                authorization,
                authorization_file_sha256,
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProductionLegacyApplyEntryError {
    #[error("production one-shot {action} is fail-closed: {reason}")]
    Capability {
        action: &'static str,
        reason: &'static str,
    },
    #[cfg(feature = "bare-metal-fault-matrix")]
    #[error(transparent)]
    FaultMatrix(#[from] FaultMatrixError),
    #[error("native release archive failed the complete read-only admission preflight")]
    ReleasePreflight(#[source] ExecutorError),
    #[error(transparent)]
    Apply(#[from] LegacyApplyError),
}

/// The only production start entry. Capability admission happens before the
/// journal path is opened and before a concrete executor can be constructed.
pub async fn start_production_legacy_apply(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    authorization_file_sha256: &str,
    now_unix: i64,
) -> Result<LegacyApplyResult, ProductionLegacyApplyEntryError> {
    require_production_entry_capability("apply", spec)?;
    // Complete release validation must precede opening the lifecycle journal,
    // constructing an executor, fencing the source, or writing any target.
    inspect_release_archive_read_only(spec)
        .map_err(ProductionLegacyApplyEntryError::ReleasePreflight)?;
    let journal_root = &spec
        .legacy_apply_execution()
        .expect("capability requires schema-v4 execution")
        .journal
        .root;
    let mut executor =
        ProductionLegacyApplyExecutor::for_resume(authorization, authorization_file_sha256);
    start_legacy_apply(spec, authorization, now_unix, journal_root, &mut executor)
        .await
        .map_err(Into::into)
}

/// The only production resume entry. The same spec-aware capability gate is
/// re-evaluated before the existing journal can be opened.
pub async fn resume_production_legacy_apply(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    authorization_file_sha256: &str,
) -> Result<LegacyApplyResult, ProductionLegacyApplyEntryError> {
    require_production_entry_capability("resume", spec)?;
    let journal_root = &spec
        .legacy_apply_execution()
        .expect("capability requires schema-v4 execution")
        .journal
        .root;
    let mut executor =
        ProductionLegacyApplyExecutor::for_resume(authorization, authorization_file_sha256);
    resume_legacy_apply(spec, authorization, journal_root, &mut executor)
        .await
        .map_err(Into::into)
}

/// Feature-only start entry for the destructive matrix guest.
#[cfg(feature = "bare-metal-fault-matrix")]
pub async fn start_bare_metal_fault_matrix_legacy_apply(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    authorization_file_sha256: &str,
    now_unix: i64,
) -> Result<LegacyApplyResult, ProductionLegacyApplyEntryError> {
    require_installed_fault_case(&spec.operation_id)?;
    inspect_release_archive_read_only(spec)
        .map_err(ProductionLegacyApplyEntryError::ReleasePreflight)?;
    let journal_root = &spec
        .legacy_apply_execution()
        .ok_or(ProductionLegacyApplyEntryError::Capability {
            action: "matrix_apply",
            reason: "matrix_requires_schema_v4_legacy_execution",
        })?
        .journal
        .root;
    let mut executor =
        ProductionLegacyApplyExecutor::for_resume(authorization, authorization_file_sha256);
    start_legacy_apply(spec, authorization, now_unix, journal_root, &mut executor)
        .await
        .map_err(Into::into)
}

/// Feature-only resume entry. An exact existing ready record consumes the
/// selected hook across process restarts, preventing a second injection.
#[cfg(feature = "bare-metal-fault-matrix")]
pub async fn resume_bare_metal_fault_matrix_legacy_apply(
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
    authorization_file_sha256: &str,
) -> Result<LegacyApplyResult, ProductionLegacyApplyEntryError> {
    require_installed_fault_case(&spec.operation_id)?;
    let journal_root = &spec
        .legacy_apply_execution()
        .ok_or(ProductionLegacyApplyEntryError::Capability {
            action: "matrix_resume",
            reason: "matrix_requires_schema_v4_legacy_execution",
        })?
        .journal
        .root;
    let mut executor =
        ProductionLegacyApplyExecutor::for_resume(authorization, authorization_file_sha256);
    resume_legacy_apply(spec, authorization, journal_root, &mut executor)
        .await
        .map_err(Into::into)
}

fn require_production_entry_capability(
    action: &'static str,
    spec: &ProvisionSpec,
) -> Result<(), ProductionLegacyApplyEntryError> {
    if let Some(blocker) = production_legacy_apply_capability_for_spec(spec).blocker() {
        return Err(ProductionLegacyApplyEntryError::Capability {
            action,
            reason: blocker.report_message(),
        });
    }
    Ok(())
}

impl LegacyApplyExecutor for ProductionLegacyApplyExecutor {
    fn fence_source<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let mut source = BareMetalLegacySource::from_manifest(spec)
                .map_err(|_| failed(ApplyOutcomeCode::FenceUncertain, "source_policy_invalid"))?;
            matrix_before!(SourceFenceCommit);
            let proof = source.fence_source(spec, head).await?;
            matrix_after_success!(SourceFenceCommit);
            Ok(proof)
        })
    }

    fn drain_source<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let mut source = BareMetalLegacySource::from_manifest(spec)
                .map_err(|_| failed(ApplyOutcomeCode::DrainIncomplete, "source_policy_invalid"))?;
            matrix_before!(SourceDrainCommit);
            let proof = source.drain_source(spec, head).await?;
            matrix_after_success!(SourceDrainCommit);
            Ok(proof)
        })
    }

    fn backup_and_restore_test<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<BackupRestoreProof, StageFailure>> {
        Box::pin(async move {
            // Run every source condition that could still require operator
            // repair before creating the immutable, no-clobber archive.  A
            // second short final check remains after the restore drill to
            // detect drift, but it must never be the first time we discover a
            // repairable source blocker.
            let admitted = build_inspection(spec, InspectionMode::FencedFinal)
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "pre_backup_source_admission_failed",
                    )
                })?;
            if !admitted.blockers.is_empty() {
                return Err(failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "pre_backup_source_admission_blocked",
                ));
            }
            require_targets_still_empty(self.authorized_redis.as_ref(), &admitted)?;
            let mut source = BareMetalLegacySource::from_manifest(spec)
                .map_err(|_| failed(ApplyOutcomeCode::BackupInvalid, "source_policy_invalid"))?;
            matrix_before!(BackupArchivePublish);
            let proof = source.backup_and_restore_test(spec, head).await?;
            matrix_after_success!(BackupArchivePublish);
            Ok(proof)
        })
    }

    fn final_recheck<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        reviewed_inspect_review_sha256: &'a str,
        head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<FinalRecheckProof, StageFailure>> {
        Box::pin(async move {
            // The complete, repairable source admission already passed before
            // the immutable archive was created. This post-backup seal is
            // deliberately shorter and can recover when old Redis has since
            // disappeared: targets must still be empty, MySQL data/schema and
            // unit fences are re-read, and the traffic fact comes from the
            // self-contained encrypted archive. If Redis is reachable, the
            // traffic loader additionally performs strict read-only live
            // reconciliation and rejects any unknown key.
            let target_observation =
                inspect_target_bundle(legacy_target(spec)?)
                    .await
                    .map_err(|_| {
                        failed(
                            ApplyOutcomeCode::TargetDrift,
                            "fenced_target_inspection_failed",
                        )
                    })?;
            require_target_bundle_still_empty(self.authorized_redis.as_ref(), &target_observation)?;
            let target_proof = proof_from_serializable(
                b"v2board-post-backup-target-empty-v1\0",
                &target_observation,
                ApplyOutcomeCode::TargetDrift,
                "post_backup_target_empty_report_failed",
            )?;
            let history = verified_history(spec, reviewed_inspect_review_sha256)?;
            let source_drained = source_drained_snapshot(&history)?;
            let traffic = load_verified_frozen_traffic(spec, source_drained)
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "post_backup_archived_traffic_verification_failed",
                    )
                })?;
            require_archived_source_redis_authorized(
                self.authorized_redis.as_ref(),
                traffic.receipt(),
            )?;
            matrix_before!(ArchiveMaterializationPublish);
            let materialization = ensure_verified_archive_materialization(
                spec,
                ArchiveMaterializationAnchor::Journal(head),
            )
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::BackupInvalid,
                    "archive_materialization_failed",
                )
            })?;
            matrix_after_success!(ArchiveMaterializationPublish);
            let mut source = BareMetalLegacySource::from_manifest(spec)
                .map_err(|_| failed(ApplyOutcomeCode::FenceUncertain, "source_policy_invalid"))?;
            source
                .final_recheck(
                    spec,
                    reviewed_inspect_review_sha256,
                    head,
                    target_proof.report_sha256(),
                    traffic.receipt(),
                    materialization.database_url(),
                )
                .await
        })
    }

    fn bootstrap_targets<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
        history: &'a [ApplyJournalSnapshot],
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let redis_binding = self.redis_binding(spec, permit).await?;
            matrix_before!(TargetHostBootstrap);
            let bootstrapped = {
                let mut host = activation_executor(spec, permit)?;
                bootstrap_empty_initial_targets(spec, permit, redis_binding, &mut host).map_err(
                    |_| {
                        failed(
                            ApplyOutcomeCode::TargetDrift,
                            "target_bootstrap_or_exact_reconcile_failed",
                        )
                    },
                )?
            };
            matrix_after_success!(TargetHostBootstrap);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let target = legacy_target(spec)?;
                    let pool = PgPool::connect(&target.postgres.migration_database_url)
                        .await
                        .map_err(|_| {
                            failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed")
                        })?;
                    matrix_before!(PostgresSchemaBootstrap);
                    bootstrap_postgres_schema(&pool, spec, permit)
                        .await
                        .map_err(|_| {
                            failed(
                                ApplyOutcomeCode::TargetDrift,
                                "postgres_baseline_bootstrap_failed",
                            )
                        })?;
                    matrix_after_success!(PostgresSchemaBootstrap);
                    let operation_exists: bool = sqlx::query_scalar(
                        "SELECT EXISTS (SELECT 1 FROM v2_lifecycle_operation WHERE operation_id::text = $1)",
                    )
                    .bind(&spec.operation_id)
                    .fetch_one(&pool)
                    .await
                    .map_err(|_| {
                        failed(
                            ApplyOutcomeCode::IoFailure,
                            "lifecycle_probe_failed",
                        )
                    })?;
                    let authorization_audit =
                        self.authorization_audit.ledger_binding().map_err(|_| {
                            failed(
                                ApplyOutcomeCode::TargetDrift,
                                "authorization_audit_binding_invalid",
                            )
                        })?;
                    if operation_exists {
                        reconcile_mirrored_history(&pool, spec, history, &authorization_audit)
                            .await?;
                    } else {
                        let execution = execution(spec)?;
                        let binding = LifecycleLedgerBinding::new(
                            permit.source_fingerprint_sha256(),
                            &execution.backup.backup_reference,
                            authorization_audit,
                        )
                        .map_err(|_| {
                            failed(
                                ApplyOutcomeCode::TargetDrift,
                                "lifecycle_binding_invalid",
                            )
                        })?;
                        matrix_before!(LifecycleLedgerBootstrap);
                        bootstrap_lifecycle_ledger(&pool, spec, permit, history, &binding)
                            .await
                            .map_err(|_| {
                                failed(
                                    ApplyOutcomeCode::TargetDrift,
                                    "lifecycle_ledger_bootstrap_failed",
                                )
                            })?;
                        matrix_after_success!(LifecycleLedgerBootstrap);
                    }
                    VerifiedStageProof::new(bootstrapped.target_proof_sha256().to_string())
                        .map_err(|_| {
                            failed(
                                ApplyOutcomeCode::TargetDrift,
                                "target_bootstrap_proof_invalid",
                            )
                        })
                })
            })
        })
    }

    fn mirror_journal_history<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        history: &'a [ApplyJournalSnapshot],
    ) -> ApplyFuture<'a, Result<(), StageFailure>> {
        Box::pin(async move {
            let authorization_audit = self.authorization_audit.ledger_binding().map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "authorization_audit_binding_invalid",
                )
            })?;
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            matrix_before!(LifecycleJournalMirror);
            reconcile_mirrored_history(&pool, spec, history, &authorization_audit).await?;
            matrix_after_success!(LifecycleJournalMirror);
            Ok(())
        })
    }

    fn copy_postgres<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            matrix_before!(PostgresBulkCopy);
            let verification = execute_or_verify_copy(spec, permit, true).await?;
            matrix_after_success!(PostgresBulkCopy);
            stage_proof_for_copy(&verification)
        })
    }

    fn verify_postgres<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let verification = execute_or_verify_copy(spec, permit, false).await?;
            let proof = stage_proof_for_copy(&verification)?;
            if permit.checkpoint_proof_sha256() != Some(proof.report_sha256()) {
                return Err(failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "postgres_independent_report_mismatch",
                ));
            }
            Ok(proof)
        })
    }

    fn project_clickhouse<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            if !legacy_clickhouse_production_blockers().is_empty() {
                return Err(failed(
                    ApplyOutcomeCode::ConversionFailed,
                    "clickhouse_runtime_admission_blocker",
                ));
            }
            let verification = execute_or_verify_copy(spec, permit, false).await?;
            let copy_sha = legacy_copy_verification_sha256(&verification).map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "postgres_report_hash_failed",
                )
            })?;
            if permit.checkpoint_proof_sha256() != Some(copy_sha.as_str()) {
                return Err(failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "postgres_report_checkpoint_mismatch",
                ));
            }
            matrix_before!(ClickhouseProjectionCommit);
            let proof = project_legacy_clickhouse(spec, permit, &verification)
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ConversionFailed,
                        "clickhouse_exact_projection_failed",
                    )
                })?
                .stage_proof()
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ConversionFailed,
                        "clickhouse_projection_proof_invalid",
                    )
                })?;
            matrix_after_success!(ClickhouseProjectionCommit);
            Ok(proof)
        })
    }

    fn materialize_runtime<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let execution = execution(spec)?;
            let release = ReleaseArtifactSpec::new(
                &execution.release.release_id,
                &execution.release.archive_sha256,
            )
            .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "release_binding_invalid"))?;
            matrix_before!(ReleaseArtifactReconcile);
            tokio::task::block_in_place(|| verify_release_artifact_for_one_shot(spec)).map_err(
                |_| {
                    failed(
                        ApplyOutcomeCode::IoFailure,
                        "release_archive_or_tree_verification_failed",
                    )
                },
            )?;
            matrix_after_success!(ReleaseArtifactReconcile);
            let mut host = activation_executor(spec, permit)?;
            let release_proof = host
                .verify_release_artifact(&spec.operation_id, &release)
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::IoFailure,
                        "release_preflight_or_reconcile_failed",
                    )
                })?;
            let bundle = materialize_role_configs(spec).map_err(|_| {
                failed(
                    ApplyOutcomeCode::IoFailure,
                    "runtime_config_materialization_failed",
                )
            })?;
            matrix_before!(RuntimeConfigInstall);
            let receipt = host.install_role_configs_atomically(&bundle).map_err(|_| {
                failed(ApplyOutcomeCode::IoFailure, "runtime_config_install_failed")
            })?;
            matrix_after_success!(RuntimeConfigInstall);
            let verification = host.verify_role_configs(&bundle, &receipt).map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "runtime_config_verification_failed",
                )
            })?;
            // No seed file is ever written. The short-lived, typed candidate
            // derived from the manifest goes straight through the migration
            // principal into the encrypted append-only authority. On a crash
            // retry, the domain helper accepts only the exact existing value
            // and rejects mismatches or orphan rows.
            let operator_authority = ensure_initial_operator_authority(spec, permit).await?;
            proof_from_serializable(
                b"v2board-runtime-materialization-v1\0",
                &(release_proof, receipt, verification, operator_authority),
                ApplyOutcomeCode::VerificationMismatch,
                "runtime_materialization_proof_failed",
            )
        })
    }

    fn verify_nodes_offline<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            let nodes =
                NativeNodeCutover::new(spec, &pool, permit.installation_id()).map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ActivationFailed,
                        "node_cutover_binding_invalid",
                    )
                })?;
            let report = nodes.verify_empty_before_authority().await.map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "node_offline_verification_failed",
                )
            })?;
            VerifiedStageProof::new(report.report_sha256().to_string()).map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "node_cutover_report_invalid",
                )
            })
        })
    }

    fn collect_activation_evidence<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<ActivationEvidence, StageFailure>> {
        Box::pin(async move {
            let history = verified_history(spec, permit.inspect_review_sha256())?;
            let current = history.last().ok_or_else(|| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "activation_journal_missing",
                )
            })?;
            let data = checkpoint_proof(&history, ApplyCheckpoint::PostgresValueVerified)?;
            let analytics = checkpoint_proof(&history, ApplyCheckpoint::ClickhouseProjected)?;
            let nodes = checkpoint_proof(&history, ApplyCheckpoint::NodesVerified)?;
            if permit.checkpoint_proof_sha256() != Some(nodes.as_str()) {
                return Err(failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "activation_journal_binding_mismatch",
                ));
            }

            let postgres_verification = execute_or_verify_copy(spec, permit, false).await?;
            let observed_data =
                legacy_copy_verification_sha256(&postgres_verification).map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "pre_authority_postgres_report_hash_failed",
                    )
                })?;
            if observed_data != data {
                return Err(failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "pre_authority_postgres_values_changed",
                ));
            }
            verify_legacy_clickhouse_projection_read_only(
                spec,
                permit.installation_id(),
                &analytics,
            )
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "pre_authority_clickhouse_projection_changed",
                )
            })?;
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            let node_cutover = NativeNodeCutover::new(spec, &pool, permit.installation_id())
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ActivationFailed,
                        "pre_authority_node_binding_invalid",
                    )
                })?;
            let observed_nodes =
                node_cutover
                    .verify_empty_before_authority()
                    .await
                    .map_err(|_| {
                        failed(
                            ApplyOutcomeCode::ActivationFailed,
                            "pre_authority_node_offline_reverification_failed",
                        )
                    })?;
            if observed_nodes.report_sha256() != nodes {
                return Err(failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "pre_authority_node_report_changed",
                ));
            }
            let source_drained = source_drained_snapshot(&history)?;
            let traffic = load_verified_frozen_traffic(spec, source_drained)
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "pre_authority_archived_traffic_verification_failed",
                    )
                })?;
            require_archived_source_redis_authorized(
                self.authorized_redis.as_ref(),
                traffic.receipt(),
            )?;
            let current_targets =
                inspect_target_bundle(legacy_target(spec)?)
                    .await
                    .map_err(|_| {
                        failed(
                            ApplyOutcomeCode::TargetDrift,
                            "pre_authority_target_identity_probe_failed",
                        )
                    })?;
            require_authorized_target_redis(
                self.authorized_redis.as_ref(),
                &current_targets.redis,
            )?;
            let materialization = ensure_verified_archive_materialization(
                spec,
                ArchiveMaterializationAnchor::Target(permit),
            )
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::BackupInvalid,
                    "pre_authority_archive_materialization_failed",
                )
            })?;
            let mut source = BareMetalLegacySource::from_manifest(spec)
                .map_err(|_| failed(ApplyOutcomeCode::FenceUncertain, "source_policy_invalid"))?;
            source
                .final_recheck(
                    spec,
                    permit.inspect_review_sha256(),
                    current,
                    permit.final_recheck_report_sha256(),
                    traffic.receipt(),
                    materialization.database_url(),
                )
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::FenceUncertain,
                        "source_fence_final_observation_failed",
                    )
                })?;
            tokio::task::block_in_place(|| verify_native_units_stopped_before_authority(spec))
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ActivationFailed,
                        "native_units_not_stopped_before_authority",
                    )
                })?;
            Ok(ActivationEvidence {
                data_verification_report_sha256: data,
                analytics_projection_report_sha256: analytics,
                node_cutover_report_sha256: nodes,
                old_writers_fenced: true,
                new_writers_still_stopped: true,
                postgres_is_transaction_authority: true,
                clickhouse_is_rebuildable_projection: true,
            })
        })
    }

    fn commit_native_authority_once<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
        evidence: &'a ActivationEvidence,
    ) -> ApplyFuture<'a, Result<NativeAuthorityBinding, StageFailure>> {
        Box::pin(async move {
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            let history = verified_history(spec, permit.inspect_review_sha256())?;
            let current = history.last().ok_or_else(|| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_journal_missing",
                )
            })?;
            let authorization_audit = self.authorization_audit.ledger_binding().map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_authorization_audit_invalid",
                )
            })?;
            if let Some(commit) =
                observe_native_activation_commit(&pool, spec, current, &authorization_audit)
                    .await
                    .map_err(|_| {
                        failed(
                            ApplyOutcomeCode::ActivationFailed,
                            "authority_commit_observation_failed",
                        )
                    })?
            {
                return Ok(commit.native_authority_binding().clone());
            }
            let proof = NativeActivationProofBinding::new(
                &evidence.data_verification_report_sha256,
                &evidence.analytics_projection_report_sha256,
                &evidence.node_cutover_report_sha256,
            )
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_proof_binding_invalid",
                )
            })?;
            matrix_before!(NativeAuthorityCommit);
            let commit = commit_native_activation(
                &pool,
                spec,
                permit,
                current,
                &proof,
                &authorization_audit,
            )
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_commit_failed_or_ambiguous",
                )
            })?;
            matrix_after_success!(NativeAuthorityCommit);
            Ok(commit.native_authority_binding().clone())
        })
    }

    fn start_native_services_once<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableNativeStartPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            matrix_before!(ArchiveMaterializationDestroy);
            destroy_verified_archive_materialization_after_authority(spec, permit)
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ActivationFailed,
                        "archive_materialization_cleanup_failed",
                    )
                })?;
            matrix_after_success!(ArchiveMaterializationDestroy);
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            let authority = permit.native_authority_binding();
            let authorization_audit = self.authorization_audit.ledger_binding().map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "native_start_authorization_audit_invalid",
                )
            })?;
            let authority_exact: bool = sqlx::query_scalar(
                "SELECT COUNT(*) = 1 FROM v2_lifecycle_activation_commit c \
                 JOIN v2_system_installation i ON i.installation_id = c.installation_id AND i.singleton = 1 \
                 JOIN v2_lifecycle_operation o ON o.operation_id = c.operation_id AND o.installation_id = c.installation_id \
                 WHERE c.operation_id::text = $1 AND c.installation_id::text = $2 \
                   AND c.journal_generation = $3 AND c.journal_event_sha256 = $4 \
                   AND c.data_verification_report_sha256 = $5 \
                   AND c.analytics_projection_report_sha256 = $6 \
                   AND c.node_cutover_report_sha256 = $7 \
                   AND o.authorized_snapshot_report_sha256 = $8 \
                   AND o.authorized_snapshot_report_binding_hmac_sha256 = $9 \
                   AND o.authorization_binding_hmac_sha256 = $10 \
                   AND o.authorization_file_sha256 = $11 \
                   AND i.lineage = 'native' AND i.state = 'active' AND i.activated_at = c.committed_at",
            )
            .bind(permit.operation_id())
            .bind(permit.installation_id())
            .bind(i64::try_from(authority.nodes_verified_generation()).map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_generation_invalid",
                )
            })?)
            .bind(authority.nodes_verified_event_sha256())
            .bind(authority.data_verification_report_sha256())
            .bind(authority.analytics_projection_report_sha256())
            .bind(authority.node_cutover_report_sha256())
            .bind(authorization_audit.authorized_snapshot_report_sha256())
            .bind(authorization_audit.authorized_snapshot_report_binding_hmac_sha256())
            .bind(authorization_audit.authorization_binding_hmac_sha256())
            .bind(authorization_audit.authorization_file_sha256())
            .fetch_one(&pool)
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_ledger_read_failed",
                )
            })?;
            if !authority_exact {
                return Err(failed(
                    ApplyOutcomeCode::ActivationFailed,
                    "authority_ledger_binding_mismatch",
                ));
            }
            matrix_before!(NativeServicesStart);
            let services =
                tokio::task::block_in_place(|| start_native_units_after_authority(spec, permit))
                    .map_err(|_| {
                        failed(
                            ApplyOutcomeCode::ActivationFailed,
                            "native_service_start_or_readiness_failed",
                        )
                    })?;
            matrix_after_success!(NativeServicesStart);
            let nodes =
                NativeNodeCutover::new(spec, &pool, permit.installation_id()).map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ActivationFailed,
                        "node_cutover_binding_invalid",
                    )
                })?;
            matrix_before!(NodeActivationCommit);
            let activated = nodes
                .complete_empty_inventory_after_native_authority(
                    authority.node_cutover_report_sha256(),
                    permit,
                    &services.api,
                )
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::ActivationFailed,
                        "post_authority_node_activation_failed",
                    )
                })?;
            matrix_after_success!(NodeActivationCommit);
            proof_from_serializable(
                b"v2board-native-service-and-node-start-v1\0",
                &(
                    services,
                    activated.report_sha256(),
                    activated.activation_request_id(),
                    activated.activated_node_set_sha256(),
                    activated.node_count(),
                ),
                ApplyOutcomeCode::ActivationFailed,
                "native_start_report_hash_failed",
            )
        })
    }

    fn retire_source<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a DurableMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        Box::pin(async move {
            let mut source = BareMetalLegacySource::from_manifest(spec)
                .map_err(|_| failed(ApplyOutcomeCode::RetirementFailed, "source_policy_invalid"))?;
            matrix_before!(SourceRetirementCommit);
            let proof = source.retire_local_source(spec, permit)?;
            matrix_after_success!(SourceRetirementCommit);
            Ok(proof)
        })
    }

    fn collect_completion_evidence<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        permit: &'a CompletionRecoveryPermit,
    ) -> ApplyFuture<'a, Result<CompletionEvidence, StageFailure>> {
        Box::pin(async move {
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            let ledger_completed: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM v2_lifecycle_operation WHERE operation_id::text = $1 AND state = 'completed' AND checkpoint = 15)",
            )
            .bind(permit.operation_id())
            .fetch_one(&pool)
            .await
            .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "completion_ledger_probe_failed"))?;
            let archive = if ledger_completed {
                verify_persisted_backup_archive_after_ledger_completion(spec)
            } else {
                verify_persisted_backup_archive(spec)
            }
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::BackupInvalid,
                    "verified_backup_archive_unavailable",
                )
            })?;
            let native_runtime_report_sha256 =
                tokio::task::block_in_place(|| verify_native_runtime_after_cutover(spec)).map_err(
                    |_| {
                        failed(
                            ApplyOutcomeCode::RetirementFailed,
                            "native_runtime_or_compatibility_verification_failed",
                        )
                    },
                )?;
            let observation = tokio::task::block_in_place(|| {
                verify_source_retirement_for_completion(spec, permit)
            })
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::RetirementFailed,
                    "source_retirement_completion_proof_invalid",
                )
            })?;
            let authority = permit.native_authority_binding();
            Ok(CompletionEvidence {
                data_verification_report_sha256: authority
                    .data_verification_report_sha256()
                    .to_string(),
                analytics_projection_report_sha256: authority
                    .analytics_projection_report_sha256()
                    .to_string(),
                node_cutover_report_sha256: authority.node_cutover_report_sha256().to_string(),
                old_writers_fenced: true,
                postgres_is_transaction_authority: true,
                clickhouse_is_rebuildable_projection: true,
                native_runtime_report_sha256,
                native_runtime_running_and_verified: true,
                verified_backup_reference: archive.backup_reference().to_string(),
                verified_backup_receipt_sha256: archive.receipt_sha256().to_string(),
                verified_backup_artifact_sha256: archive.encrypted_backup_sha256().to_string(),
                source_retired: observation.source_access_permanently_disabled,
                mysql_reachable: observation.mysql_reachable_with_old_credentials,
                source_redis_reachable: observation
                    .source_default_redis_reachable_with_old_credentials
                    || observation.source_cache_redis_reachable_with_old_credentials,
                source_access_permanently_disabled: observation.source_access_permanently_disabled,
                legacy_runtime_compat: false,
            })
        })
    }

    fn complete_permanent_ledger<'a>(
        &'a mut self,
        spec: &'a ProvisionSpec,
        completed: &'a ApplyJournalSnapshot,
        evidence: &'a CompletionEvidence,
    ) -> ApplyFuture<'a, Result<(), StageFailure>> {
        Box::pin(async move {
            let authorization_path = &execution(spec)?.journal.authorization_path;
            let (authorization, authorization_file_sha256) =
                ApplyAuthorization::load_with_file_sha256(authorization_path).map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "completion_authorization_reload_failed",
                    )
                })?;
            authorization.verify_resume_binding(spec).map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "completion_authorization_binding_invalid",
                )
            })?;
            let reloaded_authorization_audit = AuthorizationAuditIdentity::from_authorization(
                &authorization,
                &authorization_file_sha256,
            );
            if reloaded_authorization_audit != self.authorization_audit {
                return Err(failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "completion_authorization_identity_changed",
                ));
            }
            let authorization_audit =
                reloaded_authorization_audit.ledger_binding().map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "completion_authorization_audit_invalid",
                    )
                })?;
            let target = legacy_target(spec)?;
            let pool = PgPool::connect(&target.postgres.migration_database_url)
                .await
                .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
            let proof = CompletionProofBinding::new(
                &evidence.data_verification_report_sha256,
                &evidence.analytics_projection_report_sha256,
                &evidence.node_cutover_report_sha256,
                &evidence.native_runtime_report_sha256,
                evidence.native_runtime_running_and_verified,
                &evidence.verified_backup_reference,
                &evidence.verified_backup_artifact_sha256,
            )
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "completion_proof_binding_invalid",
                )
            })?;
            // The permanent database fact must precede destruction of either
            // disposable recovery input.
            matrix_before!(LifecycleCompletionCommit);
            complete_lifecycle_ledger(&pool, spec, completed, &proof, &authorization_audit)
                .await
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::IoFailure,
                        "permanent_completion_ledger_failed",
                    )
                })?;
            matrix_after_success!(LifecycleCompletionCommit);
            matrix_before!(RuntimeDecryptionIdentityDestroy);
            cleanup_runtime_decryption_identity_after_completion(spec, completed).map_err(
                |_| {
                    failed(
                        ApplyOutcomeCode::IoFailure,
                        "runtime_age_identity_cleanup_failed",
                    )
                },
            )?;
            matrix_after_success!(RuntimeDecryptionIdentityDestroy);
            Ok(())
        })
    }
}

impl ProductionLegacyApplyExecutor {
    async fn redis_binding(
        &self,
        spec: &ProvisionSpec,
        permit: &DurableTargetMutationPermit,
    ) -> Result<TargetRedisInspectionBinding, StageFailure> {
        let authorized = self.authorized_redis.as_ref().ok_or_else(|| {
            failed(
                ApplyOutcomeCode::TargetDrift,
                "authorized_redis_identity_missing",
            )
        })?;
        let observed = inspect_target_bundle(legacy_target(spec)?)
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::TargetDrift,
                    "target_redis_identity_probe_failed",
                )
            })?;
        require_authorized_target_redis(Some(authorized), &observed.redis)?;
        TargetRedisInspectionBinding::from_recovery_observation(
            permit.inspect_review_sha256(),
            &authorized.target_run_id,
            vec![
                authorized.source_default_run_id.clone(),
                authorized.source_cache_run_id.clone(),
            ],
        )
        .map_err(|_| {
            failed(
                ApplyOutcomeCode::TargetDrift,
                "recovery_redis_binding_invalid",
            )
        })
    }
}

fn activation_executor(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
) -> Result<
    BareMetalActivationExecutor<ProcessCommandRunner, DenyNativeAuthorityCommitter>,
    StageFailure,
> {
    let execution = execution(spec)?;
    let release = ReceiptBinding::new(
        execution.receipts.release_archive.path.clone(),
        execution.receipts.release_archive.sha256.clone(),
    )
    .map_err(|_| {
        failed(
            ApplyOutcomeCode::IoFailure,
            "release_receipt_binding_invalid",
        )
    })?;
    let policy = NativeActivationPolicy::for_legacy_apply_target(release)
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "activation_policy_invalid"))?;
    let observer = BareMetalRetirementObserver::from_manifest(spec)
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "retirement_observer_invalid"))?;
    BareMetalActivationExecutor::new(
        ProcessCommandRunner,
        DenyNativeAuthorityCommitter,
        Box::new(observer),
        permit.clone(),
        policy,
    )
    .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "activation_executor_invalid"))
}

async fn execute_or_verify_copy(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    execute: bool,
) -> Result<LegacyCopyVerification, StageFailure> {
    let (_source, target) = legacy_source_and_target(spec)?;
    let history = verified_history(spec, permit.inspect_review_sha256())?;
    let source_drained = source_drained_snapshot(&history)?;
    let traffic = load_verified_frozen_traffic(spec, source_drained)
        .await
        .map_err(|_| {
            failed(
                ApplyOutcomeCode::VerificationMismatch,
                "frozen_traffic_verification_failed",
            )
        })?;
    let materialization =
        ensure_verified_archive_materialization(spec, ArchiveMaterializationAnchor::Target(permit))
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::BackupInvalid,
                    "archive_materialization_reconcile_failed",
                )
            })?;
    if materialization.source_fingerprint_sha256() != permit.source_fingerprint_sha256()
        || materialization.source_schema_sha256() != LEGACY_SEMANTIC_SCHEMA_SHA256
    {
        return Err(failed(
            ApplyOutcomeCode::VerificationMismatch,
            "archive_materialization_binding_mismatch",
        ));
    }
    let source_pool = MySqlPool::connect(materialization.database_url())
        .await
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "source_mysql_connect_failed"))?;
    let target_pool = PgPool::connect(&target.postgres.migration_database_url)
        .await
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "postgres_connect_failed"))?;
    let binding = ConversionRunBinding {
        operation_id: permit.operation_id().to_string(),
        target_installation_id: permit.installation_id().to_string(),
        source_snapshot_sha256: permit.source_fingerprint_sha256().to_string(),
        source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
        registry_sha256: registry_sha256().map_err(|_| {
            failed(
                ApplyOutcomeCode::VerificationMismatch,
                "converter_registry_invalid",
            )
        })?,
    };
    let mut adapter =
        LegacyCopyAdapter::new(&source_pool, &target_pool, binding, DEFAULT_BATCH_SIZE)
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::VerificationMismatch,
                    "converter_snapshot_open_failed",
                )
            })?;
    let result = if execute {
        let mut sink =
            PostgresDurableCopyCheckpointSink::new(&target_pool, permit.backup_reference_sha256())
                .map_err(|_| {
                    failed(
                        ApplyOutcomeCode::VerificationMismatch,
                        "copy_checkpoint_binding_invalid",
                    )
                })?;
        adapter
            .execute(&mut sink, &traffic)
            .await
            .map(|(_, report)| report)
    } else {
        adapter.verify_completed(&traffic).await
    }
    .map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            if execute {
                "postgres_copy_or_exact_reconcile_failed"
            } else {
                "postgres_value_verification_failed"
            },
        )
    });
    let close = adapter.finish_source_snapshot().await.map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "source_snapshot_close_failed",
        )
    });
    match (result, close) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

async fn reconcile_mirrored_history(
    pool: &PgPool,
    spec: &ProvisionSpec,
    history: &[ApplyJournalSnapshot],
    authorization_audit: &AuthorizationAuditBinding,
) -> Result<(), StageFailure> {
    if history.is_empty() {
        return Err(failed(
            ApplyOutcomeCode::VerificationMismatch,
            "journal_history_empty",
        ));
    }
    let row = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
        "SELECT journal_generation, journal_event_sha256, \
                authorized_snapshot_report_sha256, \
                authorized_snapshot_report_binding_hmac_sha256, \
                authorization_binding_hmac_sha256, authorization_file_sha256 \
         FROM v2_lifecycle_operation WHERE operation_id::text = $1",
    )
    .bind(&spec.operation_id)
    .fetch_one(pool)
    .await
    .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "lifecycle_head_read_failed"))?;
    let generation = usize::try_from(row.0).map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "lifecycle_generation_invalid",
        )
    })?;
    let anchor = history.get(generation).ok_or_else(|| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "lifecycle_history_prefix_missing",
        )
    })?;
    if anchor.generation() != generation as u64
        || anchor.event_sha256() != row.1
        || row.2 != authorization_audit.authorized_snapshot_report_sha256()
        || row.3 != authorization_audit.authorized_snapshot_report_binding_hmac_sha256()
        || row.4 != authorization_audit.authorization_binding_hmac_sha256()
        || row.5 != authorization_audit.authorization_file_sha256()
    {
        return Err(failed(
            ApplyOutcomeCode::VerificationMismatch,
            "lifecycle_history_prefix_mismatch",
        ));
    }
    for snapshot in history.iter().skip(generation + 1) {
        mirror_lifecycle_snapshot(pool, spec, snapshot, authorization_audit)
            .await
            .map_err(|_| {
                failed(
                    ApplyOutcomeCode::IoFailure,
                    "lifecycle_history_mirror_failed",
                )
            })?;
    }
    Ok(())
}

fn verified_history(
    spec: &ProvisionSpec,
    inspect_review_sha256: &str,
) -> Result<Vec<ApplyJournalSnapshot>, StageFailure> {
    let execution = execution(spec)?;
    let binding = ApplyJournalBinding::new(&spec.operation_id, inspect_review_sha256)
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "journal_binding_invalid"))?;
    let (journal, _) = ApplyJournal::open(&execution.journal.root, binding)
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "journal_open_failed"))?;
    journal
        .verified_history()
        .map_err(|_| failed(ApplyOutcomeCode::IoFailure, "journal_history_invalid"))
}

fn source_drained_snapshot(
    history: &[ApplyJournalSnapshot],
) -> Result<&ApplyJournalSnapshot, StageFailure> {
    history
        .iter()
        .find(|snapshot| {
            snapshot.checkpoint() == ApplyCheckpoint::SourceDrained
                && snapshot.outcome_code().is_none()
                && snapshot.checkpoint_proof_sha256().is_some()
        })
        .ok_or_else(|| {
            failed(
                ApplyOutcomeCode::VerificationMismatch,
                "source_drained_anchor_missing",
            )
        })
}

fn checkpoint_proof(
    history: &[ApplyJournalSnapshot],
    checkpoint: ApplyCheckpoint,
) -> Result<String, StageFailure> {
    history
        .iter()
        .find(|snapshot| {
            snapshot.checkpoint() == checkpoint
                && snapshot.outcome_code().is_none()
                && snapshot.checkpoint_proof_sha256().is_some()
        })
        .and_then(|snapshot| snapshot.checkpoint_proof_sha256())
        .map(str::to_string)
        .ok_or_else(|| {
            failed(
                ApplyOutcomeCode::VerificationMismatch,
                "required_checkpoint_proof_missing",
            )
        })
}

fn stage_proof_for_copy(
    verification: &LegacyCopyVerification,
) -> Result<VerifiedStageProof, StageFailure> {
    let sha = legacy_copy_verification_sha256(verification).map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "postgres_report_hash_failed",
        )
    })?;
    VerifiedStageProof::new(sha).map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "postgres_report_proof_invalid",
        )
    })
}

fn proof_from_serializable(
    domain: &[u8],
    value: &impl Serialize,
    outcome: ApplyOutcomeCode,
    code: &'static str,
) -> Result<VerifiedStageProof, StageFailure> {
    let bytes = serde_json::to_vec(value).map_err(|_| failed(outcome, code))?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
    VerifiedStageProof::new(hex::encode(digest.finalize())).map_err(|_| failed(outcome, code))
}

async fn ensure_initial_operator_authority(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
) -> Result<InitialOperatorAuthorityProof, StageFailure> {
    let target = legacy_target(spec)?;
    let installation_id = uuid::Uuid::parse_str(permit.installation_id()).map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "operator_authority_installation_id_invalid",
        )
    })?;
    let pool = PgPool::connect(&target.postgres.migration_database_url)
        .await
        .map_err(|_| {
            failed(
                ApplyOutcomeCode::IoFailure,
                "operator_authority_postgres_connect_failed",
            )
        })?;
    let lifecycle_installation = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT installation_id FROM v2_system_installation WHERE singleton = 1",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|_| {
        failed(
            ApplyOutcomeCode::IoFailure,
            "operator_authority_installation_read_failed",
        )
    })?;
    if lifecycle_installation != Some(installation_id) {
        return Err(failed(
            ApplyOutcomeCode::TargetDrift,
            "operator_authority_installation_mismatch",
        ));
    }

    let candidate = spec.normalized_operator_config_candidate().map_err(|_| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "operator_authority_candidate_invalid",
        )
    })?;
    let actor = format!("lifecycle:{}", spec.operation_id);
    let snapshot = operator_config::ensure_initial_authority_exact(
        &pool,
        installation_id,
        spec.operator_app_key(),
        candidate.as_map(),
        &actor,
    )
    .await
    .map_err(|_| {
        failed(
            ApplyOutcomeCode::TargetDrift,
            "operator_authority_seed_or_exact_reconcile_failed",
        )
    })?;

    Ok(InitialOperatorAuthorityProof {
        operation_id: spec.operation_id.clone(),
        installation_id: permit.installation_id().to_string(),
        revision: snapshot.revision,
        revision_id: snapshot.revision_id.hyphenated().to_string(),
        format_version: snapshot.format_version,
        config_hmac_sha256: snapshot.config_hmac_sha256,
    })
}

fn require_targets_still_empty(
    authorized: Option<&AuthorizedRedisIdentity>,
    current: &ProvisionPlan,
) -> Result<(), StageFailure> {
    let postgres = current.target_postgres.as_ref().ok_or_else(target_drift)?;
    let clickhouse = current
        .target_clickhouse
        .as_ref()
        .ok_or_else(target_drift)?;
    let redis = current.target_redis.as_ref().ok_or_else(target_drift)?;
    if !postgres.database_absent
        || !postgres.roles_absent
        || !clickhouse.database_absent
        || !clickhouse.principals_absent
        || redis.key_count != 0
    {
        return Err(target_drift());
    }
    require_authorized_target_redis(authorized, redis)?;
    let source = current.source_redis.as_ref().ok_or_else(target_drift)?;
    let authorized = authorized.ok_or_else(target_drift)?;
    if source.source_default_run_id != authorized.source_default_run_id
        || source.source_cache_run_id != authorized.source_cache_run_id
    {
        return Err(target_drift());
    }
    Ok(())
}

fn require_target_bundle_still_empty(
    authorized: Option<&AuthorizedRedisIdentity>,
    current: &TargetBundle,
) -> Result<(), StageFailure> {
    if !current.postgres.database_absent
        || !current.postgres.roles_absent
        || !current.clickhouse.database_absent
        || !current.clickhouse.principals_absent
        || current.redis.key_count != 0
    {
        return Err(target_drift());
    }
    require_authorized_target_redis(authorized, &current.redis)
}

fn require_authorized_target_redis(
    authorized: Option<&AuthorizedRedisIdentity>,
    current: &TargetRedisInspection,
) -> Result<(), StageFailure> {
    let authorized = authorized.ok_or_else(target_drift)?;
    if current.target_run_id != authorized.target_run_id
        || current.target_database_index != authorized.target_database_index
    {
        return Err(target_drift());
    }
    Ok(())
}

fn require_archived_source_redis_authorized(
    authorized: Option<&AuthorizedRedisIdentity>,
    receipt: &VerifiedFrozenTrafficReceipt,
) -> Result<(), StageFailure> {
    let authorized = authorized.ok_or_else(target_drift)?;
    if receipt.source_default_run_id != authorized.source_default_run_id {
        return Err(failed(
            ApplyOutcomeCode::SourceDrift,
            "archived_source_redis_identity_mismatch",
        ));
    }
    Ok(())
}

fn execution(spec: &ProvisionSpec) -> Result<&crate::manifest::LegacyExecutionSpec, StageFailure> {
    spec.legacy_apply_execution().ok_or_else(|| {
        failed(
            ApplyOutcomeCode::VerificationMismatch,
            "schema_v4_execution_missing",
        )
    })
}

fn legacy_target(spec: &ProvisionSpec) -> Result<&TargetSpec, StageFailure> {
    match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { target, .. } => Ok(target),
        _ => Err(failed(
            ApplyOutcomeCode::VerificationMismatch,
            "legacy_target_missing",
        )),
    }
}

fn legacy_source_and_target(
    spec: &ProvisionSpec,
) -> Result<(&SourceSpec, &TargetSpec), StageFailure> {
    match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, target, .. } => Ok((source, target)),
        _ => Err(failed(
            ApplyOutcomeCode::VerificationMismatch,
            "legacy_flow_missing",
        )),
    }
}

fn target_drift() -> StageFailure {
    failed(
        ApplyOutcomeCode::TargetDrift,
        "fenced_target_no_longer_empty",
    )
}

#[cfg(feature = "bare-metal-fault-matrix")]
async fn matrix_before_impl(point: BareMetalFaultPoint) -> Result<(), StageFailure> {
    crate::bare_metal_fault_matrix::before(point)
        .await
        .map_err(|error| {
            StageFailure::sanitized(ApplyOutcomeCode::ProcessInterrupted, error.sanitized_code())
        })
}

#[cfg(feature = "bare-metal-fault-matrix")]
async fn matrix_after_success_impl(point: BareMetalFaultPoint) -> Result<(), StageFailure> {
    crate::bare_metal_fault_matrix::after_success(point)
        .await
        .map_err(|error| {
            StageFailure::sanitized(ApplyOutcomeCode::ProcessInterrupted, error.sanitized_code())
        })
}

fn failed(outcome: ApplyOutcomeCode, code: &'static str) -> StageFailure {
    StageFailure::sanitized(outcome, code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_capability_names_its_unavailable_reason() {
        const { assert!(!PRODUCTION_LEGACY_APPLY_CAPABILITY.is_available()) };
        assert_eq!(
            PRODUCTION_LEGACY_APPLY_CAPABILITY.blocker(),
            Some(ProductionLegacyApplyBlocker::AwaitingBareMetalFaultMatrixAndSafetyAudit)
        );
    }

    #[test]
    fn stage_failures_remain_sanitized() {
        let failure = failed(
            ApplyOutcomeCode::ActivationFailed,
            "post_authority_native_starter_not_implemented",
        );
        assert_eq!(
            failure.code(),
            "post_authority_native_starter_not_implemented"
        );
    }
}
