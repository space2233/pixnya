import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writable } from "svelte/store";
import { reconcilePreferredConnectionMode } from "$lib/preferences";
import type { ConnectionMode, SessionSnapshot } from "$lib/types";

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
      applySessionSnapshot(payload);
    });
    const snapshot = await invoke<SessionSnapshot>("restore_session");
    applySessionSnapshot(snapshot);
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
  reconcilePreferredConnectionMode(snapshot);
  session.set(snapshot);
}

export async function logoutSession(): Promise<SessionSnapshot> {
  const snapshot = await invoke<SessionSnapshot>("logout");
  applySessionSnapshot(snapshot);
  return snapshot;
}

export async function switchSessionConnectionMode(mode: ConnectionMode): Promise<SessionSnapshot> {
  const snapshot = await invoke<SessionSnapshot>("switch_connection_mode", { mode });
  applySessionSnapshot(snapshot);
  return snapshot;
}

export function disposeSessionListenerForTests(): void {
  unlisten?.();
  unlisten = null;
  initialization = null;
  session.set(loggedOut);
  sessionRestoring.set(true);
}
