export interface ProfileMediaSnapshot {
  avatarUrl: string | null;
  backgroundImageUrl: string | null;
}

const snapshots = new Map<string, ProfileMediaSnapshot>();

export function profileMediaSnapshotKey(
  accountId: string,
  mode: ConnectionMode | null | undefined,
): string {
  const scope = mode === "compatible" ? "insecure" : "verified";
  return `${accountId.trim()}:${scope}`;
}

export function readProfileMediaSnapshot(key: string): ProfileMediaSnapshot | null {
  const snapshot = snapshots.get(key.trim());
  return snapshot ? { ...snapshot } : null;
}

export function writeProfileMediaSnapshot(
  key: string,
  snapshot: ProfileMediaSnapshot,
): void {
  const normalizedKey = key.trim();
  if (!normalizedKey) return;
  snapshots.set(normalizedKey, { ...snapshot });
}

export function clearProfileMediaSnapshots(): void {
  snapshots.clear();
}
import type { ConnectionMode } from "./types.ts";
