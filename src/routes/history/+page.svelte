<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
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
      notice = snapshot.enabled ? "已开始在本机记录浏览历史。" : "已停止记录；现有历史仍保留，可单独清除。";
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
      notice = `已从本机删除 ${removed.entriesRemoved} 条浏览记录。`;
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
    return value === "artwork" ? "作品" : value === "novel" ? "小说" : "作者";
  }

  function formatViewedAt(value: number): string {
    return new Intl.DateTimeFormat("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(new Date(value * 1000));
  }
</script>

<svelte:head><title>浏览历史 · PixNya</title></svelte:head>

<AppShell title="浏览历史">
  <main class="history-page">
    <header class="page-heading">
      <div><span>仅保存在本机</span><h1>浏览历史</h1><p>最近查看的作品、小说和作者，最多保留 {snapshot?.limit ?? 500} 条。</p></div>
      {#if snapshot}
        <button class:disabled={!snapshot.enabled} type="button" disabled={savingPreference} onclick={toggleHistory}>
          <Icon name="history" size={18} />{savingPreference ? "保存中…" : snapshot.enabled ? "正在记录" : "已停止记录"}
        </button>
      {/if}
    </header>

    {#if status === "loading"}
      <section class="state">正在读取本机浏览历史…</section>
    {:else if status === "error"}
      <section class="state error" role="alert"><p>{errorMessage}</p><button type="button" onclick={loadHistory}>重试</button></section>
    {:else if snapshot}
      <section class="history-panel">
        <div class="toolbar">
          <label><Icon name="search" size={16} /><input bind:value={query} placeholder="搜索标题或作者" aria-label="搜索浏览历史" /></label>
          <div class="kind-filters" aria-label="历史类型">
            {#each [["all", "全部"], ["artwork", "作品"], ["novel", "小说"], ["user", "作者"]] as option}
              <button class:active={kind === option[0]} type="button" onclick={() => (kind = option[0] as "all" | HistoryKind)}>{option[1]}</button>
            {/each}
          </div>
          <button class="clear" class:confirm={confirmingClear} type="button" disabled={clearing || snapshot.entries.length === 0} onclick={clearHistory}>
            {clearing ? "清除中…" : confirmingClear ? "再次点击确认" : "清除全部"}
          </button>
        </div>

        {#if notice}<p class="notice" role="status">{notice}</p>{/if}
        {#if !snapshot.enabled}
          <p class="paused"><Icon name="shield" size={17} />当前不会写入新的浏览记录；现有记录仍只保存在本机。</p>
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
                <button type="button" aria-label={`从历史中移除 ${entry.title}`} disabled={!!pendingKey} onclick={() => removeEntry(entry)}>
                  {pendingKey === `${entry.kind}-${entry.resourceId}` ? "…" : "×"}
                </button>
              </article>
            {/each}
          </div>
        {:else}
          <div class="empty"><Icon name="history" size={34} /><h2>{snapshot.entries.length ? "没有符合条件的记录" : "还没有浏览记录"}</h2><p>{snapshot.enabled ? "打开作品、小说或作者详情后会出现在这里。" : "启用记录后，新查看的内容会保存在这里。"}</p></div>
        {/if}
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .history-page { width: min(980px, 100%); margin: 0 auto; padding: 26px 28px 70px; }
  .page-heading { display: flex; gap: 24px; align-items: flex-end; justify-content: space-between; }
  .page-heading span { color: var(--pixiv-blue); font-size: 9px; font-weight: 750; }
  .page-heading h1 { margin: 7px 0 0; font-size: 25px; }
  .page-heading p { margin: 7px 0 0; color: var(--muted); font-size: 10px; }
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
