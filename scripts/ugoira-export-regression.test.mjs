import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

test("Ugoira export is a cancellable background task with strict budgets and cleanup", async () => {
  const [rust, runtime] = await Promise.all([
    source("src-tauri/src/ugoira_export.rs"),
    source("src-tauri/src/lib.rs"),
  ]);
  assert.match(rust, /enum UgoiraExportFormat[\s\S]*Gif[\s\S]*Apng[\s\S]*Webm/);
  assert.match(rust, /MAX_EXPORT_FRAMES/);
  assert.match(rust, /MAX_DECODED_MEMORY_BYTES/);
  assert.match(rust, /cache_available_bytes/);
  const prepareStart = rust.indexOf("fn prepare_and_encode(");
  const prepareEnd = rust.indexOf("fn load_prepared_ugoira(", prepareStart);
  const prepare = rust.slice(prepareStart, prepareEnd);
  assert.ok(
    prepare.indexOf("cache_available_bytes") < prepare.indexOf("for (index, frame) in prepared.frames.iter().enumerate()"),
    "space and decoded-memory budgets must be checked before staging the full frame set",
  );
  assert.match(prepare, /validate_all_frame_dimensions\(&frame_paths, cancelled\)/);
  assert.match(rust, /FRAME_PROBE_TIMEOUT/);
  assert.match(rust, /FRAME_VALIDATION_TIMEOUT/);
  assert.match(rust, /MAX_ENCODING_TIMEOUT/);
  assert.match(runtime, /cancelled\.as_deref\(\)/);
  assert.match(rust, /AtomicBool/);
  assert.match(rust, /spawn_blocking/);
  assert.doesNotMatch(rust, /async fn prepare_and_encode/);
  assert.match(rust, /child\.kill\(\)/);
  assert.match(rust, /remove_file/);
  assert.match(rust, /-progress/);
  assert.match(rust, /libvpx-vp9/);
  assert.match(rust, /-f[\s\S]*apng/);
  assert.match(rust, /palettegen/);
});

test("artwork UI exposes format selection, progress, cancellation, and failure reason", async () => {
  const [page, api, backend] = await Promise.all([
    source("src/routes/artworks/[id]/+page.svelte"),
    source("src/lib/pixiv-api.ts"),
    source("src-tauri/src/ugoira_export.rs"),
  ]);
  assert.match(page, /startUgoiraExport/);
  assert.match(page, /cancelUgoiraExportTask/);
  assert.match(page, /getUgoiraExportTask/);
  assert.match(page, /ugoiraExportFormat/);
  assert.match(api, /start_ugoira_export/);
  assert.match(api, /cancel_ugoira_export_task/);
  assert.match(api, /get_ugoira_export_task/);
  assert.match(page, /ugoiraExportSupported = !\/Android\/i\.test\(navigator\.userAgent\)/);
  assert.match(page, /\{#if detail\.illustration\.kind === "ugoira"\}\s*<UgoiraPlayer/);
  assert.match(page, /\{#if detail\.illustration\.kind === "ugoira" && ugoiraExportSupported\}\s*<div class="ugoira-export">/);
  assert.match(backend, /if task_root\.exists\(\) \{\s*fs::remove_dir_all\(&task_root\)\.map_err\(\|_\| "staging_cleanup_failed"\)\?/);
});

test("Android generated exports stay inside the authorized tree", async () => {
  const plugin = await source(
    "src-tauri/gen/android/app/src/main/java/io/github/space2233/pixnya/ExportDirectoryPlugin.kt",
  );
  assert.match(plugin, /findChild\(treeUri, rootDocument, fileName\)/);
  assert.match(plugin, /if \(existing != null\) throw ExportConflictException\(\)/);
  assert.match(plugin, /DocumentsContract\.deleteDocument\(activity\.contentResolver, target\)/);
});
