import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { planTauriAndroidGradleBridge } from "./generate-tauri-android-gradle-bridge.mjs";

const appManifest = path.resolve("src-tauri", "Cargo.toml");
const tauriManifest = path.resolve("fixtures", "tauri-2.11.5", "Cargo.toml");
const tauriBuildManifest = path.resolve("fixtures", "tauri-build-2.6.3", "Cargo.toml");

const metadataFixture = () => ({
  packages: [
    { id: "pixnya", name: "pixnya", manifest_path: appManifest },
    {
      id: "tauri",
      name: "tauri",
      version: "2.11.5",
      links: "Tauri",
      manifest_path: tauriManifest,
    },
    {
      id: "tauri-build",
      name: "tauri-build",
      version: "2.6.3",
      manifest_path: tauriBuildManifest,
    },
  ],
  resolve: {
    root: "pixnya",
    nodes: [
      {
        id: "pixnya",
        deps: [
          { name: "tauri", pkg: "tauri", dep_kinds: [{ kind: null, target: null }] },
          {
            name: "tauri_build",
            pkg: "tauri-build",
            dep_kinds: [{ kind: "build", target: null }],
          },
        ],
      },
    ],
  },
});

const fakeFiles = {
  isDirectory: () => true,
  isFile: () => true,
  readText: () =>
    'generate_gradle_files implementation(\\"androidx.lifecycle:lifecycle-process:2.10.0\\")',
};

test("plans the ignored Gradle bridge from the locked Android Tauri graph", () => {
  const plan = planTauriAndroidGradleBridge(metadataFixture(), {
    appManifest,
    ...fakeFiles,
  });

  assert.match(plan.settings, /include ':tauri-android'/);
  assert.match(plan.settings, /project\(':tauri-android'\)\.projectDir = new File/);
  assert.match(plan.appBuild, /implementation\(project\(":tauri-android"\)\)/);
  assert.match(plan.appBuild, /lifecycle-process:2\.10\.0/);
});

test("fails closed when the Android app gains a direct Tauri plugin", () => {
  const metadata = metadataFixture();
  metadata.packages.push({
    id: "tauri-plugin-example",
    name: "tauri-plugin-example",
    version: "2.0.0",
    links: "tauri-plugin-example",
    manifest_path: path.resolve("fixtures", "tauri-plugin-example", "Cargo.toml"),
  });
  metadata.resolve.nodes[0].deps.push({
    name: "tauri_plugin_example",
    pkg: "tauri-plugin-example",
    dep_kinds: [{ kind: null, target: null }],
  });

  assert.throws(
    () =>
      planTauriAndroidGradleBridge(metadata, {
        appManifest,
        ...fakeFiles,
      }),
    /Android Tauri plugin/,
  );
});

test("fails closed when the pinned generator contract changes", () => {
  for (const mutate of [
    (metadata) => {
      metadata.packages.find((candidate) => candidate.id === "tauri").links = "Changed";
    },
    (metadata) => {
      metadata.packages.find((candidate) => candidate.id === "tauri-build").version = "2.7.0";
    },
  ]) {
    const metadata = metadataFixture();
    mutate(metadata);
    assert.throws(() =>
      planTauriAndroidGradleBridge(metadata, {
        appManifest,
        ...fakeFiles,
      }),
    );
  }
});
