export const COMMENT_MODERATION_CHANGED_EVENT = "pixiv-client:comment-moderation-changed";

const COMMENT_MODERATION_KEY = "pixiv-client.comment-moderation";
const MAX_MUTED_COMMENTS = 500;
const MAX_LOCAL_REPORTS = 500;

export const LOCAL_REPORT_REASONS = [
  "色情或低俗内容",
  "仇恨言论",
  "恐怖主义内容",
  "危险组织或行为",
  "敏感事件",
  "欺凌或骚扰",
  "危险商品",
  "大麻相关内容",
  "烟草或酒精相关内容",
] as const;

export type LocalReportReason = (typeof LOCAL_REPORT_REASONS)[number];

export interface MutedCommentRecord {
  commentId: string;
  userId?: string | null;
  mutedAtUnixSeconds: number;
}

export interface LocalCommentReport {
  commentId: string;
  resourceKind: "illustration" | "novel";
  resourceId: string;
  reason: LocalReportReason;
  reportedAtUnixSeconds: number;
}

export interface CommentModerationSnapshot {
  mutedComments: MutedCommentRecord[];
  localReports: LocalCommentReport[];
}

type StorageLike = Pick<Storage, "getItem" | "setItem">;

const EMPTY_SNAPSHOT: CommentModerationSnapshot = { mutedComments: [], localReports: [] };

function browserStorage(): StorageLike | null {
  return typeof window === "undefined" ? null : window.localStorage;
}

function validId(value: unknown): value is string {
  return typeof value === "string" && /^\d+$/.test(value);
}

function sanitizeSnapshot(value: unknown): CommentModerationSnapshot {
  if (!value || typeof value !== "object") return structuredClone(EMPTY_SNAPSHOT);
  const candidate = value as Partial<CommentModerationSnapshot>;
  const mutedComments = Array.isArray(candidate.mutedComments)
    ? candidate.mutedComments
      .filter((item): item is MutedCommentRecord => Boolean(
        item && validId(item.commentId) && Number.isFinite(item.mutedAtUnixSeconds),
      ))
      .slice(-MAX_MUTED_COMMENTS)
    : [];
  const localReports = Array.isArray(candidate.localReports)
    ? candidate.localReports
      .filter((item): item is LocalCommentReport => Boolean(
        item
        && validId(item.commentId)
        && validId(item.resourceId)
        && (item.resourceKind === "illustration" || item.resourceKind === "novel")
        && LOCAL_REPORT_REASONS.includes(item.reason)
        && Number.isFinite(item.reportedAtUnixSeconds),
      ))
      .slice(-MAX_LOCAL_REPORTS)
    : [];
  return { mutedComments, localReports };
}

export function readCommentModerationSnapshot(
  storage: StorageLike | null = browserStorage(),
): CommentModerationSnapshot {
  if (!storage) return structuredClone(EMPTY_SNAPSHOT);
  try {
    const raw = storage.getItem(COMMENT_MODERATION_KEY);
    return raw ? sanitizeSnapshot(JSON.parse(raw)) : structuredClone(EMPTY_SNAPSHOT);
  } catch {
    return structuredClone(EMPTY_SNAPSHOT);
  }
}

function writeSnapshot(snapshot: CommentModerationSnapshot, storage: StorageLike | null): void {
  if (!storage) return;
  storage.setItem(COMMENT_MODERATION_KEY, JSON.stringify(sanitizeSnapshot(snapshot)));
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent(COMMENT_MODERATION_CHANGED_EVENT));
  }
}

export function isCommentMuted(
  commentId: string,
  storage: StorageLike | null = browserStorage(),
): boolean {
  return readCommentModerationSnapshot(storage).mutedComments.some((item) => item.commentId === commentId);
}

export function muteComment(
  commentId: string,
  userId?: string | null,
  storage: StorageLike | null = browserStorage(),
  nowUnixSeconds = Math.floor(Date.now() / 1000),
): CommentModerationSnapshot {
  if (!validId(commentId)) return readCommentModerationSnapshot(storage);
  const snapshot = readCommentModerationSnapshot(storage);
  snapshot.mutedComments = [
    ...snapshot.mutedComments.filter((item) => item.commentId !== commentId),
    { commentId, userId: validId(userId) ? userId : null, mutedAtUnixSeconds: nowUnixSeconds },
  ].slice(-MAX_MUTED_COMMENTS);
  writeSnapshot(snapshot, storage);
  return snapshot;
}

export function unmuteComment(
  commentId: string,
  storage: StorageLike | null = browserStorage(),
): CommentModerationSnapshot {
  const snapshot = readCommentModerationSnapshot(storage);
  snapshot.mutedComments = snapshot.mutedComments.filter((item) => item.commentId !== commentId);
  writeSnapshot(snapshot, storage);
  return snapshot;
}

export function recordLocalReport(
  report: Omit<LocalCommentReport, "reportedAtUnixSeconds">,
  storage: StorageLike | null = browserStorage(),
  nowUnixSeconds = Math.floor(Date.now() / 1000),
): CommentModerationSnapshot {
  if (!validId(report.commentId) || !validId(report.resourceId) || !LOCAL_REPORT_REASONS.includes(report.reason)) {
    return readCommentModerationSnapshot(storage);
  }
  const snapshot = readCommentModerationSnapshot(storage);
  snapshot.localReports = [
    ...snapshot.localReports,
    { ...report, reportedAtUnixSeconds: nowUnixSeconds },
  ].slice(-MAX_LOCAL_REPORTS);
  snapshot.mutedComments = [
    ...snapshot.mutedComments.filter((item) => item.commentId !== report.commentId),
    { commentId: report.commentId, userId: null, mutedAtUnixSeconds: nowUnixSeconds },
  ].slice(-MAX_MUTED_COMMENTS);
  writeSnapshot(snapshot, storage);
  return snapshot;
}
