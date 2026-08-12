use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SETTINGS_FILE: &str = "storage-settings-v1.json";
const SETTINGS_STAGING_FILE: &str = ".storage-settings-v1.staging";
const SETTINGS_BACKUP_FILE: &str = ".storage-settings-v1.backup";
const FORMAT_VERSION: u8 = 1;
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
pub const DEFAULT_CACHE_LIMIT_BYTES: u64 = 256 * MIB;
pub const STORAGE_RESERVE_BYTES: u64 = 512 * MIB;
pub const STORAGE_WARNING_BYTES: u64 = 2 * GIB;
pub const ALLOWED_CACHE_LIMIT_BYTES: [u64; 6] =
    [128 * MIB, 256 * MIB, 512 * MIB, GIB, 5 * GIB, 10 * GIB];
const MAX_SCANNED_ENTRIES: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageHealth {
    Healthy,
    Low,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub health: StorageHealth,
    pub data_total_bytes: u64,
    pub data_available_bytes: u64,
    pub cache_total_bytes: u64,
    pub cache_available_bytes: u64,
    pub writable_download_bytes: u64,
    pub offline_bytes: u64,
    pub cache_bytes: u64,
    pub cache_limit_bytes: Option<u64>,
    pub cache_remaining_quota_bytes: Option<u64>,
    pub reserve_bytes: u64,
    pub warning_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    InvalidCacheLimit,
    InvalidWriteSize,
    InsufficientSpace {
        available_bytes: u64,
        required_bytes: u64,
        reserve_bytes: u64,
    },
    Io,
    StateUnavailable,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCacheLimit => formatter.write_str("unsupported media cache limit"),
            Self::InvalidWriteSize => formatter.write_str("invalid storage write size"),
            Self::InsufficientSpace { .. } => {
                formatter.write_str("not enough free space after the application reserve")
            }
            Self::Io => formatter.write_str("storage information or settings I/O failed"),
            Self::StateUnavailable => formatter.write_str("storage policy state is unavailable"),
        }
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct StoredSettings {
    format_version: u8,
    cache_limit_bytes: Option<u64>,
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            cache_limit_bytes: Some(DEFAULT_CACHE_LIMIT_BYTES),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct VolumeSpace {
    total_bytes: u64,
    available_bytes: u64,
}

pub struct StorageManager {
    data_root: PathBuf,
    cache_root: PathBuf,
    settings: Mutex<StoredSettings>,
}

impl StorageManager {
    pub fn open(
        data_root: impl Into<PathBuf>,
        cache_root: impl Into<PathBuf>,
    ) -> Result<Self, StorageError> {
        let data_root = data_root.into();
        let cache_root = cache_root.into();
        fs::create_dir_all(&data_root).map_err(|_| StorageError::Io)?;
        fs::create_dir_all(&cache_root).map_err(|_| StorageError::Io)?;
        restore_interrupted_settings(&data_root)?;
        let settings = load_settings(&data_root).unwrap_or_default();
        persist_settings(&data_root, settings)?;
        Ok(Self {
            data_root,
            cache_root,
            settings: Mutex::new(settings),
        })
    }

    pub fn cache_limit_bytes(&self) -> Result<Option<u64>, StorageError> {
        self.settings
            .lock()
            .map(|settings| settings.cache_limit_bytes)
            .map_err(|_| StorageError::StateUnavailable)
    }

    pub fn set_cache_limit(&self, cache_limit_bytes: Option<u64>) -> Result<(), StorageError> {
        if cache_limit_bytes.is_some_and(|limit| !ALLOWED_CACHE_LIMIT_BYTES.contains(&limit)) {
            return Err(StorageError::InvalidCacheLimit);
        }
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| StorageError::StateUnavailable)?;
        let updated = StoredSettings {
            cache_limit_bytes,
            ..*settings
        };
        persist_settings(&self.data_root, updated)?;
        *settings = updated;
        Ok(())
    }

    pub fn reset_settings(&self) -> Result<(), StorageError> {
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| StorageError::StateUnavailable)?;
        let defaults = StoredSettings::default();
        persist_settings(&self.data_root, defaults)?;
        *settings = defaults;
        Ok(())
    }

    pub fn status(&self) -> Result<StorageStatus, StorageError> {
        let data = volume_space(&self.data_root)?;
        let cache = volume_space(&self.cache_root)?;
        let offline_bytes = directory_size(&self.data_root.join("offline-library"))?;
        let cache_bytes = directory_size(&self.cache_root.join("media-v1"))?;
        let cache_limit_bytes = self.cache_limit_bytes()?;
        Ok(evaluate_status(
            data,
            cache,
            offline_bytes,
            cache_bytes,
            cache_limit_bytes,
        ))
    }

    pub fn ensure_offline_write(&self, required_bytes: u64) -> Result<(), StorageError> {
        ensure_write_capacity(volume_space(&self.data_root)?, required_bytes)
    }

    pub fn allows_cache_write(&self, required_bytes: u64) -> Result<bool, StorageError> {
        match ensure_write_capacity(volume_space(&self.cache_root)?, required_bytes) {
            Ok(()) => Ok(true),
            Err(StorageError::InsufficientSpace { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn evaluate_status(
    data: VolumeSpace,
    cache: VolumeSpace,
    offline_bytes: u64,
    cache_bytes: u64,
    cache_limit_bytes: Option<u64>,
) -> StorageStatus {
    let least_available = data.available_bytes.min(cache.available_bytes);
    let health = if least_available <= STORAGE_RESERVE_BYTES {
        StorageHealth::Critical
    } else if least_available <= STORAGE_WARNING_BYTES {
        StorageHealth::Low
    } else {
        StorageHealth::Healthy
    };
    StorageStatus {
        health,
        data_total_bytes: data.total_bytes,
        data_available_bytes: data.available_bytes,
        cache_total_bytes: cache.total_bytes,
        cache_available_bytes: cache.available_bytes,
        writable_download_bytes: data.available_bytes.saturating_sub(STORAGE_RESERVE_BYTES),
        offline_bytes,
        cache_bytes,
        cache_limit_bytes,
        cache_remaining_quota_bytes: cache_limit_bytes
            .map(|limit| limit.saturating_sub(cache_bytes)),
        reserve_bytes: STORAGE_RESERVE_BYTES,
        warning_bytes: STORAGE_WARNING_BYTES,
    }
}

fn ensure_write_capacity(space: VolumeSpace, required_bytes: u64) -> Result<(), StorageError> {
    if required_bytes == 0 {
        return Err(StorageError::InvalidWriteSize);
    }
    let required_with_reserve = required_bytes.saturating_add(STORAGE_RESERVE_BYTES);
    if space.available_bytes < required_with_reserve {
        Err(StorageError::InsufficientSpace {
            available_bytes: space.available_bytes,
            required_bytes,
            reserve_bytes: STORAGE_RESERVE_BYTES,
        })
    } else {
        Ok(())
    }
}

fn load_settings(root: &Path) -> Option<StoredSettings> {
    let bytes = fs::read(root.join(SETTINGS_FILE)).ok()?;
    if bytes.len() > 4096 {
        return None;
    }
    let settings: StoredSettings = serde_json::from_slice(&bytes).ok()?;
    (settings.format_version == FORMAT_VERSION
        && settings
            .cache_limit_bytes
            .is_none_or(|limit| ALLOWED_CACHE_LIMIT_BYTES.contains(&limit)))
    .then_some(settings)
}

fn persist_settings(root: &Path, settings: StoredSettings) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|_| StorageError::Io)?;
    let target = root.join(SETTINGS_FILE);
    let staging = root.join(SETTINGS_STAGING_FILE);
    let backup = root.join(SETTINGS_BACKUP_FILE);
    fs::write(&staging, bytes).map_err(|_| StorageError::Io)?;
    let replacing = target.exists();
    if replacing {
        let _ = fs::remove_file(&backup);
        fs::rename(&target, &backup).map_err(|_| StorageError::Io)?;
    }
    if fs::rename(&staging, &target).is_err() {
        if replacing {
            let _ = fs::rename(&backup, &target);
        }
        return Err(StorageError::Io);
    }
    if replacing {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn restore_interrupted_settings(root: &Path) -> Result<(), StorageError> {
    let target = root.join(SETTINGS_FILE);
    let staging = root.join(SETTINGS_STAGING_FILE);
    let backup = root.join(SETTINGS_BACKUP_FILE);
    if !target.exists() && backup.is_file() {
        fs::rename(&backup, &target).map_err(|_| StorageError::Io)?;
    } else if backup.exists() {
        fs::remove_file(&backup).map_err(|_| StorageError::Io)?;
    }
    if staging.exists() {
        fs::remove_file(staging).map_err(|_| StorageError::Io)?;
    }
    Ok(())
}

fn directory_size(root: &Path) -> Result<u64, StorageError> {
    if !root.exists() {
        return Ok(0);
    }
    let mut total = 0_u64;
    let mut scanned = 0_usize;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| StorageError::Io)? {
            let entry = entry.map_err(|_| StorageError::Io)?;
            scanned = scanned.saturating_add(1);
            if scanned > MAX_SCANNED_ENTRIES {
                return Err(StorageError::Io);
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| StorageError::Io)?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}

#[cfg(windows)]
fn volume_space(path: &Path) -> Result<VolumeSpace, StorageError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0_u64;
    let mut total = 0_u64;
    let mut total_free = 0_u64;
    let succeeded =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut total_free) };
    if succeeded == 0 {
        Err(StorageError::Io)
    } else {
        Ok(VolumeSpace {
            total_bytes: total,
            available_bytes: available,
        })
    }
}

// libc exposes these statvfs counters with different widths across Unix targets.
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn volume_space(path: &Path) -> Result<VolumeSpace, StorageError> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| StorageError::Io)?;
    let mut result = MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), result.as_mut_ptr()) } != 0 {
        return Err(StorageError::Io);
    }
    let result = unsafe { result.assume_init() };
    let block_size = result.f_frsize as u64;
    Ok(VolumeSpace {
        total_bytes: (result.f_blocks as u64).saturating_mul(block_size),
        available_bytes: (result.f_bavail as u64).saturating_mul(block_size),
    })
}

#[cfg(not(any(unix, windows)))]
fn volume_space(_path: &Path) -> Result<VolumeSpace, StorageError> {
    Err(StorageError::Io)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_write_capacity, evaluate_status, StorageError, StorageHealth, StorageManager,
        VolumeSpace, ALLOWED_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES, GIB,
        STORAGE_RESERVE_BYTES, STORAGE_WARNING_BYTES,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pixiv-client-storage-{name}-{nonce}"))
    }

    #[test]
    fn persists_only_supported_cache_limits_and_resets_to_default() {
        let root = test_root("settings");
        let manager = StorageManager::open(root.join("data"), root.join("cache")).unwrap();
        assert_eq!(
            manager.cache_limit_bytes().unwrap(),
            Some(DEFAULT_CACHE_LIMIT_BYTES)
        );
        manager
            .set_cache_limit(Some(ALLOWED_CACHE_LIMIT_BYTES[2]))
            .unwrap();
        assert_eq!(
            manager.cache_limit_bytes().unwrap(),
            Some(ALLOWED_CACHE_LIMIT_BYTES[2])
        );
        assert_eq!(
            manager.set_cache_limit(Some(123)),
            Err(StorageError::InvalidCacheLimit)
        );
        drop(manager);

        let reopened = StorageManager::open(root.join("data"), root.join("cache")).unwrap();
        assert_eq!(
            reopened.cache_limit_bytes().unwrap(),
            Some(ALLOWED_CACHE_LIMIT_BYTES[2])
        );
        reopened.reset_settings().unwrap();
        assert_eq!(
            reopened.cache_limit_bytes().unwrap(),
            Some(DEFAULT_CACHE_LIMIT_BYTES)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unlimited_cache_preserves_disk_reserve_without_reporting_a_quota() {
        let status = evaluate_status(
            VolumeSpace {
                total_bytes: 20 * GIB,
                available_bytes: 10 * GIB,
            },
            VolumeSpace {
                total_bytes: 20 * GIB,
                available_bytes: 10 * GIB,
            },
            0,
            12 * GIB,
            None,
        );

        assert_eq!(status.cache_limit_bytes, None);
        assert_eq!(status.cache_remaining_quota_bytes, None);
        assert_eq!(
            status.writable_download_bytes,
            10 * GIB - STORAGE_RESERVE_BYTES
        );
    }

    #[test]
    fn corrupt_settings_fail_closed_to_the_bounded_default() {
        let root = test_root("corrupt");
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("storage-settings-v1.json"), b"{not-json}").unwrap();
        let manager = StorageManager::open(&data, root.join("cache")).unwrap();
        assert_eq!(
            manager.cache_limit_bytes().unwrap(),
            Some(DEFAULT_CACHE_LIMIT_BYTES)
        );
        let persisted = fs::read_to_string(data.join("storage-settings-v1.json")).unwrap();
        assert!(persisted.contains(&DEFAULT_CACHE_LIMIT_BYTES.to_string()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn health_and_headroom_use_the_least_available_application_volume() {
        let status = evaluate_status(
            VolumeSpace {
                total_bytes: 10 * STORAGE_WARNING_BYTES,
                available_bytes: STORAGE_WARNING_BYTES + 1,
            },
            VolumeSpace {
                total_bytes: 10 * STORAGE_WARNING_BYTES,
                available_bytes: STORAGE_RESERVE_BYTES + 1,
            },
            42,
            17,
            Some(128),
        );
        assert_eq!(status.health, StorageHealth::Low);
        assert_eq!(
            status.writable_download_bytes,
            STORAGE_WARNING_BYTES + 1 - STORAGE_RESERVE_BYTES
        );
        assert_eq!(status.cache_remaining_quota_bytes, Some(111));

        let critical = evaluate_status(
            VolumeSpace {
                total_bytes: STORAGE_WARNING_BYTES,
                available_bytes: STORAGE_RESERVE_BYTES,
            },
            VolumeSpace {
                total_bytes: STORAGE_WARNING_BYTES,
                available_bytes: STORAGE_WARNING_BYTES,
            },
            0,
            0,
            Some(128),
        );
        assert_eq!(critical.health, StorageHealth::Critical);
    }

    #[test]
    fn write_preflight_preserves_the_fixed_application_reserve() {
        let exact = VolumeSpace {
            total_bytes: u64::MAX,
            available_bytes: STORAGE_RESERVE_BYTES + 4096,
        };
        assert_eq!(ensure_write_capacity(exact, 4096), Ok(()));
        assert!(matches!(
            ensure_write_capacity(exact, 4097),
            Err(StorageError::InsufficientSpace {
                available_bytes,
                required_bytes: 4097,
                reserve_bytes: STORAGE_RESERVE_BYTES,
            }) if available_bytes == exact.available_bytes
        ));
        assert_eq!(
            ensure_write_capacity(exact, 0),
            Err(StorageError::InvalidWriteSize)
        );
    }

    #[test]
    fn status_counts_only_owned_offline_and_media_cache_roots() {
        let root = test_root("status");
        let data = root.join("data");
        let cache = root.join("cache");
        let manager = StorageManager::open(&data, &cache).unwrap();
        fs::create_dir_all(data.join("offline-library/item/assets")).unwrap();
        fs::create_dir_all(cache.join("media-v1/verified")).unwrap();
        fs::write(
            data.join("offline-library/item/assets/page.bin"),
            vec![1_u8; 19],
        )
        .unwrap();
        fs::write(cache.join("media-v1/verified/cache.bin"), vec![2_u8; 23]).unwrap();
        fs::write(data.join("unrelated.bin"), vec![3_u8; 101]).unwrap();

        let status = manager.status().unwrap();
        assert_eq!(status.offline_bytes, 19);
        assert_eq!(status.cache_bytes, 23);
        assert!(status.data_total_bytes >= status.data_available_bytes);
        assert!(status.cache_total_bytes >= status.cache_available_bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
