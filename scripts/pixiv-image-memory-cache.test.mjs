import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const cache = await import("../src/lib/pixiv-image-memory-cache.ts");
const profileMedia = await import("../src/lib/profile-media-memory.ts");

test("Pixiv image leases deduplicate in-flight work and synchronously reuse warm object URLs", async () => {
  cache.clearPixivImageMemoryCache();
  let resolveLoad;
  let loadCount = 0;
  const loader = () => {
    loadCount += 1;
    return new Promise((resolve) => { resolveLoad = resolve; });
  };
  const first = cache.acquirePixivImageSource("account:ech:thumbnail:url", loader);
  const second = cache.acquirePixivImageSource("account:ech:thumbnail:url", loader);
  assert.equal(first.source, null);
  assert.equal(second.source, null);
  assert.equal(loadCount, 1);

  resolveLoad(new Uint8Array([1, 2, 3, 4]));
  const source = await first.ready;
  assert.equal(await second.ready, source);
  first.release();
  second.release();

  const warm = cache.acquirePixivImageSource("account:ech:thumbnail:url", loader);
  assert.equal(warm.source, source);
  assert.equal(await warm.ready, source);
  assert.equal(loadCount, 1);
  warm.release();
  cache.clearPixivImageMemoryCache();
  assert.equal(cache.pixivImageMemoryCacheStatsForTests().entries, 0);
});

test("clearing invalidates pending loaders and bounded LRU entries revoke object URLs", async () => {
  cache.clearPixivImageMemoryCache();
  let resolvePending;
  const pending = cache.acquirePixivImageSource(
    "pending",
    () => new Promise((resolve) => { resolvePending = resolve; }),
  );
  cache.clearPixivImageMemoryCache();
  resolvePending(new Uint8Array([1]));
  await assert.rejects(pending.ready);
  pending.release();
  assert.equal(cache.pixivImageMemoryCacheStatsForTests().entries, 0);

  const originalRevoke = URL.revokeObjectURL;
  let revoked = 0;
  URL.revokeObjectURL = (value) => {
    revoked += 1;
    originalRevoke.call(URL, value);
  };
  try {
    for (let index = 0; index < 200; index += 1) {
      const lease = cache.acquirePixivImageSource(
        `entry-${index}`,
        async () => new Uint8Array([index % 255]),
      );
      await lease.ready;
      lease.release();
    }
    assert.ok(cache.pixivImageMemoryCacheStatsForTests().entries <= 192);
    assert.ok(revoked > 0);
  } finally {
    URL.revokeObjectURL = originalRevoke;
    cache.clearPixivImageMemoryCache();
  }
});

test("clearing stops reuse without reloading or breaking an image that is already mounted", async () => {
  cache.clearPixivImageMemoryCache();
  const originalRevoke = URL.revokeObjectURL;
  let revoked = 0;
  URL.revokeObjectURL = (value) => {
    revoked += 1;
    originalRevoke.call(URL, value);
  };
  try {
    const mounted = cache.acquirePixivImageSource(
      "mounted",
      async () => new Uint8Array([1, 2, 3]),
    );
    await mounted.ready;
    cache.clearPixivImageMemoryCache();
    assert.equal(cache.pixivImageMemoryCacheStatsForTests().entries, 0);
    assert.equal(revoked, 0);
    mounted.release();
    assert.equal(revoked, 1);
  } finally {
    URL.revokeObjectURL = originalRevoke;
    cache.clearPixivImageMemoryCache();
  }
});

test("an old lease error cannot invalidate a newer generation with the same key", async () => {
  cache.clearPixivImageMemoryCache();
  const oldLease = cache.acquirePixivImageSource("same-key", async () => new Uint8Array([1]));
  await oldLease.ready;
  cache.clearPixivImageMemoryCache();

  const newLease = cache.acquirePixivImageSource("same-key", async () => new Uint8Array([2]));
  const newSource = await newLease.ready;
  oldLease.invalidate();
  const warmLease = cache.acquirePixivImageSource("same-key", async () => new Uint8Array([3]));
  assert.equal(warmLease.source, newSource);

  oldLease.release();
  newLease.release();
  warmLease.release();
  cache.clearPixivImageMemoryCache();
});

test("profile media snapshots are scoped by account and explicitly cleared", () => {
  profileMedia.clearProfileMediaSnapshots();
  const insecure = profileMedia.profileMediaSnapshotKey("account-a", "compatible");
  const standard = profileMedia.profileMediaSnapshotKey("account-a", "standard");
  const ech = profileMedia.profileMediaSnapshotKey("account-a", "ech");
  assert.notEqual(insecure, standard);
  assert.equal(standard, ech);
  profileMedia.writeProfileMediaSnapshot(insecure, {
    avatarUrl: "https://i.pximg.net/avatar-a.jpg",
    backgroundImageUrl: "https://i.pximg.net/background-a.jpg",
  });
  assert.equal(profileMedia.readProfileMediaSnapshot(insecure)?.backgroundImageUrl, "https://i.pximg.net/background-a.jpg");
  assert.equal(profileMedia.readProfileMediaSnapshot(standard), null);
  profileMedia.clearProfileMediaSnapshots();
  assert.equal(profileMedia.readProfileMediaSnapshot(insecure), null);
});

test("PixivImage and profile use warm media immediately while refreshing in the background", async () => {
  const [pixivImage, profile, session, api] = await Promise.all([
    readFile(new URL("../src/lib/components/PixivImage.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/routes/profile/+page.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/session.ts", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/pixiv-api.ts", import.meta.url), "utf8"),
  ]);
  assert.match(pixivImage, /acquirePixivImageSource/);
  assert.match(pixivImage, /lease\.source/);
  assert.doesNotMatch(pixivImage, /URL\.revokeObjectURL/);
  assert.match(pixivImage, /if \(rendered !== image\) return;[\s\S]*onstatus\?\.\("error"\)/);
  assert.match(profile, /readProfileMediaSnapshot/);
  assert.match(profile, /writeProfileMediaSnapshot/);
  assert.match(profile, /requestedProfileKey !== profileKey/);
  assert.match(profile, /onDestroy/);
  assert.match(profile, /backgroundImageUrl/);
  assert.match(session, /clearPixivImageMemoryCache/);
  assert.match(session, /clearProfileMediaSnapshots/);
  assert.match(api, /clear_media_cache[\s\S]*clearPixivImageMemoryCache/);
});

test("detail-page offline reuse falls back to PixivImage without a new cache interface", async () => {
  const offlineImage = await readFile(
    new URL("../src/lib/components/OfflineImage.svelte", import.meta.url),
    "utf8",
  );
  assert.match(offlineImage, /readOfflineAsset/);
  assert.match(offlineImage, /fallbackUrl/);
  assert.match(offlineImage, /<PixivImage url=\{fallbackUrl\}/);
  assert.doesNotMatch(offlineImage, /fetch_pixiv_thumbnail/);
});
