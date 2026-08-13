import assert from "node:assert/strict";
import test from "node:test";
import { loadAllBookmarkTags } from "../src/lib/bookmark-tags.ts";
import fs from "node:fs";

const illustrationsPage = fs.readFileSync(new URL("../src/lib/components/BrowsePage.svelte", import.meta.url), "utf8");
const novelsPage = fs.readFileSync(new URL("../src/routes/novels/+page.svelte", import.meta.url), "utf8");

test("bookmark tags load every page and deduplicate stable names", async () => {
  const cursors = [];
  const tags = await loadAllBookmarkTags(async (cursor) => {
    cursors.push(cursor ?? null);
    if (!cursor) return { tags: [{ name: "cat", count: 2 }], nextCursor: "page-2" };
    return { tags: [{ name: "cat", count: 3 }, { name: "story", count: 1 }], nextCursor: null };
  });
  assert.deepEqual(cursors, [null, "page-2"]);
  assert.deepEqual(tags, [{ name: "cat", count: 3 }, { name: "story", count: 1 }]);
});

test("changing bookmark visibility clears tags and stale selections", () => {
  assert.match(illustrationsPage, /function selectFilter[\s\S]*selectedBookmarkTag = "";[\s\S]*selectedBookmarkIds = \[\]/);
  assert.match(novelsPage, /function changeBookmarkRestrict[\s\S]*selectedBookmarkTag = "";[\s\S]*selectedNovelIds = \[\]/);
});

test("bookmark tag pagination rejects a repeated cursor", async () => {
  await assert.rejects(
    loadAllBookmarkTags(async () => ({ tags: [], nextCursor: "same" })),
    (error) => error?.kind === "invalid_response",
  );
});
