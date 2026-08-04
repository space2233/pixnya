import assert from "node:assert/strict";
import test from "node:test";

import {
  bottomNavigationKeys,
  contentTabKeys,
  getNavigationItem,
  navigationKeyForPath,
  sideNavigationSections,
} from "../src/lib/navigation.ts";

test("each visible menu destination has one canonical href", () => {
  const visibleKeys = [
    ...sideNavigationSections.flatMap((section) => section.items),
    ...bottomNavigationKeys,
    ...contentTabKeys,
    "settings",
  ];

  for (const key of new Set(visibleKeys)) {
    const item = getNavigationItem(key);
    assert.equal(navigationKeyForPath(item.href), key);
  }
});

test("desktop following and mobile new-work entries share one destination", () => {
  const following = getNavigationItem("following");
  assert.ok(sideNavigationSections.some((section) => section.items.includes("following")));
  assert.ok(bottomNavigationKeys.includes("following"));
  assert.equal(following.href, "/following");
  assert.equal(following.compactLabel, "新作");
});

test("route aliases select the correct shared menu state", () => {
  assert.equal(navigationKeyForPath("/login"), "profile");
  assert.equal(navigationKeyForPath("/login/"), "profile");
  assert.equal(navigationKeyForPath("/settings"), "settings");
  assert.equal(navigationKeyForPath("/settings/network"), "settings");
  assert.equal(navigationKeyForPath("/artworks/42"), "artworks");
  assert.equal(navigationKeyForPath("/missing"), null);
});
