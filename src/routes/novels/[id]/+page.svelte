<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkComments from "$lib/components/ArtworkComments.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import {
    describeDataFailure,
    enqueueDownload,
    getNovelDetail,
    recordBrowsingHistory,
    setNovelBookmark,
  } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { r18DefaultVisible } from "$lib/preferences";
  import { recordSearchHistory } from "$lib/search-history";
  import { session, sessionRestoring } from "$lib/session";
  import type { BookmarkRestrict, NovelDetail } from "$lib/types";

  let detail = $state<NovelDetail | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let downloadPending = $state(false);
  let downloadMessage = $state("");
  let bookmarked = $state(false);
  let bookmarkRestrict = $state<BookmarkRestrict>("public");
  let bookmarkPending = $state(false);
  let bookmarkError = $state("");
  let revealRestricted = $state(false);
  let requestedKey = $state("");
  let requestSequence = 0;
  let novelId = $derived(page.params.id ?? "");
  let caption = $derived(detail ? plainPixivText(detail.novel.caption) : "");
  let restricted = $derived((detail?.novel.xRestrict ?? 0) > 0);

  type NovelDetailSnapshot = {
    detail: NovelDetail | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    downloadMessage: string;
    bookmarked: boolean;
    bookmarkRestrict: BookmarkRestrict;
    bookmarkError: string;
    revealRestricted: boolean;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<NovelDetailSnapshot>({
      detail, status, errorMessage, downloadMessage, bookmarked, bookmarkRestrict,
      bookmarkError, revealRestricted, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<NovelDetailSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      detail = value.detail;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      downloadMessage = value.downloadMessage;
      bookmarked = value.bookmarked;
      bookmarkRestrict = value.bookmarkRestrict;
      bookmarkError = value.bookmarkError;
      revealRestricted = value.revealRestricted;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
      downloadPending = false;
      bookmarkPending = false;
    },
  };

  $effect(() => {
    bookmarked = detail?.novel.isBookmarked ?? false;
    bookmarkError = "";
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && novelId ? `${sessionKey}:${novelId}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      detail = null;
      status = "idle";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadNovelDetail(key, novelId);
    }
  });

  async function loadNovelDetail(key: string, id: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    detail = null;
    revealRestricted = false;
    try {
      const nextDetail = await getNovelDetail(id);
      if (sequence !== requestSequence || key !== requestedKey) return;
      detail = nextDetail;
      status = "ready";
      void recordBrowsingHistory({
        kind: "novel",
        resourceId: nextDetail.novel.id,
        title: nextDetail.novel.title || m.common_untitled(),
        subtitle: nextDetail.novel.author.name || m.common_unknown_author(),
        thumbnailUrl: nextDetail.novel.coverUrl,
      }).catch(() => undefined);
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedKey) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  function compact(value: number): string {
    return new Intl.NumberFormat(currentAppLocale(), {
      notation: "compact",
      maximumFractionDigits: 1,
    }).format(value);
  }

  async function saveOffline() {
    if (downloadPending || !detail) return;
    downloadPending = true;
    downloadMessage = "";
    try {
      await enqueueDownload(
        "novel",
        detail.novel.id,
        detail.novel.title,
        detail.novel.author.name,
      );
      downloadMessage = m.download_added();
    } catch (error) {
      downloadMessage = describeDataFailure(error);
    } finally {
      downloadPending = false;
    }
  }

  async function toggleBookmark() {
    if (!detail || bookmarkPending) return;
    const previous = bookmarked;
    bookmarked = !previous;
    bookmarkPending = true;
    bookmarkError = "";
    try {
      await setNovelBookmark(detail.novel.id, bookmarked, bookmarkRestrict);
      detail.novel.isBookmarked = bookmarked;
    } catch (error) {
      bookmarked = previous;
      bookmarkError = describeDataFailure(error);
    } finally {
      bookmarkPending = false;
    }
  }
</script>

<svelte:head><title>{detail?.novel.title || m.novel_detail()} · PixNya</title></svelte:head>

<AppShell title={m.novel_detail()}>
  <main class="detail-page">
    <ReturnLink fallback="/novels" label={m.novel_return_source()} />

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state">
        <Icon name="user" size={27} />
        <div><h1>{m.novel_login_title()}</h1><p>{m.novel_login_description()}</p></div>
        <a href="/login?mode=standard">{m.common_go_to_login()}</a>
      </section>
    {:else if status === "loading"}
      <section class="state">
        <span class="spinner"></span>
        <div><h1>{m.novel_loading_title()}</h1><p>{m.novel_loading_description()}</p></div>
      </section>
    {:else if status === "error"}
      <section class="state error" role="alert">
        <span>!</span>
        <div><h1>{m.novel_load_failed()}</h1><p>{errorMessage}</p></div>
        <button type="button" onclick={() => loadNovelDetail(requestedKey, novelId)}>{m.common_retry()}</button>
      </section>
    {:else if detail}
      <article class="detail-card">
        <div class="cover" class:concealed={restricted && !$r18DefaultVisible && !revealRestricted}>
          <PixivImage url={detail.novel.coverUrl} alt="" cacheKind="preview" />
          {#if restricted && !$r18DefaultVisible && !revealRestricted}
            <button class="reveal-cover" type="button" onclick={() => (revealRestricted = true)}>
              {m.novel_reveal_cover()}
            </button>
          {/if}
        </div>

        <div class="detail-copy">
          <div class="badges">
            {#if detail.novel.series}<span>{m.novel_series_badge()}</span>{/if}
            {#if detail.isOriginal}<span>{m.novel_original_badge()}</span>{/if}
            {#if detail.novel.aiType === 2}<span>AI</span>{/if}
            {#if restricted}<span class="restricted-badge">R-18</span>{/if}
          </div>
          <h1>{detail.novel.title || m.common_untitled()}</h1>
          <a class="author" href={`/users/${detail.novel.author.id}`}>{detail.novel.author.name}</a>
          {#if caption}<p class="caption">{caption}</p>{/if}

          {#if detail.novel.series}
            <a class="series-link" href={`/series/novels/${detail.novel.series.id}`}>
              <small>{m.novel_series_belongs()}</small><strong>{detail.novel.series.title}</strong><i>›</i>
            </a>
          {/if}

          <dl>
            <div><dt>{m.novel_word_count()}</dt><dd>{compact(detail.novel.textLength)}</dd></div>
            <div><dt>{m.common_view_count()}</dt><dd>{compact(detail.novel.totalViews)}</dd></div>
            <div><dt>{m.common_bookmark_count()}</dt><dd>{compact(detail.novel.totalBookmarks)}</dd></div>
            <div><dt>{m.common_comment_count()}</dt><dd>{compact(detail.novel.totalComments)}</dd></div>
          </dl>

          <div class="tags">
            {#each detail.novel.tags as tag}
              <a href={`/search?q=${encodeURIComponent(tag)}`} onclick={() => recordSearchHistory(tag)}>#{tag}</a>
            {/each}
          </div>

          <div class="detail-actions">
            <a class="read-button" href={`/novels/${detail.novel.id}/read`}>
              <Icon name="book" size={18} />{m.novel_start_reading()}
            </a>
            <label class="bookmark-scope">
              <span>{m.common_bookmark_visibility()}</span>
              <select bind:value={bookmarkRestrict} aria-label={m.novel_bookmark_visibility_label()}>
                <option value="public">{m.novel_public_bookmark()}</option>
                <option value="private">{m.novel_private_bookmark()}</option>
              </select>
            </label>
            <button
              class="bookmark-button"
              type="button"
              class:active={bookmarked}
              disabled={bookmarkPending}
              onclick={toggleBookmark}
            >
              <Icon name="heart" size={17} />{bookmarkPending ? m.common_processing() : bookmarked ? m.novel_bookmarked() : m.novel_bookmark()}
            </button>
            <button class="offline-button" type="button" disabled={downloadPending} onclick={saveOffline}>
              <Icon name="download" size={17} />{downloadPending ? m.common_queueing() : m.common_offline_save()}
            </button>
          </div>
          {#if bookmarkError}<p class="action-message error" role="alert">{bookmarkError}</p>{/if}
          {#if downloadMessage}<p class="action-message" role="status">{downloadMessage}</p>{/if}
        </div>
      </article>

      <ArtworkComments novelId={novelId} initialCount={detail.novel.totalComments} />
    {/if}
  </main>
</AppShell>

<style>
  .detail-page { width: min(980px, 100%); margin: 0 auto; padding: 24px 28px 55px; }
  .state { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 14px; align-items: center; margin-top: 22px; padding: 21px; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .state h1 { margin: 0; font-size: 16px; }
  .state p { margin: 5px 0 0; color: var(--muted); font-size: 9px; }
  .state a, .state button { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; text-decoration: none; }
  .state.error > span { display: grid; width: 36px; height: 36px; place-items: center; color: #a34e5d; border-radius: 50%; background: #fff0f3; }
  .spinner { width: 29px; height: 29px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .detail-card { display: grid; grid-template-columns: minmax(210px, 280px) minmax(0, 1fr); gap: 0; overflow: hidden; margin-top: 18px; border: 1px solid var(--line); border-radius: 14px; background: white; }
  .cover { position: relative; min-height: 390px; overflow: hidden; background: #edf1f4; }
  .cover :global(img) { position: absolute; inset: 0; }
  .cover.concealed :global(img) { filter: blur(24px) brightness(.65); transform: scale(1.12); }
  .reveal-cover { position: absolute; z-index: 2; inset: 0; width: 100%; color: white; border: 0; background: rgba(20, 24, 28, .34); cursor: pointer; font-size: 10px; font-weight: 700; }
  .detail-copy { min-width: 0; padding: 30px 32px; }
  .badges { display: flex; flex-wrap: wrap; gap: 6px; }
  .badges span { padding: 4px 8px; color: #55778b; border-radius: 4px; background: #eef7fc; font-size: 8px; font-weight: 700; }
  .badges .restricted-badge { color: #a64055; background: #fff0f3; }
  h1 { margin: 13px 0 0; font-size: clamp(22px, 3vw, 32px); line-height: 1.35; overflow-wrap: anywhere; }
  .author { display: inline-block; margin-top: 10px; color: var(--pixiv-blue); font-size: 11px; text-decoration: none; }
  .caption { display: -webkit-box; overflow: hidden; margin: 16px 0 0; color: var(--muted); font-size: 10px; line-height: 1.75; line-clamp: 4; -webkit-box-orient: vertical; -webkit-line-clamp: 4; white-space: pre-line; }
  .series-link { display: grid; grid-template-columns: minmax(0, 1fr) auto; margin-top: 17px; padding: 12px 14px; color: inherit; border-radius: 8px; background: #f0f8fd; text-decoration: none; }
  .series-link small, .series-link strong { display: block; overflow: hidden; grid-column: 1; text-overflow: ellipsis; white-space: nowrap; }
  .series-link small { color: var(--pixiv-blue); font-size: 8px; }
  .series-link strong { margin-top: 4px; font-size: 10px; }
  .series-link i { grid-column: 2; grid-row: 1 / 3; align-self: center; color: #8da1ad; font-size: 20px; font-style: normal; }
  dl { display: grid; grid-template-columns: repeat(4, 1fr); margin: 18px 0 0; border: 1px solid var(--line); border-radius: 8px; }
  dl div { min-width: 0; padding: 11px 5px; text-align: center; }
  dl div + div { border-left: 1px solid var(--line); }
  dt { color: var(--muted); font-size: 8px; }
  dd { margin: 4px 0 0; font-size: 11px; font-weight: 700; }
  .tags { display: flex; flex-wrap: wrap; gap: 6px 10px; margin-top: 17px; }
  .tags a { color: #5e7d90; font-size: 9px; text-decoration: none; }
  .detail-actions { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 9px; margin-top: 22px; }
  .read-button { display: flex; height: 44px; grid-column: 1 / -1; gap: 8px; align-items: center; justify-content: center; color: white; border-radius: 22px; background: var(--pixiv-blue); font-size: 11px; font-weight: 700; text-decoration: none; }
  .bookmark-scope { display: grid; min-width: 0; height: 42px; grid-template-columns: auto minmax(0, 1fr); gap: 7px; align-items: center; padding: 0 11px; border: 1px solid var(--line); border-radius: 21px; }
  .bookmark-scope span { color: var(--muted); font-size: 8px; white-space: nowrap; }
  .bookmark-scope select { min-width: 0; width: 100%; height: 30px; padding: 0 4px; border: 0; background: transparent; font-size: 9px; }
  .bookmark-button, .offline-button { display: flex; min-width: 0; height: 42px; gap: 7px; align-items: center; justify-content: center; color: #59636a; border: 1px solid var(--line); border-radius: 21px; background: white; cursor: pointer; font-size: 9px; font-weight: 700; }
  .bookmark-button.active { color: #ff4060; border-color: #ffd0d8; background: #fff7f8; }
  .bookmark-button.active :global(svg) { fill: currentColor; }
  .offline-button { color: var(--pixiv-blue); border-color: #b9def5; }
  .bookmark-button:disabled, .offline-button:disabled { cursor: wait; opacity: .6; }
  .action-message { margin: 9px 0 0; color: var(--muted); font-size: 8px; text-align: center; }
  .action-message.error { color: #a34e5d; }
  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 720px) {
    .detail-page { padding: 16px 12px 86px; }
    .detail-card { grid-template-columns: 1fr; }
    .cover { width: 112px; min-height: 168px; margin: 20px auto 0; border-radius: 8px; }
    .detail-copy { padding: 20px 18px; }
    h1 { font-size: 21px; }
    .detail-actions { grid-template-columns: 1fr; }
    .read-button { grid-column: auto; }
    .bookmark-scope, .bookmark-button, .offline-button { width: 100%; }
    .state { grid-template-columns: 38px minmax(0, 1fr); }
    .state a, .state button { grid-column: 1 / -1; text-align: center; }
  }

  @media (max-width: 460px) {
    .cover { width: 96px; min-height: 142px; margin-top: 16px; }
    .detail-copy { padding: 17px 15px; }
    .caption { margin-top: 14px; }
    dl { grid-template-columns: repeat(2, 1fr); }
    dl div:nth-child(3) { border-left: 0; border-top: 1px solid var(--line); }
    dl div:nth-child(4) { border-top: 1px solid var(--line); }
  }
</style>
