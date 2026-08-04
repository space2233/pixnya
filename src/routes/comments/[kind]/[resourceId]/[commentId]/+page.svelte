<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import CommentCard from "$lib/components/CommentCard.svelte";
  import CommentComposer from "$lib/components/CommentComposer.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { recallCommentRoot, type CommentResourceKind } from "$lib/comment-thread-memory";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import {
    addIllustrationComment,
    addNovelComment,
    describeDataFailure,
    getCommentReplies,
    getNovelCommentReplies,
  } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type { PixivComment } from "$lib/types";

  let replies = $state<PixivComment[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let paginationError = $state("");
  let submitting = $state(false);
  let submitError = $state("");
  let requestedKey = $state("");
  let requestSequence = 0;

  let rawKind = $derived(page.params.kind ?? "");
  let resourceKind = $derived<CommentResourceKind>(rawKind === "novel" ? "novel" : "illustration");
  let kindValid = $derived(rawKind === "illustration" || rawKind === "novel");
  let resourceId = $derived(page.params.resourceId ?? "");
  let commentId = $derived(page.params.commentId ?? "");
  let rootComment = $derived(recallCommentRoot(resourceKind, resourceId, commentId));
  let fallback = $derived(resourceKind === "novel" ? `/novels/${resourceId}` : `/artworks/${resourceId}`);
  let focusComposer = $derived(page.url.searchParams.get("compose") === "1");

  type ReplyPageSnapshot = {
    replies: PixivComment[];
    nextCursor: string | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    paginationError: string;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<ReplyPageSnapshot>({
      replies, nextCursor, status, errorMessage, paginationError, requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<ReplyPageSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      replies = value.replies;
      nextCursor = value.nextCursor;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      paginationError = value.paginationError;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
      loadingMore = false;
      submitting = false;
    },
  };

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = kindValid && resourceId && commentId && sessionKey
      ? `${sessionKey}:${resourceKind}:${resourceId}:${commentId}`
      : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      replies = [];
      nextCursor = null;
      status = "idle";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadReplies(key);
    }
  });

  function requestReplies(cursor?: string) {
    return resourceKind === "novel"
      ? getNovelCommentReplies(commentId, cursor)
      : getCommentReplies(commentId, cursor);
  }

  function createReply(text: string) {
    return resourceKind === "novel"
      ? addNovelComment(resourceId, text, commentId)
      : addIllustrationComment(resourceId, text, commentId);
  }

  async function loadReplies(key: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    paginationError = "";
    replies = [];
    nextCursor = null;
    try {
      const result = await requestReplies();
      if (sequence !== requestSequence || key !== requestedKey) return;
      replies = result.comments;
      nextCursor = result.nextCursor ?? null;
      status = "ready";
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedKey) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadMoreReplies() {
    const cursor = nextCursor;
    if (!cursor || loadingMore) return;
    const key = requestedKey;
    const sequence = requestSequence;
    loadingMore = true;
    paginationError = "";
    try {
      const result = await requestReplies(cursor);
      if (sequence !== requestSequence || key !== requestedKey) return;
      const known = new Set(replies.map((reply) => reply.id));
      replies = [...replies, ...result.comments.filter((reply) => !known.has(reply.id))];
      nextCursor = result.nextCursor ?? null;
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) paginationError = describeDataFailure(error);
    } finally {
      loadingMore = false;
    }
  }

  async function submitReply(text: string): Promise<boolean> {
    if (submitting) return false;
    const key = requestedKey;
    const sequence = requestSequence;
    submitting = true;
    submitError = "";
    try {
      const created = await createReply(text);
      if (sequence !== requestSequence || key !== requestedKey) return false;
      replies = [created, ...replies.filter((reply) => reply.id !== created.id)];
      return true;
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) submitError = describeDataFailure(error);
      return false;
    } finally {
      if (sequence === requestSequence && key === requestedKey) submitting = false;
    }
  }
</script>

<svelte:head><title>评论回复 · PixNya</title></svelte:head>

<AppShell title="评论回复">
  <main class="reply-page">
    <ReturnLink {fallback} label="返回作品评论" />

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state-card"><h1>登录后查看回复</h1><p>回复内容通过登录后的 App API 载入。</p><a href="/login?mode=standard">前往登录</a></section>
    {:else if !kindValid || !/^\d+$/.test(resourceId) || !/^\d+$/.test(commentId)}
      <section class="state-card error"><h1>回复地址无效</h1><p>请返回作品详情后重新打开评论。</p></section>
    {:else}
      <section class="thread-card">
        <header><div><span>评论线程</span><h1>{rootComment?.user?.name ? `回复 @${rootComment.user.name}` : "评论回复"}</h1></div><strong>{replies.length} 条已载入回复</strong></header>
        {#if rootComment}
          <div class="root-comment"><CommentCard comment={rootComment} {resourceKind} {resourceId} /></div>
        {:else}
          <p class="root-missing">直接打开了回复页；根评论摘要会在从作品详情进入时显示。</p>
        {/if}
        <div class="composer-wrap">
          <CommentComposer
            placeholder="回复这条评论"
            submitLabel="发布回复"
            {submitting}
            errorMessage={submitError}
            autofocus={focusComposer}
            onsubmit={submitReply}
          />
        </div>

        {#if status === "loading"}
          <div class="reply-state"><span class="spinner"></span><p>正在载入回复…</p></div>
        {:else if status === "error"}
          <div class="reply-state error" role="alert"><p>{errorMessage}</p><button type="button" onclick={() => loadReplies(requestedKey)}>重试</button></div>
        {:else if replies.length === 0}
          <p class="empty">还没有回复。</p>
        {:else}
          <div class="reply-list">
            {#each replies as reply (reply.id)}
              <CommentCard comment={reply} {resourceKind} {resourceId} />
            {/each}
          </div>
        {/if}
        {#if paginationError}<p class="inline-error" role="alert">{paginationError}</p>{/if}
        {#if nextCursor && status === "ready"}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMoreReplies}>{loadingMore ? "正在载入…" : "加载更多回复"}</button>{/if}
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .reply-page { box-sizing: border-box; width: min(900px, 100%); min-height: 100%; margin: 0 auto; padding: 28px 24px 100px; }
  .thread-card { margin-top: 18px; overflow: hidden; border: 1px solid var(--line); border-radius: 14px; background: white; }
  .thread-card > header { display: flex; align-items: end; justify-content: space-between; padding: 22px; border-bottom: 1px solid var(--line); }
  .thread-card > header span { color: var(--pixiv-blue); font-size: 8px; font-weight: 700; }
  .thread-card > header h1 { margin: 5px 0 0; font-size: 21px; }
  .thread-card > header strong { color: var(--muted); font-size: 8px; font-weight: 500; }
  .root-comment { margin: 16px; overflow: hidden; border: 1px solid #cfe9f8; border-radius: 10px; background: #f7fcff; }
  .root-missing { margin: 16px; padding: 14px; color: var(--muted); border-radius: 8px; background: #f7f8f9; font-size: 9px; }
  .composer-wrap { padding: 0 16px 16px; }
  .reply-list { overflow: hidden; border-top: 1px solid var(--line); }
  .reply-list :global(> article + article) { border-top: 1px solid var(--line); }
  .reply-state { display: flex; gap: 11px; align-items: center; justify-content: center; min-height: 130px; border-top: 1px solid var(--line); color: var(--muted); font-size: 9px; }
  .reply-state.error { color: #a44f5e; }
  .reply-state button, .load-more { height: 34px; padding: 0 15px; border: 0; border-radius: 17px; cursor: pointer; font-size: 9px; font-weight: 700; }
  .reply-state button { color: white; background: var(--pixiv-blue); }
  .empty { margin: 0; padding: 40px; color: var(--muted); border-top: 1px solid var(--line); text-align: center; font-size: 9px; }
  .load-more { display: block; min-width: 140px; margin: 18px auto; color: #59636a; border: 1px solid var(--line); background: white; }
  .load-more:disabled { cursor: wait; opacity: .58; }
  .inline-error { margin: 13px; color: #a44f5e; text-align: center; font-size: 8px; }
  .spinner { width: 25px; height: 25px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .state-card { display: grid; gap: 8px; margin-top: 18px; padding: 24px; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .state-card h1, .state-card p { margin: 0; }
  .state-card p { color: var(--muted); font-size: 9px; }
  .state-card a { width: fit-content; margin-top: 7px; padding: 9px 15px; color: white; border-radius: 18px; background: var(--pixiv-blue); text-decoration: none; font-size: 9px; font-weight: 700; }
  .state-card.error { color: #a44f5e; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 620px) {
    .reply-page { padding: 18px 12px 92px; }
    .thread-card > header { align-items: start; padding: 17px 15px; }
    .thread-card > header h1 { font-size: 18px; }
    .root-comment { margin: 11px; }
    .composer-wrap { padding: 0 11px 11px; }
  }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
