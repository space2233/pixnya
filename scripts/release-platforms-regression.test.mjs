import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("only Windows x64 and both Android ABIs produce release artifacts", async () => {
  const workflow = await read(".github/workflows/release.yml");
  assert.doesNotMatch(workflow, /^  (?:windows-arm64|linux):/m);
  assert.doesNotMatch(workflow, /arm64-setup\.exe|\.AppImage/);
  assert.match(workflow, /check-windows-pe-architecture\.ps1 .*?-ExpectedArchitecture x64/);
  assert.match(workflow, /name: windows-x64/);
  assert.match(workflow, /abi: arm64-v8a/);
  assert.match(workflow, /abi: armeabi-v7a/);
  assert.match(workflow, /needs: \[preflight, rust-advisories, windows, android\]/);
});
