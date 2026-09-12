import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

test("the novel reader is an immersive paginated surface without search or speech", () => {
  const page = source("../src/routes/novels/[id]/read/+page.svelte");
  const reader = source("../src/lib/components/NovelImmersiveReader.svelte");
  const shell = source("../src/lib/components/AppShell.svelte");
  const css = source("../src/app.css");

  assert.match(page, /<AppShell immersive/);
  assert.match(page, /<NovelImmersiveReader/);
  assert.match(page, /parseNovelText/);
  assert.match(page, /pixiv-client:novel-progress:/);
  assert.match(shell, /immersive = false/);
  assert.match(css, /\.app-frame\.immersive/);
  assert.match(page, /NOVEL_READER_LOAD_ATTEMPTS/);
  assert.match(page, /shouldRetryNovelReaderLoad/);
  assert.match(reader, /m\.novel_reader_contents\(\)/);
  assert.match(reader, /m\.novel_reader_night\(\)/);
  assert.match(reader, /m\.novel_reader_more\(\)/);
  assert.match(reader, /m\.novel_reader_previous_chapter\(\)/);
  assert.match(reader, /m\.novel_reader_next_chapter\(\)/);
  assert.match(reader, /class="page-clip"/);
  assert.match(reader, /contain: paint/);
  assert.match(reader, /paginateNovelProgressive/);
  assert.match(reader, /materializePage/);
  assert.match(reader, /NOVEL_READER_LONG_TEXT_WEIGHT/);
  assert.match(reader, /visibleItems/);
  assert.match(reader, /class="paged measure"|class="measure paged"/);
  assert.match(reader, /toggleChrome|chromeOpen/);
  assert.match(reader, /writeNovelReaderPreferences/);
  assert.match(reader, /<ReturnLink compact/);
  assert.doesNotMatch(reader, /columnWidth|column-count|column-fill/);
  assert.doesNotMatch(reader, /translate3d\(-\$\{pageIndex/);
  assert.doesNotMatch(reader, /\{#each blocks as block/);
  assert.doesNotMatch(reader, /offsetLeft/);
  assert.doesNotMatch(reader, /type="search"/);
  assert.doesNotMatch(reader, /speechSynthesis|webkitSpeech|utterance/i);
  assert.doesNotMatch(page, /type="search"/);
  assert.doesNotMatch(reader, /\{@html/);
  assert.doesNotMatch(reader, /target="_blank"/);
});
