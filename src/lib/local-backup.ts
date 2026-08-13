import {
  readDesktopSidebarExpanded,
  readR18DefaultVisible,
  readReducedMotion,
  writeDesktopSidebarExpanded,
  writeR18DefaultVisible,
  writeReducedMotion,
} from "$lib/preferences";
import { readSearchHistory } from "$lib/search-history";

const SEARCH_HISTORY_KEY = "pixiv-client.search-history.v1";
const NOVEL_PROGRESS_PREFIX = "pixiv-client:novel-progress:";
const SIDEBAR_KEY = "pixiv-client.sidebar";
const REDUCED_MOTION_KEY = "pixiv-client.reduced-motion";
const R18_KEY = "pixiv-client.r18-default-visible";
const MAX_PROGRESS_ITEMS = 10_000;

export interface FrontendBackupState {
  searchHistory: string[];
  novelReadingProgress: Record<string, number>;
  sidebarExpanded: boolean;
  reducedMotion: boolean;
  r18DefaultVisible: boolean;
}

export function collectFrontendBackupState(): FrontendBackupState {
  const novelReadingProgress: Record<string, number> = {};
  if (typeof localStorage !== "undefined") {
    for (let index = 0; index < localStorage.length; index += 1) {
      const key = localStorage.key(index);
      if (!key?.startsWith(NOVEL_PROGRESS_PREFIX)) continue;
      const novelId = key.slice(NOVEL_PROGRESS_PREFIX.length);
      if (!/^[1-9][0-9]{0,19}$/.test(novelId)) continue;
      const progress = Number(localStorage.getItem(key));
      if (!Number.isFinite(progress)) continue;
      novelReadingProgress[novelId] = Math.round(Math.min(1, Math.max(0, progress)) * 1_000_000);
      if (Object.keys(novelReadingProgress).length >= MAX_PROGRESS_ITEMS) break;
    }
  }
  return {
    searchHistory: readSearchHistory(),
    novelReadingProgress,
    sidebarExpanded: readDesktopSidebarExpanded(),
    reducedMotion: readReducedMotion(),
    r18DefaultVisible: readR18DefaultVisible(),
  };
}

export function restoreFrontendBackupState(state: FrontendBackupState): void {
  validateFrontendBackupState(state);
  if (typeof localStorage === "undefined") return;
  const keys = backupKeys();
  const previous = new Map(keys.map((key) => [key, localStorage.getItem(key)]));
  try {
    for (const key of keys) localStorage.removeItem(key);
    localStorage.setItem(SEARCH_HISTORY_KEY, JSON.stringify(state.searchHistory));
    for (const [novelId, progress] of Object.entries(state.novelReadingProgress)) {
      localStorage.setItem(`${NOVEL_PROGRESS_PREFIX}${novelId}`, String(progress / 1_000_000));
    }
    writeDesktopSidebarExpanded(state.sidebarExpanded);
    writeReducedMotion(state.reducedMotion);
    writeR18DefaultVisible(state.r18DefaultVisible);
  } catch (error) {
    for (const key of backupKeys()) localStorage.removeItem(key);
    for (const [key, value] of previous) {
      if (value !== null) localStorage.setItem(key, value);
    }
    throw error;
  }
}

function backupKeys(): string[] {
  const keys = [SEARCH_HISTORY_KEY, SIDEBAR_KEY, REDUCED_MOTION_KEY, R18_KEY];
  if (typeof localStorage === "undefined") return keys;
  for (let index = 0; index < localStorage.length; index += 1) {
    const key = localStorage.key(index);
    if (key?.startsWith(NOVEL_PROGRESS_PREFIX)) keys.push(key);
  }
  return keys;
}

function validateFrontendBackupState(state: FrontendBackupState): void {
  if (!Array.isArray(state.searchHistory) || state.searchHistory.length > 8) throw new Error("invalid_backup");
  if (Object.keys(state.novelReadingProgress).length > MAX_PROGRESS_ITEMS) throw new Error("invalid_backup");
  for (const [id, progress] of Object.entries(state.novelReadingProgress)) {
    if (!/^[1-9][0-9]{0,19}$/.test(id) || !Number.isInteger(progress) || progress < 0 || progress > 1_000_000) {
      throw new Error("invalid_backup");
    }
  }
  if (state.searchHistory.some((item) => typeof item !== "string" || !item.trim() || item.length > 512)) {
    throw new Error("invalid_backup");
  }
}
