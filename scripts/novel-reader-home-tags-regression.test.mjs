import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (...segments) => readFileSync(path.join(root, ...segments), "utf8");

test("novel metadata and reading live on separate routes", () => {
  const detail = read("src", "routes", "novels", "[id]", "+page.svelte");
  const readerPath = path.join(root, "src", "routes", "novels", "[id]", "read", "+page.svelte");

  assert.ok(existsSync(readerPath), "an independent /novels/[id]/read route must exist");
  const reader = readFileSync(readerPath, "utf8");

  assert.match(detail, /href=\{`\/novels\/\$\{detail\.novel\.id\}\/read`\}/);
  assert.match(detail, /class="read-button"/);
  assert.doesNotMatch(detail, /class="reader-controls"/);
  assert.doesNotMatch(detail, /class="novel-body"/);
  assert.match(reader, /class="reader-controls"/);
  assert.match(reader, /class="novel-body(?:\s|\")/);
});

test("novel detail actions reflow instead of colliding on mobile", () => {
  const detail = read("src", "routes", "novels", "[id]", "+page.svelte");

  assert.match(detail, /class="detail-actions"/);
  assert.match(detail, /@media \(max-width: 720px\)[\s\S]*?\.detail-actions\s*\{[\s\S]*?grid-template-columns:\s*1fr/);
});

test("home tags restore and replace the most recent successful response", () => {
  const cachePath = path.join(root, "src", "lib", "home-tag-cache.ts");
  assert.ok(existsSync(cachePath), "home tag cache module must exist");

  const cache = readFileSync(cachePath, "utf8");
  const home = read("src", "lib", "components", "BrowsePage.svelte");

  assert.match(cache, /export function loadHomeTagCache/);
  assert.match(cache, /export function saveHomeTagCache/);
  assert.match(cache, /PIXIV_CLIENT_HOME_TAGS_V1/);
  assert.match(home, /loadHomeTagCache\(\)/);
  assert.match(home, /saveHomeTagCache\(/);
  assert.doesNotMatch(home, /const topics\s*=/);
});
