export const COMMENT_MODERATION_CHANGED_EVENT = "pixiv-client:comment-moderation-changed";

const COMMENT_MODERATION_KEY = "pixiv-client.comment-moderation";
const MAX_MUTED_COMMENTS = 500;
const MAX_LOCAL_REPORTS = 500;

export const LOCAL_REPORT_REASONS = [
  "sexual_or_vulgar",
  "hate_speech",
  "terrorism",
  "dangerous_organization",
  "sensitive_event",
  "bullying_or_harassment",
  "dangerous_goods",
  "cannabis",
  "tobacco_or_alcohol",
] as const;

export type LocalReportReason = (typeof LOCAL_REPORT_REASONS)[number];

const LEGACY_REPORT_REASONS: Record<string, LocalReportReason> = {
  "\u8272\u60c5\u6216\u4f4e\u4fd7\u5185\u5bb9": "sexual_or_vulgar",
  "\u4ec7\u6068\u8a00\u8bba": "hate_speech",
  "\u6050\u6016\u4e3b\u4e49\u5185\u5bb9": "terrorism",
  "\u5371\u9669\u7ec4\u7ec7\u6216\u884c\u4e3a": "dangerous_organization",
  "\u654f\u611f\u4e8b\u4ef6": "sensitive_event",
  "\u6b3a\u51cc\u6216\u9a9a\u6270": "bullying_or_harassment",
  "\u5371\u9669\u5546\u54c1": "dangerous_goods",
  "\u5927\u9ebb\u76f8\u5173\u5185\u5bb9": "cannabis",
  "\u70df\u8349\u6216\u9152\u7cbe\u76f8\u5173\u5185\u5bb9": "tobacco_or_alcohol",
};

function normalizeReportReason(value: unknown): LocalReportReason | null {
  if (typeof value !== "string") return null;
  if (LOCAL_REPORT_REASONS.includes(value as LocalReportReason)) return value as LocalReportReason;
  return LEGACY_REPORT_REASONS[value] ?? null;
}

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
      .flatMap((item) => {
        const reason = normalizeReportReason(item?.reason);
        if (
          !item
          || !validId(item.commentId)
          || !validId(item.resourceId)
          || (item.resourceKind !== "illustration" && item.resourceKind !== "novel")
          || !reason
          || !Number.isFinite(item.reportedAtUnixSeconds)
        ) return [];
        return [{ ...item, reason } as LocalCommentReport];
      })
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
