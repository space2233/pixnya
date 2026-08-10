import { mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  inspectAndroidGradleSupplyChain,
  inspectAndroidGradleVerificationArtifacts,
} from "./check-android-gradle-supply-chain.mjs";
import { buildGradleLicenseReview } from "./gradle-license-evidence.mjs";

const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = fileURLToPath(new URL("../", import.meta.url));

function parseArguments(argv) {
  const options = {
    output: join(projectRoot, "gradle-license-review.json"),
    gradleUserHome: process.env.GRADLE_USER_HOME,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument !== "--output" && argument !== "--gradle-user-home") {
      throw new Error(`Unknown argument: ${argument}`);
    }
    const value = argv[index + 1];
    if (!value) throw new Error(`${argument} requires a path.`);
    if (argument === "--output") options.output = resolve(value);
    else options.gradleUserHome = resolve(value);
    index += 1;
  }
  return options;
}

function runCli() {
  const options = parseArguments(process.argv.slice(2));
  const inventory = inspectAndroidGradleSupplyChain(projectRoot);
  const review = buildGradleLicenseReview(inventory, {
    gradleUserHome: options.gradleUserHome,
    verifiedArtifacts: inspectAndroidGradleVerificationArtifacts(projectRoot),
  });
  mkdirSync(dirname(options.output), { recursive: true });
  const temporaryOutput = `${options.output}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryOutput, `${JSON.stringify(review, null, 2)}\n`, "utf8");
    rmSync(options.output, { force: true });
    renameSync(temporaryOutput, options.output);
  } catch (error) {
    rmSync(temporaryOutput, { force: true });
    throw error;
  }
  console.log(
    `Wrote ${options.output} with reviewed Maven license evidence for ${review.components.length} locked Gradle components.`,
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
