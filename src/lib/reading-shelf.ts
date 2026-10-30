import type { NovelSummary } from "./types";

export const READING_SHELF_KEY = "pixiv-client.reading-shelf.v1";
export const READING_SHELF_CHANGED_EVENT = "pixiv-client:reading-shelf-changed";
export const NOVEL_PROGRESS_PREFIX = "pixiv-client:novel-progress:";
export const MAX_SHELF_ENTRIES = 500;
export type ReadingStatus = "planned" | "reading" | "finished";
export interface ReadingShelfEntry {
  id: string;
  title: string;
  author: string;
  seriesId: string | null;
  seriesTitle: string | null;
  xRestrict: number;
  status: ReadingStatus;
  updatedAt: number;
  lastReadAt: number;
}

export function validNovelId(id: unknown): id is string {
  return typeof id === "string" && /^[1-9][0-9]{0,19}$/.test(id);
}

export function isReadingShelfEntry(value: unknown): value is ReadingShelfEntry {
  if (!value || typeof value !== "object") return false;
  const entry = value as ReadingShelfEntry;
  return validNovelId(entry.id)
    && typeof entry.title === "string" && entry.title.length <= 1000
    && typeof entry.author === "string" && entry.author.length <= 1000
    && (entry.seriesId === null || validNovelId(entry.seriesId))
    && (entry.seriesTitle === null || (typeof entry.seriesTitle === "string" && entry.seriesTitle.length <= 1000))
    && Number.isInteger(entry.xRestrict) && entry.xRestrict >= 0 && entry.xRestrict <= 2
    && ["planned", "reading", "finished"].includes(entry.status)
    && Number.isSafeInteger(entry.updatedAt) && entry.updatedAt >= 0
    && Number.isSafeInteger(entry.lastReadAt) && entry.lastReadAt >= 0;
}

export function readReadingShelf(): ReadingShelfEntry[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const entries: unknown = JSON.parse(localStorage.getItem(READING_SHELF_KEY) ?? "[]");
    return Array.isArray(entries) ? mergeReadingShelves([], entries.filter(isReadingShelfEntry)) : [];
  } catch { return []; }
}

export function mergeReadingShelves(current: ReadingShelfEntry[], incoming: ReadingShelfEntry[]): ReadingShelfEntry[] {
  const entries = new Map<string, ReadingShelfEntry>();
  for (const entry of [...current, ...incoming]) {
    const previous = entries.get(entry.id);
    if (!previous) {
      entries.set(entry.id, entry);
      continue;
    }
    // Status/metadata edits and reading sessions have independent clocks.
    // Keep the reading clock paired with the progress selected during restore.
    const latest = entry.updatedAt >= previous.updatedAt ? entry : previous;
    entries.set(entry.id, { ...latest, lastReadAt: Math.max(entry.lastReadAt, previous.lastReadAt) });
  }
  return [...entries.values()].sort((a, b) => b.updatedAt - a.updatedAt).slice(0, MAX_SHELF_ENTRIES);
}

export function writeReadingShelf(entries: ReadingShelfEntry[]): void {
  if (typeof localStorage === "undefined") return;
  if (entries.length > MAX_SHELF_ENTRIES || !entries.every(isReadingShelfEntry)) throw new Error("invalid_reading_shelf");
  localStorage.setItem(READING_SHELF_KEY, JSON.stringify(entries));
  notifyReadingShelf();
}

export function readNovelProgress(id: string): number {
  if (typeof localStorage === "undefined" || !validNovelId(id)) return 0;
  const value = Number(localStorage.getItem(`${NOVEL_PROGRESS_PREFIX}${id}`) ?? "0");
  return Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0;
}

export function saveNovelProgress(id: string, progress: number): void {
  if (typeof localStorage === "undefined" || !validNovelId(id) || !Number.isFinite(progress)) return;
  localStorage.setItem(`${NOVEL_PROGRESS_PREFIX}${id}`, String(Math.min(1, Math.max(0, progress))));
  const now = Date.now();
  writeReadingShelf(readReadingShelf().map((entry) => entry.id === id
    ? { ...entry, status: progress >= 1 ? "finished" : "reading", updatedAt: now, lastReadAt: now }
    : entry));
}

export function addNovelToShelf(novel: NovelSummary, status: ReadingStatus = "planned"): void {
  const current = readReadingShelf();
  const previous = current.find((entry) => entry.id === novel.id);
  const now = Date.now();
  const entry: ReadingShelfEntry = {
    id: novel.id, title: novel.title.slice(0, 1000), author: novel.author.name.slice(0, 1000),
    seriesId: novel.series?.id ?? null, seriesTitle: novel.series?.title?.slice(0, 1000) ?? null,
    xRestrict: novel.xRestrict, status,
    updatedAt: now, lastReadAt: status === "reading" ? now : previous?.lastReadAt ?? 0,
  };
  writeReadingShelf(mergeReadingShelves(current, [entry]));
}

export function setReadingStatus(id: string, status: ReadingStatus): void {
  writeReadingShelf(readReadingShelf().map((entry) => entry.id === id ? { ...entry, status, updatedAt: Date.now() } : entry));
}

export function removeFromReadingShelf(id: string): void {
  writeReadingShelf(readReadingShelf().filter((entry) => entry.id !== id));
}

export function recentReading(entries: ReadingShelfEntry[], limit = 5): ReadingShelfEntry[] {
  return entries.filter((entry) => entry.status === "reading" && entry.lastReadAt > 0)
    .sort((a, b) => b.lastReadAt - a.lastReadAt).slice(0, limit);
}

export function lastReadInSeries(entries: ReadingShelfEntry[], seriesId: string): ReadingShelfEntry | undefined {
  return entries.filter((entry) => entry.seriesId === seriesId && entry.lastReadAt > 0)
    .sort((a, b) => b.lastReadAt - a.lastReadAt)[0];
}

function notifyReadingShelf() {
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent(READING_SHELF_CHANGED_EVENT));
}
