use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MANIFEST_FILE: &str = "manifest.json";
const ASSET_DIRECTORY: &str = "assets";
const MAX_ASSET_BYTES: u64 = 512 * 1024 * 1024;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineKind {
    Artwork,
    Novel,
    Ugoira,
}

impl OfflineKind {
    fn code(self) -> &'static str {
        match self {
            Self::Artwork => "artwork",
            Self::Novel => "novel",
            Self::Ugoira => "ugoira",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDraft {
    pub kind: OfflineKind,
    pub resource_id: String,
    pub title: String,
    pub author: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineEntry {
    pub key: String,
    pub kind: OfflineKind,
    pub resource_id: String,
    pub title: String,
    pub author: String,
    pub cover_url: Option<String>,
    pub stored_at_unix_seconds: u64,
    pub asset_count: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub entry_count: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineAsset {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineEntryFingerprint {
    pub entry_key: String,
    pub kind: OfflineKind,
    pub resource_id: String,
    pub assets: Vec<OfflineAssetFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineAssetFingerprint {
    pub content_type: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedEntry {
    pub directory: PathBuf,
    pub directory_name: String,
    pub file_count: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    InvalidIdentifier,
    InvalidAssetName,
    InvalidContentType,
    AssetTooLarge,
    EntryNotFound,
    AssetNotFound,
    InvalidManifest,
    ExportConflict,
    ExportUnavailable,
    Io,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidIdentifier => "invalid offline resource identifier",
            Self::InvalidAssetName => "invalid offline asset name",
            Self::InvalidContentType => "invalid offline asset content type",
            Self::AssetTooLarge => "offline asset is too large",
            Self::EntryNotFound => "offline entry was not found",
            Self::AssetNotFound => "offline asset was not found",
            Self::InvalidManifest => "offline manifest is invalid",
            Self::ExportConflict => "export destination contains unrelated data",
            Self::ExportUnavailable => "offline entry export failed",
            Self::Io => "offline library I/O failed",
        })
    }
}

impl std::error::Error for LibraryError {}

#[derive(Clone)]
pub struct OfflineLibrary {
    root: PathBuf,
}

impl OfflineLibrary {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, LibraryError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|_| LibraryError::Io)?;
        for child in fs::read_dir(&root).map_err(|_| LibraryError::Io)? {
            let child = child.map_err(|_| LibraryError::Io)?;
            if child
                .file_name()
                .to_string_lossy()
                .starts_with(".batch-remove-")
            {
                let _ = if child.file_type().map_err(|_| LibraryError::Io)?.is_dir() {
                    fs::remove_dir_all(child.path())
                } else {
                    fs::remove_file(child.path())
                };
            }
        }
        Ok(Self { root })
    }

    pub fn store_entry<F>(&self, draft: EntryDraft, build: F) -> Result<OfflineEntry, LibraryError>
    where
        F: FnOnce(&mut EntryWriter) -> Result<(), LibraryError>,
    {
        let resource_id = normalized_resource_id(&draft.resource_id)?;
        let key = format!("{}-{resource_id}", draft.kind.code());
        let target = self.root.join(&key);
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = self.root.join(format!(".{key}.staging-{sequence}"));
        let backup = self.root.join(format!(".{key}.backup-{sequence}"));
        if staging.exists() {
            fs::remove_dir_all(&staging).map_err(|_| LibraryError::Io)?;
        }
        fs::create_dir_all(staging.join(ASSET_DIRECTORY)).map_err(|_| LibraryError::Io)?;

        let result = (|| {
            let mut writer = EntryWriter {
                asset_root: staging.join(ASSET_DIRECTORY),
                assets: Vec::new(),
            };
            build(&mut writer)?;
            if writer.assets.is_empty() {
                return Err(LibraryError::AssetNotFound);
            }
            let size_bytes = writer.assets.iter().map(|asset| asset.size_bytes).sum();
            let entry = OfflineEntry {
                key,
                kind: draft.kind,
                resource_id,
                title: draft.title,
                author: draft.author,
                cover_url: draft.cover_url,
                stored_at_unix_seconds: unix_seconds(),
                asset_count: u32::try_from(writer.assets.len()).unwrap_or(u32::MAX),
                size_bytes,
            };
            let manifest = StoredManifest {
                entry: entry.clone(),
                assets: writer.assets,
            };
            let manifest_bytes =
                serde_json::to_vec_pretty(&manifest).map_err(|_| LibraryError::InvalidManifest)?;
            fs::write(staging.join(MANIFEST_FILE), manifest_bytes).map_err(|_| LibraryError::Io)?;
            let replacing = target.exists();
            if replacing {
                fs::rename(&target, &backup).map_err(|_| LibraryError::Io)?;
            }
            if fs::rename(&staging, &target).is_err() {
                if replacing {
                    let _ = fs::rename(&backup, &target);
                }
                return Err(LibraryError::Io);
            }
            if replacing {
                let _ = fs::remove_dir_all(&backup);
            }
            Ok(entry)
        })();
        if result.is_err() && staging.exists() {
            let _ = fs::remove_dir_all(staging);
        }
        result
    }

    pub fn list_entries(&self) -> Result<Vec<OfflineEntry>, LibraryError> {
        let mut entries = Vec::new();
        for child in fs::read_dir(&self.root).map_err(|_| LibraryError::Io)? {
            let child = child.map_err(|_| LibraryError::Io)?;
            if !child.file_type().map_err(|_| LibraryError::Io)?.is_dir()
                || child.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            if let Ok(manifest) = read_manifest(&child.path()) {
                entries.push(manifest.entry);
            }
        }
        entries.sort_by(|left, right| {
            right
                .stored_at_unix_seconds
                .cmp(&left.stored_at_unix_seconds)
                .then_with(|| left.key.cmp(&right.key))
        });
        Ok(entries)
    }

    pub fn entry_fingerprints(&self) -> Result<Vec<OfflineEntryFingerprint>, LibraryError> {
        let entries = self.list_entries()?;
        let mut fingerprints = Vec::with_capacity(entries.len());
        for entry in entries {
            let entry_path = self.entry_path(&entry.key)?;
            let manifest = read_manifest(&entry_path)?;
            if manifest.entry != entry {
                return Err(LibraryError::InvalidManifest);
            }
            let mut assets = Vec::with_capacity(manifest.assets.len());
            for asset in manifest.assets {
                validate_asset_name(&asset.name)?;
                validate_content_type(&asset.content_type)?;
                let mut file = fs::File::open(entry_path.join(ASSET_DIRECTORY).join(&asset.name))
                    .map_err(|_| LibraryError::AssetNotFound)?;
                let mut hasher = Sha256::new();
                let mut read_bytes = 0_u64;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = file.read(&mut buffer).map_err(|_| LibraryError::Io)?;
                    if count == 0 {
                        break;
                    }
                    read_bytes = read_bytes
                        .checked_add(
                            u64::try_from(count).map_err(|_| LibraryError::InvalidManifest)?,
                        )
                        .ok_or(LibraryError::InvalidManifest)?;
                    if read_bytes > asset.size_bytes || read_bytes > MAX_ASSET_BYTES {
                        return Err(LibraryError::InvalidManifest);
                    }
                    hasher.update(&buffer[..count]);
                }
                if read_bytes != asset.size_bytes {
                    return Err(LibraryError::InvalidManifest);
                }
                assets.push(OfflineAssetFingerprint {
                    content_type: asset.content_type,
                    sha256: format!("{:x}", hasher.finalize()),
                });
            }
            assets.sort_by(|left, right| {
                left.content_type
                    .cmp(&right.content_type)
                    .then_with(|| left.sha256.cmp(&right.sha256))
            });
            fingerprints.push(OfflineEntryFingerprint {
                entry_key: entry.key,
                kind: entry.kind,
                resource_id: entry.resource_id,
                assets,
            });
        }
        Ok(fingerprints)
    }

    pub fn read_asset(&self, key: &str, name: &str) -> Result<OfflineAsset, LibraryError> {
        let entry_path = self.entry_path(key)?;
        let manifest = read_manifest(&entry_path)?;
        if manifest.entry.key != key {
            return Err(LibraryError::InvalidManifest);
        }
        let asset = manifest
            .assets
            .iter()
            .find(|asset| asset.name == name)
            .ok_or(LibraryError::AssetNotFound)?;
        if asset.size_bytes > MAX_ASSET_BYTES {
            return Err(LibraryError::AssetTooLarge);
        }
        let bytes = fs::read(entry_path.join(ASSET_DIRECTORY).join(&asset.name))
            .map_err(|_| LibraryError::AssetNotFound)?;
        if bytes.len() as u64 != asset.size_bytes {
            return Err(LibraryError::InvalidManifest);
        }
        Ok(OfflineAsset {
            content_type: asset.content_type.clone(),
            bytes,
        })
    }

    pub fn get_entry(&self, key: &str) -> Result<OfflineEntry, LibraryError> {
        let entry_path = self.entry_path(key)?;
        let manifest = read_manifest(&entry_path)?;
        if manifest.entry.key != key {
            return Err(LibraryError::InvalidManifest);
        }
        Ok(manifest.entry)
    }

    /// Materializes one validated entry as a user-visible directory.
    ///
    /// The directory name is derived only from the validated entry kind and numeric resource ID.
    /// Existing directories are replaced only when their marker belongs to the same entry, so a
    /// user-created folder can never be silently overwritten.
    pub fn export_entry(
        &self,
        key: &str,
        destination_root: impl AsRef<Path>,
    ) -> Result<ExportedEntry, LibraryError> {
        let entry_path = self.entry_path(key)?;
        let manifest = read_manifest(&entry_path)?;
        if manifest.entry.key != key {
            return Err(LibraryError::InvalidManifest);
        }

        let destination_root = destination_root.as_ref();
        fs::create_dir_all(destination_root).map_err(|_| LibraryError::ExportUnavailable)?;
        let root_metadata =
            fs::metadata(destination_root).map_err(|_| LibraryError::ExportUnavailable)?;
        if !root_metadata.is_dir() {
            return Err(LibraryError::ExportUnavailable);
        }

        let directory_name = format!(
            "{}-{}",
            manifest.entry.kind.code(),
            manifest.entry.resource_id
        );
        let target = destination_root.join(&directory_name);
        ensure_replaceable_export_target(&target, key)?;

        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = destination_root.join(format!(".{directory_name}.staging-{sequence}"));
        let backup = destination_root.join(format!(".{directory_name}.backup-{sequence}"));
        remove_owned_export_path(&staging)?;
        remove_owned_export_path(&backup)?;
        fs::create_dir(&staging).map_err(|_| LibraryError::ExportUnavailable)?;

        let result = (|| {
            for asset in &manifest.assets {
                validate_asset_name(&asset.name)?;
                validate_content_type(&asset.content_type)?;
                if asset.size_bytes > MAX_ASSET_BYTES {
                    return Err(LibraryError::AssetTooLarge);
                }
                let source = entry_path.join(ASSET_DIRECTORY).join(&asset.name);
                let copied = fs::copy(source, staging.join(&asset.name))
                    .map_err(|_| LibraryError::ExportUnavailable)?;
                if copied != asset.size_bytes {
                    return Err(LibraryError::InvalidManifest);
                }
            }

            let marker =
                serde_json::to_vec_pretty(&manifest).map_err(|_| LibraryError::InvalidManifest)?;
            fs::write(staging.join(EXPORT_MARKER_FILE), marker)
                .map_err(|_| LibraryError::ExportUnavailable)?;

            let replacing = target.exists();
            if replacing {
                fs::rename(&target, &backup).map_err(|_| LibraryError::ExportUnavailable)?;
            }
            if fs::rename(&staging, &target).is_err() {
                if replacing {
                    let _ = fs::rename(&backup, &target);
                }
                return Err(LibraryError::ExportUnavailable);
            }
            if replacing {
                remove_owned_export_path(&backup)?;
            }
            Ok(ExportedEntry {
                directory: target,
                directory_name,
                file_count: u32::try_from(manifest.assets.len() + 1).unwrap_or(u32::MAX),
                size_bytes: manifest.entry.size_bytes,
            })
        })();

        if result.is_err() {
            let _ = remove_owned_export_path(&staging);
        }
        result
    }

    pub fn remove_entry(&self, key: &str) -> Result<bool, LibraryError> {
        let path = self.entry_path(key)?;
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(path).map_err(|_| LibraryError::Io)?;
        Ok(true)
    }

    pub fn remove_entries(&self, keys: &[String]) -> Result<Vec<String>, LibraryError> {
        if keys.is_empty() || keys.len() > 1_000 {
            return Err(LibraryError::InvalidIdentifier);
        }
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut planned = Vec::with_capacity(keys.len());
        let mut seen = std::collections::HashSet::with_capacity(keys.len());
        for (index, key) in keys.iter().enumerate() {
            let source = self.entry_path(key)?;
            if !seen.insert(key.clone()) || !source.is_dir() {
                return Err(if source.exists() {
                    LibraryError::InvalidIdentifier
                } else {
                    LibraryError::EntryNotFound
                });
            }
            let manifest = read_manifest(&source)?;
            if manifest.entry.key != *key {
                return Err(LibraryError::InvalidManifest);
            }
            let quarantine = self.root.join(format!(".batch-remove-{sequence}-{index}"));
            if quarantine.exists() {
                return Err(LibraryError::Io);
            }
            planned.push((key.clone(), source, quarantine));
        }

        for (renamed, (_, source, quarantine)) in planned.iter().enumerate() {
            if fs::rename(source, quarantine).is_err() {
                for (_, restore_source, restore_quarantine) in planned[..renamed].iter().rev() {
                    fs::rename(restore_quarantine, restore_source).map_err(|_| LibraryError::Io)?;
                }
                return Err(LibraryError::Io);
            }
        }
        // Moving every entry into the hidden quarantine is the atomic logical commit.
        // `open` retries physical cleanup later, so a cleanup failure can never leave a
        // caller-visible batch only partly deleted.
        Ok(planned.into_iter().map(|(key, _, _)| key).collect())
    }

    pub fn stats(&self) -> Result<LibraryStats, LibraryError> {
        let entries = self.list_entries()?;
        Ok(LibraryStats {
            entry_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            size_bytes: entries.iter().map(|entry| entry.size_bytes).sum(),
        })
    }

    pub fn clear(&self) -> Result<LibraryStats, LibraryError> {
        let removed = self.stats()?;
        for child in fs::read_dir(&self.root).map_err(|_| LibraryError::Io)? {
            let child = child.map_err(|_| LibraryError::Io)?;
            let file_type = child.file_type().map_err(|_| LibraryError::Io)?;
            if file_type.is_dir() {
                fs::remove_dir_all(child.path()).map_err(|_| LibraryError::Io)?;
            } else {
                fs::remove_file(child.path()).map_err(|_| LibraryError::Io)?;
            }
        }
        Ok(removed)
    }

    fn entry_path(&self, key: &str) -> Result<PathBuf, LibraryError> {
        let (kind, resource_id) = key.split_once('-').ok_or(LibraryError::InvalidIdentifier)?;
        if !matches!(kind, "artwork" | "novel" | "ugoira") {
            return Err(LibraryError::InvalidIdentifier);
        }
        let resource_id = normalized_resource_id(resource_id)?;
        Ok(self.root.join(format!("{kind}-{resource_id}")))
    }
}

pub struct EntryWriter {
    asset_root: PathBuf,
    assets: Vec<StoredAsset>,
}

impl EntryWriter {
    pub fn write_asset(
        &mut self,
        name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), LibraryError> {
        validate_asset_name(name)?;
        validate_content_type(content_type)?;
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(LibraryError::AssetTooLarge);
        }
        if self.assets.iter().any(|asset| asset.name == name) {
            return Err(LibraryError::InvalidAssetName);
        }
        fs::write(self.asset_root.join(name), bytes).map_err(|_| LibraryError::Io)?;
        self.assets.push(StoredAsset {
            name: name.to_owned(),
            content_type: content_type.to_owned(),
            size_bytes: bytes.len() as u64,
        });
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct StoredManifest {
    entry: OfflineEntry,
    assets: Vec<StoredAsset>,
}

#[derive(Serialize, Deserialize)]
struct StoredAsset {
    name: String,
    content_type: String,
    size_bytes: u64,
}

const EXPORT_MARKER_FILE: &str = "pixiv-client-entry.json";

fn ensure_replaceable_export_target(target: &Path, key: &str) -> Result<(), LibraryError> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(LibraryError::ExportUnavailable),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LibraryError::ExportConflict);
    }
    let marker = read_manifest_from_file(&target.join(EXPORT_MARKER_FILE))
        .map_err(|_| LibraryError::ExportConflict)?;
    if marker.entry.key == key {
        Ok(())
    } else {
        Err(LibraryError::ExportConflict)
    }
}

fn remove_owned_export_path(path: &Path) -> Result<(), LibraryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(LibraryError::ExportConflict),
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|_| LibraryError::ExportUnavailable)
        }
        Ok(_) => Err(LibraryError::ExportConflict),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(LibraryError::ExportUnavailable),
    }
}

fn read_manifest(entry_path: &Path) -> Result<StoredManifest, LibraryError> {
    read_manifest_from_file(&entry_path.join(MANIFEST_FILE))
}

fn read_manifest_from_file(path: &Path) -> Result<StoredManifest, LibraryError> {
    let bytes = fs::read(path).map_err(|_| LibraryError::EntryNotFound)?;
    if bytes.len() > 1024 * 1024 {
        return Err(LibraryError::InvalidManifest);
    }
    serde_json::from_slice(&bytes).map_err(|_| LibraryError::InvalidManifest)
}

fn normalized_resource_id(candidate: &str) -> Result<String, LibraryError> {
    let value = candidate
        .parse::<u64>()
        .map_err(|_| LibraryError::InvalidIdentifier)?;
    if value == 0 {
        return Err(LibraryError::InvalidIdentifier);
    }
    Ok(value.to_string())
}

fn validate_asset_name(name: &str) -> Result<(), LibraryError> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name != MANIFEST_FILE
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(LibraryError::InvalidAssetName)
    }
}

fn validate_content_type(content_type: &str) -> Result<(), LibraryError> {
    if matches!(
        content_type,
        "application/json"
            | "application/zip"
            | "text/plain; charset=utf-8"
            | "image/jpeg"
            | "image/png"
            | "image/gif"
            | "image/webp"
            | "image/avif"
    ) {
        Ok(())
    } else {
        Err(LibraryError::InvalidContentType)
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{EntryDraft, LibraryError, OfflineKind, OfflineLibrary};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pixiv-client-library-{name}-{nonce}"))
    }

    #[test]
    fn stores_lists_reads_and_removes_an_entry() {
        let root = test_root("roundtrip");
        let library = OfflineLibrary::open(&root).unwrap();
        let entry = library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Novel,
                    resource_id: "42".to_owned(),
                    title: "Story".to_owned(),
                    author: "Alice".to_owned(),
                    cover_url: None,
                },
                |writer| writer.write_asset("content.txt", "text/plain; charset=utf-8", b"hello"),
            )
            .unwrap();
        assert_eq!(entry.key, "novel-42");
        assert_eq!(library.list_entries().unwrap(), [entry]);
        assert_eq!(
            library.read_asset("novel-42", "content.txt").unwrap().bytes,
            b"hello"
        );
        assert_eq!(library.get_entry("novel-42").unwrap().key, "novel-42");
        assert_eq!(library.stats().unwrap().size_bytes, 5);
        let replaced = library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Novel,
                    resource_id: "42".to_owned(),
                    title: "Story, revised".to_owned(),
                    author: "Alice".to_owned(),
                    cover_url: None,
                },
                |writer| writer.write_asset("content.txt", "text/plain; charset=utf-8", b"new"),
            )
            .unwrap();
        assert_eq!(replaced.title, "Story, revised");
        assert_eq!(library.list_entries().unwrap(), [replaced]);
        assert_eq!(
            library.read_asset("novel-42", "content.txt").unwrap().bytes,
            b"new"
        );
        assert_eq!(library.stats().unwrap().size_bytes, 3);
        assert!(library.remove_entry("novel-42").unwrap());
        assert!(library.list_entries().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_empty_transactions() {
        let root = test_root("validation");
        let library = OfflineLibrary::open(&root).unwrap();
        let draft = EntryDraft {
            kind: OfflineKind::Artwork,
            resource_id: "7".to_owned(),
            title: String::new(),
            author: String::new(),
            cover_url: None,
        };
        assert_eq!(
            library.store_entry(draft.clone(), |writer| writer.write_asset(
                "../secret",
                "image/jpeg",
                b"x"
            )),
            Err(LibraryError::InvalidAssetName)
        );
        assert_eq!(
            library.store_entry(draft, |_writer| Ok(())),
            Err(LibraryError::AssetNotFound)
        );
        assert_eq!(
            library.read_asset("novel-../../x", "content.txt"),
            Err(LibraryError::InvalidIdentifier)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_read_an_entry_whose_manifest_key_does_not_match() {
        let root = test_root("manifest-key");
        let library = OfflineLibrary::open(&root).unwrap();
        library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Artwork,
                    resource_id: "7".to_owned(),
                    title: String::new(),
                    author: String::new(),
                    cover_url: None,
                },
                |writer| writer.write_asset("page-0001.jpg", "image/jpeg", b"image"),
            )
            .unwrap();
        let manifest_path = root.join("artwork-7").join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["entry"]["key"] = serde_json::Value::String("artwork-8".into());
        manifest["entry"]["resourceId"] = serde_json::Value::String("8".into());
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        assert_eq!(
            library.read_asset("artwork-7", "page-0001.jpg"),
            Err(LibraryError::InvalidManifest)
        );
        assert_eq!(
            library.get_entry("artwork-7"),
            Err(LibraryError::InvalidManifest)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clears_the_library_root_without_touching_adjacent_user_data() {
        let parent = test_root("clear-boundary");
        let root = parent.join("offline-library");
        let adjacent = parent.join("keep.txt");
        fs::create_dir_all(&root).unwrap();
        fs::write(&adjacent, b"keep").unwrap();
        let library = OfflineLibrary::open(&root).unwrap();
        library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Artwork,
                    resource_id: "99".into(),
                    title: "Cached work".into(),
                    author: "Author".into(),
                    cover_url: None,
                },
                |writer| writer.write_asset("page-0.jpg", "image/jpeg", b"image"),
            )
            .unwrap();
        fs::write(root.join("orphan.tmp"), b"partial").unwrap();

        let removed = library.clear().unwrap();

        assert_eq!(removed.entry_count, 1);
        assert_eq!(removed.size_bytes, 5);
        assert!(library.list_entries().unwrap().is_empty());
        assert!(fs::read(&adjacent).is_ok_and(|bytes| bytes == b"keep"));
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn exports_an_entry_atomically_and_replaces_only_its_own_directory() {
        let parent = test_root("export-roundtrip");
        let library = OfflineLibrary::open(parent.join("library")).unwrap();
        let destination = parent.join("exports");
        library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Artwork,
                    resource_id: "123".into(),
                    title: "Unsafe / title".into(),
                    author: "Artist".into(),
                    cover_url: None,
                },
                |writer| writer.write_asset("page-001.jpg", "image/jpeg", b"first"),
            )
            .unwrap();

        let first = library.export_entry("artwork-123", &destination).unwrap();
        assert_eq!(first.directory_name, "artwork-123");
        assert_eq!(first.file_count, 2);
        assert_eq!(
            fs::read(first.directory.join("page-001.jpg")).unwrap(),
            b"first"
        );
        assert!(first.directory.join("pixiv-client-entry.json").is_file());

        library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Artwork,
                    resource_id: "123".into(),
                    title: "Updated".into(),
                    author: "Artist".into(),
                    cover_url: None,
                },
                |writer| writer.write_asset("page-002.png", "image/png", b"second"),
            )
            .unwrap();
        let replaced = library.export_entry("artwork-123", &destination).unwrap();
        assert!(!replaced.directory.join("page-001.jpg").exists());
        assert_eq!(
            fs::read(replaced.directory.join("page-002.png")).unwrap(),
            b"second"
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_an_unrelated_export_directory() {
        let parent = test_root("export-conflict");
        let library = OfflineLibrary::open(parent.join("library")).unwrap();
        library
            .store_entry(
                EntryDraft {
                    kind: OfflineKind::Novel,
                    resource_id: "456".into(),
                    title: "Story".into(),
                    author: "Writer".into(),
                    cover_url: None,
                },
                |writer| writer.write_asset("content.txt", "text/plain; charset=utf-8", b"text"),
            )
            .unwrap();
        let collision = parent.join("exports").join("novel-456");
        fs::create_dir_all(&collision).unwrap();
        fs::write(collision.join("keep.txt"), b"user data").unwrap();

        assert_eq!(
            library.export_entry("novel-456", parent.join("exports")),
            Err(LibraryError::ExportConflict)
        );
        assert_eq!(fs::read(collision.join("keep.txt")).unwrap(), b"user data");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn fingerprints_assets_and_batch_removal_is_all_or_nothing() {
        let root = test_root("fingerprints-batch");
        let library = OfflineLibrary::open(&root).unwrap();
        for (kind, id) in [(OfflineKind::Artwork, "8"), (OfflineKind::Ugoira, "8")] {
            library
                .store_entry(
                    EntryDraft {
                        kind,
                        resource_id: id.into(),
                        title: "Duplicate".into(),
                        author: "Artist".into(),
                        cover_url: None,
                    },
                    |writer| writer.write_asset("asset.png", "image/png", b"same"),
                )
                .unwrap();
        }
        let fingerprints = library.entry_fingerprints().unwrap();
        assert_eq!(fingerprints.len(), 2);
        assert_eq!(fingerprints[0].assets[0].sha256.len(), 64);
        assert_eq!(fingerprints[0].assets[0], fingerprints[1].assets[0]);

        assert_eq!(
            library.remove_entries(&["artwork-8".into(), "bad-key".into()]),
            Err(LibraryError::InvalidIdentifier)
        );
        assert_eq!(library.list_entries().unwrap().len(), 2);
        let removed = library
            .remove_entries(&["artwork-8".into(), "ugoira-8".into()])
            .unwrap();
        assert_eq!(removed.len(), 2);
        assert!(library.list_entries().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
