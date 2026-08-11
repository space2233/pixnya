<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import {
    currentAppLocale,
    m,
    readLanguagePreference,
    setLanguagePreference,
    type LanguagePreference,
  } from "$lib/i18n";
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
    standard: m.settings_connection_standard(),
    ech: m.settings_connection_ech(),
  };

  const localDataFailureLabels: Record<LocalDataClearFailure, string> = {
    secure_storage: m.settings_failure_secure_storage(),
    session: m.settings_failure_session(),
    login_state: m.settings_failure_login_state(),
    transport_state: m.settings_failure_transport_state(),
    offline_library: m.settings_failure_offline_library(),
    media_cache: m.settings_failure_media_cache(),
    login_web_view: m.settings_failure_login_webview(),
    diagnostic_log: m.settings_failure_diagnostic_log(),
    download_queue: m.settings_failure_download_queue(),
    storage_settings: m.settings_failure_storage(),
    export_settings: m.settings_failure_export(),
    update_settings: m.settings_failure_updates(),
    local_catalog: m.settings_failure_catalog(),
    browsing_history: m.settings_failure_history(),
  };

  const LOCAL_DATA_CLEAR_PROTOCOL = "CLEAR_LOCAL_DATA";

  const cacheLimitOptions = [
    { bytes: 128 * 1024 ** 2, label: "128 MiB" },
    { bytes: 256 * 1024 ** 2, label: "256 MiB" },
    { bytes: 512 * 1024 ** 2, label: "512 MiB" },
    { bytes: 1024 ** 3, label: "1 GiB" },
  ] as const;

  let appStatus = $state<AppStatus | null>(null);
  let preferredConnectionMode = $state<PreferredConnectionMode>("standard");
  let languagePreference = $state<LanguagePreference>("system");
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
    languagePreference = readLanguagePreference();
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
      const selection = await selectExportDestination(m.settings_export_directory());
      exportDestination = selection.status;
      if (!selection.cancelled) {
        exportDestinationNotice = m.settings_export_authorized();
      }
    } catch {
      exportDestinationNotice = m.settings_export_authorize_failed();
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
      exportDestinationNotice = m.settings_export_removed();
    } catch {
      exportDestinationNotice = m.settings_export_remove_failed();
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
        ? m.settings_auto_export_on()
        : m.settings_auto_export_off();
    } catch {
      exportDestinationNotice = m.settings_auto_export_failed();
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
      storageNotice = m.settings_cache_limit_saved({ size: formatBytes(cacheLimitBytes) });
    } catch {
      storageNotice = m.settings_cache_limit_failed();
      storageNoticeIsError = true;
      await Promise.all([loadStorageStatus(), loadMediaCacheStats()]);
    } finally {
      isSavingCacheLimit = false;
    }
  }

  function updateLanguage(event: Event) {
    const preference = (event.currentTarget as HTMLSelectElement).value as LanguagePreference;
    setLanguagePreference(preference);
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
      updateNotice = m.settings_update_preferences_saved();
    } catch {
      updateNotice = m.settings_update_preferences_failed();
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
          updateNotice = m.settings_update_found_notice({ version: updateSnapshot.available?.version ?? m.common_new_version() });
          break;
        case "up_to_date":
          updateNotice = m.settings_update_latest_notice();
          break;
        case "not_configured":
          updateNotice = m.settings_update_not_configured_notice();
          break;
        case "failed":
          updateNotice = describeUpdateFailure(updateSnapshot);
          updateNoticeIsError = true;
          break;
        default:
          updateNotice = m.settings_update_check_complete();
      }
    } catch {
      updateNotice = m.settings_update_check_failed();
      updateNoticeIsError = true;
    } finally {
      isCheckingUpdates = false;
    }
  }

  async function downloadApplicationUpdate() {
    if (!updateSnapshot || isApplyingUpdate) return;
    isApplyingUpdate = true;
    updateNotice = m.settings_update_downloading_notice();
    updateNoticeIsError = false;
    try {
      updateSnapshot = await downloadUpdate();
      if (updateSnapshot.phase === "ready_to_install") {
        updateNotice = m.settings_update_ready_notice();
      } else if (updateSnapshot.phase === "failed") {
        updateNotice = describeUpdateFailure(updateSnapshot);
        updateNoticeIsError = true;
      }
    } catch {
      updateNotice = m.settings_update_download_failed();
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
      ? m.settings_update_opening_android()
      : m.settings_update_opening_signed();
    updateNoticeIsError = false;
    try {
      updateSnapshot = await installUpdate();
      if (updateSnapshot.phase === "awaiting_system_action") {
        updateNotice = updateSnapshot.readyToInstall
          ? m.settings_update_allow_install()
          : m.settings_update_system_opened();
      } else if (updateSnapshot.phase === "failed") {
        updateNotice = describeUpdateFailure(updateSnapshot);
        updateNoticeIsError = true;
      }
    } catch {
      updateNotice = m.settings_update_install_failed();
      updateNoticeIsError = true;
      await loadUpdateSnapshot();
    } finally {
      isApplyingUpdate = false;
    }
  }

  async function cancelApplicationUpdate() {
    updateNotice = m.settings_update_cancelling();
    updateNoticeIsError = false;
    try {
      updateSnapshot = await cancelUpdate();
      updateNotice = m.settings_update_cancelled_notice();
    } catch {
      updateNotice = m.settings_update_cancel_failed();
      updateNoticeIsError = true;
    }
  }

  function describeUpdateFailure(snapshot: UpdateSnapshot): string {
    switch (snapshot.failure) {
      case "busy":
        return m.settings_update_failure_busy();
      case "invalid_source_configuration":
        return m.settings_update_failure_source();
      case "network_or_manifest":
        return m.settings_update_failure_manifest();
      case "platform_unavailable":
        return m.settings_update_failure_platform();
      case "update_unavailable":
        return m.settings_update_failure_changed();
      case "download_verification":
        return m.settings_update_failure_verification();
      case "installation_unavailable":
        return m.settings_update_failure_installation();
      case "cancelled":
        return m.settings_update_failure_cancelled();
      default:
        return m.settings_update_failure_unknown();
    }
  }

  function describeUpdatePhase(snapshot: UpdateSnapshot | null): string {
    if (!snapshot) return m.settings_update_phase_reading();
    switch (snapshot.phase) {
      case "checking":
        return m.settings_update_phase_checking();
      case "available":
        return m.settings_update_phase_found({ version: snapshot.available?.version ?? m.common_new_version() });
      case "downloading":
        return m.settings_update_phase_downloading();
      case "ready_to_install":
        return m.settings_update_phase_ready();
      case "installing":
        return m.settings_update_phase_installing();
      case "awaiting_system_action":
        return snapshot.readyToInstall ? m.settings_update_phase_authorization() : m.settings_update_phase_confirmation();
      case "up_to_date":
        return m.settings_update_phase_latest();
      case "not_configured":
        return m.settings_update_phase_unconfigured();
      case "failed":
        return m.settings_update_phase_failed();
      default:
        return m.settings_update_phase_channel();
    }
  }

  function formatUpdateProgress(snapshot: UpdateSnapshot): string {
    const total = snapshot.totalBytes ?? snapshot.available?.sizeBytes ?? null;
    return total
      ? `${formatBytes(snapshot.downloadedBytes)} / ${formatBytes(total)}`
      : m.settings_update_downloaded({ size: formatBytes(snapshot.downloadedBytes) });
  }

  function formatUpdateCheckTime(value?: number | null): string {
    if (!value) return m.settings_update_never_checked();
    return new Date(value * 1000).toLocaleString(currentAppLocale());
  }

  async function toggleBrowsingHistory() {
    if (!browsingHistory || isSavingBrowsingHistory) return;
    isSavingBrowsingHistory = true;
    browsingHistoryNotice = null;
    browsingHistoryNoticeIsError = false;
    try {
      browsingHistory = await setBrowsingHistoryEnabled(!browsingHistory.enabled);
      browsingHistoryNotice = browsingHistory.enabled
        ? m.settings_history_enabled_notice()
        : m.settings_history_disabled_notice();
    } catch {
      browsingHistoryNotice = m.settings_history_save_failed();
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
      diagnosticLogNotice = m.settings_log_exported({ count: result.entryCount, destination: result.destination });
      await loadDiagnosticLogSummary();
    } catch {
      diagnosticLogNotice = m.settings_log_export_failed();
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
      diagnosticLogNotice = m.settings_log_cleared({ count: removed.entryCount });
      showClearDiagnosticLogDialog = false;
      await loadDiagnosticLogSummary();
    } catch {
      diagnosticLogNotice = m.settings_log_clear_failed();
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
      cacheNotice = m.settings_cache_cleared({ count: removed.entryCount, size: formatBytes(removed.sizeBytes) });
      showClearCacheDialog = false;
      await Promise.all([loadMediaCacheStats(), loadStorageStatus()]);
    } catch {
      cacheNotice = m.settings_cache_clear_failed();
    } finally {
      isClearingCache = false;
    }
  }

  function openClearLocalDataDialog() {
    localDataConfirmation = "";
    showClearLocalDataDialog = true;
  }

  async function confirmClearAllLocalData() {
    if (localDataConfirmation !== m.settings_clear_confirmation_word()) return;
    isClearingLocalData = true;
    localDataNotice = null;
    localDataNoticeIsError = false;
    try {
      const report = await clearLocalData(LOCAL_DATA_CLEAR_PROTOCOL);
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

      const removed = m.settings_local_data_removed_summary({
        tasks: report.downloadTasksRemoved,
        offline: report.offlineEntriesRemoved,
        collections: report.localCollectionsRemoved,
        tags: report.localTagsRemoved,
        history: report.browsingHistoryEntriesRemoved,
        cache: report.cacheEntriesRemoved,
        logs: report.diagnosticLogEntriesRemoved,
        frontend: frontend.localKeysRemoved + frontend.sessionKeysRemoved,
      });
      if (report.complete) {
        localDataNotice = m.settings_local_data_cleared({ summary: removed });
      } else {
        const failures = report.failedSteps.map((step) => localDataFailureLabels[step]).join(", ");
        localDataNotice = m.settings_local_data_partial({ failures });
        localDataNoticeIsError = true;
      }
    } catch {
      localDataNotice = m.settings_local_data_failed();
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
  <title>{m.settings_title()} · PixNya</title>
</svelte:head>

<AppShell title={m.settings_title()}>
  <div class="settings-page">
    <header class="settings-heading">
      <div>
        <span>PIXNYA</span>
        <h1>{m.settings_title()}</h1>
        <p>{m.settings_description()}</p>
      </div>
      <div class="runtime-state" class:online={appStatus !== null}>
        <i></i>
        <span>{appStatus ? `${appStatus.platform} · ${appStatus.architecture}` : m.settings_core_unavailable()}</span>
      </div>
    </header>

    <div class="settings-layout">
      <nav class="settings-index" aria-label={m.settings_categories()}>
        <a href="#account"><Icon name="user" size={18} />{m.settings_account()}</a>
        <a href="#connection"><Icon name="shield" size={18} />{m.settings_connection()}</a>
        <a href="#interface"><Icon name="settings" size={18} />{m.settings_interface()}</a>
        <a href="#storage"><Icon name="image" size={18} />{m.settings_storage()}</a>
        <a href="#updates"><Icon name="download" size={18} />{m.settings_updates()}</a>
        <a href="#privacy"><Icon name="shield" size={18} />{m.settings_privacy()}</a>
      </nav>

      <div class="settings-sections">
        <section id="account" class="settings-section">
          <header>
            <span><Icon name="user" size={20} /></span>
            <div><h2>{m.settings_account()}</h2><p>{m.settings_account_description()}</p></div>
          </header>
          <div class="setting-list">
            <a class="setting-row" href="/profile">
              <div><strong>{m.settings_pixiv_account()}</strong><small>{m.settings_pixiv_account_description()}</small></div>
              <span class="row-value muted">{$session.loggedIn ? ($session.user?.name ?? m.settings_logged_in()) : m.settings_logged_out()}</span><i>›</i>
            </a>
            <a class="setting-row" href={`/login?mode=${preferredConnectionMode}`}>
              <div><strong>{m.settings_web_login()}</strong><small>{m.settings_web_login_description()}</small></div>
              <span class="row-value">{m.settings_use_connection({ mode: connectionLabels[preferredConnectionMode] })}</span><i>›</i>
            </a>
          </div>
        </section>

        <section id="connection" class="settings-section">
          <header>
            <span class="safe"><Icon name="shield" size={20} /></span>
            <div><h2>{m.settings_connection()}</h2><p>{m.settings_connection_description()}</p></div>
          </header>
          <div class="setting-list">
            <a class="setting-row" href="/settings/network">
              <div><strong>{m.settings_default_connection()}</strong><small>{m.settings_default_connection_description()}</small></div>
              <span class="row-value accent">{connectionLabels[preferredConnectionMode]}</span><i>›</i>
            </a>
            <div class="setting-row static-row">
              <div><strong>{m.settings_login_tls()}</strong><small>{m.settings_login_tls_description()}</small></div>
              <span class="policy-badge">{m.settings_always_verify()}</span>
            </div>
            <div class="setting-row static-row">
              <div><strong>{m.settings_low_security()}</strong><small>{m.settings_low_security_description()}</small></div>
              <span class="policy-badge warning">{m.settings_temporary()}</span>
            </div>
          </div>
        </section>

        <section id="interface" class="settings-section">
          <header>
            <span><Icon name="settings" size={20} /></span>
            <div><h2>{m.settings_interface()}</h2><p>{m.settings_interface_description()}</p></div>
          </header>
          <div class="setting-list">
            <div class="setting-row control-row language-row">
              <div>
                <strong>{m.language_settings_title()}</strong>
                <small>{m.language_settings_description()}</small>
              </div>
              <select
                aria-label={m.language_settings_current()}
                value={languagePreference}
                onchange={updateLanguage}
              >
                <option value="system">{m.language_system()}</option>
                <option value="zh-CN">{m.language_simplified_chinese()}</option>
                <option value="zh-TW">{m.language_traditional_chinese()}</option>
                <option value="en-US">{m.language_english()}</option>
              </select>
            </div>
            <div class="setting-row control-row">
              <div><strong>{m.settings_sidebar()}</strong><small>{m.settings_sidebar_description()}</small></div>
              <button
                class="switch"
                class:on={desktopSidebarExpanded}
                type="button"
                role="switch"
                aria-checked={desktopSidebarExpanded}
                aria-label={m.settings_sidebar()}
                onclick={toggleDesktopSidebar}
              ><span></span></button>
            </div>
            <div class="setting-row control-row">
              <div><strong>{m.settings_reduced_motion()}</strong><small>{m.settings_reduced_motion_description()}</small></div>
              <button
                class="switch"
                class:on={reducedMotion}
                type="button"
                role="switch"
                aria-checked={reducedMotion}
                aria-label={m.settings_reduced_motion()}
                onclick={toggleReducedMotion}
              ><span></span></button>
            </div>
            <div class="setting-row control-row">
              <div><strong>{m.settings_r18()}</strong><small>{m.settings_r18_description()}</small></div>
              <button
                class="switch"
                class:on={$r18DefaultVisible}
                type="button"
                role="switch"
                aria-checked={$r18DefaultVisible}
                aria-label={m.settings_r18()}
                onclick={toggleR18DefaultVisible}
              ><span></span></button>
            </div>
          </div>
        </section>

        <section id="storage" class="settings-section">
          <header>
            <span><Icon name="image" size={20} /></span>
            <div><h2>{m.settings_storage()}</h2><p>{m.settings_storage_description()}</p></div>
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
                    ? m.settings_storage_critical()
                    : storageStatus?.health === "low"
                      ? m.settings_storage_low()
                      : storageStatus
                        ? m.settings_storage_healthy()
                        : m.settings_storage_reading()}
                </strong>
                <small>
                  {storageStatus
                    ? m.settings_storage_summary({ writable: formatBytes(storageStatus.writableDownloadBytes), offline: formatBytes(storageStatus.offlineBytes), reserve: formatBytes(storageStatus.reserveBytes) })
                    : m.settings_storage_checking()}
                </small>
              </div>
              <span>{storageStatus ? m.settings_storage_available({ size: formatBytes(storageStatus.dataAvailableBytes) }) : m.settings_reading()}</span>
            </div>
            <div class="setting-row static-row">
              <div><strong>{m.settings_content_scope()}</strong><small>{m.settings_content_scope_description()}</small></div>
              <span class="row-value">{m.settings_follow_pixiv()}</span>
            </div>
            <div class="setting-row static-row">
              <div><strong>{m.settings_online_media()}</strong><small>{m.settings_online_media_description()}</small></div>
              <span class="policy-badge">{m.settings_controlled_loading()}</span>
            </div>
            <div class="setting-row cache-row">
              <div>
                <strong>{m.settings_media_cache()}</strong>
                <small>
                  {mediaCacheStats
                    ? m.settings_media_cache_summary({ verified: formatBytes(mediaCacheStats.verifiedBytes), insecure: formatBytes(mediaCacheStats.insecureBytes), limit: formatBytes(mediaCacheStats.maxBytes) })
                    : m.settings_media_cache_reading()}
                </small>
              </div>
              <div class="cache-control">
                <span>{mediaCacheStats ? m.settings_item_size({ count: mediaCacheStats.entryCount, size: formatBytes(mediaCacheStats.sizeBytes) }) : m.settings_reading()}</span>
                <button type="button" disabled={!mediaCacheStats || isClearingCache} onclick={() => (showClearCacheDialog = true)}>{m.settings_clear_cache()}</button>
              </div>
            </div>
            <div class="setting-row control-row cache-limit-row">
              <div>
                <strong>{m.settings_cache_limit()}</strong>
                <small>{m.settings_cache_limit_description()}</small>
              </div>
              <select
                aria-label={m.settings_cache_limit()}
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
                <strong>{m.settings_export_directory()}</strong>
                <small>
                  {exportDestination?.configured
                    ? `${exportDestination.kind === "android_document_tree" ? m.settings_android_document_directory() : m.settings_system_directory()} · ${exportDestination.accessible ? m.settings_authorization_valid() : m.settings_permission_unavailable()}`
                    : exportDestination
                      ? m.settings_export_unselected()
                      : m.settings_export_reading()}
                </small>
              </div>
              <div class="cache-control export-destination-control">
                <span title={exportDestination?.label ?? ""}>
                  {exportDestination?.label ?? m.settings_private_directory()}
                </span>
                <button type="button" disabled={isSelectingExportDestination} onclick={chooseExportDestination}>
                  {isSelectingExportDestination ? m.common_processing() : exportDestination?.configured ? m.settings_change() : m.settings_choose_directory()}
                </button>
                {#if exportDestination?.configured}
                  <button class="secondary-action" type="button" disabled={isSelectingExportDestination} onclick={removeExportDestination}>{m.settings_revoke()}</button>
                {/if}
              </div>
            </div>
            <div class="setting-row control-row">
              <div>
                <strong>{m.settings_auto_export()}</strong>
                <small>{m.settings_auto_export_description()}</small>
              </div>
              <button
                class="switch"
                class:on={exportDestination?.autoExport ?? true}
                type="button"
                role="switch"
                aria-checked={exportDestination?.autoExport ?? true}
                aria-label={m.settings_auto_export()}
                disabled={!exportDestination || isSavingAutoExport}
                onclick={toggleAutoExportDownloads}
              ><span></span></button>
            </div>
            {#if exportDestinationNotice}
              <p class="local-data-notice" class:error={exportDestinationNoticeIsError} role="status">{exportDestinationNotice}</p>
            {/if}
            <a class="setting-row" href="/offline">
              <div><strong>{m.settings_offline_queue()}</strong><small>{m.settings_offline_queue_description()}</small></div>
              <span class="row-value">
                {offlineStats && downloadQueueStats
                  ? m.settings_offline_queue_summary({ active: downloadQueueStats.activeCount, count: offlineStats.entryCount, size: formatBytes(offlineStats.sizeBytes) })
                  : m.settings_reading()}
              </span><i>›</i>
            </a>
          </div>
        </section>

        <section id="updates" class="settings-section">
          <header>
            <span><Icon name="download" size={20} /></span>
            <div><h2>{m.settings_updates()}</h2><p>{m.settings_updates_description()}</p></div>
          </header>
          <div class="setting-list">
            <div class="setting-row update-status-row">
              <div>
                <strong>{describeUpdatePhase(updateSnapshot)}</strong>
                <small>
                  GitHub Releases · {updateSnapshot?.installer === "android_system" ? m.settings_android_installer() : m.settings_tauri_installer()}
                  · {formatUpdateCheckTime(updateSnapshot?.lastAttemptedAtUnixSeconds ?? updateSnapshot?.lastCheckedAtUnixSeconds)}
                </small>
              </div>
              <button
                class="update-check-button"
                type="button"
                disabled={!updateSnapshot || isCheckingUpdates || isApplyingUpdate || updateSnapshot.phase === "downloading" || updateSnapshot.phase === "installing"}
                onclick={checkForApplicationUpdate}
              >{isCheckingUpdates ? m.settings_checking() : m.settings_check_now()}</button>
            </div>
            <div class="setting-row control-row">
              <div><strong>{m.settings_auto_check()}</strong><small>{m.settings_auto_check_description()}</small></div>
              <button
                class="switch"
                class:on={updateSnapshot?.preferences.autoCheck ?? true}
                type="button"
                role="switch"
                aria-checked={updateSnapshot?.preferences.autoCheck ?? true}
                aria-label={m.settings_auto_check()}
                disabled={!updateSnapshot || isSavingUpdatePreferences}
                onclick={toggleAutomaticUpdateCheck}
              ><span></span></button>
            </div>
            <div class="setting-row control-row">
              <div><strong>{m.settings_auto_download()}</strong><small>{m.settings_auto_download_description()}</small></div>
              <button
                class="switch"
                class:on={updateSnapshot?.preferences.autoDownload ?? false}
                type="button"
                role="switch"
                aria-checked={updateSnapshot?.preferences.autoDownload ?? false}
                aria-label={m.settings_auto_download()}
                disabled={!updateSnapshot || !updateSnapshot.sourceConfigured || isSavingUpdatePreferences}
                onclick={toggleAutomaticUpdateDownload}
              ><span></span></button>
            </div>
            {#if updateSnapshot?.installer === "android_system"}
              <div class="setting-row control-row">
                <div><strong>{m.settings_unmetered()}</strong><small>{m.settings_unmetered_description()}</small></div>
                <button
                  class="switch"
                  class:on={updateSnapshot.preferences.unmeteredOnly}
                  type="button"
                  role="switch"
                  aria-checked={updateSnapshot.preferences.unmeteredOnly}
                  aria-label={m.settings_unmetered()}
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
                <p>{updateSnapshot.available.notes || m.settings_no_release_notes()}</p>
                {#if updateSnapshot.phase === "downloading"}
                  <div class="update-progress" role="progressbar" aria-valuemin="0" aria-valuemax={updateSnapshot.totalBytes ?? undefined} aria-valuenow={updateSnapshot.downloadedBytes}>
                    <span style={`width: ${updateSnapshot.totalBytes ? Math.min(100, updateSnapshot.downloadedBytes / updateSnapshot.totalBytes * 100) : 0}%`}></span>
                  </div>
                  <small>{formatUpdateProgress(updateSnapshot)}</small>
                {/if}
                <div class="update-actions">
                  {#if updateSnapshot.phase === "available" || updateSnapshot.phase === "failed"}
                    <button type="button" disabled={isApplyingUpdate || !updateSnapshot.sourceConfigured} onclick={downloadApplicationUpdate}>{m.settings_download_verify()}</button>
                  {/if}
                  {#if updateSnapshot.phase === "ready_to_install" || (updateSnapshot.phase === "awaiting_system_action" && updateSnapshot.readyToInstall)}
                    <button class="primary" type="button" disabled={isApplyingUpdate} onclick={installApplicationUpdate}>
                      {updateSnapshot.phase === "awaiting_system_action" ? m.settings_continue_install() : updateSnapshot.installer === "android_system" ? m.settings_open_installer() : m.settings_install_update()}
                    </button>
                  {/if}
                  {#if updateSnapshot.phase === "downloading"}
                    <button type="button" onclick={cancelApplicationUpdate}>{m.settings_cancel_download()}</button>
                  {:else if updateSnapshot.phase === "ready_to_install"}
                    <button type="button" disabled={isApplyingUpdate} onclick={cancelApplicationUpdate}>{m.settings_delete_update()}</button>
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
            <div><h2>{m.settings_privacy()}</h2><p>{m.settings_privacy_description()}</p></div>
          </header>
          <div class="setting-list">
            <div class="setting-row static-row">
              <div><strong>{m.settings_telemetry()}</strong><small>{m.settings_telemetry_description()}</small></div>
              <span class="policy-badge">{m.settings_off()}</span>
            </div>
            <div class="setting-row control-row">
              <div>
                <strong>{m.settings_history()}</strong>
                <small>
                  {browsingHistory
                    ? m.settings_history_summary({ count: browsingHistory.entries.length, limit: browsingHistory.limit })
                    : m.settings_history_reading()}
                  · <a class="inline-link" href="/history">{m.settings_manage_history()}</a>
                </small>
              </div>
              <button
                class="switch"
                class:on={browsingHistory?.enabled ?? false}
                type="button"
                role="switch"
                aria-label={m.settings_history_aria()}
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
                <strong>{m.settings_diagnostic_log()}</strong>
                <small>
                  {diagnosticLogSummary
                    ? m.settings_diagnostic_summary({ count: diagnosticLogSummary.entryCount, size: formatBytes(diagnosticLogSummary.retainedBytes), days: diagnosticLogSummary.retentionDays, limit: formatBytes(diagnosticLogSummary.maxBytes) })
                    : m.settings_diagnostic_reading()}
                  · {m.settings_diagnostic_exclusions()}
                </small>
              </div>
              <div class="cache-control diagnostic-log-control">
                <button type="button" disabled={!diagnosticLogSummary || isExportingDiagnosticLog} onclick={exportLocalDiagnosticLog}>
                  {isExportingDiagnosticLog ? m.offline_exporting() : m.settings_export_log()}
                </button>
                <button type="button" disabled={!diagnosticLogSummary || isClearingDiagnosticLog} onclick={() => (showClearDiagnosticLogDialog = true)}>{m.settings_clear_log()}</button>
              </div>
            </div>
            {#if diagnosticLogNotice}
              <p class="local-data-notice" class:error={diagnosticLogNoticeIsError} role="status">{diagnosticLogNotice}</p>
            {/if}
            <div class="setting-row danger-row">
              <div><strong>{m.settings_clear_all()}</strong><small>{m.settings_clear_all_description()}</small></div>
              <button type="button" disabled={isClearingLocalData} onclick={openClearLocalDataDialog}>{m.settings_clear_data()}</button>
            </div>
            {#if localDataNotice}
              <p class="local-data-notice" class:error={localDataNoticeIsError} role="status">{localDataNotice}</p>
            {/if}
            <div class="setting-row static-row">
              <div><strong>{m.settings_version()}</strong><small>{m.settings_app_nature()}</small></div>
              <span class="row-value">PixNya {appStatus?.version ?? "1.0.0"}</span>
            </div>
          </div>
        </section>

        <p class="settings-legal">{m.settings_legal()}</p>
      </div>
    </div>
  </div>
</AppShell>

{#if showClearCacheDialog}
  <div class="cache-dialog-layer">
    <button class="cache-dialog-scrim" type="button" aria-label={m.settings_cancel_cache_clear()} onclick={() => (showClearCacheDialog = false)}></button>
    <div role="alertdialog" aria-modal="true" aria-labelledby="cache-dialog-title" class="cache-dialog">
      <span><Icon name="image" size={22} /></span>
      <div>
        <small>{m.settings_cache_dialog_eyebrow()}</small>
        <h2 id="cache-dialog-title">{m.settings_cache_dialog_title()}</h2>
      </div>
      <p>{m.settings_cache_dialog_description()}</p>
      <div class="cache-dialog-actions">
        <button type="button" disabled={isClearingCache} onclick={() => (showClearCacheDialog = false)}>{m.common_cancel()}</button>
        <button class="confirm-clear" type="button" disabled={isClearingCache} onclick={confirmClearMediaCache}>{isClearingCache ? m.settings_clearing_cache() : m.settings_confirm_cache_clear()}</button>
      </div>
    </div>
  </div>
{/if}

{#if showClearDiagnosticLogDialog}
  <div class="cache-dialog-layer">
    <button class="cache-dialog-scrim" type="button" aria-label={m.settings_cancel_log_clear()} disabled={isClearingDiagnosticLog} onclick={() => (showClearDiagnosticLogDialog = false)}></button>
    <div role="alertdialog" aria-modal="true" aria-labelledby="diagnostic-log-dialog-title" class="cache-dialog">
      <span><Icon name="shield" size={22} /></span>
      <div>
        <small>{m.settings_log_dialog_eyebrow()}</small>
        <h2 id="diagnostic-log-dialog-title">{m.settings_log_dialog_title()}</h2>
      </div>
      <p>{m.settings_log_dialog_description({ count: diagnosticLogSummary?.entryCount ?? 0 })}</p>
      <div class="cache-dialog-actions">
        <button type="button" disabled={isClearingDiagnosticLog} onclick={() => (showClearDiagnosticLogDialog = false)}>{m.common_cancel()}</button>
        <button class="confirm-clear" type="button" disabled={isClearingDiagnosticLog} onclick={confirmClearDiagnosticLog}>{isClearingDiagnosticLog ? m.settings_clearing() : m.settings_confirm_clear()}</button>
      </div>
    </div>
  </div>
{/if}

{#if showClearLocalDataDialog}
  <div class="cache-dialog-layer">
    <button
      class="cache-dialog-scrim"
      type="button"
      aria-label={m.settings_cancel_local_clear()}
      disabled={isClearingLocalData}
      onclick={() => (showClearLocalDataDialog = false)}
    ></button>
    <div role="alertdialog" aria-modal="true" aria-labelledby="local-data-dialog-title" class="cache-dialog destructive-dialog">
      <span><Icon name="shield" size={22} /></span>
      <div>
        <small>{m.settings_irreversible()}</small>
        <h2 id="local-data-dialog-title">{m.settings_local_dialog_title()}</h2>
      </div>
      <p>{m.settings_local_dialog_description({
        offline: offlineStats?.entryCount ?? 0,
        cache: mediaCacheStats?.entryCount ?? 0,
        history: browsingHistory?.entries.length ?? 0,
        logs: diagnosticLogSummary?.entryCount ?? 0,
      })}</p>
      <label class="confirmation-field">
        <span>{m.settings_clear_confirmation_prompt({ word: m.settings_clear_confirmation_word() })}</span>
        <input
          bind:value={localDataConfirmation}
          autocomplete="off"
          spellcheck="false"
          disabled={isClearingLocalData}
          placeholder={m.settings_clear_confirmation_word()}
        />
      </label>
      <div class="cache-dialog-actions">
        <button type="button" disabled={isClearingLocalData} onclick={() => (showClearLocalDataDialog = false)}>{m.common_cancel()}</button>
        <button
          class="confirm-clear destructive"
          type="button"
          disabled={isClearingLocalData || localDataConfirmation !== m.settings_clear_confirmation_word()}
          onclick={confirmClearAllLocalData}
        >{isClearingLocalData ? m.settings_clearing() : m.settings_permanent_clear()}</button>
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

  .cache-limit-row select,
  .language-row select {
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

  .cache-limit-row select:focus,
  .language-row select:focus {
    border-color: var(--pixiv-blue);
    outline: 3px solid rgba(0, 150, 250, 0.1);
  }

  .cache-limit-row select:disabled,
  .language-row select:disabled {
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

    .cache-limit-row select,
    .language-row select {
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
