<script lang="ts">
  import PixivImage from "$lib/components/PixivImage.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { describeDataFailure, setNovelBookmark } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { r18DefaultVisible } from "$lib/preferences";
  import type { NovelSummary } from "$lib/types";

  let { novel }: { novel: NovelSummary } = $props();
  let caption = $derived(plainPixivText(novel.caption));
  let bookmarked = $state(false);
  let bookmarkPending = $state(false);
  let bookmarkError = $state("");
  let revealRestricted = $state(false);
  let restricted = $derived(novel.xRestrict > 0);

  $effect(() => {
    bookmarked = novel.isBookmarked;
    bookmarkError = "";
  });

  function compact(value: number): string {
    return new Intl.NumberFormat("zh-CN", { notation: "compact", maximumFractionDigits: 1 }).format(value);
  }

  async function toggleBookmark() {
    if (bookmarkPending) return;
    const previous = bookmarked;
    bookmarked = !previous;
    bookmarkPending = true;
    bookmarkError = "";
    try {
      await setNovelBookmark(novel.id, bookmarked);
    } catch (error) {
      bookmarked = previous;
      bookmarkError = describeDataFailure(error);
    } finally {
      bookmarkPending = false;
    }
  }
</script>

<article class="novel-card">
  <div class="cover" class:concealed={restricted && !$r18DefaultVisible && !revealRestricted}>
    <a class="cover-link" href={`/novels/${novel.id}`} aria-label={`阅读小说：${novel.title || "无题"}`}></a>
    <PixivImage url={novel.coverUrl} alt="" />
    <span>{novel.pageCount} 页</span>
    {#if restricted && !$r18DefaultVisible && !revealRestricted}
      <button class="reveal" type="button" onclick={() => (revealRestricted = true)}>R-18 · 点击显示</button>
    {/if}
  </div>
  <div class="copy">
    <div class="badges">
      {#if novel.series}<span>系列</span>{/if}
      {#if novel.aiType === 2}<span>AI</span>{/if}
      {#if novel.xRestrict > 0}<span>R-18</span>{/if}
      <button type="button" class:active={bookmarked} disabled={bookmarkPending} title={bookmarkError || (bookmarked ? "取消收藏" : "收藏小说")} aria-label={bookmarked ? "取消收藏" : "收藏小说"} onclick={toggleBookmark}><Icon name="heart" size={16} /></button>
    </div>
    <h2><a href={`/novels/${novel.id}`}>{novel.title || "无题"}</a></h2>
    {#if caption}<p>{caption}</p>{/if}
    <a class="author" href={`/users/${novel.author.id}`}>{novel.author.name || novel.author.account}</a>
    <div class="meta"><span>{compact(novel.textLength)} 字</span><span>♥ {compact(novel.totalBookmarks)}</span><span>👁 {compact(novel.totalViews)}</span></div>
    <div class="tags">{#each novel.tags.slice(0, 4) as tag}<span>#{tag}</span>{/each}</div>
  </div>
</article>

<style>
  .novel-card { display: grid; grid-template-columns: 118px minmax(0,1fr); min-width: 0; overflow: hidden; border: 1px solid var(--line); border-radius: 11px; background: white; }
  .cover { position: relative; min-height: 166px; overflow: hidden; background: #edf1f4; }
  .cover.concealed :global(img) { filter: blur(18px) brightness(.7); transform: scale(1.12); }
  .cover-link { position: absolute; z-index: 1; inset: 0; }
  .cover :global(img) { position: absolute; inset: 0; }
  .cover > span { position: absolute; z-index: 2; right: 7px; bottom: 7px; padding: 4px 7px; color: white; border-radius: 10px; background: rgba(30,34,38,.7); font-size: 8px; }
  .reveal { position: absolute; z-index: 3; inset: 0; width: 100%; color: white; border: 0; background: rgba(20,24,28,.38); cursor: pointer; font-size: 9px; font-weight: 700; }
  .copy { min-width: 0; padding: 14px; }
  .badges { display: flex; min-height: 22px; gap: 5px; align-items: center; }
  .badges span { padding: 3px 6px; color: #55778b; border-radius: 3px; background: #eef7fc; font-size: 7px; font-weight: 700; }
  .badges button { display: grid; width: 25px; height: 25px; margin-left: auto; place-items: center; color: #778087; border: 0; border-radius: 50%; background: #f2f4f5; cursor: pointer; } .badges button.active { color: #ff4060; } .badges button.active :global(svg) { fill: currentColor; } .badges button:disabled { cursor: wait; opacity: .6; }
  h2 { overflow: hidden; margin: 9px 0 0; font-size: 14px; text-overflow: ellipsis; white-space: nowrap; }
  h2 a, .author { color: inherit; text-decoration: none; }
  h2 a:hover, .author:hover { color: var(--pixiv-blue); }
  p { display: -webkit-box; overflow: hidden; margin: 8px 0; color: var(--muted); font-size: 9px; line-height: 1.55; line-clamp: 2; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
  .author { display: block; overflow: hidden; color: #5e666b; font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .meta, .tags { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 9px; color: var(--soft-muted); font-size: 8px; }
  .tags { gap: 5px; color: #648398; }
  @media (max-width: 520px) { .novel-card { grid-template-columns: 92px minmax(0,1fr); } .cover { min-height: 142px; } .copy { padding: 11px; } }
</style>
