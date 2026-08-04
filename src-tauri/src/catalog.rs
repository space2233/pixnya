use crate::{offline_library, ApiCommandError, AuthenticatedDataState};
use pixiv_client_local_catalog::{
    CatalogClearStats, CatalogCollection, CatalogSnapshot, EntryOrganization, LocalCatalog,
};
use tauri::Manager;

#[derive(Default)]
pub(crate) struct CatalogState {
    operation: tokio::sync::Mutex<()>,
}

pub(crate) fn open_catalog(app: &tauri::AppHandle) -> Result<LocalCatalog, ApiCommandError> {
    let path = crate::paths::app_data_dir(app)
        .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
        .join("local-catalog-v1.sqlite3");
    LocalCatalog::open(path).map_err(ApiCommandError::from)
}

#[tauri::command]
pub(crate) async fn get_local_catalog_snapshot(
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<CatalogSnapshot, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(&app)?;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let entries = library.list_entries()?;
        let keys = entries
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        catalog.snapshot(&keys).map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn create_local_collection(
    name: String,
    app: tauri::AppHandle,
) -> Result<CatalogCollection, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        catalog
            .create_collection(&name)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn rename_local_collection(
    collection_id: i64,
    name: String,
    app: tauri::AppHandle,
) -> Result<CatalogCollection, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        catalog
            .rename_collection(collection_id, &name)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn delete_local_collection(
    collection_id: i64,
    app: tauri::AppHandle,
) -> Result<(), ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        catalog
            .delete_collection(collection_id)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn organize_offline_entry(
    entry_key: String,
    collection_id: Option<i64>,
    tags: Vec<String>,
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<EntryOrganization, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(&app)?;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let exists = library
            .list_entries()?
            .into_iter()
            .any(|entry| entry.key == entry_key);
        if !exists {
            return Err(ApiCommandError::OfflineNotFound);
        }
        catalog
            .organize_entry(&entry_key, collection_id, &tags)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

pub(crate) async fn clear_local_catalog(
    app: &tauri::AppHandle,
) -> Result<CatalogClearStats, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let catalog = open_catalog(app)?;
    tauri::async_runtime::spawn_blocking(move || catalog.clear().map_err(ApiCommandError::from))
        .await
        .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}
