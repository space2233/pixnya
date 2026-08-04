import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const api = source("../crates/api/src/lib.rs");
const backend = source("../src-tauri/src/lib.rs");
const frontend = source("../src/lib/pixiv-api.ts");
const following = source("../src/routes/following/+page.svelte");
const browse = source("../src/lib/components/BrowsePage.svelte");
const users = source("../src/routes/following/users/+page.svelte");
const tabs = source("../src/lib/components/FollowingTabs.svelte");
const profile = source("../src/routes/profile/+page.svelte");

test("Rust API binds followed-user pagination to owner, visibility, and endpoint", () => {
  assert.match(api, /const USER_FOLLOWING_PATH:\s*&str\s*=\s*"\/v1\/user\/following"/);
  assert.match(api, /pub fn followed_users\([\s\S]*?user_id:\s*&str[\s\S]*?restrict:\s*&str[\s\S]*?Result<UserPreviewPage/);
  assert.match(api, /decode_cursor\(cursor, USER_FOLLOWING_PATH, &bindings\)/);
  assert.match(api, /user_preview_page_from_envelope\(envelope, USER_FOLLOWING_PATH, &bindings\)/);
  assert.match(api, /fn user_preview_page_from_envelope\([\s\S]*?expected_path:\s*&str/);
});

test("Tauri derives the followed-list owner from the authenticated Rust session", () => {
  assert.match(backend, /async fn get_followed_users\(/);
  assert.match(backend, /api\.followed_users\(token, user_id, &restrict, cursor\.as_deref\(\), signature\)/);
  assert.match(backend, /get_followed_users,/);
  assert.doesNotMatch(frontend, /getFollowedUsers\([^)]*userId/);
  assert.match(frontend, /invoke<UserPreviewPage>\("get_followed_users"/);
});

test("following works and followed authors share one active navigation destination", () => {
  assert.match(following, /BrowsePage section="following"/);
  assert.match(browse, /section === "following"[\s\S]*?<FollowingTabs/);
  assert.match(tabs, /href="\/following"/);
  assert.match(tabs, /href="\/following\/users"/);
  assert.match(users, /getFollowedUsers/);
  assert.match(users, /UserPreviewCard/);
  assert.match(users, /公开关注/);
  assert.match(users, /非公开关注/);
  assert.match(profile, /href="\/following\/users"/);
});
