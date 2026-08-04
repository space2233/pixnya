use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "android")]
use crate::android_update::{AndroidUpdateError, PreparedAndroidUpdate};
#[cfg(not(target_os = "android"))]
use crate::desktop_update::{DesktopUpdateError, PreparedDesktopUpdate};

#[cfg(target_os = "android")]
#[allow(dead_code)]
pub struct AndroidUpdateInstallerPlugin(pub tauri::plugin::PluginHandle<tauri::Wry>);

#[cfg(target_os = "android")]
pub fn android_update_installer_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("update_installer")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("io.github.space2233.pixnya", "UpdateInstallerPlugin")?;
            app.manage(AndroidUpdateInstallerPlugin(handle));
            Ok(())
        })
        .build()
}

const SETTINGS_DIRECTORY: &str = "updates";
const SETTINGS_FILE: &str = "state.json";
const SETTINGS_STAGING_FILE: &str = "state.next.json";
const SETTINGS_BACKUP_FILE: &str = "state.previous.json";
const UPDATE_STATE_SCHEMA_VERSION: u32 = 1;
const AUTOMATIC_CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateTrigger {
    Startup,
    Scheduled,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    NotConfigured,
    Checking,
    UpToDate,
    Available,
    Downloading,
    ReadyToInstall,
    Installing,
    AwaitingSystemAction,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateInstaller {
    DesktopTauri,
    AndroidSystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateFailure {
    Busy,
    InvalidSourceConfiguration,
    #[allow(dead_code)]
    NetworkOrManifest,
    LocalStateUnavailable,
    PlatformUnavailable,
    UpdateUnavailable,
    DownloadVerification,
    #[allow(dead_code)]
    InstallationUnavailable,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferences {
    pub auto_check: bool,
    pub auto_download: bool,
    pub unmetered_only: bool,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            auto_check: true,
            auto_download: false,
            unmetered_only: cfg!(target_os = "android"),
        }
    }
}

impl UpdatePreferences {
    fn normalized(mut self) -> Self {
        if !self.auto_check {
            self.auto_download = false;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSnapshot {
    pub current_version: String,
    pub source: &'static str,
    pub source_configured: bool,
    pub installer: UpdateInstaller,
    pub preferences: UpdatePreferences,
    pub phase: UpdatePhase,
    pub ready_to_install: bool,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub last_attempted_at_unix_seconds: Option<u64>,
    pub last_checked_at_unix_seconds: Option<u64>,
    pub available: Option<AvailableUpdate>,
    pub failure: Option<UpdateFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredUpdateState {
    schema_version: u32,
    preferences: UpdatePreferences,
    #[serde(default)]
    last_attempted_at_unix_seconds: Option<u64>,
    last_checked_at_unix_seconds: Option<u64>,
    available: Option<AvailableUpdate>,
}

impl Default for StoredUpdateState {
    fn default() -> Self {
        Self {
            schema_version: UPDATE_STATE_SCHEMA_VERSION,
            preferences: UpdatePreferences::default(),
            last_attempted_at_unix_seconds: None,
            last_checked_at_unix_seconds: None,
            available: None,
        }
    }
}

impl StoredUpdateState {
    fn fail_closed() -> Self {
        let mut state = Self::default();
        state.preferences.auto_check = false;
        state.preferences.auto_download = false;
        state
    }
}

pub struct UpdateManagerState {
    runtime: Arc<Mutex<UpdateRuntimeState>>,
    cancelled: Arc<AtomicBool>,
    #[cfg(not(target_os = "android"))]
    prepared: Mutex<Option<PreparedDesktopUpdate>>,
    #[cfg(target_os = "android")]
    prepared: Mutex<Option<PreparedAndroidUpdate>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateActivity {
    Checking,
    Downloading,
    Installing,
    Clearing,
}

#[derive(Default)]
struct UpdateRuntimeState {
    active: Option<UpdateActivity>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    ready_version: Option<String>,
    awaiting_system_action: bool,
}

impl Default for UpdateManagerState {
    fn default() -> Self {
        Self {
            runtime: Arc::new(Mutex::new(UpdateRuntimeState::default())),
            cancelled: Arc::new(AtomicBool::new(false)),
            prepared: Mutex::new(None),
        }
    }
}

struct OperationGuard {
    runtime: Arc<Mutex<UpdateRuntimeState>>,
    activity: UpdateActivity,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut runtime) = self.runtime.lock() {
            if runtime.active == Some(self.activity) {
                runtime.active = None;
            }
        }
    }
}

#[cfg(target_os = "android")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidInstallStatus {
    can_request_package_installs: bool,
    requires_user_confirmation: bool,
    awaiting_system_action: bool,
    sdk_int: u32,
    active_network_metered: bool,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidInstallPayload {
    apk_path: String,
    expected_version_code: u64,
    expected_size: u64,
    expected_sha256: String,
    expected_certificate_sha256: String,
    expected_abi: String,
}

#[derive(Clone)]
struct UpdateSourceConfig {
    endpoint: String,
    public_key: String,
}

impl UpdateSourceConfig {
    fn from_build() -> Option<Self> {
        #[cfg(not(target_os = "android"))]
        let values = (
            option_env!("PIXNYA_UPDATER_ENDPOINT"),
            option_env!("PIXNYA_UPDATER_PUBKEY"),
        );
        #[cfg(target_os = "android")]
        let values = (
            option_env!("PIXNYA_ANDROID_UPDATE_MANIFEST"),
            option_env!("PIXNYA_ANDROID_UPDATE_PUBKEY"),
        );

        let endpoint = values.0?.trim();
        let public_key = values.1?.trim();
        if endpoint.is_empty() || public_key.is_empty() {
            return None;
        }
        Some(Self {
            endpoint: endpoint.to_owned(),
            public_key: public_key.to_owned(),
        })
    }

    fn validate(&self) -> bool {
        tauri::Url::parse(&self.endpoint).ok().is_some_and(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("github.com")
                && url.port().is_none()
                && url.username().is_empty()
                && url.password().is_none()
                && (url
                    .path()
                    .starts_with("/space2233/pixnya/releases/download/")
                    || url
                        .path()
                        .starts_with("/space2233/pixnya/releases/latest/download/"))
        }) && !self.public_key.trim().is_empty()
    }
}

#[tauri::command]
pub fn get_update_snapshot(
    app: AppHandle,
    manager: tauri::State<'_, UpdateManagerState>,
) -> UpdateSnapshot {
    let stored = match load_state(&app) {
        Ok(stored) => stored,
        Err(failure) => {
            return snapshot(
                &app,
                manager.inner(),
                StoredUpdateState::fail_closed(),
                UpdatePhase::Failed,
                Some(failure),
            );
        }
    };
    let config = UpdateSourceConfig::from_build();
    let phase = if config.as_ref().is_some_and(UpdateSourceConfig::validate) {
        if stored.available.is_some() {
            UpdatePhase::Available
        } else {
            UpdatePhase::Idle
        }
    } else {
        UpdatePhase::NotConfigured
    };
    snapshot(&app, manager.inner(), stored, phase, None)
}

#[tauri::command]
pub fn set_update_preferences(
    preferences: UpdatePreferences,
    app: AppHandle,
    manager: tauri::State<'_, UpdateManagerState>,
) -> Result<UpdateSnapshot, UpdateFailure> {
    let mut stored = load_state(&app)?;
    stored.preferences = preferences.normalized();
    persist_state(&app, &stored)?;
    let configured = UpdateSourceConfig::from_build()
        .as_ref()
        .is_some_and(UpdateSourceConfig::validate);
    let phase = if !configured {
        UpdatePhase::NotConfigured
    } else if stored.available.is_some() {
        UpdatePhase::Available
    } else {
        UpdatePhase::Idle
    };
    let should_download = stored.preferences.auto_download && stored.available.is_some();
    let result = snapshot(&app, manager.inner(), stored, phase, None);
    if should_download {
        schedule_automatic_download(app.clone());
    }
    Ok(result)
}

#[tauri::command]
pub async fn check_for_updates(
    trigger: UpdateTrigger,
    app: AppHandle,
    manager: tauri::State<'_, UpdateManagerState>,
) -> Result<UpdateSnapshot, UpdateFailure> {
    Ok(check(&app, manager.inner(), trigger).await)
}

pub fn start_startup_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let manager = app.state::<UpdateManagerState>();
        let _ = check(&app, manager.inner(), UpdateTrigger::Startup).await;
    });
}

async fn check(
    app: &AppHandle,
    manager: &UpdateManagerState,
    trigger: UpdateTrigger,
) -> UpdateSnapshot {
    let mut stored = match load_state(app) {
        Ok(stored) => stored,
        Err(failure) => {
            return snapshot(
                app,
                manager,
                StoredUpdateState::fail_closed(),
                UpdatePhase::Failed,
                Some(failure),
            );
        }
    };
    if trigger != UpdateTrigger::Manual && !automatic_check_due(&stored, unix_seconds()) {
        let phase = if stored.available.is_some() {
            UpdatePhase::Available
        } else {
            UpdatePhase::Idle
        };
        return snapshot(app, manager, stored, phase, None);
    }

    let config = match UpdateSourceConfig::from_build() {
        Some(config) if config.validate() => config,
        Some(_) => {
            return snapshot(
                app,
                manager,
                stored,
                UpdatePhase::Failed,
                Some(UpdateFailure::InvalidSourceConfiguration),
            )
        }
        None => return snapshot(app, manager, stored, UpdatePhase::NotConfigured, None),
    };

    let guard = match begin_operation(manager, UpdateActivity::Checking) {
        Ok(guard) => guard,
        Err(failure) => {
            return snapshot(app, manager, stored, UpdatePhase::Failed, Some(failure));
        }
    };

    let attempted_at = unix_seconds();
    stored.last_attempted_at_unix_seconds = Some(attempted_at);
    if persist_state(app, &stored).is_err() {
        return snapshot(
            app,
            manager,
            stored,
            UpdatePhase::Failed,
            Some(UpdateFailure::LocalStateUnavailable),
        );
    }

    let result = platform_check(app, &config).await;
    drop(guard);
    match result {
        Ok(available) => {
            stored.last_checked_at_unix_seconds = Some(attempted_at);
            stored.available = available;
            if persist_state(app, &stored).is_err() {
                return snapshot(
                    app,
                    manager,
                    stored,
                    UpdatePhase::Failed,
                    Some(UpdateFailure::LocalStateUnavailable),
                );
            }
            let phase = if stored.available.is_some() {
                UpdatePhase::Available
            } else {
                UpdatePhase::UpToDate
            };
            let should_download = stored.preferences.auto_download && stored.available.is_some();
            let result = snapshot(app, manager, stored, phase, None);
            if should_download {
                schedule_automatic_download(app.clone());
            }
            result
        }
        Err(failure) => snapshot(app, manager, stored, UpdatePhase::Failed, Some(failure)),
    }
}

fn begin_operation(
    manager: &UpdateManagerState,
    activity: UpdateActivity,
) -> Result<OperationGuard, UpdateFailure> {
    let mut runtime = manager
        .runtime
        .lock()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    if runtime.active.is_some() {
        return Err(UpdateFailure::Busy);
    }
    runtime.active = Some(activity);
    if activity == UpdateActivity::Downloading {
        runtime.downloaded_bytes = 0;
        runtime.total_bytes = None;
        runtime.awaiting_system_action = false;
    }
    drop(runtime);
    Ok(OperationGuard {
        runtime: manager.runtime.clone(),
        activity,
    })
}

#[cfg(not(target_os = "android"))]
async fn platform_check(
    app: &AppHandle,
    config: &UpdateSourceConfig,
) -> Result<Option<AvailableUpdate>, UpdateFailure> {
    let update = crate::desktop_update::check(app, &config.endpoint, &config.public_key)
        .await
        .map_err(desktop_failure)?;
    Ok(update.map(|candidate| AvailableUpdate {
        version: candidate.summary.version,
        notes: candidate.summary.notes,
        published_at: candidate.summary.published_at,
        size_bytes: None,
    }))
}

#[cfg(target_os = "android")]
async fn platform_check(
    app: &AppHandle,
    config: &UpdateSourceConfig,
) -> Result<Option<AvailableUpdate>, UpdateFailure> {
    let status = android_install_status(app).await?;
    let endpoint = config.endpoint.clone();
    let public_key = config.public_key.clone();
    let current_version = app.package_info().version.to_string();
    let candidate = tauri::async_runtime::spawn_blocking(move || {
        crate::android_update::fetch_candidate(
            &endpoint,
            &public_key,
            &current_version,
            std::env::consts::ARCH,
            status.sdk_int,
        )
    })
    .await
    .map_err(|_| UpdateFailure::PlatformUnavailable)?
    .map_err(android_failure)?;
    Ok(candidate.map(|candidate| AvailableUpdate {
        version: candidate.version,
        notes: candidate.notes,
        published_at: candidate.published_at,
        size_bytes: Some(candidate.size),
    }))
}

#[tauri::command]
pub async fn download_update(
    app: AppHandle,
    manager: tauri::State<'_, UpdateManagerState>,
) -> Result<UpdateSnapshot, UpdateFailure> {
    Ok(download(&app, manager.inner(), false).await)
}

async fn download(
    app: &AppHandle,
    manager: &UpdateManagerState,
    automatic: bool,
) -> UpdateSnapshot {
    let stored = match load_state(app) {
        Ok(stored) => stored,
        Err(failure) => {
            return snapshot(
                app,
                manager,
                StoredUpdateState::fail_closed(),
                UpdatePhase::Failed,
                Some(failure),
            );
        }
    };
    let available = match stored.available.clone() {
        Some(available) => available,
        None => {
            return snapshot(
                app,
                manager,
                stored,
                UpdatePhase::Failed,
                Some(UpdateFailure::UpdateUnavailable),
            );
        }
    };
    let config = match UpdateSourceConfig::from_build() {
        Some(config) if config.validate() => config,
        _ => {
            return snapshot(
                app,
                manager,
                stored,
                UpdatePhase::Failed,
                Some(UpdateFailure::InvalidSourceConfiguration),
            );
        }
    };

    #[cfg(target_os = "android")]
    if automatic && stored.preferences.unmetered_only {
        match android_install_status(app).await {
            Ok(status) if status.active_network_metered => {
                return snapshot(app, manager, stored, UpdatePhase::Available, None);
            }
            Err(failure) => {
                return snapshot(app, manager, stored, UpdatePhase::Failed, Some(failure));
            }
            _ => {}
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = automatic;

    let guard = match begin_operation(manager, UpdateActivity::Downloading) {
        Ok(guard) => guard,
        Err(failure) => {
            return snapshot(app, manager, stored, UpdatePhase::Failed, Some(failure));
        }
    };
    manager.cancelled.store(false, Ordering::Release);
    clear_prepared(manager);

    let result = platform_download(app, manager, &config, &available).await;
    drop(guard);
    match result {
        Ok(()) => {
            mark_ready(manager, available.version.clone());
            snapshot(app, manager, stored, UpdatePhase::ReadyToInstall, None)
        }
        Err(failure) => {
            reset_download_progress(manager);
            snapshot(app, manager, stored, UpdatePhase::Failed, Some(failure))
        }
    }
}

#[cfg(not(target_os = "android"))]
async fn platform_download(
    app: &AppHandle,
    manager: &UpdateManagerState,
    config: &UpdateSourceConfig,
    available: &AvailableUpdate,
) -> Result<(), UpdateFailure> {
    let candidate = crate::desktop_update::check(app, &config.endpoint, &config.public_key)
        .await
        .map_err(desktop_failure)?
        .filter(|candidate| candidate.summary.version == available.version)
        .ok_or(UpdateFailure::UpdateUnavailable)?;
    let public_key = config.public_key.clone();
    let runtime = manager.runtime.clone();
    let cancelled = manager.cancelled.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        crate::desktop_update::download(candidate, &public_key, &cancelled, |downloaded, total| {
            update_progress(&runtime, downloaded, total);
        })
    })
    .await
    .map_err(|_| UpdateFailure::PlatformUnavailable)?
    .map_err(desktop_failure)?;
    *manager
        .prepared
        .lock()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)? = Some(prepared);
    Ok(())
}

#[cfg(target_os = "android")]
async fn platform_download(
    app: &AppHandle,
    manager: &UpdateManagerState,
    config: &UpdateSourceConfig,
    available: &AvailableUpdate,
) -> Result<(), UpdateFailure> {
    let status = android_install_status(app).await?;
    let endpoint = config.endpoint.clone();
    let public_key = config.public_key.clone();
    let current_version = app.package_info().version.to_string();
    let candidate = tauri::async_runtime::spawn_blocking(move || {
        crate::android_update::fetch_candidate(
            &endpoint,
            &public_key,
            &current_version,
            std::env::consts::ARCH,
            status.sdk_int,
        )
    })
    .await
    .map_err(|_| UpdateFailure::PlatformUnavailable)?
    .map_err(android_failure)?
    .filter(|candidate| candidate.version == available.version)
    .ok_or(UpdateFailure::UpdateUnavailable)?;
    let update_directory = app
        .path()
        .app_cache_dir()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?
        .join("updates");
    let runtime = manager.runtime.clone();
    let cancelled = manager.cancelled.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        crate::android_update::download_candidate(
            &candidate,
            &update_directory,
            &cancelled,
            |downloaded, total| update_progress(&runtime, downloaded, Some(total)),
        )
    })
    .await
    .map_err(|_| UpdateFailure::PlatformUnavailable)?
    .map_err(android_failure)?;
    *manager
        .prepared
        .lock()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)? = Some(prepared);
    Ok(())
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    manager: tauri::State<'_, UpdateManagerState>,
) -> Result<UpdateSnapshot, UpdateFailure> {
    let stored = load_state(&app)?;
    let guard = match begin_operation(manager.inner(), UpdateActivity::Installing) {
        Ok(guard) => guard,
        Err(failure) => {
            return Ok(snapshot(
                &app,
                manager.inner(),
                stored,
                UpdatePhase::Failed,
                Some(failure),
            ));
        }
    };
    let result = platform_install(&app, manager.inner()).await;
    drop(guard);
    match result {
        Ok(awaiting_system_action) => {
            let can_retry_install = manager
                .prepared
                .lock()
                .is_ok_and(|prepared| prepared.is_some());
            mark_awaiting_system_action(manager.inner(), awaiting_system_action, can_retry_install);
            Ok(snapshot(
                &app,
                manager.inner(),
                stored,
                if awaiting_system_action {
                    UpdatePhase::AwaitingSystemAction
                } else {
                    UpdatePhase::Idle
                },
                None,
            ))
        }
        Err(failure) => Ok(snapshot(
            &app,
            manager.inner(),
            stored,
            UpdatePhase::Failed,
            Some(failure),
        )),
    }
}

#[cfg(not(target_os = "android"))]
async fn platform_install(
    _app: &AppHandle,
    manager: &UpdateManagerState,
) -> Result<bool, UpdateFailure> {
    let prepared = manager
        .prepared
        .lock()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?
        .take()
        .ok_or(UpdateFailure::UpdateUnavailable)?;
    match crate::desktop_update::install(&prepared) {
        Ok(()) => Ok(true),
        Err(error) => {
            *manager
                .prepared
                .lock()
                .map_err(|_| UpdateFailure::LocalStateUnavailable)? = Some(prepared);
            Err(desktop_failure(error))
        }
    }
}

#[cfg(target_os = "android")]
async fn platform_install(
    app: &AppHandle,
    manager: &UpdateManagerState,
) -> Result<bool, UpdateFailure> {
    let prepared = manager
        .prepared
        .lock()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?
        .take()
        .ok_or(UpdateFailure::UpdateUnavailable)?;
    let status = android_install_status(app).await?;
    if !status.can_request_package_installs {
        let requested = app
            .state::<AndroidUpdateInstallerPlugin>()
            .0
            .clone()
            .run_mobile_plugin_async::<AndroidInstallStatus>("requestInstallPermission", ())
            .await;
        *manager
            .prepared
            .lock()
            .map_err(|_| UpdateFailure::LocalStateUnavailable)? = Some(prepared);
        return requested
            .map(|status| status.awaiting_system_action)
            .map_err(|_| UpdateFailure::InstallationUnavailable);
    }
    let payload = AndroidInstallPayload {
        apk_path: prepared.apk_path.to_string_lossy().into_owned(),
        expected_version_code: prepared.candidate.version_code,
        expected_size: prepared.candidate.size,
        expected_sha256: prepared.candidate.sha256.clone(),
        expected_certificate_sha256: prepared.candidate.certificate_sha256.clone(),
        expected_abi: prepared.candidate.abi.clone(),
    };
    match app
        .state::<AndroidUpdateInstallerPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<AndroidInstallStatus>("installApk", payload)
        .await
    {
        Ok(status) => Ok(status.awaiting_system_action),
        Err(_) => {
            *manager
                .prepared
                .lock()
                .map_err(|_| UpdateFailure::LocalStateUnavailable)? = Some(prepared);
            Err(UpdateFailure::InstallationUnavailable)
        }
    }
}

#[tauri::command]
pub fn cancel_update(
    app: AppHandle,
    manager: tauri::State<'_, UpdateManagerState>,
) -> Result<UpdateSnapshot, UpdateFailure> {
    manager.cancelled.store(true, Ordering::Release);
    let active = manager
        .runtime
        .lock()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?
        .active;
    if active.is_none() {
        clear_prepared(manager.inner());
        reset_update_runtime(manager.inner());
    }
    let stored = load_state(&app)?;
    let phase = if stored.available.is_some() {
        UpdatePhase::Available
    } else {
        UpdatePhase::Idle
    };
    Ok(snapshot(&app, manager.inner(), stored, phase, None))
}

fn schedule_automatic_download(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let manager = app.state::<UpdateManagerState>();
        let _ = download(&app, manager.inner(), true).await;
    });
}

fn clear_prepared(manager: &UpdateManagerState) {
    if let Ok(mut prepared) = manager.prepared.lock() {
        #[cfg(target_os = "android")]
        if let Some(prepared) = prepared.take() {
            let _ = fs::remove_file(prepared.apk_path);
        }
        #[cfg(not(target_os = "android"))]
        prepared.take();
    }
}

fn update_progress(
    runtime: &Arc<Mutex<UpdateRuntimeState>>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    if let Ok(mut runtime) = runtime.lock() {
        runtime.downloaded_bytes = downloaded_bytes;
        runtime.total_bytes = total_bytes;
    }
}

fn mark_ready(manager: &UpdateManagerState, version: String) {
    if let Ok(mut runtime) = manager.runtime.lock() {
        runtime.ready_version = Some(version);
        runtime.awaiting_system_action = false;
    }
}

fn mark_awaiting_system_action(
    manager: &UpdateManagerState,
    awaiting: bool,
    can_retry_install: bool,
) {
    if let Ok(mut runtime) = manager.runtime.lock() {
        runtime.awaiting_system_action = awaiting;
        if awaiting && !can_retry_install {
            runtime.ready_version = None;
        }
    }
}

fn reset_download_progress(manager: &UpdateManagerState) {
    if let Ok(mut runtime) = manager.runtime.lock() {
        runtime.downloaded_bytes = 0;
        runtime.total_bytes = None;
    }
}

fn reset_update_runtime(manager: &UpdateManagerState) {
    if let Ok(mut runtime) = manager.runtime.lock() {
        *runtime = UpdateRuntimeState::default();
    }
}

#[cfg(not(target_os = "android"))]
fn desktop_failure(error: DesktopUpdateError) -> UpdateFailure {
    match error {
        DesktopUpdateError::Cancelled => UpdateFailure::Cancelled,
        DesktopUpdateError::InvalidConfiguration => UpdateFailure::InvalidSourceConfiguration,
        DesktopUpdateError::Network => UpdateFailure::NetworkOrManifest,
        DesktopUpdateError::Platform => UpdateFailure::PlatformUnavailable,
        DesktopUpdateError::Verification => UpdateFailure::DownloadVerification,
    }
}

#[cfg(target_os = "android")]
fn android_failure(error: AndroidUpdateError) -> UpdateFailure {
    match error {
        AndroidUpdateError::Cancelled => UpdateFailure::Cancelled,
        AndroidUpdateError::InvalidManifest
        | AndroidUpdateError::InvalidSignature
        | AndroidUpdateError::Network => UpdateFailure::NetworkOrManifest,
        AndroidUpdateError::Storage => UpdateFailure::LocalStateUnavailable,
        AndroidUpdateError::Verification => UpdateFailure::DownloadVerification,
    }
}

#[cfg(target_os = "android")]
async fn android_install_status(app: &AppHandle) -> Result<AndroidInstallStatus, UpdateFailure> {
    let status = app
        .state::<AndroidUpdateInstallerPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<AndroidInstallStatus>("getInstallStatus", ())
        .await
        .map_err(|_| UpdateFailure::PlatformUnavailable)?;
    let _ = (
        status.requires_user_confirmation,
        status.awaiting_system_action,
    );
    Ok(status)
}

fn snapshot(
    app: &AppHandle,
    manager: &UpdateManagerState,
    stored: StoredUpdateState,
    fallback_phase: UpdatePhase,
    failure: Option<UpdateFailure>,
) -> UpdateSnapshot {
    let (phase, ready_to_install, downloaded_bytes, total_bytes) = manager
        .runtime
        .lock()
        .map(|runtime| {
            let phase = match runtime.active {
                Some(UpdateActivity::Checking) => UpdatePhase::Checking,
                Some(UpdateActivity::Downloading) => UpdatePhase::Downloading,
                Some(UpdateActivity::Installing) => UpdatePhase::Installing,
                Some(UpdateActivity::Clearing) => fallback_phase,
                None if runtime.awaiting_system_action => UpdatePhase::AwaitingSystemAction,
                None if runtime.ready_version.is_some() => UpdatePhase::ReadyToInstall,
                None => fallback_phase,
            };
            (
                phase,
                runtime.ready_version.is_some(),
                runtime.downloaded_bytes,
                runtime.total_bytes,
            )
        })
        .unwrap_or((fallback_phase, false, 0, None));
    UpdateSnapshot {
        current_version: app.package_info().version.to_string(),
        source: "github_releases",
        source_configured: UpdateSourceConfig::from_build()
            .as_ref()
            .is_some_and(UpdateSourceConfig::validate),
        installer: if cfg!(target_os = "android") {
            UpdateInstaller::AndroidSystem
        } else {
            UpdateInstaller::DesktopTauri
        },
        preferences: stored.preferences,
        phase,
        ready_to_install,
        downloaded_bytes,
        total_bytes,
        last_attempted_at_unix_seconds: stored.last_attempted_at_unix_seconds,
        last_checked_at_unix_seconds: stored.last_checked_at_unix_seconds,
        available: stored.available,
        failure,
    }
}

fn automatic_check_due(stored: &StoredUpdateState, now: u64) -> bool {
    if !stored.preferences.auto_check {
        return false;
    }
    stored
        .last_attempted_at_unix_seconds
        .or(stored.last_checked_at_unix_seconds)
        .is_none_or(|last| now.saturating_sub(last) >= AUTOMATIC_CHECK_INTERVAL_SECONDS)
}

fn state_root_path(app: &AppHandle) -> Result<PathBuf, UpdateFailure> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?
        .join(SETTINGS_DIRECTORY))
}

fn state_root(app: &AppHandle) -> Result<PathBuf, UpdateFailure> {
    let root = state_root_path(app)?;
    fs::create_dir_all(&root).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    Ok(root)
}

fn load_state(app: &AppHandle) -> Result<StoredUpdateState, UpdateFailure> {
    let root = state_root(app)?;
    let mut state = load_state_at(&root)?;
    if normalize_available_update(&mut state, &app.package_info().version.to_string())? {
        persist_state_at(&root, &state)?;
    }
    Ok(state)
}

fn normalize_available_update(
    state: &mut StoredUpdateState,
    current_version: &str,
) -> Result<bool, UpdateFailure> {
    let Some(available) = state.available.as_ref() else {
        return Ok(false);
    };
    let current = semver::Version::parse(current_version)
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    let offered = semver::Version::parse(&available.version)
        .map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    if offered <= current {
        state.available = None;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn load_state_at(root: &Path) -> Result<StoredUpdateState, UpdateFailure> {
    recover_state_files(root)?;
    let path = root.join(SETTINGS_FILE);
    if !path.is_file() {
        return Ok(StoredUpdateState::default());
    }
    let bytes = fs::read(path).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    let mut state: StoredUpdateState =
        serde_json::from_slice(&bytes).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    if state.schema_version != UPDATE_STATE_SCHEMA_VERSION {
        return Err(UpdateFailure::LocalStateUnavailable);
    }
    state.preferences = state.preferences.normalized();
    Ok(state)
}

pub fn clear_update_state(
    app: &AppHandle,
    manager: &UpdateManagerState,
) -> Result<(), UpdateFailure> {
    let guard = begin_operation(manager, UpdateActivity::Clearing)?;
    manager.cancelled.store(true, Ordering::Release);
    clear_prepared(manager);
    let root = state_root_path(app)?;
    clear_state_at(&root)?;
    #[cfg(target_os = "android")]
    {
        let packages = app
            .path()
            .app_cache_dir()
            .map_err(|_| UpdateFailure::LocalStateUnavailable)?
            .join("updates");
        match fs::remove_dir_all(packages) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(UpdateFailure::LocalStateUnavailable),
        }
    }
    drop(guard);
    reset_update_runtime(manager);
    Ok(())
}

fn clear_state_at(root: &Path) -> Result<(), UpdateFailure> {
    if !root.exists() {
        return Ok(());
    }
    for file in [SETTINGS_FILE, SETTINGS_STAGING_FILE, SETTINGS_BACKUP_FILE] {
        match fs::remove_file(root.join(file)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(UpdateFailure::LocalStateUnavailable),
        }
    }
    match fs::remove_dir(root) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) => {}
        Err(_) => return Err(UpdateFailure::LocalStateUnavailable),
    }
    Ok(())
}

fn persist_state(app: &AppHandle, state: &StoredUpdateState) -> Result<(), UpdateFailure> {
    let root = state_root(app)?;
    persist_state_at(&root, state)
}

fn persist_state_at(root: &Path, state: &StoredUpdateState) -> Result<(), UpdateFailure> {
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    let target = root.join(SETTINGS_FILE);
    let staging = root.join(SETTINGS_STAGING_FILE);
    let backup = root.join(SETTINGS_BACKUP_FILE);
    fs::write(&staging, bytes).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    let replacing = target.exists();
    if replacing {
        let _ = fs::remove_file(&backup);
        fs::rename(&target, &backup).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    }
    if fs::rename(&staging, &target).is_err() {
        if replacing {
            let _ = fs::rename(&backup, &target);
        }
        return Err(UpdateFailure::LocalStateUnavailable);
    }
    if replacing {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn recover_state_files(root: &Path) -> Result<(), UpdateFailure> {
    let target = root.join(SETTINGS_FILE);
    let staging = root.join(SETTINGS_STAGING_FILE);
    let backup = root.join(SETTINGS_BACKUP_FILE);
    if !target.exists() && backup.is_file() {
        fs::rename(&backup, &target).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    } else if backup.exists() {
        fs::remove_file(&backup).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    }
    if staging.exists() {
        fs::remove_file(staging).map_err(|_| UpdateFailure::LocalStateUnavailable)?;
    }
    Ok(())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        automatic_check_due, clear_state_at, load_state_at, normalize_available_update,
        persist_state_at, recover_state_files, AvailableUpdate, StoredUpdateState, UpdateFailure,
        UpdatePreferences, AUTOMATIC_CHECK_INTERVAL_SECONDS, SETTINGS_BACKUP_FILE, SETTINGS_FILE,
        SETTINGS_STAGING_FILE,
    };
    use std::{fs, path::PathBuf};

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("pixnya-update-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn automatic_checks_are_limited_to_once_per_day() {
        assert!(automatic_check_due(&StoredUpdateState::default(), 10));
        let state = StoredUpdateState {
            last_attempted_at_unix_seconds: Some(10),
            ..StoredUpdateState::default()
        };
        assert!(!automatic_check_due(
            &state,
            10 + AUTOMATIC_CHECK_INTERVAL_SECONDS - 1
        ));
        assert!(automatic_check_due(
            &state,
            10 + AUTOMATIC_CHECK_INTERVAL_SECONDS
        ));
    }

    #[test]
    fn a_failed_attempt_still_delays_the_next_automatic_check() {
        let state = StoredUpdateState {
            last_attempted_at_unix_seconds: Some(500),
            last_checked_at_unix_seconds: None,
            ..StoredUpdateState::default()
        };

        assert!(!automatic_check_due(&state, 501));
    }

    #[test]
    fn an_installed_or_older_available_version_is_removed_from_state() {
        let mut state = StoredUpdateState {
            available: Some(AvailableUpdate {
                version: "0.25.0".to_owned(),
                notes: None,
                published_at: None,
                size_bytes: None,
            }),
            ..StoredUpdateState::default()
        };

        assert!(normalize_available_update(&mut state, "0.25.0").unwrap());
        assert!(state.available.is_none());
    }

    #[test]
    fn disabling_checks_also_disables_automatic_downloads() {
        let preferences = UpdatePreferences {
            auto_check: false,
            auto_download: true,
            unmetered_only: true,
        }
        .normalized();
        assert!(!preferences.auto_check);
        assert!(!preferences.auto_download);
    }

    #[test]
    fn corrupt_state_fails_closed_instead_of_restoring_automatic_defaults() {
        let root = temp_root("corrupt");
        fs::write(root.join(SETTINGS_FILE), b"not-json").unwrap();

        assert_eq!(
            load_state_at(&root),
            Err(UpdateFailure::LocalStateUnavailable)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn clearing_update_state_removes_target_staging_and_backup_files() {
        let root = temp_root("clear");
        for file in [SETTINGS_FILE, SETTINGS_STAGING_FILE, SETTINGS_BACKUP_FILE] {
            fs::write(root.join(file), b"owned-update-state").unwrap();
        }

        clear_state_at(&root).unwrap();

        for file in [SETTINGS_FILE, SETTINGS_STAGING_FILE, SETTINGS_BACKUP_FILE] {
            assert!(!root.join(file).exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_state_replacement_restores_the_previous_file() {
        let root = temp_root("recovery");
        let initial = StoredUpdateState {
            last_checked_at_unix_seconds: Some(42),
            ..StoredUpdateState::default()
        };
        persist_state_at(&root, &initial).unwrap();
        fs::rename(root.join(SETTINGS_FILE), root.join(SETTINGS_BACKUP_FILE)).unwrap();
        fs::write(root.join(SETTINGS_STAGING_FILE), b"incomplete").unwrap();

        recover_state_files(&root).unwrap();

        assert!(root.join(SETTINGS_FILE).is_file());
        assert!(!root.join(SETTINGS_BACKUP_FILE).exists());
        assert!(!root.join(SETTINGS_STAGING_FILE).exists());
        let restored: StoredUpdateState =
            serde_json::from_slice(&fs::read(root.join(SETTINGS_FILE)).unwrap()).unwrap();
        assert_eq!(restored.last_checked_at_unix_seconds, Some(42));
        let _ = fs::remove_dir_all(root);
    }
}
