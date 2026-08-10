import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import spdxLicenseCatalog from "spdx-license-list/full.js";

import { inspectAndroidGradleSupplyChain } from "./check-android-gradle-supply-chain.mjs";
import {
  gradlePackagesFromReview,
  validateGradleLicenseReview,
} from "./gradle-license-evidence.mjs";

const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = fileURLToPath(new URL("../", import.meta.url));

export const CANONICAL_GPL3_SHA256 =
  "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986";

// These exact package releases omit `license` from package-lock.json. Their
// shipped LICENSE files are canonical MIT texts; the digest prevents a future
// package update from silently inheriting the override.
const npmLicenseOverrides = {
  "@inlang/plugin-message-format@4.4.1": {
    license: "MIT",
    evidence: "node_modules/@inlang/plugin-message-format/LICENSE",
    sha256: "5f3b025ba92b2e3aec1cf74725dbaad5d5d4aaf24b52ce6479f21eb37c8cf9ae",
  },
  "sqlite-wasm-kysely@0.3.0": {
    license: "MIT",
    evidence: "node_modules/sqlite-wasm-kysely/LICENSE",
    sha256: "5f3b025ba92b2e3aec1cf74725dbaad5d5d4aaf24b52ce6479f21eb37c8cf9ae",
  },
};

export const cargoMetadataArguments = [
  "metadata",
  "--locked",
  "--offline",
  "--format-version",
  "1",
];

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function normalizedText(value) {
  return value.replace(/\r\n/g, "\n");
}

const licenseEvidenceName = /^(?:LICENSE|LICENCE|COPYING)(?:[._-].*)?$/i;
const noticeEvidenceName = /^(?:NOTICE|AUTHORS|COPYRIGHT|PATENTS)(?:[._-].*)?$/i;
const licenseBundleMarker = "PixNya third-party license bundle\n";

function safeBundleSegment(value) {
  const safe = String(value)
    .normalize("NFKC")
    .replace(/[^A-Za-z0-9._+-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
  return safe || "package";
}

function expressionTerms(expression) {
  const tokens = String(expression).match(/[A-Za-z0-9][A-Za-z0-9.+-]*/g) ?? [];
  const licenses = [];
  const exceptions = [];
  let expectsException = false;
  for (const token of tokens) {
    if (token === "AND" || token === "OR") continue;
    if (token === "WITH") {
      expectsException = true;
      continue;
    }
    (expectsException ? exceptions : licenses).push(token);
    expectsException = false;
  }
  return {
    licenses: [...new Set(licenses)],
    exceptions: [...new Set(exceptions)],
  };
}

function evidenceFiles(sourceDirectory) {
  if (typeof sourceDirectory !== "string" || !sourceDirectory) return [];
  if (!existsSync(sourceDirectory) || !statSync(sourceDirectory).isDirectory()) return [];
  return readdirSync(sourceDirectory)
    .filter((name) => licenseEvidenceName.test(name) || noticeEvidenceName.test(name))
    .filter((name) => lstatSync(join(sourceDirectory, name)).isFile())
    .sort((left, right) => left.localeCompare(right));
}

function writeReviewedText(path, value, description) {
  if (typeof value !== "string" || value.trim().length < 20) {
    throw new Error(`${description} has no reviewed full text.`);
  }
  writeFileSync(path, normalizedText(value).replace(/\n?$/, "\n"), "utf8");
}

function prepareOwnedBundleDestination(outputDirectory) {
  const marker = join(outputDirectory, ".pixnya-license-bundle");
  if (!existsSync(outputDirectory)) return;
  if (!existsSync(marker) || readFileSync(marker, "utf8") !== licenseBundleMarker) {
    throw new Error(`Refusing to replace unowned license bundle directory: ${outputDirectory}`);
  }
  rmSync(outputDirectory, { recursive: true, force: false });
}

export function writeThirdPartyLicenseBundle({
  outputDirectory,
  packages,
  licenseCatalog,
  exceptionCatalog = {},
}) {
  if (typeof outputDirectory !== "string" || !outputDirectory) {
    throw new Error("A third-party license bundle output directory is required.");
  }
  if (!Array.isArray(packages) || !licenseCatalog || typeof licenseCatalog !== "object") {
    throw new Error("Third-party license bundle inputs are invalid.");
  }

  const stagingDirectory = `${outputDirectory}.staging-${randomUUID()}`;
  mkdirSync(join(stagingDirectory, "packages"), { recursive: true });

  const indexPackages = [];
  let upstreamLicenseFileCount = 0;
  let noticeFileCount = 0;
  let declarationEvidenceFileCount = 0;
  let fallbackPackageCount = 0;
  let classifiedReferencePackageCount = 0;
  try {
    for (const entry of [...packages].sort((left, right) =>
      left.identity.localeCompare(right.identity),
    )) {
      if (
        typeof entry.identity !== "string" ||
        typeof entry.spdxLicense !== "string" ||
        (entry.sourceDirectory &&
          (!existsSync(entry.sourceDirectory) || !statSync(entry.sourceDirectory).isDirectory()) &&
          !entry.optional)
      ) {
        throw new Error(`${entry.identity ?? "unknown dependency"} has no local package evidence.`);
      }

      const directoryName = `${safeBundleSegment(entry.ecosystem)}-${safeBundleSegment(entry.name)}-${safeBundleSegment(entry.version)}-${sha256(entry.identity).slice(0, 12)}`;
      const packageDirectory = join(stagingDirectory, "packages", directoryName);
      mkdirSync(packageDirectory, { recursive: true });
      const copiedEvidence = [];
      const declarationEvidence = [];
      for (const evidence of entry.declarationEvidence ?? []) {
        if (
          typeof evidence?.name !== "string" ||
          !/^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$/.test(evidence.name) ||
          (typeof evidence.content !== "string" && !Buffer.isBuffer(evidence.content))
        ) {
          throw new Error(`${entry.identity} has invalid declaration evidence.`);
        }
        const content = Buffer.from(evidence.content);
        if (content.length === 0 || content.length > 2 * 1024 * 1024 || content.includes(0)) {
          throw new Error(`${entry.identity} has invalid declaration evidence ${evidence.name}.`);
        }
        writeFileSync(join(packageDirectory, evidence.name), content);
        declarationEvidence.push(evidence.name);
        declarationEvidenceFileCount += 1;
      }
      let hasUpstreamLicense = false;
      for (const name of evidenceFiles(entry.sourceDirectory)) {
        const content = readFileSync(join(entry.sourceDirectory, name));
        if (
          content.length === 0 ||
          content.length > 2 * 1024 * 1024 ||
          content.includes(0)
        ) {
          throw new Error(`${entry.identity} has invalid license evidence ${name}.`);
        }
        writeFileSync(join(packageDirectory, name), content);
        copiedEvidence.push(name);
        if (licenseEvidenceName.test(name) && content.toString("utf8").trim().length >= 20) {
          hasUpstreamLicense = true;
          upstreamLicenseFileCount += 1;
        } else if (noticeEvidenceName.test(name)) {
          noticeFileCount += 1;
        }
      }

      const canonicalEvidence = [];
      const classifiedReferenceEvidence = [];
      if (!hasUpstreamLicense) {
        const terms = expressionTerms(entry.spdxLicense);
        if (terms.licenses.length === 0) {
          throw new Error(`${entry.identity} has no reviewed full text for ${entry.spdxLicense}.`);
        }
        for (const identifier of terms.licenses) {
          const record = licenseCatalog[identifier];
          const reviewedReference = (entry.licenseReferences ?? []).find(
            (candidate) => candidate.licenseId === identifier,
          );
          if (!record && reviewedReference) {
            const name = `LICENSE-REFERENCE-${safeBundleSegment(identifier)}.txt`;
            writeReviewedText(
              join(packageDirectory, name),
              reviewedReference.extractedText,
              `${entry.identity} reviewed license reference ${identifier}`,
            );
            classifiedReferenceEvidence.push({
              name,
              licenseId: identifier,
              classification: reviewedReference.classification,
              seeAlso: reviewedReference.seeAlso ?? [],
            });
            continue;
          }
          if (!record || typeof record.licenseText !== "string") {
            throw new Error(`${entry.identity} has no reviewed full text for ${identifier}.`);
          }
          const name = `SPDX-${safeBundleSegment(identifier)}.txt`;
          writeReviewedText(
            join(packageDirectory, name),
            record.licenseText,
            `${entry.identity} SPDX license ${identifier}`,
          );
          canonicalEvidence.push(name);
        }
        for (const identifier of terms.exceptions) {
          const text = exceptionCatalog[identifier];
          if (typeof text !== "string") {
            throw new Error(`${entry.identity} has no reviewed full text for ${identifier}.`);
          }
          const name = `SPDX-EXCEPTION-${safeBundleSegment(identifier)}.txt`;
          writeReviewedText(
            join(packageDirectory, name),
            text,
            `${entry.identity} SPDX exception ${identifier}`,
          );
          canonicalEvidence.push(name);
        }
        fallbackPackageCount += 1;
        if (classifiedReferenceEvidence.length > 0) classifiedReferencePackageCount += 1;
      }

      const metadata = {
        ecosystem: entry.ecosystem,
        name: entry.name,
        version: entry.version,
        declaredLicense: entry.license,
        spdxLicense: entry.spdxLicense,
        purl: entry.purl,
        downloadLocation: entry.resolved,
        authors: Array.isArray(entry.authors) ? entry.authors : [],
        repository: entry.repository ?? null,
        upstreamEvidence: copiedEvidence,
        declarationEvidence,
        canonicalFallbackEvidence: canonicalEvidence,
        classifiedReferenceEvidence,
      };
      writeFileSync(join(packageDirectory, "METADATA.json"), `${JSON.stringify(metadata, null, 2)}\n`);
      indexPackages.push({ identity: entry.identity, directory: `packages/${directoryName}`, ...metadata });
    }

    const summary = {
      packageCount: indexPackages.length,
      upstreamLicenseFileCount,
      noticeFileCount,
      declarationEvidenceFileCount,
      fallbackPackageCount,
      classifiedReferencePackageCount,
    };
    writeFileSync(
      join(stagingDirectory, "INDEX.json"),
      `${JSON.stringify({ schemaVersion: 1, ...summary, packages: indexPackages }, null, 2)}\n`,
    );
    writeFileSync(join(stagingDirectory, ".pixnya-license-bundle"), licenseBundleMarker);
    prepareOwnedBundleDestination(outputDirectory);
    renameSync(stagingDirectory, outputDirectory);
    return summary;
  } catch (error) {
    rmSync(stagingDirectory, { recursive: true, force: true });
    throw error;
  }
}

export function normalizeSpdxLicenseExpression(value) {
  const legacyDualLicenseExpressions = new Map([
    ["Apache-2.0 / MIT", "Apache-2.0 OR MIT"],
    ["Apache-2.0/MIT", "Apache-2.0 OR MIT"],
    ["MIT/Apache-2.0", "MIT OR Apache-2.0"],
    ["BSD-3-Clause/MIT", "BSD-3-Clause OR MIT"],
    ["Unlicense/MIT", "Unlicense OR MIT"],
  ]);
  return legacyDualLicenseExpressions.get(value) ?? value;
}

export function computeLockFingerprint(packageLockText, cargoLockText, gradleFingerprint = null) {
  return sha256(
    `package-lock.json\0${normalizedText(packageLockText)}\0Cargo.lock\0${normalizedText(cargoLockText)}${gradleFingerprint ? `\0android-gradle\0${gradleFingerprint}` : ""}`,
  );
}

export function assertCanonicalProjectLicense(licenseText) {
  const actual = sha256(normalizedText(licenseText));
  if (actual !== CANONICAL_GPL3_SHA256) {
    throw new Error(
      `LICENSE is not the unmodified canonical GPL-3.0 text (expected ${CANONICAL_GPL3_SHA256}, got ${actual}).`,
    );
  }
}

function npmPackageName(packagePath) {
  const marker = "node_modules/";
  const index = packagePath.lastIndexOf(marker);
  if (index < 0) throw new Error(`Unsupported npm lockfile package path: ${packagePath}`);
  return packagePath.slice(index + marker.length);
}

function npmPurl(name, version) {
  if (name.startsWith("@")) {
    const slash = name.indexOf("/");
    if (slash < 0) throw new Error(`Invalid scoped npm package name: ${name}`);
    return `pkg:npm/${encodeURIComponent(name.slice(0, slash))}/${encodeURIComponent(name.slice(slash + 1))}@${encodeURIComponent(version)}`;
  }
  return `pkg:npm/${encodeURIComponent(name)}@${encodeURIComponent(version)}`;
}

function resolveNpmLicense(name, version, entry, root) {
  if (typeof entry.license === "string" && entry.license.trim()) return entry.license.trim();

  const identity = `${name}@${version}`;
  const override = npmLicenseOverrides[identity];
  if (!override) return "NOASSERTION";

  const evidencePath = join(root, override.evidence);
  if (!existsSync(evidencePath)) {
    throw new Error(
      `${identity} needs local license evidence at ${override.evidence}; run npm ci before generating notices.`,
    );
  }
  const actual = sha256(readFileSync(evidencePath));
  if (actual !== override.sha256) {
    throw new Error(
      `${identity} license evidence changed (expected ${override.sha256}, got ${actual}); review the package before updating the override.`,
    );
  }
  return override.license;
}

function npmChecksum(integrity) {
  if (typeof integrity !== "string") return null;
  for (const candidate of integrity.trim().split(/\s+/)) {
    const separator = candidate.indexOf("-");
    if (separator < 0) continue;
    const algorithm = candidate.slice(0, separator).toUpperCase();
    if (!new Set(["SHA1", "SHA256", "SHA384", "SHA512"]).has(algorithm)) continue;
    try {
      return {
        algorithm,
        checksumValue: Buffer.from(candidate.slice(separator + 1), "base64")
          .toString("hex")
          .toUpperCase(),
      };
    } catch {
      // A malformed lockfile is reported below as missing integrity evidence.
    }
  }
  return null;
}

export function collectNpmPackages(packageLock, { root = projectRoot } = {}) {
  if (packageLock.lockfileVersion !== 3 || !packageLock.packages) {
    throw new Error("Expected an npm lockfileVersion 3 package-lock.json.");
  }

  const packages = [];
  for (const [packagePath, entry] of Object.entries(packageLock.packages)) {
    if (!packagePath || entry.link) continue;
    const name = npmPackageName(packagePath);
    if (typeof entry.version !== "string" || !entry.version) {
      throw new Error(`npm dependency ${name} has no locked version.`);
    }
    const license = resolveNpmLicense(name, entry.version, entry, root);
    packages.push({
      ecosystem: "npm",
      name,
      version: entry.version,
      license,
      spdxLicense: normalizeSpdxLicenseExpression(license),
      development: Boolean(entry.dev),
      optional: Boolean(entry.optional),
      resolved: typeof entry.resolved === "string" ? entry.resolved : "NOASSERTION",
      checksum: npmChecksum(entry.integrity),
      purl: npmPurl(name, entry.version),
      identity: `npm:${name}@${entry.version}:${entry.resolved ?? packagePath}`,
      sourceDirectory: join(root, ...packagePath.split("/")),
    });
  }
  return deduplicatePackages(packages);
}

function parseTomlString(block, field) {
  const pattern = new RegExp(`^${field}\\s*=\\s*("(?:[^"\\\\]|\\\\.)*")\\s*$`, "m");
  const match = block.match(pattern);
  return match ? JSON.parse(match[1]) : null;
}

export function parseCargoLockChecksums(cargoLockText) {
  const checksums = new Map();
  const packagePattern = /\[\[package\]\]\s*\n([\s\S]*?)(?=\n\[\[package\]\]|\s*$)/g;
  for (const match of cargoLockText.matchAll(packagePattern)) {
    const block = match[1];
    const name = parseTomlString(block, "name");
    const version = parseTomlString(block, "version");
    const source = parseTomlString(block, "source");
    const checksum = parseTomlString(block, "checksum");
    if (name && version && checksum) {
      checksums.set(`${name}\0${version}\0${source ?? ""}`, checksum);
    }
  }
  return checksums;
}

export function collectCargoPackages(metadata, cargoLockText) {
  if (!Array.isArray(metadata?.packages) || !Array.isArray(metadata?.workspace_members)) {
    throw new Error("cargo metadata returned an unsupported document.");
  }
  const workspaceMembers = new Set(metadata.workspace_members);
  const checksums = parseCargoLockChecksums(cargoLockText);
  const packages = metadata.packages
    .filter((entry) => !workspaceMembers.has(entry.id))
    .map((entry) => {
      const source = entry.source ?? "";
      const checksumValue = checksums.get(`${entry.name}\0${entry.version}\0${source}`);
      return {
        ecosystem: "cargo",
        name: entry.name,
        version: entry.version,
        license:
          typeof entry.license === "string" && entry.license.trim()
            ? entry.license.trim()
            : "NOASSERTION",
        spdxLicense: normalizeSpdxLicenseExpression(
          typeof entry.license === "string" && entry.license.trim()
            ? entry.license.trim()
            : "NOASSERTION",
        ),
        development: false,
        optional: false,
        resolved: source.startsWith("registry+")
          ? `https://crates.io/crates/${encodeURIComponent(entry.name)}/${encodeURIComponent(entry.version)}/download`
          : "NOASSERTION",
        checksum: checksumValue
          ? { algorithm: "SHA256", checksumValue: checksumValue.toUpperCase() }
          : null,
        purl: `pkg:cargo/${encodeURIComponent(entry.name)}@${encodeURIComponent(entry.version)}`,
        identity: `cargo:${entry.name}@${entry.version}:${source}`,
        sourceDirectory:
          typeof entry.manifest_path === "string" && entry.manifest_path
            ? dirname(entry.manifest_path)
            : null,
        authors: Array.isArray(entry.authors) ? entry.authors : [],
        repository: typeof entry.repository === "string" ? entry.repository : null,
      };
    });
  return deduplicatePackages(packages);
}

function deduplicatePackages(packages) {
  return [...new Map(packages.map((entry) => [entry.identity, entry])).values()].sort(
    (left, right) =>
      left.ecosystem.localeCompare(right.ecosystem) ||
      left.name.localeCompare(right.name) ||
      left.version.localeCompare(right.version) ||
      left.identity.localeCompare(right.identity),
  );
}

export function assertKnownDependencyLicenses(packages) {
  const unknown = packages.filter((entry) => entry.license === "NOASSERTION");
  if (unknown.length > 0) {
    const identities = unknown.slice(0, 20).map((entry) => entry.identity).join(", ");
    throw new Error(
      `${unknown.length} locked dependencies have no verified license metadata: ${identities}${unknown.length > 20 ? ", …" : ""}`,
    );
  }
}

function markdownCell(value) {
  return String(value).replaceAll("|", "\\|").replaceAll("\n", " ");
}

function renderDependencyTable(packages) {
  if (packages.length === 0) return "_None._\n";
  const lines = [
    "| Package | Version | Declared license | Notes |",
    "|---|---:|---|---|",
  ];
  for (const entry of packages) {
    const packageName = entry.resolved.startsWith("https://")
      ? `[${markdownCell(entry.name)}](${entry.resolved})`
      : markdownCell(entry.name);
    const notes = [entry.optional ? "optional" : null, entry.development ? "build/development" : null]
      .filter(Boolean)
      .join(", ");
    lines.push(
      `| ${packageName} | ${markdownCell(entry.version)} | ${markdownCell(entry.license)} | ${notes || "runtime/target-dependent"} |`,
    );
  }
  return `${lines.join("\n")}\n`;
}

export function renderThirdPartyNotices({
  version,
  fingerprint,
  npmPackages,
  cargoPackages,
  gradlePackages = [],
}) {
  const npmRuntime = npmPackages.filter((entry) => !entry.development);
  const npmBuild = npmPackages.filter((entry) => entry.development);
  return `<!-- Generated by scripts/generate-supply-chain-artifacts.mjs. Do not edit by hand. -->
# PixNya third-party dependency notices

PixNya itself is licensed under GNU GPL-3.0-only; the complete project license is in [LICENSE](LICENSE).

Copyright (C) 2026 PixNya contributors

This inventory records the reviewed license expressions for the exact dependency versions locked for PixNya ${version}. It is generated locally from \`package-lock.json\`, \`Cargo.lock\`, the Android Gradle lock and verification graph, installed npm/Cargo package evidence, and the tracked Maven license review. It is not legal advice and does not replace any upstream license or notice file shipped with a dependency.

- Lock fingerprint: \`sha256:${fingerprint}\`
- npm runtime/optional packages: ${npmRuntime.length}
- npm build/development packages: ${npmBuild.length}
- Rust target-dependent locked packages: ${cargoPackages.length}
- Android Gradle/Maven locked components: ${gradlePackages.length}
- SPDX SBOM command: \`node scripts/generate-supply-chain-artifacts.mjs\`

## npm runtime and optional dependencies

${renderDependencyTable(npmRuntime)}
## npm build and development dependencies

${renderDependencyTable(npmBuild)}
## Rust dependencies

Cargo's lock graph includes target-specific dependencies for Windows, Linux, Android and their build tooling. A package listed here is not necessarily present in every platform artifact.

${renderDependencyTable(cargoPackages)}
## Android Gradle and Maven dependencies

These components come from the strictly locked Android app, buildscript and buildSrc graphs. Their Maven declarations are tied to the checked-in Gradle verification fingerprint; metadata-only LicenseRef classifications remain explicit.

${renderDependencyTable(gradlePackages)}`;
}

function spdxPackageId(entry) {
  return `SPDXRef-Package-${entry.ecosystem}-${sha256(entry.identity).slice(0, 20)}`;
}

function spdxPackage(entry) {
  const result = {
    SPDXID: spdxPackageId(entry),
    name: entry.name,
    versionInfo: entry.version,
    downloadLocation: entry.resolved,
    filesAnalyzed: false,
    licenseConcluded: "NOASSERTION",
    licenseDeclared: entry.spdxLicense,
    copyrightText: "NOASSERTION",
    externalRefs: [
      {
        referenceCategory: "PACKAGE-MANAGER",
        referenceType: "purl",
        referenceLocator: entry.purl,
      },
    ],
  };
  if (entry.checksum) result.checksums = [entry.checksum];
  const comments = [];
  if (entry.development) comments.push("Build/development dependency");
  if (entry.spdxLicense !== entry.license) {
    comments.push(`Upstream legacy license expression: ${entry.license}`);
  }
  if (comments.length > 0) result.packageComment = comments.join("; ");
  return result;
}

export function buildSpdxDocument({ version, fingerprint, packages, created }) {
  const rootId = "SPDXRef-Package-PixNya";
  const dependencyPackages = packages.map(spdxPackage);
  const licenseReferences = new Map();
  for (const entry of packages) {
    for (const reference of entry.licenseReferences ?? []) {
      const normalized = {
        licenseId: reference.licenseId,
        extractedText: reference.extractedText,
        name: reference.name,
        comment: `Evidence classification: ${reference.classification}`,
        seeAlsos: [...new Set(reference.seeAlso ?? [])].sort(),
      };
      const existing = licenseReferences.get(reference.licenseId);
      if (existing) {
        const existingWithoutUrls = { ...existing, seeAlsos: [] };
        const normalizedWithoutUrls = { ...normalized, seeAlsos: [] };
        if (JSON.stringify(existingWithoutUrls) !== JSON.stringify(normalizedWithoutUrls)) {
          throw new Error(`Conflicting extracted SPDX evidence for ${reference.licenseId}.`);
        }
        existing.seeAlsos = [...new Set([...existing.seeAlsos, ...normalized.seeAlsos])].sort();
      } else {
        licenseReferences.set(reference.licenseId, normalized);
      }
    }
  }
  const document = {
    spdxVersion: "SPDX-2.3",
    dataLicense: "CC0-1.0",
    SPDXID: "SPDXRef-DOCUMENT",
    name: `PixNya-${version}-dependency-sbom`,
    documentNamespace: `https://github.com/space2233/pixnya/sbom/${encodeURIComponent(version)}/${fingerprint}`,
    creationInfo: {
      created,
      creators: ["Tool: PixNya offline lockfile SBOM generator"],
    },
    packages: [
      {
        SPDXID: rootId,
        name: "PixNya",
        versionInfo: version,
        downloadLocation: "NOASSERTION",
        filesAnalyzed: false,
        licenseConcluded: "GPL-3.0-only",
        licenseDeclared: "GPL-3.0-only",
        copyrightText: "Copyright (C) 2026 PixNya contributors",
      },
      ...dependencyPackages,
    ],
    relationships: [
      {
        spdxElementId: "SPDXRef-DOCUMENT",
        relationshipType: "DESCRIBES",
        relatedSpdxElement: rootId,
      },
      ...dependencyPackages.map((entry) => ({
        spdxElementId: rootId,
        relationshipType: "DEPENDS_ON",
        relatedSpdxElement: entry.SPDXID,
      })),
    ],
  };
  if (licenseReferences.size > 0) {
    document.hasExtractedLicensingInfos = [...licenseReferences.values()].sort((left, right) =>
      left.licenseId.localeCompare(right.licenseId),
    );
  }
  return document;
}

function createdTimestamp() {
  const epoch = process.env.SOURCE_DATE_EPOCH;
  const date = epoch === undefined ? new Date() : new Date(Number(epoch) * 1000);
  if (Number.isNaN(date.getTime())) {
    throw new Error("SOURCE_DATE_EPOCH must contain Unix seconds.");
  }
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

function loadCargoMetadata(root) {
  try {
    return JSON.parse(
      execFileSync(process.env.CARGO ?? "cargo", cargoMetadataArguments, {
        cwd: root,
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024,
        windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"],
      }),
    );
  } catch (error) {
    const details = error?.stderr?.toString().trim();
    throw new Error(
      `Unable to inspect Cargo dependencies without network access. Run \`cargo fetch --locked\` once, then retry.${details ? `\n${details}` : ""}`,
    );
  }
}

export function checkTrackedSupplyChainFiles(root = projectRoot) {
  const packageLockText = readFileSync(join(root, "package-lock.json"), "utf8");
  const cargoLockText = readFileSync(join(root, "Cargo.lock"), "utf8");
  const gradleInventory = inspectAndroidGradleSupplyChain(root);
  const gradleReview = JSON.parse(readFileSync(join(root, "gradle-license-review.json"), "utf8"));
  validateGradleLicenseReview(gradleInventory, gradleReview);
  const fingerprint = computeLockFingerprint(
    packageLockText,
    cargoLockText,
    gradleInventory.fingerprint,
  );
  assertCanonicalProjectLicense(readFileSync(join(root, "LICENSE"), "utf8"));

  const notices = readFileSync(join(root, "THIRD_PARTY_NOTICES.md"), "utf8");
  if (!notices.startsWith("<!-- Generated by scripts/generate-supply-chain-artifacts.mjs.")) {
    throw new Error("THIRD_PARTY_NOTICES.md is not a recognized generated inventory.");
  }
  if (!notices.includes(`Lock fingerprint: \`sha256:${fingerprint}\``)) {
    throw new Error(
      "THIRD_PARTY_NOTICES.md is stale for the npm, Cargo or Android Gradle locked graph; regenerate it offline.",
    );
  }
  return fingerprint;
}

function parseArguments(argv) {
  const options = {
    check: false,
    notices: join(projectRoot, "THIRD_PARTY_NOTICES.md"),
    sbom: null,
    licensesDir: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") {
      options.check = true;
    } else if (
      argument === "--notices" ||
      argument === "--sbom" ||
      argument === "--licenses-dir"
    ) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a path.`);
      const key = argument === "--licenses-dir" ? "licensesDir" : argument.slice(2);
      options[key] = resolve(projectRoot, value);
      index += 1;
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  return options;
}

function runCli() {
  const options = parseArguments(process.argv.slice(2));
  if (options.check) {
    const fingerprint = checkTrackedSupplyChainFiles();
    console.log(`Supply-chain files match lock fingerprint sha256:${fingerprint}.`);
    return;
  }

  const packageLockText = readFileSync(join(projectRoot, "package-lock.json"), "utf8");
  const cargoLockText = readFileSync(join(projectRoot, "Cargo.lock"), "utf8");
  const packageLock = JSON.parse(packageLockText);
  const version = packageLock.packages?.[""]?.version;
  if (typeof version !== "string" || !version) {
    throw new Error("package-lock.json does not declare the PixNya version.");
  }
  assertCanonicalProjectLicense(readFileSync(join(projectRoot, "LICENSE"), "utf8"));

  const gradleInventory = inspectAndroidGradleSupplyChain(projectRoot);
  const gradleReview = JSON.parse(
    readFileSync(join(projectRoot, "gradle-license-review.json"), "utf8"),
  );
  const gradlePackages = gradlePackagesFromReview(gradleInventory, gradleReview);
  const fingerprint = computeLockFingerprint(
    packageLockText,
    cargoLockText,
    gradleInventory.fingerprint,
  );
  const npmPackages = collectNpmPackages(packageLock);
  const cargoPackages = collectCargoPackages(loadCargoMetadata(projectRoot), cargoLockText);
  const packages = [...npmPackages, ...cargoPackages, ...gradlePackages];
  assertKnownDependencyLicenses(packages);

  const notices = renderThirdPartyNotices({
    version,
    fingerprint,
    npmPackages,
    cargoPackages,
    gradlePackages,
  });
  const sbomPath =
    options.sbom ?? join(projectRoot, "artifacts", "supply-chain", `pixnya-${version}.spdx.json`);
  const licensesDirectory =
    options.licensesDir ??
    join(projectRoot, "artifacts", "supply-chain", `pixnya-${version}-third-party-licenses`);
  const sbom = buildSpdxDocument({
    version,
    fingerprint,
    packages,
    created: createdTimestamp(),
  });

  mkdirSync(dirname(options.notices), { recursive: true });
  mkdirSync(dirname(sbomPath), { recursive: true });
  writeFileSync(options.notices, notices, "utf8");
  writeFileSync(sbomPath, `${JSON.stringify(sbom, null, 2)}\n`, "utf8");
  const licenseBundle = writeThirdPartyLicenseBundle({
    outputDirectory: licensesDirectory,
    packages,
    licenseCatalog: spdxLicenseCatalog,
  });
  console.log(`Wrote ${options.notices}`);
  console.log(`Wrote ${sbomPath}`);
  console.log(`Wrote ${licensesDirectory}`);
  console.log(
    `Recorded ${packages.length} locked npm, Cargo and Gradle third-party packages with verified license metadata.`,
  );
  console.log(
    `Collected ${licenseBundle.upstreamLicenseFileCount} upstream license files and used reviewed SPDX text for ${licenseBundle.fallbackPackageCount} packages.`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(scriptPath)) {
  try {
    runCli();
  } catch (error) {
    console.error(error?.message ?? error);
    process.exitCode = 1;
  }
}
