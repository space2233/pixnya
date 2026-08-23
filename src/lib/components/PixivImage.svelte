<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    acquirePixivImageSource,
  } from "$lib/pixiv-image-memory-cache";
  import { session } from "$lib/session";
  import type { MediaCacheKind } from "$lib/types";

  type LoadStatus = "loading" | "ready" | "error";

  let {
    url,
    alt = "",
    fit = "cover",
    cacheKind = "thumbnail",
    onstatus,
  }: {
    url?: string | null;
    alt?: string;
    fit?: "cover" | "contain";
    cacheKind?: MediaCacheKind | null;
    onstatus?: (status: LoadStatus) => void;
  } = $props();

  let rendered = $state<{ source: string; invalidate: () => void } | null>(null);
  $effect(() => {
    const requestedUrl = url;
    const account = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "logged-out";
    const securityScope = $session.connectionMode === "compatible" ? "insecure" : "verified";
    const cacheKey = `${account}:${securityScope}:${cacheKind ?? "transient"}:${requestedUrl ?? ""}`;
    rendered = null;
    if (!requestedUrl) {
      onstatus?.("error");
      return;
    }

    let disposed = false;
    const lease = acquirePixivImageSource(cacheKey, async () => {
      const buffer = await invoke<ArrayBuffer>("fetch_pixiv_thumbnail", {
        url: requestedUrl,
        cacheKind,
      });
      return new Uint8Array(buffer);
    });
    if (lease.source) {
      rendered = { source: lease.source, invalidate: lease.invalidate };
      onstatus?.("ready");
    } else {
      onstatus?.("loading");
    }
    void lease.ready
      .then((nextSource) => {
        if (disposed) return;
        rendered = { source: nextSource, invalidate: lease.invalidate };
        onstatus?.("ready");
      })
      .catch(() => {
        if (!disposed) onstatus?.("error");
      });

    return () => {
      disposed = true;
      lease.release();
    };
  });

  function handleImageError(image: { source: string; invalidate: () => void }) {
    image.invalidate();
    if (rendered !== image) return;
    rendered = null;
    onstatus?.("error");
  }
</script>

{#if rendered}
  {@const image = rendered}
  {#key image.source}
    <img
    src={image.source}
    {alt}
    draggable="false"
    style:object-fit={fit}
    onerror={() => handleImageError(image)}
  />
  {/key}
{/if}

<style>
  img {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
