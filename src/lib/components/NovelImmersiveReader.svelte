<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import type { NovelBlock } from "$lib/novel-text";
  import {
    readNovelReaderPreferences,
    readReducedMotion,
    writeNovelReaderPreferences,
    type NovelReaderTheme,
  } from "$lib/preferences";
  import type { NovelContent, NovelDetail } from "$lib/types";

  let {
    detail,
    content,
    blocks,
    fallback,
    initialProgress = 0,
    onprogress,
  }: {
    detail: NovelDetail;
    content: NovelContent;
    blocks: NovelBlock[];
    fallback: string;
    initialProgress?: number;
    onprogress: (value: number) => void;
  } = $props();

  const stored = readNovelReaderPreferences();
  let fontSize = $state(stored.fontSize);
  let lineHeight = $state(stored.lineHeight);
  let theme = $state<NovelReaderTheme>(stored.theme);
  let chromeOpen = $state(false);
  let panel = $state<null | "toc" | "settings" | "more">(null);
  let pageIndex = $state(0);
  let totalPages = $state(1);
  let viewport = $state<HTMLElement | null>(null);
  let clip = $state<HTMLElement | null>(null);
  let article = $state<HTMLElement | null>(null);
  let pointerStartX = 0;
  let pointerActive = false;
  let restored = $state(false);

  let previous = $derived(content.seriesNavigation.previous);
  let next = $derived(content.seriesNavigation.next);
  let seriesId = $derived(content.seriesId ?? detail.novel.series?.id ?? null);
  let seriesTitle = $derived(content.seriesTitle ?? detail.novel.series?.title ?? m.novel_series_label());
  let chapters = $derived(blocks.flatMap((block, index) => block.kind === "chapter" ? [{ index, text: block.text }] : []));
  let currentChapter = $derived.by(() => {
    if (chapters.length === 0) return detail.novel.title || m.common_untitled();
    let title = chapters[0].text;
    for (const chapter of chapters) {
      if (pageOfChapter(chapter.index) <= pageIndex) title = chapter.text;
    }
    return title;
  });
  let pageLabel = $derived(m.novel_reader_page_status({ current: String(pageIndex + 1), total: String(totalPages) }));

  $effect(() => {
    fontSize;
    lineHeight;
    theme;
    blocks;
    viewport;
    clip;
    article;
    void tick().then(() => layoutPages(!untrack(() => restored)));
  });

  $effect(() => {
    const el = clip ?? viewport;
    if (!el) return;
    const observer = new ResizeObserver(() => layoutPages(false));
    observer.observe(el);
    return () => observer.disconnect();
  });

  $effect(() => {
    pageIndex;
    totalPages;
    restored;
    if (!restored) return;
    untrack(() => {
      applyTransform();
      const maximum = Math.max(1, totalPages - 1);
      onprogress(totalPages <= 1 ? 1 : pageIndex / maximum);
    });
  });

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        chromeOpen = false;
        panel = null;
        return;
      }
      if (event.target instanceof HTMLElement && ["INPUT", "SELECT", "TEXTAREA"].includes(event.target.tagName)) return;
      if (event.key === "ArrowLeft" || event.key === "PageUp") {
        event.preventDefault();
        turnPage(-1);
      }
      if (event.key === "ArrowRight" || event.key === "PageDown" || event.key === " ") {
        event.preventDefault();
        turnPage(1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function persistPrefs() {
    writeNovelReaderPreferences({ fontSize, lineHeight, theme });
  }

  function pageWidth(): number {
    const el = clip ?? viewport;
    if (!el) return 1;
    return Math.max(1, Math.floor(el.clientWidth));
  }

  function layoutPages(restore: boolean) {
    if (!article || !clip) return;
    const width = pageWidth();
    article.style.width = `${width}px`;
    article.style.maxWidth = `${width}px`;
    article.style.columnWidth = `${width}px`;
    article.style.setProperty("-webkit-column-width", `${width}px`);
    article.style.columnGap = "0px";
    article.style.setProperty("-webkit-column-gap", "0px");
    const measured = Math.max(1, Math.round(article.scrollWidth / width));
    totalPages = measured;
    if (restore) {
      const ratio = Number.isFinite(initialProgress) ? Math.min(1, Math.max(0, initialProgress)) : 0;
      pageIndex = Math.min(measured - 1, Math.round(ratio * Math.max(measured - 1, 1)));
      restored = true;
    } else {
      pageIndex = Math.min(pageIndex, measured - 1);
    }
    applyTransform();
  }

  function applyTransform() {
    if (!article) return;
    const width = pageWidth();
    const reduced = readReducedMotion();
    article.style.transition = reduced ? "none" : "transform 180ms ease";
    article.style.transform = `translate3d(-${pageIndex * width}px, 0, 0)`;
  }

  function turnPage(delta: number) {
    const nextIndex = pageIndex + delta;
    if (nextIndex < 0 || nextIndex >= totalPages) return;
    pageIndex = nextIndex;
    chromeOpen = false;
    panel = null;
  }

  function goToPage(index: number, keepChrome = false) {
    pageIndex = Math.min(totalPages - 1, Math.max(0, index));
    if (!keepChrome) {
      chromeOpen = false;
      panel = null;
    }
  }

  function pageOfChapter(blockIndex: number): number {
    if (!article) return 0;
    const heading = article.querySelector(`[data-block="${blockIndex}"]`);
    if (!(heading instanceof HTMLElement)) return 0;
    const width = pageWidth();
    return Math.min(totalPages - 1, Math.max(0, Math.floor(heading.offsetLeft / width)));
  }

  function toggleChrome() {
    chromeOpen = !chromeOpen;
    if (!chromeOpen) panel = null;
  }

  function openPanel(nextPanel: "toc" | "settings" | "more") {
    chromeOpen = true;
    panel = panel === nextPanel ? null : nextPanel;
  }

  function toggleNight() {
    theme = theme === "dark" ? "paper" : "dark";
    persistPrefs();
  }

  function isChromeTarget(target: EventTarget | null): boolean {
    return target instanceof Element && Boolean(target.closest("[data-chrome], a, button, input, label"));
  }

  function onPointerDown(event: PointerEvent) {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    if (isChromeTarget(event.target)) return;
    pointerActive = true;
    pointerStartX = event.clientX;
  }

  function onPointerUp(event: PointerEvent) {
    if (!pointerActive) return;
    pointerActive = false;
    if (isChromeTarget(event.target)) return;
    const dx = event.clientX - pointerStartX;
    if (Math.abs(dx) > 48) {
      turnPage(dx < 0 ? 1 : -1);
      return;
    }
    if (chromeOpen) {
      chromeOpen = false;
      panel = null;
      return;
    }
    const ratio = event.clientX / Math.max(1, window.innerWidth);
    if (ratio < 0.22) turnPage(-1);
    else if (ratio > 0.78) turnPage(1);
    else toggleChrome();
  }
</script>

<div
  class="reader"
  class:theme-paper={theme === "paper"}
  class:theme-white={theme === "white"}
  class:theme-dark={theme === "dark"}
  class:chrome-open={chromeOpen}
>
  {#if !chromeOpen}
    <p class="ghost-chapter">{currentChapter}</p>
  {/if}

  <div
    class="viewport"
    role="region"
    aria-label={m.novel_reader_title()}
    bind:this={viewport}
    onpointerdown={onPointerDown}
    onpointerup={onPointerUp}
    onpointercancel={() => (pointerActive = false)}
  >
    <div class="page-clip" bind:this={clip}>
      <article
        class="paged"
        bind:this={article}
        style={`--reader-font:${fontSize}px;--reader-line:${lineHeight}`}
      >
        {#each blocks as block, index (index)}
          {#if block.kind === "chapter"}
            <h2 data-chapter data-block={index}>{block.text}</h2>
          {:else if block.kind === "page_break"}
            <hr />
          {:else if block.kind === "artwork_link"}
            <a class="embed" href={`/artworks/${block.id}`}>{m.novel_reader_artwork_link({ id: block.id })}</a>
          {:else if block.kind === "uploaded_image"}
            <div class="embed muted">{m.novel_reader_uploaded_image({ id: block.id })}</div>
          {:else if block.kind === "external_link"}
            <div class="embed external">{block.label}<small>{block.url}</small></div>
          {:else}
            <p>{block.text}</p>
          {/if}
        {/each}
      </article>
    </div>
  </div>

  {#if !chromeOpen}
    <p class="ghost-progress">{pageLabel}</p>
  {/if}

  {#if chromeOpen}
    <header class="chrome-top" data-chrome>
      <ReturnLink compact {fallback} label={m.novel_reader_back()} />
      <div class="chrome-title">
        <strong>{detail.novel.title || m.common_untitled()}</strong>
        <span>{detail.novel.author.name}</span>
      </div>
    </header>

    <footer class="chrome-bottom" data-chrome>
      <div class="chapter-row">
        {#if previous?.viewable}
          <a href={`/novels/${previous.id}/read`}>{m.novel_reader_previous_chapter()}</a>
        {:else}
          <button type="button" disabled>{m.novel_reader_previous_chapter()}</button>
        {/if}
        <input
          type="range"
          min="0"
          max={Math.max(0, totalPages - 1)}
          value={pageIndex}
          aria-label={pageLabel}
          oninput={(event) => goToPage(Number((event.currentTarget as HTMLInputElement).value), true)}
        />
        {#if next?.viewable}
          <a href={`/novels/${next.id}/read`}>{m.novel_reader_next_chapter()}</a>
        {:else}
          <button type="button" disabled>{m.novel_reader_next_chapter()}</button>
        {/if}
      </div>
      <nav class="tools" aria-label={m.novel_reader_title()}>
        <button type="button" class:active={panel === "toc"} onclick={() => openPanel("toc")}>
          <Icon name="list" size={20} /><span>{m.novel_reader_contents()}</span>
        </button>
        <button type="button" class:active={theme === "dark"} onclick={toggleNight}>
          <Icon name="moon" size={20} /><span>{m.novel_reader_night()}</span>
        </button>
        <button type="button" class:active={panel === "settings"} onclick={() => openPanel("settings")}>
          <Icon name="settings" size={20} /><span>{m.settings_title()}</span>
        </button>
        <button type="button" class:active={panel === "more"} onclick={() => openPanel("more")}>
          <Icon name="dots" size={20} /><span>{m.novel_reader_more()}</span>
        </button>
      </nav>
    </footer>
  {/if}

  {#if panel === "toc"}
    <section class="sheet" data-chrome>
      <h2>{m.novel_reader_contents_title()}</h2>
      {#if chapters.length === 0}
        <p>{m.novel_reader_no_chapters()}</p>
      {:else}
        {#each chapters as chapter}
          <button type="button" onclick={() => goToPage(pageOfChapter(chapter.index))}>{chapter.text}</button>
        {/each}
      {/if}
    </section>
  {:else if panel === "settings"}
    <section class="sheet" data-chrome>
      <label>{m.novel_reader_font_size()} <input type="range" min="14" max="28" step="1" bind:value={fontSize} onchange={persistPrefs} /></label>
      <label>{m.novel_reader_line_height()} <input type="range" min="1.4" max="2.4" step="0.1" bind:value={lineHeight} onchange={persistPrefs} /></label>
      <div class="themes">
        <span>{m.novel_reader_background()}</span>
        <button type="button" class:active={theme === "paper"} onclick={() => { theme = "paper"; persistPrefs(); }}>{m.novel_reader_theme_paper()}</button>
        <button type="button" class:active={theme === "white"} onclick={() => { theme = "white"; persistPrefs(); }}>{m.novel_reader_theme_white()}</button>
        <button type="button" class:active={theme === "dark"} onclick={() => { theme = "dark"; persistPrefs(); }}>{m.novel_reader_theme_dark()}</button>
      </div>
    </section>
  {:else if panel === "more"}
    <section class="sheet" data-chrome>
      <a href={fallback}>{m.novel_reader_open_detail()}</a>
      {#if seriesId}
        <a href={`/series/novels/${seriesId}`}>{m.novel_reader_open_series()}: {seriesTitle}</a>
      {/if}
    </section>
  {/if}
</div>

<style>
  .reader {
    position: relative;
    display: flex;
    height: 100dvh;
    min-height: 100dvh;
    overflow: hidden;
    flex-direction: column;
    color: #33302a;
    background: #fbf7ec;
    user-select: text;
    -webkit-user-select: text;
  }
  .reader.theme-white { color: #282b2e; background: #fff; }
  .reader.theme-dark { color: #d8d5cf; background: #202326; }
  .ghost-chapter, .ghost-progress {
    position: absolute;
    z-index: 2;
    right: 22px;
    left: 22px;
    margin: 0;
    color: color-mix(in srgb, currentColor 38%, transparent);
    font-size: var(--type-caption);
    text-align: center;
    pointer-events: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ghost-chapter { top: calc(10px + env(safe-area-inset-top, 0px)); }
  .ghost-progress { bottom: calc(10px + env(safe-area-inset-bottom, 0px)); }
  .viewport {
    flex: 1;
    min-width: 0;
    min-height: 0;
    padding: calc(42px + env(safe-area-inset-top, 0px)) 22px calc(36px + env(safe-area-inset-bottom, 0px));
    overflow: hidden;
    touch-action: none;
  }
  .page-clip {
    height: 100%;
    width: 100%;
    min-width: 0;
    overflow: hidden;
    contain: paint;
    isolation: isolate;
  }
  .paged {
    height: 100%;
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
    column-fill: auto;
    column-gap: 0;
    column-count: auto;
  }
  .paged p {
    margin: 0 0 1.15em;
    font-size: var(--reader-font);
    line-height: var(--reader-line);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .paged h2 {
    margin: 1.4em 0 1em;
    font-size: calc(var(--reader-font) * 1.3);
    text-align: center;
    font-weight: 700;
  }
  .paged hr {
    width: 44%;
    margin: 2.4em auto;
    border: 0;
    border-top: 1px solid currentColor;
    opacity: .25;
  }
  .embed {
    display: block;
    margin: 1.5em 0;
    padding: 14px;
    color: var(--pixiv-blue);
    border: 1px solid currentColor;
    border-radius: 7px;
    font-size: calc(var(--reader-font) * .72);
    text-align: center;
    text-decoration: none;
    break-inside: avoid;
  }
  .embed.muted { color: #8a8e91; border-style: dashed; }
  .embed.external small { display: block; margin-top: 5px; color: var(--muted); font-size: .72em; overflow-wrap: anywhere; }
  .chrome-top, .chrome-bottom, .sheet {
    position: absolute;
    z-index: 6;
    right: 0;
    left: 0;
    color: #ececec;
    background: #2b2b2b;
  }
  .chrome-top {
    top: 0;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: calc(8px + env(safe-area-inset-top, 0px)) 10px 12px;
  }
  .chrome-title { min-width: 0; }
  .chrome-title strong, .chrome-title span {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chrome-title strong { font-size: var(--type-body); font-weight: 650; }
  .chrome-title span { margin-top: 2px; color: #b0b0b0; font-size: var(--type-caption); }
  .chrome-bottom { bottom: 0; padding: 10px 16px calc(10px + env(safe-area-inset-bottom, 0px)); }
  .chapter-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
  }
  .chapter-row a, .chapter-row button {
    padding: 0;
    color: #f3f3f3;
    border: 0;
    background: transparent;
    font-size: var(--type-body);
    font-weight: 700;
    text-decoration: none;
    white-space: nowrap;
  }
  .chapter-row button:disabled, .chapter-row a { cursor: pointer; }
  .chapter-row button:disabled { color: #777; cursor: default; }
  .chapter-row input {
    width: 100%;
    accent-color: var(--pixiv-blue);
  }
  .tools {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    margin-top: 8px;
  }
  .tools button {
    display: grid;
    gap: 4px;
    place-items: center;
    padding: 8px 0 4px;
    color: #d8d8d8;
    border: 0;
    background: transparent;
    font-size: var(--type-body);
  }
  .tools button.active { color: #fff; }
  .sheet {
    bottom: 0;
    display: grid;
    gap: 10px;
    max-height: 48%;
    overflow: auto;
    padding: 18px 18px calc(92px + env(safe-area-inset-bottom, 0px));
    border-radius: 16px 16px 0 0;
  }
  .sheet h2 { margin: 0; font-size: var(--type-body); }
  .sheet p, .sheet a, .sheet label {
    color: #e8e8e8;
    font-size: var(--type-small);
    text-decoration: none;
  }
  .sheet > button, .sheet a {
    display: block;
    padding: 10px 0;
    color: #e8e8e8;
    border: 0;
    border-bottom: 1px solid #3a3a3a;
    background: transparent;
    font-size: var(--type-body);
    text-align: left;
    text-decoration: none;
  }
  .sheet label { display: grid; gap: 6px; }
  .themes { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .themes button {
    padding: 6px 12px;
    color: #ddd;
    border: 1px solid #555;
    border-radius: 999px;
    background: transparent;
    font-size: var(--type-body);
  }
  .themes button.active { color: #fff; border-color: var(--pixiv-blue); background: #1d4f72; }
</style>
