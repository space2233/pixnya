import { createHash } from "node:crypto";
import { basename, dirname, resolve } from "node:path";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";

const PACKAGE_NAME = "io.github.space2233.pixnya";
const MAX_APK_BYTES = 1024 * 1024 * 1024;

function parseArguments(values) {
  const parsed = new Map();
  for (let index = 0; index < values.length; index += 2) {
    const key = values[index];
    const value = values[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      throw new Error(`Invalid argument near ${key ?? "<end>"}`);
    }
    parsed.set(key.slice(2), value);
  }
  return parsed;
}

function requireArgument(argumentsMap, name) {
  const value = argumentsMap.get(name)?.trim();
  if (!value) {
    throw new Error(`Missing required --${name} argument`);
  }
  return value;
}

function normalizeDigest(value) {
  const digest = value.replaceAll(":", "").toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(digest)) {
    throw new Error("The Android release certificate SHA-256 must contain 64 hexadecimal digits");
  }
  return digest;
}

function androidVersionCode(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
  if (!match) {
    throw new Error(`Version ${version} is not a stable major.minor.patch version`);
  }
  const [, majorText, minorText, patchText] = match;
  const [major, minor, patch] = [majorText, minorText, patchText].map(Number);
  if (minor > 999 || patch > 999) {
    throw new Error("Minor and patch versions must be at most 999 for Android versionCode mapping");
  }
  const code = major * 1_000_000 + minor * 1_000 + patch;
  if (!Number.isSafeInteger(code) || code <= 0 || code > 2_100_000_000) {
    throw new Error(`Android versionCode ${code} is outside the supported range`);
  }
  return code;
}

async function apkArtifact(abi, filePath, baseUrl, certificateSha256, version) {
  const absolutePath = resolve(filePath);
  const metadata = await stat(absolutePath);
  if (!metadata.isFile() || metadata.size <= 0 || metadata.size > MAX_APK_BYTES) {
    throw new Error(`${absolutePath} is not a valid bounded APK file`);
  }
  if (!absolutePath.toLowerCase().endsWith(".apk")) {
    throw new Error(`${absolutePath} does not have an .apk extension`);
  }
  if (!basename(absolutePath).includes(version)) {
    throw new Error(`${absolutePath} does not contain the release version ${version}`);
  }
  const content = await readFile(absolutePath);
  return {
    abi,
    url: new URL(encodeURIComponent(basename(absolutePath)), baseUrl).toString(),
    size: metadata.size,
    sha256: createHash("sha256").update(content).digest("hex"),
    packageName: PACKAGE_NAME,
    certificateSha256,
  };
}

async function main() {
  const argumentsMap = parseArguments(process.argv.slice(2));
  const rootPackage = JSON.parse(await readFile(resolve("package.json"), "utf8"));
  const version = rootPackage.version;
  const versionCode = androidVersionCode(version);
  const certificateSha256 = normalizeDigest(requireArgument(argumentsMap, "certificate-sha256"));
  const baseUrl = new URL(requireArgument(argumentsMap, "base-url"));
  if (baseUrl.protocol !== "https:" || baseUrl.hostname !== "github.com") {
    throw new Error("--base-url must be an HTTPS github.com Release download directory");
  }
  const expectedPrefix = "/space2233/pixnya/releases/download/";
  if (!baseUrl.pathname.startsWith(expectedPrefix) || !baseUrl.pathname.endsWith("/")) {
    throw new Error(`--base-url must start with ${expectedPrefix} and end with /`);
  }

  const outputPath = resolve(argumentsMap.get("output") ?? "artifacts/android-latest.json");
  const publishedAt = argumentsMap.get("published-at") ?? new Date().toISOString();
  if (Number.isNaN(Date.parse(publishedAt))) {
    throw new Error("--published-at must be an ISO-8601 timestamp");
  }
  const notesPath = argumentsMap.get("notes-file");
  const notes = notesPath ? (await readFile(resolve(notesPath), "utf8")).trim() : undefined;
  if (notes && notes.length > 64 * 1024) {
    throw new Error("Release notes exceed the 64 KiB Android manifest limit");
  }

  const artifactRequests = [
    apkArtifact(
      "arm64-v8a",
      requireArgument(argumentsMap, "arm64"),
      baseUrl,
      certificateSha256,
      version,
    ),
  ];
  const optionalArmv7 = argumentsMap.get("armv7")?.trim();
  if (optionalArmv7) {
    artifactRequests.push(apkArtifact(
      "armeabi-v7a",
      optionalArmv7,
      baseUrl,
      certificateSha256,
      version,
    ));
  }
  const artifacts = await Promise.all(artifactRequests);
  const manifest = {
    schemaVersion: 1,
    versionName: version,
    versionCode,
    publishedAt,
    ...(notes ? { notes } : {}),
    minSdk: 29,
    artifacts,
  };
  const serialized = `${JSON.stringify(manifest, null, 2)}\n`;
  await mkdir(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp`;
  await writeFile(temporaryPath, serialized, "utf8");
  await rm(outputPath, { force: true });
  await rename(temporaryPath, outputPath);
  process.stdout.write(`${outputPath}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
