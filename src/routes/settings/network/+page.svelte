<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import {
    readInsecureMediaWarningSuppressed,
    readPreferredConnectionMode,
    readUnsafeConnectionWarningSuppressed,
    writeInsecureMediaWarningSuppressed,
    writePreferredConnectionMode,
    writeUnsafeConnectionWarningSuppressed,
  } from "$lib/preferences";
  import type {
    AppStatus,
    ConnectionDiagnosticReport,
    ConnectionProbe,
    ConnectionMode,
    PolicyFailure,
    RoutePlan,
  } from "$lib/types";

  const modeOptions: Array<{
    id: ConnectionMode;
    title: () => string;
    subtitle: () => string;
    description: () => string;
    tag: () => string;
  }> = [
    {
      id: "standard",
      title: m.login_mode_standard,
      subtitle: m.login_transport_system,
      description: m.network_standard_description,
      tag: m.network_recommended,
    },
    {
      id: "ech",
      title: m.login_mode_ech,
      subtitle: () => "Encrypted Client Hello",
      description: m.network_ech_description,
      tag: m.network_strict,
    },
    {
      id: "compatible",
      title: m.login_mode_compatible,
      subtitle: m.network_compatible_subtitle,
      description: m.network_compatible_description,
      tag: m.network_high_risk,
    },
  ];

  const transportLabels: Record<RoutePlan["transport"], () => string> = {
    system: m.login_transport_system,
    ech: () => "TLS 1.3 + ECH",
    compatible_direct: m.login_transport_compatible,
    web_view_system: m.login_transport_webview_system,
    web_view_proxy: m.login_transport_webview_proxy,
    web_view_insecure_bridge: m.login_transport_insecure_bridge,
  };

  const echLabels: Record<RoutePlan["echRequirement"], () => string> = {
    not_applicable: m.network_ech_not_required,
    accepted: m.network_ech_accepted,
    platform_managed: m.network_ech_platform,
    preflight_only: m.network_ech_preflight,
  };

  let selectedMode = $state<ConnectionMode>("standard");
  let appStatus = $state<AppStatus | null>(null);
  let probeReport = $state<ConnectionProbe | null>(null);
  let routePlan = $state<RoutePlan | null>(null);
  let policyError = $state<string | null>(null);
  let isChecking = $state(false);
  let unsafeAcknowledged = $state(false);
  let unsafeWarningSuppressed = $state(false);
  let insecureMediaWarningSuppressed = $state(false);
  let suppressFutureWarnings = $state(false);
  let showUnsafeDialog = $state(false);
  let diagnosticReport = $state<ConnectionDiagnosticReport | null>(null);
  let isDiagnosing = $state(false);
  let copyState = $state<"idle" | "copied" | "failed">("idle");
  let reportTextArea = $state<HTMLTextAreaElement>();

  onMount(() => {
    void initialize();
  });

  async function initialize() {
    try {
      appStatus = await invoke<AppStatus>("get_app_status");
      unsafeWarningSuppressed = readUnsafeConnectionWarningSuppressed();
      insecureMediaWarningSuppressed = readInsecureMediaWarningSuppressed();
      await selectMode(readPreferredConnectionMode(), false);
    } catch (error) {
      policyError = describeError(error);
    }
  }

  function requestMode(mode: ConnectionMode) {
    if (mode === "compatible" && (!unsafeAcknowledged || selectedMode !== "compatible")) {
      if (unsafeWarningSuppressed) {
        unsafeAcknowledged = true;
        void selectMode("compatible", true);
        return;
      }
      suppressFutureWarnings = false;
      showUnsafeDialog = true;
      return;
    }

    if (mode !== "compatible") unsafeAcknowledged = false;
    void selectMode(mode, mode === "compatible" && unsafeAcknowledged);
  }

  function cancelUnsafeMode() {
    showUnsafeDialog = false;
  }

  function confirmUnsafeMode() {
    if (suppressFutureWarnings) {
      writeUnsafeConnectionWarningSuppressed(true);
      unsafeWarningSuppressed = true;
    }
    unsafeAcknowledged = true;
    showUnsafeDialog = false;
    void selectMode("compatible", true);
  }

  function restoreUnsafeWarnings() {
    writeUnsafeConnectionWarningSuppressed(false);
    unsafeWarningSuppressed = false;
    suppressFutureWarnings = false;
  }

  function restoreInsecureMediaWarnings() {
    writeInsecureMediaWarningSuppressed(false);
    insecureMediaWarningSuppressed = false;
  }

  async function selectMode(mode: ConnectionMode, acknowledged: boolean) {
    selectedMode = mode;
    isChecking = true;
    probeReport = null;
    routePlan = null;
    policyError = null;
    diagnosticReport = null;
    copyState = "idle";

    try {
      probeReport = await invoke<ConnectionProbe>("probe_connection", {
        mode,
        traffic: "api",
        host: "app-api.pixiv.net",
        unsafeAcknowledged: acknowledged,
      });
      routePlan = probeReport.route;
      writePreferredConnectionMode(mode);
    } catch (error) {
      policyError = describeError(error);
    } finally {
      isChecking = false;
    }
  }

  function describeError(error: unknown): string {
    const failure = error as PolicyFailure;
    const messages: Record<string, () => string> = {
      ech_unavailable: m.login_error_ech_unavailable,
      compatible_direct_unavailable: m.login_error_compatible_direct_unavailable,
      web_view_proxy_unavailable: m.login_error_web_view_proxy_unavailable,
      web_view_transport_unavailable: m.network_error_webview_transport,
      invalid_host: m.login_error_invalid_host,
      unsafe_acknowledgement_required: m.login_error_unsafe_acknowledgement_required,
      insecure_transport_forbidden: m.login_error_insecure_transport_forbidden,
      dns_query_failed: m.login_error_dns_query_failed,
      ech_config_unavailable: m.login_error_ech_config_unavailable,
      ech_not_accepted: m.login_error_ech_not_accepted,
      connection_failed: m.login_error_connection_failed,
      http_protocol_error: m.login_error_http_protocol_error,
    };

    if (failure && typeof failure === "object" && failure.kind) {
      return messages[failure.kind]?.() ?? m.network_error_policy_fallback({ kind: failure.kind });
    }

    return typeof error === "string" ? error : m.network_error_core();
  }

  function continueToLogin() {
    window.location.assign(`/login?mode=${selectedMode}`);
  }

  async function runDiagnostics() {
    if (selectedMode === "compatible" && !unsafeAcknowledged) {
      showUnsafeDialog = true;
      return;
    }

    isDiagnosing = true;
    diagnosticReport = null;
    copyState = "idle";
    try {
      diagnosticReport = await invoke<ConnectionDiagnosticReport>(
        "run_connection_diagnostics",
        {
          mode: selectedMode,
          unsafeAcknowledged,
        },
      );
    } catch (error) {
      policyError = describeError(error);
    } finally {
      isDiagnosing = false;
    }
  }

  async function copyDiagnosticReport() {
    if (!diagnosticReport) return;

    try {
      await navigator.clipboard.writeText(diagnosticReport.text);
      copyState = "copied";
      return;
    } catch {
      reportTextArea?.focus();
      reportTextArea?.select();
      copyState = document.execCommand("copy") ? "copied" : "failed";
    }
  }

  const diagnosticTargetLabels = {
    api: () => "API",
    media: m.network_target_media,
    login: m.network_target_login,
  } as const;

  const diagnosticStatusLabels = {
    reachable: m.network_status_reachable,
    unreachable: m.network_status_unreachable,
    platform_route_ready: m.network_status_platform_ready,
  } as const;
</script>

<svelte:head>
  <title>{m.network_title()} · PixNya</title>
</svelte:head>

<AppShell title={m.network_title()}>
  <div class="page-wrap">
    <ReturnLink fallback="/settings" label={m.network_return_settings()} />
    <header class="page-heading">
      <div>
        <h1>{m.network_title()}</h1>
        <p>{m.network_description()}</p>
      </div>
      <div class="core-state" class:online={appStatus !== null}>
        <span></span>
        {appStatus
          ? m.network_core_version({ version: appStatus.version })
          : m.network_core_connecting()}
      </div>
    </header>

    <section class="setup-card" aria-labelledby="connection-heading">
      <div class="card-heading">
        <div>
          <span>{m.network_card_eyebrow()}</span>
          <h2 id="connection-heading">{m.network_connection_method()}</h2>
        </div>
        <small>{m.network_realtime_check()}</small>
      </div>

      <div class="mode-grid">
        {#each modeOptions as option}
          <button
            type="button"
            class="mode-card"
            class:selected={selectedMode === option.id}
            aria-pressed={selectedMode === option.id}
            class:danger={option.id === "compatible"}
            onclick={() => requestMode(option.id)}
          >
            <span class="mode-radio" aria-hidden="true"></span>
            <span class="mode-copy">
              <strong>{option.title()}</strong>
              <small>{option.subtitle()}</small>
              <span>{option.description()}</span>
            </span>
            <span class="mode-tag">{option.tag()}</span>
          </button>
        {/each}
      </div>

      <div class="connection-result" aria-live="polite">
        <div class="result-heading">
          <span class="result-dot" class:checking={isChecking}></span>
          <div>
            <small>{m.network_connection_check()}</small>
            <strong>{isChecking ? m.network_checking() : m.network_current_result()}</strong>
          </div>
        </div>

        {#if routePlan && probeReport}
          <dl>
            <div>
              <dt>{m.network_transport_route()}</dt>
              <dd>{transportLabels[routePlan.transport]()} · HTTP {probeReport.httpStatus}</dd>
            </div>
            <div>
              <dt>{m.network_connection_address()}</dt>
              <dd>{probeReport.connectedIp ?? m.network_system_selected()} · {probeReport.latencyMs} ms</dd>
            </div>
            <div>
              <dt>TLS / ECH</dt>
              <dd>{probeReport.tlsSummary} · {echLabels[routePlan.echRequirement]()}</dd>
            </div>
          </dl>
        {:else if policyError}
          <div class="result-error"><b>!</b><span>{policyError}</span></div>
        {:else}
          <p class="result-placeholder">{m.network_waiting_policy()}</p>
        {/if}
      </div>

      {#if selectedMode === "compatible" && unsafeAcknowledged}
        <div class="unsafe-mode-note" role="status">
          <b>{m.network_unsafe_enabled()}</b>
          <span>
            {unsafeWarningSuppressed
              ? m.network_unsafe_suppressed()
              : m.network_unsafe_session()}
          </span>
        </div>
      {/if}

      {#if unsafeWarningSuppressed}
        <div class="suppressed-warning-note" role="status">
          <div>
            <b>{m.network_warning_disabled()}</b>
            <span>{m.network_warning_disabled_detail()}</span>
          </div>
          <button type="button" onclick={restoreUnsafeWarnings}>{m.network_restore_warning()}</button>
        </div>
      {/if}

      {#if insecureMediaWarningSuppressed}
        <div class="suppressed-warning-note" role="status">
          <div>
            <b>{m.network_media_warning_disabled()}</b>
            <span>{m.network_media_warning_detail()}</span>
          </div>
          <button type="button" onclick={restoreInsecureMediaWarnings}>{m.network_restore_media_warning()}</button>
        </div>
      {/if}

      <footer class="setup-actions">
        <div class="runtime-note">
          <strong>{appStatus?.platform ?? "desktop"} · {appStatus?.architecture ?? "unknown"}</strong>
          <span>
            {selectedMode === "compatible"
              ? m.network_runtime_compatible()
              : selectedMode === "ech"
                ? unsafeWarningSuppressed
                  ? m.network_runtime_ech_suppressed()
                  : m.network_runtime_ech()
                : m.network_runtime_standard()}
          </span>
        </div>
        <div class="action-buttons">
          <button
            class="diagnostic-button"
            type="button"
            disabled={!appStatus || isChecking || isDiagnosing}
            onclick={runDiagnostics}
          >
            {isDiagnosing ? m.network_diagnosing() : m.network_run_diagnostics()}
          </button>
          <button
            class="primary-button"
            type="button"
            disabled={!appStatus || isChecking || isDiagnosing}
            onclick={continueToLogin}
          >
            {m.network_go_login()}
            <span aria-hidden="true">›</span>
          </button>
        </div>
      </footer>
    </section>

    {#if diagnosticReport}
      <section class="diagnostic-card" aria-labelledby="diagnostic-heading">
        <header>
          <div>
            <span>{m.network_diagnostic_eyebrow()}</span>
            <h2 id="diagnostic-heading">{m.network_diagnostic_title()}</h2>
          </div>
          <button type="button" onclick={copyDiagnosticReport}>
            {copyState === "copied"
              ? m.common_copied()
              : copyState === "failed"
                ? m.common_copy_manually()
                : m.common_copy_report()}
          </button>
        </header>

        <div class="diagnostic-grid">
          {#each diagnosticReport.checks as check}
            <article class:failed={check.status === "unreachable"}>
              <div class="diagnostic-status-dot"></div>
              <div>
                <small>{diagnosticTargetLabels[check.target]()} · {check.host}</small>
                <strong>{diagnosticStatusLabels[check.status]()}</strong>
                <span>
                  {check.route
                    ? transportLabels[check.route.transport]()
                    : check.failure
                      ? describeError({ kind: check.failure })
                      : m.network_waiting_runtime()}
                </span>
                {#if check.httpStatus}
                  <em>HTTP {check.httpStatus} · {check.latencyMs ?? 0} ms</em>
                {:else if check.candidateAddressCount}
                  <em>{m.network_candidate_addresses({ count: check.candidateAddressCount })}</em>
                {/if}
              </div>
            </article>
          {/each}
        </div>

        <label for="diagnostic-report-text">{m.network_redacted_text()}</label>
        <textarea
          id="diagnostic-report-text"
          bind:this={reportTextArea}
          readonly
          value={diagnosticReport.text}
          rows="13"
          spellcheck="false"
        ></textarea>
        <p>{m.network_report_privacy()}</p>
      </section>
    {/if}

    <p class="legal-note">{m.pixnya_unofficial_notice()}</p>
  </div>
</AppShell>

{#if showUnsafeDialog}
  <div class="risk-dialog-layer">
    <button
      class="risk-dialog-scrim"
      type="button"
      aria-label={m.network_cancel_unsafe()}
      onclick={cancelUnsafeMode}
    ></button>
    <div
      class="risk-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="risk-dialog-title"
      aria-describedby="risk-dialog-description"
    >
      <span class="risk-badge">!</span>
      <div>
        <small>{m.network_risk_eyebrow()}</small>
        <h2 id="risk-dialog-title">{m.network_risk_title()}</h2>
      </div>
      <p id="risk-dialog-description">{m.network_risk_description()}</p>
      <ul>
        <li>{m.network_risk_item_default()}</li>
        <li>{m.network_risk_item_android()}</li>
        <li>{m.network_risk_item_memory()}</li>
        <li>{m.network_risk_item_token()}</li>
        <li>{m.network_risk_item_api()}</li>
        <li>{m.network_risk_item_session()}</li>
      </ul>
      <label class="suppress-warning-choice">
        <input type="checkbox" bind:checked={suppressFutureWarnings} />
        <span><b>{m.login_warning_suppress()}</b><small>{m.login_warning_restore()}</small></span>
      </label>
      <div class="risk-dialog-actions">
        <button type="button" onclick={cancelUnsafeMode}>{m.common_cancel()}</button>
        <button class="danger-confirm" type="button" onclick={confirmUnsafeMode}>
          {suppressFutureWarnings ? m.login_confirm_forever() : m.network_confirm_session()}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page-wrap {
    width: min(1120px, 100%);
    margin: 0 auto;
    padding: 34px 28px 48px;
  }

  .page-heading {
    display: flex;
    gap: 24px;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 22px;
  }

  .page-heading h1 {
    margin: 0;
    font-size: 24px;
    letter-spacing: -0.02em;
  }

  .page-heading p {
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 12px;
  }

  .core-state {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--muted);
    font-size: 11px;
  }

  .core-state span {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #bbb;
  }

  .core-state.online span {
    background: var(--success);
  }

  .setup-card {
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: white;
  }

  .card-heading {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    padding: 22px 24px 18px;
    border-bottom: 1px solid var(--line);
  }

  .card-heading span {
    color: var(--pixiv-blue);
    font-size: 10px;
    font-weight: 700;
  }

  .card-heading h2 {
    margin: 5px 0 0;
    font-size: 18px;
  }

  .card-heading > small {
    color: var(--soft-muted);
    font-size: 11px;
  }

  .mode-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    padding: 20px 24px;
  }

  .mode-card {
    position: relative;
    display: flex;
    min-height: 166px;
    padding: 18px;
    flex-direction: column;
    text-align: left;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 9px;
    background: #fff;
    cursor: pointer;
    transition: border-color 150ms ease, box-shadow 150ms ease;
  }

  .mode-card:hover {
    border-color: #b9dcf4;
  }

  .mode-card.selected {
    border-color: var(--pixiv-blue);
    box-shadow: 0 0 0 1px var(--pixiv-blue);
  }

  .mode-card.danger.selected {
    border-color: #e85d75;
    box-shadow: 0 0 0 1px #e85d75;
  }

  .mode-radio {
    position: absolute;
    top: 17px;
    right: 17px;
    width: 17px;
    height: 17px;
    border: 1px solid #bbb;
    border-radius: 50%;
  }

  .selected .mode-radio {
    border: 5px solid var(--pixiv-blue);
  }

  .mode-copy strong,
  .mode-copy small,
  .mode-copy > span {
    display: block;
  }

  .mode-copy strong {
    padding-right: 28px;
    font-size: 15px;
  }

  .mode-copy small {
    margin-top: 4px;
    color: var(--pixiv-blue);
    font-size: 10px;
  }

  .mode-copy > span {
    margin-top: 14px;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.65;
  }

  .mode-tag {
    width: fit-content;
    margin-top: auto;
    padding: 4px 8px;
    color: #777;
    border-radius: 4px;
    background: #f3f3f3;
    font-size: 9px;
    font-weight: 700;
  }

  .selected .mode-tag {
    color: #007acb;
    background: #e9f6ff;
  }

  .mode-card.danger .mode-tag {
    color: #ae3148;
    background: #fff0f3;
  }

  .mode-card.danger.selected .mode-radio {
    border-color: #e85d75;
  }

  .connection-result {
    display: grid;
    min-height: 98px;
    grid-template-columns: 170px 1fr;
    gap: 24px;
    align-items: center;
    margin: 0 24px;
    padding: 18px 0;
    border-top: 1px solid var(--line);
  }

  .result-heading {
    display: flex;
    gap: 11px;
    align-items: center;
  }

  .result-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--success);
  }

  .result-dot.checking {
    background: #f2aa3d;
  }

  .result-heading small,
  .result-heading strong {
    display: block;
  }

  .result-heading small {
    margin-bottom: 4px;
    color: var(--soft-muted);
    font-size: 9px;
  }

  .result-heading strong {
    font-size: 12px;
  }

  .connection-result dl {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
    margin: 0;
  }

  .connection-result dl div {
    min-width: 0;
    padding-left: 14px;
    border-left: 1px solid var(--line);
  }

  .connection-result dt {
    margin-bottom: 5px;
    color: var(--soft-muted);
    font-size: 9px;
  }

  .connection-result dd {
    margin: 0;
    overflow: hidden;
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-error {
    display: flex;
    gap: 10px;
    align-items: center;
    color: #b3475b;
    font-size: 11px;
    line-height: 1.5;
  }

  .result-error b {
    display: grid;
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 50%;
    background: #fff0f3;
  }

  .result-placeholder {
    margin: 0;
    color: var(--soft-muted);
    font-size: 11px;
  }

  .unsafe-mode-note {
    display: flex;
    gap: 6px 18px;
    align-items: center;
    margin: 0 24px 18px;
    padding: 12px 14px;
    color: #8f3446;
    border: 1px solid #ffd0da;
    border-radius: 8px;
    background: #fff4f6;
    font-size: 10px;
  }

  .unsafe-mode-note span {
    color: #a65b69;
  }

  .suppressed-warning-note {
    display: flex;
    gap: 18px;
    align-items: center;
    justify-content: space-between;
    margin: 0 24px 18px;
    padding: 12px 14px;
    color: #765c34;
    border: 1px solid #eeddb8;
    border-radius: 8px;
    background: #fffaf0;
  }

  .suppressed-warning-note b,
  .suppressed-warning-note span {
    display: block;
  }

  .suppressed-warning-note b {
    font-size: 10px;
  }

  .suppressed-warning-note span {
    margin-top: 3px;
    color: #8a7859;
    font-size: 9px;
    line-height: 1.5;
  }

  .suppressed-warning-note button {
    min-height: 34px;
    flex: 0 0 auto;
    padding: 0 13px;
    color: #765c34;
    border: 1px solid #dcc99d;
    border-radius: 17px;
    background: white;
    cursor: pointer;
    font-size: 9px;
    font-weight: 700;
    white-space: nowrap;
  }

  .setup-actions {
    display: flex;
    gap: 20px;
    align-items: center;
    justify-content: space-between;
    padding: 18px 24px;
    background: var(--soft-surface);
  }

  .runtime-note strong,
  .runtime-note span {
    display: block;
  }

  .runtime-note strong {
    font-size: 10px;
    text-transform: uppercase;
  }

  .runtime-note span {
    margin-top: 4px;
    color: var(--muted);
    font-size: 9px;
  }

  .primary-button {
    display: flex;
    min-width: 190px;
    height: 42px;
    align-items: center;
    justify-content: center;
    padding: 0 18px;
    color: white;
    border: 0;
    border-radius: 21px;
    background: var(--pixiv-blue);
    cursor: pointer;
    font-size: 12px;
    font-weight: 700;
  }

  .action-buttons {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .diagnostic-button {
    height: 42px;
    padding: 0 18px;
    color: var(--pixiv-blue);
    border: 1px solid #b9dcf4;
    border-radius: 21px;
    background: white;
    cursor: pointer;
    font-size: 11px;
    font-weight: 700;
  }

  .diagnostic-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .diagnostic-card {
    margin-top: 18px;
    padding: 22px 24px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: white;
  }

  .diagnostic-card > header {
    display: flex;
    gap: 16px;
    align-items: center;
    justify-content: space-between;
  }

  .diagnostic-card > header span {
    color: var(--pixiv-blue);
    font-size: 9px;
    font-weight: 700;
  }

  .diagnostic-card h2 {
    margin: 5px 0 0;
    font-size: 17px;
  }

  .diagnostic-card > header button {
    min-height: 36px;
    padding: 0 14px;
    color: var(--pixiv-blue);
    border: 1px solid #b9dcf4;
    border-radius: 18px;
    background: #f4faff;
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
  }

  .diagnostic-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin: 18px 0;
  }

  .diagnostic-grid article {
    display: grid;
    min-width: 0;
    grid-template-columns: 9px 1fr;
    gap: 10px;
    padding: 14px;
    border: 1px solid #dcefe5;
    border-radius: 8px;
    background: #f7fcf9;
  }

  .diagnostic-grid article.failed {
    border-color: #ffd4dc;
    background: #fff7f8;
  }

  .diagnostic-status-dot {
    width: 8px;
    height: 8px;
    margin-top: 3px;
    border-radius: 50%;
    background: var(--success);
  }

  .failed .diagnostic-status-dot {
    background: #e85d75;
  }

  .diagnostic-grid small,
  .diagnostic-grid strong,
  .diagnostic-grid span,
  .diagnostic-grid em {
    display: block;
  }

  .diagnostic-grid small {
    overflow: hidden;
    color: var(--soft-muted);
    font-size: 8px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .diagnostic-grid strong {
    margin-top: 5px;
    font-size: 11px;
  }

  .diagnostic-grid span {
    margin-top: 4px;
    color: var(--muted);
    font-size: 9px;
    line-height: 1.5;
  }

  .diagnostic-grid em {
    margin-top: 8px;
    color: var(--pixiv-blue);
    font-size: 8px;
    font-style: normal;
  }

  .diagnostic-card > label {
    display: block;
    margin-bottom: 7px;
    color: var(--muted);
    font-size: 9px;
  }

  .diagnostic-card textarea {
    width: 100%;
    resize: vertical;
    padding: 13px;
    color: #414141;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: #f8f8f8;
    font: 9px/1.65 ui-monospace, SFMono-Regular, Consolas, monospace;
  }

  .diagnostic-card > p {
    margin: 8px 0 0;
    color: var(--soft-muted);
    font-size: 8px;
  }

  .primary-button:hover {
    background: var(--pixiv-blue-hover);
  }

  .primary-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .primary-button span {
    margin-left: auto;
    font-size: 21px;
    line-height: 1;
  }

  .legal-note {
    margin: 32px 0 0;
    color: var(--soft-muted);
    font-size: 9px;
    text-align: center;
  }

  .risk-dialog-layer {
    position: fixed;
    z-index: 100;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 20px;
  }

  .risk-dialog-scrim {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    background: rgba(20, 20, 24, 0.58);
  }

  .risk-dialog {
    position: relative;
    display: grid;
    width: min(480px, 100%);
    grid-template-columns: 42px 1fr;
    gap: 0 14px;
    padding: 24px;
    border: 1px solid #ffd0da;
    border-radius: 14px;
    background: white;
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.24);
  }

  .risk-badge {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    color: white;
    border-radius: 50%;
    background: #e85d75;
    font-weight: 800;
  }

  .risk-dialog small {
    color: #e85d75;
    font-size: 9px;
    font-weight: 700;
  }

  .risk-dialog h2 {
    margin: 4px 0 0;
    font-size: 18px;
  }

  .risk-dialog p,
  .risk-dialog ul,
  .risk-dialog-actions {
    grid-column: 1 / -1;
  }

  .risk-dialog p {
    margin: 20px 0 0;
    color: #5c4147;
    font-size: 11px;
    line-height: 1.75;
  }

  .risk-dialog ul {
    margin: 13px 0 0;
    padding-left: 18px;
    color: #7a6469;
    font-size: 10px;
    line-height: 1.8;
  }

  .suppress-warning-choice {
    display: flex;
    grid-column: 1 / -1;
    gap: 10px;
    align-items: flex-start;
    margin-top: 16px;
    padding: 12px;
    border: 1px solid #eadde0;
    border-radius: 8px;
    background: #fffafb;
    cursor: pointer;
  }

  .suppress-warning-choice input {
    width: 17px;
    height: 17px;
    flex: 0 0 auto;
    margin: 1px 0 0;
    accent-color: #d94f68;
  }

  .suppress-warning-choice b,
  .suppress-warning-choice small {
    display: block;
  }

  .suppress-warning-choice b {
    color: #5c4147;
    font-size: 10px;
  }

  .suppress-warning-choice small {
    margin-top: 3px;
    color: #8d747a;
    font-size: 9px;
  }

  .risk-dialog-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 22px;
  }

  .risk-dialog-actions button {
    min-height: 38px;
    padding: 0 16px;
    border: 1px solid var(--line);
    border-radius: 19px;
    background: white;
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
  }

  .risk-dialog-actions .danger-confirm {
    color: white;
    border-color: #d94f68;
    background: #d94f68;
  }

  @media (max-width: 959px) {
    .page-wrap {
      padding: 24px 20px 36px;
    }

    .page-heading h1 {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .page-heading p {
      margin-top: 0;
    }
  }

  @media (max-width: 760px) {
    .page-heading h1 {
      font-size: 20px;
    }

    .core-state {
      display: none;
    }

    .mode-grid {
      grid-template-columns: 1fr;
      padding: 16px;
    }

    .mode-card {
      min-height: 142px;
    }

    .connection-result {
      grid-template-columns: 1fr;
      gap: 14px;
      margin: 0 16px;
    }

    .connection-result dl {
      grid-template-columns: 1fr;
      gap: 10px;
    }

    .unsafe-mode-note {
      align-items: flex-start;
      margin: 0 16px 16px;
      flex-direction: column;
    }

    .suppressed-warning-note {
      align-items: stretch;
      margin: 0 16px 16px;
      flex-direction: column;
    }

    .setup-actions {
      align-items: stretch;
      padding: 16px;
      flex-direction: column;
    }

    .action-buttons {
      width: 100%;
      align-items: stretch;
      flex-direction: column;
    }

    .diagnostic-button,
    .action-buttons .primary-button {
      width: 100%;
    }

    .diagnostic-card {
      padding: 18px 16px;
    }

    .diagnostic-grid {
      grid-template-columns: 1fr;
    }

    .primary-button {
      width: 100%;
    }

  }

  @media (max-width: 420px) {
    .page-wrap {
      padding: 20px 12px 30px;
    }

    .card-heading {
      padding: 18px 16px 14px;
    }

    .risk-dialog {
      padding: 20px;
    }

    .risk-dialog-actions {
      align-items: stretch;
      flex-direction: column-reverse;
    }

    .risk-dialog-actions button {
      width: 100%;
    }
  }
</style>
