const MAX_RETURN_ENTRIES = 32;
const RETURN_ENTRY_TTL_MS = 30 * 60 * 1000;

export type NavigationLocation = {
  url: string;
  navigationIndex: number | null;
  scrollX: number;
  scrollY: number;
};

type ReturnEntry = {
  source: NavigationLocation;
  destinationUrl: string;
  destinationNavigationIndex: number | null;
  createdAt: number;
};

type PendingPosition = {
  sourceUrl: string;
  sourceNavigationIndex: number | null;
  scrollX: number;
  scrollY: number;
  createdAt: number;
};

export type ReturnNavigationAdapter = {
  current(): NavigationLocation;
  readStack(): unknown;
  writeStack(entries: ReturnEntry[]): void;
  readPending(): unknown;
  writePending(pending: PendingPosition | null): void;
  historyBack(): void;
  replaceWithFallback(url: string): void;
  restoreScroll(x: number, y: number): void;
  now(): number;
};

export type ReturnNavigator = {
  capture(destination: string): boolean;
  returnToPrevious(fallback: string): "history" | "fallback";
  restorePendingPosition(): boolean;
  restoreAfterHistoryPop(previousUrl: string): boolean;
};

function pathnameOf(value: string): string {
  try {
    return new URL(value, "https://pixnya.invalid").pathname.replace(/\/$/, "") || "/";
  } catch {
    return "";
  }
}

function normalizeInternalUrl(value: string): string | null {
  if (!value.startsWith("/")) return null;
  try {
    const parsed = new URL(value, "https://pixnya.invalid");
    if (parsed.origin !== "https://pixnya.invalid") return null;
    return `${parsed.pathname}${parsed.search}${parsed.hash}`;
  } catch {
    return null;
  }
}

export function isReturnDestination(value: string): boolean {
  const pathname = pathnameOf(value);
  return [
    /^\/artworks\/[^/]+$/,
    /^\/novels\/[^/]+(?:\/read)?$/,
    /^\/users\/[^/]+$/,
    /^\/series\/(?:artworks|novels)\/[^/]+$/,
    /^\/comments\/(?:illustration|novel)\/[^/]+\/[^/]+$/,
    /^\/offline\/(?:artworks|novels|ugoira)\/[^/]+$/,
    /^\/login$/,
    /^\/settings\/network$/,
  ].some((pattern) => pattern.test(pathname));
}

function finiteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function navigationIndex(value: unknown): value is number | null {
  return value === null || Number.isInteger(value);
}

function isLocation(value: unknown): value is NavigationLocation {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<NavigationLocation>;
  return typeof candidate.url === "string"
    && navigationIndex(candidate.navigationIndex)
    && finiteNumber(candidate.scrollX)
    && finiteNumber(candidate.scrollY);
}

function isEntry(value: unknown): value is ReturnEntry {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<ReturnEntry>;
  return isLocation(candidate.source)
    && typeof candidate.destinationUrl === "string"
    && navigationIndex(candidate.destinationNavigationIndex)
    && finiteNumber(candidate.createdAt);
}

function isPending(value: unknown): value is PendingPosition {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<PendingPosition>;
  return typeof candidate.sourceUrl === "string"
    && navigationIndex(candidate.sourceNavigationIndex)
    && finiteNumber(candidate.scrollX)
    && finiteNumber(candidate.scrollY)
    && finiteNumber(candidate.createdAt);
}

function indexMatches(expected: number | null, actual: number | null): boolean {
  return expected === null || actual === null || expected === actual;
}

export function createReturnNavigator(adapter: ReturnNavigationAdapter): ReturnNavigator {
  function activeStack(): ReturnEntry[] {
    const now = adapter.now();
    const stored = adapter.readStack();
    if (!Array.isArray(stored)) return [];
    return stored
      .filter(isEntry)
      .filter((entry) => now - entry.createdAt <= RETURN_ENTRY_TTL_MS)
      .slice(-MAX_RETURN_ENTRIES);
  }

  return {
    capture(destination) {
      const destinationUrl = normalizeInternalUrl(destination);
      if (!destinationUrl || !isReturnDestination(destinationUrl)) return false;

      const source = adapter.current();
      if (!normalizeInternalUrl(source.url) || source.url === destinationUrl) return false;

      const entry: ReturnEntry = {
        source: {
          ...source,
          scrollX: Math.max(0, source.scrollX),
          scrollY: Math.max(0, source.scrollY),
        },
        destinationUrl,
        destinationNavigationIndex: source.navigationIndex === null
          ? null
          : source.navigationIndex + 1,
        createdAt: adapter.now(),
      };
      adapter.writeStack([...activeStack(), entry].slice(-MAX_RETURN_ENTRIES));
      return true;
    },

    returnToPrevious(fallback) {
      const current = adapter.current();
      const stack = activeStack();
      let matchIndex = -1;
      for (let index = stack.length - 1; index >= 0; index -= 1) {
        const entry = stack[index];
        if (entry.destinationUrl === current.url
          && indexMatches(entry.destinationNavigationIndex, current.navigationIndex)) {
          matchIndex = index;
          break;
        }
      }

      if (matchIndex >= 0) {
        const entry = stack[matchIndex];
        adapter.writeStack(stack.slice(0, matchIndex));
        adapter.writePending({
          sourceUrl: entry.source.url,
          sourceNavigationIndex: entry.source.navigationIndex,
          scrollX: entry.source.scrollX,
          scrollY: entry.source.scrollY,
          createdAt: adapter.now(),
        });
        adapter.historyBack();
        return "history";
      }

      adapter.writePending(null);
      adapter.replaceWithFallback(normalizeInternalUrl(fallback) ?? "/");
      return "fallback";
    },

    restorePendingPosition() {
      const pending = adapter.readPending();
      if (!isPending(pending) || adapter.now() - pending.createdAt > RETURN_ENTRY_TTL_MS) {
        adapter.writePending(null);
        return false;
      }

      const current = adapter.current();
      if (current.url !== pending.sourceUrl
        || !indexMatches(pending.sourceNavigationIndex, current.navigationIndex)) {
        return false;
      }

      adapter.writePending(null);
      adapter.restoreScroll(pending.scrollX, pending.scrollY);
      return true;
    },

    restoreAfterHistoryPop(previousUrl) {
      const destinationUrl = normalizeInternalUrl(previousUrl);
      if (!destinationUrl) return false;
      const current = adapter.current();
      const stack = activeStack();
      let matchIndex = -1;
      for (let index = stack.length - 1; index >= 0; index -= 1) {
        const entry = stack[index];
        if (entry.destinationUrl === destinationUrl
          && entry.source.url === current.url
          && indexMatches(entry.source.navigationIndex, current.navigationIndex)) {
          matchIndex = index;
          break;
        }
      }
      if (matchIndex < 0) return false;

      const entry = stack[matchIndex];
      adapter.writeStack(stack.slice(0, matchIndex));
      adapter.writePending(null);
      adapter.restoreScroll(entry.source.scrollX, entry.source.scrollY);
      return true;
    },
  };
}
