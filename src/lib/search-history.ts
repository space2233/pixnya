const SEARCH_HISTORY_KEY = "pixiv-client.search-history.v1";
export const SEARCH_HISTORY_LIMIT_KEY = "pixiv-client.search-history-limit.v1";
export const SEARCH_HISTORY_CHANGED_EVENT = "pixnya:search-history-changed";
export const SEARCH_HISTORY_LIMIT_OPTIONS = [8, 20, 50, 100, null] as const;
export const DEFAULT_SEARCH_HISTORY_LIMIT = 8;
const MAX_SEARCH_TERM_BYTES = 512;
const MAX_SEARCH_TERM_CHARACTERS = 100;

export type SearchHistoryLimit = (typeof SEARCH_HISTORY_LIMIT_OPTIONS)[number];

export function searchHistoryLimitOrDefault(
  limit: SearchHistoryLimit | undefined,
): SearchHistoryLimit {
  return limit === undefined ? DEFAULT_SEARCH_HISTORY_LIMIT : limit;
}

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
    const limit = readSearchHistoryLimit();
    const history: string[] = [];
    const seen = new Set<string>();
    for (const value of parsed) {
      if (typeof value !== "string") continue;
      const normalized = normalizeSearchHistoryValue(value);
      if (!normalized || seen.has(normalized)) continue;
      seen.add(normalized);
      history.push(normalized);
      if (limit !== null && history.length >= limit) break;
    }
    return history;
  } catch {
    return [];
  }
}

export function readSearchHistoryLimit(): SearchHistoryLimit {
  const storage = getSearchHistoryStorage();
  if (!storage) return DEFAULT_SEARCH_HISTORY_LIMIT;
  try {
    const value = storage.getItem(SEARCH_HISTORY_LIMIT_KEY);
    if (value === "unlimited") return null;
    const numeric = Number(value);
    return SEARCH_HISTORY_LIMIT_OPTIONS.includes(numeric as SearchHistoryLimit)
      ? (numeric as SearchHistoryLimit)
      : DEFAULT_SEARCH_HISTORY_LIMIT;
  } catch {
    return DEFAULT_SEARCH_HISTORY_LIMIT;
  }
}

export function writeSearchHistoryLimit(limit: SearchHistoryLimit): string[] {
  const normalizedLimit = SEARCH_HISTORY_LIMIT_OPTIONS.includes(limit)
    ? limit
    : DEFAULT_SEARCH_HISTORY_LIMIT;
  const storage = getSearchHistoryStorage();
  try {
    storage?.setItem(
      SEARCH_HISTORY_LIMIT_KEY,
      normalizedLimit === null ? "unlimited" : String(normalizedLimit),
    );
  } catch {
    // The in-memory result still follows the selected limit.
  }
  const history = applyLimit(readSearchHistory(), normalizedLimit);
  try {
    storage?.setItem(SEARCH_HISTORY_KEY, JSON.stringify(history));
  } catch {
    // Search remains available when WebView storage is full or unavailable.
  }
  notifySearchHistoryChanged();
  return history;
}

export function recordSearchHistory(value: string): string[] {
  const normalized = normalizeSearchHistoryValue(value);
  if (!normalized) return readSearchHistory();

  const history = applyLimit(
    [normalized, ...readSearchHistory().filter((item) => item !== normalized)],
    readSearchHistoryLimit(),
  );
  const storage = getSearchHistoryStorage();
  try {
    storage?.setItem(SEARCH_HISTORY_KEY, JSON.stringify(history));
  } catch {
    // Searching and tag navigation remain available without WebView storage.
  }
  notifySearchHistoryChanged();
  return history;
}

export function normalizeSearchHistoryValue(value: string): string {
  const normalized = value.trim();
  if (
    !normalized
    || Array.from(normalized).length > MAX_SEARCH_TERM_CHARACTERS
    || new TextEncoder().encode(normalized).byteLength > MAX_SEARCH_TERM_BYTES
    || Array.from(normalized).some((character) => /\p{Cc}/u.test(character))
  ) {
    return "";
  }
  return normalized;
}

function applyLimit(history: string[], limit: SearchHistoryLimit): string[] {
  return limit === null ? history : history.slice(0, limit);
}

export function clearSearchHistory(): void {
  const storage = getSearchHistoryStorage();
  try {
    storage?.removeItem(SEARCH_HISTORY_KEY);
  } catch {
    // The caller still clears its in-memory history.
  }
  notifySearchHistoryChanged();
}

function notifySearchHistoryChanged(): void {
  if (typeof window !== "undefined" && typeof window.dispatchEvent === "function") {
    window.dispatchEvent(new Event(SEARCH_HISTORY_CHANGED_EVENT));
  }
}
