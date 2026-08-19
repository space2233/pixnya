import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("../", import.meta.url));
const sourceRoot = path.join(projectRoot, "src");

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(target);
    return /\.(?:css|svelte)$/.test(entry.name) ? [target] : [];
  });
}

const typographyValues = new Set([
  "var(--type-caption)",
  "var(--type-small)",
  "var(--type-body)",
  "var(--type-label)",
  "var(--type-section)",
  "var(--type-title)",
]);
const readerValues = new Set([
  "var(--font)",
  "var(--reader-font)",
  "calc(var(--font)*1.3)",
  "calc(var(--reader-font) * 1.3)",
  "calc(var(--reader-font) * .72)",
  ".72em",
]);

test("all application text uses the shared six-level typography scale", () => {
  const appCss = readFileSync(path.join(sourceRoot, "app.css"), "utf8");
  const expectedTokens = {
    caption: "0.6875rem",
    small: "0.75rem",
    body: "0.875rem",
    label: "0.9375rem",
    section: "1.125rem",
    title: "1.5rem",
  };
  for (const [name, value] of Object.entries(expectedTokens)) {
    assert.match(appCss, new RegExp(`--type-${name}:\\s*${value.replace(".", "\\.")}`));
  }

  const violations = [];
  for (const file of sourceFiles(sourceRoot)) {
    const source = readFileSync(file, "utf8");
    for (const match of source.matchAll(/font-size\s*:\s*([^;}\r\n]+)/g)) {
      const value = match[1].trim();
      const relative = path.relative(projectRoot, file).replaceAll("\\", "/");
      const isReader = relative === "src/routes/novels/[id]/read/+page.svelte"
        || relative === "src/routes/offline/novels/[id]/+page.svelte";
      if (!typographyValues.has(value) && !(isReader && readerValues.has(value))) {
        violations.push(`${relative}: ${value}`);
      }
    }
    for (const match of source.matchAll(/(?:^|[;{])\s*font\s*:\s*([^;}\r\n]+)/gm)) {
      const value = match[1].trim();
      if (/\b\d+(?:\.\d+)?(?:px|rem)\b/.test(value)) {
        const relative = path.relative(projectRoot, file).replaceAll("\\", "/");
        violations.push(`${relative}: font shorthand ${value}`);
      }
    }
  }

  assert.deepEqual(violations, []);
});

test("text buttons use the shared body size while symbol-only controls keep icon sizing", () => {
  const symbolOnlySelectors = new Set([
    ".viewer header button",
    ".viewer footer button",
    ".page-nav",
    ".history-list article > button",
  ]);
  const violations = [];
  const files = sourceFiles(sourceRoot);
  const buttonClasses = new Set(
    files
      .filter((file) => file.endsWith(".svelte"))
      .flatMap((file) => [...readFileSync(file, "utf8").matchAll(/<button\b([\s\S]*?)>/g)])
      .flatMap((button) => [
        ...[...button[1].matchAll(/\bclass\s*=\s*"([^"]*)"/g)]
          .flatMap((match) => match[1].split(/\s+/).filter(Boolean)),
        ...[...button[1].matchAll(/\bclass:([a-zA-Z0-9_-]+)/g)].map((match) => match[1]),
      ]),
  );

  for (const file of files) {
    const source = readFileSync(file, "utf8");
    const styles = file.endsWith(".css")
      ? [source]
      : [...source.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((match) => match[1]);
    for (const style of styles) {
      for (const rule of style.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
        const selectors = rule[1].split(",").map((selector) => selector.replace(/\s+/g, " ").trim());
        const fontSize = rule[2].match(/font-size\s*:\s*([^;}\r\n]+)/)?.[1].trim();
        if (!fontSize) continue;
        for (const selector of selectors.filter((candidate) => {
          const target = candidate.split(/[\s>+~]+/).filter(Boolean).at(-1) ?? "";
          return /\bbutton\b/.test(target)
            || [...buttonClasses].some((className) =>
              new RegExp(`\\.${className.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?![a-zA-Z0-9_-])`).test(target),
            );
        })) {
          if (symbolOnlySelectors.has(selector) || fontSize === "var(--type-body)") continue;
          violations.push(
            `${path.relative(projectRoot, file).replaceAll("\\", "/")}: ${selector} -> ${fontSize}`,
          );
        }
      }
    }
  }

  assert.deepEqual(violations, []);
});

test("mobile AppShell pages hide only a duplicated semantic page title", () => {
  const violations = [];
  for (const file of sourceFiles(path.join(sourceRoot, "routes"))) {
    const source = readFileSync(file, "utf8");
    const shell = source.match(/<AppShell\s+title=\{m\.([a-zA-Z0-9_]+)\(\)\}/);
    const heading = source.match(/<h1(?<attributes>[^>]*)>\{m\.([a-zA-Z0-9_]+)\(\)\}<\/h1>/);
    if (shell && heading && shell[1] === heading[2] && !/\bpage-title\b/.test(heading.groups.attributes)) {
      violations.push(path.relative(projectRoot, file).replaceAll("\\", "/"));
    }
  }
  assert.deepEqual(violations, []);

  const appCss = readFileSync(path.join(sourceRoot, "app.css"), "utf8");
  assert.match(
    appCss,
    /@media \(max-width: 959px\)[\s\S]*\.app-content \.page-title\s*\{[\s\S]*display:\s*none/,
  );
});
