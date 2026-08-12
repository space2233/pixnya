<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import ConnectionModePicker from "$lib/components/ConnectionModePicker.svelte";
  import { safeConnectionReturnTarget } from "$lib/connection-onboarding";
  import { m } from "$lib/i18n";
  import { writePreferredConnectionMode } from "$lib/preferences";
  import { initializeSession, switchSessionConnectionMode } from "$lib/session";
  import type { ConnectionMode, ConnectionProbe, SessionSnapshot } from "$lib/types";

  let selected = $state<ConnectionMode | null>(null);
  let probeState = $state<"idle" | "checking" | "available" | "failed">("idle");
  let probeMessage = $state("");
  let probeSequence = 0;
  let currentSession = $state<SessionSnapshot>({ loggedIn: false });
  let isContinuing = $state(false);

  onMount(() => {
    void initializeSession().then((snapshot) => (currentSession = snapshot)).catch(() => {});
  });

  async function chooseMode(mode: ConnectionMode) {
    selected = mode;
    const sequence = ++probeSequence;
    probeState = "checking";
    probeMessage = "";
    try {
      const report = await invoke<ConnectionProbe>("probe_connection", {
        mode,
        traffic: "api",
        host: "app-api.pixiv.net",
        unsafeAcknowledged: true,
      });
      if (sequence !== probeSequence || selected !== mode) return;
      probeState = "available";
      probeMessage = `${report.latencyMs} ms`;
    } catch {
      if (sequence !== probeSequence || selected !== mode) return;
      probeState = "failed";
    }
  }

  async function completeSetup() {
    if (!selected || isContinuing || probeState === "checking") return;
    isContinuing = true;
    try {
      currentSession = await initializeSession().catch(() => currentSession);
      let modeToPersist = selected;
      if (
        probeState === "available" &&
        currentSession.loggedIn &&
        currentSession.connectionMode !== selected
      ) {
        currentSession = await switchSessionConnectionMode(selected);
      } else if (currentSession.loggedIn && probeState !== "available") {
        modeToPersist = currentSession.connectionMode ?? selected;
      }
      writePreferredConnectionMode(modeToPersist);
      await goto(safeConnectionReturnTarget(page.url.searchParams.get("returnTo")), {
        replaceState: true,
      });
    } catch {
      probeState = "failed";
    } finally {
      isContinuing = false;
    }
  }
</script>

<svelte:head><title>{m.connection_setup_title()} · PixNya</title></svelte:head>

<main class="setup-page">
  <section class="setup-card">
    <div class="brand">PixNya</div>
    <h1>{m.connection_setup_title()}</h1>
    <ConnectionModePicker
      {selected}
      state={probeState}
      message={probeMessage}
      disabled={isContinuing}
      onselect={chooseMode}
    />
    <button
      class="continue"
      type="button"
      disabled={!selected || probeState === "checking" || isContinuing}
      onclick={completeSetup}
    >{isContinuing ? m.common_processing() : m.connection_setup_continue()}</button>
  </section>
</main>

<style>
  :global(body) { background: #f7f8fa; }
  .setup-page {
    min-height: 100dvh;
    box-sizing: border-box;
    display: grid;
    place-items: center;
    padding: max(28px, env(safe-area-inset-top)) max(18px, env(safe-area-inset-right)) max(28px, env(safe-area-inset-bottom)) max(18px, env(safe-area-inset-left));
  }
  .setup-card {
    width: min(460px, 100%);
    box-sizing: border-box;
    padding: 34px;
    border: 1px solid var(--line);
    border-radius: 24px;
    background: white;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.06);
  }
  .brand { color: var(--brand); font-size: 25px; font-weight: 800; }
  h1 { margin: 24px 0; font-size: 27px; }
  .continue {
    width: 100%;
    min-height: 48px;
    margin-top: 22px;
    border: 0;
    border-radius: 24px;
    background: var(--brand);
    color: white;
    font-weight: 700;
    cursor: pointer;
  }
  .continue:disabled { opacity: 0.45; cursor: default; }
  @media (max-width: 600px) {
    .setup-page { place-items: stretch; background: white; }
    .setup-card { align-self: center; padding: 18px 4px; border: 0; box-shadow: none; }
  }
</style>
