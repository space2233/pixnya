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
  assert.match(list, /publishNovelBookmarkState\(expectedUserId, resourceId, false\)/);
  clearNovelBookmarkState(account);
});

test("async bookmark completions remain bound to the account that started the mutation", async () => {
  const illustrationState = await import("../src/lib/illustration-bookmark-state.ts");
  const novelState = await import("../src/lib/novel-bookmark-state.ts");
  const initiatingAccount = "account-a";
  const currentAccountAfterAwait = "account-b";

  illustrationState.clearIllustrationBookmarkState();
  novelState.clearNovelBookmarkState();
  illustrationState.publishIllustrationBookmarkState(initiatingAccount, "illust-7", false);
  novelState.publishNovelBookmarkState(initiatingAccount, "novel-7", false);

  assert.equal(
    illustrationState.resolveIllustrationBookmarkState(currentAccountAfterAwait, "illust-7", true),
    true,
  );
  assert.equal(
    novelState.resolveNovelBookmarkState(currentAccountAfterAwait, "novel-7", true),
    true,
  );

  const [novelCard, novelDetail, artworkCard, artworkDetail, novelList, artworkList] = await Promise.all([
    source("src/lib/components/NovelCard.svelte"),
    source("src/routes/novels/[id]/+page.svelte"),
    source("src/lib/components/ArtworkCard.svelte"),
    source("src/routes/artworks/[id]/+page.svelte"),
    source("src/routes/novels/+page.svelte"),
    source("src/lib/components/BrowsePage.svelte"),
  ]);
  assert.match(
    novelCard,
    /const account = bookmarkAccount;[\s\S]*?const novelId = novel\.id;[\s\S]*?await setNovelBookmark\(novelId, next\)[\s\S]*?if \(bookmarkAccount !== account\) return;[\s\S]*?publishNovelBookmarkState\(account, novelId, next\)/,
  );
  assert.match(
    novelDetail,
    /const account = bookmarkAccount;[\s\S]*?const novelId = targetDetail\.novel\.id;[\s\S]*?await setNovelBookmark\(novelId, next, bookmarkRestrict\)[\s\S]*?if \(bookmarkAccount !== account\) return;[\s\S]*?publishNovelBookmarkState\(account, novelId, next\)/,
  );
  assert.match(
    artworkCard,
    /const account = bookmarkAccount;[\s\S]*?const illustrationId = illustration\.id;[\s\S]*?await setIllustrationBookmark\(illustrationId, next\)[\s\S]*?if \(bookmarkAccount !== account\) return;[\s\S]*?publishIllustrationBookmarkState\(account, illustrationId, next\)/,
  );
  assert.match(
    artworkDetail,
    /const targetDetail = detail;[\s\S]*?const illustrationId = targetDetail\.illustration\.id;[\s\S]*?await setIllustrationBookmark\(illustrationId, next, bookmarkRestrict\)[\s\S]*?if \(bookmarkAccount !== account\) return;[\s\S]*?publishIllustrationBookmarkState\(account, illustrationId, next\)/,
  );
  assert.match(
    novelList,
    /const expectedKey = requestedSession;[\s\S]*?const expectedTag = batchTag\.trim\(\);[\s\S]*?batchUpdateBookmarks\(updates, expectedUserId\)[\s\S]*?if \(\$session\.user\?\.id !== expectedUserId\) return;[\s\S]*?publishNovelBookmarkState\(expectedUserId, resourceId, false\)[\s\S]*?if \(requestedSession !== expectedKey\) return;/,
  );
  assert.match(
    artworkList,
    /const expectedKey = requestedKey;[\s\S]*?batchUpdateBookmarks\([\s\S]*?expectedUserId\)[\s\S]*?if \(\$session\.user\?\.id !== expectedUserId\) return;[\s\S]*?publishIllustrationBookmarkState\(expectedUserId, resourceId, false\)[\s\S]*?if \(requestedKey !== expectedKey\) return;/,
  );
});

test("leaving an account discards bookmark overlays before that account is restored", async () => {
  const lifecycle = await import("../src/lib/bookmark-session-transition.ts");
  const illustrationState = await import("../src/lib/illustration-bookmark-state.ts");
  const novelState = await import("../src/lib/novel-bookmark-state.ts");
  const sessionSource = await source("src/lib/session.ts");
  const user = { id: "account-a", name: "A", account: "a", isPremium: false };
  const authenticated = { loggedIn: true, user, connectionMode: "standard" };
  const loggedOut = { loggedIn: false };

  illustrationState.clearIllustrationBookmarkState();
  novelState.clearNovelBookmarkState();
  illustrationState.publishIllustrationBookmarkState(user.id, "illust-7", true);
  novelState.publishNovelBookmarkState(user.id, "novel-7", true);

  lifecycle.clearBookmarkOverlaysForSessionTransition(authenticated, loggedOut);
  novelState.publishNovelBookmarkState(user.id, "novel-7", true);
  lifecycle.clearBookmarkOverlaysForSessionTransition(loggedOut, authenticated);

  assert.equal(
    illustrationState.resolveIllustrationBookmarkState(user.id, "illust-7", false),
    false,
  );
  assert.equal(novelState.resolveNovelBookmarkState(user.id, "novel-7", false), false);
  assert.match(
    sessionSource,
    /clearBookmarkOverlaysForSessionTransition\(currentSnapshot, snapshot\)/,
  );
});
