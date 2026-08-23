const MAX_ENTRIES = 192;
const MAX_BYTES = 96 * 1024 * 1024;

type CacheEntry = {
  source: string | null;
  ready: Promise<string>;
  references: number;
  sizeBytes: number;
  lastUsed: number;
  retired: boolean;
};

export type PixivImageSourceLease = {
  source: string | null;
  ready: Promise<string>;
  release: () => void;
  invalidate: () => void;
};

const entries = new Map<string, CacheEntry>();
let clock = 0;
let totalBytes = 0;

export function acquirePixivImageSource(
  key: string,
  load: () => Promise<Uint8Array>,
): PixivImageSourceLease {
  let entry = entries.get(key);
  if (!entry) {
    entry = {
      source: null,
      ready: Promise.resolve(""),
      references: 0,
      sizeBytes: 0,
      lastUsed: ++clock,
      retired: false,
    };
    const pendingEntry = entry;
    entries.set(key, pendingEntry);
    let loadResult: Promise<Uint8Array>;
    try {
      loadResult = load();
    } catch (error) {
      loadResult = Promise.reject(error);
    }
    pendingEntry.ready = loadResult
      .then((bytes) => {
        if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0) {
          throw new Error("empty Pixiv media response");
        }
        if (entries.get(key) !== pendingEntry) {
          throw new Error("Pixiv image cache entry was cleared");
        }
        const payload = bytes.buffer.slice(
          bytes.byteOffset,
          bytes.byteOffset + bytes.byteLength,
        ) as ArrayBuffer;
        const objectUrl = URL.createObjectURL(new Blob([payload]));
        if (entries.get(key) !== pendingEntry) {
          URL.revokeObjectURL(objectUrl);
          throw new Error("Pixiv image cache entry was cleared");
        }
        pendingEntry.source = objectUrl;
        pendingEntry.sizeBytes = bytes.byteLength;
        pendingEntry.lastUsed = ++clock;
        totalBytes += bytes.byteLength;
        prune();
        return objectUrl;
      })
      .catch((error) => {
        if (entries.get(key) === pendingEntry) entries.delete(key);
        throw error;
      });
  }

  entry.references += 1;
  entry.lastUsed = ++clock;
  let released = false;
  return {
    source: entry.source,
    ready: entry.ready,
    release: () => {
      if (released) return;
      released = true;
      entry.references = Math.max(0, entry.references - 1);
      if (entry.retired) {
        if (entry.references === 0 && entry.source) {
          URL.revokeObjectURL(entry.source);
          entry.source = null;
        }
        return;
      }
      if (entries.get(key) !== entry) return;
      entry.lastUsed = ++clock;
      prune();
    },
    invalidate: () => invalidateEntry(key, entry),
  };
}

export function clearPixivImageMemoryCache(): void {
  const previous = [...entries.values()];
  entries.clear();
  totalBytes = 0;
  for (const entry of previous) {
    entry.retired = true;
    if (entry.references === 0 && entry.source) {
      URL.revokeObjectURL(entry.source);
      entry.source = null;
    }
  }
}

export function pixivImageMemoryCacheStatsForTests(): { entries: number; totalBytes: number } {
  return { entries: entries.size, totalBytes };
}

function invalidateEntry(key: string, entry: CacheEntry): void {
  if (entries.get(key) === entry) {
    entries.delete(key);
    totalBytes = Math.max(0, totalBytes - entry.sizeBytes);
  }
  entry.retired = true;
  if (entry.source) {
    URL.revokeObjectURL(entry.source);
    entry.source = null;
  }
}

function prune(): void {
  while (entries.size > MAX_ENTRIES || totalBytes > MAX_BYTES) {
    const candidate = [...entries.entries()]
      .filter(([, entry]) => entry.references === 0 && entry.source !== null)
      .sort((left, right) => left[1].lastUsed - right[1].lastUsed)[0];
    if (!candidate) return;
    const [key, entry] = candidate;
    entries.delete(key);
    totalBytes = Math.max(0, totalBytes - entry.sizeBytes);
    URL.revokeObjectURL(entry.source!);
  }
}
