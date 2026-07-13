//! Encrypted, one-shot legacy source backup and isolated restore drill.
//!
//! The only durable backup artifact produced here is the age-encrypted byte
//! stream. Its versioned plaintext framing contains the HMAC-verified frozen
//! Redis traffic receipt followed by the MySQL dump. Restore verification
//! decrypts that exact on-disk artifact, validates the framing and receipt
//! before feeding only the dump frame to the fixed MySQL 8 client, and
//! never materializes plaintext as a file. An HMAC-bound ownership state makes
//! cleanup of an interrupted restore safe without treating an arbitrary
//! non-empty database as disposable.

use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use url::Url;

use crate::{
    ProvisionSpec,
    apply_journal::{
        ApplyCheckpoint, ApplyJournal, ApplyJournalSnapshot, ApplyJournalState,
        DurableNativeStartPermit, DurableTargetMutationPermit, backup_reference_sha256,
    },
    legacy_apply::BackupRestoreProof,
    legacy_converter::{LEGACY_SEMANTIC_SCHEMA_SHA256, LegacyConversionStrategy},
    manifest::{LegacyRuntimeReceiptKind, ProvisionFlow, SourceSpec},
    native_legacy_source::{
        VerifiedFrozenTrafficReceipt, fingerprint_mysql_and_schema_for_strategy,
        fingerprint_mysql_for_strategy, remove_verified_frozen_traffic_receipt,
        verify_frozen_traffic_receipt, verify_frozen_traffic_receipt_bytes,
        verify_redis_fence_for_backup,
    },
};

const AGE_PATH: &str = "/usr/bin/age";
const MYSQL_DUMP_PATH: &str = "/usr/bin/mysqldump";
const MYSQL_CLIENT_PATH: &str = "/usr/bin/mysql";
const DF_PATH: &str = "/usr/bin/df";
const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;
const MAX_CONTROL_FILE_BYTES: u64 = 128 * 1024;
const MAX_TRAFFIC_RECEIPT_BYTES: u64 = 256 * 1024 * 1024;
const RESTORE_DATABASE_TIMEOUT: Duration = Duration::from_secs(30);
const LEGACY_ARCHIVE_FORMAT_VERSION: u32 = 1;
const LEGACY_ARCHIVE_MAGIC: &[u8] = b"v2board-legacy-age-bundle\0";
const LEGACY_ARCHIVE_TRAFFIC_TAG: &[u8] = b"traffic-receipt\0";
const LEGACY_ARCHIVE_MYSQL_TAG: &[u8] = b"mysql-dump\0";
const LEGACY_ARCHIVE_STREAM_TO_EOF: u64 = u64::MAX;
const BACKUP_STATE_SCHEMA_VERSION: u32 = 2;
const BACKUP_RECEIPT_SCHEMA_VERSION: u32 = 3;
const ARCHIVE_MATERIALIZATION_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("legacy backup policy is invalid")]
    InvalidPolicy,
    #[error("legacy backup input is unsafe or differs from its bound digest")]
    InvalidInput,
    #[error("legacy backup state conflicts with a prior attempt")]
    Conflict,
    #[error("legacy source or restore server version is unsupported")]
    UnsupportedServer,
    #[error("legacy source changed during the backup drill")]
    SourceDrift,
    #[error("isolated restore database was not empty before ownership was reserved")]
    RestoreNotEmpty,
    #[error("encrypted backup command failed")]
    DumpFailed,
    #[error("actual encrypted artifact could not be decrypted and restored")]
    RestoreFailed,
    #[error("restored values differ from the fenced source")]
    RestoreMismatch,
    #[error("isolated restore cleanup could not be proved")]
    CleanupFailed,
    #[error("legacy backup receipt is invalid")]
    ReceiptInvalid,
    #[error("legacy backup filesystem operation failed")]
    Filesystem,
    #[error("legacy backup database operation failed")]
    Database,
}

#[derive(Clone, Debug)]
struct ClientEndpoint {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
    ssl_mode: Option<SslMode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SslMode {
    Disabled,
    Preferred,
    Required,
    VerifyCa,
    VerifyIdentity,
}

impl ClientEndpoint {
    fn parse(value: &str) -> Result<Self, BackupError> {
        let url = Url::parse(value).map_err(|_| BackupError::InvalidPolicy)?;
        if url.scheme() != "mysql" || url.fragment().is_some() {
            return Err(BackupError::InvalidPolicy);
        }
        let host = url
            .host_str()
            .filter(|value| !value.is_empty())
            .ok_or(BackupError::InvalidPolicy)?
            .to_string();
        let username = decode(url.username())?;
        let password = url.password().map(decode).transpose()?.unwrap_or_default();
        let database = decode(url.path().strip_prefix('/').unwrap_or_default())?;
        if username.is_empty()
            || password.is_empty()
            || database.is_empty()
            || database.contains('/')
            || [
                host.as_str(),
                username.as_str(),
                password.as_str(),
                database.as_str(),
            ]
            .iter()
            .any(|value| {
                value
                    .bytes()
                    .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
            })
        {
            return Err(BackupError::InvalidPolicy);
        }
        let mut ssl_mode = None;
        for (key, value) in url.query_pairs() {
            if key != "ssl-mode" || ssl_mode.is_some() {
                // A connection option which SQLx understands is not
                // automatically safe or equivalent for both CLI families.
                return Err(BackupError::InvalidPolicy);
            }
            ssl_mode = Some(
                match value.to_ascii_lowercase().replace('-', "_").as_str() {
                    "disabled" => SslMode::Disabled,
                    "preferred" => SslMode::Preferred,
                    "required" => SslMode::Required,
                    "verify_ca" => SslMode::VerifyCa,
                    "verify_identity" => SslMode::VerifyIdentity,
                    _ => return Err(BackupError::InvalidPolicy),
                },
            );
        }
        Ok(Self {
            host,
            port: url.port().unwrap_or(3306),
            username,
            password,
            database,
            ssl_mode,
        })
    }

    fn redacted_identity(&self) -> String {
        format!(
            "mysql://{}:{}/{}",
            self.host.to_ascii_lowercase(),
            self.port,
            self.database
        )
    }

    fn defaults_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(b"[client]\nprotocol=TCP\n");
        append_option(&mut result, "host", &self.host);
        append_option(&mut result, "port", &self.port.to_string());
        append_option(&mut result, "user", &self.username);
        append_option(&mut result, "password", &self.password);
        if let Some(mode) = self.ssl_mode {
            let value = match mode {
                SslMode::Disabled => "DISABLED",
                SslMode::Preferred => "PREFERRED",
                SslMode::Required => "REQUIRED",
                SslMode::VerifyCa => "VERIFY_CA",
                SslMode::VerifyIdentity => "VERIFY_IDENTITY",
            };
            append_option(&mut result, "ssl-mode", value);
        }
        result
    }
}

impl Drop for ClientEndpoint {
    fn drop(&mut self) {
        // The manifest already owns another copy, but avoid retaining an extra
        // decoded password in this stage longer than necessary.
        self.password.clear();
    }
}

fn append_option(buffer: &mut Vec<u8>, key: &str, value: &str) {
    buffer.extend_from_slice(key.as_bytes());
    buffer.extend_from_slice(b"=\"");
    for byte in value.bytes() {
        match byte {
            b'\\' | b'"' => {
                buffer.push(b'\\');
                buffer.push(byte);
            }
            b'\t' => buffer.extend_from_slice(b"\\t"),
            other => buffer.push(other),
        }
    }
    buffer.extend_from_slice(b"\"\n");
}

fn decode(value: &str) -> Result<String, BackupError> {
    percent_decode_str(value)
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| BackupError::InvalidPolicy)
}

/// The exact source-drain receipt admitted into the sole encrypted archive.
/// Construction happens only after the source receipt's scoped HMAC, full
/// delta set, and SourceDrained journal binding have been verified.
#[derive(Clone)]
struct TrafficReceiptArchiveInput {
    bytes: Arc<[u8]>,
    sha256: String,
    maintenance_fenced_generation: u64,
    maintenance_fenced_event_sha256: String,
    sorted_user_delta_count: u64,
    sorted_user_delta_sha256: String,
    upload_delta_sum: String,
    download_delta_sum: String,
}

impl TrafficReceiptArchiveInput {
    fn from_verified(
        bytes: Vec<u8>,
        receipt: &VerifiedFrozenTrafficReceipt,
    ) -> Result<Self, BackupError> {
        if bytes.is_empty()
            || bytes.len() as u64 > MAX_TRAFFIC_RECEIPT_BYTES
            || !is_lower_sha256(&receipt.receipt_sha256)
            || receipt.maintenance_fenced_generation == 0
            || !is_lower_sha256(&receipt.maintenance_fenced_event_sha256)
            || !is_lower_sha256(&receipt.sorted_user_delta_sha256)
            || receipt.upload_delta_sum.parse::<u128>().is_err()
            || receipt.download_delta_sum.parse::<u128>().is_err()
            || hex::encode(Sha256::digest(&bytes)) != receipt.receipt_sha256
        {
            return Err(BackupError::ReceiptInvalid);
        }
        Ok(Self {
            bytes: Arc::from(bytes),
            sha256: receipt.receipt_sha256.clone(),
            maintenance_fenced_generation: receipt.maintenance_fenced_generation,
            maintenance_fenced_event_sha256: receipt.maintenance_fenced_event_sha256.clone(),
            sorted_user_delta_count: receipt.sorted_user_delta_count,
            sorted_user_delta_sha256: receipt.sorted_user_delta_sha256.clone(),
            upload_delta_sum: receipt.upload_delta_sum.clone(),
            download_delta_sum: receipt.download_delta_sum.clone(),
        })
    }

    fn byte_len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn require_exact_initial_source_drained_head(
        &self,
        head: &ApplyJournalSnapshot,
    ) -> Result<(), BackupError> {
        if head.checkpoint() != ApplyCheckpoint::SourceDrained
            || self.maintenance_fenced_generation.checked_add(1) != Some(head.generation())
            || head.previous_event_sha256() != Some(self.maintenance_fenced_event_sha256.as_str())
            || head.checkpoint_proof_sha256() != Some(self.sha256.as_str())
        {
            return Err(BackupError::ReceiptInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArchiveFrameVerification {
    format_version: u32,
    traffic_receipt_sha256: String,
    traffic_receipt_bytes: u64,
    mysql_dump_bytes: u64,
}

fn invalid_archive_frame() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid legacy archive frame")
}

fn sha256_binary(value: &str) -> io::Result<[u8; 32]> {
    let mut result = [0_u8; 32];
    hex::decode_to_slice(value, &mut result).map_err(|_| invalid_archive_frame())?;
    Ok(result)
}

fn write_legacy_archive_plaintext(
    mut mysql_dump: impl Read,
    mut encrypted_plaintext: impl Write,
    traffic: &TrafficReceiptArchiveInput,
) -> io::Result<ArchiveFrameVerification> {
    encrypted_plaintext.write_all(LEGACY_ARCHIVE_MAGIC)?;
    encrypted_plaintext.write_all(&LEGACY_ARCHIVE_FORMAT_VERSION.to_be_bytes())?;
    encrypted_plaintext.write_all(LEGACY_ARCHIVE_TRAFFIC_TAG)?;
    encrypted_plaintext.write_all(&traffic.byte_len().to_be_bytes())?;
    encrypted_plaintext.write_all(&sha256_binary(&traffic.sha256)?)?;
    encrypted_plaintext.write_all(&traffic.bytes)?;
    encrypted_plaintext.write_all(LEGACY_ARCHIVE_MYSQL_TAG)?;
    encrypted_plaintext.write_all(&LEGACY_ARCHIVE_STREAM_TO_EOF.to_be_bytes())?;
    let mysql_dump_bytes = io::copy(&mut mysql_dump, &mut encrypted_plaintext)?;
    if mysql_dump_bytes == 0 {
        return Err(invalid_archive_frame());
    }
    encrypted_plaintext.flush()?;
    Ok(ArchiveFrameVerification {
        format_version: LEGACY_ARCHIVE_FORMAT_VERSION,
        traffic_receipt_sha256: traffic.sha256.clone(),
        traffic_receipt_bytes: traffic.byte_len(),
        mysql_dump_bytes,
    })
}

fn read_exact_tag(input: &mut impl Read, expected: &[u8]) -> io::Result<()> {
    let mut actual = vec![0_u8; expected.len()];
    input.read_exact(&mut actual)?;
    if actual != expected {
        return Err(invalid_archive_frame());
    }
    Ok(())
}

fn read_u32(input: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0_u8; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_u64(input: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0_u8; 8];
    input.read_exact(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

struct DecryptedTrafficFrame {
    format_version: u32,
    bytes: Vec<u8>,
    sha256: String,
}

/// Parses and authenticates the archive framing through the MySQL stream
/// boundary. The returned reader is positioned at the first dump byte.
fn read_legacy_archive_traffic_frame(
    decrypted_plaintext: &mut impl Read,
) -> io::Result<DecryptedTrafficFrame> {
    read_exact_tag(decrypted_plaintext, LEGACY_ARCHIVE_MAGIC)?;
    let format_version = read_u32(decrypted_plaintext)?;
    if format_version != LEGACY_ARCHIVE_FORMAT_VERSION {
        return Err(invalid_archive_frame());
    }
    read_exact_tag(decrypted_plaintext, LEGACY_ARCHIVE_TRAFFIC_TAG)?;
    let receipt_bytes = read_u64(decrypted_plaintext)?;
    if receipt_bytes == 0 || receipt_bytes > MAX_TRAFFIC_RECEIPT_BYTES {
        return Err(invalid_archive_frame());
    }
    let mut declared_sha256 = [0_u8; 32];
    decrypted_plaintext.read_exact(&mut declared_sha256)?;
    let receipt_len = usize::try_from(receipt_bytes).map_err(|_| invalid_archive_frame())?;
    let mut bytes = vec![0_u8; receipt_len];
    decrypted_plaintext.read_exact(&mut bytes)?;
    let actual_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    if actual_sha256 != declared_sha256 {
        return Err(invalid_archive_frame());
    }
    read_exact_tag(decrypted_plaintext, LEGACY_ARCHIVE_MYSQL_TAG)?;
    if read_u64(decrypted_plaintext)? != LEGACY_ARCHIVE_STREAM_TO_EOF {
        return Err(invalid_archive_frame());
    }
    Ok(DecryptedTrafficFrame {
        format_version,
        bytes,
        sha256: hex::encode(declared_sha256),
    })
}

/// Validates the complete traffic frame before writing the first MySQL dump
/// byte. `expected_traffic` was constructed only after scoped-HMAC validation,
/// so an exact byte comparison revalidates that the decrypted frame contains
/// precisely those HMAC-authenticated receipt bytes.
fn restore_legacy_archive_plaintext(
    mut decrypted_plaintext: impl Read,
    mut mysql_stdin: impl Write,
    expected_traffic: &TrafficReceiptArchiveInput,
) -> io::Result<ArchiveFrameVerification> {
    let traffic = read_legacy_archive_traffic_frame(&mut decrypted_plaintext)?;
    if traffic.sha256 != expected_traffic.sha256
        || traffic.bytes.as_slice() != expected_traffic.bytes.as_ref()
    {
        return Err(invalid_archive_frame());
    }
    let mysql_dump_bytes = io::copy(&mut decrypted_plaintext, &mut mysql_stdin)?;
    if mysql_dump_bytes == 0 {
        return Err(invalid_archive_frame());
    }
    mysql_stdin.flush()?;
    Ok(ArchiveFrameVerification {
        format_version: traffic.format_version,
        traffic_receipt_sha256: expected_traffic.sha256.clone(),
        traffic_receipt_bytes: expected_traffic.byte_len(),
        mysql_dump_bytes,
    })
}

fn read_legacy_archive_traffic_and_discard_dump(
    mut decrypted_plaintext: impl Read,
) -> io::Result<(Vec<u8>, ArchiveFrameVerification)> {
    let traffic = read_legacy_archive_traffic_frame(&mut decrypted_plaintext)?;
    let mysql_dump_bytes = io::copy(&mut decrypted_plaintext, &mut io::sink())?;
    if mysql_dump_bytes == 0 {
        return Err(invalid_archive_frame());
    }
    let receipt_bytes = traffic.bytes.len() as u64;
    let verification = ArchiveFrameVerification {
        format_version: traffic.format_version,
        traffic_receipt_sha256: traffic.sha256,
        traffic_receipt_bytes: receipt_bytes,
        mysql_dump_bytes,
    };
    Ok((traffic.bytes, verification))
}

#[derive(Clone, Debug)]
struct Toolchain {
    age: PathBuf,
    dump: PathBuf,
    client: PathBuf,
    timeout: Duration,
    maximum_encrypted_bytes: u64,
}

impl Toolchain {
    fn production(timeout: Duration, maximum_encrypted_bytes: u64) -> Result<Self, BackupError> {
        let age = validate_program(AGE_PATH, &[AGE_PATH])?;
        let _ = validate_program(DF_PATH, &[DF_PATH])?;
        let dump = validate_program(MYSQL_DUMP_PATH, &[MYSQL_DUMP_PATH])?;
        let client = validate_program(MYSQL_CLIENT_PATH, &[MYSQL_CLIENT_PATH])?;
        verify_tool_version(&age, ToolVersion::Age)?;
        verify_tool_version(&dump, ToolVersion::MysqlDump)?;
        verify_tool_version(&client, ToolVersion::MysqlClient)?;
        Ok(Self {
            age,
            dump,
            client,
            timeout,
            maximum_encrypted_bytes,
        })
    }

    fn dump_args(&self, defaults_path: &Path, database: &str) -> Vec<OsString> {
        let mut args = vec![
            OsString::from(format!(
                "--defaults-extra-file={}",
                defaults_path.to_string_lossy()
            )),
            "--single-transaction".into(),
            "--quick".into(),
            "--skip-lock-tables".into(),
            "--hex-blob".into(),
            // Preflight rejects routines, events, and triggers. Deliberately
            // exclude them instead of demanding broad metadata privileges.
            "--skip-triggers".into(),
            "--default-character-set=utf8mb4".into(),
        ];
        args.push("--set-gtid-purged=OFF".into());
        // Keep the dump independent of server-side statistics and broad
        // tablespace privileges. The production tool contract is MySQL 8.
        args.push("--column-statistics=0".into());
        args.push("--no-tablespaces".into());
        args.push("--".into());
        args.push(database.into());
        args
    }

    fn client_args(&self, defaults_path: &Path, database: &str) -> Vec<OsString> {
        vec![
            OsString::from(format!(
                "--defaults-extra-file={}",
                defaults_path.to_string_lossy()
            )),
            "--binary-mode".into(),
            "--default-character-set=utf8mb4".into(),
            "--".into(),
            database.into(),
        ]
    }

    fn create_encrypted_dump(
        &self,
        endpoint: &ClientEndpoint,
        defaults_path: &Path,
        recipient_path: &Path,
        traffic: &TrafficReceiptArchiveInput,
        partial_path: &Path,
    ) -> Result<(Artifact, ArchiveFrameVerification), BackupError> {
        prepare_partial_path(partial_path)?;
        let output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(partial_path)
            .map_err(|_| BackupError::Filesystem)?;
        let mut dump = command(&self.dump)
            .args(self.dump_args(defaults_path, &endpoint.database))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| BackupError::DumpFailed)?;
        let dump_stdout = dump.stdout.take().ok_or(BackupError::DumpFailed)?;
        let mut age = command(&self.age)
            .arg("--encrypt")
            .arg("--recipients-file")
            .arg(recipient_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| {
                terminate(&mut dump);
                BackupError::DumpFailed
            })?;
        let age_stdin = age.stdin.take().ok_or(BackupError::DumpFailed)?;
        let traffic_for_frame = traffic.clone();
        let frame_thread = thread::spawn(move || {
            write_legacy_archive_plaintext(dump_stdout, age_stdin, &traffic_for_frame)
        });
        let age_stdout = age.stdout.take().ok_or(BackupError::DumpFailed)?;
        let output_limit = self.maximum_encrypted_bytes;
        let output_thread =
            thread::spawn(move || copy_bounded_and_fsync(age_stdout, output, output_limit));
        let dump_stderr = bounded_reader(dump.stderr.take().ok_or(BackupError::DumpFailed)?);
        let age_stderr = bounded_reader(age.stderr.take().ok_or(BackupError::DumpFailed)?);
        let statuses = wait_pair(&mut dump, &mut age, self.timeout);
        let artifact = output_thread
            .join()
            .map_err(|_| BackupError::DumpFailed)?
            .map_err(|_| BackupError::DumpFailed)?;
        let frame = frame_thread
            .join()
            .map_err(|_| BackupError::DumpFailed)?
            .map_err(|_| BackupError::DumpFailed)?;
        let dump_diagnostic = dump_stderr
            .join()
            .map_err(|_| BackupError::DumpFailed)?
            .map_err(|_| BackupError::DumpFailed)?;
        let age_diagnostic = age_stderr
            .join()
            .map_err(|_| BackupError::DumpFailed)?
            .map_err(|_| BackupError::DumpFailed)?;
        if !statuses.is_ok_and(|(dump, age)| dump.success() && age.success())
            || dump_diagnostic.1
            || age_diagnostic.1
            || artifact.bytes == 0
        {
            let _ = remove_secure_file(partial_path);
            return Err(BackupError::DumpFailed);
        }
        Ok((artifact, frame))
    }

    fn restore_encrypted_dump(
        &self,
        endpoint: &ClientEndpoint,
        defaults_path: &Path,
        identity_path: &Path,
        artifact_path: &Path,
        expected_traffic: &TrafficReceiptArchiveInput,
    ) -> Result<ArchiveFrameVerification, BackupError> {
        let encrypted = open_owner_only_file(artifact_path, self.maximum_encrypted_bytes)?;
        let mut age = command(&self.age)
            .arg("--decrypt")
            .arg("--identity")
            .arg(identity_path)
            .stdin(Stdio::from(encrypted))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| BackupError::RestoreFailed)?;
        let plaintext = age.stdout.take().ok_or(BackupError::RestoreFailed)?;
        let mut mysql = command(&self.client)
            .args(self.client_args(defaults_path, &endpoint.database))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| {
                terminate(&mut age);
                BackupError::RestoreFailed
            })?;
        let mysql_stdin = mysql.stdin.take().ok_or(BackupError::RestoreFailed)?;
        let traffic_for_restore = expected_traffic.clone();
        let frame_thread = thread::spawn(move || {
            restore_legacy_archive_plaintext(plaintext, mysql_stdin, &traffic_for_restore)
        });
        let age_stderr = bounded_reader(age.stderr.take().ok_or(BackupError::RestoreFailed)?);
        let mysql_stdout = bounded_reader(mysql.stdout.take().ok_or(BackupError::RestoreFailed)?);
        let mysql_stderr = bounded_reader(mysql.stderr.take().ok_or(BackupError::RestoreFailed)?);
        let statuses = wait_pair(&mut age, &mut mysql, self.timeout);
        let diagnostics = [age_stderr, mysql_stdout, mysql_stderr]
            .into_iter()
            .map(|reader| {
                reader
                    .join()
                    .map_err(|_| BackupError::RestoreFailed)?
                    .map_err(|_| BackupError::RestoreFailed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame = frame_thread
            .join()
            .map_err(|_| BackupError::RestoreFailed)?
            .map_err(|_| BackupError::RestoreFailed)?;
        if !statuses.is_ok_and(|(age, mysql)| age.success() && mysql.success())
            || diagnostics.iter().any(|(_, overflow)| *overflow)
        {
            return Err(BackupError::RestoreFailed);
        }
        Ok(frame)
    }

    fn decrypt_verified_traffic_frame(
        &self,
        identity_path: &Path,
        artifact_path: &Path,
    ) -> Result<(Vec<u8>, ArchiveFrameVerification), BackupError> {
        let encrypted = open_owner_only_file(artifact_path, self.maximum_encrypted_bytes)?;
        let mut age = command(&self.age)
            .arg("--decrypt")
            .arg("--identity")
            .arg(identity_path)
            .stdin(Stdio::from(encrypted))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| BackupError::RestoreFailed)?;
        let plaintext = age.stdout.take().ok_or(BackupError::RestoreFailed)?;
        let frame_thread =
            thread::spawn(move || read_legacy_archive_traffic_and_discard_dump(plaintext));
        let stderr = bounded_reader(age.stderr.take().ok_or(BackupError::RestoreFailed)?);
        let status = wait_single(&mut age, self.timeout).map_err(|_| BackupError::RestoreFailed)?;
        let frame = frame_thread
            .join()
            .map_err(|_| BackupError::RestoreFailed)?
            .map_err(|_| BackupError::RestoreFailed)?;
        let (_, stderr_overflow) = stderr
            .join()
            .map_err(|_| BackupError::RestoreFailed)?
            .map_err(|_| BackupError::RestoreFailed)?;
        if !status.success() || stderr_overflow {
            return Err(BackupError::RestoreFailed);
        }
        Ok(frame)
    }
}

fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_clear().env("LC_ALL", "C").env("LANG", "C");
    command
}

fn validate_program(path: &str, allowed: &[&str]) -> Result<PathBuf, BackupError> {
    let declared = Path::new(path);
    if !allowed.iter().any(|allowed| declared == Path::new(allowed)) {
        return Err(BackupError::InvalidPolicy);
    }
    let link = fs::symlink_metadata(declared).map_err(|_| BackupError::InvalidInput)?;
    if link.uid() != 0 || (!link.file_type().is_symlink() && link.permissions().mode() & 0o022 != 0)
    {
        return Err(BackupError::InvalidInput);
    }
    let canonical = fs::canonicalize(declared).map_err(|_| BackupError::InvalidInput)?;
    let metadata = fs::metadata(&canonical).map_err(|_| BackupError::InvalidInput)?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.permissions().mode() & 0o111 == 0
        || !canonical.starts_with("/usr/bin")
    {
        return Err(BackupError::InvalidInput);
    }
    Ok(canonical)
}

#[derive(Clone, Copy)]
enum ToolVersion {
    Age,
    MysqlDump,
    MysqlClient,
}

fn verify_tool_version(program: &Path, expected: ToolVersion) -> Result<(), BackupError> {
    let mut child = command(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| BackupError::InvalidInput)?;
    let stdout = bounded_reader(child.stdout.take().ok_or(BackupError::InvalidInput)?);
    let stderr = bounded_reader(child.stderr.take().ok_or(BackupError::InvalidInput)?);
    let status = wait_single(&mut child, Duration::from_secs(30))?;
    let (mut stdout, stdout_overflow) = stdout
        .join()
        .map_err(|_| BackupError::InvalidInput)?
        .map_err(|_| BackupError::InvalidInput)?;
    let (stderr, stderr_overflow) = stderr
        .join()
        .map_err(|_| BackupError::InvalidInput)?
        .map_err(|_| BackupError::InvalidInput)?;
    stdout.extend_from_slice(&stderr);
    let text = String::from_utf8(stdout).map_err(|_| BackupError::InvalidInput)?;
    let lower = text.to_ascii_lowercase();
    let valid = status.success()
        && !stdout_overflow
        && !stderr_overflow
        && !lower.is_empty()
        && match expected {
            ToolVersion::Age => {
                lower.contains("age")
                    || lower.starts_with('v')
                    || (lower.as_bytes().first().is_some_and(u8::is_ascii_digit)
                        && lower.contains('.'))
            }
            ToolVersion::MysqlDump | ToolVersion::MysqlClient => {
                !lower.contains("mariadb")
                    && !lower.contains("percona")
                    && (lower.contains("ver 8.") || lower.contains("distrib 8."))
            }
        };
    if !valid {
        return Err(BackupError::UnsupportedServer);
    }
    Ok(())
}

fn wait_pair(
    first: &mut Child,
    second: &mut Child,
    timeout: Duration,
) -> Result<(ExitStatus, ExitStatus), BackupError> {
    let deadline = Instant::now() + timeout;
    let mut first_status = None;
    let mut second_status = None;
    loop {
        if first_status.is_none() {
            first_status = first.try_wait().map_err(|_| BackupError::DumpFailed)?;
        }
        if second_status.is_none() {
            second_status = second.try_wait().map_err(|_| BackupError::DumpFailed)?;
        }
        if let (Some(first), Some(second)) = (first_status, second_status) {
            return Ok((first, second));
        }
        if Instant::now() >= deadline {
            terminate(first);
            terminate(second);
            return Err(BackupError::DumpFailed);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn wait_single(child: &mut Child, timeout: Duration) -> Result<ExitStatus, BackupError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|_| BackupError::Filesystem)? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            terminate(child);
            return Err(BackupError::Filesystem);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn bounded_reader(
    reader: impl Read + Send + 'static,
) -> thread::JoinHandle<io::Result<(Vec<u8>, bool)>> {
    thread::spawn(move || read_bounded(reader, MAX_DIAGNOSTIC_BYTES))
}

fn read_bounded(mut reader: impl Read, maximum: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut kept = Vec::new();
    let mut overflow = false;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let available = maximum.saturating_sub(kept.len());
        let retained = available.min(count);
        kept.extend_from_slice(&buffer[..retained]);
        overflow |= retained != count;
    }
    Ok((kept, overflow))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Artifact {
    sha256: String,
    bytes: u64,
}

fn copy_bounded_and_fsync(
    mut reader: impl Read,
    mut output: File,
    maximum: u64,
) -> io::Result<Artifact> {
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("bounded output overflow"))?;
        if total > maximum {
            return Err(io::Error::other("bounded output exceeded"));
        }
        output.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
    }
    output.sync_all()?;
    Ok(Artifact {
        sha256: hex::encode(digest.finalize()),
        bytes: total,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RestorePhase {
    Reserved,
    DumpCommitted,
    RestoreInProgress,
    Destroyed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RestoreStatePayload {
    document_kind: String,
    schema_version: u32,
    operation_id: String,
    manifest_binding_hmac_sha256: String,
    backup_reference: String,
    source_identity_sha256: String,
    restore_identity_sha256: String,
    recipient_sha256: String,
    decryption_identity_sha256: String,
    initial_source_fingerprint_sha256: String,
    source_drained_generation: u64,
    source_drained_event_sha256: String,
    source_drained_proof_sha256: String,
    archive_format_version: u32,
    traffic_receipt_sha256: String,
    traffic_receipt_bytes: u64,
    traffic_sorted_user_delta_count: u64,
    traffic_sorted_user_delta_sha256: String,
    traffic_upload_delta_sum: String,
    traffic_download_delta_sum: String,
    traffic_redis_physically_verified_before_archive: bool,
    started_at_unix: i64,
    phase: RestorePhase,
    encrypted_backup_sha256: Option<String>,
    encrypted_backup_bytes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RestoreStateEnvelope {
    payload: RestoreStatePayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BackupReceiptPayload {
    document_kind: String,
    schema_version: u32,
    operation_id: String,
    source_drained_generation: u64,
    source_drained_event_sha256: String,
    source_drained_proof_sha256: String,
    manifest_binding_hmac_sha256: String,
    backup_reference: String,
    source_server_identity_sha256: String,
    source_identity_sha256: String,
    restore_identity_sha256: String,
    source_fingerprint_before_sha256: String,
    source_fingerprint_after_sha256: String,
    restored_fingerprint_sha256: String,
    archive_format_version: u32,
    traffic_receipt_sha256: String,
    traffic_receipt_bytes: u64,
    traffic_sorted_user_delta_count: u64,
    traffic_sorted_user_delta_sha256: String,
    traffic_upload_delta_sum: String,
    traffic_download_delta_sum: String,
    traffic_redis_physically_verified_before_archive: bool,
    traffic_receipt_hmac_verified: bool,
    traffic_receipt_restored_from_encrypted_archive: bool,
    mysql_dump_bytes_restored: u64,
    encrypted_backup_sha256: String,
    encrypted_backup_bytes: u64,
    recipient_sha256: String,
    decryption_identity_sha256: String,
    actual_encrypted_bytes_decrypted: bool,
    isolated_restore_destroyed: bool,
    started_at_unix: i64,
    completed_at_unix: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BackupReceiptEnvelope {
    payload: BackupReceiptPayload,
    hmac_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedBackupArchive {
    backup_reference: String,
    source_fingerprint_sha256: String,
    archive_format_version: u32,
    traffic_receipt_sha256: String,
    traffic_receipt_bytes: u64,
    traffic_sorted_user_delta_count: u64,
    traffic_sorted_user_delta_sha256: String,
    traffic_upload_delta_sum: String,
    traffic_download_delta_sum: String,
    encrypted_backup_sha256: String,
    encrypted_backup_bytes: u64,
    receipt_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ArchiveMaterializationPhase {
    Reserved,
    RestoreInProgress,
    Ready,
    Destroying,
    Destroyed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchiveMaterializationStatePayload {
    document_kind: String,
    schema_version: u32,
    operation_id: String,
    manifest_binding_hmac_sha256: String,
    backup_reference_sha256: String,
    backup_receipt_sha256: String,
    encrypted_backup_sha256: String,
    source_fingerprint_sha256: String,
    source_schema_sha256: String,
    restore_identity_sha256: String,
    phase: ArchiveMaterializationPhase,
    materialized_fingerprint_sha256: Option<String>,
    materialized_schema_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchiveMaterializationStateEnvelope {
    payload: ArchiveMaterializationStatePayload,
    hmac_sha256: String,
}

/// Journal capability accepted by the pre-authority archive materializer.
/// Both variants carry the immutable backup proof/reference; callers cannot
/// materialize an uncommitted dump merely by knowing its filesystem path.
pub(crate) enum ArchiveMaterializationAnchor<'a> {
    Journal(&'a ApplyJournalSnapshot),
    Target(&'a DurableTargetMutationPermit),
}

/// Exact, operation-owned MySQL 8 recovery database reconstructed from
/// the verified age archive. The URL contains a credential and therefore is
/// deliberately neither `Debug` nor serializable.
pub(crate) struct VerifiedArchiveMaterialization {
    database_url: String,
    source_fingerprint_sha256: String,
    source_schema_sha256: String,
}

impl VerifiedArchiveMaterialization {
    pub(crate) fn database_url(&self) -> &str {
        &self.database_url
    }

    pub(crate) fn source_fingerprint_sha256(&self) -> &str {
        &self.source_fingerprint_sha256
    }

    pub(crate) fn source_schema_sha256(&self) -> &str {
        &self.source_schema_sha256
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupRestorePrerequisiteInspection {
    pub fixed_root_owned_binaries_ready: bool,
    pub recipient_digest_ready: bool,
    pub runtime_identity_digest_ready: bool,
    pub source_server_version: Option<String>,
    pub source_server_version_comment: Option<String>,
    pub source_mysql8_supported: bool,
    pub command_limits_valid: bool,
    pub estimated_source_bytes: Option<u64>,
    pub maximum_encrypted_backup_bytes: u64,
    pub output_available_bytes: Option<u64>,
    pub output_capacity_ready: bool,
    pub restore_admin_connected: bool,
    pub restore_server_version: Option<String>,
    pub restore_server_version_comment: Option<String>,
    pub restore_server_supported: bool,
    pub restore_identity_distinct: bool,
    pub restore_database_absent_or_empty: bool,
    pub restore_create_drop_privileges_observed: bool,
    pub blockers: Vec<String>,
}

impl BackupRestorePrerequisiteInspection {
    pub fn ready(&self) -> bool {
        self.blockers.is_empty()
    }
}

impl VerifiedBackupArchive {
    pub fn backup_reference(&self) -> &str {
        &self.backup_reference
    }

    pub fn encrypted_backup_sha256(&self) -> &str {
        &self.encrypted_backup_sha256
    }

    pub fn source_fingerprint_sha256(&self) -> &str {
        &self.source_fingerprint_sha256
    }

    pub const fn archive_format_version(&self) -> u32 {
        self.archive_format_version
    }

    pub fn traffic_receipt_sha256(&self) -> &str {
        &self.traffic_receipt_sha256
    }

    pub const fn traffic_receipt_bytes(&self) -> u64 {
        self.traffic_receipt_bytes
    }

    pub const fn traffic_sorted_user_delta_count(&self) -> u64 {
        self.traffic_sorted_user_delta_count
    }

    pub fn traffic_sorted_user_delta_sha256(&self) -> &str {
        &self.traffic_sorted_user_delta_sha256
    }

    pub fn traffic_upload_delta_sum(&self) -> &str {
        &self.traffic_upload_delta_sum
    }

    pub fn traffic_download_delta_sum(&self) -> &str {
        &self.traffic_download_delta_sum
    }

    pub const fn encrypted_backup_bytes(&self) -> u64 {
        self.encrypted_backup_bytes
    }

    pub fn receipt_sha256(&self) -> &str {
        &self.receipt_sha256
    }
}

struct SecretScratch {
    path: PathBuf,
}

impl SecretScratch {
    fn create(path: PathBuf, expected: &mut Vec<u8>) -> Result<Self, BackupError> {
        ensure_private_parent(&path)?;
        if fs::symlink_metadata(&path).is_ok() {
            let existing = read_owner_only_limited(&path, MAX_CONTROL_FILE_BYTES)?;
            if existing != *expected {
                expected.fill(0);
                return Err(BackupError::Conflict);
            }
            remove_secure_file(&path)?;
        }
        let result = write_create_new_fsync(&path, expected);
        expected.fill(0);
        result?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn remove(mut self) -> Result<(), BackupError> {
        remove_secure_file(&self.path)?;
        self.path = PathBuf::new();
        Ok(())
    }
}

impl Drop for SecretScratch {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = remove_secure_file(&self.path);
        }
    }
}

/// Performs every non-mutating backup/restore prerequisite check before the
/// operator's single irreversible authorization. A failed probe is represented
/// as a bounded blocker instead of being deferred until the source is fenced.
pub async fn inspect_backup_restore_prerequisites(
    spec: &ProvisionSpec,
) -> BackupRestorePrerequisiteInspection {
    let Some(execution) = spec.legacy_apply_execution() else {
        return BackupRestorePrerequisiteInspection {
            fixed_root_owned_binaries_ready: false,
            recipient_digest_ready: false,
            runtime_identity_digest_ready: false,
            source_server_version: None,
            source_server_version_comment: None,
            source_mysql8_supported: false,
            command_limits_valid: false,
            estimated_source_bytes: None,
            maximum_encrypted_backup_bytes: 0,
            output_available_bytes: None,
            output_capacity_ready: false,
            restore_admin_connected: false,
            restore_server_version: None,
            restore_server_version_comment: None,
            restore_server_supported: false,
            restore_identity_distinct: false,
            restore_database_absent_or_empty: false,
            restore_create_drop_privileges_observed: false,
            blockers: vec!["legacy backup execution policy is unavailable".into()],
        };
    };
    let policy = &execution.backup;
    let mut inspection = BackupRestorePrerequisiteInspection {
        fixed_root_owned_binaries_ready: false,
        recipient_digest_ready: false,
        runtime_identity_digest_ready: false,
        source_server_version: None,
        source_server_version_comment: None,
        source_mysql8_supported: false,
        command_limits_valid: (300..=604_800).contains(&policy.command_timeout_seconds)
            && (16 * 1024 * 1024..=16 * 1024_u64.pow(4))
                .contains(&policy.maximum_encrypted_backup_bytes),
        estimated_source_bytes: None,
        maximum_encrypted_backup_bytes: policy.maximum_encrypted_backup_bytes,
        output_available_bytes: None,
        output_capacity_ready: false,
        restore_admin_connected: false,
        restore_server_version: None,
        restore_server_version_comment: None,
        restore_server_supported: false,
        restore_identity_distinct: false,
        restore_database_absent_or_empty: false,
        restore_create_drop_privileges_observed: false,
        blockers: Vec::new(),
    };
    if Toolchain::production(
        Duration::from_secs(policy.command_timeout_seconds),
        policy.maximum_encrypted_backup_bytes,
    )
    .is_ok()
    {
        inspection.fixed_root_owned_binaries_ready = true;
    } else {
        inspection
            .blockers
            .push("backup fixed root-owned age/MySQL 8/df binaries are unavailable".into());
    }
    inspection.recipient_digest_ready = verify_bound_input(
        &policy.encryption_recipient_path,
        &policy.encryption_recipient_sha256,
    )
    .is_ok();
    if !inspection.recipient_digest_ready {
        inspection
            .blockers
            .push("backup age recipient file is missing, unsafe, or digest-mismatched".into());
    }
    inspection.runtime_identity_digest_ready = verify_bound_input(
        &policy.decryption_identity_path,
        &policy.decryption_identity_sha256,
    )
    .is_ok();
    if !inspection.runtime_identity_digest_ready {
        inspection.blockers.push(
            "backup age identity is not mounted owner-only at the bound runtime-secret path".into(),
        );
    }
    if !inspection.command_limits_valid {
        inspection
            .blockers
            .push("backup timeout or encrypted byte ceiling is outside the frozen bounds".into());
    }
    if let Ok(available) = output_available_bytes(&policy.encrypted_backup_output_path) {
        inspection.output_available_bytes = Some(available);
        inspection.output_capacity_ready = available
            >= policy
                .maximum_encrypted_backup_bytes
                .saturating_add(1024 * 1024);
    }
    if !inspection.output_capacity_ready {
        inspection.blockers.push(
            "backup output filesystem free space is below the manifest-bound streaming ceiling"
                .into(),
        );
    }

    let source = match legacy_source(spec) {
        Ok(source) => source,
        Err(_) => {
            inspection
                .blockers
                .push("legacy source backup identity is unavailable".into());
            return inspection;
        }
    };
    let source_endpoint = ClientEndpoint::parse(&source.database_url).ok();
    let restore_endpoint = ClientEndpoint::parse(&policy.isolated_restore_database_url).ok();
    inspection.restore_identity_distinct = source_endpoint
        .as_ref()
        .zip(restore_endpoint.as_ref())
        .is_some_and(|(source, restore)| source.redacted_identity() != restore.redacted_identity());
    if !inspection.restore_identity_distinct {
        inspection
            .blockers
            .push("isolated restore identity aliases the legacy source".into());
    }

    if let Ok(identity) = mysql_server_identity(&source.database_url).await {
        inspection.source_mysql8_supported = verify_mysql8_server_identity(&identity).is_ok();
        inspection.source_server_version = Some(identity.version);
        inspection.source_server_version_comment = Some(identity.version_comment);
    }
    if !inspection.source_mysql8_supported {
        inspection
            .blockers
            .push("legacy source must be reachable Oracle MySQL 8.0 or 8.4".into());
    }
    match estimate_source_bytes(&source.database_url).await {
        Ok(estimated) => inspection.estimated_source_bytes = Some(estimated),
        Err(_) => inspection
            .blockers
            .push("legacy source byte estimate could not be read".into()),
    }
    if inspection
        .estimated_source_bytes
        .is_some_and(|estimated| estimated > policy.maximum_encrypted_backup_bytes)
    {
        inspection
            .blockers
            .push("legacy source estimate exceeds maximum_encrypted_backup_bytes".into());
    }

    if let Ok(admin) = inspect_restore_admin(&policy.isolated_restore_database_url).await {
        inspection.restore_admin_connected = true;
        inspection.restore_server_supported =
            verify_mysql8_server_identity(&admin.identity).is_ok();
        inspection.restore_server_version = Some(admin.identity.version);
        inspection.restore_server_version_comment = Some(admin.identity.version_comment);
        inspection.restore_database_absent_or_empty = admin.database_absent_or_empty;
        inspection.restore_create_drop_privileges_observed = admin.create_drop_privileges;
    }
    if !inspection.restore_admin_connected || !inspection.restore_server_supported {
        inspection
            .blockers
            .push("isolated restore admin endpoint is unreachable or unsupported".into());
    }
    if !inspection.restore_database_absent_or_empty {
        inspection
            .blockers
            .push("isolated restore database exists and is not empty".into());
    }
    if !inspection.restore_create_drop_privileges_observed {
        inspection.blockers.push(
            "isolated restore admin lacks observable CREATE and DROP database privileges".into(),
        );
    }
    inspection
}

struct RestoreAdminInspection {
    identity: MysqlServerIdentity,
    database_absent_or_empty: bool,
    create_drop_privileges: bool,
}

async fn inspect_restore_admin(url: &str) -> Result<RestoreAdminInspection, BackupError> {
    let endpoint = ClientEndpoint::parse(url)?;
    let pool = admin_pool(url).await?;
    let (version, version_comment) =
        sqlx::query_as::<_, (String, String)>("SELECT VERSION(), @@version_comment")
            .fetch_one(&pool)
            .await
            .map_err(|_| BackupError::Database)?;
    let identity = MysqlServerIdentity {
        version,
        version_comment,
    };
    validate_server_identity_text(&identity)?;
    let absent = !database_exists(&pool, &endpoint.database).await?;
    let empty = absent || object_count(&pool, &endpoint.database).await? == 0;
    let grants = sqlx::query_as::<_, (i64, i64)>(
        "SELECT \
           EXISTS(SELECT 1 FROM information_schema.USER_PRIVILEGES \
                  WHERE REPLACE(GRANTEE, CHAR(39), '') = CURRENT_USER() \
                    AND PRIVILEGE_TYPE = 'CREATE') \
           OR EXISTS(SELECT 1 FROM information_schema.SCHEMA_PRIVILEGES \
                     WHERE REPLACE(GRANTEE, CHAR(39), '') = CURRENT_USER() AND TABLE_SCHEMA = ? \
                       AND PRIVILEGE_TYPE = 'CREATE') AS can_create, \
           EXISTS(SELECT 1 FROM information_schema.USER_PRIVILEGES \
                  WHERE REPLACE(GRANTEE, CHAR(39), '') = CURRENT_USER() \
                    AND PRIVILEGE_TYPE = 'DROP') \
           OR EXISTS(SELECT 1 FROM information_schema.SCHEMA_PRIVILEGES \
                     WHERE REPLACE(GRANTEE, CHAR(39), '') = CURRENT_USER() AND TABLE_SCHEMA = ? \
                       AND PRIVILEGE_TYPE = 'DROP') AS can_drop",
    )
    .bind(&endpoint.database)
    .bind(&endpoint.database)
    .fetch_one(&pool)
    .await
    .map_err(|_| BackupError::Database)?;
    pool.close().await;
    Ok(RestoreAdminInspection {
        identity,
        database_absent_or_empty: empty,
        create_drop_privileges: grants.0 == 1 && grants.1 == 1,
    })
}

async fn estimate_source_bytes(url: &str) -> Result<u64, BackupError> {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(RESTORE_DATABASE_TIMEOUT)
        .connect(url)
        .await
        .map_err(|_| BackupError::Database)?;
    let value = sqlx::query_scalar::<_, String>(
        "SELECT CAST(COALESCE(SUM(DATA_LENGTH + INDEX_LENGTH), 0) AS CHAR) \
         FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE()",
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| BackupError::Database)?;
    pool.close().await;
    value.parse::<u64>().map_err(|_| BackupError::Database)
}

fn original_source_drained_snapshot(
    spec: &ProvisionSpec,
    current: &ApplyJournalSnapshot,
) -> Result<ApplyJournalSnapshot, BackupError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let (journal, latest) = ApplyJournal::open(&execution.journal.root, current.binding().clone())
        .map_err(|_| BackupError::ReceiptInvalid)?;
    if latest != *current {
        return Err(BackupError::ReceiptInvalid);
    }
    journal
        .verified_history()
        .map_err(|_| BackupError::ReceiptInvalid)?
        .into_iter()
        .find(|snapshot| {
            snapshot.checkpoint() == ApplyCheckpoint::SourceDrained
                && snapshot.outcome_code().is_none()
                && snapshot.checkpoint_proof_sha256().is_some()
        })
        .ok_or(BackupError::ReceiptInvalid)
}

fn current_traffic_archive_input(
    spec: &ProvisionSpec,
) -> Result<TrafficReceiptArchiveInput, BackupError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let receipt = verify_frozen_traffic_receipt(spec).map_err(|_| BackupError::ReceiptInvalid)?;
    let bytes = read_owner_only_limited(
        &execution.receipts.source_drain_path,
        MAX_TRAFFIC_RECEIPT_BYTES,
    )?;
    if receipt.operation_id != spec.operation_id {
        return Err(BackupError::ReceiptInvalid);
    }
    TrafficReceiptArchiveInput::from_verified(bytes, &receipt)
}

pub(crate) async fn perform_backup_restore(
    spec: &ProvisionSpec,
    head: &ApplyJournalSnapshot,
) -> Result<BackupRestoreProof, BackupError> {
    let source = legacy_source(spec)?;
    let strategy = legacy_conversion_strategy(spec)?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let policy = &execution.backup;
    let source_drained = original_source_drained_snapshot(spec, head)?;
    let source_drained_proof = source_drained_proof(&source_drained)?.to_string();
    let timeout = Duration::from_secs(policy.command_timeout_seconds);
    let source_endpoint = ClientEndpoint::parse(&source.database_url)?;
    let restore_endpoint = ClientEndpoint::parse(&policy.isolated_restore_database_url)?;
    let recipient_sha256 = verify_bound_input(
        &policy.encryption_recipient_path,
        &policy.encryption_recipient_sha256,
    )?;
    let decryption_identity_sha256 = verify_bound_input(
        &policy.decryption_identity_path,
        &policy.decryption_identity_sha256,
    )?;
    let toolchain = Toolchain::production(timeout, policy.maximum_encrypted_backup_bytes)?;
    if matches!(
        fs::symlink_metadata(&policy.encrypted_backup_output_path),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    ) {
        require_output_capacity(
            &policy.encrypted_backup_output_path,
            policy.maximum_encrypted_backup_bytes,
        )?;
    }
    if fs::symlink_metadata(&execution.receipts.backup_restore_path).is_ok() {
        // Lost acknowledgement after the complete HMAC receipt was published
        // must not require the old MySQL endpoint to come back. The verified
        // archive/state already bind the original SourceDrained proof and all
        // three equal fingerprints.
        let archive = verify_persisted_backup_archive(spec)?;
        let state = read_restore_state(spec, &policy.isolated_restore_state_path)?
            .ok_or(BackupError::ReceiptInvalid)?;
        if state.source_drained_proof_sha256 != source_drained_proof
            || archive.receipt_sha256().is_empty()
        {
            return Err(BackupError::ReceiptInvalid);
        }
        return BackupRestoreProof::new(archive.receipt_sha256(), archive.backup_reference())
            .map_err(|_| BackupError::ReceiptInvalid);
    }
    let source_server_identity = mysql_server_identity(&source.database_url).await?;
    verify_mysql8_server_identity(&source_server_identity)?;
    let restore_server_identity =
        mysql_server_identity(&admin_url(&policy.isolated_restore_database_url)?).await?;
    verify_mysql8_server_identity(&restore_server_identity)?;
    let source_fingerprint = fingerprint_mysql_for_strategy(&source.database_url, strategy)
        .await
        .map_err(|_| BackupError::Database)?;
    let source_identity_sha256 = domain_hash(
        b"v2board-legacy-backup-source-identity-v1\0",
        source_endpoint.redacted_identity().as_bytes(),
    );
    let restore_identity_sha256 = domain_hash(
        b"v2board-legacy-backup-restore-identity-v1\0",
        restore_endpoint.redacted_identity().as_bytes(),
    );
    let now = unix_now()?;
    let state_path = &policy.isolated_restore_state_path;
    let receipt_path = &execution.receipts.backup_restore_path;
    let artifact_path = &policy.encrypted_backup_output_path;
    let partial_path = partial_artifact_path(artifact_path, &spec.operation_id)?;
    let receipt_partial_path = sibling(receipt_path, ".receipt.partial")?;
    reconcile_hardlink_commit(&partial_path, artifact_path)?;
    reconcile_hardlink_commit(&receipt_partial_path, receipt_path)?;
    let artifact_present = match fs::symlink_metadata(artifact_path) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => return Err(BackupError::Filesystem),
    };
    let existing_state = read_restore_state(spec, state_path)?;
    let traffic = match fs::symlink_metadata(&execution.receipts.source_drain_path) {
        Ok(_) => Some(current_traffic_archive_input(spec)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => return Err(BackupError::Filesystem),
    };
    if !artifact_present {
        let current_traffic = traffic.as_ref().ok_or(BackupError::ReceiptInvalid)?;
        current_traffic.require_exact_initial_source_drained_head(&source_drained)?;
        verify_redis_fence_for_backup(spec, &source_drained)
            .await
            .map_err(|_| BackupError::ReceiptInvalid)?;
    } else if traffic.is_none() && fs::symlink_metadata(receipt_path).is_err() {
        // The plaintext source fact is removed only after the HMAC backup
        // receipt is durable. Before that point it remains mandatory for a
        // restore-drill retry.
        return Err(BackupError::ReceiptInvalid);
    }
    let traffic_state = traffic
        .as_ref()
        .map(|traffic| {
            (
                traffic.sha256.clone(),
                traffic.byte_len(),
                traffic.sorted_user_delta_count,
                traffic.sorted_user_delta_sha256.clone(),
                traffic.upload_delta_sum.clone(),
                traffic.download_delta_sum.clone(),
            )
        })
        .or_else(|| {
            existing_state.as_ref().map(|state| {
                (
                    state.traffic_receipt_sha256.clone(),
                    state.traffic_receipt_bytes,
                    state.traffic_sorted_user_delta_count,
                    state.traffic_sorted_user_delta_sha256.clone(),
                    state.traffic_upload_delta_sum.clone(),
                    state.traffic_download_delta_sum.clone(),
                )
            })
        })
        .ok_or(BackupError::ReceiptInvalid)?;
    let expected_base = RestoreStatePayload {
        document_kind: "legacy_backup_restore_state".into(),
        schema_version: BACKUP_STATE_SCHEMA_VERSION,
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        backup_reference: policy.backup_reference.clone(),
        source_identity_sha256,
        restore_identity_sha256,
        recipient_sha256,
        decryption_identity_sha256,
        initial_source_fingerprint_sha256: source_fingerprint.clone(),
        source_drained_generation: source_drained.generation(),
        source_drained_event_sha256: source_drained.event_sha256().to_string(),
        source_drained_proof_sha256: source_drained_proof,
        archive_format_version: LEGACY_ARCHIVE_FORMAT_VERSION,
        traffic_receipt_sha256: traffic_state.0,
        traffic_receipt_bytes: traffic_state.1,
        traffic_sorted_user_delta_count: traffic_state.2,
        traffic_sorted_user_delta_sha256: traffic_state.3,
        traffic_upload_delta_sum: traffic_state.4,
        traffic_download_delta_sum: traffic_state.5,
        traffic_redis_physically_verified_before_archive: true,
        started_at_unix: now,
        phase: RestorePhase::Reserved,
        encrypted_backup_sha256: None,
        encrypted_backup_bytes: None,
    };
    let mut state = match existing_state {
        Some(existing) => {
            verify_state_base(&existing, &expected_base)?;
            verify_state_phase(&existing)?;
            verify_state_anchor(&existing, &source_drained)?;
            if existing.initial_source_fingerprint_sha256 != source_fingerprint {
                cleanup_owned_restore(
                    &policy.isolated_restore_database_url,
                    &restore_endpoint.database,
                )
                .await?;
                return Err(BackupError::SourceDrift);
            }
            existing
        }
        None => {
            if fs::symlink_metadata(artifact_path).is_ok()
                || fs::symlink_metadata(receipt_path).is_ok()
                || fs::symlink_metadata(&partial_path).is_ok()
                || fs::symlink_metadata(&receipt_partial_path).is_ok()
            {
                return Err(BackupError::Conflict);
            }
            require_absent_or_empty_restore(
                &policy.isolated_restore_database_url,
                &restore_endpoint.database,
            )
            .await?;
            persist_restore_state(spec, state_path, &expected_base)?;
            expected_base
        }
    };

    if fs::symlink_metadata(receipt_path).is_err()
        && fs::symlink_metadata(&receipt_partial_path).is_ok()
    {
        let artifact = hash_owner_only_file(artifact_path, policy.maximum_encrypted_backup_bytes)?;
        recover_verified_receipt_partial(&receipt_partial_path, receipt_path, |partial_path| {
            verify_existing_receipt(
                spec,
                partial_path,
                &state,
                &artifact,
                head,
                &source_server_identity,
            )
            .map(|_| ())
        })?;
    }

    if fs::symlink_metadata(receipt_path).is_ok() {
        cleanup_owned_restore(
            &policy.isolated_restore_database_url,
            &restore_endpoint.database,
        )
        .await?;
        let artifact = hash_owner_only_file(artifact_path, policy.maximum_encrypted_backup_bytes)?;
        let receipt = verify_existing_receipt(
            spec,
            receipt_path,
            &state,
            &artifact,
            head,
            &source_server_identity,
        )?;
        remove_verified_frozen_traffic_receipt(spec, &receipt.traffic_receipt_sha256)
            .map_err(|_| BackupError::Filesystem)?;
        return BackupRestoreProof::new(
            hex::encode(Sha256::digest(read_owner_only_limited(
                receipt_path,
                MAX_CONTROL_FILE_BYTES,
            )?)),
            receipt.backup_reference,
        )
        .map_err(|_| BackupError::ReceiptInvalid);
    }

    let traffic = traffic.as_ref().ok_or(BackupError::ReceiptInvalid)?;

    cleanup_owned_restore(
        &policy.isolated_restore_database_url,
        &restore_endpoint.database,
    )
    .await?;

    let artifact = if fs::symlink_metadata(artifact_path).is_ok() {
        hash_owner_only_file(artifact_path, policy.maximum_encrypted_backup_bytes)?
    } else {
        let source_defaults_path = scratch_path(artifact_path, &spec.operation_id, "source")?;
        let mut defaults = source_endpoint.defaults_bytes();
        let scratch = SecretScratch::create(source_defaults_path, &mut defaults)?;
        let toolchain_for_dump = toolchain.clone();
        let endpoint_for_dump = source_endpoint.clone();
        let defaults_path = scratch.path().to_path_buf();
        let recipient_path = policy.encryption_recipient_path.clone();
        let traffic_for_dump = traffic.clone();
        let partial_for_dump = partial_path.clone();
        let (produced, frame) = tokio::task::spawn_blocking(move || {
            toolchain_for_dump.create_encrypted_dump(
                &endpoint_for_dump,
                &defaults_path,
                &recipient_path,
                &traffic_for_dump,
                &partial_for_dump,
            )
        })
        .await
        .map_err(|_| BackupError::DumpFailed)??;
        scratch.remove()?;
        let after_dump = fingerprint_mysql_for_strategy(&source.database_url, strategy)
            .await
            .map_err(|_| BackupError::Database)?;
        if after_dump != source_fingerprint {
            remove_secure_file(&partial_path)?;
            return Err(BackupError::SourceDrift);
        }
        if frame.format_version != LEGACY_ARCHIVE_FORMAT_VERSION
            || frame.traffic_receipt_sha256 != traffic.sha256
            || frame.traffic_receipt_bytes != traffic.byte_len()
            || frame.mysql_dump_bytes == 0
        {
            remove_secure_file(&partial_path)?;
            return Err(BackupError::DumpFailed);
        }
        commit_no_clobber(&partial_path, artifact_path)?;
        let committed = hash_owner_only_file(artifact_path, policy.maximum_encrypted_backup_bytes)?;
        if committed != produced {
            return Err(BackupError::Conflict);
        }
        committed
    };
    if state
        .encrypted_backup_sha256
        .as_ref()
        .is_some_and(|expected| expected != &artifact.sha256)
        || state
            .encrypted_backup_bytes
            .is_some_and(|expected| expected != artifact.bytes)
    {
        return Err(BackupError::Conflict);
    }
    if state.phase == RestorePhase::Reserved {
        state.phase = RestorePhase::DumpCommitted;
        state.encrypted_backup_sha256 = Some(artifact.sha256.clone());
        state.encrypted_backup_bytes = Some(artifact.bytes);
        persist_restore_state(spec, state_path, &state)?;
    }

    if state.phase == RestorePhase::DumpCommitted {
        state.phase = RestorePhase::RestoreInProgress;
        persist_restore_state(spec, state_path, &state)?;
    }
    let restore_defaults_path = scratch_path(artifact_path, &spec.operation_id, "restore")?;
    let mut defaults = restore_endpoint.defaults_bytes();
    let scratch = SecretScratch::create(restore_defaults_path, &mut defaults)?;
    if let Err(error) = create_empty_restore_database(
        &policy.isolated_restore_database_url,
        &restore_endpoint.database,
    )
    .await
    {
        let cleanup = cleanup_owned_restore(
            &policy.isolated_restore_database_url,
            &restore_endpoint.database,
        )
        .await;
        scratch.remove()?;
        cleanup?;
        return Err(error);
    }
    let toolchain_for_restore = toolchain.clone();
    let endpoint_for_restore = restore_endpoint.clone();
    let defaults_path = scratch.path().to_path_buf();
    let identity_path = policy.decryption_identity_path.clone();
    let artifact_for_restore = artifact_path.clone();
    let traffic_for_restore = traffic.clone();
    let restore_result = match tokio::task::spawn_blocking(move || {
        toolchain_for_restore.restore_encrypted_dump(
            &endpoint_for_restore,
            &defaults_path,
            &identity_path,
            &artifact_for_restore,
            &traffic_for_restore,
        )
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(BackupError::RestoreFailed),
    };
    let scratch_cleanup = scratch.remove();
    let verification: Result<(String, String, ArchiveFrameVerification), BackupError> =
        match restore_result {
            Ok(frame)
                if frame.format_version == LEGACY_ARCHIVE_FORMAT_VERSION
                    && frame.traffic_receipt_sha256 == traffic.sha256
                    && frame.traffic_receipt_bytes == traffic.byte_len()
                    && frame.mysql_dump_bytes > 0 =>
            {
                async {
                    let restored = fingerprint_mysql_for_strategy(
                        &policy.isolated_restore_database_url,
                        strategy,
                    )
                    .await
                    .map_err(|_| BackupError::RestoreFailed)?;
                    let source_after =
                        fingerprint_mysql_for_strategy(&source.database_url, strategy)
                            .await
                            .map_err(|_| BackupError::Database)?;
                    if source_after != source_fingerprint {
                        Err(BackupError::SourceDrift)
                    } else if restored != source_fingerprint {
                        Err(BackupError::RestoreMismatch)
                    } else {
                        Ok((restored, source_after, frame))
                    }
                }
                .await
            }
            Ok(_) => Err(BackupError::RestoreFailed),
            Err(error) => Err(error),
        };
    let cleanup = cleanup_owned_restore(
        &policy.isolated_restore_database_url,
        &restore_endpoint.database,
    )
    .await;
    scratch_cleanup?;
    cleanup?;
    let (restored_fingerprint, source_after, restored_frame) = verification?;
    if state.phase != RestorePhase::Destroyed {
        state.phase = RestorePhase::Destroyed;
        persist_restore_state(spec, state_path, &state)?;
    }

    let receipt_payload = BackupReceiptPayload {
        document_kind: "legacy_backup_restore_receipt".into(),
        schema_version: BACKUP_RECEIPT_SCHEMA_VERSION,
        operation_id: spec.operation_id.clone(),
        source_drained_generation: state.source_drained_generation,
        source_drained_event_sha256: state.source_drained_event_sha256.clone(),
        source_drained_proof_sha256: state.source_drained_proof_sha256.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        backup_reference: policy.backup_reference.clone(),
        source_server_identity_sha256: mysql_server_identity_sha256(&source_server_identity),
        source_identity_sha256: state.source_identity_sha256.clone(),
        restore_identity_sha256: state.restore_identity_sha256.clone(),
        source_fingerprint_before_sha256: source_fingerprint,
        source_fingerprint_after_sha256: source_after,
        restored_fingerprint_sha256: restored_fingerprint,
        archive_format_version: restored_frame.format_version,
        traffic_receipt_sha256: restored_frame.traffic_receipt_sha256,
        traffic_receipt_bytes: restored_frame.traffic_receipt_bytes,
        traffic_sorted_user_delta_count: traffic.sorted_user_delta_count,
        traffic_sorted_user_delta_sha256: traffic.sorted_user_delta_sha256.clone(),
        traffic_upload_delta_sum: traffic.upload_delta_sum.clone(),
        traffic_download_delta_sum: traffic.download_delta_sum.clone(),
        traffic_redis_physically_verified_before_archive: true,
        traffic_receipt_hmac_verified: true,
        traffic_receipt_restored_from_encrypted_archive: true,
        mysql_dump_bytes_restored: restored_frame.mysql_dump_bytes,
        encrypted_backup_sha256: artifact.sha256,
        encrypted_backup_bytes: artifact.bytes,
        recipient_sha256: state.recipient_sha256.clone(),
        decryption_identity_sha256: state.decryption_identity_sha256.clone(),
        actual_encrypted_bytes_decrypted: true,
        isolated_restore_destroyed: true,
        started_at_unix: state.started_at_unix,
        completed_at_unix: unix_now()?,
    };
    let receipt_bytes = receipt_envelope_bytes(spec, receipt_payload)?;
    write_immutable_output(receipt_path, &receipt_bytes)?;
    let report_sha256 = hex::encode(Sha256::digest(&receipt_bytes));
    remove_verified_frozen_traffic_receipt(spec, &state.traffic_receipt_sha256)
        .map_err(|_| BackupError::Filesystem)?;
    BackupRestoreProof::new(report_sha256, &policy.backup_reference)
        .map_err(|_| BackupError::ReceiptInvalid)
}

/// Re-verifies the sole permanent legacy archive without replaying the restore
/// drill. Completion may bind this result only after the drill's HMAC receipt
/// says the exact encrypted bytes were decrypted, fingerprinted, and the
/// operation-owned isolated database was destroyed.
pub fn verify_persisted_backup_archive(
    spec: &ProvisionSpec,
) -> Result<VerifiedBackupArchive, BackupError> {
    verify_persisted_backup_archive_inner(spec, true)
}

/// Decrypts the sole archive into bounded memory, validates its framing and
/// embedded source-drain receipt with the original scoped HMAC parser, and
/// discards (never imports or materializes) the MySQL frame. This is the resume
/// source for the copy stage after the already-validated legacy Redis instance
/// becomes unavailable.
pub async fn load_verified_traffic_receipt_from_backup_archive(
    spec: &ProvisionSpec,
) -> Result<VerifiedFrozenTrafficReceipt, BackupError> {
    let archive = verify_persisted_backup_archive(spec)?;
    load_verified_traffic_archive_input(spec, &archive)
        .await
        .map(|(_, receipt)| receipt)
}

async fn load_verified_traffic_archive_input(
    spec: &ProvisionSpec,
    archive: &VerifiedBackupArchive,
) -> Result<(TrafficReceiptArchiveInput, VerifiedFrozenTrafficReceipt), BackupError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let policy = &execution.backup;
    let toolchain = Toolchain::production(
        Duration::from_secs(policy.command_timeout_seconds),
        policy.maximum_encrypted_backup_bytes,
    )?;
    let identity_path = policy.decryption_identity_path.clone();
    let artifact_path = policy.encrypted_backup_output_path.clone();
    let (bytes, frame) = tokio::task::spawn_blocking(move || {
        toolchain.decrypt_verified_traffic_frame(&identity_path, &artifact_path)
    })
    .await
    .map_err(|_| BackupError::RestoreFailed)??;
    if frame.format_version != archive.archive_format_version()
        || frame.traffic_receipt_sha256 != archive.traffic_receipt_sha256()
        || frame.traffic_receipt_bytes != archive.traffic_receipt_bytes()
        || frame.mysql_dump_bytes == 0
    {
        return Err(BackupError::ReceiptInvalid);
    }
    let receipt = verify_frozen_traffic_receipt_bytes(spec, &bytes)
        .map_err(|_| BackupError::ReceiptInvalid)?;
    if receipt.operation_id != spec.operation_id
        || receipt.receipt_sha256 != archive.traffic_receipt_sha256()
        || receipt.sorted_user_delta_count != archive.traffic_sorted_user_delta_count()
        || receipt.sorted_user_delta_sha256 != archive.traffic_sorted_user_delta_sha256()
        || receipt.upload_delta_sum != archive.traffic_upload_delta_sum()
        || receipt.download_delta_sum != archive.traffic_download_delta_sum()
    {
        return Err(BackupError::ReceiptInvalid);
    }
    let input = TrafficReceiptArchiveInput::from_verified(bytes, &receipt)?;
    Ok((input, receipt))
}

/// Reconstructs and continuously re-verifies the operation-owned recovery
/// database from the sole encrypted archive. The database remains present
/// through the last pre-authority value verification; it is never a runtime
/// fallback and accepts no writes outside this restore routine.
pub(crate) async fn ensure_verified_archive_materialization(
    spec: &ProvisionSpec,
    anchor: ArchiveMaterializationAnchor<'_>,
) -> Result<VerifiedArchiveMaterialization, BackupError> {
    let archive = verify_persisted_backup_archive(spec)?;
    let strategy = legacy_conversion_strategy(spec)?;
    verify_materialization_anchor(spec, &archive, anchor)?;
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let policy = &execution.backup;
    let endpoint = ClientEndpoint::parse(&policy.isolated_restore_database_url)?;
    let state_path = archive_materialization_state_path(policy)?;
    let expected = expected_materialization_state(spec, &archive, &endpoint)?;
    let mut state = match read_archive_materialization_state(spec, &state_path)? {
        Some(state) => {
            verify_archive_materialization_state_base(&state, &expected)?;
            verify_archive_materialization_state_shape(&state)?;
            state
        }
        None => {
            require_absent_or_empty_restore(
                &policy.isolated_restore_database_url,
                &endpoint.database,
            )
            .await?;
            persist_archive_materialization_state(spec, &state_path, &expected)?;
            expected
        }
    };

    if matches!(
        state.phase,
        ArchiveMaterializationPhase::Destroying | ArchiveMaterializationPhase::Destroyed
    ) {
        return Err(BackupError::Conflict);
    }
    if state.phase == ArchiveMaterializationPhase::Reserved {
        state.phase = ArchiveMaterializationPhase::RestoreInProgress;
        persist_archive_materialization_state(spec, &state_path, &state)?;
    }

    let database_present =
        restore_database_exists(&policy.isolated_restore_database_url, &endpoint.database).await?;
    let exact = if database_present {
        match fingerprint_mysql_and_schema_for_strategy(
            &policy.isolated_restore_database_url,
            strategy,
        )
        .await
        {
            Ok((fingerprint, schema))
                if fingerprint == archive.source_fingerprint_sha256()
                    && schema == LEGACY_SEMANTIC_SCHEMA_SHA256 =>
            {
                true
            }
            Ok(_) if state.phase == ArchiveMaterializationPhase::Ready => {
                // A ready database changing underneath the lifecycle process
                // is target drift, not permission to drop arbitrary data.
                return Err(BackupError::Conflict);
            }
            Ok(_) | Err(_) => false,
        }
    } else {
        false
    };

    if !exact {
        if state.phase == ArchiveMaterializationPhase::Ready && database_present {
            return Err(BackupError::Conflict);
        }
        restore_archive_materialization(spec, &archive, &endpoint).await?;
    }
    let (fingerprint, schema) =
        fingerprint_mysql_and_schema_for_strategy(&policy.isolated_restore_database_url, strategy)
            .await
            .map_err(|_| BackupError::RestoreMismatch)?;
    if fingerprint != archive.source_fingerprint_sha256() || schema != LEGACY_SEMANTIC_SCHEMA_SHA256
    {
        return Err(BackupError::RestoreMismatch);
    }
    if state.phase != ArchiveMaterializationPhase::Ready {
        state.phase = ArchiveMaterializationPhase::Ready;
        state.materialized_fingerprint_sha256 = Some(fingerprint.clone());
        state.materialized_schema_sha256 = Some(schema.clone());
        persist_archive_materialization_state(spec, &state_path, &state)?;
    } else if state.materialized_fingerprint_sha256.as_deref() != Some(fingerprint.as_str())
        || state.materialized_schema_sha256.as_deref() != Some(schema.as_str())
    {
        return Err(BackupError::Conflict);
    }
    Ok(VerifiedArchiveMaterialization {
        database_url: policy.isolated_restore_database_url.clone(),
        source_fingerprint_sha256: fingerprint,
        source_schema_sha256: schema,
    })
}

/// Destroys the archive materialization only after the native-authority event
/// itself is fsync-durable. `Destroying` is persisted before DROP so a lost
/// acknowledgement can only resume cleanup, never recreate a pre-authority
/// source after authority has changed.
pub(crate) async fn destroy_verified_archive_materialization_after_authority(
    spec: &ProvisionSpec,
    permit: &DurableNativeStartPermit,
) -> Result<(), BackupError> {
    if permit.operation_id() != spec.operation_id {
        return Err(BackupError::Conflict);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let policy = &execution.backup;
    let endpoint = ClientEndpoint::parse(&policy.isolated_restore_database_url)?;
    let archive = verify_persisted_backup_archive(spec)?;
    let expected = expected_materialization_state(spec, &archive, &endpoint)?;
    let state_path = archive_materialization_state_path(policy)?;
    let mut state = read_archive_materialization_state(spec, &state_path)?
        .ok_or(BackupError::ReceiptInvalid)?;
    verify_archive_materialization_state_base(&state, &expected)?;
    verify_archive_materialization_state_shape(&state)?;
    match state.phase {
        ArchiveMaterializationPhase::Ready => {
            state.phase = ArchiveMaterializationPhase::Destroying;
            persist_archive_materialization_state(spec, &state_path, &state)?;
        }
        ArchiveMaterializationPhase::Destroying => {}
        ArchiveMaterializationPhase::Destroyed => {
            if restore_database_exists(&policy.isolated_restore_database_url, &endpoint.database)
                .await?
            {
                return Err(BackupError::Conflict);
            }
            return Ok(());
        }
        ArchiveMaterializationPhase::Reserved | ArchiveMaterializationPhase::RestoreInProgress => {
            return Err(BackupError::Conflict);
        }
    }
    cleanup_owned_restore(&policy.isolated_restore_database_url, &endpoint.database).await?;
    state.phase = ArchiveMaterializationPhase::Destroyed;
    persist_archive_materialization_state(spec, &state_path, &state)?;
    Ok(())
}

async fn restore_archive_materialization(
    spec: &ProvisionSpec,
    archive: &VerifiedBackupArchive,
    endpoint: &ClientEndpoint,
) -> Result<(), BackupError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let policy = &execution.backup;
    cleanup_owned_restore(&policy.isolated_restore_database_url, &endpoint.database).await?;
    create_empty_restore_database(&policy.isolated_restore_database_url, &endpoint.database)
        .await?;
    let (traffic, _) = load_verified_traffic_archive_input(spec, archive).await?;
    let toolchain = Toolchain::production(
        Duration::from_secs(policy.command_timeout_seconds),
        policy.maximum_encrypted_backup_bytes,
    )?;
    let defaults_path = scratch_path(
        &policy.encrypted_backup_output_path,
        &spec.operation_id,
        "materialization",
    )?;
    let mut defaults = endpoint.defaults_bytes();
    let scratch = SecretScratch::create(defaults_path, &mut defaults)?;
    let toolchain_for_restore = toolchain.clone();
    let endpoint_for_restore = endpoint.clone();
    let defaults_path = scratch.path().to_path_buf();
    let identity_path = policy.decryption_identity_path.clone();
    let artifact_path = policy.encrypted_backup_output_path.clone();
    let result = match tokio::task::spawn_blocking(move || {
        toolchain_for_restore.restore_encrypted_dump(
            &endpoint_for_restore,
            &defaults_path,
            &identity_path,
            &artifact_path,
            &traffic,
        )
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(BackupError::RestoreFailed),
    };
    let scratch_cleanup = scratch.remove();
    scratch_cleanup?;
    let frame = result?;
    if frame.format_version != archive.archive_format_version()
        || frame.traffic_receipt_sha256 != archive.traffic_receipt_sha256()
        || frame.traffic_receipt_bytes != archive.traffic_receipt_bytes()
        || frame.mysql_dump_bytes == 0
    {
        return Err(BackupError::RestoreFailed);
    }
    Ok(())
}

fn verify_materialization_anchor(
    spec: &ProvisionSpec,
    archive: &VerifiedBackupArchive,
    anchor: ArchiveMaterializationAnchor<'_>,
) -> Result<(), BackupError> {
    let expected_reference = backup_reference_sha256(archive.backup_reference())
        .map_err(|_| BackupError::ReceiptInvalid)?;
    let (operation_id, receipt, reference) = match anchor {
        ArchiveMaterializationAnchor::Journal(head) => {
            if head.checkpoint() < ApplyCheckpoint::BackupRestoreVerified
                || head.checkpoint() > ApplyCheckpoint::NodesVerified
                || !matches!(
                    head.state(),
                    ApplyJournalState::Running | ApplyJournalState::Verifying
                )
            {
                return Err(BackupError::ReceiptInvalid);
            }
            (
                head.binding().operation_id(),
                head.backup_restore_proof_sha256()
                    .ok_or(BackupError::ReceiptInvalid)?,
                head.backup_reference_sha256()
                    .ok_or(BackupError::ReceiptInvalid)?,
            )
        }
        ArchiveMaterializationAnchor::Target(permit) => (
            permit.operation_id(),
            permit.backup_restore_proof_sha256(),
            permit.backup_reference_sha256(),
        ),
    };
    if operation_id != spec.operation_id
        || receipt != archive.receipt_sha256()
        || reference != expected_reference
    {
        return Err(BackupError::ReceiptInvalid);
    }
    Ok(())
}

fn expected_materialization_state(
    spec: &ProvisionSpec,
    archive: &VerifiedBackupArchive,
    endpoint: &ClientEndpoint,
) -> Result<ArchiveMaterializationStatePayload, BackupError> {
    Ok(ArchiveMaterializationStatePayload {
        document_kind: "legacy_archive_materialization_state".into(),
        schema_version: ARCHIVE_MATERIALIZATION_STATE_SCHEMA_VERSION,
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        backup_reference_sha256: backup_reference_sha256(archive.backup_reference())
            .map_err(|_| BackupError::ReceiptInvalid)?,
        backup_receipt_sha256: archive.receipt_sha256().to_string(),
        encrypted_backup_sha256: archive.encrypted_backup_sha256().to_string(),
        source_fingerprint_sha256: archive.source_fingerprint_sha256().to_string(),
        source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
        restore_identity_sha256: domain_hash(
            b"v2board-legacy-backup-restore-identity-v1\0",
            endpoint.redacted_identity().as_bytes(),
        ),
        phase: ArchiveMaterializationPhase::Reserved,
        materialized_fingerprint_sha256: None,
        materialized_schema_sha256: None,
    })
}

fn archive_materialization_state_path(
    policy: &crate::manifest::LegacyBackupExecutionSpec,
) -> Result<PathBuf, BackupError> {
    sibling(
        &policy.isolated_restore_state_path,
        ".archive-materialization",
    )
}

fn read_archive_materialization_state(
    spec: &ProvisionSpec,
    path: &Path,
) -> Result<Option<ArchiveMaterializationStatePayload>, BackupError> {
    if matches!(
        fs::symlink_metadata(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    ) {
        return Ok(None);
    }
    let bytes = read_owner_only_limited(path, MAX_CONTROL_FILE_BYTES)?;
    let envelope: ArchiveMaterializationStateEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| BackupError::ReceiptInvalid)?;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::BackupRestore,
        &canonical,
        &envelope.hmac_sha256,
    ) {
        return Err(BackupError::ReceiptInvalid);
    }
    Ok(Some(envelope.payload))
}

fn persist_archive_materialization_state(
    spec: &ProvisionSpec,
    path: &Path,
    payload: &ArchiveMaterializationStatePayload,
) -> Result<(), BackupError> {
    verify_archive_materialization_state_shape(payload)?;
    if let Some(existing) = read_archive_materialization_state(spec, path)? {
        verify_archive_materialization_state_base(&existing, payload)?;
        verify_archive_materialization_transition(&existing, payload)?;
    }
    let canonical = serde_json::to_vec(payload).map_err(|_| BackupError::ReceiptInvalid)?;
    let hmac_sha256 = spec
        .source_receipt_binding_hmac_sha256(LegacyRuntimeReceiptKind::BackupRestore, &canonical)
        .ok_or(BackupError::ReceiptInvalid)?;
    let bytes = serde_json::to_vec(&ArchiveMaterializationStateEnvelope {
        payload: payload.clone(),
        hmac_sha256,
    })
    .map_err(|_| BackupError::ReceiptInvalid)?;
    write_atomic_owner_only(path, &bytes)
}

fn verify_archive_materialization_transition(
    existing: &ArchiveMaterializationStatePayload,
    requested: &ArchiveMaterializationStatePayload,
) -> Result<(), BackupError> {
    verify_archive_materialization_state_base(existing, requested)?;
    verify_archive_materialization_state_shape(existing)?;
    verify_archive_materialization_state_shape(requested)?;
    let rank = |phase| match phase {
        ArchiveMaterializationPhase::Reserved => 0,
        ArchiveMaterializationPhase::RestoreInProgress => 1,
        ArchiveMaterializationPhase::Ready => 2,
        ArchiveMaterializationPhase::Destroying => 3,
        ArchiveMaterializationPhase::Destroyed => 4,
    };
    let old = rank(existing.phase);
    let new = rank(requested.phase);
    if new < old || new > old + 1 || (new == old && *existing != *requested) {
        return Err(BackupError::Conflict);
    }
    Ok(())
}

fn verify_archive_materialization_state_base(
    actual: &ArchiveMaterializationStatePayload,
    expected: &ArchiveMaterializationStatePayload,
) -> Result<(), BackupError> {
    if actual.document_kind != expected.document_kind
        || actual.schema_version != expected.schema_version
        || actual.operation_id != expected.operation_id
        || actual.manifest_binding_hmac_sha256 != expected.manifest_binding_hmac_sha256
        || actual.backup_reference_sha256 != expected.backup_reference_sha256
        || actual.backup_receipt_sha256 != expected.backup_receipt_sha256
        || actual.encrypted_backup_sha256 != expected.encrypted_backup_sha256
        || actual.source_fingerprint_sha256 != expected.source_fingerprint_sha256
        || actual.source_schema_sha256 != expected.source_schema_sha256
        || actual.restore_identity_sha256 != expected.restore_identity_sha256
    {
        return Err(BackupError::Conflict);
    }
    Ok(())
}

fn verify_archive_materialization_state_shape(
    state: &ArchiveMaterializationStatePayload,
) -> Result<(), BackupError> {
    if state.document_kind != "legacy_archive_materialization_state"
        || state.schema_version != ARCHIVE_MATERIALIZATION_STATE_SCHEMA_VERSION
        || !is_lower_sha256(&state.manifest_binding_hmac_sha256)
        || !is_lower_sha256(&state.backup_reference_sha256)
        || !is_lower_sha256(&state.backup_receipt_sha256)
        || !is_lower_sha256(&state.encrypted_backup_sha256)
        || !is_lower_sha256(&state.source_fingerprint_sha256)
        || state.source_schema_sha256 != LEGACY_SEMANTIC_SCHEMA_SHA256
        || !is_lower_sha256(&state.restore_identity_sha256)
    {
        return Err(BackupError::ReceiptInvalid);
    }
    let exact_proof = state.materialized_fingerprint_sha256.as_deref()
        == Some(state.source_fingerprint_sha256.as_str())
        && state.materialized_schema_sha256.as_deref() == Some(state.source_schema_sha256.as_str());
    let shape_valid = match state.phase {
        ArchiveMaterializationPhase::Reserved | ArchiveMaterializationPhase::RestoreInProgress => {
            state.materialized_fingerprint_sha256.is_none()
                && state.materialized_schema_sha256.is_none()
        }
        ArchiveMaterializationPhase::Ready
        | ArchiveMaterializationPhase::Destroying
        | ArchiveMaterializationPhase::Destroyed => exact_proof,
    };
    if !shape_valid {
        return Err(BackupError::ReceiptInvalid);
    }
    Ok(())
}

fn require_archive_materialization_destroyed(spec: &ProvisionSpec) -> Result<(), BackupError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let path = archive_materialization_state_path(&execution.backup)?;
    let state =
        read_archive_materialization_state(spec, &path)?.ok_or(BackupError::ReceiptInvalid)?;
    verify_archive_materialization_state_shape(&state)?;
    if state.phase != ArchiveMaterializationPhase::Destroyed {
        return Err(BackupError::Conflict);
    }
    Ok(())
}

async fn restore_database_exists(url: &str, database: &str) -> Result<bool, BackupError> {
    let pool = admin_pool(url).await?;
    let exists = database_exists(&pool, database).await?;
    pool.close().await;
    Ok(exists)
}

/// Terminal recovery verifier for the narrow crash window after the
/// permanent PostgreSQL completion ledger committed and the runtime copy of
/// the age identity was removed. Callers must first prove that the exact
/// completed filesystem head is already present in PostgreSQL.
pub(crate) fn verify_persisted_backup_archive_after_ledger_completion(
    spec: &ProvisionSpec,
) -> Result<VerifiedBackupArchive, BackupError> {
    verify_persisted_backup_archive_inner(spec, false)
}

/// Reconciles disposal of plaintext recovery inputs after the fsync-durable
/// completion event has bound the verified archive. The source-drain receipt is
/// normally removed immediately after the restore drill; checking it here
/// closes its unlink lost-ack window. The external secret source remains
/// responsible for supplying the age identity again for disaster recovery.
pub fn cleanup_runtime_decryption_identity_after_completion(
    spec: &ProvisionSpec,
    completed: &ApplyJournalSnapshot,
) -> Result<VerifiedBackupArchive, BackupError> {
    if completed.state() != ApplyJournalState::Completed
        || completed.checkpoint() != ApplyCheckpoint::CompletionVerified
        || completed.binding().operation_id() != spec.operation_id
    {
        return Err(BackupError::ReceiptInvalid);
    }
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let identity = &execution.backup.decryption_identity_path;
    let exists = match fs::symlink_metadata(identity) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => return Err(BackupError::Filesystem),
    };
    let archive = verify_persisted_backup_archive_inner(spec, exists)?;
    require_archive_materialization_destroyed(spec)?;
    let expected_reference = backup_reference_sha256(archive.backup_reference())
        .map_err(|_| BackupError::ReceiptInvalid)?;
    if completed.backup_restore_proof_sha256() != Some(archive.receipt_sha256())
        || completed.backup_reference_sha256() != Some(expected_reference.as_str())
    {
        return Err(BackupError::ReceiptInvalid);
    }
    remove_verified_frozen_traffic_receipt(spec, archive.traffic_receipt_sha256())
        .map_err(|_| BackupError::Filesystem)?;
    if exists {
        remove_secure_file(identity)?;
        if !matches!(
            fs::symlink_metadata(identity),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        ) {
            return Err(BackupError::Filesystem);
        }
    }
    Ok(archive)
}

fn verify_persisted_backup_archive_inner(
    spec: &ProvisionSpec,
    require_runtime_identity: bool,
) -> Result<VerifiedBackupArchive, BackupError> {
    let execution = spec
        .legacy_apply_execution()
        .ok_or(BackupError::InvalidPolicy)?;
    let policy = &execution.backup;
    verify_bound_input(
        &policy.encryption_recipient_path,
        &policy.encryption_recipient_sha256,
    )?;
    if require_runtime_identity {
        verify_bound_input(
            &policy.decryption_identity_path,
            &policy.decryption_identity_sha256,
        )?;
    }
    let partial = partial_artifact_path(&policy.encrypted_backup_output_path, &spec.operation_id)?;
    let receipt_partial = sibling(&execution.receipts.backup_restore_path, ".receipt.partial")?;
    reconcile_hardlink_commit(&partial, &policy.encrypted_backup_output_path)?;
    reconcile_hardlink_commit(&receipt_partial, &execution.receipts.backup_restore_path)?;
    let state = read_restore_state(spec, &policy.isolated_restore_state_path)?
        .ok_or(BackupError::ReceiptInvalid)?;
    verify_state_phase(&state)?;
    if state.phase != RestorePhase::Destroyed {
        return Err(BackupError::ReceiptInvalid);
    }
    let receipt_path = &execution.receipts.backup_restore_path;
    let bytes = read_owner_only_limited(receipt_path, MAX_CONTROL_FILE_BYTES)?;
    let envelope: BackupReceiptEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| BackupError::ReceiptInvalid)?;
    let payload = envelope.payload;
    let artifact = hash_owner_only_file(
        &policy.encrypted_backup_output_path,
        policy.maximum_encrypted_backup_bytes,
    )?;
    if state.document_kind != "legacy_backup_restore_state"
        || state.operation_id != spec.operation_id
        || state.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
        || state.backup_reference != policy.backup_reference
        || state.recipient_sha256 != policy.encryption_recipient_sha256
        || state.decryption_identity_sha256 != policy.decryption_identity_sha256
        || !spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::BackupRestore,
            &canonical,
            &envelope.hmac_sha256,
        )
        || payload.document_kind != "legacy_backup_restore_receipt"
        || payload.schema_version != BACKUP_RECEIPT_SCHEMA_VERSION
        || payload.operation_id != spec.operation_id
        || payload.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
        || payload.backup_reference != policy.backup_reference
        || payload.source_server_identity_sha256.len() != 64
        || payload.source_drained_generation != state.source_drained_generation
        || payload.source_drained_event_sha256 != state.source_drained_event_sha256
        || payload.source_drained_proof_sha256 != state.source_drained_proof_sha256
        || payload.source_drained_generation == 0
        || payload.source_drained_event_sha256.len() != 64
        || payload.source_drained_proof_sha256.len() != 64
        || payload.source_fingerprint_before_sha256 != state.initial_source_fingerprint_sha256
        || payload.source_fingerprint_after_sha256 != state.initial_source_fingerprint_sha256
        || payload.restored_fingerprint_sha256 != state.initial_source_fingerprint_sha256
        || payload.source_identity_sha256 != state.source_identity_sha256
        || payload.restore_identity_sha256 != state.restore_identity_sha256
        || payload.archive_format_version != LEGACY_ARCHIVE_FORMAT_VERSION
        || payload.archive_format_version != state.archive_format_version
        || payload.traffic_receipt_sha256 != state.traffic_receipt_sha256
        || payload.traffic_receipt_bytes != state.traffic_receipt_bytes
        || payload.traffic_sorted_user_delta_count != state.traffic_sorted_user_delta_count
        || payload.traffic_sorted_user_delta_sha256 != state.traffic_sorted_user_delta_sha256
        || payload.traffic_upload_delta_sum != state.traffic_upload_delta_sum
        || payload.traffic_download_delta_sum != state.traffic_download_delta_sum
        || !payload.traffic_redis_physically_verified_before_archive
        || !payload.traffic_receipt_hmac_verified
        || !payload.traffic_receipt_restored_from_encrypted_archive
        || payload.mysql_dump_bytes_restored == 0
        || payload.encrypted_backup_sha256 != artifact.sha256
        || payload.encrypted_backup_bytes != artifact.bytes
        || payload.recipient_sha256 != policy.encryption_recipient_sha256
        || payload.decryption_identity_sha256 != policy.decryption_identity_sha256
        || !payload.actual_encrypted_bytes_decrypted
        || !payload.isolated_restore_destroyed
        || payload.started_at_unix != state.started_at_unix
        || payload.completed_at_unix < payload.started_at_unix
    {
        return Err(BackupError::ReceiptInvalid);
    }
    match fs::symlink_metadata(&execution.receipts.source_drain_path) {
        Ok(_) => {
            let current =
                verify_frozen_traffic_receipt(spec).map_err(|_| BackupError::ReceiptInvalid)?;
            let current_bytes = read_owner_only_limited(
                &execution.receipts.source_drain_path,
                MAX_TRAFFIC_RECEIPT_BYTES,
            )?;
            if current.operation_id != spec.operation_id
                || current.receipt_sha256 != payload.traffic_receipt_sha256
                || current_bytes.len() as u64 != payload.traffic_receipt_bytes
                || current.maintenance_fenced_generation.checked_add(1)
                    != Some(payload.source_drained_generation)
                || current.sorted_user_delta_count != payload.traffic_sorted_user_delta_count
                || current.sorted_user_delta_sha256 != payload.traffic_sorted_user_delta_sha256
                || current.upload_delta_sum != payload.traffic_upload_delta_sum
                || current.download_delta_sum != payload.traffic_download_delta_sum
            {
                return Err(BackupError::ReceiptInvalid);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(BackupError::Filesystem),
    }
    Ok(VerifiedBackupArchive {
        backup_reference: payload.backup_reference,
        source_fingerprint_sha256: payload.source_fingerprint_after_sha256,
        archive_format_version: payload.archive_format_version,
        traffic_receipt_sha256: payload.traffic_receipt_sha256,
        traffic_receipt_bytes: payload.traffic_receipt_bytes,
        traffic_sorted_user_delta_count: payload.traffic_sorted_user_delta_count,
        traffic_sorted_user_delta_sha256: payload.traffic_sorted_user_delta_sha256,
        traffic_upload_delta_sum: payload.traffic_upload_delta_sum,
        traffic_download_delta_sum: payload.traffic_download_delta_sum,
        encrypted_backup_sha256: artifact.sha256,
        encrypted_backup_bytes: artifact.bytes,
        receipt_sha256: hex::encode(Sha256::digest(&bytes)),
    })
}

fn legacy_source(spec: &ProvisionSpec) -> Result<&SourceSpec, BackupError> {
    match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => Ok(source),
        _ => Err(BackupError::InvalidPolicy),
    }
}

fn legacy_conversion_strategy(
    spec: &ProvisionSpec,
) -> Result<LegacyConversionStrategy, BackupError> {
    LegacyConversionStrategy::for_schema_version(spec.schema_version)
        .map_err(|_| BackupError::InvalidPolicy)
}

fn verify_state_base(
    actual: &RestoreStatePayload,
    expected: &RestoreStatePayload,
) -> Result<(), BackupError> {
    if actual.document_kind != expected.document_kind
        || actual.schema_version != expected.schema_version
        || actual.operation_id != expected.operation_id
        || actual.manifest_binding_hmac_sha256 != expected.manifest_binding_hmac_sha256
        || actual.backup_reference != expected.backup_reference
        || actual.source_identity_sha256 != expected.source_identity_sha256
        || actual.restore_identity_sha256 != expected.restore_identity_sha256
        || actual.recipient_sha256 != expected.recipient_sha256
        || actual.decryption_identity_sha256 != expected.decryption_identity_sha256
        || actual.started_at_unix <= 0
        || actual.source_drained_generation != expected.source_drained_generation
        || actual.source_drained_event_sha256 != expected.source_drained_event_sha256
        || actual.source_drained_proof_sha256 != expected.source_drained_proof_sha256
        || actual.archive_format_version != expected.archive_format_version
        || actual.traffic_receipt_sha256 != expected.traffic_receipt_sha256
        || actual.traffic_receipt_bytes != expected.traffic_receipt_bytes
        || actual.traffic_sorted_user_delta_count != expected.traffic_sorted_user_delta_count
        || actual.traffic_sorted_user_delta_sha256 != expected.traffic_sorted_user_delta_sha256
        || actual.traffic_upload_delta_sum != expected.traffic_upload_delta_sum
        || actual.traffic_download_delta_sum != expected.traffic_download_delta_sum
        || !actual.traffic_redis_physically_verified_before_archive
    {
        return Err(BackupError::Conflict);
    }
    Ok(())
}

fn source_drained_proof(head: &ApplyJournalSnapshot) -> Result<&str, BackupError> {
    if head.checkpoint() != ApplyCheckpoint::SourceDrained {
        return Err(BackupError::ReceiptInvalid);
    }
    head.checkpoint_proof_sha256()
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
        .ok_or(BackupError::ReceiptInvalid)
}

fn verify_state_anchor(
    state: &RestoreStatePayload,
    head: &ApplyJournalSnapshot,
) -> Result<(), BackupError> {
    let current_proof = source_drained_proof(head)?;
    if current_proof != state.source_drained_proof_sha256
        || state.source_drained_generation != head.generation()
        || state.source_drained_event_sha256 != head.event_sha256()
        || state.source_drained_event_sha256.len() != 64
        || !state
            .source_drained_event_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(BackupError::ReceiptInvalid);
    }
    // `generation` and `event` are the original audit anchor. A durable
    // needs-recovery/resume event may change both while the SourceDrained
    // checkpoint proof remains inherited and immutable.
    Ok(())
}

fn verify_state_phase(state: &RestoreStatePayload) -> Result<(), BackupError> {
    if state.schema_version != BACKUP_STATE_SCHEMA_VERSION
        || state.archive_format_version != LEGACY_ARCHIVE_FORMAT_VERSION
        || !is_lower_sha256(&state.traffic_receipt_sha256)
        || state.traffic_receipt_bytes == 0
        || state.traffic_receipt_bytes > MAX_TRAFFIC_RECEIPT_BYTES
        || !is_lower_sha256(&state.traffic_sorted_user_delta_sha256)
        || state.traffic_upload_delta_sum.parse::<u128>().is_err()
        || state.traffic_download_delta_sum.parse::<u128>().is_err()
        || !state.traffic_redis_physically_verified_before_archive
    {
        return Err(BackupError::ReceiptInvalid);
    }
    let artifact_bound = state.encrypted_backup_sha256.as_ref().is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    }) && state.encrypted_backup_bytes.is_some_and(|value| value > 0);
    let valid = match state.phase {
        RestorePhase::Reserved => {
            state.encrypted_backup_sha256.is_none() && state.encrypted_backup_bytes.is_none()
        }
        RestorePhase::DumpCommitted | RestorePhase::RestoreInProgress | RestorePhase::Destroyed => {
            artifact_bound
        }
    };
    if !valid {
        return Err(BackupError::ReceiptInvalid);
    }
    Ok(())
}

fn persist_restore_state(
    spec: &ProvisionSpec,
    path: &Path,
    payload: &RestoreStatePayload,
) -> Result<(), BackupError> {
    if let Some(existing) = read_restore_state(spec, path)? {
        verify_state_base(&existing, payload)?;
        verify_monotonic_state_transition(&existing, payload)?;
    }
    let canonical = serde_json::to_vec(payload).map_err(|_| BackupError::ReceiptInvalid)?;
    let hmac_sha256 = spec
        .source_receipt_binding_hmac_sha256(LegacyRuntimeReceiptKind::BackupRestore, &canonical)
        .ok_or(BackupError::ReceiptInvalid)?;
    let bytes = serde_json::to_vec(&RestoreStateEnvelope {
        payload: payload.clone(),
        hmac_sha256,
    })
    .map_err(|_| BackupError::ReceiptInvalid)?;
    write_atomic_owner_only(path, &bytes)
}

fn verify_monotonic_state_transition(
    existing: &RestoreStatePayload,
    requested: &RestoreStatePayload,
) -> Result<(), BackupError> {
    let rank = |phase| match phase {
        RestorePhase::Reserved => 0,
        RestorePhase::DumpCommitted => 1,
        RestorePhase::RestoreInProgress => 2,
        RestorePhase::Destroyed => 3,
    };
    let old = rank(existing.phase);
    let new = rank(requested.phase);
    if new < old || new > old + 1 {
        return Err(BackupError::Conflict);
    }
    if existing.started_at_unix != requested.started_at_unix
        || existing.initial_source_fingerprint_sha256 != requested.initial_source_fingerprint_sha256
        || existing.source_drained_generation != requested.source_drained_generation
        || existing.source_drained_event_sha256 != requested.source_drained_event_sha256
        || existing.source_drained_proof_sha256 != requested.source_drained_proof_sha256
        || existing.archive_format_version != requested.archive_format_version
        || existing.traffic_receipt_sha256 != requested.traffic_receipt_sha256
        || existing.traffic_receipt_bytes != requested.traffic_receipt_bytes
        || existing.traffic_sorted_user_delta_count != requested.traffic_sorted_user_delta_count
        || existing.traffic_sorted_user_delta_sha256 != requested.traffic_sorted_user_delta_sha256
        || existing.traffic_upload_delta_sum != requested.traffic_upload_delta_sum
        || existing.traffic_download_delta_sum != requested.traffic_download_delta_sum
        || existing.traffic_redis_physically_verified_before_archive
            != requested.traffic_redis_physically_verified_before_archive
    {
        return Err(BackupError::Conflict);
    }
    if new == old && existing != requested {
        return Err(BackupError::Conflict);
    }
    if old >= 1
        && (existing.encrypted_backup_sha256 != requested.encrypted_backup_sha256
            || existing.encrypted_backup_bytes != requested.encrypted_backup_bytes)
    {
        return Err(BackupError::Conflict);
    }
    Ok(())
}

fn read_restore_state(
    spec: &ProvisionSpec,
    path: &Path,
) -> Result<Option<RestoreStatePayload>, BackupError> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(None);
    }
    let bytes = read_owner_only_limited(path, MAX_CONTROL_FILE_BYTES)?;
    let envelope: RestoreStateEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| BackupError::ReceiptInvalid)?;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::BackupRestore,
        &canonical,
        &envelope.hmac_sha256,
    ) {
        return Err(BackupError::ReceiptInvalid);
    }
    Ok(Some(envelope.payload))
}

fn receipt_envelope_bytes(
    spec: &ProvisionSpec,
    payload: BackupReceiptPayload,
) -> Result<Vec<u8>, BackupError> {
    let canonical = serde_json::to_vec(&payload).map_err(|_| BackupError::ReceiptInvalid)?;
    let hmac_sha256 = spec
        .source_receipt_binding_hmac_sha256(LegacyRuntimeReceiptKind::BackupRestore, &canonical)
        .ok_or(BackupError::ReceiptInvalid)?;
    serde_json::to_vec(&BackupReceiptEnvelope {
        payload,
        hmac_sha256,
    })
    .map_err(|_| BackupError::ReceiptInvalid)
}

fn verify_existing_receipt(
    spec: &ProvisionSpec,
    path: &Path,
    state: &RestoreStatePayload,
    artifact: &Artifact,
    head: &ApplyJournalSnapshot,
    source_server_identity: &MysqlServerIdentity,
) -> Result<BackupReceiptPayload, BackupError> {
    let bytes = read_owner_only_limited(path, MAX_CONTROL_FILE_BYTES)?;
    let envelope: BackupReceiptEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::ReceiptInvalid)?;
    let canonical =
        serde_json::to_vec(&envelope.payload).map_err(|_| BackupError::ReceiptInvalid)?;
    let payload = envelope.payload;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        LegacyRuntimeReceiptKind::BackupRestore,
        &canonical,
        &envelope.hmac_sha256,
    ) || payload.document_kind != "legacy_backup_restore_receipt"
        || payload.schema_version != BACKUP_RECEIPT_SCHEMA_VERSION
        || payload.operation_id != spec.operation_id
        || payload.source_drained_generation != state.source_drained_generation
        || payload.source_drained_event_sha256 != state.source_drained_event_sha256
        || payload.source_drained_proof_sha256 != state.source_drained_proof_sha256
        || source_drained_proof(head)? != state.source_drained_proof_sha256
        || payload.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
        || payload.backup_reference != state.backup_reference
        || payload.source_server_identity_sha256
            != mysql_server_identity_sha256(source_server_identity)
        || payload.source_identity_sha256 != state.source_identity_sha256
        || payload.restore_identity_sha256 != state.restore_identity_sha256
        || payload.source_fingerprint_before_sha256 != state.initial_source_fingerprint_sha256
        || payload.source_fingerprint_after_sha256 != state.initial_source_fingerprint_sha256
        || payload.restored_fingerprint_sha256 != state.initial_source_fingerprint_sha256
        || payload.archive_format_version != state.archive_format_version
        || payload.archive_format_version != LEGACY_ARCHIVE_FORMAT_VERSION
        || payload.traffic_receipt_sha256 != state.traffic_receipt_sha256
        || payload.traffic_receipt_bytes != state.traffic_receipt_bytes
        || payload.traffic_sorted_user_delta_count != state.traffic_sorted_user_delta_count
        || payload.traffic_sorted_user_delta_sha256 != state.traffic_sorted_user_delta_sha256
        || payload.traffic_upload_delta_sum != state.traffic_upload_delta_sum
        || payload.traffic_download_delta_sum != state.traffic_download_delta_sum
        || !payload.traffic_redis_physically_verified_before_archive
        || !payload.traffic_receipt_hmac_verified
        || !payload.traffic_receipt_restored_from_encrypted_archive
        || payload.mysql_dump_bytes_restored == 0
        || payload.encrypted_backup_sha256 != artifact.sha256
        || payload.encrypted_backup_bytes != artifact.bytes
        || payload.recipient_sha256 != state.recipient_sha256
        || payload.decryption_identity_sha256 != state.decryption_identity_sha256
        || !payload.actual_encrypted_bytes_decrypted
        || !payload.isolated_restore_destroyed
        || payload.started_at_unix != state.started_at_unix
        || payload.completed_at_unix < payload.started_at_unix
        || state.phase != RestorePhase::Destroyed
    {
        return Err(BackupError::ReceiptInvalid);
    }
    Ok(payload)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MysqlServerIdentity {
    version: String,
    version_comment: String,
}

async fn mysql_server_identity(url: &str) -> Result<MysqlServerIdentity, BackupError> {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(RESTORE_DATABASE_TIMEOUT)
        .connect(url)
        .await
        .map_err(|_| BackupError::Database)?;
    let (version, version_comment) =
        sqlx::query_as::<_, (String, String)>("SELECT VERSION(), @@version_comment")
            .fetch_one(&pool)
            .await
            .map_err(|_| BackupError::Database)?;
    pool.close().await;
    let identity = MysqlServerIdentity {
        version,
        version_comment,
    };
    validate_server_identity_text(&identity)?;
    Ok(identity)
}

fn validate_server_identity_text(identity: &MysqlServerIdentity) -> Result<(), BackupError> {
    if [&identity.version, &identity.version_comment]
        .into_iter()
        .any(|value| value.is_empty() || value.len() > 256 || value.chars().any(char::is_control))
    {
        return Err(BackupError::UnsupportedServer);
    }
    Ok(())
}

fn verify_mysql8_server_identity(identity: &MysqlServerIdentity) -> Result<(), BackupError> {
    validate_server_identity_text(identity)?;
    let distribution =
        format!("{} {}", identity.version, identity.version_comment).to_ascii_lowercase();
    let comment = identity.version_comment.to_ascii_lowercase();
    if !(comment.contains("mysql community server") || comment.contains("mysql enterprise"))
        || distribution.contains("mariadb")
        || distribution.contains("percona")
    {
        return Err(BackupError::UnsupportedServer);
    }
    let mut parts = identity.version.split(['.', '-']);
    let major = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or(BackupError::UnsupportedServer)?;
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or(BackupError::UnsupportedServer)?;
    let _patch = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or(BackupError::UnsupportedServer)?;
    if major != 8 || !matches!(minor, 0 | 4) {
        return Err(BackupError::UnsupportedServer);
    }
    Ok(())
}

fn mysql_server_identity_sha256(identity: &MysqlServerIdentity) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"v2board-legacy-mysql8-server-identity-v1\0");
    for value in [&identity.version, &identity.version_comment] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
}

async fn require_absent_or_empty_restore(url: &str, database: &str) -> Result<(), BackupError> {
    let pool = admin_pool(url).await?;
    if database_exists(&pool, database).await? && object_count(&pool, database).await? != 0 {
        pool.close().await;
        return Err(BackupError::RestoreNotEmpty);
    }
    pool.close().await;
    Ok(())
}

async fn create_empty_restore_database(url: &str, database: &str) -> Result<(), BackupError> {
    let pool = admin_pool(url).await?;
    if !database_exists(&pool, database).await? {
        let statement = format!(
            "CREATE DATABASE {} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
            quote_identifier(database)
        );
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .map_err(|_| BackupError::Database)?;
    }
    if object_count(&pool, database).await? != 0 {
        pool.close().await;
        return Err(BackupError::RestoreNotEmpty);
    }
    pool.close().await;
    Ok(())
}

async fn cleanup_owned_restore(url: &str, database: &str) -> Result<(), BackupError> {
    let pool = admin_pool(url).await?;
    if database_exists(&pool, database).await? {
        let statement = format!("DROP DATABASE {}", quote_identifier(database));
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .map_err(|_| BackupError::CleanupFailed)?;
    }
    let absent = !database_exists(&pool, database).await?;
    pool.close().await;
    if !absent {
        return Err(BackupError::CleanupFailed);
    }
    Ok(())
}

async fn admin_pool(url: &str) -> Result<Pool<MySql>, BackupError> {
    MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(RESTORE_DATABASE_TIMEOUT)
        .connect(&admin_url(url)?)
        .await
        .map_err(|_| BackupError::Database)
}

fn admin_url(url: &str) -> Result<String, BackupError> {
    let mut admin = Url::parse(url).map_err(|_| BackupError::InvalidPolicy)?;
    admin.set_path("/information_schema");
    Ok(admin.into())
}

fn require_output_capacity(path: &Path, maximum: u64) -> Result<(), BackupError> {
    let available = output_available_bytes(path)?;
    let required = maximum
        .checked_add(1024 * 1024)
        .ok_or(BackupError::InvalidPolicy)?;
    if available < required {
        return Err(BackupError::Filesystem);
    }
    Ok(())
}

fn output_available_bytes(path: &Path) -> Result<u64, BackupError> {
    let parent = path.parent().ok_or(BackupError::Filesystem)?;
    ensure_private_parent(path)?;
    let df = validate_program(DF_PATH, &[DF_PATH])?;
    let mut child = command(&df)
        .arg("-B1")
        .arg("--output=avail")
        .arg(parent)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| BackupError::Filesystem)?;
    let stdout = bounded_reader(child.stdout.take().ok_or(BackupError::Filesystem)?);
    let stderr = bounded_reader(child.stderr.take().ok_or(BackupError::Filesystem)?);
    let status = wait_single(&mut child, Duration::from_secs(30))?;
    let (stdout, stdout_overflow) = stdout
        .join()
        .map_err(|_| BackupError::Filesystem)?
        .map_err(|_| BackupError::Filesystem)?;
    let (_, stderr_overflow) = stderr
        .join()
        .map_err(|_| BackupError::Filesystem)?
        .map_err(|_| BackupError::Filesystem)?;
    if !status.success() || stdout_overflow || stderr_overflow || stdout.len() > 4096 {
        return Err(BackupError::Filesystem);
    }
    std::str::from_utf8(&stdout)
        .ok()
        .and_then(|value| {
            value
                .lines()
                .rev()
                .find_map(|line| line.trim().parse::<u64>().ok())
        })
        .ok_or(BackupError::Filesystem)
}

async fn database_exists(pool: &Pool<MySql>, database: &str) -> Result<bool, BackupError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = ?",
    )
    .bind(database)
    .fetch_one(pool)
    .await
    .map_err(|_| BackupError::Database)?;
    Ok(count == 1)
}

async fn object_count(pool: &Pool<MySql>, database: &str) -> Result<i64, BackupError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT \
             (SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = ?) + \
             (SELECT COUNT(*) FROM information_schema.routines WHERE routine_schema = ?) + \
             (SELECT COUNT(*) FROM information_schema.events WHERE event_schema = ?) + \
             (SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema = ?)",
    )
    .bind(database)
    .bind(database)
    .bind(database)
    .bind(database)
    .fetch_one(pool)
    .await
    .map_err(|_| BackupError::Database)
}

fn quote_identifier(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn verify_bound_input(path: &Path, expected_sha256: &str) -> Result<String, BackupError> {
    let bytes = read_owner_only_limited(path, MAX_CONTROL_FILE_BYTES)?;
    if bytes.is_empty() {
        return Err(BackupError::InvalidInput);
    }
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != expected_sha256 {
        return Err(BackupError::InvalidInput);
    }
    Ok(actual)
}

fn hash_owner_only_file(path: &Path, maximum: u64) -> Result<Artifact, BackupError> {
    let mut file = open_owner_only_file(path, maximum)?;
    let mut digest = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| BackupError::Filesystem)?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or(BackupError::Filesystem)?;
        if bytes > maximum {
            return Err(BackupError::Filesystem);
        }
        digest.update(&buffer[..count]);
    }
    if bytes == 0 {
        return Err(BackupError::Filesystem);
    }
    Ok(Artifact {
        sha256: hex::encode(digest.finalize()),
        bytes,
    })
}

fn open_owner_only_file(path: &Path, maximum: u64) -> Result<File, BackupError> {
    ensure_private_parent(path)?;
    let before = fs::symlink_metadata(path).map_err(|_| BackupError::Filesystem)?;
    if !before.file_type().is_file()
        || before.file_type().is_symlink()
        || before.uid() != 0
        || before.permissions().mode() & 0o077 != 0
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > maximum
    {
        return Err(BackupError::Filesystem);
    }
    let file = File::open(path).map_err(|_| BackupError::Filesystem)?;
    let opened = file.metadata().map_err(|_| BackupError::Filesystem)?;
    if opened.dev() != before.dev() || opened.ino() != before.ino() {
        return Err(BackupError::Filesystem);
    }
    Ok(file)
}

fn read_owner_only_limited(path: &Path, maximum: u64) -> Result<Vec<u8>, BackupError> {
    let mut file = open_owner_only_file(path, maximum)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| BackupError::Filesystem)?;
    Ok(bytes)
}

fn ensure_private_parent(path: &Path) -> Result<(), BackupError> {
    let parent = path.parent().ok_or(BackupError::Filesystem)?;
    let metadata = fs::symlink_metadata(parent).map_err(|_| BackupError::Filesystem)?;
    let canonical = fs::canonicalize(parent).map_err(|_| BackupError::Filesystem)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o077 != 0
        || canonical != parent
    {
        return Err(BackupError::Filesystem);
    }
    Ok(())
}

fn write_create_new_fsync(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    ensure_private_parent(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| BackupError::Filesystem)?;
    file.write_all(bytes).map_err(|_| BackupError::Filesystem)?;
    file.sync_all().map_err(|_| BackupError::Filesystem)?;
    sync_parent(path)
}

fn write_immutable_output(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    if fs::symlink_metadata(path).is_ok() {
        let existing = read_owner_only_limited(path, MAX_CONTROL_FILE_BYTES)?;
        return if existing == bytes {
            Ok(())
        } else {
            Err(BackupError::Conflict)
        };
    }
    let temporary = sibling(path, ".receipt.partial")?;
    if fs::symlink_metadata(&temporary).is_ok() {
        let existing = read_owner_only_limited(&temporary, MAX_CONTROL_FILE_BYTES)?;
        if existing != bytes {
            return Err(BackupError::Conflict);
        }
        remove_secure_file(&temporary)?;
    }
    write_create_new_fsync(&temporary, bytes)?;
    commit_no_clobber(&temporary, path)
}

fn write_atomic_owner_only(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    ensure_private_parent(path)?;
    if fs::symlink_metadata(path).is_ok() {
        let _ = read_owner_only_limited(path, MAX_CONTROL_FILE_BYTES)?;
    }
    let temporary = sibling(path, ".state.partial")?;
    if fs::symlink_metadata(&temporary).is_ok() {
        remove_secure_file(&temporary)?;
    }
    write_create_new_fsync(&temporary, bytes)?;
    fs::rename(&temporary, path).map_err(|_| BackupError::Filesystem)?;
    sync_parent(path)
}

fn commit_no_clobber(partial: &Path, final_path: &Path) -> Result<(), BackupError> {
    ensure_private_parent(final_path)?;
    fs::hard_link(partial, final_path).map_err(|_| BackupError::Conflict)?;
    fs::remove_file(partial).map_err(|_| BackupError::Filesystem)?;
    sync_parent(final_path)
}

fn reconcile_hardlink_commit(partial: &Path, final_path: &Path) -> Result<(), BackupError> {
    if partial.parent() != final_path.parent() {
        return Err(BackupError::Conflict);
    }
    ensure_private_parent(final_path)?;
    let partial_metadata = match fs::symlink_metadata(partial) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(BackupError::Filesystem),
    };
    let final_metadata = match fs::symlink_metadata(final_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(BackupError::Filesystem),
    };
    let safe = partial_metadata.file_type().is_file()
        && !partial_metadata.file_type().is_symlink()
        && final_metadata.file_type().is_file()
        && !final_metadata.file_type().is_symlink()
        && partial_metadata.uid() == 0
        && final_metadata.uid() == 0
        && partial_metadata.permissions().mode() & 0o077 == 0
        && final_metadata.permissions().mode() & 0o077 == 0
        && partial_metadata.nlink() == 2
        && final_metadata.nlink() == 2
        && partial_metadata.dev() == final_metadata.dev()
        && partial_metadata.ino() == final_metadata.ino();
    if !safe {
        return Err(BackupError::Conflict);
    }
    fs::remove_file(partial).map_err(|_| BackupError::Filesystem)?;
    sync_parent(final_path)?;
    let final_after = fs::symlink_metadata(final_path).map_err(|_| BackupError::Filesystem)?;
    if final_after.nlink() != 1
        || final_after.dev() != final_metadata.dev()
        || final_after.ino() != final_metadata.ino()
    {
        return Err(BackupError::Conflict);
    }
    Ok(())
}

fn recover_verified_receipt_partial(
    partial: &Path,
    final_path: &Path,
    verify: impl FnOnce(&Path) -> Result<(), BackupError>,
) -> Result<bool, BackupError> {
    if partial.parent() != final_path.parent() {
        return Err(BackupError::Conflict);
    }
    match fs::symlink_metadata(partial) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(BackupError::Filesystem),
    }
    if fs::symlink_metadata(final_path).is_ok() {
        return Err(BackupError::Conflict);
    }
    // The caller verifies the complete receipt envelope against the HMAC-bound
    // manifest, monotonic Destroyed state, exact encrypted artifact, and the
    // inherited SourceDrained checkpoint proof. Only those already-frozen
    // bytes are published; completed_at is never regenerated on recovery.
    verify(partial)?;
    commit_no_clobber(partial, final_path)?;
    Ok(true)
}

fn prepare_partial_path(path: &Path) -> Result<(), BackupError> {
    ensure_private_parent(path)?;
    if fs::symlink_metadata(path).is_ok() {
        remove_secure_file(path)?;
    }
    Ok(())
}

fn remove_secure_file(path: &Path) -> Result<(), BackupError> {
    ensure_private_parent(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| BackupError::Filesystem)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(BackupError::Conflict);
    }
    fs::remove_file(path).map_err(|_| BackupError::Filesystem)?;
    sync_parent(path)
}

fn sync_parent(path: &Path) -> Result<(), BackupError> {
    File::open(path.parent().ok_or(BackupError::Filesystem)?)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| BackupError::Filesystem)
}

fn partial_artifact_path(path: &Path, operation_id: &str) -> Result<PathBuf, BackupError> {
    sibling(path, &format!(".{operation_id}.partial"))
}

fn scratch_path(path: &Path, operation_id: &str, kind: &str) -> Result<PathBuf, BackupError> {
    sibling(path, &format!(".{operation_id}.{kind}.cnf"))
}

fn sibling(path: &Path, suffix: &str) -> Result<PathBuf, BackupError> {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(BackupError::Filesystem)?;
    Ok(path.with_file_name(format!(".{name}{suffix}")))
}

fn domain_hash(domain: &[u8], value: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
    hex::encode(digest.finalize())
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn unix_now() -> Result<i64, BackupError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_secs()).ok())
        .filter(|value| *value > 0)
        .ok_or(BackupError::Filesystem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_journal::{ApplyJournal, ApplyJournalBinding, ApplyOutcomeCode};
    use uuid::Uuid;

    fn traffic_input(bytes: &[u8]) -> TrafficReceiptArchiveInput {
        let receipt = VerifiedFrozenTrafficReceipt {
            operation_id: "40aa4a80-eb4b-4b25-9c3b-e17ed047873d".into(),
            maintenance_fenced_generation: 2,
            maintenance_fenced_event_sha256: "c".repeat(64),
            source_default_run_id: "e".repeat(40),
            frozen_upload_key: "v2board_upload".into(),
            frozen_download_key: "v2board_download".into(),
            fenced_at_unix: 1_700_000_000,
            upload_fields: 0,
            download_fields: 0,
            sorted_user_delta_count: 0,
            sorted_user_delta_sha256: "d".repeat(64),
            upload_delta_sum: "10".into(),
            download_delta_sum: "20".into(),
            deltas: Vec::new(),
            delta_applied_exactly_once: true,
            receipt_sha256: hex::encode(Sha256::digest(bytes)),
        };
        TrafficReceiptArchiveInput::from_verified(bytes.to_vec(), &receipt).expect("traffic input")
    }

    fn state(phase: RestorePhase) -> RestoreStatePayload {
        let artifact_bound = phase != RestorePhase::Reserved;
        RestoreStatePayload {
            document_kind: "legacy_backup_restore_state".into(),
            schema_version: BACKUP_STATE_SCHEMA_VERSION,
            operation_id: "40aa4a80-eb4b-4b25-9c3b-e17ed047873d".into(),
            manifest_binding_hmac_sha256: "1".repeat(64),
            backup_reference: "backup:test/archive".into(),
            source_identity_sha256: "2".repeat(64),
            restore_identity_sha256: "3".repeat(64),
            recipient_sha256: "4".repeat(64),
            decryption_identity_sha256: "5".repeat(64),
            initial_source_fingerprint_sha256: "6".repeat(64),
            source_drained_generation: 3,
            source_drained_event_sha256: "7".repeat(64),
            source_drained_proof_sha256: "8".repeat(64),
            archive_format_version: LEGACY_ARCHIVE_FORMAT_VERSION,
            traffic_receipt_sha256: "a".repeat(64),
            traffic_receipt_bytes: 512,
            traffic_sorted_user_delta_count: 2,
            traffic_sorted_user_delta_sha256: "b".repeat(64),
            traffic_upload_delta_sum: "11".into(),
            traffic_download_delta_sum: "12".into(),
            traffic_redis_physically_verified_before_archive: true,
            started_at_unix: 1_700_000_000,
            phase,
            encrypted_backup_sha256: artifact_bound.then(|| "9".repeat(64)),
            encrypted_backup_bytes: artifact_bound.then_some(1024),
        }
    }

    fn materialization_state(
        phase: ArchiveMaterializationPhase,
    ) -> ArchiveMaterializationStatePayload {
        let verified = matches!(
            phase,
            ArchiveMaterializationPhase::Ready
                | ArchiveMaterializationPhase::Destroying
                | ArchiveMaterializationPhase::Destroyed
        );
        ArchiveMaterializationStatePayload {
            document_kind: "legacy_archive_materialization_state".into(),
            schema_version: ARCHIVE_MATERIALIZATION_STATE_SCHEMA_VERSION,
            operation_id: "40aa4a80-eb4b-4b25-9c3b-e17ed047873d".into(),
            manifest_binding_hmac_sha256: "1".repeat(64),
            backup_reference_sha256: "2".repeat(64),
            backup_receipt_sha256: "3".repeat(64),
            encrypted_backup_sha256: "4".repeat(64),
            source_fingerprint_sha256: "5".repeat(64),
            source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.into(),
            restore_identity_sha256: "6".repeat(64),
            phase,
            materialized_fingerprint_sha256: verified.then(|| "5".repeat(64)),
            materialized_schema_sha256: verified.then(|| LEGACY_SEMANTIC_SCHEMA_SHA256.into()),
        }
    }

    #[test]
    fn archive_materialization_state_is_monotonic_and_binds_exact_proofs() {
        let reserved = materialization_state(ArchiveMaterializationPhase::Reserved);
        let restoring = materialization_state(ArchiveMaterializationPhase::RestoreInProgress);
        let ready = materialization_state(ArchiveMaterializationPhase::Ready);
        let destroying = materialization_state(ArchiveMaterializationPhase::Destroying);
        let destroyed = materialization_state(ArchiveMaterializationPhase::Destroyed);
        for (before, after) in [
            (&reserved, &restoring),
            (&restoring, &ready),
            (&ready, &destroying),
            (&destroying, &destroyed),
        ] {
            verify_archive_materialization_transition(before, after).expect("forward transition");
        }
        assert!(verify_archive_materialization_transition(&ready, &restoring).is_err());
        assert!(verify_archive_materialization_transition(&reserved, &ready).is_err());
        assert!(verify_archive_materialization_transition(&destroyed, &destroyed).is_ok());

        let mut wrong_fingerprint = ready.clone();
        wrong_fingerprint.materialized_fingerprint_sha256 = Some("f".repeat(64));
        assert!(verify_archive_materialization_state_shape(&wrong_fingerprint).is_err());
        let mut changed_archive = destroying.clone();
        changed_archive.encrypted_backup_sha256 = "e".repeat(64);
        assert!(verify_archive_materialization_transition(&ready, &changed_archive).is_err());
    }

    #[test]
    fn defaults_keep_credentials_out_of_argv_and_environment() {
        let endpoint = ClientEndpoint::parse(
            "mysql://legacy:super-secret@127.0.0.1/v2board?ssl-mode=VERIFY_IDENTITY",
        )
        .expect("endpoint");
        let toolchain = Toolchain {
            age: AGE_PATH.into(),
            dump: MYSQL_DUMP_PATH.into(),
            client: MYSQL_CLIENT_PATH.into(),
            timeout: Duration::from_secs(60),
            maximum_encrypted_bytes: 1024 * 1024,
        };
        let dump = toolchain.dump_args(Path::new("/private/source.cnf"), &endpoint.database);
        let restore = toolchain.client_args(Path::new("/private/restore.cnf"), &endpoint.database);
        for argument in dump.iter().chain(&restore) {
            let value = argument.to_string_lossy();
            assert!(!value.contains("super-secret"));
            assert!(!value.contains("mysql://"));
        }
        let defaults = endpoint.defaults_bytes();
        assert!(String::from_utf8_lossy(&defaults).contains("super-secret"));
    }

    #[tokio::test]
    #[ignore = "requires real age/MySQL 8 tools and two disposable MySQL 8 databases"]
    async fn real_age_stream_dump_restore_preserves_the_reference_fingerprint() {
        let source_url =
            std::env::var("V2BOARD_BACKUP_E2E_SOURCE_URL").expect("V2BOARD_BACKUP_E2E_SOURCE_URL");
        let restore_url = std::env::var("V2BOARD_BACKUP_E2E_RESTORE_URL")
            .expect("V2BOARD_BACKUP_E2E_RESTORE_URL");
        let identity_path = PathBuf::from(
            std::env::var("V2BOARD_BACKUP_E2E_AGE_IDENTITY_PATH")
                .expect("V2BOARD_BACKUP_E2E_AGE_IDENTITY_PATH"),
        );
        let recipient_path = PathBuf::from(
            std::env::var("V2BOARD_BACKUP_E2E_AGE_RECIPIENT_PATH")
                .expect("V2BOARD_BACKUP_E2E_AGE_RECIPIENT_PATH"),
        );
        let source = ClientEndpoint::parse(&source_url).expect("source endpoint");
        let restore = ClientEndpoint::parse(&restore_url).expect("restore endpoint");
        cleanup_owned_restore(&restore_url, &restore.database)
            .await
            .expect("clear operation-owned restore database");
        create_empty_restore_database(&restore_url, &restore.database)
            .await
            .expect("create first empty restore database");
        let root = std::env::temp_dir().join(format!(
            "v2board-real-backup-e2e-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("private backup test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("private backup test permissions");
        let source_defaults_path = root.join("source.cnf");
        let restore_defaults_path = root.join("restore.cnf");
        let mut source_defaults = source.defaults_bytes();
        let mut restore_defaults = restore.defaults_bytes();
        let source_scratch = SecretScratch::create(source_defaults_path, &mut source_defaults)
            .expect("source defaults scratch");
        let restore_scratch = SecretScratch::create(restore_defaults_path, &mut restore_defaults)
            .expect("restore defaults scratch");
        let toolchain = Toolchain::production(Duration::from_secs(300), 1024 * 1024 * 1024)
            .expect("real backup toolchain");
        let traffic = traffic_input(br#"{"payload":"encrypted-traffic-frame"}"#);
        let artifact_path = root.join("reference.sql.age");
        let (artifact, written_frame) = toolchain
            .create_encrypted_dump(
                &source,
                source_scratch.path(),
                &recipient_path,
                &traffic,
                &artifact_path,
            )
            .expect("stream mysqldump through framing and age");
        assert!(artifact.bytes > 0);
        let restored_frame = toolchain
            .restore_encrypted_dump(
                &restore,
                restore_scratch.path(),
                &identity_path,
                &artifact_path,
                &traffic,
            )
            .expect("decrypt frame and feed only SQL to isolated MySQL 8");
        assert_eq!(restored_frame, written_frame);
        for strategy in [
            LegacyConversionStrategy::PreserveAll,
            LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs,
        ] {
            assert_eq!(
                fingerprint_mysql_for_strategy(&source_url, strategy)
                    .await
                    .expect("source strategy fingerprint"),
                fingerprint_mysql_for_strategy(&restore_url, strategy)
                    .await
                    .expect("restored strategy fingerprint")
            );
        }
        cleanup_owned_restore(&restore_url, &restore.database)
            .await
            .expect("drop first verified restore");
        create_empty_restore_database(&restore_url, &restore.database)
            .await
            .expect("recreate empty archive materialization");
        let recovered_frame = toolchain
            .restore_encrypted_dump(
                &restore,
                restore_scratch.path(),
                &identity_path,
                &artifact_path,
                &traffic,
            )
            .expect("re-materialize the same encrypted archive after source loss");
        assert_eq!(recovered_frame, written_frame);
        for strategy in [
            LegacyConversionStrategy::PreserveAll,
            LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs,
        ] {
            assert_eq!(
                fingerprint_mysql_for_strategy(&source_url, strategy)
                    .await
                    .expect("source fingerprint after re-materialization"),
                fingerprint_mysql_for_strategy(&restore_url, strategy)
                    .await
                    .expect("re-materialized strategy fingerprint")
            );
        }
        cleanup_owned_restore(&restore_url, &restore.database)
            .await
            .expect("destroy archive materialization");
        drop(source_scratch);
        drop(restore_scratch);
        fs::remove_dir_all(root).expect("remove real backup test root");
    }

    #[test]
    fn mysql8_has_one_fixed_safe_dump_contract() {
        let mysql = Toolchain {
            age: AGE_PATH.into(),
            dump: MYSQL_DUMP_PATH.into(),
            client: MYSQL_CLIENT_PATH.into(),
            timeout: Duration::from_secs(60),
            maximum_encrypted_bytes: 1024 * 1024,
        };
        assert!(
            mysql
                .dump_args(Path::new("/x"), "v2board")
                .contains(&OsString::from("--set-gtid-purged=OFF"))
        );
        for required in [
            "--skip-triggers",
            "--column-statistics=0",
            "--no-tablespaces",
        ] {
            assert!(
                mysql
                    .dump_args(Path::new("/x"), "v2board")
                    .contains(&OsString::from(required))
            );
        }
        for forbidden in ["--routines", "--events", "--triggers"] {
            assert!(
                !mysql
                    .dump_args(Path::new("/x"), "v2board")
                    .contains(&OsString::from(forbidden))
            );
        }
        let oracle = |version: &str, version_comment: &str| MysqlServerIdentity {
            version: version.to_string(),
            version_comment: version_comment.to_string(),
        };
        assert!(
            verify_mysql8_server_identity(&oracle("8.0.44", "MySQL Community Server - GPL"))
                .is_ok()
        );
        assert!(verify_mysql8_server_identity(&oracle("8.4.7", "MySQL Enterprise Server")).is_ok());
        for unsupported in [
            oracle("5.7.44", "MySQL Community Server - GPL"),
            oracle("9.0.1", "MySQL Community Server - GPL"),
            oracle("8.0.37-29", "Percona Server (GPL)"),
            oracle("10.11.8-MariaDB", "mariadb.org binary distribution"),
            oracle("8.0.44", "Amazon Aurora MySQL"),
            oracle("8.0.44", "Unknown SQL proxy"),
        ] {
            assert!(verify_mysql8_server_identity(&unsupported).is_err());
        }
    }

    #[test]
    fn versioned_archive_round_trip_keeps_traffic_out_of_mysql_input() {
        let traffic = traffic_input(br#"{"hmac_sha256":"bound","deltas":[[1,10,20]]}"#);
        let dump = b"CREATE TABLE v2_user(id BIGINT);\nINSERT INTO v2_user VALUES (1);\n";
        let mut plaintext = Vec::new();
        let written = write_legacy_archive_plaintext(&dump[..], &mut plaintext, &traffic)
            .expect("archive frame");
        let mut mysql_input = Vec::new();
        let restored = restore_legacy_archive_plaintext(&plaintext[..], &mut mysql_input, &traffic)
            .expect("restore frame");
        assert_eq!(written, restored);
        assert_eq!(mysql_input, dump);
        assert!(
            !mysql_input
                .windows(traffic.bytes.len())
                .any(|window| window == traffic.bytes.as_ref())
        );
    }

    #[test]
    fn traffic_tamper_fails_before_any_mysql_byte_is_written() {
        let traffic = traffic_input(br#"{"hmac_sha256":"bound","deltas":[[1,10,20]]}"#);
        let mut plaintext = Vec::new();
        write_legacy_archive_plaintext(&b"SELECT 1;\n"[..], &mut plaintext, &traffic)
            .expect("archive frame");
        let receipt_offset = LEGACY_ARCHIVE_MAGIC.len()
            + std::mem::size_of::<u32>()
            + LEGACY_ARCHIVE_TRAFFIC_TAG.len()
            + std::mem::size_of::<u64>()
            + 32;
        plaintext[receipt_offset] ^= 1;
        let mut mysql_input = Vec::new();
        assert!(
            restore_legacy_archive_plaintext(&plaintext[..], &mut mysql_input, &traffic).is_err()
        );
        assert!(mysql_input.is_empty());
    }

    #[test]
    fn framing_version_and_empty_dump_fail_closed() {
        let traffic = traffic_input(br#"{"hmac_sha256":"bound","deltas":[]}"#);
        let mut plaintext = Vec::new();
        write_legacy_archive_plaintext(&b"SELECT 1;\n"[..], &mut plaintext, &traffic)
            .expect("archive frame");
        plaintext[LEGACY_ARCHIVE_MAGIC.len() + 3] ^= 1;
        assert!(
            restore_legacy_archive_plaintext(&plaintext[..], &mut Vec::new(), &traffic).is_err()
        );
        assert!(write_legacy_archive_plaintext(io::empty(), Vec::new(), &traffic).is_err());
    }

    #[test]
    fn bounded_copy_fails_closed_without_publishing() {
        let directory = std::env::temp_dir().join(format!("backup-limit-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("permissions");
        let path = directory.join("partial.age");
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .expect("output");
        assert!(copy_bounded_and_fsync(&b"12345"[..], output, 4).is_err());
        fs::remove_file(path).expect("partial cleanup");
        fs::remove_dir(directory).expect("directory cleanup");
    }

    #[test]
    fn immutable_output_retries_exactly_and_rejects_conflict() {
        let directory = std::env::temp_dir().join(format!("backup-receipt-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("permissions");
        let path = directory.join("receipt.json");
        write_immutable_output(&path, b"first").expect("first write");
        write_immutable_output(&path, b"first").expect("exact retry");
        assert!(matches!(
            write_immutable_output(&path, b"second"),
            Err(BackupError::Conflict)
        ));
        assert_eq!(read_owner_only_limited(&path, 64).expect("read"), b"first");
        fs::remove_file(path).expect("receipt cleanup");
        fs::remove_dir(directory).expect("directory cleanup");
    }

    #[test]
    fn interrupted_hardlink_publish_is_reconciled_without_relaxing_link_checks() {
        let directory = std::env::temp_dir().join(format!("backup-link-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("permissions");
        let partial = directory.join(".archive.partial");
        let final_path = directory.join("archive.age");
        write_create_new_fsync(&partial, b"encrypted").expect("partial");
        fs::hard_link(&partial, &final_path).expect("crash-window final link");
        assert!(matches!(
            open_owner_only_file(&final_path, 1024),
            Err(BackupError::Filesystem)
        ));
        reconcile_hardlink_commit(&partial, &final_path).expect("reconcile exact inode");
        assert!(!partial.exists());
        assert_eq!(
            read_owner_only_limited(&final_path, 1024).expect("single-link final"),
            b"encrypted"
        );
        fs::remove_file(final_path).expect("artifact cleanup");
        fs::remove_dir(directory).expect("directory cleanup");
    }

    #[test]
    fn verified_receipt_partial_is_published_without_regenerating_lost_ack_bytes() {
        let directory = std::env::temp_dir().join(format!("backup-receipt-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("permissions");
        let partial = directory.join(".backup-restore.json.receipt.partial");
        let final_path = directory.join("backup-restore.json");
        let frozen = br#"{"completed_at_unix":1700000000,"hmac_sha256":"frozen"}"#;
        write_create_new_fsync(&partial, frozen).expect("fsynced receipt temporary");

        let promoted = recover_verified_receipt_partial(&partial, &final_path, |path| {
            let actual = read_owner_only_limited(path, 1024)?;
            if actual != frozen {
                return Err(BackupError::ReceiptInvalid);
            }
            Ok(())
        })
        .expect("validated recovery publishes original bytes");
        assert!(promoted);
        assert!(!partial.exists());
        assert_eq!(
            read_owner_only_limited(&final_path, 1024).expect("published receipt"),
            frozen
        );

        // Simulate losing the acknowledgement after publication. Recovery
        // finds no temporary to regenerate and the final bytes stay exact.
        assert!(
            !recover_verified_receipt_partial(&partial, &final_path, |_| {
                panic!("absent temporary must not be rebuilt")
            })
            .expect("lost-ack retry")
        );
        assert_eq!(
            read_owner_only_limited(&final_path, 1024).expect("same receipt"),
            frozen
        );
        fs::remove_file(final_path).expect("receipt cleanup");
        fs::remove_dir(directory).expect("directory cleanup");
    }

    #[test]
    fn unverified_receipt_partial_is_never_published_or_overwritten() {
        let directory = std::env::temp_dir().join(format!("backup-reject-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("permissions");
        let partial = directory.join(".backup-restore.json.receipt.partial");
        let final_path = directory.join("backup-restore.json");
        write_create_new_fsync(&partial, b"tampered").expect("partial");
        assert!(matches!(
            recover_verified_receipt_partial(&partial, &final_path, |_| {
                Err(BackupError::ReceiptInvalid)
            }),
            Err(BackupError::ReceiptInvalid)
        ));
        assert_eq!(
            read_owner_only_limited(&partial, 1024).expect("unchanged partial"),
            b"tampered"
        );
        assert!(!final_path.exists());
        fs::remove_file(partial).expect("partial cleanup");
        fs::remove_dir(directory).expect("directory cleanup");
    }

    #[test]
    fn restore_state_transitions_never_regress_or_skip() {
        let reserved = state(RestorePhase::Reserved);
        let dump = state(RestorePhase::DumpCommitted);
        let restoring = state(RestorePhase::RestoreInProgress);
        let destroyed = state(RestorePhase::Destroyed);
        verify_monotonic_state_transition(&reserved, &dump).expect("reserve to dump");
        verify_monotonic_state_transition(&dump, &restoring).expect("dump to restore");
        verify_monotonic_state_transition(&restoring, &destroyed).expect("restore to destroy");
        assert!(verify_monotonic_state_transition(&destroyed, &restoring).is_err());
        assert!(verify_monotonic_state_transition(&reserved, &restoring).is_err());
        assert!(verify_monotonic_state_transition(&destroyed, &destroyed).is_ok());
    }

    #[test]
    fn source_drained_proof_survives_lost_ack_recovery_events() {
        let parent = std::env::temp_dir().join(format!("backup-journal-{}", Uuid::new_v4()));
        fs::create_dir(&parent).expect("parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).expect("permissions");
        let root = parent.join("journal");
        let binding =
            ApplyJournalBinding::new("40aa4a80-eb4b-4b25-9c3b-e17ed047873d", "a".repeat(64))
                .expect("binding");
        let (journal, pending) = ApplyJournal::create_pending(&root, binding).expect("journal");
        let running = journal.begin(&pending).expect("begin");
        let fenced = journal
            .checkpoint_with_proof(&running, ApplyCheckpoint::MaintenanceFenced, "b".repeat(64))
            .expect("fence");
        let drained = journal
            .checkpoint_with_proof(&fenced, ApplyCheckpoint::SourceDrained, "c".repeat(64))
            .expect("drain");
        let original_event = drained.event_sha256().to_string();
        let original_generation = drained.generation();
        let recovery = journal
            .mark_needs_recovery(&drained, ApplyOutcomeCode::ProcessInterrupted)
            .expect("needs recovery");
        let resumed = journal.resume(&recovery).expect("resume");
        assert_ne!(resumed.event_sha256(), original_event);
        assert!(resumed.generation() > original_generation);
        assert_eq!(
            source_drained_proof(&resumed).expect("inherited proof"),
            "c".repeat(64)
        );
        fs::remove_dir_all(parent).expect("journal cleanup");
    }

    #[test]
    fn scratch_file_rejects_conflicting_crash_residue() {
        let directory = std::env::temp_dir().join(format!("backup-scratch-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("permissions");
        let path = directory.join("client.cnf");
        write_create_new_fsync(&path, b"old-secret").expect("residue");
        let mut expected = b"new-secret".to_vec();
        assert!(matches!(
            SecretScratch::create(path.clone(), &mut expected),
            Err(BackupError::Conflict)
        ));
        assert!(expected.iter().all(|byte| *byte == 0));
        fs::remove_file(path).expect("residue cleanup");
        fs::remove_dir(directory).expect("directory cleanup");
    }
}
