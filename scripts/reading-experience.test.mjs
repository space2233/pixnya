import assert from "node:assert/strict";
import test from "node:test";
import { loadNovelForReading, readingChapterHref } from "../src/lib/novel-reading-source.ts";
import * as shelf from "../src/lib/reading-shelf.ts";
import { collectFrontendBackupState, restoreFrontendBackupState } from "../src/lib/local-backup.ts";
import { readNovelReaderPreferences, writeNovelReaderPreferences, writeVolumePageTurnEnabled, readVolumePageTurnEnabled } from "../src/lib/preferences.ts";

const values = new Map();
let failKey = null;
globalThis.localStorage = {
  get length() { return values.size; },
  key(index) { return [...values.keys()][index] ?? null; },
  getItem(key) { return values.get(key) ?? null; },
  setItem(key, value) { if (key === failKey) { failKey = null; throw new Error("disk_full"); } values.set(key, String(value)); },
  removeItem(key) { values.delete(key); },
};
globalThis.window = { localStorage, dispatchEvent() {} };
globalThis.document = { documentElement: { dataset: {} } };
const novel = { id: "42", title: "A novel", author: { name: "Author" }, xRestrict: 0, series: { id: "7", title: "Series" } };
const detail = { novel };
const content = { novelId: "42", text: "First paragraph", seriesNavigation: {} };

test("downloaded novels load without a session or any network request", async () => {
  const api = {
    readOfflineText: async (key, name) => { assert.equal(key, "novel-42"); return JSON.stringify(name === "detail.json" ? detail : content); },
    getNovelDetail() { assert.fail("must stay local"); }, getNovelContent() { assert.fail("must stay local"); },
  };
  assert.equal((await loadNovelForReading("42", false, false, api)).offline, true);
  assert.equal((await loadNovelForReading("42", true, true, api)).offline, true);
});

test("corrupt or wrong offline content can fall back online only from the online entry", async () => {
  let requests = 0;
  const api = {
    readOfflineText: async () => JSON.stringify({ novelId: "99", novel: { id: "99" } }),
    getNovelDetail: async () => { requests++; return detail; },
    getNovelContent: async () => { requests++; return content; },
  };
  await assert.rejects(loadNovelForReading("42", true, true, api));
  await assert.rejects(loadNovelForReading("42", false, false, api), /reading_login_required/);
  assert.equal(requests, 0);
  assert.equal((await loadNovelForReading("42", false, true, api)).offline, false);
  assert.equal(requests, 2);
  await assert.rejects(loadNovelForReading("../42", false, true, api), /invalid_novel_id/);
});

test("offline chapter navigation never links to an undownloaded novel", () => {
  const downloaded = new Set(["42"]);
  assert.equal(readingChapterHref("42", true, downloaded), "/offline/novels/42");
  assert.equal(readingChapterHref("43", true, downloaded), null);
  assert.equal(readingChapterHref("43", false, downloaded), "/novels/43/read");
  assert.equal(readingChapterHref("42", false, downloaded), "/novels/42/read", "online reading keeps online navigation while the loader prefers local bytes");
  assert.equal(readingChapterHref("../42", false, downloaded), null);
});

test("online and offline readers retain legacy progress and share shelf status", () => {
  values.clear();
  localStorage.setItem("pixiv-client:novel-progress:42", "0.73");
  assert.equal(shelf.readNovelProgress("42"), 0.73);
  shelf.addNovelToShelf(novel);
  assert.equal(shelf.readReadingShelf()[0].status, "planned");
  shelf.addNovelToShelf(novel, "reading");
  shelf.saveNovelProgress("42", 0.8);
  assert.equal(shelf.recentReading(shelf.readReadingShelf())[0].id, "42");
  assert.equal(shelf.lastReadInSeries(shelf.readReadingShelf(), "7").id, "42");
  shelf.saveNovelProgress("42", 1);
  assert.equal(shelf.readReadingShelf()[0].status, "finished");
  assert.deepEqual(shelf.recentReading(shelf.readReadingShelf()), []);
  shelf.removeFromReadingShelf("42");
  assert.equal(shelf.readNovelProgress("42"), 1, "removing a shelf card must not erase position");
});

test("new backups restore reading settings, volume paging, shelf and progress together", () => {
  values.clear();
  writeNovelReaderPreferences({ fontSize: 24, lineHeight: 2.2, theme: "dark" });
  writeVolumePageTurnEnabled(true);
  shelf.addNovelToShelf(novel, "reading");
  shelf.saveNovelProgress("42", 0.73);
  const backup = collectFrontendBackupState();
  values.clear();
  restoreFrontendBackupState(backup);
  assert.deepEqual(readNovelReaderPreferences(), { fontSize: 24, lineHeight: 2.2, theme: "dark" });
  assert.equal(readVolumePageTurnEnabled(), true);
  assert.equal(shelf.readReadingShelf()[0].id, "42");
  assert.equal(shelf.readNovelProgress("42"), 0.73);
});

test("old backups preserve settings that their format did not contain", () => {
  values.clear();
  writeNovelReaderPreferences({ fontSize: 22, lineHeight: 2, theme: "dark" });
  writeVolumePageTurnEnabled(true);
  const backup = collectFrontendBackupState();
  delete backup.readerPreferences; delete backup.volumePageTurn; delete backup.readingShelf;
  restoreFrontendBackupState(backup);
  assert.equal(readNovelReaderPreferences().fontSize, 22);
  assert.equal(readVolumePageTurnEnabled(), true);
});

test("merging keeps the newer local reading session even when it is a re-read at a lower percentage", () => {
  values.clear();
  shelf.addNovelToShelf(novel, "reading");
  shelf.saveNovelProgress("42", 0.2);
  const backup = collectFrontendBackupState();
  backup.readingShelf[0].updatedAt = 1;
  backup.readingShelf[0].lastReadAt = 1;
  backup.novelReadingProgress["42"] = 900000;
  restoreFrontendBackupState(backup, "merge");
  assert.equal(shelf.readNovelProgress("42"), 0.2);
});

test("successive backup merges preserve reading timestamps separately from status edits", () => {
  values.clear();
  shelf.addNovelToShelf(novel, "reading");
  shelf.saveNovelProgress("42", 0.2);
  const original = collectFrontendBackupState();
  original.readingShelf[0].updatedAt = 200;
  original.readingShelf[0].lastReadAt = 200;
  restoreFrontendBackupState(original);

  const statusEdit = structuredClone(original);
  Object.assign(statusEdit.readingShelf[0], { status: "finished", updatedAt: 300, lastReadAt: 100 });
  statusEdit.novelReadingProgress["42"] = 900000;
  restoreFrontendBackupState(statusEdit, "merge");
  assert.equal(shelf.readNovelProgress("42"), 0.2);
  assert.equal(shelf.readReadingShelf()[0].status, "finished");
  assert.equal(shelf.readReadingShelf()[0].updatedAt, 300);
  assert.equal(shelf.readReadingShelf()[0].lastReadAt, 200);

  const olderRead = structuredClone(original);
  Object.assign(olderRead.readingShelf[0], { updatedAt: 150, lastReadAt: 150 });
  olderRead.novelReadingProgress["42"] = 800000;
  restoreFrontendBackupState(olderRead, "merge");
  assert.equal(shelf.readNovelProgress("42"), 0.2);
  assert.equal(shelf.readReadingShelf()[0].lastReadAt, 200);
  assert.equal(shelf.readReadingShelf()[0].status, "finished");
});

test("shelf merge keeps newer reading clocks from either side without mutating inputs", () => {
  const entry = { id: "42", title: "Novel", author: "Author", seriesId: null, seriesTitle: null,
    xRestrict: 0, status: "reading", updatedAt: 200, lastReadAt: 200 };
  const edited = { ...entry, title: "Updated title", status: "finished", updatedAt: 300, lastReadAt: 100 };
  const expected = { ...edited, lastReadAt: 200 };
  assert.deepEqual(shelf.mergeReadingShelves([entry], [edited]), [expected]);
  assert.deepEqual(shelf.mergeReadingShelves([edited], [entry]), [expected]);
  assert.equal(edited.lastReadAt, 100);
  assert.equal(entry.status, "reading");
});

test("backup collection bounds valid progress entries while skipping corrupt local records", () => {
  values.clear();
  values.set("pixiv-client:novel-progress:../bad", "0.4");
  values.set("pixiv-client:novel-progress:0", "0.4");
  values.set("pixiv-client:novel-progress:99999", "NaN");
  for (let id = 1; id <= 10002; id += 1) values.set(`pixiv-client:novel-progress:${id}`, "0.12345678");
  const progress = collectFrontendBackupState().novelReadingProgress;
  assert.equal(Object.keys(progress).length, 10000);
  assert.equal(progress["1"], 123457);
  assert.equal(progress["10000"], 123457);
  assert.equal(progress["10001"], undefined);
  assert.equal(progress["99999"], undefined);
  assert.equal(progress["0"], undefined);
  values.clear();
});

test("invalid settings are rejected before writes; failed restore rolls all reading data back", () => {
  values.clear();
  shelf.addNovelToShelf(novel, "reading");
  const backup = collectFrontendBackupState();
  assert.throws(() => restoreFrontendBackupState({ ...backup, readerPreferences: { fontSize: 99, lineHeightTenths: 19, theme: "paper" } }), /invalid_backup/);
  writeNovelReaderPreferences({ fontSize: 25, theme: "dark" });
  const before = [...values.entries()].sort();
  failKey = "pixiv-client.reader-prefs.v1";
  assert.throws(() => restoreFrontendBackupState(backup), /disk_full/);
  assert.deepEqual([...values.entries()].sort(), before);
});
