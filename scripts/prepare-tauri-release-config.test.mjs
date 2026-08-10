import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const projectRoot = process.cwd();
const generator = path.join(projectRoot, "scripts", "prepare-tauri-release-config.mjs");
const workflow = readFileSync(
  path.join(projectRoot, ".github", "workflows", "release.yml"),
  "utf8",
);

const minisignKeyBytes = Buffer.alloc(42);
Buffer.from("Ed", "ascii").copy(minisignKeyBytes);
const minisignPublicKey = [
  "untrusted comment: minisign public key: 0000000000000000",
  minisignKeyBytes.toString("base64"),
  "",
].join("\n");
const encodedPublicKey = Buffer.from(minisignPublicKey, "utf8").toString("base64");

async function runGenerator(output, publicKey) {
  return execFileAsync(process.execPath, [generator, "--output", output], {
    cwd: projectRoot,
    env: { ...process.env, PIXNYA_UPDATER_PUBKEY: publicKey },
    windowsHide: true,
  });
}

test("release config keeps the validated updater public key in single-Base64 form", async () => {
  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "pixnya-release-config-"));
  const output = path.join(temporaryRoot, "tauri-release.conf.json");
  try {
    await runGenerator(output, encodedPublicKey);
    const config = JSON.parse(await readFile(output, "utf8"));

    assert.deepEqual(config, {
      bundle: { createUpdaterArtifacts: true },
      plugins: { updater: { pubkey: encodedPublicKey } },
    });
    assert.equal(
      Buffer.from(config.plugins.updater.pubkey, "base64").toString("utf8"),
      minisignPublicKey,
    );
    assert.deepEqual(await readdir(temporaryRoot), ["tauri-release.conf.json"]);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("release config rejects raw, double-Base64, and incomplete updater public keys", async () => {
  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "pixnya-release-config-invalid-"));
  try {
    for (const [label, publicKey] of [
      ["raw", minisignPublicKey],
      ["double", Buffer.from(encodedPublicKey, "utf8").toString("base64")],
      ["incomplete", Buffer.from("untrusted comment: missing key\n", "utf8").toString("base64")],
    ]) {
      const output = path.join(temporaryRoot, `${label}.json`);
      await assert.rejects(runGenerator(output, publicKey), (error) => {
        assert.match(error.stderr, /complete minisign public key/i);
        return true;
      });
    }
    assert.deepEqual(await readdir(temporaryRoot), []);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("desktop release jobs generate and consume the temporary config without changing Android", () => {
  const windowsJob = workflow.match(/\n  windows:[\s\S]*?\n  linux:/)?.[0];
  const linuxJob = workflow.match(/\n  linux:[\s\S]*?\n  android:/)?.[0];
  const androidJob = workflow.match(/\n  android:[\s\S]*?\n  draft-release:/)?.[0];

  assert.ok(windowsJob, "Windows release job is missing");
  assert.ok(linuxJob, "Linux release job is missing");
  assert.ok(androidJob, "Android release job is missing");
  for (const job of [windowsJob, linuxJob]) {
    assert.match(job, /node scripts\/prepare-tauri-release-config\.mjs --output/);
    assert.match(job, /npm run tauri -- build --config ["']?\$[^\s"']+/);
  }
  assert.doesNotMatch(androidJob, /prepare-tauri-release-config|android build[^\n]*--config/);
});
