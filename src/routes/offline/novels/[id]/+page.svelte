<script lang="ts">
  import { page } from "$app/state";
  import NovelReadingPage from "$lib/components/NovelReadingPage.svelte";
</script>

<NovelReadingPage novelId={page.params.id ?? ""} offlineOnly />
