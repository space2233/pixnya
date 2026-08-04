<script lang="ts">
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import {
    readUnsafeConnectionWarningSuppressed,
    writeUnsafeConnectionWarningSuppressed,
  } from "$lib/preferences";
  import { applySessionSnapshot } from "$lib/session";
  import type {
    AppStatus,
    ConnectionMode,
    LoginCompletionProgress,
    LoginCompletionResult,
    LoginLaunchResult,
    LoginPreparation,
    PolicyFailure,
    RoutePlan,
    SessionSnapshot,
  } from "$lib/types";

  const modeLabels: Record<ConnectionMode, string> = {
    standard: "标准模式",
    ech: "ECH 直连",
    compatible: "低安全直连",
  };

  const transportLabels: Record<RoutePlan["transport"], string> = {
    system: "系统网络",
    ech: "TLS 1.3 + ECH",
    compatible_direct: "兼容直连",
    web_view_system: "WebView 系统网络",
    web_view_proxy: "WebView 本地 CONNECT 代理",
    web_view_insecure_bridge: "WebView 一次性低安全 TLS 桥",
  };

  function routeNoteTitle(connectionMode: ConnectionMode): string {
    if (usesAndroidBridge && connectionMode === "ech") return "严格 ECH 预检 + 低安全登录桥";
    if (usesAndroidBridge) return "固定 Pixiv IP + 低安全登录桥";
    if (connectionMode === "ech") return "严格 ECH 预检 + 平台 WebView";
    if (connectionMode === "compatible") return "固定 Pixiv IP + 完整 TLS 验证";
    return "系统 WebView 网络";
  }

  function routeNoteBody(connectionMode: ConnectionMode): string {
    if (usesAndroidBridge) {
      return connectionMode === "ech"
        ? "Rust 会先确认目标接受 ECH；Android 官方页面随后通过一次性本地 TLS 桥连接固定 Pixiv IP。网页桥关闭上游 SNI 与证书验证，回调后的令牌交换则重新使用强制 ECH 与证书验证。"
        : "Android 官方页面和回调后的令牌交换都会连接内置 Pixiv IP，并关闭上游 SNI 与证书验证。桥不记录正文，但密码、验证码与令牌都可能被中间人读取或修改。";
    }
    if (connectionMode === "ech") {
      return "打开前由 Rust 确认目标接受 ECH；官方页面自身的 TLS 由平台 WebView 管理，界面不会把平台状态冒充为可验证的 ECH Accepted。";
    }
    if (connectionMode === "compatible") {
      return "登录 WebView 仍以原域名执行 SNI 与证书验证；回调后的令牌交换会连接内置 Pixiv IP，并关闭 SNI 与证书验证。令牌可能被中间人读取或修改。";
    }
    return "官方页面使用系统 DNS、系统代理与平台 TLS；应用不注入脚本，也不读取页面输入。";
  }

  let mode = $state<ConnectionMode>("standard");
  let preparation = $state<LoginPreparation | null>(null);
  let errorMessage = $state<string | null>(null);
  let isPreparing = $state(true);
  let isOpening = $state(false);
  let isCompleting = $state(false);
  let awaitingUnsafeAcknowledgement = $state(false);
  let unsafeAcknowledged = $state(false);
  let suppressFutureWarnings = $state(false);
  let launchResult = $state<LoginLaunchResult | null>(null);
  let activeMobileLaunchId: number | null = null;
  let mobileLaunchWasHidden = false;
  let ownsAttempt = false;
  let usesAndroidBridge = $state(false);
  let loginFinished = false;
  let destroyed = false;
  let unlistenCompleted: UnlistenFn | null = null;
  let unlistenFailed: UnlistenFn | null = null;
  let unlistenProgress: UnlistenFn | null = null;
  let completionStage = $state<
    "callback_received" | LoginCompletionProgress["stage"]
  >("callback_received");

  onMount(() => {
    const requestedMode = new URLSearchParams(window.location.search).get("mode");
    if (
      requestedMode === "standard" ||
      requestedMode === "ech" ||
      requestedMode === "compatible"
    ) {
      mode = requestedMode;
    }
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
      const status = await invoke<AppStatus>("get_app_status");
      usesAndroidBridge = status.platform === "android" && mode !== "standard";

      if (mode === "compatible" || usesAndroidBridge) {
        if (readUnsafeConnectionWarningSuppressed()) {
          unsafeAcknowledged = true;
          await prepare(true);
        } else {
          isPreparing = false;
          awaitingUnsafeAcknowledgement = true;
        }
      } else {
        await prepare(false);
      }
    } catch (error) {
      isPreparing = false;
      errorMessage = describeError(error);
    }
  }

  onDestroy(() => {
    if (ownsAttempt) void invoke("cancel_interactive_login");
  });

  function cancelUnsafeLogin() {
    window.location.assign("/");
  }

  function confirmUnsafeLogin() {
    if (suppressFutureWarnings) writeUnsafeConnectionWarningSuppressed(true);
    unsafeAcknowledged = true;
    awaitingUnsafeAcknowledgement = false;
    void prepare(true);
  }

  async function prepare(acknowledged = unsafeAcknowledged) {
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
        unsafeAcknowledged,
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
    if (completionStage === "callback_verified") return "回调已验证，正在连接令牌服务…";
    if (completionStage === "transport_ready") return "连接已就绪，正在交换登录令牌…";
    if (completionStage === "token_received") return "令牌已取得，正在写入安全存储…";
    if (completionStage === "session_saved") return "登录状态已保存，正在打开个人主页…";
    return "正在读取官方登录结果…";
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
    const messages: Record<string, string> = {
      ech_unavailable: "当前平台的登录 WebView 无法满足严格 ECH 要求。",
      compatible_direct_unavailable: "该登录主机不在低安全直连白名单中。",
      web_view_proxy_unavailable: "登录 WebView 无法安全复用低安全直连；该路线已停止。",
      unsafe_acknowledgement_required: "启用低安全模式前必须确认风险。",
      insecure_transport_forbidden: "OAuth 与 token 交换禁止使用低安全直连。",
      invalid_host: "登录主机配置无效。",
      invalid_callback_configuration: "OAuth callback 配置无效。",
      secure_random_unavailable: "系统安全随机源不可用，已停止登录。",
      state_unavailable: "登录状态暂时不可用，请重试。",
      attempt_unavailable: "登录会话已失效，请重新准备。",
      attempt_not_pending: "登录会话已经结束，请重新进入此页面。",
      invalid_authorization_url: "官方登录地址校验失败，页面未打开。",
      proxy_start_failed: "无法启动本机登录代理，请切换连接方式重试。",
      window_creation_failed: "无法创建独立登录窗口。",
      mobile_plugin_unavailable: "Android 登录组件不可用，请重新安装应用。",
      dns_query_failed: "ECH 预检无法取得加密 DNS 响应。",
      ech_config_unavailable: "ECH 预检没有取得可用配置，登录页未打开。",
      ech_not_accepted: "服务器未接受 ECH；严格模式没有降级到普通 TLS。",
      connection_failed: "ECH 预检连接失败，请切换连接方式重试。",
      http_protocol_error: "ECH 预检收到异常响应，登录页未打开。",
      oauth_configuration_unavailable: "此构建没有配置 Pixiv OAuth 兼容参数，无法交换登录结果。",
      invalid_callback: "Pixiv 返回了无效的登录结果，请重新登录。",
      callback_state_mismatch: "登录回调与当前会话不匹配，已拒绝处理。",
      authorization_denied: "你取消了 Pixiv 授权。",
      launch_mismatch: "登录结果不属于当前窗口，已拒绝处理。",
      token_client_unavailable: "无法创建证书验证连接，请检查系统时间与 TLS 环境。",
      token_transport_unavailable: "所选连接模式无法建立令牌交换通道，请检查连接诊断后重试。",
      token_request_failed: "无法通过安全连接交换登录令牌，请稍后重试。",
      token_rejected: "Pixiv 拒绝了登录令牌交换，请重新登录。",
      invalid_token_response: "Pixiv 返回的登录数据格式无效。",
      secure_storage_unavailable: "平台安全存储不可用，令牌未保存。",
      session_unavailable: "本地会话状态不可用，请重启应用。",
    };

    if (failure && typeof failure === "object" && failure.kind) {
      return messages[failure.kind] ?? `登录准备失败：${failure.kind}`;
    }

    return typeof error === "string" ? error : "无法连接 Rust 登录内核。";
  }
</script>

<svelte:head>
  <title>官方登录 · PixNya</title>
</svelte:head>

<AppShell title="登录">
  <div class="login-page">
    <ReturnLink fallback="/settings/network" label="返回连接与安全" />

    <header class="login-heading">
      <div class="pixiv-symbol"><span>p</span></div>
      <div>
        <h1>使用 Pixiv 官方页面登录</h1>
        <p>本应用没有密码输入框；低安全桥只转发页面流量，不解析或记录请求正文。</p>
      </div>
    </header>

    <section class="login-panel" aria-live="polite">
      <div class="session-summary">
        <span class="eyebrow">一次性登录会话</span>
        <h2>{modeLabels[mode]}</h2>
        <p>每次进入此页都会生成新的 PKCE verifier，并与不可导出的登录窗口绑定。</p>

        <ol class="security-steps">
          <li class:ready={preparation !== null}>
            <b>{preparation ? "✓" : "1"}</b>
            <span><strong>生成 PKCE</strong><small>Rust 内存 · S256 · 256-bit entropy</small></span>
          </li>
          <li class:ready={preparation !== null}>
            <b>{preparation ? "✓" : "2"}</b>
            <span><strong>锁定回调边界</strong><small>私有窗口 + launch ID + scheme/host/path</small></span>
          </li>
          <li class:ready={launchResult !== null}>
            <b>{launchResult ? "✓" : "3"}</b>
            <span>
              <strong>加载官方登录页</strong>
              <small>{launchResult ? "独立 WebView 已启动" : "等待你确认打开"}</small>
            </span>
          </li>
        </ol>
      </div>

      <div class="session-status">
        {#if isPreparing}
          <div class="status-banner loading">
            <span></span>
            <div><small>正在准备</small><strong>建立安全登录上下文…</strong></div>
          </div>
        {:else if awaitingUnsafeAcknowledgement}
          <div class="status-banner risky">
            <b>!</b>
            <div>
              <small>需要确认风险</small>
              <strong>{usesAndroidBridge ? "网页登录将关闭上游 SNI 与证书验证" : "兼容模式包含低安全 API 路线"}</strong>
            </div>
          </div>
          <div class="login-risk-prompt">
            <strong>为什么还需要确认？</strong>
            {#if usesAndroidBridge}
              <p>
                WebView 会校验本次会话的一次性本地证书；桥再连接内置 Pixiv IP，并关闭上游
                SNI 与服务器证书验证。攻击者可能读取或修改登录流量。桥不解析或记录正文，
                但数据会以明文经过应用内存。{mode === "ech"
                  ? "后续令牌交换会重新强制使用经过验证的 Rust ECH。"
                  : "后续令牌交换也会使用低安全直连，refresh token 与 access token 均可能被窃取。"}
              </p>
            {:else}
              <p>
                登录 WebView 仍保持端到端 TLS；回调后的 OAuth 令牌交换、API 与图片请求会使用
                固定 IP，并关闭 SNI 与证书验证。攻击者可能窃取 refresh token、access token
                或修改响应。
              </p>
            {/if}
            <label class="suppress-warning-choice">
              <input type="checkbox" bind:checked={suppressFutureWarnings} />
              <span><b>以后不再提醒</b><small>可在“连接与安全”中恢复警告</small></span>
            </label>
            <div class="risk-actions">
              <button type="button" onclick={cancelUnsafeLogin}>返回选择安全模式</button>
              <button class="danger-button" type="button" onclick={confirmUnsafeLogin}>
                {suppressFutureWarnings ? "我了解风险，不再提醒" : "我了解风险，继续检查"}
              </button>
            </div>
          </div>
        {:else if isCompleting}
          <div class="status-banner loading">
            <span></span>
            <div><small>正在完成登录</small><strong>{completionStatusText()}</strong></div>
          </div>
        {:else if errorMessage}
          <div class="status-banner failed">
            <b>!</b>
            <div><small>无法开始登录</small><strong>{errorMessage}</strong></div>
          </div>
          <button class="secondary-button" type="button" onclick={() => prepare()}>重新检查</button>
        {:else if preparation && !preparation.oauthConfigurationReady}
          <div class="status-banner failed">
            <b>!</b>
            <div>
              <small>此构建缺少 OAuth 配置</small>
              <strong>需要在本地构建环境注入兼容参数后才能完成登录</strong>
            </div>
          </div>
        {:else if preparation}
          <div class="status-banner ready">
            <b>✓</b>
            <div><small>安全上下文已就绪</small><strong>可以创建独立登录 WebView</strong></div>
          </div>

          <dl class="login-details">
            <div><dt>登录页路线</dt><dd>{transportLabels[preparation.route.transport]}</dd></div>
            <div><dt>PKCE</dt><dd>{preparation.pkceMethod}</dd></div>
            <div><dt>Callback</dt><dd>{preparation.callbackTarget}</dd></div>
            <div><dt>证书域名</dt><dd>{preparation.route.certificateHost}</dd></div>
          </dl>

          <div class:warning-note={mode === "ech" || mode === "compatible"} class="route-note">
            <strong>{routeNoteTitle(mode)}</strong>
            <p>{routeNoteBody(mode)}</p>
          </div>

          <button
            class="official-button"
            type="button"
            disabled={isOpening || isCompleting}
            onclick={openOfficialLogin}
          >
            {isOpening
              ? "正在打开…"
              : launchResult
                ? "重新打开 Pixiv 官方登录页"
                : "打开 Pixiv 官方登录页"}
          </button>

          {#if launchResult}
            <p class="launch-result">
              已使用 {transportLabels[launchResult.route.transport]} 打开；登录完成后会验证私有回调，
              {mode === "compatible" ? "按已确认的低安全路线" : "通过证书验证连接"}交换令牌，
              并把 refresh token 写入平台安全存储。
            </p>
          {/if}
        {/if}
      </div>
    </section>

    <div class="privacy-note">
      <strong>登录安全说明</strong>
      <span>
        不注入页面脚本 · 低安全桥不记录正文 · {mode === "compatible"
          ? "令牌交换同样属于低安全直连"
          : "令牌交换验证证书"} · 不记录完整回调 URL
      </span>
    </div>
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

  .login-heading p {
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.6;
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

  .session-summary > p {
    margin: 0;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.7;
  }

  .security-steps {
    display: grid;
    gap: 14px;
    margin: 26px 0 0;
    padding: 0;
    list-style: none;
  }

  .security-steps li {
    display: flex;
    gap: 11px;
    align-items: center;
  }

  .security-steps b {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    color: #999;
    border: 1px solid #d8d8d8;
    border-radius: 50%;
    font-size: 10px;
  }

  .security-steps li.ready b {
    color: white;
    border-color: var(--success);
    background: var(--success);
  }

  .security-steps strong,
  .security-steps small {
    display: block;
  }

  .security-steps strong {
    font-size: 11px;
  }

  .security-steps small {
    margin-top: 3px;
    color: var(--soft-muted);
    font-size: 9px;
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

  .status-banner.risky {
    color: #9e3b4e;
    background: #fff1f4;
  }

  .status-banner.risky > b {
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

  .route-note {
    padding: 14px;
    border-radius: 8px;
    background: #f2f8fd;
  }

  .route-note.warning-note {
    background: #fff8ea;
  }

  .route-note strong {
    font-size: 11px;
  }

  .route-note p {
    margin: 6px 0 0;
    color: #637482;
    font-size: 9px;
    line-height: 1.65;
  }

  .route-note.warning-note p {
    color: #806f50;
  }

  .login-risk-prompt {
    margin-top: 14px;
    padding: 15px;
    border: 1px solid #ffd0da;
    border-radius: 8px;
    background: #fff8f9;
  }

  .login-risk-prompt strong {
    color: #92394a;
    font-size: 11px;
  }

  .login-risk-prompt p {
    margin: 7px 0 0;
    color: #7f6066;
    font-size: 9px;
    line-height: 1.7;
  }

  .risk-actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }

  .login-risk-prompt .suppress-warning-choice {
    display: flex;
    gap: 9px;
    align-items: flex-start;
    margin-top: 13px;
    padding: 10px;
    border: 1px solid #eadde0;
    border-radius: 7px;
    background: white;
    cursor: pointer;
  }

  .login-risk-prompt .suppress-warning-choice input {
    width: 17px;
    height: 17px;
    flex: 0 0 auto;
    margin: 0;
    accent-color: #d94f68;
  }

  .login-risk-prompt .suppress-warning-choice b,
  .login-risk-prompt .suppress-warning-choice small {
    display: block;
  }

  .login-risk-prompt .suppress-warning-choice b {
    color: #5c4147;
    font-size: 9px;
  }

  .login-risk-prompt .suppress-warning-choice small {
    margin-top: 2px;
    color: #8d747a;
    font-size: 8px;
  }

  .login-risk-prompt button {
    min-height: 36px;
    flex: 1;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: 18px;
    background: white;
    cursor: pointer;
    font-size: 9px;
    font-weight: 700;
  }

  .login-risk-prompt .danger-button {
    color: white;
    border-color: #d94f68;
    background: #d94f68;
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

  .launch-result {
    margin: 10px 4px 0;
    color: var(--muted);
    font-size: 9px;
    line-height: 1.6;
  }

  .secondary-button {
    color: var(--text);
    border: 1px solid var(--line);
    background: white;
    cursor: pointer;
  }

  .privacy-note {
    display: flex;
    gap: 8px 20px;
    align-items: center;
    justify-content: center;
    margin-top: 18px;
    color: var(--soft-muted);
    font-size: 9px;
  }

  .privacy-note strong {
    color: #777;
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

    .privacy-note {
      align-items: flex-start;
      flex-direction: column;
    }

    .risk-actions {
      flex-direction: column;
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
