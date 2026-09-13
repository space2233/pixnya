import { invoke } from "@tauri-apps/api/core";

export const VOLUME_PAGE_EVENT = "pixnya-volume-page";

export type VolumePageDirection = "next" | "previous";

export function volumePageDirectionFromKey(event: KeyboardEvent): VolumePageDirection | null {
  if (event.key === "AudioVolumeDown" || event.code === "VolumeDown") return "next";
  if (event.key === "AudioVolumeUp" || event.code === "VolumeUp") return "previous";
  return null;
}

export function volumePageDirectionFromDetail(value: unknown): VolumePageDirection | null {
  if (value === "next" || value === "previous") return value;
  if (value && typeof value === "object" && "direction" in value) {
    const direction = (value as { direction?: unknown }).direction;
    if (direction === "next" || direction === "previous") return direction;
  }
  return null;
}

export function isVolumePageTurnSupported(
  userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent,
): boolean {
  return /Android/i.test(userAgent);
}

export async function setVolumePageTurnCapture(enabled: boolean): Promise<void> {
  try {
    await invoke("set_volume_page_turn_capture", { enabled });
  } catch {
    // Desktop and older Android builds have no volume-key capture plugin.
  }
}
