use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const INDEX_FILE: &str = "index.json";
const INDEX_STAGING_FILE: &str = ".index.staging";
const INDEX_BACKUP_FILE: &str = ".index.backup";
const FORMAT_VERSION: u8 = 2;
const MAX_SOURCE_KEY_BYTES: usize = 4096;
const INDEX_FLUSH_HIT_INTERVAL: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheScope {
    Verified,
    Insecure,
}

impl CacheScope {
    fn code(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Insecure => "insecure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheKind {
    Thumbnail,
    Preview,
    Original,
}

impl CacheKind {
    fn code(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Preview => "preview",
            Self::Original => "original",
        }
    }

    fn eviction_priority(self) -> u8 {
        match self {
            Self::Original => 0,
            Self::Preview => 1,
            Self::Thumbnail => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub entry_count: u32,
    pub size_bytes: u64,
    pub verified_bytes: u64,
    pub insecure_bytes: u64,
    pub thumbnail_bytes: u64,
    pub preview_bytes: u64,
    pub original_bytes: u64,
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheError {
    InvalidSourceKey,
    AssetTooLarge,
    Io,
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSourceKey => "invalid media cache source key",
            Self::AssetTooLarge => "media cache asset exceeds the configured capacity",
            Self::Io => "media cache I/O failed",
        })
    }
}

impl std::error::Error for CacheError {}

pub struct MediaCache {
    root: PathBuf,
    max_bytes: Option<u64>,
    index: StoredIndex,
    index_dirty: bool,
    unpersisted_hits: u32,
}

impl MediaCache {
    pub fn open(root: impl Into<PathBuf>, max_bytes: Option<u64>) -> Result<Self, CacheError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|_| CacheError::Io)?;
        restore_interrupted_index(&root)?;
        ensure_scope_directories(&root)?;

        let index_path = root.join(INDEX_FILE);
        let index = if index_path.is_file() {
            fs::read(&index_path)
                .ok()
                .filter(|bytes| bytes.len() <= 8 * 1024 * 1024)
                .and_then(|bytes| serde_json::from_slice::<StoredIndex>(&bytes).ok())
                .filter(|index| index.format_version == FORMAT_VERSION)
        } else {
            Some(StoredIndex::default())
        };

        let mut cache = Self {
            root,
            max_bytes,
            index: index.unwrap_or_default(),
            index_dirty: false,
            unpersisted_hits: 0,
        };
        cache.reconcile()?;
        cache.trim_to_capacity()?;
        cache.persist_index()?;
        Ok(cache)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn max_bytes(&self) -> Option<u64> {
        self.max_bytes
    }

    pub fn set_max_bytes(&mut self, max_bytes: Option<u64>) -> Result<(), CacheError> {
        if self.max_bytes == max_bytes {
            return Ok(());
        }
        self.max_bytes = max_bytes;
        self.trim_to_capacity()?;
        self.persist_index()
    }

    pub fn get(
        &mut self,
        scope: CacheScope,
        kind: CacheKind,
        source_key: &str,
        max_asset_bytes: u64,
    ) -> Result<Option<Vec<u8>>, CacheError> {
        validate_source_key(source_key)?;
        let id = cache_id(scope, kind, source_key);
        let Some(entry) = self.index.entries.get(&id).cloned() else {
            return Ok(None);
        };
        if entry.scope != scope || entry.kind != kind || entry.size_bytes > max_asset_bytes {
            self.remove_indexed_entry(&id, &entry);
            self.persist_index()?;
            return Ok(None);
        }

        let path = self.entry_path(&id, entry.scope);
        let bytes = match fs::read(path) {
            Ok(bytes)
                if bytes.len() as u64 == entry.size_bytes
                    && content_sha256(&bytes) == entry.content_sha256 =>
            {
                bytes
            }
            _ => {
                self.remove_indexed_entry(&id, &entry);
                self.persist_index()?;
                return Ok(None);
            }
        };
        let access_order = self.next_access_order();
        if let Some(entry) = self.index.entries.get_mut(&id) {
            entry.access_order = access_order;
        }
        self.mark_hit_dirty()?;
        Ok(Some(bytes))
    }

    pub fn put(
        &mut self,
        scope: CacheScope,
        kind: CacheKind,
        source_key: &str,
        bytes: &[u8],
    ) -> Result<(), CacheError> {
        validate_source_key(source_key)?;
        if bytes.is_empty()
            || self
                .max_bytes
                .is_some_and(|limit| bytes.len() as u64 > limit)
        {
            return Err(CacheError::AssetTooLarge);
        }

        let id = cache_id(scope, kind, source_key);
        let target = self.entry_path(&id, scope);
        let staging = target.with_extension("staging");
        fs::write(&staging, bytes).map_err(|_| CacheError::Io)?;
        if target.exists() {
            fs::remove_file(&target).map_err(|_| CacheError::Io)?;
        }
        if fs::rename(&staging, &target).is_err() {
            let _ = fs::remove_file(&staging);
            return Err(CacheError::Io);
        }

        let access_order = self.next_access_order();
        self.index.entries.insert(
            id,
            StoredEntry {
                scope,
                kind,
                size_bytes: bytes.len() as u64,
                content_sha256: content_sha256(bytes),
                access_order,
            },
        );
        self.trim_to_capacity()?;
        self.persist_index()
    }

    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats {
            max_bytes: self.max_bytes,
            ..CacheStats::default()
        };
        stats.entry_count = u32::try_from(self.index.entries.len()).unwrap_or(u32::MAX);
        for entry in self.index.entries.values() {
            stats.size_bytes = stats.size_bytes.saturating_add(entry.size_bytes);
            match entry.scope {
                CacheScope::Verified => {
                    stats.verified_bytes = stats.verified_bytes.saturating_add(entry.size_bytes)
                }
                CacheScope::Insecure => {
                    stats.insecure_bytes = stats.insecure_bytes.saturating_add(entry.size_bytes)
                }
            }
            match entry.kind {
                CacheKind::Thumbnail => {
                    stats.thumbnail_bytes = stats.thumbnail_bytes.saturating_add(entry.size_bytes)
                }
                CacheKind::Preview => {
                    stats.preview_bytes = stats.preview_bytes.saturating_add(entry.size_bytes)
                }
                CacheKind::Original => {
                    stats.original_bytes = stats.original_bytes.saturating_add(entry.size_bytes)
                }
            }
        }
        stats
    }

    pub fn clear(&mut self) -> Result<CacheStats, CacheError> {
        let removed = self.stats();
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(|_| CacheError::Io)?;
        }
        fs::create_dir_all(&self.root).map_err(|_| CacheError::Io)?;
        ensure_scope_directories(&self.root)?;
        self.index = StoredIndex::default();
        self.persist_index()?;
        Ok(removed)
    }

    fn reconcile(&mut self) -> Result<(), CacheError> {
        let root = self.root.clone();
        self.index.entries.retain(|id, entry| {
            entry.kind != CacheKind::Original
                && valid_cache_id(id)
                && root
                    .join(entry.scope.code())
                    .join(format!("{id}.bin"))
                    .metadata()
                    .is_ok_and(|metadata| metadata.is_file() && metadata.len() == entry.size_bytes)
        });
        let indexed: BTreeSet<PathBuf> = self
            .index
            .entries
            .iter()
            .map(|(id, entry)| self.entry_path(id, entry.scope))
            .collect();
        for scope in [CacheScope::Verified, CacheScope::Insecure] {
            let directory = self.root.join(scope.code());
            for item in fs::read_dir(directory).map_err(|_| CacheError::Io)? {
                let item = item.map_err(|_| CacheError::Io)?;
                if item.file_type().map_err(|_| CacheError::Io)?.is_file()
                    && !indexed.contains(&item.path())
                {
                    fs::remove_file(item.path()).map_err(|_| CacheError::Io)?;
                }
            }
        }
        Ok(())
    }

    fn trim_to_capacity(&mut self) -> Result<(), CacheError> {
        let mut total: u64 = self
            .index
            .entries
            .values()
            .map(|entry| entry.size_bytes)
            .sum();
        let Some(max_bytes) = self.max_bytes else {
            return Ok(());
        };
        if total <= max_bytes {
            return Ok(());
        }
        let mut oldest: Vec<(String, StoredEntry)> = self
            .index
            .entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect();
        oldest.sort_by_key(|(_, entry)| (entry.kind.eviction_priority(), entry.access_order));
        for (id, entry) in oldest {
            if total <= max_bytes {
                break;
            }
            total = total.saturating_sub(entry.size_bytes);
            self.remove_indexed_entry(&id, &entry);
        }
        Ok(())
    }

    fn remove_indexed_entry(&mut self, id: &str, entry: &StoredEntry) {
        self.index.entries.remove(id);
        let _ = fs::remove_file(self.entry_path(id, entry.scope));
    }

    fn next_access_order(&mut self) -> u64 {
        let current = self.index.next_access_order.max(1);
        self.index.next_access_order = current.saturating_add(1);
        current
    }

    fn entry_path(&self, id: &str, scope: CacheScope) -> PathBuf {
        self.root.join(scope.code()).join(format!("{id}.bin"))
    }

    fn mark_hit_dirty(&mut self) -> Result<(), CacheError> {
        self.index_dirty = true;
        self.unpersisted_hits = self.unpersisted_hits.saturating_add(1);
        if self.unpersisted_hits >= INDEX_FLUSH_HIT_INTERVAL {
            self.persist_index()?;
        }
        Ok(())
    }

    fn persist_index(&mut self) -> Result<(), CacheError> {
        let bytes = serde_json::to_vec(&self.index).map_err(|_| CacheError::Io)?;
        let staging = self.root.join(INDEX_STAGING_FILE);
        let index = self.root.join(INDEX_FILE);
        let backup = self.root.join(INDEX_BACKUP_FILE);
        fs::write(&staging, bytes).map_err(|_| CacheError::Io)?;
        let replacing = index.exists();
        if replacing {
            let _ = fs::remove_file(&backup);
            fs::rename(&index, &backup).map_err(|_| CacheError::Io)?;
        }
        if fs::rename(&staging, &index).is_err() {
            if replacing {
                let _ = fs::rename(&backup, &index);
            }
            return Err(CacheError::Io);
        }
        if replacing {
            let _ = fs::remove_file(backup);
        }
        self.index_dirty = false;
        self.unpersisted_hits = 0;
        Ok(())
    }
}

impl Drop for MediaCache {
    fn drop(&mut self) {
        if self.index_dirty {
            let _ = self.persist_index();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEntry {
    scope: CacheScope,
    kind: CacheKind,
    size_bytes: u64,
    content_sha256: String,
    access_order: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredIndex {
    format_version: u8,
    next_access_order: u64,
    entries: BTreeMap<String, StoredEntry>,
}

impl Default for StoredIndex {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            next_access_order: 1,
            entries: BTreeMap::new(),
        }
    }
}

fn validate_source_key(source_key: &str) -> Result<(), CacheError> {
    if source_key.is_empty()
        || source_key.len() > MAX_SOURCE_KEY_BYTES
        || source_key.chars().any(char::is_control)
    {
        Err(CacheError::InvalidSourceKey)
    } else {
        Ok(())
    }
}

fn cache_id(scope: CacheScope, kind: CacheKind, source_key: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pixiv-client-media-cache-v1\0");
    digest.update(scope.code().as_bytes());
    digest.update(b"\0");
    digest.update(kind.code().as_bytes());
    digest.update(b"\0");
    digest.update(source_key.as_bytes());
    format!("{:x}", digest.finalize())
}

fn content_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_cache_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_scope_directories(root: &Path) -> Result<(), CacheError> {
    for scope in [CacheScope::Verified, CacheScope::Insecure] {
        fs::create_dir_all(root.join(scope.code())).map_err(|_| CacheError::Io)?;
    }
    Ok(())
}

fn restore_interrupted_index(root: &Path) -> Result<(), CacheError> {
    let index = root.join(INDEX_FILE);
    let backup = root.join(INDEX_BACKUP_FILE);
    let staging = root.join(INDEX_STAGING_FILE);
    if !index.exists() && backup.is_file() {
        fs::rename(&backup, &index).map_err(|_| CacheError::Io)?;
    } else if backup.exists() {
        fs::remove_file(&backup).map_err(|_| CacheError::Io)?;
    }
    if staging.exists() {
        fs::remove_file(staging).map_err(|_| CacheError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CacheKind, CacheScope, MediaCache, StoredIndex, INDEX_FILE};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pixiv-client-cache-{name}-{nonce}"))
    }

    #[test]
    fn stores_entries_without_persisting_source_urls_and_separates_security_scopes() {
        let root = test_root("privacy");
        let mut cache = MediaCache::open(&root, Some(64)).unwrap();
        let source = "i.pximg.net/img-master/42_p0_master1200.jpg";

        cache
            .put(CacheScope::Verified, CacheKind::Preview, source, b"safe")
            .unwrap();
        cache
            .put(CacheScope::Insecure, CacheKind::Preview, source, b"unsafe")
            .unwrap();

        assert_eq!(
            cache
                .get(CacheScope::Verified, CacheKind::Preview, source, 16)
                .unwrap(),
            Some(b"safe".to_vec())
        );
        assert_eq!(
            cache
                .get(CacheScope::Insecure, CacheKind::Preview, source, 16)
                .unwrap(),
            Some(b"unsafe".to_vec())
        );
        for file in walk_files(&root) {
            let bytes = fs::read(&file).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains(source));
            assert!(!file.to_string_lossy().contains("42_p0"));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn evicts_least_recently_used_entries_to_the_configured_capacity() {
        let root = test_root("lru");
        let mut cache = MediaCache::open(&root, Some(6)).unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "one", b"111")
            .unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "two", b"22")
            .unwrap();
        assert!(cache
            .get(CacheScope::Verified, CacheKind::Thumbnail, "one", 8)
            .unwrap()
            .is_some());
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "three", b"333")
            .unwrap();

        assert!(cache
            .get(CacheScope::Verified, CacheKind::Thumbnail, "one", 8)
            .unwrap()
            .is_some());
        assert!(cache
            .get(CacheScope::Verified, CacheKind::Thumbnail, "two", 8)
            .unwrap()
            .is_none());
        assert!(cache.stats().size_bytes <= 6);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn evicts_originals_and_previews_before_thumbnails() {
        let root = test_root("kind-priority");
        let mut cache = MediaCache::open(&root, Some(8)).unwrap();
        cache
            .put(
                CacheScope::Verified,
                CacheKind::Thumbnail,
                "old-thumbnail",
                b"1111",
            )
            .unwrap();
        cache
            .put(
                CacheScope::Verified,
                CacheKind::Preview,
                "new-preview",
                b"2222",
            )
            .unwrap();
        cache
            .put(
                CacheScope::Verified,
                CacheKind::Original,
                "newest-original",
                b"3333",
            )
            .unwrap();

        assert!(cache
            .get(
                CacheScope::Verified,
                CacheKind::Original,
                "newest-original",
                8
            )
            .unwrap()
            .is_none());
        assert!(cache
            .get(
                CacheScope::Verified,
                CacheKind::Thumbnail,
                "old-thumbnail",
                8
            )
            .unwrap()
            .is_some());

        cache
            .put(
                CacheScope::Verified,
                CacheKind::Thumbnail,
                "latest-thumbnail",
                b"4444",
            )
            .unwrap();
        assert!(cache
            .get(CacheScope::Verified, CacheKind::Preview, "new-preview", 8)
            .unwrap()
            .is_none());
        assert_eq!(cache.stats().thumbnail_bytes, 8);
        assert_eq!(cache.stats().preview_bytes, 0);
        assert_eq!(cache.stats().original_bytes, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unlimited_cache_keeps_entries_without_capacity_eviction() {
        let root = test_root("unlimited");
        let mut cache = MediaCache::open(&root, None).unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "one", b"1111")
            .unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "two", b"2222")
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.size_bytes, 8);
        assert_eq!(stats.max_bytes, None);
        assert_eq!(
            cache
                .get(CacheScope::Verified, CacheKind::Thumbnail, "one", 8)
                .unwrap(),
            Some(b"1111".to_vec())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reopening_the_cache_restores_a_warm_entry() {
        let root = test_root("reopen");
        {
            let mut cache = MediaCache::open(&root, Some(64)).unwrap();
            cache
                .put(CacheScope::Verified, CacheKind::Thumbnail, "warm", b"image")
                .unwrap();
        }

        let mut reopened = MediaCache::open(&root, Some(64)).unwrap();
        assert_eq!(
            reopened
                .get(CacheScope::Verified, CacheKind::Thumbnail, "warm", 16)
                .unwrap(),
            Some(b"image".to_vec())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reopening_purges_legacy_originals_without_dropping_previews() {
        let root = test_root("legacy-original-purge");
        {
            let mut cache = MediaCache::open(&root, None).unwrap();
            cache
                .put(
                    CacheScope::Verified,
                    CacheKind::Preview,
                    "preview",
                    b"preview",
                )
                .unwrap();
            cache
                .put(
                    CacheScope::Verified,
                    CacheKind::Original,
                    "original",
                    b"original",
                )
                .unwrap();
            assert_eq!(cache.stats().original_bytes, 8);
        }

        let mut reopened = MediaCache::open(&root, None).unwrap();
        assert_eq!(reopened.stats().original_bytes, 0);
        assert_eq!(
            reopened
                .get(CacheScope::Verified, CacheKind::Preview, "preview", 16)
                .unwrap(),
            Some(b"preview".to_vec())
        );
        assert_eq!(
            reopened
                .get(CacheScope::Verified, CacheKind::Original, "original", 16)
                .unwrap(),
            None
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_corrupt_index_is_rebuilt_without_serving_orphaned_bytes() {
        let root = test_root("corrupt-index");
        let mut cache = MediaCache::open(&root, Some(64)).unwrap();
        cache
            .put(
                CacheScope::Verified,
                CacheKind::Thumbnail,
                "orphan",
                b"image",
            )
            .unwrap();
        fs::write(root.join(INDEX_FILE), b"not-json").unwrap();

        let mut reopened = MediaCache::open(&root, Some(64)).unwrap();
        assert_eq!(reopened.stats().entry_count, 0);
        assert_eq!(
            reopened
                .get(CacheScope::Verified, CacheKind::Thumbnail, "orphan", 16)
                .unwrap(),
            None
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_length_cache_corruption_is_not_served_after_reopen() {
        let root = test_root("corrupt-bytes");
        {
            let mut cache = MediaCache::open(&root, Some(64)).unwrap();
            cache
                .put(
                    CacheScope::Verified,
                    CacheKind::Thumbnail,
                    "corrupt",
                    b"image",
                )
                .unwrap();
        }
        let data_file = walk_files(&root)
            .into_iter()
            .find(|path| path.extension().is_some_and(|extension| extension == "bin"))
            .unwrap();
        fs::write(data_file, b"xxxxx").unwrap();

        let mut reopened = MediaCache::open(&root, Some(64)).unwrap();
        assert_eq!(
            reopened
                .get(CacheScope::Verified, CacheKind::Thumbnail, "corrupt", 16)
                .unwrap(),
            None
        );
        assert_eq!(reopened.stats().entry_count, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reopening_with_a_smaller_limit_immediately_trims_least_recently_used_entries() {
        let root = test_root("limit-shrink");
        {
            let mut cache = MediaCache::open(&root, None).unwrap();
            cache
                .put(CacheScope::Verified, CacheKind::Thumbnail, "old", b"111")
                .unwrap();
            cache
                .put(CacheScope::Verified, CacheKind::Thumbnail, "new", b"222")
                .unwrap();
        }

        let mut limited = MediaCache::open(&root, Some(3)).unwrap();
        assert!(limited
            .get(CacheScope::Verified, CacheKind::Thumbnail, "old", 8)
            .unwrap()
            .is_none());
        assert_eq!(
            limited
                .get(CacheScope::Verified, CacheKind::Thumbnail, "new", 8)
                .unwrap(),
            Some(b"222".to_vec())
        );
        assert_eq!(limited.stats().size_bytes, 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shrinking_the_limit_on_an_open_cache_trims_without_reopening() {
        let root = test_root("limit-shrink-resident");
        let mut cache = MediaCache::open(&root, None).unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "old", b"111")
            .unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "new", b"222")
            .unwrap();

        cache.set_max_bytes(Some(3)).unwrap();
        assert!(cache
            .get(CacheScope::Verified, CacheKind::Thumbnail, "old", 8)
            .unwrap()
            .is_none());
        assert_eq!(
            cache
                .get(CacheScope::Verified, CacheKind::Thumbnail, "new", 8)
                .unwrap(),
            Some(b"222".to_vec())
        );
        assert_eq!(cache.stats().size_bytes, 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clearing_cache_never_removes_the_adjacent_offline_library() {
        let parent = test_root("clear-boundary");
        let root = parent.join("media-cache");
        let offline = parent.join("offline-library");
        fs::create_dir_all(&offline).unwrap();
        fs::write(offline.join("keep.txt"), b"download").unwrap();
        let mut cache = MediaCache::open(&root, Some(64)).unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "one", b"cached")
            .unwrap();

        let removed = cache.clear().unwrap();

        assert_eq!(removed.entry_count, 1);
        assert!(offline.join("keep.txt").is_file());
        assert_eq!(cache.stats().entry_count, 0);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn cache_hits_do_not_rewrite_the_index_until_a_flush() {
        let root = test_root("hit-no-rewrite");
        let mut cache = MediaCache::open(&root, Some(64)).unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "warm", b"image")
            .unwrap();
        let index_path = root.join(INDEX_FILE);
        let before = fs::read(&index_path).unwrap();

        assert_eq!(
            cache
                .get(CacheScope::Verified, CacheKind::Thumbnail, "warm", 16)
                .unwrap(),
            Some(b"image".to_vec())
        );
        assert_eq!(fs::read(&index_path).unwrap(), before);

        drop(cache);
        let flushed = fs::read(&index_path).unwrap();
        assert_ne!(flushed, before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dropping_a_dirty_cache_persists_lru_order_for_the_next_open() {
        let root = test_root("dirty-lru-flush");
        {
            let mut cache = MediaCache::open(&root, Some(6)).unwrap();
            cache
                .put(CacheScope::Verified, CacheKind::Thumbnail, "one", b"111")
                .unwrap();
            cache
                .put(CacheScope::Verified, CacheKind::Thumbnail, "two", b"22")
                .unwrap();
            assert!(cache
                .get(CacheScope::Verified, CacheKind::Thumbnail, "one", 8)
                .unwrap()
                .is_some());
        }

        let mut cache = MediaCache::open(&root, Some(6)).unwrap();
        cache
            .put(CacheScope::Verified, CacheKind::Thumbnail, "three", b"333")
            .unwrap();
        assert!(cache
            .get(CacheScope::Verified, CacheKind::Thumbnail, "one", 8)
            .unwrap()
            .is_some());
        assert!(cache
            .get(CacheScope::Verified, CacheKind::Thumbnail, "two", 8)
            .unwrap()
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_hits_still_persist_index_removal_immediately() {
        let root = test_root("corrupt-hit-persist");
        let mut cache = MediaCache::open(&root, Some(64)).unwrap();
        cache
            .put(
                CacheScope::Verified,
                CacheKind::Thumbnail,
                "corrupt",
                b"image",
            )
            .unwrap();
        let data_file = walk_files(&root)
            .into_iter()
            .find(|path| path.extension().is_some_and(|extension| extension == "bin"))
            .unwrap();
        fs::write(data_file, b"xxxxx").unwrap();

        assert_eq!(
            cache
                .get(CacheScope::Verified, CacheKind::Thumbnail, "corrupt", 16)
                .unwrap(),
            None
        );
        let index: StoredIndex =
            serde_json::from_slice(&fs::read(root.join(INDEX_FILE)).unwrap()).unwrap();
        assert!(index.entries.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn walk_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_owned()];
        while let Some(path) = pending.pop() {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    pending.push(entry.path());
                } else {
                    files.push(entry.path());
                }
            }
        }
        files
    }
}
