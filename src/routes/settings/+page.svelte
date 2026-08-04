<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { clearFrontendLocalData } from "$lib/local-data";
  import {
    clearLocalData,
    clearDiagnosticLogs,
    clearExportDestination,
    clearMediaCache,
    exportDiagnosticLogs,
    getBrowsingHistory,
    getDiagnosticLogSummary,
    getDownloadQueueStats,
    getExportDestinationStatus,
    getMediaCacheStats,
    getOfflineStats,
    getStorageStatus,
    selectExportDestination,
    setAutoExportDownloads,
    setBrowsingHistoryEnabled,
    setMediaCacheLimit,
  } from "$lib/pixiv-api";
  import {
    readDesktopSidebarExpanded,
    readPreferredConnectionMode,
    readReducedMotion,
    r18DefaultVisible,
    writeDesktopSidebarExpanded,
    writeR18DefaultVisible,
    writeReducedMotion,
    type PreferredConnectionMode,
  } from "$lib/preferences";
  import { applySessionSnapshot, session } from "$lib/session";
  import {
    cancelUpdate,
    checkForUpdates,
    downloadUpdate,
    getUpdateSnapshot,
    installUpdate,
    saveUpdatePreferences,
  } from "$lib/updates";
  import type {
    AppStatus,
    DiagnosticLogSummary,
    DownloadQueueStats,
    ExportDestinationStatus,
    HistorySnapshot,
    LocalDataClearFailure,
    MediaCacheStats,
    OfflineStats,
    StorageStatus,
    UpdatePreferences,
    UpdateSnapshot,
  } from "$lib/types";

  const connectionLabels: Record<PreferredConnectionMode, string> = {
    standard: "标准模式",
    ech: "ECH 直连",
  };

  const localDataFailureLabels: Record<LocalDataClearFailure, string> = {
    secure_storage: "安全存储",
    session: "当前会话",
    login_state: "登录临时状态",
    transport_state: "网络临时状态",
    offline_library: "离线资料库",
    media_cache: "媒体缓存",
    login_web_view: "登录 WebView 数据",
    diagnostic_log: "脱敏诊断日志",
    download_queue: "下载队列",
    storage_settings: "存储设置",
    export_settings: "导出目录设置",
    update_settings: "更新设置",
    local_catalog: "本地收藏夹与标签",
    browsing_history: "浏览历史",
  };

  const cacheLimitOptions = [
    { bytes: 128 * 1024 ** 2, label: "128 MiB" },
    { bytes: 256 * 1024 ** 2, label: "256 MiB" },
    { bytes: 512 * 1024 ** 2, label: "512 MiB" },
    { bytes: 1024 ** 3, label: "1 GiB" },
  ] as const;

  let appStatus = $state<AppStatus | null>(null);
  let preferredConnectionMode = $state<PreferredConnectionMode>("standard");
  let desktopSidebarExpanded = $state(true);
  let reducedMotion = $state(false);
  let offlineStats = $state<OfflineStats | null>(null);
  let downloadQueueStats = $state<DownloadQueueStats | null>(null);
  let mediaCacheStats = $state<MediaCacheStats | null>(null);
  let storageStatus = $state<StorageStatus | null>(null);
  let isSavingCacheLimit = $state(false);
  let storageNotice = $state<string | null>(null);
  let storageNoticeIsError = $state(false);
  let exportDestination = $state<ExportDestinationStatus | null>(null);
  let isSelectingExportDestination = $state(false);
  let isSavingAutoExport = $state(false);
  let exportDestinationNotice = $state<string | null>(null);
  let exportDestinationNoticeIsError = $state(false);
  let showClearCacheDialog = $state(false);
  let isClearingCache = $state(false);
  let cacheNotice = $state<string | null>(null);
  let showClearLocalDataDialog = $state(false);
  let localDataConfirmation = $state("");
  let isClearingLocalData = $state(false);
  let localDataNotice = $state<string | null>(null);
  let localDataNoticeIsError = $state(false);
  let diagnosticLogSummary = $state<DiagnosticLogSummary | null>(null);
  let isExportingDiagnosticLog = $state(false);
  let isClearingDiagnosticLog = $state(false);
  let showClearDiagnosticLogDialog = $state(false);
  let diagnosticLogNotice = $state<string | null>(null);
  let diagnosticLogNoticeIsError = $state(false);
  let browsingHistory = $state<HistorySnapshot | null>(null);
  let isSavingBrowsingHistory = $state(false);
  let browsingHistoryNotice = $state<string | null>(null);
  let browsingHistoryNoticeIsError = $state(false);
  let updateSnapshot = $state<UpdateSnapshot | null>(null);
  let isCheckingUpdates = $state(false);
  let isApplyingUpdate = $state(false);
  let isSavingUpdatePreferences = $state(false);
  let updateNotice = $state<string | null>(null);
  let updateNoticeIsError = $state(false);

  onMount(() => {
    preferredConnectionMode = readPreferredConnectionMode();
    desktopSidebarExpanded = readDesktopSidebarExpanded();
    reducedMotion = readReducedMotion();
    void loadStatus();
    void loadOfflineStats();
    void loadDownloadQueueStats();
    void loadMediaCacheStats();
    void loadStorageStatus();
    void loadExportDestination();
    void loadDiagnosticLogSummary();
    void loadBrowsingHistory();
    void loadUpdateSnapshot();
    const updatePoll = window.setInterval(() => {
      if (
        updateSnapshot?.phase === "checking" ||
        updateSnapshot?.phase === "downloading" ||
        updateSnapshot?.phase === "installing"
      ) {
        void loadUpdateSnapshot();
      }
    }, 750);
    return () => window.clearInterval(updatePoll);
  });

  async function loadStatus() {
    try {
      appStatus = await invoke<AppStatus>("get_app_status");
    } catch {
      appStatus = null;
    }
  }

  async function loadOfflineStats() {
    try {
      offlineStats = await getOfflineStats();
    } catch {
      offlineStats = null;
    }
  }

  async function loadDownloadQueueStats() {
    try {
      downloadQueueStats = await getDownloadQueueStats();
    } catch {
      downloadQueueStats = null;
    }
  }

  async function loadMediaCacheStats() {
    try {
      mediaCacheStats = await getMediaCacheStats();
    } catch {
      mediaCacheStats = null;
    }
  }

  async function loadStorageStatus() {
    try {
      storageStatus = await getStorageStatus();
    } catch {
      storageStatus = null;
    }
  }

  async function loadExportDestination() {
    try {
      exportDestination = await getExportDestinationStatus();
    } catch {
      exportDestination = null;
    }
  }

  async function chooseExportDestination() {
    isSelectingExportDestination = true;
    exportDestinationNotice = null;
    exportDestinationNoticeIsError = false;
    try {
      const selection = await selectExportDestination();
      exportDestination = selection.status;
      if (!selection.cancelled) {
        exportDestinationNotice = "导出目录已授权；后续下载可自动写入该目录，应用私有离线副本仍会保留。";
      }
    } catch {
      exportDestinationNotice = "无法获得导出目录的持续写入权限；原有设置没有改变。";
      exportDestinationNoticeIsError = true;
      await loadExportDestination();
    } finally {
      isSelectingExportDestination = false;
    }
  }

  async function removeExportDestination() {
    isSelectingExportDestination = true;
    exportDestinationNotice = null;
    exportDestinationNoticeIsError = false;
    try {
      exportDestination = await clearExportDestination();
      exportDestinationNotice = "已撤销导出目录设置；已导出的文件不会被删除。";
    } catch {
      exportDestinationNotice = "导出目录设置未能清除。";
      exportDestinationNoticeIsError = true;
    } finally {
      isSelectingExportDestination = false;
    }
  }

  async function toggleAutoExportDownloads() {
    if (!exportDestination || isSavingAutoExport) return;
    isSavingAutoExport = true;
    exportDestinationNotice = null;
    exportDestinationNoticeIsError = false;
    try {
      exportDestination = await setAutoExportDownloads(!exportDestination.autoExport);
      exportDestinationNotice = exportDestination.autoExport
        ? "下载完成后自动导出已开启。"
        : "自动导出已关闭；仍可在离线资料库中手动导出。";
    } catch {
      exportDestinationNotice = "自动导出设置保存失败。";
      exportDestinationNoticeIsError = true;
      await loadExportDestination();
    } finally {
      isSavingAutoExport = false;
    }
  }

  async function updateCacheLimit(event: Event) {
    const element = event.currentTarget as HTMLSelectElement;
    const cacheLimitBytes = Number(element.value);
    isSavingCacheLimit = true;
    storageNotice = null;
    storageNoticeIsError = false;
    try {
      storageStatus = await setMediaCacheLimit(cacheLimitBytes);
      await loadMediaCacheStats();
      storageNotice = `缓存上限已调整为 ${formatBytes(cacheLimitBytes)}；超出部分已按最近最少使用顺序清理。`;
    } catch {
      storageNotice = "缓存上限保存失败，原有离线内容没有改变。";
      storageNoticeIsError = true;
      await Promise.all([loadStorageStatus(), loadMediaCacheStats()]);
    } finally {
      isSavingCacheLimit = false;
    }
  }

  async function loadDiagnosticLogSummary() {
    try {
      diagnosticLogSummary = await getDiagnosticLogSummary();
    } catch {
      diagnosticLogSummary = null;
    }
  }

  async function loadBrowsingHistory() {
    try {
      browsingHistory = await getBrowsingHistory();
    } catch {
      browsingHistory = null;
    }
  }

  async function loadUpdateSnapshot() {
    try {
      updateSnapshot = await getUpdateSnapshot();
    } catch {
      updateSnapshot = null;
    }
  }

  async function updatePreferences(preferences: UpdatePreferences) {
    if (isSavingUpdatePreferences) return;
    isSavingUpdatePreferences = true;
    updateNotice = null;
    updateNoticeIsError = false;
    try {
      updateSnapshot = await saveUpdatePreferences(preferences);
      updateNotice = "更新设置已保存在本机。";
    } catch {
      updateNotice = "更新设置保存失败，原有设置没有改变。";
      updateNoticeIsError = true;
      await loadUpdateSnapshot();
    } finally {
      isSavingUpdatePreferences = false;
    }
  }

  function toggleAutomaticUpdateCheck() {
    if (!updateSnapshot) return;
    const autoCheck = !updateSnapshot.preferences.autoCheck;
    void updatePreferences({
      ...updateSnapshot.preferences,
      autoCheck,
      autoDownload: autoCheck ? updateSnapshot.preferences.autoDownload : false,
    });
  }

  function toggleAutomaticUpdateDownload() {
    if (!updateSnapshot) return;
    const autoDownload = !updateSnapshot.preferences.autoDownload;
    void updatePreferences({
      ...updateSnapshot.preferences,
      autoCheck: autoDownload ? true : updateSnapshot.preferences.autoCheck,
      autoDownload,
    });
  }

  function toggleUnmeteredUpdateDownloads() {
    if (!updateSnapshot) return;
    void updatePreferences({
      ...updateSnapshot.preferences,
      unmeteredOnly: !updateSnapshot.preferences.unmeteredOnly,
    });
  }

  async function checkForApplicationUpdate() {
    if (isCheckingUpdates || isApplyingUpdate) return;
    isCheckingUpdates = true;
    updateNotice = null;
    updateNoticeIsError = false;
    try {
      updateSnapshot = await checkForUpdates("manual");
      switch (updateSnapshot.phase) {
        case "available":
          updateNotice = `发现 PixNya ${updateSnapshot.available?.version ?? "新版本"}。`;
          break;
        case "up_to_date":
          updateNotice = "当前已经是最新稳定版。";
          break;
        case "not_configured":
          updateNotice = "GitHub Releases 发布地址和签名公钥尚未写入正式构建。";
          break;
        case "failed":
          updateNotice = describeUpdateFailure(updateSnapshot);
          updateNoticeIsError = true;
          break;
        default:
          updateNotice = "更新检查已完成。";
      }
    } catch {
      updateNotice = "更新检查未能启动；应用的其他功能不受影响。";
      updateNoticeIsError = true;
    } finally {
      isCheckingUpdates = false;
    }
  }

  async function downloadApplicationUpdate() {
    if (!updateSnapshot || isApplyingUpdate) return;
    isApplyingUpdate = true;
    updateNotice = "正在下载并验证更新包…";
    updateNoticeIsError = false;
    try {
      updateSnapshot = await downloadUpdate();
      if (updateSnapshot.phase === "ready_to_install") {
        updateNotice = "更新包已下载并通过签名、版本与平台校验，可以安装。";
      } else if (updateSnapshot.phase === "failed") {
        updateNotice = describeUpdateFailure(updateSnapshot);
        updateNoticeIsError = true;
      }
    } catch {
      updateNotice = "更新包下载未能启动；已下载的不完整数据不会用于安装。";
      updateNoticeIsError = true;
      await loadUpdateSnapshot();
    } finally {
      isApplyingUpdate = false;
    }
  }

  async function installApplicationUpdate() {
    if (!updateSnapshot || isApplyingUpdate) return;
    isApplyingUpdate = true;
    updateNotice = updateSnapshot.installer === "android_system"
      ? "正在打开 Android 系统安装流程…"
      : "正在启动签名安装程序…";
    updateNoticeIsError = false;
    try {
      updateSnapshot = await installUpdate();
      if (updateSnapshot.phase === "awaiting_system_action") {
        updateNotice = updateSnapshot.readyToInstall
          ? "请在系统设置中允许 PixNya 安装应用，返回后点击“继续安装”。"
          : "系统安装界面已打开；请在那里确认更新。";
      } else if (updateSnapshot.phase === "failed") {
        updateNotice = describeUpdateFailure(updateSnapshot);
        updateNoticeIsError = true;
      }
    } catch {
      updateNotice = "无法启动安全安装流程；现有应用与数据没有改变。";
      updateNoticeIsError = true;
      await loadUpdateSnapshot();
    } finally {
      isApplyingUpdate = false;
    }
  }

  async function cancelApplicationUpdate() {
    updateNotice = "正在取消更新下载…";
    updateNoticeIsError = false;
    try {
      updateSnapshot = await cancelUpdate();
      updateNotice = "更新下载已取消；不完整文件已丢弃。";
    } catch {
      updateNotice = "取消请求未能提交，下载完成前请勿安装。";
      updateNoticeIsError = true;
    }
  }

  function describeUpdateFailure(snapshot: UpdateSnapshot): string {
    switch (snapshot.failure) {
      case "busy":
        return "已有更新检查正在进行。";
      case "invalid_source_configuration":
        return "更新源配置无效；PixNya 拒绝连接不受信任的发布地址。";
      case "network_or_manifest":
        return "无法读取或验证 GitHub Releases 更新清单。";
      case "platform_unavailable":
        return "当前平台无法启动安全更新组件。";
      case "update_unavailable":
        return "可用更新已经变化，请重新检查。";
      case "download_verification":
        return "更新包未通过签名、哈希、版本或平台校验，已拒绝安装。";
      case "installation_unavailable":
        return "系统安装组件不可用；更新包仍保留，可稍后重试。";
      case "cancelled":
        return "更新下载已取消。";
      default:
        return "无法读取本机更新状态。";
    }
  }

  function describeUpdatePhase(snapshot: UpdateSnapshot | null): string {
    if (!snapshot) return "正在读取更新状态";
    switch (snapshot.phase) {
      case "checking":
        return "正在检查更新";
      case "available":
        return `发现 PixNya ${snapshot.available?.version ?? "新版本"}`;
      case "downloading":
        return "正在下载并验证更新";
      case "ready_to_install":
        return "更新包已验证，可以安装";
      case "installing":
        return "正在启动安装流程";
      case "awaiting_system_action":
        return snapshot.readyToInstall ? "等待系统授权" : "等待系统安装确认";
      case "up_to_date":
        return "当前已经是最新稳定版";
      case "not_configured":
        return "发布源等待配置";
      case "failed":
        return "上次更新操作失败";
      default:
        return "PixNya 稳定更新通道";
    }
  }

  function formatUpdateProgress(snapshot: UpdateSnapshot): string {
    const total = snapshot.totalBytes ?? snapshot.available?.sizeBytes ?? null;
    return total
      ? `${formatBytes(snapshot.downloadedBytes)} / ${formatBytes(total)}`
      : `${formatBytes(snapshot.downloadedBytes)} 已下载`;
  }

  function formatUpdateCheckTime(value?: number | null): string {
    if (!value) return "从未检查";
    return new Date(value * 1000).toLocaleString();
  }

  async function toggleBrowsingHistory() {
    if (!browsingHistory || isSavingBrowsingHistory) return;
    isSavingBrowsingHistory = true;
    browsingHistoryNotice = null;
    browsingHistoryNoticeIsError = false;
    try {
      browsingHistory = await setBrowsingHistoryEnabled(!browsingHistory.enabled);
      browsingHistoryNotice = browsingHistory.enabled
        ? "已开始在本机记录浏览历史。"
        : "已停止记录；现有历史仍保留，可在浏览历史页单独清除。";
    } catch {
      browsingHistoryNotice = "浏览历史设置保存失败，原有设置没有改变。";
      browsingHistoryNoticeIsError = true;
      await loadBrowsingHistory();
    } finally {
      isSavingBrowsingHistory = false;
    }
  }

  async function exportLocalDiagnosticLog() {
    isExportingDiagnosticLog = true;
    diagnosticLogNotice = null;
    diagnosticLogNoticeIsError = false;
    try {
      const result = await exportDiagnosticLogs();
      diagnosticLogNotice = `已将 ${result.entryCount} 条脱敏记录导出到 ${result.destination}`;
      await loadDiagnosticLogSummary();
    } catch {
      diagnosticLogNotice = "日志导出失败；没有上传或发送任何本机数据。";
      diagnosticLogNoticeIsError = true;
    } finally {
      isExportingDiagnosticLog = false;
    }
  }

  async function confirmClearDiagnosticLog() {
    isClearingDiagnosticLog = true;
    diagnosticLogNotice = null;
    diagnosticLogNoticeIsError = false;
    try {
      const removed = await clearDiagnosticLogs();
      diagnosticLogNotice = `已清除 ${removed.entryCount} 条本机脱敏诊断记录。`;
      showClearDiagnosticLogDialog = false;
      await loadDiagnosticLogSummary();
    } catch {
      diagnosticLogNotice = "脱敏诊断日志清除失败。";
      diagnosticLogNoticeIsError = true;
    } finally {
      isClearingDiagnosticLog = false;
    }
  }

  async function confirmClearMediaCache() {
    isClearingCache = true;
    cacheNotice = null;
    try {
      const removed = await clearMediaCache();
      cacheNotice = `已清理 ${removed.entryCount} 项、${formatBytes(removed.sizeBytes)} 在线媒体缓存。`;
      showClearCacheDialog = false;
      await Promise.all([loadMediaCacheStats(), loadStorageStatus()]);
    } catch {
      cacheNotice = "缓存清理失败；离线资料与登录状态没有改变。";
    } finally {
      isClearingCache = false;
    }
  }

  function openClearLocalDataDialog() {
    localDataConfirmation = "";
    showClearLocalDataDialog = true;
  }

  async function confirmClearAllLocalData() {
    if (localDataConfirmation !== "清除") return;
    isClearingLocalData = true;
    localDataNotice = null;
    localDataNoticeIsError = false;
    try {
      const report = await clearLocalData(localDataConfirmation);
      const frontend = clearFrontendLocalData();
      applySessionSnapshot({ loggedIn: false });
      preferredConnectionMode = "standard";
      desktopSidebarExpanded = true;
      reducedMotion = false;
      showClearLocalDataDialog = false;
      localDataConfirmation = "";
      await Promise.all([
        loadOfflineStats(),
        loadDownloadQueueStats(),
        loadMediaCacheStats(),
        loadStorageStatus(),
        loadExportDestination(),
        loadDiagnosticLogSummary(),
        loadBrowsingHistory(),
        loadUpdateSnapshot(),
      ]);

      const removed = `${report.downloadTasksRemoved} 个下载任务、${report.offlineEntriesRemoved} 项离线内容、${report.localCollectionsRemoved} 个本地收藏夹、${report.localTagsRemoved} 个本地标签、${report.browsingHistoryEntriesRemoved} 条浏览历史、${report.cacheEntriesRemoved} 项缓存、${report.diagnosticLogEntriesRemoved} 条诊断日志和 ${frontend.localKeysRemoved + frontend.sessionKeysRemoved} 项页面数据`;
      if (report.complete) {
        localDataNotice = `本机数据已清除：${removed}。应用已恢复默认设置。`;
      } else {
        const failures = report.failedSteps.map((step) => localDataFailureLabels[step]).join("、");
        localDataNotice = `已清除可处理的数据，但以下项目失败：${failures}。请重启应用后再次执行。`;
        localDataNoticeIsError = true;
      }
    } catch {
      localDataNotice = "清除操作未能启动，本机数据没有被报告为已全部清除。";
      localDataNoticeIsError = true;
    } finally {
      isClearingLocalData = false;
    }
  }

  function formatBytes(value: number): string {
    if (value < 1024) return `${value} B`;
    if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KiB`;
    if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
    return `${(value / 1024 ** 3).toFixed(2)} GiB`;
  }

  function toggleDesktopSidebar() {
    desktopSidebarExpanded = !desktopSidebarExpanded;
    writeDesktopSidebarExpanded(desktopSidebarExpanded);
  }

  function toggleReducedMotion() {
    reducedMotion = !reducedMotion;
    writeReducedMotion(reducedMotion);
  }

  function toggleR18DefaultVisible() {
    writeR18DefaultVisible(!$r18DefaultVisible);
  }
</script>

<svelte:head>
  <title>设置 · PixNya</title>
</svelte:head>

<AppShell title="设置">
  <div class="settings-page">
    <header class="settings-heading">
      <div>
        <span>PIXNYA</span>
        <h1>设置</h1>
        <p>集中管理账号、连接、界面、存储和隐私选项。</p>
      </div>
      <div class="runtime-state" class:online={appStatus !== null}>
        <i></i>
        <span>{appStatus ? `${appStatus.platform} · ${appStatus.architecture}` : "核心状态不可用"}</span>
      </div>
    </header>

    <div class="settings-layout">
      <nav class="settings-index" aria-label="设置分类">
        <a href="#account"><Icon name="user" size={18} />账号与登录</a>
        <a href="#connection"><Icon name="shield" size={18} />连接与安全</a>
        <a href="#interface"><Icon name="settings" size={18} />界面</a>
        <a href="#storage"><Icon name="image" size={18} />内容与存储</a>
        <a href="#updates"><Icon name="download" size={18} />应用更新</a>
        <a href="#privacy"><Icon name="shield" size={18} />隐私</a>
      </nav>

      <div class="settings-sections">
        <section id="account" class="settings-section">
          <header>
            <span><Icon name="user" size={20} /></span>
            <div><h2>账号与登录</h2><p>账号资料只会在登录完成后显示。</p></div>
          </header>
          <div class="setting-list">
            <a class="setting-row" href="/profile">
              <div><strong>Pixiv 账号</strong><small>查看登录状态、资料和账号内容</small></div>
              <span class="row-value muted">{$session.loggedIn ? ($session.user?.name ?? "已登录") : "未登录"}</span><i>›</i>
            </a>
            <a class="setting-row" href={`/login?mode=${preferredConnectionMode}`}>
              <div><strong>官方网页登录</strong><small>Android 非标准模式会先检查浏览器传输能力</small></div>
              <span class="row-value">使用{connectionLabels[preferredConnectionMode]}</span><i>›</i>
            </a>
          </div>
        </section>

        <section id="connection" class="settings-section">
          <header>
            <span class="safe"><Icon name="shield" size={20} /></span>
            <div><h2>连接与安全</h2><p>连接模式、实时诊断以及登录网络边界。</p></div>
          </header>
          <div class="setting-list">
            <a class="setting-row" href="/settings/network">
              <div><strong>默认连接方式</strong><small>标准、严格 ECH 与临时低安全直连</small></div>
              <span class="row-value accent">{connectionLabels[preferredConnectionMode]}</span><i>›</i>
            </a>
            <div class="setting-row static-row">
              <div><strong>登录 TLS</strong><small>官方网页登录、授权码和 token 交换不会忽略证书错误</small></div>
              <span class="policy-badge">始终验证</span>
            </div>
            <div class="setting-row static-row">
              <div><strong>低安全直连</strong><small>只能逐次确认，不能保存为默认连接，也不会自动回退</small></div>
              <span class="policy-badge warning">临时启用</span>
            </div>
          </div>
        </section>

        <section id="interface" class="settings-section">
          <header>
            <span><Icon name="settings" size={20} /></span>
            <div><h2>界面</h2><p>这些选项立即生效并保存在本机。</p></div>
          </header>
          <div class="setting-list">
            <div class="setting-row control-row">
              <div><strong>默认展开桌面侧栏</strong><small>仍可随时使用左上角菜单按钮切换</small></div>
              <button
                class="switch"
                class:on={desktopSidebarExpanded}
                type="button"
                role="switch"
                aria-checked={desktopSidebarExpanded}
                aria-label="默认展开桌面侧栏"
                onclick={toggleDesktopSidebar}
              ><span></span></button>
            </div>
            <div class="setting-row control-row">
              <div><strong>简化界面动效</strong><small>减少侧栏、抽屉和控件的过渡动画</small></div>
              <button
                class="switch"
                class:on={reducedMotion}
                type="button"
                role="switch"
                aria-checked={reducedMotion}
                aria-label="简化界面动效"
                onclick={toggleReducedMotion}
              ><span></span></button>
            </div>
            <div class="setting-row control-row">
              <div><strong>默认显示 R18</strong><small>仅解除本客户端遮罩；内容范围仍跟随 Pixiv 账号设置</small></div>
              <button
                class="switch"
                class:on={$r18DefaultVisible}
                type="button"
                role="switch"
                aria-checked={$r18DefaultVisible}
                aria-label="默认显示 R18"
                onclick={toggleR18DefaultVisible}
              ><span></span></button>
            </div>
          </div>
        </section>

        <section id="storage" class="settings-section">
          <header>
            <span><Icon name="image" size={20} /></span>
            <div><h2>内容与存储</h2><p>媒体显示原则、离线下载与本机空间。</p></div>
          </header>
          <div class="setting-list">
            <div
              class="storage-health-row"
              class:low={storageStatus?.health === "low"}
              class:critical={storageStatus?.health === "critical"}
              role="status"
            >
              <div>
                <strong>
                  {storageStatus?.health === "critical"
                    ? "存储空间不足，下载写入已受限"
                    : storageStatus?.health === "low"
                      ? "存储空间较低"
                      : storageStatus
                        ? "存储空间充足"
                        : "正在读取存储空间"}
                </strong>
                <small>
                  {storageStatus
                    ? `下载还可安全写入 ${formatBytes(storageStatus.writableDownloadBytes)} · 已离线 ${formatBytes(storageStatus.offlineBytes)} · 始终为系统保留 ${formatBytes(storageStatus.reserveBytes)}`
                    : "正在通过本机存储接口检查应用数据与缓存所在卷"}
                </small>
              </div>
              <span>{storageStatus ? formatBytes(storageStatus.dataAvailableBytes) + " 可用" : "读取中"}</span>
            </div>
            <div class="setting-row static-row">
              <div><strong>内容显示范围</strong><small>不绕过 Pixiv 账号的年龄与浏览设置</small></div>
              <span class="row-value">跟随 Pixiv</span>
            </div>
            <div class="setting-row static-row">
              <div><strong>在线媒体</strong><small>图片经 Rust 网络层按需读取，不向页面暴露登录令牌</small></div>
              <span class="policy-badge">受控加载</span>
            </div>
            <div class="setting-row cache-row">
              <div>
                <strong>在线媒体缓存</strong>
                <small>
                  {mediaCacheStats
                    ? `已验证 ${formatBytes(mediaCacheStats.verifiedBytes)} · 低安全 ${formatBytes(mediaCacheStats.insecureBytes)} · 上限 ${formatBytes(mediaCacheStats.maxBytes)}`
                    : "正在读取缓存占用"}
                </small>
              </div>
              <div class="cache-control">
                <span>{mediaCacheStats ? `${mediaCacheStats.entryCount} 项 · ${formatBytes(mediaCacheStats.sizeBytes)}` : "读取中"}</span>
                <button type="button" disabled={!mediaCacheStats || isClearingCache} onclick={() => (showClearCacheDialog = true)}>清理缓存</button>
              </div>
            </div>
            <div class="setting-row control-row cache-limit-row">
              <div>
                <strong>在线媒体缓存上限</strong>
                <small>修改后立即按最近最少使用顺序收缩；不会删除下载队列或离线资料库</small>
              </div>
              <select
                aria-label="在线媒体缓存上限"
                disabled={!storageStatus || isSavingCacheLimit}
                value={storageStatus?.cacheLimitBytes ?? 256 * 1024 ** 2}
                onchange={updateCacheLimit}
              >
                {#each cacheLimitOptions as option}
                  <option value={option.bytes}>{option.label}</option>
                {/each}
              </select>
            </div>
            {#if storageNotice}
              <p class="local-data-notice" class:error={storageNoticeIsError} role="status">{storageNotice}</p>
            {/if}
            {#if cacheNotice}<p class="cache-notice" role="status">{cacheNotice}</p>{/if}
            <div class="setting-row cache-row export-destination-row">
              <div>
                <strong>下载导出目录</strong>
                <small>
                  {exportDestination?.configured
                    ? `${exportDestination.kind === "android_document_tree" ? "Android 文档目录" : "系统文件夹"} · ${exportDestination.accessible ? "授权有效" : "权限不可用"}`
                    : exportDestination
                      ? "尚未选择；下载仍会安全保存在应用私有离线资料库"
                      : "正在读取目录授权状态"}
                </small>
              </div>
              <div class="cache-control export-destination-control">
                <span title={exportDestination?.label ?? ""}>
                  {exportDestination?.label ?? "应用私有目录"}
                </span>
                <button type="button" disabled={isSelectingExportDestination} onclick={chooseExportDestination}>
                  {isSelectingExportDestination ? "处理中…" : exportDestination?.configured ? "更改" : "选择目录"}
                </button>
                {#if exportDestination?.configured}
                  <button class="secondary-action" type="button" disabled={isSelectingExportDestination} onclick={removeExportDestination}>撤销</button>
                {/if}
              </div>
            </div>
            <div class="setting-row control-row">
              <div>
                <strong>下载完成后自动导出</strong>
                <small>先保留可验证的应用私有副本，再原子写入所选目录；关闭后仍可逐项手动导出</small>
              </div>
              <button
                class="switch"
                class:on={exportDestination?.autoExport ?? true}
                type="button"
                role="switch"
                aria-checked={exportDestination?.autoExport ?? true}
                aria-label="下载完成后自动导出"
                disabled={!exportDestination || isSavingAutoExport}
                onclick={toggleAutoExportDownloads}
              ><span></span></button>
            </div>
            {#if exportDestinationNotice}
              <p class="local-data-notice" class:error={exportDestinationNoticeIsError} role="status">{exportDestinationNotice}</p>
            {/if}
            <a class="setting-row" href="/offline">
              <div><strong>下载队列与离线资料库</strong><small>SQLite 队列会在退出后保留；作品、小说与 Ugoira 下载串行写入应用私有目录</small></div>
              <span class="row-value">
                {offlineStats && downloadQueueStats
                  ? `${downloadQueueStats.activeCount} 个待处理 · ${offlineStats.entryCount} 项 · ${formatBytes(offlineStats.sizeBytes)}`
                  : "读取中"}
              </span><i>›</i>
            </a>
          </div>
        </section>

        <section id="updates" class="settings-section">
          <header>
            <span><Icon name="download" size={20} /></span>
            <div><h2>应用更新</h2><p>通过 GitHub Releases 获取稳定版，安装前始终由你确认。</p></div>
          </header>
          <div class="setting-list">
            <div class="setting-row update-status-row">
              <div>
                <strong>{describeUpdatePhase(updateSnapshot)}</strong>
                <small>
                  GitHub Releases · {updateSnapshot?.installer === "android_system" ? "Android 系统安装器" : "Tauri 签名安装"}
                  · {formatUpdateCheckTime(updateSnapshot?.lastAttemptedAtUnixSeconds ?? updateSnapshot?.lastCheckedAtUnixSeconds)}
                </small>
              </div>
              <button
                class="update-check-button"
                type="button"
                disabled={!updateSnapshot || isCheckingUpdates || isApplyingUpdate || updateSnapshot.phase === "downloading" || updateSnapshot.phase === "installing"}
                onclick={checkForApplicationUpdate}
              >{isCheckingUpdates ? "检查中…" : "立即检查"}</button>
            </div>
            <div class="setting-row control-row">
              <div><strong>自动检查更新</strong><small>启动后检查一次，成功检查后 24 小时内不再重复</small></div>
              <button
                class="switch"
                class:on={updateSnapshot?.preferences.autoCheck ?? true}
                type="button"
                role="switch"
                aria-checked={updateSnapshot?.preferences.autoCheck ?? true}
                aria-label="自动检查更新"
                disabled={!updateSnapshot || isSavingUpdatePreferences}
                onclick={toggleAutomaticUpdateCheck}
              ><span></span></button>
            </div>
            <div class="setting-row control-row">
              <div><strong>自动下载更新</strong><small>默认关闭；只下载通过签名与平台匹配检查的正式产物</small></div>
              <button
                class="switch"
                class:on={updateSnapshot?.preferences.autoDownload ?? false}
                type="button"
                role="switch"
                aria-checked={updateSnapshot?.preferences.autoDownload ?? false}
                aria-label="自动下载更新"
                disabled={!updateSnapshot || !updateSnapshot.sourceConfigured || isSavingUpdatePreferences}
                onclick={toggleAutomaticUpdateDownload}
              ><span></span></button>
            </div>
            {#if updateSnapshot?.installer === "android_system"}
              <div class="setting-row control-row">
                <div><strong>仅在非计费网络自动下载</strong><small>不会影响手动检查；安装仍由 Android 系统界面确认</small></div>
                <button
                  class="switch"
                  class:on={updateSnapshot.preferences.unmeteredOnly}
                  type="button"
                  role="switch"
                  aria-checked={updateSnapshot.preferences.unmeteredOnly}
                  aria-label="仅在非计费网络自动下载更新"
                  disabled={!updateSnapshot.preferences.autoDownload || isSavingUpdatePreferences}
                  onclick={toggleUnmeteredUpdateDownloads}
                ><span></span></button>
              </div>
            {/if}
            {#if updateSnapshot?.available}
              <div class="update-release-notes">
                <strong>PixNya {updateSnapshot.available.version}</strong>
                {#if updateSnapshot.available.publishedAt || updateSnapshot.available.sizeBytes}
                  <small>
                    {updateSnapshot.available.publishedAt ?? ""}
                    {updateSnapshot.available.sizeBytes ? ` · ${formatBytes(updateSnapshot.available.sizeBytes)}` : ""}
                  </small>
                {/if}
                <p>{updateSnapshot.available.notes || "此版本没有提供发布说明。"}</p>
                {#if updateSnapshot.phase === "downloading"}
                  <div class="update-progress" role="progressbar" aria-valuemin="0" aria-valuemax={updateSnapshot.totalBytes ?? undefined} aria-valuenow={updateSnapshot.downloadedBytes}>
                    <span style={`width: ${updateSnapshot.totalBytes ? Math.min(100, updateSnapshot.downloadedBytes / updateSnapshot.totalBytes * 100) : 0}%`}></span>
                  </div>
                  <small>{formatUpdateProgress(updateSnapshot)}</small>
                {/if}
                <div class="update-actions">
                  {#if updateSnapshot.phase === "available" || updateSnapshot.phase === "failed"}
                    <button type="button" disabled={isApplyingUpdate || !updateSnapshot.sourceConfigured} onclick={downloadApplicationUpdate}>下载并验证</button>
                  {/if}
                  {#if updateSnapshot.phase === "ready_to_install" || (updateSnapshot.phase === "awaiting_system_action" && updateSnapshot.readyToInstall)}
                    <button class="primary" type="button" disabled={isApplyingUpdate} onclick={installApplicationUpdate}>
                      {updateSnapshot.phase === "awaiting_system_action" ? "继续安装" : updateSnapshot.installer === "android_system" ? "打开系统安装器" : "安装更新"}
                    </button>
                  {/if}
                  {#if updateSnapshot.phase === "downloading"}
                    <button type="button" onclick={cancelApplicationUpdate}>取消下载</button>
                  {:else if updateSnapshot.phase === "ready_to_install"}
                    <button type="button" disabled={isApplyingUpdate} onclick={cancelApplicationUpdate}>删除更新包</button>
                  {/if}
                </div>
              </div>
            {/if}
            {#if updateNotice}
              <p class="local-data-notice" class:error={updateNoticeIsError} role="status">{updateNotice}</p>
            {/if}
          </div>
        </section>

        <section id="privacy" class="settings-section">
          <header>
            <span class="safe"><Icon name="shield" size={20} /></span>
            <div><h2>隐私与关于</h2><p>不可被普通设置关闭的安全策略。</p></div>
          </header>
          <div class="setting-list">
            <div class="setting-row static-row">
              <div><strong>遥测与广告标识</strong><small>当前版本不上传使用数据，也不创建广告标识</small></div>
              <span class="policy-badge">关闭</span>
            </div>
            <div class="setting-row control-row">
              <div>
                <strong>本机浏览历史</strong>
                <small>
                  {browsingHistory
                    ? `已保存 ${browsingHistory.entries.length}/${browsingHistory.limit} 条；关闭后保留现有记录`
                    : "正在读取浏览历史设置"}
                  · <a class="inline-link" href="/history">管理历史</a>
                </small>
              </div>
              <button
                class="switch"
                class:on={browsingHistory?.enabled ?? false}
                type="button"
                role="switch"
                aria-label="在本机记录浏览历史"
                aria-checked={browsingHistory?.enabled ?? false}
                disabled={!browsingHistory || isSavingBrowsingHistory}
                onclick={toggleBrowsingHistory}
              ><span></span></button>
            </div>
            {#if browsingHistoryNotice}
              <p class="local-data-notice" class:error={browsingHistoryNoticeIsError} role="status">{browsingHistoryNotice}</p>
            {/if}
            <div class="setting-row cache-row diagnostic-log-row">
              <div>
                <strong>脱敏诊断日志</strong>
                <small>
                  {diagnosticLogSummary
                    ? `${diagnosticLogSummary.entryCount} 条 · ${formatBytes(diagnosticLogSummary.retainedBytes)} · 保留 ${diagnosticLogSummary.retentionDays} 天 · 上限 ${formatBytes(diagnosticLogSummary.maxBytes)}`
                    : "正在读取本机日志状态"}
                  ；不记录令牌、Cookie、URL、账号/作品编号、搜索词或响应正文
                </small>
              </div>
              <div class="cache-control diagnostic-log-control">
                <button type="button" disabled={!diagnosticLogSummary || isExportingDiagnosticLog} onclick={exportLocalDiagnosticLog}>
                  {isExportingDiagnosticLog ? "导出中…" : "导出日志"}
                </button>
                <button type="button" disabled={!diagnosticLogSummary || isClearingDiagnosticLog} onclick={() => (showClearDiagnosticLogDialog = true)}>清除日志</button>
              </div>
            </div>
            {#if diagnosticLogNotice}
              <p class="local-data-notice" class:error={diagnosticLogNoticeIsError} role="status">{diagnosticLogNotice}</p>
            {/if}
            <div class="setting-row danger-row">
              <div><strong>清除所有本机数据</strong><small>退出账号，并删除 Cookie、下载队列、离线内容、缓存、浏览与搜索历史、诊断日志、阅读进度、更新配置和界面偏好</small></div>
              <button type="button" disabled={isClearingLocalData} onclick={openClearLocalDataDialog}>清除数据</button>
            </div>
            {#if localDataNotice}
              <p class="local-data-notice" class:error={localDataNoticeIsError} role="status">{localDataNotice}</p>
            {/if}
            <div class="setting-row static-row">
              <div><strong>PixNya 版本</strong><small>非官方、开源侧载应用</small></div>
              <span class="row-value">PixNya {appStatus?.version ?? "0.28.2"}</span>
            </div>
          </div>
        </section>

        <p class="settings-legal">PixNya 为非官方项目，与 pixiv Inc. 无隶属或授权关系。</p>
      </div>
    </div>
  </div>
</AppShell>

{#if showClearCacheDialog}
  <div class="cache-dialog-layer">
    <button class="cache-dialog-scrim" type="button" aria-label="取消清理缓存" onclick={() => (showClearCacheDialog = false)}></button>
    <div role="alertdialog" aria-modal="true" aria-labelledby="cache-dialog-title" class="cache-dialog">
      <span><Icon name="image" size={22} /></span>
      <div>
        <small>仅清理临时内容</small>
        <h2 id="cache-dialog-title">清理在线媒体缓存？</h2>
      </div>
      <p>将删除缩略图和预览图缓存，包括与低安全链路隔离保存的内容。不会删除离线资料库、下载作品、登录令牌或界面设置。</p>
      <div class="cache-dialog-actions">
        <button type="button" disabled={isClearingCache} onclick={() => (showClearCacheDialog = false)}>取消</button>
        <button class="confirm-clear" type="button" disabled={isClearingCache} onclick={confirmClearMediaCache}>{isClearingCache ? "正在清理…" : "确认清理"}</button>
      </div>
    </div>
  </div>
{/if}

{#if showClearDiagnosticLogDialog}
  <div class="cache-dialog-layer">
    <button class="cache-dialog-scrim" type="button" aria-label="取消清除诊断日志" disabled={isClearingDiagnosticLog} onclick={() => (showClearDiagnosticLogDialog = false)}></button>
    <div role="alertdialog" aria-modal="true" aria-labelledby="diagnostic-log-dialog-title" class="cache-dialog">
      <span><Icon name="shield" size={22} /></span>
      <div>
        <small>仅删除本机诊断记录</small>
        <h2 id="diagnostic-log-dialog-title">清除脱敏诊断日志？</h2>
      </div>
      <p>将删除当前保留的 {diagnosticLogSummary?.entryCount ?? 0} 条诊断记录。不会删除登录状态、离线内容、媒体缓存或已经导出的文本文件。</p>
      <div class="cache-dialog-actions">
        <button type="button" disabled={isClearingDiagnosticLog} onclick={() => (showClearDiagnosticLogDialog = false)}>取消</button>
        <button class="confirm-clear" type="button" disabled={isClearingDiagnosticLog} onclick={confirmClearDiagnosticLog}>{isClearingDiagnosticLog ? "正在清除…" : "确认清除"}</button>
      </div>
    </div>
  </div>
{/if}

{#if showClearLocalDataDialog}
  <div class="cache-dialog-layer">
    <button
      class="cache-dialog-scrim"
      type="button"
      aria-label="取消清除本机数据"
      disabled={isClearingLocalData}
      onclick={() => (showClearLocalDataDialog = false)}
    ></button>
    <div role="alertdialog" aria-modal="true" aria-labelledby="local-data-dialog-title" class="cache-dialog destructive-dialog">
      <span><Icon name="shield" size={22} /></span>
      <div>
        <small>此操作无法撤销</small>
        <h2 id="local-data-dialog-title">清除所有本机数据？</h2>
      </div>
      <p>
        将退出 Pixiv 账号，并永久删除本机安全存储中的令牌、登录 WebView Cookie、
        {offlineStats?.entryCount ?? 0} 项离线内容、{mediaCacheStats?.entryCount ?? 0} 项媒体缓存、{browsingHistory?.entries.length ?? 0} 条浏览历史、{diagnosticLogSummary?.entryCount ?? 0} 条诊断日志、搜索历史、小说阅读进度及界面设置。
      </p>
      <label class="confirmation-field">
        <span>请输入“清除”以确认</span>
        <input
          bind:value={localDataConfirmation}
          autocomplete="off"
          spellcheck="false"
          disabled={isClearingLocalData}
          placeholder="清除"
        />
      </label>
      <div class="cache-dialog-actions">
        <button type="button" disabled={isClearingLocalData} onclick={() => (showClearLocalDataDialog = false)}>取消</button>
        <button
          class="confirm-clear destructive"
          type="button"
          disabled={isClearingLocalData || localDataConfirmation !== "清除"}
          onclick={confirmClearAllLocalData}
        >{isClearingLocalData ? "正在清除…" : "永久清除"}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-page {
    width: min(1040px, 100%);
    margin: 0 auto;
    padding: 34px 28px 56px;
  }

  .settings-heading {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    justify-content: space-between;
    margin-bottom: 26px;
  }

  .settings-heading > div > span {
    color: var(--pixiv-blue);
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.08em;
  }

  .settings-heading h1 {
    margin: 5px 0 0;
    font-size: 28px;
    letter-spacing: -0.03em;
  }

  .settings-heading p {
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 11px;
  }

  .runtime-state {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--muted);
    font-size: 10px;
  }

  .runtime-state i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #bbb;
  }

  .runtime-state.online i {
    background: var(--success);
  }

  .settings-layout {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    gap: 24px;
    align-items: start;
  }

  .settings-index {
    position: sticky;
    top: calc(var(--topbar-height) + 20px);
    display: grid;
    gap: 3px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: 11px;
    background: white;
  }

  .settings-index a {
    display: flex;
    min-height: 42px;
    gap: 11px;
    align-items: center;
    padding: 0 11px;
    color: #666;
    border-radius: 7px;
    font-size: 11px;
    text-decoration: none;
  }

  .settings-index a:hover {
    color: var(--text);
    background: #f4f4f4;
  }

  .settings-sections {
    display: grid;
    gap: 18px;
    min-width: 0;
  }

  .settings-section {
    scroll-margin-top: calc(var(--topbar-height) + 20px);
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: white;
  }

  .settings-section > header {
    display: flex;
    gap: 13px;
    align-items: center;
    padding: 18px 20px 15px;
    border-bottom: 1px solid var(--line);
    background: #fbfbfb;
  }

  .settings-section > header > span {
    display: grid;
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    place-items: center;
    color: var(--pixiv-blue);
    border-radius: 10px;
    background: #eaf7ff;
  }

  .settings-section > header > span.safe {
    color: #24895a;
    background: #eaf7f0;
  }

  .settings-section h2 {
    margin: 0;
    font-size: 15px;
  }

  .settings-section header p {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 9px;
  }

  .setting-list {
    display: grid;
  }

  .storage-health-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    padding: 15px 20px;
    color: #23744f;
    border-bottom: 1px solid #d9eee3;
    background: #f2fbf6;
  }

  .storage-health-row.low {
    color: #976020;
    border-color: #f0dfc2;
    background: #fff9ee;
  }

  .storage-health-row.critical {
    color: #a23d3d;
    border-color: #efd1d1;
    background: #fff5f5;
  }

  .storage-health-row strong,
  .storage-health-row small {
    display: block;
  }

  .storage-health-row strong {
    font-size: 11px;
  }

  .storage-health-row small {
    margin-top: 5px;
    color: currentColor;
    font-size: 9px;
    line-height: 1.55;
    opacity: 0.78;
  }

  .storage-health-row > span {
    padding: 5px 9px;
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.72);
    font-size: 8px;
    font-weight: 700;
    white-space: nowrap;
  }

  .setting-row {
    display: grid;
    min-height: 68px;
    grid-template-columns: minmax(0, 1fr) auto 14px;
    gap: 14px;
    align-items: center;
    padding: 13px 20px;
    color: var(--text);
    text-decoration: none;
  }

  .setting-row + .setting-row {
    border-top: 1px solid var(--line);
  }

  a.setting-row:hover {
    background: #fafafa;
  }

  .setting-row strong,
  .setting-row small {
    display: block;
  }

  .setting-row strong {
    font-size: 11px;
  }

  .setting-row small {
    margin-top: 5px;
    color: var(--muted);
    font-size: 9px;
    line-height: 1.5;
  }

  .inline-link {
    color: var(--pixiv-blue);
    font-weight: 700;
    text-decoration: none;
  }

  .inline-link:hover {
    text-decoration: underline;
  }

  .setting-row > i {
    color: #aaa;
    font-size: 18px;
    font-style: normal;
  }

  .static-row,
  .control-row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .cache-row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .cache-limit-row select {
    min-width: 112px;
    min-height: 34px;
    padding: 0 28px 0 11px;
    color: #444;
    border: 1px solid #d8d8d8;
    border-radius: 8px;
    background: white;
    font: inherit;
    font-size: 10px;
  }

  .cache-limit-row select:focus {
    border-color: var(--pixiv-blue);
    outline: 3px solid rgba(0, 150, 250, 0.1);
  }

  .cache-limit-row select:disabled {
    opacity: 0.55;
  }

  .danger-row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .danger-row strong {
    color: #b33b3b;
  }

  .danger-row button {
    min-height: 32px;
    padding: 0 12px;
    color: #b33b3b;
    border: 1px solid #efc0c0;
    border-radius: 16px;
    background: #fff7f7;
    cursor: pointer;
    font-size: 9px;
    font-weight: 700;
  }

  .danger-row button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .cache-control {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .cache-control span {
    color: var(--muted);
    font-size: 9px;
    white-space: nowrap;
  }

  .diagnostic-log-control {
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .export-destination-control {
    max-width: min(430px, 46vw);
  }

  .export-destination-control > span {
    overflow: hidden;
    max-width: 240px;
    text-overflow: ellipsis;
  }

  .cache-control button {
    min-height: 32px;
    padding: 0 12px;
    color: var(--pixiv-blue);
    border: 1px solid #b9dcf4;
    border-radius: 16px;
    background: #f4faff;
    cursor: pointer;
    font-size: 9px;
    font-weight: 700;
  }

  .cache-control button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .cache-control .secondary-action {
    color: #777;
    border-color: #ddd;
    background: white;
  }

  .cache-notice {
    margin: 0;
    padding: 9px 20px;
    color: #2a7955;
    border-top: 1px solid #dcefe5;
    background: #f5fbf8;
    font-size: 9px;
  }

  .local-data-notice {
    margin: 0;
    padding: 10px 20px;
    color: #2a7955;
    border-top: 1px solid #dcefe5;
    background: #f5fbf8;
    font-size: 9px;
    line-height: 1.6;
  }

  .local-data-notice.error {
    color: #9a3737;
    border-color: #f0d4d4;
    background: #fff7f7;
  }

  .update-status-row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .update-check-button {
    min-height: 34px;
    padding: 0 14px;
    color: white;
    border: 0;
    border-radius: 17px;
    background: var(--pixiv-blue);
    cursor: pointer;
    font: inherit;
    font-size: 9px;
    font-weight: 700;
    white-space: nowrap;
  }

  .update-check-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .update-release-notes {
    display: grid;
    gap: 5px;
    padding: 16px 20px;
    border-top: 1px solid var(--line);
    background: #f8fcff;
  }

  .update-release-notes strong {
    color: #222;
    font-size: 11px;
  }

  .update-release-notes small,
  .update-release-notes p {
    margin: 0;
    color: var(--muted);
    font-size: 9px;
    line-height: 1.7;
  }

  .update-progress {
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: #dceffc;
  }

  .update-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--pixiv-blue);
    transition: width 160ms ease;
  }

  .update-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }

  .update-actions button {
    min-height: 34px;
    padding: 0 14px;
    color: #555;
    border: 1px solid #d5e5ef;
    border-radius: 17px;
    background: white;
    cursor: pointer;
    font: inherit;
    font-size: 9px;
    font-weight: 700;
  }

  .update-actions button.primary {
    color: white;
    border-color: var(--pixiv-blue);
    background: var(--pixiv-blue);
  }

  .update-actions button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .update-release-notes p {
    max-height: 8.5em;
    overflow: auto;
    white-space: pre-wrap;
  }

  .row-value {
    max-width: 210px;
    color: #555;
    font-size: 10px;
    text-align: right;
  }

  .row-value.muted {
    color: var(--muted);
  }

  .row-value.accent {
    color: var(--pixiv-blue);
    font-weight: 700;
  }

  .policy-badge {
    padding: 5px 9px;
    border-radius: 10px;
    color: #278258;
    background: #eaf7f0;
    font-size: 8px;
    font-weight: 700;
    white-space: nowrap;
  }

  .policy-badge.warning {
    color: #a66231;
    background: #fff2e6;
  }

  .switch {
    position: relative;
    width: 42px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 12px;
    background: #d6d6d6;
    cursor: pointer;
    transition: background 150ms ease;
  }

  .switch span {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
    transition: transform 150ms ease;
  }

  .switch.on {
    background: var(--pixiv-blue);
  }

  .switch.on span {
    transform: translateX(18px);
  }

  .switch:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .settings-legal {
    margin: 6px 0 0;
    color: var(--soft-muted);
    font-size: 9px;
    text-align: center;
  }

  .cache-dialog-layer {
    position: fixed;
    z-index: 120;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 20px;
  }

  .cache-dialog-scrim {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    background: rgba(20, 20, 24, 0.58);
  }

  .cache-dialog {
    position: relative;
    display: grid;
    width: min(460px, 100%);
    grid-template-columns: 42px 1fr;
    gap: 0 14px;
    padding: 24px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: white;
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.24);
  }

  .cache-dialog > span {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    color: var(--pixiv-blue);
    border-radius: 12px;
    background: #eaf7ff;
  }

  .cache-dialog small {
    color: var(--pixiv-blue);
    font-size: 9px;
    font-weight: 700;
  }

  .cache-dialog h2 {
    margin: 4px 0 0;
    font-size: 18px;
  }

  .cache-dialog p,
  .cache-dialog-actions {
    grid-column: 1 / -1;
  }

  .cache-dialog p {
    margin: 20px 0 0;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.75;
  }

  .cache-dialog-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 22px;
  }

  .cache-dialog-actions button {
    min-height: 38px;
    padding: 0 16px;
    border: 1px solid var(--line);
    border-radius: 19px;
    background: white;
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
  }

  .cache-dialog-actions .confirm-clear {
    color: white;
    border-color: var(--pixiv-blue);
    background: var(--pixiv-blue);
  }

  .destructive-dialog > span {
    color: #b33b3b;
    background: #fff0f0;
  }

  .destructive-dialog small {
    color: #b33b3b;
  }

  .confirmation-field {
    display: grid;
    grid-column: 1 / -1;
    gap: 7px;
    margin-top: 18px;
  }

  .confirmation-field span {
    color: #555;
    font-size: 9px;
    font-weight: 700;
  }

  .confirmation-field input {
    min-height: 42px;
    padding: 0 13px;
    border: 1px solid #d8d8d8;
    border-radius: 8px;
    outline: none;
    font: inherit;
  }

  .confirmation-field input:focus {
    border-color: #b33b3b;
    box-shadow: 0 0 0 3px rgba(179, 59, 59, 0.1);
  }

  .cache-dialog-actions .confirm-clear.destructive {
    border-color: #b33b3b;
    background: #b33b3b;
  }

  .cache-dialog-actions button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  @media (max-width: 820px) {
    .settings-page {
      padding: 24px 18px 42px;
    }

    .settings-heading h1 {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .settings-heading p {
      margin-top: 3px;
    }

    .runtime-state {
      display: none;
    }

    .settings-layout {
      grid-template-columns: 1fr;
      gap: 16px;
    }

    .settings-index {
      position: static;
      display: flex;
      overflow-x: auto;
      padding: 6px;
      scrollbar-width: none;
    }

    .settings-index::-webkit-scrollbar {
      display: none;
    }

    .settings-index a {
      flex: 0 0 auto;
      min-height: 36px;
      padding: 0 10px;
    }
  }

  @media (max-width: 520px) {
    .settings-page {
      padding: 20px 12px 34px;
    }

    .settings-heading {
      margin-bottom: 18px;
    }

    .settings-section > header {
      padding: 16px;
    }

    .setting-row {
      min-height: 72px;
      gap: 8px 10px;
      padding: 13px 16px;
    }

    .setting-row strong {
      font-size: 12px;
    }

    .setting-row small {
      font-size: 9px;
    }

    .row-value {
      max-width: 110px;
      font-size: 9px;
    }

    .policy-badge {
      padding: 5px 7px;
    }

    .cache-row {
      grid-template-columns: 1fr;
    }

    .storage-health-row {
      grid-template-columns: 1fr;
      gap: 9px;
      padding: 14px 16px;
    }

    .storage-health-row > span {
      justify-self: start;
    }

    .cache-limit-row {
      grid-template-columns: 1fr;
    }

    .update-status-row {
      grid-template-columns: 1fr;
    }

    .update-check-button {
      width: 100%;
    }

    .cache-limit-row select {
      width: 100%;
    }

    .cache-control {
      justify-content: space-between;
    }

    .export-destination-control {
      max-width: 100%;
      justify-content: flex-start;
      flex-wrap: wrap;
    }

    .export-destination-control > span {
      width: 100%;
      max-width: none;
      white-space: normal;
      overflow-wrap: anywhere;
    }

    .cache-dialog {
      padding: 20px;
    }

    .cache-dialog-actions {
      align-items: stretch;
      flex-direction: column-reverse;
    }

    .cache-dialog-actions button {
      width: 100%;
    }
  }
</style>
