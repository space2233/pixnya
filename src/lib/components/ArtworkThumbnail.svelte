<script lang="ts">
  import PixivImage from "$lib/components/PixivImage.svelte";

  let {
    url,
    alt,
    tone = 1,
  }: {
    url?: string | null;
    alt: string;
    tone?: number;
  } = $props();

  let ready = $state(false);
  let failed = $state(false);

  function updateStatus(status: "loading" | "ready" | "error") {
    ready = status === "ready";
    failed = status === "error";
  }
</script>

<div class="thumbnail tone-{((tone - 1) % 6) + 1}" class:failed>
  <PixivImage {url} {alt} onstatus={updateStatus} />
  {#if failed}
    <span class="fallback" aria-hidden="true">p</span>
  {:else if !ready}
    <span class="loader" aria-label="正在载入缩略图"></span>
  {/if}
</div>

<style>
  .thumbnail {
    position: absolute;
    inset: 0;
    display: grid;
    overflow: hidden;
    place-items: center;
    background: #eaf1f5;
  }

  .loader {
    width: 28%;
    aspect-ratio: 1;
    border: 3px solid rgba(255, 255, 255, 0.65);
    border-top-color: var(--pixiv-blue);
    border-radius: 50%;
    animation: spin 0.85s linear infinite;
  }

  .fallback {
    display: grid;
    width: 34%;
    aspect-ratio: 1;
    place-items: center;
    color: rgba(255, 255, 255, 0.9);
    border-radius: 24%;
    background: rgba(0, 150, 250, 0.56);
    font-size: clamp(18px, 4vw, 34px);
    font-weight: 700;
  }

  .tone-1 { background: linear-gradient(145deg, #e1f3fb, #cbe1ee); }
  .tone-2 { background: linear-gradient(145deg, #f4e6ef, #e7ccdc); }
  .tone-3 { background: linear-gradient(145deg, #f4f0dc, #dfd4ae); }
  .tone-4 { background: linear-gradient(145deg, #e7e4f5, #d1caeb); }
  .tone-5 { background: linear-gradient(145deg, #e3f1e9, #c9e1d3); }
  .tone-6 { background: linear-gradient(145deg, #f5e8df, #e9cdbc); }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @media (prefers-reduced-motion: reduce) {
    .loader { animation: none; }
  }
</style>
