import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { validateReleaseCandidate } from "./validate-release-candidate.mjs";

const version = "1.0.0";
const commitSha = "0123456789abcdef0123456789abcdef01234567";
const workflowCommitSha = "89abcdef0123456789abcdef0123456789abcdef";
const repository = "space2233/pixnya";

const assetNames = [
  `PixNya_${version}_x64-setup.exe`,
  `PixNya_${version}_arm64-setup.exe`,
  `PixNya_${version}_amd64.AppImage`,
  `pixnya-${version}-android-arm64-v8a.apk`,
  `pixnya-${version}-android-armeabi-v7a.apk`,
  `pixnya-${version}-verification.tar.gz`,
  "latest.json",
  "android-latest.json",
  "android-latest.json.minisig",
  "SHA256SUMS.txt",
];

const completeNotes = `# PixNya ${version}

## 中文

- 新增通知、评论管理和动图导出。
- 支持 Windows x64、Windows ARM64、Linux x64、Android ARM64 和 Android ARM32（Android 10+）。

## English

- Added notifications, comment management, and animation export.
- Supports Windows x64, Windows ARM64, Linux x64, Android ARM64, and Android ARM32 (Android 10+).`;

const sha256 = (value) => createHash("sha256").update(value).digest("hex");

async function createCandidate() {
  const assetsDir = await mkdtemp(path.join(os.tmpdir(), "pixnya-release-candidate-"));
  const contents = new Map();
  for (const name of assetNames.filter((name) => name !== "SHA256SUMS.txt")) {
    contents.set(name, `fixture:${name}`);
  }
  const windowsArchive = `PixNya_${version}_x64-setup.exe`;
  const windowsArm64Archive = `PixNya_${version}_arm64-setup.exe`;
  const linuxArchive = `PixNya_${version}_amd64.AppImage`;
  const androidApk = `pixnya-${version}-android-arm64-v8a.apk`;
  const androidArmv7Apk = `pixnya-${version}-android-armeabi-v7a.apk`;
  const windowsSignature = Buffer.from("windows minisign fixture").toString("base64");
  const windowsArm64Signature = Buffer.from("windows arm64 minisign fixture").toString("base64");
  const linuxSignature = Buffer.from("linux minisign fixture").toString("base64");
  contents.set("latest.json", `${JSON.stringify({
    version,
    notes: completeNotes,
    pub_date: "2026-08-10T00:00:00.000Z",
    platforms: {
      "windows-x86_64": {
        signature: windowsSignature,
        url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(windowsArchive)}`,
      },
      "windows-aarch64": {
        signature: windowsArm64Signature,
        url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(windowsArm64Archive)}`,
      },
      "linux-x86_64": {
        signature: linuxSignature,
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
    artifacts: [
      {
        abi: "arm64-v8a",
        url: `https://github.com/${repository}/releases/download/v${version}/${androidApk}`,
        size: Buffer.byteLength(contents.get(androidApk)),
        sha256: sha256(contents.get(androidApk)),
        packageName: "io.github.space2233.pixnya",
        certificateSha256: "a".repeat(64),
      },
      {
        abi: "armeabi-v7a",
        url: `https://github.com/${repository}/releases/download/v${version}/${androidArmv7Apk}`,
        size: Buffer.byteLength(contents.get(androidArmv7Apk)),
        sha256: sha256(contents.get(androidArmv7Apk)),
        packageName: "io.github.space2233.pixnya",
        certificateSha256: "a".repeat(64),
      },
    ],
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
  const provenanceText = [
    "project=PixNya",
    `version=${version}`,
    `source_repository=https://github.com/${repository}`,
    `source_commit=${commitSha}`,
    `release_workflow_commit=${commitSha}`,
    "source_ref=refs/heads/main",
    "workflow_run=https://github.com/space2233/pixnya/actions/runs/123",
    "",
  ].join("\n");
  return { assetsDir, release, tag, provenanceText };
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

test("a Draft with unfinished release notes cannot cross the stable publication boundary", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  candidate.release.body += "\nPENDING";

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

test("recovered artifacts keep their source commit while trusting the selected main finalizer", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));

  candidate.provenanceText = candidate.provenanceText.replace(
    `release_workflow_commit=${commitSha}`,
    `release_workflow_commit=${workflowCommitSha}`,
  );

  assert.doesNotThrow(() => validateReleaseCandidate({
    ...candidate,
    version,
    commitSha,
    workflowCommitSha,
    repository,
  }));

  candidate.provenanceText = candidate.provenanceText.replace(
    `release_workflow_commit=${workflowCommitSha}`,
    `release_workflow_commit=${"f".repeat(40)}`,
  );

  assert.throws(
    () => validateReleaseCandidate({
      ...candidate,
      version,
      commitSha,
      workflowCommitSha,
      repository,
    }),
    /release workflow commit does not match the selected main commit/,
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

test("a desktop manifest cannot swap Windows architecture assets", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "latest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const x64Url = manifest.platforms["windows-x86_64"].url;
  manifest.platforms["windows-x86_64"].url = manifest.platforms["windows-aarch64"].url;
  manifest.platforms["windows-aarch64"].url = x64Url;
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await refreshCandidateIntegrity(candidate);

  assert.throws(
    () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
    /desktop update URL does not match the verified Release asset: windows-x86_64/,
  );
});

test("a desktop manifest must contain the exact released platform set", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "latest.json");
  const original = JSON.parse(await readFile(manifestPath, "utf8"));

  for (const mutate of [
    (manifest) => { delete manifest.platforms["windows-aarch64"]; },
    (manifest) => { manifest.platforms["darwin-aarch64"] = manifest.platforms["windows-aarch64"]; },
  ]) {
    const manifest = structuredClone(original);
    mutate(manifest);
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    await refreshCandidateIntegrity(candidate);
    assert.throws(
      () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
      /desktop update manifest platform set does not match/,
    );
  }
});

test("the Android artifact order is irrelevant when both released ABIs are present", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "android-latest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.artifacts.reverse();
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await refreshCandidateIntegrity(candidate);

  assert.doesNotThrow(() => validateReleaseCandidate({ ...candidate, version, commitSha, repository }));
});

test("the Android manifest rejects a missing, duplicate, or swapped ABI asset", async (context) => {
  const candidate = await createCandidate();
  context.after(() => rm(candidate.assetsDir, { recursive: true, force: true }));
  const manifestPath = path.join(candidate.assetsDir, "android-latest.json");
  const original = JSON.parse(await readFile(manifestPath, "utf8"));

  for (const mutate of [
    (manifest) => manifest.artifacts.pop(),
    (manifest) => { manifest.artifacts[1].abi = "arm64-v8a"; },
    (manifest) => {
      const url = manifest.artifacts[0].url;
      manifest.artifacts[0].url = manifest.artifacts[1].url;
      manifest.artifacts[1].url = url;
    },
  ]) {
    const manifest = structuredClone(original);
    mutate(manifest);
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    await refreshCandidateIntegrity(candidate);
    assert.throws(
      () => validateReleaseCandidate({ ...candidate, version, commitSha, repository }),
      /Android update manifest (?:must contain|contains duplicate|URL does not match)/,
    );
  }
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
