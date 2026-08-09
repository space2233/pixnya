<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import NovelCard from "$lib/components/NovelCard.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import { describeDataFailure, getNovelSeries } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { session, sessionRestoring } from "$lib/session";
  import type { NovelSeriesDetail, NovelSummary } from "$lib/types";

  let series = $state<NovelSeriesDetail | null>(null);
  let firstNovel = $state<NovelSummary | null>(null);
  let novels = $state<NovelSummary[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let loadMoreError = $state("");
  let requestedKey = $state("");
  let requestSequence = 0;
  let seriesId = $derived(page.params.id ?? "");
  let caption = $derived(series ? plainPixivText(series.caption) : "");

  type NovelSeriesSnapshot = {
    series: NovelSeriesDetail | null;
    firstNovel: NovelSummary | null;
    novels: NovelSummary[];
    nextCursor: string | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    loadMoreError: string;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<NovelSeriesSnapshot>({
      series, firstNovel, novels, nextCursor, status, errorMessage, loadMoreError, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<NovelSeriesSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      series = value.series;
      firstNovel = value.firstNovel;
      novels = value.novels;
      nextCursor = value.nextCursor;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      loadMoreError = value.loadMoreError;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
      loadingMore = false;
    },
  };

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && seriesId ? `${sessionKey}:${seriesId}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      series = null;
      firstNovel = null;
      novels = [];
      nextCursor = null;
      status = "idle";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadSeries(key, seriesId);
    }
  });

  function uniqueNovels(items: NovelSummary[]): NovelSummary[] {
    const seen = new Set<string>();
    return items.filter((item) => !seen.has(item.id) && seen.add(item.id));
  }

  async function loadSeries(key: string, id: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    loadMoreError = "";
    series = null;
    firstNovel = null;
    novels = [];
    nextCursor = null;
    try {
      const result = await getNovelSeries(id);
      if (sequence !== requestSequence || key !== requestedKey) return;
      series = result.series;
      firstNovel = result.firstNovel;
      novels = uniqueNovels(result.novels.length ? result.novels : [result.firstNovel]);
      nextCursor = result.nextCursor ?? null;
      status = "ready";
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedKey) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadMore() {
    const cursor = nextCursor;
    if (!cursor || loadingMore || !series) return;
    const sequence = requestSequence;
    const key = requestedKey;
    loadingMore = true;
    loadMoreError = "";
    try {
      const result = await getNovelSeries(series.id, cursor);
      if (sequence !== requestSequence || key !== requestedKey) return;
      novels = uniqueNovels([...novels, ...result.novels]);
      nextCursor = result.nextCursor ?? null;
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) {
        loadMoreError = describeDataFailure(error);
      }
    } finally {
      loadingMore = false;
    }
  }

  function compact(value: number): string {
    return new Intl.NumberFormat(currentAppLocale(), { notation: "compact", maximumFractionDigits: 1 }).format(value);
  }
</script>

<svelte:head><title>{series?.title || m.novel_series_label()} · PixNya</title></svelte:head>

<AppShell title={m.novel_series_label()}>
  <main class="series-page">
    <ReturnLink fallback="/novels" label={m.novel_return_source()} />

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state-card"><Icon name="user" size={28} /><div><h1>{m.novel_series_login_title()}</h1><p>{m.novel_series_login_description()}</p></div><a href="/login?mode=standard">{m.common_go_to_login()}</a></section>
    {:else if status === "loading"}
      <section class="state-card"><span class="spinner"></span><div><h1>{m.novel_series_loading_title()}</h1><p>{m.novel_series_loading_description()}</p></div></section>
    {:else if status === "error"}
      <section class="state-card error" role="alert"><span>!</span><div><h1>{m.series_load_failed()}</h1><p>{errorMessage}</p></div><button type="button" onclick={() => loadSeries(requestedKey, seriesId)}>{m.common_retry()}</button></section>
    {:else if series}
      <section class="series-hero">
        <div class="series-cover"><PixivImage url={firstNovel?.coverUrl} alt="" /></div>
        <div class="series-copy">
          <div class="eyebrow">{m.novel_series_summary({ count: series.contentCount })}</div>
          <h1>{series.title || m.series_unnamed()}</h1>
          {#if caption}<p>{caption}</p>{/if}
          <a class="author" href={`/users/${series.author.id}`}>{series.author.name || series.author.account}</a>
          <div class="series-meta">
            <span>{m.novel_series_characters({ count: compact(series.totalCharacterCount) })}</span>
            <span>{series.isConcluded ? m.series_concluded() : m.series_ongoing()}</span>
            {#if series.isOriginal}<span>{m.novel_original_badge()}</span>{/if}
            {#if series.watchlistAdded}<span>{m.series_watchlisted()}</span>{/if}
          </div>
          {#if firstNovel}<a class="start" href={`/novels/${firstNovel.id}/read`}>{m.novel_series_start_reading()}</a>{/if}
        </div>
      </section>

      <section class="contents">
        <header><div><h2>{m.series_contents()}</h2><p>{m.novel_series_contents_description()}</p></div><strong>{novels.length} / {series.contentCount}</strong></header>
        {#if novels.length}
          <div class="novel-grid">{#each novels as novel (novel.id)}<NovelCard {novel} />{/each}</div>
        {:else}<p class="empty">{m.novel_series_empty()}</p>{/if}
        {#if loadMoreError}<p class="load-error" role="alert">{loadMoreError}</p>{/if}
        {#if nextCursor}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? m.common_loading() : m.novel_series_load_more()}</button>{/if}
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .series-page { width: min(1160px,100%); margin: 0 auto; padding: 24px 28px 58px; }
  .author:hover { color: var(--pixiv-blue); }
  .state-card { display: grid; grid-template-columns: 46px minmax(0,1fr) auto; gap: 16px; align-items: center; margin-top: 22px; padding: 22px; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .state-card h1 { margin: 0; font-size: 17px; } .state-card p { margin: 5px 0 0; color: var(--muted); font-size: 10px; }
  .state-card a, .state-card button { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: 10px; font-weight: 700; text-decoration: none; }
  .state-card.error > span { display: grid; width: 40px; height: 40px; place-items: center; color: #b65364; border-radius: 50%; background: #fff0f3; font-weight: 800; }
  .spinner { width: 32px; height: 32px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .series-hero { display: grid; grid-template-columns: 190px minmax(0,1fr); gap: 28px; margin-top: 20px; padding: 26px; border: 1px solid var(--line); border-radius: 14px; background: white; }
  .series-cover { position: relative; overflow: hidden; aspect-ratio: .72; border-radius: 9px; background: #edf1f4; }
  .series-cover :global(img) { position: absolute; inset: 0; }
  .series-copy { align-self: center; min-width: 0; }
  .eyebrow { color: var(--pixiv-blue); font-size: 9px; font-weight: 750; }
  .series-copy h1 { margin: 9px 0 0; font-size: 27px; line-height: 1.3; }
  .series-copy > p { max-width: 700px; margin: 13px 0 0; color: #62676b; font-size: 10px; line-height: 1.75; white-space: pre-line; }
  .author { display: inline-block; margin-top: 14px; color: #3f484e; font-size: 10px; font-weight: 700; text-decoration: none; }
  .series-meta { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; color: var(--muted); font-size: 8px; }
  .series-meta span { padding: 5px 8px; border-radius: 4px; background: #f3f6f8; }
  .start { display: inline-flex; min-height: 36px; align-items: center; margin-top: 18px; padding: 0 17px; color: white; border-radius: 18px; background: var(--pixiv-blue); font-size: 9px; font-weight: 750; text-decoration: none; }
  .contents { margin-top: 34px; }
  .contents header { display: flex; align-items: end; justify-content: space-between; gap: 16px; }
  .contents h2 { margin: 0; font-size: 19px; } .contents header p { margin: 5px 0 0; color: var(--muted); font-size: 9px; } .contents header strong { color: var(--muted); font-size: 9px; }
  .novel-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 14px; margin-top: 16px; }
  .empty { padding: 36px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; font-size: 10px; text-align: center; }
  .load-error { color: #a65865; font-size: 9px; text-align: center; }
  .load-more { display: block; min-width: 150px; height: 38px; margin: 26px auto 0; color: #59636a; border: 1px solid var(--line); border-radius: 19px; background: white; cursor: pointer; font-size: 10px; font-weight: 700; }
  .load-more:disabled { cursor: wait; opacity: .65; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 820px) { .novel-grid { grid-template-columns: 1fr; } }
  @media (max-width: 620px) { .series-page { padding: 16px 12px 88px; } .series-hero { grid-template-columns: 84px minmax(0,1fr); gap: 16px; padding: 16px; } .series-copy h1 { margin-top: 6px; font-size: 20px; } .start { justify-content: center; } .state-card { grid-template-columns: 38px minmax(0,1fr); } .state-card a, .state-card button { grid-column: 1 / -1; text-align: center; } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
