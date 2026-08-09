import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const activity = readFileSync(androidPackagePath("LoginActivity.kt"), "utf8");
const plugin = readFileSync(androidPackagePath("LoginWebViewPlugin.kt"), "utf8");
const layout = source(
  "../src-tauri/gen/android/app/src/main/res/layout/activity_login.xml",
);
const rust = source("../src-tauri/src/lib.rs");
const proxy = source("../crates/network/src/login_proxy.rs");

test("Android login chrome reserves the system status-bar inset", () => {
  assert.match(layout, /android:id="@\+id\/login_root"/);
  assert.match(activity, /ViewCompat\.setOnApplyWindowInsetsListener/);
  assert.match(activity, /WindowInsetsCompat\.Type\.systemBars\(\)/);
  assert.match(activity, /updatePadding\([\s\S]*top\s*=\s*systemBars\.top/);
});

test("closing the login activity never paints an about:blank frame", () => {
  const destroyStart = activity.indexOf("override fun onDestroy()");
  const destroyEnd = activity.indexOf("companion object", destroyStart);
  assert.ok(destroyStart >= 0 && destroyEnd > destroyStart);
  const onDestroy = activity.slice(destroyStart, destroyEnd);
  assert.doesNotMatch(onDestroy, /loadUrl\("about:blank"\)/);
});

test("Android ECH and compatible login both use the explicit low-security bridge", () => {
  assert.doesNotMatch(rust, /AndroidWebViewEchUnavailable/);
  assert.doesNotMatch(rust, /AndroidWebViewDirectUnavailable/);
  assert.match(
    rust,
    /ConnectionMode::Ech\s*\|\s*ConnectionMode::Compatible[\s\S]*?LoginProxyMode::InsecureTlsBridge/,
  );
  assert.match(
    activity,
    /mode == MODE_ECH \|\| mode == MODE_COMPATIBLE[\s\S]*?setProxyOverride/,
  );
});

test("the bridge is loopback-only, Pixiv-only, and disables upstream SNI verification", () => {
  assert.match(proxy, /TcpListener::bind\(\("127\.0\.0\.1", 0\)\)/);
  assert.match(proxy, /compatible_socket_address\(&host, port\)/);
  assert.match(proxy, /LoginProxyMode::InsecureTlsBridge/);
  assert.match(proxy, /ServerName::IpAddress/);
  assert.match(proxy, /NoServerCertificateVerification/);
  assert.match(proxy, /copy_bidirectional/);
});

test("Android accepts only the per-session bridge certificate and cancels every other TLS error", () => {
  assert.match(activity, /EXTRA_BRIDGE_CERT_SHA256/);
  assert.match(rust, /bridge_cert_sha256\s*=\s*proxy\.certificate_sha256\(\)/);
  assert.match(plugin, /bridgeCertSha256[\s\S]*?EXTRA_BRIDGE_CERT_SHA256/);
  assert.match(
    activity,
    /isBridgeMode\(\)[\s\S]*?isAllowedBridgeUrl\(error\.url\)[\s\S]*?isPinnedBridgeCertificate\(error\.certificate\)/,
  );
  assert.match(activity, /handler\.proceed\(\)[\s\S]*?else\s*\{[\s\S]*?handler\.cancel\(\)/);
  assert.match(proxy, /generate_simple_self_signed/);
  assert.match(proxy, /certificate_sha256/);
});

test("OAuth transport preparation overlaps the time spent in the official login activity", () => {
  assert.match(rust, /prepared_oauth_client:\s*Option<PreparedOAuthClient>/);
  const launchStart = rust.indexOf("async fn open_interactive_login(");
  const launchEnd = rust.indexOf("fn build_authorization_url", launchStart);
  const launch = rust.slice(launchStart, launchEnd);
  const prepareAt = launch.indexOf("begin_oauth_client_preparation(");
  const openSurfaceAt = launch.indexOf("open_login_surface(");
  assert.ok(prepareAt >= 0, "OAuth transport is not prepared when login opens.");
  assert.ok(
    openSurfaceAt > prepareAt,
    "OAuth transport preparation must begin before the official login surface opens.",
  );

  const completionStart = rust.indexOf("async fn complete_captured_login(");
  const completionEnd = rust.indexOf("async fn install_login_tokens", completionStart);
  const completion = rust.slice(completionStart, completionEnd);
  assert.match(completion, /prepared_oauth_client/);
  assert.match(completion, /finish_oauth_client_preparation/);
  assert.match(rust, /client\.warm_transport\(\)/);
});

test("login completion reports truthful stages instead of labeling all waits as callback validation", () => {
  const loginPage = source("../src/routes/login/+page.svelte");
  assert.match(rust, /"callback_verified"/);
  assert.match(rust, /"transport_ready"/);
  assert.match(rust, /"token_received"/);
  assert.match(rust, /"session_saved"/);
  assert.match(loginPage, /pixiv-login-progress/);
  assert.match(loginPage, /m\.login_completion_callback_verified\(\)/);
  assert.match(loginPage, /m\.login_completion_token_received\(\)/);
});

test("desktop login window closes as soon as the callback is captured", () => {
  const navigationStart = rust.indexOf(".on_navigation(move |url|");
  const navigationEnd = rust.indexOf(".on_new_window", navigationStart);
  const navigation = rust.slice(navigationStart, navigationEnd);
  const destroyAt = navigation.indexOf("window.destroy()");
  const completeAt = navigation.indexOf("complete_captured_login(");
  assert.ok(destroyAt >= 0, "Desktop callback does not close the private login window.");
  assert.ok(
    completeAt > destroyAt,
    "Desktop login window must close before waiting for token exchange.",
  );
});
