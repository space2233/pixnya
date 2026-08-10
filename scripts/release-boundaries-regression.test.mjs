import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { validateStableReleaseNotes } from "./validate-release-notes.mjs";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");
const readGenerated = async (relativePath) => {
  try {
    return await read(relativePath);
  } catch (error) {
    if (error?.code === "ENOENT") {
      return null;
    }
    throw error;
  }
};

test("all user-visible package versions agree on the 0.29.0 feature release", async () => {
  const [workspace, packageJson, packageLock, tauri, androidProperties, androidIgnore, settings, readme] = await Promise.all([
    read("Cargo.toml"),
    read("package.json"),
    read("package-lock.json"),
    read("src-tauri/tauri.conf.json"),
    readGenerated("src-tauri/gen/android/app/tauri.properties"),
    read("src-tauri/gen/android/app/.gitignore"),
    read("src/routes/settings/+page.svelte"),
    read("README.md"),
  ]);
  assert.match(workspace, /version = "0\.29\.0"/);
  assert.equal(JSON.parse(packageJson).version, "0.29.0");
  assert.equal(JSON.parse(packageLock).version, "0.29.0");
  assert.equal(JSON.parse(tauri).version, "0.29.0");
  assert.match(androidIgnore, /^\/tauri\.properties$/m);
  if (androidProperties !== null) {
    assert.match(androidProperties, /tauri\.android\.versionName=0\.29\.0/);
    assert.match(androidProperties, /tauri\.android\.versionCode=29000/);
  }
  assert.match(settings, /appStatus\?\.version \?\? "0\.29\.0"/);
  assert.match(readme, /当前源码版本 `0\.29\.0` 是首个稳定版的候选基线/);
});

test("Android releases ARM64 while retaining ARMv7 as a deferred manual target", async () => {
  const [packageJson, arm64, armv7, workflow, manifestGenerator] = await Promise.all([
    read("package.json"),
    read("scripts/build-android-arm64-debug.ps1"),
    read("scripts/build-android-armv7-debug.ps1"),
    read(".github/workflows/release.yml"),
    read("scripts/generate-android-update-manifest.mjs"),
  ]);
  const scripts = JSON.parse(packageJson).scripts;
  assert.ok(scripts["build:android:arm64:debug"]);
  assert.ok(scripts["build:android:armv7:debug"]);
  assert.match(arm64, /--target aarch64 --split-per-abi/);
  assert.match(armv7, /--target armv7 --split-per-abi/);
  assert.match(workflow, /tauriTarget: aarch64/);
  assert.doesNotMatch(workflow, /tauriTarget: armv7/);
  assert.doesNotMatch(workflow, /--armv7/);
  assert.match(manifestGenerator, /optionalArmv7/);
});

test("Linux verification compiles the actual Tauri desktop target", async () => {
  const [workflow, script, runner] = await Promise.all([
    read(".github/workflows/linux.yml"),
    read("scripts/check-linux.sh"),
    read("scripts/run-test-suite.mjs"),
  ]);
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
  assert.match(workflow, /npm run test:quick/);
  assert.match(workflow, /bash scripts\/check-linux\.sh rust-only/);
  assert.match(script, /npm run test:rust/);
  assert.match(script, /npx tauri build --debug --no-bundle/);
  assert.match(runner, /"test", "--workspace"/);
});

test("formal releases are gated by main-branch full verification and signed artifact checks", async () => {
  const [workflow, androidBridgeGenerator, androidIgnore, androidAppIgnore, androidAppBuild] = await Promise.all([
    read(".github/workflows/release.yml"),
    read("scripts/generate-tauri-android-gradle-bridge.mjs"),
    read("src-tauri/gen/android/.gitignore"),
    read("src-tauri/gen/android/app/.gitignore"),
    read("src-tauri/gen/android/app/build.gradle.kts"),
  ]);
  assert.match(workflow, /Require the main release source/);
  assert.match(workflow, /refs\/heads\/main/);
  assert.match(workflow, /npm run test:full/);
  assert.match(workflow, /npm audit --omit=dev --audit-level=low/);
  assert.match(workflow, /npm audit --audit-level=high/);
  assert.match(workflow, /EmbarkStudios\/cargo-deny-action@[0-9a-f]{40} # v2/);
  assert.doesNotMatch(workflow, /uses: [^\n]+@(v\d+|stable)\s*$/m);
  assert.match(workflow, /toolchain: 1\.97\.1/);
  assert.match(workflow, /generate-supply-chain-artifacts\.mjs --check/);
  assert.match(workflow, /cargo fetch --locked/);
  const preflightRustSetup = workflow.indexOf(
    "uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c",
  );
  const preflightRustVersion = workflow.indexOf("toolchain: 1.97.1", preflightRustSetup);
  const cargoFetch = workflow.indexOf("cargo fetch --locked");
  const fullTest = workflow.indexOf("npm run test:full");
  assert.ok(
    preflightRustSetup >= 0 &&
      preflightRustVersion > preflightRustSetup &&
      preflightRustVersion < cargoFetch &&
      cargoFetch < fullTest,
    "the clean release runner must pin Rust and hydrate Cargo before offline supply-chain tests",
  );
  assert.match(workflow, /actions\/setup-java@[0-9a-f]{40} # v5/);
  assert.match(workflow, /android-actions\/setup-android@[0-9a-f]{40} # v3/);
  assert.match(workflow, /node scripts\/generate-tauri-android-gradle-bridge\.mjs/);
  assert.match(
    androidBridgeGenerator,
    /\["metadata", "--locked", "--offline", "--filter-platform", "aarch64-linux-android"/,
  );
  assert.match(androidBridgeGenerator, /"--manifest-path", "src-tauri\/Cargo\.toml"/);
  assert.match(androidBridgeGenerator, /tauriPackage\.version !== "2\.11\.5"/);
  assert.match(androidBridgeGenerator, /tauriBuildPackage\.version !== "2\.6\.3"/);
  assert.match(androidBridgeGenerator, /Android Tauri plugin/);
  assert.match(androidBridgeGenerator, /include ':tauri-android'/);
  assert.match(androidBridgeGenerator, /implementation\(project\(":tauri-android"\)\)/);
  assert.match(androidIgnore, /^\/tauri\.settings\.gradle$/m);
  assert.match(androidAppIgnore, /^\/tauri\.build\.gradle\.kts$/m);
  assert.match(workflow, /\.\/gradlew --no-daemon :app:resolveLockedDependencies buildEnvironment/);
  assert.match(workflow, /\.\/gradlew --no-daemon --offline :app:resolveLockedDependencies buildEnvironment/);
  assert.doesNotMatch(workflow, /:app:dependencies buildEnvironment/);
  assert.match(androidAppBuild, /tasks\.register\("resolveLockedDependencies"\)/);
  assert.match(androidAppBuild, /file\("gradle\.lockfile"\)/);
  assert.match(androidAppBuild, /arm64ReleaseCompileClasspath/);
  assert.match(androidAppBuild, /arm64ReleaseRuntimeClasspath/);
  assert.match(
    androidAppBuild,
    /reviewedMetadataConfiguration = "implementationDependenciesMetadata"/,
  );
  assert.match(androidAppBuild, /projectPaths == setOf\(":tauri-android"\)/);
  assert.match(androidAppBuild, /it in releaseCompileLock && it in releaseRuntimeLock/);
  assert.match(androidAppBuild, /variant-ambiguous between debug and release/);
  assert.match(androidAppBuild, /com\.fasterxml\.jackson\.core:jackson-databind:2\.22\.1/);
  assert.match(androidAppBuild, /project :tauri-android/);
  assert.match(workflow, /complete-gradle-pom-verification\.mjs --check/);
  assert.match(workflow, /generate-gradle-license-review\.mjs/);
  assert.match(workflow, /--output "\$RUNNER_TEMP\/gradle-license-review\.json"/);
  assert.match(workflow, /chmod \+x src-tauri\/gen\/android\/gradlew/);
  const androidSetup = workflow.indexOf("android-actions/setup-android@");
  const androidBridgeGeneration = workflow.indexOf(
    "node scripts/generate-tauri-android-gradle-bridge.mjs",
  );
  const onlineGradleResolution = workflow.indexOf(
    "./gradlew --no-daemon :app:resolveLockedDependencies buildEnvironment",
  );
  const offlineGradleResolution = workflow.indexOf(
    "./gradlew --no-daemon --offline :app:resolveLockedDependencies buildEnvironment",
  );
  const pomEvidenceCheck = workflow.indexOf("complete-gradle-pom-verification.mjs --check");
  const supplyCheck = workflow.indexOf("generate-supply-chain-artifacts.mjs --check");
  assert.ok(androidSetup < onlineGradleResolution, "the clean runner must configure Android before resolving Gradle");
  assert.ok(
    androidSetup < androidBridgeGeneration && androidBridgeGeneration < onlineGradleResolution,
    "the clean runner must generate and validate Tauri's ignored Android Gradle bridge before Gradle starts",
  );
  assert.ok(onlineGradleResolution < offlineGradleResolution, "the locked graph must be hydrated before the offline proof");
  assert.ok(offlineGradleResolution < pomEvidenceCheck, "offline Gradle resolution must precede POM evidence checks");
  assert.ok(pomEvidenceCheck < supplyCheck, "verified Maven POM evidence must precede the combined SBOM");
  assert.ok(
    workflow.indexOf("check-android-gradle-supply-chain.mjs --check") <
      workflow.indexOf("generate-supply-chain-artifacts.mjs --check"),
    "the verified Gradle inventory must be emitted before the combined SPDX/license bundle",
  );
  assert.match(workflow, /pixnya-\$\{PIXNYA_RELEASE_VERSION\}\.spdx\.json/);
  assert.match(workflow, /check-android-gradle-supply-chain\.mjs --check/);
  assert.match(workflow, /android-gradle-dependencies\.json/);
  assert.match(workflow, /generate-android-runtime-sbom\.mjs/);
  assert.match(workflow, /android-runtime\.spdx\.json/);
  assert.match(workflow, /check-android-build-tool-osv-baseline\.mjs/);
  assert.match(workflow, /android-build-tools-osv\.json/);
  assert.match(workflow, /pixnya-\$\{PIXNYA_RELEASE_VERSION\}-third-party-licenses\.tar\.gz/);
  assert.match(workflow, /--licenses-dir/);
  assert.match(workflow, /pixnya-\$\{PIXNYA_RELEASE_VERSION\}-source\.tar\.gz/);
  assert.match(workflow, /OSV_SCANNER="\$RUNNER_TEMP\/osv-scanner"/);
  assert.match(
    workflow,
    /edcfc41d257db36148f065055655fe3fcfc434b0b423ea67468a84c207524e0c/,
  );
  assert.match(
    workflow,
    /"\$OSV_SCANNER" scan --sbom="release-artifacts\/pixnya-\$\{PIXNYA_RELEASE_VERSION\}-android-runtime\.spdx\.json"/,
  );
  const runtimeOsvScan = workflow.indexOf(
    '"$OSV_SCANNER" scan --sbom="release-artifacts/pixnya-${PIXNYA_RELEASE_VERSION}-android-runtime.spdx.json"',
  );
  const strictOsvFailures = workflow.lastIndexOf("set -e", runtimeOsvScan);
  assert.ok(
    strictOsvFailures >= 0 &&
      strictOsvFailures < runtimeOsvScan &&
      runtimeOsvScan < workflow.indexOf("set +e", runtimeOsvScan),
    "the exception-free runtime scan must fail preflight on every nonzero scanner status",
  );
  assert.doesNotMatch(workflow, /android-runtime[^\n]*--ignore/i);
  assert.doesNotMatch(workflow, /android-gradle-advisories:/);
  assert.doesNotMatch(workflow, /osv-scanner-reusable\.yml/);
  assert.doesNotMatch(workflow, /google\/osv-scanner-action/);
  assert.equal(
    workflow.match(/needs: \[preflight, rust-advisories\]/g)?.length,
    3,
    "all platform builds must wait for the in-preflight Android runtime scan and Rust advisory gate",
  );
  assert.match(workflow, /Signer #1 certificate SHA-256 digest/);
  assert.match(workflow, /check-android-arm64-apk\.ps1/);
  assert.match(workflow, /package: name='io\.github\.space2233\.pixnya'/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm dist\/android-latest\.json/);
  assert.match(workflow, /MANIFEST_NOTES_FILE="\$RUNNER_TEMP\/update-manifest-notes\.md"/);
  assert.match(workflow, /See https:\/\/github\.com\/%s\/releases\/tag\/v%s/);
  assert.equal(
    workflow.match(/--notes-file "\$MANIFEST_NOTES_FILE"/g)?.length,
    2,
    "desktop and Android update manifests must use notes that cannot retain Draft PENDING text",
  );
  assert.match(workflow, /Atomically bind the release tag to the source commit/);
  assert.match(workflow, /--request POST/);
  assert.match(workflow, /git\/refs/);
  assert.match(workflow, /refs\/tags\/v\$\{PIXNYA_RELEASE_VERSION\}/);
  assert.match(workflow, /createdTag\.object\?\.sha/);
  assert.match(workflow, /existingRelease\.draft !== true/);
  assert.match(workflow, /A published release cannot be reused/);
  assert.doesNotMatch(workflow, /target_commitish:/);
  assert.match(workflow, /BUILD-PROVENANCE\.txt/);
  assert.match(workflow, /PIXNYA_UPDATE_REPOSITORY: \$\{\{ github\.repository \}\}/);
  assert.doesNotMatch(workflow, /release_repository|RELEASE_REPOSITORY_TOKEN/);
  assert.match(workflow, /stable releases require the official repository to be anonymously readable/);
  assert.match(workflow, /metadata\.visibility !== "public"/);
  assert.match(workflow, /exactly one %s/);
  assert.match(workflow, /Expected exactly 20 whitelisted release files/);
  assert.match(workflow, /prerelease: \$\{\{ startsWith\(inputs\.version, '0\.'\) \}\}/);
  assert.match(workflow, /make_latest: \$\{\{ startsWith\(inputs\.version, '0\.'\) && 'false' \|\| 'true' \}\}/);
  assert.match(workflow, /overwrite_files: true/);
  assert.match(workflow, /environment:\s+name: production-release/);
  assert.equal(
    workflow.match(/uses: actions\/checkout@[0-9a-f]{40}/g)?.length,
    workflow.match(/persist-credentials: false/g)?.length,
    "every release checkout must drop its Git credential",
  );
  assert.match(workflow, /repository: \$\{\{ github\.repository \}\}/);
  assert.match(workflow, /--repository "\$PIXNYA_UPDATE_REPOSITORY"/);

  const assetsVerified = workflow.indexOf("Expected exactly 20 whitelisted release files");
  const tagReserved = workflow.indexOf("Atomically bind the release tag to the source commit");
  const releaseCreated = workflow.indexOf("Create or resume a draft Release for manual verification");
  const bindingVerified = workflow.indexOf("Verify the draft Release remains bound to the source commit");
  assert.ok(assetsVerified < tagReserved, "all release assets must be verified before reserving the tag");
  assert.ok(tagReserved < releaseCreated, "the exact tag must be reserved before creating the draft release");
  assert.ok(releaseCreated < bindingVerified, "the tag and draft release binding must be checked after upload");
});

test("stable release notes disclose every public distribution boundary", async () => {
  const [workflow, template, validator] = await Promise.all([
    read(".github/workflows/release.yml"),
    read("docs/RELEASE_NOTES_TEMPLATE.md"),
    read("scripts/validate-release-notes.mjs"),
  ]);
  const requiredSections = [
    "## Unofficial status and platforms",
    "## API and OAuth boundary",
    "## Low-security connections",
    "## Source, licenses, SBOM, and checksums",
    "## Upgrade verification and limitations",
  ];

  for (const section of requiredSections) {
    assert.match(template, new RegExp(section.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  assert.match(template, /^Failure-path coverage:/m);
  assert.match(template, /^Known limitations:/m);
  assert.match(workflow, /node scripts\/validate-release-notes\.mjs/);
  assert.match(workflow, /--commit "\$GITHUB_SHA"/);
  assert.match(workflow, /10#\$\{SOURCE_VERSION%%\.\*\} >= 1/);
  assert.match(validator, /PUBLIC_RELEASE_REQUIRED_SECTIONS/);
  assert.match(validator, /unfinished template placeholder/);

  const version = "1.0.0";
  const commitSha = "0123456789abcdef0123456789abcdef01234567";
  const completeNotes = `${requiredSections[0]}
PixNya is unofficial. Windows x64, Linux x64, Android ARM64.

${requiredSections[1]}
The non-public App API may change; OAuth build parameters are extractable.

${requiredSections[2]}
Compatibility mode is off by default and carries man-in-the-middle risk.

${requiredSections[3]}
Source commit: ${commitSha}
GPL-3.0-only
LICENSE.txt
pixnya-${version}-source.tar.gz
pixnya-${version}-third-party-licenses.tar.gz
pixnya-${version}.spdx.json
pixnya-${version}-android-runtime.spdx.json
SHA256SUMS.txt

${requiredSections[4]}
- Windows x64: 0.29.0 -> ${version}; Windows 11 24H2; PASS
- Linux x64: 0.29.0 -> ${version}; Ubuntu 24.04; PASS
- Android ARM64: 0.29.0 -> ${version}; Android 15 test device; PASS
Failure-path coverage: wrong signature, corrupted manifest, interrupted download, low space, cancelled install, retry
Known limitations: Windows binaries are not Authenticode-signed.`;

  assert.doesNotThrow(() => validateStableReleaseNotes({ notes: completeNotes, version, commitSha }));
  const pendingNotes = completeNotes
    .replace(/; Windows 11 24H2; PASS/, "; PENDING after Draft artifacts")
    .replace(/; Ubuntu 24\.04; PASS/, "; PENDING after Draft artifacts")
    .replace(/; Android 15 test device; PASS/, "; PENDING after Draft artifacts")
    .replace(/Failure-path coverage:.*/, "Failure-path coverage: PENDING after Draft artifacts")
    .replace(/Known limitations:.*/, "Known limitations: PENDING after Draft artifacts");
  assert.throws(() => validateStableReleaseNotes({ notes: pendingNotes, version, commitSha }));
  assert.doesNotThrow(() => validateStableReleaseNotes({
    notes: pendingNotes,
    version,
    commitSha,
    allowPendingUpgrades: true,
  }));
  for (const invalidNotes of [
    completeNotes.replace(commitSha, "{{full commit SHA}}"),
    completeNotes.replace(`pixnya-${version}.spdx.json`, "combined SBOM"),
    completeNotes.replace("- Android ARM64: 0.29.0 -> 1.0.0; Android 15 test device; PASS", ""),
    completeNotes.replace("PASS", "{{result}}"),
    completeNotes.replace("0.29.0 -> 1.0.0; Windows 11 24H2", "1.0.0 -> 1.0.0; Windows 11 24H2"),
    completeNotes.replace("; Ubuntu 24.04; PASS", "; PASS"),
    completeNotes.replace(/Failure-path coverage:.*\n/, ""),
    completeNotes.replace("Known limitations: Windows binaries are not Authenticode-signed.", "Known limitations:"),
    completeNotes.replace("Compatibility mode is off by default and carries man-in-the-middle risk.", "Compatibility mode is off by default and carries man-in-the-middle risk. PENDING"),
    completeNotes.replace("OAuth build parameters are extractable.", "OAuth is used."),
    completeNotes.replace("GPL-3.0-only\n", ""),
  ]) {
    assert.throws(() => validateStableReleaseNotes({ notes: invalidNotes, version, commitSha }));
  }
});

test("stable publication revalidates the signed Draft instead of trusting the build run", async () => {
  const workflow = await read(".github/workflows/publish-release.yml");

  assert.match(workflow, /name: Publish verified stable release/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /Require the main stable source/);
  assert.match(workflow, /refs\/heads\/main/);
  assert.match(workflow, /environment:\s+name: production-release/);
  assert.match(workflow, /permissions:\s+contents: write/);
  assert.match(workflow, /actions\/checkout@[0-9a-f]{40}/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /metadata\.visibility !== "public"/);
  assert.match(workflow, /release\.draft !== true/);
  assert.match(workflow, /tag\.object\?\.sha !== process\.env\.GITHUB_SHA/);
  assert.match(workflow, /validate-release-candidate\.mjs/);
  assert.doesNotMatch(workflow, /allow-pending-upgrades/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm "\$LINUX_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm candidate\/android-latest\.json/);
  assert.match(workflow, /sdkmanager "build-tools;36\.0\.0"/);
  assert.match(workflow, /"\$APKSIGNER" verify --verbose --print-certs/);
  assert.match(workflow, /Signer #1 certificate SHA-256 digest/);
  assert.match(workflow, /APK certificate does not match the signed Android update manifest/);
  assert.match(workflow, /The Draft changed while it was being verified/);
  assert.match(workflow, /--method PATCH/);
  assert.match(workflow, /-F draft=false/);
  assert.match(workflow, /-F prerelease=false/);
  assert.match(workflow, /-f make_latest=true/);
  assert.match(workflow, /The stable Release was not published as expected/);

  const strictValidation = workflow.indexOf("validate-release-candidate.mjs");
  const signatureValidation = workflow.indexOf('minisign -Vm "$WINDOWS_ARCHIVE"');
  const immutabilityValidation = workflow.indexOf("The Draft changed while it was being verified");
  const publication = workflow.indexOf("--method PATCH");
  assert.ok(strictValidation < signatureValidation, "candidate metadata and checksums must pass before signatures");
  assert.ok(signatureValidation < immutabilityValidation, "signatures must pass before the final metadata recheck");
  assert.ok(immutabilityValidation < publication, "the Draft must remain unchanged until publication");
});

test("throwaway thumbnail prototypes stay outside the formal application bundle", async () => {
  const packageJson = JSON.parse(await read("package.json"));
  assert.equal(packageJson.scripts["prototype:thumbnails"], undefined);
  for (const relativePath of [
    "src/routes/prototype/vite-smoke/+page.svelte",
    "src/routes/prototype/thumbnail-placeholders/+page.svelte",
    "public/prototype/thumbnail-placeholders.html",
  ]) {
    await assert.rejects(read(relativePath), (error) => error?.code === "ENOENT");
  }
});

test("unsupported notification and posting surfaces stay explicit and non-interactive", async () => {
  const [notifications, shell] = await Promise.all([
    read("src/routes/notifications/+page.svelte"),
    read("src/lib/components/AppShell.svelte"),
  ]);
  assert.match(notifications, /m\.notifications_unsupported_title\(\)/);
  assert.match(notifications, /m\.notifications_boundary_description\(\)/);
  assert.match(shell, /class="text-action" type="button" disabled/);
  assert.match(shell, /m\.shell_post_unavailable\(\)/);
});
