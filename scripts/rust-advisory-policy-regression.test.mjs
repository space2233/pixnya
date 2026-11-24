import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const policyPath = path.join(process.cwd(), "deny.toml");
test("audit every release target without advisory exceptions", async () => {
  const policy = await readFile(policyPath, "utf8");
  const [, targets] = policy.match(/targets\s*=\s*\[([^\]]+)\]/) ?? [];
  assert.deepEqual([...targets.matchAll(/"([^"]+)"/g)].map((match) => match[1]), [
    "x86_64-pc-windows-msvc",
    "aarch64-linux-android",
    "armv7-linux-androideabi",
  ]);
  assert.match(policy, /^unmaintained = "all"$/m);
  assert.match(policy, /^unsound = "all"$/m);
  assert.match(policy, /^unused-ignored-advisory = "deny"$/m);
  assert.match(policy, /^ignore = \[\]$/m);
});
