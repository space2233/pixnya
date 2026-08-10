import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("the application shell uses PixNya as its product brand", async () => {
  const appShell = await read("src/lib/components/AppShell.svelte");

  assert.match(appShell, /class="side-brand"[\s\S]*?<strong>PixNya<\/strong>/);
  assert.doesNotMatch(
    appShell,
    /class="side-brand"[\s\S]*?<strong>pixiv<\/strong>\s*<span>client<\/span>/i,
  );
});

test("diagnostic exports and licensing use the PixNya product name", async () => {
  const [diagnosticLog, networkDiagnostics, license, thirdPartyNotices] = await Promise.all([
    read("crates/diagnostic-log/src/lib.rs"),
    read("crates/network/src/diagnostics.rs"),
    read("LICENSE"),
    read("THIRD_PARTY_NOTICES.md"),
  ]);
  const publicationText = `${diagnosticLog}\n${networkDiagnostics}\n${license}\n${thirdPartyNotices}`;

  assert.match(publicationText, /PixNya diagnostics \(redacted\)/);
  assert.doesNotMatch(networkDiagnostics, /pub text: String|连接诊断报告/);
  assert.match(license, /GNU GENERAL PUBLIC LICENSE/);
  assert.match(thirdPartyNotices, /Copyright \(C\) 2026 PixNya contributors/);
  assert.doesNotMatch(publicationText, /Pixiv Client/i);
});
