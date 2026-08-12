import assert from "node:assert/strict";
import test from "node:test";

import { inspectAndroidGradleSupplyChain } from "./check-android-gradle-supply-chain.mjs";
import {
  androidReleaseRuntimeConfigurations,
  createAndroidRuntimeSpdx,
} from "./generate-android-runtime-sbom.mjs";

test("Android runtime SBOM requires identical locked ARM64 and ARMv7 release graphs", () => {
  const inventory = inspectAndroidGradleSupplyChain();
  const sbom = createAndroidRuntimeSpdx(inventory, "2026-08-09T00:00:00Z");
  const purls = sbom.packages.map((entry) => entry.externalRefs[0].referenceLocator);

  assert.ok(purls.includes("pkg:maven/com.fasterxml.jackson.core/jackson-databind@2.22.1"));
  assert.ok(purls.includes("pkg:maven/com.fasterxml.jackson.core/jackson-core@2.22.1"));
  assert.ok(!purls.some((value) => value.includes("jackson-databind@2.15.3")));
  assert.ok(!purls.some((value) => value.includes("io.netty")));
  assert.equal(new Set(purls).size, purls.length);

  const selectedGraphs = androidReleaseRuntimeConfigurations.map((configuration) => new Set(
    inventory.components
      .filter(
        (component) =>
          component.lockfiles.includes("app/gradle.lockfile") &&
          component.configurations.includes(configuration),
      )
      .map((component) => component.coordinate),
  ));
  assert.deepEqual([...selectedGraphs[0]].sort(), [...selectedGraphs[1]].sort());
  assert.equal(sbom.packages.length, selectedGraphs[0].size);
  assert.equal(sbom.name, "PixNya Android ARM release runtime");
});

test("Android runtime SBOM output is deterministic for a fixed graph and epoch", () => {
  const inventory = inspectAndroidGradleSupplyChain();
  const first = createAndroidRuntimeSpdx(inventory, "2026-08-09T00:00:00Z");
  const second = createAndroidRuntimeSpdx(inventory, "2026-08-09T00:00:00Z");
  assert.deepEqual(first, second);
  assert.match(first.documentNamespace, /\/sbom\/android-runtime\/[a-f0-9]{64}$/);
});

test("Android runtime SBOM generation fails closed when the release graph is absent", () => {
  assert.throws(
    () => createAndroidRuntimeSpdx({ components: [] }, "2026-08-09T00:00:00Z"),
    /No components were locked/,
  );
});

test("Android runtime SBOM generation fails closed when one ABI graph is missing or diverges", () => {
  const inventory = inspectAndroidGradleSupplyChain();
  const armv7Configuration = androidReleaseRuntimeConfigurations[1];
  const missingArmv7 = {
    ...inventory,
    components: inventory.components.map((component) => ({
      ...component,
      configurations: component.configurations.filter((value) => value !== armv7Configuration),
    })),
  };
  assert.throws(
    () => createAndroidRuntimeSpdx(missingArmv7, "2026-08-09T00:00:00Z"),
    /No components were locked/,
  );

  const divergent = structuredClone(inventory);
  const component = divergent.components.find((entry) =>
    entry.configurations.includes(armv7Configuration));
  component.configurations = component.configurations.filter((value) => value !== armv7Configuration);
  assert.throws(
    () => createAndroidRuntimeSpdx(divergent, "2026-08-09T00:00:00Z"),
    /release runtime graphs differ/,
  );
});
