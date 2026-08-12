import assert from "node:assert/strict";
import test from "node:test";

import { validateStableReleaseNotes } from "./validate-release-notes.mjs";

const version = "1.3.0";
const commitSha = "0123456789abcdef0123456789abcdef01234567";
const completeNotes = `# PixNya ${version}

## \u4e2d\u6587

- \u652f\u6301 Windows x64\u3001Windows ARM64\u3001Linux x64\u3001Android ARM64 \u548c Android ARM32\u3002

## English

- Supports Windows x64, Windows ARM64, Linux x64, Android ARM64, and Android ARM32.`;

test("stable notes name every formally released architecture", () => {
  assert.doesNotThrow(() => validateStableReleaseNotes({ notes: completeNotes, version, commitSha }));

  for (const platform of ["Windows x64", "Windows ARM64", "Linux x64", "Android ARM64", "Android ARM32"]) {
    assert.throws(
      () => validateStableReleaseNotes({
        notes: completeNotes.replaceAll(platform, "unsupported architecture"),
        version,
        commitSha,
      }),
      new RegExp(`supported platform is missing: ${platform.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`),
    );
  }
});
