import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function createStorage() {
  const values = new Map();
  return {
    getItem(key) {
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
    clear() {
      values.clear();
    },
  };
}

globalThis.localStorage = createStorage();
globalThis.window = { dispatchEvent() {} };

const preferences = await import("../src/lib/preferences.ts");
const authority = await import("../src/lib/connection-mode-authority.ts");

test("an authenticated session snapshot repairs a conflicting local connection choice", () => {
  preferences.writePreferredConnectionMode("compatible");

  const resolved = preferences.reconcilePreferredConnectionMode({
    loggedIn: true,
    connectionMode: "ech",
  });

  assert.equal(resolved, "ech");
  assert.equal(preferences.readPreferredConnectionMode(), "ech");
});

test("a logged-out session continues to use the explicit local connection choice", () => {
  preferences.writePreferredConnectionMode("compatible");

  assert.equal(
    preferences.reconcilePreferredConnectionMode({ loggedIn: false }),
    "compatible",
  );
});

test("a probed connection choice is committed only to the unchanged session authority", () => {
  const aliceStandard = {
    loggedIn: true,
    user: { id: "alice" },
    connectionMode: "standard",
  };

  assert.equal(authority.sameConnectionModeAuthority(aliceStandard, aliceStandard), true);
  assert.equal(
    authority.sameConnectionModeAuthority(aliceStandard, {
      ...aliceStandard,
      connectionMode: "ech",
    }),
    false,
  );
  assert.equal(
    authority.sameConnectionModeAuthority(aliceStandard, {
      ...aliceStandard,
      user: { id: "bob" },
    }),
    false,
  );
  assert.equal(
    authority.sameConnectionModeAuthority(aliceStandard, { loggedIn: false }),
    false,
  );
  assert.equal(
    authority.sameConnectionModeAuthority({ loggedIn: false }, { loggedIn: false }),
    true,
  );
});

test("connection settings surfaces resolve display state through session authority", () => {
  const settings = readFileSync(
    new URL("../src/routes/settings/+page.svelte", import.meta.url),
    "utf8",
  );
  const network = readFileSync(
    new URL("../src/routes/settings/network/+page.svelte", import.meta.url),
    "utf8",
  );
  const session = readFileSync(new URL("../src/lib/session.ts", import.meta.url), "utf8");

  assert.match(settings, /reconcilePreferredConnectionMode/);
  assert.match(
    settings,
    /\$session\.loggedIn\s*&&\s*\$session\.connectionMode[\s\S]*?\$session\.connectionMode[\s\S]*?:\s*preferredConnectionMode/,
  );
  assert.match(network, /reconcilePreferredConnectionMode/);
  assert.match(network, /sameConnectionModeAuthority/);
  assert.match(
    network,
    /const sessionMode = \$session\.loggedIn \? \$session\.connectionMode : null/,
  );
  assert.doesNotMatch(network, /readPreferredConnectionMode\(\)\s*\?\?\s*snapshot\?\.connectionMode/);
  assert.doesNotMatch(
    network,
    /if \(saveOnSuccess\)[\s\S]*?initializeSession\(\)/,
    "saving a probe must not consult initializeSession's cached initial snapshot",
  );
  assert.match(
    network,
    /sameConnectionModeAuthority\(sessionAtProbeStart, currentSession\)[\s\S]*?switchSessionConnectionMode/,
  );
  assert.match(
    network,
    /catch \(error\)[\s\S]*?probeState = "failed";[\s\S]*?probeMessage = describeError\(error\);[\s\S]*?restoreAuthoritativeSelection\(\$session, false\)/,
    "a failed probe restores the authoritative selection without erasing its error feedback",
  );
  assert.match(
    network,
    /function restoreAuthoritativeSelection\(snapshot: SessionSnapshot, resetStatus = true\)[\s\S]*?if \(resetStatus\)[\s\S]*?probeState = "idle";[\s\S]*?probeMessage = "";/,
  );
  assert.match(session, /reconcilePreferredConnectionMode\(snapshot\)/);
});

test("first-run connection controls use defined theme tokens and expose the continue action", () => {
  const setup = readFileSync(
    new URL("../src/routes/setup/connection/+page.svelte", import.meta.url),
    "utf8",
  );
  const picker = readFileSync(
    new URL("../src/lib/components/ConnectionModePicker.svelte", import.meta.url),
    "utf8",
  );
  const appCss = readFileSync(new URL("../src/app.css", import.meta.url), "utf8");
  const definedTokens = new Set(
    [...appCss.matchAll(/--([a-z0-9-]+)\s*:/gi)].map((match) => match[1]),
  );
  const setupTokens = [...`${setup}\n${picker}`.matchAll(/var\(--([a-z0-9-]+)\)/gi)]
    .map((match) => match[1]);

  assert.ok(setupTokens.length > 0);
  assert.deepEqual(
    setupTokens.filter((token) => !definedTokens.has(token)),
    [],
    "an undefined color token can make the first-run continue button invisible",
  );
  assert.match(setup, /class="continue"[\s\S]*onclick=\{completeSetup\}/);
  assert.match(setup, /await goto\([\s\S]*replaceState:\s*true/);
});

test("connection choice uses the shared PixNya card, selection, and status language", () => {
  const setup = readFileSync(
    new URL("../src/routes/setup/connection/+page.svelte", import.meta.url),
    "utf8",
  );
  const picker = readFileSync(
    new URL("../src/lib/components/ConnectionModePicker.svelte", import.meta.url),
    "utf8",
  );

  assert.match(setup, /class="brand-mark"/);
  assert.doesNotMatch(setup, /brand-symbol|brand-letter/);
  assert.match(setup, /background:\s*var\(--soft-surface\)/);
  assert.match(setup, /grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  assert.match(setup, /\.continue:hover:not\(:disabled\)/);
  assert.match(picker, /class:available=\{state === "available"\}/);
  assert.match(picker, /class:checking=\{state === "checking"\}/);
  assert.match(
    picker,
    /\.mode-list button\.selected\s*\{[\s\S]*?border-color:\s*var\(--pixiv-blue\)/,
  );
  assert.match(picker, /\.probe-state\.available[\s\S]*?var\(--success\)/);
});
