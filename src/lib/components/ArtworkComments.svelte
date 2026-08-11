<script lang="ts">
  import CommentCard from "$lib/components/CommentCard.svelte";
  import CommentComposer from "$lib/components/CommentComposer.svelte";
  import { m } from "$lib/i18n";
  import {
    recallCommentThread,
    rememberCommentRoot,
    rememberCommentThread,
    type CommentResourceKind,
  } from "$lib/comment-thread-memory";
  import {
    addIllustrationComment,
    addNovelComment,
    deleteIllustrationComment,
    deleteNovelComment,
    describeDataFailure,
    getIllustrationComments,
    getNovelComments,
  } from "$lib/pixiv-api";
  import { session } from "$lib/session";
  import type { CommentSubmission, PixivComment } from "$lib/types";

  let {
    illustrationId,
    novelId,
    initialCount = 0,
  }: { illustrationId?: string; novelId?: string; initialCount?: number } = $props();

  let comments = $state<PixivComment[]>([]);
  let nextCursor = $state<string | null>(null);
  let totalComments = $state(0);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let paginationError = $state("");
  let submitting = $state(false);
  let submitError = $state("");
  let requestedKey = $state("");
  let requestSequence = 0;
  let resourceId = $derived(novelId ?? illustrationId ?? "");
  let resourceKind = $derived<CommentResourceKind>(novelId ? "novel" : "illustration");

  $effect(() => {
    const key = resourceId ? `${resourceKind}:${resourceId}` : "";
    if (!key || requestedKey === key) return;
    requestedKey = key;
    const remembered = recallCommentThread(resourceKind, resourceId);
    if (remembered) {
      comments = remembered.comments;
      nextCursor = remembered.nextCursor;
      totalComments = remembered.totalComments;
      status = "ready";
      errorMessage = "";
      return;
    }
    totalComments = initialCount;
    void loadComments(key, resourceId);
  });

  function requestComments(id: string, cursor?: string) {
    return resourceKind === "novel"
      ? getNovelComments(id, cursor)
      : getIllustrationComments(id, cursor);
  }

  function createComment(id: string, submission: CommentSubmission) {
    return resourceKind === "novel"
      ? addNovelComment(id, submission.text, undefined, submission.stampId)
      : addIllustrationComment(id, submission.text, undefined, submission.stampId);
  }

  function rememberCurrentThread() {
    rememberCommentThread(resourceKind, resourceId, { comments, nextCursor, totalComments });
  }

  async function loadComments(key: string, id: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    paginationError = "";
    comments = [];
    nextCursor = null;
    try {
      const result = await requestComments(id);
      if (sequence !== requestSequence || requestedKey !== key) return;
      comments = result.comments;
      nextCursor = result.nextCursor ?? null;
      totalComments = result.totalComments;
      status = "ready";
      rememberCurrentThread();
    } catch (error) {
      if (sequence !== requestSequence || requestedKey !== key) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function refreshComments() {
    if (!resourceId || status === "loading") return;
    await loadComments(requestedKey, resourceId);
  }

  async function loadMoreComments() {
    const cursor = nextCursor;
    if (!cursor || loadingMore) return;
    const id = resourceId;
    const key = requestedKey;
    const sequence = requestSequence;
    loadingMore = true;
    paginationError = "";
    try {
      const result = await requestComments(id, cursor);
      if (sequence !== requestSequence || key !== requestedKey) return;
      const known = new Set(comments.map((comment) => comment.id));
      comments = [...comments, ...result.comments.filter((comment) => !known.has(comment.id))];
      nextCursor = result.nextCursor ?? null;
      totalComments = Math.max(totalComments, result.totalComments);
      rememberCurrentThread();
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) {
        paginationError = describeDataFailure(error);
      }
    } finally {
      loadingMore = false;
    }
  }

  async function submitComment(submission: CommentSubmission): Promise<boolean> {
    if (submitting) return false;
    const key = requestedKey;
    const sequence = requestSequence;
    submitting = true;
    submitError = "";
    try {
      const created = await createComment(resourceId, submission);
      if (sequence !== requestSequence || key !== requestedKey) return false;
      comments = [created, ...comments.filter((comment) => comment.id !== created.id)];
      totalComments += 1;
      rememberCurrentThread();
      return true;
    } catch (error) {
      if (sequence === requestSequence && key === requestedKey) {
        submitError = describeDataFailure(error);
      }
      return false;
    } finally {
      if (sequence === requestSequence && key === requestedKey) submitting = false;
    }
  }

  async function deleteComment(comment: PixivComment): Promise<boolean> {
    try {
      if (resourceKind === "novel") await deleteNovelComment(comment.id);
      else await deleteIllustrationComment(comment.id);
      comments = comments.filter((candidate) => candidate.id !== comment.id);
      totalComments = Math.max(0, totalComments - 1);
      rememberCurrentThread();
      return true;
    } catch (error) {
      submitError = describeDataFailure(error);
      return false;
    }
  }

  function repliesPath(comment: PixivComment, compose = false): string {
    const path = `/comments/${resourceKind}/${resourceId}/${comment.id}`;
    return compose ? `${path}?compose=1` : path;
  }

  function rememberRoot(comment: PixivComment) {
    rememberCommentRoot(resourceKind, resourceId, comment);
    rememberCurrentThread();
  }
</script>

<section class="comments-section" aria-labelledby="comments-title">
  <header>
    <div><h2 id="comments-title">{m.comments_title()}</h2><p>{m.comments_count({ count: totalComments })}</p></div>
    <button type="button" disabled={status === "loading"} onclick={refreshComments}>{m.common_refresh()}</button>
  </header>

  <CommentComposer
    onsubmit={submitComment}
    {submitting}
    errorMessage={submitError}
  />

  {#if status === "loading"}
    <div class="comment-state"><span class="spinner"></span><p>{m.comments_loading()}</p></div>
  {:else if status === "error"}
    <div class="comment-state error" role="alert"><span>!</span><p>{errorMessage}</p><button type="button" onclick={() => loadComments(requestedKey, resourceId)}>{m.common_retry()}</button></div>
  {:else if comments.length === 0}
    <p class="empty">{m.comments_empty()}</p>
  {:else}
    <div class="comment-list">
      {#each comments as comment (comment.id)}
        <CommentCard
          {comment}
          {resourceKind}
          {resourceId}
          replyHref={repliesPath(comment, true)}
          repliesHref={repliesPath(comment)}
          onopen={() => rememberRoot(comment)}
          canDelete={comment.user?.id === $session.user?.id}
          ondelete={() => deleteComment(comment)}
        />
      {/each}
    </div>
  {/if}

  {#if paginationError}<p class="inline-error center" role="alert">{paginationError}</p>{/if}
  {#if nextCursor && status === "ready"}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMoreComments}>{loadingMore ? m.common_loading() : m.comments_load_more()}</button>{/if}
</section>

<style>
  .comments-section { margin-top: 42px; }
  header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  header h2 { margin: 0; font-size: 18px; }
  header p { margin: 5px 0 0; color: var(--muted); font-size: 9px; }
  header > button { height: 31px; padding: 0 14px; color: var(--pixiv-blue); border: 1px solid #bde1f7; border-radius: 16px; background: white; cursor: pointer; font-size: 8px; }
  button:disabled { cursor: wait; opacity: .58; }
  .comment-state { display: flex; gap: 12px; align-items: center; justify-content: center; min-height: 120px; margin-top: 14px; color: var(--muted); border: 1px dashed var(--line); border-radius: 9px; font-size: 10px; }
  .comment-state.error { color: #9d5964; }
  .comment-state.error > span { display: grid; width: 30px; height: 30px; place-items: center; border-radius: 50%; background: #fff0f3; }
  .comment-state button { min-width: 76px; height: 34px; color: white; border: 0; border-radius: 17px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; }
  .spinner { width: 25px; height: 25px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .empty { padding: 34px; color: var(--muted); border: 1px dashed var(--line); border-radius: 9px; font-size: 10px; text-align: center; }
  .comment-list { margin-top: 14px; overflow: hidden; border: 1px solid var(--line); border-radius: 10px; background: white; }
  .comment-list :global(> article + article) { border-top: 1px solid var(--line); }
  .inline-error { margin: 8px 0 0; color: #a44f5e; font-size: 8px; }
  .inline-error.center { text-align: center; }
  .load-more { display: block; min-width: 132px; height: 34px; margin: 20px auto 0; color: #59636a; border: 1px solid var(--line); border-radius: 17px; background: white; cursor: pointer; font-size: 9px; font-weight: 700; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
