use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs::{self, File},
    io::{BufReader, Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

use flate2::bufread::GzDecoder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::EntryType;
use thiserror::Error;

const MAX_RELEASE_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_RELEASE_UNPACKED_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_RELEASE_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_RELEASE_ENTRIES: usize = 100_000;
const MAX_SHA256SUMS_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RELEASE_METADATA_BYTES: u64 = 64 * 1024;
const MAX_SYSTEMD_UNIT_BYTES: u64 = 128 * 1024;

const CANONICAL_API_UNIT_BYTES: &[u8] =
    include_bytes!("../../../../../deploy/systemd/v2board-api.service");
const CANONICAL_CLOUDFLARED_UNIT_BYTES: &[u8] =
    include_bytes!("../../../../../deploy/systemd/v2board-cloudflared.service");
const CANONICAL_WORKER_UNIT_BYTES: &[u8] =
    include_bytes!("../../../../../deploy/systemd/v2board-worker.service");

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadOnlyReleaseArchiveInspection {
    pub release_id: String,
    pub archive_sha256: String,
    pub source_revision: String,
    pub entry_count: u64,
    pub regular_file_count: u64,
    pub internal_checksum_count: u64,
    pub virtual_tree_sha256: String,
    pub complete_structure_verified: bool,
    pub internal_sha256sums_verified: bool,
    pub systemd_contract_verified: bool,
    pub target_filesystem_unchanged: bool,
}

#[derive(Debug, Error)]
#[error("{code}")]
pub struct ReleaseArchiveError {
    code: &'static str,
}

impl ReleaseArchiveError {
    const fn new(code: &'static str) -> Self {
        Self { code }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndexedEntryKind {
    Regular,
    Directory,
    Symlink,
}

#[derive(Clone, Debug)]
struct IndexedEntry {
    kind: IndexedEntryKind,
    mode: u32,
    size: u64,
    sha256: Option<String>,
    captured: Option<Vec<u8>>,
    link_target: Option<PathBuf>,
}

pub fn inspect_native_release_archive_read_only(
    archive_path: &Path,
    release_id: &str,
    expected_archive_sha256: &str,
) -> Result<ReadOnlyReleaseArchiveInspection, ReleaseArchiveError> {
    validate_release_binding(release_id, expected_archive_sha256)?;
    let mut file = open_release_archive(archive_path)?;
    let before = file
        .metadata()
        .map_err(|_| ReleaseArchiveError::new("release_archive_metadata_failed"))?;
    let archive_sha256 = hash_open_file(&mut file)?;
    if archive_sha256 != expected_archive_sha256 {
        return Err(ReleaseArchiveError::new("release_archive_digest_mismatch"));
    }
    let inspection = inspect_open_archive(&mut file, release_id, &archive_sha256)?;
    if hash_open_file(&mut file)? != archive_sha256 {
        return Err(ReleaseArchiveError::new(
            "release_archive_changed_during_inspection",
        ));
    }
    let after = fs::symlink_metadata(archive_path)
        .map_err(|_| ReleaseArchiveError::new("release_archive_metadata_failed"))?;
    if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len() {
        return Err(ReleaseArchiveError::new("release_archive_path_replaced"));
    }
    Ok(inspection)
}

fn validate_release_binding(
    release_id: &str,
    expected_sha256: &str,
) -> Result<(), ReleaseArchiveError> {
    if release_id.is_empty()
        || matches!(release_id, "." | "..")
        || release_id.len() > 128
        || !release_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ReleaseArchiveError::new("release_id_invalid"));
    }
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ReleaseArchiveError::new("release_archive_sha256_invalid"));
    }
    Ok(())
}

fn open_release_archive(path: &Path) -> Result<File, ReleaseArchiveError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|_| ReleaseArchiveError::new("release_archive_open_failed"))?;
    let file =
        File::open(path).map_err(|_| ReleaseArchiveError::new("release_archive_open_failed"))?;
    let opened = file
        .metadata()
        .map_err(|_| ReleaseArchiveError::new("release_archive_metadata_failed"))?;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || !opened.is_file()
        || path_metadata.dev() != opened.dev()
        || path_metadata.ino() != opened.ino()
        || opened.uid() != 0
        || opened.permissions().mode() & 0o777 != 0o400
        || opened.len() == 0
        || opened.len() > MAX_RELEASE_ARCHIVE_BYTES
    {
        return Err(ReleaseArchiveError::new("release_archive_unsafe"));
    }
    Ok(file)
}

fn hash_open_file(file: &mut File) -> Result<String, ReleaseArchiveError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ReleaseArchiveError::new("release_archive_seek_failed"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| ReleaseArchiveError::new("release_archive_read_failed"))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| ReleaseArchiveError::new("release_archive_size_overflow"))?;
        if total > MAX_RELEASE_ARCHIVE_BYTES {
            return Err(ReleaseArchiveError::new("release_archive_unsafe"));
        }
        digest.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ReleaseArchiveError::new("release_archive_seek_failed"))?;
    Ok(hex::encode(digest.finalize()))
}

fn inspect_open_archive(
    file: &mut File,
    release_id: &str,
    archive_sha256: &str,
) -> Result<ReadOnlyReleaseArchiveInspection, ReleaseArchiveError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ReleaseArchiveError::new("release_archive_seek_failed"))?;
    let compressed_size = file
        .metadata()
        .map_err(|_| ReleaseArchiveError::new("release_archive_metadata_failed"))?
        .len();
    let decoder = GzDecoder::new(BufReader::new(&mut *file));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|_| ReleaseArchiveError::new("release_archive_entries_invalid"))?;
    let mut indexed = BTreeMap::new();
    let mut unpacked = 0_u64;
    let mut entry_count = 0_usize;
    for entry in entries {
        let mut entry =
            entry.map_err(|_| ReleaseArchiveError::new("release_archive_entry_invalid"))?;
        count_archive_entry(&mut entry_count)?;
        let kind = match entry.header().entry_type() {
            EntryType::Regular => IndexedEntryKind::Regular,
            EntryType::Directory => IndexedEntryKind::Directory,
            EntryType::Symlink => IndexedEntryKind::Symlink,
            _ => {
                return Err(ReleaseArchiveError::new(
                    "release_archive_entry_type_forbidden",
                ));
            }
        };
        let path = normalize_path(
            entry
                .path()
                .map_err(|_| ReleaseArchiveError::new("release_archive_path_invalid"))?
                .as_ref(),
        )?;
        if path.as_os_str().is_empty() {
            if kind == IndexedEntryKind::Directory {
                continue;
            }
            return Err(ReleaseArchiveError::new("release_archive_path_invalid"));
        }
        if path.to_str().is_none()
            || path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(is_forbidden_legacy_filename)
            || indexed.contains_key(&path)
        {
            return Err(ReleaseArchiveError::new("release_archive_path_invalid"));
        }
        let mode = entry
            .header()
            .mode()
            .map_err(|_| ReleaseArchiveError::new("release_archive_mode_invalid"))?;
        let expected_mode = match kind {
            IndexedEntryKind::Directory => 0o755,
            IndexedEntryKind::Symlink => 0o777,
            IndexedEntryKind::Regular if path.parent() == Some(Path::new("bin")) => 0o755,
            IndexedEntryKind::Regular => 0o644,
        };
        if mode != expected_mode {
            return Err(ReleaseArchiveError::new("release_archive_mode_invalid"));
        }
        let size = entry.size();
        if size > MAX_RELEASE_FILE_BYTES || (kind != IndexedEntryKind::Regular && size != 0) {
            return Err(ReleaseArchiveError::new(
                "release_archive_entry_size_invalid",
            ));
        }
        unpacked = unpacked
            .checked_add(size)
            .ok_or_else(|| ReleaseArchiveError::new("release_archive_size_overflow"))?;
        if unpacked > MAX_RELEASE_UNPACKED_BYTES {
            return Err(ReleaseArchiveError::new(
                "release_archive_unpacked_size_exceeded",
            ));
        }
        let (sha256, captured, link_target) = match kind {
            IndexedEntryKind::Regular => {
                if entry.link_name_bytes().is_some() {
                    return Err(ReleaseArchiveError::new(
                        "release_archive_unexpected_link_target",
                    ));
                }
                let capture_limit = match path.to_str() {
                    Some("SHA256SUMS") => Some(MAX_SHA256SUMS_BYTES),
                    Some("RELEASE") => Some(MAX_RELEASE_METADATA_BYTES),
                    Some(
                        "systemd/v2board-api.service"
                        | "systemd/v2board-cloudflared.service"
                        | "systemd/v2board-worker.service",
                    ) => Some(MAX_SYSTEMD_UNIT_BYTES),
                    _ => None,
                };
                if capture_limit.is_some_and(|limit| size == 0 || size > limit) {
                    return Err(ReleaseArchiveError::new(
                        "release_archive_contract_file_size_invalid",
                    ));
                }
                let mut digest = Sha256::new();
                let mut captured = capture_limit.map(|_| Vec::with_capacity(size as usize));
                let mut read_size = 0_u64;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = entry.read(&mut buffer).map_err(|_| {
                        ReleaseArchiveError::new("release_archive_entry_read_failed")
                    })?;
                    if count == 0 {
                        break;
                    }
                    read_size = read_size
                        .checked_add(count as u64)
                        .ok_or_else(|| ReleaseArchiveError::new("release_archive_size_overflow"))?;
                    digest.update(&buffer[..count]);
                    if let Some(bytes) = captured.as_mut() {
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                }
                if read_size != size {
                    return Err(ReleaseArchiveError::new(
                        "release_archive_entry_size_mismatch",
                    ));
                }
                (Some(hex::encode(digest.finalize())), captured, None)
            }
            IndexedEntryKind::Directory => {
                if entry.link_name_bytes().is_some() {
                    return Err(ReleaseArchiveError::new(
                        "release_archive_unexpected_link_target",
                    ));
                }
                (None, None, None)
            }
            IndexedEntryKind::Symlink => {
                let target = entry
                    .link_name()
                    .map_err(|_| {
                        ReleaseArchiveError::new("release_archive_symlink_target_invalid")
                    })?
                    .ok_or_else(|| {
                        ReleaseArchiveError::new("release_archive_symlink_target_missing")
                    })?
                    .into_owned();
                validate_symlink(&path, &target)?;
                (None, None, Some(target))
            }
        };
        indexed.insert(
            path,
            IndexedEntry {
                kind,
                mode,
                size,
                sha256,
                captured,
                link_target,
            },
        );
    }
    // `tar` stops at its end markers. Drain the gzip member and prove that the
    // only remaining uncompressed tar padding is zero, then prove there is no
    // second gzip member or opaque compressed suffix outside the indexed tree.
    let mut decoder = archive.into_inner();
    let mut trailing = [0_u8; 64 * 1024];
    loop {
        let count = decoder
            .read(&mut trailing)
            .map_err(|_| ReleaseArchiveError::new("release_archive_trailer_invalid"))?;
        if count == 0 {
            break;
        }
        if trailing[..count].iter().any(|byte| *byte != 0) {
            return Err(ReleaseArchiveError::new("release_archive_trailing_payload"));
        }
    }
    let mut compressed = decoder.into_inner();
    if !compressed.buffer().is_empty()
        || compressed
            .stream_position()
            .map_err(|_| ReleaseArchiveError::new("release_archive_seek_failed"))?
            != compressed_size
    {
        return Err(ReleaseArchiveError::new(
            "release_archive_compressed_suffix",
        ));
    }
    if indexed.is_empty() {
        return Err(ReleaseArchiveError::new("release_archive_empty"));
    }
    validate_parents(&indexed)?;
    require_exact_children(
        &indexed,
        Path::new(""),
        &["bin", "frontend", "systemd", "RELEASE", "SHA256SUMS"],
    )?;
    require_exact_children(
        &indexed,
        Path::new("bin"),
        &["v2board-api", "v2board-workers", "v2board-analytics-schema"],
    )?;
    require_exact_children(
        &indexed,
        Path::new("systemd"),
        &[
            "v2board-api.service",
            "v2board-cloudflared.service",
            "v2board-worker.service",
        ],
    )?;
    require_exact_children(
        &indexed,
        Path::new("frontend"),
        &["current", "previous", "releases"],
    )?;
    for path in [
        "bin/v2board-api",
        "bin/v2board-workers",
        "bin/v2board-analytics-schema",
    ] {
        let binary = require_kind(&indexed, Path::new(path), IndexedEntryKind::Regular)?;
        if binary.size == 0 || binary.mode & 0o111 == 0 {
            return Err(ReleaseArchiveError::new("release_archive_binary_invalid"));
        }
    }
    for path in ["frontend/current", "frontend/previous"] {
        let link = require_kind(&indexed, Path::new(path), IndexedEntryKind::Symlink)?;
        let resolved = resolve_link(Path::new(path), link)?;
        require_kind(&indexed, &resolved, IndexedEntryKind::Directory)?;
        if path == "frontend/current" {
            for index in [
                resolved.join("user/index.html"),
                resolved.join("admin/index.html"),
            ] {
                let file = require_kind(&indexed, &index, IndexedEntryKind::Regular)?;
                if !(1..=1024 * 1024).contains(&file.size) {
                    return Err(ReleaseArchiveError::new("release_archive_frontend_invalid"));
                }
            }
        }
    }
    let source_revision = parse_release_metadata(captured_utf8(&indexed, "RELEASE")?)?;
    if captured_bytes(&indexed, "systemd/v2board-api.service")? != CANONICAL_API_UNIT_BYTES
        || captured_bytes(&indexed, "systemd/v2board-cloudflared.service")?
            != CANONICAL_CLOUDFLARED_UNIT_BYTES
        || captured_bytes(&indexed, "systemd/v2board-worker.service")?
            != CANONICAL_WORKER_UNIT_BYTES
    {
        return Err(ReleaseArchiveError::new("release_systemd_contract_invalid"));
    }
    let internal_checksum_count = verify_sha256sums(&indexed)?;
    let virtual_tree_sha256 = virtual_tree_sha256(&indexed)?;
    let regular_file_count = indexed
        .values()
        .filter(|entry| entry.kind == IndexedEntryKind::Regular)
        .count();
    Ok(ReadOnlyReleaseArchiveInspection {
        release_id: release_id.to_string(),
        archive_sha256: archive_sha256.to_string(),
        source_revision,
        entry_count: entry_count
            .try_into()
            .map_err(|_| ReleaseArchiveError::new("release_archive_too_many_entries"))?,
        regular_file_count: regular_file_count
            .try_into()
            .map_err(|_| ReleaseArchiveError::new("release_archive_too_many_entries"))?,
        internal_checksum_count,
        virtual_tree_sha256,
        complete_structure_verified: true,
        internal_sha256sums_verified: true,
        systemd_contract_verified: true,
        target_filesystem_unchanged: true,
    })
}

fn count_archive_entry(entry_count: &mut usize) -> Result<(), ReleaseArchiveError> {
    *entry_count = entry_count.saturating_add(1);
    if *entry_count > MAX_RELEASE_ENTRIES {
        return Err(ReleaseArchiveError::new("release_archive_too_many_entries"));
    }
    Ok(())
}

fn is_forbidden_legacy_filename(name: &str) -> bool {
    matches!(
        name,
        "umi.js"
            | "umi.css"
            | "components.chunk.css"
            | "vendors.async.js"
            | "components.async.js"
            | "env.example.js"
            | "custom.css"
            | "custom.js"
    )
}

fn normalize_path(path: &Path) -> Result<PathBuf, ReleaseArchiveError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ReleaseArchiveError::new(
                    "release_archive_path_not_relative",
                ));
            }
        }
    }
    Ok(normalized)
}

fn validate_symlink(path: &Path, target: &Path) -> Result<(), ReleaseArchiveError> {
    if !matches!(
        path.to_str(),
        Some("frontend/current" | "frontend/previous")
    ) || target.is_absolute()
    {
        return Err(ReleaseArchiveError::new(
            "release_archive_symlink_not_allowed",
        ));
    }
    let mut resolved = path
        .parent()
        .ok_or_else(|| ReleaseArchiveError::new("release_archive_symlink_parent_missing"))?
        .to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => resolved.push(value),
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(ReleaseArchiveError::new(
                        "release_archive_symlink_escaped_root",
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ReleaseArchiveError::new(
                    "release_archive_symlink_target_absolute",
                ));
            }
        }
    }
    if !resolved.starts_with("frontend/releases") || resolved == Path::new("frontend/releases") {
        return Err(ReleaseArchiveError::new(
            "release_archive_symlink_target_outside_releases",
        ));
    }
    Ok(())
}

fn validate_parents(indexed: &BTreeMap<PathBuf, IndexedEntry>) -> Result<(), ReleaseArchiveError> {
    for path in indexed.keys() {
        let mut parent = path.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            require_kind(indexed, value, IndexedEntryKind::Directory)?;
            parent = value.parent();
        }
    }
    Ok(())
}

fn require_exact_children(
    indexed: &BTreeMap<PathBuf, IndexedEntry>,
    parent: &Path,
    expected: &[&str],
) -> Result<(), ReleaseArchiveError> {
    let actual = indexed
        .keys()
        .filter(|path| path.parent() == Some(parent))
        .filter_map(|path| path.file_name().and_then(OsStr::to_str))
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(ReleaseArchiveError::new(
            "release_archive_structure_invalid",
        ));
    }
    Ok(())
}

fn require_kind<'a>(
    indexed: &'a BTreeMap<PathBuf, IndexedEntry>,
    path: &Path,
    kind: IndexedEntryKind,
) -> Result<&'a IndexedEntry, ReleaseArchiveError> {
    indexed
        .get(path)
        .filter(|entry| entry.kind == kind)
        .ok_or_else(|| ReleaseArchiveError::new("release_archive_structure_invalid"))
}

fn resolve_link(path: &Path, entry: &IndexedEntry) -> Result<PathBuf, ReleaseArchiveError> {
    let target = entry
        .link_target
        .as_deref()
        .ok_or_else(|| ReleaseArchiveError::new("release_archive_symlink_target_missing"))?;
    let mut resolved = path
        .parent()
        .ok_or_else(|| ReleaseArchiveError::new("release_archive_symlink_parent_missing"))?
        .to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => resolved.push(value),
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(ReleaseArchiveError::new(
                        "release_archive_symlink_escaped_root",
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ReleaseArchiveError::new(
                    "release_archive_symlink_target_absolute",
                ));
            }
        }
    }
    Ok(resolved)
}

fn captured_bytes<'a>(
    indexed: &'a BTreeMap<PathBuf, IndexedEntry>,
    path: &str,
) -> Result<&'a [u8], ReleaseArchiveError> {
    require_kind(indexed, Path::new(path), IndexedEntryKind::Regular)?
        .captured
        .as_deref()
        .ok_or_else(|| ReleaseArchiveError::new("release_archive_contract_file_missing"))
}

fn captured_utf8<'a>(
    indexed: &'a BTreeMap<PathBuf, IndexedEntry>,
    path: &str,
) -> Result<&'a str, ReleaseArchiveError> {
    std::str::from_utf8(captured_bytes(indexed, path)?)
        .map_err(|_| ReleaseArchiveError::new("release_archive_contract_file_not_utf8"))
}

fn parse_release_metadata(text: &str) -> Result<String, ReleaseArchiveError> {
    if text.contains('\r') || text.as_bytes().contains(&0) {
        return Err(ReleaseArchiveError::new("release_metadata_invalid"));
    }
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| ReleaseArchiveError::new("release_metadata_line_invalid"))?;
        if key.is_empty()
            || value.is_empty()
            || !matches!(
                key,
                "format"
                    | "source_revision"
                    | "target_os"
                    | "target_distribution"
                    | "target_distribution_version"
                    | "target_arch"
            )
            || fields.insert(key, value).is_some()
        {
            return Err(ReleaseArchiveError::new("release_metadata_field_invalid"));
        }
    }
    if fields.len() != 6
        || fields.get("format") != Some(&"v2board-native-release-v1")
        || fields.get("target_os") != Some(&"linux")
        || fields.get("target_distribution") != Some(&"debian")
        || fields.get("target_distribution_version") != Some(&"13")
        || fields.get("target_arch") != Some(&"amd64")
    {
        return Err(ReleaseArchiveError::new(
            "release_metadata_contract_invalid",
        ));
    }
    let revision = fields
        .get("source_revision")
        .copied()
        .ok_or_else(|| ReleaseArchiveError::new("release_source_revision_missing"))?;
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ReleaseArchiveError::new("release_source_revision_invalid"));
    }
    Ok(revision.to_string())
}

fn verify_sha256sums(
    indexed: &BTreeMap<PathBuf, IndexedEntry>,
) -> Result<u64, ReleaseArchiveError> {
    let text = captured_utf8(indexed, "SHA256SUMS")?;
    if text.is_empty() || text.contains('\r') || text.as_bytes().contains(&0) {
        return Err(ReleaseArchiveError::new("release_sha256sums_invalid"));
    }
    let mut covered = BTreeSet::new();
    for line in text.lines() {
        if line.len() < 67 {
            return Err(ReleaseArchiveError::new("release_sha256sums_invalid"));
        }
        let (digest, remainder) = line.split_at(64);
        if !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !remainder.starts_with("  ")
        {
            return Err(ReleaseArchiveError::new("release_sha256sums_invalid"));
        }
        let path = normalize_path(Path::new(&remainder[2..]))?;
        if path.as_os_str().is_empty()
            || path == Path::new("SHA256SUMS")
            || !covered.insert(path.clone())
        {
            return Err(ReleaseArchiveError::new("release_sha256sums_invalid"));
        }
        let entry = require_kind(indexed, &path, IndexedEntryKind::Regular)?;
        if entry.sha256.as_deref() != Some(digest) {
            return Err(ReleaseArchiveError::new("release_internal_checksum_failed"));
        }
    }
    let expected = indexed
        .iter()
        .filter_map(|(path, entry)| {
            (entry.kind == IndexedEntryKind::Regular && path != Path::new("SHA256SUMS"))
                .then_some(path.clone())
        })
        .collect::<BTreeSet<_>>();
    if covered != expected {
        return Err(ReleaseArchiveError::new(
            "release_sha256sums_coverage_incomplete",
        ));
    }
    covered
        .len()
        .try_into()
        .map_err(|_| ReleaseArchiveError::new("release_archive_too_many_entries"))
}

fn virtual_tree_sha256(
    indexed: &BTreeMap<PathBuf, IndexedEntry>,
) -> Result<String, ReleaseArchiveError> {
    let mut digest = Sha256::new();
    digest.update(b"v2board-native-release-archive-index-v1\0");
    for (path, entry) in indexed {
        let path = path
            .to_str()
            .ok_or_else(|| ReleaseArchiveError::new("release_archive_path_invalid"))?;
        digest.update((path.len() as u64).to_be_bytes());
        digest.update(path.as_bytes());
        digest.update(entry.mode.to_be_bytes());
        digest.update(entry.size.to_be_bytes());
        match entry.kind {
            IndexedEntryKind::Regular => {
                digest.update(b"F");
                digest.update(
                    entry
                        .sha256
                        .as_deref()
                        .ok_or_else(|| ReleaseArchiveError::new("release_archive_digest_missing"))?
                        .as_bytes(),
                );
            }
            IndexedEntryKind::Directory => digest.update(b"D"),
            IndexedEntryKind::Symlink => {
                digest.update(b"L");
                let target = entry.link_target.as_deref().ok_or_else(|| {
                    ReleaseArchiveError::new("release_archive_symlink_target_missing")
                })?;
                let target = target
                    .to_str()
                    .ok_or_else(|| {
                        ReleaseArchiveError::new("release_archive_symlink_target_invalid")
                    })?
                    .as_bytes();
                digest.update((target.len() as u64).to_be_bytes());
                digest.update(target);
            }
        }
    }
    Ok(hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        sync::atomic::{AtomicU64, Ordering},
    };

    use flate2::{Compression, write::GzEncoder};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy)]
    enum ArchiveMutation {
        None,
        InternalChecksum,
        MissingChecksum,
        SystemdUnit,
        CloudflaredUnit,
        UnexpectedRootEntry,
        CompressedSuffix,
        SecondGzipMember,
        InvalidMode,
    }

    fn test_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "v2board-release-archive-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn write_complete_archive(mutation: ArchiveMutation) -> PathBuf {
        let path = test_path("complete.tar.gz");
        let mut files = BTreeMap::<&str, Vec<u8>>::from([
            ("bin/v2board-api", b"api-binary".to_vec()),
            ("bin/v2board-workers", b"worker-binary".to_vec()),
            (
                "bin/v2board-analytics-schema",
                b"analytics-schema-binary".to_vec(),
            ),
            (
                "frontend/releases/content-a/user/index.html",
                b"<html>user</html>".to_vec(),
            ),
            (
                "frontend/releases/content-a/admin/index.html",
                b"<html>admin</html>".to_vec(),
            ),
            (
                "systemd/v2board-api.service",
                CANONICAL_API_UNIT_BYTES.to_vec(),
            ),
            (
                "systemd/v2board-cloudflared.service",
                CANONICAL_CLOUDFLARED_UNIT_BYTES.to_vec(),
            ),
            (
                "systemd/v2board-worker.service",
                CANONICAL_WORKER_UNIT_BYTES.to_vec(),
            ),
            (
                "RELEASE",
                concat!(
                    "format=v2board-native-release-v1\n",
                    "source_revision=0123456789abcdef0123456789abcdef01234567\n",
                    "target_os=linux\n",
                    "target_distribution=debian\n",
                    "target_distribution_version=13\n",
                    "target_arch=amd64\n"
                )
                .as_bytes()
                .to_vec(),
            ),
        ]);
        if matches!(mutation, ArchiveMutation::SystemdUnit) {
            files
                .get_mut("systemd/v2board-api.service")
                .expect("API unit")
                .extend_from_slice(b"# drift\n");
        }
        if matches!(mutation, ArchiveMutation::CloudflaredUnit) {
            files
                .get_mut("systemd/v2board-cloudflared.service")
                .expect("cloudflared unit")
                .extend_from_slice(b"# drift\n");
        }
        if matches!(mutation, ArchiveMutation::UnexpectedRootEntry) {
            files.insert("unexpected", b"unexpected".to_vec());
        }
        let mut checksum_lines = files
            .iter()
            .map(|(entry_path, bytes)| {
                format!("{}  {entry_path}\n", hex::encode(Sha256::digest(bytes)))
            })
            .collect::<Vec<_>>();
        if matches!(mutation, ArchiveMutation::MissingChecksum) {
            checksum_lines.pop();
        }
        let mut checksums = checksum_lines.concat();
        if matches!(mutation, ArchiveMutation::InternalChecksum) {
            let replacement = if checksums.starts_with('0') { "1" } else { "0" };
            checksums.replace_range(0..1, replacement);
        }
        files.insert("SHA256SUMS", checksums.into_bytes());

        let file = File::create(&path).expect("archive file");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for directory in [
            "bin",
            "frontend",
            "frontend/releases",
            "frontend/releases/content-a",
            "frontend/releases/content-a/user",
            "frontend/releases/content-a/admin",
            "systemd",
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Directory);
            header.set_mode(0o755);
            header.set_size(0);
            header.set_path(directory).expect("directory path");
            header.set_cksum();
            builder
                .append(&header, io::empty())
                .expect("append directory");
        }
        for (entry_path, bytes) in files {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            let mode = if matches!(mutation, ArchiveMutation::InvalidMode)
                && entry_path == "bin/v2board-api"
            {
                0o100
            } else if entry_path.starts_with("bin/") {
                0o755
            } else {
                0o644
            };
            header.set_mode(mode);
            header.set_size(bytes.len() as u64);
            header.set_path(entry_path).expect("file path");
            header.set_cksum();
            builder
                .append(&header, bytes.as_slice())
                .expect("append file");
        }
        for link in ["frontend/current", "frontend/previous"] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_mode(0o777);
            header.set_size(0);
            header.set_path(link).expect("symlink path");
            header
                .set_link_name("releases/content-a")
                .expect("symlink target");
            header.set_cksum();
            builder
                .append(&header, io::empty())
                .expect("append symlink");
        }
        let encoder = builder.into_inner().expect("finish tar");
        let mut file = encoder.finish().expect("finish gzip");
        match mutation {
            ArchiveMutation::CompressedSuffix => file
                .write_all(b"opaque-suffix")
                .expect("append compressed suffix"),
            ArchiveMutation::SecondGzipMember => {
                let mut second = GzEncoder::new(file, Compression::default());
                second.write_all(b"second-member").expect("second member");
                second.finish().expect("finish second member");
            }
            _ => {}
        }
        path
    }

    fn write_structured_archive(entries: &[(&str, EntryType, Option<&str>)]) -> PathBuf {
        let path = test_path("structured.tar.gz");
        let file = File::create(&path).expect("structured archive");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (entry_path, entry_type, link_target) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(*entry_type);
            header.set_mode(if *entry_type == EntryType::Symlink {
                0o777
            } else {
                0o644
            });
            header.set_size(0);
            header.set_path(entry_path).expect("entry path");
            if let Some(link_target) = link_target {
                header
                    .set_link_name(link_target)
                    .expect("entry link target");
            }
            header.set_cksum();
            builder.append(&header, io::empty()).expect("append entry");
        }
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip");
        path
    }

    fn inspect_fixture(
        path: &Path,
    ) -> Result<ReadOnlyReleaseArchiveInspection, ReleaseArchiveError> {
        let mut file = File::open(path).expect("open archive");
        inspect_open_archive(
            &mut file,
            "release-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
    }

    #[test]
    fn complete_release_tree_is_verified() {
        let archive = write_complete_archive(ArchiveMutation::None);
        let inspection = inspect_fixture(&archive).expect("valid release archive");
        assert_eq!(inspection.entry_count, 19);
        assert_eq!(inspection.regular_file_count, 10);
        assert_eq!(inspection.internal_checksum_count, 9);
        assert_eq!(
            inspection.source_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert!(inspection.complete_structure_verified);
        assert!(inspection.internal_sha256sums_verified);
        assert!(inspection.systemd_contract_verified);
        fs::remove_file(archive).expect("remove archive");
    }

    #[test]
    fn checksum_systemd_tree_mode_and_compressed_suffix_drift_are_rejected() {
        for mutation in [
            ArchiveMutation::InternalChecksum,
            ArchiveMutation::MissingChecksum,
            ArchiveMutation::SystemdUnit,
            ArchiveMutation::CloudflaredUnit,
            ArchiveMutation::UnexpectedRootEntry,
            ArchiveMutation::CompressedSuffix,
            ArchiveMutation::SecondGzipMember,
            ArchiveMutation::InvalidMode,
        ] {
            let archive = write_complete_archive(mutation);
            assert!(inspect_fixture(&archive).is_err());
            fs::remove_file(archive).expect("remove archive");
        }
    }

    #[test]
    fn hardlinks_duplicates_unexpected_link_targets_and_escapes_are_rejected() {
        for entries in [
            vec![("bin/v2board-api", EntryType::Link, Some("outside"))],
            vec![
                ("bin/v2board-api", EntryType::Regular, None),
                ("bin/v2board-api", EntryType::Regular, None),
            ],
            vec![("frontend/current", EntryType::Symlink, Some("../../etc"))],
            vec![("bin/v2board-api", EntryType::Regular, Some("unexpected"))],
        ] {
            let archive = write_structured_archive(&entries);
            assert!(inspect_fixture(&archive).is_err());
            fs::remove_file(archive).expect("remove archive");
        }
        assert!(normalize_path(Path::new("../escape")).is_err());
        assert!(normalize_path(Path::new("/absolute")).is_err());
    }

    #[test]
    fn entry_limit_counts_every_tar_entry_including_repeated_roots() {
        let mut count = MAX_RELEASE_ENTRIES;
        assert_eq!(
            count_archive_entry(&mut count).unwrap_err().code,
            "release_archive_too_many_entries"
        );
    }

    #[test]
    fn path_segment_release_ids_are_rejected() {
        let digest = "a".repeat(64);
        for release_id in [".", ".."] {
            assert_eq!(
                validate_release_binding(release_id, &digest)
                    .unwrap_err()
                    .code,
                "release_id_invalid"
            );
        }
    }
}
