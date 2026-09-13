<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import type { NovelBlock } from "$lib/novel-text";
  import {
    type ChapterMark,
    type PageBlockView,
    type PageCursor,
    type PageSlice,
    NOVEL_READER_LONG_TEXT_WEIGHT,
    NOVEL_READER_PAGINATE_YIELD_EVERY,
    NOVEL_READER_PAGINATE_YIELD_EVERY_LONG,
    NOVEL_READER_RELAYOUT_DEBOUNCE_MS,
    chapterMarksFromPages,
    chapterTitleAtPage,
    compareCursor,
    estimatePageCount,
    materializePage,
    normalizeCursor,
    pageIndexForChapter,
    paginateNextPage,
    paginateNovelProgressive,
    restorePageIndex,
    sliceContentWeight,
    startCursor,
    totalContentWeight,
  } from "$lib/novel-reader-pagination";
  import {
    PREFERENCES_CHANGED_EVENT,
    readNovelReaderPreferences,
    readVolumePageTurnEnabled,
    writeNovelReaderPreferences,
    writeVolumePageTurnEnabled,
    type NovelReaderTheme,
  } from "$lib/preferences";
  import {
    VOLUME_PAGE_EVENT,
    isVolumePageTurnSupported,
    setVolumePageTurnCapture,
    volumePageDirectionFromDetail,
    volumePageDirectionFromKey,
  } from "$lib/volume-page-turn";
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
  let volumePageTurn = $state(readVolumePageTurnEnabled());
  const volumePageTurnSupported = isVolumePageTurnSupported();
  let chromeOpen = $state(false);
  let panel = $state<null | "toc" | "settings" | "more">(null);
  let pageIndex = $state(0);
  let totalPages = $state(1);
  let pageSlices = $state<PageSlice[]>([]);
  let chapterMarks = $state<ChapterMark[]>([]);
  let viewport = $state<HTMLElement | null>(null);
  let clip = $state<HTMLElement | null>(null);
  let measureRoot = $state<HTMLElement | null>(null);
  let pointerStartX = 0;
  let pointerActive = false;
  let restored = $state(false);
  let paginationComplete = $state(false);
  let userTurned = false;
  let layoutGeneration = 0;
  let lastLayoutKey = "";
  let relayoutTimer: ReturnType<typeof setTimeout> | null = null;
  let probeStart: PageCursor | null = null;
  let probeEnd: PageCursor | null = null;

  let previous = $derived(content.seriesNavigation.previous);
  let next = $derived(content.seriesNavigation.next);
  let seriesId = $derived(content.seriesId ?? detail.novel.series?.id ?? null);
  let seriesTitle = $derived(content.seriesTitle ?? detail.novel.series?.title ?? m.novel_series_label());
  let chapters = $derived(blocks.flatMap((block, index) => block.kind === "chapter" ? [{ index, text: block.text }] : []));
  let fallbackTitle = $derived(detail.novel.title || m.common_untitled());
  let currentChapter = $derived(
    chapters.length === 0 ? fallbackTitle : chapterTitleAtPage(chapterMarks, pageIndex, chapters[0].text),
  );
  let pageLabel = $derived(m.novel_reader_page_status({ current: String(pageIndex + 1), total: String(totalPages) }));
  let visibleItems = $derived(
    pageSlices.length === 0
      ? []
      : materializePage(blocks, pageSlices[Math.min(pageIndex, pageSlices.length - 1)]),
  );

  $effect(() => {
    fontSize;
    lineHeight;
    blocks;
    clip;
    measureRoot;
    scheduleRelayout(!untrack(() => restored), untrack(() => pageSlices.length === 0));
  });

  $effect(() => {
    const el = clip;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      const key = layoutKey(el);
      if (key === lastLayoutKey) return;
      scheduleRelayout(false, false);
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

  $effect(() => {
    pageIndex;
    totalPages;
    restored;
    paginationComplete;
    if (!restored || !paginationComplete) return;
    untrack(() => {
      const maximum = Math.max(1, totalPages - 1);
      onprogress(totalPages <= 1 ? 1 : pageIndex / maximum);
    });
  });

  onMount(() => {
    const applyVolumePage = (direction: "next" | "previous") => {
      if (!volumePageTurn) return;
      turnPage(direction === "next" ? 1 : -1);
    };
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
      if (!volumePageTurnSupported) return;
      const volumeDirection = volumePageDirectionFromKey(event);
      if (volumeDirection) {
        event.preventDefault();
        applyVolumePage(volumeDirection);
      }
    };
    const onVolumePage = (event: Event) => {
      const direction = volumePageDirectionFromDetail((event as CustomEvent).detail);
      if (direction) applyVolumePage(direction);
    };
    const onPreferences = () => {
      volumePageTurn = readVolumePageTurnEnabled();
      if (volumePageTurnSupported) void setVolumePageTurnCapture(volumePageTurn);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener(PREFERENCES_CHANGED_EVENT, onPreferences);
    if (volumePageTurnSupported) {
      window.addEventListener(VOLUME_PAGE_EVENT, onVolumePage);
      void setVolumePageTurnCapture(volumePageTurn);
    }
    return () => {
      layoutGeneration += 1;
      if (relayoutTimer !== null) clearTimeout(relayoutTimer);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener(PREFERENCES_CHANGED_EVENT, onPreferences);
      if (volumePageTurnSupported) {
        window.removeEventListener(VOLUME_PAGE_EVENT, onVolumePage);
        void setVolumePageTurnCapture(false);
      }
    };
  });

  function persistVolumePageTurn() {
    writeVolumePageTurnEnabled(volumePageTurn);
    void setVolumePageTurnCapture(volumePageTurn);
  }

  function persistPrefs() {
    writeNovelReaderPreferences({ fontSize, lineHeight, theme });
  }

  function layoutKey(el: HTMLElement): string {
    return `${Math.floor(el.clientWidth)}x${Math.floor(el.clientHeight)}:${fontSize}:${lineHeight}:${blocks.length}`;
  }

  function scheduleRelayout(restore: boolean, immediate: boolean) {
    if (relayoutTimer !== null) {
      clearTimeout(relayoutTimer);
      relayoutTimer = null;
    }
    const run = () => {
      relayoutTimer = null;
      void relayout(restore);
    };
    if (immediate) {
      run();
      return;
    }
    relayoutTimer = setTimeout(run, NOVEL_READER_RELAYOUT_DEBOUNCE_MS);
  }

  async function relayout(restore: boolean) {
    if (!clip || !measureRoot) return;
    if (clip.clientWidth < 32 || clip.clientHeight < 32) return;
    const key = layoutKey(clip);
    const keepRatio = !restore && paginationComplete && totalPages > 0
      ? (totalPages <= 1 ? 0 : pageIndex / Math.max(totalPages - 1, 1))
      : null;
    const gen = ++layoutGeneration;
    lastLayoutKey = key;
    paginationComplete = false;
    probeStart = null;
    probeEnd = null;

    const overflows = (slice: PageSlice) => measureOverflow(slice);
    const origin = startCursor();
    const first = blocks.length === 0
      ? { start: origin, end: origin }
      : paginateNextPage(blocks, origin, overflows);
    if (gen !== layoutGeneration) return;
    const sample = Math.max(1, sliceContentWeight(blocks, first));
    const estimatedTotal = estimatePageCount(totalContentWeight(blocks), sample);
    if (restore) {
      pageSlices = [first];
      totalPages = Math.max(1, estimatedTotal);
      chapterMarks = chapterMarksFromPages(blocks, [first]);
      if (!userTurned) pageIndex = 0;
    }

    const weight = totalContentWeight(blocks);
    const pages = await paginateNovelProgressive(blocks, overflows, {
      initialPages: [first],
      yieldEvery: weight >= NOVEL_READER_LONG_TEXT_WEIGHT
        ? NOVEL_READER_PAGINATE_YIELD_EVERY_LONG
        : NOVEL_READER_PAGINATE_YIELD_EVERY,
      shouldAbort: () => gen !== layoutGeneration,
      yieldFn: () => new Promise((resolve) => setTimeout(resolve, 0)),
      onProgress: (next) => {
        if (gen !== layoutGeneration || !restore) return;
        pageSlices = next;
        totalPages = Math.max(estimatedTotal, next.length);
        if (!userTurned) {
          const target = restorePageIndex(initialProgress, estimatedTotal);
          if (next.length > target) pageIndex = target;
        }
      },
    });
    if (gen !== layoutGeneration || !pages) return;
    pageSlices = pages;
    totalPages = Math.max(1, pages.length);
    chapterMarks = chapterMarksFromPages(blocks, pages);
    paginationComplete = true;
    if (restore && !userTurned) pageIndex = restorePageIndex(initialProgress, totalPages);
    else if (keepRatio !== null) pageIndex = restorePageIndex(keepRatio, totalPages);
    else pageIndex = Math.min(pageIndex, Math.max(0, totalPages - 1));
    restored = true;
    probeStart = null;
    probeEnd = null;
    mountMeasure([]);
  }

  function measureOverflow(slice: PageSlice): boolean {
    if (!measureRoot) return false;
    const start = normalizeCursor(blocks, slice.start);
    const end = normalizeCursor(blocks, slice.end);
    const measuredStart = probeStart;
    const measuredEnd = probeEnd;
    const canAppend =
      measuredStart !== null
      && measuredEnd !== null
      && compareCursor(measuredStart, start) === 0
      && compareCursor(end, measuredEnd) > 0;
    if (canAppend && measuredEnd) {
      const tail = materializePage(blocks, { start: measuredEnd, end });
      for (const item of tail) measureRoot.append(createBlockElement(item));
    } else {
      mountMeasure(materializePage(blocks, { start, end }));
    }
    probeStart = start;
    probeEnd = end;
    return measureRoot.scrollHeight > measureRoot.clientHeight + 1;
  }

  function mountMeasure(items: PageBlockView[]) {
    if (!measureRoot) return;
    measureRoot.replaceChildren(...items.map((item) => createBlockElement(item)));
  }

  function createBlockElement(item: PageBlockView): HTMLElement {
    if (item.kind === "chapter") {
      const heading = document.createElement("h2");
      heading.dataset.chapter = "";
      heading.textContent = item.text;
      return heading;
    }
    if (item.kind === "page_break") return document.createElement("hr");
    if (item.kind === "artwork_link") {
      const link = document.createElement("a");
      link.className = "embed";
      link.href = `/artworks/${item.id}`;
      link.textContent = m.novel_reader_artwork_link({ id: item.id });
      return link;
    }
    if (item.kind === "uploaded_image") {
      const el = document.createElement("div");
      el.className = "embed muted";
      el.textContent = m.novel_reader_uploaded_image({ id: item.id });
      return el;
    }
    if (item.kind === "external_link") {
      const el = document.createElement("div");
      el.className = "embed external";
      el.append(item.label);
      const small = document.createElement("small");
      small.textContent = item.url;
      el.append(small);
      return el;
    }
    const paragraph = document.createElement("p");
    if (item.continuation) paragraph.className = "continue";
    paragraph.textContent = item.text;
    return paragraph;
  }

  function reachablePageCount(): number {
    return Math.max(1, pageSlices.length);
  }

  function turnPage(delta: number) {
    const nextIndex = pageIndex + delta;
    if (nextIndex < 0 || nextIndex >= reachablePageCount()) return;
    userTurned = true;
    pageIndex = nextIndex;
    chromeOpen = false;
    panel = null;
  }

  function goToPage(index: number, keepChrome = false) {
    userTurned = true;
    pageIndex = Math.min(reachablePageCount() - 1, Math.max(0, index));
    if (!keepChrome) {
      chromeOpen = false;
      panel = null;
    }
  }

  function goToChapter(blockIndex: number) {
    const page = pageIndexForChapter(blocks, pageSlices, blockIndex);
    if (page < 0) return;
    goToPage(page);
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
    <div class="page-clip" bind:this={clip} style={`--reader-font:${fontSize}px;--reader-line:${lineHeight}`}>
      <article class="paged">
        {#each visibleItems as item (item.key)}
          {#if item.kind === "chapter"}
            <h2 data-chapter>{item.text}</h2>
          {:else if item.kind === "page_break"}
            <hr />
          {:else if item.kind === "artwork_link"}
            <a class="embed" href={`/artworks/${item.id}`}>{m.novel_reader_artwork_link({ id: item.id })}</a>
          {:else if item.kind === "uploaded_image"}
            <div class="embed muted">{m.novel_reader_uploaded_image({ id: item.id })}</div>
          {:else if item.kind === "external_link"}
            <div class="embed external">{item.label}<small>{item.url}</small></div>
          {:else}
            <p class:continue={item.continuation}>{item.text}</p>
          {/if}
        {/each}
      </article>
      <div class="paged measure" bind:this={measureRoot} aria-hidden="true"></div>
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
          max={Math.max(0, reachablePageCount() - 1)}
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
          <button type="button" onclick={() => goToChapter(chapter.index)}>{chapter.text}</button>
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
      {#if volumePageTurnSupported}
        <label class="toggle">
          <span>{m.settings_volume_page_turn()}</span>
          <input type="checkbox" role="switch" bind:checked={volumePageTurn} onchange={persistVolumePageTurn} />
        </label>
      {/if}
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
    --reader-progress-reserve: calc(var(--type-caption) * 2 + 14px);
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
    padding: calc(42px + env(safe-area-inset-top, 0px)) 22px calc(var(--reader-progress-reserve) + env(safe-area-inset-bottom, 0px));
    overflow: hidden;
    touch-action: none;
  }
  .page-clip {
    position: relative;
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
    overflow: hidden;
    padding-bottom: var(--reader-progress-reserve);
  }
  .paged.measure {
    position: absolute;
    inset: 0;
    visibility: hidden;
    pointer-events: none;
  }
  .paged p {
    margin: 0 0 1.15em;
    font-size: var(--reader-font);
    line-height: var(--reader-line);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .paged p.continue { margin-top: 0; }
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
  .sheet label.toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .sheet label.toggle input { width: 20px; height: 20px; accent-color: var(--pixiv-blue); }
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
