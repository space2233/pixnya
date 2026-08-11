import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { validateStableReleaseNotes } from "./validate-release-notes.mjs";

const requireValue = (condition, message) => {
  if (!condition) throw new Error(message);
};

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const hashFile = (filePath) => createHash("sha256").update(readFileSync(filePath)).digest("hex");

export const releaseAssetPatterns = (version) => {
  const escapedVersion = escapeRegExp(version);
  return [
    ["Windows NSIS updater installer", /\.exe$/],
    ["Linux AppImage updater", /\.AppImage$/],
    ["Android ARM64 APK", new RegExp(`^pixnya-${escapedVersion}-android-arm64-v8a\\.apk$`)],
    ["verification bundle", new RegExp(`^pixnya-${escapedVersion}-verification\\.tar\\.gz$`)],
    ["desktop update manifest", /^latest\.json$/],
    ["Android update manifest", /^android-latest\.json$/],
    ["Android update manifest signature", /^android-latest\.json\.minisig$/],
    ["SHA-256 checksums", /^SHA256SUMS\.txt$/],
  ];
};

const parseProvenance = (value) => {
  const entries = new Map();
  for (const line of value.replace(/\r\n?/g, "\n").split("\n")) {
    if (line === "") continue;
    const separator = line.indexOf("=");
    requireValue(separator > 0, `invalid build provenance line: ${line}`);
    const key = line.slice(0, separator);
    requireValue(!entries.has(key), `duplicate build provenance key: ${key}`);
    entries.set(key, line.slice(separator + 1));
  }
  return entries;
};

const verifyChecksums = (assetsDir, assetNames) => {
  const checksumName = "SHA256SUMS.txt";
  const expectedNames = assetNames.filter((name) => name !== checksumName).sort();
  const checksums = new Map();
  const lines = readFileSync(path.join(assetsDir, checksumName), "utf8")
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .filter(Boolean);
  for (const line of lines) {
    const match = line.match(/^([0-9a-f]{64})  ([^/\\]+)$/i);
    requireValue(Boolean(match), `invalid SHA256SUMS entry: ${line}`);
    const [, digest, name] = match;
    requireValue(!checksums.has(name), `duplicate SHA256SUMS entry: ${name}`);
    checksums.set(name, digest.toLowerCase());
  }
  requireValue(
    JSON.stringify([...checksums.keys()].sort()) === JSON.stringify(expectedNames),
    "SHA256SUMS does not cover the exact release asset set",
  );
  for (const [name, expectedDigest] of checksums) {
    requireValue(hashFile(path.join(assetsDir, name)) === expectedDigest, `checksum mismatch: ${name}`);
  }
};

const readJsonAsset = (assetsDir, name) => {
  try {
    return JSON.parse(readFileSync(path.join(assetsDir, name), "utf8"));
  } catch (error) {
    throw new Error(`${name} is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
};

const releaseAssetUrl = (repository, version, name) =>
  `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(name)}`;

const validateEmbeddedNotes = (notes, name) => {
  requireValue(typeof notes === "string" && notes.trim() !== "", `${name} update notes are empty`);
  requireValue(!/(?:\bPENDING\b|待验收|{{|}})/i.test(notes), `${name} update notes are unfinished`);
};

const verifyUpdateManifests = (assetsDir, assetNames, version, repository) => {
  const desktopManifest = readJsonAsset(assetsDir, "latest.json");
  requireValue(desktopManifest.version === version, "desktop update manifest version does not match");
  requireValue(!Number.isNaN(Date.parse(desktopManifest.pub_date)), "desktop update publication date is invalid");
  validateEmbeddedNotes(desktopManifest.notes, "desktop");
  const desktopTargets = [
    ["windows-x86_64", assetNames.find((name) => name.endsWith(".exe"))],
    ["linux-x86_64", assetNames.find((name) => name.endsWith(".AppImage"))],
  ];
  requireValue(
    JSON.stringify(Object.keys(desktopManifest.platforms ?? {}).sort()) ===
      JSON.stringify(desktopTargets.map(([target]) => target).sort()),
    "desktop update manifest platform set does not match",
  );
  for (const [target, archiveName] of desktopTargets) {
    const entry = desktopManifest.platforms[target];
    requireValue(
      entry?.url === releaseAssetUrl(repository, version, archiveName),
      `desktop update URL does not match the verified Release asset: ${target}`,
    );
    requireValue(typeof entry?.signature === "string" && entry.signature !== "", `desktop update signature is missing: ${target}`);
    const signature = Buffer.from(entry.signature, "base64");
    requireValue(
      signature.length > 0 && signature.toString("base64") === entry.signature,
      `desktop update signature is not canonical Base64: ${target}`,
    );
  }

  const androidManifest = readJsonAsset(assetsDir, "android-latest.json");
  const [major, minor, patch] = version.split(".").map(Number);
  const versionCode = major * 1_000_000 + minor * 1_000 + patch;
  requireValue(androidManifest.schemaVersion === 1, "Android update manifest schema is invalid");
  requireValue(androidManifest.versionName === version, "Android update manifest version does not match");
  requireValue(androidManifest.versionCode === versionCode, "Android update manifest versionCode does not match");
  requireValue(androidManifest.minSdk === 29, "Android update manifest minSdk does not match");
  requireValue(!Number.isNaN(Date.parse(androidManifest.publishedAt)), "Android update publication date is invalid");
  validateEmbeddedNotes(androidManifest.notes, "Android");
  requireValue(Array.isArray(androidManifest.artifacts) && androidManifest.artifacts.length === 1, "Android update manifest must contain exactly one ARM64 artifact");
  const artifact = androidManifest.artifacts[0];
  const apkName = `pixnya-${version}-android-arm64-v8a.apk`;
  const apkPath = path.join(assetsDir, apkName);
  requireValue(artifact.abi === "arm64-v8a", "Android update manifest ABI is invalid");
  requireValue(artifact.url === releaseAssetUrl(repository, version, apkName), "Android update URL does not match the verified Release asset");
  requireValue(artifact.size === statSync(apkPath).size, "Android update manifest APK size does not match");
  requireValue(artifact.sha256 === hashFile(apkPath), "Android update manifest APK digest does not match");
  requireValue(artifact.packageName === "io.github.space2233.pixnya", "Android update package name does not match");
  requireValue(/^[0-9a-f]{64}$/i.test(artifact.certificateSha256 ?? ""), "Android update certificate digest is invalid");
};

export const releaseCandidateSnapshot = (release) => {
  const material = {
    id: release.id,
    tag_name: release.tag_name,
    draft: release.draft,
    prerelease: release.prerelease,
    body: release.body,
    assets: [...release.assets]
      .map(({ id, name, size, updated_at: updatedAt, digest }) => ({ id, name, size, updatedAt, digest }))
      .sort((left, right) => left.name.localeCompare(right.name)),
  };
  return createHash("sha256").update(JSON.stringify(material)).digest("hex");
};

export function validateReleaseCandidate({ release, tag, assetsDir, version, commitSha, repository, provenanceText }) {
  requireValue(/^[1-9][0-9]*\.[0-9]+\.[0-9]+$/.test(version), "stable candidate version is invalid");
  requireValue(/^[0-9a-f]{40}$/i.test(commitSha), "candidate commit must be a full Git SHA");
  requireValue(/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository), "candidate repository is invalid");
  requireValue(release?.tag_name === `v${version}`, "Draft tag does not match the requested version");
  requireValue(release?.draft === true, "release candidate is not a Draft");
  requireValue(release?.prerelease === false, "stable release candidate is marked as a prerelease");
  requireValue(tag?.ref === `refs/tags/v${version}`, "Git tag reference does not match the requested version");
  requireValue(tag?.object?.type === "commit", "release tag does not point directly to a commit");
  requireValue(tag.object?.sha === commitSha, "release tag does not point to the candidate commit");

  validateStableReleaseNotes({ notes: release.body ?? "", version, commitSha });

  requireValue(Array.isArray(release.assets), "Draft assets are missing");
  requireValue(release.assets.length === 8, `expected exactly 8 Draft assets, found ${release.assets.length}`);
  const remoteNames = release.assets.map((asset) => asset.name).sort();
  requireValue(new Set(remoteNames).size === remoteNames.length, "Draft contains duplicate asset names");
  for (const asset of release.assets) {
    requireValue(Number.isInteger(asset.id) && asset.id > 0, `Draft asset has no stable ID: ${asset.name}`);
    requireValue(Number.isInteger(asset.size) && asset.size > 0, `Draft asset is empty: ${asset.name}`);
    requireValue(path.basename(asset.name) === asset.name, `Draft asset has an unsafe name: ${asset.name}`);
  }
  for (const [description, pattern] of releaseAssetPatterns(version)) {
    const matches = remoteNames.filter((name) => pattern.test(name));
    requireValue(matches.length === 1, `expected exactly one ${description}, found ${matches.length}`);
  }

  requireValue(existsSync(assetsDir) && statSync(assetsDir).isDirectory(), "downloaded candidate directory is missing");
  const localNames = readdirSync(assetsDir)
    .filter((name) => statSync(path.join(assetsDir, name)).isFile())
    .sort();
  requireValue(
    JSON.stringify(localNames) === JSON.stringify(remoteNames),
    "downloaded asset set does not match the Draft metadata",
  );
  for (const asset of release.assets) {
    requireValue(statSync(path.join(assetsDir, asset.name)).size === asset.size, `downloaded asset size mismatch: ${asset.name}`);
  }
  verifyChecksums(assetsDir, localNames);
  verifyUpdateManifests(assetsDir, localNames, version, repository);

  requireValue(typeof provenanceText === "string" && provenanceText !== "", "build provenance is missing from the verification bundle");
  const provenance = parseProvenance(provenanceText);
  requireValue(provenance.get("project") === "PixNya", "build provenance project is invalid");
  requireValue(provenance.get("version") === version, "build provenance version does not match");
  requireValue(provenance.get("source_repository") === `https://github.com/${repository}`, "build provenance repository does not match");
  requireValue(provenance.get("source_commit") === commitSha, "build provenance commit does not match");
  requireValue(
    /^[0-9a-f]{40}$/i.test(provenance.get("release_workflow_commit") ?? ""),
    "build provenance release workflow commit is invalid",
  );

  return { snapshot: releaseCandidateSnapshot(release) };
}

const parseArguments = (argv) => {
  const values = new Map();
  for (let index = 2; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    requireValue(key?.startsWith("--") && value !== undefined, `invalid argument: ${key ?? ""}`);
    values.set(key, value);
  }
  return values;
};

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const args = parseArguments(process.argv);
    const required = ["--release-json", "--tag-json", "--assets-dir", "--version", "--commit", "--repository", "--provenance-file"];
    for (const name of required) requireValue(args.has(name), `missing argument: ${name}`);
    const release = JSON.parse(readFileSync(args.get("--release-json"), "utf8"));
    const tag = JSON.parse(readFileSync(args.get("--tag-json"), "utf8"));
    const result = validateReleaseCandidate({
      release,
      tag,
      assetsDir: args.get("--assets-dir"),
      version: args.get("--version"),
      commitSha: args.get("--commit"),
      repository: args.get("--repository"),
      provenanceText: readFileSync(args.get("--provenance-file"), "utf8"),
    });
    if (args.has("--snapshot-output")) writeFileSync(args.get("--snapshot-output"), `${result.snapshot}\n`);
    if (args.has("--expect-snapshot")) {
      const expected = readFileSync(args.get("--expect-snapshot"), "utf8").trim();
      requireValue(result.snapshot === expected, "The Draft changed while it was being verified");
    }
    console.log(`Verified stable Draft candidate v${args.get("--version")} (${result.snapshot}).`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
