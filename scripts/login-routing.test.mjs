import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const rust = source("../src-tauri/src/lib.rs");
const loginRoute = source("../src-tauri/src/login_route.rs");
const proxy = source("../crates/network/src/login_proxy.rs");
const loginPage = source("../src/routes/login/+page.svelte");
const androidActivity = readFileSync(androidPackagePath("LoginActivity.kt"), "utf8");
const manifest = source("../src-tauri/gen/android/app/src/main/AndroidManifest.xml");

test("official login URL uses public PKCE parameters and is launched by the UI", () => {
  assert.match(rust, /https:\/\/app-api\.pixiv\.net\/web\/v1\/login/);
  assert.match(rust, /append_pair\("code_challenge"/);
  assert.match(rust, /append_pair\("code_challenge_method"/);
  assert.match(rust, /append_pair\("client", "pixiv-android"\)/);
  assert.doesNotMatch(rust, /append_pair\("client_secret"/i);
  assert.match(loginPage, /invoke<LoginLaunchResult>\("open_interactive_login"/);
  assert.match(loginPage, /onclick=\{openOfficialLogin\}/);
});

test("desktop compatibility and ECH modes configure distinct WebView paths", () => {
  assert.match(rust, /builder = builder\.proxy_url\(proxy_url\)/);
  assert.match(rust, /mode == ConnectionMode::Ech/);
  assert.match(rust, /--enable-features=EncryptedClientHello/);
  assert.match(rust, /accepted_by_rust_preflight/);
  assert.match(proxy, /method != "CONNECT"/);
  assert.match(proxy, /if port != 443/);
});

test("login preparation and launch share one route-validation module", () => {
  assert.match(loginRoute, /pub\(crate\) fn evaluate_login_route/);
  assert.match(loginRoute, /requires_user_acknowledgement/);
  assert.match(rust, /evaluate_login_route\(/);
  assert.doesNotMatch(
    rust,
    /mode == ConnectionMode::Compatible \|\| route\.requires_user_acknowledgement/,
  );
});

test("Android waits for proxy callbacks and scopes the bridge certificate exception", () => {
  const setProxy = androidActivity.indexOf("controller.setProxyOverride");
  const loadAfterSet = androidActivity.indexOf("loadOfficialPage(url)", setProxy);
  const clearProxy = androidActivity.indexOf("controller.clearProxyOverride", loadAfterSet);
  const loadAfterClear = androidActivity.indexOf("loadOfficialPage(url)", clearProxy);

  assert.ok(setProxy >= 0 && loadAfterSet > setProxy);
  assert.ok(clearProxy > loadAfterSet && loadAfterClear > clearProxy);
  assert.match(
    androidActivity,
    /onReceivedSslError[\s\S]*?isPinnedBridgeCertificate[\s\S]*?handler\.proceed\(\)[\s\S]*?handler\.cancel\(\)/,
  );
  assert.doesNotMatch(androidActivity, /addDirect\(/);
  assert.match(
    manifest,
    /android:name="\.LoginActivity"[\s\S]*?android:exported="false"/,
  );
});
