import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const sourceRoots = ["crates", "src", "src-tauri/src"];
const sourceExtensions = new Set([".rs", ".svelte", ".ts", ".js", ".mjs"]);

async function sourceFiles(relativeDirectory) {
  const directory = path.join(root, relativeDirectory);
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const relativePath = path.join(relativeDirectory, entry.name);
    if (entry.isDirectory()) files.push(...(await sourceFiles(relativePath)));
    else if (sourceExtensions.has(path.extname(entry.name))) files.push(relativePath);
  }
  return files;
}

test("compiled dependency manifests do not include PixEz or Flutter", async () => {
  const manifests = await Promise.all(
    ["Cargo.toml", "package.json", "package-lock.json"].map((file) =>
      readFile(path.join(root, file), "utf8"),
    ),
  );
  const dependencies = manifests.join("\n").toLowerCase();
  assert.doesNotMatch(dependencies, /pixez|notsfsssf|flutter|\bdart\b/);
});

test("compiled source contains no copied PixEz package or repository references", async () => {
  const files = (await Promise.all(sourceRoots.map(sourceFiles))).flat();
  for (const file of files) {
    const source = await readFile(path.join(root, file), "utf8");
    assert.doesNotMatch(source, /package:pixez|notsfsssf\/pixez|pixez-flutter/i, file);
  }
});

test("the project contains no Dart source outside research documentation", async () => {
  const dartFiles = [];
  async function visit(relativeDirectory) {
    const entries = await readdir(path.join(root, relativeDirectory), { withFileTypes: true });
    for (const entry of entries) {
      if (["node_modules", "target", "build", ".svelte-kit", "artifacts", "docs", "gen"].includes(entry.name)) continue;
      const relativePath = path.join(relativeDirectory, entry.name);
      if (entry.isDirectory()) await visit(relativePath);
      else if (entry.name.endsWith(".dart")) dartFiles.push(relativePath);
    }
  }
  await visit(".");
  assert.deepEqual(dartFiles, []);
});
