import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(path, import.meta.url), "utf8");

test("reuse-first cleanup is dry-run by default and stays inside target", async () => {
  const source = await read("./cleanup-target-reuse-first.ps1");

  assert.match(source, /\[switch\]\$Execute/);
  assert.match(source, /if \(-not \$Execute\)/);
  assert.match(source, /StartsWith\(\$targetPrefix/);
  assert.match(source, /FileAttributes\]::ReparsePoint/);
  assert.match(source, /Remove-Item -LiteralPath/);
  assert.doesNotMatch(source, /cargo clean/i);
  assert.doesNotMatch(source, /git clean/i);
});

test("cleanup only matches the retired main crate names", async () => {
  const source = await read("./cleanup-target-reuse-first.ps1");

  assert.match(source, /\^pixiv_client_lib-/);
  assert.match(source, /\^pixiv-client-\[0-9a-f\]/);
  assert.doesNotMatch(source, /\*pixiv\*/);
  assert.match(source, /armv7-linux-androideabi/);
  assert.match(source, /aarch64-linux-android/);
});

test("artifact and CI builds disable Cargo incremental output", async () => {
  const files = await Promise.all([
    read("./build-desktop-debug.ps1"),
    read("./build-android-arm64-debug.ps1"),
    read("./build-android-armv7-debug.ps1"),
  ]);
  const linux = await read("./check-linux.sh");

  for (const source of files) {
    assert.match(source, /\$env:CARGO_INCREMENTAL = '0'/);
    assert.match(source, /audit-target-storage\.ps1/);
  }
  assert.match(linux, /export CARGO_INCREMENTAL=0/);
});
