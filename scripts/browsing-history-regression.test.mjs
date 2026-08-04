import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("local history owns a bounded, validated, private SQLite store", async () => {
  const source = await read("crates/local-history/src/lib.rs");
  assert.match(source, /const SCHEMA_VERSION: u32 = 1/);
  assert.match(source, /const HISTORY_LIMIT: usize = 500/);
  assert.match(source, /TransactionBehavior::Immediate/);
  assert.match(source, /pragma_update\(None, "journal_mode", "WAL"\)/);
  assert.match(source, /PRIMARY KEY \(kind, resource_id\)/);
  assert.match(source, /ORDER BY view_order DESC LIMIT \?1/);
  assert.match(source, /LIMIT -1 OFFSET \?1/);
  assert.match(source, /https:\/\/i\.pximg\.net\//);
  assert.match(source, /https:\/\/s\.pximg\.net\//);
  assert.match(source, /disabling_history_preserves_existing_rows_and_rejects_new_records/);
  assert.match(source, /enforces_the_bounded_most_recent_history_limit/);
});

test("Tauri serializes history commands and includes history in full local-data clearing", async () => {
  const [history, runtime, paths] = await Promise.all([
    read("src-tauri/src/history.rs"),
    read("src-tauri/src/lib.rs"),
    read("src-tauri/src/paths.rs"),
  ]);
  assert.match(history, /browsing-history-v1\.sqlite3/);
  assert.match(history, /state\.operation\.lock\(\)\.await/g);
  for (const command of [
    "get_browsing_history",
    "set_browsing_history_enabled",
    "record_browsing_history",
    "remove_browsing_history_entry",
    "clear_browsing_history",
  ]) {
    assert.match(runtime, new RegExp(`\\b${command},`));
  }
  assert.match(runtime, /manage\(HistoryState::default\(\)\)/);
  assert.match(runtime, /history::clear_all_history\(&app\)\.await/);
  assert.match(runtime, /LocalDataClearFailure::BrowsingHistory/);
  assert.match(runtime, /browsing_history_entries_removed/);
  assert.match(paths, /#\[cfg\(debug_assertions\)\][\s\S]*PIXIV_CLIENT_TEST_ROOT/);
});

test("artwork, novel, and user details record history without coupling content success to storage", async () => {
  const pages = await Promise.all([
    read("src/routes/artworks/[id]/+page.svelte"),
    read("src/routes/novels/[id]/+page.svelte"),
    read("src/routes/users/[id]/+page.svelte"),
  ]);
  for (const [index, page] of pages.entries()) {
    const expectedKind = ["artwork", "novel", "user"][index];
    assert.match(page, /recordBrowsingHistory\(\{/);
    assert.match(page, new RegExp(`kind: "${expectedKind}"`));
    assert.match(page, /\}\)\.catch\(\(\) => undefined\)/);
  }
});

test("history page, shared navigation, and settings operate on the same backend state", async () => {
  const [page, navigation, settings, api, types] = await Promise.all([
    read("src/routes/history/+page.svelte"),
    read("src/lib/navigation.ts"),
    read("src/routes/settings/+page.svelte"),
    read("src/lib/pixiv-api.ts"),
    read("src/lib/types.ts"),
  ]);
  assert.match(page, /getBrowsingHistory\(\)/);
  assert.match(page, /setBrowsingHistoryEnabled\(!snapshot\.enabled\)/);
  assert.match(page, /removeBrowsingHistoryEntry\(entry\.kind, entry\.resourceId\)/);
  assert.match(page, /clearBrowsingHistory\(\)/);
  assert.match(page, /最多保留 \{snapshot\?\.limit \?\? 500\} 条/);
  assert.match(navigation, /href: "\/history"/);
  assert.match(settings, /setBrowsingHistoryEnabled\(!browsingHistory\.enabled\)/);
  assert.match(settings, /report\.browsingHistoryEntriesRemoved/);
  assert.match(settings, /href="\/history"/);
  assert.match(api, /invoke<HistorySnapshot>\("get_browsing_history"\)/);
  assert.match(types, /export type HistoryKind = "artwork" \| "novel" \| "user"/);
  assert.doesNotMatch(page, /localStorage|sessionStorage/);
});

test("mobile history controls remain readable and collapse to a single-column toolbar", async () => {
  const page = await read("src/routes/history/+page.svelte");
  assert.match(page, /@media \(max-width: 680px\)/);
  assert.match(page, /\.toolbar \{ grid-template-columns: 1fr; \}/);
  assert.match(page, /\.kind-filters \{ display: grid; grid-template-columns: repeat\(4, 1fr\); \}/);
  assert.match(page, /padding: 18px 10px 90px/);
});
