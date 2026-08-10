import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultProjectRoot = fileURLToPath(new URL("../", import.meta.url));
const defaultBaselinePath = join(
  defaultProjectRoot,
  "docs",
  "android-gradle-osv-risk-baseline.json",
);

export const baselineSchemaVersion = 1;
export const scannerVersion = "2.5.0";
export const expectedToolchain = Object.freeze({
  tauri: "2.11.5",
  androidGradlePlugin: "8.11.0",
  kotlinGradlePlugin: "1.9.25",
  gradle: "8.14.3",
  jdk: "17",
});

export const scopeDefinitions = Object.freeze([
  {
    id: "android-buildscript-classpath",
    lockfile: "src-tauri/gen/android/buildscript-gradle.lockfile",
    configurationPolicy: "exact:classpath",
    packagedInArm64Runtime: false,
  },
  {
    id: "android-buildsrc-build-time",
    lockfile: "src-tauri/gen/android/buildSrc/gradle.lockfile",
    configurationPolicy: "exact:buildScriptClasspath,runtimeClasspath,testRuntimeClasspath",
    packagedInArm64Runtime: false,
  },
  {
    id: "android-internal-unified-test-platform",
    lockfile: "src-tauri/gen/android/app/gradle.lockfile",
    configurationPolicy: "prefix:_internal-unified-test-platform-",
    packagedInArm64Runtime: false,
  },
]);

const scopeById = new Map(scopeDefinitions.map((scope) => [scope.id, scope]));
const reviewWindowDays = Object.freeze({
  CRITICAL: 14,
  HIGH: 30,
  MODERATE: 30,
  LOW: 30,
});

function requiredText(value, description) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`${description} must be a non-empty string.`);
  }
  return value.trim();
}

function sortedUnique(values) {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

function parseDate(value, description) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    throw new Error(`${description} must use YYYY-MM-DD.`);
  }
  const parsed = new Date(`${value}T00:00:00.000Z`);
  if (Number.isNaN(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== value) {
    throw new Error(`${description} is not a valid calendar date.`);
  }
  return parsed;
}

function addDays(dateText, days) {
  const value = parseDate(dateText, "reviewedAt");
  value.setUTCDate(value.getUTCDate() + days);
  return value.toISOString().slice(0, 10);
}

function normalizePath(value) {
  return value.replaceAll("\\", "/").replace(/^\.\//, "");
}

function parseLockfile(text, relativePath) {
  const entries = new Map();
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || line.startsWith("empty=")) continue;
    const separator = line.lastIndexOf("=");
    if (separator <= 0) throw new Error(`${relativePath} contains an invalid lock entry: ${line}`);
    const coordinate = line.slice(0, separator);
    const configurations = sortedUnique(line.slice(separator + 1).split(",").filter(Boolean));
    if (coordinate.split(":").length !== 3 || configurations.length === 0) {
      throw new Error(`${relativePath} contains an unsupported lock entry: ${line}`);
    }
    entries.set(coordinate, configurations);
  }
  return entries;
}

function loadLockfiles(projectRoot) {
  return new Map(
    scopeDefinitions.map((scope) => [
      scope.id,
      parseLockfile(
        readFileSync(join(projectRoot, ...scope.lockfile.split("/")), "utf8"),
        scope.lockfile,
      ),
    ]),
  );
}

function scopesForReportPath(reportPath) {
  const normalized = normalizePath(requiredText(reportPath, "OSV result source path"));
  const matches = scopeDefinitions.filter(
    (scope) => normalized === scope.lockfile || normalized.endsWith(`/${scope.lockfile}`),
  );
  if (matches.length === 0) {
    throw new Error(`OSV result is outside the reviewed Android lockfiles: ${reportPath}`);
  }
  return matches;
}

function configurationMatches(scope, configurations) {
  const exactPrefix = "exact:";
  if (scope.configurationPolicy.startsWith(exactPrefix)) {
    const expected = scope.configurationPolicy.slice(exactPrefix.length).split(",").sort();
    return configurations.join("\0") === expected.join("\0");
  }
  const prefixPrefix = "prefix:";
  if (scope.configurationPolicy.startsWith(prefixPrefix)) {
    const prefix = scope.configurationPolicy.slice(prefixPrefix.length);
    return configurations.length > 0 && configurations.every((value) => value.startsWith(prefix));
  }
  throw new Error(`Unsupported scope configuration policy: ${scope.configurationPolicy}`);
}

function verifyConfigurations(scopes, configurations, coordinate) {
  if (configurations.includes("arm64ReleaseRuntimeClasspath")) {
    throw new Error(
      `${coordinate} is present in arm64ReleaseRuntimeClasspath; runtime findings can never use this baseline.`,
    );
  }
  const matching = scopes.filter((scope) => configurationMatches(scope, configurations));
  if (matching.length !== 1) {
    throw new Error(`${coordinate} is not confined to one exact reviewed build-only configuration scope.`);
  }
  return matching[0];
}

function fixedVersions(vulnerability) {
  return sortedUnique(
    (vulnerability.affected ?? []).flatMap((affected) =>
      (affected.ranges ?? []).flatMap((range) =>
        (range.events ?? []).map((event) => event.fixed).filter(Boolean),
      ),
    ),
  );
}

function findingKey(advisory, mavenCoordinate, version) {
  return `${advisory}|${mavenCoordinate}|${version}`;
}

export function collectObservedFindings(report, lockfiles) {
  if (!Array.isArray(report?.results) || report.results.length === 0) {
    throw new Error("OSV report contains no lockfile results.");
  }
  const findings = new Map();

  for (const result of report.results) {
    const candidateScopes = scopesForReportPath(result?.source?.path);
    const lockedEntries = lockfiles.get(candidateScopes[0].id);
    if (!(lockedEntries instanceof Map)) {
      throw new Error(`Lock data is missing for ${candidateScopes[0].lockfile}.`);
    }

    for (const packageResult of result.packages ?? []) {
      if (!Array.isArray(packageResult.vulnerabilities) || packageResult.vulnerabilities.length === 0) {
        continue;
      }
      const mavenCoordinate = requiredText(
        packageResult?.package?.name,
        "OSV Maven group/artifact coordinate",
      );
      const version = requiredText(packageResult?.package?.version, "OSV Maven package version");
      if (packageResult?.package?.ecosystem !== "Maven" || mavenCoordinate.split(":").length !== 2) {
        throw new Error(`Only exact Maven findings are accepted: ${mavenCoordinate}@${version}`);
      }
      const lockedCoordinate = `${mavenCoordinate}:${version}`;
      const configurations = lockedEntries.get(lockedCoordinate);
      if (!configurations) {
        throw new Error(
          `${lockedCoordinate} is reported by OSV but absent from ${candidateScopes[0].lockfile}.`,
        );
      }
      const scope = verifyConfigurations(candidateScopes, configurations, lockedCoordinate);

      for (const vulnerability of packageResult.vulnerabilities) {
        const advisory = requiredText(vulnerability.id, "OSV advisory id");
        if (!/^GHSA-[23456789cfghjmpqrvwx]{4}-[23456789cfghjmpqrvwx]{4}-[23456789cfghjmpqrvwx]{4}$/.test(advisory)) {
          throw new Error(`The baseline requires a canonical GHSA id, received ${advisory}.`);
        }
        const severity = requiredText(
          vulnerability?.database_specific?.severity,
          `${advisory} severity`,
        ).toUpperCase();
        if (!(severity in reviewWindowDays)) {
          throw new Error(`${advisory} has unsupported severity ${severity}.`);
        }
        const fixes = fixedVersions(vulnerability);
        if (fixes.length === 0) throw new Error(`${advisory} has no machine-readable fixed version.`);

        const key = findingKey(advisory, mavenCoordinate, version);
        const existing = findings.get(key);
        if (existing) {
          if (existing.severity !== severity || existing.fixedVersions.join("\0") !== fixes.join("\0")) {
            throw new Error(`${key} has inconsistent OSV metadata across lockfiles.`);
          }
          existing.scopes = sortedUnique([...existing.scopes, scope.id]);
        } else {
          findings.set(key, {
            advisory,
            mavenCoordinate,
            version,
            severity,
            scopes: [scope.id],
            fixedVersions: fixes,
          });
        }
      }
    }
  }

  if (findings.size === 0) throw new Error("OSV report contains no findings to compare with the baseline.");
  return [...findings.values()].sort((left, right) =>
    findingKey(left.advisory, left.mavenCoordinate, left.version).localeCompare(
      findingKey(right.advisory, right.mavenCoordinate, right.version),
    ),
  );
}

function inspectToolchain(projectRoot) {
  const rootBuild = readFileSync(join(projectRoot, "src-tauri", "gen", "android", "build.gradle.kts"), "utf8");
  const buildSrcBuild = readFileSync(
    join(projectRoot, "src-tauri", "gen", "android", "buildSrc", "build.gradle.kts"),
    "utf8",
  );
  const wrapper = readFileSync(
    join(projectRoot, "src-tauri", "gen", "android", "gradle", "wrapper", "gradle-wrapper.properties"),
    "utf8",
  );
  const cargoLock = readFileSync(join(projectRoot, "Cargo.lock"), "utf8");
  const workflow = readFileSync(join(projectRoot, ".github", "workflows", "release.yml"), "utf8");
  const match = (source, pattern, description) => {
    const value = source.match(pattern)?.[1];
    if (!value) throw new Error(`Unable to determine ${description} from tracked sources.`);
    return value;
  };
  const androidGradlePlugin = match(
    rootBuild,
    /classpath\("com\.android\.tools\.build:gradle:([^"\n]+)"\)/,
    "Android Gradle Plugin version",
  );
  const buildSrcAgp = match(
    buildSrcBuild,
    /implementation\("com\.android\.tools\.build:gradle:([^"\n]+)"\)/,
    "buildSrc Android Gradle Plugin version",
  );
  if (androidGradlePlugin !== buildSrcAgp) throw new Error("Root and buildSrc AGP versions disagree.");
  return {
    tauri: match(cargoLock, /name = "tauri"\r?\nversion = "([^"]+)"/, "Tauri version"),
    androidGradlePlugin,
    kotlinGradlePlugin: match(
      rootBuild,
      /classpath\("org\.jetbrains\.kotlin:kotlin-gradle-plugin:([^"\n]+)"\)/,
      "Kotlin Gradle Plugin version",
    ),
    gradle: match(wrapper, /gradle-([0-9.]+)-bin\.zip/, "Gradle wrapper version"),
    jdk: match(workflow, /java-version:\s*['"]?([0-9]+)['"]?/, "release JDK version"),
  };
}

function chainFor(finding) {
  const chain = [
    `Tauri ${expectedToolchain.tauri} generated Android project`,
    `com.android.tools.build:gradle:${expectedToolchain.androidGradlePlugin}`,
  ];
  if (
    finding.scopes.some((scope) =>
      ["android-buildscript-classpath", "android-buildsrc-build-time"].includes(scope),
    )
  ) {
    chain.push("Gradle JVM build classpath");
  }
  if (finding.scopes.includes("android-internal-unified-test-platform")) {
    chain.push("Android Gradle Plugin Unified Test Platform internal configuration");
  }
  chain.push(`${finding.mavenCoordinate}:${finding.version}`);
  return chain;
}

function reasonFor(finding) {
  const locations = finding.scopes.includes("android-internal-unified-test-platform")
    ? "AGP internal Unified Test Platform and/or Gradle JVM build classpaths"
    : "Gradle JVM build classpaths";
  const familyReason = finding.mavenCoordinate.startsWith("io.netty:")
    ? "The release tasks do not start UTP, emulator-control, test-result, proxy, HTTP, or HTTP/2 services backed by these Netty modules."
    : finding.mavenCoordinate.startsWith("com.google.protobuf:")
      ? "The affected Protobuf message path belongs to UTP services, and the release does not execute UTP or instrumentation tests."
      : finding.mavenCoordinate.startsWith("org.bouncycastle:")
        ? "The release does not use GOST CTR, LDAP certificate lookup, OpenPGP, or the affected Bouncy Castle paths; APK signing uses Android/JDK signing tooling."
        : finding.mavenCoordinate === "org.apache.commons:commons-compress"
          ? "The build accepts no user-supplied archives; SDKs and dependencies are pinned and their artifacts are SHA-256 verified."
          : finding.mavenCoordinate === "org.bitbucket.b_c:jose4j"
            ? "No release task accepts or validates an external JWT with this build-only jose4j copy."
            : finding.mavenCoordinate === "org.jdom:jdom2"
              ? "No release task parses user-supplied XML with this build-only JDOM copy; project XML is repository controlled."
            : "The affected code path is not executed by the reviewed Android ARM64 release tasks.";
  return (
    `The coordinate is confined to ${locations}; it is absent from ` +
    "arm64ReleaseRuntimeClasspath and is not packaged in the APK. " +
    familyReason
  );
}

export function createBaseline(observed, { reviewedAt, owner, trackingIssue }) {
  parseDate(reviewedAt, "reviewedAt");
  requiredText(owner, "owner");
  requiredText(trackingIssue, "trackingIssue");
  return {
    schemaVersion: baselineSchemaVersion,
    scanner: { name: "OSV-Scanner", version: scannerVersion },
    toolchain: { ...expectedToolchain },
    policy: {
      purpose: "Temporary fail-closed exceptions for build-only and AGP internal-test findings",
      runtimeExceptionsAllowed: false,
      normalReviewWindowDays: 30,
      criticalReviewWindowDays: 14,
    },
    scopeDefinitions: scopeDefinitions.map((scope) => ({ ...scope })),
    exceptions: observed.map((finding) => ({
      advisory: finding.advisory,
      mavenCoordinate: finding.mavenCoordinate,
      version: finding.version,
      severity: finding.severity,
      scopes: [...finding.scopes],
      owner,
      upstreamChain: chainFor(finding),
      unreachableReason: reasonFor(finding),
      fixedVersions: [...finding.fixedVersions],
      trackingIssue,
      reviewedAt,
      expiresAt: addDays(reviewedAt, reviewWindowDays[finding.severity]),
    })),
  };
}

function assertEqualJson(actual, expected, description) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${description} does not match the reviewed value.`);
  }
}

export function validateBaseline(baseline, observed, { asOf, actualToolchain = expectedToolchain } = {}) {
  const today = asOf ?? new Date().toISOString().slice(0, 10);
  const todayDate = parseDate(today, "asOf");
  if (baseline?.schemaVersion !== baselineSchemaVersion) {
    throw new Error(`Unsupported Android OSV baseline schema: ${baseline?.schemaVersion}.`);
  }
  assertEqualJson(baseline.scanner, { name: "OSV-Scanner", version: scannerVersion }, "Scanner pin");
  assertEqualJson(baseline.toolchain, expectedToolchain, "Baseline toolchain");
  assertEqualJson(actualToolchain, expectedToolchain, "Tracked Android toolchain");
  assertEqualJson(baseline.scopeDefinitions, scopeDefinitions, "Scope definitions");
  if (baseline?.policy?.runtimeExceptionsAllowed !== false) {
    throw new Error("Android ARM64 runtime exceptions are forbidden.");
  }

  const entries = baseline.exceptions;
  if (!Array.isArray(entries) || entries.length === 0) throw new Error("OSV baseline has no exceptions.");
  if (entries.length !== observed.length) {
    throw new Error(`OSV baseline has ${entries.length} entries but the report has ${observed.length} findings.`);
  }

  for (let index = 0; index < observed.length; index += 1) {
    const expected = observed[index];
    const entry = entries[index];
    const key = findingKey(expected.advisory, expected.mavenCoordinate, expected.version);
    if (findingKey(entry?.advisory, entry?.mavenCoordinate, entry?.version) !== key) {
      throw new Error(`OSV baseline/report mismatch at index ${index}; expected ${key}.`);
    }
    if (entry.severity !== expected.severity) throw new Error(`${key} severity changed.`);
    assertEqualJson(entry.scopes, expected.scopes, `${key} scopes`);
    assertEqualJson(entry.fixedVersions, expected.fixedVersions, `${key} fixed versions`);
    if (entry.scopes.some((scope) => !scopeById.has(scope))) {
      throw new Error(`${key} references an unknown scope.`);
    }
    requiredText(entry.owner, `${key} owner`);
    requiredText(entry.trackingIssue, `${key} trackingIssue`);
    if (!/^PIXNYA-SEC-[A-Z0-9-]+$/.test(entry.trackingIssue)) {
      throw new Error(`${key} trackingIssue must be a stable PIXNYA-SEC-* id.`);
    }
    if (!Array.isArray(entry.upstreamChain) || entry.upstreamChain.length < 3) {
      throw new Error(`${key} must record its upstream chain.`);
    }
    entry.upstreamChain.forEach((value, chainIndex) =>
      requiredText(value, `${key} upstreamChain[${chainIndex}]`),
    );
    const reason = requiredText(entry.unreachableReason, `${key} unreachableReason`);
    if (!reason.includes("absent from arm64ReleaseRuntimeClasspath") || !reason.includes("not packaged")) {
      throw new Error(`${key} must document why it is absent from the ARM64 runtime.`);
    }
    const reviewedAt = parseDate(entry.reviewedAt, `${key} reviewedAt`);
    const expiresAt = parseDate(entry.expiresAt, `${key} expiresAt`);
    if (reviewedAt > todayDate) throw new Error(`${key} was reviewed in the future.`);
    if (expiresAt < todayDate) throw new Error(`${key} expired on ${entry.expiresAt}.`);
    const maximumExpiry = parseDate(
      addDays(entry.reviewedAt, reviewWindowDays[entry.severity]),
      `${key} maximum expiry`,
    );
    if (expiresAt <= reviewedAt || expiresAt > maximumExpiry) {
      throw new Error(
        `${key} expiry exceeds the ${reviewWindowDays[entry.severity]}-day review window.`,
      );
    }
  }
  return { findingCount: observed.length, asOf: today };
}

function parseArguments(argv) {
  const options = { initialize: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--initialize") {
      options.initialize = true;
      continue;
    }
    if (["--report", "--baseline", "--as-of", "--reviewed-at", "--owner", "--tracking-issue"].includes(argument)) {
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value.`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value;
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${argument}`);
  }
  if (!options.report) throw new Error("--report is required.");
  return options;
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const projectRoot = defaultProjectRoot;
  const reportPath = resolve(projectRoot, options.report);
  const baselinePath = resolve(projectRoot, options.baseline ?? defaultBaselinePath);
  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const observed = collectObservedFindings(report, loadLockfiles(projectRoot));
  if (options.initialize) {
    const baseline = createBaseline(observed, {
      reviewedAt: options.reviewedAt,
      owner: options.owner,
      trackingIssue: options.trackingIssue,
    });
    writeFileSync(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`, "utf8");
    console.log(`Initialized ${baselinePath} with ${observed.length} reviewed build-only findings.`);
    return;
  }
  const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
  const result = validateBaseline(baseline, observed, {
    asOf: options.asOf,
    actualToolchain: inspectToolchain(projectRoot),
  });
  console.log(
    `Android build-tool OSV baseline passed for ${result.findingCount} exact findings as of ${result.asOf}.`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error?.message ?? error);
    process.exitCode = 1;
  }
}
