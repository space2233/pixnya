<script lang="ts">
  import { onMount } from "svelte";
  import { m } from "$lib/i18n";
  import { addNovelToShelf, readReadingShelf, READING_SHELF_CHANGED_EVENT } from "$lib/reading-shelf";
  import type { NovelSummary } from "$lib/types";
  let { novel }: { novel: NovelSummary } = $props();
  let ids = $state<string[]>([]);
  let error = $state("");
  onMount(() => {
    const refresh = () => { ids = readReadingShelf().map((entry) => entry.id); };
    refresh(); window.addEventListener(READING_SHELF_CHANGED_EVENT, refresh);
    return () => window.removeEventListener(READING_SHELF_CHANGED_EVENT, refresh);
  });
  function add() { try { addNovelToShelf(novel); error = ""; } catch { error = m.reading_save_failed(); } }
</script>
{#if ids.includes(novel.id)}<a href="/novels/shelf">{m.reading_on_shelf()}</a>
{:else}<button type="button" onclick={add}>{m.reading_add_planned()}</button>{/if}
{#if error}<span role="alert">{error}</span>{/if}
<style>
  a, button { display: inline-flex; align-items: center; justify-content: center; min-height: 42px; padding: 0 16px; border: 1px solid var(--line); border-radius: 22px; background: var(--surface); color: var(--pixiv-blue); text-decoration: none; font-size: var(--type-body); }
  span { color: var(--danger); font-size: var(--type-small); }
</style>
