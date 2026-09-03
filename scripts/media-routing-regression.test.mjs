import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const appShell = source("../src/lib/components/AppShell.svelte");
const profilePage = source("../src/routes/profile/+page.svelte");
const artworkThumbnail = source("../src/lib/components/ArtworkThumbnail.svelte");
const thumbnailSkeleton = source("../src/lib/components/ThumbnailSkeleton.svelte");
const browsePage = source("../src/lib/components/BrowsePage.svelte");
const pixivImage = source("../src/lib/components/PixivImage.svelte");
const ugoiraPlayer = source("../src/lib/components/UgoiraPlayer.svelte");
const artworkViewer = source("../src/lib/components/ArtworkImageViewer.svelte");
const preferences = source("../src/lib/preferences.ts");
const rustCommands = source("../src-tauri/src/lib.rs");
const mediaCache = source("../crates/media-cache/src/lib.rs");

test("all remote Pixiv images cross the Rust media pipeline", () => {
  assert.match(appShell, /<PixivImage[\s\S]*url=\{avatarUrl\}/);
  assert.match(profilePage, /<PixivImage[\s\S]*url=\{avatarUrl\}/);
  assert.match(artworkThumbnail, /<PixivImage/);
  assert.doesNotMatch(appShell, /<img[\s\S]*src=\{avatarUrl\}/);
  assert.doesNotMatch(profilePage, /<img[\s\S]*src=\{avatarUrl\}/);
});

test("Android IPC media bytes are normalized before Blob decoding", () => {
  assert.match(pixivImage, /new Uint8Array\(buffer\)/);
  assert.match(pixivImage, /fetch_pixiv_thumbnail/);
  assert.doesNotMatch(pixivImage, /requestInsecureMediaFallback|MEDIA_RETRY_EVENT/);
});

test("artwork thumbnails use the selected neutral skeleton placeholder", () => {
  assert.match(artworkThumbnail, /import ThumbnailSkeleton/);
  assert.match(artworkThumbnail, /<ThumbnailSkeleton \/>/);
  assert.match(artworkThumbnail, /m\.thumbnail_unavailable\(\)/);
  assert.match(thumbnailSkeleton, /class="skeleton-art"/);
  assert.match(thumbnailSkeleton, /linear-gradient\(135deg, #f1f3f5 52%, #eceff1 52%\)/);
  assert.match(thumbnailSkeleton, /animation: sweep 1\.5s infinite/);
  assert.doesNotMatch(artworkThumbnail, /class="fallback"[^>]*>p</);
});

test("the animated thumbnail skeleton starts on the first browse render", () => {
  assert.match(browsePage, /import ThumbnailSkeleton from "\$lib\/components\/ThumbnailSkeleton\.svelte"/);
  assert.match(browsePage, /<ThumbnailSkeleton \/>/);
  assert.doesNotMatch(browsePage, /<div class="work-cover tone-/);
});

test("media requests keep the selected global connection mode and never prompt for a fallback", () => {
  assert.doesNotMatch(rustCommands, /UnsafeMediaAcknowledgementRequired/);
  assert.doesNotMatch(rustCommands, /acknowledge_insecure_media_fallback/);
  assert.doesNotMatch(rustCommands, /media_fallback_generation|media_mode_for/);
  assert.doesNotMatch(appShell, /mediaRisk|media_risk_|InsecureMediaWarning|MEDIA_FALLBACK/);
  assert.doesNotMatch(pixivImage, /unsafe_media_acknowledgement_required|requestInsecureMediaFallback/);
  assert.doesNotMatch(ugoiraPlayer, /unsafe_media_acknowledgement_required|requestInsecureMediaFallback/);
  assert.doesNotMatch(preferences, /insecure-media-warning|InsecureMediaWarning/);
});

test("full-resolution viewer media stays transient instead of filling the disk cache", () => {
  assert.match(artworkViewer, /cacheKind=\{currentPage\.originalUrl \? null : "preview"\}/);
  assert.match(artworkViewer, /fallbackCacheKind=\{currentPage\.originalUrl \? null : "preview"\}/);
  assert.doesNotMatch(artworkViewer, /cacheKind="original"/);
  assert.match(rustCommands, /cache_kind\.filter\([\s\S]*CacheKind::Thumbnail \| CacheKind::Preview/);
  assert.match(rustCommands, /fn ensure_resident_media_cache/);
  assert.match(rustCommands, /if let Some\(cache_kind\) = cache_kind[\s\S]*ensure_resident_media_cache/);
  assert.match(rustCommands, /store_epoch\.is_current\(expected_epoch\)/);
  assert.match(rustCommands, /async fn clear_media_cache[\s\S]*epoch\.advance\(\)/);
  assert.match(rustCommands, /async fn clear_local_data[\s\S]*cache_epoch\.advance\(\)/);
});

test("thumbnail fetches reuse a process-resident media cache instead of reopening on every hit", () => {
  const fetchStart = rustCommands.indexOf("async fn fetch_pixiv_thumbnail");
  const fetchEnd = rustCommands.indexOf("async fn get_media_cache_stats");
  const fetch = rustCommands.slice(fetchStart, fetchEnd);
  assert.ok(fetchStart >= 0 && fetchEnd > fetchStart);
  assert.doesNotMatch(fetch, /MediaCache::open/);
  assert.match(fetch, /ensure_resident_media_cache/);
  assert.match(rustCommands, /gate: Arc<Mutex<Option<MediaCache>>>/);
  assert.match(mediaCache, /entry\.kind != CacheKind::Original/);
  assert.match(mediaCache, /fn mark_hit_dirty/);
  assert.match(mediaCache, /INDEX_FLUSH_HIT_INTERVAL/);
  assert.match(mediaCache, /reopening_purges_legacy_originals_without_dropping_previews/);
  assert.match(mediaCache, /cache_hits_do_not_rewrite_the_index_until_a_flush/);
  assert.doesNotMatch(artworkThumbnail, /readOffline|OfflineImage/);
  assert.doesNotMatch(browsePage, /readOfflineText|OfflineImage/);
});
