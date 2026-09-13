<script lang="ts">
  import { m } from "$lib/i18n";
  import { loadMoreObserverEnabled, observeLoadMore } from "$lib/infinite-scroll";

  let {
    enabled = false,
    loading = false,
    error = "",
    onload,
    loadingLabel,
    retryLabel,
  }: {
    enabled?: boolean;
    loading?: boolean;
    error?: string;
    onload: () => void;
    loadingLabel?: string;
    retryLabel?: string;
  } = $props();

  const canObserve = $derived(loadMoreObserverEnabled({ enabled, loading, error }));
  const visible = $derived(enabled || loading || Boolean(error));
</script>

{#if visible}
  <div class="load-more-on-scroll" aria-busy={loading ? "true" : undefined}>
    {#if error}
      <p role="alert">{error}</p>
      <button type="button" onclick={onload}>{retryLabel ?? m.common_retry()}</button>
    {:else}
      <div
        class="load-more-sentinel"
        use:observeLoadMore={{ onLoad: onload, enabled: canObserve }}
        aria-hidden="true"
      ></div>
      {#if loading}
        <p role="status">{loadingLabel ?? m.common_loading()}</p>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .load-more-on-scroll {
    display: grid;
    gap: 9px;
    justify-items: center;
    margin-top: 26px;
  }

  .load-more-sentinel {
    width: 100%;
    height: 1px;
  }

  p {
    margin: 0;
    color: var(--muted);
    font-size: var(--type-caption);
  }

  p[role="alert"] {
    color: #a05a63;
  }

  button {
    min-width: 122px;
    height: 36px;
    color: #555f66;
    border: 1px solid var(--line);
    border-radius: 18px;
    background: white;
    cursor: pointer;
    font-size: var(--type-body);
    font-weight: 700;
  }

  button:hover {
    color: var(--pixiv-blue);
    border-color: #b8def7;
  }
</style>
