import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const rust = source("../src-tauri/src/lib.rs");
const activity = readFileSync(androidPackagePath("LoginActivity.kt"), "utf8");
const plugin = readFileSync(androidPackagePath("LoginWebViewPlugin.kt"), "utf8");
const profile = source("../src/routes/profile/+page.svelte");
const login = source("../src/routes/login/+page.svelte");
const session = source("../src/lib/session.ts");

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
  assert.match(
    rust,
    /save_refresh_token\(&app, credential\.token\(\), old_mode\)[\s\S]*?if rollback\.is_err\(\)[\s\S]*?delete_refresh_token\(&app\)[\s\S]*?session_state\.clear\(\)/,
  );
});
