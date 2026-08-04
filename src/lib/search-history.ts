const SEARCH_HISTORY_KEY = "pixiv-client.search-history.v1";
const SEARCH_HISTORY_LIMIT = 8;

function getSearchHistoryStorage(): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readSearchHistory(): string[] {
  const storage = getSearchHistoryStorage();
  if (!storage) return [];
  try {
    const parsed: unknown = JSON.parse(storage.getItem(SEARCH_HISTORY_KEY) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((value): value is string => typeof value === "string")
      .map((value) => value.trim())
      .filter(Boolean)
      .slice(0, SEARCH_HISTORY_LIMIT);
  } catch {
    return [];
  }
}

export function recordSearchHistory(value: string): string[] {
  const normalized = value.trim();
  if (!normalized) return readSearchHistory();

  const history = [
    normalized,
    ...readSearchHistory().filter((item) => item !== normalized),
  ].slice(0, SEARCH_HISTORY_LIMIT);
  const storage = getSearchHistoryStorage();
  try {
    storage?.setItem(SEARCH_HISTORY_KEY, JSON.stringify(history));
  } catch {
    // Searching and tag navigation remain available without WebView storage.
  }
  return history;
}

export function clearSearchHistory(): void {
  const storage = getSearchHistoryStorage();
  try {
    storage?.removeItem(SEARCH_HISTORY_KEY);
  } catch {
    // The caller still clears its in-memory history.
  }
}
