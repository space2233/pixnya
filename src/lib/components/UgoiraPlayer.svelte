<script lang="ts">
  import { onDestroy } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { m } from "$lib/i18n";
  import { describeDataFailure, prepareUgoira, readOfflineAsset } from "$lib/pixiv-api";
  import type { PreparedUgoiraFrame } from "$lib/types";

  let { illustrationId, previewUrl, title }: { illustrationId: string; previewUrl?: string | null; title: string } = $props();
  let status = $state<"idle" | "preparing" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let sources = $state<string[]>([]);
  let frames = $state<PreparedUgoiraFrame[]>([]);
  let frameIndex = $state(0);
  let loadedCount = $state(0);
  let playing = $state(false);

  $effect(() => {
    if (!playing || status !== "ready" || !frames.length || !sources.length) return;
    const delay = Math.max(16, frames[frameIndex]?.delayMs ?? 100);
    const timer = window.setTimeout(() => {
      frameIndex = (frameIndex + 1) % sources.length;
    }, delay);
    return () => window.clearTimeout(timer);
  });

  onDestroy(revokeSources);

  function revokeSources() {
    for (const source of sources) URL.revokeObjectURL(source);
    sources = [];
  }

  function mimeFor(name: string): string {
    const extension = name.split(".").pop()?.toLowerCase();
    if (extension === "png") return "image/png";
    if (extension === "gif") return "image/gif";
    if (extension === "webp") return "image/webp";
    return "image/jpeg";
  }

  async function loadAnimation() {
    if (status === "preparing" || status === "loading") return;
    revokeSources();
    frameIndex = 0;
    loadedCount = 0;
    playing = false;
    status = "preparing";
    errorMessage = "";
    const nextSources: string[] = [];
    try {
      const prepared = await prepareUgoira(illustrationId);
      frames = prepared.frames;
      status = "loading";
      for (const frame of prepared.frames) {
        const buffer = await readOfflineAsset(prepared.entry.key, frame.assetName);
        const bytes = new Uint8Array(buffer);
        if (!bytes.byteLength) throw new Error("empty ugoira frame");
        nextSources.push(URL.createObjectURL(new Blob([bytes], { type: mimeFor(frame.assetName) })));
        loadedCount += 1;
      }
      sources = nextSources;
      status = "ready";
      playing = true;
    } catch (error) {
      for (const source of nextSources) URL.revokeObjectURL(source);
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }
</script>

<div class="ugoira-player">
  {#if status === "ready" && sources[frameIndex]}
    <img src={sources[frameIndex]} alt={m.ugoira_frame_alt({ title, frame: frameIndex + 1 })} draggable="false" />
    <div class="player-bar">
      <button type="button" onclick={() => (playing = !playing)}>{playing ? m.ugoira_pause() : m.ugoira_play()}</button>
      <span>{frameIndex + 1} / {sources.length}</span>
      <span>{m.ugoira_saved_offline()}</span>
    </div>
  {:else}
    <PixivImage url={previewUrl} alt={m.ugoira_preview_alt({ title })} fit="contain" cacheKind="preview" />
    <div class="load-overlay">
      {#if status === "preparing"}<span class="spinner"></span><strong>{m.ugoira_preparing()}</strong>
      {:else if status === "loading"}<span class="spinner"></span><strong>{m.ugoira_loading_frame({ loaded: loadedCount, total: frames.length })}</strong>
      {:else}<Icon name="image" size={34} /><strong>{status === "error" ? m.ugoira_load_failed() : m.ugoira_load()}</strong>{/if}
      {#if errorMessage}<p role="alert">{errorMessage}</p>{/if}
      {#if status === "idle" || status === "error"}<button type="button" onclick={loadAnimation}>{status === "error" ? m.common_retry() : m.ugoira_download_play()}</button>{/if}
    </div>
  {/if}
</div>

<style>
  .ugoira-player { position: relative; display: grid; min-height: 420px; overflow: hidden; place-items: center; border-radius: 10px; background: #f2f3f4; }
  .ugoira-player > :global(img) { width: 100%; height: 100%; max-height: 82vh; object-fit: contain; }
  .load-overlay { position: absolute; z-index: 2; display: grid; gap: 10px; place-items: center; padding: 20px; color: #495157; border-radius: 12px; background: rgba(255,255,255,.9); text-align: center; }
  .load-overlay strong { font-size: var(--type-small); } .load-overlay p { max-width: 320px; margin: 0; color: #a14f5c; font-size: var(--type-caption); }
  .load-overlay button, .player-bar button { padding: 9px 17px; color: white; border: 0; border-radius: 18px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .spinner { width: 28px; height: 28px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .player-bar { position: absolute; z-index: 3; right: 10px; bottom: 10px; left: 10px; display: flex; gap: 10px; align-items: center; padding: 8px 10px; color: white; border-radius: 8px; background: rgba(25,28,31,.72); font-size: var(--type-caption); }
  .player-bar span:last-child { margin-left: auto; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 620px) { .ugoira-player { min-height: 300px; } .player-bar span:last-child { display: none; } }
</style>
