type BookmarkListener = (bookmarked: boolean) => void;

const MAX_ENTRIES_PER_ACCOUNT = 1_024;
const valuesByAccount = new Map<string, Map<string, boolean>>();
const listenersByAccount = new Map<string, Map<string, Set<BookmarkListener>>>();

function validKey(value: string): boolean {
  return value.trim().length > 0;
}

export function resolveIllustrationBookmarkState(
  account: string,
  illustrationId: string,
  fallback: boolean,
): boolean {
  if (!validKey(account) || !validKey(illustrationId)) return fallback;
  return valuesByAccount.get(account)?.get(illustrationId) ?? fallback;
}

export function publishIllustrationBookmarkState(
  account: string,
  illustrationId: string,
  bookmarked: boolean,
): void {
  if (!validKey(account) || !validKey(illustrationId)) return;

  let accountValues = valuesByAccount.get(account);
  if (!accountValues) {
    accountValues = new Map();
    valuesByAccount.set(account, accountValues);
  }
  accountValues.delete(illustrationId);
  accountValues.set(illustrationId, bookmarked);
  while (accountValues.size > MAX_ENTRIES_PER_ACCOUNT) {
    const oldest = accountValues.keys().next().value;
    if (oldest === undefined) break;
    accountValues.delete(oldest);
  }

  for (const listener of listenersByAccount.get(account)?.get(illustrationId) ?? []) {
    listener(bookmarked);
  }
}

export function subscribeIllustrationBookmarkState(
  account: string,
  illustrationId: string,
  listener: BookmarkListener,
): () => void {
  if (!validKey(account) || !validKey(illustrationId)) return () => undefined;

  let accountListeners = listenersByAccount.get(account);
  if (!accountListeners) {
    accountListeners = new Map();
    listenersByAccount.set(account, accountListeners);
  }
  let illustrationListeners = accountListeners.get(illustrationId);
  if (!illustrationListeners) {
    illustrationListeners = new Set();
    accountListeners.set(illustrationId, illustrationListeners);
  }
  illustrationListeners.add(listener);

  return () => {
    illustrationListeners.delete(listener);
    if (illustrationListeners.size === 0) accountListeners.delete(illustrationId);
    if (accountListeners.size === 0) listenersByAccount.delete(account);
  };
}

export function clearIllustrationBookmarkState(account?: string): void {
  if (account === undefined) {
    valuesByAccount.clear();
    return;
  }
  valuesByAccount.delete(account);
}
