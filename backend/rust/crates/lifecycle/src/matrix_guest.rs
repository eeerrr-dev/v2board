//! Linux-only guest for destructive, disposable-machine fault evidence.
//!
//! This module is reachable only from the `bare-metal-fault-matrix`-gated
//! binary. It deliberately has fixed input/output locations: accepting a
//! production manifest path or a caller-selected marker would weaken the
//! physical safety boundary of the harness.

use std::{
    collections::BTreeSet,
    env, fs,
    fs::{File, Metadata, OpenOptions},
    io::{Read, Seek},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::{Host, Url};
use v2board_provision::bare_metal_fault_matrix::{
    BareMetalFaultCase, BareMetalFaultControllerConfig, BareMetalFaultMode, BareMetalFaultPoint,
    FAULT_MODES, FAULT_POINTS, fault_case_binding_sha256, fault_catalog_sha256,
    install_bare_metal_fault_case,
};

pub(crate) const MATRIX_ROOT: &str = "/var/lib/v2board/fault-matrix";
pub(crate) const MANIFEST_PATH: &str = "/var/lib/v2board/fault-matrix/manifest.json";
pub(crate) const MARKER_PATH: &str = "/etc/v2board/bare-metal-fault-matrix-disposable.json";
pub(crate) const ADMISSION_EVIDENCE_PATH: &str =
    "/var/lib/v2board/fault-matrix/admission-evidence.json";
pub(crate) const OUTCOME_EVIDENCE_PATH: &str =
    "/var/lib/v2board/fault-matrix/outcome-evidence.json";
pub(crate) const CONTROL_DIR: &str = "/var/lib/v2board/fault-matrix/control";
pub(crate) const GUEST_UNIT: &str = "v2board-fault-matrix-guest.service";
const MARKER_FORMAT: &str = "v2board-bare-metal-fault-matrix-disposable-v1";
const ADMISSION_FORMAT: &str = "v2board-bare-metal-fault-matrix-admission-v1";
const TEST_DATABASE_PREFIX: &str = "v2board_matrix_";
const MAX_MARKER_BYTES: u64 = 64 * 1024;
const MAX_RELEASE_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_RELEASE_METADATA_BYTES: u64 = 4096;
const MAX_VERSION_BYTES: usize = 4096;
const O_NOFOLLOW: i32 = 0o400000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GuestAction {
    ListCases,
    Start,
    Resume,
}

#[derive(Debug, Eq, PartialEq)]
struct GuestCommand {
    action: GuestAction,
    fault_case: Option<String>,
    run_id: Option<String>,
    operation_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DisposableMarker {
    protocol: String,
    disposable: bool,
    expires_at_unix: i64,
    guest_id: String,
    snapshot_id: String,
    interruption_mechanism: String,
    machine_id_sha256: String,
    guest_binary_sha256: String,
    manifest_sha256: String,
    operation_id: String,
    run_id: String,
    source_revision: String,
    release_sha256: String,
    fault_point_set_sha256: String,
    case_id: String,
    fault_point: BareMetalFaultPoint,
    mode: BareMetalFaultMode,
    case_binding_sha256: String,
    unit_allowlist: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ReleaseEvidence {
    release_id: String,
    archive_sha256: String,
    source_revision: String,
    target_os: String,
    target_arch: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct MachineEvidence {
    machine_id_sha256: String,
    os_release_sha256: String,
    kernel_release: String,
    systemd_version: String,
    mysql_version: String,
    postgres_version: String,
    redis_version: String,
    clickhouse_version: String,
}

#[derive(Debug, Serialize)]
struct AdmissionEvidence<'a> {
    format: &'static str,
    status: &'static str,
    operation_id: &'a str,
    run_id: &'a str,
    fault_case: &'a str,
    fault_point: BareMetalFaultPoint,
    mode: BareMetalFaultMode,
    case_binding_sha256: &'a str,
    fault_catalog_sha256: &'a str,
    guest_id: &'a str,
    snapshot_id: &'a str,
    interruption_mechanism: &'a str,
    marker_sha256: &'a str,
    manifest_sha256: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    authorization_file_sha256: &'a str,
    inspect_review_sha256: &'a str,
    authorized_snapshot_report_sha256: &'a str,
    guest_binary_sha256: &'a str,
    source_revision: &'a str,
    release: &'a ReleaseEvidence,
    machine: &'a MachineEvidence,
    systemd_unit_allowlist: &'a [String],
    secrets_redacted: bool,
}

#[derive(Debug)]
struct SecureFile {
    bytes: Vec<u8>,
    sha256: String,
}

#[derive(Debug)]
struct SecureStreamFile {
    file: File,
    sha256: String,
}

struct UnitPolicy<'a> {
    api: &'a str,
    worker: &'a str,
    legacy_writers: &'a [String],
    legacy_workers: &'a [String],
    legacy_schedulers: &'a [String],
    mysql: &'a str,
    default_redis: &'a str,
    cache_redis: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SecureMetadata {
    uid: u32,
    gid: u32,
    mode: u32,
    nlink: u64,
    is_file: bool,
}

impl From<&Metadata> for SecureMetadata {
    fn from(metadata: &Metadata) -> Self {
        Self {
            uid: metadata.uid(),
            gid: metadata.gid(),
            mode: metadata.permissions().mode() & 0o7777,
            nlink: metadata.nlink(),
            is_file: metadata.file_type().is_file(),
        }
    }
}

pub(crate) async fn main() -> anyhow::Result<()> {
    let command = parse_args(env::args().skip(1))?;
    match command.action {
        GuestAction::ListCases => print_case_catalog(
            command.run_id.as_deref().expect("list requires run id"),
            command
                .operation_id
                .as_deref()
                .expect("list requires operation id"),
        ),
        GuestAction::Start | GuestAction::Resume => {
            let fault_case = command
                .fault_case
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("fault case is required"))?;
            run_guest(command.action, fault_case).await
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<GuestCommand> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [action, run_flag, run_id, operation_flag, operation_id]
            if action == "list-cases"
                && run_flag == "--run-id"
                && operation_flag == "--operation-id" =>
        {
            Ok(GuestCommand {
                action: GuestAction::ListCases,
                fault_case: None,
                run_id: Some(run_id.clone()),
                operation_id: Some(operation_id.clone()),
            })
        }
        [action, case_flag, fault_case]
            if matches!(action.as_str(), "start" | "resume") && case_flag == "--case" =>
        {
            if fault_case.is_empty()
                || fault_case.len() > 128
                || !fault_case.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
            {
                anyhow::bail!("--case must use the closed lowercase <point>--<mode> grammar");
            }
            Ok(GuestCommand {
                action: if action == "start" {
                    GuestAction::Start
                } else {
                    GuestAction::Resume
                },
                fault_case: Some(fault_case.clone()),
                run_id: None,
                operation_id: None,
            })
        }
        _ => anyhow::bail!(
            "invalid command; use `list-cases --run-id <uuid> --operation-id <uuid>`, `start --case <point--mode>`, or `resume --case <point--mode>`"
        ),
    }
}

fn print_case_catalog(run_id: &str, operation_id: &str) -> anyhow::Result<()> {
    let run_id = canonical_uuid(run_id, "run_id")?;
    let operation_id = canonical_uuid(operation_id, "operation_id")?;
    let cases = FAULT_POINTS
        .iter()
        .flat_map(|point| {
            FAULT_MODES.iter().map(move |mode| BareMetalFaultCase {
                point: *point,
                mode: *mode,
            })
        })
        .map(|case| {
            Ok(serde_json::json!({
                "case_id": case.id(),
                "fault_point": case.point,
                "mode": case.mode,
                "case_binding_sha256": fault_case_binding_sha256(&operation_id, &run_id, case)?,
            }))
        })
        .collect::<Result<Vec<_>, v2board_provision::bare_metal_fault_matrix::FaultMatrixError>>(
        )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "protocol": "v2board-bare-metal-fault-catalog-v1",
            "run_id": run_id,
            "operation_id": operation_id,
            "production_capability_available": v2board_provision::PRODUCTION_LEGACY_APPLY_CAPABILITY.is_available(),
            "fault_point_set_sha256": fault_catalog_sha256(),
            "cases": cases,
        }))?
    );
    Ok(())
}

async fn run_guest(action: GuestAction, fault_case_id: &str) -> anyhow::Result<()> {
    require_linux_root()?;
    require_guest_systemd_unit(action)?;
    require_exact_directory(Path::new(MATRIX_ROOT), 0o700)?;
    require_exact_directory(Path::new(CONTROL_DIR), 0o700)?;

    let marker_file = open_secure_file(Path::new(MARKER_PATH), MAX_MARKER_BYTES, &[0o600])?;
    let marker = serde_json::from_slice::<DisposableMarker>(&marker_file.bytes)
        .map_err(|error| anyhow::anyhow!("disposable marker is not strict JSON: {error}"))?;
    validate_marker_shape(&marker, unix_now()?, fault_case_id)?;
    let fault_case = parse_fault_case(fault_case_id)?;
    if marker.fault_point != fault_case.point || marker.mode != fault_case.mode {
        anyhow::bail!("disposable marker fault point/mode do not match the requested case");
    }
    if marker.fault_point_set_sha256 != fault_catalog_sha256()
        || marker.case_binding_sha256
            != fault_case_binding_sha256(&marker.operation_id, &marker.run_id, fault_case)?
    {
        anyhow::bail!("disposable marker does not bind the linked typed fault catalog and case");
    }

    let machine_id_sha256 = read_machine_id_sha256()?;
    if marker.machine_id_sha256 != machine_id_sha256 {
        anyhow::bail!("disposable marker belongs to a different machine");
    }

    let guest_binary = env::current_exe()?;
    let guest_binary_file = open_secure_file(
        &guest_binary,
        512 * 1024 * 1024,
        &[0o500, 0o555, 0o700, 0o755],
    )?;
    if marker.guest_binary_sha256 != guest_binary_file.sha256 {
        anyhow::bail!("disposable marker does not bind this guest binary");
    }

    let manifest_file = open_secure_file(Path::new(MANIFEST_PATH), 1024 * 1024, &[0o400, 0o600])?;
    if marker.manifest_sha256 != manifest_file.sha256 {
        anyhow::bail!("disposable marker does not bind the dedicated manifest");
    }
    let raw_manifest: Value = serde_json::from_slice(&manifest_file.bytes)?;
    validate_disposable_manifest(&raw_manifest, &marker)?;
    let spec = v2board_provision::load_provision_spec(MANIFEST_PATH)?;
    if spec.operation_id != marker.operation_id {
        anyhow::bail!("hydrated manifest operation does not match disposable marker");
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or_else(|| anyhow::anyhow!("matrix requires a schema-v4 legacy execution"))?;
    validate_unit_allowlist(
        UnitPolicy {
            api: &execution.systemd.api_unit,
            worker: &execution.systemd.worker_unit,
            legacy_writers: &execution.systemd.legacy_writer_units,
            legacy_workers: &execution.systemd.legacy_worker_units,
            legacy_schedulers: &execution.systemd.legacy_scheduler_units,
            mysql: &execution.source_control.datastores.mysql.unit,
            default_redis: &execution.source_control.datastores.default_redis.unit,
            cache_redis: &execution.source_control.datastores.cache_redis.unit,
        },
        &marker.unit_allowlist,
    )?;

    let archive = open_secure_streaming_file(
        &execution.release.archive_path,
        MAX_RELEASE_ARCHIVE_BYTES,
        &[0o400],
    )?;
    if archive.sha256 != execution.release.archive_sha256 || archive.sha256 != marker.release_sha256
    {
        anyhow::bail!("release archive digest is not bound by both manifest and marker");
    }
    let release =
        read_release_evidence(archive.file, &execution.release.release_id, &archive.sha256)?;
    if release.source_revision != marker.source_revision {
        anyhow::bail!("release source revision does not match disposable marker");
    }

    let _controller = install_bare_metal_fault_case(BareMetalFaultControllerConfig {
        operation_id: spec.operation_id.clone(),
        run_id: marker.run_id.clone(),
        control_dir: PathBuf::from(CONTROL_DIR),
        case: fault_case,
    })?;

    let authorization_path = execution.journal.authorization_path.clone();
    let (authorization, authorization_file_sha256) = match action {
        GuestAction::Start => {
            let inspection = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::Online,
            )
            .await?;
            let authorization =
                v2board_provision::ApplyAuthorization::issue_bare_metal_fault_matrix(
                    &spec,
                    &inspection,
                    &spec.operation_id,
                    unix_now()?,
                )?;
            authorization.write_new(&authorization_path)?;
            v2board_provision::ApplyAuthorization::load_with_file_sha256(&authorization_path)?
        }
        GuestAction::Resume => {
            let loaded =
                v2board_provision::ApplyAuthorization::load_with_file_sha256(&authorization_path)?;
            loaded.0.verify_resume_binding(&spec)?;
            loaded
        }
        GuestAction::ListCases => unreachable!("catalog handled before admission"),
    };

    let machine = machine_evidence(machine_id_sha256)?;
    let evidence = AdmissionEvidence {
        format: ADMISSION_FORMAT,
        status: "admitted",
        operation_id: &spec.operation_id,
        run_id: &marker.run_id,
        fault_case: &marker.case_id,
        fault_point: marker.fault_point,
        mode: marker.mode,
        case_binding_sha256: &marker.case_binding_sha256,
        fault_catalog_sha256: fault_catalog_sha256(),
        guest_id: &marker.guest_id,
        snapshot_id: &marker.snapshot_id,
        interruption_mechanism: &marker.interruption_mechanism,
        marker_sha256: &marker_file.sha256,
        manifest_sha256: &manifest_file.sha256,
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256(),
        authorization_file_sha256: &authorization_file_sha256,
        inspect_review_sha256: &authorization.inspect_review_sha256,
        authorized_snapshot_report_sha256: &authorization.authorized_snapshot_report_sha256,
        guest_binary_sha256: &guest_binary_file.sha256,
        source_revision: &marker.source_revision,
        release: &release,
        machine: &machine,
        systemd_unit_allowlist: &marker.unit_allowlist,
        secrets_redacted: true,
    };
    let evidence_bytes = canonical_pretty_json(&evidence)?;
    persist_or_verify_admission(action, &evidence_bytes)?;

    let result = match action {
        GuestAction::Start => {
            v2board_provision::production_legacy_apply::start_bare_metal_fault_matrix_legacy_apply(
                &spec,
                &authorization,
                &authorization_file_sha256,
                unix_now()?,
            )
            .await?
        }
        GuestAction::Resume => {
            v2board_provision::production_legacy_apply::resume_bare_metal_fault_matrix_legacy_apply(
                &spec,
                &authorization,
                &authorization_file_sha256,
            )
            .await?
        }
        GuestAction::ListCases => unreachable!(),
    };
    if !result.completed || !result.mysql_runtime_retired {
        anyhow::bail!("matrix execution returned without terminal retirement");
    }
    let ready = open_secure_file(
        &Path::new(CONTROL_DIR).join(v2board_provision::bare_metal_fault_matrix::FAULT_READY_FILE),
        64 * 1024,
        &[0o600],
    )?;
    let ready_record = serde_json::from_slice::<
        v2board_provision::bare_metal_fault_matrix::FaultReadyRecord,
    >(&ready.bytes)?;
    if ready_record.operation_id != spec.operation_id
        || ready_record.run_id != marker.run_id
        || ready_record.point != marker.fault_point
        || ready_record.mode != marker.mode
        || ready_record.catalog_sha256 != marker.fault_point_set_sha256
        || ready_record.case_binding_sha256 != marker.case_binding_sha256
    {
        anyhow::bail!("fault-ready evidence does not match the admitted case binding");
    }
    let outcome = serde_json::json!({
        "protocol": "v2board-bare-metal-fault-matrix-guest-outcome-v1",
        "status": "passed",
        "operation_id": spec.operation_id,
        "run_id": marker.run_id,
        "case_id": marker.case_id,
        "fault_point": marker.fault_point,
        "mode": marker.mode,
        "interruption_mechanism": marker.interruption_mechanism,
        "guest_id": marker.guest_id,
        "snapshot_id": marker.snapshot_id,
        "case_binding_sha256": marker.case_binding_sha256,
        "fault_point_set_sha256": marker.fault_point_set_sha256,
        "source_revision": marker.source_revision,
        "manifest_sha256": manifest_file.sha256,
        "release_sha256": release.archive_sha256,
        "admission_evidence_sha256": sha256(&evidence_bytes),
        "ready_sha256": ready.sha256,
        "result": result,
        "secrets_redacted": true
    });
    write_new_durable(
        Path::new(OUTCOME_EVIDENCE_PATH),
        &canonical_pretty_json(&outcome)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    Ok(())
}

fn validate_marker_shape(
    marker: &DisposableMarker,
    now_unix: i64,
    requested_case: &str,
) -> anyhow::Result<()> {
    if marker.protocol != MARKER_FORMAT || !marker.disposable {
        anyhow::bail!("marker is not an explicit disposable-machine authorization");
    }
    if marker.expires_at_unix <= now_unix || marker.expires_at_unix > now_unix + 24 * 60 * 60 {
        anyhow::bail!("disposable marker is expired or valid for longer than 24 hours");
    }
    for (name, digest) in [
        ("machine_id_sha256", &marker.machine_id_sha256),
        ("guest_binary_sha256", &marker.guest_binary_sha256),
        ("manifest_sha256", &marker.manifest_sha256),
        ("release_sha256", &marker.release_sha256),
        ("fault_point_set_sha256", &marker.fault_point_set_sha256),
        ("case_binding_sha256", &marker.case_binding_sha256),
    ] {
        if !is_lower_hex(digest, 64) {
            anyhow::bail!("marker {name} must be a lowercase SHA-256");
        }
    }
    if marker.source_revision.len() != 40 || !is_lower_hex(&marker.source_revision, 40) {
        anyhow::bail!("marker source_revision must be a real lowercase git SHA");
    }
    if canonical_uuid(&marker.operation_id, "operation_id").is_err()
        || canonical_uuid(&marker.run_id, "run_id").is_err()
        || marker.case_id != requested_case
    {
        anyhow::bail!("marker does not bind the requested operation and fault case");
    }
    if !safe_identifier(&marker.guest_id) || !safe_identifier(&marker.snapshot_id) {
        anyhow::bail!("marker guest_id and snapshot_id must be safe nonempty identifiers");
    }
    let mechanism_valid = matches!(
        (marker.mode, marker.interruption_mechanism.as_str()),
        (
            BareMetalFaultMode::Before | BareMetalFaultMode::LostAcknowledgement,
            "process_error"
        ) | (BareMetalFaultMode::SigkillReady, "sigkill" | "hard_reset")
    );
    if !mechanism_valid {
        anyhow::bail!("marker interruption mechanism does not match the typed fault mode");
    }
    if marker.unit_allowlist.is_empty() {
        anyhow::bail!("marker systemd unit allowlist cannot be empty");
    }
    let unique = marker.unit_allowlist.iter().collect::<BTreeSet<_>>();
    if unique.len() != marker.unit_allowlist.len() {
        anyhow::bail!("marker systemd unit allowlist contains duplicates");
    }
    Ok(())
}

fn validate_disposable_manifest(root: &Value, marker: &DisposableMarker) -> anyhow::Result<()> {
    let object = root
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("matrix manifest must be an object"))?;
    if object.get("schema_version").and_then(Value::as_u64) != Some(4)
        || object.get("kind").and_then(Value::as_str) != Some("legacy_reference_migration")
        || object.get("operation_id").and_then(Value::as_str) != Some(marker.operation_id.as_str())
    {
        anyhow::bail!("matrix manifest must be the marker-bound schema-v4 legacy operation");
    }

    let source = object_at(object, "source")?;
    for key in [
        "database_url",
        "database_fence_url",
        "redis_default_url",
        "redis_cache_url",
    ] {
        require_loopback_url(string_at(source, key)?, key)?;
    }
    require_test_database_url(string_at(source, "database_url")?, "source.database_url")?;

    let target = object_at(object, "target")?;
    let postgres = object_at(target, "postgres")?;
    require_loopback_url(
        string_at(postgres, "bootstrap_database_url")?,
        "target.postgres.bootstrap_database_url",
    )?;
    for key in [
        "migration_database_url",
        "api_database_url",
        "worker_database_url",
    ] {
        let value = string_at(postgres, key)?;
        require_loopback_url(value, key)?;
        require_test_database_url(value, key)?;
    }
    let migration_database = url_database_name(string_at(postgres, "migration_database_url")?)?;
    for key in ["api_database_url", "worker_database_url"] {
        if url_database_name(string_at(postgres, key)?)? != migration_database {
            anyhow::bail!("all PostgreSQL runtime roles must target one matrix database");
        }
    }

    require_loopback_url(string_at(target, "redis_url")?, "target.redis_url")?;
    let clickhouse = object_at(target, "clickhouse")?;
    require_loopback_url(
        string_at(clickhouse, "endpoint")?,
        "target.clickhouse.endpoint",
    )?;
    let clickhouse_database = string_at(clickhouse, "database")?;
    if !valid_test_database_name(clickhouse_database) {
        anyhow::bail!("ClickHouse database must use the reserved matrix prefix");
    }

    let execution = object_at(object, "execution")?;
    let backup = object_at(execution, "backup")?;
    let restore_url = string_at(backup, "isolated_restore_database_url")?;
    require_loopback_url(
        restore_url,
        "execution.backup.isolated_restore_database_url",
    )?;
    require_test_database_url(
        restore_url,
        "execution.backup.isolated_restore_database_url",
    )?;
    Ok(())
}

fn validate_unit_allowlist(policy: UnitPolicy<'_>, declared: &[String]) -> anyhow::Result<()> {
    if policy.api != "v2board-api.service" || policy.worker != "v2board-worker.service" {
        anyhow::bail!("native matrix units diverge from the frozen release contract");
    }
    let mut expected = BTreeSet::from([
        policy.api.to_string(),
        policy.worker.to_string(),
        policy.mysql.to_string(),
        policy.default_redis.to_string(),
        policy.cache_redis.to_string(),
    ]);
    for unit in policy
        .legacy_writers
        .iter()
        .chain(policy.legacy_workers)
        .chain(policy.legacy_schedulers)
    {
        expected.insert(unit.clone());
    }
    for unit in expected.iter().filter(|unit| {
        unit.as_str() != "v2board-api.service" && unit.as_str() != "v2board-worker.service"
    }) {
        if !unit.starts_with("v2board-matrix-")
            || !(unit.ends_with(".service") || unit.ends_with(".timer"))
        {
            anyhow::bail!(
                "every legacy/source unit must use the throwaway v2board-matrix-* namespace"
            );
        }
    }
    let declared = declared.iter().cloned().collect::<BTreeSet<_>>();
    if declared != expected {
        anyhow::bail!(
            "marker unit allowlist must exactly equal every unit reachable from the manifest"
        );
    }
    Ok(())
}

fn read_release_evidence(
    mut archive: File,
    release_id: &str,
    archive_sha256: &str,
) -> anyhow::Result<ReleaseEvidence> {
    archive.rewind()?;
    let decoder = GzDecoder::new(archive);
    let mut tar = tar::Archive::new(decoder);
    let mut release_bytes = None;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let path = normalize_archive_path(&path)?;
        if path == Path::new("RELEASE") {
            if release_bytes.is_some() || !entry.header().entry_type().is_file() {
                anyhow::bail!("release archive must contain one regular RELEASE file");
            }
            if entry.size() == 0 || entry.size() > MAX_RELEASE_METADATA_BYTES {
                anyhow::bail!("release metadata has an unsafe size");
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            release_bytes = Some(bytes);
        }
    }
    let bytes = release_bytes.ok_or_else(|| anyhow::anyhow!("release metadata is missing"))?;
    parse_release_metadata(&bytes, release_id, archive_sha256)
}

fn parse_release_metadata(
    bytes: &[u8],
    release_id: &str,
    archive_sha256: &str,
) -> anyhow::Result<ReleaseEvidence> {
    let text = std::str::from_utf8(bytes)?;
    let mut fields = std::collections::BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("release metadata line is malformed"))?;
        if !matches!(
            key,
            "format" | "source_revision" | "target_os" | "target_arch"
        ) || value.is_empty()
            || fields.insert(key, value).is_some()
        {
            anyhow::bail!("release metadata contains an unknown, empty, or duplicate field");
        }
    }
    if fields.len() != 4
        || fields.get("format") != Some(&"v2board-native-release-v1")
        || fields.get("target_os") != Some(&"linux")
        || fields.get("target_arch") != Some(&"amd64")
    {
        anyhow::bail!("release metadata is not the frozen Linux amd64 contract");
    }
    let source_revision = fields["source_revision"];
    if source_revision.len() != 40 || !is_lower_hex(source_revision, 40) {
        anyhow::bail!("release metadata source revision is not a real git SHA");
    }
    Ok(ReleaseEvidence {
        release_id: release_id.to_string(),
        archive_sha256: archive_sha256.to_string(),
        source_revision: source_revision.to_string(),
        target_os: "linux".to_string(),
        target_arch: "amd64".to_string(),
    })
}

fn normalize_archive_path(path: &Path) -> anyhow::Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("release archive contains an unsafe path");
            }
        }
    }
    Ok(normalized)
}

fn require_loopback_url(value: &str, field: &str) -> anyhow::Result<()> {
    let url = Url::parse(value).map_err(|_| anyhow::anyhow!("{field} is not a URL"))?;
    let loopback = match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(address)) => address
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        None => false,
    };
    if !loopback {
        anyhow::bail!("{field} must use a literal loopback address");
    }
    Ok(())
}

fn require_test_database_url(value: &str, field: &str) -> anyhow::Result<()> {
    let database = url_database_name(value)?;
    if !valid_test_database_name(&database) {
        anyhow::bail!("{field} database must use the reserved matrix prefix");
    }
    Ok(())
}

fn url_database_name(value: &str) -> anyhow::Result<String> {
    let url = Url::parse(value)?;
    let database = url.path().trim_start_matches('/');
    if database.is_empty() || database.contains('/') {
        anyhow::bail!("database URL must contain exactly one database name");
    }
    Ok(database.to_string())
}

fn valid_test_database_name(value: &str) -> bool {
    value.starts_with(TEST_DATABASE_PREFIX)
        && value.len() > TEST_DATABASE_PREFIX.len()
        && value.len() <= 63
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn object_at<'a>(
    parent: &'a serde_json::Map<String, Value>,
    key: &str,
) -> anyhow::Result<&'a serde_json::Map<String, Value>> {
    parent
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("manifest field {key} must be an object"))
}

fn string_at<'a>(parent: &'a serde_json::Map<String, Value>, key: &str) -> anyhow::Result<&'a str> {
    parent
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("manifest field {key} must be a string"))
}

fn require_linux_root() -> anyhow::Result<()> {
    if env::consts::OS != "linux" {
        anyhow::bail!("bare-metal fault matrix runs only on Linux");
    }
    let status = fs::read_to_string("/proc/self/status")?;
    let uid_line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| anyhow::anyhow!("cannot read process UIDs"))?;
    let uids = uid_line
        .split_whitespace()
        .skip(1)
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;
    if uids.len() != 4 || uids.iter().any(|uid| *uid != 0) {
        anyhow::bail!(
            "bare-metal fault matrix requires all real/effective/saved/fs UIDs to be root"
        );
    }
    Ok(())
}

fn require_guest_systemd_unit(action: GuestAction) -> anyhow::Result<()> {
    let cgroup = fs::read_to_string("/proc/self/cgroup")?;
    let unit_matches = cgroup.lines().any(|line| {
        line.rsplit_once(':')
            .and_then(|(_, path)| path.rsplit('/').next())
            == Some(GUEST_UNIT)
    });
    if !unit_matches || matches!(action, GuestAction::ListCases) {
        anyhow::bail!("guest must run in the dedicated throwaway systemd unit");
    }
    Ok(())
}

fn require_exact_directory(path: &Path, mode: u32) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != 0
        || metadata.gid() != 0
        || metadata.permissions().mode() & 0o7777 != mode
    {
        anyhow::bail!(
            "{} must be a root:root directory with mode {mode:o}",
            path.display()
        );
    }
    Ok(())
}

fn open_secure_file(
    path: &Path,
    max_bytes: u64,
    allowed_modes: &[u32],
) -> anyhow::Result<SecureFile> {
    open_secure_file_with_nlinks(path, max_bytes, allowed_modes, &[1])
}

fn open_secure_file_with_nlinks(
    path: &Path,
    max_bytes: u64,
    allowed_modes: &[u32],
    allowed_nlinks: &[u64],
) -> anyhow::Result<SecureFile> {
    require_secure_parent(path)?;
    let before = fs::symlink_metadata(path)?;
    validate_secure_metadata_with_nlinks(
        SecureMetadata::from(&before),
        allowed_modes,
        allowed_nlinks,
    )?;
    if before.len() == 0 || before.len() > max_bytes {
        anyhow::bail!("{} has an unsafe size", path.display());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)?;
    let after = file.metadata()?;
    if before.dev() != after.dev() || before.ino() != after.ino() {
        anyhow::bail!("{} changed while it was opened", path.display());
    }
    validate_secure_metadata_with_nlinks(
        SecureMetadata::from(&after),
        allowed_modes,
        allowed_nlinks,
    )?;
    let mut bytes = Vec::with_capacity(after.len() as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.len() as u64 != after.len() {
        anyhow::bail!("{} changed while it was read", path.display());
    }
    let sha256 = sha256(&bytes);
    file.rewind()?;
    Ok(SecureFile { bytes, sha256 })
}

fn open_secure_streaming_file(
    path: &Path,
    max_bytes: u64,
    allowed_modes: &[u32],
) -> anyhow::Result<SecureStreamFile> {
    require_secure_parent(path)?;
    let before = fs::symlink_metadata(path)?;
    validate_secure_metadata(SecureMetadata::from(&before), allowed_modes)?;
    if before.len() == 0 || before.len() > max_bytes {
        anyhow::bail!("{} has an unsafe size", path.display());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)?;
    let after = file.metadata()?;
    if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len() {
        anyhow::bail!("{} changed while it was opened", path.display());
    }
    validate_secure_metadata(SecureMetadata::from(&after), allowed_modes)?;
    let mut digest = Sha256::new();
    let mut read_bytes = 0_u64;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        read_bytes = read_bytes
            .checked_add(count as u64)
            .ok_or_else(|| anyhow::anyhow!("streamed file size overflow"))?;
        digest.update(&chunk[..count]);
    }
    if read_bytes != after.len() {
        anyhow::bail!("{} changed while it was hashed", path.display());
    }
    file.rewind()?;
    Ok(SecureStreamFile {
        file,
        sha256: hex_string(&digest.finalize()),
    })
}

fn require_secure_parent(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("security-sensitive file has no parent"))?;
    if !parent.is_absolute() {
        anyhow::bail!("security-sensitive file parent must be absolute");
    }
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)?;
        let mode = metadata.permissions().mode() & 0o7777;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != 0
            || metadata.gid() != 0
            || mode & 0o022 != 0
        {
            anyhow::bail!(
                "every security-sensitive file ancestor must be a root-owned non-writable directory"
            );
        }
    }
    Ok(())
}

fn canonical_uuid(value: &str, field: &str) -> anyhow::Result<String> {
    let parsed = uuid::Uuid::parse_str(value)
        .map_err(|_| anyhow::anyhow!("{field} must be a canonical UUID"))?;
    if parsed.is_nil() || parsed.hyphenated().to_string() != value {
        anyhow::bail!("{field} must be a canonical lowercase non-nil UUID");
    }
    Ok(value.to_string())
}

fn parse_fault_case(value: &str) -> anyhow::Result<BareMetalFaultCase> {
    let (point, mode) = value
        .split_once("--")
        .ok_or_else(|| anyhow::anyhow!("fault case must use the typed <point>--<mode> id"))?;
    if mode.contains("--") {
        anyhow::bail!("fault case contains more than one separator");
    }
    let case = BareMetalFaultCase {
        point: point.parse()?,
        mode: mode.parse()?,
    };
    if case.id() != value {
        anyhow::bail!("fault case is not canonical");
    }
    Ok(case)
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn canonical_pretty_json(value: &impl Serialize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn persist_or_verify_admission(action: GuestAction, expected: &[u8]) -> anyhow::Result<()> {
    let path = Path::new(ADMISSION_EVIDENCE_PATH);
    match action {
        GuestAction::Start => write_new_durable(path, expected),
        GuestAction::Resume => write_new_durable(path, expected),
        GuestAction::ListCases => unreachable!(),
    }
}

fn write_new_durable(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    require_secure_parent(path)?;
    if fs::symlink_metadata(path).is_ok() {
        return recover_published_evidence(path, bytes);
    }
    let parent = path.parent().expect("validated parent");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("evidence output has no safe filename"))?;
    let temporary = parent.join(format!(".{file_name}.{}.writing", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(O_NOFOLLOW)
        .open(&temporary)?;
    use std::io::Write;
    let result = (|| -> anyhow::Result<bool> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if let Err(error) = fs::hard_link(&temporary, path) {
            if fs::symlink_metadata(path).is_ok() {
                return Ok(false);
            }
            return Err(error.into());
        }
        File::open(parent)?.sync_all()?;
        // Publication plus the first directory fsync is the commit point.
        // A failed orphan cleanup must not turn durable evidence into an
        // apparent failure; the next invocation reconciles nlink=2.
        if fs::remove_file(&temporary).is_ok() {
            let _ = File::open(parent).and_then(|directory| directory.sync_all());
        }
        Ok(true)
    })();
    match result {
        Ok(true) => Ok(()),
        Ok(false) => {
            let _ = fs::remove_file(&temporary);
            recover_published_evidence(path, bytes)
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn recover_published_evidence(path: &Path, expected: &[u8]) -> anyhow::Result<()> {
    require_secure_parent(path)?;
    let before = fs::symlink_metadata(path)?;
    let metadata = SecureMetadata::from(&before);
    if !metadata.is_file
        || metadata.uid != 0
        || metadata.gid != 0
        || metadata.mode != 0o600
        || !matches!(metadata.nlink, 1 | 2)
    {
        anyhow::bail!("published evidence has unsafe ownership, mode, type, or link count");
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)?;
    let opened = file.metadata()?;
    if before.dev() != opened.dev() || before.ino() != opened.ino() {
        anyhow::bail!("published evidence changed while it was opened");
    }
    let mut actual = Vec::new();
    file.read_to_end(&mut actual)?;
    if actual != expected {
        anyhow::bail!("published evidence conflicts with the expected canonical bytes");
    }
    if metadata.nlink == 1 {
        return Ok(());
    }

    let parent = path.parent().expect("validated parent");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("evidence output has no safe filename"))?;
    let prefix = format!(".{file_name}.");
    let mut orphan = None;
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".writing") {
            continue;
        }
        let candidate = fs::symlink_metadata(entry.path())?;
        if candidate.dev() == before.dev()
            && candidate.ino() == before.ino()
            && orphan.replace(entry.path()).is_some()
        {
            anyhow::bail!("published evidence has multiple conflicting temporary links");
        }
    }
    let orphan = orphan.ok_or_else(|| {
        anyhow::anyhow!("published evidence nlink=2 has no exact recoverable temporary link")
    })?;
    fs::remove_file(orphan)?;
    File::open(parent)?.sync_all()?;
    if fs::symlink_metadata(path)?.nlink() != 1 {
        anyhow::bail!("published evidence link reconciliation did not converge");
    }
    Ok(())
}

fn validate_secure_metadata(metadata: SecureMetadata, allowed_modes: &[u32]) -> anyhow::Result<()> {
    validate_secure_metadata_with_nlinks(metadata, allowed_modes, &[1])
}

fn validate_secure_metadata_with_nlinks(
    metadata: SecureMetadata,
    allowed_modes: &[u32],
    allowed_nlinks: &[u64],
) -> anyhow::Result<()> {
    if !metadata.is_file
        || metadata.uid != 0
        || metadata.gid != 0
        || !allowed_nlinks.contains(&metadata.nlink)
        || !allowed_modes.contains(&metadata.mode)
    {
        anyhow::bail!(
            "security-sensitive input must be root:root, regular, expected-link-count, and exact-mode"
        );
    }
    Ok(())
}

fn read_machine_id_sha256() -> anyhow::Result<String> {
    let bytes = fs::read("/etc/machine-id")?;
    let text = std::str::from_utf8(&bytes)?;
    let value = text.trim();
    if value.len() != 32 || !is_lower_hex(value, 32) {
        anyhow::bail!("machine-id is not a canonical 32-byte lowercase identifier");
    }
    Ok(sha256(value.as_bytes()))
}

fn machine_evidence(machine_id_sha256: String) -> anyhow::Result<MachineEvidence> {
    let os_release_sha256 = sha256(&fs::read("/etc/os-release")?);
    Ok(MachineEvidence {
        machine_id_sha256,
        os_release_sha256,
        kernel_release: fixed_version_command("/usr/bin/uname", &["-r"])?,
        systemd_version: fixed_version_command("/usr/bin/systemctl", &["--version"])?,
        mysql_version: first_available_version(&[
            ("/usr/bin/mysqld", &["--version"]),
            ("/usr/sbin/mysqld", &["--version"]),
        ])?,
        postgres_version: fixed_version_command("/usr/bin/psql", &["--version"])?,
        redis_version: fixed_version_command("/usr/bin/redis-server", &["--version"])?,
        clickhouse_version: fixed_version_command("/usr/bin/clickhouse-client", &["--version"])?,
    })
}

fn first_available_version(candidates: &[(&str, &[&str])]) -> anyhow::Result<String> {
    for (program, args) in candidates {
        if Path::new(program).is_file() {
            return fixed_version_command(program, args);
        }
    }
    anyhow::bail!("required version command is not installed")
}

fn fixed_version_command(program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() || output.stdout.len() + output.stderr.len() > MAX_VERSION_BYTES {
        anyhow::bail!("fixed version command failed or returned excessive output");
    }
    let bytes = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let value = std::str::from_utf8(bytes)?.trim();
    if value.is_empty()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\n')
    {
        anyhow::bail!("fixed version command returned unsafe output");
    }
    Ok(value.lines().next().unwrap_or_default().to_string())
}

fn sha256(bytes: &[u8]) -> String {
    hex_string(Sha256::digest(bytes).as_slice())
}

fn hex_string(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unix_now() -> anyhow::Result<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(seconds).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use serde_json::json;

    use super::*;

    fn marker() -> DisposableMarker {
        DisposableMarker {
            protocol: MARKER_FORMAT.to_string(),
            disposable: true,
            expires_at_unix: 1_100,
            guest_id: "matrix-guest-01".to_string(),
            snapshot_id: "clean-snapshot-01".to_string(),
            interruption_mechanism: "process_error".to_string(),
            machine_id_sha256: "a".repeat(64),
            guest_binary_sha256: "b".repeat(64),
            manifest_sha256: "c".repeat(64),
            operation_id: "018f4d5e-1234-7abc-8def-123456789abc".to_string(),
            run_id: "018f4d5e-5678-7abc-8def-123456789abc".to_string(),
            source_revision: "d".repeat(40),
            release_sha256: "e".repeat(64),
            fault_point_set_sha256: fault_catalog_sha256().to_string(),
            case_id: "source_fence_commit--before".to_string(),
            fault_point: BareMetalFaultPoint::SourceFenceCommit,
            mode: BareMetalFaultMode::Before,
            case_binding_sha256: fault_case_binding_sha256(
                "018f4d5e-1234-7abc-8def-123456789abc",
                "018f4d5e-5678-7abc-8def-123456789abc",
                BareMetalFaultCase {
                    point: BareMetalFaultPoint::SourceFenceCommit,
                    mode: BareMetalFaultMode::Before,
                },
            )
            .unwrap(),
            unit_allowlist: vec!["v2board-matrix-mysql.service".to_string()],
        }
    }

    #[test]
    fn cli_is_closed_and_does_not_accept_paths() {
        assert!(!v2board_provision::PRODUCTION_LEGACY_APPLY_CAPABILITY.is_available());
        assert_eq!(
            parse_args(
                [
                    "list-cases",
                    "--run-id",
                    "018f4d5e-5678-7abc-8def-123456789abc",
                    "--operation-id",
                    "018f4d5e-1234-7abc-8def-123456789abc",
                ]
                .map(str::to_owned),
            )
            .unwrap(),
            GuestCommand {
                action: GuestAction::ListCases,
                fault_case: None,
                run_id: Some("018f4d5e-5678-7abc-8def-123456789abc".to_string()),
                operation_id: Some("018f4d5e-1234-7abc-8def-123456789abc".to_string()),
            }
        );
        assert_eq!(
            parse_args(["start", "--case", "source_fence_commit--before"].map(str::to_owned))
                .unwrap(),
            GuestCommand {
                action: GuestAction::Start,
                fault_case: Some("source_fence_commit--before".to_string()),
                run_id: None,
                operation_id: None,
            }
        );
        assert!(parse_args(["start", "--manifest", "/tmp/prod.json"].map(str::to_owned)).is_err());
        assert!(parse_args(["resume", "--case", "point;rm:before"].map(str::to_owned)).is_err());
    }

    #[test]
    fn marker_must_be_current_machine_bound_and_short_lived() {
        let mut value = marker();
        validate_marker_shape(&value, 1_000, "source_fence_commit--before").unwrap();
        value.disposable = false;
        assert!(validate_marker_shape(&value, 1_000, "source_fence_commit--before").is_err());
        value = marker();
        value.expires_at_unix = 1_000;
        assert!(validate_marker_shape(&value, 1_000, "source_fence_commit--before").is_err());
        value = marker();
        value.expires_at_unix = 1_000 + 86_401;
        assert!(validate_marker_shape(&value, 1_000, "source_fence_commit--before").is_err());
        value = marker();
        value.machine_id_sha256 = "A".repeat(64);
        assert!(validate_marker_shape(&value, 1_000, "source_fence_commit--before").is_err());
        value = marker();
        value.case_id = "source_drain_commit--before".to_string();
        assert!(validate_marker_shape(&value, 1_000, "source_fence_commit--before").is_err());
    }

    #[test]
    fn sensitive_file_metadata_is_exact() {
        let safe = SecureMetadata {
            uid: 0,
            gid: 0,
            mode: 0o400,
            nlink: 1,
            is_file: true,
        };
        validate_secure_metadata(safe, &[0o400, 0o600]).unwrap();
        for unsafe_metadata in [
            SecureMetadata { uid: 1000, ..safe },
            SecureMetadata { gid: 1000, ..safe },
            SecureMetadata {
                mode: 0o640,
                ..safe
            },
            SecureMetadata { nlink: 2, ..safe },
            SecureMetadata {
                is_file: false,
                ..safe
            },
        ] {
            assert!(validate_secure_metadata(unsafe_metadata, &[0o400, 0o600]).is_err());
        }
        validate_secure_metadata_with_nlinks(
            SecureMetadata { nlink: 2, ..safe },
            &[0o400],
            &[1, 2],
        )
        .unwrap();
    }

    #[test]
    fn durable_evidence_recovers_the_published_nlink_two_window() {
        let root = Path::new("/root").join(format!(
            "fault-matrix-evidence-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let final_path = root.join("evidence.json");
        let temporary = root.join(format!(".evidence.json.{}.writing", uuid::Uuid::new_v4()));
        let bytes = b"{\"status\":\"admitted\"}\n";
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        fs::hard_link(&temporary, &final_path).unwrap();
        assert_eq!(fs::metadata(&final_path).unwrap().nlink(), 2);

        write_new_durable(&final_path, bytes).unwrap();
        assert_eq!(fs::metadata(&final_path).unwrap().nlink(), 1);
        assert!(!temporary.exists());
        write_new_durable(&final_path, bytes).unwrap();
        assert!(write_new_durable(&final_path, b"conflict\n").is_err());

        fs::remove_file(&final_path).unwrap();
        fs::remove_dir(&root).unwrap();
    }

    #[test]
    fn unit_allowlist_is_exact_and_legacy_units_are_throwaway_only() {
        let writers = vec!["v2board-matrix-legacy-api.service".to_string()];
        let workers = vec!["v2board-matrix-legacy-worker.service".to_string()];
        let schedulers = vec!["v2board-matrix-legacy-scheduler.timer".to_string()];
        let policy = || UnitPolicy {
            api: "v2board-api.service",
            worker: "v2board-worker.service",
            legacy_writers: &writers,
            legacy_workers: &workers,
            legacy_schedulers: &schedulers,
            mysql: "v2board-matrix-mysql.service",
            default_redis: "v2board-matrix-redis.service",
            cache_redis: "v2board-matrix-redis.service",
        };
        let exact = vec![
            "v2board-api.service".to_string(),
            "v2board-worker.service".to_string(),
            "v2board-matrix-legacy-api.service".to_string(),
            "v2board-matrix-legacy-worker.service".to_string(),
            "v2board-matrix-legacy-scheduler.timer".to_string(),
            "v2board-matrix-mysql.service".to_string(),
            "v2board-matrix-redis.service".to_string(),
        ];
        validate_unit_allowlist(policy(), &exact).unwrap();
        assert!(validate_unit_allowlist(policy(), &exact[..exact.len() - 1]).is_err());

        let unsafe_policy = UnitPolicy {
            mysql: "mysql.service",
            ..policy()
        };
        assert!(validate_unit_allowlist(unsafe_policy, &exact).is_err());
    }

    #[test]
    fn urls_require_literal_loopback_and_reserved_database_names() {
        require_loopback_url("mysql://u:p@127.0.0.1:3306/v2board_matrix_source", "source").unwrap();
        require_loopback_url("redis://u:p@[::1]:6379/0", "redis").unwrap();
        assert!(
            require_loopback_url("mysql://u:p@localhost:3306/v2board_matrix_source", "source")
                .is_err()
        );
        assert!(
            require_loopback_url("mysql://u:p@10.0.0.2:3306/v2board_matrix_source", "source")
                .is_err()
        );
        require_test_database_url("postgres://u:p@127.0.0.1/v2board_matrix_target", "target")
            .unwrap();
        assert!(require_test_database_url("postgres://u:p@127.0.0.1/v2board", "target").is_err());
        assert!(!valid_test_database_name("v2board_matrix_"));
        assert!(!valid_test_database_name("v2board_matrix_BAD"));
    }

    #[test]
    fn manifest_admission_rejects_remote_and_non_test_datastores() {
        let marker = marker();
        let mut manifest = json!({
            "schema_version": 4,
            "operation_id": marker.operation_id,
            "kind": "legacy_reference_migration",
            "source": {
                "database_url": "mysql://u:p@127.0.0.1/v2board_matrix_source",
                "database_fence_url": "mysql://f:p@127.0.0.1",
                "redis_default_url": "redis://u:p@127.0.0.1/0",
                "redis_cache_url": "redis://u:p@127.0.0.1/1"
            },
            "target": {
                "postgres": {
                    "bootstrap_database_url": "postgres://b:p@127.0.0.1/postgres",
                    "migration_database_url": "postgres://m:p@127.0.0.1/v2board_matrix_target",
                    "api_database_url": "postgres://a:p@127.0.0.1/v2board_matrix_target",
                    "worker_database_url": "postgres://w:p@127.0.0.1/v2board_matrix_target"
                },
                "clickhouse": {
                    "endpoint": "http://127.0.0.1:8123",
                    "database": "v2board_matrix_analytics"
                },
                "redis_url": "redis://:p@127.0.0.1/2"
            },
            "execution": {
                "backup": {
                    "isolated_restore_database_url": "mysql://r:p@127.0.0.1/v2board_matrix_restore"
                }
            }
        });
        validate_disposable_manifest(&manifest, &marker).unwrap();
        manifest["source"]["database_url"] =
            Value::String("mysql://u:p@192.0.2.1/v2board_matrix_source".to_string());
        assert!(validate_disposable_manifest(&manifest, &marker).is_err());
        manifest["source"]["database_url"] =
            Value::String("mysql://u:p@127.0.0.1/v2board".to_string());
        assert!(validate_disposable_manifest(&manifest, &marker).is_err());
    }

    #[test]
    fn release_metadata_is_exact() {
        let valid = b"format=v2board-native-release-v1\nsource_revision=0123456789abcdef0123456789abcdef01234567\ntarget_os=linux\ntarget_arch=amd64\n";
        let evidence = parse_release_metadata(valid, "release-1", &"a".repeat(64)).unwrap();
        assert_eq!(
            evidence.source_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        for invalid in [
            b"format=v2board-native-release-v1\nsource_revision=unknown\ntarget_os=linux\ntarget_arch=amd64\n".as_slice(),
            b"format=v2board-native-release-v1\nsource_revision=0123456789abcdef0123456789abcdef01234567\ntarget_os=linux\ntarget_arch=arm64\n".as_slice(),
            b"format=v2board-native-release-v1\nformat=v2board-native-release-v1\nsource_revision=0123456789abcdef0123456789abcdef01234567\ntarget_os=linux\ntarget_arch=amd64\n".as_slice(),
        ] {
            assert!(parse_release_metadata(invalid, "release-1", &"a".repeat(64)).is_err());
        }
        assert_eq!(
            normalize_archive_path(Path::new("./RELEASE")).unwrap(),
            Path::new("RELEASE")
        );
        assert!(normalize_archive_path(Path::new("../RELEASE")).is_err());
        assert!(normalize_archive_path(Path::new("/RELEASE")).is_err());
    }
}
