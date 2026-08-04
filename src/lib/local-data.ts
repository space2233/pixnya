const APPLICATION_STORAGE_PREFIXES = ["pixiv-client.", "pixiv-client:"] as const;
const APPLICATION_PREFERENCES_CHANGED_EVENT = "pixiv-client:preferences-changed";

export interface FrontendLocalDataClearReport {
  localKeysRemoved: number;
  sessionKeysRemoved: number;
}

export function clearFrontendLocalData(): FrontendLocalDataClearReport {
  if (typeof window === "undefined") {
    return { localKeysRemoved: 0, sessionKeysRemoved: 0 };
  }

  const localKeysRemoved = clearApplicationKeys(localStorage);
  const sessionKeysRemoved = clearApplicationKeys(sessionStorage);
  if (typeof document !== "undefined") {
    document.documentElement.dataset.reducedMotion = "false";
  }
  window.dispatchEvent(new CustomEvent(APPLICATION_PREFERENCES_CHANGED_EVENT));
  return { localKeysRemoved, sessionKeysRemoved };
}

function clearApplicationKeys(storage: Storage): number {
  const keys: string[] = [];
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index);
    if (key && APPLICATION_STORAGE_PREFIXES.some((prefix) => key.startsWith(prefix))) {
      keys.push(key);
    }
  }
  for (const key of keys) storage.removeItem(key);
  return keys.length;
}
