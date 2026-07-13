//! Systematic crash/retry matrix for the external mutations orchestrated by
//! `run_legacy_apply`.
//!
//! This is deliberately a pure orchestration harness. It proves that the
//! journal never authorizes a later mutation early, that a reconstructed
//! executor reconciles an already-applied mutation after either an ambiguous
//! acknowledgement or abrupt process loss, and that completion recovery does
//! not need a live legacy datastore after source retirement. Real database,
//! Redis, systemd, filesystem, and network fault tests remain separate
//! production-gate requirements.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tokio::sync::Notify;

use super::*;

/// Closed, reviewable list of executor calls that may durably mutate an
/// external system. Read-only verification calls are intentionally absent.
const DESTRUCTIVE_STAGES: &[LegacyApplyStage] = &[
    LegacyApplyStage::FenceSource,
    LegacyApplyStage::DrainSource,
    LegacyApplyStage::BackupRestore,
    LegacyApplyStage::FinalRecheck,
    LegacyApplyStage::BootstrapTargets,
    LegacyApplyStage::MirrorJournal,
    LegacyApplyStage::CopyPostgres,
    LegacyApplyStage::VerifyPostgres,
    LegacyApplyStage::ProjectClickhouse,
    LegacyApplyStage::MaterializeRuntime,
    LegacyApplyStage::VerifyNodes,
    LegacyApplyStage::CollectActivationEvidence,
    LegacyApplyStage::CommitNativeAuthority,
    LegacyApplyStage::StartNativeServices,
    LegacyApplyStage::RetireSource,
    LegacyApplyStage::CompleteLedger,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaultMode {
    Before,
    LostAcknowledgement,
    ProcessTerminated,
}

const FAULT_MODES: &[FaultMode] = &[
    FaultMode::Before,
    FaultMode::LostAcknowledgement,
    FaultMode::ProcessTerminated,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaultAction {
    Succeed,
    FailBefore,
    FailAfter,
    HangAfter,
}

impl FaultAction {
    const fn side_effect_happened(self) -> bool {
        matches!(self, Self::Succeed | Self::FailAfter | Self::HangAfter)
    }
}

#[derive(Debug)]
struct SimulatedWorld {
    fault_stage: LegacyApplyStage,
    fault_mode: FaultMode,
    fault_consumed: bool,
    calls: BTreeMap<&'static str, u32>,
    committed_effects: BTreeMap<&'static str, u32>,
    archive_committed: bool,
    mysql_reachable: bool,
    redis_reachable: bool,
    native_authority: Option<NativeAuthorityBinding>,
    native_started: bool,
    source_retired: bool,
    permanent_ledger_completed: bool,
    maximum_mirrored_generation: u64,
    drop_legacy_immediately_after_archive: bool,
}

impl SimulatedWorld {
    fn new(fault_stage: LegacyApplyStage, fault_mode: FaultMode) -> Self {
        Self {
            fault_stage,
            fault_mode,
            fault_consumed: false,
            calls: BTreeMap::new(),
            committed_effects: BTreeMap::new(),
            archive_committed: false,
            mysql_reachable: true,
            redis_reachable: true,
            native_authority: None,
            native_started: false,
            source_retired: false,
            permanent_ledger_completed: false,
            maximum_mirrored_generation: 0,
            drop_legacy_immediately_after_archive: fault_stage == LegacyApplyStage::FinalRecheck
                && fault_mode == FaultMode::ProcessTerminated,
        }
    }

    fn call_count(&self, stage: LegacyApplyStage) -> u32 {
        self.calls.get(stage_name(stage)).copied().unwrap_or(0)
    }

    fn effect_count(&self, stage: LegacyApplyStage) -> u32 {
        self.committed_effects
            .get(stage_name(stage))
            .copied()
            .unwrap_or(0)
    }

    fn record_call(&mut self, stage: LegacyApplyStage) -> FaultAction {
        *self.calls.entry(stage_name(stage)).or_default() += 1;
        let action = if !self.fault_consumed && self.fault_stage == stage {
            self.fault_consumed = true;
            match self.fault_mode {
                FaultMode::Before => FaultAction::FailBefore,
                FaultMode::LostAcknowledgement => FaultAction::FailAfter,
                FaultMode::ProcessTerminated => FaultAction::HangAfter,
            }
        } else {
            FaultAction::Succeed
        };
        if action.side_effect_happened() {
            // A retry may reconcile the same desired state, but it must not
            // create a second logical mutation.
            self.committed_effects.entry(stage_name(stage)).or_insert(1);
            match stage {
                LegacyApplyStage::BackupRestore => {
                    self.archive_committed = true;
                    if self.drop_legacy_immediately_after_archive {
                        self.mysql_reachable = false;
                        self.redis_reachable = false;
                    }
                }
                LegacyApplyStage::StartNativeServices => self.native_started = true,
                LegacyApplyStage::RetireSource => {
                    assert!(self.archive_committed, "retirement requires the archive");
                    assert!(self.native_started, "retirement requires native cutover");
                    self.mysql_reachable = false;
                    self.redis_reachable = false;
                    self.source_retired = true;
                }
                LegacyApplyStage::CompleteLedger => self.permanent_ledger_completed = true,
                _ => {}
            }
        }
        action
    }
}

#[derive(Clone)]
struct FaultExecutor {
    world: Arc<Mutex<SimulatedWorld>>,
    terminated_after_effect: Arc<Notify>,
}

impl FaultExecutor {
    fn action(&self, stage: LegacyApplyStage) -> FaultAction {
        let action = self.world.lock().expect("world lock").record_call(stage);
        if action == FaultAction::HangAfter {
            self.terminated_after_effect.notify_one();
        }
        action
    }

    fn proof(
        &self,
        stage: LegacyApplyStage,
        byte: char,
    ) -> ApplyFuture<'static, Result<VerifiedStageProof, StageFailure>> {
        let action = self.action(stage);
        Box::pin(resolve_action(
            action,
            VerifiedStageProof::new(byte.to_string().repeat(64)).expect("valid proof"),
        ))
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

async fn resolve_action<T>(action: FaultAction, success: T) -> Result<T, StageFailure> {
    match action {
        FaultAction::Succeed => Ok(success),
        FaultAction::FailBefore | FaultAction::FailAfter => Err(StageFailure::sanitized(
            ApplyOutcomeCode::ProcessInterrupted,
            "injected_process_interruption",
        )),
        FaultAction::HangAfter => std::future::pending().await,
    }
}

impl LegacyApplyExecutor for FaultExecutor {
    fn fence_source<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::FenceSource, '1')
    }

    fn drain_source<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::DrainSource, '2')
    }

    fn backup_and_restore_test<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<BackupRestoreProof, StageFailure>> {
        let action = self.action(LegacyApplyStage::BackupRestore);
        Box::pin(resolve_action(
            action,
            BackupRestoreProof::new("3".repeat(64), "backup:test/snapshot-1")
                .expect("valid backup proof"),
        ))
    }

    fn final_recheck<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _reviewed_inspect_review_sha256: &'a str,
        _head: &'a ApplyJournalSnapshot,
    ) -> ApplyFuture<'a, Result<FinalRecheckProof, StageFailure>> {
        let world = self.world.lock().expect("world lock");
        assert!(world.archive_committed);
        drop(world);
        let action = self.action(LegacyApplyStage::FinalRecheck);
        Box::pin(resolve_action(
            action,
            FinalRecheckProof::new("4".repeat(64), "5".repeat(64), true, true)
                .expect("valid final proof"),
        ))
    }

    fn bootstrap_targets<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
        _history: &'a [ApplyJournalSnapshot],
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::BootstrapTargets, '6')
    }

    fn mirror_journal_history<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        history: &'a [ApplyJournalSnapshot],
    ) -> ApplyFuture<'a, Result<(), StageFailure>> {
        let action = self.action(LegacyApplyStage::MirrorJournal);
        if action.side_effect_happened() {
            let generation = history.last().expect("nonempty history").generation();
            let mut world = self.world.lock().expect("world lock");
            world.maximum_mirrored_generation = world.maximum_mirrored_generation.max(generation);
        }
        Box::pin(resolve_action(action, ()))
    }

    fn copy_postgres<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::CopyPostgres, '7')
    }

    fn verify_postgres<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::VerifyPostgres, '8')
    }

    fn project_clickhouse<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::ProjectClickhouse, '9')
    }

    fn materialize_runtime<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::MaterializeRuntime, 'a')
    }

    fn verify_nodes_offline<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::VerifyNodes, 'b')
    }

    fn collect_activation_evidence<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableTargetMutationPermit,
    ) -> ApplyFuture<'a, Result<ActivationEvidence, StageFailure>> {
        let action = self.action(LegacyApplyStage::CollectActivationEvidence);
        Box::pin(resolve_action(action, Self::activation_evidence()))
    }

    fn commit_native_authority_once<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        permit: &'a DurableTargetMutationPermit,
        evidence: &'a ActivationEvidence,
    ) -> ApplyFuture<'a, Result<NativeAuthorityBinding, StageFailure>> {
        let action = self.action(LegacyApplyStage::CommitNativeAuthority);
        let candidate = NativeAuthorityBinding::new(
            permit.generation(),
            permit.event_sha256(),
            &evidence.data_verification_report_sha256,
            &evidence.analytics_projection_report_sha256,
            &evidence.node_cutover_report_sha256,
        )
        .expect("valid native authority");
        let authority = if action.side_effect_happened() {
            let mut world = self.world.lock().expect("world lock");
            world.native_authority.get_or_insert(candidate).clone()
        } else {
            candidate
        };
        Box::pin(resolve_action(action, authority))
    }

    fn start_native_services_once<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        permit: &'a DurableNativeStartPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        let world = self.world.lock().expect("world lock");
        assert_eq!(
            world.native_authority.as_ref(),
            Some(permit.native_authority_binding())
        );
        drop(world);
        self.proof(LegacyApplyStage::StartNativeServices, 'c')
    }

    fn retire_source<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a DurableMutationPermit,
    ) -> ApplyFuture<'a, Result<VerifiedStageProof, StageFailure>> {
        self.proof(LegacyApplyStage::RetireSource, 'd')
    }

    fn collect_completion_evidence<'a>(
        &'a mut self,
        _spec: &'a ProvisionSpec,
        _permit: &'a CompletionRecoveryPermit,
    ) -> ApplyFuture<'a, Result<CompletionEvidence, StageFailure>> {
        let world = self.world.lock().expect("world lock");
        assert!(world.archive_committed);
        assert!(world.source_retired);
        assert!(!world.mysql_reachable);
        assert!(!world.redis_reachable);
        drop(world);
        Box::pin(async {
            let activation = Self::activation_evidence();
            Ok(CompletionEvidence {
                data_verification_report_sha256: activation.data_verification_report_sha256,
                analytics_projection_report_sha256: activation.analytics_projection_report_sha256,
                node_cutover_report_sha256: activation.node_cutover_report_sha256,
                old_writers_fenced: true,
                postgres_is_transaction_authority: true,
                clickhouse_is_rebuildable_projection: true,
                native_runtime_report_sha256: "f".repeat(64),
                native_runtime_running_and_verified: true,
                verified_backup_reference: "backup:test/snapshot-1".to_string(),
                verified_backup_receipt_sha256: "3".repeat(64),
                verified_backup_artifact_sha256: "e".repeat(64),
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
        let action = self.action(LegacyApplyStage::CompleteLedger);
        Box::pin(resolve_action(action, ()))
    }
}

fn stage_name(stage: LegacyApplyStage) -> &'static str {
    match stage {
        LegacyApplyStage::FenceSource => "fence_source",
        LegacyApplyStage::DrainSource => "drain_source",
        LegacyApplyStage::BackupRestore => "backup_restore",
        LegacyApplyStage::FinalRecheck => "final_recheck",
        LegacyApplyStage::BootstrapTargets => "bootstrap_targets",
        LegacyApplyStage::MirrorJournal => "mirror_journal",
        LegacyApplyStage::CopyPostgres => "copy_postgres",
        LegacyApplyStage::VerifyPostgres => "verify_postgres",
        LegacyApplyStage::ProjectClickhouse => "project_clickhouse",
        LegacyApplyStage::MaterializeRuntime => "materialize_runtime",
        LegacyApplyStage::VerifyNodes => "verify_nodes",
        LegacyApplyStage::CollectActivationEvidence => "collect_activation_evidence",
        LegacyApplyStage::CommitNativeAuthority => "commit_native_authority",
        LegacyApplyStage::StartNativeServices => "start_native_services",
        LegacyApplyStage::RetireSource => "retire_source",
        LegacyApplyStage::CollectCompletionEvidence => "collect_completion_evidence",
        LegacyApplyStage::CompleteLedger => "complete_ledger",
    }
}

fn predecessor(stage: LegacyApplyStage) -> (ApplyCheckpoint, ApplyJournalState) {
    match stage {
        LegacyApplyStage::FenceSource => {
            (ApplyCheckpoint::PendingDurable, ApplyJournalState::Running)
        }
        LegacyApplyStage::DrainSource => (
            ApplyCheckpoint::MaintenanceFenced,
            ApplyJournalState::Running,
        ),
        LegacyApplyStage::BackupRestore => {
            (ApplyCheckpoint::SourceDrained, ApplyJournalState::Running)
        }
        LegacyApplyStage::FinalRecheck => (
            ApplyCheckpoint::BackupRestoreVerified,
            ApplyJournalState::Running,
        ),
        LegacyApplyStage::BootstrapTargets => (
            ApplyCheckpoint::InstallationIdentityReserved,
            ApplyJournalState::Running,
        ),
        LegacyApplyStage::MirrorJournal | LegacyApplyStage::CopyPostgres => (
            ApplyCheckpoint::TargetsBootstrapped,
            ApplyJournalState::Running,
        ),
        LegacyApplyStage::VerifyPostgres => (
            ApplyCheckpoint::PostgresBulkCopied,
            ApplyJournalState::Verifying,
        ),
        LegacyApplyStage::ProjectClickhouse => (
            ApplyCheckpoint::PostgresValueVerified,
            ApplyJournalState::Verifying,
        ),
        LegacyApplyStage::MaterializeRuntime => (
            ApplyCheckpoint::ClickhouseProjected,
            ApplyJournalState::Verifying,
        ),
        LegacyApplyStage::VerifyNodes => (
            ApplyCheckpoint::RuntimeMaterialized,
            ApplyJournalState::Verifying,
        ),
        LegacyApplyStage::CollectActivationEvidence => {
            (ApplyCheckpoint::NodesVerified, ApplyJournalState::Verifying)
        }
        LegacyApplyStage::CommitNativeAuthority => {
            (ApplyCheckpoint::NodesVerified, ApplyJournalState::Verifying)
        }
        LegacyApplyStage::StartNativeServices => (
            ApplyCheckpoint::NativeAuthorityCommitted,
            ApplyJournalState::Verifying,
        ),
        LegacyApplyStage::RetireSource => (
            ApplyCheckpoint::CutoverCommitted,
            ApplyJournalState::Verifying,
        ),
        LegacyApplyStage::CompleteLedger => (
            ApplyCheckpoint::CompletionVerified,
            ApplyJournalState::Completed,
        ),
        LegacyApplyStage::CollectCompletionEvidence => {
            panic!("read-only stage is not in the destructive matrix")
        }
    }
}

fn private_test_root(stage: LegacyApplyStage, mode: FaultMode) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "v2board-legacy-fault-matrix-{}-{stage:?}-{mode:?}",
        std::process::id()
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

fn authorization(spec: &ProvisionSpec) -> ApplyAuthorization {
    ApplyAuthorization {
        authorization_version: 3,
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        inspect_review_sha256: "0".repeat(64),
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
    }
}

fn open_journal(
    root: &Path,
    spec: &ProvisionSpec,
    authorization: &ApplyAuthorization,
) -> (ApplyJournal, ApplyJournalSnapshot) {
    let binding =
        ApplyJournalBinding::new(&spec.operation_id, &authorization.inspect_review_sha256)
            .expect("journal binding");
    ApplyJournal::open(root, binding).expect("open journal after reconstructed process")
}

async fn execute_attempt(
    root: PathBuf,
    world: Arc<Mutex<SimulatedWorld>>,
    terminated_after_effect: Arc<Notify>,
) -> Result<LegacyApplyResult, LegacyApplyError> {
    let spec = crate::manifest::tests::legacy_spec_for_orchestration();
    let authorization = authorization(&spec);
    let (journal, head) = open_journal(&root, &spec, &authorization);
    let mut executor = FaultExecutor {
        world,
        terminated_after_effect,
    };
    run_legacy_apply(&spec, &authorization, &journal, head, &mut executor).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_external_mutation_recovers_before_after_ack_and_after_process_loss() {
    for &stage in DESTRUCTIVE_STAGES {
        for &mode in FAULT_MODES {
            let root = private_test_root(stage, mode);
            let spec = crate::manifest::tests::legacy_spec_for_orchestration();
            let authorization = authorization(&spec);
            let binding =
                ApplyJournalBinding::new(&spec.operation_id, &authorization.inspect_review_sha256)
                    .expect("journal binding");
            ApplyJournal::create_pending(&root, binding).expect("durable pending journal");
            let world = Arc::new(Mutex::new(SimulatedWorld::new(stage, mode)));
            let terminated_after_effect = Arc::new(Notify::new());

            let attempt = tokio::spawn(execute_attempt(
                root.clone(),
                Arc::clone(&world),
                Arc::clone(&terminated_after_effect),
            ));
            match mode {
                FaultMode::Before | FaultMode::LostAcknowledgement => {
                    let error = attempt
                        .await
                        .expect("faulted process returned normally")
                        .expect_err("injected stage must stop the invocation");
                    assert!(matches!(
                        error,
                        LegacyApplyError::Stage { stage: observed, .. } if observed == stage
                    ));
                }
                FaultMode::ProcessTerminated => {
                    terminated_after_effect.notified().await;
                    attempt.abort();
                    assert!(
                        attempt
                            .await
                            .expect_err("task must be terminated")
                            .is_cancelled()
                    );
                }
            }

            let (journal, interrupted) = open_journal(&root, &spec, &authorization);
            let (expected_checkpoint, expected_phase) = predecessor(stage);
            assert_eq!(
                interrupted.checkpoint(),
                expected_checkpoint,
                "{stage:?}/{mode:?}"
            );
            match mode {
                FaultMode::Before | FaultMode::LostAcknowledgement
                    if stage != LegacyApplyStage::CompleteLedger =>
                {
                    assert_eq!(interrupted.state(), ApplyJournalState::NeedsRecovery);
                    assert!(interrupted.can_resume());
                }
                _ => {
                    assert_eq!(interrupted.state(), expected_phase);
                    assert_eq!(interrupted.outcome_code(), None);
                }
            }
            {
                let interrupted_world = world.lock().expect("world lock");
                assert_eq!(
                    interrupted_world.effect_count(stage),
                    if mode == FaultMode::Before { 0 } else { 1 },
                    "before/after-side-effect classification drifted for {stage:?}/{mode:?}"
                );
                if stage == LegacyApplyStage::RetireSource && mode == FaultMode::ProcessTerminated {
                    // This is the cold-archive recovery boundary: retirement
                    // has made both legacy credentials unreachable, but its
                    // checkpoint acknowledgement was never written. The next
                    // process must reconcile retirement and complete using
                    // only durable receipts/archive plus the native target.
                    assert!(interrupted_world.archive_committed);
                    assert!(!interrupted_world.mysql_reachable);
                    assert!(!interrupted_world.redis_reachable);
                }
                if stage == LegacyApplyStage::FinalRecheck && mode == FaultMode::ProcessTerminated {
                    // The archive was already complete when both legacy
                    // datastores disappeared. The process then died after
                    // rebuilding the archive materialization but before the
                    // FinalRecheck journal acknowledgement.
                    assert!(interrupted_world.archive_committed);
                    assert!(!interrupted_world.mysql_reachable);
                    assert!(!interrupted_world.redis_reachable);
                }
            }
            drop(journal);

            // A new executor instance and a freshly opened journal model the
            // next lifecycle process. NeedsRecovery is the public resume
            // command's only preparatory state transition.
            let (journal, interrupted) = open_journal(&root, &spec, &authorization);
            let resumed = if interrupted.state() == ApplyJournalState::NeedsRecovery {
                journal
                    .resume(&interrupted)
                    .expect("resume interrupted phase")
            } else {
                interrupted
            };
            let mut reconstructed = FaultExecutor {
                world: Arc::clone(&world),
                terminated_after_effect: Arc::new(Notify::new()),
            };
            let result =
                run_legacy_apply(&spec, &authorization, &journal, resumed, &mut reconstructed)
                    .await
                    .expect("reconstructed process reconciles and completes");
            assert!(result.completed);
            assert!(result.mysql_runtime_retired);
            let history = journal.verified_history().expect("valid final hash chain");
            assert_eq!(
                history.last().expect("completed head").state(),
                ApplyJournalState::Completed
            );

            let world = world.lock().expect("world lock");
            assert!(world.fault_consumed, "matrix fault was not reached");
            assert_eq!(world.effect_count(stage), 1, "logical mutation duplicated");
            if stage == LegacyApplyStage::MirrorJournal {
                assert!(
                    world.call_count(stage) >= 2,
                    "the recurring mirror mutation was not retried"
                );
            } else {
                assert_eq!(
                    world.call_count(stage),
                    2,
                    "faulted mutation was not reconciled exactly once"
                );
            }
            assert!(world.archive_committed);
            assert!(world.source_retired);
            assert!(!world.mysql_reachable);
            assert!(!world.redis_reachable);
            assert!(world.permanent_ledger_completed);
            assert!(world.maximum_mirrored_generation > 0);
            drop(world);
            fs::remove_dir_all(root).expect("remove fault-matrix test root");
        }
    }
}

#[test]
fn destructive_stage_classification_is_closed_over_the_public_stage_enum() {
    for stage in [
        LegacyApplyStage::FenceSource,
        LegacyApplyStage::DrainSource,
        LegacyApplyStage::BackupRestore,
        LegacyApplyStage::FinalRecheck,
        LegacyApplyStage::BootstrapTargets,
        LegacyApplyStage::MirrorJournal,
        LegacyApplyStage::CopyPostgres,
        LegacyApplyStage::VerifyPostgres,
        LegacyApplyStage::ProjectClickhouse,
        LegacyApplyStage::MaterializeRuntime,
        LegacyApplyStage::VerifyNodes,
        LegacyApplyStage::CollectActivationEvidence,
        LegacyApplyStage::CommitNativeAuthority,
        LegacyApplyStage::StartNativeServices,
        LegacyApplyStage::RetireSource,
        LegacyApplyStage::CollectCompletionEvidence,
        LegacyApplyStage::CompleteLedger,
    ] {
        let expected = stage != LegacyApplyStage::CollectCompletionEvidence;
        assert_eq!(DESTRUCTIVE_STAGES.contains(&stage), expected, "{stage:?}");
    }
}
