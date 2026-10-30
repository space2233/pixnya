<script lang="ts">
  import { page } from "$app/state";
  import NovelReadingPage, { type ReadingSnapshot } from "$lib/components/NovelReadingPage.svelte";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  let reader = $state<NovelReadingPage>();
  export const snapshot = {
    capture: () => reader ? rememberNavigationView(reader.captureSnapshot()) : null,
    restore: (key: unknown) => { reader?.restoreSnapshot(recallNavigationView<ReadingSnapshot | null>(key)); },
  };
</script>

<NovelReadingPage novelId={page.params.id ?? ""} bind:this={reader} />
