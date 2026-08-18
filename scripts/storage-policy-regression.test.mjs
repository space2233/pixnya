import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("storage policy owns cross-platform space checks and atomic bounded settings", async () => {
  const [workspace, manifest, policy] = await Promise.all([
    read("Cargo.toml"),
    read("crates/storage-policy/Cargo.toml"),
    read("crates/storage-policy/src/lib.rs"),
  ]);
  assert.match(workspace, /"crates\/storage-policy"/);
  assert.match(manifest, /Win32_Storage_FileSystem/);
  assert.match(manifest, /libc = "0\.2\.189"/);
  assert.match(policy, /GetDiskFreeSpaceExW/);
  assert.match(policy, /libc::statvfs/);
  assert.match(
    policy,
    /#\[cfg\(unix\)\]\s*#\[allow\(clippy::unnecessary_cast\)\]\s*fn volume_space/,
  );
  assert.match(policy, /STORAGE_RESERVE_BYTES: u64 = 512 \* MIB/);
  assert.match(policy, /ALLOWED_CACHE_LIMIT_BYTES/);
  assert.match(policy, /persist_settings/);
  assert.match(policy, /restore_interrupted_settings/);
  assert.match(policy, /pub fn ensure_offline_write/);
  assert.match(policy, /pub fn allows_cache_write/);
});

test("Tauri applies storage policy before offline writes and best-effort cache writes", async () => {
  const application = await read("src-tauri/src/lib.rs");
  assert.match(application, /StoragePolicyState/);
  assert.match(application, /storage\.ensure_offline_write\(1\)/);
  assert.match(application, /ensure_offline_write\(media\.bytes\.len\(\) as u64\)/);
  assert.match(application, /storage\.ensure_offline_write\(required_bytes\.max\(1\)\)/);
  assert.match(application, /storage\.ensure_offline_write\(expected_bytes\.max\(1\)\)/);
  assert.match(application, /allows_cache_write\(media\.bytes\.len\(\) as u64\)/);
  assert.match(application, /get_storage_status,/);
  assert.match(application, /set_media_cache_limit,/);
  assert.match(application, /storage_settings_reset/);
});

test("settings exposes storage health, safe headroom, and supported cache limits", async () => {
  const [types, api, settings, view] = await Promise.all([
    read("src/lib/types.ts"),
    read("src/lib/pixiv-api.ts"),
    read("src/routes/settings/storage/+page.svelte"),
    read("src/lib/storage-status-view.ts"),
  ]);
  assert.match(types, /export interface StorageStatus/);
  assert.match(types, /export type StorageHealth = "healthy" \| "low" \| "critical"/);
  assert.match(api, /invoke<StorageStatus>\("get_storage_status"\)/);
  assert.match(api, /invoke<StorageStatus>\("set_media_cache_limit"/);
  assert.match(settings, /storageMetric\(storageLoadState, metric\)/);
  assert.match(view, /offlineBytes \+ state\.value\.cacheBytes/);
  assert.match(view, /state\.value\.dataAvailableBytes/);
  assert.match(settings, /m\.settings_cache_limit\(\)/);
  assert.match(types, /cacheLimitBytes: number \| null/);
  assert.match(types, /cacheRemainingQuotaBytes: number \| null/);
  assert.match(api, /setMediaCacheLimit\(cacheLimitBytes: number \| null\)/);
  for (const label of ["128 MiB", "256 MiB", "512 MiB", "1 GiB", "5 GiB", "10 GiB"]) {
    assert.match(settings, new RegExp(label));
  }
  assert.match(settings, /value === "unlimited" \? null/);
});
