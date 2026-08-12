import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

test("Android ARM builds share the guarded stale-library cleanup", async () => {
  const [arm64, armv7, common, oauthEnvironment] = await Promise.all([
    source("scripts/build-android-arm64-debug.ps1"),
    source("scripts/build-android-armv7-debug.ps1"),
    source("scripts/android-build-common.ps1"),
    source("scripts/import-oauth-env.ps1"),
  ]);
  assert.match(arm64, /android-build-common\.ps1/);
  assert.match(armv7, /android-build-common\.ps1/);
  assert.match(arm64, /Remove-StaleTauriNativeLibraryLinks/);
  assert.match(armv7, /Remove-StaleTauriNativeLibraryLinks/);
  assert.match(common, /LinkType -ne 'SymbolicLink'/);
  assert.match(common, /StartsWith\(\$targetPrefix, \[System\.StringComparison\]::OrdinalIgnoreCase\)/);
  assert.match(oauthEnvironment, /\[Environment\]::GetEnvironmentVariable/);
  assert.doesNotMatch(oauthEnvironment, /Get-Item -Path "Env:\$_"/);
});

test("ARM64 and ARMv7 builds reject duplicate application libraries and unexpected ABIs", async () => {
  const [arm64, armv7, check] = await Promise.all([
    source("scripts/build-android-arm64-debug.ps1"),
    source("scripts/build-android-armv7-debug.ps1"),
    source("scripts/check-android-apk.ps1"),
  ]);
  assert.match(arm64, /check-android-apk\.ps1/);
  assert.match(arm64, /-ExpectedAbi 'arm64-v8a'/);
  assert.match(armv7, /check-android-apk\.ps1/);
  assert.match(armv7, /-ExpectedAbi 'armeabi-v7a'/);
  assert.match(armv7, /app\\build\\outputs\\apk\\arm\\debug\\app-arm-debug\.apk/);
  assert.match(check, /\$MaximumBytes = 90MB/);
  assert.match(check, /\$applicationLibraries\.Count -ne 1/);
  assert.match(check, /\$unexpectedAbis\.Count -gt 0/);
});
