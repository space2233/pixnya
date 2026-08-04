import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("series APIs are exposed through Rust with resource-bound cursors", async () => {
  const [api, tauri, bridge] = await Promise.all([
    read("crates/api/src/lib.rs"),
    read("src-tauri/src/lib.rs"),
    read("src/lib/pixiv-api.ts"),
  ]);

  assert.match(api, /ILLUSTRATION_SERIES_PATH: &str = "\/v1\/illust\/series"/);
  assert.match(api, /NOVEL_SERIES_PATH: &str = "\/v2\/novel\/series"/);
  assert.match(api, /decode_cursor\(cursor, ILLUSTRATION_SERIES_PATH, &bindings\)/);
  assert.match(api, /decode_cursor\(cursor, NOVEL_SERIES_PATH, &bindings\)/);
  assert.match(api, /illust_series_id/);
  assert.match(api, /series_navigation/);
  assert.match(tauri, /async fn get_illustration_series/);
  assert.match(tauri, /async fn get_novel_series/);
  assert.match(tauri, /get_illustration_series,/);
  assert.match(tauri, /get_novel_series,/);
  assert.match(bridge, /invoke<IllustrationSeriesPage>\("get_illustration_series"/);
  assert.match(bridge, /invoke<NovelSeriesPage>\("get_novel_series"/);
});

test("independent series pages and continuous navigation are wired", async () => {
  const [artSeries, novelSeries, artDetail, novelDetail, novelReader, resolver] = await Promise.all([
    read("src/routes/series/artworks/[id]/+page.svelte"),
    read("src/routes/series/novels/[id]/+page.svelte"),
    read("src/routes/artworks/[id]/+page.svelte"),
    read("src/routes/novels/[id]/+page.svelte"),
    read("src/routes/novels/[id]/read/+page.svelte"),
    read("src/lib/artwork-series-navigation.ts"),
  ]);

  assert.match(artSeries, /getIllustrationSeries/);
  assert.match(artSeries, /rememberArtworkSeriesPage/);
  assert.match(artSeries, /从第一部开始连续浏览/);
  assert.match(novelSeries, /getNovelSeries/);
  assert.match(novelSeries, /从第一篇开始连续阅读/);
  assert.match(artDetail, /resolveArtworkSeriesNavigation/);
  assert.match(artDetail, /\/series\/artworks\//);
  assert.match(artDetail, /上一篇|上一部/);
  assert.match(artDetail, /下一篇|下一部/);
  assert.match(novelDetail, /\/series\/novels\//);
  assert.match(novelDetail, /\/read/);
  assert.match(novelReader, /content\.seriesNavigation\.previous/);
  assert.match(novelReader, /content\.seriesNavigation\.next/);
  assert.match(novelReader, /\/read/);
  assert.match(novelDetail, /\/series\/novels\//);
  assert.match(resolver, /MAX_LOOKUP_PAGES/);
  assert.match(resolver, /entry\.nextCursor/);
});
