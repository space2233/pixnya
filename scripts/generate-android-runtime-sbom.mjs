import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { inspectAndroidGradleSupplyChain } from "./check-android-gradle-supply-chain.mjs";

const scriptPath = fileURLToPath(import.meta.url);
const defaultProjectRoot = fileURLToPath(new URL("../", import.meta.url));
export const androidReleaseRuntimeConfiguration = "arm64ReleaseRuntimeClasspath";

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sourceDateEpoch() {
  const raw = process.env.SOURCE_DATE_EPOCH;
  if (raw === undefined || raw === "") return new Date(0).toISOString().replace(".000", "");
  if (!/^\d+$/.test(raw)) throw new Error("SOURCE_DATE_EPOCH must be an unsigned integer.");
  return new Date(Number(raw) * 1000).toISOString().replace(".000", "");
}

function parseCoordinate(coordinate) {
  const parts = coordinate.split(":");
  if (parts.length !== 3 || parts.some((part) => !part)) {
    throw new Error(`Unsupported Maven coordinate in Android runtime graph: ${coordinate}`);
  }
  return { group: parts[0], artifact: parts[1], version: parts[2] };
}

function purl({ group, artifact, version }) {
  return `pkg:maven/${encodeURIComponent(group)}/${encodeURIComponent(artifact)}@${encodeURIComponent(version)}`;
}

export function createAndroidRuntimeSpdx(inventory, created = sourceDateEpoch()) {
  const selected = inventory.components.filter(
    (component) =>
      component.lockfiles.includes("app/gradle.lockfile") &&
      component.configurations.includes(androidReleaseRuntimeConfiguration),
  );
  if (selected.length === 0) {
    throw new Error(`No components were locked for ${androidReleaseRuntimeConfiguration}.`);
  }

  const packages = selected.map((component) => {
    const coordinate = parseCoordinate(component.coordinate);
    return {
      SPDXID: `SPDXRef-Package-${sha256(component.coordinate).slice(0, 24)}`,
      name: `${coordinate.group}:${coordinate.artifact}`,
      versionInfo: coordinate.version,
      downloadLocation: "NOASSERTION",
      filesAnalyzed: false,
      licenseConcluded: "NOASSERTION",
      licenseDeclared: "NOASSERTION",
      copyrightText: "NOASSERTION",
      externalRefs: [
        {
          referenceCategory: "PACKAGE-MANAGER",
          referenceType: "purl",
          referenceLocator: purl(coordinate),
        },
      ],
    };
  });
  packages.sort((left, right) => left.name.localeCompare(right.name) || left.versionInfo.localeCompare(right.versionInfo));

  const graphFingerprint = sha256(
    packages.map((entry) => entry.externalRefs[0].referenceLocator).join("\n"),
  );
  return {
    spdxVersion: "SPDX-2.3",
    dataLicense: "CC0-1.0",
    SPDXID: "SPDXRef-DOCUMENT",
    name: "PixNya Android ARM64 release runtime",
    documentNamespace: `https://github.com/space2233/pixnya/sbom/android-runtime/${graphFingerprint}`,
    creationInfo: {
      created,
      creators: ["Tool: PixNya generate-android-runtime-sbom.mjs"],
    },
    packages,
    relationships: packages.map((entry) => ({
      spdxElementId: "SPDXRef-DOCUMENT",
      relationshipType: "DESCRIBES",
      relatedSpdxElement: entry.SPDXID,
    })),
  };
}

function parseArguments(argv) {
  const options = { output: null, projectRoot: defaultProjectRoot };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--output" || argument === "--project-root") {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value.`);
      if (argument === "--output") options.output = resolve(value);
      else options.projectRoot = resolve(value);
      index += 1;
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  if (!options.output) throw new Error("--output is required.");
  return options;
}

function runCli() {
  const options = parseArguments(process.argv.slice(2));
  const inventory = inspectAndroidGradleSupplyChain(options.projectRoot);
  const sbom = createAndroidRuntimeSpdx(inventory);
  mkdirSync(dirname(options.output), { recursive: true });
  writeFileSync(options.output, `${JSON.stringify(sbom, null, 2)}\n`, "utf8");
  console.log(
    `Wrote ${options.output} with ${sbom.packages.length} ${androidReleaseRuntimeConfiguration} packages.`,
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
