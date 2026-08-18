<script lang="ts">
  import { m } from "$lib/i18n";
  import { readOfflineAsset } from "$lib/pixiv-api";

  let { entryKey, assetNames, alt = "", fit = "contain" }: { entryKey: string; assetNames: string[]; alt?: string; fit?: "cover" | "contain" } = $props();
  let source = $state<string | null>(null);

  $effect(() => {
    const key = entryKey;
    const names = [...assetNames];
    let disposed = false;
    let objectUrl: string | null = null;
    source = null;
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
    })();
    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  });
</script>

  {#if source}<img src={source} {alt} draggable="false" style:object-fit={fit} />{:else}<span class="placeholder" aria-label={alt}>{m.offline_image()}</span>{/if}

<style>
  img { display: block; width: 100%; height: 100%; }
  .placeholder { display: grid; width: 100%; height: 100%; min-height: 100px; place-items: center; color: #9ba2a7; background: #edf1f4; font-size: var(--type-caption); }
</style>
