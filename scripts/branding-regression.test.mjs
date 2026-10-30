import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

async function pngSize(relativePath) {
  const bytes = await readFile(path.join(root, relativePath));
  assert.equal(bytes.subarray(0, 8).toString("hex"), "89504e470d0a1a0a", relativePath);
  return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
}

test("packaged desktop icons and favicon retain the mascot source and required sizes", async () => {
  const source = await pngSize("src-tauri/icons/pixnya-mascot-source.png");
  assert.equal(source[0], source[1]);
  assert.ok(source[0] >= 1024);
  for (const [name, size] of [["32x32", 32], ["128x128", 128], ["128x128@2x", 256], ["icon", 512]]) {
    assert.deepEqual(await pngSize(`src-tauri/icons/${name}.png`), [size, size]);
  }
  assert.deepEqual(await readFile(path.join(root, "static/favicon.png")),
    await readFile(path.join(root, "src-tauri/icons/128x128.png")));
  const config = JSON.parse(await read("src-tauri/tauri.conf.json"));
  for (const icon of config.bundle.icon) assert.ok((await readFile(path.join(root, "src-tauri", icon))).length > 0);
});

test("Android launchers have correctly sized density assets and adaptive mascot references", async () => {
  const res = "src-tauri/gen/android/app/src/main/res";
  for (const [density, size, foreground] of [["mdpi", 48, 108], ["hdpi", 72, 162], ["xhdpi", 96, 216], ["xxhdpi", 144, 324], ["xxxhdpi", 192, 432]]) {
    for (const name of ["ic_launcher", "ic_launcher_round", "ic_launcher_foreground"]) {
      const expected = name.endsWith("foreground") ? foreground : size;
      assert.deepEqual(await pngSize(`${res}/mipmap-${density}/${name}.png`), [expected, expected], `${density}/${name}`);
    }
  }
  for (const name of ["ic_launcher", "ic_launcher_round"]) {
    const adaptive = await read(`${res}/mipmap-anydpi-v26/${name}.xml`);
    assert.match(adaptive, /<adaptive-icon\b/);
    assert.match(adaptive, /@mipmap\/ic_launcher_foreground/);
    assert.match(adaptive, /@color\/ic_launcher_background/);
  }
  assert.match(await read(`${res}/values/ic_launcher_background.xml`), /name="ic_launcher_background"/);
  const manifest = await read("src-tauri/gen/android/app/src/main/AndroidManifest.xml");
  assert.match(manifest, /android:icon="@mipmap\/ic_launcher"/);
  assert.match(manifest, /android:roundIcon="@mipmap\/ic_launcher_round"/);
});

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
