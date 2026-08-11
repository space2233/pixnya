import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { classifyNotificationLink } from "../src/lib/notification-link.ts";

const root = new URL("../", import.meta.url);

test("notification links are restricted to Pixiv and known in-app resources", () => {
  assert.deepEqual(classifyNotificationLink("https://www.pixiv.net/artworks/123"), {
    kind: "internal",
    href: "/artworks/123",
  });
  assert.deepEqual(classifyNotificationLink("https://www.pixiv.net/novel/show.php?id=456"), {
    kind: "internal",
    href: "/novels/456",
  });
  assert.deepEqual(classifyNotificationLink("https://www.pixiv.net/users/789"), {
    kind: "internal",
    href: "/users/789",
  });
  assert.deepEqual(classifyNotificationLink("https://www.pixiv.net/info.php?id=1"), {
    kind: "external",
    href: "https://www.pixiv.net/info.php?id=1",
  });
  assert.equal(classifyNotificationLink("http://www.pixiv.net/artworks/123"), null);
  assert.equal(classifyNotificationLink("https://pixiv.net/artworks/123"), null);
  assert.equal(classifyNotificationLink("https://www.pixiv.net.evil.example/artworks/123"), null);
  assert.equal(classifyNotificationLink("javascript:alert(1)"), null);
});

test("notification UI is read-only, paginated, and uses the safe link boundary", async () => {
  const page = await readFile(new URL("src/routes/notifications/+page.svelte", root), "utf8");
  const api = await readFile(new URL("src/lib/pixiv-api.ts", root), "utf8");
  const shell = await readFile(new URL("src/lib/components/AppShell.svelte", root), "utf8");
  assert.match(page, /getNotifications/);
  assert.match(page, /getNotificationViewMore/);
  assert.match(page, /groupCursors/);
  assert.match(page, /page\.nextCursor/);
  assert.match(page, /getNotificationViewMore\(item\.id, cursor \?\? undefined\)/);
  assert.match(page, /classifyNotificationLink/);
  assert.match(page, /openPixivUrl/);
  assert.match(page, /requestSequence/);
  assert.match(page, /expectedSession !== sessionKey/);
  assert.doesNotMatch(page, /setInterval|mark.*read|notification.*post/i);
  assert.match(api, /invoke<NotificationPage>\("get_notifications"/);
  assert.match(api, /invoke<NotificationPage>\("get_notification_view_more"/);
  assert.doesNotMatch(shell, /shell_notifications_unavailable/);
});
