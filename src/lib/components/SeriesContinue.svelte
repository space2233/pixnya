<script lang="ts">
  import { onMount } from "svelte";
  import { m } from "$lib/i18n";
  import { readReadingShelf, lastReadInSeries, READING_SHELF_CHANGED_EVENT, type ReadingShelfEntry } from "$lib/reading-shelf";
  let { seriesId }: { seriesId: string } = $props();
  let entries = $state<ReadingShelfEntry[]>([]);
  let last = $derived(lastReadInSeries(entries, seriesId));
  onMount(() => {
    const refresh = () => { entries = readReadingShelf(); };
    refresh(); window.addEventListener(READING_SHELF_CHANGED_EVENT, refresh);
    return () => window.removeEventListener(READING_SHELF_CHANGED_EVENT, refresh);
  });
</script>
{#if last}<a href={`/novels/${last.id}/read`}>{m.reading_continue()}: {last.title}</a>{/if}
<style>a { display: block; margin-top: 12px; color: var(--pixiv-blue); font-size: var(--type-body); text-decoration: none; overflow-wrap: anywhere; }</style>
