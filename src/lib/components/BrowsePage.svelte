<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkCard from "$lib/components/ArtworkCard.svelte";
  import ContentTabs from "$lib/components/ContentTabs.svelte";
  import FollowingTabs from "$lib/components/FollowingTabs.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/i18n";
  import { loadHomeTagCache, saveHomeTagCache } from "$lib/home-tag-cache";
  import {
    describeDataFailure,
    getBookmarkedIllustrations,
    getFollowedIllustrations,
    getRankingIllustrations,
    getRecommendedIllustrations,
    getRecommendedManga,
    getTrendingTags,
  } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type {
    BookmarkRestrict,
    IllustrationPage,
    IllustrationSummary,
    RankingMode,
    TrendingTag,
  } from "$lib/types";

  export type BrowseSection =
    | "home"
    | "artworks"
    | "manga"
    | "novels"
    | "following"
    | "discover"
    | "ranking"
    | "bookmarks";

  export type BrowsePageSnapshot = {
    selectedFilter: string;
    illustrations: IllustrationSummary[];
    trendingTags: TrendingTag[];
    cachedTagNames: string[];
    dataStatus: "idle" | "loading" | "ready" | "error";
    dataError: string;
    nextCursor: string | null;
    paginationError: string;
    requestedKey: string;
    trendingSession: string;
  };

  type Definition = {
    title: () => string;
    heading: () => string;
    subtitle: () => string;
    sectionTitle: () => string;
    sectionHint: () => string;
    filters: readonly BrowseFilter[];
    tabs: boolean;
    layout: "artwork" | "portrait" | "novel";
  };

  type BrowseFilter =
    | "recommended"
    | "following"
    | "popular"
    | "series"
    | "short"
    | "for_you"
    | "trending_tags"
    | "today"
    | "week"
    | "month"
    | "public"
    | "private";

  const filterLabels: Record<BrowseFilter, () => string> = {
    recommended: m.filter_recommended,
    following: m.filter_following,
    popular: m.filter_popular,
    series: m.filter_series,
    short: m.filter_short,
    for_you: m.filter_for_you,
    trending_tags: m.filter_trending_tags,
    today: m.filter_today,
    week: m.filter_week,
    month: m.filter_month,
    public: m.filter_public,
    private: m.filter_private,
  };

  const definitions: Record<BrowseSection, Definition> = {
    home: {
      title: m.navigation_home,
      heading: m.browse_home_heading,
      subtitle: m.browse_home_subtitle,
      sectionTitle: m.browse_home_section,
      sectionHint: m.browse_home_hint,
      filters: ["recommended", "following", "popular"],
      tabs: true,
      layout: "artwork",
    },
    artworks: {
      title: m.navigation_artworks,
      heading: m.navigation_artworks,
      subtitle: m.browse_artworks_subtitle,
      sectionTitle: m.browse_artworks_section,
      sectionHint: m.browse_artworks_hint,
      filters: ["recommended", "following"],
      tabs: true,
      layout: "artwork",
    },
    manga: {
      title: m.navigation_manga,
      heading: m.navigation_manga,
      subtitle: m.browse_manga_subtitle,
      sectionTitle: m.browse_manga_section,
      sectionHint: m.browse_manga_hint,
      filters: [],
      tabs: true,
      layout: "portrait",
    },
    novels: {
      title: m.navigation_novels,
      heading: m.navigation_novels,
      subtitle: m.browse_novels_subtitle,
      sectionTitle: m.browse_novels_section,
      sectionHint: m.browse_novels_hint,
      filters: ["recommended", "series", "short"],
      tabs: true,
      layout: "novel",
    },
    following: {
      title: m.navigation_following,
      heading: m.navigation_following,
      subtitle: m.browse_following_subtitle,
      sectionTitle: m.browse_following_section,
      sectionHint: m.browse_following_hint,
      filters: [],
      tabs: false,
      layout: "artwork",
    },
    discover: {
      title: m.navigation_discover,
      heading: m.navigation_discover,
      subtitle: m.browse_discover_subtitle,
      sectionTitle: m.browse_discover_section,
      sectionHint: m.browse_discover_hint,
      filters: ["for_you", "trending_tags"],
      tabs: false,
      layout: "artwork",
    },
    ranking: {
      title: m.navigation_ranking,
      heading: m.navigation_ranking,
      subtitle: m.browse_ranking_subtitle,
      sectionTitle: m.browse_ranking_section,
      sectionHint: m.browse_ranking_hint,
      filters: ["today", "week", "month"],
      tabs: false,
      layout: "artwork",
    },
    bookmarks: {
      title: m.navigation_bookmarks,
      heading: m.navigation_bookmarks,
      subtitle: m.browse_bookmarks_subtitle,
      sectionTitle: m.browse_bookmarks_section,
      sectionHint: m.browse_bookmarks_hint,
      filters: ["public", "private"],
      tabs: false,
      layout: "artwork",
    },
  };

  let { section }: { section: BrowseSection } = $props();
  let definition = $derived(definitions[section]);
  let selectedFilter = $state<BrowseFilter | "">("");
  let illustrations = $state<IllustrationSummary[]>([]);
  let trendingTags = $state<TrendingTag[]>([]);
  let cachedTagNames = $state<string[]>(loadHomeTagCache());
  let dataStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let dataError = $state("");
  let nextCursor = $state<string | null>(null);
  let loadingMore = $state(false);
  let paginationError = $state("");
  let requestedKey = $state("");
  let trendingSession = $state("");
  let requestSequence = 0;
  let trendingSequence = 0;
  let supportsContent = $derived(
    ["home", "artworks", "manga", "following", "discover", "ranking", "bookmarks"].includes(section),
  );
  let showContent = $derived(supportsContent && dataStatus === "ready");
  let featuredIllustrations = $derived(section === "home" ? illustrations.slice(0, 6) : []);
  let collectionIllustrations = $derived(
    section === "home" ? illustrations.slice(6) : illustrations,
  );
  let topicTags = $derived(
    (trendingTags.length > 0
      ? trendingTags.slice(0, 12).map((tag) => tag.name)
      : cachedTagNames
    ).map((tag) => `#${tag}`),
  );

  $effect.pre(() => {
    if (!selectedFilter || !definition.filters.includes(selectedFilter)) {
      selectedFilter = definition.filters[0] ?? "";
    }
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && supportsContent ? `${sessionKey}:${section}:${selectedFilter}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      illustrations = [];
      nextCursor = null;
      loadingMore = false;
      paginationError = "";
      dataStatus = "idle";
      dataError = "";
      return;
    }
    if (requestedKey !== key) {
      requestedKey = key;
      void loadContent(key);
    }
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    if (!sessionKey || (section !== "home" && section !== "discover")) {
      trendingSequence += 1;
      trendingSession = "";
      trendingTags = [];
      return;
    }
    if (trendingSession !== sessionKey) {
      trendingSession = sessionKey;
      void loadTrendingTags(sessionKey);
    }
  });

  async function requestContentPage(cursor?: string): Promise<IllustrationPage> {
    if (section === "home" || section === "artworks") {
      if (selectedFilter === "following") return getFollowedIllustrations(cursor);
      if (selectedFilter === "popular") return getRankingIllustrations("day", cursor);
      return getRecommendedIllustrations(cursor);
    }
    if (section === "manga") return getRecommendedManga(cursor);
    if (section === "following") return getFollowedIllustrations(cursor);
    if (section === "ranking") {
      const mode: RankingMode = selectedFilter === "week" ? "week" : selectedFilter === "month" ? "month" : "day";
      return getRankingIllustrations(mode, cursor);
    }
    if (section === "bookmarks") {
      const restrict: BookmarkRestrict = selectedFilter === "private" ? "private" : "public";
      return getBookmarkedIllustrations(restrict, cursor);
    }
    if (section === "discover" && selectedFilter === "trending_tags") {
      const tags = await getTrendingTags();
      acceptTrendingTags(tags);
      return { illustrations: tags.map((tag) => tag.illustration), nextCursor: null };
    }
    return getRecommendedIllustrations(cursor);
  }

  async function loadContent(key: string) {
    const sequence = ++requestSequence;
    dataStatus = "loading";
    dataError = "";
    nextCursor = null;
    paginationError = "";
    try {
      const page = await requestContentPage();
      if (sequence !== requestSequence || requestedKey !== key) return;
      illustrations = page.illustrations;
      nextCursor = page.nextCursor ?? null;
      dataStatus = "ready";
    } catch (error) {
      if (sequence !== requestSequence || requestedKey !== key) return;
      dataError = describeDataFailure(error);
      dataStatus = "error";
    }
  }

  async function loadTrendingTags(sessionKey: string) {
    const sequence = ++trendingSequence;
    try {
      const tags = await getTrendingTags();
      if (sequence === trendingSequence && trendingSession === sessionKey) acceptTrendingTags(tags);
    } catch {
      if (sequence === trendingSequence && trendingSession === sessionKey) trendingTags = [];
    }
  }

  function acceptTrendingTags(tags: TrendingTag[]) {
    if (tags.length === 0) return;
    trendingTags = tags;
    cachedTagNames = saveHomeTagCache(tags);
  }

  function retryContent() {
    if (requestedKey) void loadContent(requestedKey);
  }

  async function loadMoreContent() {
    const cursor = nextCursor;
    if (!cursor || loadingMore) return;
    const sequence = ++requestSequence;
    const key = requestedKey;
    loadingMore = true;
    paginationError = "";
    try {
      const page = await requestContentPage(cursor);
      if (sequence !== requestSequence || requestedKey !== key) return;
      const knownIds = new Set(illustrations.map((illustration) => illustration.id));
      illustrations = [
        ...illustrations,
        ...page.illustrations.filter((illustration) => !knownIds.has(illustration.id)),
      ];
      nextCursor = page.nextCursor ?? null;
    } catch (error) {
      if (sequence !== requestSequence || requestedKey !== key) return;
      paginationError = describeDataFailure(error);
    } finally {
      loadingMore = false;
    }
  }

  export function captureSnapshot(): BrowsePageSnapshot {
    return {
      selectedFilter,
      illustrations,
      trendingTags,
      cachedTagNames,
      dataStatus,
      dataError,
      nextCursor,
      paginationError,
      requestedKey,
      trendingSession,
    };
  }

  export function restoreSnapshot(snapshot: BrowsePageSnapshot): void {
    requestSequence += 1;
    trendingSequence += 1;
    selectedFilter = definition.filters.includes(snapshot.selectedFilter as BrowseFilter)
      ? (snapshot.selectedFilter as BrowseFilter)
      : "";
    illustrations = snapshot.illustrations;
    trendingTags = snapshot.trendingTags;
    cachedTagNames = snapshot.cachedTagNames;
    dataStatus = snapshot.dataStatus === "loading" ? "idle" : snapshot.dataStatus;
    dataError = snapshot.dataError;
    nextCursor = snapshot.nextCursor;
    paginationError = snapshot.paginationError;
    requestedKey = snapshot.dataStatus === "loading" ? "" : snapshot.requestedKey;
    trendingSession = snapshot.trendingSession;
    loadingMore = false;
  }
</script>

<svelte:head>
  <title>{definition.title()} · PixNya</title>
</svelte:head>

<AppShell title={definition.title()}>
  {#if definition.tabs}<ContentTabs />{/if}

  <div class="browse-page" class:with-tabs={definition.tabs}>
    {#if section === "following"}<FollowingTabs />{/if}
    {#if (section === "home" || section === "discover") && topicTags.length > 0}
      <div class="topic-strip" aria-label={m.browse_recommended_tags()}>
        {#each topicTags as topic, index}
          <a href={`/search?q=${encodeURIComponent(topic.slice(1))}`} class:accent={index === 0}>
            {topic}
          </a>
        {/each}
      </div>
    {/if}

    <header
      class="browse-heading"
      class:repeated-title={section !== "home"}
      class:home-feed-only={section === "home" && ($sessionRestoring || $session.loggedIn)}
    >
      {#if section !== "home" || (!$sessionRestoring && !$session.loggedIn)}
        <div>
          <h1>{definition.heading()}</h1>
          <p>{definition.subtitle()}</p>
        </div>
      {/if}
      {#if definition.filters.length > 0}
        <nav class="filter-tabs" aria-label={m.browse_filter_label({ title: definition.title() })}>
          {#each definition.filters as filter}
            <button
              type="button"
              class:active={selectedFilter === filter}
              aria-pressed={selectedFilter === filter}
              onclick={() => (selectedFilter = filter)}
            >{filterLabels[filter]()}</button>
          {/each}
        </nav>
      {/if}
    </header>

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="account-callout">
        <span class="callout-icon"><Icon name="user" size={21} /></span>
        <div>
          <strong>{m.browse_sign_in_title()}</strong>
          <p>{m.browse_sign_in_description()}</p>
        </div>
        <div class="callout-actions">
          <a class="secondary-link" href="/settings/network">{m.browse_check_connection()}</a>
          <a class="primary-link" href="/login?mode=standard">{m.browse_go_to_login()}</a>
        </div>
      </section>
    {:else if supportsContent && dataStatus === "error"}
      <section class="data-error" role="alert">
        <div><strong>{m.browse_load_failed()}</strong><p>{dataError}</p></div>
        <button type="button" onclick={retryContent}>{m.common_retry()}</button>
      </section>
    {/if}

    {#if section === "home"}
      <section class="content-section featured-section" aria-labelledby="featured-title">
        <header class="section-heading">
          <div><h2 id="featured-title">{m.browse_featured_title()}</h2><p>{m.browse_featured_hint()}</p></div>
          <a href="/following">{m.browse_view_new()} <span aria-hidden="true">›</span></a>
        </header>
        <div class="featured-grid" aria-label={showContent ? m.browse_featured_title() : m.browse_featured_loading()}>
          {#if showContent && featuredIllustrations.length > 0}
            {#each featuredIllustrations as illustration, index (illustration.id)}
              <ArtworkCard {illustration} tone={(index % 6) + 1} />
            {/each}
          {:else}
            {#each Array(6) as _, index}
              <article class="work-card loading-card">
                <div class="work-cover tone-{(index % 6) + 1}"><span></span><i></i></div>
                <div class="skeleton-line title-line"></div>
                <div class="skeleton-line author-line"></div>
              </article>
            {/each}
          {/if}
        </div>
      </section>
    {/if}

    <section class="content-section" aria-labelledby="collection-title">
      <header class="section-heading">
        <div>
          <h2 id="collection-title">{definition.sectionTitle()}</h2>
          <p>{selectedFilter ? `${filterLabels[selectedFilter]()} · ` : ""}{definition.sectionHint()}</p>
        </div>
        {#if section === "bookmarks"}
          <span class="local-state"><Icon name="shield" size={14} /> {m.browse_local_state({ state: $sessionRestoring ? m.session_restoring() : $session.loggedIn ? m.session_signed_in() : m.session_signed_out() })}</span>
        {/if}
      </header>

      <div
        class="collection-grid"
        class:portrait={definition.layout === "portrait"}
        class:novel={definition.layout === "novel"}
        aria-label={showContent ? definition.sectionTitle() : m.browse_waiting_for({ title: definition.sectionTitle() })}
      >
        {#if showContent && collectionIllustrations.length > 0}
          {#each collectionIllustrations as illustration, index (illustration.id)}
            <ArtworkCard
              {illustration}
              tone={((index + 2) % 6) + 1}
              rank={section === "ranking" ? index + 1 : undefined}
            />
          {/each}
        {:else if showContent}
          <p class="empty-state">{m.browse_empty()}</p>
        {:else if !supportsContent}
          <p class="empty-state">{m.browse_open_novels()}</p>
        {:else}
          {#each Array(definition.layout === "novel" ? 6 : 8) as _, index}
            <article class="work-card loading-card">
              <div class="work-cover tone-{((index + 2) % 6) + 1}">
                {#if section === "ranking"}<b class="rank-number">{index + 1}</b>{/if}
                <span></span><i></i>
              </div>
              <div class="card-copy">
                <div class="skeleton-line title-line"></div>
                <div class="skeleton-line author-line"></div>
                {#if definition.layout === "novel"}
                  <div class="skeleton-line excerpt-line"></div>
                  <small>{m.browse_novel_metadata()}</small>
                {/if}
              </div>
            </article>
          {/each}
        {/if}
      </div>
      {#if showContent && nextCursor}
        <div class="load-more">
          {#if paginationError}<p role="alert">{paginationError}</p>{/if}
          <button type="button" disabled={loadingMore} onclick={loadMoreContent}>
            {loadingMore ? m.common_loading() : m.common_load_more()}
          </button>
        </div>
      {/if}
    </section>

    <p class="page-footnote">{m.browse_token_notice()}</p>
  </div>
</AppShell>

<style>
  .browse-page {
    width: min(1120px, 100%);
    margin: 0 auto;
    padding: 28px 28px 52px;
  }

  .topic-strip {
    display: flex;
    gap: 9px;
    margin: 0 0 28px;
    overflow-x: auto;
    padding: 2px 0 8px;
    scrollbar-width: none;
  }

  .topic-strip::-webkit-scrollbar {
    display: none;
  }

  .topic-strip a {
    flex: 0 0 auto;
    padding: 10px 16px;
    color: #65717a;
    border-radius: 6px;
    background: #edf2f5;
    font-size: 11px;
    font-weight: 700;
    text-decoration: none;
  }

  .topic-strip a:nth-child(2n) {
    color: #7e6782;
    background: #f2eaf3;
  }

  .topic-strip a:nth-child(3n) {
    color: #746c4f;
    background: #f2efdf;
  }

  .topic-strip a.accent {
    color: white;
    background: var(--pixiv-blue);
  }

  .browse-heading {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    justify-content: space-between;
  }

  .browse-heading.home-feed-only {
    justify-content: flex-start;
  }

  .browse-heading h1 {
    margin: 0;
    font-size: 24px;
    letter-spacing: -0.02em;
  }

  .browse-heading p {
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 11px;
  }

  .filter-tabs {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 22px;
    background: #f4f4f4;
  }

  .filter-tabs button {
    min-width: 62px;
    height: 32px;
    padding: 0 13px;
    color: #777;
    border: 0;
    border-radius: 17px;
    background: transparent;
    cursor: pointer;
    font-size: 10px;
    font-weight: 600;
  }

  .filter-tabs button.active {
    color: #333;
    background: white;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
  }

  .account-callout {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    margin-top: 22px;
    padding: 15px 17px;
    border: 1px solid #dceefb;
    border-radius: 10px;
    background: #f5fbff;
  }

  .callout-icon {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    color: var(--pixiv-blue);
    border-radius: 50%;
    background: white;
  }

  .account-callout strong {
    font-size: 12px;
  }

  .account-callout p {
    margin: 4px 0 0;
    color: #6e8492;
    font-size: 9px;
    line-height: 1.5;
  }

  .callout-actions {
    display: flex;
    gap: 8px;
  }

  .callout-actions a {
    display: grid;
    min-height: 36px;
    place-items: center;
    padding: 0 15px;
    border-radius: 18px;
    font-size: 10px;
    font-weight: 700;
    text-decoration: none;
  }

  .secondary-link {
    color: #526672;
    border: 1px solid #d5e6f1;
    background: white;
  }

  .primary-link {
    color: white;
    background: var(--pixiv-blue);
  }

  .data-error {
    display: flex;
    gap: 16px;
    align-items: center;
    justify-content: space-between;
    margin-top: 22px;
    padding: 14px 16px;
    border: 1px solid #f0d9dc;
    border-radius: 10px;
    background: #fff8f9;
  }

  .data-error strong { font-size: 12px; }

  .data-error p {
    margin: 4px 0 0;
    color: #8b686d;
    font-size: 9px;
  }

  .data-error button {
    flex: 0 0 auto;
    min-width: 70px;
    height: 34px;
    color: white;
    border: 0;
    border-radius: 17px;
    background: var(--pixiv-blue);
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
  }

  .content-section {
    margin-top: 36px;
  }

  .section-heading {
    display: flex;
    min-height: 34px;
    align-items: flex-end;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  .section-heading h2 {
    margin: 0;
    font-size: 18px;
  }

  .section-heading p {
    margin: 5px 0 0;
    color: var(--muted);
    font-size: 9px;
  }

  .section-heading > a {
    color: var(--muted);
    font-size: 10px;
    text-decoration: none;
  }

  .section-heading > a:hover {
    color: var(--pixiv-blue);
  }

  .local-state {
    display: flex;
    gap: 5px;
    align-items: center;
    color: var(--muted);
    font-size: 9px;
  }

  .featured-grid,
  .collection-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 22px 16px;
  }

  .featured-grid {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .work-card {
    min-width: 0;
  }

  .empty-state {
    grid-column: 1 / -1;
    margin: 0;
    padding: 40px 16px;
    color: var(--muted);
    border: 1px dashed var(--line);
    border-radius: 8px;
    font-size: 10px;
    text-align: center;
  }

  .load-more {
    display: grid;
    gap: 9px;
    justify-items: center;
    margin-top: 26px;
  }

  .load-more p {
    margin: 0;
    color: #a05a63;
    font-size: 9px;
  }

  .load-more button {
    min-width: 122px;
    height: 36px;
    color: #555f66;
    border: 1px solid var(--line);
    border-radius: 18px;
    background: white;
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
  }

  .load-more button:hover:not(:disabled) {
    color: var(--pixiv-blue);
    border-color: #b8def7;
  }

  .load-more button:disabled { cursor: wait; opacity: 0.65; }

  .work-cover {
    position: relative;
    display: grid;
    overflow: hidden;
    aspect-ratio: 1;
    place-items: center;
    border-radius: 7px;
    background: #eaf1f5;
  }

  .portrait .work-cover {
    aspect-ratio: 0.78;
  }

  .work-cover > span {
    width: 45%;
    aspect-ratio: 1;
    border-radius: 48% 52% 55% 45%;
    background: rgba(255, 255, 255, 0.62);
    transform: rotate(18deg);
  }

  .work-cover > i {
    position: absolute;
    right: -10%;
    bottom: -24%;
    width: 75%;
    aspect-ratio: 1;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.35);
  }

  .tone-1 { background: linear-gradient(145deg, #e1f3fb, #cbe1ee); }
  .tone-2 { background: linear-gradient(145deg, #f4e6ef, #e7ccdc); }
  .tone-3 { background: linear-gradient(145deg, #f4f0dc, #dfd4ae); }
  .tone-4 { background: linear-gradient(145deg, #e7e4f5, #d1caeb); }
  .tone-5 { background: linear-gradient(145deg, #e3f1e9, #c9e1d3); }
  .tone-6 { background: linear-gradient(145deg, #f5e8df, #e9cdbc); }

  .skeleton-line {
    height: 7px;
    border-radius: 4px;
    background: #eceff1;
  }

  .title-line {
    width: 78%;
    margin-top: 9px;
  }

  .author-line {
    width: 48%;
    height: 6px;
    margin-top: 7px;
    background: #f1f2f3;
  }

  .rank-number {
    position: absolute;
    z-index: 1;
    top: 9px;
    left: 9px;
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    color: #555;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.9);
    font-size: 10px;
  }

  .collection-grid.novel {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .novel .work-card {
    display: grid;
    grid-template-columns: 108px minmax(0, 1fr);
    gap: 14px;
    padding: 13px;
    border: 1px solid var(--line);
    border-radius: 9px;
  }

  .novel .work-cover {
    aspect-ratio: 0.72;
  }

  .novel .title-line {
    width: 68%;
    margin-top: 7px;
  }

  .novel .excerpt-line {
    width: 94%;
    height: 36px;
    margin-top: 20px;
  }

  .novel small {
    display: block;
    margin-top: 12px;
    color: var(--soft-muted);
    font-size: 8px;
  }

  .page-footnote {
    margin: 38px 0 0;
    color: var(--soft-muted);
    font-size: 9px;
    text-align: center;
  }

  @media (max-width: 959px) {
    .browse-page {
      padding: 24px 20px 42px;
    }

    .browse-heading.repeated-title h1 {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .browse-heading.repeated-title p {
      margin-top: 0;
    }

    .featured-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 720px) {
    .topic-strip {
      margin-right: -20px;
    }

    .browse-heading {
      align-items: stretch;
      flex-direction: column;
    }

    .browse-heading h1 {
      font-size: 21px;
    }

    .filter-tabs {
      align-self: flex-start;
    }

    .account-callout {
      grid-template-columns: 38px minmax(0, 1fr);
    }

    .callout-icon {
      width: 38px;
      height: 38px;
    }

    .callout-actions {
      grid-column: 1 / -1;
      justify-content: flex-end;
    }

    .collection-grid,
    .featured-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 18px 11px;
    }

    .collection-grid.novel {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 420px) {
    .browse-page {
      padding-right: 12px;
      padding-left: 12px;
    }

    .topic-strip {
      margin-right: -12px;
    }

    .filter-tabs {
      width: 100%;
    }

    .filter-tabs button {
      min-width: 0;
      flex: 1;
    }

    .account-callout {
      align-items: start;
      padding: 14px;
    }

    .callout-actions a {
      flex: 1;
    }

    .novel .work-card {
      grid-template-columns: 88px minmax(0, 1fr);
    }
  }
</style>
