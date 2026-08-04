import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

const apiAdapter = source("../crates/api/src/lib.rs");
const rustCommands = source("../src-tauri/src/lib.rs");
const frontendApi = source("../src/lib/pixiv-api.ts");
const frontendTypes = source("../src/lib/types.ts");

test("authenticated Pixiv requests keep both tokens behind the Rust IPC boundary", () => {
  assert.match(apiAdapter, /\.bearer_auth\(access_token\)/);
  assert.match(apiAdapter, /X-Client-Time/);
  assert.match(apiAdapter, /X-Client-Hash/);
  assert.match(rustCommands, /ensure_authenticated_context/);
  assert.match(rustCommands, /get_recommended_illustrations/);
  assert.doesNotMatch(frontendApi, /accessToken|refreshToken|Authorization/i);
  assert.doesNotMatch(frontendTypes, /accessToken|refreshToken/);
});

test("pagination cursors and thumbnail URLs cannot redirect Rust to arbitrary hosts", () => {
  assert.match(apiAdapter, /url\.host_str\(\) == Some\(API_HOST\)/);
  assert.match(apiAdapter, /bindings_match/);
  assert.match(apiAdapter, /Some\("i\.pximg\.net" \| "s\.pximg\.net"\)/);
  assert.match(rustCommands, /MAX_THUMBNAIL_BYTES/);
  assert.match(rustCommands, /TrafficClass::Media/);
});

test("expired API sessions refresh once before authenticated data is requested", () => {
  assert.match(rustCommands, /API_TOKEN_MINIMUM_TTL_SECONDS/);
  assert.match(rustCommands, /refresh_context_after_rejection/);
  assert.match(
    rustCommands,
    /ApiCommandError::AuthenticationRequired[\s\S]*refresh_context_after_rejection/,
  );
  assert.match(rustCommands, /invalidate_session_generation/);
});
