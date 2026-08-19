import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
async function source(path) { return readFile(new URL(path, root), "utf8"); }

test("novel discovery, bookmarks, and comments stay behind validated Rust endpoints", async () => {
  const api = await source("crates/api/src/lib.rs");
  for (const endpoint of [
    "/v1/search/novel", "/v1/user/novels", "/v1/novel/follow",
    "/v1/user/bookmarks/novel", "/v1/novel/ranking", "/v2/novel/bookmark/add",
    "/v1/novel/bookmark/delete", "/v3/novel/comments",
    "/v2/novel/comment/replies", "/v1/novel/comment/add",
  ]) assert.match(api, new RegExp(endpoint.replaceAll("/", "\\/")));
  for (const method of [
    "search_novels", "user_novels", "followed_novels", "bookmarked_novels",
    "ranking_novels", "add_novel_bookmark", "delete_novel_bookmark",
    "novel_comments", "novel_comment_replies", "add_novel_comment",
  ]) assert.match(api, new RegExp(`pub fn ${method}\\(`));
  assert.match(api, /fn novel_comment_cursor_is_locked_to_the_novel/);
});

test("Tauri registers novel reads and serializes novel writes", async () => {
  const backend = await source("src-tauri/src/lib.rs");
  for (const command of [
    "search_novels", "get_user_novels", "get_followed_novels",
    "get_bookmarked_novels", "get_ranking_novels", "set_novel_bookmark",
    "get_novel_comments", "get_novel_comment_replies", "add_novel_comment",
  ]) {
    assert.match(backend, new RegExp(`async fn ${command}\\(`));
    assert.match(backend, new RegExp(`\\n\\s+${command},`));
  }
  assert.match(backend, /async fn execute_authenticated_mutation/);
  assert.doesNotMatch(backend, /access_token:\s*String/);
});

test("novel UI exposes real search, account lists, bookmarks, and comments", async () => {
  const search = await source("src/routes/search/+page.svelte");
  const user = await source("src/routes/users/[id]/+page.svelte");
  const novels = await source("src/routes/novels/+page.svelte");
  const reader = await source("src/routes/novels/[id]/+page.svelte");
  const card = await source("src/lib/components/NovelCard.svelte");
  const comments = await source("src/lib/components/ArtworkComments.svelte");
  const replies = await source("src/routes/comments/[kind]/[resourceId]/[commentId]/+page.svelte");
  assert.match(search, /searchNovels/);
  assert.match(user, /getUserNovels/);
  for (const symbol of ["getFollowedNovels", "getBookmarkedNovels", "getRankingNovels"]) assert.match(novels, new RegExp(symbol));
  assert.match(reader, /setNovelBookmark/);
  assert.match(reader, /<ArtworkComments novelId=/);
  assert.match(card, /setNovelBookmark/);
  for (const symbol of ["getNovelComments", "addNovelComment"]) assert.match(comments, new RegExp(symbol));
  for (const symbol of ["getNovelCommentReplies", "addNovelComment"]) assert.match(replies, new RegExp(symbol));
});

test("novel pagination discards a page requested for a stale list key", async () => {
  const novels = await source("src/routes/novels/+page.svelte");
  const loadMore = novels.slice(
    novels.indexOf("async function loadMore()"),
    novels.indexOf("</script>"),
  );
  assert.match(loadMore, /const key = requestedSession/);
  assert.match(loadMore, /const sequence = \+\+requestSequence/);
  assert.match(
    loadMore,
    /await requestPage\(cursor\)[\s\S]*?if \(sequence !== requestSequence \|\| key !== requestedSession\) return;[\s\S]*?novels =/,
  );
  assert.match(
    loadMore,
    /catch \(error\) \{[\s\S]*?if \(sequence !== requestSequence \|\| key !== requestedSession\) return;[\s\S]*?errorMessage =/,
  );
  assert.match(
    loadMore,
    /finally \{[\s\S]*?if \(sequence === requestSequence && key === requestedSession\) loadingMore = false;/,
  );
});
