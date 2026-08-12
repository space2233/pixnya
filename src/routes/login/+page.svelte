<script lang="ts">
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import { readPreferredConnectionMode } from "$lib/preferences";
  import { applySessionSnapshot } from "$lib/session";
  import type {
    ConnectionMode,
    LoginCompletionProgress,
    LoginCompletionResult,
    LoginLaunchResult,
    LoginPreparation,
    PolicyFailure,
    RoutePlan,
    SessionSnapshot,
  } from "$lib/types";

  const modeLabels: Record<ConnectionMode, () => string> = {
    standard: m.login_mode_standard,
    ech: m.login_mode_ech,
    compatible: m.login_mode_compatible,
  };

  const transportLabels: Record<RoutePlan["transport"], () => string> = {
    system: m.login_transport_system,
    ech: () => "TLS 1.3 + ECH",
    compatible_direct: m.login_transport_compatible,
    web_view_system: m.login_transport_webview_system,
    web_view_proxy: m.login_transport_webview_proxy,
    web_view_insecure_bridge: m.login_transport_insecure_bridge,
  };

  let mode = $state<ConnectionMode>("standard");
  let preparation = $state<LoginPreparation | null>(null);
  let errorMessage = $state<string | null>(null);
  let isPreparing = $state(true);
  let isOpening = $state(false);
  let isCompleting = $state(false);
  let launchResult = $state<LoginLaunchResult | null>(null);
  let activeMobileLaunchId: number | null = null;
  let mobileLaunchWasHidden = false;
  let ownsAttempt = false;
  let loginFinished = false;
  let destroyed = false;
  let unlistenCompleted: UnlistenFn | null = null;
  let unlistenFailed: UnlistenFn | null = null;
  let unlistenProgress: UnlistenFn | null = null;
  let completionStage = $state<
    "callback_received" | LoginCompletionProgress["stage"]
  >("callback_received");

  onMount(() => {
    mode = readPreferredConnectionMode() ?? "standard";
    void initializeForPlatform();
    void attachLoginListeners();

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      destroyed = true;
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      unlistenCompleted?.();
      unlistenFailed?.();
      unlistenProgress?.();
    };
  });

  async function attachLoginListeners() {
    const completed = await listen<SessionSnapshot>(
      "pixiv-login-completed",
      ({ payload }) => void finishSuccessfulLogin(payload),
    );
    if (destroyed) {
      completed();
      return;
    }
    unlistenCompleted = completed;

    const failed = await listen<PolicyFailure>("pixiv-login-failed", ({ payload }) => {
      ownsAttempt = false;
      isCompleting = false;
      errorMessage = describeError(payload);
    });
    if (destroyed) {
      failed();
      return;
    }
    unlistenFailed = failed;

    const progress = await listen<LoginCompletionProgress>(
      "pixiv-login-progress",
      ({ payload }) => {
        if (!loginFinished) {
          isCompleting = true;
          completionStage = payload.stage;
        }
      },
    );
    if (destroyed) {
      progress();
      return;
    }
    unlistenProgress = progress;
  }

  async function initializeForPlatform() {
    try {
      await prepare();
    } catch (error) {
      isPreparing = false;
      errorMessage = describeError(error);
    }
  }

  onDestroy(() => {
    if (ownsAttempt) void invoke("cancel_interactive_login");
  });

  function unsafeAcknowledged(): boolean {
    return mode === "compatible";
  }

  async function prepare(acknowledged = unsafeAcknowledged()) {
    isPreparing = true;
    errorMessage = null;
    preparation = null;
    launchResult = null;

    try {
      preparation = await invoke<LoginPreparation>("prepare_interactive_login", {
        mode,
        unsafeAcknowledged: acknowledged,
      });
      ownsAttempt = true;
    } catch (error) {
      errorMessage = describeError(error);
    } finally {
      isPreparing = false;
    }
  }

  async function openOfficialLogin() {
    if (!preparation || isOpening) return;

    isOpening = true;
    errorMessage = null;
    try {
      launchResult = await invoke<LoginLaunchResult>("open_interactive_login", {
        mode,
        unsafeAcknowledged: unsafeAcknowledged(),
        windowTitle: m.login_head_title(),
      });
      if (launchResult.target === "android_login_activity") {
        activeMobileLaunchId = launchResult.launchId;
      }
    } catch (error) {
      errorMessage = describeError(error);
    } finally {
      isOpening = false;
    }
  }

  function handleVisibilityChange() {
    if (document.visibilityState === "hidden") {
      mobileLaunchWasHidden = true;
      return;
    }

    if (activeMobileLaunchId !== null && mobileLaunchWasHidden) {
      const launchId = activeMobileLaunchId;
      activeMobileLaunchId = null;
      mobileLaunchWasHidden = false;
      void completeMobileLogin(launchId);
    }
  }

  async function completeMobileLogin(launchId: number) {
    completionStage = "callback_received";
    isCompleting = true;
    try {
      const result = await invoke<LoginCompletionResult>("complete_mobile_interactive_login", {
        launchId,
      });
      if (result.status === "completed" && result.session) {
        await finishSuccessfulLogin(result.session);
      }
    } catch (error) {
      ownsAttempt = false;
      errorMessage = describeError(error);
    } finally {
      isCompleting = false;
    }
  }

  function completionStatusText(): string {
    if (completionStage === "callback_verified") return m.login_completion_callback_verified();
    if (completionStage === "transport_ready") return m.login_completion_transport_ready();
    if (completionStage === "token_received") return m.login_completion_token_received();
    if (completionStage === "session_saved") return m.login_completion_session_saved();
    return m.login_completion_reading();
  }

  async function finishSuccessfulLogin(snapshot: SessionSnapshot) {
    if (loginFinished) return;
    loginFinished = true;
    ownsAttempt = false;
    activeMobileLaunchId = null;
    applySessionSnapshot(snapshot);
    await goto("/profile");
  }

  function describeError(error: unknown): string {
    const failure = error as PolicyFailure;
    const messages: Record<string, () => string> = {
      ech_unavailable: m.login_error_ech_unavailable,
      compatible_direct_unavailable: m.login_error_compatible_direct_unavailable,
      web_view_proxy_unavailable: m.login_error_web_view_proxy_unavailable,
      unsafe_acknowledgement_required: m.login_error_unsafe_acknowledgement_required,
      insecure_transport_forbidden: m.login_error_insecure_transport_forbidden,
      invalid_host: m.login_error_invalid_host,
      invalid_callback_configuration: m.login_error_invalid_callback_configuration,
      secure_random_unavailable: m.login_error_secure_random_unavailable,
      state_unavailable: m.login_error_state_unavailable,
      attempt_unavailable: m.login_error_attempt_unavailable,
      attempt_not_pending: m.login_error_attempt_not_pending,
      invalid_authorization_url: m.login_error_invalid_authorization_url,
      proxy_start_failed: m.login_error_proxy_start_failed,
      window_creation_failed: m.login_error_window_creation_failed,
      mobile_plugin_unavailable: m.login_error_mobile_plugin_unavailable,
      dns_query_failed: m.login_error_dns_query_failed,
      ech_config_unavailable: m.login_error_ech_config_unavailable,
      ech_not_accepted: m.login_error_ech_not_accepted,
      connection_failed: m.login_error_connection_failed,
      http_protocol_error: m.login_error_http_protocol_error,
      oauth_configuration_unavailable: m.login_error_oauth_configuration_unavailable,
      invalid_callback: m.login_error_invalid_callback,
      callback_state_mismatch: m.login_error_callback_state_mismatch,
      authorization_denied: m.login_error_authorization_denied,
      launch_mismatch: m.login_error_launch_mismatch,
      token_client_unavailable: m.login_error_token_client_unavailable,
      token_transport_unavailable: m.login_error_token_transport_unavailable,
      token_request_failed: m.login_error_token_request_failed,
      token_rejected: m.login_error_token_rejected,
      invalid_token_response: m.login_error_invalid_token_response,
      secure_storage_unavailable: m.login_error_secure_storage_unavailable,
      session_unavailable: m.login_error_session_unavailable,
    };

    if (failure && typeof failure === "object" && failure.kind) {
      return messages[failure.kind]?.() ?? m.login_error_fallback({ kind: failure.kind });
    }

    return typeof error === "string" ? error : m.login_error_core();
  }
</script>

<svelte:head>
  <title>{m.login_head_title()}</title>
</svelte:head>

<AppShell title={m.login_title()}>
  <div class="login-page">
    <ReturnLink fallback="/settings/network" label={m.login_return_security()} />

    <header class="login-heading">
      <div class="pixiv-symbol"><span>p</span></div>
      <div>
        <h1>{m.login_heading()}</h1>
      </div>
    </header>

    <section class="login-panel" aria-live="polite">
      <div class="session-summary">
        <span class="eyebrow">{m.login_session_eyebrow()}</span>
        <h2>{modeLabels[mode]()}</h2>
      </div>

      <div class="session-status">
        {#if isPreparing}
          <div class="status-banner loading">
            <span></span>
            <div><small>{m.login_preparing()}</small><strong>{m.login_preparing_context()}</strong></div>
          </div>
        {:else if isCompleting}
          <div class="status-banner loading">
            <span></span>
            <div><small>{m.login_completing()}</small><strong>{completionStatusText()}</strong></div>
          </div>
        {:else if errorMessage}
          <div class="status-banner failed">
            <b>!</b>
            <div><small>{m.login_cannot_start()}</small><strong>{errorMessage}</strong></div>
          </div>
          <button class="secondary-button" type="button" onclick={() => prepare()}>{m.login_recheck()}</button>
        {:else if preparation && !preparation.oauthConfigurationReady}
          <div class="status-banner failed">
            <b>!</b>
            <div>
              <small>{m.login_oauth_missing()}</small>
              <strong>{m.login_oauth_missing_detail()}</strong>
            </div>
          </div>
        {:else if preparation}
          <div class="status-banner ready">
            <b>✓</b>
            <div><small>{m.login_context_ready()}</small><strong>{m.login_context_ready_detail()}</strong></div>
          </div>

          <dl class="login-details">
            <div><dt>{m.login_route_label()}</dt><dd>{transportLabels[preparation.route.transport]()}</dd></div>
            <div><dt>PKCE</dt><dd>{preparation.pkceMethod}</dd></div>
            <div><dt>Callback</dt><dd>{preparation.callbackTarget}</dd></div>
            <div><dt>{m.login_certificate_host()}</dt><dd>{preparation.route.certificateHost}</dd></div>
          </dl>

          <button
            class="official-button"
            type="button"
            disabled={isOpening || isCompleting}
            onclick={openOfficialLogin}
          >
            {isOpening
              ? m.login_opening()
              : launchResult
                ? m.login_reopen_official()
                : m.login_open_official()}
          </button>

        {/if}
      </div>
    </section>

  </div>
</AppShell>

<style>
  .login-page {
    width: min(920px, 100%);
    margin: 0 auto;
    padding: 36px 28px 60px;
  }

  .login-heading {
    display: flex;
    gap: 17px;
    align-items: center;
    margin: 30px 0 24px;
  }

  .pixiv-symbol {
    display: grid;
    width: 54px;
    height: 54px;
    flex: 0 0 auto;
    place-items: center;
    color: white;
    border-radius: 13px;
    background: var(--pixiv-blue);
    font-size: 31px;
    font-weight: 800;
  }

  .pixiv-symbol span {
    display: block;
    line-height: 1;
    transform: translateY(-0.1em);
  }

  .login-heading h1 {
    margin: 0;
    font-size: 23px;
  }

  .login-panel {
    display: grid;
    grid-template-columns: minmax(260px, 0.8fr) minmax(360px, 1.2fr);
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: white;
  }

  .session-summary,
  .session-status {
    padding: 28px;
  }

  .session-summary {
    border-right: 1px solid var(--line);
    background: #fafafa;
  }

  .eyebrow {
    color: var(--pixiv-blue);
    font-size: 10px;
    font-weight: 700;
  }

  .session-summary h2 {
    margin: 7px 0 9px;
    font-size: 21px;
  }

  .status-banner {
    display: flex;
    min-height: 68px;
    gap: 13px;
    align-items: center;
    padding: 14px;
    border-radius: 8px;
    background: #f6f6f6;
  }

  .status-banner > b,
  .status-banner > span {
    display: grid;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 50%;
  }

  .status-banner > span {
    background: #ddd;
  }

  .status-banner.ready {
    background: #eef9f3;
  }

  .status-banner.ready > b {
    color: white;
    background: var(--success);
  }

  .status-banner.failed {
    color: #a43e52;
    background: #fff1f4;
  }

  .status-banner.failed > b {
    background: #ffdce3;
  }

  .status-banner small,
  .status-banner strong {
    display: block;
  }

  .status-banner small {
    margin-bottom: 4px;
    color: var(--muted);
    font-size: 9px;
  }

  .status-banner strong {
    font-size: 11px;
    line-height: 1.45;
  }

  .login-details {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    margin: 16px 0;
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--line);
  }

  .login-details div {
    min-width: 0;
    padding: 12px;
    background: white;
  }

  .login-details dt {
    margin-bottom: 5px;
    color: var(--soft-muted);
    font-size: 9px;
  }

  .login-details dd {
    margin: 0;
    overflow: hidden;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .official-button,
  .secondary-button {
    width: 100%;
    height: 42px;
    margin-top: 14px;
    border-radius: 21px;
    font-size: 11px;
    font-weight: 700;
  }

  .official-button {
    color: white;
    border: 0;
    background: var(--pixiv-blue);
    cursor: pointer;
  }

  .official-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .secondary-button {
    color: var(--text);
    border: 1px solid var(--line);
    background: white;
    cursor: pointer;
  }

  @media (max-width: 720px) {
    .login-page {
      padding: 24px 16px 38px;
    }

    .login-heading {
      margin-top: 24px;
    }

    .login-heading h1 {
      font-size: 19px;
    }

    .login-panel {
      grid-template-columns: 1fr;
    }

    .session-summary {
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }

    .session-summary,
    .session-status {
      padding: 22px;
    }

  }

  @media (max-width: 420px) {
    .login-page {
      padding-right: 12px;
      padding-left: 12px;
    }

    .pixiv-symbol {
      width: 46px;
      height: 46px;
      font-size: 27px;
    }

    .login-details {
      grid-template-columns: 1fr;
    }
  }
</style>
