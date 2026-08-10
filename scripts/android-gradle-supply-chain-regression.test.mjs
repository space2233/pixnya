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
