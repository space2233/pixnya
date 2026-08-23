import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  adjacentViewerPage,
  clampViewerScale,
  panViewer,
  pinchViewer,
  zoomViewerAt,
} from "../src/lib/image-viewer.ts";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

test("viewer scale is finite and confined to the supported 1x through 6x range", () => {
  assert.equal(clampViewerScale(Number.NaN), 1);
  assert.equal(clampViewerScale(-10), 1);
  assert.equal(clampViewerScale(2.5), 2.5);
  assert.equal(clampViewerScale(99), 6);
});

test("wheel or button zoom preserves the selected visual anchor", () => {
  const result = zoomViewerAt(
    { scale: 1, x: 0, y: 0 },
    2,
    { x: 100, y: 50 },
    { width: 1000, height: 800 },
  );
  assert.deepEqual(result, { scale: 2, x: -100, y: -50 });
  assert.equal((100 - result.x) / result.scale, 100);
  assert.equal((50 - result.y) / result.scale, 50);
});

test("pan and pinch transforms cannot lose the image outside the viewport", () => {
  const viewport = { width: 400, height: 300 };
  assert.deepEqual(
    panViewer({ scale: 2, x: 0, y: 0 }, { x: 900, y: -900 }, viewport),
    { scale: 2, x: 200, y: -150 },
  );
  assert.deepEqual(
    pinchViewer(
      { scale: 2, x: 0, y: 0 },
      { x: 0, y: 0 },
      { x: 20, y: 10 },
      1.5,
      viewport,
    ),
    { scale: 3, x: 20, y: 10 },
  );
});

test("keyboard page changes stop at the first and last image", () => {
  assert.equal(adjacentViewerPage(0, 3, -1), 0);
  assert.equal(adjacentViewerPage(0, 3, 1), 1);
  assert.equal(adjacentViewerPage(2, 3, 1), 2);
  assert.equal(adjacentViewerPage(0, 0, 1), 0);
});

test("online and offline artwork routes share the guarded image viewer", async () => {
  const [component, online, offline, paths] = await Promise.all([
    source("src/lib/components/ArtworkImageViewer.svelte"),
    source("src/routes/artworks/[id]/+page.svelte"),
    source("src/routes/offline/artworks/[id]/+page.svelte"),
    source("src-tauri/src/paths.rs"),
  ]);
  assert.match(online, /<ArtworkImageViewer/);
  assert.match(online, /originalUrl: image\.originalUrl/);
  assert.match(offline, /<ArtworkImageViewer/);
  assert.match(offline, /entryKey: key/);
  assert.match(component, /<PixivImage/);
  assert.match(component, /cacheKind=\{currentPage\.originalUrl \? null : "preview"\}/);
  assert.match(component, /<OfflineImage/);
  assert.match(component, /role="dialog"/);
  assert.match(component, /aria-modal="true"/);
  assert.match(component, /handleWheel/);
  assert.match(component, /pinchViewer/);
  assert.match(component, /event\.key === "Escape"/);
  assert.match(component, /event\.key === "ArrowLeft"/);
  assert.match(component, /event\.key === "ArrowRight"/);
  assert.match(component, /window\.history\.pushState/);
  assert.match(component, /window\.addEventListener\("popstate"/);
  assert.match(component, /touch-action: none/);
  assert.doesNotMatch(component, /<img\s+src=\{currentPage\.(?:originalUrl|previewUrl)/);
  assert.match(paths, /#\[cfg\(debug_assertions\)\][\s\S]*PIXIV_CLIENT_TEST_ROOT/);
  assert.match(paths, /#\[cfg\(not\(debug_assertions\)\)\][\s\S]*None/);
});
