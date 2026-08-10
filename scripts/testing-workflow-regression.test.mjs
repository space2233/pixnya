import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (relativePath) => readFile(new URL(`../${relativePath}`, import.meta.url), "utf8");

test("package scripts expose quick, Rust, and full test tiers", async () => {
  const packageJson = JSON.parse(await read("package.json"));

  assert.equal(packageJson.scripts.test, "node scripts/run-test-suite.mjs quick");
  assert.equal(packageJson.scripts["test:quick"], "node scripts/run-test-suite.mjs quick");
  assert.equal(packageJson.scripts["test:rust"], "node scripts/run-test-suite.mjs rust");
  assert.equal(packageJson.scripts["test:full"], "node scripts/run-test-suite.mjs full");
});

test("quick tests run all Node regressions before Svelte checks", async () => {
  const runner = await read("scripts/run-test-suite.mjs");

  assert.match(runner, /entry\.name\.endsWith\("\.test\.mjs"\)/);
  assert.match(runner, /"--test",\s*\.\.\.testFiles/);
  assert.match(runner, /npmInvocation\("run", "check"\)/);
  assert.doesNotMatch(runner, /shell:\s*true/);
  assert.ok(runner.indexOf("Node regression tests") < runner.indexOf("Svelte type and accessibility checks"));
});

test("Rust tests remain strict without recreating incremental caches", async () => {
  const runner = await read("scripts/run-test-suite.mjs");

  assert.match(runner, /CARGO_INCREMENTAL: "0"/);
  assert.match(runner, /"fmt", "--all", "--", "--check"/);
  assert.match(runner, /"clippy", "--workspace", "--all-targets"/);
  assert.match(runner, /"test", "--workspace"/);
});

test("Linux CI hydrates the pinned Rust graph before quick tests and defers platform packages", async () => {
  const workflow = await read(".github/workflows/linux.yml");
  const rustSetupIndex = workflow.indexOf(
    "uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c",
  );
  const cargoFetchIndex = workflow.indexOf("cargo fetch --locked");
  const quickIndex = workflow.indexOf("npm run test:quick");
  const aptIndex = workflow.indexOf("sudo apt-get update");

  assert.ok(rustSetupIndex >= 0);
  assert.ok(cargoFetchIndex >= 0);
  assert.ok(quickIndex >= 0);
  assert.ok(aptIndex >= 0);
  assert.ok(rustSetupIndex < cargoFetchIndex);
  assert.ok(cargoFetchIndex < quickIndex);
  assert.ok(quickIndex < aptIndex);
  assert.match(workflow, /toolchain: 1\.97\.1/);
  assert.match(workflow, /components: clippy, rustfmt/);
  assert.doesNotMatch(workflow, /rustup default stable/);
  assert.match(workflow, /bash scripts\/check-linux\.sh rust-only/);
});
