<script lang="ts">
  import { onDestroy } from "svelte";
  import { m } from "$lib/i18n";
  import { readOfflineAsset } from "$lib/pixiv-api";
  import type { UgoiraMetadata } from "$lib/types";

  let { entryKey, metadata, title }: { entryKey: string; metadata: UgoiraMetadata; title: string } = $props();
  let sources = $state<string[]>([]);
  let frameIndex = $state(0);
  let loaded = $state(0);
  let loading = $state(false);
  let playing = $state(false);
  let errorMessage = $state("");

  $effect(() => {
    if (!playing || !sources.length) return;
    const timer = window.setTimeout(() => frameIndex = (frameIndex + 1) % sources.length, Math.max(16, metadata.frames[frameIndex]?.delayMs ?? 100));
    return () => window.clearTimeout(timer);
  });

  onDestroy(() => { for (const source of sources) URL.revokeObjectURL(source); });

  function assetName(index: number, fileName: string): string {
    const extension = fileName.split(".").pop()?.toLowerCase();
    const safeExtension = ["jpg", "jpeg", "png", "gif", "webp"].includes(extension ?? "") ? extension : "jpg";
    return `frame-${String(index).padStart(6, "0")}.${safeExtension}`;
  }

  async function load() {
    if (loading) return;
    loading = true; errorMessage = ""; loaded = 0;
    const next: string[] = [];
    try {
      for (const [index, frame] of metadata.frames.entries()) {
        const buffer = await readOfflineAsset(entryKey, assetName(index, frame.fileName));
        next.push(URL.createObjectURL(new Blob([new Uint8Array(buffer)])));
        loaded += 1;
      }
      sources = next; frameIndex = 0; playing = true;
    } catch {
      for (const source of next) URL.revokeObjectURL(source);
      errorMessage = m.ugoira_offline_error();
    }
    finally { loading = false; }
  }
</script>

<div class="player">
  {#if sources[frameIndex]}<img src={sources[frameIndex]} alt={m.ugoira_frame_alt({ title, frame: frameIndex + 1 })} />{:else}<div class="empty"><strong>{loading ? m.ugoira_loading_frame({ loaded, total: metadata.frames.length }) : m.ugoira_offline_title()}</strong>{#if errorMessage}<p>{errorMessage}</p>{/if}<button type="button" disabled={loading} onclick={load}>{errorMessage ? m.common_retry() : m.ugoira_play()}</button></div>{/if}
  {#if sources.length}<div class="controls"><button type="button" onclick={() => playing = !playing}>{playing ? m.ugoira_pause() : m.ugoira_play()}</button><span>{frameIndex + 1} / {sources.length}</span></div>{/if}
</div>

<style>
  .player { position: relative; display: grid; min-height: 420px; overflow: hidden; place-items: center; border-radius: 10px; background: #edf1f4; }
  img { display: block; max-width: 100%; max-height: 82vh; object-fit: contain; }
  .empty { display: grid; gap: 11px; place-items: center; color: var(--muted); text-align: center; } .empty p { margin: 0; color: #a34e5d; font-size: var(--type-caption); }
  button { padding: 9px 17px; color: white; border: 0; border-radius: 18px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .controls { position: absolute; right: 10px; bottom: 10px; left: 10px; display: flex; gap: 10px; align-items: center; padding: 8px 10px; color: white; border-radius: 8px; background: rgba(25,28,31,.72); font-size: var(--type-caption); }
</style>
