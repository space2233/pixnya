import assert from "node:assert/strict";
import test from "node:test";
import {
  NOVEL_READER_LOAD_ATTEMPTS,
  shouldRetryNovelReaderLoad,
} from "../src/lib/novel-reader-load.ts";

test("novel reader load retries a fixed number of times then stops", () => {
  assert.equal(NOVEL_READER_LOAD_ATTEMPTS, 3);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "invalid_response" }, 1), true);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "invalid_response" }, 2), true);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "invalid_response" }, 3), false);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "request_failed" }, 2), true);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "transport_unavailable" }, 1), true);
});

test("novel reader load does not retry authentication or identifier failures", () => {
  assert.equal(shouldRetryNovelReaderLoad({ kind: "authentication_required" }, 1), false);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "invalid_identifier" }, 1), false);
  assert.equal(shouldRetryNovelReaderLoad({ kind: "invalid_input" }, 1), false);
});
