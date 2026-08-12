import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const rust = source("../src-tauri/src/lib.rs");
const secureStorage = source("../src-tauri/src/secure_storage.rs");
const sessionSwitch = source("../src-tauri/src/session_switch.rs");
const activity = readFileSync(androidPackagePath("LoginActivity.kt"), "utf8");
const plugin = readFileSync(androidPackagePath("LoginWebViewPlugin.kt"), "utf8");
const profile = source("../src/routes/profile/+page.svelte");
const login = source("../src/routes/login/+page.svelte");
const session = source("../src/lib/session.ts");

function assertOrdered(sourceText, fragments) {
  let cursor = -1;
  for (const fragment of fragments) {
    const next = sourceText.indexOf(fragment, cursor + 1);
    assert.notEqual(next, -1, `missing ordered fragment: ${fragment}`);
    assert.ok(next > cursor, `fragment is out of order: ${fragment}`);
    cursor = next;
  }
}

test("Android captures only the expected Pixiv callback inside the private login activity", () => {
  assert.match(activity, /uri\.scheme\?\.lowercase\(\)\s*==\s*"pixiv"/);
  assert.match(activity, /uri\.host\?\.lowercase\(\)\s*==\s*"account"/);
  assert.match(activity, /uri\.path\s*==\s*"\/login"/);
  assert.match(activity, /LoginResultRegistry\.publish/);
  assert.doesNotMatch(activity, /addJavascriptInterface/);
});

test("the callback URL remains native-to-Rust and never crosses the frontend API", () => {
  assert.match(plugin, /fun takeLoginResult/);
  assert.match(rust, /complete_mobile_interactive_login/);
  assert.doesNotMatch(login, /callbackUrl/);
});

test("Android refresh tokens are encrypted with an AndroidKeyStore AES-GCM key", () => {
  assert.match(plugin, /AndroidKeyStore/);
  assert.match(plugin, /AES\/GCM\/NoPadding/);
  assert.match(plugin, /fun saveRefreshToken/);
  assert.match(plugin, /fun loadRefreshToken/);
  assert.match(plugin, /fun deleteRefreshToken/);
});

test("desktop and Android credential replacement fail closed around the write", () => {
  const desktopSave = secureStorage.slice(
    secureStorage.lastIndexOf("pub(crate) async fn save_refresh_token("),
    secureStorage.lastIndexOf("pub(crate) async fn load_refresh_token("),
  );
  assertOrdered(desktopSave, [
    "mark_desktop_credentials_invalid()?",
    ".set_password(credential.as_str())",
    "clear_desktop_invalidation_marker()",
  ]);

  const androidSave = plugin.slice(
    plugin.indexOf("fun save(context:"),
    plugin.indexOf("fun load(context:"),
  );
  assertOrdered(androidSave, [
    "invalidate(context)",
    ".putString(IV_KEY",
    "clearInvalidation(context)",
  ]);
});

test("session commands expose profile data without exposing either token", () => {
  assert.match(rust, /restore_session/);
  assert.match(rust, /get_session_status/);
  assert.match(rust, /logout/);
  assert.doesNotMatch(profile, /accessToken|refreshToken/);
  assert.match(profile, /initializeSession/);
  assert.match(session, /invoke<SessionSnapshot>\("restore_session"/);
});

test("the selected connection mode follows OAuth exchange and persisted refresh credentials", () => {
  assert.match(rust, /traffic:\s*TrafficClass::OAuth/);
  assert.match(rust, /build_client\(&ProbeRequest/);
  assert.match(rust, /save_refresh_token\(app, refresh_token\.as_str\(\), mode\)/);
  assert.match(plugin, /connectionMode/);
  assert.match(login, /unsafeAcknowledged\(\): boolean/);
  assert.match(login, /return mode === "compatible"/);
  assert.doesNotMatch(login, /URLSearchParams\(window\.location\.search\)[\s\S]*?get\("mode"\)/);
  assert.doesNotMatch(login, /login_risk_bridge_compatible|login_warning/);
  assert.doesNotMatch(profile, /profile_compatible_refresh_risk|session-risk/);
  assert.match(rust, /session_switch\s*\.\s*switch\(/);
  assert.match(sessionSwitch, /rollback_failure_invalidates_both_session_and_stored_credential/);
});
