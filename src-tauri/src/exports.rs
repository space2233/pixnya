use crate::{offline_library, ApiCommandError, AuthenticatedDataState};
#[cfg(not(target_os = "android"))]
use pixiv_client_library::ExportedEntry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;

const SETTINGS_FILE: &str = "export-settings-v1.json";
const SETTINGS_STAGING_FILE: &str = ".export-settings-v1.staging";
const SETTINGS_BACKUP_FILE: &str = ".export-settings-v1.backup";
const SETTINGS_VERSION: u8 = 1;
#[cfg(target_os = "android")]
const ANDROID_STAGING_DIRECTORY: &str = "export-staging-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExportDestinationKind {
    #[cfg(not(target_os = "android"))]
    DesktopDirectory,
    #[cfg(target_os = "android")]
    AndroidDocumentTree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportDestinationStatus {
    pub configured: bool,
    pub kind: Option<ExportDestinationKind>,
    pub label: Option<String>,
    pub accessible: bool,
    pub auto_export: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportDestinationSelection {
    pub cancelled: bool,
    pub status: ExportDestinationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfflineExportResult {
    pub key: String,
    pub destination: String,
    pub directory_name: String,
    pub file_count: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredExportSettings {
    format_version: u8,
    desktop_directory: Option<String>,
    auto_export: bool,
}

impl Default for StoredExportSettings {
    fn default() -> Self {
        Self {
            format_version: SETTINGS_VERSION,
            desktop_directory: None,
            auto_export: true,
        }
    }
}

struct ExportManager {
    data_root: PathBuf,
    #[cfg(target_os = "android")]
    staging_root: PathBuf,
    settings: Mutex<StoredExportSettings>,
}

impl ExportManager {
    fn open(
        data_root: impl Into<PathBuf>,
        cache_root: impl Into<PathBuf>,
    ) -> Result<Self, ApiCommandError> {
        let data_root = data_root.into();
        let cache_root = cache_root.into();
        #[cfg(target_os = "android")]
        let staging_root = cache_root.join(ANDROID_STAGING_DIRECTORY);
        #[cfg(not(target_os = "android"))]
        let _ = cache_root;
        fs::create_dir_all(&data_root).map_err(|_| ApiCommandError::ExportUnavailable)?;
        restore_interrupted_settings(&data_root)?;
        let settings = load_settings(&data_root).unwrap_or_default();
        persist_settings(&data_root, &settings)?;
        Ok(Self {
            data_root,
            #[cfg(target_os = "android")]
            staging_root,
            settings: Mutex::new(settings),
        })
    }

    fn auto_export(&self) -> Result<bool, ApiCommandError> {
        self.settings
            .lock()
            .map(|settings| settings.auto_export)
            .map_err(|_| ApiCommandError::StateUnavailable)
    }

    fn set_auto_export(&self, auto_export: bool) -> Result<(), ApiCommandError> {
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        let updated = StoredExportSettings {
            auto_export,
            ..settings.clone()
        };
        persist_settings(&self.data_root, &updated)?;
        *settings = updated;
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    fn desktop_directory(&self) -> Result<Option<PathBuf>, ApiCommandError> {
        self.settings
            .lock()
            .map(|settings| settings.desktop_directory.as_ref().map(PathBuf::from))
            .map_err(|_| ApiCommandError::StateUnavailable)
    }

    #[cfg(not(target_os = "android"))]
    fn set_desktop_directory(&self, path: &Path) -> Result<(), ApiCommandError> {
        let canonical = fs::canonicalize(path).map_err(|_| ApiCommandError::ExportUnavailable)?;
        if !canonical.is_dir() || !canonical.is_absolute() {
            return Err(ApiCommandError::ExportUnavailable);
        }
        let value = canonical
            .to_str()
            .filter(|value| !value.is_empty())
            .ok_or(ApiCommandError::ExportUnavailable)?
            .to_owned();
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        let updated = StoredExportSettings {
            desktop_directory: Some(value),
            ..settings.clone()
        };
        persist_settings(&self.data_root, &updated)?;
        *settings = updated;
        Ok(())
    }

    fn clear_settings(&self) -> Result<(), ApiCommandError> {
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        let defaults = StoredExportSettings::default();
        persist_settings(&self.data_root, &defaults)?;
        *settings = defaults;
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    fn clear_desktop_directory(&self) -> Result<(), ApiCommandError> {
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        let updated = StoredExportSettings {
            desktop_directory: None,
            ..settings.clone()
        };
        persist_settings(&self.data_root, &updated)?;
        *settings = updated;
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct ExportState {
    manager: Mutex<Option<Arc<ExportManager>>>,
    operation: tokio::sync::Mutex<()>,
}

fn export_manager(app: &tauri::AppHandle) -> Result<Arc<ExportManager>, ApiCommandError> {
    let state = app.state::<ExportState>();
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    if let Some(manager) = manager.as_ref() {
        return Ok(manager.clone());
    }
    let data_root =
        crate::paths::app_data_dir(app).map_err(|_| ApiCommandError::ExportUnavailable)?;
    let cache_root =
        crate::paths::app_cache_dir(app).map_err(|_| ApiCommandError::ExportUnavailable)?;
    let opened = Arc::new(ExportManager::open(data_root, cache_root)?);
    *manager = Some(opened.clone());
    Ok(opened)
}

#[tauri::command]
pub(crate) async fn get_export_destination_status(
    app: tauri::AppHandle,
) -> Result<ExportDestinationStatus, ApiCommandError> {
    platform_status(&app).await
}

#[tauri::command]
pub(crate) async fn select_export_destination(
    app: tauri::AppHandle,
) -> Result<ExportDestinationSelection, ApiCommandError> {
    select_platform_destination(&app).await
}

#[tauri::command]
pub(crate) async fn clear_export_destination(
    app: tauri::AppHandle,
) -> Result<ExportDestinationStatus, ApiCommandError> {
    clear_platform_destination(&app).await?;
    platform_status(&app).await
}

#[tauri::command]
pub(crate) async fn set_auto_export_downloads(
    auto_export: bool,
    app: tauri::AppHandle,
) -> Result<ExportDestinationStatus, ApiCommandError> {
    export_manager(&app)?.set_auto_export(auto_export)?;
    platform_status(&app).await
}

#[tauri::command]
pub(crate) async fn export_offline_entry(
    entry_key: String,
    app: tauri::AppHandle,
) -> Result<OfflineExportResult, ApiCommandError> {
    if !platform_status(&app).await?.configured {
        return Err(ApiCommandError::ExportDestinationUnavailable);
    }
    perform_export(&app, entry_key).await
}

pub(crate) async fn auto_export_offline_entry(
    app: &tauri::AppHandle,
    entry_key: &str,
) -> Result<Option<OfflineExportResult>, ApiCommandError> {
    let status = platform_status(app).await?;
    if !status.configured || !status.auto_export {
        return Ok(None);
    }
    perform_export(app, entry_key.to_owned()).await.map(Some)
}

pub(crate) async fn clear_all_export_settings(app: &tauri::AppHandle) -> Result<(), ()> {
    clear_platform_destination(app).await.map_err(|_| ())?;
    export_manager(app)
        .and_then(|manager| manager.clear_settings())
        .map_err(|_| ())
}

async fn perform_export(
    app: &tauri::AppHandle,
    entry_key: String,
) -> Result<OfflineExportResult, ApiCommandError> {
    let state = app.state::<ExportState>();
    let _operation = state.operation.lock().await;
    let library_permit = app
        .state::<AuthenticatedDataState>()
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;

    #[cfg(not(target_os = "android"))]
    {
        let manager = export_manager(app)?;
        let destination = manager
            .desktop_directory()?
            .ok_or(ApiCommandError::ExportDestinationUnavailable)?;
        let library = offline_library(app)?;
        let export_key = entry_key.clone();
        let receipt = tauri::async_runtime::spawn_blocking(move || {
            let _permit = library_permit;
            library.export_entry(&export_key, &destination)
        })
        .await
        .map_err(|_| ApiCommandError::ExportUnavailable)??;
        Ok(result_from_receipt(entry_key, receipt))
    }

    #[cfg(target_os = "android")]
    {
        let manager = export_manager(app)?;
        let staging_root = manager.staging_root.clone();
        let library = offline_library(app)?;
        let export_key = entry_key.clone();
        let receipt = tauri::async_runtime::spawn_blocking(move || {
            let _permit = library_permit;
            prepare_android_staging_root(&staging_root)?;
            library
                .export_entry(&export_key, &staging_root)
                .map_err(ApiCommandError::from)
        })
        .await
        .map_err(|_| ApiCommandError::ExportUnavailable)??;

        let plugin_result = app
            .state::<AndroidExportPlugin>()
            .0
            .clone()
            .run_mobile_plugin_async::<AndroidExportResult>(
                "exportDirectory",
                AndroidExportPayload {
                    source_directory: receipt.directory.to_string_lossy().into_owned(),
                    directory_name: receipt.directory_name.clone(),
                    expected_file_count: receipt.file_count,
                    expected_size_bytes: receipt.size_bytes,
                },
            )
            .await;
        let _ = tauri::async_runtime::spawn_blocking({
            let path = receipt.directory.clone();
            move || remove_android_staging_entry(&path)
        })
        .await;
        let destination = plugin_result
            .map_err(|_| ApiCommandError::ExportUnavailable)?
            .destination;
        Ok(OfflineExportResult {
            key: entry_key,
            destination,
            directory_name: receipt.directory_name,
            file_count: receipt.file_count,
            size_bytes: receipt.size_bytes,
        })
    }
}

#[cfg(not(target_os = "android"))]
fn result_from_receipt(key: String, receipt: ExportedEntry) -> OfflineExportResult {
    OfflineExportResult {
        key,
        destination: receipt.directory.to_string_lossy().into_owned(),
        directory_name: receipt.directory_name,
        file_count: receipt.file_count,
        size_bytes: receipt.size_bytes,
    }
}

#[cfg(not(target_os = "android"))]
async fn platform_status(
    app: &tauri::AppHandle,
) -> Result<ExportDestinationStatus, ApiCommandError> {
    let manager = export_manager(app)?;
    let directory = manager.desktop_directory()?;
    let accessible = directory.as_ref().is_some_and(|path| path.is_dir());
    Ok(ExportDestinationStatus {
        configured: directory.is_some(),
        kind: directory
            .as_ref()
            .map(|_| ExportDestinationKind::DesktopDirectory),
        label: directory.map(|path| path.to_string_lossy().into_owned()),
        accessible,
        auto_export: manager.auto_export()?,
    })
}

#[cfg(not(target_os = "android"))]
async fn select_platform_destination(
    app: &tauri::AppHandle,
) -> Result<ExportDestinationSelection, ApiCommandError> {
    use tauri_plugin_dialog::DialogExt;

    let manager = export_manager(app)?;
    let initial = manager.desktop_directory()?;
    let picker_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut picker = picker_app.dialog().file().set_title("选择 PixNya 导出目录");
        if let Some(initial) = initial {
            picker = picker.set_directory(initial);
        }
        picker.blocking_pick_folder()
    })
    .await
    .map_err(|_| ApiCommandError::ExportUnavailable)?;

    let Some(picked) = picked else {
        return Ok(ExportDestinationSelection {
            cancelled: true,
            status: platform_status(app).await?,
        });
    };
    let path = picked
        .into_path()
        .map_err(|_| ApiCommandError::ExportUnavailable)?;
    manager.set_desktop_directory(&path)?;
    Ok(ExportDestinationSelection {
        cancelled: false,
        status: platform_status(app).await?,
    })
}

#[cfg(not(target_os = "android"))]
async fn clear_platform_destination(app: &tauri::AppHandle) -> Result<(), ApiCommandError> {
    export_manager(app)?.clear_desktop_directory()
}

#[cfg(target_os = "android")]
pub(crate) struct AndroidExportPlugin(pub(crate) tauri::plugin::PluginHandle<tauri::Wry>);

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct EmptyMobilePayload {}

#[cfg(target_os = "android")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidDestinationStatus {
    configured: bool,
    label: Option<String>,
    accessible: bool,
}

#[cfg(target_os = "android")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidDestinationSelection {
    cancelled: bool,
    configured: bool,
    label: Option<String>,
    accessible: bool,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidExportPayload {
    source_directory: String,
    directory_name: String,
    expected_file_count: u32,
    expected_size_bytes: u64,
}

#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct AndroidExportResult {
    destination: String,
}

#[cfg(target_os = "android")]
pub(crate) fn android_export_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("export_destination")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("io.github.space2233.pixnya", "ExportDirectoryPlugin")?;
            app.manage(AndroidExportPlugin(handle));
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
async fn platform_status(
    app: &tauri::AppHandle,
) -> Result<ExportDestinationStatus, ApiCommandError> {
    let manager = export_manager(app)?;
    let status = app
        .state::<AndroidExportPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<AndroidDestinationStatus>(
            "getDirectoryStatus",
            EmptyMobilePayload {},
        )
        .await
        .map_err(|_| ApiCommandError::ExportUnavailable)?;
    Ok(ExportDestinationStatus {
        configured: status.configured,
        kind: status
            .configured
            .then_some(ExportDestinationKind::AndroidDocumentTree),
        label: status.label,
        accessible: status.accessible,
        auto_export: manager.auto_export()?,
    })
}

#[cfg(target_os = "android")]
async fn select_platform_destination(
    app: &tauri::AppHandle,
) -> Result<ExportDestinationSelection, ApiCommandError> {
    let selected = app
        .state::<AndroidExportPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<AndroidDestinationSelection>(
            "selectDirectory",
            EmptyMobilePayload {},
        )
        .await
        .map_err(|_| ApiCommandError::ExportUnavailable)?;
    let auto_export = export_manager(app)?.auto_export()?;
    Ok(ExportDestinationSelection {
        cancelled: selected.cancelled,
        status: ExportDestinationStatus {
            configured: selected.configured,
            kind: selected
                .configured
                .then_some(ExportDestinationKind::AndroidDocumentTree),
            label: selected.label,
            accessible: selected.accessible,
            auto_export,
        },
    })
}

#[cfg(target_os = "android")]
async fn clear_platform_destination(app: &tauri::AppHandle) -> Result<(), ApiCommandError> {
    app.state::<AndroidExportPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<()>("clearDirectory", EmptyMobilePayload {})
        .await
        .map_err(|_| ApiCommandError::ExportUnavailable)
}

#[cfg(target_os = "android")]
fn prepare_android_staging_root(root: &Path) -> Result<(), ApiCommandError> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(ApiCommandError::ExportUnavailable)
        }
        Ok(_) => {
            for child in fs::read_dir(root).map_err(|_| ApiCommandError::ExportUnavailable)? {
                let child = child.map_err(|_| ApiCommandError::ExportUnavailable)?;
                let metadata = fs::symlink_metadata(child.path())
                    .map_err(|_| ApiCommandError::ExportUnavailable)?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(ApiCommandError::ExportUnavailable);
                }
                fs::remove_dir_all(child.path()).map_err(|_| ApiCommandError::ExportUnavailable)?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(root).map_err(|_| ApiCommandError::ExportUnavailable)?;
        }
        Err(_) => return Err(ApiCommandError::ExportUnavailable),
    }
    Ok(())
}

#[cfg(target_os = "android")]
fn remove_android_staging_entry(path: &Path) -> Result<(), ApiCommandError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ApiCommandError::ExportUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ApiCommandError::ExportUnavailable);
    }
    fs::remove_dir_all(path).map_err(|_| ApiCommandError::ExportUnavailable)
}

fn load_settings(root: &Path) -> Option<StoredExportSettings> {
    let bytes = fs::read(root.join(SETTINGS_FILE)).ok()?;
    if bytes.len() > 16 * 1024 {
        return None;
    }
    let settings: StoredExportSettings = serde_json::from_slice(&bytes).ok()?;
    if settings.format_version != SETTINGS_VERSION {
        return None;
    }
    if let Some(path) = settings.desktop_directory.as_ref() {
        if path.is_empty() || !Path::new(path).is_absolute() {
            return None;
        }
    }
    Some(settings)
}

fn persist_settings(root: &Path, settings: &StoredExportSettings) -> Result<(), ApiCommandError> {
    let bytes =
        serde_json::to_vec_pretty(settings).map_err(|_| ApiCommandError::ExportUnavailable)?;
    let target = root.join(SETTINGS_FILE);
    let staging = root.join(SETTINGS_STAGING_FILE);
    let backup = root.join(SETTINGS_BACKUP_FILE);
    fs::write(&staging, bytes).map_err(|_| ApiCommandError::ExportUnavailable)?;
    let replacing = target.exists();
    if replacing {
        let _ = fs::remove_file(&backup);
        fs::rename(&target, &backup).map_err(|_| ApiCommandError::ExportUnavailable)?;
    }
    if fs::rename(&staging, &target).is_err() {
        if replacing {
            let _ = fs::rename(&backup, &target);
        }
        return Err(ApiCommandError::ExportUnavailable);
    }
    if replacing {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn restore_interrupted_settings(root: &Path) -> Result<(), ApiCommandError> {
    let target = root.join(SETTINGS_FILE);
    let staging = root.join(SETTINGS_STAGING_FILE);
    let backup = root.join(SETTINGS_BACKUP_FILE);
    if !target.exists() && backup.is_file() {
        fs::rename(&backup, &target).map_err(|_| ApiCommandError::ExportUnavailable)?;
    } else if backup.exists() {
        fs::remove_file(&backup).map_err(|_| ApiCommandError::ExportUnavailable)?;
    }
    if staging.exists() {
        fs::remove_file(staging).map_err(|_| ApiCommandError::ExportUnavailable)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ExportManager;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pixiv-client-export-{name}-{nonce}"))
    }

    #[test]
    fn persists_desktop_destination_and_auto_export_preference() {
        let root = test_root("settings");
        let destination = root.join("chosen");
        fs::create_dir_all(&destination).unwrap();
        let manager = ExportManager::open(root.join("data"), root.join("cache")).unwrap();
        manager.set_desktop_directory(&destination).unwrap();
        manager.set_auto_export(false).unwrap();
        drop(manager);

        let reopened = ExportManager::open(root.join("data"), root.join("cache")).unwrap();
        assert_eq!(
            reopened.desktop_directory().unwrap(),
            Some(fs::canonicalize(&destination).unwrap())
        );
        assert!(!reopened.auto_export().unwrap());
        reopened.clear_settings().unwrap();
        assert_eq!(reopened.desktop_directory().unwrap(), None);
        assert!(reopened.auto_export().unwrap());
        assert!(destination.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_settings_fall_back_without_leaving_partial_files() {
        let root = test_root("corrupt");
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("export-settings-v1.json"), b"not-json").unwrap();

        let manager = ExportManager::open(&data, root.join("cache")).unwrap();
        assert_eq!(manager.desktop_directory().unwrap(), None);
        assert!(manager.auto_export().unwrap());
        assert!(!data.join(".export-settings-v1.staging").exists());
        assert!(!data.join(".export-settings-v1.backup").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
