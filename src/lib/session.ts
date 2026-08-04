import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writable } from "svelte/store";
import type { SessionSnapshot } from "$lib/types";

const loggedOut: SessionSnapshot = { loggedIn: false };

export const session = writable<SessionSnapshot>(loggedOut);
export const sessionRestoring = writable<boolean>(true);

let initialization: Promise<SessionSnapshot> | null = null;
let unlisten: UnlistenFn | null = null;

export function initializeSession(): Promise<SessionSnapshot> {
  if (initialization) return initialization;
  sessionRestoring.set(true);
  initialization = initialize();
  return initialization;
}

async function initialize(): Promise<SessionSnapshot> {
  try {
    unlisten = await listen<SessionSnapshot>("pixiv-session-changed", ({ payload }) => {
      session.set(payload);
    });
    const snapshot = await invoke<SessionSnapshot>("restore_session");
    session.set(snapshot);
    return snapshot;
  } catch (error) {
    unlisten?.();
    unlisten = null;
    session.set(loggedOut);
    initialization = null;
    throw error;
  } finally {
    sessionRestoring.set(false);
  }
}

export function applySessionSnapshot(snapshot: SessionSnapshot): void {
  session.set(snapshot);
}

export async function logoutSession(): Promise<SessionSnapshot> {
  const snapshot = await invoke<SessionSnapshot>("logout");
  session.set(snapshot);
  return snapshot;
}

export function disposeSessionListenerForTests(): void {
  unlisten?.();
  unlisten = null;
  initialization = null;
  session.set(loggedOut);
  sessionRestoring.set(true);
}
