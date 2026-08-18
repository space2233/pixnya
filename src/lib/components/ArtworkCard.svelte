<script lang="ts">
  import ArtworkThumbnail from "$lib/components/ArtworkThumbnail.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import {
    publishIllustrationBookmarkState,
    resolveIllustrationBookmarkState,
    subscribeIllustrationBookmarkState,
  } from "$lib/illustration-bookmark-state";
  import { m } from "$lib/i18n";
  import { describeDataFailure, setIllustrationBookmark } from "$lib/pixiv-api";
  import { r18DefaultVisible } from "$lib/preferences";
  import { session } from "$lib/session";
  import type { IllustrationSummary } from "$lib/types";

  let {
    illustration,
    tone = 1,
    rank,
    selectable = false,
    selected = false,
    onSelect,
  }: {
    illustration: IllustrationSummary;
    tone?: number;
    rank?: number;
    selectable?: boolean;
    selected?: boolean;
    onSelect?: (selected: boolean) => void;
  } = $props();

  let revealRestricted = $state(false);
  let restricted = $derived(illustration.xRestrict > 0);
  let bookmarked = $state(false);
  let bookmarkPending = $state(false);
  let bookmarkError = $state("");
  let bookmarkAccount = $derived($session.loggedIn ? ($session.user?.id ?? "logged-in") : "");

  $effect(() => {
    const account = bookmarkAccount;
    const illustrationId = illustration.id;
    bookmarked = resolveIllustrationBookmarkState(
      account,
      illustrationId,
      illustration.isBookmarked,
    );
    bookmarkError = "";
    return subscribeIllustrationBookmarkState(account, illustrationId, (next) => {
      bookmarked = next;
      bookmarkError = "";
    });
  });

  async function toggleBookmark(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (bookmarkPending) return;
    const previous = bookmarked;
    const next = !previous;
    const account = bookmarkAccount;
    bookmarked = next;
    bookmarkPending = true;
    bookmarkError = "";
    try {
      await setIllustrationBookmark(illustration.id, next);
      publishIllustrationBookmarkState(account, illustration.id, next);
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
      aria-label={m.artwork_view({ title: illustration.title || m.common_untitled() })}
    ></a>
    <ArtworkThumbnail
      url={illustration.thumbnailUrl}
      alt={restricted && !$r18DefaultVisible && !revealRestricted ? m.restricted_thumbnail_hidden() : illustration.title || m.artwork_untitled()}
      {tone}
    />
    {#if rank !== undefined}<span class="rank-number">{rank}</span>{/if}
    {#if illustration.pageCount > 1}
      <span class="page-count">▣ {illustration.pageCount}</span>
    {/if}
    {#if illustration.aiType === 2}<span class="ai-label">AI</span>{/if}
    {#if selectable}
      <button
        type="button"
        class="select-toggle"
        class:selected
        aria-pressed={selected}
        aria-label={m.bookmark_select_work({ title: illustration.title || m.common_untitled() })}
        onclick={(event) => { event.preventDefault(); event.stopPropagation(); onSelect?.(!selected); }}
      >{selected ? "✓" : ""}</button>
    {/if}
    <button
      type="button"
      class="bookmark"
      class:active={bookmarked}
      class:pending={bookmarkPending}
      disabled={bookmarkPending}
      aria-label={bookmarked ? m.bookmark_remove() : m.artwork_bookmark()}
      title={bookmarkError || (bookmarked ? m.bookmark_remove() : m.artwork_bookmark())}
      onclick={toggleBookmark}
    >
      <Icon name="heart" size={19} />
    </button>
    {#if restricted && !$r18DefaultVisible && !revealRestricted}
      <button type="button" class="reveal" onclick={() => (revealRestricted = true)}>
        {illustration.xRestrict >= 2 ? "R-18G" : "R-18"} · {m.restricted_reveal()}
      </button>
    {/if}
  </div>
  <h3 title={illustration.title}>
    <a href={`/artworks/${illustration.id}`}>{illustration.title || m.common_untitled()}</a>
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
    font-size: var(--type-caption);
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

  .select-toggle { position:absolute;z-index:4;top:8px;right:8px;width:28px;height:28px;border:2px solid white;border-radius:50%;background:#20283299;color:white;font-weight:800;box-shadow:0 1px 5px #0004;cursor:pointer }
  .select-toggle.selected { border-color:var(--pixiv-blue);background:var(--pixiv-blue) }
  .select-toggle + .bookmark { display:none }

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
    font-size: var(--type-body);
    font-weight: 700;
  }

  h3 {
    overflow: hidden;
    margin: 8px 0 0;
    color: #25292c;
    font-size: var(--type-small);
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
    font-size: var(--type-caption);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bookmark-error {
    display: block;
    overflow: hidden;
    margin-top: 4px;
    color: #a44f5e;
    font-size: var(--type-caption);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
