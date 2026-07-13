//! Bare-metal source-side adapter for the one-shot legacy migration.
//!
//! This module deliberately does not implement the complete
//! `LegacyApplyExecutor`.  It supplies the source-only stages which a complete
//! executor must delegate to: local systemd writer fencing, loss-aware Redis
//! drain/logout, an exact repeatable-read MySQL fingerprint, externally-bound
//! backup/restore evidence, irreversible local retirement, and post-retirement
//! probes made with the old credentials.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::IpAddr,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use redis::aio::MultiplexedConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use tokio::time::timeout;
use url::Url;
use uuid::Uuid;

use crate::{
    ProvisionSpec,
    apply_journal::{
        ApplyCheckpoint, ApplyJournalSnapshot, ApplyOutcomeCode, DurableMutationPermit,
    },
    inspect::semantic_schema_hash,
    legacy_apply::{
        BackupRestoreProof, CompletionRecoveryPermit, FinalRecheckProof, StageFailure,
        VerifiedStageProof,
    },
    legacy_backup::{
        VerifiedBackupArchive, perform_backup_restore, verify_persisted_backup_archive,
    },
    legacy_converter::{
        DEFAULT_BATCH_SIZE, LEGACY_SEMANTIC_SCHEMA_SHA256, LegacyConversionStrategy,
    },
    legacy_copy::fingerprint_legacy_source_for_strategy,
    manifest::{
        LegacyRuntimeReceiptKind, ProvisionFlow, SourceSpec, scheduler_unit_counterpart,
        scheduler_units_are_exact_pairs,
    },
    native_activation::{LegacySourceRetirementObserver, ObservedLegacySourceRetirement},
    target_activation::{ExecutorError, LegacySourceRetirementRequest},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const DATASTORE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;
const MAX_TRAFFIC_RECEIPT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FROZEN_TRAFFIC_USERS: usize = 2_000_000;
const MAX_SYSTEMD_DEFINITION_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FIXED_BINARY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CGROUP_DIRECTORIES: usize = 4_096;
const MAX_CGROUP_PROCESSES: usize = 65_536;
const MAX_PROCESS_FILE_DESCRIPTORS: usize = 262_144;
const REDIS_SCAN_COUNT: usize = 512;
const REDIS_DELETE_BATCH: usize = 512;
const QUEUE_DRAIN_TIMEOUT: Duration = Duration::from_secs(300);
const QUEUE_DRAIN_POLL: Duration = Duration::from_secs(1);
const REDIS_WRITE_PAUSE_MILLISECONDS: u64 = 86_400_000;

const SYSTEMCTL_PATH: &str = "/usr/bin/systemctl";
const ID_PATH: &str = "/usr/bin/id";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LegacyProgram {
    Systemctl,
    Id,
}

impl LegacyProgram {
    const fn path(self) -> &'static str {
        match self {
            Self::Systemctl => SYSTEMCTL_PATH,
            Self::Id => ID_PATH,
        }
    }
}

struct LegacyCommandRequest {
    program: LegacyProgram,
    args: Vec<OsString>,
    redactions: Vec<Vec<u8>>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl LegacyCommandRequest {
    fn new(program: LegacyProgram) -> Self {
        Self {
            program,
            args: Vec::new(),
            redactions: Vec::new(),
            timeout: COMMAND_TIMEOUT,
            max_output_bytes: MAX_OUTPUT_BYTES,
        }
    }

    fn arg(mut self, value: impl Into<OsString>) -> Self {
        self.args.push(value.into());
        self
    }

    #[cfg(test)]
    fn redact(mut self, value: &str) -> Self {
        if !value.is_empty() {
            self.redactions.push(value.as_bytes().to_vec());
        }
        self
    }

    fn validate(&self) -> Result<(), LegacyRunnerError> {
        if self.args.len() > 128
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(300)
            || self.max_output_bytes == 0
            || self.max_output_bytes > 1024 * 1024
            || self.redactions.iter().any(Vec::is_empty)
        {
            return Err(LegacyRunnerError::InvalidRequest);
        }
        if self.redactions.iter().any(|secret| {
            self.args.iter().any(|argument| {
                argument
                    .as_encoded_bytes()
                    .windows(secret.len())
                    .any(|part| part == secret)
            })
        }) {
            return Err(LegacyRunnerError::SecretInArgv);
        }
        Ok(())
    }
}

impl Drop for LegacyCommandRequest {
    fn drop(&mut self) {
        for secret in &mut self.redactions {
            secret.fill(0);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyCommandOutput {
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl LegacyCommandOutput {
    #[cfg(test)]
    fn success(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            exit_code: 0,
            stdout: stdout.into(),
            stderr: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LegacyRunnerError {
    #[error("invalid fixed command request")]
    InvalidRequest,
    #[error("secret material was detected in argv")]
    SecretInArgv,
    #[error("fixed command binary is missing or unsafe")]
    UnsafeProgram,
    #[error("command spawn failed")]
    Spawn,
    #[error("command wait failed")]
    Wait,
    #[error("command timed out")]
    Timeout,
    #[error("command output exceeded its bounded limit")]
    OutputLimit,
    #[error("command output reader failed")]
    OutputRead,
}

trait LegacyCommandRunner: Send {
    fn run(
        &mut self,
        request: LegacyCommandRequest,
    ) -> Result<LegacyCommandOutput, LegacyRunnerError>;
}

#[derive(Default)]
struct ProcessLegacyCommandRunner;

impl LegacyCommandRunner for ProcessLegacyCommandRunner {
    fn run(
        &mut self,
        request: LegacyCommandRequest,
    ) -> Result<LegacyCommandOutput, LegacyRunnerError> {
        request.validate()?;
        let program = validated_program_path(request.program)?;
        let mut child = Command::new(program)
            .args(&request.args)
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| LegacyRunnerError::Spawn)?;
        let stdout = child.stdout.take().ok_or(LegacyRunnerError::OutputRead)?;
        let stderr = child.stderr.take().ok_or(LegacyRunnerError::OutputRead)?;
        let limit = request.max_output_bytes;
        let stdout_thread = thread::spawn(move || read_bounded(stdout, limit));
        let stderr_thread = thread::spawn(move || read_bounded(stderr, limit));
        let deadline = Instant::now() + request.timeout;
        let status = loop {
            match child.try_wait().map_err(|_| LegacyRunnerError::Wait)? {
                Some(status) => break status,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err(LegacyRunnerError::Timeout);
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        };
        let (mut stdout, stdout_overflow) = stdout_thread
            .join()
            .map_err(|_| LegacyRunnerError::OutputRead)?
            .map_err(|_| LegacyRunnerError::OutputRead)?;
        let (mut stderr, stderr_overflow) = stderr_thread
            .join()
            .map_err(|_| LegacyRunnerError::OutputRead)?
            .map_err(|_| LegacyRunnerError::OutputRead)?;
        redact_all(&mut stdout, &request.redactions);
        redact_all(&mut stderr, &request.redactions);
        if stdout_overflow || stderr_overflow {
            return Err(LegacyRunnerError::OutputLimit);
        }
        Ok(LegacyCommandOutput {
            exit_code: status.code().unwrap_or(-1),
            stdout,
            stderr,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyWriterUnits {
    pub api: Vec<String>,
    pub workers: Vec<String>,
    pub schedulers: Vec<String>,
    pub local_datastores: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyUnitRole {
    IngressWriter,
    DrainWorker,
    Scheduler,
    Mysql,
    DefaultRedis,
    CacheRedis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreAuthorizationLegacyUnitInspection {
    pub unit: String,
    pub roles: Vec<LegacyUnitRole>,
    pub fragment_path: PathBuf,
    pub effective_definition_sha256: String,
    pub exec_start_sha256: String,
    pub control_group: String,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: String,
    pub main_pid_present: bool,
    pub restart_configured: bool,
    pub safely_stoppable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreAuthorizationDatastoreBindingInspection {
    pub role: LegacyUnitRole,
    pub unit: String,
    pub endpoint_address: String,
    pub endpoint_port: u16,
    pub listener_owned_by_declared_unit_cgroup: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreAuthorizationSourceControlInspection {
    pub effective_root: bool,
    pub fixed_systemctl_path: PathBuf,
    pub fixed_systemctl_sha256: String,
    pub cgroup_v2: bool,
    pub units: Vec<PreAuthorizationLegacyUnitInspection>,
    pub datastore_bindings: Vec<PreAuthorizationDatastoreBindingInspection>,
    pub all_declared_units_loaded_state_safe_restart_configured_and_stoppable: bool,
    pub all_datastore_listeners_owned_by_declared_unit_cgroups: bool,
    pub mysql_fence_credential_independent: bool,
    pub mysql_fence_grants_exact: bool,
    pub mysql_persisted_globals_load_enabled: bool,
    pub mysql_transaction_inventory_visible: bool,
    pub redis_6_2_pause_write_supported: bool,
    pub redis_fence_acl_complete: bool,
    pub datastore_write_fence_capabilities_ready: bool,
}

impl LegacyWriterUnits {
    fn validate(&self) -> Result<(), SourceError> {
        if self.api.is_empty() || self.workers.is_empty() || self.schedulers.is_empty() {
            return Err(SourceError::InvalidPolicy);
        }
        let mut all = BTreeSet::new();
        for unit in self
            .api
            .iter()
            .chain(&self.workers)
            .chain(&self.schedulers)
            .chain(&self.local_datastores)
        {
            if !valid_systemd_unit(unit) || !all.insert(unit) {
                return Err(SourceError::InvalidPolicy);
            }
        }
        if !scheduler_units_are_exact_pairs(&self.schedulers) {
            return Err(SourceError::InvalidPolicy);
        }
        Ok(())
    }

    fn writers(&self) -> impl Iterator<Item = &String> {
        self.api.iter().chain(&self.workers).chain(&self.schedulers)
    }

    fn ingress_writers(&self) -> impl Iterator<Item = &String> {
        self.api.iter().chain(&self.schedulers)
    }

    fn all_retirement_units(&self) -> impl Iterator<Item = &String> {
        self.writers().chain(&self.local_datastores)
    }

    fn digest(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(b"v2board-legacy-writer-unit-inventory-v1\0");
        for (kind, units) in [
            (b"api".as_slice(), &self.api),
            (b"workers".as_slice(), &self.workers),
            (b"schedulers".as_slice(), &self.schedulers),
            (b"local_datastores".as_slice(), &self.local_datastores),
        ] {
            digest_field(&mut digest, kind);
            for unit in units {
                digest_field(&mut digest, unit.as_bytes());
            }
        }
        hex::encode(digest.finalize())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LegacySourcePolicy {
    units: LegacyWriterUnits,
}

impl LegacySourcePolicy {
    fn from_manifest(spec: &ProvisionSpec) -> Result<Self, SourceError> {
        let execution = spec
            .legacy_apply_execution()
            .ok_or(SourceError::InvalidPolicy)?;
        let mut local_datastores = BTreeSet::new();
        for datastore in [
            &execution.source_control.datastores.mysql,
            &execution.source_control.datastores.default_redis,
            &execution.source_control.datastores.cache_redis,
        ] {
            local_datastores.insert(datastore.unit.clone());
        }
        Ok(Self {
            units: LegacyWriterUnits {
                api: execution.systemd.legacy_writer_units.clone(),
                workers: execution.systemd.legacy_worker_units.clone(),
                schedulers: execution.systemd.legacy_scheduler_units.clone(),
                local_datastores: local_datastores.into_iter().collect(),
            },
        })
    }

    pub fn validate(&self) -> Result<(), SourceError> {
        self.units.validate()?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("legacy source policy is invalid")]
    InvalidPolicy,
    #[error("the provision spec is not a legacy reference migration")]
    WrongProvisionKind,
    #[error("a fixed bare-metal command failed")]
    Command,
    #[error("a declared systemd unit definition or runtime identity is unsafe")]
    UnsafeUnitIdentity,
    #[error("the host does not expose the required systemd cgroup-v2 process identity")]
    CgroupIdentity,
    #[error("a source datastore listener is not owned by its declared systemd unit")]
    ListenerOwnership,
    #[error("a required legacy unit is not safely stopped")]
    UnitStillActive,
    #[error("a required external receipt is invalid or does not bind this input")]
    ReceiptInvalid,
    #[error("source Redis contains durable or unclassified state")]
    RedisDrainIncomplete,
    #[error("source Redis operation failed")]
    Redis,
    #[error("source MySQL operation failed")]
    Mysql,
    #[error("source MySQL repeatable-read fingerprint differs from the backup")]
    SourceDrift,
    #[error("target-empty evidence is malformed")]
    TargetEvidence,
    #[error("post-retirement probe runtime failed")]
    ProbeRuntime,
    #[error("the built-in encrypted backup/isolated-restore drill failed")]
    Backup,
}

struct PreflightUnitRuntime {
    report: PreAuthorizationLegacyUnitInspection,
    process_ids: BTreeSet<u32>,
    socket_inodes: BTreeSet<u64>,
}

/// Performs the source-host portion of the online, read-only inspection. It
/// proves that each manually declared datastore unit actually owns the exact
/// loopback listener named by the source URL. No unit is stopped, enabled,
/// disabled, or otherwise mutated here.
pub async fn inspect_pre_authorization_source_control(
    spec: &ProvisionSpec,
) -> Result<PreAuthorizationSourceControlInspection, SourceError> {
    let policy = LegacySourcePolicy::from_manifest(spec)?;
    policy.validate()?;
    let source = legacy_source(spec)?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::InvalidPolicy)?;
    let mut runner = ProcessLegacyCommandRunner;
    ensure_effective_root(&mut runner)?;

    let systemctl_path = validated_program_path(LegacyProgram::Systemctl)
        .map_err(|_| SourceError::UnsafeUnitIdentity)?;
    let (_, fixed_systemctl_sha256) =
        hash_root_owned_regular_file(&systemctl_path, MAX_FIXED_BINARY_BYTES)?;
    let cgroup_root = canonical_cgroup_v2_root()?;

    let mut roles_by_unit = BTreeMap::<String, BTreeSet<LegacyUnitRole>>::new();
    for unit in &execution.systemd.legacy_writer_units {
        roles_by_unit
            .entry(unit.clone())
            .or_default()
            .insert(LegacyUnitRole::IngressWriter);
    }
    for unit in &execution.systemd.legacy_worker_units {
        roles_by_unit
            .entry(unit.clone())
            .or_default()
            .insert(LegacyUnitRole::DrainWorker);
    }
    for unit in &execution.systemd.legacy_scheduler_units {
        roles_by_unit
            .entry(unit.clone())
            .or_default()
            .insert(LegacyUnitRole::Scheduler);
    }
    for (role, datastore) in [
        (
            LegacyUnitRole::Mysql,
            &execution.source_control.datastores.mysql,
        ),
        (
            LegacyUnitRole::DefaultRedis,
            &execution.source_control.datastores.default_redis,
        ),
        (
            LegacyUnitRole::CacheRedis,
            &execution.source_control.datastores.cache_redis,
        ),
    ] {
        roles_by_unit
            .entry(datastore.unit.clone())
            .or_default()
            .insert(role);
    }

    let mut runtimes = BTreeMap::new();
    for (unit, roles) in roles_by_unit {
        let runtime = inspect_active_unit(
            &mut runner,
            &cgroup_root,
            &unit,
            roles.into_iter().collect(),
        )?;
        runtimes.insert(unit, runtime);
    }

    let datastore_inputs = [
        (
            LegacyUnitRole::Mysql,
            execution.source_control.datastores.mysql.unit.as_str(),
            source.database_url.as_str(),
        ),
        (
            LegacyUnitRole::DefaultRedis,
            execution
                .source_control
                .datastores
                .default_redis
                .unit
                .as_str(),
            source.redis_default_url.as_str(),
        ),
        (
            LegacyUnitRole::CacheRedis,
            execution
                .source_control
                .datastores
                .cache_redis
                .unit
                .as_str(),
            source.redis_cache_url.as_str(),
        ),
    ];
    let mut datastore_bindings = Vec::new();
    for (role, unit, url) in datastore_inputs {
        let (address, port) = source_listener_endpoint(url)?;
        let runtime = runtimes.get(unit).ok_or(SourceError::InvalidPolicy)?;
        let owned_inodes = listener_inodes_owned_by_unit(runtime, address, port)?;
        if owned_inodes.is_empty() {
            return Err(SourceError::ListenerOwnership);
        }
        // A distinct declared datastore cgroup must never share ownership of
        // the same listener. The only allowed reuse is the same Redis unit for
        // two logical DB numbers, which the manifest validates separately.
        if runtimes.iter().any(|(candidate_unit, candidate)| {
            candidate_unit != unit
                && candidate
                    .socket_inodes
                    .iter()
                    .any(|inode| owned_inodes.contains(inode))
        }) {
            return Err(SourceError::ListenerOwnership);
        }
        datastore_bindings.push(PreAuthorizationDatastoreBindingInspection {
            role,
            unit: unit.to_string(),
            endpoint_address: address.to_string(),
            endpoint_port: port,
            listener_owned_by_declared_unit_cgroup: true,
        });
    }

    let fence_capabilities = inspect_datastore_write_fence_capabilities(source).await?;
    Ok(PreAuthorizationSourceControlInspection {
        effective_root: true,
        fixed_systemctl_path: systemctl_path,
        fixed_systemctl_sha256,
        cgroup_v2: true,
        units: runtimes
            .into_values()
            .map(|runtime| runtime.report)
            .collect(),
        datastore_bindings,
        all_declared_units_loaded_state_safe_restart_configured_and_stoppable: true,
        all_datastore_listeners_owned_by_declared_unit_cgroups: true,
        mysql_fence_credential_independent: fence_capabilities.mysql_credential_independent,
        mysql_fence_grants_exact: fence_capabilities.mysql_grants_exact,
        mysql_persisted_globals_load_enabled: fence_capabilities.persisted_globals_load_enabled,
        mysql_transaction_inventory_visible: fence_capabilities.transaction_inventory_visible,
        redis_6_2_pause_write_supported: fence_capabilities.redis_pause_write_supported,
        redis_fence_acl_complete: fence_capabilities.redis_acl_complete,
        datastore_write_fence_capabilities_ready: fence_capabilities.ready(),
    })
}

struct DatastoreWriteFenceCapabilityInspection {
    mysql_credential_independent: bool,
    mysql_grants_exact: bool,
    persisted_globals_load_enabled: bool,
    transaction_inventory_visible: bool,
    redis_pause_write_supported: bool,
    redis_acl_complete: bool,
}

impl DatastoreWriteFenceCapabilityInspection {
    fn ready(&self) -> bool {
        self.mysql_credential_independent
            && self.mysql_grants_exact
            && self.persisted_globals_load_enabled
            && self.transaction_inventory_visible
            && self.redis_pause_write_supported
            && self.redis_acl_complete
    }
}

async fn inspect_datastore_write_fence_capabilities(
    source: &SourceSpec,
) -> Result<DatastoreWriteFenceCapabilityInspection, SourceError> {
    let fence_url = source
        .database_fence_url
        .as_deref()
        .ok_or(SourceError::InvalidPolicy)?;
    let reader_url = Url::parse(&source.database_url).map_err(|_| SourceError::InvalidPolicy)?;
    let fence_identity = Url::parse(fence_url).map_err(|_| SourceError::InvalidPolicy)?;
    let mysql_credential_independent = reader_url.username() != fence_identity.username()
        && reader_url.password() != fence_identity.password();
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(DATASTORE_TIMEOUT)
        .connect(fence_url)
        .await
        .map_err(|_| SourceError::Mysql)?;
    let grants = sqlx::query_scalar::<_, String>("SHOW GRANTS")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
    let mysql_grants_exact = exact_mysql_fence_grants(&grants);
    let persisted_globals_load_enabled =
        sqlx::query_scalar::<_, i64>("SELECT @@GLOBAL.persisted_globals_load + 0")
            .fetch_one(&pool)
            .await
            .is_ok_and(|value| value == 1);
    let transaction_inventory_visible =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM information_schema.innodb_trx")
            .fetch_one(&pool)
            .await
            .is_ok();
    pool.close().await;

    let default = inspect_redis_fence_capability(&source.redis_default_url).await?;
    let cache =
        if redis_identity(&source.redis_default_url)? == redis_identity(&source.redis_cache_url)? {
            default
        } else {
            inspect_redis_fence_capability(&source.redis_cache_url).await?
        };
    Ok(DatastoreWriteFenceCapabilityInspection {
        mysql_credential_independent,
        mysql_grants_exact,
        persisted_globals_load_enabled,
        transaction_inventory_visible,
        redis_pause_write_supported: default.0 && cache.0,
        redis_acl_complete: default.1 && cache.1,
    })
}

fn exact_mysql_fence_grants(grants: &[String]) -> bool {
    let mut observed = BTreeSet::new();
    for grant in grants {
        let uppercase = grant.to_ascii_uppercase();
        let Some(privileges) = uppercase.strip_prefix("GRANT ") else {
            return false;
        };
        let Some((privileges, _)) = privileges.split_once(" ON *.* TO ") else {
            return false;
        };
        for privilege in privileges.split(',').map(str::trim) {
            if !matches!(privilege, "USAGE" | "PROCESS" | "SYSTEM_VARIABLES_ADMIN") {
                return false;
            }
            observed.insert(privilege.to_string());
        }
    }
    observed.contains("PROCESS") && observed.contains("SYSTEM_VARIABLES_ADMIN")
}

async fn inspect_redis_fence_capability(url: &str) -> Result<(bool, bool), SourceError> {
    let mut connection = redis_connection(url).await?;
    let server_info = redis::cmd("INFO")
        .arg("server")
        .query_async::<String>(&mut connection)
        .await
        .map_err(|_| SourceError::Redis)?;
    let version = server_info
        .lines()
        .find_map(|line| line.strip_prefix("redis_version:"))
        .map(str::trim)
        .unwrap_or_default();
    let pause_supported = redis_version_at_least_6_2(version);
    let whoami = redis::cmd("ACL")
        .arg("WHOAMI")
        .query_async::<String>(&mut connection)
        .await;
    let acl_complete = if let Ok(username) = whoami {
        match redis::cmd("ACL")
            .arg("GETUSER")
            .arg(username)
            .query_async::<BTreeMap<String, redis::Value>>(&mut connection)
            .await
        {
            Ok(user) => redis_acl_allows_lifecycle_fence(&user),
            Err(_) => false,
        }
    } else {
        false
    };
    Ok((pause_supported, acl_complete))
}

fn redis_version_at_least_6_2(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(major) = parts.next().and_then(|part| part.parse::<u64>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|part| part.parse::<u64>().ok()) else {
        return false;
    };
    major > 6 || (major == 6 && minor >= 2)
}

fn redis_acl_allows_lifecycle_fence(user: &BTreeMap<String, redis::Value>) -> bool {
    let strings = |key: &str| {
        user.get(key)
            .and_then(|value| redis::from_redis_value::<Vec<String>>(value.clone()).ok())
            .unwrap_or_default()
    };
    let flags = strings("flags");
    let keys = strings("keys");
    let commands = user
        .get("commands")
        .and_then(|value| redis::from_redis_value::<String>(value.clone()).ok())
        .unwrap_or_default();
    flags.iter().any(|flag| flag == "on")
        && !flags.iter().any(|flag| flag == "off")
        && keys.iter().any(|pattern| pattern == "~*")
        && commands
            .split_ascii_whitespace()
            .any(|rule| rule == "+@all")
        && !commands
            .split_ascii_whitespace()
            .any(|rule| rule.starts_with('-'))
}

fn inspect_active_unit<R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    cgroup_root: &Path,
    unit: &str,
    roles: Vec<LegacyUnitRole>,
) -> Result<PreflightUnitRuntime, SourceError> {
    let output = run_success(
        runner,
        LegacyCommandRequest::new(LegacyProgram::Systemctl)
            .arg("show")
            .arg("--no-pager")
            .arg("--property=Id")
            .arg("--property=LoadState")
            .arg("--property=ActiveState")
            .arg("--property=SubState")
            .arg("--property=UnitFileState")
            .arg("--property=FragmentPath")
            .arg("--property=DropInPaths")
            .arg("--property=ControlGroup")
            .arg("--property=MainPID")
            .arg("--property=CanStop")
            .arg("--property=RefuseManualStop")
            .arg("--property=Type")
            .arg("--property=ExecStart")
            .arg("--property=Environment")
            .arg("--property=EnvironmentFiles")
            .arg("--property=Triggers")
            .arg("--property=TriggeredBy")
            .arg(unit),
    )?;
    let properties = parse_systemctl_properties(&output.stdout)?;
    let property = |name: &'static str| {
        properties
            .get(name)
            .map(String::as_str)
            .ok_or(SourceError::UnsafeUnitIdentity)
    };
    let active_state = property("ActiveState")?;
    let sub_state = property("SubState")?;
    let unit_file_state = property("UnitFileState")?;
    let is_scheduler = roles.contains(&LegacyUnitRole::Scheduler);
    let is_timer = is_scheduler && unit.ends_with(".timer");
    let is_processless_scheduler_service = is_scheduler
        && unit.ends_with(".service")
        && ((active_state == "inactive" && matches!(sub_state, "dead" | "exited"))
            || (active_state == "active" && sub_state == "exited"));
    let scheduler_relation_valid = if is_scheduler {
        let counterpart =
            scheduler_unit_counterpart(unit).ok_or(SourceError::UnsafeUnitIdentity)?;
        let relation = if is_timer {
            property("Triggers")?
        } else {
            property("TriggeredBy")?
        };
        exact_systemd_unit_relation(relation, &counterpart)
    } else {
        true
    };
    let has_datastore_role = roles.iter().any(|role| {
        matches!(
            role,
            LegacyUnitRole::Mysql | LegacyUnitRole::DefaultRedis | LegacyUnitRole::CacheRedis
        )
    });
    let role_shape_valid = scheduler_relation_valid
        && if is_timer {
            roles.len() == 1
                && unit.ends_with(".timer")
                && matches!(sub_state, "waiting" | "running" | "elapsed")
        } else if is_processless_scheduler_service {
            roles.len() == 1
        } else {
            unit.ends_with(".service") && matches!(sub_state, "running" | "exited")
        };
    let restart_configured = matches!(
        unit_file_state,
        "enabled"
            | "enabled-runtime"
            | "static"
            | "indirect"
            | "generated"
            | "alias"
            | "linked"
            | "linked-runtime"
    );
    let safely_stoppable = property("CanStop")? == "yes" && property("RefuseManualStop")? == "no";
    if property("Id")? != unit
        || property("LoadState")? != "loaded"
        || (active_state != "active" && !is_processless_scheduler_service)
        || !role_shape_valid
        || !restart_configured
        || !safely_stoppable
    {
        return Err(SourceError::UnsafeUnitIdentity);
    }

    let fragment = PathBuf::from(property("FragmentPath")?);
    let (fragment, fragment_sha256) =
        hash_root_owned_regular_file(&fragment, MAX_SYSTEMD_DEFINITION_BYTES)?;
    let mut definition_digest = Sha256::new();
    definition_digest.update(b"v2board-preauth-systemd-effective-definition-v1\0");
    digest_field(
        &mut definition_digest,
        fragment.as_os_str().as_encoded_bytes(),
    );
    digest_field(&mut definition_digest, fragment_sha256.as_bytes());
    for drop_in in property("DropInPaths")?.split_ascii_whitespace() {
        let (path, sha256) =
            hash_root_owned_regular_file(Path::new(drop_in), MAX_SYSTEMD_DEFINITION_BYTES)?;
        digest_field(&mut definition_digest, path.as_os_str().as_encoded_bytes());
        digest_field(&mut definition_digest, sha256.as_bytes());
    }
    for name in [
        "Type",
        "ExecStart",
        "Environment",
        "EnvironmentFiles",
        "Triggers",
        "TriggeredBy",
    ] {
        digest_field(&mut definition_digest, name.as_bytes());
        digest_field(&mut definition_digest, property(name)?.as_bytes());
    }
    let exec_start_sha256 = domain_hash_fields(
        b"v2board-preauth-systemd-exec-start-v1\0",
        [property("ExecStart")?.as_bytes()],
    );

    let main_pid = property("MainPID")?
        .parse::<u32>()
        .map_err(|_| SourceError::UnsafeUnitIdentity)?;
    let control_group = property("ControlGroup")?;
    let (process_ids, socket_inodes) = if is_timer || is_processless_scheduler_service {
        if main_pid != 0 {
            return Err(SourceError::UnsafeUnitIdentity);
        }
        (BTreeSet::new(), BTreeSet::new())
    } else {
        if main_pid == 0 || control_group.is_empty() {
            return Err(SourceError::CgroupIdentity);
        }
        let process_ids = collect_cgroup_process_ids(cgroup_root, control_group)?;
        if !process_ids.contains(&main_pid) {
            return Err(SourceError::CgroupIdentity);
        }
        let socket_inodes = collect_process_socket_inodes(&process_ids)?;
        if has_datastore_role && socket_inodes.is_empty() {
            return Err(SourceError::ListenerOwnership);
        }
        (process_ids, socket_inodes)
    };
    Ok(PreflightUnitRuntime {
        report: PreAuthorizationLegacyUnitInspection {
            unit: unit.to_string(),
            roles,
            fragment_path: fragment,
            effective_definition_sha256: hex::encode(definition_digest.finalize()),
            exec_start_sha256,
            control_group: control_group.to_string(),
            active_state: active_state.to_string(),
            sub_state: sub_state.to_string(),
            unit_file_state: unit_file_state.to_string(),
            main_pid_present: main_pid != 0,
            restart_configured,
            safely_stoppable,
        },
        process_ids,
        socket_inodes,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RedisDrainReport {
    pub source_default_run_id: String,
    pub frozen_upload_key: String,
    pub frozen_download_key: String,
    pub traffic_delta_users: u64,
    pub traffic_delta_sha256: String,
    pub upload_delta_sum: String,
    pub download_delta_sum: String,
    pub traffic_delta_applied_exactly_once: bool,
    pub traffic_receipt_sha256: String,
    pub durable_queue_items: u64,
    pub traffic_fields: u64,
    pub traffic_reset_locks: u64,
    pub queue_notify_keys_deleted: u64,
    pub horizon_metadata_keys_deleted: u64,
    pub session_cache_token_keys_deleted: u64,
    pub unknown_default_owned_keys: u64,
    pub unknown_cache_owned_keys: u64,
    pub logout_all: bool,
    pub default_namespace_empty_after_drain: bool,
    pub cache_namespace_empty_after_drain: bool,
}

pub struct BareMetalLegacySource {
    runner: Box<dyn LegacyCommandRunner>,
    policy: LegacySourcePolicy,
}

impl BareMetalLegacySource {
    pub fn from_manifest(spec: &ProvisionSpec) -> Result<Self, SourceError> {
        let policy = LegacySourcePolicy::from_manifest(spec)?;
        policy.validate()?;
        Ok(Self {
            runner: Box::new(ProcessLegacyCommandRunner),
            policy,
        })
    }

    pub async fn fence_source(
        &mut self,
        spec: &ProvisionSpec,
        head: &ApplyJournalSnapshot,
    ) -> Result<VerifiedStageProof, StageFailure> {
        self.fence_source_inner(spec, head)
            .map_err(|_| stage_failure(ApplyOutcomeCode::FenceUncertain, "source_fence_failed"))
    }

    fn fence_source_inner(
        &mut self,
        spec: &ProvisionSpec,
        head: &ApplyJournalSnapshot,
    ) -> Result<VerifiedStageProof, SourceError> {
        let _source = legacy_source(spec)?;
        ensure_effective_root(&mut *self.runner)?;
        for unit in self.policy.units.ingress_writers() {
            run_success(
                &mut *self.runner,
                LegacyCommandRequest::new(LegacyProgram::Systemctl)
                    .arg("mask")
                    .arg("--now")
                    .arg(unit),
            )?;
        }
        verify_units_stopped(&mut *self.runner, self.policy.units.ingress_writers(), true)?;
        // Queue workers remain active only long enough to drain already
        // durable work, but disabling them now prevents a reboot from
        // silently reintroducing an old writer before crash recovery resumes.
        for unit in &self.policy.units.workers {
            run_success(
                &mut *self.runner,
                LegacyCommandRequest::new(LegacyProgram::Systemctl)
                    .arg("disable")
                    .arg(unit),
            )?;
        }
        verify_units_restart_disabled(&mut *self.runner, self.policy.units.workers.iter())?;
        let receipt_sha256 = persist_source_fence_receipt(spec, head, &self.policy.units)?;
        VerifiedStageProof::new(receipt_sha256).map_err(|_| SourceError::ReceiptInvalid)
    }

    pub async fn drain_source(
        &mut self,
        spec: &ProvisionSpec,
        head: &ApplyJournalSnapshot,
    ) -> Result<VerifiedStageProof, StageFailure> {
        self.drain_source_inner(spec, head).await.map_err(|error| {
            let outcome = if matches!(error, SourceError::RedisDrainIncomplete) {
                ApplyOutcomeCode::DrainIncomplete
            } else {
                ApplyOutcomeCode::IoFailure
            };
            stage_failure(outcome, "source_redis_drain_failed")
        })
    }

    async fn drain_source_inner(
        &mut self,
        spec: &ProvisionSpec,
        head: &ApplyJournalSnapshot,
    ) -> Result<VerifiedStageProof, SourceError> {
        let source = legacy_source(spec)?;
        verify_units_stopped(&mut *self.runner, self.policy.units.ingress_writers(), true)?;
        if let Some(receipt_sha256) =
            verify_existing_redis_fence(spec, head, &self.policy.units, &mut *self.runner).await?
        {
            return VerifiedStageProof::new(receipt_sha256)
                .map_err(|_| SourceError::ReceiptInvalid);
        }
        let redis_units = redis_units(spec)?;
        if let Some(armed) = load_redis_fence_armed_receipt(spec, head, &self.policy.units)? {
            let default_reachable = redis_probe(&source.redis_default_url).await;
            let cache_reachable = redis_probe(&source.redis_cache_url).await;
            let same_process = redis_process_identity(&source.redis_default_url)?
                == redis_process_identity(&source.redis_cache_url)?;
            if same_process && default_reachable != cache_reachable {
                return Err(SourceError::ProbeRuntime);
            }
            verify_offline_redis_units(
                spec,
                &mut *self.runner,
                default_reachable,
                cache_reachable,
            )?;
            if default_reachable || cache_reachable {
                let snapshot_pause =
                    pause_reachable_redis_writes(source, default_reachable, cache_reachable)
                        .await?;
                verify_reachable_post_pause_snapshot(
                    spec,
                    source,
                    &armed.payload,
                    default_reachable,
                    cache_reachable,
                )
                .await?;
                require_redis_pause_ack_window(&snapshot_pause)?;
                let stop_pause =
                    pause_reachable_redis_writes(source, default_reachable, cache_reachable)
                        .await?;
                require_redis_pause_ack_window(&stop_pause)?;
            }
            stop_masked_redis_units(&mut *self.runner, &redis_units)?;
            return finalize_redis_fence(
                spec,
                head,
                &self.policy.units,
                &redis_units,
                &armed.receipt_sha256,
                &mut *self.runner,
            )
            .await;
        }
        self.wait_for_legacy_queues(source).await?;
        for unit in &self.policy.units.workers {
            run_success(
                &mut *self.runner,
                LegacyCommandRequest::new(LegacyProgram::Systemctl)
                    .arg("mask")
                    .arg("--now")
                    .arg(unit),
            )?;
        }
        verify_units_stopped(&mut *self.runner, self.policy.units.workers.iter(), true)?;
        let horizon_prefix = source_horizon_physical_prefix(source)?;
        let report = drain_legacy_redis(spec, source, &horizon_prefix, Some(head)).await?;
        if report.durable_queue_items != 0
            || report.traffic_reset_locks != 0
            || report.unknown_default_owned_keys != 0
            || report.unknown_cache_owned_keys != 0
            || !report.logout_all
            || !report.default_namespace_empty_after_drain
            || !report.cache_namespace_empty_after_drain
        {
            return Err(SourceError::RedisDrainIncomplete);
        }
        let bytes = serde_json::to_vec(&report).map_err(|_| SourceError::Redis)?;
        let report_sha = domain_hash_fields(
            b"v2board-source-drain-report-v1\0",
            [
                spec.operation_id.as_bytes(),
                head.event_sha256().as_bytes(),
                bytes.as_slice(),
            ],
        );
        ensure_effective_root(&mut *self.runner)?;
        for unit in &redis_units {
            run_success(
                &mut *self.runner,
                LegacyCommandRequest::new(LegacyProgram::Systemctl)
                    .arg("mask")
                    .arg(unit),
            )?;
        }
        let snapshot_pause = pause_reachable_redis_writes(source, true, true).await?;
        let traffic = verify_frozen_traffic_receipt(spec)?;
        let post_pause_snapshot = post_pause_redis_snapshot(spec, source, true, true).await?;
        require_redis_pause_ack_window(&snapshot_pause)?;
        let stop_pause = pause_reachable_redis_writes(source, true, true).await?;
        require_redis_pause_ack_window(&stop_pause)?;
        let armed_receipt_sha256 = persist_redis_fence_armed_receipt(
            spec,
            head,
            &self.policy.units,
            &report_sha,
            &traffic.receipt_sha256,
            &post_pause_snapshot,
            stop_pause.audit_unix,
        )?;
        let armed = load_redis_fence_armed_receipt(spec, head, &self.policy.units)?
            .ok_or(SourceError::ReceiptInvalid)?;
        if armed.receipt_sha256 != armed_receipt_sha256 {
            return Err(SourceError::ReceiptInvalid);
        }
        require_redis_pause_ack_window(&stop_pause)?;
        stop_masked_redis_units(&mut *self.runner, &redis_units)?;
        finalize_redis_fence(
            spec,
            head,
            &self.policy.units,
            &redis_units,
            &armed_receipt_sha256,
            &mut *self.runner,
        )
        .await
    }

    async fn wait_for_legacy_queues(&mut self, source: &SourceSpec) -> Result<(), SourceError> {
        let deadline = Instant::now() + QUEUE_DRAIN_TIMEOUT;
        loop {
            let (durable_items, ambiguous_keys) = inspect_durable_queues(source).await?;
            if ambiguous_keys != 0 {
                return Err(SourceError::RedisDrainIncomplete);
            }
            if durable_items == 0 {
                return Ok(());
            }
            // A recovery never restarts old workers. If they were already
            // stopped while durable work remains, operator recovery is needed.
            verify_units_running(&mut *self.runner, self.policy.units.workers.iter())?;
            verify_units_stopped(&mut *self.runner, self.policy.units.ingress_writers(), true)?;
            if Instant::now() >= deadline {
                return Err(SourceError::RedisDrainIncomplete);
            }
            tokio::time::sleep(QUEUE_DRAIN_POLL).await;
        }
    }

    pub async fn backup_and_restore_test(
        &mut self,
        spec: &ProvisionSpec,
        head: &ApplyJournalSnapshot,
    ) -> Result<BackupRestoreProof, StageFailure> {
        self.backup_and_restore_test_inner(spec, head)
            .await
            .map_err(|_| {
                stage_failure(
                    ApplyOutcomeCode::BackupInvalid,
                    "encrypted_backup_restore_drill_failed",
                )
            })
    }

    async fn backup_and_restore_test_inner(
        &mut self,
        spec: &ProvisionSpec,
        head: &ApplyJournalSnapshot,
    ) -> Result<BackupRestoreProof, SourceError> {
        perform_backup_restore(spec, head)
            .await
            .map_err(|_| SourceError::Backup)
    }

    pub async fn final_recheck(
        &mut self,
        spec: &ProvisionSpec,
        reviewed_inspect_review_sha256: &str,
        head: &ApplyJournalSnapshot,
        target_empty_report_sha256: &str,
        archived_traffic: &VerifiedFrozenTrafficReceipt,
        archive_materialization_database_url: &str,
    ) -> Result<FinalRecheckProof, StageFailure> {
        self.final_recheck_inner(
            spec,
            reviewed_inspect_review_sha256,
            head,
            target_empty_report_sha256,
            archived_traffic,
            archive_materialization_database_url,
        )
        .await
        .map_err(|error| {
            let outcome = if matches!(error, SourceError::SourceDrift) {
                ApplyOutcomeCode::SourceDrift
            } else if matches!(error, SourceError::TargetEvidence) {
                ApplyOutcomeCode::TargetDrift
            } else {
                ApplyOutcomeCode::FenceUncertain
            };
            stage_failure(outcome, "fenced_final_recheck_failed")
        })
    }

    async fn final_recheck_inner(
        &mut self,
        spec: &ProvisionSpec,
        reviewed_inspect_review_sha256: &str,
        head: &ApplyJournalSnapshot,
        target_empty_report_sha256: &str,
        archived_traffic: &VerifiedFrozenTrafficReceipt,
        archive_materialization_database_url: &str,
    ) -> Result<FinalRecheckProof, SourceError> {
        if !is_lower_sha256(reviewed_inspect_review_sha256)
            || !is_lower_sha256(target_empty_report_sha256)
        {
            return Err(SourceError::TargetEvidence);
        }
        let (source, _, _) = legacy_backup(spec)?;
        let strategy = legacy_conversion_strategy(spec)?;
        verify_units_stopped(&mut *self.runner, self.policy.units.writers(), true)?;
        let (fingerprint, source_schema_sha256) = fingerprint_mysql_and_schema_for_strategy(
            archive_materialization_database_url,
            strategy,
        )
        .await?;
        let archive = verify_persisted_backup_archive(spec).map_err(|_| SourceError::Backup)?;
        if fingerprint != archive.source_fingerprint_sha256()
            || source_schema_sha256 != LEGACY_SEMANTIC_SCHEMA_SHA256
            || archived_traffic.operation_id != spec.operation_id
            || archived_traffic.receipt_sha256 != archive.traffic_receipt_sha256()
            || archived_traffic.sorted_user_delta_count != archive.traffic_sorted_user_delta_count()
            || archived_traffic.sorted_user_delta_sha256
                != archive.traffic_sorted_user_delta_sha256()
            || archived_traffic.upload_delta_sum != archive.traffic_upload_delta_sum()
            || archived_traffic.download_delta_sum != archive.traffic_download_delta_sum()
        {
            return Err(SourceError::SourceDrift);
        }
        ensure_effective_root(&mut *self.runner)?;
        let execution = spec
            .legacy_apply_execution()
            .ok_or(SourceError::InvalidPolicy)?;
        let mysql_unit = &execution.source_control.datastores.mysql.unit;
        let existing_armed = load_datastore_fence_armed_receipt(spec, head, &self.policy.units)?;
        let (armed_receipt_sha256, live_source_observation) =
            if mysql_endpoint_reachable(&source.database_url).await? {
                let (
                    live_fingerprint,
                    live_schema,
                    active_transactions,
                    replication_channels,
                    group_replication_members,
                ) = arm_mysql_durable_write_fence(source, strategy).await?;
                if live_fingerprint != archive.source_fingerprint_sha256()
                    || live_schema != LEGACY_SEMANTIC_SCHEMA_SHA256
                    || active_transactions != 0
                    || replication_channels != 0
                    || group_replication_members != 0
                {
                    return Err(SourceError::SourceDrift);
                }
                (
                    persist_datastore_fence_armed_receipt(
                        spec,
                        head,
                        &self.policy.units,
                        mysql_unit,
                        &archive,
                        &live_fingerprint,
                        &live_schema,
                    )?,
                    "mysql_super_read_only_armed_and_archive_exact",
                )
            } else {
                let receipt = existing_armed.ok_or(SourceError::ReceiptInvalid)?;
                if receipt.payload.backup_receipt_sha256 != archive.receipt_sha256()
                    || receipt.payload.encrypted_backup_sha256 != archive.encrypted_backup_sha256()
                    || receipt.payload.archive_source_fingerprint_sha256 != fingerprint
                    || receipt.payload.archive_source_schema_sha256 != source_schema_sha256
                {
                    return Err(SourceError::SourceDrift);
                }
                verify_units_masked_and_stopped(&mut *self.runner, std::iter::once(mysql_unit))?;
                (
                    receipt.receipt_sha256,
                    "mysql_offline_with_verified_durable_fence_arm",
                )
            };
        run_success(
            &mut *self.runner,
            LegacyCommandRequest::new(LegacyProgram::Systemctl)
                .arg("mask")
                .arg("--now")
                .arg(mysql_unit),
        )?;
        verify_units_masked_and_stopped(&mut *self.runner, std::iter::once(mysql_unit))?;
        for unit in self
            .policy
            .units
            .local_datastores
            .iter()
            .filter(|unit| *unit != mysql_unit)
        {
            // Redis was write-paused and retired as part of SourceDrained.
            // Reissuing the exact mask is idempotent and makes the aggregate
            // final fence explicit before target mutation starts.
            run_success(
                &mut *self.runner,
                LegacyCommandRequest::new(LegacyProgram::Systemctl)
                    .arg("mask")
                    .arg("--now")
                    .arg(unit),
            )?;
        }
        verify_units_masked_and_stopped(
            &mut *self.runner,
            self.policy.units.local_datastores.iter(),
        )?;
        let probes = run_retirement_probes_async((
            source.database_url.clone(),
            source.redis_default_url.clone(),
            source.redis_cache_url.clone(),
        ))
        .await;
        if probes.mysql_reachable || probes.default_redis_reachable || probes.cache_redis_reachable
        {
            return Err(SourceError::ProbeRuntime);
        }
        let datastore_fence_receipt_sha256 = persist_datastore_fence_receipt(
            spec,
            head,
            &self.policy.units,
            &archive,
            &armed_receipt_sha256,
            &probes,
        )?;
        let report_sha = domain_hash_fields(
            b"v2board-fenced-final-recheck-v1\0",
            [
                spec.operation_id.as_bytes(),
                reviewed_inspect_review_sha256.as_bytes(),
                head.event_sha256().as_bytes(),
                fingerprint.as_bytes(),
                source_schema_sha256.as_bytes(),
                archived_traffic.receipt_sha256.as_bytes(),
                archived_traffic.sorted_user_delta_sha256.as_bytes(),
                archived_traffic.upload_delta_sum.as_bytes(),
                archived_traffic.download_delta_sum.as_bytes(),
                target_empty_report_sha256.as_bytes(),
                self.policy.units.digest().as_bytes(),
                live_source_observation.as_bytes(),
                datastore_fence_receipt_sha256.as_bytes(),
            ],
        );
        FinalRecheckProof::new(report_sha, fingerprint, true, true)
            .map_err(|_| SourceError::SourceDrift)
    }

    /// Irreversibly disables all declared local legacy services. Schema v4
    /// supports no externally-managed source here: direct probes using the old
    /// credentials must fail after every dedicated unit is disabled.
    pub fn retire_local_source(
        &mut self,
        spec: &ProvisionSpec,
        permit: &DurableMutationPermit,
    ) -> Result<VerifiedStageProof, StageFailure> {
        self.retire_local_source_inner(spec, permit).map_err(|_| {
            stage_failure(
                ApplyOutcomeCode::RetirementFailed,
                "local_source_retirement_failed",
            )
        })
    }

    fn retire_local_source_inner(
        &mut self,
        spec: &ProvisionSpec,
        permit: &DurableMutationPermit,
    ) -> Result<VerifiedStageProof, SourceError> {
        let authority = permit
            .native_authority_binding()
            .ok_or(SourceError::ReceiptInvalid)?;
        if permit.operation_id() != spec.operation_id
            || permit.generation() == 0
            || !is_lower_sha256(permit.event_sha256())
        {
            return Err(SourceError::ReceiptInvalid);
        }
        let source = legacy_source(spec)?;
        ensure_effective_root(&mut *self.runner)?;
        for unit in self.policy.units.all_retirement_units() {
            run_success(
                &mut *self.runner,
                LegacyCommandRequest::new(LegacyProgram::Systemctl)
                    .arg("mask")
                    .arg("--now")
                    .arg(unit),
            )?;
        }
        verify_units_masked_and_stopped(
            &mut *self.runner,
            self.policy.units.all_retirement_units(),
        )?;
        let probes = run_retirement_probes((
            source.database_url.clone(),
            source.redis_default_url.clone(),
            source.redis_cache_url.clone(),
        ))?;
        if probes.mysql_reachable || probes.default_redis_reachable || probes.cache_redis_reachable
        {
            return Err(SourceError::ProbeRuntime);
        }
        let receipt_sha256 = persist_source_retirement_receipt(
            spec,
            permit,
            &self.policy.units,
            authority.nodes_verified_event_sha256(),
            &probes,
        )?;
        VerifiedStageProof::new(receipt_sha256).map_err(|_| SourceError::ReceiptInvalid)
    }
}

fn require_redis_pause_ack_window(ack: &RedisPauseAck) -> Result<(), SourceError> {
    let pause_duration = Duration::from_millis(REDIS_WRITE_PAUSE_MILLISECONDS);
    if ack
        .monotonic
        .elapsed()
        .checked_add(COMMAND_TIMEOUT)
        .is_none_or(|required| required >= pause_duration)
    {
        return Err(SourceError::RedisDrainIncomplete);
    }
    Ok(())
}

async fn mysql_endpoint_reachable(database_url: &str) -> Result<bool, SourceError> {
    let pool = match MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(DATASTORE_TIMEOUT)
        .connect(database_url)
        .await
    {
        Ok(pool) => pool,
        Err(_) => return Ok(false),
    };
    let result = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&pool)
        .await;
    pool.close().await;
    result
        .map(|value| value == 1)
        .map_err(|_| SourceError::Mysql)
}

async fn arm_mysql_durable_write_fence(
    source: &SourceSpec,
    strategy: LegacyConversionStrategy,
) -> Result<(String, String, u64, u64, u64), SourceError> {
    let fence_url = source
        .database_fence_url
        .as_deref()
        .ok_or(SourceError::InvalidPolicy)?;
    match strategy {
        LegacyConversionStrategy::PreserveAll => {
            arm_mysql_durable_write_fence_urls(fence_url, &source.database_url).await
        }
        LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs => {
            arm_mysql_durable_write_fence_urls_for_strategy(
                fence_url,
                &source.database_url,
                strategy,
            )
            .await
        }
    }
}

async fn arm_mysql_durable_write_fence_urls(
    fence_url: &str,
    reader_url: &str,
) -> Result<(String, String, u64, u64, u64), SourceError> {
    arm_mysql_durable_write_fence_urls_for_strategy(
        fence_url,
        reader_url,
        LegacyConversionStrategy::PreserveAll,
    )
    .await
}

async fn arm_mysql_durable_write_fence_urls_for_strategy(
    fence_url: &str,
    reader_url: &str,
    strategy: LegacyConversionStrategy,
) -> Result<(String, String, u64, u64, u64), SourceError> {
    let fence_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(DATASTORE_TIMEOUT)
        .connect(fence_url)
        .await
        .map_err(|_| SourceError::Mysql)?;
    sqlx::raw_sql("SET PERSIST super_read_only = ON")
        .execute(&fence_pool)
        .await
        .map_err(|_| SourceError::Mysql)?;
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let active_transactions = loop {
        let (read_only, super_read_only, active): (i64, i64, i64) = sqlx::query_as(
            "SELECT @@GLOBAL.read_only + 0, \
                    @@GLOBAL.super_read_only + 0, \
                    (SELECT COUNT(*) FROM information_schema.innodb_trx)",
        )
        .fetch_one(&fence_pool)
        .await
        .map_err(|_| SourceError::Mysql)?;
        if read_only != 1 || super_read_only != 1 || active < 0 {
            return Err(SourceError::Mysql);
        }
        if active == 0 {
            break 0_u64;
        }
        if Instant::now() >= deadline {
            return Err(SourceError::SourceDrift);
        }
        tokio::time::sleep(QUEUE_DRAIN_POLL).await;
    };
    let (fingerprint, schema) =
        fingerprint_mysql_and_schema_for_strategy(reader_url, strategy).await?;
    let source_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(DATASTORE_TIMEOUT)
        .connect(reader_url)
        .await
        .map_err(|_| SourceError::Mysql)?;
    let replication_channels = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM performance_schema.replication_connection_status",
    )
    .fetch_one(&source_pool)
    .await
    .map_err(|_| SourceError::Mysql)?;
    let group_replication_members = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM performance_schema.replication_group_members",
    )
    .fetch_one(&source_pool)
    .await
    .map_err(|_| SourceError::Mysql)?;
    source_pool.close().await;
    let (read_only, super_read_only, active): (i64, i64, i64) = sqlx::query_as(
        "SELECT @@GLOBAL.read_only + 0, \
                @@GLOBAL.super_read_only + 0, \
                (SELECT COUNT(*) FROM information_schema.innodb_trx)",
    )
    .fetch_one(&fence_pool)
    .await
    .map_err(|_| SourceError::Mysql)?;
    fence_pool.close().await;
    if read_only != 1
        || super_read_only != 1
        || active != 0
        || replication_channels != 0
        || group_replication_members != 0
    {
        return Err(SourceError::SourceDrift);
    }
    Ok((
        fingerprint,
        schema,
        active_transactions,
        u64::try_from(replication_channels).map_err(|_| SourceError::Mysql)?,
        u64::try_from(group_replication_members).map_err(|_| SourceError::Mysql)?,
    ))
}

#[derive(Clone)]
pub struct BareMetalRetirementObserver;

impl BareMetalRetirementObserver {
    pub fn from_manifest(spec: &ProvisionSpec) -> Result<Self, SourceError> {
        let policy = LegacySourcePolicy::from_manifest(spec)?;
        policy.validate()?;
        Ok(Self)
    }
}

impl LegacySourceRetirementObserver for BareMetalRetirementObserver {
    fn observe(
        &mut self,
        request: &LegacySourceRetirementRequest<'_>,
    ) -> Result<ObservedLegacySourceRetirement, ExecutorError> {
        let source = request.source();
        let urls = (
            source.database_url.clone(),
            source.redis_default_url.clone(),
            source.redis_cache_url.clone(),
        );
        let probes = thread::spawn(move || run_retirement_probes(urls))
            .join()
            .map_err(|_| ExecutorError::sanitized("source_probe_thread_failed"))?
            .map_err(|_| ExecutorError::sanitized("source_probe_runtime_failed"))?;
        Ok(ObservedLegacySourceRetirement {
            mysql_reachable_with_old_credentials: probes.mysql_reachable,
            source_default_redis_reachable_with_old_credentials: probes.default_redis_reachable,
            source_cache_redis_reachable_with_old_credentials: probes.cache_redis_reachable,
            source_access_permanently_disabled: !probes.mysql_reachable
                && !probes.default_redis_reachable
                && !probes.cache_redis_reachable,
            mysql_probe_evidence: format!("probe:mysql:{}", probes.mysql_evidence_sha256),
            source_redis_probe_evidence: format!(
                "probe:redis:{}:{}",
                probes.default_redis_evidence_sha256, probes.cache_redis_evidence_sha256
            ),
            credential_probe_evidence: format!(
                "old-credential-probes:{}:{}:{}",
                probes.mysql_evidence_sha256,
                probes.default_redis_evidence_sha256,
                probes.cache_redis_evidence_sha256
            ),
        })
    }
}

/// Terminal verifier used immediately before the permanent completion ledger.
/// An unreachable socket alone is never sufficient: the exact HMAC receipt,
/// journal predecessor, native authority, systemd mask state, and fresh
/// old-credential probes must all agree.
pub fn verify_source_retirement_for_completion(
    spec: &ProvisionSpec,
    permit: &CompletionRecoveryPermit,
) -> Result<ObservedLegacySourceRetirement, SourceError> {
    if permit.operation_id() != spec.operation_id {
        return Err(SourceError::ReceiptInvalid);
    }
    let policy = LegacySourcePolicy::from_manifest(spec)?;
    policy.validate()?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let receipt_file = existing_receipt_file(&execution.receipts.source_retirement_path)?
        .ok_or(SourceError::ReceiptInvalid)?;
    let bytes = read_owner_only_file(&receipt_file, MAX_RECEIPT_BYTES)?;
    let envelope: SourceRetirementReceiptEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let payload = envelope.payload;
    let authority = permit.native_authority_binding();
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::SourceRetirement,
        &canonical,
        &envelope.hmac_sha256,
    ) || hex::encode(Sha256::digest(&bytes)) != permit.source_retirement_report_sha256()
        || payload.schema_version != 1
        || payload.operation_id != spec.operation_id
        || payload.journal_anchor_checkpoint != ApplyCheckpoint::CutoverCommitted
        || payload.result_checkpoint != ApplyCheckpoint::SourceRetired
        || payload.journal_anchor_generation.checked_add(1)
            != Some(permit.source_retired_generation())
        || payload.journal_anchor_event_sha256 != permit.source_retired_predecessor_event_sha256()
        || payload.native_authority_nodes_event_sha256 != authority.nodes_verified_event_sha256()
        || payload.unit_inventory_sha256 != policy.units.digest()
        || !payload.all_declared_units_masked_and_inactive
        || !payload.mysql_unreachable_with_old_credentials
        || !payload.default_redis_unreachable_with_old_credentials
        || !payload.cache_redis_unreachable_with_old_credentials
        || payload.retired_at_unix <= 0
    {
        return Err(SourceError::ReceiptInvalid);
    }

    let mut runner = ProcessLegacyCommandRunner;
    ensure_effective_root(&mut runner)?;
    verify_units_masked_and_stopped(&mut runner, policy.units.all_retirement_units())?;
    let source = legacy_source(spec)?;
    let probes = run_retirement_probes((
        source.database_url.clone(),
        source.redis_default_url.clone(),
        source.redis_cache_url.clone(),
    ))?;
    if probes.mysql_reachable
        || probes.default_redis_reachable
        || probes.cache_redis_reachable
        || probes.mysql_evidence_sha256 != payload.mysql_probe_sha256
        || probes.default_redis_evidence_sha256 != payload.default_redis_probe_sha256
        || probes.cache_redis_evidence_sha256 != payload.cache_redis_probe_sha256
    {
        return Err(SourceError::ProbeRuntime);
    }
    Ok(ObservedLegacySourceRetirement {
        mysql_reachable_with_old_credentials: false,
        source_default_redis_reachable_with_old_credentials: false,
        source_cache_redis_reachable_with_old_credentials: false,
        source_access_permanently_disabled: true,
        mysql_probe_evidence: format!("probe:mysql:{}", probes.mysql_evidence_sha256),
        source_redis_probe_evidence: format!(
            "probe:redis:{}:{}",
            probes.default_redis_evidence_sha256, probes.cache_redis_evidence_sha256
        ),
        credential_probe_evidence: format!(
            "old-credential-probes:{}:{}:{}",
            probes.mysql_evidence_sha256,
            probes.default_redis_evidence_sha256,
            probes.cache_redis_evidence_sha256
        ),
    })
}

fn legacy_source(spec: &ProvisionSpec) -> Result<&SourceSpec, SourceError> {
    match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => Ok(source),
        _ => Err(SourceError::WrongProvisionKind),
    }
}

fn legacy_conversion_strategy(
    spec: &ProvisionSpec,
) -> Result<LegacyConversionStrategy, SourceError> {
    LegacyConversionStrategy::for_schema_version(spec.schema_version)
        .map_err(|_| SourceError::InvalidPolicy)
}

fn legacy_backup(spec: &ProvisionSpec) -> Result<(&SourceSpec, &str, &str), SourceError> {
    let source = legacy_source(spec)?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    Ok((
        source,
        &execution.backup.backup_reference,
        &execution.backup.isolated_restore_database_url,
    ))
}

fn stage_failure(outcome: ApplyOutcomeCode, code: &'static str) -> StageFailure {
    StageFailure::sanitized(outcome, code)
}

fn valid_systemd_unit(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && (value.ends_with(".service") || value.ends_with(".timer"))
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@' | b':')
        })
}

fn exact_systemd_unit_relation(value: &str, expected: &str) -> bool {
    let mut units = value.split_ascii_whitespace();
    units.next() == Some(expected) && units.next().is_none()
}

fn ensure_effective_root<R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
) -> Result<(), SourceError> {
    let output = run_success(
        runner,
        LegacyCommandRequest::new(LegacyProgram::Id).arg("-u"),
    )?;
    if strict_text(&output.stdout)? != "0" {
        return Err(SourceError::Command);
    }
    Ok(())
}

fn run_success<R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    request: LegacyCommandRequest,
) -> Result<LegacyCommandOutput, SourceError> {
    let output = runner.run(request).map_err(|_| SourceError::Command)?;
    if output.exit_code != 0 {
        return Err(SourceError::Command);
    }
    Ok(output)
}

fn verify_units_stopped<'a, R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    units: impl Iterator<Item = &'a String>,
    require_disabled: bool,
) -> Result<(), SourceError> {
    for unit in units {
        let output = run_success(
            runner,
            LegacyCommandRequest::new(LegacyProgram::Systemctl)
                .arg("show")
                .arg("--property=LoadState")
                .arg("--property=ActiveState")
                .arg("--property=SubState")
                .arg("--property=UnitFileState")
                .arg("--value")
                .arg(unit),
        )?;
        let text = strict_text(&output.stdout)?;
        let fields = text.lines().collect::<Vec<_>>();
        let stopped = fields.len() == 4
            && fields[0] == "loaded"
            && fields[1] == "inactive"
            && matches!(fields[2], "dead" | "failed" | "exited")
            && (!require_disabled || matches!(fields[3], "disabled" | "masked"));
        if !stopped {
            return Err(SourceError::UnitStillActive);
        }
    }
    Ok(())
}

fn verify_units_running<'a, R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    units: impl Iterator<Item = &'a String>,
) -> Result<(), SourceError> {
    for unit in units {
        let output = run_success(
            runner,
            LegacyCommandRequest::new(LegacyProgram::Systemctl)
                .arg("show")
                .arg("--property=LoadState")
                .arg("--property=ActiveState")
                .arg("--property=SubState")
                .arg("--value")
                .arg(unit),
        )?;
        let text = strict_text(&output.stdout)?;
        let fields = text.lines().collect::<Vec<_>>();
        if fields.len() != 3
            || fields[0] != "loaded"
            || fields[1] != "active"
            || !matches!(fields[2], "running" | "exited")
        {
            return Err(SourceError::UnitStillActive);
        }
    }
    Ok(())
}

fn verify_units_restart_disabled<'a, R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    units: impl Iterator<Item = &'a String>,
) -> Result<(), SourceError> {
    for unit in units {
        let output = run_success(
            runner,
            LegacyCommandRequest::new(LegacyProgram::Systemctl)
                .arg("show")
                .arg("--property=LoadState")
                .arg("--property=UnitFileState")
                .arg("--value")
                .arg(unit),
        )?;
        let fields = strict_text(&output.stdout)?.lines().collect::<Vec<_>>();
        if fields.len() != 2 || fields[0] != "loaded" || !matches!(fields[1], "disabled" | "masked")
        {
            return Err(SourceError::UnitStillActive);
        }
    }
    Ok(())
}

fn verify_units_masked_and_stopped<'a, R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    units: impl Iterator<Item = &'a String>,
) -> Result<(), SourceError> {
    for unit in units {
        let output = run_success(
            runner,
            LegacyCommandRequest::new(LegacyProgram::Systemctl)
                .arg("show")
                .arg("--property=LoadState")
                .arg("--property=ActiveState")
                .arg("--property=SubState")
                .arg("--property=UnitFileState")
                .arg("--value")
                .arg(unit),
        )?;
        let fields = strict_text(&output.stdout)?.lines().collect::<Vec<_>>();
        if fields.len() != 4
            || !matches!(fields[0], "loaded" | "masked")
            || fields[1] != "inactive"
            || !matches!(fields[2], "dead" | "failed" | "exited")
            || fields[3] != "masked"
        {
            return Err(SourceError::UnitStillActive);
        }
    }
    Ok(())
}

fn strict_text(bytes: &[u8]) -> Result<&str, SourceError> {
    if bytes.is_empty() || bytes.len() > MAX_OUTPUT_BYTES || bytes.contains(&0) {
        return Err(SourceError::Command);
    }
    std::str::from_utf8(bytes)
        .map(str::trim)
        .map_err(|_| SourceError::Command)
}

fn validated_program_path(program: LegacyProgram) -> Result<PathBuf, LegacyRunnerError> {
    let canonical =
        fs::canonicalize(program.path()).map_err(|_| LegacyRunnerError::UnsafeProgram)?;
    if !canonical.is_absolute()
        || !(canonical.starts_with("/usr/bin") || canonical.starts_with("/bin"))
    {
        return Err(LegacyRunnerError::UnsafeProgram);
    }
    let metadata = fs::metadata(&canonical).map_err(|_| LegacyRunnerError::UnsafeProgram)?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.permissions().mode() & 0o111 == 0
    {
        return Err(LegacyRunnerError::UnsafeProgram);
    }
    Ok(canonical)
}

fn parse_systemctl_properties(bytes: &[u8]) -> Result<BTreeMap<String, String>, SourceError> {
    let text = strict_text(bytes)?;
    let mut properties = BTreeMap::new();
    for line in text.lines() {
        let (name, value) = line
            .split_once('=')
            .ok_or(SourceError::UnsafeUnitIdentity)?;
        if name.is_empty()
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric())
            || properties
                .insert(name.to_string(), value.to_string())
                .is_some()
        {
            return Err(SourceError::UnsafeUnitIdentity);
        }
    }
    Ok(properties)
}

fn hash_root_owned_regular_file(
    path: &Path,
    maximum_bytes: u64,
) -> Result<(PathBuf, String), SourceError> {
    if !path.is_absolute() || maximum_bytes == 0 {
        return Err(SourceError::UnsafeUnitIdentity);
    }
    let canonical = fs::canonicalize(path).map_err(|_| SourceError::UnsafeUnitIdentity)?;
    if !canonical.is_absolute() {
        return Err(SourceError::UnsafeUnitIdentity);
    }
    require_root_owned_ancestor_chain(&canonical)?;
    let mut file = File::open(&canonical).map_err(|_| SourceError::UnsafeUnitIdentity)?;
    let before = file
        .metadata()
        .map_err(|_| SourceError::UnsafeUnitIdentity)?;
    let path_before =
        fs::symlink_metadata(&canonical).map_err(|_| SourceError::UnsafeUnitIdentity)?;
    if !before.is_file()
        || !path_before.is_file()
        || path_before.file_type().is_symlink()
        || path_before.dev() != before.dev()
        || path_before.ino() != before.ino()
        || before.uid() != 0
        || before.permissions().mode() & 0o022 != 0
        || before.len() == 0
        || before.len() > maximum_bytes
    {
        return Err(SourceError::UnsafeUnitIdentity);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| SourceError::UnsafeUnitIdentity)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(SourceError::UnsafeUnitIdentity)?;
        if total > maximum_bytes {
            return Err(SourceError::UnsafeUnitIdentity);
        }
        digest.update(&buffer[..read]);
    }
    let after = file
        .metadata()
        .map_err(|_| SourceError::UnsafeUnitIdentity)?;
    let path_after =
        fs::symlink_metadata(&canonical).map_err(|_| SourceError::UnsafeUnitIdentity)?;
    if total != before.len()
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || !path_after.is_file()
        || path_after.file_type().is_symlink()
        || path_after.dev() != before.dev()
        || path_after.ino() != before.ino()
    {
        return Err(SourceError::UnsafeUnitIdentity);
    }
    Ok((canonical, hex::encode(digest.finalize())))
}

fn require_root_owned_ancestor_chain(path: &Path) -> Result<(), SourceError> {
    let mut current = path.parent();
    while let Some(directory) = current {
        let metadata =
            fs::symlink_metadata(directory).map_err(|_| SourceError::UnsafeUnitIdentity)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != 0
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(SourceError::UnsafeUnitIdentity);
        }
        current = directory.parent();
    }
    Ok(())
}

fn canonical_cgroup_v2_root() -> Result<PathBuf, SourceError> {
    let root = fs::canonicalize("/sys/fs/cgroup").map_err(|_| SourceError::CgroupIdentity)?;
    let controllers = root.join("cgroup.controllers");
    if !root.is_dir() || !controllers.is_file() {
        return Err(SourceError::CgroupIdentity);
    }
    Ok(root)
}

fn collect_cgroup_process_ids(
    cgroup_root: &Path,
    control_group: &str,
) -> Result<BTreeSet<u32>, SourceError> {
    if !control_group.starts_with('/')
        || control_group.contains("//")
        || control_group.split('/').any(|part| part == "..")
    {
        return Err(SourceError::CgroupIdentity);
    }
    let group = fs::canonicalize(cgroup_root.join(control_group.trim_start_matches('/')))
        .map_err(|_| SourceError::CgroupIdentity)?;
    if !group.starts_with(cgroup_root) || !group.is_dir() {
        return Err(SourceError::CgroupIdentity);
    }
    let mut stack = vec![group];
    let mut visited = 0_usize;
    let mut processes = BTreeSet::new();
    while let Some(directory) = stack.pop() {
        visited = visited.checked_add(1).ok_or(SourceError::CgroupIdentity)?;
        if visited > MAX_CGROUP_DIRECTORIES {
            return Err(SourceError::CgroupIdentity);
        }
        let process_text = read_bounded_utf8_file(
            &directory.join("cgroup.procs"),
            MAX_CGROUP_PROCESSES.saturating_mul(16),
        )?;
        for line in process_text.lines() {
            let pid = line
                .parse::<u32>()
                .ok()
                .filter(|pid| *pid != 0)
                .ok_or(SourceError::CgroupIdentity)?;
            processes.insert(pid);
            if processes.len() > MAX_CGROUP_PROCESSES {
                return Err(SourceError::CgroupIdentity);
            }
        }
        for entry in fs::read_dir(&directory).map_err(|_| SourceError::CgroupIdentity)? {
            let entry = entry.map_err(|_| SourceError::CgroupIdentity)?;
            let kind = entry.file_type().map_err(|_| SourceError::CgroupIdentity)?;
            if kind.is_symlink() {
                return Err(SourceError::CgroupIdentity);
            }
            if kind.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    if processes.is_empty() {
        return Err(SourceError::CgroupIdentity);
    }
    Ok(processes)
}

fn collect_process_socket_inodes(
    process_ids: &BTreeSet<u32>,
) -> Result<BTreeSet<u64>, SourceError> {
    let mut sockets = BTreeSet::new();
    let mut descriptors = 0_usize;
    for pid in process_ids {
        let directory =
            fs::read_dir(format!("/proc/{pid}/fd")).map_err(|_| SourceError::CgroupIdentity)?;
        for entry in directory {
            let entry = entry.map_err(|_| SourceError::CgroupIdentity)?;
            descriptors = descriptors
                .checked_add(1)
                .ok_or(SourceError::CgroupIdentity)?;
            if descriptors > MAX_PROCESS_FILE_DESCRIPTORS {
                return Err(SourceError::CgroupIdentity);
            }
            let target = fs::read_link(entry.path()).map_err(|_| SourceError::CgroupIdentity)?;
            let Some(target) = target.to_str() else {
                continue;
            };
            if let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|value| value.strip_suffix(']'))
            {
                let inode = inode
                    .parse::<u64>()
                    .map_err(|_| SourceError::CgroupIdentity)?;
                sockets.insert(inode);
            }
        }
    }
    Ok(sockets)
}

fn source_listener_endpoint(value: &str) -> Result<(IpAddr, u16), SourceError> {
    let url = Url::parse(value).map_err(|_| SourceError::InvalidPolicy)?;
    let address = url
        .host_str()
        .and_then(|host| {
            host.strip_prefix('[')
                .and_then(|host| host.strip_suffix(']'))
                .unwrap_or(host)
                .parse::<IpAddr>()
                .ok()
        })
        .filter(IpAddr::is_loopback)
        .ok_or(SourceError::InvalidPolicy)?;
    let default_port = match url.scheme() {
        "mysql" => 3306,
        "redis" | "rediss" => 6379,
        _ => return Err(SourceError::InvalidPolicy),
    };
    Ok((address, url.port().unwrap_or(default_port)))
}

fn listener_inodes_owned_by_unit(
    runtime: &PreflightUnitRuntime,
    address: IpAddr,
    port: u16,
) -> Result<BTreeSet<u64>, SourceError> {
    let namespace_pid = runtime
        .process_ids
        .iter()
        .next()
        .ok_or(SourceError::CgroupIdentity)?;
    let table = match address {
        IpAddr::V4(_) => format!("/proc/{namespace_pid}/net/tcp"),
        IpAddr::V6(_) => format!("/proc/{namespace_pid}/net/tcp6"),
    };
    let rows = parse_proc_tcp_listeners(&read_bounded_utf8_file(
        Path::new(&table),
        16 * 1024 * 1024,
    )?)?;
    Ok(rows
        .into_iter()
        .filter(|row| {
            row.address == address && row.port == port && runtime.socket_inodes.contains(&row.inode)
        })
        .map(|row| row.inode)
        .collect())
}

struct ProcTcpListener {
    address: IpAddr,
    port: u16,
    inode: u64,
}

fn parse_proc_tcp_listeners(text: &str) -> Result<Vec<ProcTcpListener>, SourceError> {
    let mut listeners = Vec::new();
    for line in text.lines().skip(1) {
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 || fields[3] != "0A" {
            continue;
        }
        let (address_hex, port_hex) = fields[1]
            .split_once(':')
            .ok_or(SourceError::CgroupIdentity)?;
        let address = parse_proc_address(address_hex)?;
        let port = u16::from_str_radix(port_hex, 16).map_err(|_| SourceError::CgroupIdentity)?;
        let inode = fields[9]
            .parse::<u64>()
            .map_err(|_| SourceError::CgroupIdentity)?;
        listeners.push(ProcTcpListener {
            address,
            port,
            inode,
        });
    }
    Ok(listeners)
}

fn parse_proc_address(value: &str) -> Result<IpAddr, SourceError> {
    match value.len() {
        8 => {
            let encoded =
                u32::from_str_radix(value, 16).map_err(|_| SourceError::CgroupIdentity)?;
            Ok(IpAddr::V4(encoded.to_le_bytes().into()))
        }
        32 => {
            let mut octets = [0_u8; 16];
            for (index, chunk) in value.as_bytes().chunks_exact(8).enumerate() {
                let chunk = std::str::from_utf8(chunk).map_err(|_| SourceError::CgroupIdentity)?;
                let encoded =
                    u32::from_str_radix(chunk, 16).map_err(|_| SourceError::CgroupIdentity)?;
                octets[index * 4..index * 4 + 4].copy_from_slice(&encoded.to_le_bytes());
            }
            Ok(IpAddr::V6(octets.into()))
        }
        _ => Err(SourceError::CgroupIdentity),
    }
}

fn read_bounded_utf8_file(path: &Path, maximum_bytes: usize) -> Result<String, SourceError> {
    if maximum_bytes == 0 {
        return Err(SourceError::CgroupIdentity);
    }
    let file = File::open(path).map_err(|_| SourceError::CgroupIdentity)?;
    let mut bytes = Vec::new();
    file.take((maximum_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| SourceError::CgroupIdentity)?;
    if bytes.len() > maximum_bytes || bytes.contains(&0) {
        return Err(SourceError::CgroupIdentity);
    }
    String::from_utf8(bytes).map_err(|_| SourceError::CgroupIdentity)
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
        overflow |= retained < read;
    }
    Ok((bytes, overflow))
}

fn redact_all(bytes: &mut Vec<u8>, secrets: &[Vec<u8>]) {
    for secret in secrets {
        while let Some(index) = bytes.windows(secret.len()).position(|part| part == secret) {
            bytes.splice(index..index + secret.len(), b"[REDACTED]".iter().copied());
        }
    }
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn domain_hash_fields<'a>(domain: &[u8], fields: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest_field(&mut digest, field);
    }
    hex::encode(digest.finalize())
}

fn digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

async fn redis_connection(url: &str) -> Result<MultiplexedConnection, SourceError> {
    let client = redis::Client::open(url).map_err(|_| SourceError::Redis)?;
    timeout(DATASTORE_TIMEOUT, client.get_multiplexed_async_connection())
        .await
        .map_err(|_| SourceError::Redis)?
        .map_err(|_| SourceError::Redis)
}

async fn scan_keys(
    connection: &mut MultiplexedConnection,
    pattern: &[u8],
) -> Result<Vec<Vec<u8>>, SourceError> {
    let mut cursor = 0_u64;
    let mut keys = BTreeSet::new();
    loop {
        let (next, batch) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(REDIS_SCAN_COUNT)
            .query_async::<(u64, Vec<Vec<u8>>)>(connection)
            .await
            .map_err(|_| SourceError::Redis)?;
        keys.extend(batch);
        if next == 0 {
            break;
        }
        cursor = next;
    }
    Ok(keys.into_iter().collect())
}

async fn collection_count(
    connection: &mut MultiplexedConnection,
    key: &[u8],
) -> Result<u64, SourceError> {
    let kind = redis::cmd("TYPE")
        .arg(key)
        .query_async::<String>(connection)
        .await
        .map_err(|_| SourceError::Redis)?;
    let command = match kind.as_str() {
        "none" => return Ok(0),
        "list" => "LLEN",
        "zset" => "ZCARD",
        "set" => "SCARD",
        "hash" => "HLEN",
        _ => return Ok(1),
    };
    redis::cmd(command)
        .arg(key)
        .query_async(connection)
        .await
        .map_err(|_| SourceError::Redis)
}

async fn inspect_durable_queues(source: &SourceSpec) -> Result<(u64, u64), SourceError> {
    let mut connection = redis_connection(&source.redis_default_url).await?;
    let all = scan_keys(&mut connection, b"*queues:*")
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let owned_prefix = format!("{}queues:", source.redis_connection_prefix).into_bytes();
    let pattern = [owned_prefix.as_slice(), b"*"].concat();
    let owned = scan_keys(&mut connection, &pattern)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut durable_items = 0_u64;
    for key in &owned {
        if !key.ends_with(b":notify") {
            durable_items =
                durable_items.saturating_add(collection_count(&mut connection, key).await?);
        }
    }
    Ok((
        durable_items,
        u64::try_from(all.difference(&owned).count()).unwrap_or(u64::MAX),
    ))
}

async fn unlink_exact(
    connection: &mut MultiplexedConnection,
    keys: &[Vec<u8>],
) -> Result<u64, SourceError> {
    let mut total = 0_u64;
    for batch in keys.chunks(REDIS_DELETE_BATCH) {
        if batch.is_empty() {
            continue;
        }
        let mut command = redis::cmd("UNLINK");
        for key in batch {
            command.arg(key);
        }
        let removed = command
            .query_async::<u64>(connection)
            .await
            .map_err(|_| SourceError::Redis)?;
        total = total.saturating_add(removed);
    }
    Ok(total)
}

fn physical_cache_prefix(source: &SourceSpec) -> Result<Vec<u8>, SourceError> {
    if source.redis_connection_prefix.is_empty() || source.redis_cache_prefix.is_empty() {
        return Err(SourceError::RedisDrainIncomplete);
    }
    Ok(format!(
        "{}{}:",
        source.redis_connection_prefix, source.redis_cache_prefix
    )
    .into_bytes())
}

fn source_horizon_physical_prefix(source: &SourceSpec) -> Result<String, SourceError> {
    if source.redis_connection_prefix.is_empty() || source.redis_horizon_prefix.is_empty() {
        return Err(SourceError::RedisDrainIncomplete);
    }
    Ok(format!(
        "{}{}",
        source.redis_connection_prefix, source.redis_horizon_prefix
    ))
}

fn redis_identity(url: &str) -> Result<(String, u16, i64), SourceError> {
    let parsed = Url::parse(url).map_err(|_| SourceError::Redis)?;
    let host = parsed
        .host_str()
        .ok_or(SourceError::Redis)?
        .to_ascii_lowercase();
    let port = parsed.port_or_known_default().ok_or(SourceError::Redis)?;
    let db = parsed
        .path()
        .trim_start_matches('/')
        .parse::<i64>()
        .unwrap_or(0);
    Ok((host, port, db))
}

fn redis_process_identity(url: &str) -> Result<(String, u16), SourceError> {
    let (host, port, _) = redis_identity(url)?;
    Ok((host, port))
}

fn redis_units(spec: &ProvisionSpec) -> Result<Vec<String>, SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::InvalidPolicy)?;
    Ok([
        execution
            .source_control
            .datastores
            .default_redis
            .unit
            .clone(),
        execution.source_control.datastores.cache_redis.unit.clone(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect())
}

struct RedisPauseAck {
    audit_unix: i64,
    monotonic: Instant,
}

async fn pause_reachable_redis_writes(
    source: &SourceSpec,
    default_reachable: bool,
    cache_reachable: bool,
) -> Result<RedisPauseAck, SourceError> {
    let same_process = redis_process_identity(&source.redis_default_url)?
        == redis_process_identity(&source.redis_cache_url)?;
    if same_process && default_reachable != cache_reachable {
        return Err(SourceError::ProbeRuntime);
    }
    let mut urls = Vec::new();
    if default_reachable {
        urls.push(source.redis_default_url.as_str());
    }
    if cache_reachable && !same_process {
        urls.push(source.redis_cache_url.as_str());
    }
    let mut earliest_ack = None;
    for url in urls {
        let mut connection = redis_connection(url).await?;
        redis::cmd("CLIENT")
            .arg("PAUSE")
            .arg(REDIS_WRITE_PAUSE_MILLISECONDS)
            .arg("WRITE")
            .query_async::<()>(&mut connection)
            .await
            .map_err(|_| SourceError::Redis)?;
        if earliest_ack.is_none() {
            earliest_ack = Some(RedisPauseAck {
                audit_unix: unix_now()?,
                monotonic: Instant::now(),
            });
        }
    }
    earliest_ack.ok_or(SourceError::ProbeRuntime)
}

struct PostPauseRedisSnapshot {
    source_default_run_id: String,
    source_cache_run_id: String,
    default_sha256: String,
    cache_sha256: String,
    composite_sha256: String,
}

async fn post_pause_redis_snapshot(
    spec: &ProvisionSpec,
    source: &SourceSpec,
    default_reachable: bool,
    cache_reachable: bool,
) -> Result<PostPauseRedisSnapshot, SourceError> {
    if !default_reachable || !cache_reachable {
        return Err(SourceError::ProbeRuntime);
    }
    let receipt = verify_frozen_traffic_receipt(spec)?;
    let mut expected_upload = BTreeMap::new();
    let mut expected_download = BTreeMap::new();
    for FrozenTrafficDeltaRecord(user_id, upload, download) in &receipt.deltas {
        if let Some(upload) = upload {
            expected_upload.insert(*user_id, i128::from(*upload));
        }
        if let Some(download) = download {
            expected_download.insert(*user_id, i128::from(*download));
        }
    }
    let mut default = redis_connection(&source.redis_default_url).await?;
    let default_run_id = redis_run_id(&mut default).await?;
    let observed_upload =
        read_traffic_hash(&mut default, receipt.frozen_upload_key.as_bytes()).await?;
    let observed_download =
        read_traffic_hash(&mut default, receipt.frozen_download_key.as_bytes()).await?;
    if default_run_id != receipt.source_default_run_id
        || observed_upload != expected_upload
        || observed_download != expected_download
    {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let default_keys = scan_keys(&mut default, b"*")
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut expected_default_keys = BTreeSet::new();
    if !expected_upload.is_empty() {
        expected_default_keys.insert(receipt.frozen_upload_key.as_bytes().to_vec());
    }
    if !expected_download.is_empty() {
        expected_default_keys.insert(receipt.frozen_download_key.as_bytes().to_vec());
    }
    if default_keys != expected_default_keys {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let same_database =
        redis_identity(&source.redis_default_url)? == redis_identity(&source.redis_cache_url)?;
    let (cache_run_id, cache_sha256) = if same_database {
        (
            default_run_id.clone(),
            domain_hash_fields(
                b"v2board-post-pause-redis-cache-v1\0",
                [default_run_id.as_bytes(), b"same_logical_database"],
            ),
        )
    } else {
        let mut cache = redis_connection(&source.redis_cache_url).await?;
        let cache_run_id = redis_run_id(&mut cache).await?;
        let cache_keys = scan_keys(&mut cache, b"*").await?;
        if !cache_keys.is_empty() {
            return Err(SourceError::RedisDrainIncomplete);
        }
        let cache_sha256 = domain_hash_fields(
            b"v2board-post-pause-redis-cache-v1\0",
            [cache_run_id.as_bytes(), b"empty_keyspace"],
        );
        (cache_run_id, cache_sha256)
    };
    let upload_fields = receipt.upload_fields.to_string();
    let download_fields = receipt.download_fields.to_string();
    let default_key_count = default_keys.len().to_string();
    let default_sha256 = domain_hash_fields(
        b"v2board-post-pause-redis-default-v1\0",
        [
            spec.operation_id.as_bytes(),
            receipt.receipt_sha256.as_bytes(),
            default_run_id.as_bytes(),
            receipt.sorted_user_delta_sha256.as_bytes(),
            receipt.upload_delta_sum.as_bytes(),
            receipt.download_delta_sum.as_bytes(),
            upload_fields.as_bytes(),
            download_fields.as_bytes(),
            default_key_count.as_bytes(),
        ],
    );
    let composite_sha256 = domain_hash_fields(
        b"v2board-post-pause-redis-snapshot-v1\0",
        [
            spec.operation_id.as_bytes(),
            default_sha256.as_bytes(),
            cache_sha256.as_bytes(),
            default_run_id.as_bytes(),
            cache_run_id.as_bytes(),
        ],
    );
    Ok(PostPauseRedisSnapshot {
        source_default_run_id: default_run_id,
        source_cache_run_id: cache_run_id,
        default_sha256,
        cache_sha256,
        composite_sha256,
    })
}

async fn post_pause_default_snapshot(
    spec: &ProvisionSpec,
    source: &SourceSpec,
) -> Result<(String, String), SourceError> {
    let receipt = verify_frozen_traffic_receipt(spec)?;
    let mut expected_upload = BTreeMap::new();
    let mut expected_download = BTreeMap::new();
    for FrozenTrafficDeltaRecord(user_id, upload, download) in &receipt.deltas {
        if let Some(upload) = upload {
            expected_upload.insert(*user_id, i128::from(*upload));
        }
        if let Some(download) = download {
            expected_download.insert(*user_id, i128::from(*download));
        }
    }
    let mut connection = redis_connection(&source.redis_default_url).await?;
    let run_id = redis_run_id(&mut connection).await?;
    let upload = read_traffic_hash(&mut connection, receipt.frozen_upload_key.as_bytes()).await?;
    let download =
        read_traffic_hash(&mut connection, receipt.frozen_download_key.as_bytes()).await?;
    let keys = scan_keys(&mut connection, b"*")
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut expected_keys = BTreeSet::new();
    if !expected_upload.is_empty() {
        expected_keys.insert(receipt.frozen_upload_key.as_bytes().to_vec());
    }
    if !expected_download.is_empty() {
        expected_keys.insert(receipt.frozen_download_key.as_bytes().to_vec());
    }
    if run_id != receipt.source_default_run_id
        || upload != expected_upload
        || download != expected_download
        || keys != expected_keys
    {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let upload_fields = receipt.upload_fields.to_string();
    let download_fields = receipt.download_fields.to_string();
    let key_count = keys.len().to_string();
    let sha256 = domain_hash_fields(
        b"v2board-post-pause-redis-default-v1\0",
        [
            spec.operation_id.as_bytes(),
            receipt.receipt_sha256.as_bytes(),
            run_id.as_bytes(),
            receipt.sorted_user_delta_sha256.as_bytes(),
            receipt.upload_delta_sum.as_bytes(),
            receipt.download_delta_sum.as_bytes(),
            upload_fields.as_bytes(),
            download_fields.as_bytes(),
            key_count.as_bytes(),
        ],
    );
    Ok((run_id, sha256))
}

async fn post_pause_cache_snapshot(source: &SourceSpec) -> Result<(String, String), SourceError> {
    let mut connection = redis_connection(&source.redis_cache_url).await?;
    let run_id = redis_run_id(&mut connection).await?;
    if !scan_keys(&mut connection, b"*").await?.is_empty() {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let sha256 = domain_hash_fields(
        b"v2board-post-pause-redis-cache-v1\0",
        [run_id.as_bytes(), b"empty_keyspace"],
    );
    Ok((run_id, sha256))
}

async fn verify_reachable_post_pause_snapshot(
    spec: &ProvisionSpec,
    source: &SourceSpec,
    armed: &RedisFenceArmedReceiptPayload,
    default_reachable: bool,
    cache_reachable: bool,
) -> Result<(), SourceError> {
    if default_reachable && cache_reachable {
        let observed = post_pause_redis_snapshot(spec, source, true, true).await?;
        if observed.source_default_run_id != armed.source_default_run_id
            || observed.source_cache_run_id != armed.source_cache_run_id
            || observed.default_sha256 != armed.default_post_pause_snapshot_sha256
            || observed.cache_sha256 != armed.cache_post_pause_snapshot_sha256
            || observed.composite_sha256 != armed.post_pause_snapshot_sha256
        {
            return Err(SourceError::SourceDrift);
        }
    } else if default_reachable {
        let (run_id, sha256) = post_pause_default_snapshot(spec, source).await?;
        if run_id != armed.source_default_run_id
            || sha256 != armed.default_post_pause_snapshot_sha256
        {
            return Err(SourceError::SourceDrift);
        }
    } else if cache_reachable {
        let (run_id, sha256) = post_pause_cache_snapshot(source).await?;
        if run_id != armed.source_cache_run_id || sha256 != armed.cache_post_pause_snapshot_sha256 {
            return Err(SourceError::SourceDrift);
        }
    }
    Ok(())
}

fn verify_offline_redis_units<R: LegacyCommandRunner + ?Sized>(
    spec: &ProvisionSpec,
    runner: &mut R,
    default_reachable: bool,
    cache_reachable: bool,
) -> Result<(), SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::InvalidPolicy)?;
    let mut offline = BTreeSet::new();
    if !default_reachable {
        offline.insert(&execution.source_control.datastores.default_redis.unit);
    }
    if !cache_reachable {
        offline.insert(&execution.source_control.datastores.cache_redis.unit);
    }
    verify_units_masked_and_stopped(runner, offline.into_iter())
}

fn stop_masked_redis_units<R: LegacyCommandRunner + ?Sized>(
    runner: &mut R,
    units: &[String],
) -> Result<(), SourceError> {
    ensure_effective_root(runner)?;
    for unit in units {
        run_success(
            runner,
            LegacyCommandRequest::new(LegacyProgram::Systemctl)
                .arg("stop")
                .arg(unit),
        )?;
    }
    verify_units_masked_and_stopped(runner, units.iter())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct FrozenTrafficReport {
    operation_id: String,
    source_run_id: String,
    upload_key: String,
    download_key: String,
    upload_fields: u64,
    download_fields: u64,
    delta_users: u64,
    delta_sha256: String,
    upload_sum: i128,
    download_sum: i128,
    deltas: Vec<FrozenTrafficDeltaRecord>,
}

/// Compact, sorted, durable source fact: `(user_id, upload, download)`. A
/// `None` direction means the legacy hash had no field for that user; this is
/// distinct from an explicit zero and keeps HLEN/count verification exact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrozenTrafficDeltaRecord(pub i64, pub Option<i64>, pub Option<i64>);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceFenceReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    stopped_ingress_units: Vec<String>,
    restart_disabled_drain_worker_units: Vec<String>,
    unit_inventory_sha256: String,
    ingress_verified_disabled_and_inactive: bool,
    drain_workers_verified_restart_disabled: bool,
    fenced_at_unix: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceFenceReceiptEnvelope {
    payload: SourceFenceReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RedisFenceArmedReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    redis_units: Vec<String>,
    unit_inventory_sha256: String,
    drain_report_sha256: String,
    traffic_receipt_sha256: String,
    source_default_run_id: String,
    source_cache_run_id: String,
    default_post_pause_snapshot_sha256: String,
    cache_post_pause_snapshot_sha256: String,
    post_pause_snapshot_sha256: String,
    client_pause_write_milliseconds: u64,
    all_redis_processes_write_paused: bool,
    armed_at_unix: i64,
    pause_expires_at_unix: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RedisFenceArmedReceiptEnvelope {
    payload: RedisFenceArmedReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RedisFenceReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    armed_receipt_sha256: String,
    post_pause_snapshot_sha256: String,
    armed_client_pause_write_at_unix: i64,
    client_pause_write_milliseconds: u64,
    redis_units: Vec<String>,
    unit_inventory_sha256: String,
    all_redis_units_masked_and_inactive: bool,
    default_redis_unreachable_with_old_credentials: bool,
    cache_redis_unreachable_with_old_credentials: bool,
    default_redis_probe_sha256: String,
    cache_redis_probe_sha256: String,
    fenced_at_unix: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RedisFenceReceiptEnvelope {
    payload: RedisFenceReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DatastoreFenceReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    armed_receipt_sha256: String,
    stopped_datastore_units: Vec<String>,
    unit_inventory_sha256: String,
    all_datastore_units_masked_and_inactive: bool,
    mysql_unreachable_with_old_credentials: bool,
    default_redis_unreachable_with_old_credentials: bool,
    cache_redis_unreachable_with_old_credentials: bool,
    mysql_probe_sha256: String,
    default_redis_probe_sha256: String,
    cache_redis_probe_sha256: String,
    backup_receipt_sha256: String,
    encrypted_backup_sha256: String,
    archive_source_fingerprint_sha256: String,
    archive_source_schema_sha256: String,
    fenced_at_unix: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DatastoreFenceReceiptEnvelope {
    payload: DatastoreFenceReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DatastoreFenceArmedReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    mysql_unit: String,
    unit_inventory_sha256: String,
    super_read_only_persisted_and_active: bool,
    active_innodb_transactions: u64,
    active_replication_channels: u64,
    active_group_replication_members: u64,
    archive_exact_under_durable_write_fence: bool,
    backup_receipt_sha256: String,
    encrypted_backup_sha256: String,
    archive_source_fingerprint_sha256: String,
    archive_source_schema_sha256: String,
    armed_at_unix: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DatastoreFenceArmedReceiptEnvelope {
    payload: DatastoreFenceArmedReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceRetirementReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    native_authority_nodes_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    unit_inventory_sha256: String,
    all_declared_units_masked_and_inactive: bool,
    mysql_unreachable_with_old_credentials: bool,
    default_redis_unreachable_with_old_credentials: bool,
    cache_redis_unreachable_with_old_credentials: bool,
    mysql_probe_sha256: String,
    default_redis_probe_sha256: String,
    cache_redis_probe_sha256: String,
    retired_at_unix: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceRetirementReceiptEnvelope {
    payload: SourceRetirementReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrafficDrainReceiptPayload {
    schema_version: u32,
    operation_id: String,
    journal_anchor_generation: u64,
    journal_anchor_checkpoint: ApplyCheckpoint,
    journal_anchor_event_sha256: String,
    result_checkpoint: ApplyCheckpoint,
    source_default_run_id: String,
    frozen_upload_key: String,
    frozen_download_key: String,
    fenced_at_unix: i64,
    upload_fields: u64,
    download_fields: u64,
    sorted_user_delta_count: u64,
    sorted_user_delta_sha256: String,
    upload_delta_sum: String,
    download_delta_sum: String,
    deltas: Vec<FrozenTrafficDeltaRecord>,
    delta_applied_exactly_once: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TrafficDrainReceiptEnvelope {
    payload: TrafficDrainReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFrozenTrafficReceipt {
    pub operation_id: String,
    pub maintenance_fenced_generation: u64,
    pub maintenance_fenced_event_sha256: String,
    pub source_default_run_id: String,
    pub frozen_upload_key: String,
    pub frozen_download_key: String,
    pub fenced_at_unix: i64,
    pub upload_fields: u64,
    pub download_fields: u64,
    pub sorted_user_delta_count: u64,
    pub sorted_user_delta_sha256: String,
    pub upload_delta_sum: String,
    pub download_delta_sum: String,
    pub deltas: Vec<FrozenTrafficDeltaRecord>,
    pub delta_applied_exactly_once: bool,
    pub receipt_sha256: String,
}

async fn freeze_and_digest_traffic(
    connection: &mut MultiplexedConnection,
    spec: &ProvisionSpec,
    source: &SourceSpec,
) -> Result<FrozenTrafficReport, SourceError> {
    if Uuid::parse_str(&spec.operation_id)
        .ok()
        .is_none_or(|value| value.is_nil())
    {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let prefix = &source.redis_connection_prefix;
    let upload_source = format!("{prefix}v2board_upload_traffic");
    let download_source = format!("{prefix}v2board_download_traffic");
    let reset_lock = format!("{prefix}traffic_reset_lock");
    let frozen_upload = format!(
        "{prefix}v2board_migration:{}:frozen_upload_traffic",
        spec.operation_id
    );
    let frozen_download = format!(
        "{prefix}v2board_migration:{}:frozen_download_traffic",
        spec.operation_id
    );
    // One Lua execution is the only transition from live hashes to the
    // operation-owned frozen names. It is retry-idempotent and fails if a live
    // source and its frozen destination coexist or the reset lock is present.
    const FREEZE_SCRIPT: &str = r#"
        if redis.call('EXISTS', KEYS[5]) ~= 0 then return {-1, -1} end
        if redis.call('EXISTS', KEYS[1]) ~= 0 and redis.call('EXISTS', KEYS[3]) ~= 0 then
            return {-2, -2}
        end
        if redis.call('EXISTS', KEYS[2]) ~= 0 and redis.call('EXISTS', KEYS[4]) ~= 0 then
            return {-3, -3}
        end
        if redis.call('EXISTS', KEYS[1]) ~= 0 then
            if redis.call('RENAMENX', KEYS[1], KEYS[3]) ~= 1 then return {-4, -4} end
        end
        if redis.call('EXISTS', KEYS[2]) ~= 0 then
            if redis.call('RENAMENX', KEYS[2], KEYS[4]) ~= 1 then return {-5, -5} end
        end
        return {redis.call('EXISTS', KEYS[3]), redis.call('EXISTS', KEYS[4])}
    "#;
    let result = redis::cmd("EVAL")
        .arg(FREEZE_SCRIPT)
        .arg(5)
        .arg(&upload_source)
        .arg(&download_source)
        .arg(&frozen_upload)
        .arg(&frozen_download)
        .arg(&reset_lock)
        .query_async::<Vec<i64>>(connection)
        .await
        .map_err(|_| SourceError::Redis)?;
    if result.len() != 2 || result.iter().any(|value| *value < 0) {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let upload = read_traffic_hash(connection, frozen_upload.as_bytes()).await?;
    let download = read_traffic_hash(connection, frozen_download.as_bytes()).await?;
    let (deltas, delta_sha256, upload_sum, download_sum) =
        canonical_traffic_delta_summary(&upload, &download)?;
    let delta_users = deltas.len() as u64;
    let source_run_id = redis_run_id(connection).await?;
    Ok(FrozenTrafficReport {
        operation_id: spec.operation_id.clone(),
        source_run_id,
        upload_key: frozen_upload,
        download_key: frozen_download,
        upload_fields: upload.len() as u64,
        download_fields: download.len() as u64,
        delta_users,
        delta_sha256,
        upload_sum,
        download_sum,
        deltas,
    })
}

fn canonical_traffic_delta_summary(
    upload: &BTreeMap<i64, i128>,
    download: &BTreeMap<i64, i128>,
) -> Result<(Vec<FrozenTrafficDeltaRecord>, String, i128, i128), SourceError> {
    let mut users = BTreeSet::new();
    users.extend(upload.keys().copied());
    users.extend(download.keys().copied());
    let mut digest = Sha256::new();
    digest.update(b"v2board-frozen-traffic-user-deltas-v1\0");
    let mut upload_sum = 0_i128;
    let mut download_sum = 0_i128;
    if users.len() > MAX_FROZEN_TRAFFIC_USERS {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let mut deltas = Vec::with_capacity(users.len());
    for user_id in &users {
        let upload_value = upload.get(user_id).copied();
        let download_value = download.get(user_id).copied();
        let up = upload_value.unwrap_or(0);
        let down = download_value.unwrap_or(0);
        upload_sum = upload_sum
            .checked_add(up)
            .ok_or(SourceError::RedisDrainIncomplete)?;
        download_sum = download_sum
            .checked_add(down)
            .ok_or(SourceError::RedisDrainIncomplete)?;
        digest_field(&mut digest, user_id.to_string().as_bytes());
        digest_field(&mut digest, up.to_string().as_bytes());
        digest_field(&mut digest, down.to_string().as_bytes());
        deltas.push(FrozenTrafficDeltaRecord(
            *user_id,
            upload_value
                .map(|value| i64::try_from(value).map_err(|_| SourceError::RedisDrainIncomplete))
                .transpose()?,
            download_value
                .map(|value| i64::try_from(value).map_err(|_| SourceError::RedisDrainIncomplete))
                .transpose()?,
        ));
    }
    Ok((
        deltas,
        hex::encode(digest.finalize()),
        upload_sum,
        download_sum,
    ))
}

async fn read_traffic_hash(
    connection: &mut MultiplexedConnection,
    key: &[u8],
) -> Result<BTreeMap<i64, i128>, SourceError> {
    let kind = redis::cmd("TYPE")
        .arg(key)
        .query_async::<String>(connection)
        .await
        .map_err(|_| SourceError::Redis)?;
    if kind == "none" {
        return Ok(BTreeMap::new());
    }
    if kind != "hash" {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let mut result = BTreeMap::new();
    let mut cursor = 0_u64;
    loop {
        let (next, values) = redis::cmd("HSCAN")
            .arg(key)
            .arg(cursor)
            .arg("COUNT")
            .arg(REDIS_SCAN_COUNT)
            .query_async::<(u64, Vec<(Vec<u8>, Vec<u8>)>)>(connection)
            .await
            .map_err(|_| SourceError::Redis)?;
        for (field, value) in values {
            let user_id = std::str::from_utf8(&field)
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value > 0)
                .ok_or(SourceError::RedisDrainIncomplete)?;
            let traffic = std::str::from_utf8(&value)
                .ok()
                .and_then(|value| value.parse::<i128>().ok())
                .filter(|value| *value >= 0)
                .ok_or(SourceError::RedisDrainIncomplete)?;
            if result.insert(user_id, traffic).is_some() {
                return Err(SourceError::RedisDrainIncomplete);
            }
        }
        if next == 0 {
            break;
        }
        cursor = next;
    }
    Ok(result)
}

async fn redis_run_id(connection: &mut MultiplexedConnection) -> Result<String, SourceError> {
    let info = redis::cmd("INFO")
        .arg("server")
        .query_async::<String>(connection)
        .await
        .map_err(|_| SourceError::Redis)?;
    let run_id = info
        .lines()
        .find_map(|line| line.strip_prefix("run_id:"))
        .map(str::trim)
        .unwrap_or("");
    if run_id.len() != 40 || !run_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SourceError::RedisDrainIncomplete);
    }
    Ok(run_id.to_ascii_lowercase())
}

fn persist_source_fence_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
) -> Result<String, SourceError> {
    if head.binding().operation_id() != spec.operation_id
        || head.checkpoint() != ApplyCheckpoint::PendingDurable
        || head.generation() == 0
        || !is_lower_sha256(head.event_sha256())
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let path = &execution.receipts.source_fence_path;
    let existing_file = existing_receipt_file(path)?;
    let existing_payload = if let Some(existing_file) = existing_file {
        let bytes = read_owner_only_file(&existing_file, MAX_RECEIPT_BYTES)?;
        let envelope: SourceFenceReceiptEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
        let canonical =
            serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
        if !spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceFence,
            &canonical,
            &envelope.hmac_sha256,
        ) {
            return Err(SourceError::ReceiptInvalid);
        }
        Some(envelope.payload)
    } else {
        None
    };
    let payload = SourceFenceReceiptPayload {
        schema_version: 1,
        operation_id: spec.operation_id.clone(),
        journal_anchor_generation: existing_payload
            .as_ref()
            .map_or(head.generation(), |payload| {
                payload.journal_anchor_generation
            }),
        journal_anchor_checkpoint: ApplyCheckpoint::PendingDurable,
        journal_anchor_event_sha256: existing_payload.as_ref().map_or_else(
            || head.event_sha256().to_string(),
            |payload| payload.journal_anchor_event_sha256.clone(),
        ),
        result_checkpoint: ApplyCheckpoint::MaintenanceFenced,
        stopped_ingress_units: units.ingress_writers().cloned().collect(),
        restart_disabled_drain_worker_units: units.workers.clone(),
        unit_inventory_sha256: units.digest(),
        ingress_verified_disabled_and_inactive: true,
        drain_workers_verified_restart_disabled: true,
        fenced_at_unix: existing_payload
            .as_ref()
            .map_or_else(unix_now, |payload| Ok(payload.fenced_at_unix))?,
    };
    if payload.journal_anchor_generation == 0
        || !is_lower_sha256(&payload.journal_anchor_event_sha256)
    {
        return Err(SourceError::ReceiptInvalid);
    }
    persist_hmac_receipt(spec, LegacyRuntimeReceiptKind::SourceFence, path, &payload)
}

struct LoadedRedisFenceArmedReceipt {
    payload: RedisFenceArmedReceiptPayload,
    receipt_sha256: String,
}

fn load_redis_fence_armed_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
) -> Result<Option<LoadedRedisFenceArmedReceipt>, SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let Some(path) = existing_receipt_file(&execution.receipts.redis_fence_armed_path)? else {
        return Ok(None);
    };
    let bytes = read_owner_only_file(&path, MAX_RECEIPT_BYTES)?;
    let envelope: RedisFenceArmedReceiptEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let payload = envelope.payload;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::RedisFenceArmed,
        &canonical,
        &envelope.hmac_sha256,
    ) || payload.schema_version != 1
        || payload.operation_id != spec.operation_id
        || payload.journal_anchor_checkpoint != ApplyCheckpoint::MaintenanceFenced
        || payload.result_checkpoint != ApplyCheckpoint::SourceDrained
        || payload.journal_anchor_generation > head.generation()
        || !is_lower_sha256(&payload.journal_anchor_event_sha256)
        || payload.redis_units != redis_units(spec)?
        || payload.unit_inventory_sha256 != units.digest()
        || !is_lower_sha256(&payload.drain_report_sha256)
        || !is_lower_sha256(&payload.traffic_receipt_sha256)
        || payload.source_default_run_id.len() != 40
        || payload.source_cache_run_id.len() != 40
        || [
            payload.source_default_run_id.as_str(),
            payload.source_cache_run_id.as_str(),
        ]
        .into_iter()
        .any(|run_id| {
            !run_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        || !is_lower_sha256(&payload.default_post_pause_snapshot_sha256)
        || !is_lower_sha256(&payload.cache_post_pause_snapshot_sha256)
        || !is_lower_sha256(&payload.post_pause_snapshot_sha256)
        || payload.client_pause_write_milliseconds != REDIS_WRITE_PAUSE_MILLISECONDS
        || !payload.all_redis_processes_write_paused
        || payload.armed_at_unix <= 0
        || payload.pause_expires_at_unix
            != payload
                .armed_at_unix
                .checked_add(
                    i64::try_from(REDIS_WRITE_PAUSE_MILLISECONDS / 1000)
                        .map_err(|_| SourceError::ReceiptInvalid)?,
                )
                .ok_or(SourceError::ReceiptInvalid)?
    {
        return Err(SourceError::ReceiptInvalid);
    }
    Ok(Some(LoadedRedisFenceArmedReceipt {
        payload,
        receipt_sha256: hex::encode(Sha256::digest(&bytes)),
    }))
}

fn persist_redis_fence_armed_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
    drain_report_sha256: &str,
    traffic_receipt_sha256: &str,
    post_pause_snapshot: &PostPauseRedisSnapshot,
    pause_ack_at_unix: i64,
) -> Result<String, SourceError> {
    if head.binding().operation_id() != spec.operation_id
        || head.checkpoint() != ApplyCheckpoint::MaintenanceFenced
        || !is_lower_sha256(head.event_sha256())
        || !is_lower_sha256(drain_report_sha256)
        || !is_lower_sha256(traffic_receipt_sha256)
        || !is_lower_sha256(&post_pause_snapshot.default_sha256)
        || !is_lower_sha256(&post_pause_snapshot.cache_sha256)
        || !is_lower_sha256(&post_pause_snapshot.composite_sha256)
        || pause_ack_at_unix <= 0
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let redis_units = redis_units(spec)?;
    let existing = load_redis_fence_armed_receipt(spec, head, units)?;
    let pause_seconds = i64::try_from(REDIS_WRITE_PAUSE_MILLISECONDS / 1000)
        .map_err(|_| SourceError::ReceiptInvalid)?;
    let payload = RedisFenceArmedReceiptPayload {
        schema_version: 1,
        operation_id: spec.operation_id.clone(),
        journal_anchor_generation: head.generation(),
        journal_anchor_checkpoint: ApplyCheckpoint::MaintenanceFenced,
        journal_anchor_event_sha256: head.event_sha256().to_string(),
        result_checkpoint: ApplyCheckpoint::SourceDrained,
        redis_units,
        unit_inventory_sha256: units.digest(),
        drain_report_sha256: drain_report_sha256.to_string(),
        traffic_receipt_sha256: traffic_receipt_sha256.to_string(),
        source_default_run_id: post_pause_snapshot.source_default_run_id.clone(),
        source_cache_run_id: post_pause_snapshot.source_cache_run_id.clone(),
        default_post_pause_snapshot_sha256: post_pause_snapshot.default_sha256.clone(),
        cache_post_pause_snapshot_sha256: post_pause_snapshot.cache_sha256.clone(),
        post_pause_snapshot_sha256: post_pause_snapshot.composite_sha256.clone(),
        client_pause_write_milliseconds: REDIS_WRITE_PAUSE_MILLISECONDS,
        all_redis_processes_write_paused: true,
        armed_at_unix: pause_ack_at_unix,
        pause_expires_at_unix: pause_ack_at_unix
            .checked_add(pause_seconds)
            .ok_or(SourceError::ReceiptInvalid)?,
    };
    if let Some(existing) = existing
        && existing.payload != payload
    {
        return Err(SourceError::SourceDrift);
    }
    persist_hmac_receipt(
        spec,
        LegacyRuntimeReceiptKind::RedisFenceArmed,
        &execution.receipts.redis_fence_armed_path,
        &payload,
    )
}

async fn finalize_redis_fence<R: LegacyCommandRunner + ?Sized>(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
    redis_units: &[String],
    armed_receipt_sha256: &str,
    runner: &mut R,
) -> Result<VerifiedStageProof, SourceError> {
    verify_units_masked_and_stopped(runner, redis_units.iter())?;
    let source = legacy_source(spec)?;
    let probes = run_retirement_probes_async((
        source.database_url.clone(),
        source.redis_default_url.clone(),
        source.redis_cache_url.clone(),
    ))
    .await;
    if probes.default_redis_reachable || probes.cache_redis_reachable {
        return Err(SourceError::ProbeRuntime);
    }
    let receipt_sha256 = persist_redis_fence_receipt(
        spec,
        head,
        units,
        redis_units,
        armed_receipt_sha256,
        &probes,
    )?;
    VerifiedStageProof::new(receipt_sha256).map_err(|_| SourceError::ReceiptInvalid)
}

fn persist_redis_fence_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
    redis_units: &[String],
    armed_receipt_sha256: &str,
    probes: &RetirementProbes,
) -> Result<String, SourceError> {
    if head.binding().operation_id() != spec.operation_id
        || head.checkpoint() != ApplyCheckpoint::MaintenanceFenced
        || !is_lower_sha256(armed_receipt_sha256)
        || probes.default_redis_reachable
        || probes.cache_redis_reachable
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let armed =
        load_redis_fence_armed_receipt(spec, head, units)?.ok_or(SourceError::ReceiptInvalid)?;
    if armed.receipt_sha256 != armed_receipt_sha256 {
        return Err(SourceError::ReceiptInvalid);
    }
    let path = &execution.receipts.redis_fence_path;
    let existing_file = existing_receipt_file(path)?;
    let existing_payload = if let Some(existing_file) = existing_file {
        let bytes = read_owner_only_file(&existing_file, MAX_RECEIPT_BYTES)?;
        let envelope: RedisFenceReceiptEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
        let canonical =
            serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
        if !spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::RedisFence,
            &canonical,
            &envelope.hmac_sha256,
        ) {
            return Err(SourceError::ReceiptInvalid);
        }
        Some(envelope.payload)
    } else {
        None
    };
    let payload = RedisFenceReceiptPayload {
        schema_version: 1,
        operation_id: spec.operation_id.clone(),
        journal_anchor_generation: head.generation(),
        journal_anchor_checkpoint: ApplyCheckpoint::MaintenanceFenced,
        journal_anchor_event_sha256: head.event_sha256().to_string(),
        result_checkpoint: ApplyCheckpoint::SourceDrained,
        armed_receipt_sha256: armed_receipt_sha256.to_string(),
        post_pause_snapshot_sha256: armed.payload.post_pause_snapshot_sha256.clone(),
        armed_client_pause_write_at_unix: armed.payload.armed_at_unix,
        client_pause_write_milliseconds: armed.payload.client_pause_write_milliseconds,
        redis_units: redis_units.to_vec(),
        unit_inventory_sha256: units.digest(),
        all_redis_units_masked_and_inactive: true,
        default_redis_unreachable_with_old_credentials: true,
        cache_redis_unreachable_with_old_credentials: true,
        default_redis_probe_sha256: probes.default_redis_evidence_sha256.clone(),
        cache_redis_probe_sha256: probes.cache_redis_evidence_sha256.clone(),
        fenced_at_unix: existing_payload
            .as_ref()
            .map_or_else(unix_now, |payload| Ok(payload.fenced_at_unix))?,
    };
    if payload.fenced_at_unix <= 0
        || [
            payload.journal_anchor_event_sha256.as_str(),
            payload.armed_receipt_sha256.as_str(),
            payload.post_pause_snapshot_sha256.as_str(),
            payload.unit_inventory_sha256.as_str(),
            payload.default_redis_probe_sha256.as_str(),
            payload.cache_redis_probe_sha256.as_str(),
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
    {
        return Err(SourceError::ReceiptInvalid);
    }
    persist_hmac_receipt(spec, LegacyRuntimeReceiptKind::RedisFence, path, &payload)
}

async fn verify_existing_redis_fence<R: LegacyCommandRunner + ?Sized>(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
    runner: &mut R,
) -> Result<Option<String>, SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let Some(path) = existing_receipt_file(&execution.receipts.redis_fence_path)? else {
        return Ok(None);
    };
    let bytes = read_owner_only_file(&path, MAX_RECEIPT_BYTES)?;
    let envelope: RedisFenceReceiptEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let payload = envelope.payload;
    let receipt_sha256 = hex::encode(Sha256::digest(&bytes));
    let anchor_matches = if head.checkpoint() == ApplyCheckpoint::MaintenanceFenced {
        payload.journal_anchor_generation == head.generation()
            && payload.journal_anchor_event_sha256 == head.event_sha256()
    } else if head.checkpoint() == ApplyCheckpoint::SourceDrained {
        payload.journal_anchor_generation.checked_add(1) == Some(head.generation())
            && head.previous_event_sha256() == Some(payload.journal_anchor_event_sha256.as_str())
            && head.checkpoint_proof_sha256() == Some(receipt_sha256.as_str())
    } else {
        false
    };
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::RedisFence,
        &canonical,
        &envelope.hmac_sha256,
    ) || payload.schema_version != 1
        || payload.operation_id != spec.operation_id
        || payload.journal_anchor_checkpoint != ApplyCheckpoint::MaintenanceFenced
        || payload.result_checkpoint != ApplyCheckpoint::SourceDrained
        || !anchor_matches
        || !is_lower_sha256(&payload.armed_receipt_sha256)
        || !is_lower_sha256(&payload.post_pause_snapshot_sha256)
        || payload.armed_client_pause_write_at_unix <= 0
        || payload.client_pause_write_milliseconds != REDIS_WRITE_PAUSE_MILLISECONDS
        || payload.redis_units != redis_units(spec)?
        || payload.unit_inventory_sha256 != units.digest()
        || !payload.all_redis_units_masked_and_inactive
        || !payload.default_redis_unreachable_with_old_credentials
        || !payload.cache_redis_unreachable_with_old_credentials
        || payload.fenced_at_unix <= 0
    {
        return Err(SourceError::ReceiptInvalid);
    }
    verify_units_masked_and_stopped(runner, payload.redis_units.iter())?;
    let source = legacy_source(spec)?;
    let probes = run_retirement_probes_async((
        source.database_url.clone(),
        source.redis_default_url.clone(),
        source.redis_cache_url.clone(),
    ))
    .await;
    if probes.default_redis_reachable
        || probes.cache_redis_reachable
        || probes.default_redis_evidence_sha256 != payload.default_redis_probe_sha256
        || probes.cache_redis_evidence_sha256 != payload.cache_redis_probe_sha256
    {
        return Err(SourceError::ProbeRuntime);
    }
    Ok(Some(receipt_sha256))
}

pub(crate) async fn verify_redis_fence_for_backup(
    spec: &ProvisionSpec,
    source_drained: &ApplyJournalSnapshot,
) -> Result<(), SourceError> {
    let policy = LegacySourcePolicy::from_manifest(spec)?;
    policy.validate()?;
    let mut runner = ProcessLegacyCommandRunner;
    ensure_effective_root(&mut runner)?;
    verify_existing_redis_fence(spec, source_drained, &policy.units, &mut runner)
        .await?
        .ok_or(SourceError::ReceiptInvalid)
        .map(|_| ())
}

struct LoadedDatastoreFenceArmedReceipt {
    payload: DatastoreFenceArmedReceiptPayload,
    receipt_sha256: String,
}

fn load_datastore_fence_armed_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
) -> Result<Option<LoadedDatastoreFenceArmedReceipt>, SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let Some(path) = existing_receipt_file(&execution.receipts.datastore_fence_armed_path)? else {
        return Ok(None);
    };
    let bytes = read_owner_only_file(&path, MAX_RECEIPT_BYTES)?;
    let envelope: DatastoreFenceArmedReceiptEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let payload = envelope.payload;
    let mysql_unit = &execution.source_control.datastores.mysql.unit;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::DatastoreFenceArmed,
        &canonical,
        &envelope.hmac_sha256,
    ) || payload.schema_version != 1
        || payload.operation_id != spec.operation_id
        || payload.journal_anchor_checkpoint != ApplyCheckpoint::BackupRestoreVerified
        || payload.result_checkpoint != ApplyCheckpoint::FinalRecheckPassed
        || payload.journal_anchor_generation == 0
        || !is_lower_sha256(&payload.journal_anchor_event_sha256)
        || payload.journal_anchor_generation > head.generation()
        || payload.mysql_unit != *mysql_unit
        || payload.unit_inventory_sha256 != units.digest()
        || !payload.super_read_only_persisted_and_active
        || payload.active_innodb_transactions != 0
        || payload.active_replication_channels != 0
        || payload.active_group_replication_members != 0
        || !payload.archive_exact_under_durable_write_fence
        || payload.armed_at_unix <= 0
        || [
            payload.backup_receipt_sha256.as_str(),
            payload.encrypted_backup_sha256.as_str(),
            payload.archive_source_fingerprint_sha256.as_str(),
            payload.archive_source_schema_sha256.as_str(),
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
    {
        return Err(SourceError::ReceiptInvalid);
    }
    Ok(Some(LoadedDatastoreFenceArmedReceipt {
        payload,
        receipt_sha256: hex::encode(Sha256::digest(&bytes)),
    }))
}

fn persist_datastore_fence_armed_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
    mysql_unit: &str,
    archive: &VerifiedBackupArchive,
    fingerprint: &str,
    source_schema_sha256: &str,
) -> Result<String, SourceError> {
    if head.binding().operation_id() != spec.operation_id
        || !is_lower_sha256(head.event_sha256())
        || !is_lower_sha256(fingerprint)
        || !is_lower_sha256(source_schema_sha256)
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let existing = load_datastore_fence_armed_receipt(spec, head, units)?;
    if existing.is_none() && head.checkpoint() != ApplyCheckpoint::BackupRestoreVerified {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let payload = DatastoreFenceArmedReceiptPayload {
        schema_version: 1,
        operation_id: spec.operation_id.clone(),
        journal_anchor_generation: existing.as_ref().map_or(head.generation(), |receipt| {
            receipt.payload.journal_anchor_generation
        }),
        journal_anchor_checkpoint: ApplyCheckpoint::BackupRestoreVerified,
        journal_anchor_event_sha256: existing.as_ref().map_or_else(
            || head.event_sha256().to_string(),
            |receipt| receipt.payload.journal_anchor_event_sha256.clone(),
        ),
        result_checkpoint: ApplyCheckpoint::FinalRecheckPassed,
        mysql_unit: mysql_unit.to_string(),
        unit_inventory_sha256: units.digest(),
        super_read_only_persisted_and_active: true,
        active_innodb_transactions: 0,
        active_replication_channels: 0,
        active_group_replication_members: 0,
        archive_exact_under_durable_write_fence: true,
        backup_receipt_sha256: archive.receipt_sha256().to_string(),
        encrypted_backup_sha256: archive.encrypted_backup_sha256().to_string(),
        archive_source_fingerprint_sha256: fingerprint.to_string(),
        archive_source_schema_sha256: source_schema_sha256.to_string(),
        armed_at_unix: existing
            .as_ref()
            .map_or_else(unix_now, |receipt| Ok(receipt.payload.armed_at_unix))?,
    };
    persist_hmac_receipt(
        spec,
        LegacyRuntimeReceiptKind::DatastoreFenceArmed,
        &execution.receipts.datastore_fence_armed_path,
        &payload,
    )
}

fn persist_datastore_fence_receipt(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
    units: &LegacyWriterUnits,
    archive: &VerifiedBackupArchive,
    armed_receipt_sha256: &str,
    probes: &RetirementProbes,
) -> Result<String, SourceError> {
    if head.binding().operation_id() != spec.operation_id
        || !is_lower_sha256(head.event_sha256())
        || !is_lower_sha256(armed_receipt_sha256)
        || probes.mysql_reachable
        || probes.default_redis_reachable
        || probes.cache_redis_reachable
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let path = &execution.receipts.datastore_fence_path;
    let existing_file = existing_receipt_file(path)?;
    let existing_payload = if let Some(existing_file) = existing_file {
        let bytes = read_owner_only_file(&existing_file, MAX_RECEIPT_BYTES)?;
        let envelope: DatastoreFenceReceiptEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
        let canonical =
            serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
        if !spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::DatastoreFence,
            &canonical,
            &envelope.hmac_sha256,
        ) {
            return Err(SourceError::ReceiptInvalid);
        }
        Some(envelope.payload)
    } else {
        None
    };
    if existing_payload.is_none() && head.checkpoint() != ApplyCheckpoint::BackupRestoreVerified {
        return Err(SourceError::ReceiptInvalid);
    }
    let payload = DatastoreFenceReceiptPayload {
        schema_version: 1,
        operation_id: spec.operation_id.clone(),
        journal_anchor_generation: existing_payload
            .as_ref()
            .map_or(head.generation(), |payload| {
                payload.journal_anchor_generation
            }),
        journal_anchor_checkpoint: ApplyCheckpoint::BackupRestoreVerified,
        journal_anchor_event_sha256: existing_payload.as_ref().map_or_else(
            || head.event_sha256().to_string(),
            |payload| payload.journal_anchor_event_sha256.clone(),
        ),
        result_checkpoint: ApplyCheckpoint::FinalRecheckPassed,
        armed_receipt_sha256: armed_receipt_sha256.to_string(),
        stopped_datastore_units: units.local_datastores.clone(),
        unit_inventory_sha256: units.digest(),
        all_datastore_units_masked_and_inactive: true,
        mysql_unreachable_with_old_credentials: true,
        default_redis_unreachable_with_old_credentials: true,
        cache_redis_unreachable_with_old_credentials: true,
        mysql_probe_sha256: probes.mysql_evidence_sha256.clone(),
        default_redis_probe_sha256: probes.default_redis_evidence_sha256.clone(),
        cache_redis_probe_sha256: probes.cache_redis_evidence_sha256.clone(),
        backup_receipt_sha256: archive.receipt_sha256().to_string(),
        encrypted_backup_sha256: archive.encrypted_backup_sha256().to_string(),
        archive_source_fingerprint_sha256: archive.source_fingerprint_sha256().to_string(),
        archive_source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
        fenced_at_unix: existing_payload
            .as_ref()
            .map_or_else(unix_now, |payload| Ok(payload.fenced_at_unix))?,
    };
    if [
        payload.journal_anchor_event_sha256.as_str(),
        payload.armed_receipt_sha256.as_str(),
        payload.unit_inventory_sha256.as_str(),
        payload.mysql_probe_sha256.as_str(),
        payload.default_redis_probe_sha256.as_str(),
        payload.cache_redis_probe_sha256.as_str(),
        payload.backup_receipt_sha256.as_str(),
        payload.encrypted_backup_sha256.as_str(),
        payload.archive_source_fingerprint_sha256.as_str(),
        payload.archive_source_schema_sha256.as_str(),
    ]
    .into_iter()
    .any(|value| !is_lower_sha256(value))
    {
        return Err(SourceError::ReceiptInvalid);
    }
    persist_hmac_receipt(
        spec,
        LegacyRuntimeReceiptKind::DatastoreFence,
        path,
        &payload,
    )
}

fn persist_source_retirement_receipt(
    spec: &ProvisionSpec,
    permit: &DurableMutationPermit,
    units: &LegacyWriterUnits,
    native_authority_nodes_event_sha256: &str,
    probes: &RetirementProbes,
) -> Result<String, SourceError> {
    if permit.operation_id() != spec.operation_id
        || permit.generation() == 0
        || !is_lower_sha256(permit.event_sha256())
        || !is_lower_sha256(native_authority_nodes_event_sha256)
        || probes.mysql_reachable
        || probes.default_redis_reachable
        || probes.cache_redis_reachable
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let path = &execution.receipts.source_retirement_path;
    let existing_file = existing_receipt_file(path)?;
    let existing_payload = if let Some(existing_file) = existing_file {
        let bytes = read_owner_only_file(&existing_file, MAX_RECEIPT_BYTES)?;
        let envelope: SourceRetirementReceiptEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
        let canonical =
            serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
        if !spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceRetirement,
            &canonical,
            &envelope.hmac_sha256,
        ) {
            return Err(SourceError::ReceiptInvalid);
        }
        Some(envelope.payload)
    } else {
        None
    };
    let payload = SourceRetirementReceiptPayload {
        schema_version: 1,
        operation_id: spec.operation_id.clone(),
        journal_anchor_generation: existing_payload
            .as_ref()
            .map_or(permit.generation(), |payload| {
                payload.journal_anchor_generation
            }),
        journal_anchor_checkpoint: ApplyCheckpoint::CutoverCommitted,
        journal_anchor_event_sha256: existing_payload.as_ref().map_or_else(
            || permit.event_sha256().to_string(),
            |payload| payload.journal_anchor_event_sha256.clone(),
        ),
        native_authority_nodes_event_sha256: native_authority_nodes_event_sha256.to_string(),
        result_checkpoint: ApplyCheckpoint::SourceRetired,
        unit_inventory_sha256: units.digest(),
        all_declared_units_masked_and_inactive: true,
        mysql_unreachable_with_old_credentials: true,
        default_redis_unreachable_with_old_credentials: true,
        cache_redis_unreachable_with_old_credentials: true,
        mysql_probe_sha256: probes.mysql_evidence_sha256.clone(),
        default_redis_probe_sha256: probes.default_redis_evidence_sha256.clone(),
        cache_redis_probe_sha256: probes.cache_redis_evidence_sha256.clone(),
        retired_at_unix: existing_payload
            .as_ref()
            .map_or_else(unix_now, |payload| Ok(payload.retired_at_unix))?,
    };
    if [
        payload.journal_anchor_event_sha256.as_str(),
        payload.native_authority_nodes_event_sha256.as_str(),
        payload.unit_inventory_sha256.as_str(),
        payload.mysql_probe_sha256.as_str(),
        payload.default_redis_probe_sha256.as_str(),
        payload.cache_redis_probe_sha256.as_str(),
    ]
    .into_iter()
    .any(|value| !is_lower_sha256(value))
    {
        return Err(SourceError::ReceiptInvalid);
    }
    persist_hmac_receipt(
        spec,
        LegacyRuntimeReceiptKind::SourceRetirement,
        path,
        &payload,
    )
}

fn persist_hmac_receipt<T: Serialize>(
    spec: &ProvisionSpec,
    kind: LegacyRuntimeReceiptKind,
    path: &Path,
    payload: &T,
) -> Result<String, SourceError> {
    let canonical = serde_json::to_vec(payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let hmac_sha256 = spec
        .source_receipt_binding_hmac_sha256(kind, &canonical)
        .ok_or(SourceError::ReceiptInvalid)?;
    let bytes = serde_json::to_vec(&serde_json::json!({
        "payload": payload,
        "hmac_sha256": hmac_sha256,
    }))
    .map_err(|_| SourceError::ReceiptInvalid)?;
    write_owner_only_create_new(path, &bytes)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn persist_traffic_drain_receipt(
    spec: &ProvisionSpec,
    journal_anchor: Option<&ApplyJournalSnapshot>,
    frozen: &FrozenTrafficReport,
) -> Result<String, SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let path = &execution.receipts.source_drain_path;
    let existing_file = existing_receipt_file(path)?;
    let existing_payload = if let Some(existing_file) = existing_file {
        let bytes = read_owner_only_file(&existing_file, MAX_TRAFFIC_RECEIPT_BYTES)?;
        let envelope: TrafficDrainReceiptEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| SourceError::ReceiptInvalid)?;
        let canonical =
            serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
        if !spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::SourceDrain,
            &canonical,
            &envelope.hmac_sha256,
        ) {
            return Err(SourceError::ReceiptInvalid);
        }
        Some(envelope.payload)
    } else {
        None
    };
    let (journal_anchor_generation, journal_anchor_checkpoint, journal_anchor_event_sha256) =
        match (existing_payload.as_ref(), journal_anchor) {
            (Some(payload), _) => (
                payload.journal_anchor_generation,
                payload.journal_anchor_checkpoint,
                payload.journal_anchor_event_sha256.clone(),
            ),
            (None, Some(anchor))
                if anchor.binding().operation_id() == spec.operation_id
                    && anchor.checkpoint() == ApplyCheckpoint::MaintenanceFenced
                    && anchor.generation() > 0
                    && is_lower_sha256(anchor.event_sha256()) =>
            {
                (
                    anchor.generation(),
                    anchor.checkpoint(),
                    anchor.event_sha256().to_string(),
                )
            }
            _ => return Err(SourceError::ReceiptInvalid),
        };
    let payload = TrafficDrainReceiptPayload {
        schema_version: 1,
        operation_id: frozen.operation_id.clone(),
        journal_anchor_generation,
        journal_anchor_checkpoint,
        journal_anchor_event_sha256,
        result_checkpoint: ApplyCheckpoint::SourceDrained,
        source_default_run_id: frozen.source_run_id.clone(),
        frozen_upload_key: frozen.upload_key.clone(),
        frozen_download_key: frozen.download_key.clone(),
        fenced_at_unix: existing_payload
            .as_ref()
            .map_or_else(unix_now, |payload| Ok(payload.fenced_at_unix))?,
        upload_fields: frozen.upload_fields,
        download_fields: frozen.download_fields,
        sorted_user_delta_count: frozen.delta_users,
        sorted_user_delta_sha256: frozen.delta_sha256.clone(),
        upload_delta_sum: frozen.upload_sum.to_string(),
        download_delta_sum: frozen.download_sum.to_string(),
        deltas: frozen.deltas.clone(),
        // Empty receipts need no delta fold. A non-empty receipt remains false
        // here until the independent PostgreSQL traffic-fold ledger is sealed;
        // SourceDrained binds the frozen receipt without altering the formal
        // 27-table MySQL snapshot.
        delta_applied_exactly_once: frozen.delta_users == 0,
    };
    let canonical = serde_json::to_vec(&payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let hmac_sha256 = spec
        .source_receipt_binding_hmac_sha256(LegacyRuntimeReceiptKind::SourceDrain, &canonical)
        .ok_or(SourceError::ReceiptInvalid)?;
    let envelope = TrafficDrainReceiptEnvelope {
        payload,
        hmac_sha256,
    };
    let bytes = serde_json::to_vec(&envelope).map_err(|_| SourceError::ReceiptInvalid)?;
    write_owner_only_create_new_bounded(path, &bytes, MAX_TRAFFIC_RECEIPT_BYTES)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn summarize_traffic_delta_records(
    records: &[FrozenTrafficDeltaRecord],
) -> Result<(u64, u64, String, i128, i128), SourceError> {
    if records.len() > MAX_FROZEN_TRAFFIC_USERS {
        return Err(SourceError::ReceiptInvalid);
    }
    let mut upload_fields = 0_u64;
    let mut download_fields = 0_u64;
    let mut upload_sum = 0_i128;
    let mut download_sum = 0_i128;
    let mut previous_user = None;
    let mut digest = Sha256::new();
    digest.update(b"v2board-frozen-traffic-user-deltas-v1\0");
    for FrozenTrafficDeltaRecord(user_id, upload, download) in records {
        if *user_id <= 0
            || previous_user.is_some_and(|previous| previous >= *user_id)
            || (upload.is_none() && download.is_none())
            || upload.is_some_and(|value| value < 0)
            || download.is_some_and(|value| value < 0)
        {
            return Err(SourceError::ReceiptInvalid);
        }
        previous_user = Some(*user_id);
        let upload_value = upload.unwrap_or(0);
        let download_value = download.unwrap_or(0);
        upload_fields = upload_fields.saturating_add(u64::from(upload.is_some()));
        download_fields = download_fields.saturating_add(u64::from(download.is_some()));
        upload_sum = upload_sum
            .checked_add(i128::from(upload_value))
            .ok_or(SourceError::ReceiptInvalid)?;
        download_sum = download_sum
            .checked_add(i128::from(download_value))
            .ok_or(SourceError::ReceiptInvalid)?;
        digest_field(&mut digest, user_id.to_string().as_bytes());
        digest_field(&mut digest, upload_value.to_string().as_bytes());
        digest_field(&mut digest, download_value.to_string().as_bytes());
    }
    Ok((
        upload_fields,
        download_fields,
        hex::encode(digest.finalize()),
        upload_sum,
        download_sum,
    ))
}

pub fn verify_frozen_traffic_receipt(
    spec: &ProvisionSpec,
) -> Result<VerifiedFrozenTrafficReceipt, SourceError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let receipt_file = existing_receipt_file(&execution.receipts.source_drain_path)?
        .ok_or(SourceError::ReceiptInvalid)?;
    let bytes = read_owner_only_file(&receipt_file, MAX_TRAFFIC_RECEIPT_BYTES)?;
    verify_frozen_traffic_receipt_bytes(spec, &bytes)
}

/// Verifies an in-memory source-drain receipt with the same scoped-HMAC and
/// canonical delta checks as the owner-only filesystem path. The encrypted
/// legacy archive uses this entry point after decryption so it cannot acquire
/// a second, weaker receipt parser.
pub(crate) fn verify_frozen_traffic_receipt_bytes(
    spec: &ProvisionSpec,
    bytes: &[u8],
) -> Result<VerifiedFrozenTrafficReceipt, SourceError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_TRAFFIC_RECEIPT_BYTES {
        return Err(SourceError::ReceiptInvalid);
    }
    let source = legacy_source(spec)?;
    let envelope: TrafficDrainReceiptEnvelope =
        serde_json::from_slice(bytes).map_err(|_| SourceError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| SourceError::ReceiptInvalid)?;
    let expected_upload = format!(
        "{}v2board_migration:{}:frozen_upload_traffic",
        source.redis_connection_prefix, spec.operation_id
    );
    let expected_download = format!(
        "{}v2board_migration:{}:frozen_download_traffic",
        source.redis_connection_prefix, spec.operation_id
    );
    let payload = envelope.payload;
    let (
        record_upload_fields,
        record_download_fields,
        record_digest,
        record_upload_sum,
        record_download_sum,
    ) = summarize_traffic_delta_records(&payload.deltas)?;
    let declared_upload_sum = payload
        .upload_delta_sum
        .parse::<i128>()
        .map_err(|_| SourceError::ReceiptInvalid)?;
    let declared_download_sum = payload
        .download_delta_sum
        .parse::<i128>()
        .map_err(|_| SourceError::ReceiptInvalid)?;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::SourceDrain,
        &canonical,
        &envelope.hmac_sha256,
    ) || payload.schema_version != 1
        || payload.operation_id != spec.operation_id
        || payload.journal_anchor_generation == 0
        || payload.journal_anchor_checkpoint != ApplyCheckpoint::MaintenanceFenced
        || !is_lower_sha256(&payload.journal_anchor_event_sha256)
        || payload.result_checkpoint != ApplyCheckpoint::SourceDrained
        || payload.source_default_run_id.len() != 40
        || !payload
            .source_default_run_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || payload.frozen_upload_key != expected_upload
        || payload.frozen_download_key != expected_download
        || payload.fenced_at_unix <= 0
        || payload.delta_applied_exactly_once != (payload.sorted_user_delta_count == 0)
        || !is_lower_sha256(&payload.sorted_user_delta_sha256)
        || payload.upload_fields != record_upload_fields
        || payload.download_fields != record_download_fields
        || payload.sorted_user_delta_count != payload.deltas.len() as u64
        || payload.sorted_user_delta_sha256 != record_digest
        || declared_upload_sum != record_upload_sum
        || declared_download_sum != record_download_sum
        || declared_upload_sum < 0
        || declared_download_sum < 0
    {
        return Err(SourceError::ReceiptInvalid);
    }
    Ok(VerifiedFrozenTrafficReceipt {
        operation_id: payload.operation_id,
        maintenance_fenced_generation: payload.journal_anchor_generation,
        maintenance_fenced_event_sha256: payload.journal_anchor_event_sha256,
        source_default_run_id: payload.source_default_run_id,
        frozen_upload_key: payload.frozen_upload_key,
        frozen_download_key: payload.frozen_download_key,
        fenced_at_unix: payload.fenced_at_unix,
        upload_fields: payload.upload_fields,
        download_fields: payload.download_fields,
        sorted_user_delta_count: payload.sorted_user_delta_count,
        sorted_user_delta_sha256: payload.sorted_user_delta_sha256,
        upload_delta_sum: payload.upload_delta_sum,
        download_delta_sum: payload.download_delta_sum,
        deltas: payload.deltas,
        delta_applied_exactly_once: payload.delta_applied_exactly_once,
        receipt_sha256: hex::encode(Sha256::digest(bytes)),
    })
}

/// Removes the plaintext source-drain receipt only after an encrypted archive
/// has bound its exact digest. Missing is an idempotent lost-ack success; any
/// present file is re-opened with the owner-only checks and fully re-verified
/// before unlink plus parent-directory fsync.
pub(crate) fn remove_verified_frozen_traffic_receipt(
    spec: &ProvisionSpec,
    expected_sha256: &str,
) -> Result<(), SourceError> {
    if !is_lower_sha256(expected_sha256) {
        return Err(SourceError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let path = &execution.receipts.source_drain_path;
    let Some(existing) = existing_receipt_file(path)? else {
        return Ok(());
    };
    let verified = verify_frozen_traffic_receipt(spec)?;
    let bytes = read_owner_only_file(&existing, MAX_TRAFFIC_RECEIPT_BYTES)?;
    if verified.receipt_sha256 != expected_sha256
        || hex::encode(Sha256::digest(&bytes)) != expected_sha256
    {
        return Err(SourceError::ReceiptInvalid);
    }
    fs::remove_file(&existing).map_err(|_| SourceError::ReceiptInvalid)?;
    File::open(path.parent().ok_or(SourceError::ReceiptInvalid)?)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SourceError::ReceiptInvalid)
}

fn frozen_report_from_receipt(
    receipt: VerifiedFrozenTrafficReceipt,
) -> Result<FrozenTrafficReport, SourceError> {
    let upload_sum = receipt
        .upload_delta_sum
        .parse::<i128>()
        .map_err(|_| SourceError::ReceiptInvalid)?;
    let download_sum = receipt
        .download_delta_sum
        .parse::<i128>()
        .map_err(|_| SourceError::ReceiptInvalid)?;
    Ok(FrozenTrafficReport {
        operation_id: receipt.operation_id,
        source_run_id: receipt.source_default_run_id,
        upload_key: receipt.frozen_upload_key,
        download_key: receipt.frozen_download_key,
        upload_fields: receipt.upload_fields,
        download_fields: receipt.download_fields,
        delta_users: receipt.sorted_user_delta_count,
        delta_sha256: receipt.sorted_user_delta_sha256,
        upload_sum,
        download_sum,
        deltas: receipt.deltas,
    })
}

fn unix_now() -> Result<i64, SourceError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_secs()).ok())
        .filter(|value| *value > 0)
        .ok_or(SourceError::ReceiptInvalid)
}

pub(crate) fn read_owner_only_file(path: &Path, maximum: u64) -> Result<Vec<u8>, SourceError> {
    let path_metadata = fs::symlink_metadata(path).map_err(|_| SourceError::ReceiptInvalid)?;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.len() == 0
        || path_metadata.len() > maximum
        || path_metadata.uid() != 0
        || path_metadata.permissions().mode() & 0o077 != 0
        || path_metadata.nlink() != 1
    {
        return Err(SourceError::ReceiptInvalid);
    }
    let mut file = File::open(path).map_err(|_| SourceError::ReceiptInvalid)?;
    let opened = file.metadata().map_err(|_| SourceError::ReceiptInvalid)?;
    if opened.dev() != path_metadata.dev() || opened.ino() != path_metadata.ino() {
        return Err(SourceError::ReceiptInvalid);
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| SourceError::ReceiptInvalid)?;
    if bytes.len() as u64 != opened.len() {
        return Err(SourceError::ReceiptInvalid);
    }
    Ok(bytes)
}

fn write_owner_only_create_new(path: &Path, bytes: &[u8]) -> Result<(), SourceError> {
    publish_owner_only_no_clobber(path, bytes, MAX_RECEIPT_BYTES)
}

fn write_owner_only_create_new_bounded(
    path: &Path,
    bytes: &[u8],
    maximum: u64,
) -> Result<(), SourceError> {
    publish_owner_only_no_clobber(path, bytes, maximum)
}

pub(crate) fn publish_owner_only_no_clobber(
    path: &Path,
    bytes: &[u8],
    maximum: u64,
) -> Result<(), SourceError> {
    if bytes.is_empty() || bytes.len() as u64 > maximum || !path.is_absolute() {
        return Err(SourceError::ReceiptInvalid);
    }
    let parent = path.parent().ok_or(SourceError::ReceiptInvalid)?;
    validate_private_parent(parent)?;
    if let Some(existing) = existing_receipt_file(path)? {
        if read_owner_only_file(&existing, maximum)? != bytes {
            return Err(SourceError::SourceDrift);
        }
        if existing == path {
            return Ok(());
        }
        fs::hard_link(&existing, path).map_err(|_| SourceError::ReceiptInvalid)?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| SourceError::ReceiptInvalid)?;
        fs::remove_file(&existing).map_err(|_| SourceError::ReceiptInvalid)?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| SourceError::ReceiptInvalid)?;
        return Ok(());
    }
    let partial = receipt_partial_path(path)?;
    let writing = receipt_writing_path(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&writing)
        .map_err(|_| SourceError::ReceiptInvalid)?;
    file.write_all(bytes)
        .map_err(|_| SourceError::ReceiptInvalid)?;
    file.sync_all().map_err(|_| SourceError::ReceiptInvalid)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SourceError::ReceiptInvalid)?;
    drop(file);
    fs::hard_link(&writing, &partial).map_err(|_| SourceError::ReceiptInvalid)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SourceError::ReceiptInvalid)?;
    fs::remove_file(&writing).map_err(|_| SourceError::ReceiptInvalid)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SourceError::ReceiptInvalid)?;
    publish_owner_only_no_clobber(path, bytes, maximum)
}

pub(crate) fn existing_receipt_file(path: &Path) -> Result<Option<PathBuf>, SourceError> {
    let partial = receipt_partial_path(path)?;
    reconcile_receipt_writing(path, &partial)?;
    let final_metadata = optional_metadata(path)?;
    let partial_metadata = optional_metadata(&partial)?;
    match (final_metadata, partial_metadata) {
        (None, None) => Ok(None),
        (Some(_), None) => Ok(Some(path.to_path_buf())),
        (None, Some(_)) => Ok(Some(partial)),
        (Some(final_metadata), Some(partial_metadata)) => {
            if !final_metadata.is_file()
                || final_metadata.file_type().is_symlink()
                || !partial_metadata.is_file()
                || partial_metadata.file_type().is_symlink()
                || final_metadata.dev() != partial_metadata.dev()
                || final_metadata.ino() != partial_metadata.ino()
                || final_metadata.nlink() != 2
                || partial_metadata.nlink() != 2
            {
                return Err(SourceError::ReceiptInvalid);
            }
            fs::remove_file(&partial).map_err(|_| SourceError::ReceiptInvalid)?;
            File::open(path.parent().ok_or(SourceError::ReceiptInvalid)?)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| SourceError::ReceiptInvalid)?;
            Ok(Some(path.to_path_buf()))
        }
    }
}

fn reconcile_receipt_writing(path: &Path, partial: &Path) -> Result<(), SourceError> {
    let writing = receipt_writing_path(path)?;
    let Some(writing_metadata) = optional_metadata(&writing)? else {
        return Ok(());
    };
    if !writing_metadata.is_file()
        || writing_metadata.file_type().is_symlink()
        || writing_metadata.uid() != 0
        || writing_metadata.permissions().mode() & 0o077 != 0
    {
        return Err(SourceError::ReceiptInvalid);
    }
    if let Some(partial_metadata) = optional_metadata(partial)? {
        if !partial_metadata.is_file()
            || partial_metadata.file_type().is_symlink()
            || partial_metadata.dev() != writing_metadata.dev()
            || partial_metadata.ino() != writing_metadata.ino()
            || partial_metadata.nlink() != 2
            || writing_metadata.nlink() != 2
        {
            return Err(SourceError::ReceiptInvalid);
        }
    } else if writing_metadata.nlink() != 1 {
        return Err(SourceError::ReceiptInvalid);
    }
    fs::remove_file(&writing).map_err(|_| SourceError::ReceiptInvalid)?;
    File::open(path.parent().ok_or(SourceError::ReceiptInvalid)?)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SourceError::ReceiptInvalid)
}

fn optional_metadata(path: &Path) -> Result<Option<fs::Metadata>, SourceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(SourceError::ReceiptInvalid),
    }
}

fn receipt_partial_path(path: &Path) -> Result<PathBuf, SourceError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .ok_or(SourceError::ReceiptInvalid)?;
    Ok(path.with_file_name(format!(".{name}.partial")))
}

fn receipt_writing_path(path: &Path) -> Result<PathBuf, SourceError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .ok_or(SourceError::ReceiptInvalid)?;
    Ok(path.with_file_name(format!(".{name}.writing")))
}

fn validate_private_parent(path: &Path) -> Result<(), SourceError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| SourceError::ReceiptInvalid)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(SourceError::ReceiptInvalid);
    }
    Ok(())
}

async fn drain_legacy_redis(
    spec: &ProvisionSpec,
    source: &SourceSpec,
    horizon_physical_prefix: &str,
    journal_anchor: Option<&ApplyJournalSnapshot>,
) -> Result<RedisDrainReport, SourceError> {
    let connection_prefix = source.redis_connection_prefix.as_bytes();
    if connection_prefix.is_empty()
        || !horizon_physical_prefix.starts_with(&source.redis_connection_prefix)
    {
        return Err(SourceError::RedisDrainIncomplete);
    }
    let cache_prefix = physical_cache_prefix(source)?;
    let same_database =
        redis_identity(&source.redis_default_url)? == redis_identity(&source.redis_cache_url)?;
    let mut default = redis_connection(&source.redis_default_url).await?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or(SourceError::ReceiptInvalid)?;
    let frozen = if existing_receipt_file(&execution.receipts.source_drain_path)?.is_some() {
        frozen_report_from_receipt(verify_frozen_traffic_receipt(spec)?)?
    } else {
        freeze_and_digest_traffic(&mut default, spec, source).await?
    };
    let traffic_receipt_sha256 = persist_traffic_drain_receipt(spec, journal_anchor, &frozen)?;
    let default_pattern = [connection_prefix, b"*"].concat();
    let default_keys = scan_keys(&mut default, &default_pattern).await?;
    let queue_prefix = [connection_prefix, b"queues:"].concat();
    let reset_key = [connection_prefix, b"traffic_reset_lock"].concat();
    let horizon_prefix = horizon_physical_prefix.as_bytes();
    let mut queue_notify = Vec::new();
    let mut horizon = Vec::new();
    let mut durable_queue_items = 0_u64;
    let mut traffic_reset_locks = 0_u64;
    let mut unknown_default = 0_u64;
    for key in &default_keys {
        if key == frozen.upload_key.as_bytes() || key == frozen.download_key.as_bytes() {
            // Operation-owned frozen traffic remains intact until an exact-once
            // delta applier has durably folded it into the transactional source.
        } else if key == &reset_key {
            traffic_reset_locks = traffic_reset_locks.saturating_add(1);
        } else if key.starts_with(&queue_prefix) {
            let count = collection_count(&mut default, key).await?;
            if key.ends_with(b":notify") {
                queue_notify.push(key.clone());
            } else {
                durable_queue_items = durable_queue_items.saturating_add(count);
            }
        } else if key.starts_with(horizon_prefix) {
            horizon.push(key.clone());
        } else if same_database && key.starts_with(&cache_prefix) {
            // Cleared below as the logout-all cache namespace.
        } else {
            unknown_default = unknown_default.saturating_add(1);
        }
    }
    let traffic_fields = frozen.upload_fields.saturating_add(frozen.download_fields);
    let traffic_delta_applied_exactly_once = frozen.delta_users == 0;
    if durable_queue_items != 0 || traffic_reset_locks != 0 || unknown_default != 0 {
        return Ok(RedisDrainReport {
            source_default_run_id: frozen.source_run_id,
            frozen_upload_key: frozen.upload_key,
            frozen_download_key: frozen.download_key,
            traffic_delta_users: frozen.delta_users,
            traffic_delta_sha256: frozen.delta_sha256,
            upload_delta_sum: frozen.upload_sum.to_string(),
            download_delta_sum: frozen.download_sum.to_string(),
            traffic_delta_applied_exactly_once,
            traffic_receipt_sha256,
            durable_queue_items,
            traffic_fields,
            traffic_reset_locks,
            queue_notify_keys_deleted: 0,
            horizon_metadata_keys_deleted: 0,
            session_cache_token_keys_deleted: 0,
            unknown_default_owned_keys: unknown_default,
            unknown_cache_owned_keys: 0,
            logout_all: false,
            default_namespace_empty_after_drain: false,
            cache_namespace_empty_after_drain: false,
        });
    }
    let queue_notify_deleted = unlink_exact(&mut default, &queue_notify).await?;
    let horizon_deleted = unlink_exact(&mut default, &horizon).await?;

    let mut cache = if same_database {
        default.clone()
    } else {
        redis_connection(&source.redis_cache_url).await?
    };
    let cache_pattern = [cache_prefix.as_slice(), b"*"].concat();
    let cache_keys = scan_keys(&mut cache, &cache_pattern).await?;
    let cache_deleted = unlink_exact(&mut cache, &cache_keys).await?;
    let remaining_default = scan_keys(&mut default, &default_pattern).await?;
    let remaining_default_owned = remaining_default.iter().filter(|key| {
        !(same_database && key.starts_with(&cache_prefix))
            && key.as_slice() != frozen.upload_key.as_bytes()
            && key.as_slice() != frozen.download_key.as_bytes()
    });
    let default_empty = remaining_default_owned.count() == 0;
    let cache_empty = scan_keys(&mut cache, &cache_pattern).await?.is_empty();
    Ok(RedisDrainReport {
        source_default_run_id: frozen.source_run_id,
        frozen_upload_key: frozen.upload_key,
        frozen_download_key: frozen.download_key,
        traffic_delta_users: frozen.delta_users,
        traffic_delta_sha256: frozen.delta_sha256,
        upload_delta_sum: frozen.upload_sum.to_string(),
        download_delta_sum: frozen.download_sum.to_string(),
        traffic_delta_applied_exactly_once,
        traffic_receipt_sha256,
        durable_queue_items,
        traffic_fields,
        traffic_reset_locks,
        queue_notify_keys_deleted: queue_notify_deleted,
        horizon_metadata_keys_deleted: horizon_deleted,
        session_cache_token_keys_deleted: cache_deleted,
        unknown_default_owned_keys: unknown_default,
        unknown_cache_owned_keys: 0,
        logout_all: cache_empty,
        default_namespace_empty_after_drain: default_empty,
        cache_namespace_empty_after_drain: cache_empty,
    })
}

pub async fn fingerprint_mysql(database_url: &str) -> Result<String, SourceError> {
    fingerprint_mysql_and_schema(database_url)
        .await
        .map(|(fingerprint, _)| fingerprint)
}

pub(crate) async fn fingerprint_mysql_for_strategy(
    database_url: &str,
    strategy: LegacyConversionStrategy,
) -> Result<String, SourceError> {
    match strategy {
        LegacyConversionStrategy::PreserveAll => fingerprint_mysql(database_url).await,
        LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs => {
            fingerprint_mysql_and_schema_for_strategy(database_url, strategy)
                .await
                .map(|(fingerprint, _)| fingerprint)
        }
    }
}

pub(crate) async fn fingerprint_mysql_and_schema(
    database_url: &str,
) -> Result<(String, String), SourceError> {
    fingerprint_mysql_and_schema_inner(database_url, LegacyConversionStrategy::PreserveAll).await
}

pub(crate) async fn fingerprint_mysql_and_schema_for_strategy(
    database_url: &str,
    strategy: LegacyConversionStrategy,
) -> Result<(String, String), SourceError> {
    match strategy {
        LegacyConversionStrategy::PreserveAll => fingerprint_mysql_and_schema(database_url).await,
        LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs => {
            fingerprint_mysql_and_schema_inner(database_url, strategy).await
        }
    }
}

async fn fingerprint_mysql_and_schema_inner(
    database_url: &str,
    strategy: LegacyConversionStrategy,
) -> Result<(String, String), SourceError> {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(DATASTORE_TIMEOUT)
        .connect(database_url)
        .await
        .map_err(|_| SourceError::Mysql)?;
    let schema_before = semantic_schema_hash(&pool)
        .await
        .map_err(|_| SourceError::Mysql)?;
    let fingerprint = fingerprint_mysql_pool(&pool, strategy).await?;
    let schema_after = semantic_schema_hash(&pool)
        .await
        .map_err(|_| SourceError::Mysql)?;
    pool.close().await;
    if schema_before != schema_after {
        return Err(SourceError::SourceDrift);
    }
    Ok((fingerprint, schema_after))
}

async fn fingerprint_mysql_pool(
    pool: &MySqlPool,
    strategy: LegacyConversionStrategy,
) -> Result<String, SourceError> {
    let failed_jobs_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `failed_jobs`")
        .fetch_one(pool)
        .await
        .map_err(|_| SourceError::Mysql)?;
    if failed_jobs_before != 0 {
        return Err(SourceError::SourceDrift);
    }
    let (_, fingerprint) =
        fingerprint_legacy_source_for_strategy(pool, DEFAULT_BATCH_SIZE, strategy)
            .await
            .map_err(|_| SourceError::Mysql)?;
    let failed_jobs_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `failed_jobs`")
        .fetch_one(pool)
        .await
        .map_err(|_| SourceError::Mysql)?;
    if failed_jobs_after != 0 {
        return Err(SourceError::SourceDrift);
    }
    Ok(fingerprint.canonical_sha256)
}

struct RetirementProbes {
    mysql_reachable: bool,
    default_redis_reachable: bool,
    cache_redis_reachable: bool,
    mysql_evidence_sha256: String,
    default_redis_evidence_sha256: String,
    cache_redis_evidence_sha256: String,
}

fn run_retirement_probes(urls: (String, String, String)) -> Result<RetirementProbes, SourceError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| SourceError::ProbeRuntime)?;
    Ok(runtime.block_on(run_retirement_probes_async(urls)))
}

async fn run_retirement_probes_async(urls: (String, String, String)) -> RetirementProbes {
    let mysql_reachable = mysql_probe(&urls.0).await;
    let default_redis_reachable = redis_probe(&urls.1).await;
    let cache_redis_reachable = redis_probe(&urls.2).await;
    RetirementProbes {
        mysql_reachable,
        default_redis_reachable,
        cache_redis_reachable,
        mysql_evidence_sha256: probe_evidence("mysql", &urls.0, mysql_reachable),
        default_redis_evidence_sha256: probe_evidence(
            "redis_default",
            &urls.1,
            default_redis_reachable,
        ),
        cache_redis_evidence_sha256: probe_evidence("redis_cache", &urls.2, cache_redis_reachable),
    }
}

async fn mysql_probe(url: &str) -> bool {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(DATASTORE_TIMEOUT)
        .connect_lazy(url);
    let Ok(pool) = pool else {
        return false;
    };
    let reachable = timeout(
        DATASTORE_TIMEOUT,
        sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&pool),
    )
    .await
    .is_ok_and(|result| matches!(result, Ok(1)));
    pool.close().await;
    reachable
}

async fn redis_probe(url: &str) -> bool {
    let Ok(client) = redis::Client::open(url) else {
        return false;
    };
    let Ok(Ok(mut connection)) =
        timeout(DATASTORE_TIMEOUT, client.get_multiplexed_async_connection()).await
    else {
        return false;
    };
    timeout(
        DATASTORE_TIMEOUT,
        redis::cmd("PING").query_async::<String>(&mut connection),
    )
    .await
    .is_ok_and(|result| result.as_deref() == Ok("PONG"))
}

fn probe_evidence(kind: &str, url: &str, reachable: bool) -> String {
    domain_hash_fields(
        b"v2board-old-credential-live-probe-v1\0",
        [kind.as_bytes(), url.as_bytes(), &[u8::from(reachable)]],
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn private_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "v2board-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create private root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("set private mode");
        root
    }

    #[test]
    fn unit_inventory_is_closed_and_unambiguous() {
        let valid = LegacyWriterUnits {
            api: vec!["legacy-api.service".into()],
            workers: vec!["legacy-horizon.service".into()],
            schedulers: vec![
                "legacy-schedule.timer".into(),
                "legacy-schedule.service".into(),
            ],
            local_datastores: vec!["mysql.service".into(), "redis.service".into()],
        };
        assert!(valid.validate().is_ok());
        let mut mismatched = valid.clone();
        mismatched.schedulers[1] = "other-schedule.service".into();
        assert!(matches!(
            mismatched.validate(),
            Err(SourceError::InvalidPolicy)
        ));
        let mut duplicate = valid;
        duplicate.workers.push("legacy-api.service".into());
        assert!(matches!(
            duplicate.validate(),
            Err(SourceError::InvalidPolicy)
        ));
        assert!(exact_systemd_unit_relation(
            "legacy-schedule.service",
            "legacy-schedule.service"
        ));
        assert!(!exact_systemd_unit_relation(
            "legacy-schedule.service other.service",
            "legacy-schedule.service"
        ));
    }

    #[test]
    fn fixed_command_rejects_a_secret_in_argv() {
        let request = LegacyCommandRequest::new(LegacyProgram::Systemctl)
            .arg("status")
            .arg("secret.service")
            .redact("secret");
        assert_eq!(request.validate(), Err(LegacyRunnerError::SecretInArgv));
    }

    #[derive(Default)]
    struct MockRunner {
        calls: Vec<Vec<String>>,
    }

    impl LegacyCommandRunner for MockRunner {
        fn run(
            &mut self,
            request: LegacyCommandRequest,
        ) -> Result<LegacyCommandOutput, LegacyRunnerError> {
            let args = request
                .args
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            self.calls.push(args.clone());
            if request.program == LegacyProgram::Id {
                return Ok(LegacyCommandOutput::success(b"0\n".to_vec()));
            }
            if args.first().is_some_and(|value| value == "show") {
                if args.contains(&"--property=UnitFileState".to_string())
                    && !args.contains(&"--property=ActiveState".to_string())
                {
                    return Ok(LegacyCommandOutput::success(b"loaded\ndisabled\n".to_vec()));
                }
                return Ok(LegacyCommandOutput::success(
                    b"loaded\ninactive\ndead\ndisabled\n".to_vec(),
                ));
            }
            Ok(LegacyCommandOutput::success(Vec::new()))
        }
    }

    #[test]
    fn systemd_verification_requires_inactive_and_disabled_for_retirement() {
        let mut runner = MockRunner::default();
        let units = ["legacy-api.service".to_string()];
        verify_units_stopped(&mut runner, units.iter(), true).expect("disabled and inactive");
        assert_eq!(runner.calls.len(), 1);
        assert_eq!(
            runner.calls[0].last(),
            Some(&"legacy-api.service".to_string())
        );
    }

    #[test]
    fn drain_workers_can_remain_active_only_after_restart_is_disabled() {
        let mut runner = MockRunner::default();
        let units = ["legacy-queue.service".to_string()];
        verify_units_restart_disabled(&mut runner, units.iter())
            .expect("worker restart is disabled before drain");
        assert!(runner.calls[0].contains(&"--property=UnitFileState".to_string()));
        assert!(!runner.calls[0].contains(&"--property=ActiveState".to_string()));
    }

    #[test]
    fn probe_evidence_is_secret_free_and_state_sensitive() {
        let url = "mysql://old:super-secret@127.0.0.1/old";
        let unreachable = probe_evidence("mysql", url, false);
        let reachable = probe_evidence("mysql", url, true);
        assert!(is_lower_sha256(&unreachable));
        assert_ne!(unreachable, reachable);
        assert!(!unreachable.contains("super-secret"));
    }

    #[test]
    fn datastore_fence_capability_parsers_and_pause_budget_fail_closed() {
        assert!(exact_mysql_fence_grants(&[
            "GRANT USAGE ON *.* TO `fence`@`%`".into(),
            "GRANT PROCESS, SYSTEM_VARIABLES_ADMIN ON *.* TO `fence`@`%`".into(),
        ]));
        assert!(!exact_mysql_fence_grants(&[
            "GRANT PROCESS, SYSTEM_VARIABLES_ADMIN, INSERT ON *.* TO `fence`@`%`".into(),
        ]));
        assert!(redis_version_at_least_6_2("6.2.0"));
        assert!(!redis_version_at_least_6_2("6.0.20"));
        let fresh = RedisPauseAck {
            audit_unix: 1,
            monotonic: Instant::now(),
        };
        assert!(require_redis_pause_ack_window(&fresh).is_ok());
        let expired = RedisPauseAck {
            audit_unix: 1,
            monotonic: Instant::now()
                .checked_sub(Duration::from_millis(REDIS_WRITE_PAUSE_MILLISECONDS))
                .expect("represent expired pause"),
        };
        assert!(require_redis_pause_ack_window(&expired).is_err());
    }

    #[test]
    fn systemctl_property_parser_is_keyed_and_rejects_ambiguity() {
        let properties = parse_systemctl_properties(
            b"Id=mysql.service\nLoadState=loaded\nEnvironment=VALUE=a=b\n",
        )
        .expect("parse keyed systemctl output");
        assert_eq!(
            properties.get("Id").map(String::as_str),
            Some("mysql.service")
        );
        assert_eq!(
            properties.get("Environment").map(String::as_str),
            Some("VALUE=a=b")
        );
        assert!(parse_systemctl_properties(b"Id=a.service\nId=b.service\n").is_err());
        assert!(parse_systemctl_properties(b"not-keyed\n").is_err());
    }

    #[test]
    fn proc_listener_parser_preserves_ipv4_ipv6_port_and_inode() {
        let ipv4 = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:0CEA 00000000:0000 0A 00000000:00000000 00:00000000 00000000  999 0 12345 1 0000000000000000 100 0 0 10 0\n";
        let rows = parse_proc_tcp_listeners(ipv4).expect("parse IPv4 listener");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].address, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].port, 3306);
        assert_eq!(rows[0].inode, 12345);

        let ipv6 = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000000000000000000001000000:18EB 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  999 0 67890 1 0000000000000000 100 0 0 10 0\n";
        let rows = parse_proc_tcp_listeners(ipv6).expect("parse IPv6 listener");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].address, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].port, 6379);
        assert_eq!(rows[0].inode, 67890);
    }

    #[test]
    fn source_listener_endpoint_is_literal_loopback_with_explicit_defaults() {
        assert_eq!(
            source_listener_endpoint("mysql://u:p@127.0.0.1/db").unwrap(),
            ("127.0.0.1".parse().unwrap(), 3306)
        );
        assert_eq!(
            source_listener_endpoint("redis://u:p@[::1]/1").unwrap(),
            ("::1".parse().unwrap(), 6379)
        );
        assert!(source_listener_endpoint("redis://u:p@localhost/1").is_err());
        assert!(source_listener_endpoint("redis://u:p@10.0.0.1/1").is_err());
    }

    #[test]
    fn receipt_publication_recovers_writing_partial_and_lost_ack_windows() {
        let root = private_root("source-receipt-publication");
        let path = root.join("source-drain.json");
        let expected = br#"{"complete":true}"#;
        let writing = receipt_writing_path(&path).unwrap();
        fs::write(&writing, b"truncated").expect("simulate interrupted uncommitted write");
        fs::set_permissions(&writing, fs::Permissions::from_mode(0o600)).unwrap();
        publish_owner_only_no_clobber(&path, expected, 1024)
            .expect("uncommitted writing file is safely replaced");
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(!writing.exists());

        fs::remove_file(&path).unwrap();
        let partial = receipt_partial_path(&path).unwrap();
        fs::write(&partial, expected).expect("simulate fsynced partial");
        fs::set_permissions(&partial, fs::Permissions::from_mode(0o600)).unwrap();
        publish_owner_only_no_clobber(&path, expected, 1024)
            .expect("publish exact committed partial");
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(!partial.exists());

        fs::remove_file(&path).unwrap();
        fs::write(&partial, expected).unwrap();
        fs::set_permissions(&partial, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&partial, &path).expect("simulate lost ack after final hard link");
        publish_owner_only_no_clobber(&path, expected, 1024)
            .expect("reconcile two-link lost ack state");
        assert!(!partial.exists());
        assert_eq!(fs::metadata(&path).unwrap().nlink(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn durable_traffic_records_preserve_presence_and_reject_reordering() {
        let records = vec![
            FrozenTrafficDeltaRecord(7, Some(0), None),
            FrozenTrafficDeltaRecord(9, None, Some(17)),
        ];
        let (upload_fields, download_fields, digest, upload_sum, download_sum) =
            summarize_traffic_delta_records(&records).expect("valid durable records");
        assert_eq!((upload_fields, download_fields), (1, 1));
        assert_eq!((upload_sum, download_sum), (0, 17));
        assert!(is_lower_sha256(&digest));
        let reordered = vec![records[1].clone(), records[0].clone()];
        assert!(summarize_traffic_delta_records(&reordered).is_err());
        assert!(
            summarize_traffic_delta_records(&[FrozenTrafficDeltaRecord(7, None, None)]).is_err()
        );
    }

    #[test]
    fn traffic_digest_uses_union_and_keeps_upload_only_users() {
        let upload = BTreeMap::from([(7, 11), (9, 13)]);
        let download = BTreeMap::from([(9, 17)]);
        let (deltas, digest, upload_sum, download_sum) =
            canonical_traffic_delta_summary(&upload, &download).expect("canonical traffic");
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0], FrozenTrafficDeltaRecord(7, Some(11), None));
        assert_eq!(upload_sum, 24);
        assert_eq!(download_sum, 17);
        assert!(is_lower_sha256(&digest));

        let upload_only_dropped = BTreeMap::from([(9, 13)]);
        let (_, lossy_digest, _, _) =
            canonical_traffic_delta_summary(&upload_only_dropped, &download)
                .expect("lossy comparison");
        assert_ne!(digest, lossy_digest);
    }

    #[tokio::test]
    #[ignore = "requires disposable MySQL 8.0/8.4 reader, fence, and root fixture URLs"]
    async fn mysql_super_read_only_fence_is_durable_exact_and_retryable() {
        let reader_url =
            std::env::var("V2BOARD_LEGACY_MYSQL_TEST_URL").expect("V2BOARD_LEGACY_MYSQL_TEST_URL");
        let root_url = std::env::var("V2BOARD_LEGACY_FIXTURE_DATABASE_URL")
            .expect("V2BOARD_LEGACY_FIXTURE_DATABASE_URL");
        let mut fence = Url::parse(&reader_url).expect("reader URL");
        fence
            .set_username("v2board_fence")
            .expect("set fence username");
        fence
            .set_password(Some("v2board-fence-test-password"))
            .expect("set fence password");
        fence.set_path("");
        let fence_url = fence.to_string();
        let root = MySqlPoolOptions::new()
            .max_connections(1)
            .connect(&root_url)
            .await
            .expect("root fixture connection");
        sqlx::raw_sql(
            "SET PERSIST super_read_only = OFF; SET PERSIST read_only = OFF; \
             DROP DATABASE IF EXISTS v2board_fence_probe; \
             CREATE DATABASE v2board_fence_probe; \
             CREATE TABLE v2board_fence_probe.probe (id BIGINT PRIMARY KEY); \
             INSERT INTO v2board_fence_probe.probe VALUES (1)",
        )
        .execute(&root)
        .await
        .expect("prepare disposable write probe");
        let outcome = async {
            let fence_pool = MySqlPoolOptions::new()
                .max_connections(1)
                .connect(&fence_url)
                .await
                .expect("grant inspection fence connection");
            let grants = sqlx::query_scalar::<_, String>("SHOW GRANTS")
                .fetch_all(&fence_pool)
                .await
                .expect("SHOW GRANTS for fence user");
            fence_pool.close().await;
            assert!(
                exact_mysql_fence_grants(&grants),
                "diagnostic exact fence grants: {grants:?}"
            );
            let first = arm_mysql_durable_write_fence_urls(&fence_url, &reader_url)
                .await
                .expect("first durable fence attempt");
            let second = arm_mysql_durable_write_fence_urls(&fence_url, &reader_url)
                .await
                .expect("idempotent durable fence retry");
            if first != second || first.2 != 0 || first.3 != 0 || first.4 != 0 {
                return Err(SourceError::SourceDrift);
            }
            if sqlx::query("INSERT INTO v2board_fence_probe.probe VALUES (2)")
                .execute(&root)
                .await
                .is_ok()
            {
                return Err(SourceError::SourceDrift);
            }
            Ok(())
        }
        .await;
        sqlx::raw_sql(
            "SET PERSIST super_read_only = OFF; SET PERSIST read_only = OFF; \
             DROP DATABASE IF EXISTS v2board_fence_probe",
        )
        .execute(&root)
        .await
        .expect("reset disposable MySQL fence fixture");
        root.close().await;
        outcome.expect("durable MySQL write fence");
    }

    #[tokio::test]
    #[ignore = "requires V2BOARD_LEGACY_REDIS_TEST_URL pointing at a disposable Redis database"]
    async fn redis_traffic_freeze_is_atomic_retry_exact_and_preserves_direction_presence() {
        let redis_url =
            std::env::var("V2BOARD_LEGACY_REDIS_TEST_URL").expect("V2BOARD_LEGACY_REDIS_TEST_URL");
        let mut spec = crate::manifest::tests::legacy_spec_for_orchestration();
        let redis_prefix = format!("v2board_redis_e2e_{}:", spec.operation_id);
        let source = match &mut spec.flow {
            ProvisionFlow::LegacyReferenceMigration { source, .. } => source,
            _ => panic!("shared fixture must be a legacy migration"),
        };
        source.redis_default_url = redis_url;
        source.redis_connection_prefix = redis_prefix;
        let source = match &spec.flow {
            ProvisionFlow::LegacyReferenceMigration { source, .. } => source,
            _ => panic!("shared fixture must be a legacy migration"),
        };
        let mut connection = redis_connection(&source.redis_default_url)
            .await
            .expect("disposable Redis connection");
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut connection)
            .await
            .expect("reset disposable Redis database");
        let upload_key = format!("{}v2board_upload_traffic", source.redis_connection_prefix);
        let download_key = format!("{}v2board_download_traffic", source.redis_connection_prefix);
        redis::cmd("HSET")
            .arg(&upload_key)
            .arg("7")
            .arg("11")
            .arg("9")
            .arg("0")
            .query_async::<u64>(&mut connection)
            .await
            .expect("seed upload traffic");
        redis::cmd("HSET")
            .arg(&download_key)
            .arg("9")
            .arg("17")
            .query_async::<u64>(&mut connection)
            .await
            .expect("seed download traffic");

        let first = freeze_and_digest_traffic(&mut connection, &spec, source)
            .await
            .expect("first atomic freeze");
        let second = freeze_and_digest_traffic(&mut connection, &spec, source)
            .await
            .expect("retry observes the same frozen hashes");
        assert_eq!(first, second);
        assert_eq!(first.upload_fields, 2);
        assert_eq!(first.download_fields, 1);
        assert_eq!(first.upload_sum, 11);
        assert_eq!(first.download_sum, 17);
        assert_eq!(
            first.deltas,
            vec![
                FrozenTrafficDeltaRecord(7, Some(11), None),
                FrozenTrafficDeltaRecord(9, Some(0), Some(17)),
            ]
        );
        for key in [upload_key, download_key] {
            let kind = redis::cmd("TYPE")
                .arg(key)
                .query_async::<String>(&mut connection)
                .await
                .expect("live traffic key type");
            assert_eq!(kind, "none");
        }
        redis::cmd("CLIENT")
            .arg("PAUSE")
            .arg(5_000_u64)
            .arg("WRITE")
            .query_async::<()>(&mut connection)
            .await
            .expect("pause disposable Redis writes");
        let mut blocked = redis_connection(&source.redis_default_url)
            .await
            .expect("second disposable Redis connection");
        assert!(
            timeout(
                Duration::from_millis(100),
                redis::cmd("SET")
                    .arg("must-remain-blocked")
                    .arg("1")
                    .query_async::<()>(&mut blocked),
            )
            .await
            .is_err(),
            "CLIENT PAUSE WRITE must hold a competing writer"
        );
        redis::cmd("CLIENT")
            .arg("UNPAUSE")
            .query_async::<()>(&mut connection)
            .await
            .expect("unpause disposable Redis");
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut connection)
            .await
            .expect("clean disposable Redis database");
    }
}
