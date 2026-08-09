import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");
const readGenerated = async (relativePath) => {
  try {
    return await read(relativePath);
  } catch (error) {
    if (error?.code === "ENOENT") {
      return null;
    }
    throw error;
  }
};

test("all user-visible package versions agree on the 0.29.0 feature release", async () => {
  const [workspace, packageJson, packageLock, tauri, androidProperties, androidIgnore, settings, readme] = await Promise.all([
    read("Cargo.toml"),
    read("package.json"),
    read("package-lock.json"),
    read("src-tauri/tauri.conf.json"),
    readGenerated("src-tauri/gen/android/app/tauri.properties"),
    read("src-tauri/gen/android/app/.gitignore"),
    read("src/routes/settings/+page.svelte"),
    read("README.md"),
  ]);
  assert.match(workspace, /version = "0\.29\.0"/);
  assert.equal(JSON.parse(packageJson).version, "0.29.0");
  assert.equal(JSON.parse(packageLock).version, "0.29.0");
  assert.equal(JSON.parse(tauri).version, "0.29.0");
  assert.match(androidIgnore, /^\/tauri\.properties$/m);
  if (androidProperties !== null) {
    assert.match(androidProperties, /tauri\.android\.versionName=0\.29\.0/);
    assert.match(androidProperties, /tauri\.android\.versionCode=29000/);
  }
  assert.match(settings, /appStatus\?\.version \?\? "0\.29\.0"/);
  assert.match(readme, /当前版本为 `0\.29\.0`/);
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
  const [workflow, script, runner] = await Promise.all([
    read(".github/workflows/linux.yml"),
    read("scripts/check-linux.sh"),
    read("scripts/run-test-suite.mjs"),
  ]);
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
  assert.match(workflow, /npm run test:quick/);
  assert.match(workflow, /bash scripts\/check-linux\.sh rust-only/);
  assert.match(script, /npm run test:rust/);
  assert.match(script, /npx tauri build --debug --no-bundle/);
  assert.match(runner, /"test", "--workspace"/);
});

test("formal releases are gated by main-branch full verification and signed artifact checks", async () => {
  const workflow = await read(".github/workflows/release.yml");
  assert.match(workflow, /Require the main release source/);
  assert.match(workflow, /refs\/heads\/main/);
  assert.match(workflow, /npm run test:full/);
  assert.match(workflow, /needs: preflight/);
  assert.match(workflow, /Signer #1 certificate SHA-256 digest/);
  assert.match(workflow, /check-android-arm64-apk\.ps1/);
  assert.match(workflow, /package: name='io\.github\.space2233\.pixnya'/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm dist\/android-latest\.json/);
  assert.match(workflow, /target_commitish: \$\{\{ github\.sha \}\}/);
  assert.match(workflow, /BUILD-PROVENANCE\.txt/);
});

test("throwaway thumbnail prototypes stay outside the formal application bundle", async () => {
  const packageJson = JSON.parse(await read("package.json"));
  assert.equal(packageJson.scripts["prototype:thumbnails"], undefined);
  for (const relativePath of [
    "src/routes/prototype/vite-smoke/+page.svelte",
    "src/routes/prototype/thumbnail-placeholders/+page.svelte",
    "public/prototype/thumbnail-placeholders.html",
  ]) {
    await assert.rejects(read(relativePath), (error) => error?.code === "ENOENT");
  }
});

test("unsupported notification and posting surfaces stay explicit and non-interactive", async () => {
  const [notifications, shell] = await Promise.all([
    read("src/routes/notifications/+page.svelte"),
    read("src/lib/components/AppShell.svelte"),
  ]);
  assert.match(notifications, /m\.notifications_unsupported_title\(\)/);
  assert.match(notifications, /m\.notifications_boundary_description\(\)/);
  assert.match(shell, /class="text-action" type="button" disabled/);
  assert.match(shell, /m\.shell_post_unavailable\(\)/);
});
