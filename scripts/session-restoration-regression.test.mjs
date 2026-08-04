import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const session = source("../src/lib/session.ts");
const browse = source("../src/lib/components/BrowsePage.svelte");
const profile = source("../src/routes/profile/+page.svelte");
const authenticatedPages = [
  "../src/routes/artworks/[id]/+page.svelte",
  "../src/routes/novels/+page.svelte",
  "../src/routes/novels/[id]/+page.svelte",
  "../src/routes/search/+page.svelte",
  "../src/routes/users/[id]/+page.svelte",
].map(source);

test("logged-out prompts stay hidden until automatic session restoration has finished", () => {
  assert.match(session, /export const sessionRestoring\s*=\s*writable(?:<boolean>)?\(true\)/);
  assert.match(session, /sessionRestoring\.set\(false\)/);
  assert.match(browse, /\{#if !\$sessionRestoring && !\$session\.loggedIn\}/);
  assert.match(profile, /\{#if !\$sessionRestoring\}[\s\S]*?使用 Pixiv 登录[\s\S]*?\{\/if\}/);
  for (const page of authenticatedPages) {
    assert.doesNotMatch(page, /\{#if !\$session\.loggedIn\}/);
    assert.match(page, /\{#if !\$sessionRestoring && !\$session\.loggedIn\}/);
  }
});
