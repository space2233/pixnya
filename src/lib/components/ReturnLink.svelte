<script lang="ts">
  import { returnToPreviousLocation } from "$lib/return-navigation-browser";
  import { m } from "$lib/i18n";

  let {
    fallback,
    label = m.common_back(),
    variant = "plain",
  }: {
    fallback: string;
    label?: string;
    variant?: "plain" | "capsule";
  } = $props();
</script>

<button class="return-link" class:capsule={variant === "capsule"} type="button" onclick={() => returnToPreviousLocation(fallback)}>
  <span aria-hidden="true">‹</span>{label}
</button>

<style>
  .return-link {
    display: inline-flex;
    gap: 5px;
    align-items: center;
    padding: 4px 0;
    color: var(--muted);
    border: 0;
    background: transparent;
    cursor: pointer;
    font: inherit;
    font-size: var(--type-body);
    text-align: left;
  }
  .return-link.capsule {
    padding: 6px 14px 6px 12px;
    color: #697278;
    border-radius: 999px;
    background: #eef1f3;
    font-weight: 700;
    white-space: nowrap;
  }
  .return-link:hover { color: var(--pixiv-blue); }
  .return-link.capsule:hover { background: #e4edf3; }
  .return-link:focus-visible { outline: 2px solid color-mix(in srgb, var(--pixiv-blue) 45%, transparent); outline-offset: 3px; }
  span { font-size: var(--type-label); line-height: .8; }
</style>
