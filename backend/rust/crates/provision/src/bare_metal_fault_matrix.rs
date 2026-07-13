//! Feature-only control plane for destructive bare-metal fault tests.
//!
//! This module is deliberately absent from normal builds. A matrix guest must
//! install one operation-bound case before it can use the feature-only real
//! executor entry. Hooks are single-use across process restarts: the fsynced
//! ready record is the durable consumed marker.

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    future,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const FAULT_PROTOCOL_VERSION: u32 = 1;
pub const FAULT_READY_FILE: &str = "fault-ready.json";
const CATALOG_DOMAIN: &[u8] = b"v2board-bare-metal-fault-catalog-v1\0";
const CASE_DOMAIN: &[u8] = b"v2board-bare-metal-fault-case-v1\0";

/// Closed set of instrumented operation groups in the production composition.
///
/// A point is placed immediately around the named idempotent composition
/// operation, not around the outer lifecycle checkpoint. Some operations still
/// contain multiple lower-level datastore, receipt, or systemd primitives, so
/// this catalog is an exact statement of runner coverage rather than a claim
/// that every internal power-loss window is covered. Read-only checks and
/// filesystem journal appends are intentionally excluded.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BareMetalFaultPoint {
    SourceFenceCommit,
    SourceDrainCommit,
    BackupArchivePublish,
    ArchiveMaterializationPublish,
    TargetHostBootstrap,
    PostgresSchemaBootstrap,
    LifecycleLedgerBootstrap,
    LifecycleJournalMirror,
    PostgresBulkCopy,
    ClickhouseProjectionCommit,
    ReleaseArtifactReconcile,
    RuntimeConfigInstall,
    NativeAuthorityCommit,
    ArchiveMaterializationDestroy,
    NativeServicesStart,
    NodeActivationCommit,
    SourceRetirementCommit,
    LifecycleCompletionCommit,
    RuntimeDecryptionIdentityDestroy,
}

pub const FAULT_POINTS: &[BareMetalFaultPoint] = &[
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

impl BareMetalFaultPoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceFenceCommit => "source_fence_commit",
            Self::SourceDrainCommit => "source_drain_commit",
            Self::BackupArchivePublish => "backup_archive_publish",
            Self::ArchiveMaterializationPublish => "archive_materialization_publish",
            Self::TargetHostBootstrap => "target_host_bootstrap",
            Self::PostgresSchemaBootstrap => "postgres_schema_bootstrap",
            Self::LifecycleLedgerBootstrap => "lifecycle_ledger_bootstrap",
            Self::LifecycleJournalMirror => "lifecycle_journal_mirror",
            Self::PostgresBulkCopy => "postgres_bulk_copy",
            Self::ClickhouseProjectionCommit => "clickhouse_projection_commit",
            Self::ReleaseArtifactReconcile => "release_artifact_reconcile",
            Self::RuntimeConfigInstall => "runtime_config_install",
            Self::NativeAuthorityCommit => "native_authority_commit",
            Self::ArchiveMaterializationDestroy => "archive_materialization_destroy",
            Self::NativeServicesStart => "native_services_start",
            Self::NodeActivationCommit => "node_activation_commit",
            Self::SourceRetirementCommit => "source_retirement_commit",
            Self::LifecycleCompletionCommit => "lifecycle_completion_commit",
            Self::RuntimeDecryptionIdentityDestroy => "runtime_decryption_identity_destroy",
        }
    }
}

impl fmt::Display for BareMetalFaultPoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BareMetalFaultPoint {
    type Err = FaultMatrixError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        FAULT_POINTS
            .iter()
            .copied()
            .find(|point| point.as_str() == value)
            .ok_or(FaultMatrixError::UnknownFaultPoint)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BareMetalFaultMode {
    Before,
    LostAcknowledgement,
    SigkillReady,
}

pub const FAULT_MODES: &[BareMetalFaultMode] = &[
    BareMetalFaultMode::Before,
    BareMetalFaultMode::LostAcknowledgement,
    BareMetalFaultMode::SigkillReady,
];

impl BareMetalFaultMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::LostAcknowledgement => "lost_acknowledgement",
            Self::SigkillReady => "sigkill_ready",
        }
    }
}

impl fmt::Display for BareMetalFaultMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BareMetalFaultMode {
    type Err = FaultMatrixError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        FAULT_MODES
            .iter()
            .copied()
            .find(|mode| mode.as_str() == value)
            .ok_or(FaultMatrixError::UnknownFaultMode)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BareMetalFaultCase {
    pub point: BareMetalFaultPoint,
    pub mode: BareMetalFaultMode,
}

impl BareMetalFaultCase {
    pub fn id(self) -> String {
        format!("{}--{}", self.point, self.mode)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BareMetalFaultControllerConfig {
    pub operation_id: String,
    pub run_id: String,
    pub control_dir: PathBuf,
    pub case: BareMetalFaultCase,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultHookPhase {
    Before,
    AfterSuccess,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultHookAction {
    InjectedBefore,
    InjectedLostAcknowledgement,
    SigkillReady,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaultReadyRecord {
    pub protocol_version: u32,
    pub catalog_sha256: String,
    pub case_binding_sha256: String,
    pub operation_id: String,
    pub run_id: String,
    pub point: BareMetalFaultPoint,
    pub mode: BareMetalFaultMode,
    pub phase: FaultHookPhase,
    pub action: FaultHookAction,
    pub pid: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum FaultMatrixError {
    #[error("unknown bare-metal fault point")]
    UnknownFaultPoint,
    #[error("unknown bare-metal fault mode")]
    UnknownFaultMode,
    #[error("fault controller binding is invalid")]
    InvalidBinding,
    #[error("another fault controller is already installed")]
    AlreadyInstalled,
    #[error("fault controller is not installed for this operation")]
    NotInstalled,
    #[error("fault ready record conflicts with this case")]
    ReadyRecordConflict,
    #[error("fault ready record I/O failed: {0}")]
    ReadyRecordIo(#[source] io::Error),
    #[error("fault ready record is invalid")]
    ReadyRecordInvalid,
    #[error("injected interruption before the real mutation")]
    InjectedBefore,
    #[error("injected lost acknowledgement after the real mutation")]
    InjectedLostAcknowledgement,
}

impl FaultMatrixError {
    pub const fn sanitized_code(&self) -> &'static str {
        match self {
            Self::InjectedBefore => "matrix_injected_before_mutation",
            Self::InjectedLostAcknowledgement => "matrix_injected_lost_acknowledgement",
            _ => "matrix_fault_controller_invalid",
        }
    }
}

#[derive(Debug)]
struct Controller {
    config: BareMetalFaultControllerConfig,
    case_binding_sha256: String,
    state: AtomicU8,
}

const CONTROLLER_ARMED: u8 = 0;
const CONTROLLER_FIRING: u8 = 1;
const CONTROLLER_CONSUMED: u8 = 2;

static CONTROLLER: Mutex<Option<Arc<Controller>>> = Mutex::new(None);
static CATALOG_SHA256: OnceLock<String> = OnceLock::new();

/// Keeps the process-global controller installed for one guest invocation.
pub struct BareMetalFaultControllerGuard {
    controller: Arc<Controller>,
}

impl Drop for BareMetalFaultControllerGuard {
    fn drop(&mut self) {
        let mut slot = CONTROLLER
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if slot
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &self.controller))
        {
            *slot = None;
        }
    }
}

pub fn fault_catalog_sha256() -> &'static str {
    CATALOG_SHA256.get_or_init(|| {
        debug_assert_eq!(
            crate::production_legacy_apply::WIRED_BARE_METAL_FAULT_POINTS,
            FAULT_POINTS
        );
        let mut hasher = Sha256::new();
        hasher.update(CATALOG_DOMAIN);
        hasher.update(FAULT_PROTOCOL_VERSION.to_be_bytes());
        for point in FAULT_POINTS {
            hasher.update((point.as_str().len() as u32).to_be_bytes());
            hasher.update(point.as_str().as_bytes());
        }
        for mode in FAULT_MODES {
            hasher.update((mode.as_str().len() as u32).to_be_bytes());
            hasher.update(mode.as_str().as_bytes());
        }
        hex::encode(hasher.finalize())
    })
}

pub fn fault_case_binding_sha256(
    operation_id: &str,
    run_id: &str,
    case: BareMetalFaultCase,
) -> Result<String, FaultMatrixError> {
    validate_ids(operation_id, run_id)?;
    let mut hasher = Sha256::new();
    hasher.update(CASE_DOMAIN);
    for value in [
        fault_catalog_sha256(),
        operation_id,
        run_id,
        case.point.as_str(),
        case.mode.as_str(),
    ] {
        hasher.update((value.len() as u32).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn install_bare_metal_fault_case(
    config: BareMetalFaultControllerConfig,
) -> Result<BareMetalFaultControllerGuard, FaultMatrixError> {
    validate_ids(&config.operation_id, &config.run_id)?;
    validate_control_dir(&config.control_dir)?;
    let case_binding_sha256 =
        fault_case_binding_sha256(&config.operation_id, &config.run_id, config.case)?;
    let consumed = match read_ready_record(&config.control_dir)? {
        Some(record) => {
            validate_ready_record(&record, &config, &case_binding_sha256)?;
            reconcile_ready_hardlink(&config.control_dir)?;
            true
        }
        None => false,
    };
    let controller = Arc::new(Controller {
        config,
        case_binding_sha256,
        state: AtomicU8::new(if consumed {
            CONTROLLER_CONSUMED
        } else {
            CONTROLLER_ARMED
        }),
    });
    let mut slot = CONTROLLER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if slot.is_some() {
        return Err(FaultMatrixError::AlreadyInstalled);
    }
    *slot = Some(Arc::clone(&controller));
    Ok(BareMetalFaultControllerGuard { controller })
}

/// Feature-only entry admission: a normal production call never consults this.
pub fn require_installed_fault_case(operation_id: &str) -> Result<(), FaultMatrixError> {
    let slot = CONTROLLER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let controller = slot.as_ref().ok_or(FaultMatrixError::NotInstalled)?;
    if controller.config.operation_id != operation_id {
        return Err(FaultMatrixError::NotInstalled);
    }
    Ok(())
}

pub(crate) async fn before(point: BareMetalFaultPoint) -> Result<(), FaultMatrixError> {
    fire(point, FaultHookPhase::Before).await
}

pub(crate) async fn after_success(point: BareMetalFaultPoint) -> Result<(), FaultMatrixError> {
    fire(point, FaultHookPhase::AfterSuccess).await
}

async fn fire(point: BareMetalFaultPoint, phase: FaultHookPhase) -> Result<(), FaultMatrixError> {
    let controller = {
        let slot = CONTROLLER
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        slot.clone()
    };
    let Some(controller) = controller else {
        return Ok(());
    };
    if controller.config.case.point != point || !phase_matches(controller.config.case.mode, phase) {
        return Ok(());
    }
    match controller.state.compare_exchange(
        CONTROLLER_ARMED,
        CONTROLLER_FIRING,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(CONTROLLER_CONSUMED) => return Ok(()),
        Err(_) => return Err(FaultMatrixError::AlreadyInstalled),
    }
    let action = match controller.config.case.mode {
        BareMetalFaultMode::Before => FaultHookAction::InjectedBefore,
        BareMetalFaultMode::LostAcknowledgement => FaultHookAction::InjectedLostAcknowledgement,
        BareMetalFaultMode::SigkillReady => FaultHookAction::SigkillReady,
    };
    let record = FaultReadyRecord {
        protocol_version: FAULT_PROTOCOL_VERSION,
        catalog_sha256: fault_catalog_sha256().to_string(),
        case_binding_sha256: controller.case_binding_sha256.clone(),
        operation_id: controller.config.operation_id.clone(),
        run_id: controller.config.run_id.clone(),
        point,
        mode: controller.config.case.mode,
        phase,
        action,
        pid: std::process::id(),
    };
    if let Err(error) = persist_ready_record(&controller.config.control_dir, &record) {
        // A failed publication is not a consumed fault. Re-arm so even a
        // same-process recovery cannot proceed without durable readiness.
        controller.state.store(CONTROLLER_ARMED, Ordering::Release);
        return Err(error);
    }
    controller
        .state
        .store(CONTROLLER_CONSUMED, Ordering::Release);
    match controller.config.case.mode {
        BareMetalFaultMode::Before => Err(FaultMatrixError::InjectedBefore),
        BareMetalFaultMode::LostAcknowledgement => {
            Err(FaultMatrixError::InjectedLostAcknowledgement)
        }
        BareMetalFaultMode::SigkillReady => future::pending().await,
    }
}

const fn phase_matches(mode: BareMetalFaultMode, phase: FaultHookPhase) -> bool {
    matches!(
        (mode, phase),
        (BareMetalFaultMode::Before, FaultHookPhase::Before)
            | (
                BareMetalFaultMode::LostAcknowledgement | BareMetalFaultMode::SigkillReady,
                FaultHookPhase::AfterSuccess
            )
    )
}

fn validate_ids(operation_id: &str, run_id: &str) -> Result<(), FaultMatrixError> {
    let operation = Uuid::parse_str(operation_id).map_err(|_| FaultMatrixError::InvalidBinding)?;
    let run = Uuid::parse_str(run_id).map_err(|_| FaultMatrixError::InvalidBinding)?;
    if operation.is_nil()
        || run.is_nil()
        || operation.hyphenated().to_string() != operation_id
        || run.hyphenated().to_string() != run_id
    {
        return Err(FaultMatrixError::InvalidBinding);
    }
    Ok(())
}

fn validate_control_dir(path: &Path) -> Result<(), FaultMatrixError> {
    if !path.is_absolute() {
        return Err(FaultMatrixError::InvalidBinding);
    }
    let canonical = fs::canonicalize(path).map_err(FaultMatrixError::ReadyRecordIo)?;
    if canonical != path || !canonical.is_dir() {
        return Err(FaultMatrixError::InvalidBinding);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = canonical
            .metadata()
            .map_err(FaultMatrixError::ReadyRecordIo)?;
        if !process_uids_are_root()?
            || metadata.uid() != 0
            || metadata.gid() != 0
            || metadata.nlink() < 2
            || metadata.permissions().mode() & 0o7777 != 0o700
        {
            return Err(FaultMatrixError::InvalidBinding);
        }
    }
    Ok(())
}

fn persist_ready_record(path: &Path, record: &FaultReadyRecord) -> Result<(), FaultMatrixError> {
    let bytes = serde_json::to_vec(record).map_err(|_| FaultMatrixError::ReadyRecordInvalid)?;
    let ready = path.join(FAULT_READY_FILE);
    let temporary = path.join(format!(
        ".{FAULT_READY_FILE}.{}.{}.tmp",
        std::process::id(),
        Uuid::new_v4()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(FaultMatrixError::ReadyRecordIo)?;
    let result = file
        .write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::hard_link(&temporary, &ready))
        .and_then(|()| File::open(path)?.sync_all())
        .and_then(|()| fs::remove_file(&temporary))
        .and_then(|()| File::open(path)?.sync_all());
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(FaultMatrixError::ReadyRecordIo)?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(FaultMatrixError::ReadyRecordIo)
}

#[cfg(unix)]
fn process_uids_are_root() -> Result<bool, FaultMatrixError> {
    let status =
        fs::read_to_string("/proc/self/status").map_err(FaultMatrixError::ReadyRecordIo)?;
    let line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or(FaultMatrixError::InvalidBinding)?;
    let uids = line
        .split_ascii_whitespace()
        .skip(1)
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| FaultMatrixError::InvalidBinding)?;
    Ok(uids.len() == 4 && uids.into_iter().all(|uid| uid == 0))
}

fn read_ready_record(path: &Path) -> Result<Option<FaultReadyRecord>, FaultMatrixError> {
    let ready = path.join(FAULT_READY_FILE);
    let path_metadata = match fs::symlink_metadata(&ready) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(FaultMatrixError::ReadyRecordIo(error)),
    };
    let mut file = File::open(&ready).map_err(FaultMatrixError::ReadyRecordIo)?;
    let metadata = file.metadata().map_err(FaultMatrixError::ReadyRecordIo)?;
    if !metadata.file_type().is_file()
        || !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > 64 * 1024
    {
        return Err(FaultMatrixError::ReadyRecordInvalid);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.dev() != path_metadata.dev()
            || metadata.ino() != path_metadata.ino()
            || metadata.uid() != 0
            || metadata.gid() != 0
            // A crash after the no-replace hard-link publication but before
            // unlinking its owner-only temporary name leaves exactly two
            // links to the already-complete, fsynced inode. Both states are
            // recoverable; any broader link fan-out is rejected.
            || !(1..=2).contains(&metadata.nlink())
            || metadata.permissions().mode() & 0o7777 != 0o600
        {
            return Err(FaultMatrixError::ReadyRecordInvalid);
        }
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(FaultMatrixError::ReadyRecordIo)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(FaultMatrixError::ReadyRecordInvalid);
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| FaultMatrixError::ReadyRecordInvalid)
}

#[cfg(unix)]
fn reconcile_ready_hardlink(path: &Path) -> Result<(), FaultMatrixError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let ready = path.join(FAULT_READY_FILE);
    let metadata = fs::symlink_metadata(&ready).map_err(FaultMatrixError::ReadyRecordIo)?;
    if metadata.nlink() == 1 {
        return Ok(());
    }
    if metadata.nlink() != 2 {
        return Err(FaultMatrixError::ReadyRecordInvalid);
    }
    let mut matching_temporary = None;
    let mut entries = 0_u16;
    for entry in fs::read_dir(path).map_err(FaultMatrixError::ReadyRecordIo)? {
        entries = entries.saturating_add(1);
        if entries > 128 {
            return Err(FaultMatrixError::ReadyRecordInvalid);
        }
        let entry = entry.map_err(FaultMatrixError::ReadyRecordIo)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(&format!(".{FAULT_READY_FILE}.")) || !name.ends_with(".tmp") {
            continue;
        }
        let candidate =
            fs::symlink_metadata(entry.path()).map_err(FaultMatrixError::ReadyRecordIo)?;
        if candidate.dev() == metadata.dev() && candidate.ino() == metadata.ino() {
            if matching_temporary.is_some()
                || !candidate.file_type().is_file()
                || candidate.file_type().is_symlink()
                || candidate.uid() != 0
                || candidate.gid() != 0
                || candidate.permissions().mode() & 0o7777 != 0o600
            {
                return Err(FaultMatrixError::ReadyRecordInvalid);
            }
            matching_temporary = Some(entry.path());
        }
    }
    let temporary = matching_temporary.ok_or(FaultMatrixError::ReadyRecordInvalid)?;
    fs::remove_file(temporary).map_err(FaultMatrixError::ReadyRecordIo)?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(FaultMatrixError::ReadyRecordIo)?;
    let reconciled = fs::symlink_metadata(ready).map_err(FaultMatrixError::ReadyRecordIo)?;
    if reconciled.nlink() != 1 {
        return Err(FaultMatrixError::ReadyRecordInvalid);
    }
    Ok(())
}

#[cfg(not(unix))]
fn reconcile_ready_hardlink(_path: &Path) -> Result<(), FaultMatrixError> {
    Ok(())
}

fn validate_ready_record(
    record: &FaultReadyRecord,
    config: &BareMetalFaultControllerConfig,
    case_binding_sha256: &str,
) -> Result<(), FaultMatrixError> {
    let expected_phase = if config.case.mode == BareMetalFaultMode::Before {
        FaultHookPhase::Before
    } else {
        FaultHookPhase::AfterSuccess
    };
    let expected_action = match config.case.mode {
        BareMetalFaultMode::Before => FaultHookAction::InjectedBefore,
        BareMetalFaultMode::LostAcknowledgement => FaultHookAction::InjectedLostAcknowledgement,
        BareMetalFaultMode::SigkillReady => FaultHookAction::SigkillReady,
    };
    if record.protocol_version != FAULT_PROTOCOL_VERSION
        || record.catalog_sha256 != fault_catalog_sha256()
        || record.case_binding_sha256 != case_binding_sha256
        || record.operation_id != config.operation_id
        || record.run_id != config.run_id
        || record.point != config.case.point
        || record.mode != config.case.mode
        || record.phase != expected_phase
        || record.action != expected_action
        || record.pid == 0
    {
        return Err(FaultMatrixError::ReadyRecordConflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use super::*;

    static TEST_CONTROLLER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct PrivateDir(PathBuf);

    impl PrivateDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("v2board-matrix-core-{}-{label}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create test control dir");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .expect("protect test control dir");
            }
            Self(path)
        }
    }

    impl Drop for PrivateDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn config(dir: &Path, mode: BareMetalFaultMode) -> BareMetalFaultControllerConfig {
        BareMetalFaultControllerConfig {
            operation_id: "018f72c6-86d4-7c7b-9190-cd4bdab6f001".to_string(),
            run_id: "018f72c6-86d4-7c7b-9190-cd4bdab6f002".to_string(),
            control_dir: dir.to_path_buf(),
            case: BareMetalFaultCase {
                point: BareMetalFaultPoint::PostgresBulkCopy,
                mode,
            },
        }
    }

    #[test]
    fn catalog_is_closed_unique_and_stably_hashed() {
        assert_eq!(FAULT_POINTS.len(), 19);
        for (index, point) in FAULT_POINTS.iter().enumerate() {
            assert_eq!(
                point
                    .to_string()
                    .parse::<BareMetalFaultPoint>()
                    .expect("known point"),
                *point
            );
            assert!(!FAULT_POINTS[..index].contains(point));
        }
        for mode in FAULT_MODES {
            assert_eq!(
                mode.to_string()
                    .parse::<BareMetalFaultMode>()
                    .expect("known mode"),
                *mode
            );
        }
        assert_eq!(
            crate::production_legacy_apply::WIRED_BARE_METAL_FAULT_POINTS,
            FAULT_POINTS
        );
        assert_eq!(
            fault_catalog_sha256(),
            "ecc0d22de130ed0d170d8b98382a70e8016be448b93b7ccb969a8aab131fe7a4"
        );
    }

    #[tokio::test]
    async fn before_writes_durable_record_then_returns_interruption() {
        let _serial = TEST_CONTROLLER.lock().await;
        let dir = PrivateDir::new("before");
        let config = config(&dir.0, BareMetalFaultMode::Before);
        let _guard = install_bare_metal_fault_case(config.clone()).expect("install controller");
        let error = before(BareMetalFaultPoint::PostgresBulkCopy)
            .await
            .expect_err("inject before");
        assert!(matches!(error, FaultMatrixError::InjectedBefore));
        let record = read_ready_record(&dir.0)
            .expect("read ready")
            .expect("ready record");
        assert_eq!(record.phase, FaultHookPhase::Before);
        assert_eq!(record.action, FaultHookAction::InjectedBefore);
        validate_ready_record(
            &record,
            &config,
            &fault_case_binding_sha256(&config.operation_id, &config.run_id, config.case)
                .expect("case digest"),
        )
        .expect("record binding");
    }

    #[tokio::test]
    async fn lost_ack_is_only_injected_after_success_hook() {
        let _serial = TEST_CONTROLLER.lock().await;
        let dir = PrivateDir::new("lost-ack");
        let config = config(&dir.0, BareMetalFaultMode::LostAcknowledgement);
        let _guard = install_bare_metal_fault_case(config).expect("install controller");
        before(BareMetalFaultPoint::PostgresBulkCopy)
            .await
            .expect("before is not selected");
        let error = after_success(BareMetalFaultPoint::PostgresBulkCopy)
            .await
            .expect_err("inject lost ack");
        assert!(matches!(
            error,
            FaultMatrixError::InjectedLostAcknowledgement
        ));
    }

    #[tokio::test]
    async fn sigkill_mode_publishes_readiness_and_never_acknowledges() {
        let _serial = TEST_CONTROLLER.lock().await;
        let dir = PrivateDir::new("sigkill");
        let config = config(&dir.0, BareMetalFaultMode::SigkillReady);
        let _guard = install_bare_metal_fault_case(config).expect("install controller");
        let pending = tokio::spawn(after_success(BareMetalFaultPoint::PostgresBulkCopy));
        for _ in 0..100 {
            if dir.0.join(FAULT_READY_FILE).exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert!(dir.0.join(FAULT_READY_FILE).exists());
        assert!(!pending.is_finished());
        pending.abort();
    }

    #[tokio::test]
    async fn exact_ready_record_consumes_case_across_restart() {
        let _serial = TEST_CONTROLLER.lock().await;
        let dir = PrivateDir::new("resume");
        let config = config(&dir.0, BareMetalFaultMode::Before);
        {
            let _guard = install_bare_metal_fault_case(config.clone()).expect("install controller");
            before(BareMetalFaultPoint::PostgresBulkCopy)
                .await
                .expect_err("first process injects");
        }
        let _guard = install_bare_metal_fault_case(config).expect("resume controller");
        before(BareMetalFaultPoint::PostgresBulkCopy)
            .await
            .expect("resumed process must not inject twice");
    }

    #[tokio::test]
    async fn publication_crash_states_are_recoverable() {
        let _serial = TEST_CONTROLLER.lock().await;

        // A crash while writing the private temporary inode leaves no final
        // name, so a new guest can safely publish its own complete record.
        let before_link = PrivateDir::new("partial-temp");
        fs::write(
            before_link.0.join(".fault-ready.json.abandoned.tmp"),
            b"partial",
        )
        .expect("write abandoned temporary");
        let before_link_config = config(&before_link.0, BareMetalFaultMode::Before);
        {
            let _guard = install_bare_metal_fault_case(before_link_config)
                .expect("install after temp crash");
            before(BareMetalFaultPoint::PostgresBulkCopy)
                .await
                .expect_err("case remains armed");
        }

        // A crash after hard-link publication but before temporary cleanup
        // leaves nlink=2. The complete, fsynced ready record is authoritative
        // and consumes the case on resume.
        let after_link = PrivateDir::new("published-link");
        let config = config(&after_link.0, BareMetalFaultMode::Before);
        let record = FaultReadyRecord {
            protocol_version: FAULT_PROTOCOL_VERSION,
            catalog_sha256: fault_catalog_sha256().to_string(),
            case_binding_sha256: fault_case_binding_sha256(
                &config.operation_id,
                &config.run_id,
                config.case,
            )
            .expect("case binding"),
            operation_id: config.operation_id.clone(),
            run_id: config.run_id.clone(),
            point: config.case.point,
            mode: config.case.mode,
            phase: FaultHookPhase::Before,
            action: FaultHookAction::InjectedBefore,
            pid: std::process::id(),
        };
        let temporary = after_link.0.join(".fault-ready.json.published.tmp");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).expect("create complete temporary");
        file.write_all(&serde_json::to_vec(&record).expect("serialize ready"))
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .expect("fsync complete temporary");
        fs::hard_link(&temporary, after_link.0.join(FAULT_READY_FILE))
            .expect("publish without unlink");
        File::open(&after_link.0)
            .and_then(|directory| directory.sync_all())
            .expect("fsync published link");
        let _guard = install_bare_metal_fault_case(config).expect("resume after link crash");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::symlink_metadata(after_link.0.join(FAULT_READY_FILE))
                    .expect("reconciled ready metadata")
                    .nlink(),
                1
            );
        }
        before(BareMetalFaultPoint::PostgresBulkCopy)
            .await
            .expect("published record consumes case");
    }

    #[tokio::test]
    async fn failed_ready_publication_rearms_same_process() {
        let _serial = TEST_CONTROLLER.lock().await;
        let dir = PrivateDir::new("rearm");
        let config = config(&dir.0, BareMetalFaultMode::Before);
        let _guard = install_bare_metal_fault_case(config).expect("install controller");
        fs::remove_dir(&dir.0).expect("make first publication fail");
        assert!(matches!(
            before(BareMetalFaultPoint::PostgresBulkCopy).await,
            Err(FaultMatrixError::ReadyRecordIo(_))
        ));
        fs::create_dir(&dir.0).expect("restore control dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700))
                .expect("restore private mode");
        }
        assert!(matches!(
            before(BareMetalFaultPoint::PostgresBulkCopy).await,
            Err(FaultMatrixError::InjectedBefore)
        ));
        assert!(dir.0.join(FAULT_READY_FILE).is_file());
    }
}
