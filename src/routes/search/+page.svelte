<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkCard from "$lib/components/ArtworkCard.svelte";
  import ArtworkThumbnail from "$lib/components/ArtworkThumbnail.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import NovelCard from "$lib/components/NovelCard.svelte";
  import UserPreviewCard from "$lib/components/UserPreviewCard.svelte";
  import { m } from "$lib/i18n";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import {
    describeDataFailure,
    getTrendingTags,
    searchIllustrations,
    searchNovels,
    searchUsers,
  } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import {
    clearSearchHistory,
    readSearchHistory,
    recordSearchHistory,
    SEARCH_HISTORY_CHANGED_EVENT,
  } from "$lib/search-history";
  import type {
    IllustrationSummary,
    NovelSummary,
    TrendingTag,
    UserPreview,
  } from "$lib/types";

  const searchTypes = ["works", "novels", "users", "tags"] as const;
  type SearchType = (typeof searchTypes)[number];
  const searchTypeLabels: Record<SearchType, () => string> = {
    works: m.search_type_works,
    novels: m.search_type_novels,
    users: m.search_type_users,
    tags: m.search_type_tags,
  };
  type SearchSnapshot = {
    query: string;
    activeType: SearchType;
    illustrations: IllustrationSummary[];
    novels: NovelSummary[];
    users: UserPreview[];
    trending: TrendingTag[];
    history: string[];
    historyVisibleCount: number;
    resultStatus: "idle" | "loading" | "ready" | "error";
    trendingStatus: "idle" | "loading" | "ready" | "error";
    resultError: string;
    trendingError: string;
    paginationError: string;
    nextCursor: string | null;
    requestedKey: string;
    trendingSession: string;
  };
  const fallbackTags = [
    m.search_fallback_original,
    m.search_fallback_landscape,
    m.search_fallback_character_design,
    m.search_fallback_manga,
    m.search_fallback_light_novel,
    m.search_fallback_digital_art,
    m.search_fallback_wallpaper,
    m.search_fallback_weekly_popular,
  ];

  let query = $state("");
  let activeType = $state<SearchType>("works");
  let illustrations = $state<IllustrationSummary[]>([]);
  let novels = $state<NovelSummary[]>([]);
  let users = $state<UserPreview[]>([]);
  let trending = $state<TrendingTag[]>([]);
  let history = $state<string[]>([]);
  let historyVisibleCount = $state(20);
  let resultStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let trendingStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let resultError = $state("");
  let trendingError = $state("");
  let paginationError = $state("");
  let nextCursor = $state<string | null>(null);
  let loadingMore = $state(false);
  let requestedKey = $state("");
  let trendingSession = $state("");
  let requestSequence = 0;
  let submittedQuery = $derived(page.url.searchParams.get("q")?.trim() ?? "");
  let displayTags = $derived(trending.length ? trending.slice(0, 12) : []);
  let visibleHistory = $derived(history.slice(0, historyVisibleCount));

  export const snapshot = {
    capture: () => rememberNavigationView<SearchSnapshot>({
      query,
      activeType,
      illustrations,
      novels,
      users,
      trending,
      history,
      historyVisibleCount,
      resultStatus,
      trendingStatus,
      resultError,
      trendingError,
      paginationError,
      nextCursor,
      requestedKey,
      trendingSession,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<SearchSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      query = value.query;
      activeType = searchTypes.includes(value.activeType) ? value.activeType : "works";
      illustrations = value.illustrations;
      novels = value.novels;
      users = value.users;
      trending = value.trending;
      history = value.history;
      historyVisibleCount = Number.isInteger(value.historyVisibleCount)
        ? Math.max(20, value.historyVisibleCount)
        : 20;
      resultStatus = value.resultStatus === "loading" ? "idle" : value.resultStatus;
      trendingStatus = value.trendingStatus === "loading" ? "idle" : value.trendingStatus;
      resultError = value.resultError;
      trendingError = value.trendingError;
      paginationError = value.paginationError;
      nextCursor = value.nextCursor;
      requestedKey = value.resultStatus === "loading" ? "" : value.requestedKey;
      trendingSession = value.trendingStatus === "loading" ? "" : value.trendingSession;
      loadingMore = false;
    },
  };

  onMount(() => {
    const syncHistory = () => {
      history = readSearchHistory();
      historyVisibleCount = Math.max(20, Math.min(historyVisibleCount, history.length));
    };
    syncHistory();
    window.addEventListener(SEARCH_HISTORY_CHANGED_EVENT, syncHistory);
    return () => window.removeEventListener(SEARCH_HISTORY_CHANGED_EVENT, syncHistory);
  });

  $effect(() => {
    query = submittedQuery;
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && submittedQuery ? `${sessionKey}:${activeType}:${submittedQuery}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      illustrations = [];
      novels = [];
      users = [];
      nextCursor = null;
      resultStatus = "idle";
      resultError = "";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadResults(key, submittedQuery, activeType);
    }
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    if (!sessionKey || submittedQuery) return;
    if (sessionKey !== trendingSession) {
      trendingSession = sessionKey;
      void loadTrending(sessionKey);
    }
  });

  function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    const nextQuery = query.trim();
    if (nextQuery) saveHistory(nextQuery);
    void goto(nextQuery ? `/search?q=${encodeURIComponent(nextQuery)}` : "/search");
  }

  function saveHistory(value: string) {
    history = recordSearchHistory(value);
    historyVisibleCount = Math.max(20, Math.min(historyVisibleCount, history.length));
  }

  function clearHistory() {
    history = [];
    historyVisibleCount = 20;
    clearSearchHistory();
  }

  async function loadTrending(sessionKey: string) {
    trendingStatus = "loading";
    trendingError = "";
    try {
      const tags = await getTrendingTags();
      if (trendingSession !== sessionKey) return;
      trending = tags;
      trendingStatus = "ready";
    } catch (error) {
      if (trendingSession !== sessionKey) return;
      trendingError = describeDataFailure(error);
      trendingStatus = "error";
    }
  }

  async function loadResults(
    key: string,
    word: string,
    type: SearchType,
  ) {
    const sequence = ++requestSequence;
    resultStatus = "loading";
    resultError = "";
    paginationError = "";
    illustrations = [];
    novels = [];
    users = [];
    nextCursor = null;
    try {
      if (type === "users") {
        const result = await searchUsers(word);
        if (sequence !== requestSequence || key !== requestedKey) return;
        users = result.users;
        nextCursor = result.nextCursor ?? null;
      } else if (type === "novels") {
        const result = await searchNovels(word, "partial_match_for_tags");
        if (sequence !== requestSequence || key !== requestedKey) return;
        novels = result.novels;
        nextCursor = result.nextCursor ?? null;
      } else {
        const result = await searchIllustrations(
          word,
          type === "tags" ? "exact_match_for_tags" : "partial_match_for_tags",
        );
        if (sequence !== requestSequence || key !== requestedKey) return;
        illustrations = result.illustrations;
        nextCursor = result.nextCursor ?? null;
      }
      resultStatus = "ready";
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedKey) return;
      resultError = describeDataFailure(error);
      resultStatus = "error";
    }
  }

  async function loadMore() {
    const cursor = nextCursor;
    if (!cursor || loadingMore || !submittedQuery) return;
    const sequence = requestSequence;
    const key = requestedKey;
    loadingMore = true;
    paginationError = "";
    try {
      if (activeType === "users") {
        const result = await searchUsers(submittedQuery, cursor);
        if (sequence !== requestSequence || key !== requestedKey) return;
        const knownIds = new Set(users.map((item) => item.user.id));
        users = [...users, ...result.users.filter((item) => !knownIds.has(item.user.id))];
        nextCursor = result.nextCursor ?? null;
      } else if (activeType === "novels") {
        const result = await searchNovels(submittedQuery, "partial_match_for_tags", cursor);
        if (sequence !== requestSequence || key !== requestedKey) return;
        const knownIds = new Set(novels.map((item) => item.id));
        novels = [...novels, ...result.novels.filter((item) => !knownIds.has(item.id))];
        nextCursor = result.nextCursor ?? null;
      } else {
        const result = await searchIllustrations(
          submittedQuery,
          activeType === "tags" ? "exact_match_for_tags" : "partial_match_for_tags",
          cursor,
        );
        if (sequence !== requestSequence || key !== requestedKey) return;
        const knownIds = new Set(illustrations.map((item) => item.id));
        illustrations = [
          ...illustrations,
          ...result.illustrations.filter((item) => !knownIds.has(item.id)),
        ];
        nextCursor = result.nextCursor ?? null;
      }
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) {
        paginationError = describeDataFailure(error);
      }
    } finally {
      loadingMore = false;
    }
  }

  function retryResults() {
    if (requestedKey && submittedQuery) void loadResults(requestedKey, submittedQuery, activeType);
  }
</script>

<svelte:head><title>{m.search_title()} · PixNya</title></svelte:head>

<AppShell title={m.search_title()}>
  <main class="search-page">
    <header>
      <h1 class="page-title">{m.search_title()}</h1>
    </header>

    <form class="large-search" role="search" onsubmit={submitSearch}>
      <Icon name="search" size={21} />
      <input bind:value={query} type="search" maxlength="100" placeholder={m.search_placeholder()} aria-label={m.search_content_label()} />
      <button type="submit">{m.search_title()}</button>
    </form>

    {#if history.length > 0}
      <section class="history-card" aria-label={m.search_recent()}>
        <div class="history-heading"><Icon name="search" size={18} /><span><strong>{m.search_recent()}</strong></span><button type="button" onclick={clearHistory}>{m.search_clear()}</button></div>
        <div class="history-list">{#each visibleHistory as item}<a href={`/search?q=${encodeURIComponent(item)}`} onclick={() => saveHistory(item)}>{item}</a>{/each}</div>
        {#if visibleHistory.length < history.length}<button class="history-more" type="button" onclick={() => (historyVisibleCount += 20)}>{m.search_history_show_more()}</button>{/if}
      </section>
    {/if}

    <nav class="type-tabs" aria-label={m.search_type_label()}>
      {#each searchTypes as type}
        <button type="button" class:active={activeType === type} aria-pressed={activeType === type} onclick={() => (activeType = type)}>{searchTypeLabels[type]()}</button>
      {/each}
    </nav>

    {#if submittedQuery}
      <section class="results" aria-labelledby="results-heading">
        <div class="section-heading">
          <div><h2 id="results-heading">{m.search_results_heading({ query: submittedQuery, type: searchTypeLabels[activeType]() })}</h2><p>{m.search_results_source()}</p></div>
        </div>

        {#if !$sessionRestoring && !$session.loggedIn}
<div class="state-card"><Icon name="user" size={24} /><p>{m.search_sign_in_description()}</p><a href="/login">{m.search_go_to_login()}</a></div>
        {:else if resultStatus === "loading"}
          <div class="state-card"><span class="spinner"></span><p>{m.search_searching()}</p></div>
        {:else if resultStatus === "error"}
          <div class="state-card error" role="alert"><span>!</span><p>{resultError}</p><button type="button" onclick={retryResults}>{m.common_retry()}</button></div>
        {:else if activeType === "users" && users.length > 0}
          <div class="user-grid">{#each users as preview (preview.user.id)}<UserPreviewCard {preview} />{/each}</div>
        {:else if activeType === "novels" && novels.length > 0}
          <div class="novel-grid">{#each novels as novel (novel.id)}<NovelCard {novel} />{/each}</div>
        {:else if activeType !== "users" && activeType !== "novels" && illustrations.length > 0}
          <div class="result-grid">{#each illustrations as illustration, index (illustration.id)}<ArtworkCard {illustration} tone={(index % 6) + 1} />{/each}</div>
        {:else if resultStatus === "ready"}
          <p class="empty">{m.search_empty({ type: searchTypeLabels[activeType]() })}</p>
        {/if}

        {#if paginationError}<p class="pagination-error" role="alert">{paginationError}</p>{/if}
        {#if nextCursor && resultStatus === "ready"}
          <button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? m.common_loading() : m.common_load_more()}</button>
        {/if}
      </section>
    {:else}
      <section class="suggestions" aria-labelledby="suggestions-heading">
        <div class="suggestion-title"><span><Icon name="compass" size={19} /></span><div><h2 id="suggestions-heading">{m.search_trending_title()}</h2><p>{m.search_trending_source()}</p></div></div>

        {#if !$sessionRestoring && !$session.loggedIn}
          <div class="tag-grid fallback">
            {#each fallbackTags as tagMessage, index}
              {@const tag = tagMessage()}
              <a href={`/search?q=${encodeURIComponent(tag)}`} onclick={() => saveHistory(tag)}><span class="tag-art tone-{(index % 5) + 1}"></span><strong>#{tag}</strong><small>{m.search_trending_sign_in()}</small></a>
            {/each}
          </div>
        {:else if trendingStatus === "loading"}
          <div class="state-card"><span class="spinner"></span><p>{m.search_trending_loading()}</p></div>
        {:else if displayTags.length > 0}
          <div class="tag-grid">
            {#each displayTags as tag, index (tag.name)}
              <a href={`/search?q=${encodeURIComponent(tag.name)}`} onclick={() => saveHistory(tag.name)}>
                <span class="tag-art"><ArtworkThumbnail url={tag.illustration.thumbnailUrl} alt="" tone={(index % 6) + 1} /></span>
                <strong>#{tag.name}</strong><small>{tag.translatedName || m.search_view_related()}</small>
              </a>
            {/each}
          </div>
        {:else if trendingStatus === "error"}
          <div class="state-card error"><span>!</span><p>{trendingError}</p><button type="button" onclick={() => loadTrending(trendingSession)}>{m.common_retry()}</button></div>
        {/if}
      </section>
    {/if}

  </main>
</AppShell>

<style>
  .search-page { width: min(1040px, 100%); margin: 0 auto; padding: 34px 28px 52px; }
  header h1 { margin: 0; font-size: var(--type-title); }
  .large-search { display: grid; height: 54px; grid-template-columns: 24px minmax(0, 1fr) auto; gap: 11px; align-items: center; margin-top: 22px; padding: 0 6px 0 17px; color: #8b8b8b; border: 1px solid #dedede; border-radius: 10px; background: white; box-shadow: 0 5px 18px rgba(0,0,0,.04); }
  .large-search:focus-within { border-color: var(--pixiv-blue); box-shadow: 0 0 0 3px rgba(0,150,250,.1); }
  .large-search input { min-width: 0; border: 0; outline: 0; background: transparent; font-size: var(--type-body); }
  .large-search button { height: 42px; padding: 0 22px; color: white; border: 0; border-radius: 8px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .type-tabs { display: flex; gap: 30px; height: 56px; align-items: stretch; margin-top: 12px; border-bottom: 1px solid var(--line); }
  .type-tabs button { position: relative; padding: 0 5px; color: var(--muted); border: 0; background: transparent; cursor: pointer; font-size: var(--type-body); font-weight: 600; }
  .type-tabs button.active { color: var(--text); }
  .type-tabs button.active::after { position: absolute; right: 0; bottom: 0; left: 0; height: 3px; border-radius: 3px 3px 0 0; background: var(--pixiv-blue); content: ""; }
  .suggestions, .results { margin-top: 30px; }
  .suggestion-title { display: flex; gap: 11px; align-items: center; }
  .suggestion-title > span { display: grid; width: 38px; height: 38px; place-items: center; color: var(--pixiv-blue); border-radius: 50%; background: #eaf6ff; }
  h2 { margin: 0; font-size: var(--type-label); }
  .suggestion-title p, .section-heading p { margin: 4px 0 0; color: var(--muted); font-size: var(--type-caption); }
  .tag-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; margin-top: 16px; }
  .tag-grid a { display: grid; min-width: 0; grid-template-columns: 56px minmax(0, 1fr); grid-template-rows: 1fr 1fr; gap: 0 10px; align-items: center; padding: 9px; color: var(--text); border: 1px solid var(--line); border-radius: 8px; text-decoration: none; }
  .tag-grid a:hover { border-color: #b9dcf4; }
  .tag-art { position: relative; display: block; overflow: hidden; width: 56px; height: 56px; grid-row: 1 / -1; border-radius: 6px; }
  .tag-grid strong { align-self: end; overflow: hidden; font-size: var(--type-small); text-overflow: ellipsis; white-space: nowrap; }
  .tag-grid small { align-self: start; overflow: hidden; margin-top: 3px; color: var(--soft-muted); font-size: var(--type-caption); text-overflow: ellipsis; white-space: nowrap; }
  .section-heading { display: flex; align-items: flex-end; justify-content: space-between; margin-bottom: 16px; }
  .result-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 20px 14px; }
  .user-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
  .novel-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
  .state-card { display: flex; min-height: 100px; gap: 14px; align-items: center; justify-content: center; margin-top: 16px; padding: 18px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; font-size: var(--type-small); }
  .state-card p { margin: 0; }
  .state-card a, .state-card button { padding: 8px 14px; color: white; border: 0; border-radius: 16px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; text-decoration: none; }
  .state-card.error { color: #a65865; }
  .spinner { width: 26px; height: 26px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .empty { padding: 38px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; font-size: var(--type-small); text-align: center; }
  .pagination-error { color: #a65865; font-size: var(--type-caption); text-align: center; }
  .load-more { display: block; min-width: 116px; height: 36px; margin: 24px auto 0; color: #59636a; border: 1px solid var(--line); border-radius: 18px; background: white; cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .load-more:disabled { cursor: wait; opacity: .65; }
  .history-card { margin-top: 10px; padding: 13px 16px; border-radius: 9px; background: #f7f7f7; }
  .history-heading { display: flex; gap: 10px; align-items: center; color: #777; }
  .history-heading span { min-width: 0; flex: 1; }
  .history-heading strong { display: block; }
  .history-heading strong { color: var(--text); font-size: var(--type-small); }
  .history-heading button { color: var(--muted); border: 0; background: transparent; cursor: pointer; font-size: var(--type-body); }
  .history-list { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 13px; }
  .history-list a { padding: 6px 10px; color: #65717a; border-radius: 14px; background: white; font-size: var(--type-caption); text-decoration: none; }
  .history-more { display: block; min-height: 36px; margin: 10px auto 0; padding: 0 14px; color: var(--pixiv-blue); border: 0; background: transparent; cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .tone-1 { background: linear-gradient(145deg, #d9effb, #bad8e9); } .tone-2 { background: linear-gradient(145deg, #f3dfec, #dfbfd3); } .tone-3 { background: linear-gradient(145deg, #eee9cf, #d9c993); } .tone-4 { background: linear-gradient(145deg, #e0dcf1, #beb7df); } .tone-5 { background: linear-gradient(145deg, #dceee3, #bcd9c8); }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (min-width: 960px) { .large-search { display: none; } .type-tabs { margin-top: 22px; } }
  @media (max-width: 760px) {
    .search-page { padding: 26px 16px 42px; }
    .search-page > header h1 { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
    .tag-grid, .result-grid, .user-grid, .novel-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @media (max-width: 520px) { .tag-grid, .user-grid, .novel-grid { grid-template-columns: 1fr; } }
  @media (max-width: 420px) { .search-page { padding-right: 12px; padding-left: 12px; } .large-search button { padding: 0 15px; } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
