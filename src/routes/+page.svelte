<script lang="ts">
  import BrowsePage, { type BrowsePageSnapshot } from "$lib/components/BrowsePage.svelte";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";

  let browsePage = $state<BrowsePage>();
  export const snapshot = {
    capture: () => browsePage ? rememberNavigationView(browsePage.captureSnapshot()) : null,
    restore: (key: unknown) => {
      const value = recallNavigationView<BrowsePageSnapshot>(key);
      if (value && browsePage) browsePage.restoreSnapshot(value);
    },
  };
</script>

<BrowsePage section="home" bind:this={browsePage} />
