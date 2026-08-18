<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import { checkForUpdates, downloadUpdate, getUpdateSnapshot, installUpdate, saveUpdatePreferences } from "$lib/updates";
  import type { UpdateSnapshot } from "$lib/types";

  let snapshot = $state<UpdateSnapshot | null>(null);
  let busy = $state(false);
  let notice = $state("");
  let error = $state(false);
  onMount(() => { void getUpdateSnapshot().then((value) => (snapshot = value)).catch(() => {}); });
  async function updatePreferences(key: "autoCheck" | "autoDownload" | "unmeteredOnly") {
    if (!snapshot) return;
    await act(() => saveUpdatePreferences({ ...snapshot!.preferences, [key]: !snapshot!.preferences[key] }));
  }
  async function act(action: () => Promise<UpdateSnapshot>) {
    if (busy) return; busy = true; notice = ""; error = false;
    try { snapshot = await action(); } catch { error = true; notice = m.settings_update_check_failed(); } finally { busy = false; }
  }
  function phase(): string {
    if (!snapshot) return m.settings_update_phase_reading();
    const labels: Record<string, () => string> = {
      checking:m.settings_update_phase_checking, available:() => m.settings_update_phase_found({ version: snapshot?.available?.version ?? "" }),
      downloading:m.settings_update_phase_downloading, ready_to_install:m.settings_update_phase_ready,
      installing:m.settings_update_phase_installing, awaiting_system_action:m.settings_update_phase_confirmation,
      up_to_date:m.settings_update_phase_latest, not_configured:m.settings_update_phase_unconfigured,
      failed:m.settings_update_phase_failed, idle:m.settings_update_phase_channel,
    };
    return labels[snapshot.phase]?.() ?? snapshot.phase;
  }
</script>

<svelte:head><title>{m.settings_updates()} · PixNya</title></svelte:head>
<AppShell title={m.settings_updates()}><div class="page"><ReturnLink fallback="/settings" label={m.common_back()} /><h1 class="page-title">{m.settings_updates()}</h1><section>
  <div class="row"><strong>{phase()}</strong><button type="button" disabled={!snapshot || busy} onclick={() => act(() => checkForUpdates("manual"))}>{m.settings_check_now()}</button></div>
  <label class="row"><strong>{m.settings_auto_check()}</strong><input type="checkbox" role="switch" checked={snapshot?.preferences.autoCheck ?? true} disabled={!snapshot || busy} onchange={() => updatePreferences("autoCheck")} /></label>
  <label class="row"><strong>{m.settings_auto_download()}</strong><input type="checkbox" role="switch" checked={snapshot?.preferences.autoDownload ?? false} disabled={!snapshot || busy} onchange={() => updatePreferences("autoDownload")} /></label>
  {#if snapshot?.installer === "android_system"}<label class="row"><strong>{m.settings_unmetered()}</strong><input type="checkbox" role="switch" checked={snapshot.preferences.unmeteredOnly} disabled={busy} onchange={() => updatePreferences("unmeteredOnly")} /></label>{/if}
  {#if snapshot?.available}<div class="release"><strong>PixNya {snapshot.available.version}</strong>{#if snapshot.available.notes}<p>{snapshot.available.notes}</p>{/if}<footer>{#if snapshot.phase === "available" || snapshot.phase === "failed"}<button disabled={busy} onclick={() => act(downloadUpdate)}>{m.settings_download_verify()}</button>{/if}{#if snapshot.readyToInstall}<button disabled={busy} onclick={() => act(installUpdate)}>{snapshot.installer === "android_system" ? m.settings_open_installer() : m.settings_install_update()}</button>{/if}</footer></div>{/if}
</section>{#if notice}<p class:error role="status">{notice}</p>{/if}</div></AppShell>
<style>.page{width:min(720px,100%);box-sizing:border-box;margin:auto;padding:30px 24px 60px}h1{margin:24px 0;font-size:var(--type-title)}section{overflow:hidden;border:1px solid var(--line);border-radius:18px;background:white}.row{display:flex;min-height:62px;align-items:center;justify-content:space-between;gap:18px;padding:0 18px;border-bottom:1px solid var(--line)}button{padding:8px 14px;border:1px solid #cde7f8;border-radius:18px;background:white;color:var(--pixiv-blue)}input{accent-color:var(--pixiv-blue)}.release{padding:18px}.release p{white-space:pre-wrap}.release footer{display:flex;gap:10px}.error{color:var(--danger)}</style>
