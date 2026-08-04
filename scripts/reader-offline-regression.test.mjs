import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function source(path) {
  return readFile(new URL(path, root), "utf8");
}

test("Rust API exposes manga, novel, and Ugoira readers with bound cursors", async () => {
  const api = await source("crates/api/src/lib.rs");
  for (const endpoint of [
    "/v1/illust/recommended",
    "/v1/novel/recommended",
    "/v2/novel/detail",
    "/webview/v2/novel",
    "/v1/ugoira/metadata",
  ]) {
    assert.match(api, new RegExp(endpoint.replaceAll("/", "\\/")));
  }
  assert.doesNotMatch(api, /\/v1\/novel\/text/);
  assert.match(api, /viewer_version", "20221031_ai/);
  assert.match(api, /decode_cursor\(cursor, NOVEL_RECOMMENDED_PATH/);
  assert.match(api, /\.and_then\(validated_media_url\)/);
  assert.ok(api.includes("&& !file_name.contains(['/', '\\\\'])"));
  assert.match(api, /fn extracts_balanced_novel_json_without_accepting_a_different_id/);
  assert.match(api, /fn validates_ugoira_archive_and_frame_metadata/);
});

test("offline library confines paths and replaces entries transactionally", async () => {
  const library = await source("crates/library/src/lib.rs");
  assert.match(library, /pub struct OfflineLibrary/);
  assert.match(library, /\.staging-\{sequence\}/);
  assert.match(library, /\.backup-\{sequence\}/);
  assert.match(library, /fs::rename\(&target, &backup\)/);
  assert.match(library, /let _ = fs::rename\(&backup, &target\)/);
  assert.match(library, /normalized_resource_id/);
  assert.match(library, /validate_asset_name/);
  assert.match(library, /name\s*\.bytes\(\)\s*\.all/);
  assert.match(library, /fn rejects_traversal_and_empty_transactions/);
});

test("Tauri owns downloads, extraction, limits, and offline command registration", async () => {
  const [backend, paths] = await Promise.all([
    source("src-tauri/src/lib.rs"),
    source("src-tauri/src/paths.rs"),
  ]);
  for (const command of [
    "download_artwork",
    "download_novel",
    "prepare_ugoira",
    "list_offline_entries",
    "get_offline_stats",
    "read_offline_asset",
    "read_offline_text",
    "remove_offline_entry",
  ]) {
    assert.match(backend, new RegExp(`async fn ${command}\\(`));
    assert.match(backend, new RegExp(`\\n\\s+${command},`));
  }
  assert.match(backend, /library_gate: Arc<tokio::sync::Semaphore>/);
  assert.match(backend, /MAX_UGOIRA_ARCHIVE_BYTES/);
  assert.match(backend, /\.by_name\(&frame\.file_name\)/);
  assert.match(backend, /total_uncompressed > MAX_UGOIRA_ARCHIVE_BYTES/);
  assert.match(backend, /paths::app_data_dir\(app\).*join\("offline-library"\)/s);
  assert.match(paths, /app\.path\(\)\.app_data_dir\(\)/);
  assert.match(paths, /#\[cfg\(not\(debug_assertions\)\)\][\s\S]*None/);
});

test("frontend uses typed readers and local-only offline routes", async () => {
  const manga = await source("src/lib/components/BrowsePage.svelte");
  const novels = await source("src/routes/novels/+page.svelte");
  const novelDetail = await source("src/routes/novels/[id]/+page.svelte");
  const novelReader = await source("src/routes/novels/[id]/read/+page.svelte");
  const artworkReader = await source("src/routes/artworks/[id]/+page.svelte");
  const ugoira = await source("src/lib/components/UgoiraPlayer.svelte");
  const offline = await source("src/routes/offline/+page.svelte");
  const offlineArtwork = await source("src/routes/offline/artworks/[id]/+page.svelte");
  const artworkViewer = await source("src/lib/components/ArtworkImageViewer.svelte");
  const offlineNovel = await source("src/routes/offline/novels/[id]/+page.svelte");
  const offlineUgoira = await source("src/routes/offline/ugoira/[id]/+page.svelte");

  assert.match(manga, /getRecommendedManga/);
  assert.match(novels, /getRecommendedNovels/);
  assert.match(novelDetail, /getNovelDetail/);
  assert.match(novelDetail, /\/read/);
  assert.match(novelReader, /getNovelDetail/);
  assert.match(novelReader, /getNovelContent/);
  assert.match(novelReader, /parseNovelText/);
  assert.doesNotMatch(novelReader, /\{@html/);
  assert.doesNotMatch(novelReader, /target="_blank"/);
  assert.match(artworkReader, /<UgoiraPlayer/);
  assert.match(ugoira, /prepareUgoira/);
  assert.match(ugoira, /readOfflineAsset/);
  assert.match(ugoira, /URL\.revokeObjectURL/);
  assert.match(offline, /listOfflineEntries/);
  assert.match(offline, /removeOfflineEntry/);
  assert.match(offlineArtwork, /readOfflineText/);
  assert.match(offlineArtwork, /<ArtworkImageViewer/);
  assert.match(artworkViewer, /<OfflineImage/);
  assert.match(offlineNovel, /readOfflineText/);
  assert.match(offlineNovel, /parseNovelText/);
  assert.match(offlineUgoira, /<OfflineUgoiraPlayer/);
});
