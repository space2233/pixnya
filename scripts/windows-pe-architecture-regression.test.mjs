import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

const execFileAsync = promisify(execFile);
const projectRoot = path.resolve(import.meta.dirname, "..");
const checker = path.join(projectRoot, "scripts", "check-windows-pe-architecture.ps1");
const powershell = process.platform === "win32" ? "powershell.exe" : "pwsh";

function peFixture(machine, subsystem = 2, optionalHeaderMagic = 0x20b) {
  const peOffset = 0x80;
  const bytes = Buffer.alloc(0x200);
  bytes.writeUInt16LE(0x5a4d, 0);
  bytes.writeUInt32LE(peOffset, 0x3c);
  bytes.write("PE\0\0", peOffset, "ascii");
  bytes.writeUInt16LE(machine, peOffset + 4);
  bytes.writeUInt16LE(0xf0, peOffset + 20);
  bytes.writeUInt16LE(optionalHeaderMagic, peOffset + 24);
  bytes.writeUInt16LE(subsystem, peOffset + 24 + 68);
  return bytes;
}

async function check(file, architecture) {
  return execFileAsync(powershell, [
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    checker,
    "-Executable",
    file,
    "-ExpectedArchitecture",
    architecture,
  ], { cwd: projectRoot });
}

test("the Windows release payload checker accepts native ARM64 and x64 GUI executables", async (context) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "pixnya-pe-check-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const arm64 = path.join(directory, "pixnya-arm64.exe");
  const x64 = path.join(directory, "pixnya-x64.exe");
  await writeFile(arm64, peFixture(0xaa64));
  await writeFile(x64, peFixture(0x8664));

  await assert.doesNotReject(check(arm64, "arm64"));
  await assert.doesNotReject(check(x64, "x64"));
});

test("the Windows release payload checker rejects architecture substitution and non-GUI payloads", async (context) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "pixnya-pe-check-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const x64 = path.join(directory, "substituted.exe");
  const consoleArm64 = path.join(directory, "console.exe");
  await writeFile(x64, peFixture(0x8664));
  await writeFile(consoleArm64, peFixture(0xaa64, 3));

  await assert.rejects(check(x64, "arm64"), /Expected ARM64 PE machine 0xAA64/i);
  await assert.rejects(check(consoleArm64, "arm64"), /Windows GUI subsystem/i);
});

test("the Windows release payload checker rejects malformed PE headers", async (context) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "pixnya-pe-check-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const malformed = path.join(directory, "malformed.exe");
  await writeFile(malformed, Buffer.from("not a portable executable"));

  await assert.rejects(check(malformed, "arm64"), /valid PE executable/i);
});
