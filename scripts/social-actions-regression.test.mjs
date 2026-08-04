import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function source(path) {
  return readFile(new URL(path, root), "utf8");
}

test("Rust API exposes bookmark, follow, comment, and reply operations", async () => {
  const api = await source("crates/api/src/lib.rs");
  for (const endpoint of [
    "/v2/illust/bookmark/add",
    "/v1/illust/bookmark/delete",
    "/v1/user/follow/add",
    "/v1/user/follow/delete",
    "/v3/illust/comments",
    "/v2/illust/comment/replies",
    "/v1/illust/comment/add",
  ]) {
    assert.match(api, new RegExp(endpoint.replaceAll("/", "\\/")));
  }

  for (const method of [
    "add_illustration_bookmark",
    "delete_illustration_bookmark",
    "follow_user",
    "unfollow_user",
    "illustration_comments",
    "comment_replies",
    "add_illustration_comment",
  ]) {
    assert.match(api, new RegExp(`pub fn ${method}\\(`));
  }

  assert.match(api, /comment\.chars\(\)\.count\(\) > 140/);
  assert.match(api, /fn maps_comments_replies_and_locks_comment_cursor_to_the_illustration/);
  assert.match(api, /fn comment_input_is_trimmed_bounded_and_rejects_control_characters/);
});

test("Tauri keeps authenticated mutations in Rust and serializes writes", async () => {
  const backend = await source("src-tauri/src/lib.rs");
  for (const command of [
    "set_illustration_bookmark",
    "set_user_follow",
    "get_illustration_comments",
    "get_comment_replies",
    "add_illustration_comment",
  ]) {
    assert.match(backend, new RegExp(`async fn ${command}\\(`));
    assert.match(backend, new RegExp(`\\n\\s+${command},`));
  }

  assert.match(backend, /mutation_gate: Arc<tokio::sync::Semaphore>/);
  assert.match(backend, /mutation_gate: Arc::new\(tokio::sync::Semaphore::new\(1\)\)/);
  assert.match(backend, /async fn execute_authenticated_mutation/);
  assert.doesNotMatch(backend, /access_token:\s*String/);
});

test("artwork and user pages expose live bookmark and follow controls", async () => {
  const card = await source("src/lib/components/ArtworkCard.svelte");
  const artwork = await source("src/routes/artworks/[id]/+page.svelte");
  const user = await source("src/routes/users/[id]/+page.svelte");

  assert.match(card, /setIllustrationBookmark/);
  assert.match(card, /class:active=\{bookmarked\}/);
  assert.match(artwork, /setIllustrationBookmark/);
  assert.match(artwork, /bookmarkRestrict/);
  assert.match(user, /setUserFollow/);
  assert.match(user, /detail\.user\.isFollowed = followed/);
});

test("comments support paging, posting, and replies through the shared media path", async () => {
  const comments = await source("src/lib/components/ArtworkComments.svelte");
  const replies = await source("src/routes/comments/[kind]/[resourceId]/[commentId]/+page.svelte");
  const card = await source("src/lib/components/CommentCard.svelte");
  const content = await source("src/lib/components/CommentText.svelte");
  const composer = await source("src/lib/components/CommentComposer.svelte");
  const artwork = await source("src/routes/artworks/[id]/+page.svelte");

  for (const symbol of ["getIllustrationComments", "addIllustrationComment"]) {
    assert.match(comments, new RegExp(symbol));
  }
  for (const symbol of ["getCommentReplies", "addIllustrationComment"]) {
    assert.match(replies, new RegExp(symbol));
  }

  assert.match(composer, /maxlength="140"/);
  assert.match(content, /<PixivImage/);
  assert.match(card, /<PixivImage/);
  for (const component of [comments, replies, card, content, composer]) assert.doesNotMatch(component, /<img\b/);
  assert.match(card, /本地举报并屏蔽/);
  assert.match(comments, /\/comments\/\$\{resourceKind\}/);
  assert.match(artwork, /<ArtworkComments/);
});
