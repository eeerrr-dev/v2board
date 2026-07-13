use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const JOURNAL_VERSION: u32 = 3;
const EVENT_HASH_DOMAIN: &[u8] = b"v2board-one-shot-apply-event-v3\0";
const HEAD_HASH_DOMAIN: &[u8] = b"v2board-one-shot-apply-head-v3\0";
const NATIVE_AUTHORITY_PROOF_DOMAIN: &[u8] = b"v2board-native-authority-proof-v1\0";
const BACKUP_REFERENCE_HASH_DOMAIN: &[u8] = b"v2board-one-shot-backup-reference-v1\0";
const MAX_RECORD_BYTES: u64 = 64 * 1024;
const MAX_RECORDS: usize = 65_536;
const RECORD_NAME_DIGITS: usize = 20;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The immutable operator authorization to which every journal record is bound.
///
/// This deliberately contains no manifest fields, datastore URLs, credentials,
/// free-form notes, or error strings. The inspect report is referenced only by
/// its public SHA-256 digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApplyJournalBinding {
    operation_id: String,
    inspect_review_sha256: String,
}

impl ApplyJournalBinding {
    pub fn new(
        operation_id: impl AsRef<str>,
        inspect_review_sha256: impl AsRef<str>,
    ) -> Result<Self, ApplyJournalError> {
        let operation_id = Uuid::parse_str(operation_id.as_ref())
            .map_err(|_| ApplyJournalError::InvalidOperationId)?;
        if operation_id.is_nil() {
            return Err(ApplyJournalError::InvalidOperationId);
        }
        let inspect_review_sha256 = inspect_review_sha256.as_ref();
        if !is_lower_hex(inspect_review_sha256, 64) {
            return Err(ApplyJournalError::InvalidInspectReportHash);
        }
        Ok(Self {
            operation_id: operation_id.hyphenated().to_string(),
            inspect_review_sha256: inspect_review_sha256.to_string(),
        })
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn inspect_review_sha256(&self) -> &str {
        &self.inspect_review_sha256
    }
}

/// The durable operation state. `Failed` and `Completed` are terminal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyJournalState {
    Pending,
    Running,
    Verifying,
    NeedsRecovery,
    Failed,
    Completed,
}

/// Ordered one-shot migration checkpoints.
///
/// Checkpoints intentionally are a closed enum. Accepting arbitrary text here
/// would make it too easy to persist a URL, password, token, SQL row, or raw
/// error message in the F0 operation journal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyCheckpoint {
    PendingDurable,
    MaintenanceFenced,
    SourceDrained,
    BackupRestoreVerified,
    FinalRecheckPassed,
    InstallationIdentityReserved,
    TargetsBootstrapped,
    PostgresBulkCopied,
    PostgresValueVerified,
    ClickhouseProjected,
    RuntimeMaterialized,
    NodesVerified,
    NativeAuthorityCommitted,
    CutoverCommitted,
    SourceRetired,
    CompletionVerified,
}

impl ApplyCheckpoint {
    const fn successor(self) -> Option<Self> {
        Some(match self {
            Self::PendingDurable => Self::MaintenanceFenced,
            Self::MaintenanceFenced => Self::SourceDrained,
            Self::SourceDrained => Self::BackupRestoreVerified,
            Self::BackupRestoreVerified => Self::FinalRecheckPassed,
            Self::FinalRecheckPassed => Self::InstallationIdentityReserved,
            Self::InstallationIdentityReserved => Self::TargetsBootstrapped,
            Self::TargetsBootstrapped => Self::PostgresBulkCopied,
            Self::PostgresBulkCopied => Self::PostgresValueVerified,
            Self::PostgresValueVerified => Self::ClickhouseProjected,
            Self::ClickhouseProjected => Self::RuntimeMaterialized,
            Self::RuntimeMaterialized => Self::NodesVerified,
            Self::NodesVerified => Self::NativeAuthorityCommitted,
            Self::NativeAuthorityCommitted => Self::CutoverCommitted,
            Self::CutoverCommitted => Self::SourceRetired,
            Self::SourceRetired => Self::CompletionVerified,
            Self::CompletionVerified => return None,
        })
    }
}

/// Immutable proof returned by PostgreSQL's atomic native-authority commit.
///
/// The anchor is always a clean `verifying/nodes_verified` event from this
/// journal's own verified hash chain. It deliberately contains no service
/// start receipt: systemd activation happens only after this proof is fsynced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeAuthorityBinding {
    nodes_verified_generation: u64,
    nodes_verified_event_sha256: String,
    data_verification_report_sha256: String,
    analytics_projection_report_sha256: String,
    node_cutover_report_sha256: String,
}

impl NativeAuthorityBinding {
    pub fn new(
        nodes_verified_generation: u64,
        nodes_verified_event_sha256: impl AsRef<str>,
        data_verification_report_sha256: impl AsRef<str>,
        analytics_projection_report_sha256: impl AsRef<str>,
        node_cutover_report_sha256: impl AsRef<str>,
    ) -> Result<Self, ApplyJournalError> {
        let binding = Self {
            nodes_verified_generation,
            nodes_verified_event_sha256: nodes_verified_event_sha256.as_ref().to_string(),
            data_verification_report_sha256: data_verification_report_sha256.as_ref().to_string(),
            analytics_projection_report_sha256: analytics_projection_report_sha256
                .as_ref()
                .to_string(),
            node_cutover_report_sha256: node_cutover_report_sha256.as_ref().to_string(),
        };
        binding.validate()?;
        Ok(binding)
    }

    pub const fn nodes_verified_generation(&self) -> u64 {
        self.nodes_verified_generation
    }

    pub fn nodes_verified_event_sha256(&self) -> &str {
        &self.nodes_verified_event_sha256
    }

    pub fn data_verification_report_sha256(&self) -> &str {
        &self.data_verification_report_sha256
    }

    pub fn analytics_projection_report_sha256(&self) -> &str {
        &self.analytics_projection_report_sha256
    }

    pub fn node_cutover_report_sha256(&self) -> &str {
        &self.node_cutover_report_sha256
    }

    fn validate(&self) -> Result<(), ApplyJournalError> {
        if self.nodes_verified_generation == 0
            || [
                self.nodes_verified_event_sha256.as_str(),
                self.data_verification_report_sha256.as_str(),
                self.analytics_projection_report_sha256.as_str(),
                self.node_cutover_report_sha256.as_str(),
            ]
            .into_iter()
            .any(|value| !is_lower_hex(value, 64))
        {
            return Err(ApplyJournalError::InvalidNativeAuthorityBinding);
        }
        Ok(())
    }
}

/// A bounded, non-sensitive classification for a failed or interrupted step.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyOutcomeCode {
    ProcessInterrupted,
    IoFailure,
    SourceDrift,
    TargetDrift,
    FenceUncertain,
    DrainIncomplete,
    BackupInvalid,
    ConversionFailed,
    VerificationMismatch,
    ActivationFailed,
    RetirementFailed,
    OperatorAbort,
}

/// A verified view of the current CAS head. It is safe to serialize in a report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApplyJournalSnapshot {
    binding: ApplyJournalBinding,
    generation: u64,
    state: ApplyJournalState,
    checkpoint: ApplyCheckpoint,
    outcome_code: Option<ApplyOutcomeCode>,
    previous_event_sha256: Option<String>,
    event_sha256: String,
    recorded_at_unix_ms: u64,
    resume_state: Option<ApplyJournalState>,
    installation_id: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    backup_reference_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    source_fingerprint_sha256: Option<String>,
    checkpoint_proof_sha256: Option<String>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<String>,
    data_verification_report_sha256: Option<String>,
    analytics_projection_report_sha256: Option<String>,
    node_cutover_report_sha256: Option<String>,
}

impl ApplyJournalSnapshot {
    pub fn binding(&self) -> &ApplyJournalBinding {
        &self.binding
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn state(&self) -> ApplyJournalState {
        self.state
    }

    pub const fn checkpoint(&self) -> ApplyCheckpoint {
        self.checkpoint
    }

    pub const fn outcome_code(&self) -> Option<ApplyOutcomeCode> {
        self.outcome_code
    }

    pub fn previous_event_sha256(&self) -> Option<&str> {
        self.previous_event_sha256.as_deref()
    }

    pub fn event_sha256(&self) -> &str {
        &self.event_sha256
    }

    pub const fn recorded_at_unix_ms(&self) -> u64 {
        self.recorded_at_unix_ms
    }

    pub const fn can_resume(&self) -> bool {
        matches!(self.state, ApplyJournalState::NeedsRecovery) && self.resume_state.is_some()
    }

    /// The durable native installation UUID reserved after the fenced final
    /// recheck and before the first target mutation.
    pub fn installation_id(&self) -> Option<&str> {
        self.installation_id.as_deref()
    }

    pub fn backup_restore_proof_sha256(&self) -> Option<&str> {
        self.backup_restore_proof_sha256.as_deref()
    }

    pub fn backup_reference_sha256(&self) -> Option<&str> {
        self.backup_reference_sha256.as_deref()
    }

    pub fn final_recheck_report_sha256(&self) -> Option<&str> {
        self.final_recheck_report_sha256.as_deref()
    }

    pub fn source_fingerprint_sha256(&self) -> Option<&str> {
        self.source_fingerprint_sha256.as_deref()
    }

    pub fn checkpoint_proof_sha256(&self) -> Option<&str> {
        self.checkpoint_proof_sha256.as_deref()
    }

    pub fn native_authority_binding(&self) -> Option<NativeAuthorityBinding> {
        native_authority_binding_from_fields(
            self.native_authority_nodes_generation,
            self.native_authority_nodes_event_sha256.as_deref(),
            self.data_verification_report_sha256.as_deref(),
            self.analytics_projection_report_sha256.as_deref(),
            self.node_cutover_report_sha256.as_deref(),
        )
        .expect("verified journal snapshots have a complete authority binding")
    }
}

/// Proof that a current, fsync-durable non-terminal head was re-read before a
/// protected mutation. Mutators should require this value rather than accepting
/// an operation ID alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableMutationPermit {
    operation_id: String,
    inspect_review_sha256: String,
    generation: u64,
    event_sha256: String,
    installation_id: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    backup_reference_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    source_fingerprint_sha256: Option<String>,
    checkpoint_proof_sha256: Option<String>,
    native_authority: Option<NativeAuthorityBinding>,
}

impl DurableMutationPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn inspect_review_sha256(&self) -> &str {
        &self.inspect_review_sha256
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn event_sha256(&self) -> &str {
        &self.event_sha256
    }

    pub fn installation_id(&self) -> Option<&str> {
        self.installation_id.as_deref()
    }

    pub fn backup_restore_proof_sha256(&self) -> Option<&str> {
        self.backup_restore_proof_sha256.as_deref()
    }

    pub fn backup_reference_sha256(&self) -> Option<&str> {
        self.backup_reference_sha256.as_deref()
    }

    pub fn final_recheck_report_sha256(&self) -> Option<&str> {
        self.final_recheck_report_sha256.as_deref()
    }

    pub fn source_fingerprint_sha256(&self) -> Option<&str> {
        self.source_fingerprint_sha256.as_deref()
    }

    pub fn checkpoint_proof_sha256(&self) -> Option<&str> {
        self.checkpoint_proof_sha256.as_deref()
    }

    pub fn native_authority_binding(&self) -> Option<&NativeAuthorityBinding> {
        self.native_authority.as_ref()
    }
}

/// A stronger permit for PostgreSQL/ClickHouse/Redis target mutations. It is
/// unavailable until the fenced final recheck has passed and a non-zero native
/// installation UUID has itself been made durable in the filesystem journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableTargetMutationPermit {
    operation_id: String,
    installation_id: String,
    inspect_review_sha256: String,
    generation: u64,
    event_sha256: String,
    backup_restore_proof_sha256: String,
    backup_reference_sha256: String,
    final_recheck_report_sha256: String,
    source_fingerprint_sha256: String,
    checkpoint_proof_sha256: Option<String>,
}

impl DurableTargetMutationPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    pub fn inspect_review_sha256(&self) -> &str {
        &self.inspect_review_sha256
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn event_sha256(&self) -> &str {
        &self.event_sha256
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

    pub fn checkpoint_proof_sha256(&self) -> Option<&str> {
        self.checkpoint_proof_sha256.as_deref()
    }
}

/// The only permit accepted by the post-authority native service-start path.
/// It cannot be converted into a target-mutation permit and is available only
/// while the durable head is exactly `native_authority_committed`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNativeStartPermit {
    operation_id: String,
    installation_id: String,
    inspect_review_sha256: String,
    generation: u64,
    event_sha256: String,
    checkpoint_proof_sha256: String,
    native_authority: NativeAuthorityBinding,
}

impl DurableNativeStartPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    pub fn inspect_review_sha256(&self) -> &str {
        &self.inspect_review_sha256
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn event_sha256(&self) -> &str {
        &self.event_sha256
    }

    pub fn checkpoint_proof_sha256(&self) -> &str {
        &self.checkpoint_proof_sha256
    }

    pub fn native_authority_binding(&self) -> &NativeAuthorityBinding {
        &self.native_authority
    }
}

/// An append-only, hash-chained operation journal.
///
/// `events/NNN.json` is the immutable event ledger. `heads/NNN.json` is the
/// create-new CAS slot for that generation. Publishing a head never overwrites
/// an earlier head, so two writers racing from generation N cannot both commit
/// generation N+1. A crash after the event link but before the head link leaves
/// one verifiable orphan event; `open`/`reload` completes that same event's head
/// and never invents a replacement event.
#[derive(Clone, Debug)]
pub struct ApplyJournal {
    operation_dir: PathBuf,
    events_dir: PathBuf,
    heads_dir: PathBuf,
    binding: ApplyJournalBinding,
}

impl ApplyJournal {
    /// Creates and fsyncs the initial `pending` event before returning.
    ///
    /// `journal_root` must be absolute and owner-private. If it does not exist,
    /// only its final component is created; its parent must already exist.
    pub fn create_pending(
        journal_root: impl AsRef<Path>,
        binding: ApplyJournalBinding,
    ) -> Result<(Self, ApplyJournalSnapshot), ApplyJournalError> {
        let journal_root = journal_root.as_ref();
        prepare_journal_root(journal_root, true)?;
        let journal = Self::for_binding(journal_root, binding);
        journal.prepare_operation_directories()?;

        let events = list_record_files(&journal.events_dir)?;
        let heads = list_record_files(&journal.heads_dir)?;
        if !events.is_empty() || !heads.is_empty() {
            // Validate existing state before reporting it as an already-started
            // operation. Corruption must never be disguised as idempotency.
            let _ = journal.load_records(true)?;
            return Err(ApplyJournalError::AlreadyExists);
        }

        let event = JournalEventRecord::pending(&journal.binding)?;
        journal.publish_event(&event)?;
        journal.publish_head(&JournalHeadRecord::from_event(&event))?;
        let snapshot = journal.load_records(true)?;
        sync_dir(&journal.operation_dir)?;
        sync_dir(
            journal
                .operation_dir
                .parent()
                .ok_or(ApplyJournalError::UnsafePath(
                    "operation journal has no parent directory",
                ))?,
        )?;
        Ok((journal, snapshot))
    }

    /// Opens only the operation named by `binding.operation_id` and verifies
    /// every event, head, hash, transition, permission, and binding.
    pub fn open(
        journal_root: impl AsRef<Path>,
        binding: ApplyJournalBinding,
    ) -> Result<(Self, ApplyJournalSnapshot), ApplyJournalError> {
        let journal_root = journal_root.as_ref();
        prepare_journal_root(journal_root, false)?;
        let journal = Self::for_binding(journal_root, binding);
        if matches!(
            fs::symlink_metadata(&journal.operation_dir),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        ) {
            return Err(ApplyJournalError::OperationNotFound);
        }
        journal.validate_operation_directories()?;
        let snapshot = journal.load_records(true)?;
        Ok((journal, snapshot))
    }

    pub fn operation_dir(&self) -> &Path {
        &self.operation_dir
    }

    pub fn reload(&self) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        self.validate_operation_directories()?;
        self.load_records(true)
    }

    /// Returns the complete verified event history for permanent mirroring
    /// after the target PostgreSQL schema exists.
    pub fn verified_history(&self) -> Result<Vec<ApplyJournalSnapshot>, ApplyJournalError> {
        self.validate_operation_directories()?;
        // Complete an otherwise valid orphan head before exporting history.
        let _ = self.load_records(true)?;
        let event_paths = list_record_files(&self.events_dir)?;
        ensure_contiguous(&event_paths)?;
        let mut events = Vec::with_capacity(event_paths.len());
        for (generation, path) in event_paths {
            let event: JournalEventRecord = read_json_record(&path, "event")?;
            event.validate(generation, &self.binding, &events)?;
            events.push(event);
        }
        Ok(events
            .iter()
            .map(ApplyJournalSnapshot::from_event)
            .collect())
    }

    /// Re-reads the durable CAS head and returns a non-serializable mutation
    /// permit. Recovery, failed, and completed heads never authorize mutation.
    pub fn mutation_permit(
        &self,
        expected: &ApplyJournalSnapshot,
    ) -> Result<DurableMutationPermit, ApplyJournalError> {
        let current = self.reload()?;
        self.require_expected(expected, &current)?;
        if !matches!(
            current.state,
            ApplyJournalState::Pending | ApplyJournalState::Running | ApplyJournalState::Verifying
        ) {
            return Err(ApplyJournalError::MutationNotAuthorized(current.state));
        }
        let native_authority = current.native_authority_binding();
        Ok(DurableMutationPermit {
            operation_id: current.binding.operation_id.clone(),
            inspect_review_sha256: current.binding.inspect_review_sha256.clone(),
            generation: current.generation,
            event_sha256: current.event_sha256,
            installation_id: current.installation_id,
            backup_restore_proof_sha256: current.backup_restore_proof_sha256,
            backup_reference_sha256: current.backup_reference_sha256,
            final_recheck_report_sha256: current.final_recheck_report_sha256,
            source_fingerprint_sha256: current.source_fingerprint_sha256,
            checkpoint_proof_sha256: current.checkpoint_proof_sha256,
            native_authority,
        })
    }

    /// Returns the proof a target mutator must require. Before target creation,
    /// the filesystem journal is authoritative. After creation, callers mirror
    /// this same operation/installation/report/generation/event tuple into the
    /// permanent PostgreSQL lifecycle ledger before advancing again.
    pub fn target_mutation_permit(
        &self,
        expected: &ApplyJournalSnapshot,
    ) -> Result<DurableTargetMutationPermit, ApplyJournalError> {
        let current = self.reload()?;
        self.require_expected(expected, &current)?;
        if !matches!(
            current.state,
            ApplyJournalState::Running | ApplyJournalState::Verifying
        ) || current.checkpoint < ApplyCheckpoint::InstallationIdentityReserved
            || current.checkpoint >= ApplyCheckpoint::NativeAuthorityCommitted
        {
            return Err(ApplyJournalError::TargetMutationNotAuthorized);
        }
        let installation_id = current
            .installation_id
            .clone()
            .ok_or(ApplyJournalError::InstallationBindingRequired)?;
        let backup_restore_proof_sha256 = current
            .backup_restore_proof_sha256
            .clone()
            .ok_or(ApplyJournalError::BackupProofBindingRequired)?;
        let backup_reference_sha256 = current
            .backup_reference_sha256
            .clone()
            .ok_or(ApplyJournalError::BackupReferenceBindingRequired)?;
        let final_recheck_report_sha256 = current
            .final_recheck_report_sha256
            .clone()
            .ok_or(ApplyJournalError::FinalRecheckBindingRequired)?;
        let source_fingerprint_sha256 = current
            .source_fingerprint_sha256
            .clone()
            .ok_or(ApplyJournalError::SourceFingerprintBindingRequired)?;
        Ok(DurableTargetMutationPermit {
            operation_id: current.binding.operation_id,
            installation_id,
            inspect_review_sha256: current.binding.inspect_review_sha256,
            generation: current.generation,
            event_sha256: current.event_sha256,
            backup_restore_proof_sha256,
            backup_reference_sha256,
            final_recheck_report_sha256,
            source_fingerprint_sha256,
            checkpoint_proof_sha256: current.checkpoint_proof_sha256,
        })
    }

    /// Re-reads the fsync-durable authority checkpoint and returns the only
    /// permit that may be used to start the native API and worker. Recovery
    /// callers must first resume a `needs_recovery` event back to `verifying`.
    pub fn native_start_permit(
        &self,
        expected: &ApplyJournalSnapshot,
    ) -> Result<DurableNativeStartPermit, ApplyJournalError> {
        let current = self.reload()?;
        self.require_expected(expected, &current)?;
        if current.state != ApplyJournalState::Verifying
            || current.checkpoint != ApplyCheckpoint::NativeAuthorityCommitted
            || current.outcome_code.is_some()
        {
            return Err(ApplyJournalError::NativeStartNotAuthorized);
        }
        Ok(DurableNativeStartPermit {
            operation_id: current.binding.operation_id.clone(),
            installation_id: current
                .installation_id
                .clone()
                .ok_or(ApplyJournalError::InstallationBindingRequired)?,
            inspect_review_sha256: current.binding.inspect_review_sha256.clone(),
            generation: current.generation,
            event_sha256: current.event_sha256.clone(),
            checkpoint_proof_sha256: current
                .checkpoint_proof_sha256
                .clone()
                .ok_or(ApplyJournalError::CheckpointProofBindingRequired)?,
            native_authority: current
                .native_authority_binding()
                .ok_or(ApplyJournalError::NativeAuthorityBindingRequired)?,
        })
    }

    pub fn begin(
        &self,
        expected: &ApplyJournalSnapshot,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        self.append_transition(
            expected,
            ApplyJournalState::Running,
            expected.checkpoint,
            None,
            TransitionBindings::default(),
        )
    }

    pub fn checkpoint(
        &self,
        expected: &ApplyJournalSnapshot,
        checkpoint: ApplyCheckpoint,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        match checkpoint {
            ApplyCheckpoint::BackupRestoreVerified => {
                return Err(ApplyJournalError::BackupProofBindingRequired);
            }
            ApplyCheckpoint::FinalRecheckPassed => {
                return Err(ApplyJournalError::FinalRecheckBindingRequired);
            }
            ApplyCheckpoint::InstallationIdentityReserved => {
                return Err(ApplyJournalError::InstallationBindingRequired);
            }
            ApplyCheckpoint::NativeAuthorityCommitted => {
                return Err(ApplyJournalError::NativeAuthorityBindingRequired);
            }
            _ => {}
        }
        self.append_transition(
            expected,
            expected.state,
            checkpoint,
            None,
            TransitionBindings::default(),
        )
    }

    /// Advances one business checkpoint and binds its canonical verification
    /// report. Same-checkpoint recovery transitions inherit this value and can
    /// never replace it.
    pub fn checkpoint_with_proof(
        &self,
        expected: &ApplyJournalSnapshot,
        checkpoint: ApplyCheckpoint,
        checkpoint_proof_sha256: impl AsRef<str>,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        if matches!(
            checkpoint,
            ApplyCheckpoint::BackupRestoreVerified
                | ApplyCheckpoint::FinalRecheckPassed
                | ApplyCheckpoint::InstallationIdentityReserved
                | ApplyCheckpoint::NativeAuthorityCommitted
                | ApplyCheckpoint::CompletionVerified
        ) {
            return Err(ApplyJournalError::SpecialCheckpointMethodRequired);
        }
        let proof = checked_sha256(
            checkpoint_proof_sha256.as_ref(),
            ApplyJournalError::InvalidCheckpointProofHash,
        )?;
        self.append_transition(
            expected,
            expected.state,
            checkpoint,
            None,
            TransitionBindings {
                checkpoint_proof_sha256: Some(proof),
                ..TransitionBindings::default()
            },
        )
    }

    pub fn record_backup_restore_verified(
        &self,
        expected: &ApplyJournalSnapshot,
        backup_restore_proof_sha256: impl AsRef<str>,
        backup_reference_sha256: impl AsRef<str>,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let proof = checked_sha256(
            backup_restore_proof_sha256.as_ref(),
            ApplyJournalError::InvalidBackupProofHash,
        )?;
        let reference = checked_sha256(
            backup_reference_sha256.as_ref(),
            ApplyJournalError::InvalidBackupReferenceHash,
        )?;
        self.append_transition(
            expected,
            expected.state,
            ApplyCheckpoint::BackupRestoreVerified,
            None,
            TransitionBindings {
                checkpoint_proof_sha256: Some(proof.clone()),
                backup_restore_proof_sha256: Some(proof),
                backup_reference_sha256: Some(reference),
                ..TransitionBindings::default()
            },
        )
    }

    /// Persists the fenced final recheck digest. A failed or drifted recheck
    /// must never call this method and therefore cannot obtain a target permit.
    pub fn record_final_recheck_passed(
        &self,
        expected: &ApplyJournalSnapshot,
        final_recheck_report_sha256: impl AsRef<str>,
        source_fingerprint_sha256: impl AsRef<str>,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let report = checked_sha256(
            final_recheck_report_sha256.as_ref(),
            ApplyJournalError::InvalidFinalRecheckHash,
        )?;
        let fingerprint = checked_sha256(
            source_fingerprint_sha256.as_ref(),
            ApplyJournalError::InvalidSourceFingerprintHash,
        )?;
        self.append_transition(
            expected,
            expected.state,
            ApplyCheckpoint::FinalRecheckPassed,
            None,
            TransitionBindings {
                checkpoint_proof_sha256: Some(report.clone()),
                final_recheck_report_sha256: Some(report),
                source_fingerprint_sha256: Some(fingerprint),
                ..TransitionBindings::default()
            },
        )
    }

    /// Reserves the native installation identity after the final recheck and
    /// before target bootstrap. The UUID is generated by the apply orchestrator
    /// exactly once; crash recovery must reuse the value read from this event.
    pub fn reserve_installation_identity(
        &self,
        expected: &ApplyJournalSnapshot,
        installation_id: impl AsRef<str>,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let installation_id = Uuid::parse_str(installation_id.as_ref())
            .map_err(|_| ApplyJournalError::InvalidInstallationId)?;
        if installation_id.is_nil() {
            return Err(ApplyJournalError::InvalidInstallationId);
        }
        self.append_transition(
            expected,
            expected.state,
            ApplyCheckpoint::InstallationIdentityReserved,
            None,
            TransitionBindings {
                installation_id: Some(installation_id.hyphenated().to_string()),
                ..TransitionBindings::default()
            },
        )
    }

    /// Fsyncs the already-committed PostgreSQL authority proof before any
    /// native unit is started. The anchor may be an earlier clean
    /// `nodes_verified` event when a lost PostgreSQL acknowledgement caused a
    /// needs-recovery/resume pair at the same checkpoint.
    pub fn record_native_authority_committed(
        &self,
        expected: &ApplyJournalSnapshot,
        authority: &NativeAuthorityBinding,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        authority.validate()?;
        let current = self.reload()?;
        self.require_expected(expected, &current)?;
        if current.state != ApplyJournalState::Verifying
            || current.checkpoint != ApplyCheckpoint::NodesVerified
            || current.outcome_code.is_some()
            || current.native_authority_binding().is_some()
        {
            return Err(ApplyJournalError::NativeAuthorityAnchorMismatch);
        }
        let history = self.verified_history()?;
        let anchor = usize::try_from(authority.nodes_verified_generation)
            .ok()
            .and_then(|generation| history.get(generation))
            .ok_or(ApplyJournalError::NativeAuthorityAnchorMismatch)?;
        if anchor.generation() > current.generation()
            || anchor.state() != ApplyJournalState::Verifying
            || anchor.checkpoint() != ApplyCheckpoint::NodesVerified
            || anchor.outcome_code().is_some()
            || anchor.event_sha256() != authority.nodes_verified_event_sha256
            || anchor.native_authority_binding().is_some()
            || anchor.installation_id() != current.installation_id()
            || anchor.backup_restore_proof_sha256() != current.backup_restore_proof_sha256()
            || anchor.backup_reference_sha256() != current.backup_reference_sha256()
            || anchor.final_recheck_report_sha256() != current.final_recheck_report_sha256()
            || anchor.source_fingerprint_sha256() != current.source_fingerprint_sha256()
            || anchor.checkpoint_proof_sha256() != Some(authority.node_cutover_report_sha256())
            || latest_snapshot_checkpoint_proof(
                &history,
                anchor.generation(),
                ApplyCheckpoint::PostgresValueVerified,
            ) != Some(authority.data_verification_report_sha256())
            || latest_snapshot_checkpoint_proof(
                &history,
                anchor.generation(),
                ApplyCheckpoint::ClickhouseProjected,
            ) != Some(authority.analytics_projection_report_sha256())
        {
            return Err(ApplyJournalError::NativeAuthorityAnchorMismatch);
        }
        self.append_transition(
            &current,
            ApplyJournalState::Verifying,
            ApplyCheckpoint::NativeAuthorityCommitted,
            None,
            TransitionBindings {
                native_authority_nodes_generation: Some(authority.nodes_verified_generation),
                native_authority_nodes_event_sha256: Some(
                    authority.nodes_verified_event_sha256.clone(),
                ),
                data_verification_report_sha256: Some(
                    authority.data_verification_report_sha256.clone(),
                ),
                analytics_projection_report_sha256: Some(
                    authority.analytics_projection_report_sha256.clone(),
                ),
                node_cutover_report_sha256: Some(authority.node_cutover_report_sha256.clone()),
                checkpoint_proof_sha256: Some(native_authority_proof_sha256(authority)),
                ..TransitionBindings::default()
            },
        )
    }

    pub fn enter_verification(
        &self,
        expected: &ApplyJournalSnapshot,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        self.append_transition(
            expected,
            ApplyJournalState::Verifying,
            expected.checkpoint,
            None,
            TransitionBindings::default(),
        )
    }

    pub fn mark_needs_recovery(
        &self,
        expected: &ApplyJournalSnapshot,
        code: ApplyOutcomeCode,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        self.append_transition(
            expected,
            ApplyJournalState::NeedsRecovery,
            expected.checkpoint,
            Some(code),
            TransitionBindings::default(),
        )
    }

    pub fn resume(
        &self,
        expected: &ApplyJournalSnapshot,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let resume_state = expected
            .resume_state
            .ok_or(ApplyJournalError::NotRecoverable(expected.state))?;
        self.append_transition(
            expected,
            resume_state,
            expected.checkpoint,
            None,
            TransitionBindings::default(),
        )
    }

    pub fn mark_failed(
        &self,
        expected: &ApplyJournalSnapshot,
        code: ApplyOutcomeCode,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        self.append_transition(
            expected,
            ApplyJournalState::Failed,
            expected.checkpoint,
            Some(code),
            TransitionBindings::default(),
        )
    }

    pub fn complete(
        &self,
        expected: &ApplyJournalSnapshot,
        completion_proof_sha256: impl AsRef<str>,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let proof = checked_sha256(
            completion_proof_sha256.as_ref(),
            ApplyJournalError::InvalidCheckpointProofHash,
        )?;
        self.append_transition(
            expected,
            ApplyJournalState::Completed,
            ApplyCheckpoint::CompletionVerified,
            None,
            TransitionBindings {
                checkpoint_proof_sha256: Some(proof),
                ..TransitionBindings::default()
            },
        )
    }

    fn for_binding(journal_root: &Path, binding: ApplyJournalBinding) -> Self {
        let operation_dir = journal_root.join(binding.operation_id());
        Self {
            events_dir: operation_dir.join("events"),
            heads_dir: operation_dir.join("heads"),
            operation_dir,
            binding,
        }
    }

    fn prepare_operation_directories(&self) -> Result<(), ApplyJournalError> {
        ensure_private_dir(&self.operation_dir)?;
        ensure_private_dir(&self.events_dir)?;
        ensure_private_dir(&self.heads_dir)?;
        self.validate_operation_directories()
    }

    fn validate_operation_directories(&self) -> Result<(), ApplyJournalError> {
        validate_private_dir(&self.operation_dir)?;
        validate_private_dir(&self.events_dir)?;
        validate_private_dir(&self.heads_dir)?;
        let mut expected = ["events", "heads"].into_iter().collect::<Vec<_>>();
        expected.sort_unstable();
        let mut actual = Vec::new();
        for entry in fs::read_dir(&self.operation_dir)
            .map_err(|source| ApplyJournalError::io("read operation directory", source))?
        {
            let entry =
                entry.map_err(|source| ApplyJournalError::io("read operation entry", source))?;
            actual.push(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| ApplyJournalError::Corrupt("non-UTF-8 operation entry"))?,
            );
        }
        actual.sort_unstable();
        if actual != expected {
            return Err(ApplyJournalError::Corrupt(
                "operation directory contains unexpected entries",
            ));
        }
        Ok(())
    }

    fn append_transition(
        &self,
        expected: &ApplyJournalSnapshot,
        next_state: ApplyJournalState,
        checkpoint: ApplyCheckpoint,
        outcome_code: Option<ApplyOutcomeCode>,
        bindings: TransitionBindings,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let current = self.reload()?;
        self.require_expected(expected, &current)?;
        let validated =
            validate_transition(&current, next_state, checkpoint, outcome_code, &bindings)?;
        let generation = current
            .generation
            .checked_add(1)
            .ok_or(ApplyJournalError::GenerationOverflow)?;
        let event = JournalEventRecord::transition(
            &self.binding,
            generation,
            &current,
            next_state,
            checkpoint,
            outcome_code,
            validated,
        )?;
        self.publish_event(&event)?;
        let head = JournalHeadRecord::from_event(&event);
        match self.publish_head(&head) {
            Ok(()) => {}
            Err(ApplyJournalError::ConcurrentUpdate) => {
                // A recovery reader may have completed the exact orphan event.
                let recovered = self.load_records(false)?;
                if recovered.generation == generation
                    && recovered.event_sha256 == event.event_sha256
                {
                    return Ok(recovered);
                }
                return Err(ApplyJournalError::ConcurrentUpdate);
            }
            Err(error) => return Err(error),
        }
        self.load_records(true)
    }

    fn require_expected(
        &self,
        expected: &ApplyJournalSnapshot,
        current: &ApplyJournalSnapshot,
    ) -> Result<(), ApplyJournalError> {
        if expected.binding != self.binding || current.binding != self.binding {
            return Err(ApplyJournalError::BindingMismatch);
        }
        if expected.generation != current.generation
            || expected.event_sha256 != current.event_sha256
            || expected.state != current.state
            || expected.checkpoint != current.checkpoint
        {
            return Err(ApplyJournalError::ConcurrentUpdate);
        }
        Ok(())
    }

    fn publish_event(&self, event: &JournalEventRecord) -> Result<(), ApplyJournalError> {
        let bytes = serde_json::to_vec(event)
            .map_err(|source| ApplyJournalError::json("serialize event", source))?;
        durable_publish(
            &self.events_dir,
            &record_file_name(event.generation),
            &bytes,
        )
    }

    fn publish_head(&self, head: &JournalHeadRecord) -> Result<(), ApplyJournalError> {
        let bytes = serde_json::to_vec(head)
            .map_err(|source| ApplyJournalError::json("serialize head", source))?;
        durable_publish(&self.heads_dir, &record_file_name(head.generation), &bytes)
    }

    fn load_records(
        &self,
        recover_orphan: bool,
    ) -> Result<ApplyJournalSnapshot, ApplyJournalError> {
        let event_paths = list_record_files(&self.events_dir)?;
        let head_paths = list_record_files(&self.heads_dir)?;
        if event_paths.is_empty() && head_paths.is_empty() {
            return Err(ApplyJournalError::Uninitialized);
        }
        ensure_contiguous(&event_paths)?;
        ensure_contiguous(&head_paths)?;
        if head_paths.len() > event_paths.len()
            || event_paths.len().saturating_sub(head_paths.len()) > 1
        {
            return Err(ApplyJournalError::Corrupt(
                "event and CAS-head generations do not align",
            ));
        }

        let mut events = Vec::with_capacity(event_paths.len());
        for (generation, path) in &event_paths {
            let event: JournalEventRecord = read_json_record(path, "event")?;
            event.validate(*generation, &self.binding, &events)?;
            events.push(event);
        }
        for (generation, path) in &head_paths {
            let head: JournalHeadRecord = read_json_record(path, "head")?;
            let event = events
                .get(usize::try_from(*generation).map_err(|_| {
                    ApplyJournalError::Corrupt("head generation does not fit memory")
                })?)
                .ok_or(ApplyJournalError::Corrupt("CAS head has no matching event"))?;
            head.validate(*generation, &self.binding, event)?;
        }

        if event_paths.len() == head_paths.len() + 1 {
            if !recover_orphan {
                return Err(ApplyJournalError::OrphanEvent);
            }
            let orphan = events
                .last()
                .ok_or(ApplyJournalError::Corrupt("orphan event is missing"))?;
            match self.publish_head(&JournalHeadRecord::from_event(orphan)) {
                Ok(()) | Err(ApplyJournalError::ConcurrentUpdate) => {
                    return self.load_records(false);
                }
                Err(error) => return Err(error),
            }
        }
        events
            .last()
            .map(ApplyJournalSnapshot::from_event)
            .ok_or(ApplyJournalError::Uninitialized)
    }
}

#[derive(Default)]
struct TransitionBindings {
    installation_id: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    backup_reference_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    source_fingerprint_sha256: Option<String>,
    checkpoint_proof_sha256: Option<String>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<String>,
    data_verification_report_sha256: Option<String>,
    analytics_projection_report_sha256: Option<String>,
    node_cutover_report_sha256: Option<String>,
}

struct ValidatedTransition {
    resume_state: Option<ApplyJournalState>,
    installation_id: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    backup_reference_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    source_fingerprint_sha256: Option<String>,
    checkpoint_proof_sha256: Option<String>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<String>,
    data_verification_report_sha256: Option<String>,
    analytics_projection_report_sha256: Option<String>,
    node_cutover_report_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JournalEventRecord {
    journal_version: u32,
    operation_id: String,
    inspect_review_sha256: String,
    generation: u64,
    previous_event_sha256: Option<String>,
    previous_state: Option<ApplyJournalState>,
    state: ApplyJournalState,
    checkpoint: ApplyCheckpoint,
    outcome_code: Option<ApplyOutcomeCode>,
    resume_state: Option<ApplyJournalState>,
    installation_id: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    backup_reference_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    source_fingerprint_sha256: Option<String>,
    checkpoint_proof_sha256: Option<String>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<String>,
    data_verification_report_sha256: Option<String>,
    analytics_projection_report_sha256: Option<String>,
    node_cutover_report_sha256: Option<String>,
    recorded_at_unix_ms: u64,
    event_sha256: String,
}

#[derive(Serialize)]
struct EventHashMaterial<'a> {
    journal_version: u32,
    operation_id: &'a str,
    inspect_review_sha256: &'a str,
    generation: u64,
    previous_event_sha256: Option<&'a str>,
    previous_state: Option<ApplyJournalState>,
    state: ApplyJournalState,
    checkpoint: ApplyCheckpoint,
    outcome_code: Option<ApplyOutcomeCode>,
    resume_state: Option<ApplyJournalState>,
    installation_id: Option<&'a str>,
    backup_restore_proof_sha256: Option<&'a str>,
    backup_reference_sha256: Option<&'a str>,
    final_recheck_report_sha256: Option<&'a str>,
    source_fingerprint_sha256: Option<&'a str>,
    checkpoint_proof_sha256: Option<&'a str>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<&'a str>,
    data_verification_report_sha256: Option<&'a str>,
    analytics_projection_report_sha256: Option<&'a str>,
    node_cutover_report_sha256: Option<&'a str>,
    recorded_at_unix_ms: u64,
}

impl JournalEventRecord {
    fn pending(binding: &ApplyJournalBinding) -> Result<Self, ApplyJournalError> {
        let mut event = Self {
            journal_version: JOURNAL_VERSION,
            operation_id: binding.operation_id.clone(),
            inspect_review_sha256: binding.inspect_review_sha256.clone(),
            generation: 0,
            previous_event_sha256: None,
            previous_state: None,
            state: ApplyJournalState::Pending,
            checkpoint: ApplyCheckpoint::PendingDurable,
            outcome_code: None,
            resume_state: None,
            installation_id: None,
            backup_restore_proof_sha256: None,
            backup_reference_sha256: None,
            final_recheck_report_sha256: None,
            source_fingerprint_sha256: None,
            checkpoint_proof_sha256: None,
            native_authority_nodes_generation: None,
            native_authority_nodes_event_sha256: None,
            data_verification_report_sha256: None,
            analytics_projection_report_sha256: None,
            node_cutover_report_sha256: None,
            recorded_at_unix_ms: now_unix_ms()?,
            event_sha256: String::new(),
        };
        event.event_sha256 = event.calculate_hash()?;
        Ok(event)
    }

    #[allow(clippy::too_many_arguments)]
    fn transition(
        binding: &ApplyJournalBinding,
        generation: u64,
        current: &ApplyJournalSnapshot,
        state: ApplyJournalState,
        checkpoint: ApplyCheckpoint,
        outcome_code: Option<ApplyOutcomeCode>,
        validated: ValidatedTransition,
    ) -> Result<Self, ApplyJournalError> {
        let mut event = Self {
            journal_version: JOURNAL_VERSION,
            operation_id: binding.operation_id.clone(),
            inspect_review_sha256: binding.inspect_review_sha256.clone(),
            generation,
            previous_event_sha256: Some(current.event_sha256.clone()),
            previous_state: Some(current.state),
            state,
            checkpoint,
            outcome_code,
            resume_state: validated.resume_state,
            installation_id: validated.installation_id,
            backup_restore_proof_sha256: validated.backup_restore_proof_sha256,
            backup_reference_sha256: validated.backup_reference_sha256,
            final_recheck_report_sha256: validated.final_recheck_report_sha256,
            source_fingerprint_sha256: validated.source_fingerprint_sha256,
            checkpoint_proof_sha256: validated.checkpoint_proof_sha256,
            native_authority_nodes_generation: validated.native_authority_nodes_generation,
            native_authority_nodes_event_sha256: validated.native_authority_nodes_event_sha256,
            data_verification_report_sha256: validated.data_verification_report_sha256,
            analytics_projection_report_sha256: validated.analytics_projection_report_sha256,
            node_cutover_report_sha256: validated.node_cutover_report_sha256,
            recorded_at_unix_ms: now_unix_ms()?,
            event_sha256: String::new(),
        };
        event.event_sha256 = event.calculate_hash()?;
        Ok(event)
    }

    fn calculate_hash(&self) -> Result<String, ApplyJournalError> {
        let material = EventHashMaterial {
            journal_version: self.journal_version,
            operation_id: &self.operation_id,
            inspect_review_sha256: &self.inspect_review_sha256,
            generation: self.generation,
            previous_event_sha256: self.previous_event_sha256.as_deref(),
            previous_state: self.previous_state,
            state: self.state,
            checkpoint: self.checkpoint,
            outcome_code: self.outcome_code,
            resume_state: self.resume_state,
            installation_id: self.installation_id.as_deref(),
            backup_restore_proof_sha256: self.backup_restore_proof_sha256.as_deref(),
            backup_reference_sha256: self.backup_reference_sha256.as_deref(),
            final_recheck_report_sha256: self.final_recheck_report_sha256.as_deref(),
            source_fingerprint_sha256: self.source_fingerprint_sha256.as_deref(),
            checkpoint_proof_sha256: self.checkpoint_proof_sha256.as_deref(),
            native_authority_nodes_generation: self.native_authority_nodes_generation,
            native_authority_nodes_event_sha256: self
                .native_authority_nodes_event_sha256
                .as_deref(),
            data_verification_report_sha256: self.data_verification_report_sha256.as_deref(),
            analytics_projection_report_sha256: self.analytics_projection_report_sha256.as_deref(),
            node_cutover_report_sha256: self.node_cutover_report_sha256.as_deref(),
            recorded_at_unix_ms: self.recorded_at_unix_ms,
        };
        let bytes = serde_json::to_vec(&material)
            .map_err(|source| ApplyJournalError::json("hash event", source))?;
        let mut hasher = Sha256::new();
        hasher.update(EVENT_HASH_DOMAIN);
        hasher.update(bytes);
        Ok(hex::encode(hasher.finalize()))
    }

    fn validate(
        &self,
        generation: u64,
        binding: &ApplyJournalBinding,
        history: &[Self],
    ) -> Result<(), ApplyJournalError> {
        if self.journal_version != JOURNAL_VERSION
            || self.generation != generation
            || self.recorded_at_unix_ms == 0
        {
            return Err(ApplyJournalError::Corrupt("invalid event envelope"));
        }
        if self.operation_id != binding.operation_id
            || self.inspect_review_sha256 != binding.inspect_review_sha256
        {
            return Err(ApplyJournalError::BindingMismatch);
        }
        if !is_lower_hex(&self.event_sha256, 64) || self.calculate_hash()? != self.event_sha256 {
            return Err(ApplyJournalError::Corrupt("event hash mismatch"));
        }
        match history.last() {
            None => {
                if generation != 0
                    || self.previous_event_sha256.is_some()
                    || self.previous_state.is_some()
                    || self.state != ApplyJournalState::Pending
                    || self.checkpoint != ApplyCheckpoint::PendingDurable
                    || self.outcome_code.is_some()
                    || self.resume_state.is_some()
                    || self.installation_id.is_some()
                    || self.backup_restore_proof_sha256.is_some()
                    || self.backup_reference_sha256.is_some()
                    || self.final_recheck_report_sha256.is_some()
                    || self.source_fingerprint_sha256.is_some()
                    || self.checkpoint_proof_sha256.is_some()
                    || self.native_authority_nodes_generation.is_some()
                    || self.native_authority_nodes_event_sha256.is_some()
                    || self.data_verification_report_sha256.is_some()
                    || self.analytics_projection_report_sha256.is_some()
                    || self.node_cutover_report_sha256.is_some()
                {
                    return Err(ApplyJournalError::Corrupt("invalid initial pending event"));
                }
            }
            Some(previous) => {
                if generation != previous.generation.saturating_add(1)
                    || self.previous_event_sha256.as_deref() != Some(previous.event_sha256.as_str())
                    || self.previous_state != Some(previous.state)
                {
                    return Err(ApplyJournalError::Corrupt("event chain mismatch"));
                }
                let current = ApplyJournalSnapshot::from_event(previous);
                let validated = validate_transition(
                    &current,
                    self.state,
                    self.checkpoint,
                    self.outcome_code,
                    &TransitionBindings {
                        installation_id: self.installation_id.clone(),
                        backup_restore_proof_sha256: self.backup_restore_proof_sha256.clone(),
                        backup_reference_sha256: self.backup_reference_sha256.clone(),
                        final_recheck_report_sha256: self.final_recheck_report_sha256.clone(),
                        source_fingerprint_sha256: self.source_fingerprint_sha256.clone(),
                        checkpoint_proof_sha256: self.checkpoint_proof_sha256.clone(),
                        native_authority_nodes_generation: self.native_authority_nodes_generation,
                        native_authority_nodes_event_sha256: self
                            .native_authority_nodes_event_sha256
                            .clone(),
                        data_verification_report_sha256: self
                            .data_verification_report_sha256
                            .clone(),
                        analytics_projection_report_sha256: self
                            .analytics_projection_report_sha256
                            .clone(),
                        node_cutover_report_sha256: self.node_cutover_report_sha256.clone(),
                    },
                )?;
                if validated.resume_state != self.resume_state
                    || validated.installation_id.as_deref() != self.installation_id.as_deref()
                    || validated.backup_restore_proof_sha256.as_deref()
                        != self.backup_restore_proof_sha256.as_deref()
                    || validated.backup_reference_sha256.as_deref()
                        != self.backup_reference_sha256.as_deref()
                    || validated.final_recheck_report_sha256.as_deref()
                        != self.final_recheck_report_sha256.as_deref()
                    || validated.source_fingerprint_sha256.as_deref()
                        != self.source_fingerprint_sha256.as_deref()
                    || validated.checkpoint_proof_sha256.as_deref()
                        != self.checkpoint_proof_sha256.as_deref()
                    || validated.native_authority_nodes_generation
                        != self.native_authority_nodes_generation
                    || validated.native_authority_nodes_event_sha256.as_deref()
                        != self.native_authority_nodes_event_sha256.as_deref()
                    || validated.data_verification_report_sha256.as_deref()
                        != self.data_verification_report_sha256.as_deref()
                    || validated.analytics_projection_report_sha256.as_deref()
                        != self.analytics_projection_report_sha256.as_deref()
                    || validated.node_cutover_report_sha256.as_deref()
                        != self.node_cutover_report_sha256.as_deref()
                {
                    return Err(ApplyJournalError::Corrupt(
                        "event recovery phase does not match its transition",
                    ));
                }
                validate_native_authority_anchor_record(self, history)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JournalHeadRecord {
    journal_version: u32,
    operation_id: String,
    inspect_review_sha256: String,
    generation: u64,
    state: ApplyJournalState,
    checkpoint: ApplyCheckpoint,
    outcome_code: Option<ApplyOutcomeCode>,
    previous_event_sha256: Option<String>,
    event_sha256: String,
    installation_id: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    backup_reference_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    source_fingerprint_sha256: Option<String>,
    checkpoint_proof_sha256: Option<String>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<String>,
    data_verification_report_sha256: Option<String>,
    analytics_projection_report_sha256: Option<String>,
    node_cutover_report_sha256: Option<String>,
    head_sha256: String,
}

#[derive(Serialize)]
struct HeadHashMaterial<'a> {
    journal_version: u32,
    operation_id: &'a str,
    inspect_review_sha256: &'a str,
    generation: u64,
    state: ApplyJournalState,
    checkpoint: ApplyCheckpoint,
    outcome_code: Option<ApplyOutcomeCode>,
    previous_event_sha256: Option<&'a str>,
    event_sha256: &'a str,
    installation_id: Option<&'a str>,
    backup_restore_proof_sha256: Option<&'a str>,
    backup_reference_sha256: Option<&'a str>,
    final_recheck_report_sha256: Option<&'a str>,
    source_fingerprint_sha256: Option<&'a str>,
    checkpoint_proof_sha256: Option<&'a str>,
    native_authority_nodes_generation: Option<u64>,
    native_authority_nodes_event_sha256: Option<&'a str>,
    data_verification_report_sha256: Option<&'a str>,
    analytics_projection_report_sha256: Option<&'a str>,
    node_cutover_report_sha256: Option<&'a str>,
}

impl JournalHeadRecord {
    fn from_event(event: &JournalEventRecord) -> Self {
        let mut head = Self {
            journal_version: JOURNAL_VERSION,
            operation_id: event.operation_id.clone(),
            inspect_review_sha256: event.inspect_review_sha256.clone(),
            generation: event.generation,
            state: event.state,
            checkpoint: event.checkpoint,
            outcome_code: event.outcome_code,
            previous_event_sha256: event.previous_event_sha256.clone(),
            event_sha256: event.event_sha256.clone(),
            installation_id: event.installation_id.clone(),
            backup_restore_proof_sha256: event.backup_restore_proof_sha256.clone(),
            backup_reference_sha256: event.backup_reference_sha256.clone(),
            final_recheck_report_sha256: event.final_recheck_report_sha256.clone(),
            source_fingerprint_sha256: event.source_fingerprint_sha256.clone(),
            checkpoint_proof_sha256: event.checkpoint_proof_sha256.clone(),
            native_authority_nodes_generation: event.native_authority_nodes_generation,
            native_authority_nodes_event_sha256: event.native_authority_nodes_event_sha256.clone(),
            data_verification_report_sha256: event.data_verification_report_sha256.clone(),
            analytics_projection_report_sha256: event.analytics_projection_report_sha256.clone(),
            node_cutover_report_sha256: event.node_cutover_report_sha256.clone(),
            head_sha256: String::new(),
        };
        head.head_sha256 = head
            .calculate_hash()
            .expect("fixed journal head fields are serializable");
        head
    }

    fn calculate_hash(&self) -> Result<String, ApplyJournalError> {
        let material = HeadHashMaterial {
            journal_version: self.journal_version,
            operation_id: &self.operation_id,
            inspect_review_sha256: &self.inspect_review_sha256,
            generation: self.generation,
            state: self.state,
            checkpoint: self.checkpoint,
            outcome_code: self.outcome_code,
            previous_event_sha256: self.previous_event_sha256.as_deref(),
            event_sha256: &self.event_sha256,
            installation_id: self.installation_id.as_deref(),
            backup_restore_proof_sha256: self.backup_restore_proof_sha256.as_deref(),
            backup_reference_sha256: self.backup_reference_sha256.as_deref(),
            final_recheck_report_sha256: self.final_recheck_report_sha256.as_deref(),
            source_fingerprint_sha256: self.source_fingerprint_sha256.as_deref(),
            checkpoint_proof_sha256: self.checkpoint_proof_sha256.as_deref(),
            native_authority_nodes_generation: self.native_authority_nodes_generation,
            native_authority_nodes_event_sha256: self
                .native_authority_nodes_event_sha256
                .as_deref(),
            data_verification_report_sha256: self.data_verification_report_sha256.as_deref(),
            analytics_projection_report_sha256: self.analytics_projection_report_sha256.as_deref(),
            node_cutover_report_sha256: self.node_cutover_report_sha256.as_deref(),
        };
        let bytes = serde_json::to_vec(&material)
            .map_err(|source| ApplyJournalError::json("hash head", source))?;
        let mut hasher = Sha256::new();
        hasher.update(HEAD_HASH_DOMAIN);
        hasher.update(bytes);
        Ok(hex::encode(hasher.finalize()))
    }

    fn validate(
        &self,
        generation: u64,
        binding: &ApplyJournalBinding,
        event: &JournalEventRecord,
    ) -> Result<(), ApplyJournalError> {
        if self.journal_version != JOURNAL_VERSION
            || self.generation != generation
            || self.operation_id != binding.operation_id
            || self.inspect_review_sha256 != binding.inspect_review_sha256
        {
            return Err(ApplyJournalError::BindingMismatch);
        }
        if self.state != event.state
            || self.checkpoint != event.checkpoint
            || self.outcome_code != event.outcome_code
            || self.previous_event_sha256 != event.previous_event_sha256
            || self.event_sha256 != event.event_sha256
            || self.installation_id != event.installation_id
            || self.backup_restore_proof_sha256 != event.backup_restore_proof_sha256
            || self.backup_reference_sha256 != event.backup_reference_sha256
            || self.final_recheck_report_sha256 != event.final_recheck_report_sha256
            || self.source_fingerprint_sha256 != event.source_fingerprint_sha256
            || self.checkpoint_proof_sha256 != event.checkpoint_proof_sha256
            || self.native_authority_nodes_generation != event.native_authority_nodes_generation
            || self.native_authority_nodes_event_sha256 != event.native_authority_nodes_event_sha256
            || self.data_verification_report_sha256 != event.data_verification_report_sha256
            || self.analytics_projection_report_sha256 != event.analytics_projection_report_sha256
            || self.node_cutover_report_sha256 != event.node_cutover_report_sha256
            || !is_lower_hex(&self.head_sha256, 64)
            || self.calculate_hash()? != self.head_sha256
        {
            return Err(ApplyJournalError::Corrupt("CAS head mismatch"));
        }
        Ok(())
    }
}

impl ApplyJournalSnapshot {
    fn from_event(event: &JournalEventRecord) -> Self {
        Self {
            binding: ApplyJournalBinding {
                operation_id: event.operation_id.clone(),
                inspect_review_sha256: event.inspect_review_sha256.clone(),
            },
            generation: event.generation,
            state: event.state,
            checkpoint: event.checkpoint,
            outcome_code: event.outcome_code,
            previous_event_sha256: event.previous_event_sha256.clone(),
            event_sha256: event.event_sha256.clone(),
            recorded_at_unix_ms: event.recorded_at_unix_ms,
            resume_state: event.resume_state,
            installation_id: event.installation_id.clone(),
            backup_restore_proof_sha256: event.backup_restore_proof_sha256.clone(),
            backup_reference_sha256: event.backup_reference_sha256.clone(),
            final_recheck_report_sha256: event.final_recheck_report_sha256.clone(),
            source_fingerprint_sha256: event.source_fingerprint_sha256.clone(),
            checkpoint_proof_sha256: event.checkpoint_proof_sha256.clone(),
            native_authority_nodes_generation: event.native_authority_nodes_generation,
            native_authority_nodes_event_sha256: event.native_authority_nodes_event_sha256.clone(),
            data_verification_report_sha256: event.data_verification_report_sha256.clone(),
            analytics_projection_report_sha256: event.analytics_projection_report_sha256.clone(),
            node_cutover_report_sha256: event.node_cutover_report_sha256.clone(),
        }
    }
}

fn validate_transition(
    current: &ApplyJournalSnapshot,
    next_state: ApplyJournalState,
    checkpoint: ApplyCheckpoint,
    outcome_code: Option<ApplyOutcomeCode>,
    requested: &TransitionBindings,
) -> Result<ValidatedTransition, ApplyJournalError> {
    if matches!(
        current.state,
        ApplyJournalState::Failed | ApplyJournalState::Completed
    ) {
        return Err(ApplyJournalError::TerminalState(current.state));
    }
    let exceptional = matches!(
        next_state,
        ApplyJournalState::NeedsRecovery | ApplyJournalState::Failed
    );
    if exceptional != outcome_code.is_some() {
        return Err(ApplyJournalError::OutcomeCodeMismatch);
    }
    if checkpoint < current.checkpoint {
        return Err(ApplyJournalError::CheckpointRegression {
            current: current.checkpoint,
            requested: checkpoint,
        });
    }

    let resume_state = match (current.state, next_state) {
        (ApplyJournalState::Pending, ApplyJournalState::Running)
            if checkpoint == current.checkpoint =>
        {
            None
        }
        (ApplyJournalState::Pending, ApplyJournalState::NeedsRecovery)
            if checkpoint == current.checkpoint =>
        {
            Some(ApplyJournalState::Running)
        }
        (ApplyJournalState::Pending, ApplyJournalState::Failed)
            if checkpoint == current.checkpoint =>
        {
            None
        }
        (ApplyJournalState::Running, ApplyJournalState::Running)
            if current.checkpoint.successor() == Some(checkpoint) =>
        {
            None
        }
        (ApplyJournalState::Running, ApplyJournalState::Verifying)
            if checkpoint == current.checkpoint
                && checkpoint >= ApplyCheckpoint::PostgresBulkCopied =>
        {
            None
        }
        (ApplyJournalState::Running, ApplyJournalState::NeedsRecovery)
            if checkpoint == current.checkpoint =>
        {
            Some(ApplyJournalState::Running)
        }
        (ApplyJournalState::Running, ApplyJournalState::Failed)
            if checkpoint == current.checkpoint =>
        {
            None
        }
        (ApplyJournalState::Verifying, ApplyJournalState::Verifying)
            if current.checkpoint.successor() == Some(checkpoint) =>
        {
            None
        }
        (ApplyJournalState::Verifying, ApplyJournalState::NeedsRecovery)
            if checkpoint == current.checkpoint =>
        {
            Some(ApplyJournalState::Verifying)
        }
        (ApplyJournalState::Verifying, ApplyJournalState::Failed)
            if checkpoint == current.checkpoint =>
        {
            None
        }
        (ApplyJournalState::Verifying, ApplyJournalState::Completed)
            if current.checkpoint == ApplyCheckpoint::SourceRetired
                && checkpoint == ApplyCheckpoint::CompletionVerified =>
        {
            None
        }
        (ApplyJournalState::NeedsRecovery, ApplyJournalState::Running)
            if current.resume_state == Some(ApplyJournalState::Running)
                && checkpoint == current.checkpoint =>
        {
            None
        }
        (ApplyJournalState::NeedsRecovery, ApplyJournalState::Verifying)
            if current.resume_state == Some(ApplyJournalState::Verifying)
                && checkpoint == current.checkpoint =>
        {
            None
        }
        (ApplyJournalState::NeedsRecovery, ApplyJournalState::Failed)
            if checkpoint == current.checkpoint =>
        {
            None
        }
        _ => {
            return Err(ApplyJournalError::IllegalTransition {
                from: current.state,
                to: next_state,
            });
        }
    };
    let installation_id = match (
        current.installation_id.as_deref(),
        checkpoint,
        requested.installation_id.as_deref(),
    ) {
        (None, ApplyCheckpoint::InstallationIdentityReserved, Some(value)) => {
            let value =
                Uuid::parse_str(value).map_err(|_| ApplyJournalError::InvalidInstallationId)?;
            if value.is_nil() {
                return Err(ApplyJournalError::InvalidInstallationId);
            }
            Some(value.hyphenated().to_string())
        }
        (None, checkpoint, None) if checkpoint < ApplyCheckpoint::InstallationIdentityReserved => {
            None
        }
        (Some(current), _, None) => Some(current.to_string()),
        (Some(current), _, Some(requested)) if current == requested => Some(current.to_string()),
        (None, _, None) => return Err(ApplyJournalError::InstallationBindingRequired),
        _ => return Err(ApplyJournalError::InstallationBindingMismatch),
    };
    let backup_restore_proof_sha256 = advance_proof_binding(
        current.backup_restore_proof_sha256.as_deref(),
        checkpoint,
        ApplyCheckpoint::BackupRestoreVerified,
        requested.backup_restore_proof_sha256.as_deref(),
        ApplyJournalError::BackupProofBindingRequired,
    )?;
    let backup_reference_sha256 = advance_proof_binding(
        current.backup_reference_sha256.as_deref(),
        checkpoint,
        ApplyCheckpoint::BackupRestoreVerified,
        requested.backup_reference_sha256.as_deref(),
        ApplyJournalError::BackupReferenceBindingRequired,
    )?;
    let final_recheck_report_sha256 = advance_proof_binding(
        current.final_recheck_report_sha256.as_deref(),
        checkpoint,
        ApplyCheckpoint::FinalRecheckPassed,
        requested.final_recheck_report_sha256.as_deref(),
        ApplyJournalError::FinalRecheckBindingRequired,
    )?;
    let source_fingerprint_sha256 = advance_proof_binding(
        current.source_fingerprint_sha256.as_deref(),
        checkpoint,
        ApplyCheckpoint::FinalRecheckPassed,
        requested.source_fingerprint_sha256.as_deref(),
        ApplyJournalError::SourceFingerprintBindingRequired,
    )?;
    if backup_restore_proof_sha256.is_some() != backup_reference_sha256.is_some()
        || final_recheck_report_sha256.is_some() != source_fingerprint_sha256.is_some()
    {
        return Err(ApplyJournalError::EvidenceBindingMismatch);
    }
    let checkpoint_proof_sha256 = advance_checkpoint_proof(current, checkpoint, requested)?;
    if (checkpoint == ApplyCheckpoint::BackupRestoreVerified
        && checkpoint_proof_sha256.as_deref() != backup_restore_proof_sha256.as_deref())
        || (checkpoint == ApplyCheckpoint::FinalRecheckPassed
            && checkpoint_proof_sha256.as_deref() != final_recheck_report_sha256.as_deref())
    {
        return Err(ApplyJournalError::EvidenceBindingMismatch);
    }
    let native_authority = advance_native_authority_binding(current, checkpoint, requested)?;
    if checkpoint == ApplyCheckpoint::NativeAuthorityCommitted
        && checkpoint_proof_sha256.as_deref()
            != native_authority
                .as_ref()
                .map(native_authority_proof_sha256)
                .as_deref()
    {
        return Err(ApplyJournalError::EvidenceBindingMismatch);
    }
    Ok(ValidatedTransition {
        resume_state,
        installation_id,
        backup_restore_proof_sha256,
        backup_reference_sha256,
        final_recheck_report_sha256,
        source_fingerprint_sha256,
        checkpoint_proof_sha256,
        native_authority_nodes_generation: native_authority
            .as_ref()
            .map(NativeAuthorityBinding::nodes_verified_generation),
        native_authority_nodes_event_sha256: native_authority
            .as_ref()
            .map(|binding| binding.nodes_verified_event_sha256.clone()),
        data_verification_report_sha256: native_authority
            .as_ref()
            .map(|binding| binding.data_verification_report_sha256.clone()),
        analytics_projection_report_sha256: native_authority
            .as_ref()
            .map(|binding| binding.analytics_projection_report_sha256.clone()),
        node_cutover_report_sha256: native_authority
            .map(|binding| binding.node_cutover_report_sha256),
    })
}

fn advance_checkpoint_proof(
    current: &ApplyJournalSnapshot,
    checkpoint: ApplyCheckpoint,
    requested: &TransitionBindings,
) -> Result<Option<String>, ApplyJournalError> {
    let requested = requested.checkpoint_proof_sha256.as_deref();
    if let Some(value) = requested
        && !is_lower_hex(value, 64)
    {
        return Err(ApplyJournalError::InvalidCheckpointProofHash);
    }
    if checkpoint == current.checkpoint {
        return match (current.checkpoint_proof_sha256(), requested) {
            (None, None) => Ok(None),
            (Some(current), None) => Ok(Some(current.to_string())),
            (Some(current), Some(requested)) if current == requested => {
                Ok(Some(current.to_string()))
            }
            _ => Err(ApplyJournalError::EvidenceBindingMismatch),
        };
    }
    if checkpoint == ApplyCheckpoint::InstallationIdentityReserved && requested.is_none() {
        return Ok(None);
    }
    requested
        .map(|value| Some(value.to_string()))
        .ok_or(ApplyJournalError::CheckpointProofBindingRequired)
}

fn latest_snapshot_checkpoint_proof(
    history: &[ApplyJournalSnapshot],
    through_generation: u64,
    checkpoint: ApplyCheckpoint,
) -> Option<&str> {
    history
        .iter()
        .rev()
        .find(|snapshot| {
            snapshot.generation() <= through_generation
                && snapshot.state() == ApplyJournalState::Verifying
                && snapshot.checkpoint() == checkpoint
                && snapshot.outcome_code().is_none()
        })
        .and_then(ApplyJournalSnapshot::checkpoint_proof_sha256)
}

fn advance_native_authority_binding(
    current: &ApplyJournalSnapshot,
    checkpoint: ApplyCheckpoint,
    requested: &TransitionBindings,
) -> Result<Option<NativeAuthorityBinding>, ApplyJournalError> {
    let current = current.native_authority_binding();
    let requested = native_authority_binding_from_fields(
        requested.native_authority_nodes_generation,
        requested.native_authority_nodes_event_sha256.as_deref(),
        requested.data_verification_report_sha256.as_deref(),
        requested.analytics_projection_report_sha256.as_deref(),
        requested.node_cutover_report_sha256.as_deref(),
    )?;
    match (current, requested, checkpoint) {
        (None, None, checkpoint) if checkpoint < ApplyCheckpoint::NativeAuthorityCommitted => {
            Ok(None)
        }
        (None, Some(requested), ApplyCheckpoint::NativeAuthorityCommitted) => Ok(Some(requested)),
        (Some(current), None, _) => Ok(Some(current)),
        (Some(current), Some(requested), _) if current == requested => Ok(Some(current)),
        (None, None, _) => Err(ApplyJournalError::NativeAuthorityBindingRequired),
        _ => Err(ApplyJournalError::EvidenceBindingMismatch),
    }
}

fn native_authority_binding_from_fields(
    nodes_verified_generation: Option<u64>,
    nodes_verified_event_sha256: Option<&str>,
    data_verification_report_sha256: Option<&str>,
    analytics_projection_report_sha256: Option<&str>,
    node_cutover_report_sha256: Option<&str>,
) -> Result<Option<NativeAuthorityBinding>, ApplyJournalError> {
    match (
        nodes_verified_generation,
        nodes_verified_event_sha256,
        data_verification_report_sha256,
        analytics_projection_report_sha256,
        node_cutover_report_sha256,
    ) {
        (None, None, None, None, None) => Ok(None),
        (Some(generation), Some(event), Some(data), Some(analytics), Some(nodes)) => {
            NativeAuthorityBinding::new(generation, event, data, analytics, nodes).map(Some)
        }
        _ => Err(ApplyJournalError::InvalidNativeAuthorityBinding),
    }
}

fn validate_native_authority_anchor_record(
    event: &JournalEventRecord,
    history: &[JournalEventRecord],
) -> Result<(), ApplyJournalError> {
    let authority = native_authority_binding_from_fields(
        event.native_authority_nodes_generation,
        event.native_authority_nodes_event_sha256.as_deref(),
        event.data_verification_report_sha256.as_deref(),
        event.analytics_projection_report_sha256.as_deref(),
        event.node_cutover_report_sha256.as_deref(),
    )?;
    if event.checkpoint < ApplyCheckpoint::NativeAuthorityCommitted {
        return if authority.is_none() {
            Ok(())
        } else {
            Err(ApplyJournalError::NativeAuthorityAnchorMismatch)
        };
    }
    let authority = authority.ok_or(ApplyJournalError::NativeAuthorityBindingRequired)?;
    let anchor = usize::try_from(authority.nodes_verified_generation)
        .ok()
        .and_then(|generation| history.get(generation))
        .ok_or(ApplyJournalError::NativeAuthorityAnchorMismatch)?;
    if anchor.generation >= event.generation
        || anchor.state != ApplyJournalState::Verifying
        || anchor.checkpoint != ApplyCheckpoint::NodesVerified
        || anchor.outcome_code.is_some()
        || anchor.event_sha256 != authority.nodes_verified_event_sha256
        || anchor.native_authority_nodes_generation.is_some()
        || anchor.native_authority_nodes_event_sha256.is_some()
        || anchor.data_verification_report_sha256.is_some()
        || anchor.analytics_projection_report_sha256.is_some()
        || anchor.node_cutover_report_sha256.is_some()
        || anchor.installation_id != event.installation_id
        || anchor.backup_restore_proof_sha256 != event.backup_restore_proof_sha256
        || anchor.backup_reference_sha256 != event.backup_reference_sha256
        || anchor.final_recheck_report_sha256 != event.final_recheck_report_sha256
        || anchor.source_fingerprint_sha256 != event.source_fingerprint_sha256
        || anchor.checkpoint_proof_sha256.as_deref() != Some(authority.node_cutover_report_sha256())
        || latest_record_checkpoint_proof(
            history,
            authority.nodes_verified_generation,
            ApplyCheckpoint::PostgresValueVerified,
        ) != Some(authority.data_verification_report_sha256())
        || latest_record_checkpoint_proof(
            history,
            authority.nodes_verified_generation,
            ApplyCheckpoint::ClickhouseProjected,
        ) != Some(authority.analytics_projection_report_sha256())
    {
        return Err(ApplyJournalError::NativeAuthorityAnchorMismatch);
    }
    Ok(())
}

fn latest_record_checkpoint_proof(
    history: &[JournalEventRecord],
    through_generation: u64,
    checkpoint: ApplyCheckpoint,
) -> Option<&str> {
    history
        .iter()
        .rev()
        .find(|event| {
            event.generation <= through_generation
                && event.state == ApplyJournalState::Verifying
                && event.checkpoint == checkpoint
                && event.outcome_code.is_none()
        })
        .and_then(|event| event.checkpoint_proof_sha256.as_deref())
}

/// Canonical digest stored as the `native_authority_committed` checkpoint
/// proof. Exact report hashes remain separately available; this digest prevents
/// callers from substituting a different anchor/report tuple as one receipt.
pub fn native_authority_proof_sha256(binding: &NativeAuthorityBinding) -> String {
    let mut digest = Sha256::new();
    digest.update(NATIVE_AUTHORITY_PROOF_DOMAIN);
    digest.update(binding.nodes_verified_generation.to_be_bytes());
    digest.update(binding.nodes_verified_event_sha256.as_bytes());
    digest.update(binding.data_verification_report_sha256.as_bytes());
    digest.update(binding.analytics_projection_report_sha256.as_bytes());
    digest.update(binding.node_cutover_report_sha256.as_bytes());
    hex::encode(digest.finalize())
}

fn advance_proof_binding(
    current: Option<&str>,
    checkpoint: ApplyCheckpoint,
    binding_checkpoint: ApplyCheckpoint,
    requested: Option<&str>,
    missing_error: ApplyJournalError,
) -> Result<Option<String>, ApplyJournalError> {
    match (current, checkpoint, requested) {
        (None, checkpoint, None) if checkpoint < binding_checkpoint => Ok(None),
        (None, checkpoint, Some(value)) if checkpoint == binding_checkpoint => {
            if !is_lower_hex(value, 64) {
                return Err(ApplyJournalError::EvidenceBindingMismatch);
            }
            Ok(Some(value.to_string()))
        }
        (Some(current), _, None) => Ok(Some(current.to_string())),
        (Some(current), _, Some(requested)) if current == requested => {
            Ok(Some(current.to_string()))
        }
        (None, _, None) => Err(missing_error),
        _ => Err(ApplyJournalError::EvidenceBindingMismatch),
    }
}

fn checked_sha256(value: &str, error: ApplyJournalError) -> Result<String, ApplyJournalError> {
    if !is_lower_hex(value, 64) {
        return Err(error);
    }
    Ok(value.to_string())
}

/// Returns the only canonical, non-secret backup reference binding accepted by
/// the one-shot journal and PostgreSQL lifecycle ledger.
pub fn backup_reference_sha256(reference: &str) -> Result<String, ApplyJournalError> {
    if reference.is_empty()
        || reference.len() > 512
        || reference.trim() != reference
        || !reference.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
    {
        return Err(ApplyJournalError::InvalidBackupReference);
    }
    let length =
        u64::try_from(reference.len()).map_err(|_| ApplyJournalError::InvalidBackupReference)?;
    let mut digest = Sha256::new();
    digest.update(BACKUP_REFERENCE_HASH_DOMAIN);
    digest.update(length.to_be_bytes());
    digest.update(reference.as_bytes());
    Ok(hex::encode(digest.finalize()))
}

fn prepare_journal_root(path: &Path, create: bool) -> Result<(), ApplyJournalError> {
    if !path.is_absolute() {
        return Err(ApplyJournalError::UnsafePath(
            "journal root must be absolute",
        ));
    }
    match fs::symlink_metadata(path) {
        Ok(_) => validate_private_dir(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
            let parent = path
                .parent()
                .ok_or(ApplyJournalError::UnsafePath("journal root has no parent"))?;
            validate_directory_type(parent)?;
            create_private_dir(path)?;
            sync_dir(parent)?;
            validate_private_dir(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(ApplyJournalError::OperationNotFound)
        }
        Err(error) => Err(ApplyJournalError::io("inspect journal root", error)),
    }
}

fn ensure_private_dir(path: &Path) -> Result<(), ApplyJournalError> {
    match fs::symlink_metadata(path) {
        Ok(_) => validate_private_dir(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(ApplyJournalError::UnsafePath(
                "journal directory has no parent",
            ))?;
            validate_private_dir(parent)?;
            create_private_dir(path)?;
            sync_dir(parent)?;
            validate_private_dir(path)
        }
        Err(error) => Err(ApplyJournalError::io("inspect journal directory", error)),
    }
}

fn create_private_dir(path: &Path) -> Result<(), ApplyJournalError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => validate_private_dir(path),
        Err(error) => Err(ApplyJournalError::io("create journal directory", error)),
    }
}

fn validate_directory_type(path: &Path) -> Result<(), ApplyJournalError> {
    validate_real_directory_chain(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| ApplyJournalError::io("inspect directory", source))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(ApplyJournalError::UnsafePath(
            "journal path component is not a real directory",
        ));
    }
    Ok(())
}

fn validate_private_dir(path: &Path) -> Result<(), ApplyJournalError> {
    validate_real_directory_chain(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| ApplyJournalError::io("inspect private directory", source))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(ApplyJournalError::UnsafePath(
            "journal path component is not a real directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 || mode & 0o700 != 0o700 {
            return Err(ApplyJournalError::UnsafePermissions);
        }
    }
    Ok(())
}

fn validate_real_directory_chain(path: &Path) -> Result<(), ApplyJournalError> {
    use std::path::Component;

    if !path.is_absolute() {
        return Err(ApplyJournalError::UnsafePath(
            "journal directory chain must be absolute",
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) => {
                current.push(component.as_os_str());
            }
            Component::Normal(value) => {
                current.push(value);
                let metadata = fs::symlink_metadata(&current).map_err(|source| {
                    ApplyJournalError::io("inspect journal directory chain", source)
                })?;
                if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                    return Err(ApplyJournalError::UnsafePath(
                        "journal directory chain contains a symlink or non-directory",
                    ));
                }
            }
            Component::CurDir | Component::ParentDir => {
                return Err(ApplyJournalError::UnsafePath(
                    "journal directory chain is not normalized",
                ));
            }
        }
    }
    Ok(())
}

fn validate_private_file(path: &Path) -> Result<fs::Metadata, ApplyJournalError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|source| ApplyJournalError::io("inspect journal record", source))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.file_type().is_file() {
        return Err(ApplyJournalError::UnsafePath(
            "journal record is not a regular non-symlink file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = path_metadata.permissions().mode();
        if mode & 0o077 != 0 || mode & 0o600 != 0o600 {
            return Err(ApplyJournalError::UnsafePermissions);
        }
    }
    let len = path_metadata.len();
    if len == 0 || len > MAX_RECORD_BYTES {
        return Err(ApplyJournalError::Corrupt(
            "journal record has an unsafe size",
        ));
    }
    Ok(path_metadata)
}

fn list_record_files(dir: &Path) -> Result<BTreeMap<u64, PathBuf>, ApplyJournalError> {
    validate_private_dir(dir)?;
    let mut records = BTreeMap::new();
    for entry in fs::read_dir(dir)
        .map_err(|source| ApplyJournalError::io("read journal directory", source))?
    {
        let entry = entry.map_err(|source| ApplyJournalError::io("read journal entry", source))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ApplyJournalError::Corrupt("non-UTF-8 journal entry"))?;
        if name.starts_with(".tmp-") {
            // A temp inode is never authoritative. It may remain after a crash,
            // while its published hard link is independently validated below.
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|source| ApplyJournalError::io("inspect journal temp entry", source))?;
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(ApplyJournalError::UnsafePath(
                    "journal temp entry is not a regular non-symlink file",
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o077 != 0 {
                    return Err(ApplyJournalError::UnsafePermissions);
                }
            }
            continue;
        }
        let generation = parse_record_file_name(&name)
            .ok_or(ApplyJournalError::Corrupt("unexpected journal entry"))?;
        if records.insert(generation, entry.path()).is_some() {
            return Err(ApplyJournalError::Corrupt("duplicate journal generation"));
        }
        if records.len() > MAX_RECORDS {
            return Err(ApplyJournalError::Corrupt(
                "journal exceeds the bounded generation limit",
            ));
        }
    }
    Ok(records)
}

fn ensure_contiguous(records: &BTreeMap<u64, PathBuf>) -> Result<(), ApplyJournalError> {
    for (expected, actual) in (0_u64..).zip(records.keys().copied()) {
        if expected != actual {
            return Err(ApplyJournalError::Corrupt(
                "journal generations are not contiguous",
            ));
        }
    }
    Ok(())
}

fn read_json_record<T: for<'de> Deserialize<'de>>(
    path: &Path,
    record_kind: &'static str,
) -> Result<T, ApplyJournalError> {
    let path_metadata = validate_private_file(path)?;
    let expected_len = path_metadata.len();
    let mut file =
        File::open(path).map_err(|source| ApplyJournalError::io("open journal record", source))?;
    let file_metadata = file
        .metadata()
        .map_err(|source| ApplyJournalError::io("inspect open journal record", source))?;
    if !file_metadata.file_type().is_file() || file_metadata.len() != expected_len {
        return Err(ApplyJournalError::Corrupt(
            "journal record changed while opening",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if path_metadata.dev() != file_metadata.dev() || path_metadata.ino() != file_metadata.ino()
        {
            return Err(ApplyJournalError::Corrupt(
                "journal record inode changed while opening",
            ));
        }
    }
    let mut bytes = Vec::with_capacity(expected_len as usize);
    Read::by_ref(&mut file)
        .take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| ApplyJournalError::io("read journal record", source))?;
    if bytes.len() as u64 != expected_len {
        return Err(ApplyJournalError::Corrupt(
            "journal record changed while reading",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|source| ApplyJournalError::json(record_kind, source))
}

fn durable_publish(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), ApplyJournalError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(ApplyJournalError::Corrupt(
            "serialized journal record has an unsafe size",
        ));
    }
    validate_private_dir(dir)?;
    let destination = dir.join(name);
    let (temporary, mut file) = create_private_temp_file(dir)?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|source| ApplyJournalError::io("write journal temp record", source))?;
        file.write_all(b"\n")
            .map_err(|source| ApplyJournalError::io("terminate journal temp record", source))?;
        file.sync_all()
            .map_err(|source| ApplyJournalError::io("fsync journal temp record", source))?;
        drop(file);
        match fs::hard_link(&temporary, &destination) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(ApplyJournalError::ConcurrentUpdate);
            }
            Err(error) => {
                return Err(ApplyJournalError::io(
                    "publish immutable journal record",
                    error,
                ));
            }
        }
        fs::remove_file(&temporary)
            .map_err(|source| ApplyJournalError::io("remove journal temp link", source))?;
        sync_dir(dir)
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_private_temp_file(dir: &Path) -> Result<(PathBuf, File), ApplyJournalError> {
    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(".tmp-{}-{sequence:016x}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ApplyJournalError::io("create journal temp record", error));
            }
        }
    }
    Err(ApplyJournalError::Corrupt(
        "cannot allocate a unique journal temp record",
    ))
}

fn sync_dir(path: &Path) -> Result<(), ApplyJournalError> {
    let directory = File::open(path)
        .map_err(|source| ApplyJournalError::io("open journal directory for fsync", source))?;
    directory
        .sync_all()
        .map_err(|source| ApplyJournalError::io("fsync journal directory", source))
}

fn now_unix_ms() -> Result<u64, ApplyJournalError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApplyJournalError::ClockBeforeUnixEpoch)?
        .as_millis();
    u64::try_from(millis).map_err(|_| ApplyJournalError::ClockOverflow)
}

fn record_file_name(generation: u64) -> String {
    format!("{generation:020}.json")
}

fn parse_record_file_name(name: &str) -> Option<u64> {
    let digits = name.strip_suffix(".json")?;
    (digits.len() == RECORD_NAME_DIGITS && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyJournalError {
    #[error("operation_id must be a non-zero UUID")]
    InvalidOperationId,
    #[error("inspect report SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidInspectReportHash,
    #[error("installation_id must be a non-zero UUID")]
    InvalidInstallationId,
    #[error("backup/restore proof SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidBackupProofHash,
    #[error("backup reference SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidBackupReferenceHash,
    #[error("backup reference must be a canonical bounded non-secret opaque identifier")]
    InvalidBackupReference,
    #[error("final recheck report SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidFinalRecheckHash,
    #[error("source fingerprint SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidSourceFingerprintHash,
    #[error("checkpoint proof SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidCheckpointProofHash,
    #[error("unsafe apply journal path: {0}")]
    UnsafePath(&'static str),
    #[error("apply journal directories and records must be owner-only")]
    UnsafePermissions,
    #[error("apply journal operation was not found")]
    OperationNotFound,
    #[error("apply journal operation already exists")]
    AlreadyExists,
    #[error("apply journal has no durable pending event")]
    Uninitialized,
    #[error("apply journal binding does not match operation_id and inspect report SHA-256")]
    BindingMismatch,
    #[error("apply journal was concurrently advanced or the expected CAS head is stale")]
    ConcurrentUpdate,
    #[error("apply journal contains an event whose CAS head still requires recovery")]
    OrphanEvent,
    #[error("apply journal is corrupt: {0}")]
    Corrupt(&'static str),
    #[error("illegal apply journal transition from {from:?} to {to:?}")]
    IllegalTransition {
        from: ApplyJournalState,
        to: ApplyJournalState,
    },
    #[error("apply journal state {0:?} is terminal")]
    TerminalState(ApplyJournalState),
    #[error("apply journal state {0:?} cannot be resumed")]
    NotRecoverable(ApplyJournalState),
    #[error("apply journal state {0:?} does not authorize a protected mutation")]
    MutationNotAuthorized(ApplyJournalState),
    #[error("target mutation requires a durable final recheck and installation binding")]
    TargetMutationNotAuthorized,
    #[error("target bootstrap requires a durable installation UUID binding")]
    InstallationBindingRequired,
    #[error("backup/restore verification requires a durable proof SHA-256 binding")]
    BackupProofBindingRequired,
    #[error("backup/restore verification requires a durable backup-reference SHA-256 binding")]
    BackupReferenceBindingRequired,
    #[error("target mutation requires a durable fenced final-recheck report SHA-256 binding")]
    FinalRecheckBindingRequired,
    #[error("target mutation requires a durable source-fingerprint SHA-256 binding")]
    SourceFingerprintBindingRequired,
    #[error(
        "native authority proof must contain one NodesVerified anchor and three SHA-256 reports"
    )]
    InvalidNativeAuthorityBinding,
    #[error("native authority proof does not anchor a clean NodesVerified event in this history")]
    NativeAuthorityAnchorMismatch,
    #[error("native authority checkpoint requires the durable PostgreSQL activation binding")]
    NativeAuthorityBindingRequired,
    #[error("native service start requires the current clean NativeAuthorityCommitted head")]
    NativeStartNotAuthorized,
    #[error("this checkpoint requires its dedicated evidence-binding method")]
    SpecialCheckpointMethodRequired,
    #[error("business checkpoint advancement requires a durable verification report SHA-256")]
    CheckpointProofBindingRequired,
    #[error("apply journal installation UUID cannot change once reserved")]
    InstallationBindingMismatch,
    #[error("apply journal proof binding is invalid or changed after it was recorded")]
    EvidenceBindingMismatch,
    #[error("apply journal checkpoint regressed from {current:?} to {requested:?}")]
    CheckpointRegression {
        current: ApplyCheckpoint,
        requested: ApplyCheckpoint,
    },
    #[error(
        "failed and needs-recovery events require one bounded outcome code, and normal events forbid it"
    )]
    OutcomeCodeMismatch,
    #[error("apply journal generation overflow")]
    GenerationOverflow,
    #[error("system clock is before the Unix epoch")]
    ClockBeforeUnixEpoch,
    #[error("system clock does not fit the journal timestamp")]
    ClockOverflow,
    #[error("apply journal I/O failed while attempting to {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("apply journal JSON failed while attempting to {action}: {source}")]
    Json {
        action: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

impl ApplyJournalError {
    fn io(action: &'static str, source: io::Error) -> Self {
        Self::Io { action, source }
    }

    fn json(action: &'static str, source: serde_json::Error) -> Self {
        Self::Json { action, source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    const REPORT_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const BACKUP_PROOF_HASH: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BACKUP_REFERENCE_HASH: &str =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const FINAL_RECHECK_HASH: &str =
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SOURCE_FINGERPRINT_HASH: &str =
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const DATA_VERIFICATION_HASH: &str =
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const ANALYTICS_PROJECTION_HASH: &str =
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const NODE_CUTOVER_HASH: &str =
        "1111111111111111111111111111111111111111111111111111111111111111";
    const OPERATION_ID: &str = "40aa4a80-eb4b-4b25-9c3b-e17ed047873d";

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "v2board-apply-journal-test-{}-{}",
                std::process::id(),
                TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn binding() -> ApplyJournalBinding {
        ApplyJournalBinding::new(OPERATION_ID, REPORT_HASH).expect("valid binding")
    }

    fn create() -> (TestRoot, ApplyJournal, ApplyJournalSnapshot) {
        let root = TestRoot::new();
        let (journal, snapshot) =
            ApplyJournal::create_pending(root.path(), binding()).expect("pending journal");
        (root, journal, snapshot)
    }

    #[test]
    fn pending_is_durable_bound_and_secret_free() {
        let (root, journal, pending) = create();
        assert_eq!(pending.state(), ApplyJournalState::Pending);
        assert_eq!(pending.checkpoint(), ApplyCheckpoint::PendingDurable);
        assert_eq!(pending.generation(), 0);
        assert_eq!(pending.binding(), &binding());
        assert_eq!(pending.event_sha256().len(), 64);

        let permit = journal
            .mutation_permit(&pending)
            .expect("durable pending authorizes first mutation");
        assert_eq!(permit.operation_id(), OPERATION_ID);
        assert_eq!(permit.inspect_review_sha256(), REPORT_HASH);

        let (_, reopened) = ApplyJournal::open(root.path(), binding()).expect("reopen journal");
        assert_eq!(reopened, pending);
        let serialized = serde_json::to_string(&reopened).expect("serialize snapshot");
        for forbidden in [
            "password",
            "secret",
            "database_url",
            "redis_url",
            "clickhouse_url",
            "error_message",
        ] {
            assert!(!serialized.contains(forbidden), "leaked field {forbidden}");
        }
    }

    #[test]
    fn backup_reference_hash_is_canonical_domain_separated_and_secret_safe() {
        let first = backup_reference_sha256("backup:legacy/snapshot-1").expect("reference hash");
        let second = backup_reference_sha256("backup:legacy/snapshot-1").expect("same reference");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert_ne!(
            first,
            hex::encode(Sha256::digest(b"backup:legacy/snapshot-1"))
        );
        assert!(backup_reference_sha256(" backup:legacy/snapshot-1").is_err());
        assert!(backup_reference_sha256("s3://user:secret@example/snapshot").is_err());
    }

    #[test]
    fn rejects_wrong_operation_and_report_binding() {
        let (root, journal, _) = create();
        let wrong_report = ApplyJournalBinding::new(
            OPERATION_ID,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect("valid alternate report");
        let wrong_report_journal = ApplyJournal {
            operation_dir: journal.operation_dir.clone(),
            events_dir: journal.events_dir.clone(),
            heads_dir: journal.heads_dir.clone(),
            binding: wrong_report,
        };
        assert!(matches!(
            wrong_report_journal.reload(),
            Err(ApplyJournalError::BindingMismatch)
        ));

        let wrong_operation =
            ApplyJournalBinding::new("e0bb60eb-bb45-4393-8a04-18a3aa510497", REPORT_HASH)
                .expect("valid alternate operation");
        assert!(matches!(
            ApplyJournal::open(root.path(), wrong_operation),
            Err(ApplyJournalError::OperationNotFound) | Err(ApplyJournalError::Io { .. })
        ));
    }

    #[test]
    fn invalid_transitions_and_completed_resume_fail_closed() {
        let (_root, journal, pending) = create();
        assert!(matches!(
            journal.complete(&pending, REPORT_HASH),
            Err(ApplyJournalError::IllegalTransition { .. })
        ));

        let mut current = journal.begin(&pending).expect("begin");
        assert!(matches!(
            journal.checkpoint(&current, ApplyCheckpoint::SourceDrained),
            Err(ApplyJournalError::IllegalTransition { .. })
        ));
        for checkpoint in [
            ApplyCheckpoint::MaintenanceFenced,
            ApplyCheckpoint::SourceDrained,
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, REPORT_HASH)
                .expect("advance running checkpoint");
        }
        assert!(matches!(
            journal.checkpoint(&current, ApplyCheckpoint::BackupRestoreVerified),
            Err(ApplyJournalError::BackupProofBindingRequired)
        ));
        current = journal
            .record_backup_restore_verified(&current, BACKUP_PROOF_HASH, BACKUP_REFERENCE_HASH)
            .expect("bind backup/restore proof");
        assert!(matches!(
            journal.checkpoint(&current, ApplyCheckpoint::FinalRecheckPassed),
            Err(ApplyJournalError::FinalRecheckBindingRequired)
        ));
        current = journal
            .record_final_recheck_passed(&current, FINAL_RECHECK_HASH, SOURCE_FINGERPRINT_HASH)
            .expect("bind fenced final recheck");
        assert!(matches!(
            journal.checkpoint(&current, ApplyCheckpoint::TargetsBootstrapped),
            Err(ApplyJournalError::IllegalTransition { .. })
        ));
        assert!(matches!(
            journal.target_mutation_permit(&current),
            Err(ApplyJournalError::TargetMutationNotAuthorized)
        ));
        current = journal
            .reserve_installation_identity(&current, "e0bb60eb-bb45-4393-8a04-18a3aa510497")
            .expect("reserve installation identity");
        let target_permit = journal
            .target_mutation_permit(&current)
            .expect("target mutation permit");
        assert_eq!(
            target_permit.installation_id(),
            "e0bb60eb-bb45-4393-8a04-18a3aa510497"
        );
        assert_eq!(
            target_permit.backup_restore_proof_sha256(),
            BACKUP_PROOF_HASH
        );
        assert_eq!(
            target_permit.backup_reference_sha256(),
            BACKUP_REFERENCE_HASH
        );
        assert_eq!(
            target_permit.final_recheck_report_sha256(),
            FINAL_RECHECK_HASH
        );
        assert_eq!(
            target_permit.source_fingerprint_sha256(),
            SOURCE_FINGERPRINT_HASH
        );
        for checkpoint in [
            ApplyCheckpoint::TargetsBootstrapped,
            ApplyCheckpoint::PostgresBulkCopied,
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, REPORT_HASH)
                .expect("advance target checkpoint");
        }
        current = journal
            .enter_verification(&current)
            .expect("enter verification");
        for (checkpoint, proof) in [
            (
                ApplyCheckpoint::PostgresValueVerified,
                DATA_VERIFICATION_HASH,
            ),
            (
                ApplyCheckpoint::ClickhouseProjected,
                ANALYTICS_PROJECTION_HASH,
            ),
            (ApplyCheckpoint::RuntimeMaterialized, REPORT_HASH),
            (ApplyCheckpoint::NodesVerified, NODE_CUTOVER_HASH),
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, proof)
                .expect("advance verification checkpoint");
        }
        let authority = NativeAuthorityBinding::new(
            current.generation(),
            current.event_sha256(),
            DATA_VERIFICATION_HASH,
            ANALYTICS_PROJECTION_HASH,
            NODE_CUTOVER_HASH,
        )
        .expect("authority binding");
        current = journal
            .mark_needs_recovery(&current, ApplyOutcomeCode::ActivationFailed)
            .expect("persist uncertain activation response");
        current = journal.resume(&current).expect("resume at nodes-verified");
        current = journal
            .record_native_authority_committed(&current, &authority)
            .expect("record earlier committed authority after recovery");
        assert!(matches!(
            journal.target_mutation_permit(&current),
            Err(ApplyJournalError::TargetMutationNotAuthorized)
        ));
        let start_permit = journal
            .native_start_permit(&current)
            .expect("native start permit");
        assert_eq!(start_permit.native_authority_binding(), &authority);
        for checkpoint in [
            ApplyCheckpoint::CutoverCommitted,
            ApplyCheckpoint::SourceRetired,
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, REPORT_HASH)
                .expect("advance post-authority checkpoint");
        }
        let completed = journal.complete(&current, REPORT_HASH).expect("complete");
        assert_eq!(completed.state(), ApplyJournalState::Completed);
        assert!(matches!(
            journal.resume(&completed),
            Err(ApplyJournalError::NotRecoverable(
                ApplyJournalState::Completed
            ))
        ));
        assert!(matches!(
            journal.begin(&completed),
            Err(ApplyJournalError::TerminalState(
                ApplyJournalState::Completed
            ))
        ));
        assert!(matches!(
            journal.mutation_permit(&completed),
            Err(ApplyJournalError::MutationNotAuthorized(
                ApplyJournalState::Completed
            ))
        ));
    }

    #[test]
    fn needs_recovery_resumes_only_the_interrupted_phase() {
        let (_root, journal, pending) = create();
        let running = journal.begin(&pending).expect("begin");
        let recovery = journal
            .mark_needs_recovery(&running, ApplyOutcomeCode::ProcessInterrupted)
            .expect("mark recovery");
        assert!(recovery.can_resume());
        assert!(matches!(
            journal.enter_verification(&recovery),
            Err(ApplyJournalError::IllegalTransition { .. })
        ));
        let resumed = journal.resume(&recovery).expect("resume running");
        assert_eq!(resumed.state(), ApplyJournalState::Running);
        assert_eq!(resumed.checkpoint(), running.checkpoint());
    }

    #[test]
    fn stale_cas_head_loses_a_concurrent_advance() {
        let (_root, journal, pending) = create();
        let first_view = pending.clone();
        let second_view = pending;
        let running = journal.begin(&first_view).expect("first writer wins");
        assert_eq!(running.generation(), 1);
        assert!(matches!(
            journal.begin(&second_view),
            Err(ApplyJournalError::ConcurrentUpdate)
        ));
    }

    #[test]
    fn concurrent_threads_commit_exactly_one_generation() {
        let (_root, journal, pending) = create();
        let journal = std::sync::Arc::new(journal);
        let first_journal = std::sync::Arc::clone(&journal);
        let second_journal = std::sync::Arc::clone(&journal);
        let first = pending.clone();
        let second = pending;
        let (first_result, second_result) = std::thread::scope(|scope| {
            let first = scope.spawn(move || first_journal.begin(&first));
            let second = scope.spawn(move || second_journal.begin(&second));
            (
                first.join().expect("first thread"),
                second.join().expect("second thread"),
            )
        });
        assert_eq!(
            usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
            1
        );
        assert!(matches!(
            first_result.as_ref().err().or(second_result.as_ref().err()),
            Some(ApplyJournalError::ConcurrentUpdate)
        ));
        assert_eq!(journal.reload().expect("current head").generation(), 1);
    }

    #[test]
    fn a_single_durable_orphan_event_recovers_its_exact_head() {
        let (_root, journal, pending) = create();
        let validated = validate_transition(
            &pending,
            ApplyJournalState::Running,
            pending.checkpoint,
            None,
            &TransitionBindings::default(),
        )
        .expect("valid transition");
        let orphan = JournalEventRecord::transition(
            &journal.binding,
            1,
            &pending,
            ApplyJournalState::Running,
            pending.checkpoint,
            None,
            validated,
        )
        .expect("orphan event");
        journal.publish_event(&orphan).expect("publish event only");

        let recovered = journal.reload().expect("recover exact orphan");
        assert_eq!(recovered.generation(), 1);
        assert_eq!(recovered.event_sha256(), orphan.event_sha256);
        assert_eq!(recovered.state(), ApplyJournalState::Running);
    }

    #[test]
    fn corrupted_event_and_permissions_are_rejected() {
        let (_root, journal, _pending) = create();
        let event_path = journal.events_dir.join(record_file_name(0));
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&event_path)
            .expect("open event");
        file.write_all(b"{}\n").expect("corrupt event");
        file.sync_all().expect("fsync corruption");
        assert!(matches!(
            journal.reload(),
            Err(ApplyJournalError::Json { .. })
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&event_path, fs::Permissions::from_mode(0o644))
                .expect("weaken permissions");
            assert!(matches!(
                journal.reload(),
                Err(ApplyJournalError::UnsafePermissions)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_operation_directory_is_rejected() {
        use std::os::unix::fs::{DirBuilderExt, symlink};

        let root = TestRoot::new();
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(root.path()).expect("root");
        let elsewhere = root.path().join("elsewhere");
        builder.mode(0o700).create(&elsewhere).expect("elsewhere");
        symlink(&elsewhere, root.path().join(OPERATION_ID)).expect("operation symlink");
        assert!(matches!(
            ApplyJournal::create_pending(root.path(), binding()),
            Err(ApplyJournalError::UnsafePath(_))
        ));
    }
}
