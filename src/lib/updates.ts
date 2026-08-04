import { invoke } from "@tauri-apps/api/core";
import type { UpdatePreferences, UpdateSnapshot } from "$lib/types";

export type UpdateTrigger = "startup" | "scheduled" | "manual";

export function getUpdateSnapshot(): Promise<UpdateSnapshot> {
  return invoke<UpdateSnapshot>("get_update_snapshot");
}

export function saveUpdatePreferences(
  preferences: UpdatePreferences,
): Promise<UpdateSnapshot> {
  return invoke<UpdateSnapshot>("set_update_preferences", { preferences });
}

export function checkForUpdates(trigger: UpdateTrigger = "manual"): Promise<UpdateSnapshot> {
  return invoke<UpdateSnapshot>("check_for_updates", { trigger });
}

export function downloadUpdate(): Promise<UpdateSnapshot> {
  return invoke<UpdateSnapshot>("download_update");
}

export function installUpdate(): Promise<UpdateSnapshot> {
  return invoke<UpdateSnapshot>("install_update");
}

export function cancelUpdate(): Promise<UpdateSnapshot> {
  return invoke<UpdateSnapshot>("cancel_update");
}
