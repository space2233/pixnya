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
    <header class="setup-header">
      <div class="brand-mark">
        <strong>PixNya</strong>
      </div>
      <h1>{m.connection_setup_title()}</h1>
    </header>
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
  :global(body) { background: var(--soft-surface); }
  .setup-page {
    min-height: 100dvh;
    box-sizing: border-box;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    place-items: center;
    padding: max(28px, env(safe-area-inset-top)) max(18px, env(safe-area-inset-right)) max(28px, env(safe-area-inset-bottom)) max(18px, env(safe-area-inset-left));
  }
  .setup-card {
    width: min(430px, 100%);
    min-width: 0;
    box-sizing: border-box;
    justify-self: center;
    padding: 30px;
    border: 1px solid var(--line);
    border-radius: 18px;
    background: white;
    box-shadow: 0 14px 40px rgba(33, 61, 84, 0.08);
  }
  .setup-header { margin-bottom: 24px; }
  .brand-mark {
    display: flex;
    align-items: center;
    color: var(--pixiv-blue);
  }
  .brand-mark strong { font-size: 22px; font-weight: 800; letter-spacing: -0.025em; }
  h1 { margin: 22px 0 0; color: var(--text); font-size: 24px; line-height: 1.25; }
  .continue {
    width: 100%;
    min-height: 48px;
    margin-top: 22px;
    border: 0;
    border-radius: 24px;
    background: var(--pixiv-blue);
    color: white;
    font-weight: 700;
    cursor: pointer;
    transition: background 150ms ease, transform 150ms ease, opacity 150ms ease;
  }
  .continue:hover:not(:disabled) { background: var(--pixiv-blue-hover); }
  .continue:active:not(:disabled) { transform: translateY(1px); }
  .continue:disabled { opacity: 0.45; cursor: default; }
  @media (max-width: 600px) {
    .setup-page {
      place-items: start stretch;
      padding-top: max(56px, calc(env(safe-area-inset-top) + 28px));
      background: white;
    }
    .setup-card {
      align-self: start;
      width: 100%;
      justify-self: stretch;
      padding: 0;
      border: 0;
      box-shadow: none;
    }
    .setup-header { margin-bottom: 22px; }
    h1 { font-size: 22px; }
  }
</style>
