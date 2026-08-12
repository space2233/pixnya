<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import {
    clearBrowsingHistory,
    describeDataFailure,
    getBrowsingHistory,
    removeBrowsingHistoryEntry,
    setBrowsingHistoryEnabled,
  } from "$lib/pixiv-api";
  import type { HistoryEntry, HistoryKind, HistorySnapshot } from "$lib/types";

  let snapshot = $state<HistorySnapshot | null>(null);
  let status = $state<"loading" | "ready" | "error">("loading");
  let errorMessage = $state("");
  let query = $state("");
  let kind = $state<"all" | HistoryKind>("all");
  let pendingKey = $state("");
  let savingPreference = $state(false);
  let confirmingClear = $state(false);
  let clearing = $state(false);
  let notice = $state("");
  const kindOptions: Array<{ id: "all" | HistoryKind; label: () => string }> = [
    { id: "all", label: m.history_kind_all },
    { id: "artwork", label: m.history_kind_artwork },
    { id: "novel", label: m.history_kind_novel },
    { id: "user", label: m.history_kind_user },
  ];

  const filteredEntries = $derived.by(() => {
    const needle = query.trim().toLocaleLowerCase();
    return (snapshot?.entries ?? []).filter((entry) => {
      if (kind !== "all" && entry.kind !== kind) return false;
      return !needle || `${entry.title}\n${entry.subtitle}`.toLocaleLowerCase().includes(needle);
    });
  });

  onMount(() => void loadHistory());

  async function loadHistory() {
    status = "loading";
    errorMessage = "";
    try {
      snapshot = await getBrowsingHistory();
      status = "ready";
    } catch (error) {
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function toggleHistory() {
    if (!snapshot || savingPreference) return;
    savingPreference = true;
    notice = "";
    try {
      snapshot = await setBrowsingHistoryEnabled(!snapshot.enabled);
      notice = snapshot.enabled ? m.history_started() : m.history_stopped();
    } catch (error) {
      notice = describeDataFailure(error);
    } finally {
      savingPreference = false;
    }
  }

  async function removeEntry(entry: HistoryEntry) {
    if (!snapshot || pendingKey) return;
    pendingKey = `${entry.kind}-${entry.resourceId}`;
    try {
      await removeBrowsingHistoryEntry(entry.kind, entry.resourceId);
      snapshot = { ...snapshot, entries: snapshot.entries.filter((candidate) => candidate.kind !== entry.kind || candidate.resourceId !== entry.resourceId) };
    } catch (error) {
      notice = describeDataFailure(error);
    } finally {
      pendingKey = "";
    }
  }

  async function clearHistory() {
    if (!snapshot || clearing) return;
    if (!confirmingClear) {
      confirmingClear = true;
      return;
    }
    clearing = true;
    notice = "";
    try {
      const removed = await clearBrowsingHistory();
      snapshot = { ...snapshot, entries: [] };
      notice = m.history_removed({ count: removed.entriesRemoved });
      confirmingClear = false;
    } catch (error) {
      notice = describeDataFailure(error);
    } finally {
      clearing = false;
    }
  }

  function entryHref(entry: HistoryEntry): string {
    if (entry.kind === "artwork") return `/artworks/${entry.resourceId}`;
    if (entry.kind === "novel") return `/novels/${entry.resourceId}`;
    return `/users/${entry.resourceId}`;
  }

  function kindLabel(value: HistoryKind): string {
    return value === "artwork"
      ? m.history_kind_artwork()
      : value === "novel"
        ? m.history_kind_novel()
        : m.history_kind_user();
  }

  function formatViewedAt(value: number): string {
    return new Intl.DateTimeFormat(currentAppLocale(), {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(new Date(value * 1000));
  }
</script>

<svelte:head><title>{m.history_title()} · PixNya</title></svelte:head>

<AppShell title={m.history_title()}>
  <main class="history-page">
    <header class="page-heading">
      <div><h1>{m.history_title()}</h1></div>
      {#if snapshot}
        <button class:disabled={!snapshot.enabled} type="button" disabled={savingPreference} onclick={toggleHistory}>
          <Icon name="history" size={18} />{savingPreference ? m.history_saving() : snapshot.enabled ? m.history_recording() : m.history_recording_stopped()}
        </button>
      {/if}
    </header>

    {#if status === "loading"}
      <section class="state">{m.history_loading()}</section>
    {:else if status === "error"}
      <section class="state error" role="alert"><p>{errorMessage}</p><button type="button" onclick={loadHistory}>{m.common_retry()}</button></section>
    {:else if snapshot}
      <section class="history-panel">
        <div class="toolbar">
          <label><Icon name="search" size={16} /><input bind:value={query} placeholder={m.history_search_placeholder()} aria-label={m.history_search_label()} /></label>
          <div class="kind-filters" aria-label={m.history_type_label()}>
            {#each kindOptions as option}
              <button class:active={kind === option.id} type="button" onclick={() => (kind = option.id)}>{option.label()}</button>
            {/each}
          </div>
          <button class="clear" class:confirm={confirmingClear} type="button" disabled={clearing || snapshot.entries.length === 0} onclick={clearHistory}>
            {clearing ? m.history_clearing() : confirmingClear ? m.history_confirm_clear() : m.history_clear_all()}
          </button>
        </div>

        {#if notice}<p class="notice" role="status">{notice}</p>{/if}
        {#if !snapshot.enabled}
          <p class="paused"><Icon name="shield" size={17} />{m.history_paused()}</p>
        {/if}

        {#if filteredEntries.length > 0}
          <div class="history-list">
            {#each filteredEntries as entry (`${entry.kind}-${entry.resourceId}`)}
              <article>
                <a href={entryHref(entry)}>
                  <span class="thumbnail">
                    {#if entry.thumbnailUrl}<PixivImage url={entry.thumbnailUrl} alt="" />{/if}
                    <b>{kindLabel(entry.kind)}</b>
                  </span>
                  <span class="copy"><small>{kindLabel(entry.kind)} · {formatViewedAt(entry.viewedAtUnixSeconds)}</small><strong>{entry.title}</strong><em>{entry.subtitle}</em></span>
                  <i>›</i>
                </a>
                <button type="button" aria-label={m.history_remove_entry({ title: entry.title })} disabled={!!pendingKey} onclick={() => removeEntry(entry)}>
                  {pendingKey === `${entry.kind}-${entry.resourceId}` ? "…" : "×"}
                </button>
              </article>
            {/each}
          </div>
        {:else}
          <div class="empty"><Icon name="history" size={34} /><h2>{snapshot.entries.length ? m.history_no_matches() : m.history_empty()}</h2><p>{snapshot.enabled ? m.history_empty_enabled() : m.history_empty_disabled()}</p></div>
        {/if}
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .history-page { width: min(980px, 100%); margin: 0 auto; padding: 26px 28px 70px; }
  .page-heading { display: flex; gap: 24px; align-items: flex-end; justify-content: space-between; }
  .page-heading h1 { margin: 7px 0 0; font-size: 25px; }
  .page-heading > button { display: flex; min-width: 110px; height: 38px; gap: 7px; align-items: center; justify-content: center; color: white; border: 0; border-radius: 19px; background: #27ae72; cursor: pointer; font-size: 9px; font-weight: 700; }
  .page-heading > button.disabled { color: #6f7579; background: #e7e9eb; }
  .state { display: grid; min-height: 220px; margin-top: 22px; place-items: center; color: var(--muted); border: 1px dashed var(--line); border-radius: 12px; background: white; font-size: 10px; }
  .state.error { align-content: center; gap: 12px; color: #a44f5e; }
  .state.error p { margin: 0; }
  .state.error button { padding: 9px 18px; color: white; border: 0; border-radius: 17px; background: var(--pixiv-blue); }
  .history-panel { margin-top: 22px; overflow: hidden; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .toolbar { display: grid; grid-template-columns: minmax(180px, 1fr) auto auto; gap: 12px; align-items: center; padding: 15px; border-bottom: 1px solid var(--line); }
  .toolbar label { display: flex; height: 36px; gap: 8px; align-items: center; padding: 0 12px; color: var(--muted); border-radius: 18px; background: #f4f5f6; }
  .toolbar input { min-width: 0; flex: 1; border: 0; outline: 0; background: transparent; font-size: 10px; }
  .kind-filters { display: flex; gap: 4px; }
  .kind-filters button, .clear { height: 34px; padding: 0 12px; color: #686e72; border: 1px solid var(--line); border-radius: 17px; background: white; cursor: pointer; font-size: 9px; }
  .kind-filters button.active { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  .clear.confirm { color: white; border-color: #d95d70; background: #d95d70; }
  .clear:disabled { cursor: default; opacity: .48; }
  .notice, .paused { margin: 0; padding: 11px 16px; color: #55717f; border-bottom: 1px solid var(--line); background: #f2f9fd; font-size: 9px; }
  .paused { display: flex; gap: 7px; align-items: center; color: #826c3c; background: #fff9ea; }
  .history-list article { position: relative; border-bottom: 1px solid var(--line); }
  .history-list article:last-child { border-bottom: 0; }
  .history-list article > a { display: grid; min-width: 0; grid-template-columns: 76px minmax(0, 1fr) 20px; gap: 14px; align-items: center; padding: 12px 54px 12px 14px; color: var(--text); text-decoration: none; }
  .thumbnail { position: relative; display: grid; width: 76px; height: 60px; overflow: hidden; place-items: center; border-radius: 7px; background: #edf1f4; }
  .thumbnail :global(img) { position: absolute; inset: 0; }
  .thumbnail b { position: relative; z-index: 1; padding: 4px 6px; color: white; border-radius: 4px; background: rgba(25, 28, 30, .58); font-size: 8px; }
  .copy { min-width: 0; }
  .copy small, .copy strong, .copy em { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .copy small { color: var(--pixiv-blue); font-size: 8px; font-weight: 700; }
  .copy strong { margin-top: 6px; font-size: 12px; }
  .copy em { margin-top: 5px; color: var(--muted); font-size: 9px; font-style: normal; }
  .history-list article > a > i { color: #afb4b7; font-size: 22px; font-style: normal; }
  .history-list article > button { position: absolute; top: 50%; right: 14px; display: grid; width: 30px; height: 30px; place-items: center; color: #92989c; border: 0; border-radius: 50%; background: transparent; cursor: pointer; font-size: 19px; transform: translateY(-50%); }
  .history-list article > button:hover { color: #be5263; background: #fff0f3; }
  .empty { display: grid; min-height: 260px; place-items: center; align-content: center; color: #abb1b5; text-align: center; }
  .empty h2 { margin: 13px 0 0; color: #62686c; font-size: 15px; }
  .empty p { margin: 7px 0 0; font-size: 9px; }

  @media (max-width: 680px) {
    .history-page { padding: 18px 10px 90px; }
    .page-heading { align-items: stretch; }
    .page-heading h1 { font-size: 21px; }
    .page-heading > button { min-width: 96px; }
    .toolbar { grid-template-columns: 1fr; }
    .kind-filters { display: grid; grid-template-columns: repeat(4, 1fr); }
    .clear { width: 100%; }
    .history-list article > a { grid-template-columns: 68px minmax(0, 1fr); padding-right: 48px; }
    .thumbnail { width: 68px; height: 58px; }
    .history-list article > a > i { display: none; }
  }
</style>
