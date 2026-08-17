import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function source(path) {
  return readFile(new URL(path, root), "utf8");
}

test("a restored search result follows a bookmark changed in artwork detail", async () => {
  const {
    clearIllustrationBookmarkState,
    publishIllustrationBookmarkState,
    resolveIllustrationBookmarkState,
  } = await import("../src/lib/illustration-bookmark-state.ts");
  const account = "user-42";
  const staleSearchResult = { id: "illust-7", isBookmarked: false };

  clearIllustrationBookmarkState(account);
  assert.equal(
    resolveIllustrationBookmarkState(
      account,
      staleSearchResult.id,
      staleSearchResult.isBookmarked,
    ),
    false,
  );

  publishIllustrationBookmarkState(account, staleSearchResult.id, true);

  assert.equal(
    resolveIllustrationBookmarkState(
      account,
      staleSearchResult.id,
      staleSearchResult.isBookmarked,
    ),
    true,
  );
  assert.equal(
    resolveIllustrationBookmarkState(
      "another-user",
      staleSearchResult.id,
      staleSearchResult.isBookmarked,
    ),
    false,
  );
  clearIllustrationBookmarkState(account);
});

test("artwork cards and detail publish through the same bookmark state boundary", async () => {
  const card = await source("src/lib/components/ArtworkCard.svelte");
  const detail = await source("src/routes/artworks/[id]/+page.svelte");

  assert.match(card, /resolveIllustrationBookmarkState/);
  assert.match(card, /subscribeIllustrationBookmarkState/);
  assert.match(card, /publishIllustrationBookmarkState/);
  assert.match(detail, /resolveIllustrationBookmarkState/);
  assert.match(detail, /subscribeIllustrationBookmarkState/);
  assert.match(detail, /publishIllustrationBookmarkState/);
});

test("a restored novel list follows bookmark changes from cards, detail, and batch removal", async () => {
  const {
    clearNovelBookmarkState,
    publishNovelBookmarkState,
    resolveNovelBookmarkState,
  } = await import("../src/lib/novel-bookmark-state.ts");
  const account = "reader-42";
  clearNovelBookmarkState(account);
  assert.equal(resolveNovelBookmarkState(account, "novel-7", false), false);
  publishNovelBookmarkState(account, "novel-7", true);
  assert.equal(resolveNovelBookmarkState(account, "novel-7", false), true);
  assert.equal(resolveNovelBookmarkState("another-reader", "novel-7", false), false);

  const [card, detail, list] = await Promise.all([
    source("src/lib/components/NovelCard.svelte"),
    source("src/routes/novels/[id]/+page.svelte"),
    source("src/routes/novels/+page.svelte"),
  ]);
  for (const consumer of [card, detail]) {
    assert.match(consumer, /resolveNovelBookmarkState/);
    assert.match(consumer, /subscribeNovelBookmarkState/);
    assert.match(consumer, /publishNovelBookmarkState/);
  }
  assert.match(list, /publishNovelBookmarkState\(account, resourceId, false\)/);
  clearNovelBookmarkState(account);
});
