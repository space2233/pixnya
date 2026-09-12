<script lang="ts">
  import { returnToPreviousLocation } from "$lib/return-navigation-browser";
  import { m } from "$lib/i18n";

  let {
    fallback,
    label = m.common_back(),
    compact = false,
  }: {
    fallback: string;
    label?: string;
    compact?: boolean;
  } = $props();
</script>

<button
  class="return-link"
  class:compact
  type="button"
  aria-label={compact ? label : undefined}
  onclick={() => returnToPreviousLocation(fallback)}
>
  <span aria-hidden="true">‹</span>{#if !compact}{label}{/if}
</button>

<style>
  .return-link {
    display: inline-flex;
    max-width: 100%;
    box-sizing: border-box;
    flex-shrink: 0;
    gap: 5px;
    align-items: center;
    padding: 6px 14px 6px 12px;
    color: #697278;
    border: 0;
    border-radius: 999px;
    background: #eef1f3;
    cursor: pointer;
    font: inherit;
    font-size: var(--type-body);
    font-weight: 700;
    text-align: left;
    white-space: nowrap;
    word-break: keep-all;
  }
  .return-link:hover {
    color: var(--pixiv-blue);
    background: #e4edf3;
  }
  .return-link:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--pixiv-blue) 45%, transparent);
    outline-offset: 3px;
  }
  .return-link.compact {
    width: 40px;
    height: 40px;
    justify-content: center;
    padding: 0;
    color: inherit;
    background: transparent;
    font-weight: 400;
  }
  .return-link.compact:hover { color: inherit; background: rgba(255, 255, 255, .1); }
  span {
    flex: 0 0 auto;
    font-size: var(--type-label);
    line-height: .8;
  }
</style>
