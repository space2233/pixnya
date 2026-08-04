import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const appShell = source("../src/lib/components/AppShell.svelte");
const profilePage = source("../src/routes/profile/+page.svelte");
const artworkThumbnail = source("../src/lib/components/ArtworkThumbnail.svelte");
const pixivImage = source("../src/lib/components/PixivImage.svelte");
const mediaState = source("../src/lib/media.ts");
const rustCommands = source("../src-tauri/src/lib.rs");

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
  assert.match(pixivImage, /requestInsecureMediaFallback/);
});

test("ECH media fallback requires a session-scoped acknowledgement", () => {
  assert.match(rustCommands, /UnsafeMediaAcknowledgementRequired/);
  assert.match(rustCommands, /acknowledge_insecure_media_fallback/);
  assert.match(rustCommands, /media_fallback_generation/);
  assert.match(mediaState, /MEDIA_FALLBACK_REQUIRED_EVENT/);
  assert.match(mediaState, /MEDIA_RETRY_EVENT/);
});
