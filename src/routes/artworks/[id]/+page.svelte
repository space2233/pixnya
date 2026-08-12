<script lang="ts">
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkImageViewer from "$lib/components/ArtworkImageViewer.svelte";
  import ArtworkCard from "$lib/components/ArtworkCard.svelte";
  import ArtworkComments from "$lib/components/ArtworkComments.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import {
    publishIllustrationBookmarkState,
    resolveIllustrationBookmarkState,
    subscribeIllustrationBookmarkState,
  } from "$lib/illustration-bookmark-state";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import UgoiraPlayer from "$lib/components/UgoiraPlayer.svelte";
  import { resolveArtworkSeriesNavigation, type ArtworkSeriesNavigation } from "$lib/artwork-series-navigation";
  import {
    describeDataFailure,
    getIllustrationDetail,
    getRelatedIllustrations,
    enqueueDownload,
    recordBrowsingHistory,
    setIllustrationBookmark,
    startUgoiraExport,
    getUgoiraExportTask,
    cancelUgoiraExportTask,
  } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { r18DefaultVisible } from "$lib/preferences";
  import { recordSearchHistory } from "$lib/search-history";
  import { session, sessionRestoring } from "$lib/session";
  import type { BookmarkRestrict, IllustrationDetail, IllustrationSummary, UgoiraExportFormat, UgoiraExportTask } from "$lib/types";

  let detail = $state<IllustrationDetail | null>(null);
  let related = $state<IllustrationSummary[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let relatedError = $state("");
  let loadingMore = $state(false);
  let revealRestricted = $state(false);
  let bookmarked = $state(false);
  let bookmarkRestrict = $state<BookmarkRestrict>("public");
  let bookmarkPending = $state(false);
  let bookmarkError = $state("");
  let downloadPending = $state(false);
  let downloadMessage = $state("");
  let ugoiraExportFormat = $state<UgoiraExportFormat>("gif");
  let ugoiraExportTask = $state<UgoiraExportTask | null>(null);
  let ugoiraExportError = $state("");
  let ugoiraExportSupported = $state(false);
  let exportPollTimer: ReturnType<typeof setTimeout> | null = null;
  let seriesNavigation = $state<ArtworkSeriesNavigation | null>(null);
  let seriesNavigationLoading = $state(false);
  let requestedKey = $state("");
  let requestSequence = 0;
  let illustrationId = $derived(page.params.id ?? "");
  let bookmarkAccount = $derived($session.loggedIn ? ($session.user?.id ?? "logged-in") : "");
  let restricted = $derived((detail?.illustration.xRestrict ?? 0) > 0);
  let caption = $derived(detail ? plainPixivText(detail.caption) : "");
  let viewerPages = $derived((detail?.pages ?? []).map((image) => ({
    pageIndex: image.pageIndex,
    alt: m.artwork_page_alt({
      title: detail?.illustration.title || m.common_untitled(),
      page: image.pageIndex + 1,
    }),
    previewUrl: image.displayUrl ?? image.originalUrl,
    originalUrl: image.originalUrl ?? image.displayUrl,
  })));
  let ugoiraExportActive = $derived(ugoiraExportTask !== null && !["completed", "failed", "cancelled"].includes(ugoiraExportTask.phase));
  let ugoiraExportProgress = $derived(ugoiraExportTask && ugoiraExportTask.totalUnits > 0 ? Math.min(100, Math.round(ugoiraExportTask.completedUnits / ugoiraExportTask.totalUnits * 100)) : 0);

  onMount(() => {
    ugoiraExportSupported = !/Android/i.test(navigator.userAgent);
  });

  $effect(() => () => { if (exportPollTimer) clearTimeout(exportPollTimer); });

  type ArtworkDetailSnapshot = {
    detail: IllustrationDetail | null;
    related: IllustrationSummary[];
    nextCursor: string | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    relatedError: string;
    revealRestricted: boolean;
    bookmarked: boolean;
    bookmarkRestrict: BookmarkRestrict;
    bookmarkError: string;
    downloadMessage: string;
    seriesNavigation: ArtworkSeriesNavigation | null;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<ArtworkDetailSnapshot>({
      detail, related, nextCursor, status, errorMessage, relatedError, revealRestricted,
      bookmarked, bookmarkRestrict, bookmarkError, downloadMessage, seriesNavigation, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<ArtworkDetailSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      detail = value.detail;
      related = value.related;
      nextCursor = value.nextCursor;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      relatedError = value.relatedError;
      revealRestricted = value.revealRestricted;
      bookmarked = value.bookmarked;
      bookmarkRestrict = value.bookmarkRestrict;
      bookmarkError = value.bookmarkError;
      downloadMessage = value.downloadMessage;
      seriesNavigation = value.seriesNavigation;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
      loadingMore = false;
      bookmarkPending = false;
      downloadPending = false;
      seriesNavigationLoading = false;
    },
  };

  $effect(() => {
    const account = bookmarkAccount;
    const currentIllustrationId = detail?.illustration.id ?? "";
    bookmarked = resolveIllustrationBookmarkState(
      account,
      currentIllustrationId,
      detail?.illustration.isBookmarked ?? false,
    );
    bookmarkError = "";
    return subscribeIllustrationBookmarkState(account, currentIllustrationId, (next) => {
      bookmarked = next;
      bookmarkError = "";
    });
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && illustrationId ? `${sessionKey}:${illustrationId}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      detail = null;
      related = [];
      nextCursor = null;
      seriesNavigation = null;
      seriesNavigationLoading = false;
      status = "idle";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadArtwork(key, illustrationId);
    }
  });

  async function loadArtwork(key: string, id: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    relatedError = "";
    detail = null;
    related = [];
    nextCursor = null;
    seriesNavigation = null;
    seriesNavigationLoading = false;
    revealRestricted = false;
    try {
      const [nextDetail, relatedPage] = await Promise.all([
        getIllustrationDetail(id),
        getRelatedIllustrations(id),
      ]);
      if (sequence !== requestSequence || requestedKey !== key) return;
      detail = nextDetail;
      void recordBrowsingHistory({
        kind: "artwork",
        resourceId: nextDetail.illustration.id,
        title: nextDetail.illustration.title || m.common_untitled(),
        subtitle: nextDetail.illustration.author.name || m.common_unknown_author(),
        thumbnailUrl: nextDetail.illustration.thumbnailUrl,
      }).catch(() => undefined);
      related = relatedPage.illustrations.filter((item) => item.id !== id);
      nextCursor = relatedPage.nextCursor ?? null;
      status = "ready";
      if (nextDetail.series) {
        void loadSeriesNavigation(sequence, key, nextDetail.series.id, id);
      }
    } catch (error) {
      if (sequence !== requestSequence || requestedKey !== key) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadSeriesNavigation(
    sequence: number,
    key: string,
    seriesId: string,
    currentId: string,
  ) {
    seriesNavigationLoading = true;
    try {
      const navigation = await resolveArtworkSeriesNavigation(seriesId, currentId);
      if (sequence !== requestSequence || key !== requestedKey) return;
      seriesNavigation = navigation;
    } catch {
      if (sequence === requestSequence && key === requestedKey) seriesNavigation = null;
    } finally {
      if (sequence === requestSequence && key === requestedKey) seriesNavigationLoading = false;
    }
  }

  function retry() {
    if (requestedKey && illustrationId) void loadArtwork(requestedKey, illustrationId);
  }

  async function loadMoreRelated() {
    const cursor = nextCursor;
    if (!cursor || loadingMore || !illustrationId) return;
    const sequence = requestSequence;
    const key = requestedKey;
    loadingMore = true;
    relatedError = "";
    try {
      const nextPage = await getRelatedIllustrations(illustrationId, cursor);
      if (sequence !== requestSequence || key !== requestedKey) return;
      const knownIds = new Set([illustrationId, ...related.map((item) => item.id)]);
      related = [...related, ...nextPage.illustrations.filter((item) => !knownIds.has(item.id))];
      nextCursor = nextPage.nextCursor ?? null;
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) {
        relatedError = describeDataFailure(error);
      }
    } finally {
      loadingMore = false;
    }
  }

  async function toggleBookmark() {
    if (!detail || bookmarkPending) return;
    const previous = bookmarked;
    const next = !previous;
    const account = bookmarkAccount;
    bookmarked = next;
    bookmarkPending = true;
    bookmarkError = "";
    try {
      await setIllustrationBookmark(detail.illustration.id, next, bookmarkRestrict);
      detail.illustration.isBookmarked = next;
      detail.totalBookmarks = Math.max(0, detail.totalBookmarks + (next ? 1 : -1));
      publishIllustrationBookmarkState(account, detail.illustration.id, next);
    } catch (error) {
      bookmarked = previous;
      bookmarkError = describeDataFailure(error);
    } finally {
      bookmarkPending = false;
    }
  }

  async function saveOffline() {
    if (!detail || downloadPending) return;
    downloadPending = true;
    downloadMessage = "";
    try {
      const task = await enqueueDownload(
        detail.illustration.kind === "ugoira" ? "ugoira" : "artwork",
        detail.illustration.id,
        detail.illustration.title,
        detail.illustration.author.name,
      );
      downloadMessage = task.state === "completed" ? m.download_requeued() : m.download_added();
    } catch (error) {
      downloadMessage = describeDataFailure(error);
    } finally {
      downloadPending = false;
    }
  }

  async function beginUgoiraExport() {
    if (!detail || detail.illustration.kind !== "ugoira" || ugoiraExportActive) return;
    ugoiraExportError = "";
    try {
      ugoiraExportTask = await startUgoiraExport(detail.illustration.id, ugoiraExportFormat);
      scheduleExportPoll();
    } catch (error) {
      ugoiraExportError = describeDataFailure(error);
    }
  }

  function scheduleExportPoll() {
    if (!ugoiraExportTask || !ugoiraExportActive) return;
    if (exportPollTimer) clearTimeout(exportPollTimer);
    exportPollTimer = setTimeout(async () => {
      try {
        if (ugoiraExportTask) ugoiraExportTask = await getUgoiraExportTask(ugoiraExportTask.id);
      } catch (error) {
        ugoiraExportError = describeDataFailure(error);
      }
      scheduleExportPoll();
    }, 600);
  }

  async function cancelCurrentUgoiraExport() {
    if (!ugoiraExportTask || !ugoiraExportActive) return;
    try {
      ugoiraExportTask = await cancelUgoiraExportTask(ugoiraExportTask.id);
      scheduleExportPoll();
    } catch (error) {
      ugoiraExportError = describeDataFailure(error);
    }
  }

  function formatCount(value: number): string {
    return new Intl.NumberFormat(currentAppLocale(), { notation: "compact", maximumFractionDigits: 1 }).format(value);
  }
</script>

<svelte:head>
  <title>{detail?.illustration.title || m.artwork_detail()} · PixNya</title>
</svelte:head>

<AppShell title={m.artwork_detail()}>
  <main class="detail-page">
    <ReturnLink fallback="/artworks" label={m.artwork_return_source()} />

  {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state-card">
        <Icon name="user" size={28} />
        <div><h1>{m.artwork_login_title()}</h1><p>{m.artwork_login_description()}</p></div>
<a href="/login">{m.common_go_to_login()}</a>
      </section>
    {:else if status === "loading"}
      <section class="state-card loading" aria-live="polite">
        <span class="spinner"></span><div><h1>{m.artwork_loading_title()}</h1><p>{m.artwork_loading_description()}</p></div>
      </section>
    {:else if status === "error"}
      <section class="state-card error" role="alert">
        <span>!</span><div><h1>{m.artwork_load_failed()}</h1><p>{errorMessage}</p></div>
        <button type="button" onclick={retry}>{m.common_retry()}</button>
      </section>
    {:else if detail}
      <div class="detail-layout">
        <section class="image-column" class:concealed={restricted && !$r18DefaultVisible && !revealRestricted}>
          {#if detail.illustration.kind === "ugoira"}
            <UgoiraPlayer illustrationId={detail.illustration.id} previewUrl={detail.pages[0]?.displayUrl ?? detail.illustration.thumbnailUrl} title={detail.illustration.title || m.common_untitled()} />
          {:else if detail.pages.length > 0}
            <ArtworkImageViewer
              pages={viewerPages}
              title={detail.illustration.title || m.common_untitled()}
              concealed={restricted && !$r18DefaultVisible && !revealRestricted}
            />
          {:else}
            <div class="unavailable-image">{m.artwork_no_image()}</div>
          {/if}
          {#if restricted && !$r18DefaultVisible && !revealRestricted}
            <button class="reveal" type="button" onclick={() => (revealRestricted = true)}>
              {m.artwork_reveal({ rating: detail.illustration.xRestrict >= 2 ? "R-18G" : "R-18" })}
            </button>
          {/if}
        </section>

        <aside class="detail-info">
          <div class="title-block">
            <div class="kind-row">
              <span>{detail.illustration.kind || "illust"}</span>
              {#if detail.illustration.aiType === 2}<span>{m.artwork_ai_generated()}</span>{/if}
              {#if detail.series}<a href={`/series/artworks/${detail.series.id}`}>{m.artwork_series({ title: detail.series.title })}</a>{/if}
            </div>
            <h1>{detail.illustration.title || m.common_untitled()}</h1>
            {#if caption}<p class="caption">{caption}</p>{/if}
          </div>

          <a class="author-card" href={`/users/${detail.illustration.author.id}`}>
            <span class="author-avatar">
              <PixivImage url={detail.illustration.author.avatarUrl} alt="" />
              <b>{Array.from(detail.illustration.author.name || "P")[0]}</b>
            </span>
            <span><strong>{detail.illustration.author.name}</strong><small>@{detail.illustration.author.account}</small></span>
            <i>{detail.illustration.author.isFollowed ? m.common_following() : m.artwork_view_author()}</i>
          </a>

          <dl class="work-stats">
            <div><dt>{m.common_view_count()}</dt><dd>{formatCount(detail.totalViews)}</dd></div>
            <div><dt>{m.common_bookmark_count()}</dt><dd>{formatCount(detail.totalBookmarks)}</dd></div>
            <div><dt>{m.common_comment_count()}</dt><dd>{formatCount(detail.totalComments)}</dd></div>
          </dl>

          <div class="work-actions">
            {#if !bookmarked}
              <label>{m.common_bookmark_visibility()}
                <select bind:value={bookmarkRestrict} aria-label={m.common_bookmark_visibility()}>
                  <option value="public">{m.common_public()}</option>
                  <option value="private">{m.common_private()}</option>
                </select>
              </label>
            {/if}
            <button type="button" class:active={bookmarked} disabled={bookmarkPending} onclick={toggleBookmark}>
              <Icon name="heart" size={17} />
              {bookmarkPending ? m.common_processing() : bookmarked ? m.artwork_cancel_bookmark() : m.artwork_save_bookmark()}
            </button>
            <button type="button" disabled={downloadPending} onclick={saveOffline}><Icon name="download" size={16} />{downloadPending ? m.common_queueing() : m.common_offline_save()}</button>
            {#if detail.illustration.kind === "ugoira" && ugoiraExportSupported}
              <div class="ugoira-export">
                <label>{m.ugoira_export_format()}<select bind:value={ugoiraExportFormat} disabled={ugoiraExportActive}><option value="gif">GIF</option><option value="apng">APNG</option><option value="webm">WebM</option></select></label>
                <button type="button" disabled={ugoiraExportActive} onclick={beginUgoiraExport}>{m.ugoira_export_start()}</button>
                {#if ugoiraExportActive}<button class="cancel" type="button" onclick={cancelCurrentUgoiraExport}>{m.common_cancel()}</button>{/if}
                {#if ugoiraExportTask}<div class="export-progress"><progress max="100" value={ugoiraExportProgress}></progress><span>{m.ugoira_export_status({ phase: ugoiraExportTask.phase, progress: ugoiraExportProgress })}</span>{#if ugoiraExportTask.destination}<small>{ugoiraExportTask.destination}</small>{/if}{#if ugoiraExportTask.failure}<small class="error">{m.ugoira_export_failure({ reason: ugoiraExportTask.failure })}</small>{/if}</div>{/if}
              </div>
            {/if}
            {#if bookmarkError}<p role="alert">{bookmarkError}</p>{/if}
            {#if downloadMessage}<p role="status">{downloadMessage}</p>{/if}
          </div>

          <div class="tag-list" aria-label={m.artwork_tags_label()}>
            {#each detail.tags as tag (tag.name)}
              <a href={`/search?q=${encodeURIComponent(tag.name)}`} onclick={() => recordSearchHistory(tag.name)}>
                #{tag.name}{tag.translatedName ? ` · ${tag.translatedName}` : ""}
              </a>
            {/each}
          </div>

          <dl class="metadata">
            <div><dt>{m.artwork_dimensions()}</dt><dd>{detail.illustration.width} × {detail.illustration.height}</dd></div>
            <div><dt>{m.artwork_page_count()}</dt><dd>{detail.pages.length || detail.illustration.pageCount}</dd></div>
            {#if detail.createDate}<div><dt>{m.artwork_publish_date()}</dt><dd>{detail.createDate.slice(0, 10)}</dd></div>{/if}
            {#if detail.tools.length}<div><dt>{m.artwork_tools()}</dt><dd>{detail.tools.join(" · ")}</dd></div>{/if}
          </dl>
        </aside>
      </div>

      {#if detail.series}
        <nav class="series-navigation" aria-label={m.artwork_series_navigation()}>
          <a class="series-overview" href={`/series/artworks/${detail.series.id}`}>
            <small>{m.artwork_series_label()}</small><strong>{detail.series.title}</strong>
            {#if seriesNavigation}<span>{m.artwork_series_position({ position: seriesNavigation.position, total: seriesNavigation.total })}</span>{/if}
          </a>
          {#if seriesNavigationLoading}
            <span class="series-resolving">{m.artwork_series_locating()}</span>
          {:else}
            {#if seriesNavigation?.previous}
              <a class="series-sibling previous" href={`/artworks/${seriesNavigation.previous.id}`}><small>{m.artwork_previous()}</small><strong>{seriesNavigation.previous.title || m.common_untitled()}</strong></a>
            {:else}<span class="series-sibling disabled"><small>{m.artwork_previous()}</small><strong>{m.artwork_series_start()}</strong></span>{/if}
            {#if seriesNavigation?.next}
              <a class="series-sibling next" href={`/artworks/${seriesNavigation.next.id}`}><small>{m.artwork_next()}</small><strong>{seriesNavigation.next.title || m.common_untitled()}</strong></a>
            {:else}<span class="series-sibling disabled next"><small>{m.artwork_next()}</small><strong>{seriesNavigation ? m.artwork_series_end() : m.artwork_series_unavailable()}</strong></span>{/if}
          {/if}
        </nav>
      {/if}

      <ArtworkComments illustrationId={detail.illustration.id} initialCount={detail.totalComments} />

      <section class="related-section">
        <header><div><h2>{m.artwork_related()}</h2></div></header>
        {#if related.length > 0}
          <div class="related-grid">
            {#each related as illustration, index (illustration.id)}
              <ArtworkCard {illustration} tone={(index % 6) + 1} />
            {/each}
          </div>
        {:else}
          <p class="empty">{m.artwork_related_empty()}</p>
        {/if}
        {#if relatedError}<p class="related-error" role="alert">{relatedError}</p>{/if}
        {#if nextCursor}
          <button class="load-more" type="button" disabled={loadingMore} onclick={loadMoreRelated}>
            {loadingMore ? m.common_loading() : m.artwork_related_more()}
          </button>
        {/if}
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .detail-page { width: min(1160px, 100%); margin: 0 auto; padding: 24px 28px 56px; }
  .state-card {
    display: grid; grid-template-columns: 46px minmax(0, 1fr) auto; gap: 16px; align-items: center;
    margin-top: 22px; padding: 22px; border: 1px solid var(--line); border-radius: 12px; background: white;
  }
  .state-card > span:first-child { color: var(--pixiv-blue); }
  .state-card h1 { margin: 0; font-size: 17px; }
  .state-card p { margin: 5px 0 0; color: var(--muted); font-size: 10px; }
  .state-card a, .state-card button { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: 10px; font-weight: 700; text-decoration: none; }
  .state-card.error > span { display: grid; width: 40px; height: 40px; place-items: center; color: #b65364; border-radius: 50%; background: #fff0f3; font-weight: 800; }
  .spinner { width: 32px; height: 32px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }

  .detail-layout { display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(300px, .75fr); gap: 28px; align-items: start; margin-top: 20px; }
  .image-column { position: relative; display: grid; gap: 16px; min-width: 0; }
  .unavailable-image { display: grid; min-height: 360px; place-items: center; color: var(--muted); border-radius: 10px; background: #f3f4f5; font-size: 11px; }
  .concealed .unavailable-image { filter: blur(24px) brightness(.68); }
  .reveal { position: absolute; z-index: 3; inset: 0; color: white; border: 0; border-radius: 10px; background: rgba(23, 27, 31, .32); cursor: pointer; font-size: 13px; font-weight: 750; }

  .detail-info { position: sticky; top: calc(var(--topbar-height) + 20px); overflow: hidden; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .title-block { padding: 22px; }
  .kind-row { display: flex; flex-wrap: wrap; gap: 6px; }
  .kind-row span, .kind-row a { padding: 4px 7px; color: #607785; border-radius: 4px; background: #eef6fa; font-size: 8px; font-weight: 700; text-decoration: none; }
  .kind-row a:hover { color: var(--pixiv-blue); }
  .title-block h1 { margin: 12px 0 0; font-size: 22px; line-height: 1.3; }
  .caption { margin: 12px 0 0; color: #62676b; font-size: 10px; line-height: 1.75; white-space: pre-line; }

  .author-card { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 11px; align-items: center; padding: 15px 22px; color: var(--text); border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); text-decoration: none; }
  .author-avatar { position: relative; display: grid; width: 44px; height: 44px; overflow: hidden; place-items: center; color: white; border-radius: 50%; background: var(--pixiv-blue); }
  .author-avatar :global(img) { position: absolute; z-index: 1; inset: 0; }
  .author-card strong, .author-card small { display: block; }
  .author-card strong { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .author-card small { margin-top: 4px; color: var(--muted); font-size: 8px; }
  .author-card i { color: var(--pixiv-blue); font-size: 9px; font-style: normal; font-weight: 700; }

  .work-stats { display: grid; grid-template-columns: repeat(3, 1fr); margin: 0; border-bottom: 1px solid var(--line); }
  .work-stats div { padding: 14px 8px; text-align: center; }
  .work-stats div + div { border-left: 1px solid var(--line); }
  .work-stats dt { color: var(--muted); font-size: 8px; }
  .work-stats dd { margin: 4px 0 0; font-size: 12px; font-weight: 750; }
  .work-actions { display: flex; flex-wrap: wrap; gap: 9px; align-items: center; padding: 14px 22px; border-bottom: 1px solid var(--line); }
  .work-actions label { display: flex; gap: 7px; align-items: center; color: var(--muted); font-size: 8px; }
  .work-actions select { height: 30px; padding: 0 25px 0 9px; color: #545b60; border: 1px solid var(--line); border-radius: 15px; background: white; font-size: 9px; }
  .work-actions > button { display: flex; min-height: 34px; gap: 6px; align-items: center; justify-content: center; padding: 0 14px; color: white; border: 0; border-radius: 17px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; }
  .work-actions > button.active { color: #ff4060; border: 1px solid #ffd2db; background: #fff7f9; }
  .work-actions > button.active :global(svg) { fill: currentColor; }
  .work-actions > button:disabled { cursor: wait; opacity: .62; }
  .work-actions > p { flex-basis: 100%; margin: 0; color: #a44f5e; font-size: 8px; }
  .ugoira-export { display: flex; flex-basis: 100%; gap: 8px; align-items: center; flex-wrap: wrap; padding-top: 5px; }
  .ugoira-export > button { min-height: 32px; padding: 0 13px; color: white; border: 0; border-radius: 16px; background: var(--pixiv-blue); cursor: pointer; font-size: 8px; font-weight: 700; }
  .ugoira-export > button.cancel { color: #9d5964; border: 1px solid #efcbd1; background: white; }
  .export-progress { display: grid; flex: 1 1 100%; grid-template-columns: minmax(100px,1fr) auto; gap: 6px 10px; align-items: center; color: var(--muted); font-size: 8px; }
  .export-progress progress { width: 100%; accent-color: var(--pixiv-blue); }
  .export-progress small { grid-column: 1 / -1; overflow-wrap: anywhere; }
  .export-progress small.error { color: #a44f5e; }
  .tag-list { display: flex; flex-wrap: wrap; gap: 7px; padding: 18px 22px; border-bottom: 1px solid var(--line); }
  .tag-list a { padding: 6px 9px; color: #4c7289; border-radius: 4px; background: #f0f7fb; font-size: 8px; text-decoration: none; }
  .metadata { margin: 0; padding: 12px 22px 17px; }
  .metadata div { display: flex; gap: 16px; justify-content: space-between; padding: 7px 0; font-size: 8px; }
  .metadata dt { color: var(--muted); }
  .metadata dd { margin: 0; text-align: right; }

  .series-navigation { display: grid; grid-template-columns: minmax(180px,.7fr) repeat(2,minmax(0,1fr)); gap: 10px; margin-top: 22px; }
  .series-navigation > a, .series-navigation > span { min-width: 0; padding: 14px 16px; border: 1px solid var(--line); border-radius: 10px; background: white; text-decoration: none; }
  .series-navigation small, .series-navigation strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .series-navigation small { color: var(--muted); font-size: 8px; }
  .series-navigation strong { margin-top: 5px; color: var(--text); font-size: 10px; }
  .series-overview { border-color: #bfe7ff !important; background: #f3faff !important; }
  .series-overview span { display: block; margin-top: 6px; color: var(--pixiv-blue); font-size: 8px; font-weight: 700; }
  .series-sibling.next { text-align: right; }
  .series-sibling:not(.disabled):hover strong, .series-overview:hover strong { color: var(--pixiv-blue); }
  .series-sibling.disabled { opacity: .55; }
  .series-resolving { grid-column: 2 / -1; display: grid; place-items: center; color: var(--muted); font-size: 9px; }

  .related-section { margin-top: 42px; }
  .related-section header h2 { margin: 0; font-size: 18px; }
  .related-grid { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 22px 14px; margin-top: 16px; }
  .empty { padding: 36px; color: var(--muted); border: 1px dashed var(--line); border-radius: 9px; font-size: 10px; text-align: center; }
  .related-error { color: #a65865; font-size: 9px; text-align: center; }
  .load-more { display: block; min-width: 148px; height: 38px; margin: 26px auto 0; color: #59636a; border: 1px solid var(--line); border-radius: 19px; background: white; cursor: pointer; font-size: 10px; font-weight: 700; }
  .load-more:disabled { cursor: wait; opacity: .65; }

  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 900px) {
    .detail-layout { grid-template-columns: 1fr; }
    .detail-info { position: static; }
    .related-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
  @media (max-width: 620px) {
    .detail-page { padding: 18px 12px 44px; }
    .state-card { grid-template-columns: 40px minmax(0, 1fr); padding: 17px; }
    .state-card a, .state-card button { grid-column: 1 / -1; text-align: center; }
    .detail-layout { gap: 14px; }
    .unavailable-image { min-height: 260px; border-radius: 7px; }
    .title-block { padding: 18px; }
    .title-block h1 { font-size: 19px; }
    .author-card { padding: 14px 18px; }
    .related-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px 11px; }
    .series-navigation { grid-template-columns: repeat(2,minmax(0,1fr)); }
    .series-overview, .series-resolving { grid-column: 1 / -1; }
  }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
