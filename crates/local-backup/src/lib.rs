//! Portable, credential-free PixNya local-data backups.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;

const FORMAT_VERSION: u32 = 1;
const MANIFEST_PATH: &str = "manifest.json";
const FRONTEND_PATH: &str = "components/frontend.json";
const CATALOG_PATH: &str = "components/catalog.json";
const HISTORY_PATH: &str = "components/history.json";
const DOWNLOADS_PATH: &str = "components/downloads.json";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_COMPONENT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_OFFLINE_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_IN_MEMORY_RESTORE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ARCHIVE_UNCOMPRESSED_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendBackupState {
    pub search_history: Vec<String>,
    /// Reading position in millionths, from 0 through 1_000_000.
    pub novel_reading_progress: BTreeMap<String, u32>,
    pub sidebar_expanded: bool,
    pub reduced_motion: bool,
    pub r18_default_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableBackupData {
    pub frontend: FrontendBackupState,
    pub catalog: serde_json::Value,
    pub history: serde_json::Value,
    pub downloads: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineBackupFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineBackupSource {
    pub relative_path: String,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupCreateRequest {
    pub data: PortableBackupData,
    pub include_offline: bool,
    pub offline_files: Vec<OfflineBackupFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreResult {
    pub data: PortableBackupData,
    pub offline_files: Vec<OfflineBackupFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub format_version: u32,
    pub application_version: String,
    pub component_count: u32,
    pub offline_file_count: u32,
    pub offline_included: bool,
    pub total_bytes: u64,
    pub contains_credentials: bool,
}

pub type BackupPreview = BackupSummary;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format_version: u32,
    application_version: String,
    created_at_unix_seconds: u64,
    #[serde(default)]
    offline_included: bool,
    entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    InvalidInput,
    InvalidArchive,
    UnsupportedVersion,
    IntegrityMismatch,
    CapacityExceeded,
    Io,
}

impl fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid backup input",
            Self::InvalidArchive => "invalid backup archive",
            Self::UnsupportedVersion => "backup format is newer than this application",
            Self::IntegrityMismatch => "backup integrity verification failed",
            Self::CapacityExceeded => "backup exceeds the supported capacity",
            Self::Io => "backup filesystem operation failed",
        })
    }
}

impl std::error::Error for BackupError {}

pub struct BackupManager {
    application_version: String,
}

impl BackupManager {
    pub fn new(application_version: impl Into<String>) -> Self {
        Self {
            application_version: application_version.into(),
        }
    }

    pub fn create(
        &self,
        destination: &Path,
        request: BackupCreateRequest,
    ) -> Result<BackupSummary, BackupError> {
        validate_application_version(&self.application_version)?;
        if !request.include_offline && !request.offline_files.is_empty() {
            return Err(BackupError::InvalidInput);
        }
        let parent = destination.parent().ok_or(BackupError::InvalidInput)?;
        fs::create_dir_all(parent).map_err(|_| BackupError::Io)?;

        let mut members = vec![
            member_json(FRONTEND_PATH, &request.data.frontend)?,
            member_json(CATALOG_PATH, &request.data.catalog)?,
            member_json(HISTORY_PATH, &request.data.history)?,
            member_json(DOWNLOADS_PATH, &request.data.downloads)?,
        ];
        let mut names = members
            .iter()
            .map(|member| member.path.clone())
            .collect::<BTreeSet<_>>();
        for file in request.offline_files {
            let relative = normalized_relative_path(&file.relative_path)?;
            if file.bytes.len() as u64 > MAX_OFFLINE_FILE_BYTES {
                return Err(BackupError::CapacityExceeded);
            }
            let path = format!("offline/{relative}");
            if !names.insert(path.clone()) {
                return Err(BackupError::InvalidInput);
            }
            members.push(ArchiveMember {
                path,
                bytes: file.bytes,
            });
        }
        if members.len() + 1 > MAX_ARCHIVE_FILES {
            return Err(BackupError::CapacityExceeded);
        }
        members.sort_by(|left, right| left.path.cmp(&right.path));

        let manifest = BackupManifest {
            format_version: FORMAT_VERSION,
            application_version: self.application_version.clone(),
            created_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| BackupError::Io)?
                .as_secs(),
            offline_included: request.include_offline,
            entries: members
                .iter()
                .map(|member| ManifestEntry {
                    path: member.path.clone(),
                    size_bytes: member.bytes.len() as u64,
                    sha256: sha256_hex(&member.bytes),
                })
                .collect(),
        };
        let manifest_bytes =
            serde_json::to_vec(&manifest).map_err(|_| BackupError::InvalidInput)?;
        if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(BackupError::CapacityExceeded);
        }
        validate_total_uncompressed_size(
            manifest.entries.iter().map(|entry| entry.size_bytes),
            manifest_bytes.len() as u64,
        )?;

        let staging = staging_path(destination)?;
        let result = write_archive(&staging, &manifest_bytes, &members).and_then(|_| {
            if destination.exists() {
                fs::remove_file(destination).map_err(|_| BackupError::Io)?;
            }
            fs::rename(&staging, destination).map_err(|_| BackupError::Io)
        });
        if result.is_err() {
            let _ = fs::remove_file(&staging);
        }
        result?;
        Ok(summary_from_manifest(&manifest))
    }

    pub fn create_from_sources(
        &self,
        destination: &Path,
        data: PortableBackupData,
        include_offline: bool,
        offline_sources: Vec<OfflineBackupSource>,
    ) -> Result<BackupSummary, BackupError> {
        validate_application_version(&self.application_version)?;
        if !include_offline && !offline_sources.is_empty() {
            return Err(BackupError::InvalidInput);
        }
        let parent = destination.parent().ok_or(BackupError::InvalidInput)?;
        fs::create_dir_all(parent).map_err(|_| BackupError::Io)?;
        let mut members = vec![
            member_json(FRONTEND_PATH, &data.frontend)?,
            member_json(CATALOG_PATH, &data.catalog)?,
            member_json(HISTORY_PATH, &data.history)?,
            member_json(DOWNLOADS_PATH, &data.downloads)?,
        ];
        let mut names = members
            .iter()
            .map(|member| member.path.clone())
            .collect::<BTreeSet<_>>();
        let mut files = Vec::with_capacity(offline_sources.len());
        for source in offline_sources {
            let relative_path = normalized_relative_path(&source.relative_path)?;
            let metadata =
                fs::symlink_metadata(&source.source_path).map_err(|_| BackupError::Io)?;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > MAX_OFFLINE_FILE_BYTES
            {
                return Err(BackupError::InvalidInput);
            }
            let path = format!("offline/{relative_path}");
            if !names.insert(path.clone()) {
                return Err(BackupError::InvalidInput);
            }
            files.push(ArchiveFileSource {
                path,
                size_bytes: metadata.len(),
                sha256: sha256_file(&source.source_path)?,
                source_path: source.source_path,
            });
        }
        if members.len() + files.len() + 1 > MAX_ARCHIVE_FILES {
            return Err(BackupError::CapacityExceeded);
        }
        members.sort_by(|left, right| left.path.cmp(&right.path));
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let mut entries = members
            .iter()
            .map(|member| ManifestEntry {
                path: member.path.clone(),
                size_bytes: member.bytes.len() as u64,
                sha256: sha256_hex(&member.bytes),
            })
            .collect::<Vec<_>>();
        entries.extend(files.iter().map(|source| ManifestEntry {
            path: source.path.clone(),
            size_bytes: source.size_bytes,
            sha256: source.sha256.clone(),
        }));
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = BackupManifest {
            format_version: FORMAT_VERSION,
            application_version: self.application_version.clone(),
            created_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| BackupError::Io)?
                .as_secs(),
            offline_included: include_offline,
            entries,
        };
        let manifest_bytes =
            serde_json::to_vec(&manifest).map_err(|_| BackupError::InvalidInput)?;
        if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(BackupError::CapacityExceeded);
        }
        validate_total_uncompressed_size(
            manifest.entries.iter().map(|entry| entry.size_bytes),
            manifest_bytes.len() as u64,
        )?;
        let staging = staging_path(destination)?;
        let result = write_archive_with_files(&staging, &manifest_bytes, &members, &files)
            .and_then(|_| {
                if destination.exists() {
                    fs::remove_file(destination).map_err(|_| BackupError::Io)?;
                }
                fs::rename(&staging, destination).map_err(|_| BackupError::Io)
            });
        if result.is_err() {
            let _ = fs::remove_file(&staging);
        }
        result?;
        Ok(summary_from_manifest(&manifest))
    }

    pub fn inspect(&self, source: &Path) -> Result<BackupPreview, BackupError> {
        Ok(summary_from_manifest(&verify_archive_streaming(source)?))
    }

    pub fn restore(&self, source: &Path) -> Result<BackupRestoreResult, BackupError> {
        let mut verified = read_verified_archive(source)?;
        let frontend = take_json(&mut verified.members, FRONTEND_PATH)?;
        let catalog = take_json(&mut verified.members, CATALOG_PATH)?;
        let history = take_json(&mut verified.members, HISTORY_PATH)?;
        let downloads = take_json(&mut verified.members, DOWNLOADS_PATH)?;
        let mut offline_files = Vec::new();
        for (path, bytes) in verified.members {
            let relative = path
                .strip_prefix("offline/")
                .ok_or(BackupError::InvalidArchive)?;
            offline_files.push(OfflineBackupFile {
                relative_path: normalized_relative_path(relative)?,
                bytes,
            });
        }
        offline_files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(BackupRestoreResult {
            data: PortableBackupData {
                frontend: serde_json::from_value(frontend)
                    .map_err(|_| BackupError::InvalidArchive)?,
                catalog,
                history,
                downloads,
            },
            offline_files,
        })
    }

    pub fn restore_to_directory(
        &self,
        source: &Path,
        offline_destination: &Path,
    ) -> Result<PortableBackupData, BackupError> {
        if offline_destination.exists() {
            return Err(BackupError::InvalidInput);
        }
        let parent = offline_destination
            .parent()
            .ok_or(BackupError::InvalidInput)?;
        fs::create_dir_all(parent).map_err(|_| BackupError::Io)?;
        fs::create_dir(offline_destination).map_err(|_| BackupError::Io)?;
        let result = restore_archive_streaming(source, offline_destination);
        if result.is_err() {
            let _ = fs::remove_dir_all(offline_destination);
        }
        result
    }
}

fn verify_archive_streaming(source: &Path) -> Result<BackupManifest, BackupError> {
    let file = File::open(source).map_err(|_| BackupError::Io)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| BackupError::InvalidArchive)?;
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_FILES {
        return Err(BackupError::CapacityExceeded);
    }
    let manifest = {
        let mut member = archive
            .by_name(MANIFEST_PATH)
            .map_err(|_| BackupError::InvalidArchive)?;
        if member.size() > MAX_MANIFEST_BYTES {
            return Err(BackupError::CapacityExceeded);
        }
        let mut bytes = Vec::with_capacity(member.size() as usize);
        member
            .by_ref()
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| BackupError::InvalidArchive)?;
        serde_json::from_slice::<BackupManifest>(&bytes).map_err(|_| BackupError::InvalidArchive)?
    };
    let manifest_size = archive
        .by_name(MANIFEST_PATH)
        .map_err(|_| BackupError::InvalidArchive)?
        .size();
    if manifest.format_version > FORMAT_VERSION {
        return Err(BackupError::UnsupportedVersion);
    }
    if manifest.format_version != FORMAT_VERSION {
        return Err(BackupError::InvalidArchive);
    }
    validate_application_version(&manifest.application_version)?;
    if manifest.entries.len() + 1 != archive.len() {
        return Err(BackupError::IntegrityMismatch);
    }
    let mut declarations = BTreeMap::new();
    for entry in &manifest.entries {
        validate_archive_member_name(&entry.path)?;
        if entry.path == MANIFEST_PATH || declarations.insert(entry.path.clone(), entry).is_some() {
            return Err(BackupError::InvalidArchive);
        }
    }
    validate_declared_members(manifest.offline_included, declarations.keys())?;
    validate_total_uncompressed_size(
        manifest.entries.iter().map(|entry| entry.size_bytes),
        manifest_size,
    )?;
    let mut seen = BTreeSet::new();
    let mut total_uncompressed_bytes = manifest_size;
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|_| BackupError::InvalidArchive)?;
        let name = member.name().to_owned();
        if name == MANIFEST_PATH {
            if !seen.insert(name) {
                return Err(BackupError::InvalidArchive);
            }
            continue;
        }
        validate_archive_member_name(&name)?;
        if member.is_dir() || member.enclosed_name().is_none() || !seen.insert(name.clone()) {
            return Err(BackupError::InvalidArchive);
        }
        let declared = declarations
            .get(&name)
            .ok_or(BackupError::IntegrityMismatch)?;
        if member.size() != declared.size_bytes {
            return Err(BackupError::IntegrityMismatch);
        }
        let limit = if name.starts_with("offline/") {
            MAX_OFFLINE_FILE_BYTES
        } else {
            MAX_COMPONENT_BYTES
        };
        if member.size() > limit {
            return Err(BackupError::CapacityExceeded);
        }
        let mut digest = Sha256::new();
        let remaining = MAX_ARCHIVE_UNCOMPRESSED_BYTES
            .checked_sub(total_uncompressed_bytes)
            .ok_or(BackupError::CapacityExceeded)?;
        let copied = copy_with_digest(
            &mut member,
            &mut std::io::sink(),
            &mut digest,
            remaining.min(declared.size_bytes),
        )?;
        total_uncompressed_bytes = total_uncompressed_bytes
            .checked_add(copied)
            .ok_or(BackupError::CapacityExceeded)?;
        if copied != declared.size_bytes || format!("{:x}", digest.finalize()) != declared.sha256 {
            return Err(BackupError::IntegrityMismatch);
        }
    }
    if declarations.keys().any(|name| !seen.contains(name)) {
        return Err(BackupError::IntegrityMismatch);
    }
    Ok(manifest)
}

fn restore_archive_streaming(
    source: &Path,
    offline_destination: &Path,
) -> Result<PortableBackupData, BackupError> {
    let file = File::open(source).map_err(|_| BackupError::Io)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| BackupError::InvalidArchive)?;
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_FILES {
        return Err(BackupError::CapacityExceeded);
    }
    let manifest = {
        let mut member = archive
            .by_name(MANIFEST_PATH)
            .map_err(|_| BackupError::InvalidArchive)?;
        if member.size() > MAX_MANIFEST_BYTES {
            return Err(BackupError::CapacityExceeded);
        }
        let mut bytes = Vec::with_capacity(member.size() as usize);
        member
            .by_ref()
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| BackupError::InvalidArchive)?;
        serde_json::from_slice::<BackupManifest>(&bytes).map_err(|_| BackupError::InvalidArchive)?
    };
    let manifest_size = archive
        .by_name(MANIFEST_PATH)
        .map_err(|_| BackupError::InvalidArchive)?
        .size();
    if manifest.format_version > FORMAT_VERSION {
        return Err(BackupError::UnsupportedVersion);
    }
    if manifest.format_version != FORMAT_VERSION {
        return Err(BackupError::InvalidArchive);
    }
    validate_application_version(&manifest.application_version)?;
    if manifest.entries.len() + 1 != archive.len() {
        return Err(BackupError::IntegrityMismatch);
    }
    let mut declarations = BTreeMap::new();
    for entry in manifest.entries {
        validate_archive_member_name(&entry.path)?;
        if entry.path == MANIFEST_PATH || declarations.insert(entry.path.clone(), entry).is_some() {
            return Err(BackupError::InvalidArchive);
        }
    }
    validate_declared_members(manifest.offline_included, declarations.keys())?;
    validate_total_uncompressed_size(
        declarations.values().map(|entry| entry.size_bytes),
        manifest_size,
    )?;

    let mut seen = BTreeSet::new();
    let mut components = BTreeMap::new();
    let mut total_uncompressed_bytes = manifest_size;
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|_| BackupError::InvalidArchive)?;
        let name = member.name().to_owned();
        if name == MANIFEST_PATH {
            if !seen.insert(name) {
                return Err(BackupError::InvalidArchive);
            }
            continue;
        }
        validate_archive_member_name(&name)?;
        if member.is_dir() || member.enclosed_name().is_none() || !seen.insert(name.clone()) {
            return Err(BackupError::InvalidArchive);
        }
        let declared = declarations
            .get(&name)
            .ok_or(BackupError::IntegrityMismatch)?;
        if member.size() != declared.size_bytes {
            return Err(BackupError::IntegrityMismatch);
        }
        let mut digest = Sha256::new();
        let remaining = MAX_ARCHIVE_UNCOMPRESSED_BYTES
            .checked_sub(total_uncompressed_bytes)
            .ok_or(BackupError::CapacityExceeded)?;
        let member_limit = remaining.min(declared.size_bytes);
        if let Some(relative) = name.strip_prefix("offline/") {
            if member.size() > MAX_OFFLINE_FILE_BYTES {
                return Err(BackupError::CapacityExceeded);
            }
            let relative = normalized_relative_path(relative)?;
            let target = offline_destination.join(relative);
            let target_parent = target.parent().ok_or(BackupError::InvalidArchive)?;
            fs::create_dir_all(target_parent).map_err(|_| BackupError::Io)?;
            let mut output = File::create(&target).map_err(|_| BackupError::Io)?;
            let copied = copy_with_digest(&mut member, &mut output, &mut digest, member_limit)?;
            if copied != declared.size_bytes {
                return Err(BackupError::IntegrityMismatch);
            }
            output.sync_all().map_err(|_| BackupError::Io)?;
        } else {
            if member.size() > MAX_COMPONENT_BYTES {
                return Err(BackupError::CapacityExceeded);
            }
            let mut bytes = Vec::with_capacity(member.size() as usize);
            let copied = copy_with_digest(&mut member, &mut bytes, &mut digest, member_limit)?;
            if copied != declared.size_bytes {
                return Err(BackupError::IntegrityMismatch);
            }
            components.insert(name.clone(), bytes);
        }
        if format!("{:x}", digest.finalize()) != declared.sha256 {
            return Err(BackupError::IntegrityMismatch);
        }
        total_uncompressed_bytes = total_uncompressed_bytes
            .checked_add(declared.size_bytes)
            .ok_or(BackupError::CapacityExceeded)?;
    }
    if declarations.keys().any(|name| !seen.contains(name)) {
        return Err(BackupError::IntegrityMismatch);
    }
    let frontend = take_json(&mut components, FRONTEND_PATH)?;
    let catalog = take_json(&mut components, CATALOG_PATH)?;
    let history = take_json(&mut components, HISTORY_PATH)?;
    let downloads = take_json(&mut components, DOWNLOADS_PATH)?;
    if !components.is_empty() {
        return Err(BackupError::InvalidArchive);
    }
    Ok(PortableBackupData {
        frontend: serde_json::from_value(frontend).map_err(|_| BackupError::InvalidArchive)?,
        catalog,
        history,
        downloads,
    })
}

fn copy_with_digest(
    input: &mut impl Read,
    output: &mut impl Write,
    digest: &mut Sha256,
    max_bytes: u64,
) -> Result<u64, BackupError> {
    let mut total = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|_| BackupError::Io)?;
        if read == 0 {
            break;
        }
        let next_total = total
            .checked_add(read as u64)
            .ok_or(BackupError::CapacityExceeded)?;
        if next_total > max_bytes {
            return Err(BackupError::CapacityExceeded);
        }
        output
            .write_all(&buffer[..read])
            .map_err(|_| BackupError::Io)?;
        digest.update(&buffer[..read]);
        total = next_total;
    }
    Ok(total)
}

struct ArchiveMember {
    path: String,
    bytes: Vec<u8>,
}

struct ArchiveFileSource {
    path: String,
    source_path: PathBuf,
    size_bytes: u64,
    sha256: String,
}

struct VerifiedArchive {
    members: BTreeMap<String, Vec<u8>>,
}

fn validate_declared_members<'a>(
    offline_included: bool,
    declared: impl Iterator<Item = &'a String>,
) -> Result<(), BackupError> {
    let declared = declared.map(String::as_str).collect::<BTreeSet<_>>();
    for required in [FRONTEND_PATH, CATALOG_PATH, HISTORY_PATH, DOWNLOADS_PATH] {
        if !declared.contains(required) {
            return Err(BackupError::InvalidArchive);
        }
    }
    if declared.iter().any(|path| {
        path.starts_with("components/")
            && ![FRONTEND_PATH, CATALOG_PATH, HISTORY_PATH, DOWNLOADS_PATH].contains(path)
    }) {
        return Err(BackupError::InvalidArchive);
    }
    if !offline_included && declared.iter().any(|path| path.starts_with("offline/")) {
        return Err(BackupError::InvalidArchive);
    }
    Ok(())
}

fn validate_total_uncompressed_size(
    mut member_sizes: impl Iterator<Item = u64>,
    manifest_size: u64,
) -> Result<(), BackupError> {
    let total = member_sizes.try_fold(manifest_size, |total, size| {
        total.checked_add(size).ok_or(BackupError::CapacityExceeded)
    })?;
    if total > MAX_ARCHIVE_UNCOMPRESSED_BYTES {
        return Err(BackupError::CapacityExceeded);
    }
    Ok(())
}

fn member_json<T: Serialize>(path: &str, value: &T) -> Result<ArchiveMember, BackupError> {
    let bytes = serde_json::to_vec(value).map_err(|_| BackupError::InvalidInput)?;
    if bytes.len() as u64 > MAX_COMPONENT_BYTES {
        return Err(BackupError::CapacityExceeded);
    }
    Ok(ArchiveMember {
        path: path.to_owned(),
        bytes,
    })
}

fn write_archive(
    path: &Path,
    manifest: &[u8],
    members: &[ArchiveMember],
) -> Result<(), BackupError> {
    let file = File::create(path).map_err(|_| BackupError::Io)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    writer
        .start_file(MANIFEST_PATH, options)
        .map_err(|_| BackupError::Io)?;
    writer.write_all(manifest).map_err(|_| BackupError::Io)?;
    for member in members {
        writer
            .start_file(&member.path, options)
            .map_err(|_| BackupError::Io)?;
        writer
            .write_all(&member.bytes)
            .map_err(|_| BackupError::Io)?;
    }
    writer
        .finish()
        .map_err(|_| BackupError::Io)?
        .sync_all()
        .map_err(|_| BackupError::Io)
}

fn write_archive_with_files(
    path: &Path,
    manifest: &[u8],
    members: &[ArchiveMember],
    files: &[ArchiveFileSource],
) -> Result<(), BackupError> {
    let file = File::create(path).map_err(|_| BackupError::Io)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    writer
        .start_file(MANIFEST_PATH, options)
        .map_err(|_| BackupError::Io)?;
    writer.write_all(manifest).map_err(|_| BackupError::Io)?;
    for member in members {
        writer
            .start_file(&member.path, options)
            .map_err(|_| BackupError::Io)?;
        writer
            .write_all(&member.bytes)
            .map_err(|_| BackupError::Io)?;
    }
    for source in files {
        writer
            .start_file(&source.path, options)
            .map_err(|_| BackupError::Io)?;
        let mut input = File::open(&source.source_path).map_err(|_| BackupError::Io)?;
        let copied = std::io::copy(&mut input, &mut writer).map_err(|_| BackupError::Io)?;
        if copied != source.size_bytes || sha256_file(&source.source_path)? != source.sha256 {
            return Err(BackupError::IntegrityMismatch);
        }
    }
    writer
        .finish()
        .map_err(|_| BackupError::Io)?
        .sync_all()
        .map_err(|_| BackupError::Io)
}

fn read_verified_archive(path: &Path) -> Result<VerifiedArchive, BackupError> {
    let file = File::open(path).map_err(|_| BackupError::Io)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| BackupError::InvalidArchive)?;
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_FILES {
        return Err(BackupError::CapacityExceeded);
    }
    let mut raw_members = BTreeMap::new();
    let mut total_in_memory_bytes = 0_u64;
    for index in 0..archive.len() {
        let member = archive
            .by_index(index)
            .map_err(|_| BackupError::InvalidArchive)?;
        if member.is_dir() || member.enclosed_name().is_none() {
            return Err(BackupError::InvalidArchive);
        }
        let name = member.name().to_owned();
        validate_archive_member_name(&name)?;
        if raw_members.contains_key(&name) {
            return Err(BackupError::InvalidArchive);
        }
        let limit = if name == MANIFEST_PATH {
            MAX_MANIFEST_BYTES
        } else if name.starts_with("components/") {
            MAX_COMPONENT_BYTES
        } else if name.starts_with("offline/") {
            MAX_OFFLINE_FILE_BYTES
        } else {
            return Err(BackupError::InvalidArchive);
        };
        if member.size() > limit {
            return Err(BackupError::CapacityExceeded);
        }
        total_in_memory_bytes = total_in_memory_bytes
            .checked_add(member.size())
            .ok_or(BackupError::CapacityExceeded)?;
        if total_in_memory_bytes > MAX_IN_MEMORY_RESTORE_BYTES {
            return Err(BackupError::CapacityExceeded);
        }
        let remaining = MAX_IN_MEMORY_RESTORE_BYTES
            .checked_sub(total_in_memory_bytes - member.size())
            .ok_or(BackupError::CapacityExceeded)?;
        let read_limit = limit.min(remaining);
        let mut bytes = Vec::with_capacity(member.size().min(read_limit) as usize);
        member
            .take(read_limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| BackupError::InvalidArchive)?;
        if bytes.len() as u64 > read_limit {
            return Err(BackupError::CapacityExceeded);
        }
        raw_members.insert(name, bytes);
    }

    let manifest_bytes = raw_members
        .remove(MANIFEST_PATH)
        .ok_or(BackupError::InvalidArchive)?;
    let manifest: BackupManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| BackupError::InvalidArchive)?;
    if manifest.format_version > FORMAT_VERSION {
        return Err(BackupError::UnsupportedVersion);
    }
    if manifest.format_version != FORMAT_VERSION {
        return Err(BackupError::InvalidArchive);
    }
    validate_application_version(&manifest.application_version)?;
    if manifest.entries.len() != raw_members.len() {
        return Err(BackupError::IntegrityMismatch);
    }
    let mut declared = BTreeSet::new();
    for entry in &manifest.entries {
        validate_archive_member_name(&entry.path)?;
        if entry.path == MANIFEST_PATH || !declared.insert(entry.path.clone()) {
            return Err(BackupError::InvalidArchive);
        }
        let bytes = raw_members
            .get(&entry.path)
            .ok_or(BackupError::IntegrityMismatch)?;
        if bytes.len() as u64 != entry.size_bytes || sha256_hex(bytes) != entry.sha256 {
            return Err(BackupError::IntegrityMismatch);
        }
    }
    validate_declared_members(manifest.offline_included, declared.iter())?;
    Ok(VerifiedArchive {
        members: raw_members,
    })
}

fn take_json(
    members: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<serde_json::Value, BackupError> {
    let bytes = members.remove(path).ok_or(BackupError::InvalidArchive)?;
    serde_json::from_slice(&bytes).map_err(|_| BackupError::InvalidArchive)
}

fn normalized_relative_path(candidate: &str) -> Result<String, BackupError> {
    if candidate.is_empty() || candidate.contains('\\') || candidate.contains(':') {
        return Err(BackupError::InvalidInput);
    }
    let path = Path::new(candidate);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BackupError::InvalidInput);
    }
    let normalized = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if normalized.len() > 1024 {
        return Err(BackupError::InvalidInput);
    }
    Ok(normalized)
}

fn validate_archive_member_name(name: &str) -> Result<(), BackupError> {
    if name == MANIFEST_PATH || name.starts_with("components/") {
        if name.contains('\\') || name.contains(':') || name.len() > 1024 {
            return Err(BackupError::InvalidArchive);
        }
        let path = Path::new(name);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(BackupError::InvalidArchive);
        }
        return Ok(());
    }
    if let Some(relative) = name.strip_prefix("offline/") {
        normalized_relative_path(relative).map_err(|_| BackupError::InvalidArchive)?;
        return Ok(());
    }
    Err(BackupError::InvalidArchive)
}

fn validate_application_version(version: &str) -> Result<(), BackupError> {
    if version.is_empty()
        || version.len() > 64
        || version.chars().any(|character| character.is_control())
    {
        return Err(BackupError::InvalidInput);
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, BackupError> {
    let mut file = File::open(path).map_err(|_| BackupError::Io)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| BackupError::Io)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn staging_path(destination: &Path) -> Result<PathBuf, BackupError> {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(BackupError::InvalidInput)?;
    Ok(destination.with_file_name(format!(".{name}.staging")))
}

fn summary_from_manifest(manifest: &BackupManifest) -> BackupSummary {
    BackupSummary {
        format_version: manifest.format_version,
        application_version: manifest.application_version.clone(),
        component_count: 4,
        offline_file_count: manifest
            .entries
            .iter()
            .filter(|entry| entry.path.starts_with("offline/"))
            .count() as u32,
        offline_included: manifest.offline_included,
        total_bytes: manifest.entries.iter().map(|entry| entry.size_bytes).sum(),
        contains_credentials: false,
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    #[test]
    fn rejects_declared_archive_totals_above_the_hard_limit() {
        assert_eq!(
            validate_total_uncompressed_size([MAX_ARCHIVE_UNCOMPRESSED_BYTES, 1].into_iter(), 0,),
            Err(BackupError::CapacityExceeded)
        );
        assert_eq!(
            validate_total_uncompressed_size([MAX_ARCHIVE_UNCOMPRESSED_BYTES - 1].into_iter(), 1,),
            Ok(())
        );
    }

    #[test]
    fn rejects_actual_stream_bytes_above_the_remaining_budget() {
        let mut input = &b"four"[..];
        let mut output = Vec::new();
        let mut digest = Sha256::new();
        assert_eq!(
            copy_with_digest(&mut input, &mut output, &mut digest, 3),
            Err(BackupError::CapacityExceeded)
        );
        assert!(output.is_empty());
    }
}
