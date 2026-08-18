import type { StorageStatus } from "$lib/types";

type StorageStatusSummary = Pick<
  StorageStatus,
  "offlineBytes" | "cacheBytes" | "dataAvailableBytes"
>;

export type StorageStatusLoadState =
  | { kind: "loading" }
  | { kind: "error" }
  | { kind: "ready"; value: StorageStatusSummary };

export type StorageMetric = "usage" | "available";

export function storageMetric(
  state: StorageStatusLoadState,
  metric: StorageMetric,
): { kind: "loading" } | { kind: "error" } | { kind: "value"; bytes: number } {
  if (state.kind !== "ready") return state;
  return {
    kind: "value",
    bytes: metric === "usage"
      ? state.value.offlineBytes + state.value.cacheBytes
      : state.value.dataAvailableBytes,
  };
}
