<script lang="ts">
  import CommentText from "$lib/components/CommentText.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import {
    isCommentMuted,
    LOCAL_REPORT_REASONS,
    muteComment,
    recordLocalReport,
    unmuteComment,
    type LocalReportReason,
  } from "$lib/comment-moderation";
  import type { CommentResourceKind } from "$lib/comment-thread-memory";
  import type { PixivComment } from "$lib/types";

  let {
    comment,
    resourceKind,
    resourceId,
    replyHref,
    repliesHref,
    onopen,
  }: {
    comment: PixivComment;
    resourceKind: CommentResourceKind;
    resourceId: string;
    replyHref?: string | null;
    repliesHref?: string | null;
    onopen?: () => void;
  } = $props();

  let muted = $state(false);
  let menuOpen = $state(false);
  let reportOpen = $state(false);
  let selectedReason = $state<LocalReportReason>(LOCAL_REPORT_REASONS[0]);

  $effect(() => {
    muted = isCommentMuted(comment.id);
  });

  function mute() {
    muteComment(comment.id, comment.user?.id);
    muted = true;
    menuOpen = false;
  }

  function unmute() {
    unmuteComment(comment.id);
    muted = false;
  }

  function submitLocalReport() {
    recordLocalReport({
      commentId: comment.id,
      resourceKind,
      resourceId,
      reason: selectedReason,
    });
    reportOpen = false;
    menuOpen = false;
    muted = true;
  }

  function displayDate(value: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? value
      : date.toLocaleString("zh-CN", { dateStyle: "short", timeStyle: "short" });
  }

  function initial(): string {
    return Array.from(comment.user?.name || "P")[0] ?? "P";
  }
</script>

{#if muted}
  <article class="muted-card">
    <div><strong>此评论已在本地屏蔽</strong><p>内容仍保留在 Pixiv，仅在这台设备上隐藏。</p></div>
    <button type="button" onclick={unmute}>显示评论</button>
  </article>
{:else}
  <article class="comment-card">
    <a class="comment-avatar" href={comment.user ? `/users/${comment.user.id}` : undefined}>
      <b>{initial()}</b><PixivImage url={comment.user?.avatarUrl} alt="" />
    </a>
    <div class="comment-body">
      <div class="comment-meta">
        <strong>{comment.user?.name || "已注销用户"}</strong>
        <time>{displayDate(comment.date)}</time>
        <button class="more" type="button" aria-label="评论管理" aria-expanded={menuOpen} onclick={() => (menuOpen = !menuOpen)}>•••</button>
      </div>
      {#if comment.parent}<small class="parent-reference">回复 @{comment.parent.userName || "用户"}：{comment.parent.text}</small>{/if}
      <CommentText {comment} />
      <div class="comment-actions">
        {#if replyHref}<a href={replyHref} onclick={onopen}>回复</a>{/if}
        {#if comment.hasReplies && repliesHref}<a href={repliesHref} onclick={onopen}>查看回复</a>{/if}
      </div>
      {#if menuOpen}
        <div class="moderation-menu">
          <button type="button" onclick={mute}>本地屏蔽</button>
          <button type="button" onclick={() => { menuOpen = false; reportOpen = true; }}>本地举报并屏蔽</button>
        </div>
      {/if}
    </div>
  </article>
{/if}

{#if reportOpen}
  <div class="modal-backdrop" role="presentation" onclick={(event) => { if (event.target === event.currentTarget) reportOpen = false; }}>
    <div class="report-dialog" role="dialog" aria-modal="true" aria-labelledby={`report-title-${comment.id}`}>
      <h2 id={`report-title-${comment.id}`}>本地举报并屏蔽</h2>
      <p>选择原因后，PixNya 只会把记录保存在本机并隐藏这条评论，不会向 Pixiv 发送举报。</p>
      <div class="reason-list">
        {#each LOCAL_REPORT_REASONS as reason}
          <label><input type="radio" name={`report-${comment.id}`} value={reason} bind:group={selectedReason} /> <span>{reason}</span></label>
        {/each}
      </div>
      <div class="dialog-actions">
        <button type="button" onclick={() => (reportOpen = false)}>取消</button>
        <button class="primary" type="button" onclick={submitLocalReport}>记录并屏蔽</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .comment-card { display: grid; grid-template-columns: 38px minmax(0,1fr); gap: 11px; padding: 15px; background: white; }
  .comment-card + :global(.comment-card), .comment-card + :global(.muted-card), .muted-card + :global(.comment-card), .muted-card + :global(.muted-card) { border-top: 1px solid var(--line); }
  .comment-avatar { position: relative; display: grid; overflow: hidden; width: 38px; height: 38px; place-items: center; color: white; border-radius: 50%; background: var(--pixiv-blue); text-decoration: none; }
  .comment-avatar :global(img) { position: absolute; z-index: 1; inset: 0; }
  .comment-meta { position: relative; display: flex; gap: 10px; align-items: baseline; padding-right: 35px; }
  .comment-meta strong { font-size: 10px; }
  .comment-meta time { color: var(--soft-muted); font-size: 8px; }
  .more { position: absolute; top: -8px; right: 0; min-width: 34px; height: 30px; color: var(--muted); border: 0; background: transparent; cursor: pointer; letter-spacing: 1px; }
  .parent-reference { display: block; overflow: hidden; margin-top: 7px; padding: 7px 9px; color: var(--muted); border-left: 2px solid #baddf1; background: #f6fafc; font-size: 8px; text-overflow: ellipsis; white-space: nowrap; }
  .comment-actions { display: flex; gap: 13px; margin-top: 8px; }
  .comment-actions a { color: var(--pixiv-blue); font-size: 9px; text-decoration: none; }
  .moderation-menu { display: flex; gap: 7px; flex-wrap: wrap; margin-top: 9px; padding: 9px; border-radius: 7px; background: #f6f7f8; }
  .moderation-menu button { height: 29px; padding: 0 11px; color: #59636a; border: 1px solid var(--line); border-radius: 15px; background: white; cursor: pointer; font-size: 8px; }
  .muted-card { display: flex; gap: 16px; align-items: center; justify-content: space-between; padding: 15px; color: var(--muted); background: #fafafa; }
  .muted-card strong { color: #697279; font-size: 9px; }
  .muted-card p { margin: 4px 0 0; font-size: 8px; }
  .muted-card button { flex: 0 0 auto; height: 30px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #bde1f7; border-radius: 15px; background: white; cursor: pointer; font-size: 8px; }
  .modal-backdrop { position: fixed; z-index: 80; inset: 0; display: grid; padding: 20px; place-items: center; background: rgba(0,0,0,.42); }
  .report-dialog { box-sizing: border-box; width: min(520px, 100%); max-height: min(720px, calc(100dvh - 40px)); overflow-y: auto; padding: 22px; border-radius: 16px; background: white; box-shadow: 0 18px 60px rgba(0,0,0,.2); }
  .report-dialog h2 { margin: 0; font-size: 18px; }
  .report-dialog > p { margin: 9px 0 16px; color: var(--muted); font-size: 9px; line-height: 1.65; }
  .reason-list { display: grid; gap: 6px; }
  .reason-list label { display: flex; gap: 9px; align-items: center; min-height: 38px; padding: 0 11px; border: 1px solid var(--line); border-radius: 8px; cursor: pointer; font-size: 9px; }
  .reason-list input { accent-color: var(--pixiv-blue); }
  .dialog-actions { display: grid; grid-template-columns: 1fr 1fr; gap: 9px; margin-top: 17px; }
  .dialog-actions button { height: 38px; border: 1px solid var(--line); border-radius: 19px; background: white; cursor: pointer; font-size: 9px; font-weight: 700; }
  .dialog-actions .primary { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  @media (max-width: 620px) {
    .comment-card { grid-template-columns: 34px minmax(0,1fr); padding: 12px; }
    .comment-avatar { width: 34px; height: 34px; }
    .modal-backdrop { padding: 12px; align-items: end; }
    .report-dialog { max-height: calc(100dvh - 24px); padding: 18px; border-radius: 16px 16px 10px 10px; }
  }
</style>
