<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import { storageMetric, type StorageMetric, type StorageStatusLoadState } from "$lib/storage-status-view";
  import {
    clearExportDestination,
    clearMediaCache,
    getExportDestinationStatus,
    getMediaCacheStats,
    getStorageStatus,
    selectExportDestination,
    setAutoExportDownloads,
    setMediaCacheLimit,
  } from "$lib/pixiv-api";
  import type { ExportDestinationStatus, MediaCacheStats, StorageStatus } from "$lib/types";

  const limits = [
    ["134217728", "128 MiB"], ["268435456", "256 MiB"], ["536870912", "512 MiB"],
    ["1073741824", "1 GiB"], ["5368709120", "5 GiB"], ["10737418240", "10 GiB"],
    ["unlimited", m.settings_current_value_unlimited()],
  ] as const;
  let storage = $state<StorageStatus | null>(null);
  let storageLoadState = $state<StorageStatusLoadState>({ kind: "loading" });
  let cache = $state<MediaCacheStats | null>(null);
  let destination = $state<ExportDestinationStatus | null>(null);
  let busy = $state(false);
  let notice = $state("");
  let error = $state(false);
  let confirmCacheClear = $state(false);

  onMount(() => { void reload(); });
  async function reload() {
    const [nextStorage, nextCache, nextDestination] = await Promise.allSettled([
      getStorageStatus(), getMediaCacheStats(), getExportDestinationStatus(),
    ]);
    if (nextStorage.status === "fulfilled") {
      storage = nextStorage.value;
      storageLoadState = { kind: "ready", value: nextStorage.value };
    } else {
      storage = null;
      storageLoadState = { kind: "error" };
    }
    cache = nextCache.status === "fulfilled" ? nextCache.value : null;
    destination = nextDestination.status === "fulfilled" ? nextDestination.value : null;
  }
  async function changeLimit(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    await act(async () => {
      storage = await setMediaCacheLimit(value === "unlimited" ? null : Number(value));
      storageLoadState = { kind: "ready", value: storage };
      cache = await getMediaCacheStats();
    });
  }
  async function clearCacheNow() {
    await act(async () => {
      cache = await clearMediaCache();
      storage = await getStorageStatus();
      storageLoadState = { kind: "ready", value: storage };
      confirmCacheClear = false;
    });
  }
  async function chooseDestination() {
    await act(async () => { destination = (await selectExportDestination(m.settings_export_directory())).status; });
  }
  async function removeDestination() { await act(async () => { destination = await clearExportDestination(); }); }
  async function toggleAutoExport() {
    if (!destination) return;
    await act(async () => { destination = await setAutoExportDownloads(!destination!.autoExport); });
  }
  async function act(action: () => Promise<void>) {
    if (busy) return; busy = true; notice = ""; error = false;
    try { await action(); } catch { error = true; notice = m.settings_cache_limit_failed(); } finally { busy = false; }
  }
  function bytes(value: number): string {
    if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KiB`;
    if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
    return `${(value / 1024 ** 3).toFixed(2)} GiB`;
  }
  function metricText(metric: StorageMetric): string {
    const result = storageMetric(storageLoadState, metric);
    if (result.kind === "loading") return m.settings_reading();
    if (result.kind === "error") return m.settings_read_failed();
    return bytes(result.bytes);
  }
</script>

<svelte:head><title>{m.settings_storage()} · PixNya</title></svelte:head>
<AppShell title={m.settings_storage()}>
  <div class="page">
    <ReturnLink fallback="/settings" label={m.common_back()} />
    <h1 class="page-title">{m.settings_storage()}</h1>
    <section>
      <div class="row"><strong>{m.settings_space_usage()}</strong><span>{metricText("usage")}</span></div>
      <div class="row"><strong>{m.settings_available_space()}</strong><span>{metricText("available")}</span></div>
      <label class="row"><strong>{m.settings_cache_limit()}</strong><select disabled={!storage || busy} value={storage ? (storage.cacheLimitBytes === null ? "unlimited" : String(storage.cacheLimitBytes)) : "268435456"} onchange={changeLimit}>{#each limits as limit}<option value={limit[0]}>{limit[1]}</option>{/each}</select></label>
      <div class="row"><strong>{m.settings_media_cache()}</strong><span>{cache ? `${cache.entryCount} · ${bytes(cache.sizeBytes)}` : m.settings_reading()}</span><button type="button" disabled={!cache || busy} onclick={() => (confirmCacheClear = true)}>{m.settings_clear_cache()}</button></div>
      <div class="row"><strong>{m.settings_export_directory()}</strong><span>{destination?.label ?? "—"}</span><button type="button" disabled={busy} onclick={chooseDestination}>{destination?.configured ? m.settings_change() : m.settings_choose_directory()}</button>{#if destination?.configured}<button type="button" disabled={busy} onclick={removeDestination}>{m.settings_revoke()}</button>{/if}</div>
      <label class="row"><strong>{m.settings_auto_export()}</strong><input type="checkbox" role="switch" checked={destination?.autoExport ?? false} disabled={!destination || busy} onchange={toggleAutoExport} /></label>
      <a class="row" href="/offline"><strong>{m.settings_offline_queue()}</strong><i>›</i></a>
    </section>
    {#if notice}<p class:error role="status">{notice}</p>{/if}
  </div>
</AppShell>

{#if confirmCacheClear}<div class="dialog-layer"><button class="scrim" aria-label={m.common_cancel()} onclick={() => (confirmCacheClear=false)}></button><div role="alertdialog" aria-modal="true"><h2>{m.settings_cache_dialog_title()}</h2><p>{m.settings_cache_dialog_description()}</p><footer><button onclick={() => (confirmCacheClear=false)}>{m.common_cancel()}</button><button class="primary" disabled={busy} onclick={clearCacheNow}>{m.settings_confirm_cache_clear()}</button></footer></div></div>{/if}

<style>
  .page{width:min(760px,100%);box-sizing:border-box;margin:auto;padding:30px 24px 60px}h1{margin:24px 0;font-size:var(--type-title)}section{overflow:hidden;border:1px solid var(--line);border-radius:18px;background:white}.row{display:flex;min-height:62px;align-items:center;gap:12px;padding:0 18px;border-bottom:1px solid var(--line);color:var(--text);text-decoration:none}.row:last-child{border:0}.row span,.row i{overflow:hidden;margin-left:auto;color:var(--muted);font-size:var(--type-body);font-style:normal;text-overflow:ellipsis;white-space:nowrap}.row button{padding:8px 12px;border:1px solid #cde7f8;border-radius:16px;background:white;color:var(--pixiv-blue)}select{margin-left:auto;padding:8px;border:1px solid var(--line);border-radius:10px;background:white}input{margin-left:auto;accent-color:var(--pixiv-blue)}p.error{color:var(--danger)}.dialog-layer{position:fixed;z-index:1000;inset:0;display:grid;place-items:center;padding:20px}.scrim{position:absolute;inset:0;border:0;background:#0008}.dialog-layer>div{position:relative;width:min(420px,100%);box-sizing:border-box;padding:24px;border-radius:18px;background:white}.dialog-layer footer{display:flex;justify-content:flex-end;gap:10px}.dialog-layer footer button{padding:10px 18px;border:1px solid var(--line);border-radius:20px;background:white}.dialog-layer footer .primary{border-color:var(--pixiv-blue);background:var(--pixiv-blue);color:white}
</style>
