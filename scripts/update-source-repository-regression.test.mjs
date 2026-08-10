import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import {
  releaseRepository,
  validateReleaseBaseUrl,
} from "./release-url-policy.mjs";

const execFileAsync = promisify(execFile);
const root = process.cwd();
const desktopGenerator = path.join(root, "scripts", "generate-desktop-update-manifest.mjs");
const androidGenerator = path.join(root, "scripts", "generate-android-update-manifest.mjs");

test("desktop and Android manifests share one strict GitHub Release URL policy", () => {
  assert.equal(releaseRepository(new Map()), "space2233/pixnya");
  assert.equal(releaseRepository(new Map([["repository", "space2233/pixnya-releases"]])), "space2233/pixnya-releases");
  assert.throws(() => releaseRepository(new Map([["repository", "space2233/pixnya/extra"]])));
  assert.throws(() => releaseRepository(new Map([["repository", "-invalid/repository"]])));

  const repository = "space2233/pixnya";
  assert.doesNotThrow(() => validateReleaseBaseUrl(
    new URL("https://github.com/space2233/pixnya/releases/download/v0.29.0/"),
    repository,
  ));
  for (const candidate of [
    "http://github.com/space2233/pixnya/releases/download/v0.29.0/",
    "https://github.com/space2233/other/releases/download/v0.29.0/",
    "https://user@github.com/space2233/pixnya/releases/download/v0.29.0/",
    "https://github.com/space2233/pixnya/releases/download/v0.29.0/?asset=1",
    "https://github.com/space2233/pixnya/releases/download/v0.29.0/extra/",
  ]) {
    assert.throws(() => validateReleaseBaseUrl(new URL(candidate), repository));
  }
});

async function runGenerator(script, argumentsList) {
  return execFileAsync(process.execPath, [script, ...argumentsList], {
    cwd: root,
    windowsHide: true,
  });
}

test("manifest generators default to pixnya and accept an explicit public release repository", async () => {
  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "pixnya-update-source-"));
  try {
    const { version } = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
    const windowsArchive = path.join(temporaryRoot, `PixNya_${version}_x64-setup.nsis.zip`);
    const linuxArchive = path.join(temporaryRoot, `PixNya_${version}_amd64.AppImage.tar.gz`);
    const androidApk = path.join(temporaryRoot, `pixnya-${version}-android-arm64-v8a.apk`);
    const signature = path.join(temporaryRoot, "updater.sig");
    const encodedSignature = Buffer.from(
      "untrusted comment: test signature\nAAAA\ntrusted comment: test\nAAAA\n",
      "utf8",
    ).toString("base64");
    await Promise.all([
      writeFile(windowsArchive, "windows-archive"),
      writeFile(linuxArchive, "linux-archive"),
      writeFile(androidApk, "android-apk"),
      writeFile(signature, encodedSignature),
    ]);

    const defaultOutput = path.join(temporaryRoot, "default-latest.json");
    await runGenerator(desktopGenerator, [
      "--windows-archive", windowsArchive,
      "--windows-signature", signature,
      "--linux-archive", linuxArchive,
      "--linux-signature", signature,
      "--base-url", `https://github.com/space2233/pixnya/releases/download/v${version}/`,
      "--output", defaultOutput,
    ]);
    const defaultManifest = JSON.parse(await readFile(defaultOutput, "utf8"));
    assert.match(defaultManifest.platforms["windows-x86_64"].url, /space2233\/pixnya\/releases/);

    const repository = "space2233/pixnya-releases";
    const releaseBase = `https://github.com/${repository}/releases/download/v${version}/`;
    const desktopOutput = path.join(temporaryRoot, "alternate-latest.json");
    await runGenerator(desktopGenerator, [
      "--repository", repository,
      "--windows-archive", windowsArchive,
      "--windows-signature", signature,
      "--linux-archive", linuxArchive,
      "--linux-signature", signature,
      "--base-url", releaseBase,
      "--output", desktopOutput,
    ]);
    const desktopManifest = JSON.parse(await readFile(desktopOutput, "utf8"));
    assert.ok(desktopManifest.platforms["linux-x86_64"].url.startsWith(releaseBase));

    const androidOutput = path.join(temporaryRoot, "alternate-android-latest.json");
    await runGenerator(androidGenerator, [
      "--repository", repository,
      "--arm64", androidApk,
      "--certificate-sha256", "ab".repeat(32),
      "--base-url", releaseBase,
      "--output", androidOutput,
    ]);
    const androidManifest = JSON.parse(await readFile(androidOutput, "utf8"));
    assert.ok(androidManifest.artifacts[0].url.startsWith(releaseBase));

    await assert.rejects(
      runGenerator(androidGenerator, [
        "--repository", repository,
        "--arm64", androidApk,
        "--certificate-sha256", "ab".repeat(32),
        "--base-url", `https://github.com/space2233/pixnya/releases/download/v${version}/`,
        "--output", path.join(temporaryRoot, "mismatched.json"),
      ]),
      (error) => error?.stderr?.includes(`/${repository}/releases/download/`),
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});
