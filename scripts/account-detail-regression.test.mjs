import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function source(path) {
  return readFile(new URL(path, root), "utf8");
}

test("Rust API exposes detail, author, related, and author-work endpoints with cursor binding", async () => {
  const api = await source("crates/api/src/lib.rs");
  assert.match(api, /const ILLUSTRATION_DETAIL_PATH: &str = "\/v1\/illust\/detail"/);
  assert.match(api, /const USER_DETAIL_PATH: &str = "\/v1\/user\/detail"/);
  assert.match(api, /const USER_ILLUSTRATIONS_PATH: &str = "\/v1\/user\/illusts"/);
  assert.match(api, /const RELATED_ILLUSTRATIONS_PATH: &str = "\/v2\/illust\/related"/);
  assert.match(api, /bindings_match = expected_bindings\s*\.iter\(\)\s*\.all/);
  assert.match(api, /fn maps_detail_pages_stats_tags_and_series/);
  assert.match(api, /fn maps_user_profile_and_rejects_non_pixiv_backgrounds/);
});

test("Tauri keeps authenticated detail requests and token refresh inside Rust", async () => {
  const backend = await source("src-tauri/src/lib.rs");
  for (const command of [
    "get_illustration_detail",
    "get_related_illustrations",
    "get_user_detail",
    "get_user_illustrations",
  ]) {
    assert.match(backend, new RegExp(`async fn ${command}\\(`));
    assert.match(backend, new RegExp(`\\n\\s+${command},`));
  }
  assert.match(backend, /execute_authenticated_data_request/);
  assert.match(backend, /refresh_context_after_rejection/);
});

test("detail and author pages route every Pixiv image through PixivImage", async () => {
  const artwork = await source("src/routes/artworks/[id]/+page.svelte");
  const user = await source("src/routes/users/[id]/+page.svelte");
  const profile = await source("src/routes/profile/+page.svelte");
  const card = await source("src/lib/components/ArtworkCard.svelte");

  assert.match(artwork, /getIllustrationDetail/);
  assert.match(artwork, /getRelatedIllustrations/);
  assert.match(artwork, /<PixivImage/);
  assert.doesNotMatch(artwork, /<img\b/);
  assert.match(user, /getUserDetail/);
  assert.match(user, /getUserIllustrations/);
  assert.match(user, /<PixivImage/);
  assert.doesNotMatch(user, /<img\b/);
  assert.match(profile, /totalFollowUsers/);
  assert.match(card, /href={`\/artworks\/\$\{illustration\.id\}`}/);
  assert.match(card, /href={`\/users\/\$\{illustration\.author\.id\}`}/);
});

test("Pixiv rich text is converted to text without injecting HTML", async () => {
  const text = await source("src/lib/pixiv-text.ts");
  const artwork = await source("src/routes/artworks/[id]/+page.svelte");
  const user = await source("src/routes/users/[id]/+page.svelte");

  assert.match(text, /replace\(\/<\[\^>\]\+>\/g, ""\)/);
  assert.match(artwork, /plainPixivText\(detail\.caption\)/);
  assert.match(user, /plainPixivText\(detail\.comment\)/);
  assert.doesNotMatch(artwork, /{@html/);
  assert.doesNotMatch(user, /{@html/);
});
