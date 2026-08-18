<script lang="ts">
  import { m } from "$lib/i18n";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { tokenizeCommentText } from "$lib/comment-emoji";
  import type { PixivComment } from "$lib/types";

  let { comment }: { comment: PixivComment } = $props();
  let segments = $derived(tokenizeCommentText(comment.text));
</script>

{#if comment.stamp?.url}
      <div class="comment-stamp" aria-label={m.comment_stamp_label({ id: comment.stamp.id })}>
    <PixivImage url={comment.stamp.url} alt="" fit="contain" />
  </div>
{/if}
{#if comment.text}
  <p class="comment-text">
    {#each segments as segment}
      {#if segment.kind === "text"}
        {segment.value}
      {:else}
        <span class="inline-emoji" title={segment.token} aria-label={segment.token}>
          <PixivImage url={segment.url} alt="" fit="contain" />
        </span>
      {/if}
    {/each}
  </p>
{/if}

<style>
  .comment-text { margin: 7px 0 0; font-size: var(--type-small); line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; }
  .inline-emoji { display: inline-grid; width: 22px; height: 22px; margin: 0 2px; vertical-align: -.36em; place-items: center; border-radius: 4px; background: #f4f5f6; }
  .comment-stamp { width: 92px; height: 92px; margin-top: 8px; border-radius: 10px; background: #f6f7f8; }
</style>
