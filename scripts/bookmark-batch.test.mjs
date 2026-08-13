import assert from "node:assert/strict";
import test from "node:test";
import { buildBookmarkBatchUpdate } from "../src/lib/bookmark-batch.ts";
import fs from "node:fs";

const backend = fs.readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const api = fs.readFileSync(new URL("../src/lib/pixiv-api.ts", import.meta.url), "utf8");

const detail = {
  restrict: "private",
  tags: [
    { name: "cat", isRegistered: true },
    { name: "unused", isRegistered: false },
  ],
};

test("bookmark batch preserves registered tags and applies visibility", () => {
  assert.deepEqual(buildBookmarkBatchUpdate("illustration", "42", detail, "public"), {
    kind: "illustration", resourceId: "42", bookmarked: true, restrict: "public", tags: ["cat"],
  });
});

test("bookmark batch is bound to the account that selected the entries", () => {
  assert.match(api, /batchUpdateBookmarks\([\s\S]*expectedUserId: string/);
  assert.match(backend, /batch_update_bookmarks\([\s\S]*expected_user_id: String/);
  assert.match(backend, /execute_authenticated_mutation_for_user\([\s\S]*Some\(expected_user_id\)/);
});

test("bookmark batch adds and removes tags case-insensitively", () => {
  assert.deepEqual(buildBookmarkBatchUpdate("novel", "7", detail, "add_tag", "CAT").tags, ["cat"]);
  assert.deepEqual(buildBookmarkBatchUpdate("novel", "7", detail, "add_tag", " story ").tags, ["cat", "story"]);
  assert.deepEqual(buildBookmarkBatchUpdate("novel", "7", detail, "remove_tag", "CAT").tags, []);
});

test("bookmark batch removal preserves metadata while cancelling the bookmark", () => {
  const update = buildBookmarkBatchUpdate("novel", "7", detail, "remove");
  assert.equal(update.bookmarked, false);
  assert.equal(update.restrict, "private");
  assert.deepEqual(update.tags, ["cat"]);
});
