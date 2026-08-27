import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const route = await import("../src/lib/search-route-state.ts");

test("search route state parses reviewed types and match targets with safe defaults", () => {
  assert.deepEqual(route.readSearchRouteState(new URLSearchParams("q=cat")), {
    query: "cat",
    type: "works",
    target: "partial_match_for_tags",
  });
  assert.deepEqual(
    route.readSearchRouteState(
      new URLSearchParams("q=night&type=novels&target=title_and_caption"),
    ),
    { query: "night", type: "novels", target: "title_and_caption" },
  );
  assert.deepEqual(
    route.readSearchRouteState(
      new URLSearchParams("q=user&type=users&target=title_and_caption"),
    ),
    { query: "user", type: "users", target: "partial_match_for_tags" },
  );
  assert.deepEqual(
    route.readSearchRouteState(new URLSearchParams("q=cat&type=invalid&target=invalid")),
    { query: "cat", type: "works", target: "partial_match_for_tags" },
  );
});

test("search route hrefs omit defaults and retain meaningful non-default state", () => {
  assert.equal(
    route.searchRouteHref({ query: "cat", type: "works", target: "partial_match_for_tags" }),
    "/search?q=cat",
  );
  assert.equal(
    route.searchRouteHref({ query: "night sky", type: "novels", target: "title_and_caption" }),
    "/search?q=night+sky&type=novels&target=title_and_caption",
  );
  assert.equal(
    route.searchRouteHref({ query: "alice", type: "users", target: "title_and_caption" }),
    "/search?q=alice&type=users",
  );
  assert.equal(
    route.searchRouteHref({ query: "tag", type: "tags", target: "title_and_caption" }),
    "/search?q=tag&type=tags",
  );
});

test("effective targets and request keys keep search modes isolated", () => {
  const partial = { query: "cat", type: "works", target: "partial_match_for_tags" };
  const title = { ...partial, target: "title_and_caption" };
  assert.equal(route.effectiveSearchTarget(partial), "partial_match_for_tags");
  assert.equal(route.effectiveSearchTarget(title), "title_and_caption");
  assert.equal(
    route.effectiveSearchTarget({ ...partial, type: "tags" }),
    "exact_match_for_tags",
  );
  assert.equal(route.effectiveSearchTarget({ ...partial, type: "users" }), null);
  assert.notEqual(route.searchRequestKey("account", partial), route.searchRequestKey("account", title));
});

test("search page binds URL state to controls, requests, pagination, and snapshots", async () => {
  const page = await readFile(
    new URL("../src/routes/search/+page.svelte", import.meta.url),
    "utf8",
  );
  for (const symbol of [
    "readSearchRouteState",
    "searchRouteHref",
    "effectiveSearchTarget",
    "searchRequestKey",
    "supportsMatchTarget",
  ]) {
    assert.match(page, new RegExp(symbol));
  }
  assert.match(page, /activeTarget:\s*SearchMatchTarget/);
  assert.match(page, /activeTarget:\s*currentRoute\.target/);
  assert.match(page, /goto\(searchRouteHref\([\s\S]*replaceState:\s*true/);
  assert.match(page, /searchNovels\([\s\S]*target/);
  assert.match(page, /searchIllustrations\([\s\S]*target/);
  assert.match(page, /search_target_title_and_caption/);
  assert.match(page, /search_target_partial_tags/);
});
