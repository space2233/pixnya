import type { PixivComment } from "$lib/types";

export type CommentResourceKind = "illustration" | "novel";

export interface CommentThreadSnapshot {
  comments: PixivComment[];
  nextCursor: string | null;
  totalComments: number;
}

const MAX_COMMENT_THREADS = 32;
const threads = new Map<string, CommentThreadSnapshot>();
const roots = new Map<string, PixivComment>();

function threadKey(kind: CommentResourceKind, resourceId: string): string {
  return `${kind}:${resourceId}`;
}

function rootKey(kind: CommentResourceKind, resourceId: string, commentId: string): string {
  return `${threadKey(kind, resourceId)}:${commentId}`;
}

function trimOldest<T>(map: Map<string, T>, maximum: number): void {
  while (map.size > maximum) {
    const oldest = map.keys().next().value;
    if (typeof oldest !== "string") return;
    map.delete(oldest);
  }
}

export function rememberCommentThread(
  kind: CommentResourceKind,
  resourceId: string,
  snapshot: CommentThreadSnapshot,
): void {
  const key = threadKey(kind, resourceId);
  threads.delete(key);
  threads.set(key, snapshot);
  trimOldest(threads, MAX_COMMENT_THREADS);
}

export function recallCommentThread(
  kind: CommentResourceKind,
  resourceId: string,
): CommentThreadSnapshot | null {
  return threads.get(threadKey(kind, resourceId)) ?? null;
}

export function rememberCommentRoot(
  kind: CommentResourceKind,
  resourceId: string,
  comment: PixivComment,
): void {
  const key = rootKey(kind, resourceId, comment.id);
  roots.delete(key);
  roots.set(key, comment);
  trimOldest(roots, MAX_COMMENT_THREADS * 4);
}

export function recallCommentRoot(
  kind: CommentResourceKind,
  resourceId: string,
  commentId: string,
): PixivComment | null {
  return roots.get(rootKey(kind, resourceId, commentId)) ?? null;
}

export function forgetComment(
  kind: CommentResourceKind,
  resourceId: string,
  commentId: string,
): void {
  roots.delete(rootKey(kind, resourceId, commentId));
  const key = threadKey(kind, resourceId);
  const thread = threads.get(key);
  if (!thread || !thread.comments.some((comment) => comment.id === commentId)) return;
  threads.set(key, {
    ...thread,
    comments: thread.comments.filter((comment) => comment.id !== commentId),
    totalComments: Math.max(0, thread.totalComments - 1),
  });
}
