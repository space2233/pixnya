import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";

const source = fs.readFileSync(new URL("../src/lib/local-backup.ts", import.meta.url), "utf8");
const layout = fs.readFileSync(new URL("../src/routes/+layout.svelte", import.meta.url), "utf8");
const dataPage = fs.readFileSync(new URL("../src/routes/settings/data/+page.svelte", import.meta.url), "utf8");
const backend = fs.readFileSync(new URL("../src-tauri/src/local_backup.rs", import.meta.url), "utf8");

test("frontend backup uses an explicit credential-free local storage whitelist", () => {
  assert.match(source, /SEARCH_HISTORY_KEY/);
  assert.match(source, /NOVEL_PROGRESS_PREFIX/);
  assert.match(source, /SIDEBAR_KEY/);
  assert.match(source, /REDUCED_MOTION_KEY/);
  assert.match(source, /R18_KEY/);
  assert.doesNotMatch(source, /connection-mode|refresh.?token|access.?token|cookie/i);
});

test("interrupted restore persists and reapplies the previous frontend snapshot", () => {
  assert.match(dataPage, /startLocalBackupRestore\(strategy, previousFrontend\)/);
  assert.match(backend, /previous_frontend: FrontendBackupState/);
  assert.match(backend, /write_frontend_recovery\([\s\S]*marker\.previous_frontend/);
  assert.match(backend, /read_frontend_recovery\(&app_data\)\?\.is_some\(\)[\s\S]*BackupTransactionPending/);
  assert.match(layout, /getLocalBackupFrontendRecovery\(\)[\s\S]*restoreFrontendBackupState\(recovery\.frontend\)[\s\S]*acknowledgeLocalBackupFrontendRecovery/);
  assert.match(layout, /try \{[\s\S]*getLocalBackupFrontendRecovery\(\)[\s\S]*\} catch \{[\s\S]*routeReady = true/);
});

test("frontend restore keeps a rollback snapshot and bounds reading progress", () => {
  assert.match(source, /const previous = new Map/);
  assert.match(source, /catch \(error\)[\s\S]*previous/);
  assert.match(source, /progress < 0 \|\| progress > 1_000_000/);
});
