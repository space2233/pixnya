import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const policyPath = path.join(process.cwd(), "deny.toml");
const reviewDatePattern = /review-by=(\d{4}-\d{2}-\d{2})/;
const expectedTemporaryRustSecExceptions = new Set([
  "RUSTSEC-2024-0370",
  "RUSTSEC-2024-0411",
  "RUSTSEC-2024-0412",
  "RUSTSEC-2024-0413",
  "RUSTSEC-2024-0414",
  "RUSTSEC-2024-0415",
  "RUSTSEC-2024-0416",
  "RUSTSEC-2024-0417",
  "RUSTSEC-2024-0418",
  "RUSTSEC-2024-0419",
  "RUSTSEC-2024-0420",
  "RUSTSEC-2024-0429",
  "RUSTSEC-2025-0075",
  "RUSTSEC-2025-0080",
  "RUSTSEC-2025-0081",
  "RUSTSEC-2025-0098",
  "RUSTSEC-2025-0100",
]);

function parseTemporaryExceptions(policy) {
  const ignoreBlocks = [...policy.matchAll(/^ignore\s*=\s*\[([\s\S]*?)^\]/gm)];
  assert.equal(ignoreBlocks.length, 1, "Rust advisory policy must contain exactly one ignore array");
  const entryPattern = /\{\s*id\s*=\s*"(RUSTSEC-\d{4}-\d{4})"\s*,\s*reason\s*=\s*"([^"]+)"\s*\}/g;
  const exceptions = [...ignoreBlocks[0][1].matchAll(entryPattern)].map(([, id, reason]) => ({ id, reason }));
  const unparsed = ignoreBlocks[0][1]
    .replace(entryPattern, "")
    .replace(/#[^\n]*/g, "")
    .replace(/[\s,]/g, "");
  assert.equal(unparsed, "", "every ignored advisory must use the reviewed { id, reason } form");
  return exceptions;
}

test("Rust advisory policy keeps a narrow, expiring list of reviewed upstream exceptions", async () => {
  const policy = await readFile(policyPath, "utf8");
  const exceptions = parseTemporaryExceptions(policy);
  const ids = exceptions.map(({ id }) => id);

  assert.match(policy, /^\[advisories\]$/m);
  assert.match(policy, /^unmaintained\s*=\s*"all"$/m);
  assert.match(policy, /^unsound\s*=\s*"all"$/m);
  assert.match(policy, /^unused-ignored-advisory\s*=\s*"deny"$/m);
  assert.equal(new Set(ids).size, ids.length, "temporary RustSec exceptions must be unique");
  assert.deepEqual(new Set(ids), expectedTemporaryRustSecExceptions);

  const today = new Date();
  today.setUTCHours(0, 0, 0, 0);
  for (const { id, reason } of exceptions) {
    const match = reason.match(reviewDatePattern);
    assert.ok(match, `${id} must include a machine-readable review-by date`);
    assert.equal(match[1], "2026-10-02", `${id} must not be extended without review`);
    assert.ok(new Date(`${match[1]}T00:00:00Z`) >= today, `${id} expired on ${match[1]}`);
    assert.match(reason, /upstream=/, `${id} must identify its upstream migration path`);
    if (id === "RUSTSEC-2024-0429") {
      assert.match(reason, /risk-review=no-locked-nontest-callers/);
    }
  }
});
