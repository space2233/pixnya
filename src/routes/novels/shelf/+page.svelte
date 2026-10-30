<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import ReadingShelf from "$lib/components/ReadingShelf.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
</script>
<svelte:head><title>{m.reading_shelf_title()} · PixNya</title></svelte:head>
<AppShell title={m.reading_shelf_title()}>
  <main><ReturnLink fallback="/novels" label={m.common_back()} /><ReadingShelf /></main>
</AppShell>
<style>main { box-sizing: border-box; max-width: 1100px; margin: auto; padding: 24px 20px 80px; }</style>
