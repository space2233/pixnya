<script lang="ts">
  import ArtworkThumbnail from "$lib/components/ArtworkThumbnail.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { describeDataFailure, setIllustrationBookmark } from "$lib/pixiv-api";
  import { r18DefaultVisible } from "$lib/preferences";
  import type { IllustrationSummary } from "$lib/types";

  let {
    illustration,
    tone = 1,
    rank,
  }: {
    illustration: IllustrationSummary;
    tone?: number;
    rank?: number;
  } = $props();

  let revealRestricted = $state(false);
  let restricted = $derived(illustration.xRestrict > 0);
  let bookmarked = $state(false);
  let bookmarkPending = $state(false);
  let bookmarkError = $state("");

  $effect(() => {
    bookmarked = illustration.isBookmarked;
    bookmarkError = "";
  });

  async function toggleBookmark(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (bookmarkPending) return;
    const previous = bookmarked;
    bookmarked = !previous;
    bookmarkPending = true;
    bookmarkError = "";
    try {
      await setIllustrationBookmark(illustration.id, bookmarked);
    } catch (error) {
      bookmarked = previous;
      bookmarkError = describeDataFailure(error);
    } finally {
      bookmarkPending = false;
    }
  }
</script>

<article class="artwork-card">
  <div class="cover" class:concealed={restricted && !$r18DefaultVisible && !revealRestricted}>
    <a
      class="cover-link"
      href={`/artworks/${illustration.id}`}
      aria-label={`查看作品：${illustration.title || "无题"}`}
    ></a>
    <ArtworkThumbnail
      url={illustration.thumbnailUrl}
      alt={restricted && !$r18DefaultVisible && !revealRestricted ? "受限内容缩略图已模糊" : illustration.title || "无题作品"}
      {tone}
    />
    {#if rank !== undefined}<span class="rank-number">{rank}</span>{/if}
    {#if illustration.pageCount > 1}
      <span class="page-count">▣ {illustration.pageCount}</span>
    {/if}
    {#if illustration.aiType === 2}<span class="ai-label">AI</span>{/if}
    <button
      type="button"
      class="bookmark"
      class:active={bookmarked}
      class:pending={bookmarkPending}
      disabled={bookmarkPending}
      aria-label={bookmarked ? "取消收藏" : "收藏作品"}
      title={bookmarkError || (bookmarked ? "取消收藏" : "收藏作品")}
      onclick={toggleBookmark}
    >
      <Icon name="heart" size={19} />
    </button>
    {#if restricted && !$r18DefaultVisible && !revealRestricted}
      <button type="button" class="reveal" onclick={() => (revealRestricted = true)}>
        {illustration.xRestrict >= 2 ? "R-18G" : "R-18"} · 点击显示
      </button>
    {/if}
  </div>
  <h3 title={illustration.title}>
    <a href={`/artworks/${illustration.id}`}>{illustration.title || "无题"}</a>
  </h3>
  <p title={illustration.author.name}>
    <a href={`/users/${illustration.author.id}`}>{illustration.author.name || illustration.author.account}</a>
  </p>
  {#if bookmarkError}<small class="bookmark-error" role="alert">{bookmarkError}</small>{/if}
</article>

<style>
  .artwork-card { min-width: 0; }

  .cover {
    position: relative;
    overflow: hidden;
    aspect-ratio: 1;
    border-radius: 7px;
    background: #edf1f4;
  }

  .cover.concealed :global(img) { filter: blur(18px) brightness(0.7); transform: scale(1.12); }

  .cover-link {
    position: absolute;
    z-index: 1;
    inset: 0;
  }

  .page-count,
  .ai-label,
  .rank-number {
    position: absolute;
    z-index: 2;
    top: 7px;
    padding: 3px 6px;
    color: white;
    border-radius: 10px;
    background: rgba(45, 49, 53, 0.72);
    font-size: 9px;
    font-weight: 700;
  }

  .page-count { right: 7px; }
  .ai-label { left: 7px; }

  .rank-number {
    left: 7px;
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    padding: 0;
    color: #42484d;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.92);
  }

  .rank-number + .page-count { top: 41px; }
  .rank-number + .ai-label,
  .rank-number ~ .ai-label { top: 41px; }

  .bookmark {
    position: absolute;
    z-index: 2;
    right: 7px;
    bottom: 7px;
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    color: #32383d;
    border: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.9);
    cursor: pointer;
  }

  .bookmark.active { color: #ff4060; }
  .bookmark.active :global(svg) { fill: currentColor; }
  .bookmark.pending { cursor: wait; opacity: .68; }

  .reveal {
    position: absolute;
    z-index: 3;
    inset: 0;
    width: 100%;
    color: white;
    border: 0;
    background: rgba(20, 24, 28, 0.38);
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
  }

  h3 {
    overflow: hidden;
    margin: 8px 0 0;
    color: #25292c;
    font-size: 11px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  h3 a,
  p a {
    color: inherit;
    text-decoration: none;
  }

  h3 a:hover,
  p a:hover {
    color: var(--pixiv-blue);
  }

  p {
    overflow: hidden;
    margin: 5px 0 0;
    color: var(--muted);
    font-size: 9px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bookmark-error {
    display: block;
    overflow: hidden;
    margin-top: 4px;
    color: #a44f5e;
    font-size: 8px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
