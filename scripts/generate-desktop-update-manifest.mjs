import { basename, dirname, resolve } from "node:path";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";

import { releaseRepository, validateReleaseBaseUrl } from "./release-url-policy.mjs";

const MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024;
const MAX_SIGNATURE_BYTES = 16 * 1024;

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

async function platformEntry(archivePath, signaturePath, baseUrl, version) {
  const archive = resolve(archivePath);
  const signature = resolve(signaturePath);
  const archiveMetadata = await stat(archive);
  const signatureMetadata = await stat(signature);
  if (!archiveMetadata.isFile() || archiveMetadata.size <= 0 || archiveMetadata.size > MAX_ARCHIVE_BYTES) {
    throw new Error(`${archive} is not a valid bounded updater archive`);
  }
  if (!basename(archive).includes(version)) {
    throw new Error(`${archive} does not contain the release version ${version}`);
  }
  if (!signatureMetadata.isFile() || signatureMetadata.size <= 0 || signatureMetadata.size > MAX_SIGNATURE_BYTES) {
    throw new Error(`${signature} is not a valid bounded updater signature`);
  }
  const signatureText = (await readFile(signature, "utf8")).trim();
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(signatureText) || signatureText.length % 4 !== 0) {
    throw new Error(`${signature} is not a Base64-encoded Tauri updater signature`);
  }
  const decodedSignature = Buffer.from(signatureText, "base64").toString("utf8");
  if (
    !decodedSignature.startsWith("untrusted comment:") ||
    !decodedSignature.includes("\ntrusted comment:")
  ) {
    throw new Error(`${signature} does not contain a complete minisign signature file`);
  }
  return {
    signature: signatureText,
    url: new URL(encodeURIComponent(basename(archive)), baseUrl).toString(),
  };
}

async function writeAtomically(outputPath, content) {
  await mkdir(dirname(outputPath), { recursive: true });
  const temporaryPath = `${outputPath}.tmp`;
  await writeFile(temporaryPath, content, "utf8");
  await rm(outputPath, { force: true });
  await rename(temporaryPath, outputPath);
}

async function main() {
  const argumentsMap = parseArguments(process.argv.slice(2));
  const rootPackage = JSON.parse(await readFile(resolve("package.json"), "utf8"));
  const version = rootPackage.version;
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    throw new Error(`Version ${version} is not a stable major.minor.patch version`);
  }
  const repository = releaseRepository(argumentsMap);
  const baseUrl = new URL(requireArgument(argumentsMap, "base-url"));
  validateReleaseBaseUrl(baseUrl, repository);

  const publishedAt = argumentsMap.get("published-at") ?? new Date().toISOString();
  if (Number.isNaN(Date.parse(publishedAt))) {
    throw new Error("--published-at must be an ISO-8601 timestamp");
  }
  const notesPath = argumentsMap.get("notes-file");
  const notes = notesPath ? (await readFile(resolve(notesPath), "utf8")).trim() : "";
  if (notes.length > 64 * 1024) {
    throw new Error("Release notes exceed 64 KiB");
  }

  const platforms = {
    "windows-x86_64": await platformEntry(
      requireArgument(argumentsMap, "windows-archive"),
      requireArgument(argumentsMap, "windows-signature"),
      baseUrl,
      version,
    ),
    "windows-aarch64": await platformEntry(
      requireArgument(argumentsMap, "windows-arm64-archive"),
      requireArgument(argumentsMap, "windows-arm64-signature"),
      baseUrl,
      version,
    ),
    "linux-x86_64": await platformEntry(
      requireArgument(argumentsMap, "linux-archive"),
      requireArgument(argumentsMap, "linux-signature"),
      baseUrl,
      version,
    ),
  };
  const manifest = {
    version,
    notes,
    pub_date: publishedAt,
    platforms,
  };
  const outputPath = resolve(argumentsMap.get("output") ?? "artifacts/latest.json");
  await writeAtomically(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
  process.stdout.write(`${outputPath}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
