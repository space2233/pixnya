import { createAccountBookmarkState } from "./account-bookmark-state.ts";

const state = createAccountBookmarkState();

export function resolveNovelBookmarkState(
  account: string,
  novelId: string,
  fallback: boolean,
): boolean {
  return state.resolve(account, novelId, fallback);
}

export function publishNovelBookmarkState(
  account: string,
  novelId: string,
  bookmarked: boolean,
): void {
  state.publish(account, novelId, bookmarked);
}

export function subscribeNovelBookmarkState(
  account: string,
  novelId: string,
  listener: (bookmarked: boolean) => void,
): () => void {
  return state.subscribe(account, novelId, listener);
}

export function clearNovelBookmarkState(account?: string): void {
  state.clear(account);
}
