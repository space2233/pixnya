import type { ConnectionMode, SessionSnapshot } from "$lib/types";
import { writable } from "svelte/store";

const CONNECTION_MODE_KEY = "pixiv-client.connection-mode";
const SIDEBAR_KEY = "pixiv-client.sidebar";
const REDUCED_MOTION_KEY = "pixiv-client.reduced-motion";
const R18_DEFAULT_VISIBLE_KEY = "pixiv-client.r18-default-visible";

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

function notifyPreferencesChanged(): void {
  window.dispatchEvent(new CustomEvent(PREFERENCES_CHANGED_EVENT));
}
