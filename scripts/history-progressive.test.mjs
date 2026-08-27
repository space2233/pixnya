import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const progressive = await import("../src/lib/history-progressive.ts");

test("browsing history mounts reviewed 48-entry windows without duplicates", () => {
  const entries = Array.from({ length: 120 }, (_, index) => `entry-${index}`);
  const first = progressive.progressiveHistoryWindow(entries, 48);
  assert.equal(progressive.HISTORY_BATCH_SIZE, 48);
  assert.equal(first.visible.length, 48);
  assert.equal(first.visible[0], "entry-0");
  assert.equal(first.visible.at(-1), "entry-47");
  assert.equal(first.nextCount, 96);
  assert.equal(first.hasMore, true);

  const second = progressive.progressiveHistoryWindow(entries, first.nextCount);
  assert.equal(second.visible.length, 96);
  assert.equal(new Set(second.visible).size, 96);
  assert.equal(second.nextCount, 120);

  const final = progressive.progressiveHistoryWindow(entries, second.nextCount);
  assert.equal(final.visible.length, 120);
  assert.equal(final.hasMore, false);
});

test("invalid or stale visible counts safely return to the first batch", () => {
  const entries = Array.from({ length: 80 }, (_, index) => index);
  assert.equal(progressive.progressiveHistoryWindow(entries, 0).visible.length, 48);
  assert.equal(progressive.progressiveHistoryWindow(entries, Number.NaN).visible.length, 48);
  assert.equal(progressive.progressiveHistoryWindow(entries.slice(0, 12), 96).visible.length, 12);
});

test("history page wires progressive windows to observer, button, resets, and snapshots", async () => {
  const page = await readFile(
    new URL("../src/routes/history/+page.svelte", import.meta.url),
    "utf8",
  );
  assert.match(page, /progressiveHistoryWindow/);
  assert.match(page, /visibleEntries/);
  assert.match(page, /use:observeLoadMore/);
  assert.match(page, /class="history-more"/);
  assert.match(page, /visibleCount = HISTORY_BATCH_SIZE/);
  assert.match(page, /rootMargin:\s*"0px"/);
  assert.match(page, /rememberNavigationView/);
  assert.match(page, /recallNavigationView/);
  assert.match(page, /visibleCount,/);
  assert.doesNotMatch(page, /\{#each filteredEntries as entry/);
});

test("history defers its cold load until SvelteKit has restored a navigation snapshot", async () => {
  const [page, client] = await Promise.all([
    readFile(new URL("../src/routes/history/+page.svelte", import.meta.url), "utf8"),
    readFile(
      new URL("../node_modules/@sveltejs/kit/src/runtime/client/client.js", import.meta.url),
      "utf8",
    ),
  ]);
  const navigationCommit = client.indexOf("await commit_promise");
  const afterNavigate = client.indexOf("after_navigate_callbacks.forEach", navigationCommit);
  const restoreSnapshot = client.indexOf(
    "restore_snapshot(current_navigation_index)",
    afterNavigate,
  );
  assert.ok(navigationCommit >= 0 && afterNavigate > navigationCommit);
  assert.ok(restoreSnapshot > afterNavigate);
  assert.match(
    page,
    /onMount\(\(\) => \{[\s\S]*requestAnimationFrame\([\s\S]*if \(!viewRestored\) void loadHistory\(\)/,
  );
  assert.match(page, /cancelAnimationFrame/);
});
