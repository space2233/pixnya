import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const reviewedMavenLicenses = new Map(
  [
    ["Android Software Development Kit License Agreement", "LicenseRef-Android-SDK-License", ["https://developer.android.com/studio/terms"]],
    ["Apache 2.0", "Apache-2.0", ["http://www.apache.org/licenses/LICENSE-2.0", "http://www.apache.org/licenses/LICENSE-2.0.txt", "https://opensource.org/licenses/Apache-2.0", "https://www.apache.org/licenses/LICENSE-2.0.txt"]],
    ["Apache License v2.0", "Apache-2.0", ["http://www.apache.org/licenses/LICENSE-2.0.txt", "https://raw.githubusercontent.com/google/flatbuffers/master/LICENSE.txt"]],
    ["Apache License, Version 2.0", "Apache-2.0", ["http://www.apache.org/licenses/LICENSE-2.0.txt", "http://www.opensource.org/licenses/apache2.0.php", "https://www.apache.org/licenses/LICENSE-2.0", "https://www.apache.org/licenses/LICENSE-2.0.txt"]],
    ["Apache Software License, Version 2.0", "Apache-2.0", ["https://www.apache.org/licenses/LICENSE-2.0"]],
    ["Apache-2.0", "Apache-2.0", ["http://www.apache.org/licenses/LICENSE-2.0.txt", "https://www.apache.org/licenses/LICENSE-2.0.txt"]],
    ["Bouncy Castle Licence", "LicenseRef-Bouncy-Castle", ["https://www.bouncycastle.org/licence.html"]],
    ["BSD style", "BSD-3-Clause", ["http://kxml.cvs.sourceforge.net/viewvc/kxml/kxml2/license.txt?view=markup"]],
    ["BSD-3-Clause", "BSD-3-Clause", ["https://asm.ow2.io/license.html", "https://opensource.org/licenses/BSD-3-Clause"]],
    ["CDDL + GPLv2 with classpath exception", "LicenseRef-CDDL-GPL-Classpath", ["https://github.com/javaee/javax.annotation/blob/master/LICENSE"]],
    ["CDDL/GPLv2+CE", "LicenseRef-CDDL-GPL-Classpath", ["https://github.com/javaee/activation/blob/master/LICENSE.txt"]],
    ["Eclipse Distribution License - v 1.0", "LicenseRef-EDL-1.0", ["http://www.eclipse.org/org/documents/edl-v10.php"]],
    ["Eclipse Public License 1.0", "EPL-1.0", ["http://www.eclipse.org/legal/epl-v10.html"]],
    ["EDL 1.0", "LicenseRef-EDL-1.0", ["http://www.eclipse.org/org/documents/edl-v10.php"]],
    ["GNU LESSER GENERAL PUBLIC LICENSE 2.1", "LGPL-2.1-only", ["https://www.gnu.org/licenses/old-licenses/lgpl-2.1.en.html"]],
    ["LGPL, version 2.1", "LGPL-2.1-only", ["http://www.gnu.org/licenses/licenses.html"]],
    ["MIT License", "MIT", ["http://www.opensource.org/licenses/mit-license.php", "https://spdx.org/licenses/MIT.txt"]],
    ["Mozilla Public License 1.1 (MPL 1.1)", "MPL-1.1", ["http://www.mozilla.org/MPL/MPL-1.1.html"]],
    ["New BSD License", "BSD-3-Clause", ["http://www.opensource.org/licenses/bsd-license.php"]],
    ["Public Domain", "LicenseRef-Public-Domain", ["http://creativecommons.org/licenses/publicdomain"]],
    ["Similar to Apache License but with the acknowledgment clause removed", "LicenseRef-JDOM", ["https://raw.github.com/hunterhacker/jdom/master/LICENSE.txt"]],
    ["The Apache License, Version 2.0", "Apache-2.0", ["http://www.apache.org/licenses/LICENSE-2.0.txt"]],
    ["The Apache Software License, Version 2.0", "Apache-2.0", ["http://www.apache.org/licenses/LICENSE-2.0.txt", "https://www.apache.org/licenses/LICENSE-2.0.txt"]],
    ["The MIT License", "MIT", ["http://opensource.org/licenses/MIT", "http://www.opensource.org/licenses/mit-license.php"]],
  ].map(([name, expression, urls]) => [normalizedLicenseName(name), { expression, urls }]),
);

const reviewedLicenseReferences = {
  "LicenseRef-Android-SDK-License": {
    name: "Android Software Development Kit License Agreement",
    classification: "upstream-metadata-only",
    extractedText:
      "Upstream Maven metadata identifies these build/test components as governed by the Android Software Development Kit License Agreement. The cached Maven package does not provide a complete copy of those terms; consult the upstream terms URL recorded with the declaration before redistribution.",
  },
  "LicenseRef-EDL-1.0": {
    name: "Eclipse Distribution License 1.0",
    classification: "upstream-metadata-only",
    extractedText:
      "Upstream Maven metadata identifies this component as licensed under the Eclipse Distribution License 1.0. The evidence snapshot preserves the exact upstream declaration and URL; it is not represented by a canonical SPDX license identifier.",
  },
  "LicenseRef-CDDL-GPL-Classpath": {
    name: "Upstream CDDL/GPLv2 with Classpath exception declaration",
    classification: "upstream-metadata-only",
    extractedText:
      "Upstream Maven metadata declares a CDDL/GPLv2 dual license with the Classpath exception. The evidence snapshot preserves the exact upstream declaration and URL; this combined declaration is retained as a LicenseRef instead of guessing its precise SPDX version semantics.",
  },
  "LicenseRef-Bouncy-Castle": {
    name: "Bouncy Castle Licence",
    classification: "upstream-metadata-only",
    extractedText:
      "Upstream Maven metadata identifies this component under the Bouncy Castle Licence and records the official terms URL. The declaration is retained as a LicenseRef instead of assuming equivalence to a standardized SPDX license.",
  },
  "LicenseRef-JDOM": {
    name: "JDOM License",
    classification: "upstream-metadata-only",
    extractedText:
      "Upstream Maven metadata describes this license as similar to the Apache License with the acknowledgment clause removed. The evidence snapshot preserves the exact upstream declaration and URL; it is not represented by a canonical SPDX license identifier.",
  },
  "LicenseRef-Public-Domain": {
    name: "Upstream public-domain declaration",
    classification: "upstream-metadata-only",
    extractedText:
      "Upstream Maven metadata declares this component to be available in the public domain and records the associated upstream URL. This LicenseRef preserves that declaration without replacing it with a different standardized license.",
  },
};

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function normalizedLicenseName(value) {
  return String(value).trim().replace(/\s+/g, " ").toLowerCase();
}

function decodeXmlText(value) {
  return String(value)
    .replace(/^\s*<!\[CDATA\[([\s\S]*)\]\]>\s*$/, "$1")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&apos;", "'")
    .replaceAll("&amp;", "&")
    .trim();
}

function xmlTag(block, name) {
  const match = String(block).match(new RegExp(`<${name}(?:\\s[^>]*)?>([\\s\\S]*?)<\\/${name}>`, "i"));
  return match ? decodeXmlText(match[1]) : null;
}

function parsePom(text, coordinate) {
  const parentBlock = text.match(/<parent(?:\s[^>]*)?>([\s\S]*?)<\/parent>/i)?.[1] ?? null;
  const parent = parentBlock
    ? [xmlTag(parentBlock, "groupId"), xmlTag(parentBlock, "artifactId"), xmlTag(parentBlock, "version")]
    : [];
  if (parent.length > 0 && (parent.some((value) => !value) || parent.some((value) => value.includes("${")))) {
    throw new Error(`${coordinate} has an unsupported Maven parent declaration.`);
  }

  const licensesBlock = text.match(/<licenses(?:\s[^>]*)?>([\s\S]*?)<\/licenses>/i)?.[1] ?? "";
  const licenses = [...licensesBlock.matchAll(/<license(?:\s[^>]*)?>([\s\S]*?)<\/license>/gi)]
    .map((match) => ({
      name: xmlTag(match[1], "name"),
      url: xmlTag(match[1], "url"),
    }))
    .filter((entry) => entry.name);
  const withoutNestedMetadata = text
    .replace(/<parent(?:\s[^>]*)?>[\s\S]*?<\/parent>/i, "")
    .replace(/<licenses(?:\s[^>]*)?>[\s\S]*?<\/licenses>/i, "")
    .replace(/<developers(?:\s[^>]*)?>[\s\S]*?<\/developers>/i, "")
    .replace(/<scm(?:\s[^>]*)?>[\s\S]*?<\/scm>/i, "");
  return {
    parentCoordinate: parent.length > 0 ? parent.join(":") : null,
    licenses,
    projectUrl: xmlTag(withoutNestedMetadata, "url"),
  };
}

function coordinateParts(coordinate) {
  const parts = String(coordinate).split(":");
  if (
    parts.length !== 3 ||
    parts.some(
      (part) =>
        !/^[A-Za-z0-9_.-]+$/.test(part) ||
        part === "." ||
        part === "..",
    ) ||
    parts[0].split(".").some((segment) => !/^[A-Za-z0-9_-]+$/.test(segment))
  ) {
    throw new Error(`Unsupported Maven coordinate: ${coordinate}`);
  }
  return parts;
}

const maximumPomBytes = 4 * 1024 * 1024;
export const reviewedMavenPomRepositories = Object.freeze([
  "https://dl.google.com/dl/android/maven2/",
  "https://repo.maven.apache.org/maven2/",
]);

function pomCacheDescriptor(gradleUserHome, coordinate) {
  const [group, name, version] = coordinateParts(coordinate);
  return {
    coordinate,
    group,
    name,
    version,
    pomName: `${name}-${version}.pom`,
    versionDirectory: join(
      gradleUserHome,
      "caches",
      "modules-2",
      "files-2.1",
      group,
      name,
      version,
    ),
  };
}

function trackedPomHashes(verifiedArtifacts, descriptor) {
  if (!(verifiedArtifacts instanceof Map)) {
    throw new Error("Tracked Gradle verification artifacts are required for Maven POM hydration.");
  }
  const entries = (verifiedArtifacts.get(descriptor.coordinate) ?? []).filter(
    (entry) => entry.name === descriptor.pomName,
  );
  if (entries.length !== 1) {
    throw new Error(
      `${descriptor.coordinate} requires exactly one tracked ${descriptor.pomName} verification entry; found ${entries.length}.`,
    );
  }
  const hashes = [...new Set(entries[0].sha256 ?? [])].sort();
  if (hashes.length !== 1 || !/^[a-f0-9]{64}$/.test(hashes[0])) {
    throw new Error(
      `${descriptor.coordinate} requires exactly one tracked Maven POM SHA-256; found ${hashes.length}.`,
    );
  }
  return hashes;
}

function cachedPomCandidates(descriptor) {
  if (!existsSync(descriptor.versionDirectory)) return [];
  return readdirSync(descriptor.versionDirectory, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((entry) => {
      const candidate = join(descriptor.versionDirectory, entry.name, descriptor.pomName);
      return existsSync(candidate) ? [candidate] : [];
    });
}

function inspectCachedPom(descriptor, expectedHashes = null) {
  const candidates = cachedPomCandidates(descriptor);
  if (candidates.length === 0) return null;
  if (candidates.length !== 1) {
    throw new Error(
      `${descriptor.coordinate} requires exactly one cached ${descriptor.pomName}; found ${candidates.length}.`,
    );
  }
  const content = readFileSync(candidates[0]);
  const digest = sha256(content);
  if (expectedHashes && !expectedHashes.includes(digest)) {
    throw new Error(
      `${descriptor.coordinate} cached POM is not covered by tracked Gradle SHA-256 verification metadata.`,
    );
  }
  return { path: candidates[0], content, sha256: digest };
}

function findCachedPom(
  gradleUserHome,
  coordinate,
  verifiedArtifacts,
  { requireVerification = true } = {},
) {
  const descriptor = pomCacheDescriptor(gradleUserHome, coordinate);
  if (!existsSync(descriptor.versionDirectory)) {
    throw new Error(`${coordinate} has no local Maven cache directory; resolve the locked Gradle graph first.`);
  }
  let expectedHashes = null;
  if (requireVerification) {
    try {
      expectedHashes = trackedPomHashes(verifiedArtifacts, descriptor);
    } catch {
      throw new Error(
        `${coordinate} cached POM is not covered by tracked Gradle SHA-256 verification metadata.`,
      );
    }
  }
  const cached = inspectCachedPom(descriptor, expectedHashes);
  if (!cached) {
    throw new Error(`${coordinate} requires exactly one cached ${descriptor.pomName}; found 0.`);
  }
  return cached;
}

function repositoriesForCoordinate(coordinate) {
  const [group] = coordinateParts(coordinate);
  const googleFirst = /^(?:androidx(?:\.|$)|com\.android(?:\.|$)|com\.google\.android(?:\.|$)|com\.google\.testing\.platform(?:\.|$))/.test(
    group,
  );
  return googleFirst
    ? reviewedMavenPomRepositories
    : [...reviewedMavenPomRepositories].reverse();
}

async function readBoundedResponseBody(response, coordinate) {
  const reader = response.body?.getReader?.();
  if (!reader) throw new Error(`${coordinate} POM response has no readable byte stream.`);
  const chunks = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!(value instanceof Uint8Array)) {
        throw new Error(`${coordinate} POM response returned a non-byte chunk.`);
      }
      total += value.byteLength;
      if (total > maximumPomBytes) {
        await reader.cancel().catch(() => {});
        throw new Error(`${coordinate} POM exceeds the ${maximumPomBytes}-byte safety limit.`);
      }
      chunks.push(Buffer.from(value));
    }
  } finally {
    reader.releaseLock?.();
  }
  if (total === 0) throw new Error(`${coordinate} POM response is empty.`);
  return Buffer.concat(chunks, total);
}

export async function fetchPomFromReviewedRepositories(
  { coordinate, expectedSha256 },
  { fetchImpl = globalThis.fetch } = {},
) {
  const [group, name, version] = coordinateParts(coordinate);
  const relativePath = `${group.split(".").map(encodeURIComponent).join("/")}/${encodeURIComponent(name)}/${encodeURIComponent(version)}/${encodeURIComponent(name)}-${encodeURIComponent(version)}.pom`;
  if (typeof fetchImpl !== "function") throw new Error("A fetch implementation is required for POM hydration.");
  const failures = [];
  for (const repository of repositoriesForCoordinate(coordinate)) {
    const url = new URL(relativePath, repository);
    try {
      const response = await fetchImpl(url, {
        headers: { Accept: "application/xml,text/xml;q=0.9,*/*;q=0.1" },
        redirect: "error",
        signal: AbortSignal.timeout(30_000),
      });
      if (response.status === 404) {
        failures.push(`${url.href}: 404`);
        continue;
      }
      if (!response.ok) {
        failures.push(`${url.href}: HTTP ${response.status}`);
        continue;
      }
      const declaredLength = Number(response.headers.get("content-length"));
      if (Number.isFinite(declaredLength) && declaredLength > maximumPomBytes) {
        throw new Error(`${coordinate} POM exceeds the ${maximumPomBytes}-byte safety limit.`);
      }
      const content = await readBoundedResponseBody(response, coordinate);
      const digest = sha256(content);
      if (expectedSha256.includes(digest)) return content;
      failures.push(`${url.href}: untrusted sha256:${digest}`);
    } catch (error) {
      failures.push(`${url.href}: ${error?.message ?? error}`);
    }
  }
  throw new Error(
    `${coordinate} could not be hydrated from the reviewed Maven repositories with a tracked SHA-256 (${failures.join("; ")}).`,
  );
}

function writeVerifiedPom(descriptor, content) {
  const digestDirectory = sha256(content);
  const directory = join(descriptor.versionDirectory, digestDirectory);
  const destination = join(directory, descriptor.pomName);
  mkdirSync(directory, { recursive: true });
  if (existsSync(destination)) {
    if (!readFileSync(destination).equals(content)) {
      throw new Error(`${descriptor.coordinate} cache destination contains different POM bytes.`);
    }
    return destination;
  }
  const temporary = `${destination}.${process.pid}.${randomUUID()}.tmp`;
  try {
    writeFileSync(temporary, content, { flag: "wx" });
    renameSync(temporary, destination);
  } finally {
    rmSync(temporary, { force: true });
  }
  return destination;
}

function createConcurrencyLimiter(limit) {
  let active = 0;
  const waiting = [];
  return async (operation) => {
    if (active >= limit) {
      await new Promise((resolve) => waiting.push(resolve));
    }
    active += 1;
    try {
      return await operation();
    } finally {
      active -= 1;
      waiting.shift()?.();
    }
  };
}

export async function hydrateGradlePomEvidence(
  inventory,
  {
    gradleUserHome,
    verifiedArtifacts,
    fetchPom = fetchPomFromReviewedRepositories,
    concurrency = 12,
  } = {},
) {
  if (inventory?.schemaVersion !== 1 || !Array.isArray(inventory.components)) {
    throw new Error("Unsupported Android Gradle inventory document.");
  }
  if (!(verifiedArtifacts instanceof Map)) {
    throw new Error("Tracked Gradle verification artifacts are required for Maven POM hydration.");
  }
  if (typeof gradleUserHome !== "string" || gradleUserHome.trim() === "") {
    throw new Error("Maven POM hydration requires an explicit isolated Gradle user home.");
  }
  if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 32) {
    throw new Error("Maven POM hydration concurrency must be an integer from 1 through 32.");
  }
  const componentCoordinates = inventory.components.map((component) => component.coordinate);
  if (new Set(componentCoordinates).size !== componentCoordinates.length) {
    throw new Error("Android Gradle inventory contains duplicate Maven coordinates.");
  }

  const limit = createConcurrencyLimiter(concurrency);
  const hydrationByCoordinate = new Map();
  const ensurePom = (coordinate) => {
    const existing = hydrationByCoordinate.get(coordinate);
    if (existing) return existing;
    const operation = (async () => {
      const descriptor = pomCacheDescriptor(gradleUserHome, coordinate);
      const expectedSha256 = trackedPomHashes(verifiedArtifacts, descriptor);
      const cached = inspectCachedPom(descriptor, expectedSha256);
      if (cached) {
        return { ...cached, coordinate, pomName: descriptor.pomName, source: "cache" };
      }

      const fetched = await limit(() =>
        fetchPom({
          coordinate,
          expectedSha256,
        }),
      );
      if (!(fetched instanceof Uint8Array)) {
        throw new Error(`${coordinate} POM fetcher returned no byte content.`);
      }
      const content = Buffer.from(fetched);
      if (content.length === 0 || content.length > maximumPomBytes) {
        throw new Error(`${coordinate} POM has an invalid ${content.length}-byte size.`);
      }
      const digest = sha256(content);
      if (!expectedSha256.includes(digest)) {
        throw new Error(
          `${coordinate} downloaded POM does not match tracked Gradle SHA-256 verification metadata.`,
        );
      }
      const path = writeVerifiedPom(descriptor, content);
      return {
        path,
        content,
        sha256: digest,
        coordinate,
        pomName: descriptor.pomName,
        source: "download",
      };
    })();
    hydrationByCoordinate.set(coordinate, operation);
    return operation;
  };

  const visit = async (coordinate, ancestry = []) => {
    if (ancestry.includes(coordinate)) {
      throw new Error(`Maven parent cycle while hydrating ${[...ancestry, coordinate].join(" -> ")}.`);
    }
    const pom = await ensurePom(coordinate);
    const parsed = parsePom(pom.content.toString("utf8"), coordinate);
    if (parsed.licenses.length > 0) return;
    if (!parsed.parentCoordinate) {
      throw new Error(`${coordinate} and its Maven parents declare no license metadata.`);
    }
    await visit(parsed.parentCoordinate, [...ancestry, coordinate]);
  };

  const visits = await Promise.allSettled(
    componentCoordinates.map((coordinate) => visit(coordinate)),
  );
  const failures = visits
    .filter((result) => result.status === "rejected")
    .map((result) => result.reason)
    .filter(Boolean);
  if (failures.length > 0) {
    const messages = [...new Set(failures.map((error) => error?.message ?? String(error)))].sort();
    throw new AggregateError(
      failures,
      `Maven POM hydration failed for ${messages.length} distinct dependency paths:\n${messages
        .map((message) => `- ${message}`)
        .join("\n")}`,
    );
  }
  const hydrated = await Promise.all(hydrationByCoordinate.values());
  return {
    componentCount: componentCoordinates.length,
    pomCount: hydrated.length,
    downloadedCount: hydrated.filter((entry) => entry.source === "download").length,
    cachedCount: hydrated.filter((entry) => entry.source === "cache").length,
    poms: hydrated
      .map(({ coordinate, pomName, sha256: digest }) => ({
        coordinate,
        pomName,
        sha256: digest,
      }))
      .sort((left, right) => left.coordinate.localeCompare(right.coordinate)),
  };
}

function resolvePomLicense(
  coordinate,
  gradleUserHome,
  verifiedArtifacts,
  visited = new Set(),
  discoveredPoms = new Map(),
  requireVerification = true,
) {
  if (visited.has(coordinate)) throw new Error(`Maven parent cycle while resolving ${coordinate}.`);
  visited.add(coordinate);
  const pom = findCachedPom(gradleUserHome, coordinate, verifiedArtifacts, { requireVerification });
  discoveredPoms.set(coordinate, {
    coordinate,
    name: coordinateParts(coordinate)[1],
    version: coordinateParts(coordinate)[2],
    pomName: `${coordinateParts(coordinate)[1]}-${coordinateParts(coordinate)[2]}.pom`,
    sha256: pom.sha256,
  });
  const parsed = parsePom(pom.content.toString("utf8"), coordinate);
  const componentEvidence = { coordinate, pomSha256: pom.sha256 };
  if (parsed.licenses.length > 0) {
    return {
      componentEvidence,
      sourceCoordinate: coordinate,
      sourcePomSha256: pom.sha256,
      declaredLicenses: parsed.licenses,
      projectUrl: parsed.projectUrl,
    };
  }
  if (!parsed.parentCoordinate) {
    throw new Error(`${coordinate} and its Maven parents declare no license metadata.`);
  }
  const inherited = resolvePomLicense(
    parsed.parentCoordinate,
    gradleUserHome,
    verifiedArtifacts,
    visited,
    discoveredPoms,
    requireVerification,
  );
  return {
    componentEvidence,
    sourceCoordinate: inherited.sourceCoordinate,
    sourcePomSha256: inherited.sourcePomSha256,
    declaredLicenses: inherited.declaredLicenses,
    projectUrl: parsed.projectUrl ?? inherited.projectUrl,
  };
}

export function discoverGradlePomVerificationEntries(
  inventory,
  { gradleUserHome = process.env.GRADLE_USER_HOME ?? join(homedir(), ".gradle") } = {},
) {
  if (inventory?.schemaVersion !== 1 || !Array.isArray(inventory.components)) {
    throw new Error("Unsupported Android Gradle inventory document.");
  }
  const discoveredPoms = new Map();
  for (const component of inventory.components) {
    resolvePomLicense(
      component.coordinate,
      gradleUserHome,
      null,
      new Set(),
      discoveredPoms,
      false,
    );
  }
  return [...discoveredPoms.values()].sort((left, right) =>
    left.coordinate.localeCompare(right.coordinate),
  );
}

function expressionForLicenses(coordinate, licenses) {
  const expressions = [];
  const licenseReferences = [];
  for (const license of licenses) {
    const reviewedLicense = reviewedMavenLicenses.get(normalizedLicenseName(license.name));
    if (!reviewedLicense || !reviewedLicense.urls.includes(license.url)) {
      throw new Error(
        `${coordinate} has an unreviewed Maven license declaration: ${license.name}${license.url ? ` (${license.url})` : ""}.`,
      );
    }
    const { expression } = reviewedLicense;
    if (!expressions.includes(expression)) expressions.push(expression);
    for (const identifier of expression.match(/LicenseRef-[A-Za-z0-9.-]+/g) ?? []) {
      const reviewed = reviewedLicenseReferences[identifier];
      if (!reviewed) throw new Error(`${coordinate} has no reviewed evidence for ${identifier}.`);
      const existing = licenseReferences.find((entry) => entry.licenseId === identifier);
      const seeAlso = license.url ? [license.url] : [];
      if (existing) {
        existing.seeAlso = [...new Set([...existing.seeAlso, ...seeAlso])].sort();
      } else {
        licenseReferences.push({ licenseId: identifier, ...reviewed, seeAlso });
      }
    }
  }
  if (expressions.length === 0) throw new Error(`${coordinate} has no reviewed Maven license expression.`);
  return {
    spdxLicense: expressions
      .map((expression) => (expressions.length > 1 && /\s(?:OR|AND)\s/.test(expression) ? `(${expression})` : expression))
      .join(" OR "),
    licenseReferences: licenseReferences.sort((left, right) => left.licenseId.localeCompare(right.licenseId)),
  };
}

export function buildGradleLicenseReview(
  inventory,
  {
    gradleUserHome = process.env.GRADLE_USER_HOME ?? join(homedir(), ".gradle"),
    verifiedArtifacts,
  } = {},
) {
  if (
    inventory?.schemaVersion !== 1 ||
    typeof inventory.fingerprint !== "string" ||
    !Array.isArray(inventory.components)
  ) {
    throw new Error("Unsupported Android Gradle inventory document.");
  }
  if (!(verifiedArtifacts instanceof Map)) {
    throw new Error("Tracked Gradle verification artifacts are required for Maven license review.");
  }
  const components = inventory.components.map((component) => {
    const coordinate = component.coordinate;
    const evidence = resolvePomLicense(coordinate, gradleUserHome, verifiedArtifacts);
    const expression = expressionForLicenses(coordinate, evidence.declaredLicenses);
    return {
      coordinate,
      declaredLicense: evidence.declaredLicenses.map((entry) => entry.name).join(" OR "),
      spdxLicense: expression.spdxLicense,
      projectUrl: evidence.projectUrl ?? null,
      licenseEvidence: {
        classification:
          expression.licenseReferences.length > 0 ? "reviewed-with-license-ref" : "canonical-spdx-text",
        componentPomSha256: evidence.componentEvidence.pomSha256,
        sourceCoordinate: evidence.sourceCoordinate,
        sourcePomSha256: evidence.sourcePomSha256,
        declaredLicenses: evidence.declaredLicenses,
      },
      licenseReferences: expression.licenseReferences,
    };
  });
  components.sort((left, right) => left.coordinate.localeCompare(right.coordinate));
  const review = {
    schemaVersion: 1,
    gradleFingerprint: inventory.fingerprint,
    components,
  };
  validateGradleLicenseReview(inventory, review);
  return review;
}

export function validateGradleLicenseReview(inventory, review) {
  if (review?.schemaVersion !== 1 || !Array.isArray(review.components)) {
    throw new Error("Unsupported Gradle license review document.");
  }
  if (review.gradleFingerprint !== inventory?.fingerprint) {
    throw new Error("Gradle license review is stale for the locked Gradle graph.");
  }
  const expected = new Set((inventory.components ?? []).map((entry) => entry.coordinate));
  const actual = new Set(review.components.map((entry) => entry.coordinate));
  if (expected.size !== (inventory.components ?? []).length || actual.size !== review.components.length) {
    throw new Error("Gradle inventory or license review contains duplicate Maven coordinates.");
  }
  const missing = [...expected].filter((coordinate) => !actual.has(coordinate));
  const unexpected = [...actual].filter((coordinate) => !expected.has(coordinate));
  if (missing.length > 0 || unexpected.length > 0) {
    throw new Error(
      `Gradle license review does not exactly cover the locked graph (missing: ${missing.slice(0, 5).join(", ") || "none"}; unexpected: ${unexpected.slice(0, 5).join(", ") || "none"}).`,
    );
  }
  for (const entry of review.components) {
    if (
      typeof entry.declaredLicense !== "string" ||
      !entry.declaredLicense ||
      typeof entry.spdxLicense !== "string" ||
      !entry.spdxLicense ||
      !/^[a-f0-9]{64}$/.test(entry.licenseEvidence?.componentPomSha256 ?? "") ||
      !/^[a-f0-9]{64}$/.test(entry.licenseEvidence?.sourcePomSha256 ?? "") ||
      !Array.isArray(entry.licenseEvidence?.declaredLicenses) ||
      entry.licenseEvidence.declaredLicenses.length === 0
    ) {
      throw new Error(`${entry.coordinate ?? "unknown Maven component"} has incomplete reviewed license evidence.`);
    }
    const expectedExpression = expressionForLicenses(
      entry.coordinate,
      entry.licenseEvidence.declaredLicenses,
    );
    const expectedDeclaredLicense = entry.licenseEvidence.declaredLicenses
      .map((license) => license.name)
      .join(" OR ");
    if (
      entry.declaredLicense !== expectedDeclaredLicense ||
      entry.spdxLicense !== expectedExpression.spdxLicense ||
      JSON.stringify(entry.licenseReferences ?? []) !==
        JSON.stringify(expectedExpression.licenseReferences)
    ) {
      throw new Error(`${entry.coordinate} no longer matches the reviewed Maven license mapping.`);
    }
    for (const reference of entry.licenseReferences ?? []) {
      if (
        !/^LicenseRef-[A-Za-z0-9.-]+$/.test(reference.licenseId ?? "") ||
        typeof reference.extractedText !== "string" ||
        reference.extractedText.length < 20 ||
        reference.classification !== "upstream-metadata-only"
      ) {
        throw new Error(`${entry.coordinate} has invalid reviewed LicenseRef evidence.`);
      }
    }
    const expressionReferences = new Set(
      entry.spdxLicense.match(/LicenseRef-[A-Za-z0-9.-]+/g) ?? [],
    );
    const reviewedReferences = new Set(
      (entry.licenseReferences ?? []).map((reference) => reference.licenseId),
    );
    if (
      expressionReferences.size !== reviewedReferences.size ||
      [...expressionReferences].some((identifier) => !reviewedReferences.has(identifier))
    ) {
      throw new Error(`${entry.coordinate} does not exactly cover its SPDX LicenseRef evidence.`);
    }
  }
  return review;
}

function primaryChecksum(component) {
  const artifacts = component.artifacts ?? [];
  for (const pattern of [/\.(?:aar|jar)$/i, /\.pom$/i, /\.module$/i]) {
    const artifact = artifacts.find((entry) => pattern.test(entry.name));
    const value = artifact?.sha256?.[0];
    if (typeof value === "string" && /^[a-f0-9]{64}$/i.test(value)) {
      return { algorithm: "SHA256", checksumValue: value.toUpperCase() };
    }
  }
  return null;
}

export function gradlePackagesFromReview(inventory, review) {
  validateGradleLicenseReview(inventory, review);
  const reviews = new Map(review.components.map((entry) => [entry.coordinate, entry]));
  return [...inventory.components]
    .map((component) => {
      const [group, artifact, version] = coordinateParts(component.coordinate);
      const reviewed = reviews.get(component.coordinate);
      const declaration = {
        coordinate: component.coordinate,
        declaredLicense: reviewed.declaredLicense,
        spdxLicense: reviewed.spdxLicense,
        projectUrl: reviewed.projectUrl,
        licenseEvidence: reviewed.licenseEvidence,
        licenseReferences: reviewed.licenseReferences,
      };
      return {
        ecosystem: "maven",
        name: `${group}:${artifact}`,
        version,
        license: reviewed.declaredLicense,
        spdxLicense: reviewed.spdxLicense,
        development: component.lockfiles.every((path) => path !== "app/gradle.lockfile"),
        optional: false,
        resolved: "NOASSERTION",
        checksum: primaryChecksum(component),
        purl: `pkg:maven/${encodeURIComponent(group)}/${encodeURIComponent(artifact)}@${encodeURIComponent(version)}`,
        identity: `maven:${component.coordinate}`,
        sourceDirectory: null,
        repository: reviewed.projectUrl ?? null,
        declarationEvidence: [
          {
            name: "MAVEN-LICENSE-DECLARATION.json",
            content: `${JSON.stringify(declaration, null, 2)}\n`,
          },
        ],
        licenseReferences: reviewed.licenseReferences,
      };
    })
    .sort(
      (left, right) =>
        left.name.localeCompare(right.name) ||
        left.version.localeCompare(right.version) ||
        left.identity.localeCompare(right.identity),
    );
}
