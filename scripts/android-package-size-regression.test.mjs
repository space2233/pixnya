import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const source = (path) => readFile(new URL(path, root), "utf8");

test("ARM64 builds remove stale Tauri Rust library links before Gradle packaging", async () => {
  const build = await source("scripts/build-android-arm64-debug.ps1");
  assert.match(build, /Remove-StaleTauriNativeLibraryLinks/);
  assert.match(build, /LinkType -ne 'SymbolicLink'/);
  assert.match(build, /StartsWith\(\$targetPrefix, \[System\.StringComparison\]::OrdinalIgnoreCase\)/);
});

test("ARM64 builds reject duplicate application libraries and oversized APKs", async () => {
  const [build, check] = await Promise.all([
    source("scripts/build-android-arm64-debug.ps1"),
    source("scripts/check-android-arm64-apk.ps1"),
  ]);
  assert.match(build, /check-android-arm64-apk\.ps1/);
  assert.match(check, /\$MaximumBytes = 90MB/);
  assert.match(check, /\$applicationLibraries\.Count -ne 1/);
  assert.match(check, /\$unexpectedAbis\.Count -gt 0/);
});
