<script lang="ts">
  import { afterNavigate } from "$app/navigation";
  import { onMount } from "svelte";
  import "../app.css";
  import { applyReducedMotionPreference } from "$lib/preferences";
  import {
    captureReturnNavigation,
    restorePendingReturnPosition,
    restoreReturnAfterHistoryPop,
  } from "$lib/return-navigation-browser";

  let { children } = $props();

  afterNavigate((navigation) => {
    const restored = restorePendingReturnPosition();
    if (!restored && navigation.type === "popstate" && (navigation.delta ?? 0) < 0) {
      const previous = navigation.from?.url;
      if (previous) {
        restoreReturnAfterHistoryPop(`${previous.pathname}${previous.search}${previous.hash}`);
      }
    }
  });

  onMount(() => {
    applyReducedMotionPreference();
    document.addEventListener("click", captureReturnNavigation, true);
    restorePendingReturnPosition();
    return () => document.removeEventListener("click", captureReturnNavigation, true);
  });
</script>

{@render children()}
