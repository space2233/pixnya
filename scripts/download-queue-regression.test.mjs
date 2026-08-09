import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("download queue owns a migrated bundled-SQLite state machine", async () => {
  const [workspace, manifest, queue] = await Promise.all([
    read("Cargo.toml"),
    read("crates/download-queue/Cargo.toml"),
    read("crates/download-queue/src/lib.rs"),
  ]);
  assert.match(workspace, /rusqlite = \{ version = "0\.40\.1", features = \["bundled"\] \}/);
  assert.match(manifest, /rusqlite\.workspace = true/);
  assert.match(queue, /CREATE TABLE IF NOT EXISTS schema_migrations/);
  assert.match(queue, /CREATE TABLE IF NOT EXISTS download_tasks/);
  assert.match(queue, /UNIQUE\(kind, resource_id\)/);
  assert.match(queue, /pub fn recover_interrupted/);
  assert.match(queue, /pub fn claim_next/);
  assert.match(queue, /pub fn pause/);
  assert.match(queue, /pub fn resume/);
  assert.match(queue, /pub fn mark_failed/);
  assert.match(queue, /pub fn mark_completed/);
  const task = queue.match(/pub struct DownloadTask \{([\s\S]*?)\n\}/)?.[1] ?? "";
  assert.doesNotMatch(task, /url|token|cookie|message|response_body/i);
});

test("Tauri runs one recoverable worker and exposes bounded queue commands", async () => {
  const [application, worker] = await Promise.all([
    read("src-tauri/src/lib.rs"),
    read("src-tauri/src/downloads.rs"),
  ]);
  const backend = `${application}\n${worker}`;
  assert.match(application, /mod downloads;/);
  assert.match(backend, /struct DownloadWorkerState/);
  assert.match(backend, /recover_interrupted/);
  assert.match(backend, /run_download_worker/);
  assert.match(backend, /enqueue_download,/);
  assert.match(backend, /list_download_tasks,/);
  assert.match(backend, /pause_download_task,/);
  assert.match(backend, /resume_download_task,/);
  assert.match(backend, /remove_download_task,/);
  assert.match(backend, /pixiv-download-queue-changed/);
  assert.match(backend, /DownloadFailure::Authentication/);
  assert.match(backend, /DownloadFailure::Network/);
  assert.match(backend, /DownloadFailure::InvalidResponse/);
  assert.match(backend, /DownloadFailure::Storage/);
});

test("detail pages enqueue work and offline library controls queue state", async () => {
  const [api, types, artwork, novel, offline] = await Promise.all([
    read("src/lib/pixiv-api.ts"),
    read("src/lib/types.ts"),
    read("src/routes/artworks/[id]/+page.svelte"),
    read("src/routes/novels/[id]/+page.svelte"),
    read("src/routes/offline/+page.svelte"),
  ]);
  assert.match(types, /export interface DownloadTask/);
  assert.match(api, /invoke<DownloadTask>\("enqueue_download"/);
  assert.match(api, /invoke<DownloadTask\[]>\("list_download_tasks"\)/);
  assert.match(api, /invoke<DownloadTask>\("pause_download_task"/);
  assert.match(api, /invoke<DownloadTask>\("resume_download_task"/);
  assert.match(api, /invoke<boolean>\("remove_download_task"/);
  assert.match(artwork, /enqueueDownload/);
  assert.match(novel, /enqueueDownload/);
  assert.match(offline, /m\.offline_queue_title\(\)/);
  assert.match(offline, /pixiv-download-queue-changed/);
  assert.match(offline, /m\.offline_pause\(\)/);
  assert.match(offline, /m\.offline_continue\(\)|m\.common_retry\(\)/);
});
