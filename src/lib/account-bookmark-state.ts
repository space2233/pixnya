type BookmarkListener = (bookmarked: boolean) => void;

const MAX_ENTRIES_PER_ACCOUNT = 1_024;

export type AccountBookmarkState = {
  resolve(account: string, resourceId: string, fallback: boolean): boolean;
  publish(account: string, resourceId: string, bookmarked: boolean): void;
  subscribe(account: string, resourceId: string, listener: BookmarkListener): () => void;
  clear(account?: string): void;
};

function validKey(value: string): boolean {
  return value.trim().length > 0;
}

export function createAccountBookmarkState(): AccountBookmarkState {
  const valuesByAccount = new Map<string, Map<string, boolean>>();
  const listenersByAccount = new Map<string, Map<string, Set<BookmarkListener>>>();

  return {
    resolve(account, resourceId, fallback) {
      if (!validKey(account) || !validKey(resourceId)) return fallback;
      return valuesByAccount.get(account)?.get(resourceId) ?? fallback;
    },

    publish(account, resourceId, bookmarked) {
      if (!validKey(account) || !validKey(resourceId)) return;

      let accountValues = valuesByAccount.get(account);
      if (!accountValues) {
        accountValues = new Map();
        valuesByAccount.set(account, accountValues);
      }
      accountValues.delete(resourceId);
      accountValues.set(resourceId, bookmarked);
      while (accountValues.size > MAX_ENTRIES_PER_ACCOUNT) {
        const oldest = accountValues.keys().next().value;
        if (oldest === undefined) break;
        accountValues.delete(oldest);
      }

      for (const listener of listenersByAccount.get(account)?.get(resourceId) ?? []) {
        listener(bookmarked);
      }
    },

    subscribe(account, resourceId, listener) {
      if (!validKey(account) || !validKey(resourceId)) return () => undefined;

      let accountListeners = listenersByAccount.get(account);
      if (!accountListeners) {
        accountListeners = new Map();
        listenersByAccount.set(account, accountListeners);
      }
      let resourceListeners = accountListeners.get(resourceId);
      if (!resourceListeners) {
        resourceListeners = new Set();
        accountListeners.set(resourceId, resourceListeners);
      }
      resourceListeners.add(listener);

      return () => {
        resourceListeners.delete(listener);
        if (resourceListeners.size === 0) accountListeners.delete(resourceId);
        if (accountListeners.size === 0) listenersByAccount.delete(account);
      };
    },

    clear(account) {
      if (account === undefined) {
        valuesByAccount.clear();
        return;
      }
      valuesByAccount.delete(account);
    },
  };
}
