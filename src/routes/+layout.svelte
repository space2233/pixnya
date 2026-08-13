<script lang="ts">
  import { afterNavigate, goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import "../app.css";
  import { initializeI18n } from "$lib/i18n";
  import { applyReducedMotionPreference } from "$lib/preferences";
  import { readPreferredConnectionMode } from "$lib/preferences";
  import { connectionSetupUrl } from "$lib/connection-onboarding";
  import { acknowledgeLocalBackupFrontendRecovery, getLocalBackupFrontendRecovery } from "$lib/pixiv-api";
  import { restoreFrontendBackupState } from "$lib/local-backup";
  import {
    captureReturnNavigation,
    restorePendingReturnPosition,
    restoreReturnAfterHistoryPop,
  } from "$lib/return-navigation-browser";

  let { children } = $props();
  let routeReady = $state(false);

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
    void invoke("mark_frontend_ready").catch(() => {});
    document.addEventListener("click", captureReturnNavigation, true);
    restorePendingReturnPosition();
    let active = true;
    void (async () => {
      try {
        const recovery = await getLocalBackupFrontendRecovery();
        if (recovery) {
          restoreFrontendBackupState(recovery.frontend);
          applyReducedMotionPreference();
          await acknowledgeLocalBackupFrontendRecovery(recovery.transactionId);
        }
      } catch {
        // Keep the recovery record for a later retry, but never hold the
        // entire application behind an unavailable browser storage backend.
      }
      if (readPreferredConnectionMode() === null && page.url.pathname !== "/setup/connection") {
        const target = `${page.url.pathname}${page.url.search}${page.url.hash}`;
        await goto(connectionSetupUrl(target), { replaceState: true });
      }
      if (active) routeReady = true;
    })();
    return () => {
      active = false;
      document.removeEventListener("click", captureReturnNavigation, true);
    };
  });
</script>

{#if routeReady}{@render children()}{/if}
