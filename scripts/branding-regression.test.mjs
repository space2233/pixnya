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
  const [diagnosticLog, networkDiagnostics, license] = await Promise.all([
    read("crates/diagnostic-log/src/lib.rs"),
    read("crates/network/src/diagnostics.rs"),
    read("LICENSE"),
  ]);
  const publicationText = `${diagnosticLog}\n${networkDiagnostics}\n${license}`;

  assert.match(publicationText, /PixNya diagnostics \(redacted\)/);
  assert.match(publicationText, /PixNya 连接诊断报告/);
  assert.match(license, /Copyright \(C\) 2026 PixNya contributors/);
  assert.doesNotMatch(publicationText, /Pixiv Client/i);
});
