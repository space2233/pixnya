use crate::{
    perform_artwork_download, perform_novel_download, perform_ugoira_download,
    record_diagnostic_event, ApiCommandError, AuthenticatedDataState, SessionState,
};
use pixiv_client_diagnostic_log::{DiagnosticEntry, LogComponent, LogEvent, LogFailure, LogLevel};
use pixiv_client_download_queue::{
    DownloadFailure, DownloadKind, DownloadQueue, DownloadQueueStats, DownloadState, DownloadTask,
    NewDownloadTask, QueueError,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tauri::{Emitter, Manager};

const QUEUE_CHANGED_EVENT: &str = "pixiv-download-queue-changed";

#[derive(Clone)]
pub(crate) struct DownloadWorkerState {
    notify: Arc<tokio::sync::Notify>,
    generation: Arc<AtomicU64>,
}

impl Default for DownloadWorkerState {
    fn default() -> Self {
        Self {
            notify: Arc::new(tokio::sync::Notify::new()),
            generation: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl DownloadWorkerState {
    pub(crate) fn wake(&self) {
        self.notify.notify_one();
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.wake();
    }
}

#[derive(Clone)]
pub(crate) struct DownloadProgress {
    app: tauri::AppHandle,
    queue: DownloadQueue,
    worker: DownloadWorkerState,
    generation: u64,
    task_id: i64,
}

impl DownloadProgress {
    fn new(
        app: tauri::AppHandle,
        queue: DownloadQueue,
        worker: DownloadWorkerState,
        generation: u64,
        task_id: i64,
    ) -> Self {
        Self {
            app,
            queue,
            worker,
            generation,
            task_id,
        }
    }

    pub(crate) fn checkpoint(&self) -> Result<(), ApiCommandError> {
        if self.worker.generation() != self.generation {
            return Err(ApiCommandError::DownloadInterrupted);
        }
        let task = self.queue.get(self.task_id)?;
        if task.state != DownloadState::Running {
            return Err(ApiCommandError::DownloadInterrupted);
        }
        Ok(())
    }

    pub(crate) fn update_metadata(
        &self,
        title: Option<String>,
        author: Option<String>,
    ) -> Result<(), ApiCommandError> {
        self.checkpoint()?;
        let task = self.queue.update_metadata(self.task_id, title, author)?;
        emit_queue_changed(&self.app, Some(task));
        Ok(())
    }

    pub(crate) fn update(
        &self,
        completed_items: u32,
        total_items: u32,
        downloaded_bytes: u64,
    ) -> Result<(), ApiCommandError> {
        self.checkpoint()?;
        let task = self.queue.update_progress(
            self.task_id,
            completed_items,
            total_items,
            downloaded_bytes,
        )?;
        emit_queue_changed(&self.app, Some(task));
        Ok(())
    }
}

pub(crate) fn start_download_worker(app: tauri::AppHandle) {
    let worker = app.state::<DownloadWorkerState>().inner().clone();
    tauri::async_runtime::spawn(async move {
        run_download_worker(app, worker).await;
    });
}

pub(crate) fn wake_download_worker(app: &tauri::AppHandle) {
    app.state::<DownloadWorkerState>().wake();
}

pub(crate) async fn suspend_download_worker(app: &tauri::AppHandle) {
    let worker = app.state::<DownloadWorkerState>().inner().clone();
    worker.invalidate();
    if let Ok(queue) = download_queue(app) {
        if let Ok(Ok(recovered)) =
            tauri::async_runtime::spawn_blocking(move || queue.recover_interrupted()).await
        {
            if recovered > 0 {
                emit_queue_changed(app, None);
                record_download_event(
                    app,
                    LogLevel::Warning,
                    LogEvent::DownloadRecovered,
                    None,
                    recovered,
                );
            }
        }
    }
}

pub(crate) async fn clear_download_queue(
    app: &tauri::AppHandle,
) -> Result<DownloadQueueStats, ApiCommandError> {
    let worker = app.state::<DownloadWorkerState>().inner().clone();
    worker.invalidate();
    let queue = download_queue(app)?;
    let result = tauri::async_runtime::spawn_blocking(move || queue.clear())
        .await
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)??;
    emit_queue_changed(app, None);
    record_download_event(
        app,
        LogLevel::Info,
        LogEvent::DownloadQueueCleared,
        None,
        result.task_count,
    );
    Ok(result)
}

pub(crate) async fn run_download_worker(app: tauri::AppHandle, worker: DownloadWorkerState) {
    let queue = match download_queue(&app) {
        Ok(queue) => queue,
        Err(_) => return,
    };
    let recovery_queue = queue.clone();
    if let Ok(Ok(recovered)) =
        tauri::async_runtime::spawn_blocking(move || recovery_queue.recover_interrupted()).await
    {
        if recovered > 0 {
            emit_queue_changed(&app, None);
            record_download_event(
                &app,
                LogLevel::Warning,
                LogEvent::DownloadRecovered,
                None,
                recovered,
            );
        }
    }

    loop {
        let logged_in = app
            .state::<SessionState>()
            .snapshot()
            .is_ok_and(|snapshot| snapshot.logged_in);
        if !logged_in {
            worker.notify.notified().await;
            continue;
        }

        let claim_queue = queue.clone();
        let claimed =
            match tauri::async_runtime::spawn_blocking(move || claim_queue.claim_next()).await {
                Ok(Ok(task)) => task,
                _ => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
        let Some(task) = claimed else {
            worker.notify.notified().await;
            continue;
        };
        emit_queue_changed(&app, Some(task.clone()));
        record_download_event(&app, LogLevel::Info, LogEvent::DownloadStarted, None, 1);
        let generation = worker.generation();
        let progress = DownloadProgress::new(
            app.clone(),
            queue.clone(),
            worker.clone(),
            generation,
            task.id,
        );
        let session = app.state::<SessionState>();
        let data = app.state::<AuthenticatedDataState>().inner().clone();
        let result = match task.kind {
            DownloadKind::Artwork => perform_artwork_download(
                task.resource_id.clone(),
                &app,
                &session,
                data,
                Some(progress.clone()),
            )
            .await
            .map(|entry| (entry.key, entry.asset_count, entry.size_bytes)),
            DownloadKind::Novel => perform_novel_download(
                task.resource_id.clone(),
                &app,
                &session,
                data,
                Some(progress.clone()),
            )
            .await
            .map(|entry| (entry.key, entry.asset_count, entry.size_bytes)),
            DownloadKind::Ugoira => perform_ugoira_download(
                task.resource_id.clone(),
                &app,
                &session,
                data,
                Some(progress.clone()),
                None,
            )
            .await
            .map(|prepared| {
                (
                    prepared.entry.key,
                    prepared.entry.asset_count,
                    prepared.entry.size_bytes,
                )
            }),
        };

        let result = match result {
            Ok((entry_key, asset_count, size_bytes)) => {
                crate::exports::auto_export_offline_entry(&app, &entry_key)
                    .await
                    .map(|_| (asset_count, size_bytes))
            }
            Err(error) => Err(error),
        };

        if worker.generation() != generation {
            continue;
        }
        let final_task = match result {
            Ok((asset_count, size_bytes)) => {
                let completed = queue.mark_completed(task.id, asset_count, size_bytes).ok();
                if completed.is_some() {
                    record_download_event(
                        &app,
                        LogLevel::Info,
                        LogEvent::DownloadCompleted,
                        None,
                        asset_count,
                    );
                }
                completed
            }
            Err(error) => {
                let failure = failure_for(&error);
                let failed = match queue.get(task.id) {
                    Ok(current) if current.state == DownloadState::Paused => Some(current),
                    Ok(current) if current.state == DownloadState::Running => {
                        queue.mark_failed(task.id, failure).ok()
                    }
                    _ => None,
                };
                if failed
                    .as_ref()
                    .is_some_and(|task| task.state == DownloadState::Failed)
                {
                    record_download_event(
                        &app,
                        LogLevel::Error,
                        LogEvent::DownloadFailed,
                        Some(log_failure_for(failure)),
                        1,
                    );
                }
                failed
            }
        };
        if let Some(final_task) = final_task {
            emit_queue_changed(&app, Some(final_task));
        }
    }
}

#[tauri::command]
pub(crate) async fn enqueue_download(
    kind: DownloadKind,
    resource_id: String,
    title: Option<String>,
    author: Option<String>,
    app: tauri::AppHandle,
    worker: tauri::State<'_, DownloadWorkerState>,
) -> Result<DownloadTask, ApiCommandError> {
    let queue = download_queue(&app)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        queue.enqueue(NewDownloadTask {
            kind,
            resource_id,
            title,
            author,
        })
    })
    .await
    .map_err(|_| ApiCommandError::DownloadQueueUnavailable)??;
    emit_queue_changed(&app, Some(task.clone()));
    record_download_event(&app, LogLevel::Info, LogEvent::DownloadQueued, None, 1);
    worker.wake();
    Ok(task)
}

#[tauri::command]
pub(crate) async fn list_download_tasks(
    app: tauri::AppHandle,
) -> Result<Vec<DownloadTask>, ApiCommandError> {
    let queue = download_queue(&app)?;
    tauri::async_runtime::spawn_blocking(move || queue.list())
        .await
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)?
        .map_err(ApiCommandError::from)
}

#[tauri::command]
pub(crate) async fn get_download_queue_stats(
    app: tauri::AppHandle,
) -> Result<DownloadQueueStats, ApiCommandError> {
    let queue = download_queue(&app)?;
    tauri::async_runtime::spawn_blocking(move || queue.stats())
        .await
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)?
        .map_err(ApiCommandError::from)
}

#[tauri::command]
pub(crate) async fn pause_download_task(
    task_id: i64,
    app: tauri::AppHandle,
    worker: tauri::State<'_, DownloadWorkerState>,
) -> Result<DownloadTask, ApiCommandError> {
    let queue = download_queue(&app)?;
    let task = tauri::async_runtime::spawn_blocking(move || queue.pause(task_id))
        .await
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)??;
    emit_queue_changed(&app, Some(task.clone()));
    record_download_event(&app, LogLevel::Info, LogEvent::DownloadPaused, None, 1);
    worker.wake();
    Ok(task)
}

#[tauri::command]
pub(crate) async fn resume_download_task(
    task_id: i64,
    app: tauri::AppHandle,
    worker: tauri::State<'_, DownloadWorkerState>,
) -> Result<DownloadTask, ApiCommandError> {
    let queue = download_queue(&app)?;
    let task = tauri::async_runtime::spawn_blocking(move || queue.resume(task_id))
        .await
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)??;
    emit_queue_changed(&app, Some(task.clone()));
    record_download_event(&app, LogLevel::Info, LogEvent::DownloadResumed, None, 1);
    worker.wake();
    Ok(task)
}

#[tauri::command]
pub(crate) async fn remove_download_task(
    task_id: i64,
    app: tauri::AppHandle,
) -> Result<bool, ApiCommandError> {
    let queue = download_queue(&app)?;
    let removed = tauri::async_runtime::spawn_blocking(move || queue.remove(task_id))
        .await
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)??;
    if removed {
        emit_queue_changed(&app, None);
        record_download_event(&app, LogLevel::Info, LogEvent::DownloadRemoved, None, 1);
    }
    Ok(removed)
}

pub(crate) fn download_queue(app: &tauri::AppHandle) -> Result<DownloadQueue, ApiCommandError> {
    let path = crate::paths::app_data_dir(app)
        .map_err(|_| ApiCommandError::DownloadQueueUnavailable)?
        .join("download-queue-v1.sqlite3");
    DownloadQueue::open(path).map_err(ApiCommandError::from)
}

fn failure_for(error: &ApiCommandError) -> DownloadFailure {
    match error {
        ApiCommandError::AuthenticationRequired
        | ApiCommandError::OAuthConfigurationUnavailable
        | ApiCommandError::SecureStorageUnavailable
        | ApiCommandError::TokenRefreshFailed => DownloadFailure::Authentication,
        ApiCommandError::TransportUnavailable
        | ApiCommandError::RequestFailed
        | ApiCommandError::UpstreamRejected { .. } => DownloadFailure::Network,
        ApiCommandError::InvalidResponse
        | ApiCommandError::InvalidInput
        | ApiCommandError::InvalidIdentifier
        | ApiCommandError::InvalidMediaUrl
        | ApiCommandError::InvalidCursor
        | ApiCommandError::MediaTooLarge => DownloadFailure::InvalidResponse,
        ApiCommandError::DownloadInterrupted => DownloadFailure::Interrupted,
        ApiCommandError::StorageUnavailable
        | ApiCommandError::StorageCapacityExceeded { .. }
        | ApiCommandError::ExportUnavailable
        | ApiCommandError::ExportDestinationUnavailable => DownloadFailure::Storage,
        _ => DownloadFailure::Storage,
    }
}

fn log_failure_for(failure: DownloadFailure) -> LogFailure {
    match failure {
        DownloadFailure::Authentication => LogFailure::AuthenticationRequired,
        DownloadFailure::Network => LogFailure::NetworkUnavailable,
        DownloadFailure::InvalidResponse => LogFailure::InvalidResponse,
        DownloadFailure::Storage => LogFailure::StorageUnavailable,
        DownloadFailure::Interrupted => LogFailure::StateUnavailable,
    }
}

fn record_download_event(
    app: &tauri::AppHandle,
    level: LogLevel,
    event: LogEvent,
    failure: Option<LogFailure>,
    item_count: u32,
) {
    let mut entry =
        DiagnosticEntry::now(level, LogComponent::Download, event).with_item_count(item_count);
    if let Some(failure) = failure {
        entry = entry.with_failure(failure);
    }
    record_diagnostic_event(app, entry);
}

fn emit_queue_changed(app: &tauri::AppHandle, task: Option<DownloadTask>) {
    let _ = app.emit(QUEUE_CHANGED_EVENT, task);
}

impl From<QueueError> for ApiCommandError {
    fn from(error: QueueError) -> Self {
        match error {
            QueueError::InvalidInput => Self::InvalidInput,
            QueueError::TaskNotFound => Self::DownloadTaskNotFound,
            QueueError::InvalidTransition => Self::DownloadTransitionInvalid,
            QueueError::InvalidDatabase | QueueError::Io | QueueError::Database => {
                Self::DownloadQueueUnavailable
            }
        }
    }
}
