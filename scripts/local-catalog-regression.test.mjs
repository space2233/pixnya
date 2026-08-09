import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("local catalog owns migration, validation, and transactional organization", async () => {
  const source = await read("crates/local-catalog/src/lib.rs");
  assert.match(source, /const SCHEMA_VERSION: u32 = 1/);
  assert.match(source, /TransactionBehavior::Immediate/g);
  assert.match(source, /pragma_update\(None, "journal_mode", "WAL"\)/);
  assert.match(source, /MAX_TAGS_PER_ENTRY: usize = 16/);
  assert.match(source, /matches!\(kind, "artwork" \| "novel" \| "ugoira"\)/);
  assert.match(source, /ON DELETE SET NULL/);
  assert.match(source, /ON DELETE CASCADE/);
  assert.match(source, /visibility_filter_does_not_destroy_temporarily_missing_metadata/);
});

test("Tauri validates real offline entries and serializes catalog commands", async () => {
  const [catalog, runtime] = await Promise.all([
    read("src-tauri/src/catalog.rs"),
    read("src-tauri/src/lib.rs"),
  ]);
  assert.match(catalog, /state\.operation\.lock\(\)\.await/);
  assert.match(catalog, /library_gate[\s\S]*acquire_owned/);
  assert.match(catalog, /library[\s\S]*list_entries\(\)[\s\S]*any\(\|entry\| entry\.key == entry_key\)/);
  for (const command of [
    "get_local_catalog_snapshot",
    "create_local_collection",
    "rename_local_collection",
    "delete_local_collection",
    "organize_offline_entry",
  ]) {
    assert.match(runtime, new RegExp(`\\b${command},`));
  }
  assert.match(runtime, /manage\(CatalogState::default\(\)\)/);
});

test("offline deletion and full local-data clearing include catalog metadata", async () => {
  const runtime = await read("src-tauri/src/lib.rs");
  assert.match(runtime, /library\.remove_entry\(&key\)\?[\s\S]*catalog\.remove_entry\(&key\)\?/);
  assert.match(runtime, /catalog::clear_local_catalog\(&app\)\.await/);
  assert.match(runtime, /LocalDataClearFailure::LocalCatalog/);
  assert.match(runtime, /local_collections_removed/);
  assert.match(runtime, /local_organized_entries_removed/);
  assert.match(runtime, /local_tags_removed/);
});

test("frontend exposes collection CRUD, entry organization, and combined filters", async () => {
  const [api, types, page] = await Promise.all([
    read("src/lib/pixiv-api.ts"),
    read("src/lib/types.ts"),
    read("src/routes/offline/+page.svelte"),
  ]);
  for (const command of [
    "get_local_catalog_snapshot",
    "create_local_collection",
    "rename_local_collection",
    "delete_local_collection",
    "organize_offline_entry",
  ]) {
    assert.match(api, new RegExp(`invoke<[^>]+>\\("${command}"`));
  }
  assert.match(types, /interface LocalCatalogSnapshot/);
  assert.match(page, /const filteredEntries = \$derived\.by/);
  assert.match(page, /kindFilter !== "all"/);
  assert.match(page, /collectionFilter === "unfiled"/);
  assert.match(page, /tagFilter !== "all"/);
  assert.match(page, /sortOrder === "size"/);
  assert.match(page, /m\.offline_tags_placeholder\(\)/);
  assert.doesNotMatch(page, /localStorage|sessionStorage/);
});

test("mobile catalog controls remain readable and use a three-action entry row", async () => {
  const page = await read("src/routes/offline/+page.svelte");
  assert.match(page, /@media \(max-width: 620px\)/);
  assert.match(page, /\.catalog-tools \{ grid-template-columns: 1fr 1fr/);
  assert.match(page, /\.entry-actions \{ min-height: 48px; grid-template-columns: repeat\(3,1fr\)/);
  assert.match(page, /\.organize-editor \{ grid-template-columns: 1fr/);
  assert.match(page, /font-size: 12px/);
});
