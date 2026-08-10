import assert from "node:assert/strict";
import test from "node:test";

import { inspectAndroidGradleSupplyChain } from "./check-android-gradle-supply-chain.mjs";
import {
  androidReleaseRuntimeConfiguration,
  createAndroidRuntimeSpdx,
} from "./generate-android-runtime-sbom.mjs";

test("Android runtime SBOM contains only the locked ARM64 release runtime graph", () => {
  const inventory = inspectAndroidGradleSupplyChain();
  const sbom = createAndroidRuntimeSpdx(inventory, "2026-08-09T00:00:00Z");
  const purls = sbom.packages.map((entry) => entry.externalRefs[0].referenceLocator);

  assert.ok(purls.includes("pkg:maven/com.fasterxml.jackson.core/jackson-databind@2.22.1"));
  assert.ok(purls.includes("pkg:maven/com.fasterxml.jackson.core/jackson-core@2.22.1"));
  assert.ok(!purls.some((value) => value.includes("jackson-databind@2.15.3")));
  assert.ok(!purls.some((value) => value.includes("io.netty")));
  assert.equal(new Set(purls).size, purls.length);

  const selectedCoordinates = new Set(
    inventory.components
      .filter(
        (component) =>
          component.lockfiles.includes("app/gradle.lockfile") &&
          component.configurations.includes(androidReleaseRuntimeConfiguration),
      )
      .map((component) => component.coordinate),
  );
  assert.equal(sbom.packages.length, selectedCoordinates.size);
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
