import { createAccountBookmarkState } from "./account-bookmark-state.ts";

const state = createAccountBookmarkState();

export function resolveIllustrationBookmarkState(
  account: string,
  illustrationId: string,
  fallback: boolean,
): boolean {
  return state.resolve(account, illustrationId, fallback);
}

export function publishIllustrationBookmarkState(
  account: string,
  illustrationId: string,
  bookmarked: boolean,
): void {
  state.publish(account, illustrationId, bookmarked);
}

export function subscribeIllustrationBookmarkState(
  account: string,
  illustrationId: string,
  listener: (bookmarked: boolean) => void,
): () => void {
  return state.subscribe(account, illustrationId, listener);
}

export function clearIllustrationBookmarkState(account?: string): void {
  state.clear(account);
}
