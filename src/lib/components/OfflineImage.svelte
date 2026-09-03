<script lang="ts">
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { m } from "$lib/i18n";
  import { readOfflineAsset } from "$lib/pixiv-api";
  import type { MediaCacheKind } from "$lib/types";

  let {
    entryKey,
    assetNames,
    alt = "",
    fit = "contain",
    fallbackUrl,
    fallbackCacheKind,
  }: {
    entryKey: string;
    assetNames: string[];
    alt?: string;
    fit?: "cover" | "contain";
    fallbackUrl?: string | null;
    fallbackCacheKind?: MediaCacheKind | null;
  } = $props();
  let source = $state<string | null>(null);
  let useFallback = $state(false);

  $effect(() => {
    const key = entryKey;
    const names = [...assetNames];
    const remote = fallbackUrl;
    let disposed = false;
    let objectUrl: string | null = null;
    source = null;
    useFallback = false;
    void (async () => {
      for (const name of names) {
        try {
          const buffer = await readOfflineAsset(key, name);
          const bytes = new Uint8Array(buffer);
          if (!bytes.byteLength) continue;
          objectUrl = URL.createObjectURL(new Blob([bytes]));
          if (disposed) URL.revokeObjectURL(objectUrl);
          else source = objectUrl;
          return;
        } catch {
          // Try the next explicitly supplied filename.
        }
      }
      if (!disposed && remote) useFallback = true;
    })();
    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  });
</script>

{#if source}
  <img src={source} {alt} draggable="false" style:object-fit={fit} />
{:else if useFallback && fallbackUrl}
  <PixivImage url={fallbackUrl} {alt} {fit} cacheKind={fallbackCacheKind} />
{:else if !fallbackUrl}
  <span class="placeholder" aria-label={alt}>{m.offline_image()}</span>
{/if}

<style>
  img { display: block; width: 100%; height: 100%; }
  .placeholder { display: grid; width: 100%; height: 100%; min-height: 100px; place-items: center; color: #9ba2a7; background: #edf1f4; font-size: var(--type-caption); }
</style>
