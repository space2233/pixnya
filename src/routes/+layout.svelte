<script lang="ts">
  import { afterNavigate } from "$app/navigation";
  import { onMount } from "svelte";
  import "../app.css";
  import { initializeI18n } from "$lib/i18n";
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
    initializeI18n();
    applyReducedMotionPreference();
    document.addEventListener("click", captureReturnNavigation, true);
    restorePendingReturnPosition();
    return () => document.removeEventListener("click", captureReturnNavigation, true);
  });
</script>

{@render children()}
