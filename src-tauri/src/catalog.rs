use crate::{offline_library, ApiCommandError, AuthenticatedDataState};
use pixiv_client_local_catalog::{
    BatchOrganizationChange, CatalogClearStats, CatalogCollection, CatalogFilterDraft,
    CatalogSnapshot, EntryOrganization, LocalCatalog, SavedCatalogFilter,
};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::Manager;

#[derive(Default)]
pub(crate) struct CatalogState {
    pub(crate) operation: tokio::sync::Mutex<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicateGroup {
    reason: DuplicateReason,
    signature: String,
    entry_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DuplicateReason {
    ResourceId,
    FileHash,
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

#[tauri::command]
pub(crate) async fn batch_organize_offline_entries(
    change: BatchOrganizationChange,
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<Vec<EntryOrganization>, ApiCommandError> {
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
        let available = library
            .list_entries()?
            .into_iter()
            .map(|entry| entry.key)
            .collect::<std::collections::HashSet<_>>();
        if change.entry_keys.iter().any(|key| !available.contains(key)) {
            return Err(ApiCommandError::OfflineNotFound);
        }
        catalog
            .batch_organize_entries(&change)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn save_local_catalog_filter(
    filter: CatalogFilterDraft,
    app: tauri::AppHandle,
) -> Result<SavedCatalogFilter, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        catalog.save_filter(&filter).map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn delete_local_catalog_filter(
    filter_id: i64,
    app: tauri::AppHandle,
) -> Result<bool, ApiCommandError> {
    let state = app.state::<CatalogState>();
    let _operation = state.operation.lock().await;
    let catalog = open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        catalog
            .delete_filter(filter_id)
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn find_offline_duplicates(
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<Vec<DuplicateGroup>, ApiCommandError> {
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let fingerprints = library.entry_fingerprints()?;
        let mut resource_ids = BTreeMap::<String, Vec<String>>::new();
        let mut file_hashes = BTreeMap::<(String, String), Vec<String>>::new();
        for fingerprint in fingerprints {
            resource_ids
                .entry(fingerprint.resource_id.clone())
                .or_default()
                .push(fingerprint.entry_key.clone());
            for asset in fingerprint.assets {
                file_hashes
                    .entry((asset.content_type, asset.sha256))
                    .or_default()
                    .push(fingerprint.entry_key.clone());
            }
        }
        let mut groups = Vec::new();
        for (signature, mut entry_keys) in resource_ids {
            entry_keys.sort();
            entry_keys.dedup();
            if entry_keys.len() > 1 {
                groups.push(DuplicateGroup {
                    reason: DuplicateReason::ResourceId,
                    signature,
                    entry_keys,
                });
            }
        }
        for ((content_type, hash), mut entry_keys) in file_hashes {
            entry_keys.sort();
            entry_keys.dedup();
            if entry_keys.len() > 1 {
                groups.push(DuplicateGroup {
                    reason: DuplicateReason::FileHash,
                    signature: format!("{content_type}:{hash}"),
                    entry_keys,
                });
            }
        }
        Ok(groups)
    })
    .await
    .map_err(|_| ApiCommandError::LocalCatalogUnavailable)?
}

#[tauri::command]
pub(crate) async fn batch_remove_offline_entries(
    entry_keys: Vec<String>,
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<Vec<String>, ApiCommandError> {
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
        let available = library.list_entries()?;
        let available_keys = available
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<std::collections::HashSet<_>>();
        if entry_keys.iter().any(|key| !available_keys.contains(key)) {
            return Err(ApiCommandError::OfflineNotFound);
        }
        let previous = catalog.snapshot(&entry_keys)?.entries;
        catalog.remove_entries(&entry_keys)?;
        match library.remove_entries(&entry_keys) {
            Ok(removed) => Ok(removed),
            Err(error) => {
                if catalog.restore_entries(&previous).is_err() {
                    return Err(ApiCommandError::LocalCatalogUnavailable);
                }
                Err(ApiCommandError::from(error))
            }
        }
    })
    .await
    .map_err(|_| ApiCommandError::OfflineUnavailable)?
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
