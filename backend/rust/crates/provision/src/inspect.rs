use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ProvisionKind, ProvisionSpec,
    cold_import_capability::production_cold_import_capability_for_spec,
    release_archive::{ReadOnlyReleaseArchiveInspection, inspect_native_release_archive_read_only},
};

const MAX_AGE_IDENTITY_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionMode {
    ArchiveReadOnly,
}

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
pub struct ColdImportLossReport {
    pub legacy_redis: Vec<&'static str>,
    pub legacy_stripe: Vec<&'static str>,
    pub other: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ColdImportPreservationReport {
    pub exact_or_semantic_preservation: Vec<&'static str>,
    pub transformed_preservation: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProvisionPlan {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: ProvisionKind,
    pub manifest_binding_hmac_sha256: String,
    pub encrypted_mysql_dump: ImmutableFileInspection,
    pub age_identity: ImmutableFileInspection,
    pub native_release: ReadOnlyReleaseArchiveInspection,
    pub accepted_losses: ColdImportLossReport,
    pub preserved: ColdImportPreservationReport,
    pub legacy_mysql_contacted: bool,
    pub legacy_redis_contacted: bool,
    pub stripe_provider_contacted: bool,
    pub target_mutated: bool,
    pub dump_decryption_tested: bool,
    pub isolated_restore_tested: bool,
    pub target_empty_verified: bool,
    pub archive_binding_passed: bool,
    pub apply_available: bool,
    pub apply_blocker: Option<String>,
    pub retry_model: &'static str,
    pub report_sha256: String,
}

#[derive(Debug, Error)]
pub enum ProvisionPlanError {
    #[error("archive inspection is available only for legacy_reference_migration schema v5")]
    WrongProvisionKind,
    #[error("{field} could not be opened or read safely")]
    FileRead { field: &'static str },
    #[error("{field} must be a non-empty regular non-symlink file with owner-only permissions")]
    UnsafeFile { field: &'static str },
    #[error("{field} exceeds its manifest-bound size limit")]
    FileTooLarge { field: &'static str },
    #[error("{field} SHA-256 does not match the manifest")]
    DigestMismatch { field: &'static str },
    #[error("the encrypted MySQL dump is not an age-encrypted file")]
    NotAgeEncrypted,
    #[error("native release inspection failed: {0}")]
    Release(String),
    #[error("inspection report could not be serialized")]
    Serialize,
}

pub fn build_inspection(
    spec: &ProvisionSpec,
    _mode: InspectionMode,
) -> Result<ProvisionPlan, ProvisionPlanError> {
    let legacy = spec
        .legacy_cold_import()
        .ok_or(ProvisionPlanError::WrongProvisionKind)?;
    let (encrypted_mysql_dump, dump_prefix) = inspect_file(
        &legacy.source.encrypted_mysql_dump_path,
        &legacy.source.encrypted_mysql_dump_sha256,
        legacy.source.maximum_encrypted_dump_bytes,
        "source.encrypted_mysql_dump_path",
        true,
    )?;
    if !dump_prefix.starts_with(b"age-encryption.org/v1")
        && !dump_prefix.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----")
    {
        return Err(ProvisionPlanError::NotAgeEncrypted);
    }
    let (age_identity, _) = inspect_file(
        &legacy.source.age_identity_path,
        &legacy.source.age_identity_sha256,
        MAX_AGE_IDENTITY_BYTES,
        "source.age_identity_path",
        false,
    )?;
    let native_release = inspect_native_release_archive_read_only(
        &legacy.execution.release.archive_path,
        &legacy.execution.release.release_id,
        &legacy.execution.release.archive_sha256,
    )
    .map_err(|error| ProvisionPlanError::Release(error.to_string()))?;
    let capability = production_cold_import_capability_for_spec(spec);
    let apply_blocker = capability
        .blocker()
        .map(|blocker| blocker.report_message().to_string());
    let mut plan = ProvisionPlan {
        schema_version: spec.schema_version,
        operation_id: spec.operation_id.clone(),
        kind: spec.kind,
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        encrypted_mysql_dump,
        age_identity,
        native_release,
        accepted_losses: accepted_losses(),
        preserved: preserved_data(),
        legacy_mysql_contacted: false,
        legacy_redis_contacted: false,
        stripe_provider_contacted: false,
        target_mutated: false,
        dump_decryption_tested: false,
        isolated_restore_tested: false,
        target_empty_verified: false,
        archive_binding_passed: true,
        apply_available: capability.is_available(),
        apply_blocker,
        retry_model: "wipe_unactivated_operation_owned_target_and_restart_from_same_immutable_dump",
        report_sha256: String::new(),
    };
    plan.report_sha256 = inspection_report_sha256(&plan)?;
    Ok(plan)
}

pub fn build_plan(spec: &ProvisionSpec) -> Result<ProvisionPlan, ProvisionPlanError> {
    build_inspection(spec, InspectionMode::ArchiveReadOnly)
}

impl ProvisionPlan {
    pub const fn passed(&self) -> bool {
        self.archive_binding_passed
    }
}

fn inspect_file(
    path: &Path,
    expected_sha256: &str,
    maximum_bytes: u64,
    field: &'static str,
    capture_prefix: bool,
) -> Result<(ImmutableFileInspection, Vec<u8>), ProvisionPlanError> {
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| ProvisionPlanError::FileRead { field })?;
    let owner_only = path_metadata.permissions().mode() & 0o077 == 0;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.len() == 0
        || !owner_only
    {
        return Err(ProvisionPlanError::UnsafeFile { field });
    }
    if path_metadata.len() > maximum_bytes {
        return Err(ProvisionPlanError::FileTooLarge { field });
    }
    let mut file = File::open(path).map_err(|_| ProvisionPlanError::FileRead { field })?;
    let opened = file
        .metadata()
        .map_err(|_| ProvisionPlanError::FileRead { field })?;
    if opened.dev() != path_metadata.dev() || opened.ino() != path_metadata.ino() {
        return Err(ProvisionPlanError::UnsafeFile { field });
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    let mut prefix = Vec::with_capacity(64);
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| ProvisionPlanError::FileRead { field })?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(ProvisionPlanError::FileTooLarge { field })?;
        if total > maximum_bytes {
            return Err(ProvisionPlanError::FileTooLarge { field });
        }
        if capture_prefix && prefix.len() < 64 {
            let remaining = 64 - prefix.len();
            prefix.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        digest.update(&buffer[..count]);
    }
    let sha256 = hex::encode(digest.finalize());
    if sha256 != expected_sha256 {
        return Err(ProvisionPlanError::DigestMismatch { field });
    }
    let after = fs::symlink_metadata(path).map_err(|_| ProvisionPlanError::FileRead { field })?;
    let stable =
        opened.dev() == after.dev() && opened.ino() == after.ino() && opened.len() == after.len();
    if !stable {
        return Err(ProvisionPlanError::UnsafeFile { field });
    }
    Ok((
        ImmutableFileInspection {
            path: path.to_path_buf(),
            sha256,
            bytes: total,
            regular_non_symlink: true,
            owner_only_permissions: true,
            same_inode_before_and_after_hash: true,
        },
        prefix,
    ))
}

fn accepted_losses() -> ColdImportLossReport {
    ColdImportLossReport {
        legacy_redis: vec![
            "pending traffic after the last MySQL-persisted u/d values",
            "queued/retryable jobs and failed work",
            "sessions, OTP state and rate-limit state",
            "temporary subscription links",
            "cache, locks, leases and Horizon metadata",
        ],
        legacy_stripe: vec![
            "all legacy Stripe payment configuration rows",
            "all status 0/1 orders bound to a discarded Stripe payment",
            "provider-side objects are ignored without inspection or compensation",
            "provider callback/payment bindings on retained status 2/3/4 history",
        ],
        other: vec![
            "failed_jobs",
            "legacy nodes (manual rebuild)",
            "legacy traffic detail rows",
            "legacy operational logs",
            "legacy theme assets and runtime files",
        ],
    }
}

fn preserved_data() -> ColdImportPreservationReport {
    ColdImportPreservationReport {
        exact_or_semantic_preservation: vec![
            "users, balances and permanent subscription tokens",
            "MySQL-persisted user upload/download counters",
            "plans, groups and non-Stripe payment configuration",
            "non-Stripe orders including unfinished orders",
            "completed/cancelled/offset Stripe order business history",
            "tickets, knowledge, notices, coupons, gift cards and commissions",
        ],
        transformed_preservation: vec![
            "supported runtime configuration is rebuilt into role-owned files from the manifest",
            "retained Stripe order history has payment_id and callback_no cleared",
        ],
    }
}

fn inspection_report_sha256(plan: &ProvisionPlan) -> Result<String, ProvisionPlanError> {
    let mut value = serde_json::to_value(plan).map_err(|_| ProvisionPlanError::Serialize)?;
    value
        .as_object_mut()
        .ok_or(ProvisionPlanError::Serialize)?
        .insert(
            "report_sha256".to_string(),
            serde_json::Value::String(String::new()),
        );
    let bytes = serde_json::to_vec(&value).map_err(|_| ProvisionPlanError::Serialize)?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-cold-import-inspection-v1\0");
    digest.update(bytes);
    Ok(hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loss_report_never_claims_to_touch_legacy_redis_or_stripe() {
        let losses = accepted_losses();
        assert!(
            losses
                .legacy_redis
                .iter()
                .any(|item| item.contains("pending traffic"))
        );
        assert!(
            losses
                .legacy_stripe
                .iter()
                .any(|item| item.contains("ignored"))
        );
    }

    #[test]
    fn preservation_report_keeps_non_stripe_configuration_and_tokens() {
        let preserved = preserved_data();
        let joined = preserved.exact_or_semantic_preservation.join(" ");
        assert!(joined.contains("non-Stripe payment configuration"));
        assert!(joined.contains("subscription tokens"));
    }
}
