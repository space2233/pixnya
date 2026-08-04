import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  COMMENT_EMOJIS,
  insertCommentEmoji,
  tokenizeCommentText,
} from "../src/lib/comment-emoji.ts";
import {
  isCommentMuted,
  muteComment,
  readCommentModerationSnapshot,
  recordLocalReport,
  unmuteComment,
} from "../src/lib/comment-moderation.ts";
import {
  recallCommentRoot,
  recallCommentThread,
  rememberCommentRoot,
  rememberCommentThread,
} from "../src/lib/comment-thread-memory.ts";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

function memoryStorage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
  };
}

function comment(id = "701") {
  return {
    id,
    text: "好看(normal)(heart)",
    date: "2026-08-04T12:00:00+09:00",
    user: { id: "42", name: "Alice", account: "alice", isFollowed: false },
    hasReplies: true,
    stamp: { id: "501", url: "https://s.pximg.net/common/images/emoji/501.png" },
  };
}

test("comment emoji tokenizer preserves text and maps known Pixiv tokens", () => {
  assert.equal(COMMENT_EMOJIS.length, 38);
  assert.deepEqual(tokenizeCommentText("开头(normal)中间(unknown)结尾"), [
    { kind: "text", value: "开头" },
    {
      kind: "emoji",
      token: "(normal)",
      url: "https://s.pximg.net/common/images/emoji/101.png",
    },
    { kind: "text", value: "中间(unknown)结尾" },
  ]);
  const inserted = insertCommentEmoji("你好", "(heart)", 2, 2);
  assert.equal(inserted.value, "你好(heart)");
  assert.equal(inserted.cursor, inserted.value.length);
});

test("local mute and report persist without storing comment text", () => {
  const storage = memoryStorage();
  muteComment("701", "42", storage, 100);
  assert.equal(isCommentMuted("701", storage), true);

  recordLocalReport({
    commentId: "702",
    resourceKind: "illustration",
    resourceId: "99",
    reason: "欺凌或骚扰",
  }, storage, 101);
  const snapshot = readCommentModerationSnapshot(storage);
  assert.equal(snapshot.localReports.length, 1);
  assert.equal(snapshot.localReports[0].reason, "欺凌或骚扰");
  assert.equal(isCommentMuted("702", storage), true);
  assert.equal(JSON.stringify(snapshot).includes("评论正文"), false);

  unmuteComment("701", storage);
  assert.equal(isCommentMuted("701", storage), false);
});

test("comment thread memory restores the source list and root comment", () => {
  const rootComment = comment();
  rememberCommentThread("illustration", "99", {
    comments: [rootComment],
    nextCursor: "opaque-cursor",
    totalComments: 7,
  });
  rememberCommentRoot("illustration", "99", rootComment);
  assert.equal(recallCommentThread("illustration", "99")?.nextCursor, "opaque-cursor");
  assert.equal(recallCommentRoot("illustration", "99", "701")?.user?.name, "Alice");
});

test("UI uses an independent reply page, local moderation, and both emoji forms", async () => {
  const comments = await source("src/lib/components/ArtworkComments.svelte");
  const card = await source("src/lib/components/CommentCard.svelte");
  const content = await source("src/lib/components/CommentText.svelte");
  const composer = await source("src/lib/components/CommentComposer.svelte");
  const replies = await source("src/routes/comments/[kind]/[resourceId]/[commentId]/+page.svelte");
  const api = await source("crates/api/src/lib.rs");

  assert.match(comments, /\/comments\/\$\{resourceKind\}\/\$\{resourceId\}\/\$\{comment\.id\}/);
  assert.doesNotMatch(comments, /replyCursors|replyErrors|reply-list/);
  assert.match(card, /muteComment/);
  assert.match(card, /recordLocalReport/);
  assert.match(card, /不会向 Pixiv 发送举报/);
  assert.match(content, /comment\.stamp\?\.url/);
  assert.match(content, /tokenizeCommentText/);
  assert.match(composer, /COMMENT_EMOJIS/);
  assert.match(replies, /getCommentReplies/);
  assert.match(replies, /getNovelCommentReplies/);
  assert.match(replies, /addIllustrationComment\(resourceId, text, commentId\)/);
  assert.match(replies, /addNovelComment\(resourceId, text, commentId\)/);
  assert.match(api, /pub struct CommentStamp/);
  assert.match(api, /stamp_url: String/);
});
