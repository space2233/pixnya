import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  connectionSetupUrl,
  safeConnectionReturnTarget,
} from "../src/lib/connection-onboarding.ts";

test("first-run redirects preserve only safe in-app return targets", () => {
  assert.equal(connectionSetupUrl("/search?q=miku#results"), "/setup/connection?returnTo=%2Fsearch%3Fq%3Dmiku%23results");
  assert.equal(safeConnectionReturnTarget("/novels/42?from=home"), "/novels/42?from=home");
  assert.equal(safeConnectionReturnTarget("https://example.com/steal"), "/");
  assert.equal(safeConnectionReturnTarget("//example.com/steal"), "/");
  assert.equal(safeConnectionReturnTarget("/setup/connection"), "/");
  assert.equal(safeConnectionReturnTarget(null), "/");
});

test("root layout owns readiness and gates routes without a saved connection choice", () => {
  const layout = readFileSync(new URL("../src/routes/+layout.svelte", import.meta.url), "utf8");
  const appShell = readFileSync(
    new URL("../src/lib/components/AppShell.svelte", import.meta.url),
    "utf8",
  );
  assert.match(layout, /mark_frontend_ready/);
  assert.match(layout, /readPreferredConnectionMode\(\)/);
  assert.match(layout, /\/setup\/connection/);
  assert.match(layout, /replaceState:\s*true/);
  assert.doesNotMatch(appShell, /mark_frontend_ready/);
});

test("first connection page is standalone, shared, and warning-free", () => {
  const setup = readFileSync(
    new URL("../src/routes/setup/connection/+page.svelte", import.meta.url),
    "utf8",
  );
  assert.match(setup, /ConnectionModePicker/);
  assert.match(setup, /probe_connection/);
  assert.match(setup, /unsafeAcknowledged:\s*true/);
  assert.doesNotMatch(setup, /AppShell/);
  assert.doesNotMatch(setup, /risk|warning|danger/i);
});

test("completing first-run setup refreshes the session before applying the selected mode", () => {
  const setup = readFileSync(
    new URL("../src/routes/setup/connection/+page.svelte", import.meta.url),
    "utf8",
  );
  assert.match(
    setup,
    /async function completeSetup\(\)[\s\S]*await initializeSession\(\)[\s\S]*switchSessionConnectionMode/,
  );
  assert.match(
    setup,
    /currentSession\.loggedIn\s*&&\s*probeState\s*!==\s*"available"[\s\S]*?currentSession\.connectionMode\s*\?\?\s*selected[\s\S]*?writePreferredConnectionMode\(modeToPersist\)/,
  );
});
