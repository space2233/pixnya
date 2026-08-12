import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

const execFileAsync = promisify(execFile);
const projectRoot = path.resolve(import.meta.dirname, "..");
const checker = path.join(projectRoot, "scripts", "check-android-apk.ps1");
const powershell = process.platform === "win32" ? "powershell.exe" : "pwsh";

function crc32(bytes) {
  let value = 0xffffffff;
  for (const byte of bytes) {
    value ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      value = (value >>> 1) ^ (0xedb88320 & -(value & 1));
    }
  }
  return (value ^ 0xffffffff) >>> 0;
}

function storedZip(entries) {
  const localParts = [];
  const centralParts = [];
  let localOffset = 0;

  for (const [name, content] of entries) {
    const nameBytes = Buffer.from(name, "utf8");
    const digest = crc32(content);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(digest, 14);
    local.writeUInt32LE(content.length, 18);
    local.writeUInt32LE(content.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    localParts.push(local, nameBytes, content);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt32LE(digest, 16);
    central.writeUInt32LE(content.length, 20);
    central.writeUInt32LE(content.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(localOffset, 42);
    centralParts.push(central, nameBytes);

    localOffset += local.length + nameBytes.length + content.length;
  }

  const centralDirectory = Buffer.concat(centralParts);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralDirectory.length, 12);
  end.writeUInt32LE(localOffset, 16);
  return Buffer.concat([...localParts, centralDirectory, end]);
}

function elfFixture({ elfClass, machine, dataEncoding = 1, magic = true }) {
  const bytes = Buffer.alloc(64);
  if (magic) bytes.set([0x7f, 0x45, 0x4c, 0x46], 0);
  bytes[4] = elfClass;
  bytes[5] = dataEncoding;
  bytes[6] = 1;
  bytes.writeUInt16LE(machine, 18);
  return bytes;
}

async function check(file, abi) {
  return execFileAsync(powershell, [
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    checker,
    "-ApkPath",
    file,
    "-ExpectedAbi",
    abi,
  ], { cwd: projectRoot });
}

test("the Android APK checker accepts ELF payloads matching each ABI", async (context) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "pixnya-apk-elf-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const arm64 = path.join(directory, "arm64.apk");
  const armv7 = path.join(directory, "armv7.apk");
  await writeFile(arm64, storedZip([
    ["lib/arm64-v8a/libpixnya_lib.so", elfFixture({ elfClass: 2, machine: 0xb7 })],
    ["lib/arm64-v8a/libdependency.so", elfFixture({ elfClass: 2, machine: 0xb7 })],
  ]));
  await writeFile(armv7, storedZip([
    ["lib/armeabi-v7a/libpixnya_lib.so", elfFixture({ elfClass: 1, machine: 0x28 })],
    ["lib/armeabi-v7a/libdependency.so", elfFixture({ elfClass: 1, machine: 0x28 })],
  ]));

  await assert.doesNotReject(check(arm64, "arm64-v8a"));
  await assert.doesNotReject(check(armv7, "armeabi-v7a"));
});

test("the Android APK checker rejects a native library placed under the wrong ABI directory", async (context) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "pixnya-apk-elf-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const substituted = path.join(directory, "substituted.apk");
  await writeFile(substituted, storedZip([
    ["lib/armeabi-v7a/libpixnya_lib.so", elfFixture({ elfClass: 2, machine: 0xb7 })],
  ]));

  await assert.rejects(check(substituted, "armeabi-v7a"), /ELF class|ELF machine/i);
});

test("the Android APK checker rejects malformed and non-little-endian secondary libraries", async (context) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "pixnya-apk-elf-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const malformed = path.join(directory, "malformed.apk");
  const bigEndian = path.join(directory, "big-endian.apk");
  const validApplication = elfFixture({ elfClass: 1, machine: 0x28 });
  await writeFile(malformed, storedZip([
    ["lib/armeabi-v7a/libpixnya_lib.so", validApplication],
    ["lib/armeabi-v7a/libdependency.so", elfFixture({ elfClass: 1, machine: 0x28, magic: false })],
  ]));
  await writeFile(bigEndian, storedZip([
    ["lib/armeabi-v7a/libpixnya_lib.so", validApplication],
    ["lib/armeabi-v7a/libdependency.so", elfFixture({ elfClass: 1, machine: 0x28, dataEncoding: 2 })],
  ]));

  await assert.rejects(check(malformed, "armeabi-v7a"), /ELF magic/i);
  await assert.rejects(check(bigEndian, "armeabi-v7a"), /little-endian ELF/i);
});
