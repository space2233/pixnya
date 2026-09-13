import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

const scrollSurfaces = [
  "src/lib/components/BrowsePage.svelte",
  "src/lib/components/ArtworkComments.svelte",
  "src/routes/search/+page.svelte",
  "src/routes/novels/+page.svelte",
  "src/routes/users/[id]/+page.svelte",
  "src/routes/following/users/+page.svelte",
  "src/routes/artworks/[id]/+page.svelte",
  "src/routes/series/novels/[id]/+page.svelte",
  "src/routes/series/artworks/[id]/+page.svelte",
  "src/routes/comments/[kind]/[resourceId]/[commentId]/+page.svelte",
  "src/routes/notifications/+page.svelte",
  "src/routes/history/+page.svelte",
  "src/routes/settings/account-controls/+page.svelte",
];

test("load-more observer latches while intersecting and only refires after it is re-enabled", async () => {
  const {
    LOAD_MORE_ROOT_MARGIN,
    loadMoreObserverEnabled,
    normalizeObserveLoadMoreOptions,
    observeLoadMore,
  } = await import("../src/lib/infinite-scroll.ts");

  assert.equal(LOAD_MORE_ROOT_MARGIN, "240px 0px");
  assert.equal(loadMoreObserverEnabled({ enabled: true }), true);
  assert.equal(loadMoreObserverEnabled({ enabled: true, loading: true }), false);
  assert.equal(loadMoreObserverEnabled({ enabled: true, error: "failed" }), false);
  assert.equal(normalizeObserveLoadMoreOptions(() => {}).enabled, true);
  assert.equal(normalizeObserveLoadMoreOptions({ onLoad: () => {}, enabled: false }).enabled, false);

  const observers = [];
  class MockObserver {
    constructor(callback, options) {
      this.callback = callback;
      this.options = options;
      observers.push(this);
    }
    observe(node) {
      this.node = node;
    }
    disconnect() {
      this.disconnected = true;
    }
    fire(isIntersecting) {
      this.callback([{ isIntersecting, target: this.node }]);
    }
  }

  globalThis.IntersectionObserver = MockObserver;
  const calls = [];
  const node = {};
  const action = observeLoadMore(node, {
    onLoad: () => calls.push("load"),
    enabled: true,
  });

  assert.equal(observers.length, 1);
  assert.equal(observers[0].options.rootMargin, LOAD_MORE_ROOT_MARGIN);
  observers[0].fire(true);
  observers[0].fire(true);
  assert.deepEqual(calls, ["load"]);

  action.update({ onLoad: () => calls.push("load"), enabled: false });
  action.update({ onLoad: () => calls.push("load"), enabled: true });
  observers.at(-1).fire(true);
  assert.deepEqual(calls, ["load", "load"]);
  action.destroy();
});

test("paged lists auto-load from a shared sentinel and keep retry manual after errors", async () => {
  const [component, moduleSource, ...pages] = await Promise.all([
    source("src/lib/components/LoadMoreOnScroll.svelte"),
    source("src/lib/infinite-scroll.ts"),
    ...scrollSurfaces.map(source),
  ]);

  assert.match(moduleSource, /IntersectionObserver/);
  assert.match(moduleSource, /LOAD_MORE_ROOT_MARGIN/);
  assert.match(component, /use:observeLoadMore/);
  assert.match(component, /loadMoreObserverEnabled/);
  assert.match(component, /m\.common_retry\(\)/);
  assert.match(component, /m\.common_loading\(\)/);
  assert.doesNotMatch(component, /m\.common_load_more\(\)/);

  for (const [index, page] of pages.entries()) {
    assert.match(page, /LoadMoreOnScroll/, scrollSurfaces[index]);
    assert.doesNotMatch(page, /m\.common_load_more\(\)/, scrollSurfaces[index]);
  }

  const notifications = pages[scrollSurfaces.indexOf("src/routes/notifications/+page.svelte")];
  assert.match(notifications, /onclick=\{\(\) => expandGroup\(item\)\}/);
  assert.match(notifications, /error=\{paginationError\}/);
});
