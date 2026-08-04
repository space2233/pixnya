<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import ContentTabs from "$lib/components/ContentTabs.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import NovelCard from "$lib/components/NovelCard.svelte";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import { describeDataFailure, getBookmarkedNovels, getFollowedNovels, getRankingNovels, getRecommendedNovels } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type { BookmarkRestrict, NovelPage, NovelSummary, RankingMode } from "$lib/types";

  const sections = ["推荐", "关注", "排行", "收藏"] as const;

  let novels = $state<NovelSummary[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let requestedSession = $state("");
  let selectedSection = $state<(typeof sections)[number]>("推荐");
  let bookmarkRestrict = $state<BookmarkRestrict>("public");
  let rankingMode = $state<RankingMode>("day");
  let requestSequence = 0;

  type NovelListSnapshot = {
    novels: NovelSummary[];
    nextCursor: string | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    requestedSession: string;
    selectedSection: (typeof sections)[number];
    bookmarkRestrict: BookmarkRestrict;
    rankingMode: RankingMode;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<NovelListSnapshot>({
      novels,
      nextCursor,
      status,
      errorMessage,
      requestedSession,
      selectedSection,
      bookmarkRestrict,
      rankingMode,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<NovelListSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      novels = value.novels;
      nextCursor = value.nextCursor;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      requestedSession = value.status === "loading" ? "" : value.requestedSession;
      selectedSection = value.selectedSection;
      bookmarkRestrict = value.bookmarkRestrict;
      rankingMode = value.rankingMode;
      loadingMore = false;
    },
  };

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey ? `${sessionKey}:${selectedSection}:${bookmarkRestrict}:${rankingMode}` : "";
    if (!key) {
      requestSequence += 1;
      requestedSession = "";
      novels = [];
      nextCursor = null;
      status = "idle";
      return;
    }
    if (key !== requestedSession) {
      requestedSession = key;
      void loadNovels(key);
    }
  });

  function requestPage(cursor?: string): Promise<NovelPage> {
    if (selectedSection === "关注") return getFollowedNovels(cursor);
    if (selectedSection === "排行") return getRankingNovels(rankingMode, cursor);
    if (selectedSection === "收藏") return getBookmarkedNovels(bookmarkRestrict, cursor);
    return getRecommendedNovels(cursor);
  }

  async function loadNovels(key: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    try {
      const page = await requestPage();
      if (sequence !== requestSequence || key !== requestedSession) return;
      novels = page.novels;
      nextCursor = page.nextCursor ?? null;
      status = "ready";
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedSession) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadMore() {
    const cursor = nextCursor;
    if (!cursor || loadingMore) return;
    loadingMore = true;
    errorMessage = "";
    try {
      const page = await requestPage(cursor);
      const known = new Set(novels.map((novel) => novel.id));
      novels = [...novels, ...page.novels.filter((novel) => !known.has(novel.id))];
      nextCursor = page.nextCursor ?? null;
    } catch (error) {
      errorMessage = describeDataFailure(error);
    } finally {
      loadingMore = false;
    }
  }
</script>

<svelte:head><title>小说 · PixNya</title></svelte:head>

<AppShell title="小说">
  <ContentTabs />
  <main class="novel-page">
    <header><div><h1>小说</h1><p>阅读推荐、关注作者、排行榜与账号收藏，阅读位置保存在当前设备。</p></div></header>
    <div class="novel-toolbar">
      <nav aria-label="小说内容类型">{#each sections as section}<button type="button" class:active={selectedSection === section} onclick={() => (selectedSection = section)}>{section}</button>{/each}</nav>
      {#if selectedSection === "排行"}<select bind:value={rankingMode} aria-label="小说排行周期"><option value="day">今日</option><option value="week">本周</option><option value="month">本月</option></select>{/if}
      {#if selectedSection === "收藏"}<select bind:value={bookmarkRestrict} aria-label="小说收藏范围"><option value="public">公开收藏</option><option value="private">非公开收藏</option></select>{/if}
    </div>
    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state"><Icon name="user" size={27} /><div><h2>登录后载入小说</h2><p>列表、正文和封面通过登录后的 App API 获取。</p></div><a href="/login?mode=standard">前往登录</a></section>
    {:else if status === "loading"}
      <section class="state"><span class="spinner"></span><div><h2>正在载入{selectedSection}小说</h2><p>正在读取标题、封面、字数与系列信息…</p></div></section>
    {:else if status === "error"}
      <section class="state error" role="alert"><span>!</span><div><h2>小说载入失败</h2><p>{errorMessage}</p></div><button type="button" onclick={() => loadNovels(requestedSession)}>重试</button></section>
    {:else if status === "ready"}
      {#if novels.length}<div class="novel-grid">{#each novels as novel (novel.id)}<NovelCard {novel} />{/each}</div>{:else}<p class="empty">Pixiv 本次没有返回推荐小说。</p>{/if}
      {#if errorMessage}<p class="paging-error" role="alert">{errorMessage}</p>{/if}
      {#if nextCursor}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? "正在载入…" : "加载更多小说"}</button>{/if}
    {/if}
  </main>
</AppShell>

<style>
  .novel-page { width: min(1060px,100%); margin: 0 auto; padding: 26px 28px 64px; }
  header h1 { margin: 0; font-size: 22px; }
  header p { margin: 7px 0 0; color: var(--muted); font-size: 10px; }
  .novel-toolbar { display: flex; gap: 10px; align-items: center; justify-content: space-between; margin-top: 18px; } .novel-toolbar nav { display: flex; gap: 4px; padding: 4px; border-radius: 21px; background: #f3f3f3; } .novel-toolbar button { min-width: 58px; height: 31px; color: #777; border: 0; border-radius: 16px; background: transparent; cursor: pointer; font-size: 9px; font-weight: 700; } .novel-toolbar button.active { color: #333; background: white; box-shadow: 0 1px 4px rgba(0,0,0,.08); } .novel-toolbar select { height: 34px; padding: 0 11px; border: 1px solid var(--line); border-radius: 17px; background: white; font-size: 9px; }
  .novel-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 16px; margin-top: 22px; }
  .state { display: grid; grid-template-columns: 44px minmax(0,1fr) auto; gap: 14px; align-items: center; margin-top: 22px; padding: 20px; border: 1px solid var(--line); border-radius: 11px; background: white; }
  .state h2 { margin: 0; font-size: 15px; } .state p { margin: 5px 0 0; color: var(--muted); font-size: 9px; }
  .state a, .state button, .load-more { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; text-decoration: none; }
  .state.error > span { display: grid; width: 36px; height: 36px; place-items: center; color: #a34e5d; border-radius: 50%; background: #fff0f3; }
  .spinner { width: 29px; height: 29px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .empty { margin-top: 22px; padding: 40px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; text-align: center; }
  .paging-error { color: #a34e5d; font-size: 9px; text-align: center; }
  .load-more { display: block; min-width: 145px; margin: 24px auto 0; color: #59636a; border: 1px solid var(--line); background: white; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 760px) { .novel-grid { grid-template-columns: 1fr; } }
  @media (max-width: 620px) { .novel-page { padding: 18px 14px 90px; } .novel-toolbar { align-items: stretch; flex-direction: column; } .novel-toolbar nav { width: 100%; } .novel-toolbar nav button { min-width: 0; flex: 1; } .novel-toolbar select { align-self: flex-end; } .state { grid-template-columns: 38px minmax(0,1fr); } .state a, .state button { grid-column: 1 / -1; text-align: center; } }
</style>
