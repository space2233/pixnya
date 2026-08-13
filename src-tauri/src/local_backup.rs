use crate::{
    catalog::{open_catalog, CatalogState},
    downloads::{download_queue, suspend_download_worker, wake_download_worker},
    exports::{export_backup_file, select_backup_file, BACKUP_STAGING_DIRECTORY},
    history::{open_history, HistoryState},
    paths, ApiCommandError, AuthenticatedDataState,
};
use pixiv_client_download_queue::DownloadQueueSnapshot;
use pixiv_client_local_backup::{
    BackupError, BackupManager, BackupPreview, BackupSummary, FrontendBackupState,
    OfflineBackupSource, PortableBackupData,
};
use pixiv_client_local_catalog::CatalogSnapshot;
use pixiv_client_local_history::HistorySnapshot;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const RESTORE_STAGING_DIRECTORY: &str = ".pixnya-backup-restore-staging";
const RESTORE_ROLLBACK_DIRECTORY: &str = ".pixnya-backup-restore-rollback";
const RESTORE_MARKER_FILE: &str = ".pixnya-backup-restore-v1.json";
const RESTORE_MARKER_STAGING_FILE: &str = ".pixnya-backup-restore-v1.staging";
const FRONTEND_RECOVERY_FILE: &str = ".pixnya-backup-frontend-recovery-v1.json";
const FRONTEND_RECOVERY_STAGING_FILE: &str = ".pixnya-backup-frontend-recovery-v1.staging";
const MAX_OFFLINE_FILES: usize = 100_000;

#[derive(Default)]
pub(crate) struct LocalBackupState {
    operation: tokio::sync::Mutex<()>,
    selected: Mutex<Option<SelectedBackup>>,
    pending_transaction: Mutex<Option<u64>>,
}

#[derive(Clone)]
struct SelectedBackup {
    path: PathBuf,
    preview: BackupPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupRestoreStrategy {
    Merge,
    Replace,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupSelectionResult {
    cancelled: bool,
    label: Option<String>,
    preview: Option<BackupPreview>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupExportResult {
    destination: String,
    summary: BackupSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupRestoreStartResult {
    transaction_id: u64,
    frontend: FrontendBackupState,
    summary: BackupSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreMarker {
    transaction_id: u64,
    previous_frontend: FrontendBackupState,
    catalog: CatalogSnapshot,
    history: HistorySnapshot,
    downloads: DownloadQueueSnapshot,
    offline_included: bool,
    offline_had_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FrontendRecovery {
    transaction_id: u64,
    frontend: FrontendBackupState,
}

#[tauri::command]
pub(crate) async fn create_local_backup(
    frontend: FrontendBackupState,
    include_offline: bool,
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<BackupExportResult, ApiCommandError> {
    let state = app.state::<LocalBackupState>();
    let _operation = state.operation.lock().await;
    ensure_no_pending_restore(&state)?;
    suspend_download_worker(&app).await;
    let _resume_downloads = DownloadWorkerResumeGuard::new(app.clone());
    let catalog_state = app.state::<CatalogState>();
    let history_state = app.state::<HistoryState>();
    let _catalog_operation = catalog_state.operation.lock().await;
    let _history_operation = history_state.operation.lock().await;
    let library_permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let app_data = paths::app_data_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let cache_root = paths::app_cache_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let storage = crate::storage_manager(&app)?;
    let staging_root = cache_root.join(BACKUP_STAGING_DIRECTORY);
    let transaction_id = unique_transaction_id()?;
    let file_name = format!("pixnya-backup-{transaction_id}.pixnyabackup");
    let staging_file = staging_root.join(&file_name);
    let app_for_snapshot = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _permit = library_permit;
        fs::create_dir_all(&staging_root).map_err(|_| ApiCommandError::BackupUnavailable)?;
        let catalog = open_catalog(&app_for_snapshot)?.portable_snapshot()?;
        let history = open_history(&app_for_snapshot)?.snapshot()?;
        let downloads = download_queue(&app_for_snapshot)?.portable_snapshot()?;
        let offline_sources = if include_offline {
            collect_offline_sources(&app_data.join("offline-library"))?
        } else {
            Vec::new()
        };
        let estimated_bytes =
            offline_sources
                .iter()
                .try_fold(64 * 1024 * 1024_u64, |total, source| {
                    total
                        .checked_add(
                            fs::metadata(&source.source_path)
                                .map_err(|_| ApiCommandError::BackupUnavailable)?
                                .len(),
                        )
                        .ok_or(ApiCommandError::BackupCapacityExceeded)
                })?;
        if !storage.allows_cache_write(estimated_bytes)? {
            return Err(ApiCommandError::BackupCapacityExceeded);
        }
        let data = PortableBackupData {
            frontend,
            catalog: serde_json::to_value(catalog).map_err(|_| ApiCommandError::BackupInvalid)?,
            history: serde_json::to_value(history).map_err(|_| ApiCommandError::BackupInvalid)?,
            downloads: serde_json::to_value(downloads)
                .map_err(|_| ApiCommandError::BackupInvalid)?,
        };
        let summary = BackupManager::new(env!("CARGO_PKG_VERSION")).create_from_sources(
            &staging_file,
            data,
            include_offline,
            offline_sources,
        )?;
        Ok::<_, ApiCommandError>((staging_file, file_name, summary))
    })
    .await
    .map_err(|_| ApiCommandError::BackupUnavailable)??;
    let destination = export_backup_file(&app, &result.0, &result.1).await;
    let _ = fs::remove_file(&result.0);
    Ok(BackupExportResult {
        destination: destination?,
        summary: result.2,
    })
}

#[tauri::command]
pub(crate) async fn select_local_backup(
    app: tauri::AppHandle,
) -> Result<BackupSelectionResult, ApiCommandError> {
    let state = app.state::<LocalBackupState>();
    let _operation = state.operation.lock().await;
    ensure_no_pending_restore(&state)?;
    let Some((path, label)) = select_backup_file(&app).await? else {
        return Ok(BackupSelectionResult {
            cancelled: true,
            label: None,
            preview: None,
        });
    };
    let inspect_path = path.clone();
    let preview = tauri::async_runtime::spawn_blocking(move || {
        BackupManager::new(env!("CARGO_PKG_VERSION")).inspect(&inspect_path)
    })
    .await
    .map_err(|_| ApiCommandError::BackupUnavailable)??;
    *state
        .selected
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)? = Some(SelectedBackup {
        path,
        preview: preview.clone(),
    });
    Ok(BackupSelectionResult {
        cancelled: false,
        label: Some(label),
        preview: Some(preview),
    })
}

#[tauri::command]
pub(crate) async fn start_local_backup_restore(
    strategy: BackupRestoreStrategy,
    previous_frontend: FrontendBackupState,
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<BackupRestoreStartResult, ApiCommandError> {
    let state = app.state::<LocalBackupState>();
    let _operation = state.operation.lock().await;
    ensure_no_pending_restore(&state)?;
    let app_data = paths::app_data_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    if read_frontend_recovery(&app_data)?.is_some() {
        return Err(ApiCommandError::BackupTransactionPending);
    }
    let selected = state
        .selected
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)?
        .clone()
        .ok_or(ApiCommandError::BackupUnavailable)?;
    if selected.preview.offline_included {
        let storage = crate::storage_manager(&app)?;
        let existing_bytes = if strategy == BackupRestoreStrategy::Merge {
            storage.status()?.offline_bytes
        } else {
            0
        };
        storage.ensure_offline_write(
            selected
                .preview
                .total_bytes
                .checked_add(existing_bytes)
                .ok_or(ApiCommandError::BackupCapacityExceeded)?,
        )?;
    }
    suspend_download_worker(&app).await;
    let mut resume_downloads = DownloadWorkerResumeGuard::new(app.clone());
    let catalog_state = app.state::<CatalogState>();
    let history_state = app.state::<HistoryState>();
    let _catalog_operation = catalog_state.operation.lock().await;
    let _history_operation = history_state.operation.lock().await;
    let library_permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let selected_for_restore = selected.clone();
    let app_for_restore = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _permit = library_permit;
        restore_backend(
            &app_for_restore,
            &app_data,
            &selected_for_restore,
            strategy,
            previous_frontend,
        )
    })
    .await
    .map_err(|_| ApiCommandError::BackupUnavailable)??;
    *state
        .pending_transaction
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)? = Some(result.transaction_id);
    *state
        .selected
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)? = None;
    resume_downloads.disarm();
    Ok(result)
}

#[tauri::command]
pub(crate) fn get_local_backup_frontend_recovery(
    app: tauri::AppHandle,
) -> Result<Option<FrontendRecovery>, ApiCommandError> {
    let app_data = paths::app_data_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    read_frontend_recovery(&app_data)
}

#[tauri::command]
pub(crate) fn acknowledge_local_backup_frontend_recovery(
    transaction_id: u64,
    app: tauri::AppHandle,
) -> Result<(), ApiCommandError> {
    let app_data = paths::app_data_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let Some(recovery) = read_frontend_recovery(&app_data)? else {
        return Ok(());
    };
    if recovery.transaction_id != transaction_id {
        return Err(ApiCommandError::BackupTransactionUnavailable);
    }
    remove_frontend_recovery(&app_data)
}

#[tauri::command]
pub(crate) async fn commit_local_backup_restore(
    transaction_id: u64,
    app: tauri::AppHandle,
) -> Result<(), ApiCommandError> {
    let state = app.state::<LocalBackupState>();
    let _operation = state.operation.lock().await;
    require_pending_transaction(&state, transaction_id)?;
    let app_data = paths::app_data_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    remove_marker(&app_data)?;
    // Once the marker is removed the restored state is committed. Cleanup is
    // best-effort so a stale rollback directory cannot turn a successful
    // restore into a request to undo already committed frontend state.
    let _ = remove_path_if_present(&app_data.join(RESTORE_ROLLBACK_DIRECTORY));
    *state
        .pending_transaction
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)? = None;
    wake_download_worker(&app);
    Ok(())
}

#[tauri::command]
pub(crate) async fn rollback_local_backup_restore(
    transaction_id: u64,
    app: tauri::AppHandle,
) -> Result<(), ApiCommandError> {
    let state = app.state::<LocalBackupState>();
    let _operation = state.operation.lock().await;
    require_pending_transaction(&state, transaction_id)?;
    let catalog_state = app.state::<CatalogState>();
    let history_state = app.state::<HistoryState>();
    let _catalog_operation = catalog_state.operation.lock().await;
    let _history_operation = history_state.operation.lock().await;
    let app_data = paths::app_data_dir(&app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    rollback_from_marker(&app, &app_data)?;
    *state
        .pending_transaction
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)? = None;
    wake_download_worker(&app);
    Ok(())
}

pub(crate) fn recover_interrupted_restore(app: &tauri::AppHandle) -> Result<(), ApiCommandError> {
    let app_data = paths::app_data_dir(app).map_err(|_| ApiCommandError::BackupUnavailable)?;
    if app_data.join(RESTORE_MARKER_FILE).is_file() {
        rollback_from_marker(app, &app_data)?;
    } else {
        remove_path_if_present(&app_data.join(RESTORE_STAGING_DIRECTORY))?;
        remove_path_if_present(&app_data.join(RESTORE_ROLLBACK_DIRECTORY))?;
    }
    Ok(())
}

fn restore_backend(
    app: &tauri::AppHandle,
    app_data: &Path,
    selected: &SelectedBackup,
    strategy: BackupRestoreStrategy,
    previous_frontend: FrontendBackupState,
) -> Result<BackupRestoreStartResult, ApiCommandError> {
    let staging = app_data.join(RESTORE_STAGING_DIRECTORY);
    let rollback = app_data.join(RESTORE_ROLLBACK_DIRECTORY);
    remove_path_if_present(&staging)?;
    remove_path_if_present(&rollback)?;
    let manager = BackupManager::new(env!("CARGO_PKG_VERSION"));
    let data = manager.restore_to_directory(&selected.path, &staging)?;
    let catalog: CatalogSnapshot =
        serde_json::from_value(data.catalog).map_err(|_| ApiCommandError::BackupInvalid)?;
    let history: HistorySnapshot =
        serde_json::from_value(data.history).map_err(|_| ApiCommandError::BackupInvalid)?;
    let downloads: DownloadQueueSnapshot =
        serde_json::from_value(data.downloads).map_err(|_| ApiCommandError::BackupInvalid)?;
    let catalog_store = open_catalog(app)?;
    let history_store = open_history(app)?;
    let download_store = download_queue(app)?;
    let marker = RestoreMarker {
        transaction_id: unique_transaction_id()?,
        previous_frontend,
        catalog: catalog_store.portable_snapshot()?,
        history: history_store.snapshot()?,
        downloads: download_store.portable_snapshot()?,
        offline_included: selected.preview.offline_included,
        offline_had_root: app_data.join("offline-library").is_dir(),
    };
    write_marker(app_data, &marker)?;
    let replace = strategy == BackupRestoreStrategy::Replace;
    let applied = (|| {
        if marker.offline_included {
            prepare_offline_restore(app_data, &staging, &rollback, strategy)?;
        } else {
            remove_path_if_present(&staging)?;
        }
        catalog_store.restore_snapshot(&catalog, replace)?;
        history_store.restore_snapshot(&history, replace)?;
        download_store.restore_snapshot(&downloads, replace)?;
        Ok::<_, ApiCommandError>(())
    })();
    if let Err(error) = applied {
        if rollback_from_marker(app, app_data).is_err() {
            return Err(ApiCommandError::BackupRollbackFailed);
        }
        // The frontend never received or applied the candidate state when the
        // backend phase fails, so there is nothing for startup to recover.
        remove_frontend_recovery(app_data)?;
        return Err(error);
    }
    Ok(BackupRestoreStartResult {
        transaction_id: marker.transaction_id,
        frontend: data.frontend,
        summary: selected.preview.clone(),
    })
}

fn prepare_offline_restore(
    app_data: &Path,
    staging: &Path,
    rollback: &Path,
    strategy: BackupRestoreStrategy,
) -> Result<(), ApiCommandError> {
    let root = app_data.join("offline-library");
    if strategy == BackupRestoreStrategy::Merge && root.is_dir() {
        copy_missing_tree(&root, staging)?;
    }
    if root.exists() {
        fs::rename(&root, rollback).map_err(|_| ApiCommandError::BackupUnavailable)?;
    }
    if let Err(error) = fs::rename(staging, &root) {
        if rollback.exists() {
            let _ = fs::rename(rollback, &root);
        }
        return Err(match error.kind() {
            std::io::ErrorKind::NotFound => ApiCommandError::BackupInvalid,
            _ => ApiCommandError::BackupUnavailable,
        });
    }
    Ok(())
}

fn rollback_from_marker(app: &tauri::AppHandle, app_data: &Path) -> Result<(), ApiCommandError> {
    let marker: RestoreMarker = serde_json::from_slice(
        &fs::read(app_data.join(RESTORE_MARKER_FILE))
            .map_err(|_| ApiCommandError::BackupRollbackFailed)?,
    )
    .map_err(|_| ApiCommandError::BackupRollbackFailed)?;
    open_catalog(app)?
        .restore_snapshot(&marker.catalog, true)
        .map_err(|_| ApiCommandError::BackupRollbackFailed)?;
    open_history(app)?
        .restore_snapshot(&marker.history, true)
        .map_err(|_| ApiCommandError::BackupRollbackFailed)?;
    download_queue(app)?
        .restore_snapshot(&marker.downloads, true)
        .map_err(|_| ApiCommandError::BackupRollbackFailed)?;
    if marker.offline_included {
        let root = app_data.join("offline-library");
        let rollback = app_data.join(RESTORE_ROLLBACK_DIRECTORY);
        let staging = app_data.join(RESTORE_STAGING_DIRECTORY);
        if rollback.exists() {
            remove_path_if_present(&root)?;
            fs::rename(&rollback, &root).map_err(|_| ApiCommandError::BackupRollbackFailed)?;
        } else if !marker.offline_had_root && !staging.exists() {
            remove_path_if_present(&root)?;
        }
    }
    write_frontend_recovery(
        app_data,
        &FrontendRecovery {
            transaction_id: marker.transaction_id,
            frontend: marker.previous_frontend,
        },
    )?;
    remove_path_if_present(&app_data.join(RESTORE_STAGING_DIRECTORY))?;
    remove_marker(app_data)?;
    Ok(())
}

fn read_frontend_recovery(app_data: &Path) -> Result<Option<FrontendRecovery>, ApiCommandError> {
    let path = app_data.join(FRONTEND_RECOVERY_FILE);
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ApiCommandError::BackupUnavailable),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| ApiCommandError::BackupInvalid)
}

fn write_frontend_recovery(
    app_data: &Path,
    recovery: &FrontendRecovery,
) -> Result<(), ApiCommandError> {
    fs::create_dir_all(app_data).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let bytes = serde_json::to_vec(recovery).map_err(|_| ApiCommandError::BackupInvalid)?;
    let staging = app_data.join(FRONTEND_RECOVERY_STAGING_FILE);
    let target = app_data.join(FRONTEND_RECOVERY_FILE);
    fs::write(&staging, bytes).map_err(|_| ApiCommandError::BackupUnavailable)?;
    if target.exists() {
        fs::remove_file(&target).map_err(|_| ApiCommandError::BackupUnavailable)?;
    }
    fs::rename(staging, target).map_err(|_| ApiCommandError::BackupUnavailable)
}

fn remove_frontend_recovery(app_data: &Path) -> Result<(), ApiCommandError> {
    remove_path_if_present(&app_data.join(FRONTEND_RECOVERY_FILE))?;
    remove_path_if_present(&app_data.join(FRONTEND_RECOVERY_STAGING_FILE))
}

fn collect_offline_sources(root: &Path) -> Result<Vec<OfflineBackupSource>, ApiCommandError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    collect_offline_sources_at(root, root, &mut output)?;
    output.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

fn collect_offline_sources_at(
    root: &Path,
    directory: &Path,
    output: &mut Vec<OfflineBackupSource>,
) -> Result<(), ApiCommandError> {
    for entry in fs::read_dir(directory).map_err(|_| ApiCommandError::BackupUnavailable)? {
        let entry = entry.map_err(|_| ApiCommandError::BackupUnavailable)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| ApiCommandError::BackupUnavailable)?;
        if metadata.file_type().is_symlink() {
            return Err(ApiCommandError::BackupInvalid);
        }
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if metadata.is_dir() {
            collect_offline_sources_at(root, &entry.path(), output)?;
        } else if metadata.is_file() {
            if output.len() >= MAX_OFFLINE_FILES {
                return Err(ApiCommandError::BackupCapacityExceeded);
            }
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(root)
                .map_err(|_| ApiCommandError::BackupInvalid)?;
            output.push(OfflineBackupSource {
                relative_path: relative.to_string_lossy().replace('\\', "/"),
                source_path: entry_path,
            });
        } else {
            return Err(ApiCommandError::BackupInvalid);
        }
    }
    Ok(())
}

fn copy_missing_tree(source: &Path, destination: &Path) -> Result<(), ApiCommandError> {
    fs::create_dir_all(destination).map_err(|_| ApiCommandError::BackupUnavailable)?;
    for entry in fs::read_dir(source).map_err(|_| ApiCommandError::BackupUnavailable)? {
        let entry = entry.map_err(|_| ApiCommandError::BackupUnavailable)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| ApiCommandError::BackupUnavailable)?;
        if metadata.file_type().is_symlink() {
            return Err(ApiCommandError::BackupInvalid);
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            copy_missing_tree(&entry.path(), &target)?;
        } else if metadata.is_file() {
            if target.exists() {
                if !files_equal(&entry.path(), &target)? {
                    return Err(ApiCommandError::BackupConflict);
                }
            } else if fs::hard_link(entry.path(), &target).is_err() {
                fs::copy(entry.path(), &target).map_err(|_| ApiCommandError::BackupUnavailable)?;
            }
        }
    }
    Ok(())
}

fn files_equal(left: &Path, right: &Path) -> Result<bool, ApiCommandError> {
    use std::io::Read;
    if fs::metadata(left)
        .map_err(|_| ApiCommandError::BackupUnavailable)?
        .len()
        != fs::metadata(right)
            .map_err(|_| ApiCommandError::BackupUnavailable)?
            .len()
    {
        return Ok(false);
    }
    let mut left = fs::File::open(left).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let mut right = fs::File::open(right).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let mut left_buffer = [0_u8; 128 * 1024];
    let mut right_buffer = [0_u8; 128 * 1024];
    loop {
        let left_read = left
            .read(&mut left_buffer)
            .map_err(|_| ApiCommandError::BackupUnavailable)?;
        let right_read = right
            .read(&mut right_buffer)
            .map_err(|_| ApiCommandError::BackupUnavailable)?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn write_marker(app_data: &Path, marker: &RestoreMarker) -> Result<(), ApiCommandError> {
    fs::create_dir_all(app_data).map_err(|_| ApiCommandError::BackupUnavailable)?;
    let bytes = serde_json::to_vec(marker).map_err(|_| ApiCommandError::BackupInvalid)?;
    let staging = app_data.join(RESTORE_MARKER_STAGING_FILE);
    let target = app_data.join(RESTORE_MARKER_FILE);
    fs::write(&staging, bytes).map_err(|_| ApiCommandError::BackupUnavailable)?;
    fs::rename(staging, target).map_err(|_| ApiCommandError::BackupUnavailable)
}

fn remove_marker(app_data: &Path) -> Result<(), ApiCommandError> {
    remove_path_if_present(&app_data.join(RESTORE_MARKER_FILE))?;
    remove_path_if_present(&app_data.join(RESTORE_MARKER_STAGING_FILE))
}

fn remove_path_if_present(path: &Path) -> Result<(), ApiCommandError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ApiCommandError::BackupUnavailable),
    };
    if metadata.file_type().is_symlink() {
        return Err(ApiCommandError::BackupInvalid);
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|_| ApiCommandError::BackupUnavailable)
    } else {
        fs::remove_file(path).map_err(|_| ApiCommandError::BackupUnavailable)
    }
}

struct DownloadWorkerResumeGuard {
    app: Option<tauri::AppHandle>,
}

impl DownloadWorkerResumeGuard {
    fn new(app: tauri::AppHandle) -> Self {
        Self { app: Some(app) }
    }

    fn disarm(&mut self) {
        self.app = None;
    }
}

impl Drop for DownloadWorkerResumeGuard {
    fn drop(&mut self) {
        if let Some(app) = self.app.take() {
            wake_download_worker(&app);
        }
    }
}

fn ensure_no_pending_restore(state: &LocalBackupState) -> Result<(), ApiCommandError> {
    if state
        .pending_transaction
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)?
        .is_some()
    {
        Err(ApiCommandError::BackupTransactionPending)
    } else {
        Ok(())
    }
}

fn require_pending_transaction(state: &LocalBackupState, id: u64) -> Result<(), ApiCommandError> {
    if state
        .pending_transaction
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)?
        .as_ref()
        == Some(&id)
    {
        Ok(())
    } else {
        Err(ApiCommandError::BackupTransactionUnavailable)
    }
}

fn unique_transaction_id() -> Result<u64, ApiCommandError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiCommandError::BackupUnavailable)?;
    u64::try_from(duration.as_millis()).map_err(|_| ApiCommandError::BackupUnavailable)
}

impl From<BackupError> for ApiCommandError {
    fn from(error: BackupError) -> Self {
        match error {
            BackupError::InvalidInput | BackupError::InvalidArchive => Self::BackupInvalid,
            BackupError::UnsupportedVersion => Self::BackupUnsupported,
            BackupError::IntegrityMismatch => Self::BackupIntegrity,
            BackupError::CapacityExceeded => Self::BackupCapacityExceeded,
            BackupError::Io => Self::BackupUnavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pixnya-local-backup-{name}-{}-{}",
            std::process::id(),
            unique_transaction_id().unwrap()
        ))
    }

    fn frontend(search: &str) -> FrontendBackupState {
        FrontendBackupState {
            search_history: vec![search.to_owned()],
            novel_reading_progress: Default::default(),
            sidebar_expanded: true,
            reduced_motion: false,
            r18_default_visible: false,
        }
    }

    #[test]
    fn frontend_recovery_is_atomic_replaceable_and_acknowledgeable() {
        let root = test_root("frontend-recovery");
        let first = FrontendRecovery {
            transaction_id: 1,
            frontend: frontend("first"),
        };
        write_frontend_recovery(&root, &first).unwrap();
        assert_eq!(
            read_frontend_recovery(&root).unwrap().unwrap().frontend,
            first.frontend
        );

        let second = FrontendRecovery {
            transaction_id: 2,
            frontend: frontend("second"),
        };
        write_frontend_recovery(&root, &second).unwrap();
        assert_eq!(
            read_frontend_recovery(&root)
                .unwrap()
                .unwrap()
                .transaction_id,
            2
        );
        assert!(!root.join(FRONTEND_RECOVERY_STAGING_FILE).exists());

        remove_frontend_recovery(&root).unwrap();
        assert!(read_frontend_recovery(&root).unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_frontend_recovery_fails_closed() {
        let root = test_root("corrupt-frontend-recovery");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(FRONTEND_RECOVERY_FILE), b"not-json").unwrap();
        assert!(matches!(
            read_frontend_recovery(&root),
            Err(ApiCommandError::BackupInvalid)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
