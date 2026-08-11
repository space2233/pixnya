<script lang="ts">
  import PixivImage from "$lib/components/PixivImage.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ThumbnailSkeleton from "$lib/components/ThumbnailSkeleton.svelte";
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
    <ThumbnailSkeleton />
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

</style>
