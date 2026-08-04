const MAX_VIEW_SNAPSHOTS = 64;

let nextSnapshotId = 1;
const snapshots = new Map<string, unknown>();

export function rememberNavigationView<T>(value: T): string {
  const key = `view-${nextSnapshotId++}`;
  snapshots.set(key, value);
  while (snapshots.size > MAX_VIEW_SNAPSHOTS) {
    const oldest = snapshots.keys().next().value;
    if (typeof oldest !== "string") break;
    snapshots.delete(oldest);
  }
  return key;
}

export function recallNavigationView<T>(key: unknown): T | null {
  if (typeof key !== "string") return null;
  return (snapshots.get(key) as T | undefined) ?? null;
}
