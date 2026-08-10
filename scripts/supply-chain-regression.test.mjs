import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  CANONICAL_GPL3_SHA256,
  assertCanonicalProjectLicense,
  assertKnownDependencyLicenses,
  buildSpdxDocument,
  cargoMetadataArguments,
  checkTrackedSupplyChainFiles,
  collectCargoPackages,
  collectNpmPackages,
  computeLockFingerprint,
  normalizeSpdxLicenseExpression,
  parseCargoLockChecksums,
  renderThirdPartyNotices,
  writeThirdPartyLicenseBundle,
} from "./generate-supply-chain-artifacts.mjs";
import {
  buildGradleLicenseReview,
  gradlePackagesFromReview,
  validateGradleLicenseReview,
} from "./gradle-license-evidence.mjs";
import {
  inspectAndroidGradleSupplyChain,
  parseVerificationMetadata,
} from "./check-android-gradle-supply-chain.mjs";
import { mergePomVerificationMetadata } from "./complete-gradle-pom-verification.mjs";

const projectRoot = fileURLToPath(new URL("../", import.meta.url));
const execFileAsync = promisify(execFile);

test("the project ships the unmodified canonical GPL-3.0 text", () => {
  assert.equal(CANONICAL_GPL3_SHA256.length, 64);
  assert.doesNotThrow(() =>
    assertCanonicalProjectLicense(readFileSync(`${projectRoot}/LICENSE`, "utf8")),
  );
  assert.throws(
    () => assertCanonicalProjectLicense("GNU GENERAL PUBLIC LICENSE\nVersion 3\n"),
    /not the unmodified canonical GPL-3.0 text/,
  );
});

test("tracked notices are tied to both lockfiles without consulting the network", () => {
  const packageLockText = readFileSync(`${projectRoot}/package-lock.json`, "utf8");
  const cargoLockText = readFileSync(`${projectRoot}/Cargo.lock`, "utf8");
  const gradleInventory = inspectAndroidGradleSupplyChain(projectRoot);
  const expected = computeLockFingerprint(
    packageLockText,
    cargoLockText,
    gradleInventory.fingerprint,
  );
  assert.equal(checkTrackedSupplyChainFiles(projectRoot), expected);
  assert.deepEqual(cargoMetadataArguments, [
    "metadata",
    "--locked",
    "--offline",
    "--format-version",
    "1",
  ]);
});

test("npm inventory reads exact versions, licenses and integrity from lockfile v3", () => {
  const lockfile = {
    lockfileVersion: 3,
    packages: {
      "": { name: "fixture", version: "1.0.0" },
      "node_modules/@scope/runtime": {
        version: "2.3.4",
        resolved: "https://registry.example/@scope/runtime-2.3.4.tgz",
        integrity: `sha512-${Buffer.from("fixture checksum").toString("base64")}`,
        license: "MIT",
      },
    },
  };
  const packages = collectNpmPackages(lockfile, { root: join("fixture", "project") });
  assert.equal(packages.length, 1);
  assert.deepEqual(
    {
      name: packages[0].name,
      version: packages[0].version,
      license: packages[0].license,
      purl: packages[0].purl,
      algorithm: packages[0].checksum.algorithm,
    },
    {
      name: "@scope/runtime",
      version: "2.3.4",
      license: "MIT",
      purl: "pkg:npm/%40scope/runtime@2.3.4",
      algorithm: "SHA512",
    },
  );
  assert.equal(
    packages[0].sourceDirectory,
    join("fixture", "project", "node_modules", "@scope", "runtime"),
  );
});

test("Cargo inventory excludes workspace crates and preserves locked checksums", () => {
  const cargoLock = `version = 4

[[package]]
name = "fixture-crate"
version = "1.2.3"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[[package]]
name = "workspace-crate"
version = "0.1.0"
`;
  const metadata = {
    workspace_members: ["path+file:///workspace#workspace-crate@0.1.0"],
    packages: [
      {
        id: "path+file:///workspace#workspace-crate@0.1.0",
        name: "workspace-crate",
        version: "0.1.0",
        source: null,
        license: "GPL-3.0-only",
      },
      {
        id: "registry+https://github.com/rust-lang/crates.io-index#fixture-crate@1.2.3",
        name: "fixture-crate",
        version: "1.2.3",
          source: "registry+https://github.com/rust-lang/crates.io-index",
          license: "Apache-2.0 OR MIT",
          manifest_path: join("fixture", "cargo", "fixture-crate", "Cargo.toml"),
        },
    ],
  };
  assert.equal(parseCargoLockChecksums(cargoLock).size, 1);
  const packages = collectCargoPackages(metadata, cargoLock);
  assert.equal(packages.length, 1);
  assert.equal(packages[0].license, "Apache-2.0 OR MIT");
  assert.equal(packages[0].checksum.algorithm, "SHA256");
  assert.equal(packages[0].checksum.checksumValue.length, 64);
  assert.equal(packages[0].sourceDirectory, join("fixture", "cargo", "fixture-crate"));
});

test("Gradle license evidence resolves inherited POM declarations and stays tied to the lock graph", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "pixnya-gradle-license-review-"));
  try {
    const cacheRoot = join(temporaryRoot, "caches", "modules-2", "files-2.1");
    const writePom = (group, name, version, digestDirectory, content) => {
      const directory = join(cacheRoot, group, name, version, digestDirectory);
      mkdirSync(directory, { recursive: true });
      writeFileSync(join(directory, `${name}-${version}.pom`), content);
      return createHash("sha256").update(content).digest("hex");
    };
    const childPomSha256 = writePom(
      "example.child",
      "runtime",
      "1.0.0",
      "child-digest",
      `<project><parent><groupId>example.parent</groupId><artifactId>parent</artifactId><version>2.0.0</version></parent><url>https://example.invalid/runtime</url></project>`,
    );
    const parentPomSha256 = writePom(
      "example.parent",
      "parent",
      "2.0.0",
      "parent-digest",
      `<project><licenses><license><name>The Apache License, Version 2.0</name><url>http://www.apache.org/licenses/LICENSE-2.0.txt</url></license></licenses></project>`,
    );
    const inventory = {
      schemaVersion: 1,
      fingerprint: `sha256:${"a".repeat(64)}`,
      components: [
        {
          coordinate: "example.child:runtime:1.0.0",
          lockfiles: ["app/gradle.lockfile"],
          configurations: ["arm64ReleaseRuntimeClasspath"],
          artifacts: [
            { name: "runtime-1.0.0.jar", sha256: ["b".repeat(64)] },
          ],
        },
      ],
    };

    const verifiedArtifacts = new Map([
      [
        "example.child:runtime:1.0.0",
        [{ name: "runtime-1.0.0.pom", sha256: [childPomSha256] }],
      ],
      [
        "example.parent:parent:2.0.0",
        [{ name: "parent-2.0.0.pom", sha256: [parentPomSha256] }],
      ],
    ]);
    const review = buildGradleLicenseReview(inventory, {
      gradleUserHome: temporaryRoot,
      verifiedArtifacts,
    });
    assert.equal(review.components.length, 1);
    assert.equal(review.components[0].spdxLicense, "Apache-2.0");
    assert.equal(review.components[0].licenseEvidence.sourceCoordinate, "example.parent:parent:2.0.0");
    assert.doesNotThrow(() => validateGradleLicenseReview(inventory, review));

    const packages = gradlePackagesFromReview(inventory, review);
    assert.equal(packages[0].ecosystem, "maven");
    assert.equal(packages[0].purl, "pkg:maven/example.child/runtime@1.0.0");
    assert.equal(packages[0].declarationEvidence.length, 1);

    assert.throws(
      () => validateGradleLicenseReview({ ...inventory, fingerprint: `sha256:${"c".repeat(64)}` }, review),
      /stale for the locked Gradle graph/i,
    );
    assert.throws(
      () =>
        buildGradleLicenseReview(inventory, {
          gradleUserHome: temporaryRoot,
          verifiedArtifacts: new Map([
            [
              "example.child:runtime:1.0.0",
              [{ name: "runtime-1.0.0.pom", sha256: [childPomSha256] }],
            ],
          ]),
        }),
      /parent:2\.0\.0 cached POM is not covered by tracked Gradle SHA-256/i,
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("Gradle license review fails closed on an unknown Maven declaration", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "pixnya-gradle-license-unknown-"));
  try {
    const directory = join(
      temporaryRoot,
      "caches",
      "modules-2",
      "files-2.1",
      "example",
      "unknown",
      "1.0.0",
      "digest",
    );
    mkdirSync(directory, { recursive: true });
    writeFileSync(
      join(directory, "unknown-1.0.0.pom"),
      `<project><licenses><license><name>Unreviewed Custom Terms</name><url>https://example.invalid/license</url></license></licenses></project>`,
    );
    assert.throws(
      () =>
        buildGradleLicenseReview(
          {
            schemaVersion: 1,
            fingerprint: `sha256:${"d".repeat(64)}`,
            components: [
              {
                coordinate: "example:unknown:1.0.0",
                lockfiles: ["app/gradle.lockfile"],
                configurations: ["runtimeClasspath"],
                artifacts: [],
              },
            ],
          },
          {
            gradleUserHome: temporaryRoot,
            verifiedArtifacts: new Map([
              [
                "example:unknown:1.0.0",
                [
                  {
                    name: "unknown-1.0.0.pom",
                    sha256: [
                      createHash("sha256")
                        .update(
                          `<project><licenses><license><name>Unreviewed Custom Terms</name><url>https://example.invalid/license</url></license></licenses></project>`,
                        )
                        .digest("hex"),
                    ],
                  },
                ],
              ],
            ]),
          },
        ),
      /unreviewed Maven license declaration/i,
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("reviewed Maven POM digests extend Gradle verification metadata idempotently", () => {
  const metadata = `<?xml version="1.0" encoding="UTF-8"?>
<verification-metadata>
   <configuration><verify-metadata>true</verify-metadata><verify-signatures>false</verify-signatures></configuration>
   <components>
      <component group="example" name="child" version="1.0.0">
         <artifact name="child-1.0.0.jar">
            <sha256 value="${"a".repeat(64)}" origin="fixture"/>
         </artifact>
      </component>
   </components>
</verification-metadata>
`;
  const entries = [
    {
      coordinate: "example:child:1.0.0",
      pomName: "child-1.0.0.pom",
      sha256: "b".repeat(64),
    },
    {
      coordinate: "example:parent:2.0.0",
      pomName: "parent-2.0.0.pom",
      sha256: "c".repeat(64),
    },
  ];
  const first = mergePomVerificationMetadata(metadata, entries);
  assert.equal(first.addedCount, 2);
  const verified = parseVerificationMetadata(first.text);
  assert.deepEqual(
    verified.get("example:child:1.0.0").find((entry) => entry.name.endsWith(".pom")).sha256,
    ["b".repeat(64)],
  );
  assert.deepEqual(verified.get("example:parent:2.0.0")[0].sha256, ["c".repeat(64)]);
  const second = mergePomVerificationMetadata(first.text, entries);
  assert.equal(second.addedCount, 0);
  assert.equal(second.text, first.text);
  assert.throws(
    () =>
      mergePomVerificationMetadata(first.text, [
        { ...entries[0], sha256: "d".repeat(64) },
      ]),
    /does not match its existing tracked SHA-256/i,
  );
});

test("SPDX 2.3 SBOM describes PixNya and every locked dependency", () => {
  const npmPackages = collectNpmPackages({
    lockfileVersion: 3,
    packages: {
      "": { name: "fixture", version: "1.0.0" },
      "node_modules/runtime": { version: "1.0.0", license: "MIT" },
    },
  });
  const cargoPackages = collectCargoPackages(
    {
      workspace_members: [],
      packages: [
        {
          id: "registry+https://github.com/rust-lang/crates.io-index#rust-runtime@1.0.0",
          name: "rust-runtime",
          version: "1.0.0",
          source: "registry+https://github.com/rust-lang/crates.io-index",
          license: "BSD-3-Clause",
        },
      ],
    },
    "",
  );
  const gradlePackages = [
    {
      ecosystem: "maven",
      name: "example:android-tool",
      version: "1.0.0",
      license: "Example Android terms",
      spdxLicense: "LicenseRef-Example-Android",
      resolved: "NOASSERTION",
      purl: "pkg:maven/example/android-tool@1.0.0",
      identity: "maven:example:android-tool:1.0.0",
      development: true,
      licenseReferences: [
        {
          licenseId: "LicenseRef-Example-Android",
          name: "Example Android terms",
          classification: "upstream-metadata-only",
          extractedText: "The upstream Maven POM declares Example Android terms for this build component.",
          seeAlso: ["https://example.invalid/android-terms"],
        },
      ],
    },
  ];
  const packages = [...npmPackages, ...cargoPackages, ...gradlePackages];
  assert.doesNotThrow(() => assertKnownDependencyLicenses(packages));

  const fingerprint = "a".repeat(64);
  const notices = renderThirdPartyNotices({
    version: "1.0.0",
    fingerprint,
    npmPackages,
    cargoPackages,
    gradlePackages,
  });
  assert.match(notices, /runtime \| 1\.0\.0 \| MIT/);
  assert.match(notices, /rust-runtime\]\([^)]*\) \| 1\.0\.0 \| BSD-3-Clause/);
  assert.match(notices, /Android Gradle\/Maven locked components: 1/);
  assert.match(notices, /example:android-tool \| 1\.0\.0 \| Example Android terms/);

  const sbom = buildSpdxDocument({
    version: "1.0.0",
    fingerprint,
    packages,
    created: "2026-08-09T00:00:00Z",
  });
  assert.equal(sbom.spdxVersion, "SPDX-2.3");
  assert.equal(sbom.packages.length, 4);
  assert.equal(sbom.relationships.length, 4);
  assert.equal(sbom.packages[0].licenseDeclared, "GPL-3.0-only");
  assert.equal(
    sbom.relationships.filter((entry) => entry.relationshipType === "DEPENDS_ON").length,
    3,
  );
  assert.deepEqual(sbom.hasExtractedLicensingInfos, [
    {
      licenseId: "LicenseRef-Example-Android",
      extractedText: "The upstream Maven POM declares Example Android terms for this build component.",
      name: "Example Android terms",
      comment: "Evidence classification: upstream-metadata-only",
      seeAlsos: ["https://example.invalid/android-terms"],
    },
  ]);
});

test("strict license validation fails closed on unreviewed metadata", () => {
  assert.throws(
    () =>
      assertKnownDependencyLicenses([
        { identity: "npm:unknown@1.0.0", license: "NOASSERTION" },
      ]),
    /no verified license metadata/,
  );
});

test("legacy Cargo dual-license separators become valid SPDX OR expressions", () => {
  assert.equal(normalizeSpdxLicenseExpression("MIT/Apache-2.0"), "MIT OR Apache-2.0");
  assert.equal(normalizeSpdxLicenseExpression("BSD-3-Clause/MIT"), "BSD-3-Clause OR MIT");
  assert.equal(normalizeSpdxLicenseExpression("Apache-2.0 AND MIT"), "Apache-2.0 AND MIT");
});

test("offline license bundle preserves upstream files and supplies reviewed SPDX text", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "pixnya-license-bundle-"));
  try {
    const upstreamRoot = join(temporaryRoot, "upstream");
    const fallbackRoot = join(temporaryRoot, "fallback");
    const outputDirectory = join(temporaryRoot, "bundle");
    mkdirSync(upstreamRoot);
    mkdirSync(fallbackRoot);
    writeFileSync(join(upstreamRoot, "LICENSE-MIT"), "exact upstream MIT text\n");
    writeFileSync(join(upstreamRoot, "NOTICE"), "upstream attribution\n");

    const summary = writeThirdPartyLicenseBundle({
      outputDirectory,
      packages: [
        {
          ecosystem: "npm",
          name: "has-upstream-license",
          version: "1.0.0",
          license: "MIT",
          spdxLicense: "MIT",
          identity: "npm:has-upstream-license@1.0.0",
          resolved: "https://registry.example/has-upstream-license.tgz",
          purl: "pkg:npm/has-upstream-license@1.0.0",
          sourceDirectory: upstreamRoot,
        },
        {
          ecosystem: "cargo",
          name: "needs-standard-text",
          version: "2.0.0",
          license: "Apache-2.0 WITH LLVM-exception",
          spdxLicense: "Apache-2.0 WITH LLVM-exception",
          identity: "cargo:needs-standard-text@2.0.0",
          resolved: "https://crates.io/crates/needs-standard-text/2.0.0/download",
          purl: "pkg:cargo/needs-standard-text@2.0.0",
          sourceDirectory: fallbackRoot,
          authors: ["Fixture Authors"],
        },
        {
          ecosystem: "maven",
          name: "example:metadata-only",
          version: "3.0.0",
          license: "Example upstream terms",
          spdxLicense: "LicenseRef-Example-Terms",
          identity: "maven:example:metadata-only:3.0.0",
          resolved: "https://example.invalid/component",
          purl: "pkg:maven/example/metadata-only@3.0.0",
          sourceDirectory: null,
          declarationEvidence: [
            {
              name: "MAVEN-LICENSE-DECLARATION.json",
              content: '{"license":"Example upstream terms"}\n',
            },
          ],
          licenseReferences: [
            {
              licenseId: "LicenseRef-Example-Terms",
              classification: "upstream-metadata-only",
              extractedText:
                "The upstream Maven declaration identifies Example upstream terms at its recorded URL.",
              seeAlso: ["https://example.invalid/license"],
            },
          ],
        },
      ],
      licenseCatalog: {
        "Apache-2.0": { name: "Apache License 2.0", licenseText: "canonical Apache text\n" },
      },
      exceptionCatalog: {
        "LLVM-exception": "canonical LLVM exception text\n",
      },
    });

    assert.deepEqual(summary, {
      packageCount: 3,
      upstreamLicenseFileCount: 1,
      noticeFileCount: 1,
      declarationEvidenceFileCount: 1,
      fallbackPackageCount: 2,
      classifiedReferencePackageCount: 1,
    });
    const index = JSON.parse(await readFile(join(outputDirectory, "INDEX.json"), "utf8"));
    assert.equal(index.packages.length, 3);
    const packageDirectories = await readdir(join(outputDirectory, "packages"));
    assert.equal(packageDirectories.length, 3);
    const bundleText = await Promise.all(
      packageDirectories.flatMap((directory) =>
        [
          "LICENSE-MIT",
          "NOTICE",
          "SPDX-Apache-2.0.txt",
          "SPDX-EXCEPTION-LLVM-exception.txt",
          "MAVEN-LICENSE-DECLARATION.json",
          "LICENSE-REFERENCE-LicenseRef-Example-Terms.txt",
        ].map(
          async (name) => {
            try {
              return await readFile(join(outputDirectory, "packages", directory, name), "utf8");
            } catch {
              return "";
            }
          },
        ),
      ),
    );
    assert.ok(bundleText.includes("exact upstream MIT text\n"));
    assert.ok(bundleText.includes("upstream attribution\n"));
    assert.ok(bundleText.includes("canonical Apache text\n"));
    assert.ok(bundleText.includes("canonical LLVM exception text\n"));
    assert.ok(bundleText.includes('{"license":"Example upstream terms"}\n'));
    assert.ok(
      bundleText.includes(
        "The upstream Maven declaration identifies Example upstream terms at its recorded URL.\n",
      ),
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("offline license bundle fails closed when no upstream or canonical text exists", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "pixnya-license-bundle-missing-"));
  try {
    const sourceDirectory = join(temporaryRoot, "dependency");
    mkdirSync(sourceDirectory);
    assert.throws(
      () =>
        writeThirdPartyLicenseBundle({
          outputDirectory: join(temporaryRoot, "bundle"),
          packages: [
            {
              ecosystem: "cargo",
              name: "unknown-license",
              version: "1.0.0",
              license: "LicenseRef-Unreviewed",
              spdxLicense: "LicenseRef-Unreviewed",
              identity: "cargo:unknown-license@1.0.0",
              resolved: "NOASSERTION",
              purl: "pkg:cargo/unknown-license@1.0.0",
              sourceDirectory,
            },
          ],
          licenseCatalog: {},
          exceptionCatalog: {},
        }),
      /no reviewed full text.*LicenseRef-Unreviewed/i,
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("offline license bundle rejects an upstream license placeholder", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "pixnya-license-bundle-stub-"));
  try {
    const sourceDirectory = join(temporaryRoot, "dependency");
    mkdirSync(sourceDirectory);
    writeFileSync(join(sourceDirectory, "LICENSE"), "stub\n");
    assert.throws(
      () =>
        writeThirdPartyLicenseBundle({
          outputDirectory: join(temporaryRoot, "bundle"),
          packages: [
            {
              ecosystem: "npm",
              name: "stub-license",
              version: "1.0.0",
              license: "LicenseRef-Unreviewed",
              spdxLicense: "LicenseRef-Unreviewed",
              identity: "npm:stub-license@1.0.0",
              resolved: "https://registry.example/stub-license.tgz",
              purl: "pkg:npm/stub-license@1.0.0",
              sourceDirectory,
            },
          ],
          licenseCatalog: {},
        }),
      /no reviewed full text.*LicenseRef-Unreviewed/i,
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("supply-chain CLI emits a complete offline license directory for the locked graph", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "pixnya-license-cli-"));
  try {
    const licensesDirectory = join(temporaryRoot, "third-party-licenses");
    await execFileAsync(
      process.execPath,
      [
        join(projectRoot, "scripts", "generate-supply-chain-artifacts.mjs"),
        "--notices",
        join(temporaryRoot, "THIRD_PARTY_NOTICES.md"),
        "--sbom",
        join(temporaryRoot, "pixnya.spdx.json"),
        "--licenses-dir",
        licensesDirectory,
      ],
      {
        cwd: projectRoot,
        windowsHide: true,
        env: { ...process.env, SOURCE_DATE_EPOCH: "1786233600" },
        maxBuffer: 16 * 1024 * 1024,
      },
    );
    const index = JSON.parse(await readFile(join(licensesDirectory, "INDEX.json"), "utf8"));
    assert.equal(index.packageCount, index.packages.length);
    assert.ok(index.packageCount > 500);
    assert.ok(index.upstreamLicenseFileCount > 0);
    assert.ok(index.fallbackPackageCount > 0);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});
