use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::MysqlImportSpec;

const MAX_MYSQL_DUMP_BYTES: u64 = 1024 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImmutableFileInspection {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub regular_non_symlink: bool,
    pub owner_only_permissions: bool,
    pub same_inode_before_and_after_hash: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MysqlImportLossReport {
    pub old_redis: Vec<&'static str>,
    pub stripe: Vec<&'static str>,
    pub other: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MysqlImportPreservationReport {
    pub exact_or_semantic: Vec<&'static str>,
    pub transformed: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MysqlImportInspection {
    pub schema_version: u32,
    pub manifest_sha256: String,
    pub mysql_dump: ImmutableFileInspection,
    pub accepted_losses: MysqlImportLossReport,
    pub preserved: MysqlImportPreservationReport,
    pub old_mysql_contacted: bool,
    pub old_redis_contacted: bool,
    pub stripe_provider_contacted: bool,
    pub target_mutated: bool,
    pub report_sha256: String,
}

#[derive(Debug, Error)]
pub enum MysqlImportInspectionError {
    #[error("{field} could not be opened or read safely")]
    FileRead { field: &'static str },
    #[error(
        "{field} must be a non-empty root-owned regular non-symlink file with owner-only permissions"
    )]
    UnsafeFile { field: &'static str },
    #[error("{field} exceeds the supported size limit")]
    FileTooLarge { field: &'static str },
    #[error("{field} SHA-256 does not match the manifest")]
    DigestMismatch { field: &'static str },
    #[error("inspection report could not be serialized")]
    Serialize,
}

pub fn inspect_mysql_import(
    spec: &MysqlImportSpec,
) -> Result<MysqlImportInspection, MysqlImportInspectionError> {
    let mysql_dump = inspect_file(
        &spec.source.dump_path,
        &spec.source.dump_sha256,
        MAX_MYSQL_DUMP_BYTES,
        "source.dump_path",
    )?;
    let mut inspection = MysqlImportInspection {
        schema_version: spec.schema_version,
        manifest_sha256: spec.manifest_sha256().to_string(),
        mysql_dump,
        accepted_losses: accepted_losses(),
        preserved: preserved_data(),
        old_mysql_contacted: false,
        old_redis_contacted: false,
        stripe_provider_contacted: false,
        target_mutated: false,
        report_sha256: String::new(),
    };
    inspection.report_sha256 = inspection_report_sha256(&inspection)?;
    Ok(inspection)
}

fn inspect_file(
    path: &Path,
    expected_sha256: &str,
    maximum_bytes: u64,
    field: &'static str,
) -> Result<ImmutableFileInspection, MysqlImportInspectionError> {
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| MysqlImportInspectionError::FileRead { field })?;
    let owner_only = path_metadata.permissions().mode() & 0o077 == 0;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.len() == 0
        || !owner_only
        || path_metadata.uid() != 0
    {
        return Err(MysqlImportInspectionError::UnsafeFile { field });
    }
    if path_metadata.len() > maximum_bytes {
        return Err(MysqlImportInspectionError::FileTooLarge { field });
    }
    let mut file = File::open(path).map_err(|_| MysqlImportInspectionError::FileRead { field })?;
    let opened = file
        .metadata()
        .map_err(|_| MysqlImportInspectionError::FileRead { field })?;
    if opened.dev() != path_metadata.dev()
        || opened.ino() != path_metadata.ino()
        || opened.uid() != 0
    {
        return Err(MysqlImportInspectionError::UnsafeFile { field });
    }

    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| MysqlImportInspectionError::FileRead { field })?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(MysqlImportInspectionError::FileTooLarge { field })?;
        if total > maximum_bytes {
            return Err(MysqlImportInspectionError::FileTooLarge { field });
        }
        digest.update(&buffer[..count]);
    }
    let sha256 = hex::encode(digest.finalize());
    if sha256 != expected_sha256 {
        return Err(MysqlImportInspectionError::DigestMismatch { field });
    }

    let after =
        fs::symlink_metadata(path).map_err(|_| MysqlImportInspectionError::FileRead { field })?;
    if opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || opened.len() != after.len()
        || !after.file_type().is_file()
        || after.file_type().is_symlink()
        || after.uid() != 0
        || after.permissions().mode() & 0o077 != 0
    {
        return Err(MysqlImportInspectionError::UnsafeFile { field });
    }
    Ok(ImmutableFileInspection {
        path: path.to_path_buf(),
        sha256,
        bytes: total,
        regular_non_symlink: true,
        owner_only_permissions: true,
        same_inode_before_and_after_hash: true,
    })
}

fn accepted_losses() -> MysqlImportLossReport {
    MysqlImportLossReport {
        old_redis: vec![
            "traffic not persisted to the MySQL user u/d counters",
            "queued, retryable, and failed work",
            "sessions, one-time codes, and rate-limit state",
            "temporary subscription links",
            "cache, locks, leases, and queue metadata",
        ],
        stripe: vec![
            "all old Stripe payment configuration rows",
            "all status 0/1 orders bound to an old Stripe payment",
            "all provider-side objects, without querying or compensating Stripe",
            "provider bindings on retained status 2/3/4 order history",
        ],
        other: vec![
            "failed_jobs",
            "old nodes, routes, and credentials, which are rebuilt manually",
            "legacy MySQL v2_stat_user and v2_stat_server traffic-detail rows",
            "optional legacy v2_tutorial upgrade residue",
            "old ClickHouse event history",
            "old operational logs and mail history",
            "old theme assets and runtime files",
        ],
    }
}

fn preserved_data() -> MysqlImportPreservationReport {
    MysqlImportPreservationReport {
        exact_or_semantic: vec![
            "users, balances, and permanent subscription tokens",
            "MySQL-persisted user upload/download counters",
            "plans, groups, and non-Stripe payment configuration",
            "non-Stripe orders, including unfinished orders",
            "completed, cancelled, and offset Stripe order business history",
            "tickets, knowledge, notices, coupons, gift cards, and commissions",
            "legacy MySQL v2_stat aggregate history imported into native stat",
        ],
        transformed: vec![
            "legacy MySQL v2_* source tables map to unprefixed native PostgreSQL targets",
            "explicit manifest target/runtime values generate new API and worker configuration",
            "the one-shot Redis bootstrap credential is not emitted; execute creates persisted, distinct API and worker ACL credentials",
            "retained Stripe order history has provider payment and callback bindings cleared",
        ],
    }
}

fn inspection_report_sha256(
    inspection: &MysqlImportInspection,
) -> Result<String, MysqlImportInspectionError> {
    let mut value =
        serde_json::to_value(inspection).map_err(|_| MysqlImportInspectionError::Serialize)?;
    value
        .as_object_mut()
        .ok_or(MysqlImportInspectionError::Serialize)?
        .insert(
            "report_sha256".to_string(),
            serde_json::Value::String(String::new()),
        );
    let bytes = serde_json::to_vec(&value).map_err(|_| MysqlImportInspectionError::Serialize)?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-mysql-import-inspection-v1\0");
    digest.update(bytes);
    Ok(hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "v2board-mysql-import-inspect-{}-{nonce}-{name}",
            std::process::id()
        ))
    }

    #[test]
    fn loss_report_never_claims_to_read_old_redis_or_contact_stripe() {
        let losses = accepted_losses();
        assert!(
            losses
                .old_redis
                .iter()
                .any(|item| item.contains("not persisted"))
        );
        assert!(
            losses
                .stripe
                .iter()
                .any(|item| item.contains("without querying"))
        );
    }

    #[test]
    fn preservation_report_keeps_non_stripe_configuration_and_tokens() {
        let preserved = preserved_data();
        let joined = preserved.exact_or_semantic.join(" ");
        assert!(joined.contains("non-Stripe payment configuration"));
        assert!(joined.contains("subscription tokens"));
        assert!(
            preserved
                .transformed
                .iter()
                .any(|item| item.contains("distinct API and worker ACL credentials"))
        );
    }

    #[test]
    fn dump_inspection_binds_digest_permissions_and_regular_file() {
        let path = test_path("dump.sql");
        let link = test_path("dump-link.sql");
        let bytes = b"CREATE TABLE example (id BIGINT);";
        fs::write(&path, bytes).expect("write dump");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restrict dump");
        let digest = hex::encode(Sha256::digest(bytes));

        let inspected =
            inspect_file(&path, &digest, 1024, "source.dump_path").expect("inspect dump");
        assert_eq!(inspected.sha256, digest);
        assert_eq!(inspected.bytes, bytes.len() as u64);
        assert!(matches!(
            inspect_file(&path, &"0".repeat(64), 1024, "source.dump_path"),
            Err(MysqlImportInspectionError::DigestMismatch { .. })
        ));

        symlink(&path, &link).expect("create symlink");
        assert!(matches!(
            inspect_file(&link, &digest, 1024, "source.dump_path"),
            Err(MysqlImportInspectionError::UnsafeFile { .. })
        ));
        fs::remove_file(&link).expect("remove symlink");

        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("loosen dump");
        assert!(matches!(
            inspect_file(&path, &digest, 1024, "source.dump_path"),
            Err(MysqlImportInspectionError::UnsafeFile { .. })
        ));
        fs::remove_file(&path).expect("remove dump");
    }
}
