import { getIllustrationSeries } from "$lib/pixiv-api";
import type {
  IllustrationSeriesDetail,
  IllustrationSeriesPage,
  IllustrationSummary,
} from "$lib/types";

interface SeriesCacheEntry {
  initialized: boolean;
  series?: IllustrationSeriesDetail;
  illustrations: IllustrationSummary[];
  nextCursor?: string | null;
  complete: boolean;
  pending?: Promise<void>;
}

export interface ArtworkSeriesNavigation {
  previous?: IllustrationSummary;
  next?: IllustrationSummary;
  position: number;
  total: number;
  complete: boolean;
}

const cache = new Map<string, SeriesCacheEntry>();
const MAX_LOOKUP_PAGES = 64;

function entryFor(seriesId: string): SeriesCacheEntry {
  let entry = cache.get(seriesId);
  if (!entry) {
    entry = { initialized: false, illustrations: [], complete: false };
    cache.set(seriesId, entry);
  }
  return entry;
}

function mergePage(entry: SeriesCacheEntry, page: IllustrationSeriesPage, reset: boolean): void {
  if (reset) entry.illustrations = [];
  const known = new Set(entry.illustrations.map((item) => item.id));
  const incoming = entry.initialized && !reset
    ? page.illustrations
    : [page.firstIllustration, ...page.illustrations];
  entry.illustrations = [
    ...entry.illustrations,
    ...incoming.filter((item) => !known.has(item.id) && known.add(item.id)),
  ];
  entry.initialized = true;
  entry.series = page.series;
  entry.nextCursor = page.nextCursor ?? null;
  entry.complete = !entry.nextCursor;
}

export function rememberArtworkSeriesPage(
  page: IllustrationSeriesPage,
  reset = false,
): void {
  mergePage(entryFor(page.series.id), page, reset);
}

async function fetchNextPage(seriesId: string, entry: SeriesCacheEntry): Promise<void> {
  if (entry.pending) return entry.pending;
  if (entry.initialized && !entry.nextCursor) {
    entry.complete = true;
    return;
  }
  const cursor = entry.initialized ? (entry.nextCursor ?? undefined) : undefined;
  entry.pending = getIllustrationSeries(seriesId, cursor)
    .then((page) => {
      if (page.series.id !== seriesId) throw new Error("series-id-mismatch");
      mergePage(entry, page, !entry.initialized);
    })
    .finally(() => {
      entry.pending = undefined;
    });
  return entry.pending;
}

export async function resolveArtworkSeriesNavigation(
  seriesId: string,
  illustrationId: string,
): Promise<ArtworkSeriesNavigation | null> {
  const entry = entryFor(seriesId);
  for (let pageCount = 0; pageCount < MAX_LOOKUP_PAGES; pageCount += 1) {
    const index = entry.illustrations.findIndex((item) => item.id === illustrationId);
    if (index >= 0) {
      if (index === entry.illustrations.length - 1 && !entry.complete) {
        await fetchNextPage(seriesId, entry);
        continue;
      }
      return {
        previous: index > 0 ? entry.illustrations[index - 1] : undefined,
        next: entry.illustrations[index + 1],
        position: index + 1,
        total: entry.series?.workCount ?? entry.illustrations.length,
        complete: entry.complete,
      };
    }
    if (entry.complete) return null;
    await fetchNextPage(seriesId, entry);
  }
  return null;
}
