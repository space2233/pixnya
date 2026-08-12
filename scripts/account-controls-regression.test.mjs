import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const read = (path) => readFile(new URL(path, root), "utf8");

test("official access-block and mute APIs stay separate from local moderation", async () => {
  const [api, backend, page] = await Promise.all([
    read("src/lib/pixiv-api.ts"),
    read("src-tauri/src/lib.rs"),
    read("src/routes/settings/account-controls/+page.svelte"),
  ]);

  for (const command of [
    "get_access_blocked_users",
    "set_access_block",
    "get_mute_settings",
    "set_user_mute",
    "set_tag_mute",
  ]) {
    assert.match(api, new RegExp(`\\"${command}\\"`));
    assert.match(backend, new RegExp(`async fn ${command}\\b`));
  }
  assert.match(page, /confirm\(/);
  assert.match(page, /pendingMutation/);
  assert.match(page, /local-only/i);
  assert.doesNotMatch(page, /setLocalHidden|reportComment/);
});

test("account control writes are serialized and stale session results are rejected", async () => {
  const [backend, page] = await Promise.all([
    read("src-tauri/src/lib.rs"),
    read("src/routes/settings/account-controls/+page.svelte"),
  ]);
  assert.match(backend, /session_switch\s*\.\s*mutation_guard\(\)/);
  const mutation = backend.match(/async fn execute_authenticated_mutation[\s\S]*?\n}\n\nasync fn refresh_context_after_rejection/)?.[0] ?? "";
  assert.match(mutation, /expected_user_id[\s\S]*prepared_context/);
  assert.match(mutation, /operation_guard\(\)\.await[\s\S]*authenticated_context\(0\)/);
  assert.match(mutation, /request_authenticated_data\(context, data_state, request\)\.await/);
  assert.doesNotMatch(mutation, /execute_authenticated_data_request/);
  assert.ok(
    mutation.indexOf("let expected_user_id = ensure_authenticated_context")
      < mutation.indexOf(".mutation_guard"),
    "the account must be captured before a queued mutation waits for the write gate",
  );
  assert.match(page, /requestedSession !== sessionKey/);
  assert.match(page, /pendingMutation = false/);
});
