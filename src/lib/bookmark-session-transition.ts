import { clearIllustrationBookmarkState } from "./illustration-bookmark-state.ts";
import { clearNovelBookmarkState } from "./novel-bookmark-state.ts";
import type { SessionSnapshot } from "./types.ts";

function accountId(snapshot: SessionSnapshot): string {
  return snapshot.loggedIn ? snapshot.user?.id?.trim() ?? "" : "";
}

export function clearBookmarkOverlaysForSessionTransition(
  previous: SessionSnapshot,
  next: SessionSnapshot,
): void {
  const previousAccount = accountId(previous);
  const nextAccount = accountId(next);
  if (previousAccount === nextAccount) return;
  for (const account of [previousAccount, nextAccount]) {
    if (!account) continue;
    clearIllustrationBookmarkState(account);
    clearNovelBookmarkState(account);
  }
}
