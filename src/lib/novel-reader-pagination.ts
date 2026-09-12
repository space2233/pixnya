import type { NovelBlock } from "./novel-text.ts";

export const NOVEL_READER_RELAYOUT_DEBOUNCE_MS = 80;
export const NOVEL_READER_PAGINATE_YIELD_EVERY = 6;
export const NOVEL_READER_PAGINATE_YIELD_EVERY_LONG = 1;
export const NOVEL_READER_LONG_TEXT_WEIGHT = 50_000;

export type PageCursor = {
  index: number;
  offset: number;
};

export type PageSlice = {
  start: PageCursor;
  end: PageCursor;
};

export type ChapterMark = {
  page: number;
  text: string;
};

export type PageBlockView =
  | { key: string; kind: "paragraph"; text: string; continuation: boolean }
  | { key: string; kind: "chapter"; text: string }
  | { key: string; kind: "page_break" }
  | { key: string; kind: "artwork_link"; id: string }
  | { key: string; kind: "uploaded_image"; id: string }
  | { key: string; kind: "external_link"; label: string; url: string };

export type PageOverflowFn = (slice: PageSlice) => boolean;

export function startCursor(): PageCursor {
  return { index: 0, offset: 0 };
}

export function endCursor(blocks: readonly NovelBlock[]): PageCursor {
  return { index: blocks.length, offset: 0 };
}

export function compareCursor(left: PageCursor, right: PageCursor): number {
  if (left.index !== right.index) return left.index < right.index ? -1 : 1;
  if (left.offset !== right.offset) return left.offset < right.offset ? -1 : 1;
  return 0;
}

export function normalizeCursor(blocks: readonly NovelBlock[], cursor: PageCursor): PageCursor {
  let index = Math.max(0, cursor.index);
  let offset = Math.max(0, cursor.offset);
  while (index < blocks.length) {
    const block = blocks[index];
    if (block.kind === "paragraph") {
      if (offset < block.text.length) return { index, offset };
      index += 1;
      offset = 0;
      continue;
    }
    if (offset > 0) {
      index += 1;
      offset = 0;
      continue;
    }
    return { index, offset: 0 };
  }
  return { index: blocks.length, offset: 0 };
}

export function isEndCursor(blocks: readonly NovelBlock[], cursor: PageCursor): boolean {
  return normalizeCursor(blocks, cursor).index >= blocks.length;
}

export function restorePageIndex(progress: number, totalPages: number): number {
  const measured = Math.max(1, totalPages);
  const ratio = Number.isFinite(progress) ? Math.min(1, Math.max(0, progress)) : 0;
  return Math.min(measured - 1, Math.round(ratio * Math.max(measured - 1, 1)));
}

export function blockWeight(block: NovelBlock): number {
  if (block.kind === "paragraph" || block.kind === "chapter") return block.text.length;
  return 1;
}

export function totalContentWeight(blocks: readonly NovelBlock[]): number {
  let weight = 0;
  for (const block of blocks) weight += blockWeight(block);
  return weight;
}

export function sliceContentWeight(blocks: readonly NovelBlock[], slice: PageSlice): number {
  let weight = 0;
  for (const view of materializePage(blocks, slice)) {
    weight += view.kind === "paragraph" || view.kind === "chapter" ? view.text.length : 1;
  }
  return weight;
}

export function estimatePageCount(totalWeight: number, firstPageWeight: number): number {
  if (firstPageWeight <= 0) return 1;
  return Math.max(1, Math.round(totalWeight / firstPageWeight));
}

export function cursorAtRatio(blocks: readonly NovelBlock[], ratio: number): PageCursor {
  const total = totalContentWeight(blocks);
  if (total <= 0) return startCursor();
  const clamped = Number.isFinite(ratio) ? Math.min(1, Math.max(0, ratio)) : 0;
  if (clamped <= 0) return startCursor();
  const target = Math.min(total - 1, Math.floor(clamped * total));
  let seen = 0;
  for (let index = 0; index < blocks.length; index += 1) {
    const weight = blockWeight(blocks[index]);
    if (seen + weight > target) {
      if (blocks[index].kind === "paragraph") {
        return normalizeCursor(blocks, { index, offset: target - seen });
      }
      return { index, offset: 0 };
    }
    seen += weight;
  }
  return startCursor();
}

export function pageIndexForCursor(pages: readonly PageSlice[], cursor: PageCursor): number {
  let found = 0;
  for (let page = 0; page < pages.length; page += 1) {
    if (compareCursor(cursor, pages[page].start) >= 0 && compareCursor(cursor, pages[page].end) < 0) {
      return page;
    }
    if (compareCursor(pages[page].start, cursor) <= 0) found = page;
  }
  return found;
}

export function chapterTitleAtPage(marks: readonly ChapterMark[], page: number, fallback: string): string {
  let title = fallback;
  for (const mark of marks) {
    if (mark.page <= page) title = mark.text;
    else break;
  }
  return title;
}

export function pageIndexForChapter(
  blocks: readonly NovelBlock[],
  pages: readonly PageSlice[],
  blockIndex: number,
): number {
  for (let page = 0; page < pages.length; page += 1) {
    if (sliceContainsBlock(blocks, pages[page], blockIndex)) return page;
  }
  return -1;
}

export function materializePage(blocks: readonly NovelBlock[], slice: PageSlice): PageBlockView[] {
  const start = normalizeCursor(blocks, slice.start);
  const end = normalizeCursor(blocks, slice.end);
  const views: PageBlockView[] = [];
  if (compareCursor(start, end) >= 0) return views;

  for (let index = start.index; index < end.index; index += 1) {
    const from = index === start.index ? start.offset : 0;
    const view = viewForBlock(blocks[index], index, from, blockLimit(blocks[index]));
    if (view) views.push(view);
  }
  if (end.offset > 0 && end.index < blocks.length) {
    const from = end.index === start.index ? start.offset : 0;
    const view = viewForBlock(blocks[end.index], end.index, from, end.offset);
    if (view) views.push(view);
  }
  return views;
}

export function chapterMarksFromPages(blocks: readonly NovelBlock[], pages: readonly PageSlice[]): ChapterMark[] {
  const marks: ChapterMark[] = [];
  for (let page = 0; page < pages.length; page += 1) {
    for (const view of materializePage(blocks, pages[page])) {
      if (view.kind === "chapter") marks.push({ page, text: view.text });
    }
  }
  return marks;
}

export function paginateNextPage(
  blocks: readonly NovelBlock[],
  start: PageCursor,
  overflows: PageOverflowFn,
): PageSlice {
  const from = normalizeCursor(blocks, start);
  if (from.index >= blocks.length) return { start: from, end: from };

  let lastFit: PageCursor | null = null;
  let candidate = nextWholeEnd(from);
  while (compareCursor(candidate, from) > 0) {
    if (!overflows({ start: from, end: candidate })) {
      lastFit = candidate;
      if (candidate.index >= blocks.length) break;
      candidate = nextWholeEnd(candidate);
      continue;
    }
    if (!lastFit) {
      return { start: from, end: splitOverflowingBlock(blocks, from, from.index, from.offset, overflows) };
    }
    const overflowIndex = lastFit.index;
    if (overflowIndex >= blocks.length) return { start: from, end: lastFit };
    return {
      start: from,
      end: splitOverflowingBlock(blocks, from, overflowIndex, overflowIndex === from.index ? from.offset : 0, overflows, lastFit),
    };
  }

  return { start: from, end: lastFit ?? forceAdvance(blocks, from) };
}

export function* iterateNovelPages(
  blocks: readonly NovelBlock[],
  overflows: PageOverflowFn,
  from: PageCursor = startCursor(),
): Generator<PageSlice> {
  if (blocks.length === 0) {
    yield { start: startCursor(), end: startCursor() };
    return;
  }
  let cursor = normalizeCursor(blocks, from);
  const safety = unitCount(blocks) + 2;
  let count = 0;
  while (!isEndCursor(blocks, cursor)) {
    const page = paginateNextPage(blocks, cursor, overflows);
    const end = normalizeCursor(blocks, page.end);
    const slice =
      compareCursor(end, cursor) <= 0
        ? { start: cursor, end: forceAdvance(blocks, cursor) }
        : { start: page.start, end };
    yield slice;
    cursor = slice.end;
    count += 1;
    if (count > safety) return;
  }
}

export function paginateNovel(blocks: readonly NovelBlock[], overflows: PageOverflowFn): PageSlice[] {
  const pages = [...iterateNovelPages(blocks, overflows)];
  return pages.length > 0 ? pages : [{ start: startCursor(), end: startCursor() }];
}

export async function paginateNovelProgressive(
  blocks: readonly NovelBlock[],
  overflows: PageOverflowFn,
  options: {
    yieldEvery?: number;
    shouldAbort?: () => boolean;
    onProgress?: (pages: PageSlice[]) => void;
    yieldFn?: () => Promise<void>;
    initialPages?: readonly PageSlice[];
  } = {},
): Promise<PageSlice[] | null> {
  const yieldEvery = options.yieldEvery ?? NOVEL_READER_PAGINATE_YIELD_EVERY;
  const pages: PageSlice[] = options.initialPages ? options.initialPages.slice() : [];
  const from = pages.length > 0 ? pages[pages.length - 1].end : startCursor();
  if (blocks.length === 0) {
    const empty = pages.length > 0 ? pages : [{ start: startCursor(), end: startCursor() }];
    options.onProgress?.(empty.slice());
    return empty;
  }
  if (pages.length > 0 && isEndCursor(blocks, from)) {
    options.onProgress?.(pages.slice());
    return pages;
  }
  for (const page of iterateNovelPages(blocks, overflows, from)) {
    if (options.shouldAbort?.()) return null;
    pages.push(page);
    if (pages.length === 1 || pages.length % yieldEvery === 0) {
      options.onProgress?.(pages.slice());
      await (options.yieldFn?.() ?? Promise.resolve());
      if (options.shouldAbort?.()) return null;
    }
  }
  if (options.shouldAbort?.()) return null;
  options.onProgress?.(pages.slice());
  return pages.length > 0 ? pages : [{ start: startCursor(), end: startCursor() }];
}

function sliceContainsBlock(blocks: readonly NovelBlock[], slice: PageSlice, blockIndex: number): boolean {
  const start = normalizeCursor(blocks, slice.start);
  const end = normalizeCursor(blocks, slice.end);
  if (blockIndex < start.index || blockIndex > end.index) return false;
  if (blockIndex === end.index) return end.offset > 0;
  return true;
}

function viewForBlock(
  block: NovelBlock,
  index: number,
  from: number,
  to: number,
): PageBlockView | null {
  const key = `${index}:${from}`;
  if (block.kind === "paragraph") {
    const text = block.text.slice(from, to);
    if (!text) return null;
    return { key, kind: "paragraph", text, continuation: from > 0 };
  }
  if (from > 0) return null;
  if (block.kind === "chapter") return { key, kind: "chapter", text: block.text };
  if (block.kind === "page_break") return { key, kind: "page_break" };
  if (block.kind === "artwork_link") return { key, kind: "artwork_link", id: block.id };
  if (block.kind === "uploaded_image") return { key, kind: "uploaded_image", id: block.id };
  return { key, kind: "external_link", label: block.label, url: block.url };
}

function blockLimit(block: NovelBlock): number {
  return block.kind === "paragraph" ? block.text.length : 1;
}

function nextWholeEnd(cursor: PageCursor): PageCursor {
  return { index: cursor.index + 1, offset: 0 };
}

function unitCount(blocks: readonly NovelBlock[]): number {
  let count = 0;
  for (const block of blocks) {
    count += block.kind === "paragraph" ? Math.max(1, block.text.length) : 1;
  }
  return count;
}

function forceAdvance(blocks: readonly NovelBlock[], cursor: PageCursor): PageCursor {
  const from = normalizeCursor(blocks, cursor);
  if (from.index >= blocks.length) return from;
  const block = blocks[from.index];
  if (block.kind === "paragraph") {
    return normalizeCursor(blocks, { index: from.index, offset: from.offset + 1 });
  }
  return { index: from.index + 1, offset: 0 };
}

function splitOverflowingBlock(
  blocks: readonly NovelBlock[],
  start: PageCursor,
  blockIndex: number,
  fromOffset: number,
  overflows: PageOverflowFn,
  fallback?: PageCursor,
): PageCursor {
  const block = blocks[blockIndex];
  if (!block) return fallback ?? forceAdvance(blocks, start);
  if (block.kind !== "paragraph") {
    if (fallback) return fallback;
    return { index: blockIndex + 1, offset: 0 };
  }

  let lo = fromOffset;
  let hi = block.text.length;
  const full = normalizeCursor(blocks, { index: blockIndex, offset: hi });
  if (hi > fromOffset && !overflows({ start, end: full })) return full;

  while (lo + 1 < hi) {
    const mid = (lo + hi) >> 1;
    const end = { index: blockIndex, offset: mid };
    if (!overflows({ start, end })) lo = mid;
    else hi = mid;
  }

  if (lo === fromOffset) {
    if (fallback) return fallback;
    return forceAdvance(blocks, start);
  }
  return normalizeCursor(blocks, { index: blockIndex, offset: lo });
}
