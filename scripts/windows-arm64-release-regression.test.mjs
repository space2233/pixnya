import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("the signed release workflow builds and verifies a native Windows ARM64 payload", async () => {
  const workflow = await read(".github/workflows/release.yml");

  assert.match(workflow, /^  windows-arm64:\s*$/m);
  assert.match(workflow, /windows-arm64:[\s\S]*?runs-on: windows-11-arm/);
  assert.match(workflow, /windows-arm64:[\s\S]*?targets: aarch64-pc-windows-msvc/);
  assert.match(
    workflow,
    /windows-arm64:[\s\S]*?tauri -- build[^\n]*--target aarch64-pc-windows-msvc/,
  );
  assert.match(
    workflow,
    /windows-arm64:[\s\S]*?check-windows-pe-architecture\.ps1[\s\S]*?-ExpectedArchitecture arm64/,
  );
  assert.match(workflow, /windows-arm64:[\s\S]*?name: windows-arm64/);
  assert.match(
    workflow,
    /windows-arm64:[\s\S]*?target\/aarch64-pc-windows-msvc\/release\/bundle\/nsis\/\*\.exe/,
  );
});

test("the release finalizer keeps Windows x64 and ARM64 installers separate", async () => {
  const workflow = await read(".github/workflows/release.yml");

  assert.match(
    workflow,
    /draft-release:[\s\S]*?needs: \[preflight, rust-advisories, windows, windows-arm64, linux, android\]/,
  );
  assert.match(workflow, /needs\.windows-arm64\.result == 'success'/);
  assert.match(workflow, /needs\.windows-arm64\.result == 'skipped'/);
  assert.match(workflow, /WINDOWS_X64_ARCHIVE=/);
  assert.match(workflow, /WINDOWS_ARM64_ARCHIVE=/);
  assert.match(workflow, /--windows-arm64-archive "\$WINDOWS_ARM64_ARCHIVE"/);
  assert.match(workflow, /--windows-arm64-signature "\$\{WINDOWS_ARM64_ARCHIVE\}\.sig"/);
  assert.match(workflow, /WINDOWS_ARM64_ARCHIVE.*\.sig/s);
  assert.match(workflow, /Expected exactly 10 public release files/);
});
