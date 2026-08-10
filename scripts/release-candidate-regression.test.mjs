import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { validateReleaseCandidate } from "./validate-release-candidate.mjs";

const version = "1.0.0";
const commitSha = "0123456789abcdef0123456789abcdef01234567";
const repository = "space2233/pixnya";

const assetNames = [
  `PixNya_${version}_x64-setup.exe`,
  `PixNya_${version}_x64-setup.nsis.zip`,
  `PixNya_${version}_x64-setup.nsis.zip.sig`,
  `PixNya_${version}_amd64.AppImage`,
  `PixNya_${version}_amd64.AppImage.tar.gz`,
  `PixNya_${version}_amd64.AppImage.tar.gz.sig`,
  `pixnya-${version}-android-arm64-v8a.apk`,
  `pixnya-${version}.spdx.json`,
  `pixnya-${version}-android-runtime.spdx.json`,
  `pixnya-${version}-android-gradle-dependencies.json`,
  `pixnya-${version}-android-build-tools-osv.json`,
  `pixnya-${version}-third-party-licenses.tar.gz`,
  `pixnya-${version}-source.tar.gz`,
  "LICENSE.txt",
  "THIRD_PARTY_NOTICES.md",
  "latest.json",
  "android-latest.json",
  "android-latest.json.minisig",
  "BUILD-PROVENANCE.txt",
  "SHA256SUMS.txt",
];

const completeNotes = `## Unofficial status and platforms
PixNya is an unofficial client. Windows x64, Linux x64, Android ARM64.

## API and OAuth boundary
The non-public App API may change; OAuth build parameters are extractable.

## Low-security connections
Compatibility mode is off by default and carries man-in-the-middle risk.

## Source, licenses, SBOM, and checksums
Source commit: ${commitSha}
GPL-3.0-only
LICENSE.txt
pixnya-${version}-source.tar.gz
pixnya-${version}-third-party-licenses.tar.gz
pixnya-${version}.spdx.json
pixnya-${version}-android-runtime.spdx.json
SHA256SUMS.txt

## Upgrade verification and limitations
- Windows x64: 0.29.0 -> ${version}; Windows 11 24H2; PASS
- Linux x64: 0.29.0 -> ${version}; Ubuntu 24.04; PASS
- Android ARM64: 0.29.0 -> ${version}; Android 15 device; PASS
Failure-path coverage: wrong signature, corrupted manifest, interrupted download, low space, cancelled install, retry
Known limitations: Windows binaries are not Authenticode-signed.`;

const sha256 = (value) => createHash("sha256").update(value).digest("hex");

async function createCandidate() {
  const assetsDir = await mkdtemp(path.join(os.tmpdir(), "pixnya-release-candidate-"));
  const contents = new Map();
  for (const name of assetNames.filter((name) => name !== "SHA256SUMS.txt")) {
    let value = `fixture:${name}`;
    if (name === "BUILD-PROVENANCE.txt") {
      value = [
        "project=PixNya",
        `version=${version}`,
        `source_repository=https://github.com/${repository}`,
        `source_commit=${commitSha}`,
        "source_ref=refs/heads/main",
        "workflow_run=https://github.com/space2233/pixnya/actions/runs/123",
        "",
      ].join("\n");
    }
    contents.set(name, value);
  }
  const windowsArchive = `PixNya_${version}_x64-setup.nsis.zip`;
  const linuxArchive = `PixNya_${version}_amd64.AppImage.tar.gz`;
  const androidApk = `pixnya-${version}-android-arm64-v8a.apk`;
  contents.set(`${windowsArchive}.sig`, Buffer.from("windows minisign fixture").toString("base64"));
  contents.set(`${linuxArchive}.sig`, Buffer.from("linux minisign fixture").toString("base64"));
  contents.set("latest.json", `${JSON.stringify({
    version,
    notes: completeNotes,
    pub_date: "2026-08-10T00:00:00.000Z",
    platforms: {
      "windows-x86_64": {
        signature: contents.get(`${windowsArchive}.sig`),
        url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(windowsArchive)}`,
      },
      "linux-x86_64": {
        signature: contents.get(`${linuxArchive}.sig`),
        url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(linuxArchive)}`,
      },
    },
  }, null, 2)}\n`);
  contents.set("android-latest.json", `${JSON.stringify({
    schemaVersion: 1,
    versionName: version,
    versionCode: 1_000_000,
    publishedAt: "2026-08-10T00:00:00.000Z",
    notes: `PixNya v${version}. See the GitHub Release for verified notes.`,
    minSdk: 29,
    artifacts: [{
      abi: "arm64-v8a",
      url: `https://github.com/${repository}/releases/download/v${version}/${androidApk}`,
      size: Buffer.byteLength(contents.get(androidApk)),
      sha256: sha256(contents.get(androidApk)),
      packageName: "io.github.space2233.pixnya",
      certificateSha256: "a".repeat(64),
    }],
  }, null, 2)}\n`);
  for (const [name, value] of contents) await writeFile(path.join(assetsDir, name), value);
  const checksumBody = [...contents]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, value]) => `${sha256(value)}  ${name}`)
    .join("\n") + "\n";
  await writeFile(path.join(assetsDir, "SHA256SUMS.txt"), checksumBody);

  const release = {
    id: 123,
    tag_name: `v${version}`,
    draft: true,
    prerelease: false,
    body: completeNotes,
    assets: await Promise.all(assetNames.map(async (name, index) => ({
      id: index + 1,
      name,
      size: Buffer.byteLength(name === "SHA256SUMS.txt" ? checksumBody : contents.get(name)),
    }))),
  };
  const tag = { ref: `refs/tags/v${version}`, object: { type: "commit", sha: commitSha } };
  return { assetsDir, release, tag };
}

async function refreshCandidateIntegrity(candidate) {
  const checksumLines = [];
  for (const name of assetNames.filter((assetName) => assetName !== "SHA256SUMS.txt").sort()) {
    const value = await readFile(path.join(candidate.assetsDir, name));
    checksumLines.push(`${sha256(value)}  ${name}`);
    candidate.release.assets.find((asset) => asset.name === name).size = value.length;
  }
  const checksumBody = `${checksumLines.join("\n")}\n`;
  await writeFile(path.join(candidate.assetsDir, "SHA256SUMS.txt"), checksumBody);
  candidate.release.assets.find((asset) => asset.name === "SHA256SUMS.txt").size = Buffer.byteLength(checksumBody);
}

test("a complete signed Draft candidate passes the strict publication boundary", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));

  assert.doesNotThrow(() => validateReleaseCandidate({
    ...candidate,
    version,
    commitSha,
    repository,
  }));
});

test("a Draft with pending upgrade evidence cannot cross the stable publication boundary", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  candidate.release.body = candidate.release.body.replaceAll("; PASS", "; PENDING after Draft artifacts");

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /stable release notes still contain PENDING evidence/,
  );
});

test("a candidate bound to a different commit cannot be published", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  candidate.tag.object.sha = "f".repeat(40);

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /release tag does not point to the candidate commit/,
  );
});

test("a downloaded asset that no longer matches SHA256SUMS is rejected", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const changedAsset = path.join(candidate.assetsDir, `PixNya_${version}_x64-setup.exe`);
  const original = `fixture:PixNya_${version}_x64-setup.exe`;
  await writeFile(changedAsset, `${original.slice(0, -1)}X`);

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /checksum mismatch/,
  );
});

test("a desktop manifest cannot redirect a stable client outside the verified Release", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "latest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.platforms["windows-x86_64"].url = "https://example.com/pixnya.zip";
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await refreshCandidateIntegrity(candidate);

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /desktop update URL does not match the verified Release asset/,
  );
});

test("stable update manifests cannot retain Draft-only pending notes", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "android-latest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.notes = "PENDING after Draft artifacts";
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await refreshCandidateIntegrity(candidate);

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /Android update notes are unfinished/,
  );
});

test("the signed Android update manifest must contain stable release notes", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "android-latest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  delete manifest.notes;
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await refreshCandidateIntegrity(candidate);

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /Android update notes are empty/,
  );
});
