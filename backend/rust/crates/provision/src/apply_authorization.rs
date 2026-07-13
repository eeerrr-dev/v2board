use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ProvisionKind, ProvisionPlan, ProvisionSpec};

const AUTHORIZATION_VERSION: u32 = 3;
const MAX_AUTHORIZATION_BYTES: u64 = 64 * 1024;
const AUTHORIZATION_LIFETIME_SECONDS: i64 = 24 * 60 * 60;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyAuthorization {
    pub authorization_version: u32,
    pub operation_id: String,
    pub manifest_binding_hmac_sha256: String,
    pub inspect_review_sha256: String,
    pub inspect_review_binding_hmac_sha256: String,
    pub authorized_snapshot_report_sha256: String,
    pub authorized_snapshot_report_binding_hmac_sha256: String,
    pub reviewed_target_redis_run_id: String,
    pub reviewed_target_redis_database_index: u32,
    pub reviewed_source_default_redis_run_id: String,
    pub reviewed_source_cache_redis_run_id: String,
    pub issued_at_unix: i64,
    pub expires_at_unix: i64,
    pub irreversible_one_shot_approved: bool,
    pub authorization_binding_hmac_sha256: String,
}

#[derive(Serialize)]
struct AuthorizationPayload<'a> {
    authorization_version: u32,
    operation_id: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    inspect_review_sha256: &'a str,
    inspect_review_binding_hmac_sha256: &'a str,
    authorized_snapshot_report_sha256: &'a str,
    authorized_snapshot_report_binding_hmac_sha256: &'a str,
    reviewed_target_redis_run_id: &'a str,
    reviewed_target_redis_database_index: u32,
    reviewed_source_default_redis_run_id: &'a str,
    reviewed_source_cache_redis_run_id: &'a str,
    issued_at_unix: i64,
    expires_at_unix: i64,
    irreversible_one_shot_approved: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyAuthorizationError {
    #[error("only legacy_reference_migration can issue the one-shot legacy authorization")]
    WrongProvisionKind,
    #[error("the online inspection is not ready to authorize apply")]
    InspectionNotReady,
    #[error("the confirmation must exactly equal the operation_id")]
    ConfirmationMismatch,
    #[error("authorization timestamp is outside the supported range")]
    InvalidTimestamp,
    #[error("authorization file path must be absolute")]
    RelativePath,
    #[error("authorization output already exists")]
    AlreadyExists,
    #[error("authorization file must be a regular non-symlink file")]
    UnsafeFileType,
    #[error("authorization file must not grant group or world permissions")]
    UnsafePermissions,
    #[error("authorization file must be between 1 byte and 64 KiB")]
    UnsafeSize,
    #[error("authorization file I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("authorization JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("authorization does not bind this manifest, operation, and inspection report")]
    BindingMismatch,
    #[error("authorization is not currently valid")]
    Expired,
}

impl ApplyAuthorization {
    pub fn issue(
        spec: &ProvisionSpec,
        inspection: &ProvisionPlan,
        confirmed_operation_id: &str,
        now_unix: i64,
    ) -> Result<Self, ApplyAuthorizationError> {
        if spec.kind != ProvisionKind::LegacyReferenceMigration {
            return Err(ApplyAuthorizationError::WrongProvisionKind);
        }
        if confirmed_operation_id != spec.operation_id {
            return Err(ApplyAuthorizationError::ConfirmationMismatch);
        }
        if now_unix < 0
            || now_unix
                .checked_add(AUTHORIZATION_LIFETIME_SECONDS)
                .is_none()
        {
            return Err(ApplyAuthorizationError::InvalidTimestamp);
        }
        if !inspection.ready_for_legacy_authorization(spec) {
            return Err(ApplyAuthorizationError::InspectionNotReady);
        }
        Self::issue_from_ready_inspection(spec, inspection, now_unix)
    }

    /// Issues a real, HMAC-bound authorization from the normal online
    /// inspection for a feature-only matrix guest. The inspection may contain
    /// only the global matrix/safety-audit blocker; no operational blocker is
    /// ignored and the normal production capability remains unavailable.
    #[cfg(feature = "bare-metal-fault-matrix")]
    pub fn issue_bare_metal_fault_matrix(
        spec: &ProvisionSpec,
        inspection: &ProvisionPlan,
        confirmed_operation_id: &str,
        now_unix: i64,
    ) -> Result<Self, ApplyAuthorizationError> {
        if crate::bare_metal_fault_matrix::require_installed_fault_case(&spec.operation_id).is_err()
        {
            return Err(ApplyAuthorizationError::InspectionNotReady);
        }
        if spec.kind != ProvisionKind::LegacyReferenceMigration {
            return Err(ApplyAuthorizationError::WrongProvisionKind);
        }
        if confirmed_operation_id != spec.operation_id {
            return Err(ApplyAuthorizationError::ConfirmationMismatch);
        }
        if now_unix < 0
            || now_unix
                .checked_add(AUTHORIZATION_LIFETIME_SECONDS)
                .is_none()
        {
            return Err(ApplyAuthorizationError::InvalidTimestamp);
        }
        if !inspection.ready_for_bare_metal_fault_matrix_authorization(spec) {
            return Err(ApplyAuthorizationError::InspectionNotReady);
        }
        Self::issue_from_ready_inspection(spec, inspection, now_unix)
    }

    fn issue_from_ready_inspection(
        spec: &ProvisionSpec,
        inspection: &ProvisionPlan,
        now_unix: i64,
    ) -> Result<Self, ApplyAuthorizationError> {
        let target_redis = inspection
            .target_redis
            .as_ref()
            .ok_or(ApplyAuthorizationError::InspectionNotReady)?;
        let source_redis = inspection
            .source_redis
            .as_ref()
            .ok_or(ApplyAuthorizationError::InspectionNotReady)?;
        let expires_at_unix = now_unix + AUTHORIZATION_LIFETIME_SECONDS;
        let mut authorization = Self {
            authorization_version: AUTHORIZATION_VERSION,
            operation_id: spec.operation_id.clone(),
            manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
            inspect_review_sha256: inspection.review_binding_sha256.clone(),
            inspect_review_binding_hmac_sha256: inspection.review_binding_hmac_sha256.clone(),
            authorized_snapshot_report_sha256: inspection.report_sha256.clone(),
            authorized_snapshot_report_binding_hmac_sha256: inspection
                .report_binding_hmac_sha256
                .clone(),
            reviewed_target_redis_run_id: target_redis.target_run_id.clone(),
            reviewed_target_redis_database_index: target_redis.target_database_index,
            reviewed_source_default_redis_run_id: source_redis.source_default_run_id.clone(),
            reviewed_source_cache_redis_run_id: source_redis.source_cache_run_id.clone(),
            issued_at_unix: now_unix,
            expires_at_unix,
            irreversible_one_shot_approved: true,
            authorization_binding_hmac_sha256: String::new(),
        };
        authorization.authorization_binding_hmac_sha256 =
            spec.apply_authorization_binding_hmac_sha256(&authorization.payload_bytes());
        Ok(authorization)
    }

    /// Verify the single operator decision immediately before the durable
    /// journal is created. Source/target identity and dynamic admission were
    /// already rechecked when this owner-only authorization was issued; apply
    /// now proceeds directly to the source fence and the stopped-state final
    /// inspection.
    pub fn verify_new_apply(
        &self,
        spec: &ProvisionSpec,
        now_unix: i64,
    ) -> Result<(), ApplyAuthorizationError> {
        self.verify_resume_binding(spec)?;
        if now_unix < self.issued_at_unix
            || now_unix > self.expires_at_unix
            || self.expires_at_unix - self.issued_at_unix != AUTHORIZATION_LIFETIME_SECONDS
        {
            return Err(ApplyAuthorizationError::Expired);
        }
        Ok(())
    }

    /// Verify a previously started operation without requiring the reviewed
    /// online inspection to still describe an empty target or the original
    /// 24-hour authorization window to still be open.
    ///
    /// This is deliberately narrower than [`Self::verify`]. It may only be
    /// used after an fsync-durable apply journal already binds the same
    /// operation, manifest, and inspected report. Requiring a fresh operator
    /// authorization during forward recovery would strand a partially
    /// created target and accidentally introduce a second cutover decision.
    pub fn verify_resume_binding(
        &self,
        spec: &ProvisionSpec,
    ) -> Result<(), ApplyAuthorizationError> {
        let lifetime_valid = self.issued_at_unix >= 0
            && self
                .issued_at_unix
                .checked_add(AUTHORIZATION_LIFETIME_SECONDS)
                == Some(self.expires_at_unix);
        if self.authorization_version != AUTHORIZATION_VERSION
            || !self.irreversible_one_shot_approved
            || self.operation_id != spec.operation_id
            || self.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
            || !is_lower_sha256(&self.inspect_review_sha256)
            || !is_lower_sha256(&self.inspect_review_binding_hmac_sha256)
            || !is_lower_sha256(&self.authorized_snapshot_report_sha256)
            || !is_lower_sha256(&self.authorized_snapshot_report_binding_hmac_sha256)
            || !valid_redis_run_id(&self.reviewed_target_redis_run_id)
            || !valid_redis_run_id(&self.reviewed_source_default_redis_run_id)
            || !valid_redis_run_id(&self.reviewed_source_cache_redis_run_id)
            || self.reviewed_target_redis_run_id == self.reviewed_source_default_redis_run_id
            || self.reviewed_target_redis_run_id == self.reviewed_source_cache_redis_run_id
            || !lifetime_valid
            || !spec.verify_apply_authorization_binding_hmac_sha256(
                &self.payload_bytes(),
                &self.authorization_binding_hmac_sha256,
            )
        {
            return Err(ApplyAuthorizationError::BindingMismatch);
        }
        Ok(())
    }

    pub fn write_new(&self, path: &Path) -> Result<(), ApplyAuthorizationError> {
        if !path.is_absolute() {
            return Err(ApplyAuthorizationError::RelativePath);
        }
        if fs::symlink_metadata(path).is_ok() {
            return Err(ApplyAuthorizationError::AlreadyExists);
        }
        let parent = path.parent().ok_or(ApplyAuthorizationError::RelativePath)?;
        let parent_metadata = validate_private_parent(parent)?;
        let temporary = temporary_path(path);
        let bytes = serde_json::to_vec_pretty(self)?;
        let result = write_new_file(&temporary, &bytes).and_then(|()| {
            // `hard_link` is the portable no-replace publication primitive:
            // it atomically fails if another process creates the final name.
            fs::hard_link(&temporary, path)?;
            sync_directory(parent)?;
            fs::remove_file(&temporary)?;
            sync_directory(parent)
        });
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(ApplyAuthorizationError::Io)?;
        let published = fs::symlink_metadata(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if published.uid() != parent_metadata.uid() {
                return Err(ApplyAuthorizationError::UnsafeFileType);
            }
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ApplyAuthorizationError> {
        Self::load_with_file_sha256(path).map(|(authorization, _)| authorization)
    }

    /// Loads and validates the owner-only authorization file, returning the
    /// SHA-256 of the exact raw bytes read from disk. The digest includes the
    /// single trailing newline appended by `write_new`; retries always hash
    /// the complete file bytes before JSON deserialization.
    pub fn load_with_file_sha256(path: &Path) -> Result<(Self, String), ApplyAuthorizationError> {
        if !path.is_absolute() {
            return Err(ApplyAuthorizationError::RelativePath);
        }
        let parent = path.parent().ok_or(ApplyAuthorizationError::RelativePath)?;
        let parent_metadata = validate_private_parent(parent)?;
        let mut file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        let path_metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file()
            || !path_metadata.file_type().is_file()
            || path_metadata.file_type().is_symlink()
        {
            return Err(ApplyAuthorizationError::UnsafeFileType);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.dev() != path_metadata.dev() || metadata.ino() != path_metadata.ino() {
                return Err(ApplyAuthorizationError::UnsafeFileType);
            }
            if metadata.uid() != parent_metadata.uid() {
                return Err(ApplyAuthorizationError::UnsafeFileType);
            }
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(ApplyAuthorizationError::UnsafePermissions);
            }
        }
        if metadata.len() == 0 || metadata.len() > MAX_AUTHORIZATION_BYTES {
            return Err(ApplyAuthorizationError::UnsafeSize);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes)?;
        let file_sha256 = hex::encode(Sha256::digest(&bytes));
        Ok((serde_json::from_slice(&bytes)?, file_sha256))
    }

    fn payload_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&AuthorizationPayload {
            authorization_version: self.authorization_version,
            operation_id: &self.operation_id,
            manifest_binding_hmac_sha256: &self.manifest_binding_hmac_sha256,
            inspect_review_sha256: &self.inspect_review_sha256,
            inspect_review_binding_hmac_sha256: &self.inspect_review_binding_hmac_sha256,
            authorized_snapshot_report_sha256: &self.authorized_snapshot_report_sha256,
            authorized_snapshot_report_binding_hmac_sha256: &self
                .authorized_snapshot_report_binding_hmac_sha256,
            reviewed_target_redis_run_id: &self.reviewed_target_redis_run_id,
            reviewed_target_redis_database_index: self.reviewed_target_redis_database_index,
            reviewed_source_default_redis_run_id: &self.reviewed_source_default_redis_run_id,
            reviewed_source_cache_redis_run_id: &self.reviewed_source_cache_redis_run_id,
            issued_at_unix: self.issued_at_unix,
            expires_at_unix: self.expires_at_unix,
            irreversible_one_shot_approved: self.irreversible_one_shot_approved,
        })
        .expect("authorization payload is serializable")
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("authorization.json");
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

fn sync_directory(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}

fn validate_private_parent(path: &Path) -> Result<fs::Metadata, ApplyAuthorizationError> {
    if !path.is_absolute() {
        return Err(ApplyAuthorizationError::RelativePath);
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(ApplyAuthorizationError::UnsafeFileType);
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ApplyAuthorizationError::UnsafePermissions);
        }
    }
    Ok(metadata)
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_redis_run_id(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn authorization() -> ApplyAuthorization {
        ApplyAuthorization {
            authorization_version: AUTHORIZATION_VERSION,
            operation_id: "018f47b8-5ab1-7a00-8000-000000000001".to_string(),
            manifest_binding_hmac_sha256: "a".repeat(64),
            inspect_review_sha256: "b".repeat(64),
            inspect_review_binding_hmac_sha256: "c".repeat(64),
            authorized_snapshot_report_sha256: "e".repeat(64),
            authorized_snapshot_report_binding_hmac_sha256: "f".repeat(64),
            reviewed_target_redis_run_id: "1".repeat(40),
            reviewed_target_redis_database_index: 1,
            reviewed_source_default_redis_run_id: "2".repeat(40),
            reviewed_source_cache_redis_run_id: "3".repeat(40),
            issued_at_unix: 1_700_000_000,
            expires_at_unix: 1_700_000_000 + AUTHORIZATION_LIFETIME_SECONDS,
            irreversible_one_shot_approved: true,
            authorization_binding_hmac_sha256: "d".repeat(64),
        }
    }

    #[test]
    fn authorization_file_is_owner_only_strict_and_no_replace() {
        let root = std::env::temp_dir().join(format!(
            "v2board-apply-authorization-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create authorization test directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .expect("make authorization directory private");
        }
        let path = root.join("authorization.json");
        let expected = authorization();
        expected.write_new(&path).expect("write authorization");
        let raw = fs::read(&path).expect("read exact authorization bytes");
        assert_eq!(raw.last(), Some(&b'\n'));
        let (loaded, file_sha256) =
            ApplyAuthorization::load_with_file_sha256(&path).expect("load authorization digest");
        assert_eq!(loaded, expected);
        assert_eq!(file_sha256, hex::encode(Sha256::digest(&raw)));
        assert!(matches!(
            expected.write_new(&path),
            Err(ApplyAuthorizationError::AlreadyExists)
        ));

        let mut semantically_equal_bytes =
            serde_json::to_vec(&expected).expect("serialize compact");
        semantically_equal_bytes.push(b'\n');
        fs::write(&path, &semantically_equal_bytes).expect("rewrite authorization test bytes");
        let (semantically_equal, changed_file_sha256) =
            ApplyAuthorization::load_with_file_sha256(&path)
                .expect("load semantically equal authorization bytes");
        assert_eq!(semantically_equal, expected);
        assert_ne!(changed_file_sha256, file_sha256);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
                .expect("make authorization unsafe");
            assert!(matches!(
                ApplyAuthorization::load(&path),
                Err(ApplyAuthorizationError::UnsafePermissions)
            ));
        }
        fs::remove_dir_all(root).expect("remove authorization test directory");
    }

    #[test]
    fn resume_hmac_binds_exact_reviewed_redis_identities() {
        let spec = crate::manifest::tests::legacy_spec_for_orchestration();
        let mut authorization = ApplyAuthorization {
            authorization_version: AUTHORIZATION_VERSION,
            operation_id: spec.operation_id.clone(),
            manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
            inspect_review_sha256: "a".repeat(64),
            inspect_review_binding_hmac_sha256: "b".repeat(64),
            authorized_snapshot_report_sha256: "c".repeat(64),
            authorized_snapshot_report_binding_hmac_sha256: "d".repeat(64),
            reviewed_target_redis_run_id: "1".repeat(40),
            reviewed_target_redis_database_index: 1,
            reviewed_source_default_redis_run_id: "2".repeat(40),
            reviewed_source_cache_redis_run_id: "3".repeat(40),
            issued_at_unix: 1_700_000_000,
            expires_at_unix: 1_700_000_000 + AUTHORIZATION_LIFETIME_SECONDS,
            irreversible_one_shot_approved: true,
            authorization_binding_hmac_sha256: String::new(),
        };
        authorization.authorization_binding_hmac_sha256 =
            spec.apply_authorization_binding_hmac_sha256(&authorization.payload_bytes());
        authorization
            .verify_resume_binding(&spec)
            .expect("exact reviewed Redis identities are resumable");
        authorization
            .verify_new_apply(&spec, 1_700_000_000)
            .expect("fresh authorization can start the stopped migration");
        assert!(matches!(
            authorization
                .verify_new_apply(&spec, 1_700_000_000 + AUTHORIZATION_LIFETIME_SECONDS + 1,),
            Err(ApplyAuthorizationError::Expired)
        ));

        let mut changed_database = authorization.clone();
        changed_database.reviewed_target_redis_database_index = 2;
        assert!(matches!(
            changed_database.verify_resume_binding(&spec),
            Err(ApplyAuthorizationError::BindingMismatch)
        ));

        let mut aliased_target = authorization;
        aliased_target.reviewed_target_redis_run_id =
            aliased_target.reviewed_source_default_redis_run_id.clone();
        aliased_target.authorization_binding_hmac_sha256 =
            spec.apply_authorization_binding_hmac_sha256(&aliased_target.payload_bytes());
        assert!(matches!(
            aliased_target.verify_resume_binding(&spec),
            Err(ApplyAuthorizationError::BindingMismatch)
        ));
    }
}
