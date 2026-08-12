<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
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
    cacheKind?: MediaCacheKind;
    onstatus?: (status: LoadStatus) => void;
  } = $props();

  let source = $state<string | null>(null);
  $effect(() => {
    const requestedUrl = url;
    source = null;
    if (!requestedUrl) {
      onstatus?.("error");
      return;
    }

    onstatus?.("loading");
    let disposed = false;
    let objectUrl: string | null = null;
    void invoke<ArrayBuffer>("fetch_pixiv_thumbnail", {
      url: requestedUrl,
      cacheKind,
    })
      .then((buffer) => {
        const bytes = new Uint8Array(buffer);
        if (bytes.byteLength === 0) throw new Error("empty Pixiv media response");
        objectUrl = URL.createObjectURL(new Blob([bytes]));
        if (disposed) {
          URL.revokeObjectURL(objectUrl);
          objectUrl = null;
          return;
        }
        source = objectUrl;
        onstatus?.("ready");
      })
      .catch(() => {
        if (!disposed) onstatus?.("error");
      });

    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  });
</script>

{#if source}
  <img
    src={source}
    {alt}
    draggable="false"
    style:object-fit={fit}
    onerror={() => onstatus?.("error")}
  />
{/if}

<style>
  img {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
