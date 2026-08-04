<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkCard from "$lib/components/ArtworkCard.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import { rememberArtworkSeriesPage } from "$lib/artwork-series-navigation";
  import { describeDataFailure, getIllustrationSeries } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { session, sessionRestoring } from "$lib/session";
  import type { IllustrationSeriesDetail, IllustrationSummary } from "$lib/types";

  let series = $state<IllustrationSeriesDetail | null>(null);
  let illustrations = $state<IllustrationSummary[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let loadMoreError = $state("");
  let requestedKey = $state("");
  let requestSequence = 0;
  let seriesId = $derived(page.params.id ?? "");
  let caption = $derived(series ? plainPixivText(series.caption) : "");

  type ArtworkSeriesSnapshot = {
    series: IllustrationSeriesDetail | null;
    illustrations: IllustrationSummary[];
    nextCursor: string | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    loadMoreError: string;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<ArtworkSeriesSnapshot>({
      series, illustrations, nextCursor, status, errorMessage, loadMoreError, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<ArtworkSeriesSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      series = value.series;
      illustrations = value.illustrations;
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
      illustrations = [];
      nextCursor = null;
      status = "idle";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadSeries(key, seriesId);
    }
  });

  function uniqueWorks(items: IllustrationSummary[]): IllustrationSummary[] {
    const seen = new Set<string>();
    return items.filter((item) => !seen.has(item.id) && seen.add(item.id));
  }

  async function loadSeries(key: string, id: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    loadMoreError = "";
    series = null;
    illustrations = [];
    nextCursor = null;
    try {
      const result = await getIllustrationSeries(id);
      if (sequence !== requestSequence || key !== requestedKey) return;
      series = result.series;
      illustrations = uniqueWorks([result.firstIllustration, ...result.illustrations]);
      nextCursor = result.nextCursor ?? null;
      rememberArtworkSeriesPage({ ...result, illustrations }, true);
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
      const result = await getIllustrationSeries(series.id, cursor);
      if (sequence !== requestSequence || key !== requestedKey) return;
      illustrations = uniqueWorks([...illustrations, ...result.illustrations]);
      nextCursor = result.nextCursor ?? null;
      rememberArtworkSeriesPage(result);
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) {
        loadMoreError = describeDataFailure(error);
      }
    } finally {
      loadingMore = false;
    }
  }
</script>

<svelte:head><title>{series?.title || "作品系列"} · PixNya</title></svelte:head>

<AppShell title="作品系列">
  <main class="series-page">
    <ReturnLink fallback="/artworks" label="返回来源页" />

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state-card">
        <Icon name="user" size={28} />
        <div><h1>登录后查看作品系列</h1><p>系列目录通过登录后的 Pixiv App API 载入。</p></div>
        <a href="/login?mode=standard">前往登录</a>
      </section>
    {:else if status === "loading"}
      <section class="state-card"><span class="spinner"></span><div><h1>正在载入系列</h1><p>正在读取系列信息与连续作品…</p></div></section>
    {:else if status === "error"}
      <section class="state-card error" role="alert"><span>!</span><div><h1>系列载入失败</h1><p>{errorMessage}</p></div><button type="button" onclick={() => loadSeries(requestedKey, seriesId)}>重试</button></section>
    {:else if series}
      <section class="series-hero">
        <div class="series-cover"><PixivImage url={series.coverUrl ?? illustrations[0]?.thumbnailUrl} alt="" /></div>
        <div class="series-copy">
          <div class="eyebrow">作品系列 · {series.workCount} 部</div>
          <h1>{series.title || "未命名系列"}</h1>
          {#if caption}<p>{caption}</p>{/if}
          <a class="author" href={`/users/${series.author.id}`}>{series.author.name || series.author.account}</a>
          <div class="series-meta">
            {#if series.createDate}<span>创建于 {series.createDate.slice(0, 10)}</span>{/if}
            {#if series.watchlistAdded}<span>已加入追更</span>{/if}
          </div>
          {#if illustrations[0]}<a class="start" href={`/artworks/${illustrations[0].id}`}>从第一部开始连续浏览</a>{/if}
        </div>
      </section>

      <section class="contents">
        <header><div><h2>系列目录</h2><p>打开任一作品后，可用上一篇/下一篇连续浏览。</p></div><strong>{illustrations.length} / {series.workCount}</strong></header>
        {#if illustrations.length}
          <div class="artwork-grid">
            {#each illustrations as illustration, index (illustration.id)}
              <ArtworkCard {illustration} rank={index + 1} tone={(index % 6) + 1} />
            {/each}
          </div>
        {:else}<p class="empty">这个系列目前没有可显示的作品。</p>{/if}
        {#if loadMoreError}<p class="load-error" role="alert">{loadMoreError}</p>{/if}
        {#if nextCursor}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? "正在载入…" : "加载后续作品"}</button>{/if}
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
  .series-hero { display: grid; grid-template-columns: 210px minmax(0,1fr); gap: 28px; margin-top: 20px; padding: 26px; border: 1px solid var(--line); border-radius: 14px; background: white; }
  .series-cover { position: relative; overflow: hidden; aspect-ratio: 1; border-radius: 10px; background: #edf1f4; }
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
  .contents h2 { margin: 0; font-size: 19px; } .contents header p { margin: 5px 0 0; color: var(--muted); font-size: 9px; }
  .contents header strong { color: var(--muted); font-size: 9px; }
  .artwork-grid { display: grid; grid-template-columns: repeat(5,minmax(0,1fr)); gap: 23px 14px; margin-top: 16px; }
  .empty { padding: 36px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; font-size: 10px; text-align: center; }
  .load-error { color: #a65865; font-size: 9px; text-align: center; }
  .load-more { display: block; min-width: 150px; height: 38px; margin: 26px auto 0; color: #59636a; border: 1px solid var(--line); border-radius: 19px; background: white; cursor: pointer; font-size: 10px; font-weight: 700; }
  .load-more:disabled { cursor: wait; opacity: .65; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 900px) { .artwork-grid { grid-template-columns: repeat(3,minmax(0,1fr)); } }
  @media (max-width: 620px) { .series-page { padding: 16px 12px 88px; } .series-hero { grid-template-columns: 92px minmax(0,1fr); gap: 16px; padding: 16px; } .series-copy h1 { margin-top: 6px; font-size: 20px; } .series-copy > p { grid-column: 1 / -1; } .start { grid-column: 1 / -1; justify-content: center; } .artwork-grid { grid-template-columns: repeat(2,minmax(0,1fr)); gap: 18px 11px; } .state-card { grid-template-columns: 38px minmax(0,1fr); } .state-card a, .state-card button { grid-column: 1 / -1; text-align: center; } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
