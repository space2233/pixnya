import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const read = (path) => readFile(new URL(path, root), "utf8");

test("advanced local catalog exposes transactional batch, saved filter, and duplicate commands", async () => {
  const [backend, catalog, api] = await Promise.all([
    read("src-tauri/src/catalog.rs"),
    read("crates/local-catalog/src/lib.rs"),
    read("src/lib/pixiv-api.ts"),
  ]);
  for (const command of [
    "batch_organize_offline_entries",
    "batch_remove_offline_entries",
    "save_local_catalog_filter",
    "delete_local_catalog_filter",
    "find_offline_duplicates",
  ]) {
    assert.match(backend, new RegExp(`fn ${command}\\b`));
    assert.match(api, new RegExp(`\\"${command}\\"`));
  }
  assert.match(catalog, /transaction_with_behavior\(TransactionBehavior::Immediate\)/);
  assert.match(catalog, /catalog_saved_filters/);
  assert.match(backend, /DuplicateReason::ResourceId/);
  assert.match(backend, /DuplicateReason::FileHash/);
});

test("offline UI keeps duplicate detection report-only and supports saved compound filters", async () => {
  const page = await read("src/routes/offline/+page.svelte");
  assert.match(page, /findOfflineDuplicates/);
  assert.match(page, /saveLocalCatalogFilter/);
  assert.match(page, /storedAfter/);
  assert.match(page, /minSizeBytes/);
  assert.match(page, /selectedEntryKeys/);
  assert.doesNotMatch(page, /autoDeleteDuplicate|deleteDuplicateAutomatically/);
});

test("batch filesystem deletion commits atomically to deferred quarantine cleanup", async () => {
  const library = await read("crates/library/src/lib.rs");
  const catalog = await read("src-tauri/src/catalog.rs");
  const localCatalog = await read("crates/local-catalog/src/lib.rs");
  assert.match(library, /fs::rename\(restore_quarantine, restore_source\)[\s\S]*map_err\(\|_\| LibraryError::Io\)\?/);
  assert.doesNotMatch(library, /for \(_, _, quarantine\) in &planned[\s\S]*remove_dir_all\(quarantine\)/);
  assert.match(library, /starts_with\("\.batch-remove-"\)[\s\S]*fs::remove_dir_all\(child\.path\(\)\)/);
  assert.match(localCatalog, /pub fn restore_entries\([\s\S]*TransactionBehavior::Immediate[\s\S]*transaction\.commit/);
  assert.match(catalog, /catalog\.restore_entries\(&previous\)/);
  assert.doesNotMatch(catalog, /let _ = catalog\.organize_entry/);
});
