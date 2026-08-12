import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("offline library exports validated entries atomically without overwriting unrelated data", async () => {
  const library = await read("crates/library/src/lib.rs");
  assert.match(library, /pub fn export_entry/);
  assert.match(library, /ensure_replaceable_export_target/);
  assert.match(library, /EXPORT_MARKER_FILE: &str = "pixiv-client-entry\.json"/);
  assert.match(library, /fs::rename\(&target, &backup\)/);
  assert.match(library, /metadata\.file_type\(\)\.is_symlink\(\)/);
  assert.match(library, /LibraryError::ExportConflict/);
  assert.match(library, /refuses_to_overwrite_an_unrelated_export_directory/);
});

test("desktop and Android adapters share one export destination interface", async () => {
  const [manifest, application, exports] = await Promise.all([
    read("src-tauri/Cargo.toml"),
    read("src-tauri/src/lib.rs"),
    read("src-tauri/src/exports.rs"),
  ]);
  assert.match(manifest, /tauri-plugin-dialog = "2"/);
  assert.match(application, /plugin\(tauri_plugin_dialog::init\(\)\)/);
  assert.match(application, /plugin\(exports::android_export_plugin\(\)\)/);
  assert.match(exports, /blocking_pick_folder\(\)/);
  assert.match(exports, /register_android_plugin\("io\.github\.space2233\.pixnya", "ExportDirectoryPlugin"\)/);
  for (const command of [
    "get_export_destination_status",
    "select_export_destination",
    "clear_export_destination",
    "set_auto_export_downloads",
    "export_offline_entry",
  ]) {
    assert.match(application, new RegExp(`${command},`));
  }
});

test("Android SAF adapter retains tree permission and verifies every exported file", async () => {
  const android = await readFile(androidPackagePath("ExportDirectoryPlugin.kt"), "utf8");
  assert.match(android, /Intent\.ACTION_OPEN_DOCUMENT_TREE/);
  assert.match(android, /takePersistableUriPermission/);
  assert.match(android, /persistedUriPermissions/);
  assert.match(android, /File\(activity\.cacheDir, EXPORT_STAGING_DIRECTORY\)\.canonicalFile/);
  assert.match(android, /source\.parentFile\?\.canonicalFile == allowedRoot/);
  assert.match(android, /isOwnedExportDirectory/);
  assert.match(android, /entry\.optString\("key"\) == expectedKey/);
  assert.match(android, /openInputStream\(created\)/);
  assert.match(android, /verifiedBytes == expectedBytes/);
  assert.match(android, /releasePersistableUriPermission/);
  assert.doesNotMatch(android, /READ_EXTERNAL_STORAGE|WRITE_EXTERNAL_STORAGE|MANAGE_EXTERNAL_STORAGE/);
});

test("download worker exports before completion and settings expose manual recovery", async () => {
  const [downloads, types, api, settings, offline] = await Promise.all([
    read("src-tauri/src/downloads.rs"),
    read("src/lib/types.ts"),
    read("src/lib/pixiv-api.ts"),
    read("src/routes/settings/storage/+page.svelte"),
    read("src/routes/offline/+page.svelte"),
  ]);
  assert.match(downloads, /auto_export_offline_entry\(&app, &entry_key\)[\s\S]*?mark_completed/);
  assert.match(types, /export interface ExportDestinationStatus/);
  assert.match(
    api,
    /invoke<ExportDestinationSelection>\("select_export_destination", \{ title \}\)/,
  );
  assert.match(api, /invoke<OfflineExportResult>\("export_offline_entry"/);
  assert.match(settings, /m\.settings_export_directory\(\)/);
  assert.match(settings, /m\.settings_auto_export\(\)/);
  assert.match(offline, /exportOfflineEntry\(entry\.key\)/);
  assert.match(offline, /m\.offline_export_guidance_after\(\)/);
});

test("clearing local data revokes export authorization without deleting user exports", async () => {
  const [application, exports, settings] = await Promise.all([
    read("src-tauri/src/lib.rs"),
    read("src-tauri/src/exports.rs"),
    read("src/routes/settings/storage/+page.svelte"),
  ]);
  assert.match(application, /clear_all_export_settings\(&app\)\.await/);
  assert.match(application, /LocalDataClearFailure::ExportSettings/);
  assert.match(exports, /clear_platform_destination\(app\)\.await/);
  assert.match(exports, /manager\.clear_settings\(\)/);
  assert.match(settings, /clearExportDestination\(\)/);
});
