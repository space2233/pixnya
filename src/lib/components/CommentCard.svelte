<script lang="ts">
  import CommentText from "$lib/components/CommentText.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
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
    canDelete = false,
    ondelete,
  }: {
    comment: PixivComment;
    resourceKind: CommentResourceKind;
    resourceId: string;
    replyHref?: string | null;
    repliesHref?: string | null;
    onopen?: () => void;
    canDelete?: boolean;
    ondelete?: () => Promise<boolean>;
  } = $props();

  let muted = $state(false);
  let menuOpen = $state(false);
  let reportOpen = $state(false);
  let selectedReason = $state<LocalReportReason>(LOCAL_REPORT_REASONS[0]);
  let deleteOpen = $state(false);
  let deleting = $state(false);
  let deleteError = $state("");
  const reasonLabels: Record<LocalReportReason, () => string> = {
    sexual_or_vulgar: m.comment_reason_sexual,
    hate_speech: m.comment_reason_hate,
    terrorism: m.comment_reason_terrorism,
    dangerous_organization: m.comment_reason_dangerous_org,
    sensitive_event: m.comment_reason_sensitive_event,
    bullying_or_harassment: m.comment_reason_bullying,
    dangerous_goods: m.comment_reason_dangerous_goods,
    cannabis: m.comment_reason_cannabis,
    tobacco_or_alcohol: m.comment_reason_tobacco_alcohol,
  };

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
      : date.toLocaleString(currentAppLocale(), { dateStyle: "short", timeStyle: "short" });
  }

  function initial(): string {
    return Array.from(comment.user?.name || "P")[0] ?? "P";
  }

  async function confirmDelete() {
    if (!ondelete || deleting) return;
    deleting = true;
    deleteError = "";
    try {
      if (await ondelete()) deleteOpen = false;
      else deleteError = m.common_retry();
    } finally {
      deleting = false;
    }
  }
</script>

{#if muted}
  <article class="muted-card">
    <div><strong>{m.comment_muted_title()}</strong><p>{m.comment_muted_description()}</p></div>
    <button type="button" onclick={unmute}>{m.comment_show()}</button>
  </article>
{:else}
  <article class="comment-card">
    <a class="comment-avatar" href={comment.user ? `/users/${comment.user.id}` : undefined}>
      <b>{initial()}</b><PixivImage url={comment.user?.avatarUrl} alt="" />
    </a>
    <div class="comment-body">
      <div class="comment-meta">
        <strong>{comment.user?.name || m.comment_deleted_user()}</strong>
        <time>{displayDate(comment.date)}</time>
        <button class="more" type="button" aria-label={m.comment_manage()} aria-expanded={menuOpen} onclick={() => (menuOpen = !menuOpen)}>•••</button>
      </div>
      {#if comment.parent}<small class="parent-reference">{m.comment_reply_reference({ user: comment.parent.userName || m.comment_unknown_user(), text: comment.parent.text })}</small>{/if}
      <CommentText {comment} />
      <div class="comment-actions">
        {#if replyHref}<a href={replyHref} onclick={onopen}>{m.comment_reply()}</a>{/if}
        {#if comment.hasReplies && repliesHref}<a href={repliesHref} onclick={onopen}>{m.comment_view_replies()}</a>{/if}
      </div>
      {#if menuOpen}
        <div class="moderation-menu">
          {#if canDelete}<button class="danger" type="button" onclick={() => { menuOpen = false; deleteOpen = true; }}>{m.common_delete()}</button>{/if}
          <button type="button" onclick={mute}>{m.comment_mute_local()}</button>
          <button type="button" onclick={() => { menuOpen = false; reportOpen = true; }}>{m.comment_report_local()}</button>
        </div>
      {/if}
    </div>
  </article>
{/if}

{#if deleteOpen}
  <div class="modal-backdrop" role="presentation" onclick={(event) => { if (event.target === event.currentTarget && !deleting) deleteOpen = false; }}>
    <div class="delete-dialog" role="dialog" aria-modal="true" aria-labelledby={`delete-title-${comment.id}`}>
      <h2 id={`delete-title-${comment.id}`}>{m.common_confirm_delete()}</h2>
      <p>{m.comment_delete_confirm()}</p>
      {#if deleteError}<p class="delete-error" role="alert">{deleteError}</p>{/if}
      <div class="dialog-actions">
        <button type="button" disabled={deleting} onclick={() => (deleteOpen = false)}>{m.common_cancel()}</button>
        <button class="danger-action" type="button" disabled={deleting} onclick={confirmDelete}>{deleting ? m.comment_deleting() : m.common_delete()}</button>
      </div>
    </div>
  </div>
{/if}

{#if reportOpen}
  <div class="modal-backdrop" role="presentation" onclick={(event) => { if (event.target === event.currentTarget) reportOpen = false; }}>
    <div class="report-dialog" role="dialog" aria-modal="true" aria-labelledby={`report-title-${comment.id}`}>
      <h2 id={`report-title-${comment.id}`}>{m.comment_report_local()}</h2>
      <p>{m.comment_report_description()}</p>
      <div class="reason-list">
        {#each LOCAL_REPORT_REASONS as reason}
          <label><input type="radio" name={`report-${comment.id}`} value={reason} bind:group={selectedReason} /> <span>{reasonLabels[reason]()}</span></label>
        {/each}
      </div>
      <div class="dialog-actions">
        <button type="button" onclick={() => (reportOpen = false)}>{m.common_cancel()}</button>
        <button class="primary" type="button" onclick={submitLocalReport}>{m.comment_report_submit()}</button>
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
  .comment-meta strong { font-size: var(--type-small); }
  .comment-meta time { color: var(--soft-muted); font-size: var(--type-caption); }
  .more { position: absolute; top: -8px; right: 0; min-width: 34px; height: 30px; color: var(--muted); border: 0; background: transparent; cursor: pointer; letter-spacing: 1px; }
  .parent-reference { display: block; overflow: hidden; margin-top: 7px; padding: 7px 9px; color: var(--muted); border-left: 2px solid #baddf1; background: #f6fafc; font-size: var(--type-caption); text-overflow: ellipsis; white-space: nowrap; }
  .comment-actions { display: flex; gap: 13px; margin-top: 8px; }
  .comment-actions a { color: var(--pixiv-blue); font-size: var(--type-caption); text-decoration: none; }
  .moderation-menu { display: flex; gap: 7px; flex-wrap: wrap; margin-top: 9px; padding: 9px; border-radius: 7px; background: #f6f7f8; }
  .moderation-menu button { height: 29px; padding: 0 11px; color: #59636a; border: 1px solid var(--line); border-radius: 15px; background: white; cursor: pointer; font-size: var(--type-body); }
  .moderation-menu button.danger { color: #b24958; border-color: #f0c7ce; }
  .muted-card { display: flex; gap: 16px; align-items: center; justify-content: space-between; padding: 15px; color: var(--muted); background: #fafafa; }
  .muted-card strong { color: #697279; font-size: var(--type-caption); }
  .muted-card p { margin: 4px 0 0; font-size: var(--type-caption); }
  .muted-card button { flex: 0 0 auto; height: 30px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #bde1f7; border-radius: 15px; background: white; cursor: pointer; font-size: var(--type-body); }
  .modal-backdrop { position: fixed; z-index: 80; inset: 0; display: grid; padding: 20px; place-items: center; background: rgba(0,0,0,.42); }
  .report-dialog { box-sizing: border-box; width: min(520px, 100%); max-height: min(720px, calc(100dvh - 40px)); overflow-y: auto; padding: 22px; border-radius: 16px; background: white; box-shadow: 0 18px 60px rgba(0,0,0,.2); }
  .report-dialog h2 { margin: 0; font-size: var(--type-section); }
  .report-dialog > p { margin: 9px 0 16px; color: var(--muted); font-size: var(--type-caption); line-height: 1.65; }
  .reason-list { display: grid; gap: 6px; }
  .reason-list label { display: flex; gap: 9px; align-items: center; min-height: 38px; padding: 0 11px; border: 1px solid var(--line); border-radius: 8px; cursor: pointer; font-size: var(--type-caption); }
  .reason-list input { accent-color: var(--pixiv-blue); }
  .dialog-actions { display: grid; grid-template-columns: 1fr 1fr; gap: 9px; margin-top: 17px; }
  .dialog-actions button { height: 38px; border: 1px solid var(--line); border-radius: 19px; background: white; cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .dialog-actions .primary { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  .delete-dialog { box-sizing: border-box; width: min(420px, 100%); padding: 22px; border-radius: 16px; background: white; box-shadow: 0 18px 60px rgba(0,0,0,.2); }
  .delete-dialog h2 { margin: 0; font-size: var(--type-section); }
  .delete-dialog > p { margin: 10px 0 0; color: var(--muted); font-size: var(--type-caption); line-height: 1.65; }
  .delete-dialog .delete-error { color: #b24958; }
  .dialog-actions .danger-action { color: white; border-color: #d55464; background: #d55464; }
  @media (max-width: 620px) {
    .comment-card { grid-template-columns: 34px minmax(0,1fr); padding: 12px; }
    .comment-avatar { width: 34px; height: 34px; }
    .modal-backdrop { padding: 12px; align-items: end; }
    .report-dialog { max-height: calc(100dvh - 24px); padding: 18px; border-radius: 16px 16px 10px 10px; }
  }
</style>
