import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

function createStorage() {
  const values = new Map();
  return {
    get length() { return values.size; },
    getItem(key) { return values.get(key) ?? null; },
    setItem(key, value) { values.set(key, String(value)); },
    removeItem(key) { values.delete(key); },
    key(index) { return [...values.keys()][index] ?? null; },
    clear() { values.clear(); },
  };
}

globalThis.localStorage = createStorage();
globalThis.window = { localStorage: globalThis.localStorage };

const history = await import("../src/lib/search-history.ts");

test("search history supports larger reviewed limits and an explicit unlimited mode", () => {
  localStorage.clear();
  assert.deepEqual(history.SEARCH_HISTORY_LIMIT_OPTIONS, [8, 20, 50, 100, null]);
  assert.equal(history.readSearchHistoryLimit(), 8);
  assert.equal(history.searchHistoryLimitOrDefault(undefined), 8);
  assert.equal(history.searchHistoryLimitOrDefault(null), null);

  for (let index = 0; index < 30; index += 1) history.recordSearchHistory(`default-${index}`);
  assert.equal(history.readSearchHistory().length, 8);

  history.writeSearchHistoryLimit(20);
  for (let index = 0; index < 30; index += 1) history.recordSearchHistory(`larger-${index}`);
  assert.equal(history.readSearchHistory().length, 20);

  history.writeSearchHistoryLimit(null);
  for (let index = 0; index < 150; index += 1) history.recordSearchHistory(`unlimited-${index}`);
  assert.equal(history.readSearchHistory().length, 170);

  const trimmed = history.writeSearchHistoryLimit(50);
  assert.equal(trimmed.length, 50);
  assert.deepEqual(history.readSearchHistory(), trimmed);

  localStorage.setItem("pixiv-client.search-history-limit.v1", "invalid");
  assert.equal(history.readSearchHistoryLimit(), 8);
});

test("search history keeps MRU order and refuses values rejected by the API", () => {
  localStorage.clear();
  history.writeSearchHistoryLimit(null);
  history.recordSearchHistory("first");
  history.recordSearchHistory("second");
  history.recordSearchHistory("first");
  assert.deepEqual(history.readSearchHistory(), ["first", "second"]);

  history.recordSearchHistory("x".repeat(101));
  history.recordSearchHistory("bad\u0000value");
  assert.deepEqual(history.readSearchHistory(), ["first", "second"]);
});

test("privacy settings, backup, and every search entry point share the history preference", async () => {
  const [privacy, backup, rustBackup, appShell, search] = await Promise.all([
    readFile(new URL("../src/routes/settings/privacy/+page.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/local-backup.ts", import.meta.url), "utf8"),
    readFile(new URL("../crates/local-backup/src/lib.rs", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/components/AppShell.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/routes/search/+page.svelte", import.meta.url), "utf8"),
  ]);
  assert.match(privacy, /readSearchHistoryLimit/);
  assert.match(privacy, /writeSearchHistoryLimit/);
  assert.match(privacy, /settings_search_history_limit/);
  assert.match(privacy, /value="unlimited"/);
  assert.match(backup, /searchHistoryLimit/);
  assert.match(rustBackup, /search_history_limit:\s*Option<Option<u32>>/);
  assert.match(appShell, /recordSearchHistory\(query\)/);
  assert.match(search, /SEARCH_HISTORY_CHANGED_EVENT/);
  assert.match(search, /visibleHistory/);
  assert.match(search, /search_history_show_more/);
  assert.match(search, /historyVisibleCount:\s*number/);
  assert.match(search, /historyVisibleCount,\s*\n/);
});

test("all search inputs use the enlarged cross-platform clear control", async () => {
  const [globalCss, shell, search, offline, browsingHistory] = await Promise.all([
    readFile(new URL("../src/app.css", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/components/AppShell.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/routes/search/+page.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/routes/offline/+page.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/routes/history/+page.svelte", import.meta.url), "utf8"),
  ]);
  for (const surface of [shell, search, offline, browsingHistory]) {
    assert.match(surface, /type="search"/);
  }
  assert.match(globalCss, /input\[type="search"\]::-webkit-search-cancel-button/);
  assert.match(globalCss, /@media \(pointer: coarse\)[\s\S]*44px/);
});
