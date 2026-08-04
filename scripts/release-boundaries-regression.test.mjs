import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("all user-visible package versions agree on the 0.28.2 patch release", async () => {
  const [workspace, packageJson, packageLock, tauri, androidProperties, settings, readme] = await Promise.all([
    read("Cargo.toml"),
    read("package.json"),
    read("package-lock.json"),
    read("src-tauri/tauri.conf.json"),
    read("src-tauri/gen/android/app/tauri.properties"),
    read("src/routes/settings/+page.svelte"),
    read("README.md"),
  ]);
  assert.match(workspace, /version = "0\.28\.2"/);
  assert.equal(JSON.parse(packageJson).version, "0.28.2");
  assert.equal(JSON.parse(packageLock).version, "0.28.2");
  assert.equal(JSON.parse(tauri).version, "0.28.2");
  assert.match(androidProperties, /tauri\.android\.versionName=0\.28\.2/);
  assert.match(androidProperties, /tauri\.android\.versionCode=28002/);
  assert.match(settings, /appStatus\?\.version \?\? "0\.28\.2"/);
  assert.match(readme, /当前版本为 `0\.28\.2`/);
});

test("Android releases ARM64 while retaining ARMv7 as a deferred manual target", async () => {
  const [packageJson, arm64, armv7, workflow, manifestGenerator] = await Promise.all([
    read("package.json"),
    read("scripts/build-android-arm64-debug.ps1"),
    read("scripts/build-android-armv7-debug.ps1"),
    read(".github/workflows/release.yml"),
    read("scripts/generate-android-update-manifest.mjs"),
  ]);
  const scripts = JSON.parse(packageJson).scripts;
  assert.ok(scripts["build:android:arm64:debug"]);
  assert.ok(scripts["build:android:armv7:debug"]);
  assert.match(arm64, /--target aarch64 --split-per-abi/);
  assert.match(armv7, /--target armv7 --split-per-abi/);
  assert.match(workflow, /tauriTarget: aarch64/);
  assert.doesNotMatch(workflow, /tauriTarget: armv7/);
  assert.doesNotMatch(workflow, /--armv7/);
  assert.match(manifestGenerator, /optionalArmv7/);
});

test("Linux verification compiles the actual Tauri desktop target", async () => {
  const [workflow, script] = await Promise.all([
    read(".github/workflows/linux.yml"),
    read("scripts/check-linux.sh"),
  ]);
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
  assert.match(workflow, /bash scripts\/check-linux\.sh/);
  assert.match(script, /cargo test --workspace/);
  assert.match(script, /npx tauri build --debug --no-bundle/);
});

test("unsupported notification and posting surfaces stay explicit and non-interactive", async () => {
  const [notifications, shell] = await Promise.all([
    read("src/routes/notifications/+page.svelte"),
    read("src/lib/components/AppShell.svelte"),
  ]);
  assert.match(notifications, /不提供 Pixiv 站内通知/);
  assert.match(notifications, /Cookie 不会交给普通页面或数据接口/);
  assert.match(shell, /class="text-action" type="button" disabled/);
  assert.match(shell, /PixNya 暂不包含投稿功能/);
});
