<script lang="ts">
  import { page } from "$app/state";
  import { tick } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import NovelImmersiveReader from "$lib/components/NovelImmersiveReader.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
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
  let progress = $state(0);
  let revealRestricted = $state(false);
  let requestedKey = $state("");
  let requestSequence = 0;
  let novelId = $derived(page.params.id ?? "");
  let blocks = $derived(content ? parseNovelText(content.text, m.novel_default_chapter()) : []);
  let restricted = $derived((detail?.novel.xRestrict ?? 0) > 0);
  let fallback = $derived(`/novels/${novelId}`);

  type NovelReaderSnapshot = {
    detail: NovelDetail | null;
    content: NovelContent | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    progress: number;
    revealRestricted: boolean;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<NovelReaderSnapshot>({
      detail, content, status, errorMessage, progress, revealRestricted, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<NovelReaderSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      detail = value.detail;
      content = value.content;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      progress = value.progress;
      revealRestricted = value.revealRestricted;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
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
        title: nextDetail.novel.title || m.common_untitled(),
        subtitle: nextDetail.novel.author.name || m.common_unknown_author(),
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
    const stored = Number(localStorage.getItem(progressKey(id)) ?? "0");
    progress = Number.isFinite(stored) ? Math.min(1, Math.max(0, stored)) : 0;
  }

  function saveProgress(value: number) {
    if (!novelId) return;
    progress = value;
    localStorage.setItem(progressKey(novelId), String(value));
  }
</script>

<svelte:head><title>{detail?.novel.title || m.novel_reader_title()} · PixNya</title></svelte:head>

<AppShell immersive title={m.novel_reader_shell_title()}>
  {#if !$sessionRestoring && !$session.loggedIn}
    <section class="gate">
      <ReturnLink fallback={fallback} label={m.novel_reader_back()} />
      <div class="state">
        <Icon name="user" size={27} />
        <div><h1>{m.novel_reader_login_title()}</h1><p>{m.novel_reader_login_description()}</p></div>
        <a href="/login">{m.common_go_to_login()}</a>
      </div>
    </section>
  {:else if status === "loading" || $sessionRestoring}
    <section class="gate">
      <ReturnLink fallback={fallback} label={m.novel_reader_back()} />
      <div class="state">
        <span class="spinner"></span>
        <div><h1>{m.novel_reader_loading_title()}</h1><p>{m.novel_reader_loading_description()}</p></div>
      </div>
    </section>
  {:else if status === "error"}
    <section class="gate">
      <ReturnLink fallback={fallback} label={m.novel_reader_back()} />
      <div class="state error" role="alert">
        <span>!</span>
        <div><h1>{m.novel_reader_load_failed()}</h1><p>{errorMessage}</p></div>
        <button type="button" onclick={() => loadReader(requestedKey, novelId)}>{m.common_retry()}</button>
      </div>
    </section>
  {:else if detail && content && restricted && !$r18DefaultVisible && !revealRestricted}
    <section class="gate">
      <ReturnLink fallback={fallback} label={m.novel_reader_back()} />
      <div class="state restricted" role="status">
        <span>R18</span>
        <div><h1>{m.novel_reader_restricted_title()}</h1><p>{m.novel_reader_restricted_description()}</p></div>
        <button type="button" onclick={() => (revealRestricted = true)}>{m.novel_start_reading()}</button>
      </div>
    </section>
  {:else if detail && content}
    <NovelImmersiveReader
      {detail}
      {content}
      {blocks}
      {fallback}
      initialProgress={progress}
      onprogress={saveProgress}
    />
  {/if}
</AppShell>

<style>
  .gate {
    box-sizing: border-box;
    min-height: 100dvh;
    padding: calc(18px + env(safe-area-inset-top, 0px)) 20px calc(24px + env(safe-area-inset-bottom, 0px));
    background: #fbf7ec;
  }
  .state {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    margin-top: 22px;
    padding: 21px;
    border: 1px solid var(--line);
    border-radius: 11px;
    background: white;
  }
  .state h1 { margin: 0; font-size: var(--type-label); }
  .state p { margin: 5px 0 0; color: var(--muted); font-size: var(--type-caption); }
  .state a, .state button {
    padding: 10px 17px;
    color: white;
    border: 0;
    border-radius: 20px;
    background: var(--pixiv-blue);
    cursor: pointer;
    font-size: var(--type-body);
    font-weight: 700;
    text-decoration: none;
  }
  .state.error > span {
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    color: #a34e5d;
    border-radius: 50%;
    background: #fff0f3;
  }
  .spinner {
    width: 29px;
    height: 29px;
    border: 3px solid #dceefb;
    border-top-color: var(--pixiv-blue);
    border-radius: 50%;
    animation: spin .8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 720px) {
    .state { grid-template-columns: 38px minmax(0, 1fr); }
    .state a, .state button { grid-column: 1 / -1; text-align: center; }
  }
</style>
