<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
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
    title: string;
    subtitle: string;
    description: string;
    tag: string;
  }> = [
    {
      id: "standard",
      title: "标准模式",
      subtitle: "系统网络",
      description: "使用系统 DNS、代理与标准 TLS，适合可以直接访问 Pixiv 的网络。",
      tag: "推荐",
    },
    {
      id: "ech",
      title: "ECH 直连",
      subtitle: "Encrypted Client Hello",
      description: "Rust API 要求 TLS 1.3 ECH 成功；Android 网页登录需另行确认一次性低安全桥。",
      tag: "严格",
    },
    {
      id: "compatible",
      title: "低安全直连",
      subtitle: "固定 IP / 关闭上游校验",
      description: "API、图片以及经确认的 Android 登录可连接内置 Pixiv IP，并关闭上游 SNI/证书验证。",
      tag: "高风险",
    },
  ];

  const transportLabels: Record<RoutePlan["transport"], string> = {
    system: "系统网络",
    ech: "TLS 1.3 + ECH",
    compatible_direct: "兼容直连",
    web_view_system: "WebView 系统网络",
    web_view_proxy: "WebView 本地代理",
    web_view_insecure_bridge: "WebView 低安全 TLS 桥",
  };

  const echLabels: Record<RoutePlan["echRequirement"], string> = {
    not_applicable: "不要求",
    accepted: "必须确认 Accepted",
    platform_managed: "由平台 WebView 管理",
    preflight_only: "仅 Rust 预检 Accepted",
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
    const messages: Record<string, string> = {
      ech_unavailable: "该主机不支持当前 ECH 路线；没有回退到普通 TLS。",
      compatible_direct_unavailable: "该主机不在低安全直连白名单中。",
      web_view_proxy_unavailable: "当前平台尚未启用登录 WebView 代理。",
      web_view_transport_unavailable: "Rust 探测器不能代替平台 WebView 连接。",
      invalid_host: "请求主机无效，连接已被拒绝。",
      unsafe_acknowledgement_required: "启用低安全直连前必须确认风险。",
      insecure_transport_forbidden: "OAuth 与 token 交换禁止使用低安全直连。",
      dns_query_failed: "无法通过加密 DNS 获取 ECH 配置。",
      ech_config_unavailable: "DNS 响应中没有可用的 ECH 配置。",
      ech_not_accepted: "服务器没有接受 ECH；已按严格策略停止连接。",
      connection_failed: "目标连接失败，请检查当前网络或切换模式。",
      http_protocol_error: "TLS 已连接，但服务器没有返回有效 HTTP 响应。",
    };

    if (failure && typeof failure === "object" && failure.kind) {
      return messages[failure.kind] ?? `连接策略拒绝了请求：${failure.kind}`;
    }

    return typeof error === "string" ? error : "无法连接 Rust 核心。";
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
    api: "API",
    media: "图片",
    login: "登录页",
  } as const;

  const diagnosticStatusLabels = {
    reachable: "可连接",
    unreachable: "不可连接",
    platform_route_ready: "平台路线已就绪",
  } as const;
</script>

<svelte:head>
  <title>连接与安全 · PixNya</title>
</svelte:head>

<AppShell title="连接与安全">
  <div class="page-wrap">
    <ReturnLink fallback="/settings" label="返回设置" />
    <header class="page-heading">
      <div>
        <h1>连接与安全</h1>
        <p>为 Rust 网络层选择连接路线，并在登录前进行实时检查。</p>
      </div>
      <div class="core-state" class:online={appStatus !== null}>
        <span></span>
        {appStatus ? `核心 ${appStatus.version}` : "正在连接核心"}
      </div>
    </header>

    <section class="setup-card" aria-labelledby="connection-heading">
      <div class="card-heading">
        <div>
          <span>网络与安全</span>
          <h2 id="connection-heading">连接方式</h2>
        </div>
        <small>实时检查</small>
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
              <strong>{option.title}</strong>
              <small>{option.subtitle}</small>
              <span>{option.description}</span>
            </span>
            <span class="mode-tag">{option.tag}</span>
          </button>
        {/each}
      </div>

      <div class="connection-result" aria-live="polite">
        <div class="result-heading">
          <span class="result-dot" class:checking={isChecking}></span>
          <div>
            <small>连接检查</small>
            <strong>{isChecking ? "正在检查…" : "当前结果"}</strong>
          </div>
        </div>

        {#if routePlan && probeReport}
          <dl>
            <div>
              <dt>传输路线</dt>
              <dd>{transportLabels[routePlan.transport]} · HTTP {probeReport.httpStatus}</dd>
            </div>
            <div>
              <dt>连接地址</dt>
              <dd>{probeReport.connectedIp ?? "由系统选择"} · {probeReport.latencyMs} ms</dd>
            </div>
            <div>
              <dt>TLS / ECH</dt>
              <dd>{probeReport.tlsSummary} · {echLabels[routePlan.echRequirement]}</dd>
            </div>
          </dl>
        {:else if policyError}
          <div class="result-error"><b>!</b><span>{policyError}</span></div>
        {:else}
          <p class="result-placeholder">等待安全策略返回结果…</p>
        {/if}
      </div>

      {#if selectedMode === "compatible" && unsafeAcknowledged}
        <div class="unsafe-mode-note" role="status">
          <b>低安全直连已在本次页面会话中启用</b>
          <span>
            {unsafeWarningSuppressed
              ? "API/图片及 Android 登录桥仍为低安全路线；重复警告已按你的选择关闭。"
              : "API/图片使用低安全路线；Android 官方网页登录会在再次确认后使用一次性低安全桥。"}
          </span>
        </div>
      {/if}

      {#if unsafeWarningSuppressed}
        <div class="suppressed-warning-note" role="status">
          <div>
            <b>低安全连接警告已关闭</b>
            <span>选择低安全直连或 Android 低安全登录桥时将直接继续，但安全风险没有降低。</span>
          </div>
          <button type="button" onclick={restoreUnsafeWarnings}>恢复低安全连接警告</button>
        </div>
      {/if}

      {#if insecureMediaWarningSuppressed}
        <div class="suppressed-warning-note" role="status">
          <div>
            <b>ECH 图片连接提示已关闭</b>
            <span>新登录或应用重启后会按你的选择自动启用低安全图片路径；API、OAuth 和令牌刷新不受影响。</span>
          </div>
          <button type="button" onclick={restoreInsecureMediaWarnings}>恢复 ECH 图片连接提示</button>
        </div>
      {/if}

      <footer class="setup-actions">
        <div class="runtime-note">
          <strong>{appStatus?.platform ?? "desktop"} · {appStatus?.architecture ?? "unknown"}</strong>
          <span>
            {selectedMode === "compatible"
              ? "当前模式不验证服务器证书，请仅临时使用"
              : selectedMode === "ech"
                ? unsafeWarningSuppressed
                  ? "Rust API 验证 ECH；Android 低安全登录桥不再重复提醒"
                  : "Rust API 验证 ECH；Android 登录需单独确认低安全桥"
                : "标准模式使用系统 TLS 与证书验证"}
          </span>
        </div>
        <div class="action-buttons">
          <button
            class="diagnostic-button"
            type="button"
            disabled={!appStatus || isChecking || isDiagnosing}
            onclick={runDiagnostics}
          >
            {isDiagnosing ? "正在检查三条路线…" : "运行完整诊断"}
          </button>
          <button
            class="primary-button"
            type="button"
            disabled={!appStatus || isChecking || isDiagnosing}
            onclick={continueToLogin}
          >
            前往官方登录
            <span aria-hidden="true">›</span>
          </button>
        </div>
      </footer>
    </section>

    {#if diagnosticReport}
      <section class="diagnostic-card" aria-labelledby="diagnostic-heading">
        <header>
          <div>
            <span>脱敏诊断</span>
            <h2 id="diagnostic-heading">API、图片与登录路线</h2>
          </div>
          <button type="button" onclick={copyDiagnosticReport}>
            {copyState === "copied"
              ? "已复制"
              : copyState === "failed"
                ? "请手动复制"
                : "复制报告"}
          </button>
        </header>

        <div class="diagnostic-grid">
          {#each diagnosticReport.checks as check}
            <article class:failed={check.status === "unreachable"}>
              <div class="diagnostic-status-dot"></div>
              <div>
                <small>{diagnosticTargetLabels[check.target]} · {check.host}</small>
                <strong>{diagnosticStatusLabels[check.status]}</strong>
                <span>
                  {check.route
                    ? transportLabels[check.route.transport]
                    : check.failure
                      ? describeError({ kind: check.failure })
                      : "等待平台运行时"}
                </span>
                {#if check.httpStatus}
                  <em>HTTP {check.httpStatus} · {check.latencyMs ?? 0} ms</em>
                {:else if check.candidateAddressCount}
                  <em>{check.candidateAddressCount} 个候选地址</em>
                {/if}
              </div>
            </article>
          {/each}
        </div>

        <label for="diagnostic-report-text">可复制的脱敏文本</label>
        <textarea
          id="diagnostic-report-text"
          bind:this={reportTextArea}
          readonly
          value={diagnosticReport.text}
          rows="13"
          spellcheck="false"
        ></textarea>
        <p>报告不会写入访问令牌、Cookie、完整 OAuth URL、搜索词或浏览内容。</p>
      </section>
    {/if}

    <p class="legal-note">PixNya 为非官方项目，与 pixiv Inc. 无隶属或授权关系。</p>
  </div>
</AppShell>

{#if showUnsafeDialog}
  <div class="risk-dialog-layer">
    <button
      class="risk-dialog-scrim"
      type="button"
      aria-label="取消启用低安全直连"
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
        <small>高风险连接方式</small>
        <h2 id="risk-dialog-title">确认启用低安全直连？</h2>
      </div>
      <p id="risk-dialog-description">
        此模式会连接内置 Pixiv IP，并关闭 TLS SNI 和服务器证书验证。攻击者可能伪装成
        Pixiv，读取或修改经过 API/图片及 Android 登录桥的数据。
      </p>
      <ul>
        <li>不会作为默认模式，也不会在失败时自动启用。</li>
        <li>Android 网页登录使用一次性证书指纹锁定的本地桥；桥的上游连接不验证服务器。</li>
        <li>桥不解析或记录请求正文，但登录数据会以明文经过应用内存。</li>
        <li>授权码和 token 交换不会沿用低安全桥。</li>
        <li>未来带 access token 的 API 请求仍可能被中间人读取。</li>
        <li>默认确认仅对当前页面会话有效；勾选下方选项后可停止重复提醒。</li>
      </ul>
      <label class="suppress-warning-choice">
        <input type="checkbox" bind:checked={suppressFutureWarnings} />
        <span><b>以后不再提醒</b><small>可随时在“连接与安全”中恢复警告</small></span>
      </label>
      <div class="risk-dialog-actions">
        <button type="button" onclick={cancelUnsafeMode}>取消</button>
        <button class="danger-confirm" type="button" onclick={confirmUnsafeMode}>
          {suppressFutureWarnings ? "我了解风险，不再提醒" : "我了解风险，仅本次启用"}
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
