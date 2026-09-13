import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

test("volume keys map down to the next page and up to the previous page", async () => {
  const { volumePageDirectionFromKey, volumePageDirectionFromDetail, isVolumePageTurnSupported } =
    await import("../src/lib/volume-page-turn.ts");
  assert.equal(volumePageDirectionFromKey({ key: "AudioVolumeDown", code: "" }), "next");
  assert.equal(volumePageDirectionFromKey({ key: "AudioVolumeUp", code: "" }), "previous");
  assert.equal(volumePageDirectionFromKey({ key: "VolumeDown", code: "VolumeDown" }), "next");
  assert.equal(volumePageDirectionFromKey({ key: "a", code: "KeyA" }), null);
  assert.equal(volumePageDirectionFromDetail({ direction: "next" }), "next");
  assert.equal(volumePageDirectionFromDetail({ direction: "previous" }), "previous");
  assert.equal(volumePageDirectionFromDetail({ direction: "mute" }), null);

  assert.equal(isVolumePageTurnSupported("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"), false);
  assert.equal(isVolumePageTurnSupported("Mozilla/5.0 (X11; Linux x86_64)"), false);
  assert.equal(isVolumePageTurnSupported("Mozilla/5.0 (Linux; Android 14; Pixel 8)"), true);
});

test("Android captures volume keys only while the reader asks for them", async () => {
  const [plugin, activity, rust, helper, reader, settings] = await Promise.all([
    readFile(androidPackagePath("VolumeKeysPlugin.kt"), "utf8"),
    readFile(androidPackagePath("MainActivity.kt"), "utf8"),
    source("src-tauri/src/lib.rs"),
    source("src/lib/volume-page-turn.ts"),
    source("src/lib/components/NovelImmersiveReader.svelte"),
    source("src/routes/settings/interface/+page.svelte"),
  ]);

  assert.match(plugin, /fun setCaptureEnabled/);
  assert.match(plugin, /KEYCODE_VOLUME_DOWN -> "next"/);
  assert.match(plugin, /KEYCODE_VOLUME_UP -> "previous"/);
  assert.match(plugin, /pixnya-volume-page/);
  assert.match(activity, /override fun dispatchKeyEvent/);
  assert.match(activity, /VolumePageTurn\.captureEnabled/);
  assert.match(activity, /event\.repeatCount == 0/);
  assert.match(rust, /register_android_plugin\("io\.github\.space2233\.pixnya", "VolumeKeysPlugin"\)/);
  assert.match(rust, /async fn set_volume_page_turn_capture/);
  assert.match(rust, /set_volume_page_turn_capture,/);
  assert.match(helper, /invoke\("set_volume_page_turn_capture"/);
  assert.match(helper, /function isVolumePageTurnSupported/);
  assert.match(helper, /\/Android\/i\.test\(userAgent\)/);
  assert.match(reader, /isVolumePageTurnSupported/);
  assert.match(reader, /\{#if volumePageTurnSupported\}/);
  assert.match(reader, /setVolumePageTurnCapture\(false\)/);
  assert.match(settings, /isVolumePageTurnSupported/);
  assert.match(settings, /\{#if volumePageTurnSupported\}/);
  assert.match(settings, /writeVolumePageTurnEnabled/);
});
