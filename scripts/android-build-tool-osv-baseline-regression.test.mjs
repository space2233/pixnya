import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  collectObservedFindings,
  createBaseline,
  expectedToolchain,
  scopeDefinitions,
  validateBaseline,
} from "./check-android-build-tool-osv-baseline.mjs";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

const finding = (overrides = {}) => ({
  advisory: "GHSA-2363-cqg2-863c",
  mavenCoordinate: "org.jdom:jdom2",
  version: "2.0.6",
  severity: "HIGH",
  scopes: ["android-buildscript-classpath"],
  fixedVersions: ["2.0.6.1"],
  ...overrides,
});

const reviewedBaseline = (findings) =>
  createBaseline(findings, {
    reviewedAt: "2026-08-09",
    owner: "space2233",
    trackingIssue: "PIXNYA-SEC-ANDROID-BUILD-TOOLS-2026-08",
  });

const osvVulnerability = (overrides = {}) => ({
  id: "GHSA-2363-cqg2-863c",
  database_specific: { severity: "HIGH" },
  affected: [{ ranges: [{ events: [{ introduced: "0" }, { fixed: "2.0.6.1" }] }] }],
  ...overrides,
});

function lockMaps(appConfigurations) {
  const maps = new Map(scopeDefinitions.map((scope) => [scope.id, new Map()]));
  for (const scope of scopeDefinitions) {
    if (scope.lockfile.endsWith("app/gradle.lockfile")) {
      maps.get(scope.id).set("org.jdom:jdom2:2.0.6", appConfigurations);
    }
  }
  return maps;
}

function appReport() {
  return {
    results: [
      {
        source: { path: "/home/runner/work/pixnya/src-tauri/gen/android/app/gradle.lockfile" },
        packages: [
          {
            package: { ecosystem: "Maven", name: "org.jdom:jdom2", version: "2.0.6" },
            vulnerabilities: [osvVulnerability()],
          },
        ],
      },
    ],
  };
}

test("tracked baseline records exact, short-lived, non-runtime findings", async () => {
  const baseline = JSON.parse(await read("docs/android-gradle-osv-risk-baseline.json"));
  assert.equal(baseline.exceptions.length, 82);
  assert.deepEqual(baseline.toolchain, expectedToolchain);
  assert.equal(baseline.policy.runtimeExceptionsAllowed, false);
  assert.ok(baseline.exceptions.every((entry) => entry.severity !== "CRITICAL"));
  assert.deepEqual(
    scopeDefinitions.map((scope) => scope.id),
    [
      "android-buildscript-classpath",
      "android-buildsrc-build-time",
      "android-internal-unified-test-platform",
    ],
  );
  assert.deepEqual(baseline.scopeDefinitions, scopeDefinitions);

  for (const entry of baseline.exceptions) {
    assert.match(entry.advisory, /^GHSA-/);
    assert.match(entry.mavenCoordinate, /^[^:]+:[^:]+$/);
    assert.ok(entry.version);
    assert.ok(entry.scopes.length > 0);
    assert.ok(entry.scopes.every((scope) => scopeDefinitions.some((item) => item.id === scope)));
    assert.equal(entry.owner, "space2233");
    assert.ok(entry.upstreamChain.length >= 3);
    assert.match(entry.unreachableReason, /absent from arm64ReleaseRuntimeClasspath/);
    assert.ok(entry.fixedVersions.length > 0);
    assert.equal(entry.trackingIssue, "PIXNYA-SEC-ANDROID-BUILD-TOOLS-2026-08");
    const isKotlinBuildCacheFinding = entry.advisory === "GHSA-r937-wjx7-w2jp";
    const isUpdatedBouncyCastleFinding = ["GHSA-c3fc-8qff-9hwx", "GHSA-wg6q-6289-32hp"]
      .includes(entry.advisory);
    assert.equal(
      entry.reviewedAt,
      isUpdatedBouncyCastleFinding ? "2026-08-17" : isKotlinBuildCacheFinding ? "2026-08-13" : "2026-08-09",
    );
    assert.equal(
      entry.expiresAt,
      isUpdatedBouncyCastleFinding ? "2026-09-16" : isKotlinBuildCacheFinding ? "2026-09-12" : "2026-09-08",
    );
  }
});

test("the reviewed Android build graph pins the fixed Bouncy Castle family", async () => {
  const [rootBuild, buildSrcBuild, rootLock, buildSrcLock, baselineText] = await Promise.all([
    read("src-tauri/gen/android/build.gradle.kts"),
    read("src-tauri/gen/android/buildSrc/build.gradle.kts"),
    read("src-tauri/gen/android/buildscript-gradle.lockfile"),
    read("src-tauri/gen/android/buildSrc/gradle.lockfile"),
    read("docs/android-gradle-osv-risk-baseline.json"),
  ]);
  for (const module of ["bcprov-jdk18on", "bcpkix-jdk18on", "bcutil-jdk18on"]) {
    const coordinate = `org.bouncycastle:${module}:1.80.2`;
    assert.match(rootBuild, new RegExp(coordinate.replaceAll(".", "\\.")));
    assert.match(buildSrcBuild, new RegExp(coordinate.replaceAll(".", "\\.")));
    assert.match(rootLock, new RegExp(`^${coordinate.replaceAll(".", "\\.")}=`, "m"));
    assert.match(buildSrcLock, new RegExp(`^${coordinate.replaceAll(".", "\\.")}=`, "m"));
    assert.doesNotMatch(rootLock, new RegExp(`^org\\.bouncycastle:${module}:1\\.79=`, "m"));
    assert.doesNotMatch(buildSrcLock, new RegExp(`^org\\.bouncycastle:${module}:1\\.79=`, "m"));
  }
  assert.doesNotMatch(baselineText, /GHSA-574f-3g2m-x479/);
});

test("baseline is exact and rejects new, removed, or changed findings", () => {
  const observed = [finding()];
  const baseline = reviewedBaseline(observed);
  assert.deepEqual(
    validateBaseline(baseline, observed, { asOf: "2026-08-09" }),
    { findingCount: 1, asOf: "2026-08-09" },
  );
  assert.throws(
    () => validateBaseline(baseline, [...observed, finding({ advisory: "GHSA-389x-839f-4rhx" })], { asOf: "2026-08-09" }),
    /1 entries but the report has 2 findings/,
  );
  assert.throws(
    () => validateBaseline(baseline, [{ ...observed[0], severity: "CRITICAL" }], { asOf: "2026-08-09" }),
    /severity changed/,
  );
});

test("normal findings expire in 30 days and critical findings in at most 14 days", () => {
  const high = finding();
  const highBaseline = reviewedBaseline([high]);
  assert.doesNotThrow(() => validateBaseline(highBaseline, [high], { asOf: "2026-09-08" }));
  assert.throws(
    () => validateBaseline(highBaseline, [high], { asOf: "2026-09-09" }),
    /expired on 2026-09-08/,
  );

  const critical = finding({ severity: "CRITICAL" });
  const criticalBaseline = reviewedBaseline([critical]);
  assert.equal(criticalBaseline.exceptions[0].expiresAt, "2026-08-23");
  criticalBaseline.exceptions[0].expiresAt = "2026-08-24";
  assert.throws(
    () => validateBaseline(criticalBaseline, [critical], { asOf: "2026-08-09" }),
    /exceeds the 14-day review window/,
  );
});

test("OSV findings in the ARM64 release runtime can never enter the baseline", () => {
  assert.throws(
    () => collectObservedFindings(appReport(), lockMaps(["arm64ReleaseRuntimeClasspath"])),
    /runtime findings can never use this baseline/,
  );
  const observed = collectObservedFindings(
    appReport(),
    lockMaps(["_internal-unified-test-platform-core"]),
  );
  assert.deepEqual(observed[0].scopes, ["android-internal-unified-test-platform"]);
});

test("release preflight scans raw build-tool locks while runtime OSV remains ignore-free", async () => {
  const workflow = await read(".github/workflows/release.yml");
  assert.match(workflow, /osv-scanner_linux_amd64/);
  assert.match(workflow, /edcfc41d257db36148f065055655fe3fcfc434b0b423ea67468a84c207524e0c/);
  assert.match(workflow, /check-android-build-tool-osv-baseline\.mjs/);
  assert.match(workflow, /android-build-tools-osv\.json/);
  assert.match(workflow, /src-tauri\/gen\/android\/app\/gradle\.lockfile/);
  assert.match(workflow, /src-tauri\/gen\/android\/buildscript-gradle\.lockfile/);
  assert.match(workflow, /src-tauri\/gen\/android\/buildSrc\/gradle\.lockfile/);
  assert.match(
    workflow,
    /"\$OSV_SCANNER" scan --sbom="release-artifacts\/pixnya-\$\{PIXNYA_RELEASE_VERSION\}-android-runtime\.spdx\.json"/,
  );
  assert.doesNotMatch(workflow, /osv-scanner-reusable\.yml|google\/osv-scanner-action/);
  assert.doesNotMatch(workflow, /--sbom=[^\n]+--config|android-runtime[^\n]+ignore/i);
});

test("weekly build-tool audit is pinned, fail-closed, and always preserves its raw report", async () => {
  const workflow = await read(".github/workflows/android-build-tool-audit.yml");
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /schedule:/);
  assert.match(workflow, /cron: "17 3 \* \* 1"/);
  assert.match(workflow, /permissions:\s+contents: read/);
  assert.doesNotMatch(workflow, /contents: write/);
  assert.doesNotMatch(workflow, /uses: [^\n]+@(v\d+|main|master|stable)\s*$/m);
  assert.match(workflow, /actions\/checkout@[0-9a-f]{40} # v6/);
  assert.match(workflow, /actions\/setup-node@[0-9a-f]{40} # v6/);
  assert.match(workflow, /actions\/upload-artifact@[0-9a-f]{40} # v7/);
  assert.equal(
    workflow.match(/uses: actions\/checkout@[0-9a-f]{40}/g)?.length,
    workflow.match(/persist-credentials: false/g)?.length,
  );
  assert.match(workflow, /releases\/download\/v2\.5\.0\/osv-scanner_linux_amd64/);
  assert.match(workflow, /edcfc41d257db36148f065055655fe3fcfc434b0b423ea67468a84c207524e0c/);
  assert.match(workflow, /src-tauri\/gen\/android\/app\/gradle\.lockfile/);
  assert.match(workflow, /src-tauri\/gen\/android\/buildscript-gradle\.lockfile/);
  assert.match(workflow, /src-tauri\/gen\/android\/buildSrc\/gradle\.lockfile/);
  assert.match(workflow, /check-android-build-tool-osv-baseline\.mjs/);
  assert.match(workflow, /if: \$\{\{ always\(\) \}\}/);
  assert.match(workflow, /path: audit-artifacts\/android-build-tools-osv\.json/);
  assert.match(workflow, /if-no-files-found: error/);
  assert.doesNotMatch(workflow, /android-runtime\.spdx\.json|arm64ReleaseRuntimeClasspath/);
});
