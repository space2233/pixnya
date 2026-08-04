<script lang="ts">
  import { page } from "$app/state";
  import { tick } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import { parseNovelText } from "$lib/novel-text";
  import {
    describeDataFailure,
    getNovelContent,
    getNovelDetail,
    recordBrowsingHistory,
  } from "$lib/pixiv-api";
  import { r18DefaultVisible } from "$lib/preferences";
  import { session, sessionRestoring } from "$lib/session";
  import type { NovelContent, NovelDetail } from "$lib/types";

  let detail = $state<NovelDetail | null>(null);
  let content = $state<NovelContent | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let fontSize = $state(18);
  let lineHeight = $state(1.9);
  let theme = $state<"paper" | "white" | "dark">("paper");
  let progress = $state(0);
  let revealRestricted = $state(false);
  let readerElement = $state<HTMLElement | null>(null);
  let requestedKey = $state("");
  let requestSequence = 0;
  let novelId = $derived(page.params.id ?? "");
  let blocks = $derived(content ? parseNovelText(content.text) : []);
  let restricted = $derived((detail?.novel.xRestrict ?? 0) > 0);
  let activeSeriesId = $derived(content?.seriesId ?? detail?.novel.series?.id ?? null);
  let activeSeriesTitle = $derived(
    content?.seriesTitle ?? detail?.novel.series?.title ?? "小说系列",
  );

  type NovelReaderSnapshot = {
    detail: NovelDetail | null;
    content: NovelContent | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    fontSize: number;
    lineHeight: number;
    theme: "paper" | "white" | "dark";
    progress: number;
    revealRestricted: boolean;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<NovelReaderSnapshot>({
      detail, content, status, errorMessage, fontSize, lineHeight, theme,
      progress, revealRestricted, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<NovelReaderSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      detail = value.detail;
      content = value.content;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      fontSize = value.fontSize;
      lineHeight = value.lineHeight;
      theme = value.theme;
      progress = value.progress;
      revealRestricted = value.revealRestricted;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
      void tick().then(() => {
        if (!readerElement) return;
        const maximum = Math.max(0, readerElement.scrollHeight - readerElement.clientHeight);
        readerElement.scrollTop = maximum * progress;
      });
    },
  };

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && novelId ? `${sessionKey}:${novelId}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      detail = null;
      content = null;
      status = "idle";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadReader(key, novelId);
    }
  });

  async function loadReader(key: string, id: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    detail = null;
    content = null;
    revealRestricted = false;
    try {
      const [nextDetail, nextContent] = await Promise.all([
        getNovelDetail(id),
        getNovelContent(id),
      ]);
      if (sequence !== requestSequence || key !== requestedKey) return;
      detail = nextDetail;
      content = nextContent;
      status = "ready";
      void recordBrowsingHistory({
        kind: "novel",
        resourceId: nextDetail.novel.id,
        title: nextDetail.novel.title || "无题",
        subtitle: nextDetail.novel.author.name || "未知作者",
        thumbnailUrl: nextDetail.novel.coverUrl ?? nextContent.coverUrl,
      }).catch(() => undefined);
      await tick();
      restoreProgress(id);
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedKey) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  function progressKey(id: string): string {
    return `pixiv-client:novel-progress:${id}`;
  }

  function restoreProgress(id: string) {
    if (!readerElement) return;
    const stored = Number(localStorage.getItem(progressKey(id)) ?? "0");
    const maximum = Math.max(0, readerElement.scrollHeight - readerElement.clientHeight);
    progress = Number.isFinite(stored) ? Math.min(1, Math.max(0, stored)) : 0;
    readerElement.scrollTop = maximum * progress;
  }

  function saveProgress() {
    if (!readerElement || !novelId) return;
    const maximum = Math.max(0, readerElement.scrollHeight - readerElement.clientHeight);
    progress = maximum ? readerElement.scrollTop / maximum : 1;
    localStorage.setItem(progressKey(novelId), String(progress));
  }
</script>

<svelte:head><title>{detail?.novel.title || "小说阅读"} · PixNya</title></svelte:head>

<AppShell title="阅读">
  <main class="reading-page">
    <header class="reading-header">
      <ReturnLink fallback={`/novels/${novelId}`} label="返回小说详情" />
      {#if detail}
        <div><h1>{detail.novel.title || "无题"}</h1><p>{detail.novel.author.name}</p></div>
        <strong>{Math.round(progress * 100)}%</strong>
      {/if}
    </header>

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state">
        <Icon name="user" size={27} />
        <div><h1>登录后阅读正文</h1><p>正文通过 Pixiv 小说阅读端点载入。</p></div>
        <a href="/login?mode=standard">前往登录</a>
      </section>
    {:else if status === "loading"}
      <section class="state">
        <span class="spinner"></span>
        <div><h1>正在准备阅读器</h1><p>正在载入正文和上次阅读位置…</p></div>
      </section>
    {:else if status === "error"}
      <section class="state error" role="alert">
        <span>!</span>
        <div><h1>正文载入失败</h1><p>{errorMessage}</p></div>
        <button type="button" onclick={() => loadReader(requestedKey, novelId)}>重试</button>
      </section>
    {:else if detail && content && restricted && !$r18DefaultVisible && !revealRestricted}
      <section class="state restricted" role="status">
        <span>R18</span>
        <div><h1>这是受限小说</h1><p>当前设置要求逐次确认后显示正文。</p></div>
        <button type="button" onclick={() => (revealRestricted = true)}>开始阅读</button>
      </section>
    {:else if detail && content}
      <section class="reader-shell">
        <div class="reader-controls">
          <label>字号 <input type="range" min="14" max="28" step="1" bind:value={fontSize} /></label>
          <label>行距 <input type="range" min="1.4" max="2.4" step="0.1" bind:value={lineHeight} /></label>
          <label>背景
            <select bind:value={theme}>
              <option value="paper">纸张</option><option value="white">白色</option><option value="dark">夜间</option>
            </select>
          </label>
        </div>

        <article
          class="novel-body theme-{theme}"
          style={`--reader-font:${fontSize}px;--reader-line:${lineHeight}`}
          bind:this={readerElement}
          onscroll={saveProgress}
        >
          {#each blocks as block}
            {#if block.kind === "chapter"}<h2>{block.text}</h2>
            {:else if block.kind === "page_break"}<hr />
            {:else if block.kind === "artwork_link"}<a class="embed" href={`/artworks/${block.id}`}>查看文中插画 #{block.id}</a>
            {:else if block.kind === "uploaded_image"}<div class="embed muted">文中上传图片 #{block.id}（当前正文接口未提供可验证的 CDN 地址）</div>
            {:else if block.kind === "external_link"}<div class="embed external">{block.label}<small>{block.url}</small></div>
            {:else}<p>{block.text}</p>{/if}
          {/each}
        </article>
      </section>

      {#if activeSeriesId}
        <nav class="series-navigation" aria-label="小说系列连续阅读">
          <a class="series-overview" href={`/series/novels/${activeSeriesId}`}>
            <small>小说系列</small><strong>{activeSeriesTitle}</strong>
          </a>
          {#if content.seriesNavigation.previous?.viewable}
            <a class="series-sibling previous" href={`/novels/${content.seriesNavigation.previous.id}/read`}>
              <small>‹ 上一篇 · 第 {content.seriesNavigation.previous.contentOrder} 篇</small>
              <strong>{content.seriesNavigation.previous.title || "无题"}</strong>
            </a>
          {:else}
            <span class="series-sibling disabled"><small>‹ 上一篇</small><strong>{content.seriesNavigation.previous?.viewableMessage || "已到系列开头"}</strong></span>
          {/if}
          {#if content.seriesNavigation.next?.viewable}
            <a class="series-sibling next" href={`/novels/${content.seriesNavigation.next.id}/read`}>
              <small>下一篇 · 第 {content.seriesNavigation.next.contentOrder} 篇 ›</small>
              <strong>{content.seriesNavigation.next.title || "无题"}</strong>
            </a>
          {:else}
            <span class="series-sibling disabled next"><small>下一篇 ›</small><strong>{content.seriesNavigation.next?.viewableMessage || "已到系列末尾"}</strong></span>
          {/if}
        </nav>
      {/if}
    {/if}
  </main>
</AppShell>

<style>
  .reading-page { width: min(920px, 100%); margin: 0 auto; padding: 20px 28px 55px; }
  .reading-header { display: grid; grid-template-columns: minmax(110px, auto) minmax(0, 1fr) auto; gap: 18px; align-items: center; margin-bottom: 14px; }
  .reading-header > a { color: var(--muted); font-size: 9px; text-decoration: none; }
  .reading-header h1 { overflow: hidden; margin: 0; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
  .reading-header p { margin: 3px 0 0; color: var(--muted); font-size: 8px; }
  .reading-header > strong { color: var(--pixiv-blue); font-size: 10px; }
  .state { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 14px; align-items: center; margin-top: 22px; padding: 21px; border: 1px solid var(--line); border-radius: 11px; background: white; }
  .state h1 { margin: 0; font-size: 16px; }
  .state p { margin: 5px 0 0; color: var(--muted); font-size: 9px; }
  .state a, .state button { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; text-decoration: none; }
  .state.error > span { display: grid; width: 36px; height: 36px; place-items: center; color: #a34e5d; border-radius: 50%; background: #fff0f3; }
  .spinner { width: 29px; height: 29px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .reader-shell { min-width: 0; }
  .reader-controls { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 13px; align-items: center; padding: 12px 16px; border: 1px solid var(--line); border-radius: 10px 10px 0 0; background: white; }
  .reader-controls label { display: flex; min-width: 0; gap: 7px; align-items: center; color: var(--muted); font-size: 8px; }
  .reader-controls input { min-width: 70px; width: 100%; }
  .reader-controls select { min-width: 76px; height: 30px; border: 1px solid var(--line); border-radius: 5px; background: white; font-size: 8px; }
  .novel-body { box-sizing: border-box; height: calc(100vh - var(--topbar-height) - 124px); min-height: 520px; overflow-y: auto; padding: 48px clamp(30px, 9vw, 96px) 90px; border: 1px solid var(--line); border-top: 0; border-radius: 0 0 10px 10px; scroll-behavior: smooth; }
  .novel-body.theme-paper { color: #33302a; background: #fbf7ec; }
  .novel-body.theme-white { color: #282b2e; background: white; }
  .novel-body.theme-dark { color: #d8d5cf; border-color: #34383c; background: #202326; }
  .novel-body p { margin: 0 0 1.15em; font-size: var(--reader-font); line-height: var(--reader-line); white-space: pre-wrap; overflow-wrap: anywhere; }
  .novel-body h2 { margin: 2em 0 1.2em; font-size: calc(var(--reader-font) * 1.3); text-align: center; }
  .novel-body hr { width: 44%; margin: 3em auto; border: 0; border-top: 1px solid currentColor; opacity: .25; }
  .embed { display: block; margin: 1.5em 0; padding: 14px; color: var(--pixiv-blue); border: 1px solid currentColor; border-radius: 7px; font-size: calc(var(--reader-font) * .72); text-align: center; text-decoration: none; }
  .embed.muted { color: #8a8e91; border-style: dashed; }
  .embed.external small { display: block; margin-top: 5px; color: var(--muted); font-size: .72em; overflow-wrap: anywhere; }
  .series-navigation { display: grid; grid-template-columns: minmax(180px, .7fr) repeat(2, minmax(0, 1fr)); gap: 10px; margin-top: 18px; }
  .series-navigation > a, .series-navigation > span { min-width: 0; padding: 14px 16px; border: 1px solid var(--line); border-radius: 10px; background: white; text-decoration: none; }
  .series-navigation small, .series-navigation strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .series-navigation small { color: var(--muted); font-size: 8px; }
  .series-navigation strong { margin-top: 5px; color: var(--text); font-size: 10px; }
  .series-overview { border-color: #bfe7ff !important; background: #f3faff !important; }
  .series-sibling.next { text-align: right; }
  .series-sibling.disabled { opacity: .55; }
  .series-navigation a:hover strong { color: var(--pixiv-blue); }
  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 720px) {
    .reading-page { padding: 14px 12px 86px; }
    .reading-header { grid-template-columns: minmax(0, 1fr) auto; gap: 6px 12px; }
    .reading-header > a { grid-column: 1 / -1; }
    .reader-controls { grid-template-columns: 1fr; gap: 9px; }
    .reader-controls label { display: grid; grid-template-columns: 38px minmax(0, 1fr); }
    .novel-body { height: calc(100vh - var(--topbar-height) - var(--bottom-nav-height) - 205px); min-height: 430px; padding: 34px 22px 72px; }
    .state { grid-template-columns: 38px minmax(0, 1fr); }
    .state a, .state button { grid-column: 1 / -1; text-align: center; }
    .series-navigation { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .series-overview { grid-column: 1 / -1; }
  }
</style>
