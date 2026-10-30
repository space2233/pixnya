<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import NovelImmersiveReader from "$lib/components/NovelImmersiveReader.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import { parseNovelText } from "$lib/novel-text";
  import { loadNovelForReading, readingChapterHref } from "$lib/novel-reading-source";
  import { NOVEL_READER_LOAD_ATTEMPTS, NOVEL_READER_RETRY_DELAY_MS, shouldRetryNovelReaderLoad } from "$lib/novel-reader-load";
  import { describeDataFailure, getNovelContent, getNovelDetail, listOfflineEntries, readOfflineText, recordBrowsingHistory } from "$lib/pixiv-api";
  import { addNovelToShelf, readNovelProgress, saveNovelProgress } from "$lib/reading-shelf";
  import { r18DefaultVisible } from "$lib/preferences";
  import { session, sessionRestoring } from "$lib/session";
  import type { NovelContent, NovelDetail } from "$lib/types";

  let { novelId, offlineOnly = false }: { novelId: string; offlineOnly?: boolean } = $props();
  let detail = $state<NovelDetail | null>(null);
  let content = $state<NovelContent | null>(null);
  let status = $state<"loading" | "ready" | "error" | "login">("loading");
  let errorMessage = $state("");
  let saveError = $state("");
  let revealRestricted = $state(false);
  let progress = $state(0);
  let downloadedIds = $state<Set<string>>(new Set());
  let requestSequence = 0;
  let requestedKey = "";
  export type ReadingSnapshot = { id: string; account: string; detail: NovelDetail; content: NovelContent; revealRestricted: boolean; downloadedIds: string[] };
  export function captureSnapshot(): ReadingSnapshot | null {
    return status === "ready" && detail && content ? { id: novelId, account: $session.user?.id ?? "", detail, content, revealRestricted, downloadedIds: [...downloadedIds] } : null;
  }
  export function restoreSnapshot(value: ReadingSnapshot | null) {
    if (!value || value.id !== novelId || value.account !== ($session.user?.id ?? "")) return;
    requestSequence += 1;
    requestedKey = `${novelId}:${offlineOnly}:${$session.loggedIn}:${$session.user?.id ?? ""}`;
    detail = value.detail; content = value.content; status = "ready";
    revealRestricted = value.revealRestricted;
    downloadedIds = new Set(value.downloadedIds);
    progress = readNovelProgress(novelId);
  }
  let localNavigation = $derived(offlineOnly || !$session.loggedIn);
  let fallback = $derived(localNavigation ? "/offline" : `/novels/${novelId}`);
  let blocks = $derived(content ? parseNovelText(content.text, m.novel_default_chapter()) : []);
  let restricted = $derived((detail?.novel.xRestrict ?? 0) > 0 && !$r18DefaultVisible && !revealRestricted);
  let previousHref = $derived(content?.seriesNavigation.previous?.viewable
    ? readingChapterHref(content.seriesNavigation.previous.id, localNavigation, downloadedIds) : null);
  let nextHref = $derived(content?.seriesNavigation.next?.viewable
    ? readingChapterHref(content.seriesNavigation.next.id, localNavigation, downloadedIds) : null);

  $effect(() => {
    const id = novelId;
    const loggedIn = $session.loggedIn;
    const account = $session.user?.id;
    const onlyLocal = offlineOnly;
    if ($sessionRestoring && !onlyLocal) return;
    const key = `${id}:${onlyLocal}:${loggedIn}:${account ?? ""}`;
    if (key === requestedKey) return;
    requestedKey = key;
    void loadReader(id, onlyLocal, loggedIn);
    return () => { requestSequence += 1; };
  });

  async function loadReader(id: string, onlyLocal: boolean, loggedIn: boolean) {
    const sequence = ++requestSequence;
    detail = null; content = null; status = "loading";
    errorMessage = ""; saveError = ""; revealRestricted = false;
    downloadedIds = new Set();
    void listOfflineEntries().then((entries) => {
      if (sequence === requestSequence) downloadedIds = new Set(entries.filter((entry) => entry.kind === "novel").map((entry) => entry.resourceId));
    }).catch(() => {});
    for (let attempt = 1; attempt <= NOVEL_READER_LOAD_ATTEMPTS; attempt += 1) {
      try {
        const result = await loadNovelForReading(id, onlyLocal, loggedIn, { readOfflineText, getNovelDetail, getNovelContent });
        if (sequence !== requestSequence) return;
        detail = result.detail; content = result.content;
        progress = readNovelProgress(id);
        status = "ready";
        return;
      } catch (error) {
        if (sequence !== requestSequence) return;
        if (error instanceof Error && error.message === "reading_login_required") { status = "login"; return; }
        if (onlyLocal || !shouldRetryNovelReaderLoad(error, attempt)) {
          errorMessage = onlyLocal ? m.offline_novel_error() : describeDataFailure(error);
          status = "error";
          return;
        }
        await new Promise((resolve) => setTimeout(resolve, NOVEL_READER_RETRY_DELAY_MS));
        if (sequence !== requestSequence) return;
      }
    }
  }

  function readingStarted() {
    if (!detail || !content) return;
    try {
      addNovelToShelf({ ...detail.novel, series: content.seriesId
        ? { id: content.seriesId, title: content.seriesTitle ?? detail.novel.series?.title ?? "" }
        : detail.novel.series }, "reading");
    } catch { saveError = m.reading_save_failed(); }
    void recordBrowsingHistory({ kind: "novel", resourceId: detail.novel.id,
      title: detail.novel.title, subtitle: detail.novel.author.name,
      thumbnailUrl: detail.novel.coverUrl }).catch(() => {});
  }

  function saveProgress(value: number) {
    if (!detail) return;
    try { saveNovelProgress(detail.novel.id, value); saveError = ""; }
    catch { saveError = m.reading_save_failed(); }
  }
</script>

<svelte:head><title>{detail?.novel.title || m.novel_reader_title()} · PixNya</title></svelte:head>
<AppShell immersive title={m.novel_reader_shell_title()}>
  {#if status === "ready" && detail && content && !restricted}
    {#key novelId}
      <NovelImmersiveReader {detail} {content} {blocks} {fallback} {previousHref} {nextHref}
        offline={localNavigation} initialProgress={progress} onprogress={saveProgress} onstart={readingStarted} />
    {/key}
    {#if saveError}<p class="save-error" role="alert">{saveError}</p>{/if}
  {:else}
    <main class="gate">
      <ReturnLink fallback={fallback} label={m.novel_reader_back()} />
      <section class="state" role="status">
        {#if status === "login"}
          <h1>{m.novel_reader_login_title()}</h1><p>{m.novel_reader_login_description()}</p><a href="/login">{m.common_go_to_login()}</a>
        {:else if status === "error"}
          <h1>{m.novel_reader_load_failed()}</h1><p>{errorMessage}</p>
          <button onclick={() => loadReader(novelId, offlineOnly, $session.loggedIn)}>{m.common_retry()}</button>
        {:else if restricted}
          <h1>{m.novel_reader_restricted_title()}</h1><p>{m.novel_reader_restricted_description()}</p>
          <button onclick={() => { revealRestricted = true; }}>{m.novel_start_reading()}</button>
        {:else}<h1>{m.novel_reader_loading_title()}</h1>{/if}
      </section>
    </main>
  {/if}
</AppShell>

<style>
  .gate { box-sizing: border-box; min-height: 100dvh; padding: calc(18px + env(safe-area-inset-top)) 20px; background: #fbf7ec; }
  .state { max-width: 680px; margin: 28px auto; padding: 24px; border: 1px solid var(--line); border-radius: 18px; background: white; }
  h1 { font-size: var(--type-section); } p { color: var(--muted); }
  button, a { padding: 10px 16px; border: 0; border-radius: 20px; background: var(--pixiv-blue); color: white; font-size: var(--type-body); text-decoration: none; }
  .save-error { position: fixed; z-index: 20; bottom: 28px; left: 16px; right: 16px; padding: 12px; border-radius: 12px; background: #fff2e8; color: #873d15; font-size: var(--type-small); }
</style>
