<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ConnectionModePicker from "$lib/components/ConnectionModePicker.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { sameConnectionModeAuthority } from "$lib/connection-mode-authority";
  import { m } from "$lib/i18n";
  import {
    readPreferredConnectionMode,
    reconcilePreferredConnectionMode,
    writePreferredConnectionMode,
  } from "$lib/preferences";
  import { initializeSession, session, switchSessionConnectionMode } from "$lib/session";
  import type {
    ConnectionDiagnosticReport,
    ConnectionMode,
    ConnectionProbe,
    PolicyFailure,
    SessionSnapshot,
  } from "$lib/types";

  let selected = $state<ConnectionMode | null>(null);
  let probeState = $state<"idle" | "checking" | "available" | "failed">("idle");
  let probeMessage = $state("");
  let diagnosticReport = $state<ConnectionDiagnosticReport | null>(null);
  let isDiagnosing = $state(false);
  let copyState = $state<"idle" | "copied" | "failed">("idle");
  let reportTextArea = $state<HTMLTextAreaElement>();
  let probeSequence = 0;

  $effect(() => {
    const sessionMode = $session.loggedIn ? $session.connectionMode : null;
    if (sessionMode && probeState !== "checking") selected = sessionMode;
  });

  onMount(() => {
    void initialize();
  });

  async function initialize() {
    const snapshot = await initializeSession().catch(() => null);
    const initialMode = snapshot
      ? (reconcilePreferredConnectionMode(snapshot) ?? "standard")
      : (readPreferredConnectionMode() ?? "standard");
    selected = initialMode;
    await testMode(initialMode, false);
  }

  async function testMode(mode: ConnectionMode, saveOnSuccess = true) {
    const sessionAtProbeStart = $session;
    selected = mode;
    const sequence = ++probeSequence;
    probeState = "checking";
    probeMessage = "";
    diagnosticReport = null;
    copyState = "idle";
    try {
      const report = await invoke<ConnectionProbe>("probe_connection", {
        mode,
        traffic: "api",
        host: "app-api.pixiv.net",
        unsafeAcknowledged: true,
      });
      if (sequence !== probeSequence || selected !== mode) return;
      const currentSession = $session;
      if (!sameConnectionModeAuthority(sessionAtProbeStart, currentSession)) {
        restoreAuthoritativeSelection(currentSession);
        return;
      }
      if (saveOnSuccess) {
        if (currentSession.loggedIn && currentSession.connectionMode !== mode) {
          await switchSessionConnectionMode(mode);
          const expectedSession = { ...sessionAtProbeStart, connectionMode: mode };
          if (!sameConnectionModeAuthority(expectedSession, $session)) {
            restoreAuthoritativeSelection($session);
            return;
          }
        } else if (currentSession.loggedIn) {
          reconcilePreferredConnectionMode(currentSession);
        } else {
          writePreferredConnectionMode(mode);
        }
      }
      probeState = "available";
      probeMessage = `${report.latencyMs} ms`;
    } catch (error) {
      if (sequence !== probeSequence || selected !== mode) return;
      probeState = "failed";
      probeMessage = describeError(error);
      if (saveOnSuccess) restoreAuthoritativeSelection($session, false);
    }
  }

  function restoreAuthoritativeSelection(snapshot: SessionSnapshot, resetStatus = true) {
    if (resetStatus) {
      probeState = "idle";
      probeMessage = "";
    }
    selected = snapshot.loggedIn
      ? (snapshot.connectionMode ?? "standard")
      : (readPreferredConnectionMode() ?? "standard");
  }

  async function runDiagnostics() {
    if (!selected || isDiagnosing) return;
    isDiagnosing = true;
    diagnosticReport = null;
    copyState = "idle";
    try {
      diagnosticReport = await invoke<ConnectionDiagnosticReport>("run_connection_diagnostics", {
        mode: selected,
        unsafeAcknowledged: true,
      });
    } catch (error) {
      probeState = "failed";
      probeMessage = describeError(error);
    } finally {
      isDiagnosing = false;
    }
  }

  async function copyDiagnosticReport() {
    if (!diagnosticReport) return;
    const report = JSON.stringify(diagnosticReport, null, 2);
    try {
      await navigator.clipboard.writeText(report);
      copyState = "copied";
    } catch {
      reportTextArea?.focus();
      reportTextArea?.select();
      copyState = document.execCommand("copy") ? "copied" : "failed";
    }
  }

  function describeError(error: unknown): string {
    const failure = error as PolicyFailure;
    if (failure && typeof failure === "object" && failure.kind) {
      const known: Record<string, () => string> = {
        ech_unavailable: m.login_error_ech_unavailable,
        compatible_direct_unavailable: m.login_error_compatible_direct_unavailable,
        dns_query_failed: m.login_error_dns_query_failed,
        ech_config_unavailable: m.login_error_ech_config_unavailable,
        ech_not_accepted: m.login_error_ech_not_accepted,
        connection_failed: m.login_error_connection_failed,
        http_protocol_error: m.login_error_http_protocol_error,
      };
      return known[failure.kind]?.() ?? m.network_error_policy_fallback({ kind: failure.kind });
    }
    return typeof error === "string" ? error : m.network_error_core();
  }
</script>

<svelte:head><title>{m.network_title()} · PixNya</title></svelte:head>

<AppShell title={m.network_title()}>
  <div class="network-page">
    <ReturnLink fallback="/settings" label={m.network_return_settings()} />
    <h1 class="page-title">{m.network_title()}</h1>
    <section class="mode-card">
      <ConnectionModePicker
        {selected}
        state={probeState}
        message={probeMessage}
        disabled={probeState === "checking" || isDiagnosing}
        onselect={(mode) => testMode(mode)}
      />
      <button
        class="retest"
        type="button"
        disabled={!selected || probeState === "checking" || isDiagnosing}
        onclick={() => selected && testMode(selected, false)}
      >{m.connection_retest()}</button>
    </section>

    <details class="diagnostics">
      <summary>{m.connection_advanced_diagnostics()}</summary>
      <button type="button" disabled={!selected || isDiagnosing} onclick={runDiagnostics}>
        {isDiagnosing ? m.network_diagnosing() : m.network_run_diagnostics()}
      </button>
      {#if diagnosticReport}
        <button type="button" onclick={copyDiagnosticReport}>
          {copyState === "copied" ? m.common_copied() : m.common_copy_report()}
        </button>
        <textarea
          bind:this={reportTextArea}
          readonly
          value={JSON.stringify(diagnosticReport, null, 2)}
          rows="13"
          spellcheck="false"
          aria-label={m.network_redacted_text()}
        ></textarea>
      {/if}
    </details>
  </div>
</AppShell>

<style>
  .network-page { width: min(720px, 100%); box-sizing: border-box; margin: 0 auto; padding: 30px 24px 50px; }
  h1 { margin: 24px 0; font-size: var(--type-title); }
  .mode-card { padding: 0; }
  .diagnostics { padding: 20px; border: 1px solid var(--line); border-radius: 18px; background: white; }
  .retest, .diagnostics button {
    min-height: 40px;
    margin-top: 16px;
    padding: 0 18px;
    border: 1px solid #cde7f8;
    border-radius: 20px;
    background: white;
    color: var(--pixiv-blue);
    font-weight: 700;
    cursor: pointer;
  }
  .diagnostics { margin-top: 18px; }
  .diagnostics summary { cursor: pointer; font-weight: 700; }
  .diagnostics button + button { margin-left: 8px; }
  textarea { width: 100%; box-sizing: border-box; margin-top: 14px; padding: 12px; resize: vertical; border: 1px solid var(--line); border-radius: 12px; font: 12px/1.5 monospace; }
  @media (max-width: 600px) { .network-page { padding: 22px 18px 42px; } .mode-card, .diagnostics { padding: 14px; } }
</style>
