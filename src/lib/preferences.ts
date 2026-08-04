import type { ConnectionMode } from "$lib/types";
import { writable } from "svelte/store";

const CONNECTION_MODE_KEY = "pixiv-client.connection-mode";
const SIDEBAR_KEY = "pixiv-client.sidebar";
const REDUCED_MOTION_KEY = "pixiv-client.reduced-motion";
const R18_DEFAULT_VISIBLE_KEY = "pixiv-client.r18-default-visible";
const UNSAFE_CONNECTION_WARNING_KEY = "pixiv-client.unsafe-connection-warning";
const INSECURE_MEDIA_WARNING_KEY = "pixiv-client.insecure-media-warning";

export const PREFERENCES_CHANGED_EVENT = "pixiv-client:preferences-changed";
export const r18DefaultVisible = writable<boolean>(false);

export type PreferredConnectionMode = Exclude<ConnectionMode, "compatible">;

export function readPreferredConnectionMode(): PreferredConnectionMode {
  if (typeof window === "undefined") return "standard";
  return localStorage.getItem(CONNECTION_MODE_KEY) === "ech" ? "ech" : "standard";
}

export function writePreferredConnectionMode(mode: ConnectionMode): void {
  if (typeof window === "undefined" || mode === "compatible") return;
  localStorage.setItem(CONNECTION_MODE_KEY, mode);
  notifyPreferencesChanged();
}

export function readUnsafeConnectionWarningSuppressed(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(UNSAFE_CONNECTION_WARNING_KEY) === "suppressed";
}

export function writeUnsafeConnectionWarningSuppressed(suppressed: boolean): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(
    UNSAFE_CONNECTION_WARNING_KEY,
    suppressed ? "suppressed" : "visible",
  );
  notifyPreferencesChanged();
}

export function readInsecureMediaWarningSuppressed(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(INSECURE_MEDIA_WARNING_KEY) === "suppressed";
}

export function writeInsecureMediaWarningSuppressed(suppressed: boolean): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(
    INSECURE_MEDIA_WARNING_KEY,
    suppressed ? "suppressed" : "visible",
  );
  notifyPreferencesChanged();
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
