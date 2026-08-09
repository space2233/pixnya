import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function source(path) {
  return readFile(new URL(path, root), "utf8");
}

test("Rust API exposes discovery, ranking, follow, bookmark, and search endpoints", async () => {
  const api = await source("crates/api/src/lib.rs");
  for (const endpoint of [
    "/v1/illust/ranking",
    "/v1/trending-tags/illust",
    "/v1/search/illust",
    "/v1/search/user",
    "/v2/illust/follow",
    "/v1/user/bookmarks/illust",
  ]) {
    assert.match(api, new RegExp(endpoint.replaceAll("/", "\\/")));
  }
  assert.match(api, /normalized_search_word/);
  assert.match(api, /expected_bindings/);
  assert.match(api, /fn cursor_is_opaque_and_locked_to_endpoint_and_resource/);
});

test("Tauri registers discovery commands and derives bookmark owner from the Rust session", async () => {
  const backend = await source("src-tauri/src/lib.rs");
  for (const command of [
    "get_ranking_illustrations",
    "get_trending_tags",
    "search_illustrations",
    "search_users",
    "get_followed_illustrations",
    "get_bookmarked_illustrations",
  ]) {
    assert.match(backend, new RegExp(`async fn ${command}\\(`));
    assert.match(backend, new RegExp(`\\n\\s+${command},`));
  }
  assert.match(
    backend,
    /api\.bookmarked_illustrations\(token, user_id, &restrict, cursor\.as_deref\(\), signature\)/,
  );
});

test("search and browse pages use live APIs and the shared image pipeline", async () => {
  const search = await source("src/routes/search/+page.svelte");
  const browse = await source("src/lib/components/BrowsePage.svelte");
  const preview = await source("src/lib/components/UserPreviewCard.svelte");

  for (const symbol of ["getTrendingTags", "searchIllustrations", "searchUsers"]) {
    assert.match(search, new RegExp(symbol));
  }
  for (const symbol of [
    "getRecommendedIllustrations",
    "getRankingIllustrations",
    "getFollowedIllustrations",
    "getBookmarkedIllustrations",
  ]) {
    assert.match(browse, new RegExp(symbol));
  }
  assert.doesNotMatch(search, /<img\b/);
  assert.doesNotMatch(browse, /<img\b/);
  assert.doesNotMatch(preview, /<img\b/);
  assert.match(search, /<ArtworkThumbnail/);
  assert.match(preview, /<PixivImage/);
});

test("bookmark privacy and ranking filters map to the exact API values", async () => {
  const browse = await source("src/lib/components/BrowsePage.svelte");
  assert.match(browse, /selectedFilter === "private" \? "private" : "public"/);
  assert.match(browse, /selectedFilter === "week" \? "week"/);
  assert.match(browse, /selectedFilter === "month" \? "month" : "day"/);
  assert.doesNotMatch(browse, /filters: \["public", "private", "tags"\]/);
  assert.doesNotMatch(browse, /filters: \["for_you", "trending_tags", "new_creators"\]/);
});

test("recent searches sit directly below the page search field", async () => {
  const search = await source("src/routes/search/+page.svelte");
  const field = search.indexOf('<form class="large-search"');
  const history = search.indexOf('<section class="history-card"');
  const tabs = search.indexOf('<nav class="type-tabs"');
  assert.ok(field >= 0 && history > field && tabs > history);
});

test("logged-in home relies on feed tabs instead of repeating recommendation copy", async () => {
  const browse = await source("src/lib/components/BrowsePage.svelte");
  assert.match(browse, /class:home-feed-only=\{section === "home" && \(\$sessionRestoring \|\| \$session\.loggedIn\)\}/);
  assert.match(browse, /\{#if section !== "home" \|\| \(!\$sessionRestoring && !\$session\.loggedIn\)\}/);
  assert.match(browse, /<nav class="filter-tabs"/);
});

test("detail tag navigation records the shared recent-search history", async () => {
  const [history, search, artwork, novel] = await Promise.all([
    source("src/lib/search-history.ts"),
    source("src/routes/search/+page.svelte"),
    source("src/routes/artworks/[id]/+page.svelte"),
    source("src/routes/novels/[id]/+page.svelte"),
  ]);
  assert.match(history, /export function recordSearchHistory/);
  assert.match(history, /pixiv-client\.search-history\.v1/);
  assert.match(search, /history = recordSearchHistory\(value\)/);
  assert.match(artwork, /onclick=\{\(\) => recordSearchHistory\(tag\.name\)\}/);
  assert.match(novel, /onclick=\{\(\) => recordSearchHistory\(tag\)\}/);
});
