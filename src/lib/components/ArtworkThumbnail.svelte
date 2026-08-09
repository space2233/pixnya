<script lang="ts">
  import PixivImage from "$lib/components/PixivImage.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/i18n";

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
    <span class="quiet-failure" role="img" aria-label={m.thumbnail_unavailable()}>
      <Icon name="image" size={34} />
      <small>{m.thumbnail_unavailable()}</small>
    </span>
  {:else if !ready}
    <span class="skeleton-art" role="status" aria-label={m.thumbnail_loading()}>
      <i></i><i></i><i></i>
    </span>
  {/if}
</div>

<style>
  .thumbnail {
    position: absolute;
    inset: 0;
    display: grid;
    overflow: hidden;
    place-items: center;
    background: #f1f3f5;
  }

  .skeleton-art {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: linear-gradient(135deg, #f1f3f5 52%, #eceff1 52%);
  }

  .skeleton-art::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      105deg,
      transparent 35%,
      rgba(255, 255, 255, 0.72) 48%,
      transparent 61%
    );
    animation: sweep 1.5s infinite;
    content: "";
  }

  .skeleton-art i {
    position: absolute;
    border-radius: 999px;
    background: rgba(214, 219, 223, 0.72);
  }

  .skeleton-art i:nth-child(1) {
    bottom: 17%;
    left: 12%;
    width: 52%;
    height: 7%;
  }

  .skeleton-art i:nth-child(2) {
    bottom: 9%;
    left: 12%;
    width: 34%;
    height: 5%;
  }

  .skeleton-art i:nth-child(3) {
    top: 14%;
    right: 14%;
    width: 18%;
    aspect-ratio: 1;
  }

  .thumbnail.failed { background: #f7f7f7; }

  .quiet-failure {
    display: grid;
    width: 46%;
    gap: 10px;
    place-items: center;
    color: #aeb4b8;
    text-align: center;
  }

  .quiet-failure small {
    color: #969da2;
    font-size: 8px;
  }

  @keyframes sweep {
    from { transform: translateX(-80%); }
    to { transform: translateX(80%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .skeleton-art::after { animation: none; }
  }
</style>
