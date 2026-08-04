import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import path from "node:path";

const offline = readFileSync(path.join(process.cwd(), "src", "routes", "offline", "+page.svelte"), "utf8");

test("offline section refresh buttons stay horizontal on narrow screens", () => {
  const refreshButtons = offline.match(/class="section-refresh"/g) ?? [];
  assert.ok(refreshButtons.length >= 2, "queue and downloaded-content refresh must share the layout contract");
  assert.match(
    offline,
    /\.section-refresh\s*\{[\s\S]*?min-width:\s*(?:6[4-9]|[7-9]\d)px[\s\S]*?white-space:\s*nowrap/,
  );
  assert.match(offline, /\.section-refresh\s*\{[\s\S]*?flex:\s*0\s+0\s+auto/);
});
