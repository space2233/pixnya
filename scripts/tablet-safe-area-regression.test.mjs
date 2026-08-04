import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();

function cssBlock(css, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`))?.[1] ?? "";
}

test("tablet and desktop chrome reserve the Android status-bar safe area", async () => {
  const [css, appHtml] = await Promise.all([
    readFile(path.join(root, "src/app.css"), "utf8"),
    readFile(path.join(root, "src/app.html"), "utf8"),
  ]);

  assert.match(appHtml, /name="viewport"\s+content="[^"]*viewport-fit=cover[^"]*"/);
  assert.match(css, /--app-safe-top:\s*env\(safe-area-inset-top,\s*0px\)/);
  assert.match(
    css,
    /--app-header-height:\s*calc\(var\(--topbar-height\) \+ var\(--app-safe-top\)\)/,
  );

  for (const selector of [".side-brand", ".app-topbar"]) {
    const block = cssBlock(css, selector);
    assert.match(block, /height:\s*var\(--app-header-height\)/, `${selector} must include the safe-area height`);
    assert.match(block, /padding-top:\s*var\(--app-safe-top\)/, `${selector} must push content below the status bar`);
  }
});
