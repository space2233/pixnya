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

test("all user-visible package versions agree on the stable source version", async () => {
  const expectedVersion = "1.4.1";
  const [major, minor, patch] = expectedVersion.split(".").map(Number);
  const expectedAndroidVersionCode = major * 1_000_000 + minor * 1_000 + patch;
  const [workspace, packageJson, packageLock, tauri, androidProperties, androidIgnore, readme] = await Promise.all([
    read("Cargo.toml"),
    read("package.json"),
    read("package-lock.json"),
    read("src-tauri/tauri.conf.json"),
    readGenerated("src-tauri/gen/android/app/tauri.properties"),
    read("src-tauri/gen/android/app/.gitignore"),
    read("README.md"),
  ]);
  assert.ok(workspace.includes(`version = "${expectedVersion}"`));
  assert.equal(JSON.parse(packageJson).version, expectedVersion);
  assert.equal(JSON.parse(packageLock).version, expectedVersion);
  assert.equal(JSON.parse(tauri).version, expectedVersion);
  assert.match(androidIgnore, /^\/tauri\.properties$/m);
  if (androidProperties !== null) {
    assert.ok(androidProperties.includes(`tauri.android.versionName=${expectedVersion}`));
    assert.ok(androidProperties.includes(`tauri.android.versionCode=${expectedAndroidVersionCode}`));
  }
  assert.ok(
    readme.includes("当前源码版本 `" + expectedVersion + "`") ||
      readme.includes("当前源码版本为 `" + expectedVersion + "`"),
  );
});

test("Android releases signed ARM64 and ARMv7 split APKs", async () => {
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
  assert.match(workflow, /rustTarget: aarch64-linux-android/);
  assert.match(workflow, /abi: arm64-v8a/);
  assert.match(workflow, /tauriTarget: armv7/);
  assert.match(workflow, /rustTarget: armv7-linux-androideabi/);
  assert.match(workflow, /abi: armeabi-v7a/);
  assert.match(workflow, /-ExpectedAbi \$\{\{ matrix\.abi \}\}/);
  assert.match(workflow, /--armv7 "dist\/pixnya-\$\{PIXNYA_RELEASE_VERSION\}-android-armeabi-v7a\.apk"/);
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
  const [workflow, androidBridgeGenerator, pomHydrator, androidIgnore, androidAppIgnore, androidAppBuild] = await Promise.all([
    read(".github/workflows/release.yml"),
    read("scripts/generate-tauri-android-gradle-bridge.mjs"),
    read("scripts/hydrate-gradle-pom-evidence.mjs"),
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
  assert.match(workflow, /arguments: --all-features --locked/);
  assert.doesNotMatch(workflow, /uses: [^\n]+@(v\d+|stable)\s*$/m);
  assert.match(workflow, /toolchain: 1\.97\.1/);
  assert.match(workflow, /generate-supply-chain-artifacts\.mjs --check/);
  assert.match(
    workflow,
    /jedisct1\/minisign\/releases\/download\/0\.12\/minisign-0\.12-linux\.tar\.gz/,
  );
  assert.match(workflow, /9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73/);
  assert.match(workflow, /sha256sum --check --strict/);
  assert.match(
    workflow,
    /cd dist[\s\S]*find \. -maxdepth 1 -type f ! -name SHA256SUMS\.txt -printf '%f\\0'[\s\S]*xargs -0 sha256sum > SHA256SUMS\.txt/,
  );
  assert.doesNotMatch(
    workflow,
    /find dist[^\n]*sha256sum > dist\/SHA256SUMS\.txt/,
    "release checksums must contain attachment basenames instead of dist/ paths",
  );
  assert.doesNotMatch(workflow, /apt-get install[^\n]*minisign/);
  assert.match(workflow, /artifact_run_id:/);
  assert.match(workflow, /validate-release-artifact-run\.mjs/);
  assert.match(workflow, /artifact-ids: \$\{\{ steps\.validate-artifact-run\.outputs\.artifact_ids \}\}/);
  assert.match(workflow, /run-id: \$\{\{ inputs\.artifact_run_id \}\}/);
  assert.match(workflow, /PIXNYA_RELEASE_SOURCE_SHA/);
  assert.match(workflow, /source_commit=\$\{PIXNYA_RELEASE_SOURCE_SHA\}/);
  assert.match(workflow, /keytool -J-Duser\.language=en -exportcert/);
  assert.match(workflow, /"\$APKSIGNER" verify --verbose --print-certs-pem "\$APK"/);
  assert.match(workflow, /base64 --decode > "\$APK_CERTIFICATE"/);
  assert.match(workflow, /cmp --silent "\$APK_CERTIFICATE" "\$EXPECTED_CERTIFICATE"/);
  assert.doesNotMatch(workflow, /APK_CERTIFICATE_SHA256/);
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
  assert.match(
    workflow,
    /\.\/gradlew --no-daemon :app:resolveLockedDependencies :tauri-android:extractReleaseAnnotations buildEnvironment/,
  );
  assert.match(
    workflow,
    /\.\/gradlew --no-daemon --offline :app:resolveLockedDependencies :tauri-android:extractReleaseAnnotations buildEnvironment/,
  );
  assert.doesNotMatch(workflow, /:app:dependencies buildEnvironment/);
  assert.match(androidAppBuild, /tasks\.register\("resolveLockedDependencies"\)/);
  assert.match(androidAppBuild, /file\("gradle\.lockfile"\)/);
  assert.match(androidAppBuild, /arm64ReleaseCompileClasspath/);
  assert.match(androidAppBuild, /arm64ReleaseRuntimeClasspath/);
  assert.match(androidAppBuild, /armReleaseCompileClasspath/);
  assert.match(androidAppBuild, /armReleaseRuntimeClasspath/);
  assert.match(
    androidAppBuild,
    /reviewedMetadataConfiguration = "implementationDependenciesMetadata"/,
  );
  assert.match(androidAppBuild, /projectPaths == setOf\(":tauri-android"\)/);
  assert.match(androidAppBuild, /releaseCompileLocks\.all \{ coordinate in it \}/);
  assert.match(androidAppBuild, /releaseRuntimeLocks\.all \{ coordinate in it \}/);
  assert.match(androidAppBuild, /Android ARM64 and ARMv7 release compile lock graphs differ/);
  assert.match(androidAppBuild, /Android ARM64 and ARMv7 release runtime lock graphs differ/);
  assert.match(androidAppBuild, /variant-ambiguous between debug and release/);
  assert.match(androidAppBuild, /com\.fasterxml\.jackson\.core:jackson-databind:2\.22\.1/);
  assert.match(androidAppBuild, /project :tauri-android/);
  assert.match(workflow, /POM_EVIDENCE_HOME="\$RUNNER_TEMP\/pixnya-gradle-pom-evidence"/);
  assert.match(workflow, /hydrate-gradle-pom-evidence\.mjs/);
  assert.match(
    workflow,
    /complete-gradle-pom-verification\.mjs --check --gradle-user-home "\$POM_EVIDENCE_HOME"/,
  );
  assert.match(workflow, /generate-gradle-license-review\.mjs/);
  assert.match(workflow, /--gradle-user-home "\$POM_EVIDENCE_HOME"/);
  assert.match(workflow, /--output "\$RUNNER_TEMP\/gradle-license-review\.json"/);
  assert.match(pomHydrator, /reviewedMavenPomRepositories/);
  assert.match(pomHydrator, /discoverGradlePomVerificationEntries/);
  assert.match(pomHydrator, /hydration\.poms/);
  assert.match(workflow, /chmod \+x src-tauri\/gen\/android\/gradlew/);
  const androidSetup = workflow.indexOf("android-actions/setup-android@");
  const androidBridgeGeneration = workflow.indexOf(
    "node scripts/generate-tauri-android-gradle-bridge.mjs",
  );
  const onlineGradleResolution = workflow.indexOf(
    "./gradlew --no-daemon :app:resolveLockedDependencies :tauri-android:extractReleaseAnnotations buildEnvironment",
  );
  const offlineGradleResolution = workflow.indexOf(
    "./gradlew --no-daemon --offline :app:resolveLockedDependencies :tauri-android:extractReleaseAnnotations buildEnvironment",
  );
  const pomEvidenceHydration = workflow.indexOf("hydrate-gradle-pom-evidence.mjs");
  const pomEvidenceCheck = workflow.indexOf("complete-gradle-pom-verification.mjs --check");
  const supplyCheck = workflow.indexOf("generate-supply-chain-artifacts.mjs --check");
  assert.ok(androidSetup < onlineGradleResolution, "the clean runner must configure Android before resolving Gradle");
  assert.ok(
    androidSetup < androidBridgeGeneration && androidBridgeGeneration < onlineGradleResolution,
    "the clean runner must generate and validate Tauri's ignored Android Gradle bridge before Gradle starts",
  );
  assert.ok(
    onlineGradleResolution < pomEvidenceHydration && pomEvidenceHydration < offlineGradleResolution,
    "the online phase must hydrate every verified Maven POM before the offline proof",
  );
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
    4,
    "all platform builds must wait for the in-preflight Android runtime scan and Rust advisory gate",
  );
  assert.match(workflow, /check-android-apk\.ps1/);
  assert.match(workflow, /package: name='io\.github\.space2233\.pixnya'/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_X64_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_ARM64_ARCHIVE"/);
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
  assert.match(workflow, /Expected exactly 10 public release files/);
  assert.match(workflow, /pixnya-\$\{PIXNYA_RELEASE_VERSION\}-verification\.tar\.gz/);
  assert.match(workflow, /WINDOWS_X64_ARCHIVE="dist\/PixNya_\$\{PIXNYA_RELEASE_VERSION\}_x64-setup\.exe"/);
  assert.match(workflow, /WINDOWS_ARM64_ARCHIVE="dist\/PixNya_\$\{PIXNYA_RELEASE_VERSION\}_arm64-setup\.exe"/);
  assert.doesNotMatch(workflow, /nsis\.zip/);
  assert.doesNotMatch(workflow, /AppImage\.tar\.gz/);
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

  const assetsVerified = workflow.indexOf("Expected exactly 10 public release files");
  const tagReserved = workflow.indexOf("Atomically bind the release tag to the source commit");
  const releaseCreated = workflow.indexOf("Create or resume a draft Release for manual verification");
  const bindingVerified = workflow.indexOf("Verify the draft Release remains bound to the source commit");
  assert.ok(assetsVerified < tagReserved, "all release assets must be verified before reserving the tag");
  assert.ok(tagReserved < releaseCreated, "the exact tag must be reserved before creating the draft release");
  assert.ok(releaseCreated < bindingVerified, "the tag and draft release binding must be checked after upload");
  assert.match(workflow, /id:\s+draft_release/);
  assert.match(workflow, /RELEASE_ID:\s+\$\{\{\s*steps\.draft_release\.outputs\.id\s*\}\}/);
  assert.match(workflow, /releases\/\$\{RELEASE_ID\}/);
});

test("stable release notes stay concise and bilingual", async () => {
  const [workflow, template, validator] = await Promise.all([
    read(".github/workflows/release.yml"),
    read("docs/RELEASE_NOTES_TEMPLATE.md"),
    read("scripts/validate-release-notes.mjs"),
  ]);
  const requiredSections = ["## 中文", "## English"];

  for (const section of requiredSections) {
    assert.match(template, new RegExp(section.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  assert.doesNotMatch(template, /Low-security connections/);
  assert.doesNotMatch(template, /Source, licenses, SBOM, and checksums/);
  assert.doesNotMatch(template, /Upgrade verification and limitations/);
  assert.match(workflow, /node scripts\/validate-release-notes\.mjs/);
  assert.match(workflow, /--commit "\$GITHUB_SHA"/);
  assert.match(workflow, /10#\$\{SOURCE_VERSION%%\.\*\} >= 1/);
  assert.match(validator, /PUBLIC_RELEASE_REQUIRED_SECTIONS/);
  assert.match(validator, /unfinished template placeholder/);

  const version = "1.0.0";
  const commitSha = "0123456789abcdef0123456789abcdef01234567";
  const completeNotes = `# PixNya ${version}

${requiredSections[0]}

- 新增通知、评论管理和动图导出。
- 支持 Windows x64、Windows ARM64、Linux x64、Android ARM64 和 Android ARM32（Android 10+）。

${requiredSections[1]}

- Added notifications, comment management, and animation export.
- Supports Windows x64, Windows ARM64, Linux x64, Android ARM64, and Android ARM32 (Android 10+).`;

  assert.doesNotThrow(() => validateStableReleaseNotes({ notes: completeNotes, version, commitSha }));
  const pendingNotes = `${completeNotes}\nPENDING`;
  assert.throws(() => validateStableReleaseNotes({ notes: pendingNotes, version, commitSha }));
  const invalidNotes = [
    completeNotes.replace(`# PixNya ${version}`, "# PixNya 9.9.9"),
    completeNotes.replace("## 中文", "## Chinese"),
    completeNotes.replace(
      /## 中文[\s\S]*?## English/,
      "## 中文\n\n- Added features.\n- Supports Windows x64, Windows ARM64, Linux x64, Android ARM64, and Android ARM32.\n\n## English",
    ),
    completeNotes.replace("## English", "## 英文"),
    completeNotes.replace(/## English[\s\S]*$/, "## English\n\n- 新增功能。"),
    completeNotes.replaceAll("Android ARM64", "Android"),
    completeNotes.replaceAll("Windows ARM64", "Windows"),
    completeNotes.replaceAll("Android ARM32", "Android"),
    completeNotes.replace("notifications", "{{features}}"),
  ];
  for (const [index, notes] of invalidNotes.entries()) {
    assert.throws(
      () => validateStableReleaseNotes({ notes, version, commitSha }),
      undefined,
      `invalid concise release note fixture ${index} must be rejected`,
    );
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
  assert.doesNotMatch(workflow, /releases\/tags/);
  assert.match(workflow, /releases\?per_page=100/);
  assert.match(workflow, /PIXNYA_DRAFT_RELEASE_ID/);
  assert.match(workflow, /releases\/assets\/\$\{ASSET_ID\}/);
  assert.match(workflow, /release\.draft !== true/);
  assert.match(workflow, /CANDIDATE_SOURCE_SHA=.*source_commit=/);
  assert.match(workflow, /PIXNYA_RELEASE_SOURCE_SHA=/);
  assert.equal(
    workflow.match(/--workflow-commit "\$GITHUB_SHA"/g)?.length,
    2,
    "both candidate checks must trust the selected main finalizer commit",
  );
  assert.match(workflow, /--commit "\$CANDIDATE_SOURCE_SHA"/);
  assert.match(workflow, /--commit "\$PIXNYA_RELEASE_SOURCE_SHA"/);
  assert.match(workflow, /validate-release-candidate\.mjs/);
  assert.doesNotMatch(workflow, /allow-pending-upgrades/);
  assert.match(
    workflow,
    /jedisct1\/minisign\/releases\/download\/0\.12\/minisign-0\.12-linux\.tar\.gz/,
  );
  assert.match(workflow, /9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73/);
  assert.doesNotMatch(workflow, /apt-get install[^\n]*minisign/);
  assert.match(workflow, /WINDOWS_X64_ARCHIVE="candidate\/PixNya_\$\{PIXNYA_RELEASE_VERSION\}_x64-setup\.exe"/);
  assert.match(workflow, /WINDOWS_ARM64_ARCHIVE="candidate\/PixNya_\$\{PIXNYA_RELEASE_VERSION\}_arm64-setup\.exe"/);
  assert.match(workflow, /-name '\*\.AppImage'/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_X64_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm "\$WINDOWS_ARM64_ARCHIVE"/);
  assert.match(workflow, /minisign -Vm "\$LINUX_ARCHIVE"/);
  assert.match(workflow, /Missing embedded updater signature/);
  assert.match(workflow, /minisign -Vm candidate\/android-latest\.json/);
  assert.match(workflow, /sdkmanager "build-tools;36\.0\.0"/);
  assert.match(workflow, /"\$APKSIGNER" verify --verbose --print-certs/);
  assert.match(workflow, /APK certificate does not match the signed Android update manifest/);
  assert.match(workflow, /for ABI in arm64-v8a armeabi-v7a/);
  assert.match(workflow, /The Draft changed while it was being verified/);
  assert.match(workflow, /--method PATCH/);
  assert.match(workflow, /-F draft=false/);
  assert.match(workflow, /-F prerelease=false/);
  assert.match(workflow, /-f make_latest=true/);
  assert.match(workflow, /The stable Release was not published as expected/);

  const strictValidation = workflow.indexOf("validate-release-candidate.mjs");
  const signatureValidation = workflow.indexOf('minisign -Vm "$WINDOWS_X64_ARCHIVE"');
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

test("notifications remain read-only while unsupported posting stays non-interactive", async () => {
  const [notifications, shell] = await Promise.all([
    read("src/routes/notifications/+page.svelte"),
    read("src/lib/components/AppShell.svelte"),
  ]);
  assert.match(notifications, /getNotifications/);
  assert.doesNotMatch(notifications, /markNotification|notification.*(?:post|write)/i);
  assert.match(shell, /class="text-action" type="button" disabled/);
  assert.match(shell, /m\.shell_post_unavailable\(\)/);
});
