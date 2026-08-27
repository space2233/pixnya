export const HISTORY_BATCH_SIZE = 48;

export interface ProgressiveHistoryWindow<T> {
  visible: T[];
  hasMore: boolean;
  nextCount: number;
}

export function progressiveHistoryWindow<T>(
  entries: readonly T[],
  requestedCount: number,
): ProgressiveHistoryWindow<T> {
  const count = Number.isInteger(requestedCount) && requestedCount >= HISTORY_BATCH_SIZE
    ? requestedCount
    : HISTORY_BATCH_SIZE;
  const visible = entries.slice(0, count);
  const hasMore = visible.length < entries.length;
  return {
    visible,
    hasMore,
    nextCount: hasMore ? Math.min(entries.length, count + HISTORY_BATCH_SIZE) : count,
  };
}
