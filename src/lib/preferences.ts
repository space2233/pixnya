import type { ConnectionMode, SessionSnapshot } from "$lib/types";
import { writable } from "svelte/store";

const CONNECTION_MODE_KEY = "pixiv-client.connection-mode";
const SIDEBAR_KEY = "pixiv-client.sidebar";
const REDUCED_MOTION_KEY = "pixiv-client.reduced-motion";
const R18_DEFAULT_VISIBLE_KEY = "pixiv-client.r18-default-visible";
const VOLUME_PAGE_TURN_KEY = "pixiv-client.volume-page-turn";
const READER_PREFS_KEY = "pixiv-client.reader-prefs.v1";

export const PREFERENCES_CHANGED_EVENT = "pixiv-client:preferences-changed";
export const r18DefaultVisible = writable<boolean>(false);

export type PreferredConnectionMode = ConnectionMode;

export function readPreferredConnectionMode(): PreferredConnectionMode | null {
  if (typeof window === "undefined") return null;
  const mode = localStorage.getItem(CONNECTION_MODE_KEY);
  return mode === "standard" || mode === "ech" || mode === "compatible" ? mode : null;
}

export function writePreferredConnectionMode(mode: ConnectionMode): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(CONNECTION_MODE_KEY, mode);
  notifyPreferencesChanged();
}

export function reconcilePreferredConnectionMode(
  snapshot: SessionSnapshot,
): PreferredConnectionMode | null {
  const preferred = readPreferredConnectionMode();
  const sessionMode = snapshot.loggedIn ? snapshot.connectionMode : null;
  if (!sessionMode) return preferred;
  if (preferred !== sessionMode) writePreferredConnectionMode(sessionMode);
  return sessionMode;
}

export function readDesktopSidebarExpanded(): boolean {
  if (typeof window === "undefined") return true;
  return localStorage.getItem(SIDEBAR_KEY) !== "hidden";
}

export function writeDesktopSidebarExpanded(expanded: boolean): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(SIDEBAR_KEY, expanded ? "visible" : "hidden");
  notifyPreferencesChanged();
}

export function readReducedMotion(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(REDUCED_MOTION_KEY) === "reduced";
}

export function writeReducedMotion(reduced: boolean): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(REDUCED_MOTION_KEY, reduced ? "reduced" : "system");
  applyReducedMotionPreference();
  notifyPreferencesChanged();
}

export function applyReducedMotionPreference(): void {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.reducedMotion = readReducedMotion() ? "true" : "false";
}

export function readR18DefaultVisible(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(R18_DEFAULT_VISIBLE_KEY) === "visible";
}

export function writeR18DefaultVisible(visible: boolean): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(R18_DEFAULT_VISIBLE_KEY, visible ? "visible" : "concealed");
  r18DefaultVisible.set(visible);
  notifyPreferencesChanged();
}

export function syncR18DefaultVisible(): void {
  r18DefaultVisible.set(readR18DefaultVisible());
}

export function readVolumePageTurnEnabled(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(VOLUME_PAGE_TURN_KEY) === "on";
}

export function writeVolumePageTurnEnabled(enabled: boolean): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(VOLUME_PAGE_TURN_KEY, enabled ? "on" : "off");
  notifyPreferencesChanged();
}

export type NovelReaderTheme = "paper" | "white" | "dark";

export type NovelReaderPreferences = {
  fontSize: number;
  lineHeight: number;
  theme: NovelReaderTheme;
};

const DEFAULT_READER_PREFERENCES: NovelReaderPreferences = {
  fontSize: 18,
  lineHeight: 1.9,
  theme: "paper",
};

function clampReaderFontSize(value: number): number {
  return Math.min(28, Math.max(14, Math.round(value)));
}

function clampReaderLineHeight(value: number): number {
  return Math.min(2.4, Math.max(1.4, Math.round(value * 10) / 10));
}

function isReaderTheme(value: unknown): value is NovelReaderTheme {
  return value === "paper" || value === "white" || value === "dark";
}

export function readNovelReaderPreferences(): NovelReaderPreferences {
  if (typeof window === "undefined") return { ...DEFAULT_READER_PREFERENCES };
  try {
    const stored = JSON.parse(localStorage.getItem(READER_PREFS_KEY) ?? "null") as Partial<NovelReaderPreferences> | null;
    if (!stored || typeof stored !== "object") return { ...DEFAULT_READER_PREFERENCES };
    return {
      fontSize: typeof stored.fontSize === "number" && Number.isFinite(stored.fontSize)
        ? clampReaderFontSize(stored.fontSize)
        : DEFAULT_READER_PREFERENCES.fontSize,
      lineHeight: typeof stored.lineHeight === "number" && Number.isFinite(stored.lineHeight)
        ? clampReaderLineHeight(stored.lineHeight)
        : DEFAULT_READER_PREFERENCES.lineHeight,
      theme: isReaderTheme(stored.theme) ? stored.theme : DEFAULT_READER_PREFERENCES.theme,
    };
  } catch {
    return { ...DEFAULT_READER_PREFERENCES };
  }
}

export function writeNovelReaderPreferences(patch: Partial<NovelReaderPreferences>): NovelReaderPreferences {
  const current = readNovelReaderPreferences();
  const next: NovelReaderPreferences = {
    fontSize: clampReaderFontSize(typeof patch.fontSize === "number" ? patch.fontSize : current.fontSize),
    lineHeight: clampReaderLineHeight(typeof patch.lineHeight === "number" ? patch.lineHeight : current.lineHeight),
    theme: isReaderTheme(patch.theme) ? patch.theme : current.theme,
  };
  if (typeof window !== "undefined") {
    localStorage.setItem(READER_PREFS_KEY, JSON.stringify(next));
    notifyPreferencesChanged();
  }
  return next;
}

function notifyPreferencesChanged(): void {
  window.dispatchEvent(new CustomEvent(PREFERENCES_CHANGED_EVENT));
}
