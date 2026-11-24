<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import NovelCard from "$lib/components/NovelCard.svelte";
  import { m } from "$lib/i18n";
  import { describeDataFailure, enqueueDownload, listDownloadTasks, listOfflineEntries } from "$lib/pixiv-api";
  import type { DownloadState, DownloadTask, NovelSummary } from "$lib/types";

  let { novels }: { novels: NovelSummary[] } = $props();
  let downloaded = $state<Set<string>>(new Set());
  let tasks = $state<Map<string, DownloadState>>(new Map());
  let selected = $state<string[]>([]);
  let ready = $state(false);
  let busy = $state(false);
  let loadError = $state("");
  let feedback = $state("");
  let mounted = false;
  let generation = 0;

  const activeLabels = { queued: m.offline_state_queued, running: m.offline_state_running, paused: m.offline_state_paused };
  function chapterStatus(id: string) {
    if (downloaded.has(id)) return m.series_chapter_saved();
    const state = tasks.get(id);
    return state === "queued" || state === "running" || state === "paused" ? activeLabels[state]() : "";
  }
  let candidates = $derived(novels.filter((novel) => selected.includes(novel.id) && !chapterStatus(novel.id)));

  async function refresh() {
    const request = ++generation;
    try {
      const [entries, queue] = await Promise.all([listOfflineEntries(), listDownloadTasks()]);
      if (!mounted || request !== generation) return;
      downloaded = new Set(entries.filter((entry) => entry.kind === "novel").map((entry) => entry.resourceId));
      tasks = new Map(queue.filter((task) => task.kind === "novel").map((task) => [task.resourceId, task.state]));
      selected = selected.filter((id) => !chapterStatus(id));
      ready = true;
      loadError = "";
    } catch (error) {
      if (mounted && request === generation) {
        ready = false;
        loadError = describeDataFailure(error);
      }
    }
  }

  onMount(() => {
    mounted = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    function scheduleRefresh() {
      if (mounted && timer === undefined) timer = setTimeout(() => { timer = undefined; void refresh(); }, 150);
    }
    const subscription = listen<DownloadTask | null>("pixiv-download-queue-changed", ({ payload }) => {
      if (!payload || (payload.kind === "novel" && (payload.state !== tasks.get(payload.resourceId) || payload.state === "completed"))) scheduleRefresh();
    }).catch(() => undefined);
    void refresh();
    window.addEventListener("focus", scheduleRefresh);
    return () => {
      mounted = false;
      clearTimeout(timer);
      window.removeEventListener("focus", scheduleRefresh);
      void subscription.then((stop) => stop?.());
    };
  });

  async function queueSelected() {
    if (busy || !ready) return;
    busy = true;
    feedback = "";
    let count = 0;
    try {
      await refresh();
      if (!ready || !mounted) return;
      for (const novel of candidates) {
        if (!mounted) return;
        if (chapterStatus(novel.id)) continue;
        const task = await enqueueDownload("novel", novel.id, novel.title, novel.author.name);
        if (!mounted) return;
        tasks = new Map(tasks).set(novel.id, task.state);
        selected = selected.filter((id) => id !== novel.id);
        count++;
      }
      feedback = m.series_queued_count({ count });
    } catch (error) {
      feedback = m.series_queued_count({ count }) + " " + describeDataFailure(error);
    } finally {
      busy = false;
      if (mounted) void refresh();
    }
  }
</script>

<div class="tools">
  <button type="button" disabled={!ready || busy || !candidates.length} onclick={queueSelected}>
    {busy ? m.common_queueing() : m.series_save_selected({ count: candidates.length })}
  </button>
  {#if loadError}<span role="alert">{loadError} <button type="button" onclick={refresh}>{m.common_retry()}</button></span>{/if}
  {#if feedback}<span role="status">{feedback}</span>{/if}
</div>
<div class="novel-grid">
  {#each novels as novel (novel.id)}
    {@const status = chapterStatus(novel.id)}
    <div class="chapter">
      <NovelCard {novel} selectable={ready && !busy && !status} selected={selected.includes(novel.id)}
        onSelect={(checked) => { selected = checked ? [...selected, novel.id] : selected.filter((id) => id !== novel.id); }} />
      {#if status}<small>{status}</small>{/if}
    </div>
  {/each}
</div>

<style>
  .tools { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-top: 14px; }
  .tools button { padding: 8px 14px; border-radius: 16px; border: 1px solid var(--line); background: var(--surface); color: var(--text); }
  .tools button:enabled { color: var(--pixiv-blue); cursor: pointer; }
  .tools button:disabled { opacity: .6; }
  .novel-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 14px; margin-top: 16px; }
  .chapter { min-width: 0; }
  small { display: block; padding: 4px 10px; color: var(--muted); }
  @media (max-width: 820px) { .novel-grid { grid-template-columns: 1fr; } }
</style>
