import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

function createStorage() {
  const values = new Map();
  return {
    get length() {
      return values.size;
    },
    getItem(key) {
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
    key(index) {
      return [...values.keys()][index] ?? null;
    },
    clear() {
      values.clear();
    },
  };
}

const events = [];

globalThis.localStorage = createStorage();
globalThis.sessionStorage = createStorage();
globalThis.window = {
  dispatchEvent(event) {
    events.push(event.type);
  },
};
globalThis.document = { documentElement: { dataset: {} } };

const preferences = await import("../src/lib/preferences.ts");
const localData = await import("../src/lib/local-data.ts");

test("safe connection modes persist but compatibility mode remains temporary", () => {
  assert.equal(preferences.readPreferredConnectionMode(), "standard");
  preferences.writePreferredConnectionMode("ech");
  assert.equal(preferences.readPreferredConnectionMode(), "ech");

  preferences.writePreferredConnectionMode("compatible");
  assert.equal(preferences.readPreferredConnectionMode(), "ech");
});

test("unsafe connection warnings can be suppressed and restored without persisting compatible mode", () => {
  assert.equal(preferences.readUnsafeConnectionWarningSuppressed(), false);
  preferences.writeUnsafeConnectionWarningSuppressed(true);
  assert.equal(preferences.readUnsafeConnectionWarningSuppressed(), true);
  assert.equal(preferences.readPreferredConnectionMode(), "ech");
  preferences.writeUnsafeConnectionWarningSuppressed(false);
  assert.equal(preferences.readUnsafeConnectionWarningSuppressed(), false);

  const networkSettings = readFileSync(
    new URL("../src/routes/settings/network/+page.svelte", import.meta.url),
    "utf8",
  );
  const login = readFileSync(new URL("../src/routes/login/+page.svelte", import.meta.url), "utf8");
  assert.match(networkSettings, /m\.login_warning_suppress\(\)/);
  assert.match(networkSettings, /m\.network_restore_warning\(\)/);
  assert.match(networkSettings, /readUnsafeConnectionWarningSuppressed/);
  assert.match(login, /m\.login_warning_suppress\(\)/);
  assert.match(login, /readUnsafeConnectionWarningSuppressed/);
});

test("ECH media fallback warning stays suppressed across sessions and can be restored", () => {
  assert.equal(preferences.readInsecureMediaWarningSuppressed(), false);
  preferences.writeInsecureMediaWarningSuppressed(true);
  assert.equal(preferences.readInsecureMediaWarningSuppressed(), true);

  const appShell = readFileSync(
    new URL("../src/lib/components/AppShell.svelte", import.meta.url),
    "utf8",
  );
  const networkSettings = readFileSync(
    new URL("../src/routes/settings/network/+page.svelte", import.meta.url),
    "utf8",
  );
  assert.match(appShell, /m\.media_risk_suppress\(\)/);
  assert.match(appShell, /readInsecureMediaWarningSuppressed/);
  assert.match(appShell, /confirmInsecureMediaFallback\(false, true\)/);
  assert.match(networkSettings, /m\.network_restore_media_warning\(\)/);
  assert.match(networkSettings, /writeInsecureMediaWarningSuppressed\(false\)/);

  preferences.writeInsecureMediaWarningSuppressed(false);
  assert.equal(preferences.readInsecureMediaWarningSuppressed(), false);
});

test("interface settings persist and reduced motion applies immediately", () => {
  preferences.writeDesktopSidebarExpanded(false);
  assert.equal(preferences.readDesktopSidebarExpanded(), false);

  preferences.writeReducedMotion(true);
  assert.equal(preferences.readReducedMotion(), true);
  assert.equal(document.documentElement.dataset.reducedMotion, "true");
  assert.ok(events.every((event) => event === preferences.PREFERENCES_CHANGED_EVENT));
});

test("R18 visibility is opt-in, persists locally, and controls every restricted-content surface", () => {
  preferences.writeR18DefaultVisible(false);
  assert.equal(preferences.readR18DefaultVisible(), false);
  preferences.writeR18DefaultVisible(true);
  assert.equal(preferences.readR18DefaultVisible(), true);

  const settings = readFileSync(new URL("../src/routes/settings/+page.svelte", import.meta.url), "utf8");
  const artworkCard = readFileSync(new URL("../src/lib/components/ArtworkCard.svelte", import.meta.url), "utf8");
  const artworkDetail = readFileSync(new URL("../src/routes/artworks/[id]/+page.svelte", import.meta.url), "utf8");
  const novelCard = readFileSync(new URL("../src/lib/components/NovelCard.svelte", import.meta.url), "utf8");
  const novelDetail = readFileSync(new URL("../src/routes/novels/[id]/+page.svelte", import.meta.url), "utf8");
  const userPreview = readFileSync(new URL("../src/lib/components/UserPreviewCard.svelte", import.meta.url), "utf8");

  assert.match(settings, /m\.settings_r18\(\)/);
  assert.match(settings, /writeR18DefaultVisible/);
  for (const surface of [artworkCard, artworkDetail, novelCard, novelDetail, userPreview]) {
    assert.match(surface, /\$r18DefaultVisible/);
  }
});

test("local-data clearing removes only this application's frontend namespace", () => {
  localStorage.clear();
  sessionStorage.clear();
  localStorage.setItem("pixiv-client.search-history.v1", "search");
  localStorage.setItem("pixiv-client:novel-progress:42", "0.5");
  localStorage.setItem("another-app.keep", "safe");
  sessionStorage.setItem("pixiv-client:temporary", "secret");
  sessionStorage.setItem("another-app.session", "safe");

  const report = localData.clearFrontendLocalData();

  assert.equal(report.localKeysRemoved, 2);
  assert.equal(report.sessionKeysRemoved, 1);
  assert.equal(localStorage.getItem("another-app.keep"), "safe");
  assert.equal(sessionStorage.getItem("another-app.session"), "safe");
  assert.equal(document.documentElement.dataset.reducedMotion, "false");
});

test("settings center owns the connection entry and uses the corrected cog", () => {
  const page = readFileSync(new URL("../src/routes/settings/+page.svelte", import.meta.url), "utf8");
  const icon = readFileSync(new URL("../src/lib/components/Icon.svelte", import.meta.url), "utf8");

  assert.match(page, /href="\/settings\/network"/);
  assert.match(page, /m\.settings_account\(\)/);
  assert.match(page, /m\.settings_storage\(\)/);
  assert.match(icon, /settings:\s*\[\s*"M12\.22 2h-/);
  assert.doesNotMatch(icon, /M19\.4 15a1\.7/);
});

test("connection settings expose redacted three-target diagnostics", () => {
  const page = readFileSync(
    new URL("../src/routes/settings/network/+page.svelte", import.meta.url),
    "utf8",
  );
  const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

  assert.match(page, /run_connection_diagnostics/);
  assert.match(page, /m\.network_diagnostic_title\(\)/);
  assert.match(page, /navigator\.clipboard\.writeText\(diagnosticReport\.text\)/);
  assert.match(page, /m\.network_report_privacy\(\)/);
  assert.match(backend, /async fn run_connection_diagnostics/);
  assert.match(backend, /run_connection_diagnostics,/);
});

test("storage settings expose isolated media-cache statistics and confirmed clearing", () => {
  const page = readFileSync(new URL("../src/routes/settings/+page.svelte", import.meta.url), "utf8");
  const api = readFileSync(new URL("../src/lib/pixiv-api.ts", import.meta.url), "utf8");
  const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const cache = readFileSync(
    new URL("../crates/media-cache/src/lib.rs", import.meta.url),
    "utf8",
  );

  assert.match(page, /m\.settings_media_cache\(\)/);
  assert.match(page, /m\.settings_media_cache_summary\(/);
  assert.match(page, /m\.settings_cache_dialog_description\(\)/);
  assert.match(api, /invoke<MediaCacheStats>\("clear_media_cache", \{ confirmed: true \}\)/);
  assert.match(backend, /async fn get_media_cache_stats/);
  assert.match(backend, /async fn clear_media_cache/);
  assert.match(backend, /if !confirmed/);
  assert.match(cache, /CacheScope::Verified/);
  assert.match(cache, /CacheScope::Insecure/);
  assert.match(cache, /trim_to_capacity/);
});

test("privacy settings require typed confirmation and clear every owned data layer", () => {
  const page = readFileSync(new URL("../src/routes/settings/+page.svelte", import.meta.url), "utf8");
  const api = readFileSync(new URL("../src/lib/pixiv-api.ts", import.meta.url), "utf8");
  const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const android = readFileSync(androidPackagePath("LoginWebViewPlugin.kt"), "utf8");

  assert.match(page, /m\.settings_clear_all\(\)/);
  assert.match(page, /localDataConfirmation !== m\.settings_clear_confirmation_word\(\)/);
  assert.match(page, /clearLocalData\(LOCAL_DATA_CLEAR_PROTOCOL\)/);
  assert.match(api, /invoke<LocalDataClearReport>\("clear_local_data"/);
  assert.match(backend, /if request\.confirmation != "CLEAR_LOCAL_DATA"/);
  assert.match(backend, /delete_refresh_token\(&app\)/);
  assert.match(backend, /library\.clear\(\)/);
  assert.match(backend, /MediaCache::open[\s\S]*?\.clear\(\)/);
  assert.match(android, /fun clearLocalWebData/);
  assert.match(android, /removeAllCookies/);
  assert.match(android, /clearCache\(true\)/);
  assert.match(android, /deleteEntry\(KEY_ALIAS\)/);
});
