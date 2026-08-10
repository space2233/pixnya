import {
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  inspectAndroidGradleSupplyChain,
  inspectAndroidGradleVerificationArtifacts,
} from "./check-android-gradle-supply-chain.mjs";
import {
  discoverGradlePomVerificationEntries,
  hydrateGradlePomEvidence,
  reviewedMavenPomRepositories,
} from "./gradle-license-evidence.mjs";

const projectRoot = fileURLToPath(new URL("../", import.meta.url));

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!["--gradle-user-home", "--manifest"].includes(argument)) {
      throw new Error(`Unknown argument: ${argument}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a path.`);
    options[argument === "--manifest" ? "manifest" : "gradleUserHome"] = resolve(value);
    index += 1;
  }
  if (!options.gradleUserHome) {
    throw new Error("--gradle-user-home is required; POM evidence must use a new isolated cache.");
  }
  options.manifest ??= join(options.gradleUserHome, "pixnya-gradle-pom-evidence.json");
  return options;
}

function writeAtomically(destination, contents) {
  mkdirSync(dirname(destination), { recursive: true });
  const temporary = `${destination}.tmp-${process.pid}`;
  try {
    writeFileSync(temporary, contents, { flag: "wx" });
    rmSync(destination, { force: true });
    renameSync(temporary, destination);
  } finally {
    rmSync(temporary, { force: true });
  }
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  mkdirSync(options.gradleUserHome, { recursive: true });
  const existing = readdirSync(options.gradleUserHome);
  if (existing.length > 0) {
    throw new Error(
      `The isolated Maven POM evidence cache must start empty; found: ${existing.sort().join(", ")}`,
    );
  }

  const inventory = inspectAndroidGradleSupplyChain(projectRoot);
  const verifiedArtifacts = inspectAndroidGradleVerificationArtifacts(projectRoot);
  const hydration = await hydrateGradlePomEvidence(inventory, {
    gradleUserHome: options.gradleUserHome,
    verifiedArtifacts,
  });
  const discovered = discoverGradlePomVerificationEntries(inventory, {
    gradleUserHome: options.gradleUserHome,
  }).map(({ coordinate, pomName, sha256 }) => ({ coordinate, pomName, sha256 }));
  if (JSON.stringify(discovered) !== JSON.stringify(hydration.poms)) {
    throw new Error(
      "The hydrated Maven POM manifest does not exactly match the recursively discovered license evidence set.",
    );
  }

  const manifest = {
    schemaVersion: 1,
    gradleFingerprint: inventory.fingerprint,
    repositories: [...reviewedMavenPomRepositories],
    componentCount: hydration.componentCount,
    pomCount: hydration.pomCount,
    poms: hydration.poms,
  };
  writeAtomically(options.manifest, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(
    `Hydrated ${hydration.pomCount} checksum-verified Maven POMs for ` +
      `${hydration.componentCount} locked Gradle components (${hydration.downloadedCount} downloaded).`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error?.message ?? error);
    process.exitCode = 1;
  });
}
