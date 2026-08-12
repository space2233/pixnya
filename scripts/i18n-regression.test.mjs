import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("the message catalog exposes the same keys for all three supported locales", async () => {
  const [settingsSource, simplifiedSource, traditionalSource, englishSource] = await Promise.all([
    read("project.inlang/settings.json"),
    read("messages/zh-CN.json"),
    read("messages/zh-TW.json"),
    read("messages/en-US.json"),
  ]);
  const settings = JSON.parse(settingsSource);
  const catalogs = [simplifiedSource, traditionalSource, englishSource].map(JSON.parse);
  const messageKeys = (catalog) => Object.keys(catalog).filter((key) => key !== "$schema").sort();

  assert.equal(settings.baseLocale, "zh-CN");
  assert.deepEqual(settings.locales, ["zh-CN", "zh-TW", "en-US"]);
  assert.deepEqual(messageKeys(catalogs[1]), messageKeys(catalogs[0]));
  assert.deepEqual(messageKeys(catalogs[2]), messageKeys(catalogs[0]));
  assert.equal(catalogs[1].language_traditional_chinese, "繁體中文");
  assert.equal(catalogs[2].navigation_settings, "Settings");
  for (const catalog of catalogs) {
    assert.equal(Object.keys(catalog).some((key) => key.startsWith("media_risk_")), false);
    assert.equal(Object.keys(catalog).some((key) => key.startsWith("network_media_warning_")), false);
    assert.equal("network_restore_media_warning" in catalog, false);
  }
});

test("application source keeps user-facing copy in the message catalogs", async () => {
  const files = await sourceFiles(path.join(root, "src"));
  const hardcoded = [];
  for (const file of files) {
    const source = await readFile(file, "utf8");
    if (/[\u3400-\u9fff]/u.test(source)) hardcoded.push(path.relative(root, file));
  }
  assert.deepEqual(hardcoded, []);
});

test("native network probes return structured data instead of fixed-language summaries", async () => {
  const [gateway, types, page] = await Promise.all([
    read("crates/network/src/gateway.rs"),
    read("src/lib/types.ts"),
    read("src/routes/settings/network/+page.svelte"),
  ]);

  assert.doesNotMatch(gateway, /[\u3400-\u9fff]/u);
  assert.doesNotMatch(gateway, /dns_source|tls_summary/);
  assert.doesNotMatch(types, /dnsSource|tlsSummary/);
  assert.match(page, /ConnectionModePicker/);
  assert.match(page, /ConnectionDiagnosticReport/);
  assert.doesNotMatch(page, /dnsSource|tlsSummary/);
});

test("system language mapping distinguishes Chinese scripts and defaults to English", async () => {
  const localeModuleUrl = pathToFileURL(path.join(root, "src/lib/i18n-locale.ts")).href;
  const { mapSystemLanguages } = await import(localeModuleUrl);

  assert.equal(mapSystemLanguages(["zh-Hant-HK"]), "zh-TW");
  assert.equal(mapSystemLanguages(["zh_TW"]), "zh-TW");
  assert.equal(mapSystemLanguages(["zh-MO"]), "zh-TW");
  assert.equal(mapSystemLanguages(["zh-Hans-CN"]), "zh-CN");
  assert.equal(mapSystemLanguages(["zh-SG"]), "zh-CN");
  assert.equal(mapSystemLanguages(["ja-JP", "en-US"]), "en-US");
  assert.equal(mapSystemLanguages([]), "en-US");
});

test("language preference is local-only and shared chrome consumes generated messages", async () => {
  const [i18n, layout, navigation, shell, thumbnail, settings] = await Promise.all([
    read("src/lib/i18n.ts"),
    read("src/routes/+layout.svelte"),
    read("src/lib/navigation.ts"),
    read("src/lib/components/AppShell.svelte"),
    read("src/lib/components/ArtworkThumbnail.svelte"),
    read("src/routes/settings/interface/+page.svelte"),
  ]);

  assert.match(i18n, /pixiv-client\.interface-language/);
  assert.match(i18n, /defineCustomClientStrategy\("custom-pixnya"/);
  assert.match(i18n, /window\.location\.reload\(\)/);
  assert.match(layout, /initializeI18n\(\)/);
  assert.match(navigation, /m\.navigation_home/);
  assert.match(shell, /m\.shell_search_placeholder\(\)/);
  assert.match(thumbnail, /m\.thumbnail_unavailable\(\)/);
  assert.match(settings, /setLanguagePreference\(language\)/);
  assert.match(settings, /<option value="zh-TW">/);
});

test("generated messages switch at runtime from PixNya's stored preference", async () => {
  const values = new Map([["pixiv-client.interface-language", "en-US"]]);
  globalThis.localStorage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
  globalThis.window = {
    location: {
      href: "http://127.0.0.1:1420/settings",
      reload: () => undefined,
    },
  };

  const i18nUrl = `${pathToFileURL(path.join(root, "src/lib/i18n.ts")).href}?runtime-switch`;
  const { m } = await import(i18nUrl);
  assert.equal(m.navigation_settings(), "Settings");

  values.set("pixiv-client.interface-language", "zh-TW");
  assert.equal(m.navigation_settings(), "設定");
});

async function sourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      if (fullPath.endsWith(path.join("src", "lib", "paraglide"))) continue;
      files.push(...await sourceFiles(fullPath));
    } else if (entry.name.endsWith(".svelte") || entry.name.endsWith(".ts")) {
      files.push(fullPath);
    }
  }
  return files;
}
