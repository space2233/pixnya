import {
  READER_PREFS_KEY,
  VOLUME_PAGE_TURN_KEY,
  readNovelReaderPreferences,
  writeNovelReaderPreferences,
  readVolumePageTurnEnabled,
  writeVolumePageTurnEnabled,
  readDesktopSidebarExpanded,
  readR18DefaultVisible,
  readReducedMotion,
  writeDesktopSidebarExpanded,
  writeR18DefaultVisible,
  writeReducedMotion,
} from "./preferences.ts";
import {
  SEARCH_HISTORY_LIMIT_KEY,
  SEARCH_HISTORY_LIMIT_OPTIONS,
  readSearchHistory,
  readSearchHistoryLimit,
  normalizeSearchHistoryValue,
  searchHistoryLimitOrDefault,
  writeSearchHistoryLimit,
  type SearchHistoryLimit,
} from "./search-history.ts";

import { READING_SHELF_KEY, MAX_SHELF_ENTRIES, isReadingShelfEntry, readReadingShelf, writeReadingShelf, mergeReadingShelves, type ReadingShelfEntry } from "./reading-shelf.ts";

const SEARCH_HISTORY_KEY = "pixiv-client.search-history.v1";
const NOVEL_PROGRESS_PREFIX = "pixiv-client:novel-progress:";
const SIDEBAR_KEY = "pixiv-client.sidebar";
const REDUCED_MOTION_KEY = "pixiv-client.reduced-motion";
const R18_KEY = "pixiv-client.r18-default-visible";
const MAX_PROGRESS_ITEMS = 10_000;

export interface FrontendBackupState {
  readingShelf?: ReadingShelfEntry[];
  readerPreferences?: { fontSize: number; lineHeightTenths: number; theme: "paper" | "white" | "dark" };
  volumePageTurn?: boolean;
  searchHistory: string[];
  searchHistoryLimit?: SearchHistoryLimit;
  novelReadingProgress: Record<string, number>;
  sidebarExpanded: boolean;
  reducedMotion: boolean;
  r18DefaultVisible: boolean;
}

export function collectFrontendBackupState(): FrontendBackupState {
  const novelReadingProgress: Record<string, number> = {};
  let progressCount = 0;
  if (typeof localStorage !== "undefined") {
    for (let index = 0; index < localStorage.length; index += 1) {
      const key = localStorage.key(index);
      if (!key?.startsWith(NOVEL_PROGRESS_PREFIX)) continue;
      const novelId = key.slice(NOVEL_PROGRESS_PREFIX.length);
      if (!/^[1-9][0-9]{0,19}$/.test(novelId)) continue;
      const progress = Number(localStorage.getItem(key));
      if (!Number.isFinite(progress)) continue;
      novelReadingProgress[novelId] = Math.round(Math.min(1, Math.max(0, progress)) * 1_000_000);
      progressCount += 1;
      if (progressCount >= MAX_PROGRESS_ITEMS) break;
    }
  }
  const reader = readNovelReaderPreferences();
  return {
    readingShelf: readReadingShelf(),
    readerPreferences: { fontSize: reader.fontSize, lineHeightTenths: Math.round(reader.lineHeight * 10), theme: reader.theme },
    volumePageTurn: readVolumePageTurnEnabled(),
    searchHistory: readSearchHistory(),
    searchHistoryLimit: readSearchHistoryLimit(),
    novelReadingProgress,
    sidebarExpanded: readDesktopSidebarExpanded(),
    reducedMotion: readReducedMotion(),
    r18DefaultVisible: readR18DefaultVisible(),
  };
}

export function restoreFrontendBackupState(state: FrontendBackupState, strategy: "merge" | "replace" = "replace"): void {
  validateFrontendBackupState(state);
  if (typeof localStorage === "undefined") return;
  if (strategy === "merge") {
    state = mergeReadingBackup(collectFrontendBackupState(), state);
    validateFrontendBackupState(state);
  }
  const keys = backupKeys(state);
  const previous = new Map(keys.map((key) => [key, localStorage.getItem(key)]));
  try {
    for (const key of keys) localStorage.removeItem(key);
    writeSearchHistoryLimit(searchHistoryLimitOrDefault(state.searchHistoryLimit));
    localStorage.setItem(SEARCH_HISTORY_KEY, JSON.stringify(state.searchHistory));
    for (const [novelId, progress] of Object.entries(state.novelReadingProgress)) {
      localStorage.setItem(`${NOVEL_PROGRESS_PREFIX}${novelId}`, String(progress / 1_000_000));
    }
    if (state.readingShelf !== undefined) writeReadingShelf(state.readingShelf);
    if (state.readerPreferences !== undefined) {
      const { fontSize, lineHeightTenths, theme } = state.readerPreferences;
      writeNovelReaderPreferences({ fontSize, lineHeight: lineHeightTenths / 10, theme });
    }
    if (state.volumePageTurn !== undefined) writeVolumePageTurnEnabled(state.volumePageTurn);
    writeDesktopSidebarExpanded(state.sidebarExpanded);
    writeReducedMotion(state.reducedMotion);
    writeR18DefaultVisible(state.r18DefaultVisible);
  } catch (error) {
    for (const key of backupKeys(state)) localStorage.removeItem(key);
    for (const [key, value] of previous) {
      if (value !== null) localStorage.setItem(key, value);
    }
    throw error;
  }
}

function backupKeys(state: FrontendBackupState): string[] {
  const keys = [SEARCH_HISTORY_KEY, SEARCH_HISTORY_LIMIT_KEY, SIDEBAR_KEY, REDUCED_MOTION_KEY, R18_KEY];
  if (state.readingShelf !== undefined) keys.push(READING_SHELF_KEY);
  if (state.readerPreferences !== undefined) keys.push(READER_PREFS_KEY);
  if (state.volumePageTurn !== undefined) keys.push(VOLUME_PAGE_TURN_KEY);
  if (typeof localStorage === "undefined") return keys;
  for (let index = 0; index < localStorage.length; index += 1) {
    const key = localStorage.key(index);
    if (key?.startsWith(NOVEL_PROGRESS_PREFIX)) keys.push(key);
  }
  return keys;
}

function validateFrontendBackupState(state: FrontendBackupState): void {
  if (state.readingShelf !== undefined && (!Array.isArray(state.readingShelf)
    || state.readingShelf.length > MAX_SHELF_ENTRIES || !state.readingShelf.every(isReadingShelfEntry)
    || new Set(state.readingShelf.map((entry) => entry.id)).size !== state.readingShelf.length)) throw new Error("invalid_backup");
  if (state.readerPreferences !== undefined) {
    const prefs = state.readerPreferences;
    if (!prefs || !Number.isInteger(prefs.fontSize) || prefs.fontSize < 14 || prefs.fontSize > 28
      || !Number.isInteger(prefs.lineHeightTenths) || prefs.lineHeightTenths < 14 || prefs.lineHeightTenths > 24
      || !["paper", "white", "dark"].includes(prefs.theme)) throw new Error("invalid_backup");
  }
  if (state.volumePageTurn !== undefined && typeof state.volumePageTurn !== "boolean") throw new Error("invalid_backup");
  if (!state.novelReadingProgress || typeof state.novelReadingProgress !== "object" || Array.isArray(state.novelReadingProgress)) throw new Error("invalid_backup");
  if (!Array.isArray(state.searchHistory)) throw new Error("invalid_backup");
  const limit = searchHistoryLimitOrDefault(state.searchHistoryLimit);
  if (!SEARCH_HISTORY_LIMIT_OPTIONS.includes(limit) || (limit !== null && state.searchHistory.length > limit)) {
    throw new Error("invalid_backup");
  }
  if (Object.keys(state.novelReadingProgress).length > MAX_PROGRESS_ITEMS) throw new Error("invalid_backup");
  for (const [id, progress] of Object.entries(state.novelReadingProgress)) {
    if (!/^[1-9][0-9]{0,19}$/.test(id) || !Number.isInteger(progress) || progress < 0 || progress > 1_000_000) {
      throw new Error("invalid_backup");
    }
  }
  if (state.searchHistory.some((item) => typeof item !== "string" || normalizeSearchHistoryValue(item) !== item)) {
    throw new Error("invalid_backup");
  }
}

export function mergeReadingBackup(current: FrontendBackupState, incoming: FrontendBackupState): FrontendBackupState {
  const progress = { ...current.novelReadingProgress };
  const currentEntries = new Map((current.readingShelf ?? []).map((entry) => [entry.id, entry]));
  const incomingEntries = new Map((incoming.readingShelf ?? []).map((entry) => [entry.id, entry]));
  for (const [id, value] of Object.entries(incoming.novelReadingProgress)) {
    const local = currentEntries.get(id);
    const imported = incomingEntries.get(id);
    progress[id] = local && imported
      ? (local.lastReadAt > imported.lastReadAt ? progress[id] ?? value : value)
      : Math.max(progress[id] ?? 0, value);
  }
  return { ...incoming, novelReadingProgress: progress,
    readingShelf: mergeReadingShelves(current.readingShelf ?? [], incoming.readingShelf ?? []) };
}
