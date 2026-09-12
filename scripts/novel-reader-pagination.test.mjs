import assert from "node:assert/strict";
import test from "node:test";
import { parseNovelText } from "../src/lib/novel-text.ts";
import {
  chapterMarksFromPages,
  chapterTitleAtPage,
  cursorAtRatio,
  estimatePageCount,
  materializePage,
  pageIndexForChapter,
  pageIndexForCursor,
  paginateNextPage,
  paginateNovel,
  paginateNovelProgressive,
  restorePageIndex,
  sliceContentWeight,
  totalContentWeight,
} from "../src/lib/novel-reader-pagination.ts";

function paragraph(text) {
  return { kind: "paragraph", text };
}

function charBudgetOverflow(blocks, budget, probes) {
  return (slice) => {
    probes.count += 1;
    const views = materializePage(blocks, slice);
    let weight = 0;
    for (const view of views) {
      if (view.kind === "paragraph" || view.kind === "chapter") weight += view.text.length;
      else weight += 8;
    }
    return weight > budget;
  };
}

function flattenParagraphs(blocks) {
  return blocks
    .filter((block) => block.kind === "paragraph")
    .map((block) => block.text)
    .join("");
}

function flattenMaterialized(blocks, pages) {
  return pages
    .flatMap((page) => materializePage(blocks, page))
    .filter((view) => view.kind === "paragraph")
    .map((view) => view.text)
    .join("");
}

test("restorePageIndex matches the previous immersive reader mapping", () => {
  assert.equal(restorePageIndex(0, 1), 0);
  assert.equal(restorePageIndex(1, 1), 0);
  assert.equal(restorePageIndex(0.5, 11), 5);
  assert.equal(restorePageIndex(1, 34), 33);
});

test("a long novel is packed into small pages without quadratic overflow probes", () => {
  const blocks = Array.from({ length: 4000 }, (_, index) => paragraph(`line ${index} ${"x".repeat(72)}`));
  const probes = { count: 0 };
  const pages = paginateNovel(blocks, charBudgetOverflow(blocks, 1800, probes));
  const visibleSizes = pages.map((page) => materializePage(blocks, page).length);
  const maxVisible = Math.max(...visibleSizes);

  assert.ok(pages.length > 80, `expected many pages, got ${pages.length}`);
  assert.equal(flattenMaterialized(blocks, pages), flattenParagraphs(blocks));
  assert.ok(maxVisible < 40, `a visible page still mounts ${maxVisible} blocks`);
  assert.ok(maxVisible * 20 < blocks.length);
  assert.ok(
    probes.count < blocks.length * 4,
    `overflow probes scaled too fast: ${probes.count} for ${blocks.length} blocks`,
  );
});

test("paragraphs taller than one page are split and later joined back", () => {
  const blocks = [paragraph("abcdefghij".repeat(40))];
  const probes = { count: 0 };
  const pages = paginateNovel(blocks, charBudgetOverflow(blocks, 50, probes));
  assert.ok(pages.length >= 8);
  assert.equal(flattenMaterialized(blocks, pages), blocks[0].text);
  assert.equal(materializePage(blocks, pages[1])[0].continuation, true);
  assert.ok(probes.count < 80);
});

test("chapter titles are resolved from cached page marks without layout reads", () => {
  const blocks = [
    { kind: "chapter", text: "One" },
    paragraph("aaaa".repeat(20)),
    { kind: "chapter", text: "Two" },
    paragraph("bbbb".repeat(20)),
  ];
  const pages = paginateNovel(blocks, charBudgetOverflow(blocks, 40, { count: 0 }));
  const marks = chapterMarksFromPages(blocks, pages);
  assert.equal(marks[0]?.text, "One");
  assert.equal(chapterTitleAtPage(marks, 0, "fallback"), "One");
  assert.equal(chapterTitleAtPage(marks, pages.length - 1, "fallback"), "Two");
  assert.equal(pageIndexForChapter(blocks, pages, 2), marks.find((mark) => mark.text === "Two")?.page);
  assert.equal(pageIndexForChapter(blocks, pages, 99), -1);
});

test("atomic blocks that do not fit still advance so pagination cannot stall", () => {
  const blocks = [
    { kind: "page_break" },
    { kind: "artwork_link", id: "12" },
    paragraph("ok"),
  ];
  const pages = paginateNovel(blocks, () => true);
  const views = pages.flatMap((page) => materializePage(blocks, page));
  assert.ok(pages.length >= 3);
  assert.equal(views.some((view) => view.kind === "page_break"), true);
  assert.equal(views.some((view) => view.kind === "artwork_link" && view.id === "12"), true);
  assert.equal(views.filter((view) => view.kind === "paragraph").map((view) => view.text).join(""), "ok");
});

test("progressive pagination can yield after the first page and abort in-flight work", async () => {
  const blocks = Array.from({ length: 80 }, (_, index) => paragraph(`p${index} ${"y".repeat(40)}`));
  const yields = { count: 0 };
  const seen = [];
  const pages = await paginateNovelProgressive(blocks, charBudgetOverflow(blocks, 90, { count: 0 }), {
    yieldEvery: 4,
    yieldFn: async () => {
      yields.count += 1;
    },
    onProgress: (next) => {
      seen.push(next.length);
    },
  });
  assert.ok(pages && pages.length > 8);
  assert.equal(seen[0], 1);
  assert.ok(yields.count >= 2);

  const aborted = await paginateNovelProgressive(blocks, charBudgetOverflow(blocks, 90, { count: 0 }), {
    shouldAbort: () => true,
  });
  assert.equal(aborted, null);
});

test("a 200k-character novel can open a page without packing the whole book", () => {
  const blocks = Array.from({ length: 500 }, () => paragraph("字".repeat(400)));
  assert.equal(totalContentWeight(blocks), 200_000);

  const probes = { count: 0 };
  const pages = paginateNovel(blocks, charBudgetOverflow(blocks, 500, probes));
  const firstVisible = materializePage(blocks, pages[0]).length;
  assert.ok(pages.length >= 350, `expected hundreds of pages, got ${pages.length}`);
  assert.ok(firstVisible <= 3, `first page still mounts ${firstVisible} blocks`);
  assert.ok(
    probes.count < pages.length * 16,
    `200k packing probed too much: ${probes.count} for ${pages.length} pages`,
  );

  const seekProbes = { count: 0 };
  const preview = paginateNextPage(blocks, cursorAtRatio(blocks, 0.8), charBudgetOverflow(blocks, 500, seekProbes));
  assert.ok(seekProbes.count < 24, `seeking 80% still probed ${seekProbes.count} times`);
  assert.ok(sliceContentWeight(blocks, preview) > 0);
  assert.ok(sliceContentWeight(blocks, preview) <= 500);
  const restored = pageIndexForCursor(pages, preview.start);
  assert.ok(Math.abs(restored / Math.max(pages.length - 1, 1) - 0.8) < 0.08);
  const estimated = estimatePageCount(200_000, sliceContentWeight(blocks, pages[0]));
  assert.ok(Math.abs(estimated - pages.length) <= Math.max(3, Math.round(pages.length * 0.05)));
});

test("Pixiv novel markup still round-trips through page slices", () => {
  const blocks = parseNovelText(
    "[chapter:Start]\n\nhello\n\n[newpage]\n\nworld\n\n[pixivimage:42]\n",
    "Chapter",
  );
  const pages = paginateNovel(blocks, charBudgetOverflow(blocks, 8, { count: 0 }));
  const views = pages.flatMap((page) => materializePage(blocks, page));
  assert.equal(views.some((view) => view.kind === "chapter" && view.text === "Start"), true);
  assert.equal(views.some((view) => view.kind === "page_break"), true);
  assert.equal(views.some((view) => view.kind === "artwork_link" && view.id === "42"), true);
  assert.equal(views.filter((view) => view.kind === "paragraph").map((view) => view.text).join(""), "helloworld");
});
