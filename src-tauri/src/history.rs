use crate::ApiCommandError;
use pixiv_client_local_history::{
    HistoryClearStats, HistoryKind, HistoryRecord, HistorySnapshot, LocalHistory,
};
use tauri::Manager;

#[derive(Default)]
pub(crate) struct HistoryState {
    pub(crate) operation: tokio::sync::Mutex<()>,
}

pub(crate) fn open_history(app: &tauri::AppHandle) -> Result<LocalHistory, ApiCommandError> {
    let path = crate::paths::app_data_dir(app)
        .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
        .join("browsing-history-v1.sqlite3");
    LocalHistory::open(path).map_err(ApiCommandError::from)
}

#[tauri::command]
pub(crate) async fn get_browsing_history(
    app: tauri::AppHandle,
    state: tauri::State<'_, HistoryState>,
) -> Result<HistorySnapshot, ApiCommandError> {
    let _operation = state.operation.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        open_history(&app)?
            .snapshot()
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
}

#[tauri::command]
pub(crate) async fn set_browsing_history_enabled(
    enabled: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, HistoryState>,
) -> Result<HistorySnapshot, ApiCommandError> {
    let _operation = state.operation.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        open_history(&app)?
            .set_enabled(enabled)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
}

#[tauri::command]
pub(crate) async fn record_browsing_history(
    record: HistoryRecord,
    app: tauri::AppHandle,
    state: tauri::State<'_, HistoryState>,
) -> Result<bool, ApiCommandError> {
    let _operation = state.operation.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        open_history(&app)?
            .record(record)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
}

#[tauri::command]
pub(crate) async fn remove_browsing_history_entry(
    kind: HistoryKind,
    resource_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, HistoryState>,
) -> Result<bool, ApiCommandError> {
    let _operation = state.operation.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        open_history(&app)?
            .remove(kind, &resource_id)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
}

#[tauri::command]
pub(crate) async fn clear_browsing_history(
    app: tauri::AppHandle,
    state: tauri::State<'_, HistoryState>,
) -> Result<HistoryClearStats, ApiCommandError> {
    let _operation = state.operation.lock().await;
    tauri::async_runtime::spawn_blocking(move || {
        open_history(&app)?.clear().map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
}

pub(crate) async fn clear_all_history(
    app: &tauri::AppHandle,
) -> Result<HistoryClearStats, ApiCommandError> {
    let state = app.state::<HistoryState>();
    let _operation = state.operation.lock().await;
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        open_history(&app)?.clear().map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::BrowsingHistoryUnavailable)?
}
