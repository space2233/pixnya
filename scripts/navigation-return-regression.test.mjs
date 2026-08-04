import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  createReturnNavigator,
  isReturnDestination,
} from "../src/lib/return-navigation.ts";
import {
  recallNavigationView,
  rememberNavigationView,
} from "../src/lib/navigation-view-memory.ts";

const root = process.cwd();

function createHarness(initialUrl = "/search?q=%E7%8C%AB", initialIndex = 10) {
  let current = {
    url: initialUrl,
    navigationIndex: initialIndex,
    scrollX: 0,
    scrollY: 840,
  };
  let stack = [];
  let pending = null;
  const calls = { back: 0, fallback: [], scroll: [] };

  const navigator = createReturnNavigator({
    current: () => current,
    readStack: () => stack,
    writeStack: (value) => (stack = structuredClone(value)),
    readPending: () => pending,
    writePending: (value) => (pending = value ? structuredClone(value) : null),
    historyBack: () => (calls.back += 1),
    replaceWithFallback: (url) => calls.fallback.push(url),
    restoreScroll: (x, y) => calls.scroll.push([x, y]),
    now: () => 1_800_000_000_000,
  });

  return {
    navigator,
    calls,
    setCurrent(url, navigationIndex, scrollY = 0) {
      current = { url, navigationIndex, scrollX: 0, scrollY };
    },
    stack: () => stack,
    pending: () => pending,
  };
}

test("detail routes are recognized without treating ordinary lists as details", () => {
  assert.equal(isReturnDestination("/artworks/42"), true);
  assert.equal(isReturnDestination("/novels/7/read"), true);
  assert.equal(isReturnDestination("/users/12"), true);
  assert.equal(isReturnDestination("/series/artworks/9"), true);
  assert.equal(isReturnDestination("/offline/ugoira/3"), true);
  assert.equal(isReturnDestination("/comments/illustration/42/701"), true);
  assert.equal(isReturnDestination("/comments/novel/7/702?compose=1"), true);
  assert.equal(isReturnDestination("/artworks"), false);
  assert.equal(isReturnDestination("/search?q=cat"), false);
});

test("returning from a detail uses browser history and restores the source scroll", () => {
  const harness = createHarness();

  assert.equal(harness.navigator.capture("/artworks/42"), true);
  harness.setCurrent("/artworks/42", 11);
  assert.equal(harness.navigator.returnToPrevious("/artworks"), "history");
  assert.equal(harness.calls.back, 1);
  assert.deepEqual(harness.calls.fallback, []);

  harness.setCurrent("/search?q=%E7%8C%AB", 10);
  assert.equal(harness.navigator.restorePendingPosition(), true);
  assert.deepEqual(harness.calls.scroll, [[0, 840]]);
  assert.equal(harness.pending(), null);
});

test("nested details unwind one source at a time", () => {
  const harness = createHarness();
  harness.navigator.capture("/artworks/42");

  harness.setCurrent("/artworks/42", 11, 320);
  harness.navigator.capture("/users/8");
  harness.setCurrent("/users/8", 12);

  assert.equal(harness.navigator.returnToPrevious("/"), "history");
  harness.setCurrent("/artworks/42", 11);
  assert.equal(harness.navigator.restorePendingPosition(), true);
  assert.deepEqual(harness.calls.scroll.at(-1), [0, 320]);

  assert.equal(harness.navigator.returnToPrevious("/artworks"), "history");
  assert.equal(harness.calls.back, 2);
  assert.equal(harness.stack().length, 0);
});

test("directly opened details use a replacement fallback instead of a stale entry", () => {
  const harness = createHarness("/artworks/42", 50);

  assert.equal(harness.navigator.returnToPrevious("/artworks"), "fallback");
  assert.deepEqual(harness.calls.fallback, ["/artworks"]);
  assert.equal(harness.calls.back, 0);
});

test("a platform back gesture consumes the same entry and restores its source", () => {
  const harness = createHarness("/bookmarks", 20);
  harness.navigator.capture("/artworks/99");
  harness.setCurrent("/bookmarks", 20);

  assert.equal(harness.navigator.restoreAfterHistoryPop("/artworks/99"), true);
  assert.deepEqual(harness.calls.scroll, [[0, 840]]);
  assert.equal(harness.stack().length, 0);
});

test("external links and non-detail destinations are never captured", () => {
  const harness = createHarness();

  assert.equal(harness.navigator.capture("https://example.com/artworks/42"), false);
  assert.equal(harness.navigator.capture("/search?q=dog"), false);
  assert.equal(harness.stack().length, 0);
});

test("all detail screens delegate return behavior to the shared component", async () => {
  const detailPages = [
    "src/routes/artworks/[id]/+page.svelte",
    "src/routes/novels/[id]/+page.svelte",
    "src/routes/novels/[id]/read/+page.svelte",
    "src/routes/users/[id]/+page.svelte",
    "src/routes/series/artworks/[id]/+page.svelte",
    "src/routes/series/novels/[id]/+page.svelte",
    "src/routes/offline/artworks/[id]/+page.svelte",
    "src/routes/offline/novels/[id]/+page.svelte",
    "src/routes/offline/ugoira/[id]/+page.svelte",
  ];

  for (const relativePath of detailPages) {
    const source = await readFile(path.join(root, relativePath), "utf8");
    assert.match(source, /import ReturnLink from "\$lib\/components\/ReturnLink\.svelte"/);
    assert.match(source, /<ReturnLink\s+fallback=/);
    assert.doesNotMatch(source, /<a[^>]*>\s*‹\s*返回/);
  }
});

test("the root layout captures links and handles both button and platform returns", async () => {
  const layout = await readFile(path.join(root, "src/routes/+layout.svelte"), "utf8");
  assert.match(layout, /captureReturnNavigation/);
  assert.match(layout, /restorePendingReturnPosition/);
  assert.match(layout, /restoreReturnAfterHistoryPop/);
  assert.match(layout, /navigation\.type === "popstate"/);
});

test("navigation view memory is bounded and never restores an unknown disk-only key", () => {
  const first = rememberNavigationView({ page: 0 });
  let latest = first;
  for (let page = 1; page <= 70; page += 1) {
    latest = rememberNavigationView({ page });
  }
  assert.equal(recallNavigationView(first), null);
  assert.deepEqual(recallNavigationView(latest), { page: 70 });
  assert.equal(recallNavigationView("missing-after-reload"), null);
});

test("primary content sources preserve loaded state in SvelteKit history snapshots", async () => {
  const sourcePages = [
    "src/routes/+page.svelte",
    "src/routes/artworks/+page.svelte",
    "src/routes/bookmarks/+page.svelte",
    "src/routes/discover/+page.svelte",
    "src/routes/following/+page.svelte",
    "src/routes/manga/+page.svelte",
    "src/routes/ranking/+page.svelte",
    "src/routes/search/+page.svelte",
    "src/routes/novels/+page.svelte",
    "src/routes/following/users/+page.svelte",
    "src/routes/offline/+page.svelte",
    "src/routes/artworks/[id]/+page.svelte",
    "src/routes/novels/[id]/+page.svelte",
    "src/routes/novels/[id]/read/+page.svelte",
    "src/routes/users/[id]/+page.svelte",
    "src/routes/series/artworks/[id]/+page.svelte",
    "src/routes/series/novels/[id]/+page.svelte",
  ];
  for (const relativePath of sourcePages) {
    const source = await readFile(path.join(root, relativePath), "utf8");
    assert.match(source, /export const snapshot/);
    assert.match(source, /rememberNavigationView/);
    assert.match(source, /recallNavigationView/);
  }
});
