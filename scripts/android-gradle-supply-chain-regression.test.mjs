import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

const execFileAsync = promisify(execFile);
const root = process.cwd();
const checker = path.join(root, "scripts", "check-android-gradle-supply-chain.mjs");
const androidFixtureFiles = [
  "app/build.gradle.kts",
  "app/gradle.lockfile",
  "build.gradle.kts",
  "buildscript-gradle.lockfile",
  "buildSrc/build.gradle.kts",
  "buildSrc/gradle.lockfile",
  "gradle/verification-metadata.xml",
  "gradle/wrapper/gradle-wrapper.jar",
  "gradle/wrapper/gradle-wrapper.properties",
];
const fingerprintTextFiles = [
  "app/gradle.lockfile",
  "buildscript-gradle.lockfile",
  "buildSrc/gradle.lockfile",
  "gradle/verification-metadata.xml",
  "gradle/wrapper/gradle-wrapper.properties",
];
const sharedGradleConfigFiles = [
  "app/build.gradle.kts",
  "build.gradle.kts",
  "buildSrc/build.gradle.kts",
  "gradle.properties",
  "gradle/wrapper/gradle-wrapper.properties",
  "settings.gradle",
];

async function createFixture(context) {
  const fixtureRoot = await mkdtemp(path.join(tmpdir(), "pixnya-android-gradle-"));
  context.after(() => rm(fixtureRoot, { force: true, recursive: true }));

  const fixtureAndroidRoot = path.join(fixtureRoot, "src-tauri", "gen", "android");
  const sourceAndroidRoot = path.join(root, "src-tauri", "gen", "android");
  for (const relativePath of androidFixtureFiles) {
    const destination = path.join(fixtureAndroidRoot, relativePath);
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(path.join(sourceAndroidRoot, relativePath), destination);
  }
  return fixtureRoot;
}

test("the checked-in Android Gradle graph is locked and checksum verified offline", async () => {
  const { stdout } = await execFileAsync(process.execPath, [checker, "--check"], {
    cwd: root,
    windowsHide: true,
  });

  assert.match(stdout, /Android Gradle supply chain verified offline:/);
  assert.match(stdout, /locked components/);
});

test("verification metadata pins the JUnit BOM module required by clean buildSrc resolution", async () => {
  const metadata = await readFile(
    path.join(root, "src-tauri", "gen", "android", "gradle", "verification-metadata.xml"),
    "utf8",
  );
  const junitBom = metadata.match(
    /<component group="org\.junit" name="junit-bom" version="5\.10\.2">([\s\S]*?)<\/component>/,
  )?.[1];

  assert.ok(junitBom, "verification metadata must include org.junit:junit-bom:5.10.2");
  assert.match(
    junitBom,
    /<artifact name="junit-bom-5\.10\.2\.module">\s*<sha256 value="de23b114b3e4119a8fe6eb17bed5a3852816698bace67071579d6d927ebb080a"/,
    "the clean-runner Gradle module artifact must match Maven Central's published SHA-256",
  );
});

test("verification metadata pins the Jackson parent POM required by the Tauri Android build", async () => {
  const metadata = await readFile(
    path.join(root, "src-tauri", "gen", "android", "gradle", "verification-metadata.xml"),
    "utf8",
  );
  const jacksonBase = metadata.match(
    /<component group="com\.fasterxml\.jackson" name="jackson-base" version="2\.15\.3">([\s\S]*?)<\/component>/,
  )?.[1];

  assert.ok(jacksonBase, "verification metadata must include com.fasterxml.jackson:jackson-base:2.15.3");
  assert.match(
    jacksonBase,
    /<artifact name="jackson-base-2\.15\.3\.pom">\s*<sha256 value="4290342abf0b0e4567322ffb2d0c36e25b0a87a217bb56b35680a8dd8f8d66e4"/,
    "the Tauri Android parent POM must match the independently recomputed Maven Central SHA-256",
  );
});

test("checked-in Android Gradle configuration is machine neutral", async () => {
  const androidRoot = path.join(root, "src-tauri", "gen", "android");
  const androidIgnore = await readFile(path.join(androidRoot, ".gitignore"), "utf8");
  assert.match(
    androidIgnore,
    /^\/local\.properties$/m,
    "developer-specific sdk.dir configuration must remain local and untracked",
  );

  const violations = [];
  for (const relativePath of sharedGradleConfigFiles) {
    const content = await readFile(path.join(androidRoot, relativePath), "utf8");
    for (const [index, line] of content.split(/\r?\n/).entries()) {
      const trimmed = line.trimStart();
      if (trimmed.startsWith("#") || trimmed.startsWith("//")) {
        continue;
      }
      if (
        /(?:^|[=\s"'(,])(?:[A-Za-z]\\?:[\\/]+|\/(?![/*])|~[\\/])/.test(line) ||
        /(?:^|[=\s"'(,])\\\\[A-Za-z0-9_.-]+[\\/]/.test(line)
      ) {
        violations.push(`${relativePath}:${index + 1}: ${line.trim()}`);
      }
    }
  }

  assert.deepEqual(
    violations,
    [],
    `shared Gradle configuration must use JAVA_HOME, SDK environment variables, or ignored local.properties:\n${violations.join("\n")}`,
  );
});

test("the offline inventory is deterministic and includes direct and transitive dependencies", async (context) => {
  const fixtureRoot = await createFixture(context);
  const firstOutput = path.join(fixtureRoot, "inventory-one.json");
  const secondOutput = path.join(fixtureRoot, "inventory-two.json");

  await execFileAsync(process.execPath, [checker, "--project-root", fixtureRoot, "--output", firstOutput], {
    cwd: root,
    windowsHide: true,
  });
  await execFileAsync(process.execPath, [checker, "--project-root", fixtureRoot, "--output", secondOutput], {
    cwd: root,
    windowsHide: true,
  });

  const firstInventory = await readFile(firstOutput, "utf8");
  assert.equal(firstInventory, await readFile(secondOutput, "utf8"));
  const coordinates = JSON.parse(firstInventory).components.map(({ coordinate }) => coordinate);
  assert.ok(coordinates.includes("androidx.webkit:webkit:1.14.0"), "direct app dependency must be inventoried");
  assert.ok(
    coordinates.includes("com.fasterxml.jackson.core:jackson-databind:2.22.1"),
    "transitive Gradle dependency must be inventoried",
  );
});

test("the offline inventory fingerprint is identical for LF and CRLF Gradle text", async (context) => {
  const lfFixtureRoot = await createFixture(context);
  const crlfFixtureRoot = await createFixture(context);
  const lfAndroidRoot = path.join(lfFixtureRoot, "src-tauri", "gen", "android");
  const crlfAndroidRoot = path.join(crlfFixtureRoot, "src-tauri", "gen", "android");

  for (const relativePath of fingerprintTextFiles) {
    const source = await readFile(path.join(lfAndroidRoot, relativePath), "utf8");
    const lfSource = source.replace(/\r\n?/g, "\n");
    await writeFile(path.join(lfAndroidRoot, relativePath), lfSource, "utf8");
    await writeFile(path.join(crlfAndroidRoot, relativePath), lfSource.replaceAll("\n", "\r\n"), "utf8");
  }

  const lfOutput = path.join(lfFixtureRoot, "inventory-lf.json");
  const crlfOutput = path.join(crlfFixtureRoot, "inventory-crlf.json");
  await execFileAsync(process.execPath, [checker, "--project-root", lfFixtureRoot, "--output", lfOutput], {
    cwd: root,
    windowsHide: true,
  });
  await execFileAsync(process.execPath, [checker, "--project-root", crlfFixtureRoot, "--output", crlfOutput], {
    cwd: root,
    windowsHide: true,
  });

  const lfInventory = JSON.parse(await readFile(lfOutput, "utf8"));
  const crlfInventory = JSON.parse(await readFile(crlfOutput, "utf8"));
  assert.equal(crlfInventory.fingerprint, lfInventory.fingerprint);
});

test("the offline check fails closed when a locked component loses checksum evidence", async (context) => {
  const fixtureRoot = await createFixture(context);
  const metadataPath = path.join(
    fixtureRoot,
    "src-tauri",
    "gen",
    "android",
    "gradle",
    "verification-metadata.xml",
  );
  const metadata = await readFile(metadataPath, "utf8");
  const tampered = metadata.replace(
    /\s*<component group="androidx\.webkit" name="webkit" version="1\.14\.0">[\s\S]*?<\/component>/,
    "",
  );
  assert.notEqual(tampered, metadata, "fixture must contain the locked WebKit component");
  await writeFile(metadataPath, tampered, "utf8");

  await assert.rejects(
    execFileAsync(process.execPath, [checker, "--project-root", fixtureRoot, "--check"], {
      cwd: root,
      windowsHide: true,
    }),
    (error) => {
      assert.match(error.stderr, /Locked Gradle component has no checksum verification metadata: androidx\.webkit:webkit:1\.14\.0/);
      return true;
    },
  );
});
