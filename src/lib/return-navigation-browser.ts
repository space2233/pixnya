import { goto } from "$app/navigation";
import {
  createReturnNavigator,
  type NavigationLocation,
  type ReturnNavigationAdapter,
} from "$lib/return-navigation";

const STACK_KEY = "pixnya:return-navigation:stack:v1";
const PENDING_KEY = "pixnya:return-navigation:pending:v1";
const NAVIGATION_INDEX_KEY = "sveltekit:navigation";

function currentUrl(): string {
  return `${window.location.pathname}${window.location.search}${window.location.hash}`;
}

function currentNavigationIndex(): number | null {
  const value = window.history.state?.[NAVIGATION_INDEX_KEY];
  return Number.isInteger(value) ? value : null;
}

function readJson(key: string): unknown {
  try {
    const value = window.sessionStorage.getItem(key);
    return value ? JSON.parse(value) : null;
  } catch {
    return null;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    if (value === null) window.sessionStorage.removeItem(key);
    else window.sessionStorage.setItem(key, JSON.stringify(value));
  } catch {
    // A denied sessionStorage must not make navigation unusable.
  }
}

function restoreScrollWhenReady(x: number, y: number): void {
  const startedAt = performance.now();
  const attempt = () => {
    const maximumY = Math.max(0, document.documentElement.scrollHeight - window.innerHeight);
    window.scrollTo(x, Math.min(y, maximumY));
    const reachedTarget = maximumY >= y && Math.abs(window.scrollY - y) <= 2;
    if (!reachedTarget && performance.now() - startedAt < 4_000) {
      window.requestAnimationFrame(attempt);
    }
  };
  window.requestAnimationFrame(attempt);
}

function browserAdapter(): ReturnNavigationAdapter {
  return {
    current: (): NavigationLocation => ({
      url: currentUrl(),
      navigationIndex: currentNavigationIndex(),
      scrollX: window.scrollX,
      scrollY: window.scrollY,
    }),
    readStack: () => readJson(STACK_KEY),
    writeStack: (entries) => writeJson(STACK_KEY, entries),
    readPending: () => readJson(PENDING_KEY),
    writePending: (pending) => writeJson(PENDING_KEY, pending),
    historyBack: () => window.history.back(),
    replaceWithFallback: (url) => void goto(url, { replaceState: true }),
    restoreScroll: restoreScrollWhenReady,
    now: () => Date.now(),
  };
}

function navigator() {
  return createReturnNavigator(browserAdapter());
}

export function captureReturnNavigation(event: MouseEvent): void {
  if (event.defaultPrevented || event.button !== 0
    || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
  const target = event.target;
  if (!(target instanceof Element)) return;
  const anchor = target.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement)
    || anchor.hasAttribute("download")
    || (anchor.target && anchor.target !== "_self")
    || anchor.dataset.noReturnCapture === "true") return;

  const destination = new URL(anchor.href, window.location.href);
  if (destination.origin !== window.location.origin) return;
  navigator().capture(`${destination.pathname}${destination.search}${destination.hash}`);
}

export function returnToPreviousLocation(fallback: string): "history" | "fallback" {
  return navigator().returnToPrevious(fallback);
}

export function restorePendingReturnPosition(): boolean {
  return navigator().restorePendingPosition();
}

export function restoreReturnAfterHistoryPop(previousUrl: string): boolean {
  return navigator().restoreAfterHistoryPop(previousUrl);
}
