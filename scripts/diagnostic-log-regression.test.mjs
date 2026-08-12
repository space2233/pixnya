import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { androidPackagePath } from "./test-paths.mjs";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("diagnostic entries accept only typed, non-identifying fields", async () => {
  const source = await read("crates/diagnostic-log/src/lib.rs");
  const entry = source.match(/pub struct DiagnosticEntry \{([\s\S]*?)\n\}/)?.[1] ?? "";

  assert.ok(entry);
  assert.doesNotMatch(entry, /String|&str|Url|user|account|work_id|query|message|body/i);
  assert.match(source, /DEFAULT_MAX_BYTES: u64 = 256 \* 1024/);
  assert.match(source, /DEFAULT_RETENTION_SECONDS: u64 = 7 \* 24 \* 60 \* 60/);
  assert.match(source, /privacy=no tokens, cookies, URLs, account IDs, work IDs, search terms, or response bodies/);
});

test("application runtime has no direct logging bypass", async () => {
  const files = [
    ...(await sourceFiles("crates", new Set([".rs"]))),
    ...(await sourceFiles("src-tauri/src", new Set([".rs"]))),
    ...(await sourceFiles("src", new Set([".ts", ".svelte"]))),
  ];
  for (const file of files) {
    const source = await readFile(file, "utf8");
    assert.doesNotMatch(
      source,
      /\b(?:e?println|dbg)!|console\.(?:log|warn|error)\s*\(/,
      path.relative(root, file),
    );
  }
});

test("settings and Tauri expose local-only export and confirmed clearing", async () => {
  const [settings, api, backend, android] = await Promise.all([
    read("src/routes/settings/privacy/+page.svelte"),
    read("src/lib/pixiv-api.ts"),
    read("src-tauri/src/lib.rs"),
    readFile(androidPackagePath("LoginWebViewPlugin.kt"), "utf8"),
  ]);

  assert.match(settings, /m\.settings_diagnostic_log\(\)/);
  assert.doesNotMatch(settings, /settings_diagnostic_exclusions/);
  assert.match(api, /invoke<DiagnosticLogExportResult>\("export_diagnostic_logs"\)/);
  assert.match(api, /invoke<DiagnosticLogSummary>\("clear_diagnostic_logs", \{ confirmed: true \}\)/);
  assert.match(backend, /\.manage\(DiagnosticLogState::default\(\)\)/);
  assert.match(backend, /get_diagnostic_log_summary,/);
  assert.match(backend, /export_diagnostic_logs,/);
  assert.match(backend, /clear_diagnostic_logs,/);
  assert.match(backend, /with_diagnostic_log\(&app, DiagnosticLog::clear\)/);
  assert.match(android, /MediaStore\.Downloads\.EXTERNAL_CONTENT_URI/);
  assert.match(android, /bytes\.size > 512 \* 1024/);
  assert.match(android, /fileName\.matches\(Regex\(/);
  assert.match(android, /pixnya-diagnostics-/);
});

async function sourceFiles(relativeDirectory, extensions) {
  const directory = path.join(root, relativeDirectory);
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await sourceFiles(path.relative(root, fullPath), extensions)));
    } else if (extensions.has(path.extname(entry.name))) {
      files.push(fullPath);
    }
  }
  return files;
}
